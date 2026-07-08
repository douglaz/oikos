use super::*;

impl Settlement {
    pub(super) fn run_in_kind_wage_phase(&mut self) {
        if !self.in_kind_wage_active() {
            return;
        }
        self.expire_in_kind_contracts();
        let Some(bread) = self.provenance_bread_good() else {
            return;
        };
        let workers = self.in_kind_worker_pool(bread);
        if workers.is_empty() {
            return;
        }
        let mut owners = self.in_kind_owner_candidates(bread);
        if owners.is_empty() {
            self.in_kind_worker_unmatched = self
                .in_kind_worker_unmatched
                .saturating_add(workers.len() as u64);
            return;
        }
        self.clear_in_kind_wage_market(bread, workers, &mut owners);
    }

    pub(super) fn expire_in_kind_contracts(&mut self) {
        if self.in_kind_contracts.is_empty() {
            return;
        }
        let contracts = std::mem::take(&mut self.in_kind_contracts);
        for contract in contracts {
            let employer_live = self.private_land_live_agent(contract.employer);
            let worker_live = self.private_land_live_agent(contract.worker);
            let due = contract
                .opened_tick
                .saturating_add(u64::from(contract.term))
                <= self.econ_tick;
            if !employer_live || !worker_live || due {
                if employer_live && worker_live {
                    self.settle_in_kind_contract_grain(&contract);
                }
                self.clear_in_kind_reservation(&contract);
            } else {
                self.in_kind_contracts.push(contract);
            }
        }
    }

    pub(super) fn in_kind_worker_pool(&self, bread: GoodId) -> Vec<AgentId> {
        let mut pool = Vec::new();
        for &slot in &self.live_colonist_slots {
            let worker = self.colonists[slot].id;
            if !self.share_worker_base_eligible(worker, ShareTenancyMode::Voluntary)
                || self.forecast_commons_sufficiency(worker, bread)
            {
                continue;
            }
            pool.push(worker);
        }
        pool.sort();
        pool
    }

    pub(super) fn in_kind_owner_candidates(&mut self, _bread: GoodId) -> Vec<ShareOwnerCandidate> {
        let candidates = self.in_kind_owner_candidate_plots();
        self.in_kind_owner_candidates_total = self
            .in_kind_owner_candidates_total
            .saturating_add(candidates.len() as u64);
        let mut owners_with_candidate = BTreeSet::new();
        for candidate in &candidates {
            owners_with_candidate.insert(candidate.owner);
        }
        for owner in self.share_owner_ids() {
            if !owners_with_candidate.contains(&owner) {
                self.in_kind_owner_no_atcap_plot =
                    self.in_kind_owner_no_atcap_plot.saturating_add(1);
            }
        }
        candidates
    }

    pub(super) fn clear_in_kind_wage_market(
        &mut self,
        bread: GoodId,
        workers: Vec<AgentId>,
        owners: &mut Vec<ShareOwnerCandidate>,
    ) {
        let term = self.share_tenancy_terms().map_or(1, |(_, term)| term);
        for worker in workers {
            if owners.is_empty() {
                self.in_kind_worker_unmatched = self.in_kind_worker_unmatched.saturating_add(1);
                continue;
            }
            let wage = self.in_kind_wage_floor(worker, bread, term);
            if wage == 0 || wage > u64::from(u32::MAX) {
                self.in_kind_worker_declined = self.in_kind_worker_declined.saturating_add(1);
                continue;
            }
            let wage = wage as u32;
            if !self.share_worker_accepts_bread_now(worker, bread, wage) {
                self.in_kind_worker_declined = self.in_kind_worker_declined.saturating_add(1);
                continue;
            }

            let mut accepted = None;
            for (position, candidate) in owners.iter().enumerate() {
                let expected = self
                    .share_expected_term_output(candidate.node, term)
                    .unwrap_or(0);
                if expected <= u64::from(wage) {
                    self.in_kind_productivity_declined =
                        self.in_kind_productivity_declined.saturating_add(1);
                    continue;
                }
                if self.owner_free_self_produced_bread(candidate.owner) < u64::from(wage) {
                    self.in_kind_owner_insufficient_fund =
                        self.in_kind_owner_insufficient_fund.saturating_add(1);
                    continue;
                }
                accepted = Some((position, expected));
                break;
            }

            let Some((position, expected)) = accepted else {
                self.in_kind_worker_unmatched = self.in_kind_worker_unmatched.saturating_add(1);
                continue;
            };
            let candidate = owners.remove(position);
            self.open_in_kind_contract(worker, candidate, wage, term, expected);
        }
    }

