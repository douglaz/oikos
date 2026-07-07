use super::*;

impl Settlement {
    pub(super) fn wage_worker_has_open_escrow(&self, worker: AgentId) -> bool {
        self.wage_labor_active()
            && self
                .wage_escrows
                .iter()
                .any(|escrow| escrow.worker == worker)
    }

    pub(super) fn idle_open_wage_workers(&mut self) {
        if !self.wage_labor_active() || self.wage_escrows.is_empty() {
            return;
        }
        let workers: Vec<AgentId> = self
            .wage_escrows
            .iter()
            .map(|escrow| escrow.worker)
            .collect();
        for worker in workers {
            let _ = self.world.assign_task(worker, Task::Idle);
        }
    }

    pub(super) fn run_wage_labor_phase(
        &mut self,
        report: &mut EconTickReport,
        wage_labor_used: &mut Vec<(AgentId, u32)>,
    ) {
        if !self.wage_labor_active() {
            return;
        }
        self.reset_wage_labor_capacities();
        self.release_due_wage_escrows(report, wage_labor_used);
        if !self.wage_labor_market_open() {
            return;
        }
        let Some(recipe) = self.wage_labor_recipe() else {
            return;
        };
        let labor_qty = recipe.labor.max(1);
        let workers = self.wage_worker_quotes(recipe.output_good, labor_qty);
        if workers.is_empty() {
            return;
        }
        let employers = self.wage_hire_candidates(&recipe);
        if employers.is_empty() {
            self.wage_below_ask_not_hired = self
                .wage_below_ask_not_hired
                .saturating_add(u64::try_from(workers.len()).unwrap_or(u64::MAX));
            return;
        }
        self.clear_wage_labor_market(&recipe, labor_qty, workers, employers);
    }

    pub(super) fn reset_wage_labor_capacities(&mut self) {
        for &slot in &self.live_colonist_slots {
            let id = self.colonists[slot].id;
            if let Some(agent) = self.society.agents.get_mut(id) {
                agent.labor_capacity = 0;
            }
        }
    }

    pub(super) fn wage_labor_recipe(&self) -> Option<Recipe> {
        let recipe = self.chain.as_ref()?.content.cultivate_recipe()?.clone();
        (recipe.enabled && recipe.input_good.is_some() && recipe.output_qty > 0).then_some(recipe)
    }

    pub(super) fn wage_labor_mode(&self) -> WageLaborMode {
        self.chain
            .as_ref()
            .map_or(WageLaborMode::Voluntary, |chain| chain.wage_labor_mode)
    }

    pub(super) fn wage_worker_quotes(&self, bread: GoodId, labor_qty: u32) -> Vec<WageWorkerQuote> {
        let mode = self.wage_labor_mode();
        let mut quotes = Vec::new();
        for &slot in &self.live_colonist_slots {
            let colonist = &self.colonists[slot];
            // Only the demand-side non-owner roles the finite commons actually feeds may sell
            // labor: `Consumer`/`Gatherer` — the exact set the realized S23e draw
            // (`run_emergency_self_provision`) and the pre-market forecast below both use. Admitting
            // `Unassigned` here (a latent producer the commons never feeds) diverged from those two
            // sets, so `forecast_commons_sufficiency` returned "sufficient" for it and silently
            // gated it out anyway; keeping the three sets identical removes the asymmetry.
            if self.private_land_agent_holds_any_plot(colonist.id)
                || !matches!(colonist.vocation, Vocation::Consumer | Vocation::Gatherer)
                || self.forecast_commons_sufficiency(colonist.id, bread)
            {
                continue;
            }
            let ask = match mode {
                WageLaborMode::FiatWage => Some(Gold(1)),
                WageLaborMode::Voluntary | WageLaborMode::SubsidisedWage => {
                    self.worker_labor_ask_for_salt(colonist.id, labor_qty)
                }
            };
            if let Some(ask) = ask.filter(|ask| *ask > Gold::ZERO) {
                quotes.push(WageWorkerQuote {
                    worker: colonist.id,
                    ask,
                });
            }
        }
        quotes.sort_by_key(|quote| (quote.ask, quote.worker.0));
        quotes
    }

