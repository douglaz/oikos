use super::*;

impl Settlement {
    /// The contracted worker's steering: deposit any carry, else haul from the contracted
    /// plot with the regen-bounded room, else idle. Shared by the fast-loop steer and the
    /// idle-task assignment so the two seams can never drift apart.
    pub(super) fn share_contract_task(&self, worker: AgentId, node: NodeId) -> Task {
        if self.world.agent_carry_total(worker) > 0 {
            return Task::GoDeposit(self.exchange);
        }
        let room = self.share_contract_harvest_room(node);
        if room == 0 {
            Task::Idle
        } else {
            Task::GoHarvestWithRoom(node, room, room)
        }
    }

    pub(super) fn steer_share_contract_workers(&mut self) {
        if self.share_contracts.is_empty() {
            return;
        }
        let contracts = self.share_contracts.clone();
        for contract in contracts {
            if !self.private_land_live_agent(contract.worker) {
                continue;
            }
            let desired = self.share_contract_task(contract.worker, contract.node);
            if self.world.agent_task(contract.worker) != Some(desired) {
                self.world.assign_task(contract.worker, desired);
            }
        }
    }

    pub(super) fn run_share_tenancy_phase(&mut self) {
        if !self.share_tenancy_active() {
            return;
        }
        let renewal_hints = self.expire_share_contracts();
        let hint_count = renewal_hints.len() as u64;
        let mut renewal_fates = renewal_hints
            .keys()
            .copied()
            .map(|key| (key, None))
            .collect::<BTreeMap<_, _>>();
        let Some(bread) = self.provenance_bread_good() else {
            for fate in renewal_fates.values_mut() {
                *fate = Some(RenewalFate::BaseIneligible);
            }
            self.finalize_renewal_fates(renewal_fates, hint_count, 0);
            return;
        };
        let workers = self.share_worker_pool(bread);
        if workers.is_empty() {
            self.classify_renewal_fates_without_workers(&mut renewal_fates, bread);
            self.finalize_renewal_fates(renewal_fates, hint_count, 0);
            return;
        }
        let owners = self.share_owner_candidates(bread);
        if owners.is_empty() {
            self.classify_renewal_fates_without_owners(&mut renewal_fates, &workers, bread);
            // Workers wanted a contract but no plot passed the cap-waste gate: unmatched,
            // not declined (spec-review P2 — `worker_declined` means acceptance failed).
            self.share_worker_unmatched = self
                .share_worker_unmatched
                .saturating_add(workers.len() as u64);
            self.finalize_renewal_fates(renewal_fates, hint_count, 0);
            return;
        }
        let same_plot_renewed = self.clear_share_tenancy_market(
            bread,
            workers,
            owners,
            renewal_hints,
            &mut renewal_fates,
        );
        self.finalize_renewal_fates(renewal_fates, hint_count, same_plot_renewed);
    }

    pub(super) fn share_tenancy_mode(&self) -> ShareTenancyMode {
        self.chain
            .as_ref()
            .map_or(ShareTenancyMode::Voluntary, |chain| {
                chain.share_tenancy_mode
            })
    }

    pub(super) fn share_tenancy_terms(&self) -> Option<(u16, u16)> {
        let chain = self.chain.as_ref()?;
        let share_bps = chain.share_bps.min(10_000);
        let term = chain.share_term.max(1);
        Some((share_bps, term))
    }

    pub(super) fn expire_share_contracts(&mut self) -> BTreeMap<(AgentId, AgentId, NodeId), u16> {
        let mut renewal_hints = BTreeMap::new();
        if self.share_contracts.is_empty() {
            return renewal_hints;
        }
        let contracts = std::mem::take(&mut self.share_contracts);
        for contract in contracts {
            let owner_live = self.private_land_live_agent(contract.owner);
            let worker_live = self.private_land_live_agent(contract.worker);
            let due = contract
                .opened_tick
                .saturating_add(u64::from(contract.term))
                <= self.econ_tick;
            if !owner_live || !worker_live || due {
                if owner_live && worker_live {
                    // Term-boundary settle (review P1): this tick's haul is already in the
                    // worker's stock (the deposit transfer runs before this phase) but only
                    // converts in the own-use cultivation phase BELOW — after the contract is
                    // gone. Left unsettled, every contract's final-tick haul plus any
                    // unconverted backlog would convert 100%-to-worker on later ticks,
                    // leaking ~1/term of contract output past the split. (Dead-party
                    // dissolution settles at the death seam instead.)
                    self.settle_share_contract_grain(&contract);
                }
                self.clear_share_reservation(&contract);
                if owner_live && worker_live && due {
                    renewal_hints.insert(
                        (contract.worker, contract.owner, contract.node),
                        contract.renewals.saturating_add(1),
                    );
                }
            } else {
                self.share_contracts.push(contract);
            }
        }
        renewal_hints
    }