    pub(super) fn open_in_kind_contract(
        &mut self,
        worker: AgentId,
        candidate: ShareOwnerCandidate,
        wage_bread: u32,
        term: u16,
        expected_output: u64,
    ) {
        if wage_bread == 0 {
            return;
        }
        let Some(bread) = self.provenance_bread_good() else {
            return;
        };
        if self.owner_free_self_produced_bread(candidate.owner) < u64::from(wage_bread) {
            return;
        }
        if let Some(record) = self.land_plots.get_mut(&candidate.node) {
            if record.reserved_for.is_some() {
                return;
            }
            record.reserved_for = Some(worker);
        } else {
            return;
        }

        self.society
            .cancel_live_stock_offers_for_agent(candidate.owner, bread);
        if self
            .society
            .free_stock_after_all_reserves(candidate.owner, bread)
            < wage_bread
            || !self.society.debit_stock(candidate.owner, bread, wage_bread)
        {
            self.clear_in_kind_reservation_parts(candidate.node, worker);
            return;
        }
        if !self.society.credit_stock(worker, bread, wage_bread) {
            let _ = self
                .society
                .credit_stock(candidate.owner, bread, wage_bread);
            self.clear_in_kind_reservation_parts(candidate.node, worker);
            return;
        }

        if self.bread_provenance_active() {
            let moved = self.bread_provenance.transfer_self_produced(
                candidate.owner,
                worker,
                u64::from(wage_bread),
            );
            if moved != u64::from(wage_bread) {
                self.in_kind_endowment_funded_hires =
                    self.in_kind_endowment_funded_hires.saturating_add(1);
            }
        }
        if self.acquisition_ledger_active() {
            let moved = self.acquisition.transfer_self_produced(
                candidate.owner,
                worker,
                u64::from(wage_bread),
            );
            debug_assert_eq!(
                moved,
                u64::from(wage_bread),
                "C1N advance must move the employer's self-produced acquisition channel"
            );
        }

        let id = self.next_in_kind_contract_id;
        self.next_in_kind_contract_id = self.next_in_kind_contract_id.wrapping_add(1);
        self.in_kind_contracts.push(InKindWageContract {
            id,
            employer: candidate.owner,
            worker,
            node: candidate.node,
            wage_bread,
            term,
            opened_tick: self.econ_tick,
            grain_in_stock: 0,
            split_remainder_bps: 0,
        });
        self.in_kind_hires_total = self.in_kind_hires_total.saturating_add(1);
        self.in_kind_worker_advance_bread = self
            .in_kind_worker_advance_bread
            .saturating_add(u64::from(wage_bread));
        self.in_kind_expected_output_total = self
            .in_kind_expected_output_total
            .saturating_add(expected_output);
        self.in_kind_workers_ever.insert(worker);
        self.in_kind_employers_ever.insert(candidate.owner);
    }

    fn in_kind_wage_floor(&self, worker: AgentId, bread: GoodId, term: u16) -> u64 {
        self.forecast_term_need_unmet(worker, bread, term)
            .max(self.forecast_advance_only_term_need(worker, bread, term))
    }

    /// C1N's product goes 100% to the employer, so the worker must be able to live on the
    /// advance itself. The reused P1.5 forecast can count future commons draws; this floor keeps
    /// an accepted contract from depending on commons arriving after the starvation readback.
    fn forecast_advance_only_term_need(&self, worker: AgentId, bread: GoodId, term: u16) -> u64 {
        if term == 0 || self.dynamics.hunger_per_food == 0 {
            return 0;
        }
        let threshold = self
            .chain
            .as_ref()
            .map_or(0, |chain| chain.emergency_hunger_threshold);
        if threshold == 0 {
            return 0;
        }
        let Some(slot) = self.slot_for_id(worker) else {
            return 0;
        };
        let mut hunger = self.colonists[slot].need.hunger;
        let mut held_free = u64::from(self.society.free_stock_after_all_reserves(worker, bread));
        let mut advance_need = 0u64;
        let target_hunger = threshold.saturating_sub(1);
        for _ in 0..term {
            let needed = u64::from(food_needed_to_reach_hunger(
                hunger,
                self.dynamics.hunger_deplete,
                self.dynamics.hunger_per_food,
                target_hunger,
            ));
            let held_eat = held_free.min(needed);
            held_free = held_free.saturating_sub(held_eat);
            let top_up = needed.saturating_sub(held_eat);
            advance_need = advance_need.saturating_add(top_up);
            hunger = advance_hunger_after_food(
                hunger,
                self.dynamics.hunger_deplete,
                self.dynamics.hunger_per_food,
                self.dynamics.need_max,
                held_eat.saturating_add(top_up),
            );
        }
        advance_need
    }