    pub(super) fn wage_worker_available_labor(
        &self,
        worker: AgentId,
        labor_qty: u32,
    ) -> Option<u32> {
        if labor_qty == 0
            || self.private_land_agent_holds_any_plot(worker)
            || self.share_worker_has_contract(worker)
            || self
                .wage_escrows
                .iter()
                .any(|escrow| escrow.worker == worker)
        {
            return None;
        }
        let slot = self.slot_for_id(worker)?;
        let colonist = &self.colonists[slot];
        if !colonist.alive || !matches!(colonist.vocation, Vocation::Consumer | Vocation::Gatherer)
        {
            return None;
        }
        Some(labor_qty)
    }

    pub(super) fn wage_hire_candidates(&self, recipe: &Recipe) -> Vec<WageHireCandidate> {
        let Some(raw_output_price) = self.society.realized_price(recipe.output_good) else {
            return Vec::new();
        };
        let mode = self.wage_labor_mode();
        let mut owners: BTreeSet<AgentId> = BTreeSet::new();
        for record in self.land_plots.values() {
            owners.extend(Self::private_land_record_holders(record));
        }
        let mut candidates = Vec::new();
        for employer in owners {
            let Some(agent) = self.society.agents.get(employer) else {
                continue;
            };
            if !self.private_land_live_agent(employer)
                || !self.wage_recipe_inputs_available(employer, recipe)
            {
                continue;
            }
            let output_held = agent.stock.get(recipe.output_good);
            if output_held.checked_add(recipe.output_qty).is_none() {
                continue;
            }
            let forecast_bias = self
                .slot_for_id(employer)
                .map_or(FORECAST_BIAS_NEUTRAL_BPS, |slot| {
                    self.colonists[slot].culture.forecast_bias_bps
                });
            let Some(output_price) = forecast_output_price(
                agent,
                recipe.output_good,
                Some(raw_output_price),
                forecast_bias,
            ) else {
                continue;
            };
            // §4.3 anti-inflation guard: cap the forecast at the observed realized output price
            // (the same discipline `project_input_bid_limit` applies to input bids) so a bullish
            // belief or a positive forecast bias cannot inflate the expected revenue, and with it
            // the wage-acceptance ceiling, above the output's realized value.
            let output_price = output_price.min(raw_output_price);
            let Some(expected_revenue) = gold_mul_qty(output_price, recipe.output_qty) else {
                continue;
            };
            let free = self.society.free_gold_after_all_reserves(employer);
            let retained = self
                .wage_retained_earnings
                .get(&employer)
                .copied()
                .unwrap_or(Gold::ZERO);
            let spendable = match mode {
                WageLaborMode::Voluntary => retained.min(free),
                WageLaborMode::FiatWage | WageLaborMode::SubsidisedWage => free,
            };
            let max_total_wage = spendable.min(expected_revenue);
            if max_total_wage == Gold::ZERO {
                continue;
            }
            let max_total_wage = match mode {
                WageLaborMode::FiatWage => max_total_wage,
                WageLaborMode::Voluntary | WageLaborMode::SubsidisedWage => {
                    let Some(total) = highest_appraised_labor_total_wage(
                        agent,
                        expected_revenue,
                        max_total_wage,
                        self.econ_tick,
                        SALT,
                    ) else {
                        continue;
                    };
                    total
                }
            };
            if max_total_wage > Gold::ZERO {
                candidates.push(WageHireCandidate {
                    employer,
                    max_total_wage,
                });
            }
        }
        candidates.sort_by_key(|candidate| {
            (
                std::cmp::Reverse(candidate.max_total_wage),
                candidate.employer.0,
            )
        });
        candidates
    }

    pub(super) fn wage_recipe_inputs_available(&self, employer: AgentId, recipe: &Recipe) -> bool {
        let Some((input, input_qty)) = recipe.input_good else {
            return false;
        };
        self.society.free_stock_after_all_reserves(employer, input) >= input_qty
    }