    /// Settle the un-converted contract-sourced grain still in the worker's stock at
    /// dissolution: the owner receives its `(1 − s)` share **in kind** (same pinned bps,
    /// same floor direction — the worker keeps `floor(G · share_bps / 10_000)`), a
    /// conserved stock relocation that touches no bread ledger (grain has no provenance
    /// channel; the worker's kept grain later converts as its own bread). Worker-owned
    /// grain from an earlier settled term is deliberately excluded, so an immediate
    /// renewal cannot charge that backlog to the new/current owner. Grain still in
    /// world-carry at dissolution (≤ the carry cap, 6) stays with the worker — a disclosed
    /// residue, conserved either way. Called at term expiry and at the death seam (dead
    /// worker: the owner's share leaves before the estate routes; dead owner: the
    /// still-present dying agent is credited and the estate carries it to the heir — the
    /// wage-escrow death pattern).
    pub(super) fn settle_share_contract_grain(&mut self, contract: &ShareContract) {
        if contract.grain_in_stock == 0 {
            return;
        }
        let Some(grain) = self.cultivation_input_good() else {
            return;
        };
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
        // Carry the accumulated split residue (review R3-P2): the cumulative-floor invariant
        // (§3.3) pays the worker exactly `floor(cum_output · s / 10_000)` after every batch,
        // with the sub-unit residue held in `split_remainder_bps`. Flooring this final
        // unconverted grain fresh would drop a whole worker unit whenever the carried remainder
        // plus this grain completes an integer payout (e.g. s = 25% after three converted loaves
        // leaves 7_500 bps, so one final grain owes the worker one unit). On the pinned 1:1
        // grain→bread recipe a grain unit is one loaf of would-be output, so the identical
        // remainder-aware floor the bread split uses applies here. The contract dissolves after
        // this settle (both callers drop it), so the updated residue is not re-stored.
        let scaled = u128::from(unsettled) * u128::from(contract.share_bps)
            + u128::from(contract.split_remainder_bps);
        let worker_keep = (scaled / 10_000) as u32;
        let owner_grain = unsettled.saturating_sub(worker_keep);
        if owner_grain == 0 {
            return;
        }
        if self
            .society
            .debit_stock(contract.worker, grain, owner_grain)
        {
            if self
                .society
                .credit_stock(contract.owner, grain, owner_grain)
            {
                self.share_owner_grain_settled = self
                    .share_owner_grain_settled
                    .saturating_add(u64::from(owner_grain));
            } else {
                let _ = self
                    .society
                    .credit_stock(contract.worker, grain, owner_grain);
            }
        }
    }

    /// The eligible worker pool for this tick's match: base-eligible AND the real outside
    /// option fails (spec §3.1). Acceptance is NOT evaluated here — `N̂` is a function of
    /// the **contracted plot's** regen × term × budget (spec crux 3), so the match loop
    /// evaluates the ordinal acceptance per assigned candidate (review P1: evaluating the
    /// first sorted candidate as a proxy binds workers to plots they never accepted).
    pub(super) fn share_worker_pool(&mut self, bread: GoodId) -> Vec<AgentId> {
        let mode = self.share_tenancy_mode();
        // The forward horizon is the Voluntary probe's candidacy knob (spec §3.2). It defers
        // to the instantaneous gate for the other modes: `LineageWorker` because its outside
        // option is the lineage-aware hunger threshold (the commons forecast — and the term
        // forecast built on it — structurally skips `household.is_some()` colonists, so it
        // cannot see a lineage worker at all), and `ForcedShare` because it evaluates no
        // outside option below. Keeping the flag orthogonal to the mode (review R1).
        let forward =
            self.share_forward_provisioning_active() && mode == ShareTenancyMode::Voluntary;
        let mut pool = Vec::new();
        for &slot in &self.live_colonist_slots {
            let colonist = &self.colonists[slot];
            if !self.share_worker_base_eligible(colonist.id, mode) {
                continue;
            }
            if mode != ShareTenancyMode::ForcedShare {
                let instantaneous_fails =
                    self.share_worker_instantaneous_outside_option_fails(colonist.id, bread, mode);
                let gate_fails = if forward {
                    let term = self.share_tenancy_terms().map_or(1, |(_, term)| term);
                    self.forecast_term_need_unmet(colonist.id, bread, term) > 0
                } else {
                    instantaneous_fails
                };
                if !gate_fails {
                    continue;
                }
                if forward && !instantaneous_fails {
                    self.share_forward_only_eligibility =
                        self.share_forward_only_eligibility.saturating_add(1);
                }
            }
            pool.push(colonist.id);
        }
        pool.sort();
        pool
    }