    pub(super) fn clear_in_kind_reservation(&mut self, contract: &InKindWageContract) {
        self.clear_in_kind_reservation_parts(contract.node, contract.worker);
    }

    fn clear_in_kind_reservation_parts(&mut self, node: NodeId, worker: AgentId) {
        if let Some(record) = self.land_plots.get_mut(&node) {
            if record.reserved_for == Some(worker) {
                record.reserved_for = None;
            }
        }
    }

    pub(super) fn settle_in_kind_contract_grain(&mut self, contract: &InKindWageContract) {
        let Some(grain) = self.cultivation_input_good() else {
            return;
        };
        self.settle_in_kind_contract_in_flight_grain(contract, grain);
        if contract.grain_in_stock == 0 {
            return;
        }
        self.society
            .cancel_live_stock_offers_for_agent(contract.worker, grain);
        let held = self
            .society
            .agents
            .get(contract.worker)
            .map_or(0, |agent| agent.stock.get(grain));
        let unsettled = held.min(contract.grain_in_stock);
        if unsettled == 0 {
            return;
        }
        if self.society.debit_stock(contract.worker, grain, unsettled) {
            if self
                .society
                .credit_stock(contract.employer, grain, unsettled)
            {
                self.in_kind_employer_grain_settled = self
                    .in_kind_employer_grain_settled
                    .saturating_add(u64::from(unsettled));
            } else {
                let _ = self.society.credit_stock(contract.worker, grain, unsettled);
            }
        }
    }

    fn settle_in_kind_contract_in_flight_grain(
        &mut self,
        contract: &InKindWageContract,
        grain: GoodId,
    ) {
        let Some(slot) = self.colonist_slot_by_id.get(&contract.worker).copied() else {
            return;
        };
        if self.colonists[slot].carried_grain_source != Some(contract.node)
            || self.colonists[slot].carried_in_kind_contract_id != Some(contract.id)
        {
            return;
        }

        let carried = self.world.agent_carry(contract.worker, grain);
        if carried > 0 && self.society.credit_stock(contract.employer, grain, carried) {
            let drained = self
                .world
                .withdraw_agent_carry(contract.worker, grain, carried);
            debug_assert_eq!(
                drained, carried,
                "tagged in-kind carry must be available to drain"
            );
            self.in_kind_employer_grain_settled = self
                .in_kind_employer_grain_settled
                .saturating_add(u64::from(drained));
        }

        let pending_key = (contract.worker, grain);
        let pending = self
            .pending_deposits
            .get(&pending_key)
            .copied()
            .unwrap_or(0);
        if pending > 0 && self.society.credit_stock(contract.employer, grain, pending) {
            let drained = self.world.stockpile_withdraw(self.exchange, grain, pending);
            debug_assert_eq!(
                drained, pending,
                "tagged in-kind pending grain must be available in the exchange"
            );
            if drained == pending {
                self.pending_deposits.remove(&pending_key);
            } else if drained > 0 {
                self.pending_deposits
                    .insert(pending_key, pending.saturating_sub(drained));
            }
            self.in_kind_employer_grain_settled = self
                .in_kind_employer_grain_settled
                .saturating_add(u64::from(drained));
        }

        if self.world.agent_carry(contract.worker, grain) == 0
            && self
                .pending_deposits
                .get(&pending_key)
                .copied()
                .unwrap_or(0)
                == 0
        {
            self.colonists[slot].carried_grain_source = None;
            self.colonists[slot].carried_in_kind_contract_id = None;
        }
    }

    pub(super) fn settle_in_kind_wage_for_starvation_death(&mut self, dead: AgentId) {
        self.check_in_kind_term_survival_before_death(dead);
        self.settle_in_kind_wage_for_death(dead);
    }