    pub(super) fn clear_wage_labor_market(
        &mut self,
        recipe: &Recipe,
        labor_qty: u32,
        workers: Vec<WageWorkerQuote>,
        mut employers: Vec<WageHireCandidate>,
    ) {
        for worker in workers {
            // `worker.ask` is the total reservation wage for the whole `labor_qty` bundle
            // (`reservation_labor_ask_for_money(labor_qty, ...)`). Match and escrow that same total
            // amount; never multiply it by `labor_qty` or floor the employer's total ceiling into
            // per-unit pieces.
            let Some((index, amount)) =
                employers.iter().enumerate().find_map(|(index, candidate)| {
                    wage_hire_payment(worker.ask, candidate.max_total_wage)
                        .map(|amount| (index, amount))
                })
            else {
                self.wage_below_ask_not_hired = self.wage_below_ask_not_hired.saturating_add(1);
                continue;
            };
            let candidate = employers.remove(index);
            let Some((retained_funded, endowment_funded)) =
                self.reserve_wage_funding(candidate.employer, amount)
            else {
                continue;
            };
            if !self.debit_to_wage_escrow(candidate.employer, amount) {
                self.restore_wage_retained_earnings(candidate.employer, retained_funded);
                continue;
            }
            let contract_id = self.next_wage_contract_id;
            self.next_wage_contract_id = self.next_wage_contract_id.wrapping_add(1);
            self.wage_escrows.push(WageEscrow {
                id: contract_id,
                employer: candidate.employer,
                worker: worker.worker,
                amount,
                wage: worker.ask,
                retained_funded,
                endowment_funded,
                qty: labor_qty,
                opened_tick: self.econ_tick,
                release_tick: self.econ_tick.saturating_add(1),
                recipe: recipe.id,
                output_good: recipe.output_good,
                output_qty: recipe.output_qty,
                input: recipe.input_good,
                delivered: 0,
            });
            self.wage_hires_total = self.wage_hires_total.saturating_add(1);
            if self.current_money_good() == Some(SALT) {
                self.wage_hires_post_promotion = self.wage_hires_post_promotion.saturating_add(1);
            }
            self.wage_workers_ever.insert(worker.worker);
            self.wage_employers_ever.insert(candidate.employer);
        }
    }

    pub(super) fn reserve_wage_funding(
        &mut self,
        employer: AgentId,
        amount: Gold,
    ) -> Option<(Gold, Gold)> {
        if amount == Gold::ZERO || self.society.free_gold_after_all_reserves(employer) < amount {
            return None;
        }
        let mode = self.wage_labor_mode();
        let retained = self
            .wage_retained_earnings
            .get(&employer)
            .copied()
            .unwrap_or(Gold::ZERO);
        if mode == WageLaborMode::Voluntary && retained < amount {
            return None;
        }
        let retained_funded = retained.min(amount);
        let endowment_funded = amount.saturating_sub(retained_funded);
        if endowment_funded > Gold::ZERO && mode == WageLaborMode::Voluntary {
            return None;
        }
        let remaining = retained.saturating_sub(retained_funded);
        if remaining > Gold::ZERO {
            self.wage_retained_earnings.insert(employer, remaining);
        } else {
            self.wage_retained_earnings.remove(&employer);
        }
        Some((retained_funded, endowment_funded))
    }

    pub(super) fn restore_wage_retained_earnings(&mut self, employer: AgentId, amount: Gold) {
        if amount == Gold::ZERO {
            return;
        }
        let cap = self.wage_retained_earnings_cap(employer);
        let current = self
            .wage_retained_earnings
            .get(&employer)
            .copied()
            .unwrap_or(Gold::ZERO)
            .min(cap);
        let restored = amount.min(cap.saturating_sub(current));
        let next = current.saturating_add(restored);
        if next > Gold::ZERO {
            self.wage_retained_earnings.insert(employer, next);
        } else {
            self.wage_retained_earnings.remove(&employer);
        }
    }

    /// Spend down an owner's wage-eligible retained-earnings provenance tag when it buys goods
    /// (§4.6 anti-subsidy guard). Purely subtractive — it can never inflate the ledger — so an
    /// owner that spends its realized proceeds on goods cannot then fund a voluntary wage from
    /// them a second time. Saturates at zero; drops the entry when depleted.
    pub(super) fn debit_wage_retained_earnings(&mut self, owner: AgentId, amount: Gold) {
        if amount == Gold::ZERO {
            return;
        }
        let Some(current) = self.wage_retained_earnings.get(&owner).copied() else {
            return;
        };
        let remaining = current.saturating_sub(amount);
        if remaining > Gold::ZERO {
            self.wage_retained_earnings.insert(owner, remaining);
        } else {
            self.wage_retained_earnings.remove(&owner);
        }
    }