    /// Whether the worker's real outside option fails it this tick (spec §3.1 reuses the
    /// S23e commons forecast verbatim). The `LineageWorker` diagnostic cell is
    /// lineage-aware (spec-review P2): the commons draw structurally excludes
    /// `household.is_some()` colonists (the emergency pass skips them), so the reused
    /// forecast would report every lineage worker commons-sufficient and gate it out
    /// before acceptance ever ran — for lineage members the gate reduces to the same
    /// hunger threshold the forecast keys on, and homesteading (their REAL alternative)
    /// stays open as exactly the choice the cell probes. The forward horizon (spec §3.2)
    /// applies only to `Voluntary` for the same reason: the term forecast is built on the
    /// same lineage-blind commons roster, so it too must defer to the instantaneous
    /// lineage-aware gate for `LineageWorker` (review R1) — leaving that diagnostic cell
    /// byte-identical whether or not the forward flag is set.
    pub(super) fn share_worker_outside_option_fails(
        &self,
        worker: AgentId,
        bread: GoodId,
        mode: ShareTenancyMode,
    ) -> bool {
        if self.share_forward_provisioning_active() && mode == ShareTenancyMode::Voluntary {
            let term = self.share_tenancy_terms().map_or(1, |(_, term)| term);
            return self.forecast_term_need_unmet(worker, bread, term) > 0;
        }
        self.share_worker_instantaneous_outside_option_fails(worker, bread, mode)
    }

    pub(super) fn share_worker_base_eligible(
        &self,
        worker: AgentId,
        mode: ShareTenancyMode,
    ) -> bool {
        if self.private_land_agent_holds_any_plot(worker)
            || self.wage_worker_has_open_escrow(worker)
            || self.share_worker_has_contract(worker)
        {
            return false;
        }
        let Some(slot) = self.slot_for_id(worker) else {
            return false;
        };
        let colonist = &self.colonists[slot];
        if !colonist.alive || !matches!(colonist.vocation, Vocation::Consumer | Vocation::Gatherer)
        {
            return false;
        }
        match mode {
            ShareTenancyMode::LineageWorker => colonist.household.is_some(),
            ShareTenancyMode::Voluntary | ShareTenancyMode::ForcedShare => {
                colonist.household.is_none()
            }
        }
    }

    /// The worker's bread-denominated ordinal acceptance (spec §3.1), evaluated against
    /// the exact plot being matched: does adding `floor(N̂ · share_bps / 10_000)` bread —
    /// N̂ derived from THIS `node`'s digested state — newly provision a `Good(BREAD)` want
    /// ranked above the first unsatisfied Now-Leisure want, preserving everything above it?
    pub(super) fn share_worker_accepts_bread(
        &self,
        worker: AgentId,
        bread: GoodId,
        node: NodeId,
    ) -> bool {
        let Some((share_bps, term)) = self.share_tenancy_terms() else {
            return false;
        };
        let expected = self.share_expected_term_output(node, term).unwrap_or(0);
        let expected_share = share_bps_floor(expected, share_bps);
        if expected_share == 0 || expected_share > u64::from(u32::MAX) {
            return false;
        }
        let Some(agent) = self.society.agents.get(worker) else {
            return false;
        };
        let expected_share = expected_share as u32;
        if agent.stock.get(bread).checked_add(expected_share).is_none() {
            return false;
        }
        if self.share_worker_accepts_bread_now(worker, bread, expected_share) {
            return true;
        }
        self.share_forward_provisioning_active()
            && self.forecast_term_need_unmet(worker, bread, term) > 0
            && self.share_forward_leisure_guard(worker, bread)
    }

    pub(super) fn share_worker_accepts_bread_now(
        &self,
        worker: AgentId,
        bread: GoodId,
        expected_share: u32,
    ) -> bool {
        let Some(agent) = self.society.agents.get(worker) else {
            return false;
        };
        let before_endowment = TemporalEndowment {
            stock: &agent.stock,
            gold: agent.gold,
            receivables: &[],
            payables: &[],
            tick: Tick(self.econ_tick),
        };
        let before = provisioning_bitmap_for_money(&agent.scale, &before_endowment, GOLD);
        let mut after_stock = agent.stock.clone();
        after_stock.add(bread, expected_share);
        let after_endowment = TemporalEndowment {
            stock: &after_stock,
            gold: agent.gold,
            receivables: &[],
            payables: &[],
            tick: Tick(self.econ_tick),
        };
        let after = provisioning_bitmap_for_money(&agent.scale, &after_endowment, GOLD);
        let leisure_rank = agent.scale.iter().enumerate().find_map(|(index, want)| {
            (want.kind == WantKind::Leisure
                && matches!(want.horizon, Horizon::Now)
                && !before.get(index).copied().unwrap_or(false))
            .then_some(index)
        });
        let Some(target) = agent.scale.iter().enumerate().find_map(|(index, want)| {
            (want.kind == WantKind::Good(bread)
                && !before.get(index).copied().unwrap_or(false)
                && after.get(index).copied().unwrap_or(false)
                && leisure_rank.map(|rank| index < rank).unwrap_or(true))
            .then_some(index)
        }) else {
            return false;
        };
        preserved_provisioning_above(&before, &after, target)
    }