    pub(super) fn settle_in_kind_wage_for_death(&mut self, dead: AgentId) {
        if self.in_kind_contracts.is_empty() {
            return;
        }
        let contracts = std::mem::take(&mut self.in_kind_contracts);
        for contract in contracts {
            if contract.employer == dead || contract.worker == dead {
                self.settle_in_kind_contract_grain(&contract);
                self.clear_in_kind_reservation(&contract);
            } else {
                self.in_kind_contracts.push(contract);
            }
        }
    }

    fn check_in_kind_term_survival_before_death(&mut self, dead: AgentId) {
        let Some(contract) = self.in_kind_contract_for_worker(dead) else {
            return;
        };
        if self.econ_tick
            >= contract
                .opened_tick
                .saturating_add(u64::from(contract.term))
        {
            return;
        }
        let q = self
            .share_expected_term_output(contract.node, contract.term)
            .unwrap_or(0);
        if q > u64::from(contract.wage_bread) {
            self.in_kind_term_starvations = self.in_kind_term_starvations.saturating_add(1);
        }
    }

    pub(super) fn in_kind_worker_has_contract(&self, worker: AgentId) -> bool {
        self.in_kind_contract_for_worker(worker).is_some()
    }

    pub(super) fn in_kind_contract_for_worker(
        &self,
        worker: AgentId,
    ) -> Option<InKindWageContract> {
        self.in_kind_contracts
            .iter()
            .copied()
            .find(|contract| contract.worker == worker)
    }

    pub(super) fn in_kind_contract_for_node(&self, node: NodeId) -> Option<InKindWageContract> {
        self.in_kind_contracts
            .iter()
            .copied()
            .find(|contract| contract.node == node)
    }

    pub(super) fn in_kind_plot_reserved_against_owner(&self, owner: AgentId, node: NodeId) -> bool {
        self.in_kind_contracts
            .iter()
            .any(|contract| contract.employer == owner && contract.node == node)
    }

    pub(super) fn in_kind_contract_task(&self, worker: AgentId, node: NodeId) -> Task {
        self.share_contract_task(worker, node)
    }

    pub(super) fn steer_in_kind_contract_workers(&mut self) {
        if self.in_kind_contracts.is_empty() {
            return;
        }
        let contracts = self.in_kind_contracts.clone();
        for contract in contracts {
            if !self.private_land_live_agent(contract.worker) {
                continue;
            }
            let desired = self.in_kind_contract_task(contract.worker, contract.node);
            if self.world.agent_task(contract.worker) != Some(desired) {
                self.world.assign_task(contract.worker, desired);
            }
        }
    }

    pub(super) fn in_kind_worker_admitted_to(
        &self,
        agent: AgentId,
        node: NodeId,
        record: &LandPlotRecord,
    ) -> bool {
        record.reserved_for == Some(agent)
            && self
                .in_kind_contract_for_worker(agent)
                .is_some_and(|contract| contract.node == node)
    }

    pub(super) fn credit_in_kind_contract_grain(&mut self, worker: AgentId, qty: u32) {
        if qty == 0 {
            return;
        }
        let source = self.colonist_slot_by_id.get(&worker).map(|&slot| {
            (
                self.colonists[slot].carried_grain_source,
                self.colonists[slot].carried_in_kind_contract_id,
            )
        });
        if let Some(contract) = self
            .in_kind_contracts
            .iter_mut()
            .find(|contract| contract.worker == worker)
        {
            if source == Some((Some(contract.node), Some(contract.id))) {
                contract.grain_in_stock = contract.grain_in_stock.saturating_add(qty);
            }
        }
    }

    pub(super) fn check_in_kind_stock_drawdown(&mut self, events: &[WorkedLandEvent]) {
        if self.in_kind_contracts.is_empty() || events.is_empty() {
            return;
        }
        for event in events {
            let Some(contract) = self.in_kind_contract_for_worker(event.agent) else {
                continue;
            };
            if contract.node != event.node {
                continue;
            }
            if self
                .world
                .node(event.node)
                .is_some_and(|plot| event.moved > plot.regen_per_tick || plot.stock < plot.cap)
            {
                self.in_kind_stock_drawdown = self.in_kind_stock_drawdown.saturating_add(1);
            }
        }
    }