    pub(super) fn debit_to_wage_escrow(&mut self, employer: AgentId, amount: Gold) -> bool {
        if amount == Gold::ZERO || self.society.free_gold_after_all_reserves(employer) < amount {
            return false;
        }
        let Some(agent) = self.society.agents.get_mut(employer) else {
            return false;
        };
        let Some(next_gold) = agent.gold.checked_sub(amount) else {
            return false;
        };
        agent.gold = next_gold;
        self.wage_escrow_gold = self.wage_escrow_gold.saturating_add(amount);
        true
    }

    pub(super) fn credit_from_wage_escrow(&mut self, recipient: AgentId, amount: Gold) -> bool {
        if amount == Gold::ZERO {
            return true;
        }
        let Some(next_escrow) = self.wage_escrow_gold.checked_sub(amount) else {
            return false;
        };
        self.wage_escrow_gold = next_escrow;
        if let Some(agent) = self.society.agents.get_mut(recipient) {
            agent.gold = agent.gold.saturating_add(amount);
        } else {
            self.commons_gold = self.commons_gold.saturating_add(amount);
        }
        true
    }

    pub(super) fn release_due_wage_escrows(
        &mut self,
        report: &mut EconTickReport,
        wage_labor_used: &mut Vec<(AgentId, u32)>,
    ) {
        if self.wage_escrows.is_empty() {
            return;
        }
        let escrows = std::mem::take(&mut self.wage_escrows);
        for escrow in escrows {
            if escrow.release_tick > self.econ_tick {
                self.wage_escrows.push(escrow);
            } else {
                self.release_wage_escrow(escrow, report, wage_labor_used);
            }
        }
    }

    pub(super) fn release_wage_escrow(
        &mut self,
        escrow: WageEscrow,
        report: &mut EconTickReport,
        wage_labor_used: &mut Vec<(AgentId, u32)>,
    ) {
        self.release_wage_escrow_inner(escrow, None, report, wage_labor_used);
    }

    pub(super) fn release_wage_escrow_for_death(
        &mut self,
        escrow: WageEscrow,
        report: &mut EconTickReport,
        wage_labor_used: &mut Vec<(AgentId, u32)>,
    ) {
        self.release_wage_escrow_inner(escrow, Some(escrow.qty), report, wage_labor_used);
    }