    pub(super) fn share_owner_candidates(&mut self, bread: GoodId) -> Vec<ShareOwnerCandidate> {
        let candidates = self.share_owner_candidate_plots(bread);
        self.share_owner_candidates_total = self
            .share_owner_candidates_total
            .saturating_add(candidates.len() as u64);
        let mut owners_with_candidate = BTreeSet::new();
        for candidate in &candidates {
            owners_with_candidate.insert(candidate.owner);
        }
        let owners = self.share_owner_ids();
        for owner in owners {
            if owners_with_candidate.contains(&owner) {
                continue;
            }
            if self.share_owner_has_below_cap_plot(owner) {
                self.share_stock_opportunity_refusal =
                    self.share_stock_opportunity_refusal.saturating_add(1);
            } else {
                self.share_owner_no_atcap_plot = self.share_owner_no_atcap_plot.saturating_add(1);
            }
        }
        candidates
    }

    /// The plots that pass the owner's cap-waste dominance gate (spec §3.1): owned by a
    /// live owner, at cap with positive regen (the flow is being destroyed), not reserved,
    /// not under a live contract, and — load-bearing (review P1) — **not a plot the owner
    /// itself uses**. The instantaneous task/carry probe alone misses the boundary
    /// instant: deposits transfer and `carried_grain_source` clears BEFORE this phase, so
    /// an owner idle between trips would look detached from its working plot while regen
    /// has already topped it back to cap. Candidacy therefore also excludes the plot the
    /// owner's own target selection (`private_land_target_for_agent`) would pick right
    /// now — sharing that plot would silently displace the owner for the whole term
    /// (target selection skips reserved plots, so `reservation_collision` would stay 0)
    /// and its `(1 − s)` share would NOT be something-for-nothing.
    pub(super) fn share_owner_candidate_plots(&self, bread: GoodId) -> Vec<ShareOwnerCandidate> {
        let Some((share_bps, term)) = self.share_tenancy_terms() else {
            return Vec::new();
        };
        if share_bps >= 10_000 {
            return Vec::new();
        }
        let no_reserved: BTreeSet<NodeId> = BTreeSet::new();
        let mut owner_targets: BTreeMap<AgentId, Option<NodeId>> = BTreeMap::new();
        let mut candidates = Vec::new();
        for (&node, record) in &self.land_plots {
            let Some(owner) = record.owner else {
                continue;
            };
            if !self.private_land_live_agent(owner)
                || record.reserved_for.is_some()
                || self.share_contract_for_node(node).is_some()
                || self.share_plot_currently_owner_targeted(owner, node)
            {
                continue;
            }
            let target = *owner_targets
                .entry(owner)
                .or_insert_with(|| self.private_land_target_for_agent(owner, &no_reserved));
            if target == Some(node) {
                continue;
            }
            let Some(plot) = self.world.node(node) else {
                continue;
            };
            if plot.stock != plot.cap || plot.regen_per_tick == 0 {
                continue;
            }
            let expected = self.share_expected_term_output(node, term).unwrap_or(0);
            let owner_share = expected.saturating_sub(share_bps_floor(expected, share_bps));
            if owner_share == 0 || owner_share > u64::from(u32::MAX) {
                continue;
            }
            let Some(agent) = self.society.agents.get(owner) else {
                continue;
            };
            if agent
                .stock
                .get(bread)
                .checked_add(owner_share as u32)
                .is_none()
            {
                continue;
            }
            candidates.push(ShareOwnerCandidate {
                owner,
                node,
                cap_at_start: plot.stock,
            });
        }
        candidates.sort_by_key(|candidate| (candidate.owner.0, candidate.node.0));
        candidates
    }

    pub(super) fn share_owner_ids(&self) -> BTreeSet<AgentId> {
        self.land_plots
            .values()
            .filter_map(|record| record.owner)
            .filter(|&owner| self.private_land_live_agent(owner))
            .collect()
    }

    /// Whether the owner holds a below-cap plot that would have been offerable but for the
    /// stock opportunity cost — the disclosed out-of-scope margin `stock_opportunity_refusal`
    /// counts. Uses the same withhold conditions as candidacy (incl. the would-be-target
    /// exclusion) so the refusal classification matches what the gate actually declined.
    pub(super) fn share_owner_has_below_cap_plot(&self, owner: AgentId) -> bool {
        let no_reserved: BTreeSet<NodeId> = BTreeSet::new();
        let target = self.private_land_target_for_agent(owner, &no_reserved);
        self.land_plots.iter().any(|(&node, record)| {
            record.owner == Some(owner)
                && record.reserved_for.is_none()
                && self.share_contract_for_node(node).is_none()
                && target != Some(node)
                && !self.share_plot_currently_owner_targeted(owner, node)
                && self
                    .world
                    .node(node)
                    .is_some_and(|plot| plot.stock < plot.cap && plot.regen_per_tick > 0)
        })
    }

    pub(super) fn share_plot_currently_owner_targeted(&self, owner: AgentId, node: NodeId) -> bool {
        self.world
            .agent_task(owner)
            .and_then(Self::private_land_harvest_task_node)
            == Some(node)
            || self
                .slot_for_id(owner)
                .is_some_and(|slot| self.colonists[slot].carried_grain_source == Some(node))
    }