    pub(super) fn split_in_kind_output(
        &mut self,
        worker: AgentId,
        output_qty: u64,
        input_qty: u32,
    ) {
        if output_qty == 0 || input_qty == 0 {
            return;
        }
        let Some(index) = self
            .in_kind_contracts
            .iter()
            .position(|contract| contract.worker == worker)
        else {
            return;
        };
        let contract = self.in_kind_contracts[index];
        let contract_input = input_qty.min(contract.grain_in_stock);
        if contract_input == 0 {
            return;
        }
        let Some(bread) = self.provenance_bread_good() else {
            return;
        };
        self.in_kind_contracts[index].grain_in_stock = self.in_kind_contracts[index]
            .grain_in_stock
            .saturating_sub(contract_input);
        let split_output = if contract_input == input_qty {
            output_qty
        } else {
            output_qty
                .saturating_mul(u64::from(contract_input))
                .checked_div(u64::from(input_qty))
                .unwrap_or(0)
        };
        if split_output == 0 || split_output > u64::from(u32::MAX) {
            return;
        }
        let owner_qty = split_output as u32;
        if self.society.debit_stock(worker, bread, owner_qty) {
            if self
                .society
                .credit_stock(contract.employer, bread, owner_qty)
            {
                if self.bread_provenance_active() {
                    let moved = self.bread_provenance.transfer_recent_self_produced(
                        worker,
                        contract.employer,
                        split_output,
                    );
                    debug_assert_eq!(
                        moved, split_output,
                        "C1N split must move the worker's newly produced contract crop"
                    );
                }
                if self.acquisition_ledger_active() {
                    let moved = self.acquisition.transfer_recent_self_produced(
                        worker,
                        contract.employer,
                        split_output,
                    );
                    debug_assert_eq!(
                        moved, split_output,
                        "C1N split must move the worker's freshly credited self-produced channel"
                    );
                }
                self.in_kind_employer_bread_income = self
                    .in_kind_employer_bread_income
                    .saturating_add(split_output);
            } else {
                let _ = self.society.credit_stock(worker, bread, owner_qty);
            }
        }
    }

    pub(super) fn owner_free_self_produced_bread(&self, owner: AgentId) -> u64 {
        let Some(bread) = self.provenance_bread_good() else {
            return 0;
        };
        let free = u64::from(self.society.free_stock_after_all_reserves(owner, bread));
        let self_produced = self
            .bread_provenance
            .produced_lots
            .get(&owner)
            .map(|queue| {
                queue
                    .iter()
                    .filter(|lot| lot.producer == owner)
                    .map(|lot| lot.qty)
                    .sum::<u64>()
            })
            .unwrap_or(0);
        free.min(self_produced)
    }

    pub(super) fn in_kind_wage_active(&self) -> bool {
        self.chain
            .as_ref()
            .is_some_and(chain_runtime_in_kind_wage_active)
            && self.provenance_bread_good().is_some()
    }

    pub fn in_kind_wage_stats(&self) -> InKindWageStats {
        InKindWageStats {
            open_contracts: self.in_kind_contracts.len(),
            hires_total: self.in_kind_hires_total,
            distinct_workers: self.in_kind_workers_ever.len(),
            distinct_employers: self.in_kind_employers_ever.len(),
            worker_advance_bread: self.in_kind_worker_advance_bread,
            employer_bread_income: self.in_kind_employer_bread_income,
            expected_output_total: self.in_kind_expected_output_total,
            worker_declined: self.in_kind_worker_declined,
            worker_unmatched: self.in_kind_worker_unmatched,
            owner_candidates_total: self.in_kind_owner_candidates_total,
            owner_no_atcap_plot: self.in_kind_owner_no_atcap_plot,
            owner_insufficient_fund: self.in_kind_owner_insufficient_fund,
            productivity_declined: self.in_kind_productivity_declined,
            reservation_collision: self.in_kind_reservation_collision,
            stock_drawdown: self.in_kind_stock_drawdown,
            unattributed_deposit: self.in_kind_unattributed_deposit,
            employer_grain_settled: self.in_kind_employer_grain_settled,
            endowment_funded_hires: self.in_kind_endowment_funded_hires,
            term_starvations: self.in_kind_term_starvations,
        }
    }

    pub fn in_kind_worker_ids(&self) -> Vec<u64> {
        self.in_kind_workers_ever
            .iter()
            .map(|worker| worker.0)
            .collect()
    }
}