    pub(super) fn release_wage_escrow_inner(
        &mut self,
        mut escrow: WageEscrow,
        delivered_override: Option<u32>,
        report: &mut EconTickReport,
        wage_labor_used: &mut Vec<(AgentId, u32)>,
    ) {
        let worker_live = self.society.agents.get(escrow.worker).is_some();
        let employer_live = self.society.agents.get(escrow.employer).is_some();
        let worker_dead = self.colonist_marked_dead(escrow.worker);
        let employer_dead = self.colonist_marked_dead(escrow.employer);
        let restore_retained = !employer_dead;
        if !worker_live || !employer_live {
            self.refund_wage_escrow_with_retained(escrow, restore_retained);
            return;
        }

        let delivered = delivered_override
            .or_else(|| self.wage_worker_available_labor(escrow.worker, escrow.qty))
            .unwrap_or(0)
            .min(escrow.qty);
        escrow.delivered = delivered;
        if delivered == 0 {
            self.refund_wage_escrow_with_retained(escrow, restore_retained);
            return;
        }

        let output_qty = prorate_u32_floor(escrow.output_qty, delivered, escrow.qty);
        if output_qty == 0 {
            self.refund_wage_escrow_with_retained(escrow, restore_retained);
            return;
        }
        let input = escrow.input.and_then(|(input, input_qty)| {
            let qty = prorate_u32_floor(input_qty, delivered, escrow.qty);
            (qty > 0).then_some((input, qty))
        });
        if !self.wage_contract_can_deliver(&escrow, output_qty, input) {
            self.refund_wage_escrow_with_retained(escrow, restore_retained);
            return;
        }

        if let Some((input, input_qty)) = input {
            if !self.society.debit_stock(escrow.employer, input, input_qty) {
                self.refund_wage_escrow_with_retained(escrow, restore_retained);
                return;
            }
            *report.consumed_as_input.entry(input).or_insert(0) += u64::from(input_qty);
        }
        if !self
            .society
            .credit_stock(escrow.employer, escrow.output_good, output_qty)
        {
            if let Some((input, input_qty)) = input {
                let _ = self.society.credit_stock(escrow.employer, input, input_qty);
                let booked = report.consumed_as_input.entry(input).or_insert(0);
                *booked = booked.saturating_sub(u64::from(input_qty));
            }
            self.refund_wage_escrow_with_retained(escrow, restore_retained);
            return;
        }
        *report.produced.entry(escrow.output_good).or_insert(0) += u64::from(output_qty);
        if Some(escrow.output_good) == self.provenance_bread_good() {
            if self.bread_provenance_active() {
                let lineage = self.is_lineage_agent(escrow.employer);
                self.bread_provenance.credit_produced(
                    escrow.employer,
                    u64::from(output_qty),
                    lineage,
                );
            }
            if self.acquisition_ledger_active() {
                self.acquisition.credit(
                    escrow.employer,
                    FoodChannel::SelfProduced,
                    u64::from(output_qty),
                );
            }
        }

        let earned = prorate_gold_floor(escrow.amount, delivered, escrow.qty);
        let refund = escrow.amount.saturating_sub(earned);
        let retained_earned = prorate_gold_floor(escrow.retained_funded, delivered, escrow.qty);
        let retained_refund = escrow.retained_funded.saturating_sub(retained_earned);
        let endowment_earned = prorate_gold_floor(escrow.endowment_funded, delivered, escrow.qty);

        if self.credit_from_wage_escrow(escrow.worker, earned) {
            if !worker_dead {
                self.credit_wage_proceeds(escrow.worker, earned);
            }
            self.wage_endowment_funded_wages = self
                .wage_endowment_funded_wages
                .saturating_add(endowment_earned);
        }
        if refund > Gold::ZERO {
            let _ = self.credit_from_wage_escrow(escrow.employer, refund);
            if restore_retained {
                self.restore_wage_retained_earnings(escrow.employer, retained_refund);
            }
        }
        // Defer the labor accounting until after the market step (see the call site): the step
        // clears `tick_labor_used` at entry, so recording here would be erased.
        if !worker_dead {
            wage_labor_used.push((escrow.worker, delivered));
        }
    }

    pub(super) fn wage_contract_can_deliver(
        &self,
        escrow: &WageEscrow,
        output_qty: u32,
        input: Option<(GoodId, u32)>,
    ) -> bool {
        let Some(employer) = self.society.agents.get(escrow.employer) else {
            return false;
        };
        if employer
            .stock
            .get(escrow.output_good)
            .checked_add(output_qty)
            .is_none()
        {
            return false;
        }
        match input {
            Some((input, input_qty)) => {
                self.society
                    .free_stock_after_all_reserves(escrow.employer, input)
                    >= input_qty
            }
            None => true,
        }
    }

    pub(super) fn refund_wage_escrow(&mut self, escrow: WageEscrow) {
        self.refund_wage_escrow_with_retained(escrow, true);
    }

    pub(super) fn refund_wage_escrow_with_retained(
        &mut self,
        escrow: WageEscrow,
        restore_retained: bool,
    ) {
        let _ = self.credit_from_wage_escrow(escrow.employer, escrow.amount);
        if restore_retained {
            self.restore_wage_retained_earnings(escrow.employer, escrow.retained_funded);
        }
    }

    pub(super) fn credit_wage_proceeds(&mut self, worker: AgentId, amount: Gold) {
        if amount == Gold::ZERO {
            return;
        }
        self.wage_proceeds_buckets
            .entry(worker)
            .or_default()
            .push_back(WageProceedsLot { amount });
    }