    pub(super) fn clear_share_tenancy_market(
        &mut self,
        bread: GoodId,
        workers: Vec<AgentId>,
        mut owners: Vec<ShareOwnerCandidate>,
        renewal_hints: BTreeMap<RenewalHintKey, u16>,
        renewal_fates: &mut BTreeMap<RenewalHintKey, Option<RenewalFate>>,
    ) -> u64 {
        let forced = self.share_tenancy_mode() == ShareTenancyMode::ForcedShare;
        let mode = self.share_tenancy_mode();
        let mut matched: BTreeSet<AgentId> = BTreeSet::new();
        let hint_by_worker: BTreeMap<AgentId, RenewalHintKey> = renewal_hints
            .keys()
            .copied()
            .map(|key @ (worker, _, _)| (worker, key))
            .collect();
        let mut same_plot_renewed = 0u64;
        // Incumbent-first renewal (S22f: persistence must come from RE-CHOOSING): an
        // expiring pair is offered ITS OWN plot before the general pass, both sides
        // re-deciding from fresh state — the plot is in `owners` only if it passes the
        // cap-waste gate afresh (incl. the owner's would-be-target exclusion), and the
        // worker must re-accept that plot's expected share. Without this preference the
        // order-based greedy essentially never re-forms the exact (worker, owner, node)
        // triple, leaving the §2.1-2 renewal clause dead machinery (review P3). A pair
        // that fails either side falls through to the general pass as a fresh candidate.
        for (&(worker, owner, node), &renewals) in &renewal_hints {
            if matched.contains(&worker) || workers.binary_search(&worker).is_err() {
                Self::set_renewal_fate(
                    renewal_fates,
                    (worker, owner, node),
                    self.renewal_fate_outside_pool(worker, bread, mode),
                );
                continue;
            }
            let Some(position) = owners
                .iter()
                .position(|candidate| candidate.owner == owner && candidate.node == node)
            else {
                Self::set_renewal_fate(
                    renewal_fates,
                    (worker, owner, node),
                    RenewalFate::OwnerNotCandidate,
                );
                continue;
            };
            if !forced && !self.share_worker_accepts_bread(worker, bread, node) {
                Self::set_renewal_fate(
                    renewal_fates,
                    (worker, owner, node),
                    RenewalFate::BreadDeclined,
                );
                continue;
            }
            let candidate = owners.remove(position);
            self.open_share_contract(worker, candidate, renewals);
            renewal_fates.remove(&(worker, owner, node));
            same_plot_renewed = same_plot_renewed.saturating_add(1);
            matched.insert(worker);
        }
        for worker in workers {
            if matched.contains(&worker) {
                continue;
            }
            if owners.is_empty() {
                // Not a decline: no candidate was left to evaluate (spec-review P2 —
                // `worker_declined` means the bread acceptance itself failed).
                self.share_worker_unmatched = self.share_worker_unmatched.saturating_add(1);
                if let Some(&key) = hint_by_worker.get(&worker) {
                    Self::set_renewal_fate(renewal_fates, key, RenewalFate::OwnerNotCandidate);
                }
                continue;
            }
            // Greedy deterministic: the first candidate (owner-id, node order) whose
            // expected share this worker's ordinal evaluator accepts — acceptance is
            // evaluated against the exact plot being bound (spec crux 3), never a proxy.
            let accepted = owners.iter().position(|candidate| {
                forced || self.share_worker_accepts_bread(worker, bread, candidate.node)
            });
            match accepted {
                Some(position) => {
                    let candidate = owners.remove(position);
                    self.open_share_contract(worker, candidate, 0);
                    if let Some(&key) = hint_by_worker.get(&worker) {
                        if key.2 != candidate.node {
                            Self::set_renewal_fate(
                                renewal_fates,
                                key,
                                RenewalFate::MatchedElsewhere,
                            );
                        }
                    }
                }
                None => {
                    self.share_worker_declined = self.share_worker_declined.saturating_add(1);
                    if let Some(&key) = hint_by_worker.get(&worker) {
                        Self::set_renewal_fate(renewal_fates, key, RenewalFate::BreadDeclined);
                    }
                }
            }
        }
        same_plot_renewed
    }

    pub(super) fn open_share_contract(
        &mut self,
        worker: AgentId,
        candidate: ShareOwnerCandidate,
        renewals: u16,
    ) {
        let Some((share_bps, term)) = self.share_tenancy_terms() else {
            return;
        };
        if let Some(record) = self.land_plots.get_mut(&candidate.node) {
            if record.reserved_for.is_some() {
                return;
            }
            record.reserved_for = Some(worker);
        } else {
            return;
        }
        let id = self.next_share_contract_id;
        self.next_share_contract_id = self.next_share_contract_id.wrapping_add(1);
        self.share_contracts.push(ShareContract {
            id,
            owner: candidate.owner,
            worker,
            node: candidate.node,
            share_bps,
            term,
            opened_tick: self.econ_tick,
            renewals,
            cap_at_start: candidate.cap_at_start,
            grain_in_stock: 0,
            split_remainder_bps: 0,
        });
        self.share_contracts_total = self.share_contracts_total.saturating_add(1);
        if self.share_tenancy_mode() == ShareTenancyMode::ForcedShare {
            self.share_forced_contracts_total = self.share_forced_contracts_total.saturating_add(1);
        } else {
            self.share_voluntary_contracts_total =
                self.share_voluntary_contracts_total.saturating_add(1);
        }
        if renewals > 0 {
            self.share_renewals_total = self.share_renewals_total.saturating_add(1);
        }
        self.share_workers_ever.insert(worker);
        self.share_owners_ever.insert(candidate.owner);
    }

    pub(super) fn clear_share_reservation(&mut self, contract: &ShareContract) {
        if let Some(record) = self.land_plots.get_mut(&contract.node) {
            if record.reserved_for == Some(contract.worker) {
                record.reserved_for = None;
            }
        }
    }

    pub(super) fn settle_share_tenancy_for_death(&mut self, dead: AgentId) {
        if self.share_contracts.is_empty() {
            return;
        }
        let contracts = std::mem::take(&mut self.share_contracts);
        for contract in contracts {
            if contract.owner == dead || contract.worker == dead {
                // Death dissolution (spec §3.4): the realized split stands; the pending
                // contract-sourced grain settles exactly as expiry does (review P1 — the
                // death seam had the same final-haul leak). The dying agent's society entry
                // is still present here (the wage-escrow death pattern): a dead owner is
                // credited and the estate carries the grain to the heir; a dead worker's
                // settle pays the living owner before the estate routes the rest.
                self.settle_share_contract_grain(&contract);
                self.clear_share_reservation(&contract);
            } else {
                self.share_contracts.push(contract);
            }
        }
    }

    pub(super) fn share_worker_has_contract(&self, worker: AgentId) -> bool {
        self.share_contract_for_worker(worker).is_some()
    }

    /// A contracted share worker is admitted to exactly the plot its live contract reserves
    /// for it: the record's reservation must name the worker AND the worker's contract must
    /// be over this node. Shared by the pre-tick harvest validation and the worked-event
    /// gate so the two admission seams can never drift apart.
    pub(super) fn share_worker_admitted_to(
        &self,
        agent: AgentId,
        node: NodeId,
        record: &LandPlotRecord,
    ) -> bool {
        record.reserved_for == Some(agent)
            && self
                .share_contract_for_worker(agent)
                .is_some_and(|contract| contract.node == node)
    }

    pub(super) fn share_contract_for_worker(&self, worker: AgentId) -> Option<ShareContract> {
        self.share_contracts
            .iter()
            .copied()
            .find(|contract| contract.worker == worker)
    }

    pub(super) fn credit_share_contract_grain(&mut self, worker: AgentId, qty: u32) {
        if qty == 0 {
            return;
        }
        // Attribute only grain hauled from the contract's OWN plot to the contract's stock
        // (review R3-P2). A worker that opens or renews a contract while still carrying grain
        // from an earlier term's plot (or a pre-contract haul) would otherwise charge that
        // worker-owned residue to the current owner at the split/settle — precisely the
        // exclusion the settle path documents. `carried_grain_source` is set at every
        // worked-land harvest (:11635) and cleared only once carry AND pending both empty, so
        // it names the plot the deposited grain came from; a steered share worker harvests only
        // its contracted plot, so in steady state this matches (a no-op) and diverges only for
        // the cross-term carryover it is meant to exclude.
        let source = self.colonist_slot_by_id.get(&worker).map(|&slot| {
            (
                self.colonists[slot].carried_grain_source,
                self.colonists[slot].carried_share_contract_id,
            )
        });
        if let Some(contract) = self
            .share_contracts
            .iter_mut()
            .find(|contract| contract.worker == worker)
        {
            if source == Some((Some(contract.node), Some(contract.id))) {
                contract.grain_in_stock = contract.grain_in_stock.saturating_add(qty);
            }
        }
    }

    pub(super) fn share_contract_for_node(&self, node: NodeId) -> Option<ShareContract> {
        self.share_contracts
            .iter()
            .copied()
            .find(|contract| contract.node == node)
    }

    pub(super) fn share_plot_reserved_against_owner(&self, owner: AgentId, node: NodeId) -> bool {
        self.share_contracts
            .iter()
            .any(|contract| contract.owner == owner && contract.node == node)
    }

    pub(super) fn share_contract_harvest_room(&self, node: NodeId) -> u32 {
        self.world
            .node(node)
            .map_or(0, |plot| plot.regen_per_tick.min(plot.cap))
    }