    pub(super) fn settle_wage_labor_for_death(
        &mut self,
        dead: AgentId,
        report: &mut EconTickReport,
        wage_labor_used: &mut Vec<(AgentId, u32)>,
    ) {
        self.wage_retained_earnings.remove(&dead);
        self.wage_proceeds_buckets.remove(&dead);
        if self.wage_escrows.is_empty() {
            return;
        }
        let escrows = std::mem::take(&mut self.wage_escrows);
        for escrow in escrows {
            if escrow.employer == dead {
                if escrow.release_tick <= self.econ_tick {
                    self.release_wage_escrow_for_death(escrow, report, wage_labor_used);
                    continue;
                }
                // Dead employer (§4.5): the escrowed wage already left its balance, so route it
                // back to the still-present dying agent's gold and let the estate machinery
                // (`collect_estate`, called right after this) carry it to the heir/commons. Its
                // wage-eligible retained-earnings are DISCARDED on death (§4.6) — do NOT restore
                // them (that would re-insert a stale ledger entry for a non-live owner after the
                // map entry was just dropped above).
                let _ = self.credit_from_wage_escrow(escrow.employer, escrow.amount);
            } else if escrow.worker == dead {
                if escrow.release_tick <= self.econ_tick {
                    self.release_wage_escrow_for_death(escrow, report, wage_labor_used);
                    continue;
                }
                // Dead worker before delivery (§4.5): the full escrowed wage refunds to the
                // living employer, which keeps its earned wage-eligibility (retained restored).
                self.refund_wage_escrow(escrow);
            } else {
                self.wage_escrows.push(escrow);
            }
        }
    }

    pub(super) fn credit_wage_retained_earnings_from_sale(
        &mut self,
        seller: AgentId,
        proceeds: Gold,
    ) {
        if proceeds == Gold::ZERO || !self.wage_labor_active() {
            return;
        }
        let cap = self.wage_retained_earnings_cap(seller);
        let current = self
            .wage_retained_earnings
            .get(&seller)
            .copied()
            .unwrap_or(Gold::ZERO)
            .min(cap);
        let credit = proceeds.min(cap.saturating_sub(current));
        let next = current.saturating_add(credit);
        if next > Gold::ZERO {
            self.wage_retained_earnings.insert(seller, next);
        } else {
            self.wage_retained_earnings.remove(&seller);
        }
    }

    pub(super) fn wage_retained_earnings_cap(&self, owner: AgentId) -> Gold {
        let free = self.society.free_gold_after_all_reserves(owner);
        let held = self
            .society
            .agents
            .get(owner)
            .map_or(Gold::ZERO, |agent| agent.gold);
        free.min(held)
    }

    pub(super) fn run_wage_labor_market_attribution(&mut self, spot_trades_start: usize) {
        if !self.wage_labor_active() || self.current_money_good() != Some(SALT) {
            return;
        }
        let Some(bread) = self.provenance_bread_good() else {
            return;
        };
        // Read the full spot-trade suffix in order: money is fungible, so a wage recipient's
        // bucket must be retired by EVERY purchase, not only its output (bread) buys.
        let trades: Vec<_> = self.society.trades[spot_trades_start..].to_vec();
        for trade in trades {
            let Some(payment) = gold_mul_qty(trade.price, trade.qty) else {
                continue;
            };
            // Owners fund wages; they do not earn them. Money is fungible, so the conservative
            // anti-subsidy rule (§4.6) is that an owner's own purchases spend down its realized
            // sale proceeds FIRST: every owner buy debits `wage_retained_earnings`, so a later
            // voluntary hire can never be booked as retained-funded from proceeds that were
            // already spent on goods (which would let endowment gold masquerade as earned wage
            // capital). Owner buys still neither retire a worker bucket nor count toward the flow.
            if self.private_land_agent_holds_any_plot(trade.buyer) {
                self.debit_wage_retained_earnings(trade.buyer, payment);
                continue;
            }
            // Debit the FIFO wage-proceeds bucket for this purchase whatever the good: wage
            // income spent on a non-output good is retired here, so a later bread buy financed
            // from the depleting one-time endowment is not misattributed as wage-financed.
            let wage_paid = self.debit_wage_proceeds_fifo(trade.buyer, payment);
            if trade.good != bread {
                continue;
            }
            // Only OUTPUT (bread) buys drive the circular-flow metric: `nonowner_output_buys`
            // is the denominator, `wage_financed_output_buys` the wage-derived numerator.
            self.wage_nonowner_output_buys = self.wage_nonowner_output_buys.saturating_add(payment);
            if wage_paid > Gold::ZERO {
                self.wage_financed_output_buys =
                    self.wage_financed_output_buys.saturating_add(wage_paid);
                if self.current_or_ever_landowner(trade.seller) {
                    self.wage_circular_loop_turnovers =
                        self.wage_circular_loop_turnovers.saturating_add(1);
                }
            }
        }
    }