    /// `N̂` — the expected term output the two ordinal gates evaluate: the contracted
    /// plot's regen × the econ tick's fast-tick count × the term, bounded by the per-tick
    /// own-use labor budget. A pure function of already-digested state (spec §3.1: the
    /// cap-waste gate bounds the draw to regen, so regen IS the expected flow — no
    /// realized-experience history enters the digest). Disclosure (review P3): this is the
    /// flow bound, not a haul simulation — the realized draw is further limited by trip
    /// mechanics (the ~6-unit carry cap per round trip), so `N̂` overestimates realized
    /// output; the spec pins this construction deliberately.
    pub(super) fn share_expected_term_output(&self, node: NodeId, term: u16) -> Option<u64> {
        let plot = self.world.node(node)?;
        let recipe = self.chain.as_ref()?.content.cultivate_recipe()?;
        let input_qty = recipe.input_good.map_or(1, |(_, qty)| qty).max(1);
        let labor_runs = OWN_USE_CULTIVATION_LABOR_BUDGET
            .checked_div(recipe.labor.max(1))
            .unwrap_or(0);
        let grain_flow = u64::from(plot.regen_per_tick)
            .saturating_mul(FAST_TICKS_PER_ECON_TICK)
            .checked_div(u64::from(input_qty))
            .unwrap_or(0);
        let runs_per_tick = u64::from(labor_runs).min(grain_flow);
        Some(
            runs_per_tick
                .saturating_mul(u64::from(recipe.output_qty))
                .saturating_mul(u64::from(term)),
        )
    }

    pub(super) fn check_share_stock_drawdown(&mut self, events: &[WorkedLandEvent]) {
        if self.share_contracts.is_empty() || events.is_empty() {
            return;
        }
        for event in events {
            let Some(contract) = self.share_contract_for_worker(event.agent) else {
                continue;
            };
            if contract.node != event.node {
                continue;
            }
            if self.world.node(event.node).is_some_and(|plot| {
                event.moved > plot.regen_per_tick || plot.stock < contract.cap_at_start
            }) {
                self.share_stock_drawdown = self.share_stock_drawdown.saturating_add(1);
            }
        }
    }

    pub(super) fn split_share_output(&mut self, worker: AgentId, output_qty: u64, input_qty: u32) {
        if output_qty == 0 || input_qty == 0 {
            return;
        }
        let Some(index) = self
            .share_contracts
            .iter()
            .position(|contract| contract.worker == worker)
        else {
            return;
        };
        let contract = self.share_contracts[index];
        let contract_input = input_qty.min(contract.grain_in_stock);
        if contract_input == 0 {
            return;
        }
        let Some(bread) = self.provenance_bread_good() else {
            return;
        };
        self.share_contracts[index].grain_in_stock = self.share_contracts[index]
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
        if split_output == 0 {
            return;
        }
        // Cumulative-exact floor (review P1): `Cultivate` books ONE loaf per application,
        // so flooring each batch independently would pay the worker zero at any share
        // below 100%. The carried remainder makes the cumulative worker payout exactly
        // `floor(cumulative_output · share_bps / 10_000)` after every batch — the identical
        // integer floor the acceptance evaluator uses (§3.1/§3.3); the final sub-unit
        // residue (< 1 loaf) lapses to the owner at dissolution. Rounding direction
        // disclosed, not tunable.
        let scaled = u128::from(split_output) * u128::from(contract.share_bps)
            + u128::from(contract.split_remainder_bps);
        let worker_share = (scaled / 10_000) as u64;
        self.share_contracts[index].split_remainder_bps = (scaled % 10_000) as u16;
        let owner_share = split_output.saturating_sub(worker_share);
        let mut owner_transferred = 0u64;
        if owner_share > 0 && owner_share <= u64::from(u32::MAX) {
            let owner_qty = owner_share as u32;
            if self.society.debit_stock(worker, bread, owner_qty) {
                if self.society.credit_stock(contract.owner, bread, owner_qty) {
                    owner_transferred = owner_share;
                    if self.bread_provenance_active() {
                        self.bread_provenance
                            .transfer(worker, contract.owner, owner_share);
                    }
                    if self.acquisition_ledger_active() {
                        self.acquisition
                            .transfer_preserve(worker, contract.owner, owner_share);
                    }
                } else {
                    let _ = self.society.credit_stock(worker, bread, owner_qty);
                }
            }
        }
        self.share_owner_bread_income = self
            .share_owner_bread_income
            .saturating_add(owner_transferred);
        self.share_worker_bread_income = self
            .share_worker_bread_income
            .saturating_add(split_output.saturating_sub(owner_transferred));
    }