    pub(super) fn debit_wage_proceeds_fifo(&mut self, buyer: AgentId, mut amount: Gold) -> Gold {
        let Some(bucket) = self.wage_proceeds_buckets.get_mut(&buyer) else {
            return Gold::ZERO;
        };
        let mut debited = Gold::ZERO;
        while amount > Gold::ZERO {
            let Some(front) = bucket.front_mut() else {
                break;
            };
            let take = front.amount.min(amount);
            front.amount = front.amount.saturating_sub(take);
            amount = amount.saturating_sub(take);
            debited = debited.saturating_add(take);
            if front.amount == Gold::ZERO {
                bucket.pop_front();
            }
        }
        if bucket.is_empty() {
            self.wage_proceeds_buckets.remove(&buyer);
        }
        debited
    }

    pub(super) fn wage_labor_active(&self) -> bool {
        self.chain
            .as_ref()
            .is_some_and(chain_runtime_wage_labor_active)
            && self.provenance_bread_good().is_some()
    }

    pub(super) fn wage_labor_market_open(&self) -> bool {
        self.wage_labor_active() && self.current_money_good() == Some(SALT)
    }

    /// Total **fiat** paid out as wages over the run (summed from econ's wage-payment
    /// audit). Positive when fiat wages are legal tender and the employers hold fiat;
    /// `Gold::ZERO` when wages are specie-only (the fiat is refused, so no wage trade
    /// settles in fiat). The wage surface's composition signal — gating, not totals.
    pub fn wage_fiat_settled(&self) -> Gold {
        self.society
            .wage_payment_audit
            .iter()
            .fold(Gold::ZERO, |sum, row| sum.saturating_add(row.public_fiat))
    }

    /// Total **specie** paid out as wages over the run (summed from the wage-payment
    /// audit).
    pub fn wage_specie_settled(&self) -> Gold {
        self.society
            .wage_payment_audit
            .iter()
            .fold(Gold::ZERO, |sum, row| sum.saturating_add(row.public_specie))
    }

    pub fn wage_labor_stats(&self) -> WageLaborStats {
        WageLaborStats {
            escrow_gold: self.wage_escrow_gold,
            open_escrows: self.wage_escrows.len(),
            retained_earnings_total: self
                .wage_retained_earnings
                .values()
                .copied()
                .fold(Gold::ZERO, |sum, amount| sum.saturating_add(amount)),
            wage_proceeds_bucket_total: self
                .wage_proceeds_buckets
                .values()
                .flat_map(|bucket| bucket.iter())
                .fold(Gold::ZERO, |sum, lot| sum.saturating_add(lot.amount)),
            hires_total: self.wage_hires_total,
            hires_post_promotion: self.wage_hires_post_promotion,
            distinct_workers: self.wage_workers_ever.len(),
            distinct_employers: self.wage_employers_ever.len(),
            below_ask_not_hired: self.wage_below_ask_not_hired,
            endowment_funded_wages: self.wage_endowment_funded_wages,
            wage_financed_output_buys: self.wage_financed_output_buys,
            nonowner_output_buys: self.wage_nonowner_output_buys,
            circular_loop_turnovers: self.wage_circular_loop_turnovers,
        }
    }

    pub fn wage_labor_escrow_balanced(&self) -> bool {
        self.wage_escrow_gold == Gold::ZERO && self.wage_escrows.is_empty()
    }

    /// Close out every still-open wage escrow by refunding it to its employer — the
    /// accounting-period close the acceptance harness runs at the horizon (§5: "every wage
    /// entering escrow must be released or refunded by horizon end"). A hire on the final tick
    /// opens an escrow whose one-tick release lag (`release_tick = econ_tick + 1`) falls past
    /// the run's last tick, so it would otherwise linger and misreport a genuinely-live market
    /// as `EscrowUnbalanced`. Refunding is conserved (escrow gold → employer gold), so the
    /// per-tick money invariant still holds; the wage was already counted when the escrow
    /// opened, and the metrics window has closed, so this distorts no headline figure.
    pub fn settle_open_wage_escrows_at_horizon(&mut self) {
        let escrows = std::mem::take(&mut self.wage_escrows);
        for escrow in escrows {
            self.refund_wage_escrow(escrow);
        }
    }
}