    pub(super) fn forecast_term_need_unmet(
        &self,
        target_agent: AgentId,
        bread: GoodId,
        term: u16,
    ) -> u64 {
        if !self.rival_subsistence_commons_active() || term == 0 {
            return 0;
        }
        let threshold = self
            .chain
            .as_ref()
            .map_or(0, |chain| chain.emergency_hunger_threshold);
        if threshold == 0 || self.dynamics.hunger_per_food == 0 {
            return 0;
        }
        let target_hunger = threshold.saturating_sub(1);
        let mut members = Vec::new();
        for &slot in &self.live_colonist_slots {
            let colonist = &self.colonists[slot];
            if colonist.household.is_some()
                || !matches!(colonist.vocation, Vocation::Consumer | Vocation::Gatherer)
            {
                continue;
            }
            let held_total = self
                .society
                .agents
                .get(colonist.id)
                .map_or(0, |agent| u64::from(agent.stock.get(bread)));
            members.push(TermNeedMember {
                agent: colonist.id,
                hunger: colonist.need.hunger,
                held_free_bread: u64::from(
                    self.society
                        .free_stock_after_all_reserves(colonist.id, bread),
                ),
                held_total_bread: held_total,
            });
        }
        if !members.iter().any(|member| member.agent == target_agent) {
            return 0;
        }

        let mut commons_stock = self.subsistence_commons_stock;
        let mut target_unmet = 0u64;
        for _ in 0..term {
            let mut requests = Vec::new();
            for (index, member) in members.iter_mut().enumerate() {
                let needed = u64::from(food_needed_to_reach_hunger(
                    member.hunger,
                    self.dynamics.hunger_deplete,
                    self.dynamics.hunger_per_food,
                    target_hunger,
                ));
                if needed == 0 {
                    member.hunger = advance_hunger_after_food(
                        member.hunger,
                        self.dynamics.hunger_deplete,
                        self.dynamics.hunger_per_food,
                        self.dynamics.need_max,
                        0,
                    );
                    continue;
                }
                let held_eat = member.held_free_bread.min(needed);
                member.held_free_bread = member.held_free_bread.saturating_sub(held_eat);
                member.held_total_bread = member.held_total_bread.saturating_sub(held_eat);
                let need = needed.saturating_sub(held_eat);
                if need > 0 {
                    requests.push((member.hunger, member.agent, index, held_eat, need));
                } else {
                    member.hunger = advance_hunger_after_food(
                        member.hunger,
                        self.dynamics.hunger_deplete,
                        self.dynamics.hunger_per_food,
                        self.dynamics.need_max,
                        held_eat,
                    );
                }
            }

            let mut available = commons_stock
                .saturating_add(self.subsistence_commons_regen)
                .min(self.subsistence_commons_cap);
            requests.sort_by_key(|&(hunger, agent, _, _, _)| (std::cmp::Reverse(hunger), agent.0));
            for (_, agent, index, held_eat, need) in requests {
                let headroom = u64::from(u32::MAX).saturating_sub(members[index].held_total_bread);
                let draw = need.min(available).min(headroom);
                available = available.saturating_sub(draw);
                let unmet = need.saturating_sub(draw);
                members[index].hunger = advance_hunger_after_food(
                    members[index].hunger,
                    self.dynamics.hunger_deplete,
                    self.dynamics.hunger_per_food,
                    self.dynamics.need_max,
                    held_eat.saturating_add(draw),
                );
                if agent == target_agent {
                    target_unmet = target_unmet.saturating_add(unmet);
                }
            }
            commons_stock = available;
        }
        target_unmet
    }

    pub(super) fn share_tenancy_active(&self) -> bool {
        self.chain
            .as_ref()
            .is_some_and(chain_runtime_share_tenancy_active)
            && self.provenance_bread_good().is_some()
    }

    pub fn share_tenancy_stats(&self) -> ShareTenancyStats {
        ShareTenancyStats {
            open_contracts: self.share_contracts.len(),
            contracts_total: self.share_contracts_total,
            voluntary_contracts_total: self.share_voluntary_contracts_total,
            forced_contracts_total: self.share_forced_contracts_total,
            renewals_total: self.share_renewals_total,
            distinct_workers: self.share_workers_ever.len(),
            distinct_owners: self.share_owners_ever.len(),
            worker_bread_income: self.share_worker_bread_income,
            owner_bread_income: self.share_owner_bread_income,
            worker_declined: self.share_worker_declined,
            worker_unmatched: self.share_worker_unmatched,
            forward_only_eligibility: self.share_forward_only_eligibility,
            renewal_hints_total: self.share_renewal_hints_total,
            renewal_fed_out: self.share_renewal_fed_out,
            renewal_base_ineligible: self.share_renewal_base_ineligible,
            renewal_owner_not_candidate: self.share_renewal_owner_not_candidate,
            renewal_bread_declined: self.share_renewal_bread_declined,
            renewal_matched_elsewhere: self.share_renewal_matched_elsewhere,
            owner_candidates_total: self.share_owner_candidates_total,
            owner_no_atcap_plot: self.share_owner_no_atcap_plot,
            stock_opportunity_refusal: self.share_stock_opportunity_refusal,
            reservation_collision: self.share_reservation_collision,
            share_stock_drawdown: self.share_stock_drawdown,
            unattributed_share_deposit: self.share_unattributed_share_deposit,
            owner_grain_settled: self.share_owner_grain_settled,
        }
    }

    pub fn share_worker_ids(&self) -> Vec<u64> {
        self.share_workers_ever
            .iter()
            .map(|worker| worker.0)
            .collect()
    }
}
