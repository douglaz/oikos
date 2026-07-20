//! Econ-tick phase implementations.
//!
//! The `run_*` methods — one per phase of the economic tick (fast loop, production,
//! subsistence, markets, capital formation, imitation dynamics, births, banking, ...).
//! Extracted verbatim from `mod.rs` (pure code motion) into this sibling
//! `impl Settlement` block; the module-private methods become `pub(super)` — the exact
//! scope they already had inside `settlement`. The load-bearing phase ORDER is not
//! here: `econ_tick` in `mod.rs` remains the only place the pipeline is sequenced,
//! and its call order is untouched.

use super::*;

impl Settlement {
    /// The G8b bank phase: **deposits** then **fiduciary lending**, both routed
    /// through econ's existing M3 ledger / bank balance-sheet paths — no bank logic
    /// is added to econ. A no-op without a chartered bank, so every pre-G8b run is
    /// byte-identical.
    ///
    /// **Deposit.** Each living consumer moves `min(deposit_per_tick, its specie)`
    /// of M3 specie into the bank. [`MoneySystem::issue_demand_claim`] with
    /// `backed_by_reserves == amount` debits the depositor's specie, credits the
    /// ledger's bank reserves, and gives the depositor an equal demand claim;
    /// [`Bank::credit_reserves`] and `demand_deposits` mirror the move on the bank's
    /// balance sheet (so `sum(bank.reserves) == ledger bank_reserves` stays true).
    /// The depositor's spendable total is unchanged — specie became a claim — so the
    /// claim circulates as money in the specie's place.
    ///
    /// **Lend fiduciary.** The bank lends up to econ's
    /// [`Bank::fiduciary_lend_capacity`] for the regime, capped by a sim-side
    /// depositor-death redemption buffer, and split across the living gatherers
    /// (deterministically; the remainder lands on the lowest-id borrowers).
    /// `issue_demand_claim` with `backed_by_reserves == 0` issues claims **beyond**
    /// reserves — the ledger tracks them as `fiduciary = demand_claims −
    /// bank_reserves` — and [`Bank::record_fiduciary_loan`] books the loan. A
    /// 100%-reserve bank's capacity is zero, so the control lends nothing while its
    /// deposits still circulate. The buffer is game-only: it preserves enough excess
    /// reserves that a future depositor death can redeem the protected claims without
    /// taking the bank below its configured reserve ratio.
    ///
    /// Returns the fiduciary credit issued in this sim-side phase so the current M3
    /// record can expose it through econ's existing `bank_credit_issued` column.
    ///
    /// Deterministic: integer amounts, slot-ordered rosters, nothing drawn.
    pub(super) fn run_bank_phase(&mut self) -> Gold {
        let Some(bank_cfg) = self.bank else {
            return Gold::ZERO;
        };
        let regime = self.society.regime();
        let Some(bank_pos) = self
            .society
            .banks
            .iter()
            .position(|bank| bank.id == BANK_ID)
        else {
            return Gold::ZERO;
        };

        // Disjoint borrows: the live roster (read) and the society's ledger + bank
        // balance sheet (mutated). Borrowing the roster in place lets the deposit/lend
        // loops walk it in slot order — depositors are the living consumers, borrowers
        // the living gatherers — without collecting either into a fresh `Vec` each tick.
        let live_slots = &self.live_colonist_slots;
        let colonists = &self.colonists;
        let society = &mut self.society;
        let tick = society.tick;
        let Some(money_system) = society.money_system.as_mut() else {
            return Gold::ZERO;
        };
        let bank = &mut society.banks[bank_pos];
        let mut bank_credit_receipts = Vec::new();

        // ---- Deposit: each living consumer moves specie -> reserves + a demand claim.
        for &slot in live_slots {
            let colonist = &colonists[slot];
            if colonist.vocation != Vocation::Consumer {
                continue;
            }
            let depositor = colonist.id;
            let specie = money_system
                .balance_snapshot(depositor)
                .map(|balance| balance.public_specie)
                .unwrap_or(Gold::ZERO);
            let amount = bank_cfg.deposit_per_tick.min(specie);
            if amount == Gold::ZERO {
                continue;
            }
            money_system
                .issue_demand_claim(BANK_ID, depositor, amount, amount)
                .expect("a deposit bounded by the depositor's specie must succeed");
            bank.credit_reserves(amount)
                .expect("crediting bank reserves cannot overflow for a bounded deposit");
            bank.demand_deposits = bank
                .demand_deposits
                .checked_add(amount)
                .expect("bank demand deposits cannot overflow for a bounded deposit");
        }

        let protected_depositor_claims = live_slots
            .iter()
            .filter(|&&slot| colonists[slot].vocation == Vocation::Consumer)
            .map(|&slot| money_system.demand_claim_on(colonists[slot].id, BANK_ID))
            .try_fold(Gold::ZERO, Gold::checked_add)
            .expect("bounded G8b depositor claims cannot overflow");

        // ---- Lend fiduciary: the reserve-gated capacity that still leaves room for a
        // future depositor-death redemption, split evenly across the living gatherers in
        // slot order (the remainder lands on the lowest-slot borrowers). Zero for a
        // 100%-reserve bank (the control).
        let capacity = Self::fiduciary_lend_capacity_preserving_redemption(
            bank,
            regime,
            Gold::ZERO,
            protected_depositor_claims,
        );
        let borrower_count = live_slots
            .iter()
            .filter(|&&slot| colonists[slot].vocation == Vocation::Gatherer)
            .count() as u64;
        let mut issued_this_tick = Gold::ZERO;
        if capacity > Gold::ZERO && borrower_count > 0 {
            let base = capacity.0 / borrower_count;
            let extra = capacity.0 % borrower_count;
            let mut borrower_index: u64 = 0;
            for &slot in live_slots {
                let colonist = &colonists[slot];
                if colonist.vocation != Vocation::Gatherer {
                    continue;
                }
                let share = base + u64::from(borrower_index < extra);
                borrower_index += 1;
                if share == 0 {
                    continue;
                }
                let amount = Gold(share);
                // Defensive backstop, never fires for the even split above: the shares
                // sum to exactly the pre-computed `capacity` (base*borrowers + extra),
                // so each `amount` is within the remaining `capacity - issued_this_tick`.
                // The check re-derives the live capacity from the *mutated* balance sheet
                // (booking a fiduciary loan grows `demand_deposits`, shrinking the
                // convertible deposit capacity unit-for-unit) so the bank's reserve-gated
                // per-tick cap can never be breached even if the split logic later changes.
                if Self::fiduciary_lend_capacity_preserving_redemption(
                    bank,
                    regime,
                    issued_this_tick,
                    protected_depositor_claims,
                ) < amount
                {
                    break;
                }
                money_system
                    .issue_demand_claim(BANK_ID, colonist.id, amount, Gold::ZERO)
                    .expect("a fiduciary issue within capacity must succeed");
                bank.record_fiduciary_loan(regime, amount)
                    .expect("recording a fiduciary loan within capacity must succeed");
                bank_credit_receipts.push(CantillonReceipt {
                    tick,
                    agent: colonist.id,
                    amount,
                    source: CreditSource::BankFiduciary(BANK_ID),
                });
                issued_this_tick = issued_this_tick
                    .checked_add(amount)
                    .expect("prechecked fiduciary issuance cannot overflow");
            }
        }

        // Reconcile the agents' spendable-money caches to the mutated ledger so the
        // market this tick reads the new specie/claims and the money invariant holds.
        money_system.reconcile_agent_cache(society.agents.as_mut_slice());
        society.cantillon_receipts.extend(bank_credit_receipts);
        issued_this_tick
    }
    /// Run [`FAST_TICKS_PER_ECON_TICK`] `world` ticks, keeping idle living
    /// gatherers busy (harvest → exchange), and return the per-colonist,
    /// per-good amounts deposited into the exchange stockpile this interval plus
    /// the agents that actually completed a forage task.
    ///
    /// Deposits are detected as carry **decreases**: a gatherer only ever
    /// deposits at the exchange and harvests at its node, and `world.tick` runs
    /// at most one arrival action per agent per tick, so a per-tick carry drop is
    /// exactly a deposit (the accepted amount — overflow stays carried). Escrow
    /// carried over from a previous interval is part of the opening carry, so it
    /// transfers on the arrival that finally lands it.
    pub(super) fn run_fast_loop(&mut self) -> FastLoopReport {
        let mut deposited: BTreeMap<(AgentId, GoodId), u32> = BTreeMap::new();
        let mut foraged: BTreeSet<AgentId> = BTreeSet::new();
        let detect_forage = self.own_labor_subsistence_can_run();
        // S15: a cultivator harvests grain whose deposit may LAND a tick later (the haul
        // straddles the econ-tick boundary), by which point its `cultivating` flag may
        // have flipped off (the hysteresis). So on the cultivation path also attribute a
        // colonist that is currently CARRYING — any carry decrease is a deposit to the
        // exchange regardless of the flag, so this keeps the in-flight grain accounted.
        // Gated, so every non-cultivation run keeps the S14 flag-only attribution
        // (byte-identical).
        let cultivation_active = self.own_use_cultivation_active();
        // Opening carry baseline (the current escrow), per attributed colonist/good.
        // S14: a Gatherer deposits its harvested WOOD/grain, but a *forager* (a Consumer
        // or Unassigned colonist marked `foraging`, including a spatial lineage member)
        // deposits harvested FORAGE on the commons path — so the attribution must cover
        // both, or a forager's FORAGE would carry/deposit but never transfer to econ. ONE
        // predicate gates both the snapshot here and the carry-delta loop below, so a
        // Gatherer that also forages is counted once. Non-spatial colonists hold no world
        // carry, so the predicate is harmless for them; off the own-labor path no colonist
        // is `foraging`, so this reduces to the pre-S14 Gatherer-only set (byte-identical).
        let mut prev_carry: BTreeMap<(AgentId, GoodId), u32> = BTreeMap::new();
        // S15: ids that are CARRYING at the snapshot but are not flag-attributed — a
        // cultivator whose grain deposit lands this interval though its `cultivating`
        // flag has since flipped off. Captured by snapshot carry (not the post-deposit
        // carry, which is already drained), so the in-flight grain is still attributed.
        // Empty off the cultivation path, so the carry-delta loop reduces to the S14
        // flag-only set (byte-identical).
        let mut carrying_ids: BTreeSet<AgentId> = BTreeSet::new();
        for &slot in &self.live_colonist_slots {
            let colonist = &self.colonists[slot];
            let flagged = Self::carry_is_forage_attributed(colonist)
                || self.share_worker_has_contract(colonist.id)
                || self.in_kind_worker_has_contract(colonist.id);
            let carrying =
                cultivation_active && !flagged && self.world.agent_carry_total(colonist.id) > 0;
            if flagged || carrying {
                if carrying {
                    carrying_ids.insert(colonist.id);
                }
                for &good in &self.goods {
                    prev_carry.insert(
                        (colonist.id, good),
                        self.world.agent_carry(colonist.id, good),
                    );
                }
            }
        }
        // Exchange contents before the interval. Transfer runs *after* this
        // loop, so the only thing that changes exchange contents here is
        // deposits — letting us cross-check our carry-delta attribution against
        // the stockpile's own ledger below (debug only), even when prior clipped
        // deposits are still waiting there.
        #[cfg(debug_assertions)]
        let exchange_before: BTreeMap<GoodId, u32> = self
            .goods
            .iter()
            .map(|&g| (g, self.world.stockpile_get(self.exchange, g)))
            .collect();

        // C1R: an INDEPENDENT carry tracker for contracted share workers, so the
        // `unattributed_share_deposit` guard has a real detection to fire from (spec-review
        // P2 / the S22e vacuous-metric lesson: a counter with no increment site guards
        // nothing). The attribution scan below is predicate-gated; this tracker follows
        // every contracted worker unconditionally, and the end-of-loop reconciliation
        // charges any carry drop the predicate-gated scan failed to record. Contracts
        // cannot change inside the fast loop (matching/expiry run in the econ phase), so
        // the set is stable. Runtime-only, never digested; empty (and byte-inert) with the
        // flag off.
        let share_scan: Vec<AgentId> = self
            .share_contracts
            .iter()
            .map(|contract| contract.worker)
            .filter(|&worker| self.private_land_live_agent(worker))
            .collect();
        let mut share_prev: BTreeMap<(AgentId, GoodId), u32> = BTreeMap::new();
        let mut share_drops: BTreeMap<(AgentId, GoodId), u32> = BTreeMap::new();
        for &worker in &share_scan {
            for &good in &self.goods {
                share_prev.insert((worker, good), self.world.agent_carry(worker, good));
            }
        }
        let in_kind_scan: Vec<AgentId> = self
            .in_kind_contracts
            .iter()
            .map(|contract| contract.worker)
            .filter(|&worker| self.private_land_live_agent(worker))
            .collect();
        let mut in_kind_prev: BTreeMap<(AgentId, GoodId), u32> = BTreeMap::new();
        let mut in_kind_drops: BTreeMap<(AgentId, GoodId), u32> = BTreeMap::new();
        for &worker in &in_kind_scan {
            for &good in &self.goods {
                in_kind_prev.insert((worker, good), self.world.agent_carry(worker, good));
            }
        }

        self.idle_open_wage_workers();
        for _ in 0..FAST_TICKS_PER_ECON_TICK {
            self.assign_idle_gatherer_tasks();
            self.steer_share_contract_workers();
            self.steer_in_kind_contract_workers();
            self.private_land_validate_harvest_tasks();
            let land_harvest_before = self.private_land_harvest_snapshot();
            if detect_forage {
                let foraging_before: Vec<AgentId> = self
                    .live_colonist_slots
                    .iter()
                    .filter_map(|&slot| {
                        let id = self.colonists[slot].id;
                        matches!(self.world.agent_task(id), Some(Task::GoForage(_, _)))
                            .then_some(id)
                    })
                    .collect();
                self.world.tick();
                for id in foraging_before {
                    if self.world.agent_status(id) == Some(AgentStatus::Idle) {
                        foraged.insert(id);
                    }
                }
            } else {
                self.world.tick();
            }
            let worked_land = self.private_land_worked_events(&land_harvest_before);
            self.private_land_apply_worked_events(&worked_land);
            self.private_land_advance_idle_counters(&worked_land);
            self.check_share_stock_drawdown(&worked_land);
            self.check_in_kind_stock_drawdown(&worked_land);
            for &slot in &self.live_colonist_slots {
                let colonist = &self.colonists[slot];
                if !Self::carry_is_forage_attributed(colonist)
                    && !self.share_worker_has_contract(colonist.id)
                    && !self.in_kind_worker_has_contract(colonist.id)
                    && !carrying_ids.contains(&colonist.id)
                {
                    continue;
                }
                for &good in &self.goods {
                    let now = self.world.agent_carry(colonist.id, good);
                    let prev = prev_carry.get(&(colonist.id, good)).copied().unwrap_or(0);
                    if now < prev {
                        *deposited.entry((colonist.id, good)).or_insert(0) += prev - now;
                    }
                    prev_carry.insert((colonist.id, good), now);
                }
            }
            // C1R guard tracking: record every contracted worker's carry drops independently
            // of the attribution predicate above (reconciled after the loop).
            for &worker in &share_scan {
                for &good in &self.goods {
                    let now = self.world.agent_carry(worker, good);
                    let prev = share_prev.insert((worker, good), now).unwrap_or(0);
                    if now < prev {
                        *share_drops.entry((worker, good)).or_insert(0) += prev - now;
                    }
                }
            }
            for &worker in &in_kind_scan {
                for &good in &self.goods {
                    let now = self.world.agent_carry(worker, good);
                    let prev = in_kind_prev.insert((worker, good), now).unwrap_or(0);
                    if now < prev {
                        *in_kind_drops.entry((worker, good)).or_insert(0) += prev - now;
                    }
                }
            }
        }

        // C1R: any contracted worker's carry drop the attribution scan did not record is an
        // unattributed contract deposit — the guard the suite asserts to 0. Fires exactly
        // when the deposit-attribution predicate regresses to exclude share workers.
        for (&(worker, good), &dropped) in &share_drops {
            let recorded = deposited.get(&(worker, good)).copied().unwrap_or(0);
            if dropped > recorded {
                self.share_unattributed_share_deposit = self
                    .share_unattributed_share_deposit
                    .saturating_add(u64::from(dropped - recorded));
            }
        }
        for (&(worker, good), &dropped) in &in_kind_drops {
            let recorded = deposited.get(&(worker, good)).copied().unwrap_or(0);
            if dropped > recorded {
                self.in_kind_unattributed_deposit = self
                    .in_kind_unattributed_deposit
                    .saturating_add(u64::from(dropped - recorded));
            }
        }

        // Defend the deposit-attribution assumption: a carry decrease is taken to
        // be a deposit into the exchange, so the per-good carry drops we summed
        // must equal the exchange stockpile's actual increase over the interval
        // (it is the only stockpile, only living gatherers deposit, and transfer
        // runs after this loop). A future task that drained carry elsewhere would
        // break this equality and trip the check rather than silently misattribute.
        #[cfg(debug_assertions)]
        for &good in &self.goods {
            let increase = self
                .world
                .stockpile_get(self.exchange, good)
                .saturating_sub(exchange_before.get(&good).copied().unwrap_or(0));
            let mut attributed = 0u32;
            for (&(_, g), &q) in &deposited {
                if g == good {
                    attributed += q;
                }
            }
            debug_assert_eq!(
                attributed, increase,
                "carry-delta deposits must equal the exchange increase for {good:?}"
            );
        }

        FastLoopReport { deposited, foraged }
    }
    /// BIRTHS phase (G4b): each food-secure household, under its size cap and past its
    /// birth interval, bears one child. The newborn inherits its chosen parent's
    /// **mutated** culture (deterministic — a hash of the parent's culture and the
    /// colony's monotonic birth sequence, no `Rng`), is endowed by **conserved
    /// transfers** from that parent (a FOOD buffer it must hold plus a best-effort
    /// gold gift), and joins the society via [`Society::add_agent`] so it
    /// participates from the next econ tick. Returns the birth count. A no-op without
    /// a demography overlay.
    ///
    /// The birth is a **threshold rule**, not an optimizer: a household reproduces
    /// when it clears the need-security margin and can feed a child — the heritable
    /// ordinal patience bias does its selection work through the market
    /// (`regenerate_scale`), not a fitness function. The gold gift is best-effort
    /// (clamped to the parent's unreserved balance), so a gold-poor lineage still reproduces;
    /// poverty shapes a lineage's wealth, never its survival.
    pub(super) fn run_births(&mut self) -> u32 {
        // TEST-ONLY fault injection: a deliberate post-market mint into the commons,
        // proving the per-tick money identity spans the births phase (it must trip
        // `EconTickReport::money_conserves`). Compiled out of non-test builds.
        #[cfg(test)]
        {
            self.commons_gold = Gold(
                self.commons_gold
                    .0
                    .saturating_add(self.test_fault_mint_birth_gold),
            );
        }
        let Some(demo) = self.demography.clone() else {
            return 0;
        };
        let mut births = 0u32;
        for h in 0..demo.households.len() {
            let next_eligible = self.households[h]
                .last_birth_tick
                .map_or(demo.birth_interval, |t| t + demo.birth_interval);
            if self.econ_tick < next_eligible {
                self.birth_block_interval = self.birth_block_interval.saturating_add(1);
                continue;
            }

            // The household's living members (slots), in slot order.
            let member_slots: Vec<usize> = self
                .live_colonist_slots
                .iter()
                .copied()
                .filter(|&slot| self.colonists[slot].household == Some(h))
                .collect();
            if member_slots.is_empty() {
                continue; // extinct (no living member) — not a birth block, nothing to count
            }
            if member_slots.len() >= self.birth_cap_for_household(h, demo.max_household_size) {
                // At the size cap (the blowup bound / the artificial knob). On the
                // forage-commons path this should NOT be the binding stall — the hunger
                // ceiling should be — so this counter is the control diagnostic.
                self.birth_block_size_cap = self.birth_block_size_cap.saturating_add(1);
                continue;
            }

            // Need-security gate: every living member's hunger at or below the ceiling.
            // This is the **preventive check** — on the forage-commons path forage
            // scarcity raises hunger above the ceiling and stalls births here, so the
            // population plateaus at the carrying capacity (the load-bearing diagnostic).
            if !member_slots
                .iter()
                .all(|&slot| self.colonists[slot].need.hunger <= demo.birth_hunger_ceiling)
            {
                self.birth_block_hunger_ceiling = self.birth_block_hunger_ceiling.saturating_add(1);
                continue;
            }

            // Choose the parent: a member that can endow the child's food buffer,
            // preferring the wealthiest (most gold), ties broken to the lowest slot —
            // a fully deterministic choice. None can endow → skip. The endowment good is
            // the BIRTH-FOOD selector: the hunger staple off the forage-commons path
            // (FOOD on `lineages`, bread on the frontier), the FORAGE subsistence good
            // on it — so a fed-by-forage colony endows children from forage, not bread.
            // S15: on the cultivation path this BROADENS to any edible food the parent
            // holds (bread first, then forage), so cultivated bread can endow children.
            let mut food_buf = [self.known.hunger; 2];
            let foods = self.birth_food_options(&mut food_buf);
            let parent_slot = member_slots
                .iter()
                .copied()
                .filter(|&slot| {
                    let pid = self.colonists[slot].id;
                    self.parent_birth_food(pid, foods, demo.child_food_endowment)
                        .is_some()
                })
                .max_by_key(|&slot| {
                    let pid = self.colonists[slot].id;
                    let gold = self.society.free_gold_after_all_reserves(pid).0;
                    (gold, std::cmp::Reverse(slot))
                });
            // DH.b-obs: this producer household reached the endowment gate (interval/non-empty/
            // size-cap/hunger-ceiling passed). Capture the opportunity BEFORE the debit
            // (`event_end` excludes it); `recorded_pass` = a member can endow (a birth follows).
            self.birth_gate_obs_capture_opportunity(
                h,
                &member_slots,
                demo.child_food_endowment,
                parent_slot.is_some(),
            );
            let Some(parent_slot) = parent_slot else {
                // No member holds the child's food endowment. On the forage-commons path
                // the FORAGE selector keeps this rare (parents forage their own food);
                // a stall here means the forage flow could not even endow a child.
                self.birth_block_endowment = self.birth_block_endowment.saturating_add(1);
                continue;
            };

            let parent_id = self.colonists[parent_slot].id;
            // The endowment good the chosen parent actually holds (the first option in
            // preference order). With one option this is exactly the S14 `birth_food()`.
            let staple = self
                .parent_birth_food(parent_id, foods, demo.child_food_endowment)
                .expect("the parent was filtered for holding an endowment food");
            let parent_culture = self.colonists[parent_slot].culture;
            let parent_seed = self.colonists[parent_slot].seed;

            // The endowment: conserved TRANSFERS from the parent — the staple buffer
            // (required, already verified free after reservations) plus a best-effort
            // gold gift clamped to the parent's unreserved balance.
            if !self
                .society
                .debit_stock(parent_id, staple, demo.child_food_endowment)
            {
                continue; // guarded above; defensive
            }
            // DH.b-obs: append the post-debit BirthDebit event (tape completeness only — it is
            // past every opportunity's `event_end`, so it never enters a classification).
            self.birth_gate_obs_append_birth_debit(parent_id);
            let parent_gold = self.society.free_gold_after_all_reserves(parent_id).0;
            let gold_endow = demo.child_gold_endowment.min(parent_gold);

            // The child: inherited+mutated culture, a deterministic lifespan from its
            // own seed, the transferred endowment, and a fresh arena slot via add_agent.
            let birth_seq = self.birth_seq;
            self.birth_seq = self.birth_seq.saturating_add(1);
            let child_culture = parent_culture.inherit(birth_seq, demo.mutation_delta_bps);
            let cseed = child_seed(parent_seed, birth_seq);
            let lifespan = demo.lifespan_ticks(cseed);
            let need = NeedState::rested();
            let child_agent = build_newborn_agent(
                &need,
                &child_culture,
                &self.known,
                0,
                demo.child_food_endowment,
                staple,
            );
            let child_id = self.society.add_agent(child_agent);
            // S16: a bread birth endowment is a conserved TRANSFER — move the parent's drawn
            // produced origin to the newborn (so an inherited produced loaf stays produced).
            // Off the bread endowment (forage path) or off the ledger this is a no-op.
            if self.bread_provenance_active() && Some(staple) == self.provenance_bread_good() {
                self.bread_provenance.transfer(
                    parent_id,
                    child_id,
                    u64::from(demo.child_food_endowment),
                );
            }
            // S21d.1: the same conserved bread endowment transfer for the acquisition ledger —
            // origin preserved (an inherited bought loaf stays bought). A no-op off the bread
            // endowment / off the ledger.
            let mut burden_funding_lots: Vec<burden::BurdenLot> = Vec::new();
            if self.acquisition_ledger_active() && Some(staple) == self.acquisition_food_good() {
                let drawn = self.acquisition.transfer_preserve(
                    parent_id,
                    child_id,
                    u64::from(demo.child_food_endowment),
                );
                // C3R.e (impl-67): attribute the producer-house birth's funding by the exact drawn
                // parent→child endowment lots — criterion iii reads this window-diffed (market
                // funding = Bought/SelfProduced, no intervention-origin lot).
                if self.is_producer_household(h) {
                    for lot in &drawn {
                        self.producer_birth_funded_by_channel[lot.channel.index()] += lot.qty;
                        if lot.intervention {
                            self.producer_birth_funded_intervention += lot.qty;
                        }
                    }
                }
                if self.closure_active() {
                    burden_funding_lots = drawn.iter().map(burden::burden_lot_of).collect();
                }
            }
            // DH.b (impl-69): stream (b) — the per-birth funding record, emitted AFTER the exact
            // conserved lot transfer above (its own emission site, distinct from the
            // BirthOccurred stream below — R2-5). The class is the household's fixed closure
            // class; every household on the closed base carries one.
            if self.closure_active() {
                if let Some(&class) = self.closure.household_class.get(&h) {
                    self.burden.funding.push(burden::BurdenBirthFunding {
                        tick: self.econ_tick,
                        class,
                        parent: parent_id,
                        child: child_id,
                        q: demo.child_food_endowment,
                        lots: burden_funding_lots,
                    });
                }
            }
            if gold_endow > 0 {
                let transferred = self
                    .society
                    .transfer_gold(parent_id, child_id, Gold(gold_endow));
                debug_assert!(transferred, "the parent's gold gift must transfer");
                if transferred {
                    self.debit_earned_provisioning_gold(parent_id, Gold(gold_endow));
                    self.credit_earned_provisioning_lot(
                        child_id,
                        EarnedGoldLot {
                            source: EarnedGoldSource::Endowed,
                            amount: Gold(gold_endow),
                        },
                    );
                }
            }
            let fixed_commitment_norm =
                self.fixed_commitment_norm_prevalence()
                    .is_some_and(|prevalence| {
                        fixed_commitment_norm_seeded(self.generation_seed, child_id, prevalence)
                    });

            self.colonists.push(Colonist {
                id: child_id,
                vocation: Vocation::Consumer,
                node: None,
                // A newborn is a hearth-fed lineage member, never re-entered.
                home_vocation: Vocation::Consumer,
                home_node: None,
                need,
                culture: child_culture,
                critical_streak: 0,
                alive: true,
                latent: None,
                household: Some(h),
                parent: Some(parent_id),
                age: 0,
                lifespan: Some(lifespan),
                seed: cseed,
                estate_destination: None,
                acquired_tool: false,
                foraging: false,
                cultivating: false,
                cultivate_pressure: 0,
                cultivation_stock_pending: false,
                cultivation_skill: 0,
                cultivation_return_window: VecDeque::new(),
                cultivation_tenure: 0,
                commitment_remaining: 0,
                commitment_renewals: 0,
                adopts_commitment_norm: fixed_commitment_norm,
                next_norm_bit: None,
                commitment_norm_seed_adopter: fixed_commitment_norm,
                commitment_norm_observations: VecDeque::new(),
                carried_grain_source: None,
                carried_share_contract_id: None,
                carried_in_kind_contract_id: None,
            });
            let child_slot = self.colonists.len() - 1;
            self.live_colonist_slots.push(child_slot);
            self.colonist_slot_by_id.insert(child_id, child_slot);
            // DH.a: register the newborn (household → class) and record the conserved staple + gold
            // endowment as bucket-preserving transfers (rule 5).
            self.closure_note_birth(
                parent_id,
                child_id,
                h,
                staple,
                demo.child_food_endowment,
                Gold(gold_endow),
            );
            // DH.b (impl-69): stream (a) — `BirthOccurred` at successful newborn INSERTION into a
            // fixed Miller/Baker closure class (the qualifying-birth definition; `birth_id` = the
            // child `AgentId`). Read back through the registry the note above just updated — a
            // derivation independent of the funding record's `household_class` lookup, so the
            // completeness equality between the two streams is not tautological (R2-5).
            if self.closure_active() {
                if let Some(
                    class @ (closure::ClosureClass::Miller | closure::ClosureClass::Baker),
                ) = self.closure_class_of(child_id)
                {
                    self.burden.births.push(burden::BurdenBirthOccurred {
                        tick: self.econ_tick,
                        class,
                        parent: parent_id,
                        child: child_id,
                    });
                }
            }
            // S13: a spatial-households newborn gets a world agent at its EXACT econ id
            // (a reused arena `slot#gen` after a death recycled the slot), so
            // world_id == econ_id holds mid-run too. The slot's prior world occupant was
            // removed on death (`collect_estate` → `world.remove_agent`), so the mirror
            // insert never collides. Placement at the exchange tile is always passable.
            if self.spatial_households_active() {
                let exchange_pos = self
                    .world
                    .stockpile(self.exchange)
                    .expect("the exchange stockpile exists")
                    .pos;
                let placed = self
                    .world
                    .add_agent_with_id(child_id, exchange_pos, self.carry_cap, self.move_speed)
                    .expect("a newborn world agent mirrors its freed-or-fresh econ slot");
                debug_assert_eq!(placed, child_id, "newborn world and econ ids must coincide");
            }
            self.households[h].last_birth_tick = Some(self.econ_tick);
            self.births_total = self.births_total.saturating_add(1);
            self.birth_stock_births_by_household[h] =
                self.birth_stock_births_by_household[h].saturating_add(1);
            if self.is_producer_household(h) {
                self.producer_house_births = self.producer_house_births.saturating_add(1);
            }
            births += 1;
        }
        births
    }
    /// PRODUCTION phase (G3a): each living producer applies its recipe to the
    /// input it holds, up to the throughput cap, recording the conserved
    /// conversion (input consumed, output produced) into `report`. A no-op
    /// without a chain. Deterministic: id-ordered, no RNG, integer state.
    pub(super) fn run_production(&mut self, report: &mut EconTickReport) {
        let Some(chain) = &self.chain else {
            return;
        };
        let throughput = chain.throughput;
        let mill_recipe = chain.content.mill_recipe().id;
        let bake_recipe = chain.content.bake_recipe().id;
        // S16: a chain baker's bread is PRODUCED — credit the provenance ledger so a
        // baker-supplied bread→medium trade is attributed produced like a cultivator's.
        let provenance_bread = if self.bread_provenance_active() {
            self.provenance_bread_good()
        } else {
            None
        };
        // S21d.1: a baker's bread is SELF-PRODUCED — credit the acquisition ledger so the
        // post-promotion chain output is attributed self-produced (distinct from bought/seeded).
        let acquisition_bread = if self.acquisition_ledger_active() {
            self.acquisition_food_good()
        } else {
            None
        };
        // G6b content recipes (`None` for a plain G3a/G3b/G5b chain).
        let research_recipe = chain.content.research_recipe().map(|recipe| recipe.id);
        let confect_recipe = chain.content.tier2_recipe().map(|recipe| recipe.id);
        let cycle_a_recipe = chain.content.cycle_a_recipe().map(|recipe| recipe.id);
        let cycle_b_recipe = chain.content.cycle_b_recipe().map(|recipe| recipe.id);
        let cycle_c_recipe = chain.content.cycle_c_recipe().map(|recipe| recipe.id);
        // DH.a (P1-1): the recipe seam gate. Precomputed so the emit inside the id-loop is a partial
        // borrow of `self.closure` alone (never a `&mut self` method), disjoint from the
        // `&self.live_colonist_slots` iterator.
        let closure_on = self.closure_active();
        let closure_tick = self.econ_tick;
        // DH.b-obs: precomputed like `closure_on` above so the per-execution Production append
        // inside the id-loop is a partial borrow (`self.birth_gate_obs`/`self.society`), never a
        // `&mut self` method that would conflict with the `&self.live_colonist_slots` iterator.
        let birth_gate_obs_on = self.birth_gate_obs_active();
        let birth_gate_staple = self.birth_food();
        // `chain`/`colonists` (immutable) and `society` (mutable) are disjoint
        // fields, so id-ordered iteration here borrows them side by side. The
        // recipe ids are content data; mutation delegates to econ's existing
        // direct-recipe executor through an additive `Society` accessor.
        for &slot in &self.live_colonist_slots {
            let id = self.colonists[slot].id;
            let (recipe_id, is_research) = match self.colonists[slot].vocation {
                Vocation::Miller => (mill_recipe, false),
                Vocation::Baker => (bake_recipe, false),
                // G6b: a scholar runs research → Knowledge (drained to the counter); a
                // confectioner runs the tier-2 recipe → pastry. Skip if the content
                // carries no such recipe (a non-research chain).
                Vocation::Scholar => match research_recipe {
                    Some(recipe) => (recipe, true),
                    None => continue,
                },
                Vocation::Confectioner => match confect_recipe {
                    Some(recipe) => (recipe, false),
                    None => continue,
                },
                Vocation::CycleA => match cycle_a_recipe {
                    Some(recipe) => (recipe, false),
                    None => continue,
                },
                Vocation::CycleB => match cycle_b_recipe {
                    Some(recipe) => (recipe, false),
                    None => continue,
                },
                Vocation::CycleC => match cycle_c_recipe {
                    Some(recipe) => (recipe, false),
                    None => continue,
                },
                // A latent (Unassigned) colonist holds a tool but has not adopted
                // production, so it mills/bakes nothing until the spread makes it a
                // Miller/Baker (the role-choice phase sets that before production).
                Vocation::Gatherer | Vocation::Consumer | Vocation::Unassigned => continue,
            };
            for _ in 0..throughput {
                // The tier gate: `execute_direct_recipe_for_agent_checked` returns
                // `None` for a DISABLED recipe (the executor honors `Recipe.enabled`),
                // so a confectioner produces nothing while tier 2 is locked even while
                // holding its flour input — the G6b tier-gate test.
                let Some(applied) = self
                    .society
                    .execute_direct_recipe_for_agent_checked(id, recipe_id)
                else {
                    // Out of input, missing tool, or a gated recipe: nothing more.
                    break;
                };
                // DH.b-obs: one Production event per successful recipe execution for a
                // producer-house member (the post-market staple source; the tape is drained/
                // disabled by now, so this is a settlement-side append). A no-op off a member.
                if birth_gate_obs_on && self.birth_gate_obs.tracks(id) {
                    let free = self
                        .society
                        .free_stock_after_all_reserves(id, birth_gate_staple);
                    self.birth_gate_obs.push_production(id, free);
                }
                let (out_good, out_qty) = applied.output;
                if is_research {
                    // G6b: Knowledge is an ACCUMULATOR, not a tradeable good. Drain the
                    // produced units straight back out of the scholar's econ stock (so
                    // they never enter circulation, the digest, or the goods-conservation
                    // ledger) and add them to the per-settlement counter — reported on
                    // its own non-conserved line.
                    let drained = self.society.debit_stock(id, out_good, out_qty);
                    debug_assert!(drained, "the scholar holds the Knowledge it just produced");
                    let amount = u64::from(out_qty);
                    report.knowledge_produced = report.knowledge_produced.saturating_add(amount);
                    self.knowledge = self.knowledge.saturating_add(amount);
                } else {
                    *report.produced.entry(out_good).or_insert(0) += u64::from(out_qty);
                    if Some(out_good) == provenance_bread {
                        let lineage = self.is_lineage_agent(id);
                        self.bread_provenance
                            .credit_produced(id, u64::from(out_qty), lineage);
                    }
                    if Some(out_good) == acquisition_bread {
                        self.acquisition
                            .credit(id, FoodChannel::SelfProduced, u64::from(out_qty));
                    }
                    // DH.a (P1-1): the recipe seam — input consumed (endowed portion feeds CC2),
                    // output own-produced. Emit at the real application, not a phase diff. Research
                    // (Knowledge, drained immediately) never persists in stock and never runs under
                    // the closed marker, so it is excluded here.
                    if closure_on {
                        let (input, input_qty) = applied.input.unwrap_or((out_good, 0));
                        self.closure.record(
                            closure_tick,
                            closure::ClosureEventKind::RecipeProduction {
                                agent: id,
                                input,
                                input_qty,
                                output: out_good,
                                output_qty: out_qty,
                            },
                        );
                        // DH.b (impl-69): the stage-execution record (Mill/Bake only) — the
                        // successor-execution and staffed-flow evidence, recorded at the same
                        // real application seam with the ACTUAL `RecipeId` (R4-4). The same
                        // partial-borrow discipline: a direct field push, never a `&mut self`
                        // method.
                        if matches!(recipe_id, RecipeId::Mill | RecipeId::Bake) {
                            self.burden.executions.push(burden::BurdenStageExecution {
                                tick: closure_tick,
                                agent: id,
                                recipe: recipe_id,
                            });
                        }
                    }
                }
                // Conserved good INPUTS to any recipe — research included — are accounted
                // exactly like consumption (the conservation ledger sees every consumed
                // unit). Tools are durable and never appear here.
                if let Some((in_good, in_qty)) = applied.input {
                    *report.consumed_as_input.entry(in_good).or_insert(0) += u64::from(in_qty);
                }
            }
        }

        // G6b: having added this tick's Knowledge, check the tier-2 unlock. After the
        // research phase so the just-produced Knowledge counts toward the threshold.
        self.maybe_unlock_tier_two();
    }
    /// CAPITAL-ADVANCE phase (EXPERIMENT — see [`ChainConfig::capital_advance`]).
    /// Once money has emerged, top up any cashless active chain producer
    /// (Miller/Baker) to a small working-capital floor by transferring real,
    /// conserved money from the richest saver — so the producer can buy inputs
    /// ahead of selling output. Funded (no fiduciary credit), no repayment yet:
    /// a causal probe of whether missing working capital is what stalls the
    /// chain. A no-op unless enabled and money has emerged, so every other run is
    /// byte-identical. Deterministic: integer state, id-ordered, no RNG; the
    /// donor is chosen by most free (unreserved) gold, ties broken by lowest id.
    pub(super) fn run_capital_advance(&mut self) {
        let enabled = self
            .chain
            .as_ref()
            .is_some_and(|chain| chain.capital_advance);
        if !enabled || self.society.current_money_good().is_none() {
            return;
        }
        // Per-producer working-capital floor for one tick of input purchases.
        const FLOOR: u64 = 20;
        let live = self.live_colonist_slots.clone();
        for &slot in &live {
            let (producer_id, vocation) = {
                let colonist = &self.colonists[slot];
                (colonist.id, colonist.vocation)
            };
            if !matches!(vocation, Vocation::Miller | Vocation::Baker) {
                continue;
            }
            // One revolving loan at a time: re-borrow only once the prior loan is
            // repaid, so a producer's debt stays bounded by the floor.
            if self.capital_loans.contains_key(&producer_id) {
                continue;
            }
            let free = self.society.free_gold_after_all_reserves(producer_id).0;
            if free >= FLOOR {
                continue;
            }
            let need = FLOOR - free;
            // Richest lender by free (unreserved) gold — a saver, never a chain
            // producer, never the borrower; deterministic (ties -> lowest id).
            let lender = live
                .iter()
                .filter_map(|&lender_slot| {
                    let colonist = &self.colonists[lender_slot];
                    if colonist.id == producer_id
                        || matches!(colonist.vocation, Vocation::Miller | Vocation::Baker)
                    {
                        return None;
                    }
                    let free = self.society.free_gold_after_all_reserves(colonist.id).0;
                    (free > 0).then_some((free, colonist.id))
                })
                .max_by_key(|&(free, lender_id)| (free, std::cmp::Reverse(lender_id)));
            if let Some((free, lender_id)) = lender {
                let amount = need.min(free);
                if amount > 0 && self.move_money_conserved(lender_id, producer_id, Gold(amount)) {
                    self.capital_loans
                        .insert(producer_id, (lender_id, Gold(amount)));
                }
            }
        }
    }
    /// LOCAL PRODUCER SUBSISTENCE phase (S5 — the household/subsistence base, see
    /// [`ChainConfig::producer_subsistence`]). Before the market, top each chain
    /// producer (active Miller/Baker AND the latent pool that will adopt) up to a
    /// small staple-food floor, minting the staple FRESH from the producer's own
    /// renewable household hearth — exactly like [`Self::deliver_demography_provisions`]
    /// and NOT taken from any other agent. This is the LOCAL household allocation
    /// the endogenous milestone keeps (a producer's subsistence garden / its
    /// lineage's hearth), as distinct from the GLOBAL `run_subsistence_advance`
    /// redistribution (richest holder → producer) it turns OFF. A fed producer's
    /// money frees to bid for recipe inputs rather than reserve for its own hunger,
    /// and a latent producer survives the cold-start window to adopt. Conserved:
    /// the food is a source (`report.endowment`), eaten in the consume phase like
    /// any provision. Deterministic: slot order, integer; a no-op unless enabled.
    pub(super) fn run_producer_subsistence(&mut self, report: &mut EconTickReport) {
        let target = self
            .chain
            .as_ref()
            .map_or(0, |chain| chain.producer_subsistence);
        if target == 0 {
            return;
        }
        let staple = self.known.hunger;
        // S12: own-labor subsistence retires the producer's STAPLE mint (the food
        // scaffold) — only the WOOD/warmth provision stays an endowment (hunger-only
        // scope). A producer then earns its food by buying bread or, when idle/too
        // hungry to produce, foraging, exactly like the rest of the tail. S21d.0 retires
        // the SAME producer staple mint independent of forage — the open-survival probe,
        // so a producer's survival is a market bread purchase (the Phase B bootstrap test).
        let mint_staple = !self.own_labor_subsistence_can_run() && !self.retire_food_mints();
        let live_len = self.live_colonist_slots.len();
        for live_index in 0..live_len {
            let slot = self.live_colonist_slots[live_index];
            let (id, vocation, latent) = {
                let colonist = &self.colonists[slot];
                (colonist.id, colonist.vocation, colonist.latent)
            };
            let is_cycle_producer = matches!(
                vocation,
                Vocation::CycleA | Vocation::CycleB | Vocation::CycleC
            );
            let is_producer = is_cycle_producer
                || matches!(vocation, Vocation::Miller | Vocation::Baker)
                || (vocation == Vocation::Unassigned && latent.is_some());
            if !is_producer {
                continue;
            }
            // The producer's own hearth provisions the hunger staple AND WOOD (warmth)
            // up to the floor — exactly the two goods the demography hearth mints for
            // its members — so a producer's whole subsistence is met locally and its
            // money frees for recipe inputs. Under own-labor subsistence the staple
            // line is retired (only WOOD stays).
            for good in [staple, WOOD] {
                if good == staple && !mint_staple {
                    continue;
                }
                // C3R.e (impl-67): the B support-withdrawal gate on the cushion's STAPLE leg —
                // withdrawn once `econ_tick >= producer_support_until_tick` (inert for every non-B
                // config, where `producer_support_until_tick == None`, so byte-identical).
                if good == staple && !self.producer_support_active() {
                    continue;
                }
                // C3R.e (impl-67): the cushion's WOOD leg is DISABLED for the ENTIRE run of a B
                // cell — the bread-only acquisition ledger cannot origin-track subsidized WOOD, so
                // rather than leave untracked WOOD to survive "exhaustion" the B substrate runs
                // staple-only, constant across eras. `None` (every non-B config) leaves it intact.
                if good == WOOD && self.producer_cushion_wood_disabled() {
                    continue;
                }
                // S19 cycle producers: top subsistence up only to a low cap (well below
                // `need_max` = 12), so a producer's SINGLE per-tick barter offer is freed to
                // bid for its recipe input / the medium rather than reserved for survival —
                // the off-market-survival discipline that keeps the cycle goods (not food/WOOD)
                // the market goods. The cap is a disclosed parameter, NOT a tuned result: the
                // anchor-density sweep shows the finding (SALT leads but indirect trades don't
                // clear → no promotion) holds across densities regardless of this value.
                const CYCLE_PRODUCER_SUBSISTENCE_CAP: u16 = 4;
                let target = if is_cycle_producer {
                    let need = self.colonists[slot].need;
                    match good {
                        g if g == staple => {
                            u32::from(need.hunger.min(CYCLE_PRODUCER_SUBSISTENCE_CAP))
                        }
                        WOOD => u32::from(need.warmth.min(CYCLE_PRODUCER_SUBSISTENCE_CAP)),
                        _ => target,
                    }
                } else {
                    target
                };
                if target == 0 {
                    continue;
                }
                let held = self
                    .society
                    .agents
                    .get(id)
                    .map_or(0, |agent| agent.stock.get(good));
                if held < target {
                    // C3R.e (impl-67): a B cell's cushion STAPLE leg is a support mint —
                    // origin-flag it so the withdrawn cushion is exhaustion-tracked. The WOOD leg
                    // is disabled under B (skipped above) and untracked anyway, and every non-B
                    // cushion stays plain, so the ledger is unchanged off B.
                    let support_mint = good == staple && self.producer_support_configured();
                    self.deliver_demography_provision_unit(
                        id,
                        None,
                        good,
                        target - held,
                        support_mint,
                        report,
                    );
                }
            }
        }
    }
    /// S12 — OWN-LABOR SUBSISTENCE phase (see [`ChainConfig::own_labor_subsistence`]).
    /// Before the market (so the floor is on hand to eat this tick), credit a hungry,
    /// eligible, **unprovisioned** colonist with spare labor only when it completed a
    /// [`Task::GoForage`] in the preceding fast loop. The credited
    /// [`ChainConfig::forage_yield`] units of the FORAGE subsistence good land in its
    /// OWN econ stock — booked `report.produced` (its own labor on the forage node),
    /// NOT `report.endowment` (a mint). The same call sets the colonist's `foraging`
    /// flag, which steers the NEXT fast loop to send it to [`Task::GoForage`] instead
    /// of harvesting WOOD (the structural opportunity cost). Eligible = a
    /// spatial non-lineage colonist (`household: None`) in an untooled-or-latent role
    /// (`Consumer`/`Gatherer`/`Unassigned` — NOT an actively-producing Miller/Baker that
    /// has no spare labor). Hysteresis (`forage_hunger_in`/`out`) keeps a gatherer from
    /// thrashing between foraging and selling WOOD. FORAGE is `KnownGoods::subsistence`,
    /// ranked below bread, eaten in the consume phase and read back as hunger relief —
    /// and perishes via [`Self::run_spoilage`] if a decay rate is set. A no-op unless
    /// the gated own-labor path is active, so every other run is byte-identical.
    /// Deterministic: slot order, integer thresholds, nothing drawn.
    pub(super) fn run_own_labor_subsistence(
        &mut self,
        completed_forage: &BTreeSet<AgentId>,
        report: &mut EconTickReport,
    ) {
        // S21f: this steering phase runs on EITHER substrate — the own-labor/forage path
        // (S12/S14) OR the household-barter cultivation seam (cultivation without forage).
        // On the household-barter path there is no forage good interned, so the forage
        // extraction, the forage hysteresis, and the completed-forage credit are ALL gated
        // off below (guarded by `own_labor`) and only the cultivation steering runs.
        let own_labor = self.own_labor_subsistence_can_run();
        if !own_labor && !self.household_barter_cultivation_active() {
            return;
        }
        let chain = self
            .chain
            .as_ref()
            .expect("the cultivation steering path carries a chain");
        // The FORAGE substrate exists only on the own-labor path; the household-barter path
        // interns no forage good (so it never pollutes the value scale with a phantom
        // `known.subsistence`). `None` there, and every forage-keyed branch below is gated
        // on `own_labor`, so the household-barter path runs pure cultivation steering.
        let forage = if own_labor {
            Some(
                chain
                    .content
                    .forage()
                    .expect("the own-labor path carries a forage good"),
            )
        } else {
            None
        };
        let yield_units = chain.forage_yield;
        let h_in = chain.forage_hunger_in;
        let h_out = chain.forage_hunger_out;
        // S13: with spatial households on, a lineage member is itself spatial (it has a
        // world agent), so it joins the forage-eligible set — the ONE scoped behavior
        // change that lets the reproducing population forage. Off the flag the gate is
        // exactly `household.is_none()` (the pre-S13 non-lineage poor), so every other run
        // is byte-identical.
        let spatial_active = self.spatial_households_active();
        // S14: on the capped-commons path FORAGE is harvested from the depleting node
        // and hauled to econ (the haul cycle), NOT credited as a fixed labor yield — so
        // the per-completed-task credit is retired here. The `foraging` flag (the next
        // fast loop's forage-vs-WOOD steering) is still set below; only the credit is
        // gated off, so per-capita yield falls with the foraging population.
        let commons = self.forage_commons_active();
        // S15: the cultivation second tier. When active, a *still hungry* eligible
        // forager (forage could not keep its hunger down) escalates from foraging to
        // cultivating — steered (mutually exclusively) to GoHarvest the grain node and
        // cultivate bread instead. Off (every pre-S15 config) `cultivation` is false, so
        // `cultivating` is never set and the `foraging` steering is exactly S14.
        let cultivation = self.own_use_cultivation_active();
        // S16 buy/sell split (Codex P1c): when money-from-produced-bread is on, scope
        // forage/cultivation eligibility to LINEAGE spatial members
        // (`household.is_some() && spatial_active`), so the seeded SALT-holding consumers
        // (`household: None`) stay the goods-poor BUY side and do NOT self-forage/cultivate
        // — the division of labor S16 needs. Off the flag eligibility stays the S13/S14
        // `household.is_none() || spatial_active` exactly, so every existing run is
        // byte-identical.
        let buy_sell_split = self.cultivation_sells_surplus_active();
        // S22a: the endogenous cultivation-entry override. When active it RELAXES the S16
        // buy/sell split's household/spatial membership predicate to ANY spatial colonist
        // (lineage or not), so the food-producing class can self-form from sustained hunger
        // rather than assigned lineage identity. It overrides ONLY the membership predicate —
        // the `Consumer|Gatherer|Unassigned` vocation filter below is preserved (an active
        // Miller/Baker is still excluded). Off (every existing config) it is false, so the
        // S16/S13 eligibility is exactly as before and the run is byte-identical.
        let endogenous_entry = self.endogenous_cultivation_entry_active();
        let cultivate_input = if cultivation {
            self.cultivation_input_good()
        } else {
            None
        };
        let (cult_in, cult_out, cult_patience) = self.chain.as_ref().map_or((0, 0, 0), |c| {
            (
                c.cultivate_hunger_in,
                c.cultivate_hunger_out,
                c.cultivate_patience,
            )
        });
        // S22c: the profit-driven-retention exit modulation. When active, a currently-cultivating
        // agent past its hunger exit (no input in flight, not pressure-escalating) STAYS cultivating
        // if its realized cultivation return clears its outside option ([`Self::profit_stay_active`]).
        // Reset the per-tick counterfactual-flip diagnostic set up front; off the path it stays empty.
        let retention = self.profit_driven_retention_active();
        if retention {
            self.profit_retained_ids.clear();
        }
        // S22f: the voluntary fixed-term commitment seam. When active, an eligible UNCOMMITTED agent
        // may opt in post-money (its own realized cultivation-return signal clears the entry floor vs
        // its outside option), binding the cultivation exit for `commitment_term` ticks; at expiry it
        // re-decides from fresh returns. Reset the per-tick exit-override diagnostic up front; off the
        // path it stays empty. `money` is the hard anti-circularity gate (inert pre-money). The
        // fiat-pin control bypasses the signal and MAINTAINS `commitment_fiat_pin` concurrently
        // committed eligible agents (slot order): an already-committed agent occupies a pin slot, and
        // free slots are topped up from the uncommitted, so the forced cohort stays at the configured
        // size and re-pins on expiry rather than growing to swallow the roster.
        let commitment = self.voluntary_cultivation_commitment_active();
        let money = self.society.current_money_good() == Some(SALT);
        let commitment_term = self.commitment_term();
        let commitment_fiat_pin = self.commitment_fiat_pin();
        if commitment {
            self.commitment_exit_override_ids.clear();
        }
        self.run_commitment_norm_imitation();
        let mut fiat_pinned = 0u16;
        let live = self.live_colonist_slots.clone();
        // S22c: snapshot the START-OF-TICK cultivation roster ONCE, before the slot-by-slot loop
        // below mutates each colonist's `cultivating` flag. The colony reference outside rate (the
        // fallback `profit_stay_active` consults for a continuous cultivator with no recent outside
        // ticks of its own) pools NON-cultivating sellers; reading the LIVE flag mid-pass would let
        // a later colonist's stay decision depend on whether an earlier slot just entered/exited
        // cultivation THIS pass — roster-order dependent. Fixing the cohort to the start-of-tick
        // state removes that dependence. Empty (and never consulted) off the retention path.
        let cultivating_at_pass_start: BTreeSet<AgentId> = if retention {
            live.iter()
                .filter(|&&slot| self.colonists[slot].cultivating)
                .map(|&slot| self.colonists[slot].id)
                .collect()
        } else {
            BTreeSet::new()
        };
        for slot in live {
            let (
                id,
                eligible,
                hunger,
                was_foraging,
                was_cultivating,
                was_pressure,
                was_stock_pending,
                has_cultivation_input_in_flight,
                was_commitment_remaining,
                adopts_commitment_norm,
            ) = {
                let colonist = &self.colonists[slot];
                // The spatial poor with spare labor in an untooled-or-latent role
                // (`Consumer`/`Gatherer`/`Unassigned`). An actively-producing role
                // (Miller/Baker/Scholar/Confectioner) is excluded: it spends its one
                // world-task slot producing and is meant to earn its food by buying bread.
                // TRACKED GAP: with its staple mint retired, an active producer's only food
                // path is the bread market — unreachable on the non-spatial own-labor path
                // because SALT never monetizes, so no active producer ever forms (asserted
                // in `producer_food_path_is_feasible`). Before any own-labor config that
                // DOES monetize, an active producer must get a forage-when-too-hungry path.
                // Pre-S13 a lineage member is non-spatial and excluded (`household: None`
                // only); S13 spatial households make it spatial too, so it joins the set.
                // S16: with the buy/sell split on, only LINEAGE spatial members are eligible
                // (the seeded SALT consumers stay the buy side); off the flag this reduces
                // to the S13/S14 gate exactly (byte-identical).
                // S22a: the endogenous-entry override is a new TOP branch — when active, ANY
                // spatial colonist is eligible regardless of household, so a non-lineage role
                // under sustained hunger can enter cultivation through the same hysteresis.
                let spatial_member = if endogenous_entry {
                    spatial_active
                } else if buy_sell_split {
                    colonist.household.is_some() && spatial_active
                } else {
                    colonist.household.is_none() || spatial_active
                };
                let eligible = spatial_member
                    && matches!(
                        colonist.vocation,
                        Vocation::Consumer | Vocation::Gatherer | Vocation::Unassigned
                    );
                (
                    colonist.id,
                    eligible,
                    colonist.need.hunger,
                    colonist.foraging,
                    colonist.cultivating,
                    colonist.cultivate_pressure,
                    colonist.cultivation_stock_pending,
                    cultivate_input
                        .is_some_and(|input| self.cultivation_input_in_flight(colonist.id, input)),
                    colonist.commitment_remaining,
                    colonist.adopts_commitment_norm,
                )
            };
            // Hysteresis: start foraging at/above `h_in`, stop below `h_out`, else hold.
            // A non-eligible colonist (a lineage member, or one that adopted an active
            // producer role) never forages and clears any stale flag. S21f: on the
            // household-barter path (`!own_labor`) there is no forage good, so foraging is
            // always off — only the cultivation tier below steers these colonists.
            let forage_now = if !eligible || !own_labor {
                false
            } else if hunger >= h_in {
                true
            } else if hunger < h_out {
                false
            } else {
                was_foraging
            };
            // S15 second tier (the SUSTAINED-hunger gate): build a pressure streak while
            // hunger stays at/above `cult_in` (forage isn't bringing it down) and reset it
            // the moment hunger falls below `cult_in` (forage caught up — a transient
            // haul spike). Off the cultivation path the streak stays 0.
            let pressure = if cultivation && eligible && hunger >= cult_in {
                was_pressure.saturating_add(1)
            } else {
                0
            };
            // Escalate to cultivation once the streak reaches `cult_patience` (sustained
            // scarcity, not a transient spike), and HOLD cultivating until hunger drops
            // below `cult_out` AND every hauled grain unit has reached econ stock (so a
            // started grain haul deposits instead of stranding raw carry; settled stock
            // is drained by the cultivation phase even if this flag clears).
            // S22c: the realized post-money profit-stay term, computed ONLY for a currently-
            // cultivating eligible agent under the gate (so the cost is bounded to cultivators; off
            // the path it is always false and the disjunction is exactly today's). It joins the EXIT
            // branch only — entry stays hunger/pressure-gated (unchanged).
            let profit_stay = if retention && cultivation && eligible && was_cultivating {
                self.profit_stay_active(id, &cultivating_at_pass_start)
            } else {
                false
            };
            // S22a/S22c normal cultivation decision: entry is hunger/pressure-gated, the exit is
            // hunger/profit-gated. This is exactly the pre-S22f disjunction; S22f only adds the
            // commitment binding as an extra exit-override term below.
            let normal_cultivate = cultivation
                && eligible
                && (pressure >= cult_patience
                    || (was_cultivating
                        && (hunger >= cult_out || has_cultivation_input_in_flight || profit_stay)));
            // S22f: the voluntary fixed-term commitment steering. Off the path `commitment` is false
            // and `commitment_remaining` stays `was_commitment_remaining` (0 on every non-commitment
            // config), so `commitment_binds` is false and `cultivate_now` is exactly `normal_cultivate`.
            let mut commitment_remaining = was_commitment_remaining;
            if commitment {
                let norm_allows_commitment =
                    !self.commitment_norm_gate_active() || adopts_commitment_norm;
                if !eligible || !norm_allows_commitment {
                    // §3.5b: commitment overrides the EXIT, not vocation eligibility. An agent that
                    // left the S22a-eligible set (became an active specialized producer) drops its
                    // binding deterministically — no orphaned commitment on a non-cultivator. (A
                    // dead colonist is removed entirely, so its state drops with it.)
                    commitment_remaining = 0;
                } else if money && commitment_fiat_pin > 0 {
                    // The fiat-pin CONTROL: bypass the voluntary signal and maintain a fixed configured
                    // number of concurrently-committed eligible agents (slot order). An already-bound
                    // agent occupies a pin slot; free slots are topped up from the uncommitted, so the
                    // forced cohort re-pins on expiry without growing to swallow the roster.
                    if commitment_remaining > 0 {
                        if fiat_pinned < commitment_fiat_pin {
                            fiat_pinned = fiat_pinned.saturating_add(1);
                        } else {
                            commitment_remaining = 0;
                        }
                    } else if fiat_pinned < commitment_fiat_pin {
                        fiat_pinned = fiat_pinned.saturating_add(1);
                        commitment_remaining = commitment_term;
                        self.commitment_fiat_ever.insert(id);
                        self.commitment_committed_ever.insert(id);
                    }
                } else if commitment_remaining == 0 && money {
                    // Eligible + uncommitted + post-money: voluntary opt-in. Entry is gated by the
                    // agent's OWN realized cultivation-return signal clearing the entry floor vs its
                    // outside option — no quota/top-N, inert pre-money.
                    match self.commitment_entry_signal_clears(id, &cultivating_at_pass_start) {
                        CommitmentEntrySignal::Clears(signal) => {
                            commitment_remaining = commitment_term;
                            if self.commitment_committed_ever.contains(&id) {
                                // A re-commit from a fresh post-expiry signal — a tracked renewal
                                // (the first opt-in is not a renewal). Persistence across terms
                                // must come from re-choosing, not one long binding (§2.5).
                                self.colonists[slot].commitment_renewals =
                                    self.colonists[slot].commitment_renewals.saturating_add(1);
                            } else {
                                // First voluntary opt-in — record the uptake tick + signal value
                                // (the proof entry is traceable to the agent's own cleared signal).
                                self.commitment_uptake.insert(id, (self.econ_tick, signal));
                            }
                            self.commitment_committed_ever.insert(id);
                        }
                        CommitmentEntrySignal::BelowFloor => {
                            // A below-floor non-committer — the signal discriminates (entry is a
                            // real decision, not a universal auto-yes).
                            self.commitment_below_floor_ever.insert(id);
                        }
                        CommitmentEntrySignal::AboveFloorLoses => {}
                    }
                }
            }
            // S22f: while the term runs the agent CULTIVATES — the binding overrides the normal
            // hunger/profit exit (the ONE new exit behavior in the arc), gated entirely behind the
            // voluntary, signal-cleared, post-money entry above.
            let binding_commitment_remaining = if commitment_term <= 1 {
                0
            } else {
                commitment_remaining
            };
            let commitment_binds =
                commitment && cultivation && eligible && binding_commitment_remaining > 0;
            let cultivate_now = normal_cultivate || commitment_binds;
            // S22f (runtime-only diagnostic): a real exit-override — bound this tick but the normal
            // S22a/S22c rule would have EXITED. The mandatory non-vacuity test reads this set.
            if commitment_binds && !normal_cultivate {
                self.commitment_exit_override_ids.insert(id);
                self.commitment_exit_override_ever.insert(id);
            }
            // S22c (runtime-only diagnostic): a counterfactual exit FLIP — this agent is cultivating
            // THIS tick ONLY because `profit_stay` fired (it is past the hunger exit, has no input in
            // flight, and is not pressure-escalating), so the flag-off path would have EXITED it. The
            // mandatory non-vacuity test reads this set.
            let retained_by_profit = profit_stay
                && pressure < cult_patience
                && hunger < cult_out
                && !has_cultivation_input_in_flight;
            if retained_by_profit {
                self.profit_retained_ids.insert(id);
                self.profit_retained_ever.insert(id);
            }
            // Mutually exclusive (one world task per econ tick): cultivation takes the
            // task slot when it fires, so the colonist forages XOR cultivates — never
            // both. `cultivate_now` implies `!foraging` here.
            let (forage_now, cultivate_now) = if cultivate_now {
                (false, true)
            } else {
                (forage_now, false)
            };
            self.colonists[slot].foraging = forage_now;
            self.colonists[slot].cultivating = cultivate_now;
            self.colonists[slot].cultivate_pressure = pressure;
            self.colonists[slot].cultivation_stock_pending =
                cultivation && eligible && (was_stock_pending || was_cultivating || cultivate_now);
            // S22f: persist the per-agent commitment term, decrementing once per econ tick while it
            // binds (so a `commitment_term`-length opt-in binds exactly that many ticks; at 0 the
            // agent is uncommitted and re-decides next tick). Off the path `commitment_remaining` is
            // 0 and `commitment_binds` is false, so this writes 0 (byte-identical, and the field is
            // not serialized off the gate anyway).
            self.colonists[slot].commitment_remaining =
                binding_commitment_remaining.saturating_sub(u16::from(commitment_binds));
            // S24b: a staged norm flip takes effect AT term expiry — the tick the binding clears —
            // BEFORE the next renewal decision, so the end-of-tick adopter bit (and the digest)
            // reflect the abandonment in the same tick rather than one econ tick late. Gated and a
            // no-op off the path, and a no-op while the term still binds (`commitment_remaining > 0`).
            self.apply_staged_commitment_norm_bit_if_unbound(slot);
            debug_assert!(
                !(self.colonists[slot].foraging && self.colonists[slot].cultivating),
                "a colonist must forage XOR cultivate — never both in one econ tick"
            );
            // S21f: the completed-forage credit is a forage-path behavior. On the
            // household-barter path `own_labor` is false (and `forage` is `None`), so it is
            // skipped — the household-barter cultivator earns its food by cultivating grain,
            // not by foraging.
            if own_labor && !commons && completed_forage.contains(&id) {
                if let Some(forage) = forage {
                    self.credit_produced(id, forage, yield_units, report);
                }
            }
        }
    }
    /// OWN-USE CULTIVATION phase (S15): each *cultivating* colonist converts the grain
    /// it hauled this tick into bread by its OWN labor (the no-tool `Cultivate` recipe —
    /// booked `produced`/`consumed_as_input`, never minted), then eats up to
    /// [`ChainConfig::cultivate_consume`] of that bread through the consumption-readback
    /// seam so its hunger actually falls next tick. The remaining cultivated bread stays
    /// in stock to endow children (the broadened birth-food rule). Runs AFTER the market
    /// step (so the freshly-cultivated bread is minted after clearing and is never
    /// offered for sale — own-use) and AFTER production, BEFORE births (so a parent's
    /// just-cultivated bread can endow this tick's newborn). A no-op unless the gated
    /// cultivation path is active, so every other run is byte-identical. Deterministic:
    /// slot order, integer thresholds, nothing drawn.
    pub(super) fn run_own_use_cultivation(&mut self, report: &mut EconTickReport) {
        if !self.own_use_cultivation_active() {
            return;
        }
        let chain = self
            .chain
            .as_ref()
            .expect("the cultivation path carries a chain");
        let recipe = chain
            .content
            .cultivate_recipe()
            .expect("the cultivation path carries the Cultivate recipe");
        let recipe_id = recipe.id;
        let recipe_labor = recipe.labor;
        let input_good = recipe.input_good.map(|(good, _)| good);
        let input_qty = recipe.input_good.map_or(0, |(_, qty)| qty);
        debug_assert!(
            recipe_labor > 0,
            "Cultivate must have a positive labor cost"
        );
        let bread = chain.content.bread();
        let consume = chain.cultivate_consume;
        // S16: cultivated bread is PRODUCED-origin — credit the provenance ledger as each
        // loaf is booked (the own-use consume below is sinked by the provenance own-use pass).
        let provenance = self.bread_provenance_active();
        // S21d.1: cultivated bread is SELF-PRODUCED for the acquisition ledger too (the own-use
        // consume below is debited by the acquisition own-use pass).
        let acquisition = self.acquisition_ledger_active();
        // S22b: on the cultivation-skill path, record which agents realize cultivation output
        // this tick (output > 0) so [`Self::run_cultivation_skill`] credits skill GAIN to exactly
        // those, plus the per-agent cumulative produced-bread diagnostic. Reset the per-tick
        // producer scratch up front. Inert (and the counters stay empty) off the skill path.
        let skill_active = self.cultivation_skill_active();
        if skill_active {
            self.cultivation_skill_producers.clear();
        }
        // S22d: on the durable-cultivation-capital path, record the same realized-output set so
        // [`Self::run_cultivation_capital_formation`] credits cultivation TENURE to exactly those
        // agents (and resets everyone else), plus the per-agent cumulative produced-bread
        // diagnostic. Reset the per-tick scratch up front. Inert off the path.
        let tool_active = self.durable_cultivation_tool_active();
        if tool_active {
            self.cultivation_tool_producers.clear();
        }
        let land_owner_telemetry = self.private_land_tenure_active();
        let hunger_target = 0;
        let hunger_deplete = self.dynamics.hunger_deplete;
        let hunger_per_food = self.dynamics.hunger_per_food;
        // The goods the need readback counts as hunger relief (mirrors
        // `update_needs_and_remove_dead`): the hunger staple plus the directly-edible
        // subsistence food. Used to net out food already eaten this tick (below).
        let hunger_staple = self.known.hunger;
        let subsistence_food = self.known.subsistence;
        let live = self.live_colonist_slots.clone();
        for slot in live {
            let (id, cultivating, stock_pending, hunger) = {
                let colonist = &self.colonists[slot];
                (
                    colonist.id,
                    colonist.cultivating,
                    colonist.cultivation_stock_pending,
                    colonist.need.hunger,
                )
            };
            let share_contract = self.share_contract_for_worker(id);
            let in_kind_contract = self.in_kind_contract_for_worker(id);
            if !cultivating
                && !stock_pending
                && share_contract.is_none()
                && in_kind_contract.is_none()
            {
                continue;
            }
            // PRODUCE: convert held grain into bread up to this tick's own-labor budget.
            // Only agents that entered the cultivation path can drain settled grain after
            // the steering flag clears; unrelated grain holders keep their stock. The
            // conversion is bounded to grain that is FREE after all reserves, so the recipe
            // never consumes input backing a live ask: the hauled grain is converted at
            // home (own-use), never sold. On every shipped pre-money config the cultivator
            // posts no grain ask, so `free == held` and the whole haul converts at the same
            // count as an unbounded drain (byte-identical); the bound only bites if
            // cultivation is later composed onto a chain with a grain market (S16), where
            // the colonist cultivates whatever grain it still holds after the market and
            // conservation is preserved. Grain beyond the budget stays in stock and is
            // drained by a later cultivation phase. The executor removes grain, credits
            // bread to the colonist's own stock, and records the recipe labor; we book the
            // conserved conversion.
            let free_input = input_good.map_or(0, |input| {
                self.society.free_stock_after_all_reserves(id, input)
            });
            let max_runs = free_input.checked_div(input_qty).unwrap_or(0);
            let grain_labor_budget = max_runs.saturating_mul(recipe_labor);
            let mut remaining_labor = OWN_USE_CULTIVATION_LABOR_BUDGET.min(grain_labor_budget);
            // S22b: bread this agent actually cultivated this tick (the realized output the skill
            // accumulate rule credits — NOT the mere `cultivating` flag).
            let mut realized_bread = 0u64;
            while recipe_labor > 0 && remaining_labor >= recipe_labor {
                let Some(applied) = self
                    .society
                    .execute_direct_recipe_for_agent_checked_with_labor(
                        id,
                        recipe_id,
                        remaining_labor,
                    )
                else {
                    break; // out of grain (or no output headroom): nothing more to cultivate
                };
                remaining_labor = remaining_labor.saturating_sub(applied.labor);
                let (out_good, out_qty) = applied.output;
                *report.produced.entry(out_good).or_insert(0) += u64::from(out_qty);
                if provenance && out_good == bread {
                    let lineage = self.is_lineage_agent(id);
                    self.bread_provenance
                        .credit_produced(id, u64::from(out_qty), lineage);
                }
                if acquisition && out_good == bread {
                    self.acquisition
                        .credit(id, FoodChannel::SelfProduced, u64::from(out_qty));
                }
                if out_good == bread {
                    realized_bread += u64::from(out_qty);
                }
                if let Some((in_good, in_qty)) = applied.input {
                    *report.consumed_as_input.entry(in_good).or_insert(0) += u64::from(in_qty);
                }
                if out_good == bread {
                    let input_qty = applied
                        .input
                        .filter(|(good, _)| Some(*good) == input_good)
                        .map_or(0, |(_, qty)| qty);
                    self.split_share_output(id, u64::from(out_qty), input_qty);
                    self.split_in_kind_output(id, u64::from(out_qty), input_qty);
                }
            }
            // S22b: a tick with realized cultivation output (grain hauled AND converted to bread)
            // earns skill; record it for the skill-update pass + the cumulative produced-bread
            // diagnostic. An agent merely flagged `cultivating` while walking/blocked/on a
            // depleted node produces nothing here and so earns no skill (it decays instead).
            if realized_bread > 0 {
                if skill_active {
                    self.cultivation_skill_producers.insert(id);
                }
                // S22d: the same realized-output set drives the tenure credit (a sustained
                // PRODUCING cultivator builds, not one merely flagged `cultivating`).
                if tool_active {
                    self.cultivation_tool_producers.insert(id);
                }
                // The cumulative produced-bread diagnostic is maintained on skill/capital paths and
                // on private-land runs, where S23e reads owner surplus under mortality.
                if skill_active || tool_active || land_owner_telemetry {
                    *self.cultivation_bread_produced.entry(id).or_insert(0) += realized_bread;
                }
            }
            let has_remaining_input =
                input_good.is_some_and(|input| self.cultivation_input_in_stock(id, input));
            let has_input_in_flight =
                input_good.is_some_and(|input| self.cultivation_input_in_flight(id, input));
            self.colonists[slot].cultivation_stock_pending = cultivating
                || share_contract.is_some()
                || in_kind_contract.is_some()
                || has_remaining_input
                || has_input_in_flight;
            // CONSUME (own-use): eat up to `consume` of the cultivator's OWN bread through
            // the readback seam, so hunger advances next tick from its own stock — never a
            // market trade. The surplus stays in stock for the birth endowment. Net out any
            // food this agent ALREADY ate in this tick's market consume pass (logged after
            // the log was cleared at the market step's start): that intake advances hunger
            // on the next readback too, so a cultivator that entered the tick holding bread
            // from a prior spell does not over-eat and drain the endowment reserve.
            let held = self.society.free_stock_after_all_reserves(id, bread);
            let already_food = self
                .society
                .consumption_log_last_tick()
                .iter()
                .filter(|&&(a, g, _)| {
                    a == id && (g == hunger_staple || Some(g) == subsistence_food)
                })
                .fold(0u32, |acc, &(_, _, qty)| acc.saturating_add(qty));
            let target =
                food_needed_to_reach_hunger(hunger, hunger_deplete, hunger_per_food, hunger_target)
                    .saturating_sub(already_food);
            let eat = consume.min(held).min(target);
            self.consume_own_use_stock(id, bread, eat);
        }
    }
    /// CULTIVATION-SKILL UPDATE phase (S22b): advance each living colonist's bounded
    /// [`Colonist::cultivation_skill`] from this tick's realized cultivation output. An agent in
    /// [`Self::cultivation_skill_producers`] (it actually harvested grain AND converted it to
    /// bread this tick) gains [`ChainConfig::skill_gain`], saturating at
    /// [`ChainConfig::skill_cap`]; every other living colonist decays by
    /// [`ChainConfig::skill_decay`]. Runs AFTER [`Self::run_own_use_cultivation`] (so the
    /// producer set is filled) and BEFORE births (a newborn is added later, born at skill 0).
    /// Deterministic: slot order, integer thresholds, nothing drawn. A no-op off the gated
    /// cultivation-skill path, so every other run is byte-identical.
    pub(super) fn run_cultivation_skill(&mut self) {
        if !self.cultivation_skill_active() {
            return;
        }
        let (gain, decay, cap) = self.chain.as_ref().map_or((0, 0, 0), |chain| {
            (chain.skill_gain, chain.skill_decay, chain.skill_cap)
        });
        let producers = std::mem::take(&mut self.cultivation_skill_producers);
        let live = self.live_colonist_slots.clone();
        for slot in live {
            let colonist = &mut self.colonists[slot];
            colonist.cultivation_skill = if producers.contains(&colonist.id) {
                colonist.cultivation_skill.saturating_add(gain).min(cap)
            } else {
                colonist.cultivation_skill.saturating_sub(decay)
            };
        }
    }
    /// CULTIVATION CAPITAL FORMATION phase (S22d): the SEPARATE, gated, PRE-money-capable build
    /// of the durable, OWNED, role-specific cultivation tool (the plow) — deliberately NOT a reuse
    /// of [`Self::run_capital_formation`] (which is money-gated and hardcodes the mill/oven
    /// goods/recipes). Three deterministic sub-steps (slot order, integer state, nothing drawn):
    ///
    /// 1. **Credit tenure.** Each live colonist that realized cultivation output this tick (the
    ///    `cultivation_tool_producers` set [`Self::run_own_use_cultivation`] filled) gains one
    ///    tick of cultivation TENURE (saturating); every other live colonist RESETS to 0. Tenure
    ///    is the sustained-PRODUCING-cultivation streak that gates the build — distinct from the
    ///    hunger-entry `cultivate_pressure`.
    /// 2. **Advance + complete in-flight builds.** Each in-flight project advances one labor unit
    ///    (the builder's own labor) and completes against the builder's own stock, crediting the
    ///    durable plow (booked `produced`). A dead builder's project is dropped (its committed
    ///    WOOD was already booked `consumed_as_input` at the start tick — a forfeit, conserved).
    /// 3. **Start new builds.** Each currently-cultivating colonist whose tenure ≥
    ///    `tool_build_patience`, that holds NO plow yet, has no in-flight build, and can afford
    ///    `tool_build_wood` WOOD, commits that WOOD (booked `consumed_as_input`) into a new
    ///    [`ProjectTemplateId::BuildCultivationTool`] project + advances one labor unit; if that
    ///    completes the build, the plow credits its stock immediately, else the project is stored.
    ///
    /// The build inputs reuse the existing [`ChainConfig::tool_build_wood`]/`tool_build_labor`
    /// (the producer-capital chain is off on every cultivation scenario, so they are free). The
    /// SUNK WOOD is real and permanently consumed — the commitment that creates the owner
    /// population and the opportunity cost of leaving. The committed labor is replayed via
    /// `record_external_labor_used` so the next needs readback sees it. A no-op off the
    /// durable-cultivation-capital path, so every other run is byte-identical. Conservation holds
    /// by construction (WOOD `consumed_as_input` at start, the durable plow `produced` at
    /// completion — never a recipe-ratio change or mint).
    pub(super) fn run_cultivation_capital_formation(&mut self, report: &mut EconTickReport) {
        if !self.durable_cultivation_tool_active() {
            return;
        }
        let chain = self
            .chain
            .as_ref()
            .expect("the cultivation-capital path carries a chain");
        let Some(tool_good) = chain.content.cultivation_tool() else {
            return;
        };
        let wood_qty = chain.tool_build_wood;
        let build_labor = chain.tool_build_labor;
        let patience = chain.tool_build_patience;
        let tick = self.society.tick.0;
        let mut labor_used: Vec<(AgentId, u32)> = Vec::new();
        let live = self.live_colonist_slots.clone();

        // ---- 1. CREDIT TENURE (realized-output streak; reset otherwise).
        let producers = std::mem::take(&mut self.cultivation_tool_producers);
        for &slot in &live {
            let colonist = &mut self.colonists[slot];
            colonist.cultivation_tenure = if producers.contains(&colonist.id) {
                colonist.cultivation_tenure.saturating_add(1)
            } else {
                0
            };
        }

        // ---- 1b. NON-DURABLE / RENTED CONTROL: consume each owner's plow after the one
        // cultivation opportunity it boosted (an owner that realized output THIS tick held the plow
        // during this tick's fast-loop haul). Runs BEFORE the builds below, so a plow built THIS
        // tick — which did not boost this tick's already-past haul — is NOT consumed this tick; it
        // boosts next tick, then is consumed next tick. Booked `consumed_as_input` (a real sink),
        // so the agent must re-build (re-pay the sunk WOOD) to get the boost again — no persistent
        // ownership. Inert (the plow persists) for the durable headline.
        if self.cultivation_tool_non_durable_active() {
            for &slot in &live {
                let id = self.colonists[slot].id;
                if !producers.contains(&id) {
                    continue;
                }
                if let Some(agent) = self.society.agents.get_mut(id) {
                    if agent.stock.get(tool_good) > 0 && agent.stock.remove(tool_good, 1) {
                        *report.consumed_as_input.entry(tool_good).or_insert(0) += 1;
                        self.cultivation_tools_destroyed =
                            self.cultivation_tools_destroyed.saturating_add(1);
                    }
                }
            }
        }

        // ---- 2. ADVANCE + COMPLETE in-flight builds (each its own labor).
        let mut finished: Vec<usize> = Vec::new();
        for bi in 0..self.cultivation_tool_builds.len() {
            let builder = self.cultivation_tool_builds[bi].builder;
            let alive = self
                .colonist_slot_by_id
                .get(&builder)
                .is_some_and(|&s| self.colonists[s].alive);
            if !alive {
                // The committed WOOD was already booked `consumed_as_input` at the start tick, so
                // the forfeit needs no further booking (conservation already balanced).
                finished.push(bi);
                continue;
            }
            {
                let build = &mut self.cultivation_tool_builds[bi];
                if build.project.labor_advanced < build.template.required_labor
                    && advance_project(&mut build.project)
                {
                    labor_used.push((builder, 1));
                }
            }
            let qty = self.cultivation_tool_builds[bi].project.output_qty;
            let completed = match self.society.agents.get_mut(builder) {
                Some(agent) => {
                    let build = &mut self.cultivation_tool_builds[bi];
                    complete_project_if_ready(&mut build.project, &build.template, &mut agent.stock)
                }
                None => false,
            };
            if completed {
                *report.produced.entry(tool_good).or_insert(0) += u64::from(qty);
                self.cultivation_tools_built =
                    self.cultivation_tools_built.saturating_add(u64::from(qty));
                finished.push(bi);
            }
        }
        for &bi in finished.iter().rev() {
            self.cultivation_tool_builds.remove(bi);
        }

        // ---- 3. START new builds. A currently-cultivating colonist with sustained producing
        // tenure, no plow yet, no in-flight build, and enough saved WOOD invests its OWN WOOD +
        // labor — per-agent (no global stage choice, no single-in-flight gate).
        for &slot in &live {
            let (id, cultivating, tenure) = {
                let c = &self.colonists[slot];
                (c.id, c.cultivating, c.cultivation_tenure)
            };
            if !cultivating || tenure < patience {
                continue;
            }
            if self.cultivation_tool_builds.iter().any(|b| b.builder == id) {
                continue;
            }
            // Must hold no plow yet (else it is already an owner) and enough saved WOOD to fund
            // the build from its OWN endowment — the sunk cost that makes ownership a minority.
            let can_fund = self.society.agents.get(id).is_some_and(|agent| {
                agent.stock.get(tool_good) == 0 && agent.stock.get(WOOD) >= wood_qty
            });
            if !can_fund {
                continue;
            }
            let template = build_cultivation_tool_template(tool_good, wood_qty, build_labor);
            let pid = ProjectId(self.next_cultivation_tool_project_id);
            let started = match self.society.agents.get_mut(id) {
                Some(agent) => start_project(&template, &mut agent.stock, pid, Tick(tick)),
                None => None,
            };
            if let Some(mut project) = started {
                *report.consumed_as_input.entry(WOOD).or_insert(0) += u64::from(wood_qty);
                self.cultivation_tool_wood_consumed = self
                    .cultivation_tool_wood_consumed
                    .saturating_add(u64::from(wood_qty));
                if project.labor_advanced < template.required_labor && advance_project(&mut project)
                {
                    labor_used.push((id, 1));
                }
                self.next_cultivation_tool_project_id =
                    self.next_cultivation_tool_project_id.wrapping_add(1);
                let completed = match self.society.agents.get_mut(id) {
                    Some(agent) => {
                        complete_project_if_ready(&mut project, &template, &mut agent.stock)
                    }
                    None => false,
                };
                if completed {
                    let qty = project.output_qty;
                    *report.produced.entry(tool_good).or_insert(0) += u64::from(qty);
                    self.cultivation_tools_built =
                        self.cultivation_tools_built.saturating_add(u64::from(qty));
                } else {
                    self.cultivation_tool_builds.push(CapitalBuild {
                        builder: id,
                        slot,
                        template,
                        project,
                    });
                }
            }
        }

        for (agent, labor) in labor_used {
            self.society.record_external_labor_used(agent, labor);
        }
    }
    /// IN-KIND SUBSISTENCE ADVANCE phase (EXPERIMENT — see
    /// [`ChainConfig::subsistence_advance`]). Before the market, feed each hungry
    /// active chain producer (Miller/Baker) up to a small staple floor by
    /// transferring staple food **in kind** from the richest food-holder (a
    /// saver, never another producer, which keeps at least the same floor for
    /// itself). The live order-book trace proved a funded-but-hungry producer
    /// posts no input bid because its money is reserved for its own unmet bread
    /// want; provisioning that want frees the money so it bids for grain. The
    /// food moves holder→producer and is later eaten — conserved, no new sink.
    /// A no-op unless enabled and money has emerged. Deterministic: id-ordered,
    /// integer; donor chosen by most staple held (ties → lowest id). S16: when the
    /// staple is the tracked bread the move also follows in the produced-bread
    /// provenance ledger (donor→producer), so the produced origin tracks the
    /// physical loaf instead of stranding on the donor.
    pub(super) fn run_subsistence_advance(&mut self) {
        let enabled = self
            .chain
            .as_ref()
            .is_some_and(|chain| chain.subsistence_advance);
        if !enabled || self.society.current_money_good().is_none() {
            return;
        }
        // Staple floor that provisions a producer's present hunger ladder.
        const FEED_TARGET: u32 = 4;
        let staple = self.known.hunger;
        let live = self.live_colonist_slots.clone();
        for &slot in &live {
            let (producer_id, vocation) = {
                let colonist = &self.colonists[slot];
                (colonist.id, colonist.vocation)
            };
            if !matches!(vocation, Vocation::Miller | Vocation::Baker) {
                continue;
            }
            let held = self
                .society
                .agents
                .get(producer_id)
                .map_or(0, |agent| agent.stock.get(staple));
            if held >= FEED_TARGET {
                continue;
            }
            let need = FEED_TARGET - held;
            // Richest food-holder, never a producer, never the producer itself,
            // and keeping at least FEED_TARGET for its own subsistence.
            let donor = live
                .iter()
                .filter_map(|&donor_slot| {
                    let colonist = &self.colonists[donor_slot];
                    if colonist.id == producer_id
                        || matches!(colonist.vocation, Vocation::Miller | Vocation::Baker)
                    {
                        return None;
                    }
                    let stock = self
                        .society
                        .agents
                        .get(colonist.id)
                        .map_or(0, |agent| agent.stock.get(staple));
                    (stock > FEED_TARGET).then_some((stock, colonist.id))
                })
                .max_by_key(|&(stock, donor_id)| (stock, std::cmp::Reverse(donor_id)));
            if let Some((stock, donor_id)) = donor {
                let give = need.min(stock - FEED_TARGET);
                if give > 0 && self.society.debit_stock(donor_id, staple, give) {
                    // Conserved transfer; roll back to the donor if the credit
                    // can't land (overflow), so no food is created or destroyed.
                    if self.society.credit_stock(producer_id, staple, give) {
                        // S16: this in-kind staple move is a conserved TRANSFER, so when the
                        // advanced staple IS the tracked bread the provenance ledger must
                        // follow the physical loaf donor→producer (a produced loaf stays
                        // produced). Without this mirror the produced origin strands on the
                        // donor and the per-agent invariant (`produced ≤ stock`) breaks once
                        // the producer eats or sells the advanced bread. A no-op off the
                        // ledger or when the staple is not the tracked bread good — so every
                        // existing config stays byte-identical.
                        if self.bread_provenance_active()
                            && Some(staple) == self.provenance_bread_good()
                        {
                            self.bread_provenance
                                .transfer(donor_id, producer_id, u64::from(give));
                        }
                        // S21d.1: the acquisition-channel ledger tracks the same conserved
                        // in-kind staple move. Preserve the original channel (a bought loaf
                        // advanced to a producer is still market-acquired; a minted/seeded loaf
                        // is still seeded/minted) so the FIFO ledger stays conserved when the
                        // producer later eats or sells the advanced food.
                        if self.acquisition_ledger_active()
                            && Some(staple) == self.acquisition_food_good()
                        {
                            self.acquisition.transfer_preserve(
                                donor_id,
                                producer_id,
                                u64::from(give),
                            );
                        }
                    } else {
                        self.society.credit_stock(donor_id, staple, give);
                    }
                }
            }
        }
    }
    /// IN-KIND INPUT ADVANCE phase (EXPERIMENT — see [`ChainConfig::input_advance`]).
    /// Before production, a capitalist (the richest money-holder) buys each active
    /// producer's recipe input **in kind** from the holder with the most of it
    /// (grain for a miller from the gatherers, flour for a baker from the millers),
    /// paying the seller real money and placing the input in the producer's hands.
    /// This bypasses the value-scale gate: production no longer needs a producer to
    /// out-rank its own consumption/savings to *bid* for inputs. Conserved (money
    /// capitalist→seller, input seller→producer); it also recirculates the
    /// capitalist's idle money to the sellers. No-op unless enabled and money has
    /// emerged. Deterministic: id-ordered; capitalist/seller by most free
    /// gold / most stock (ties → lowest id).
    pub(super) fn run_input_advance(&mut self) {
        let (grain, flour) = match self.chain.as_ref() {
            Some(chain) if chain.input_advance => (chain.content.grain(), chain.content.flour()),
            _ => return,
        };
        if self.society.current_money_good().is_none() {
            return;
        }
        // A small per-tick input float — enough for a recipe application.
        const STOCK_TARGET: u32 = 3;
        let live = self.live_colonist_slots.clone();
        for &slot in &live {
            let (producer_id, vocation) = {
                let colonist = &self.colonists[slot];
                (colonist.id, colonist.vocation)
            };
            let input = match vocation {
                Vocation::Miller => grain,
                Vocation::Baker => flour,
                _ => continue,
            };
            let held = self
                .society
                .agents
                .get(producer_id)
                .map_or(0, |agent| agent.stock.get(input));
            if held >= STOCK_TARGET {
                continue;
            }
            let need = STOCK_TARGET - held;
            let pick = |key: &dyn Fn(AgentId) -> u64| -> Option<(u64, AgentId)> {
                live.iter()
                    .filter_map(|&s| {
                        let id = self.colonists[s].id;
                        if id == producer_id {
                            return None;
                        }
                        let v = key(id);
                        (v > 0).then_some((v, id))
                    })
                    .max_by_key(|&(v, id)| (v, std::cmp::Reverse(id)))
            };
            let seller = pick(&|id| {
                u64::from(
                    self.society
                        .agents
                        .get(id)
                        .map_or(0, |a| a.stock.get(input)),
                )
            });
            let capitalist = pick(&|id| self.society.free_gold_after_all_reserves(id).0);
            let (Some((seller_stock, seller_id)), Some((cap_free, cap_id))) = (seller, capitalist)
            else {
                continue;
            };
            if cap_id == seller_id {
                continue;
            }
            let price = self.realized_price(input).map_or(1, |g| g.0.max(1));
            let affordable = u32::try_from(cap_free / price).unwrap_or(u32::MAX);
            let qty = need
                .min(u32::try_from(seller_stock).unwrap_or(u32::MAX))
                .min(affordable);
            if qty == 0 {
                continue;
            }
            let cost = Gold(u64::from(qty) * price);
            // Pay the seller, then place the input. Roll back on any failure so no
            // good or money is created or destroyed.
            if self.move_money_conserved(cap_id, seller_id, cost) {
                if self.society.debit_stock(seller_id, input, qty) {
                    if !self.society.credit_stock(producer_id, input, qty) {
                        self.society.credit_stock(seller_id, input, qty);
                        self.move_money_conserved(seller_id, cap_id, cost);
                    }
                } else {
                    self.move_money_conserved(seller_id, cap_id, cost);
                }
            }
        }
    }
    /// Capital-advance REPAYMENT phase (EXPERIMENT): after the market clears,
    /// each borrower repays its revolving working-capital loan from its sales,
    /// keeping it cash-light so its future-money want stays UNMET — the incentive
    /// to keep producing survives (unlike an unrepaid gift, which satisfies the
    /// want and gets the producer de-adopted). Conserved; a no-op when there are
    /// no loans. Deterministic: id-ordered over the loan ledger.
    pub(super) fn run_capital_repayment(&mut self) {
        if self.capital_loans.is_empty() {
            return;
        }
        let borrowers: Vec<AgentId> = self.capital_loans.keys().copied().collect();
        for borrower in borrowers {
            let Some(&(lender, owed)) = self.capital_loans.get(&borrower) else {
                continue;
            };
            // Drop loans whose borrower or lender is no longer live — the estate
            // already settled that gold elsewhere; the money stays conserved in
            // the system, only the (unrecoverable) bookkeeping is dropped.
            if self.society.agents.get(borrower).is_none()
                || self.society.agents.get(lender).is_none()
            {
                self.capital_loans.remove(&borrower);
                continue;
            }
            let free = self.society.free_gold_after_all_reserves(borrower);
            let repay = free.min(owed);
            if repay > Gold::ZERO && self.move_money_conserved(borrower, lender, repay) {
                let remaining = owed.saturating_sub(repay);
                if remaining == Gold::ZERO {
                    self.capital_loans.remove(&borrower);
                } else {
                    self.capital_loans.insert(borrower, (lender, remaining));
                }
            }
        }
    }
    /// SPOILAGE phase (EXPERIMENT — see [`ChainConfig::perishable_decay_bps`]):
    /// decay every colonist's (and the commons') holdings of the **staple** food
    /// (the hunger good, plus the subsistence food if any) by the configured
    /// per-tick rate. A real sink: every removed unit is recorded in
    /// `report.spoiled` so whole-system conservation accounts it exactly. This is
    /// the inventory carrying cost that stops a satiated agent from hoarding its
    /// way out of the market — the staple decays, hunger returns, so it must keep
    /// acquiring (buying or producing). Deliberately does NOT spoil the chain's
    /// intermediates (grain/flour) — their small working stocks and large
    /// bootstrap seed buffers must survive — nor durable goods (WOOD, SALT,
    /// tools, money). A no-op unless enabled, so every other settlement is
    /// byte-identical. Deterministic: integer floor decay, id-ordered.
    pub(super) fn run_spoilage(&mut self, report: &mut EconTickReport) {
        let bps = match self.chain.as_ref() {
            Some(chain) if chain.perishable_decay_bps > 0 => u64::from(chain.perishable_decay_bps),
            _ => return,
        };
        // S16: spoiled bread is a true sink — draw it produced-first from the provenance
        // ledger. `None` (every shipped S16 config sets `perishable_decay_bps = 0`, so this
        // phase never runs there) or off the ledger leaves it untouched.
        let provenance_bread = if self.bread_provenance_active() {
            self.provenance_bread_good()
        } else {
            None
        };
        // S21d.1: spoiled bread is a true sink for the acquisition ledger too — debit it FIFO.
        let acquisition_bread = if self.acquisition_ledger_active() {
            self.acquisition_food_good()
        } else {
            None
        };
        // Spoil the **staple** food a satiated agent hoards (and the subsistence
        // food, if any) — NOT the chain's intermediates (grain/flour), whose
        // small working stocks and large bootstrap seed buffers must survive for
        // the chain to run. Targeting the satiation hoard is the point: when the
        // staple decays, hunger returns and the holder must re-enter the market.
        let mut perishable = vec![self.known.hunger];
        if let Some(subsistence) = self.known.subsistence {
            if subsistence != self.known.hunger {
                perishable.push(subsistence);
            }
        }
        // Also pressure the raw-grain hoard (threshold-protected, so the miller's
        // small working stock is exempt) so gatherers must sell before it rots.
        if let Some(chain) = self.chain.as_ref() {
            let grain = chain.content.grain();
            if !perishable.contains(&grain) {
                perishable.push(grain);
            }
        }
        // Carrying cost hits only HOARDS: the portion of holdings above a free
        // storage threshold decays. Working stock and a baker's fresh
        // about-to-be-sold output (both small) sit under the threshold and are
        // exempt, so spoilage curbs hoarding without destroying production.
        const FREE_STORAGE: u64 = 20;
        let decay = |held: u64| -> u64 { held.saturating_sub(FREE_STORAGE) * bps / 10_000 };
        let live = self.live_colonist_slots.clone();
        for &good in &perishable {
            for &slot in &live {
                let id = self.colonists[slot].id;
                let held = u64::from(self.society.agents.get(id).map_or(0, |a| a.stock.get(good)));
                let spoil = u32::try_from(decay(held)).unwrap_or(u32::MAX);
                if spoil > 0 && self.society.debit_stock(id, good, spoil) {
                    // DH.a (P1-1): the per-agent perishable-decay seam (a recorded sink). Commons
                    // spoilage below is NOT a per-agent event (§3.2 R5-5), so it is not emitted.
                    self.closure_emit(closure::ClosureEventKind::Spoilage {
                        agent: id,
                        good,
                        qty: spoil,
                    });
                    *report.spoiled.entry(good).or_insert(0) += u64::from(spoil);
                    if Some(good) == provenance_bread {
                        self.bread_provenance.sink(id, u64::from(spoil));
                    }
                    if Some(good) == acquisition_bread {
                        self.acquisition.sink(id, u64::from(spoil));
                    }
                }
            }
            let commons_held = self.commons_stock.get(&good).copied().unwrap_or(0);
            let commons_spoil = decay(commons_held);
            if commons_spoil > 0 {
                if let Some(qty) = self.commons_stock.get_mut(&good) {
                    *qty -= commons_spoil;
                    *report.spoiled.entry(good).or_insert(0) += commons_spoil;
                }
            }
        }
    }
    /// ROLE-CHOICE phase (G3b): each living colonist holding latent production
    /// capital (its [`Colonist::latent`] recipe) re-appraises that recipe against
    /// the realized prices it can observe and its own value scale, adopting the
    /// producer vocation when the spread pays and reverting to
    /// [`Vocation::Unassigned`] when it does not. A no-op without a chain and for
    /// every colonist whose `latent` is `None` (gatherers, consumers, and the
    /// **seeded** G3a producers — so the G3a config and digest are unchanged).
    ///
    /// **G5b gating — role-choice follows money.** The appraisal weighs a recipe's
    /// realized *money* spread, which exists only once a money good is priced. On a
    /// designated-money settlement (G3a/G3b) that holds from tick 0 (the money good is
    /// GOLD), so this is unchanged. On a G5b barter-start frontier there is no money
    /// good — and so no money spread — until promotion, so role-choice is **gated on
    /// the post-promotion money phase**: pre-promotion (barter) no producer role is
    /// ever adopted, and a division of labor emerges only AFTER a medium of exchange
    /// does (the load-bearing economic ordering; the spread is also `None` during
    /// barter, but the gate makes the ordering explicit rather than incidental).
    ///
    /// The decision is **ordinal**: it routes entirely through
    /// [`recipe_adoption_pays_for_money`] (econ's M2.5
    /// [`appraise_project_bundle_for_money`]), which asks whether running the recipe —
    /// selling its output at the realized output price for a future receivable, costing
    /// the realized input price plus the operating cost — newly provisions a
    /// future-**money** want on the colonist's *own* scale without breaking a higher
    /// want. The money good is the settlement's *current* one (GOLD when designated,
    /// the emerged medium post-promotion), so the appraisal and the market agree on
    /// what "money" is. There is no scalar profit number and no argmax across
    /// colonists: each decides for itself, in id order (the §pillar-1 "colonists act"
    /// rule applied to occupation). Re-running it every tick is what makes a role
    /// sticky while the spread holds and revert when it collapses. Deterministic:
    /// integer state, no RNG, id-ordered.
    pub(super) fn run_role_choice(&mut self) -> bool {
        let Some(chain) = &self.chain else {
            return false;
        };
        // Gate on the money phase: a producer appraises a realized money spread, which
        // exists only once a money good is priced. Designated-money settlements always
        // pass here (current_money_good is GOLD from tick 0, so G3a/G3b are unchanged);
        // a barter-start frontier stays in the no-role barter phase until promotion.
        let Some(money_good) = self.current_money_good() else {
            return false;
        };
        // Pull the content data into owned locals so the `&self.chain` borrow is
        // released before the loop mutates `self.colonists` (disjoint fields, but
        // the borrow checker needs the chain borrow gone first).
        let mill_recipe = chain.content.mill_recipe().clone();
        let bake_recipe = chain.content.bake_recipe().clone();
        let grain = chain.content.grain();
        let flour = chain.content.flour();
        let bread = chain.content.bread();
        let mill_good = chain.content.mill();
        let oven_good = chain.content.oven();
        let operating_cost = chain.operating_cost;
        let recurring_motive = chain.recurring_motive;
        // S7.1: when tool-acquisition eligibility is on, a colonist that HOLDS the
        // required tool is admitted to this appraisal even with no seeded `latent`.
        let tool_eligibility = chain.tool_acquisition_eligibility;
        // S11: route each colonist's per-agent fallible OUTPUT-price forecast into the
        // adopt appraisal instead of the raw realized price (input price stays observed).
        let entrepreneurial = chain.entrepreneurial_forecasts;
        // C3R.h (L2): value the recipe INPUT at a fresh non-self reservation ask instead
        // of the stale last-trade realized price. Off for every existing config.
        let stale_input_price_fix = chain.stale_input_price_fix;
        let mortal_only = self.mortal_chain_producers_active();
        let count_c3rb_rejections = self.mortal_producer_inheritance_active();
        let tick = self.society.tick.0;
        let mut changed = false;

        for &slot in &self.live_colonist_slots {
            let colonist = &self.colonists[slot];
            if mortal_only && colonist.lifespan.is_none() {
                continue;
            }
            // The recipe(s) this colonist may (re)appraise this tick, in deterministic
            // mill-before-oven order. A seeded `latent` yields exactly its one specialty
            // (the pre-S7 path; with the gate off this is the only branch, so role-choice
            // is byte-identical). Under S7.1 a colonist with no seeded `latent` yields the
            // recipe whose durable tool it now HOLDS (a built or handed mill/oven) — and
            // BOTH, when an estate transfer or inheritance leaves it holding a mill and an
            // oven, so the oven is appraised too instead of being stranded behind a
            // hard-coded mill-first pick. A colonist that is neither latent nor a
            // tool-holder keeps its vocation untouched. Re-appraising an already-adopted
            // tool-holder each tick is what lets it de-adopt when its spread collapses.
            let mut candidates: [Option<RecipeId>; 2] = [None, None];
            match colonist.latent {
                Some(recipe) => candidates[0] = Some(recipe),
                None if tool_eligibility => {
                    if let Some(agent) = self.society.agents.get(colonist.id) {
                        if agent.stock.get(mill_good) > 0 {
                            candidates[0] = Some(RecipeId::Mill);
                        }
                        if agent.stock.get(oven_good) > 0 {
                            candidates[1] = Some(RecipeId::Bake);
                        }
                    }
                }
                None => {}
            }
            if candidates.iter().all(Option::is_none) {
                continue;
            }
            let id = colonist.id;
            // S11: the colonist's heritable forecast bias (×1.0 = neutral). A Copy value,
            // so no borrow is held into the appraisal below.
            let forecast_bias = colonist.culture.forecast_bias_bps;
            // Adopt the FIRST candidate whose recipe pays on this colonist's own scale
            // (mill before oven). A colonist runs ONE vocation, so a holder of both tools
            // commits to one recipe; appraising both means the oven is chosen when the
            // milling spread does not pay (and vice versa) rather than the mill always
            // winning by position. For a seeded latent (one candidate) this is the
            // pre-S7 appraisal unchanged.
            let mut adoption: Option<Vocation> = None;
            for recipe_id in candidates.iter().flatten().copied() {
                let (recipe, output_good, stale_input_price, adopted) = match recipe_id {
                    RecipeId::Mill => (
                        &mill_recipe,
                        flour,
                        self.society.realized_price(grain),
                        Vocation::Miller,
                    ),
                    RecipeId::Bake => (
                        &bake_recipe,
                        bread,
                        self.society.realized_price(flour),
                        Vocation::Baker,
                    ),
                    // No other recipe is a latent specialty (set only at generation).
                    _ => continue,
                };
                // Preserve C3R.g's rejection precedence: an absent OUTPUT price wins even
                // when the L2 input proxy is also absent.
                let realized_output = self.society.realized_price(output_good);
                let output_price = {
                    let agent = self
                        .society
                        .agents
                        .get(id)
                        .expect("living colonist resolves in the arena");
                    if entrepreneurial {
                        forecast_output_price(agent, output_good, realized_output, forecast_bias)
                    } else {
                        realized_output
                    }
                };
                if stale_input_price_fix && output_price.is_none() {
                    self.saving_allocation_obs
                        .role_choice_diag
                        .observe(recipe_id, RoleChoiceReason::PriceAbsent);
                    if count_c3rb_rejections {
                        self.producer_recipe_pay_rejections =
                            self.producer_recipe_pay_rejections.saturating_add(1);
                    }
                    continue;
                }

                // Off retains the stale realized-price path. On uses the minimum
                // non-self holder reservation ask. With no proxy, decline explicitly:
                // passing `None` to the appraisal would manufacture a free input.
                let input_price = if stale_input_price_fix {
                    match recipe.input_good {
                        None => None,
                        // The per-unit ask, matching the per-unit `realized_price` it
                        // replaces: both consumers scale it by the recipe's `input_qty`.
                        Some((input_good, _input_qty)) => {
                            match self.fresh_input_ask(id, input_good, money_good) {
                                Some(fresh) => Some(fresh),
                                None => {
                                    self.saving_allocation_obs
                                        .role_choice_diag
                                        .observe(recipe_id, RoleChoiceReason::InputPriceAbsent);
                                    if count_c3rb_rejections {
                                        self.producer_recipe_pay_rejections =
                                            self.producer_recipe_pay_rejections.saturating_add(1);
                                    }
                                    continue;
                                }
                            }
                        }
                    }
                } else {
                    stale_input_price
                };
                // Return the exact appraised price and profit test with `pays`; the
                // diagnostic observes the decision values without re-pricing.
                let (pays, margin_positive) = {
                    let agent = self
                        .society
                        .agents
                        .get(id)
                        .expect("living colonist resolves in the arena");
                    let base_pays = recipe_adoption_pays_for_money(
                        agent,
                        recipe,
                        output_price,
                        input_price,
                        tick,
                        operating_cost,
                        money_good,
                    );
                    // Recurring owner-operator motive: also keep the role while the recipe
                    // is simply profitable at the appraised output price, so a producer
                    // whose savings ladder is full does not retire (consumption recurs — it
                    // keeps producing to keep eating). A no-op unless enabled.
                    let margin_positive =
                        recipe_is_profitable(recipe, output_price, input_price, operating_cost);
                    let pays = base_pays || (recurring_motive && margin_positive);
                    (pays, margin_positive)
                };
                // C3R.g: classify without steering. `pays` is the ground truth of
                // adoption, so it is matched FIRST — an accepted role is always Accepts
                // regardless of margin. The rejection buckets are then ordered and
                // disjoint: absent output price, then a non-positive yield-aware margin
                // (`revenue > input cost + operating cost`), then an ordinal decline at a
                // positive margin. Exhaustive by construction, without relying on any
                // cross-crate appraisal invariant relating `pays` to the margin.
                let reason = match output_price {
                    None => RoleChoiceReason::PriceAbsent,
                    Some(_) if pays => RoleChoiceReason::Accepts,
                    Some(_) if !margin_positive => RoleChoiceReason::MarginNonpositive,
                    Some(_) => RoleChoiceReason::OrdinalDecline,
                };
                self.saving_allocation_obs
                    .role_choice_diag
                    .observe(recipe_id, reason);
                if pays {
                    adoption = Some(adopted);
                    break;
                } else if count_c3rb_rejections {
                    self.producer_recipe_pay_rejections =
                        self.producer_recipe_pay_rejections.saturating_add(1);
                }
            }
            // When no candidate pays: a seeded latent or an adopted producer reverts to
            // Unassigned (the pre-S7 behaviour — it holds its tool, idle). An S7
            // tool-holder that is still feeding itself by gathering/consuming (it
            // acquired a tool but has not yet adopted) keeps that survival role rather
            // than being stranded Unassigned — it tries again next tick.
            let next = match adoption {
                Some(adopted) => adopted,
                None => match self.colonists[slot].vocation {
                    Vocation::Miller | Vocation::Baker | Vocation::Unassigned => {
                        Vocation::Unassigned
                    }
                    other => other,
                },
            };
            if self.colonists[slot].vocation != next {
                let previous = self.colonists[slot].vocation;
                if !self.role_choice_switch_ready(id, previous, next) {
                    if count_c3rb_rejections && matches!(next, Vocation::Miller | Vocation::Baker) {
                        self.producer_adoption_rejections =
                            self.producer_adoption_rejections.saturating_add(1);
                    }
                    continue;
                }
                if mortal_only
                    && self.mortal_producer_old_age_deaths > 0
                    && self.colonists[slot].lifespan.is_some()
                    && !matches!(previous, Vocation::Miller | Vocation::Baker)
                    && matches!(next, Vocation::Miller | Vocation::Baker)
                {
                    self.role_readoptions = self.role_readoptions.saturating_add(1);
                }
                if self.mortal_producer_inheritance_active()
                    && !matches!(previous, Vocation::Miller | Vocation::Baker)
                    && matches!(next, Vocation::Miller | Vocation::Baker)
                {
                    let inherited_tool = match next {
                        Vocation::Miller => {
                            self.producer_tool_inheritors.contains(&(id, mill_good))
                        }
                        Vocation::Baker => self.producer_tool_inheritors.contains(&(id, oven_good)),
                        _ => false,
                    };
                    let still_holds_tool =
                        self.society.agents.get(id).is_some_and(|agent| match next {
                            Vocation::Miller => agent.stock.get(mill_good) > 0,
                            Vocation::Baker => agent.stock.get(oven_good) > 0,
                            _ => false,
                        });
                    if inherited_tool && still_holds_tool {
                        self.heir_tool_adoptions = self.heir_tool_adoptions.saturating_add(1);
                    }
                    // DH.b (impl-69): the re-adoption succession event at the real role-choice
                    // seam — an heir with an inheritance record for the ADOPTED role's tool
                    // flips into the producer vocation. Possession at the adoption instant is
                    // recorded, not assumed (the classifier's possession bit reads it).
                    if self.closure_active() && inherited_tool {
                        let tool = match next {
                            Vocation::Miller => mill_good,
                            _ => oven_good,
                        };
                        if let Some(class) = self.closure_class_of(id) {
                            self.burden.adoptions.push(burden::BurdenRoleAdopted {
                                tick: self.econ_tick,
                                class,
                                heir: id,
                                tool,
                                role: next,
                                holds_tool: still_holds_tool,
                            });
                        }
                    }
                }
                // R1-8 (DH.b): a vocation transition is a role RELABEL only — it must preserve
                // the colonist's identity row (age, lifespan, seed, parent, household) and its
                // fixed closure class, and an adoption never creates or refreshes a lifespan.
                #[cfg(debug_assertions)]
                let identity_before = {
                    let c = &self.colonists[slot];
                    (
                        c.age,
                        c.lifespan,
                        c.seed,
                        c.parent,
                        c.household,
                        self.closure_class_of(id),
                    )
                };
                self.colonists[slot].vocation = next;
                #[cfg(debug_assertions)]
                {
                    let c = &self.colonists[slot];
                    debug_assert_eq!(
                        identity_before,
                        (
                            c.age,
                            c.lifespan,
                            c.seed,
                            c.parent,
                            c.household,
                            self.closure_class_of(id),
                        ),
                        "a vocation transition preserves identity, lifespan, and class (R1-8)"
                    );
                }
                changed = true;
            }
            // Acceptance and occupancy are different observations: a Gatherer may
            // accept Bake while the switch-readiness gate keeps it gathering. Stamp
            // only the vocation actually held after role choice has settled.
            if self.colonists[slot].vocation == Vocation::Baker {
                self.saving_allocation_obs
                    .role_choice_diag
                    .observe_baker_hold(self.econ_tick);
            }
        }
        changed
    }
    /// PRODUCTIVE RE-ENTRY (S6 — provisioning at scale). A gated, default-OFF
    /// `econ_tick` phase that runs each live **spatial non-lineage** colonist through
    /// a two-sided hysteresis on its own hunger:
    ///
    /// - **Re-enter** (hunger ≥ [`ChainConfig::reentry_hunger_in`] and **not already
    ///   feeding itself on the edible grain node**): adopt edible-grain gathering. An
    ///   idle [`Consumer`](Vocation::Consumer) (no node, produces nothing) becomes a
    ///   grain [`Gatherer`](Vocation::Gatherer); a `Gatherer` mis-allocated to a
    ///   non-edible (WOOD) node is re-pointed to the edible grain node (a hungry actor
    ///   gathers food before wood — hunger outranks wood-for-trade on its scale).
    /// - **Revert** (hunger < [`ChainConfig::reentry_hunger_out`] and currently
    ///   displaced from its home role): resume the **home** role captured at
    ///   generation — a WOOD gatherer returns to WOOD (keeping the WOOD supply alive),
    ///   an idle consumer goes idle. The gap `H_in − H_out` is the hysteresis band: a
    ///   colonist inside it holds its current node, so the phase does not thrash
    ///   node-to-node every tick.
    ///
    /// Scope (Base Fact 4): only colonists with a **world agent** and `household ==
    /// None` whose HOME is an untooled spatial role are touched. Lineage members are
    /// hearth-fed (`deliver_demography_provisions`); the latent/seeded **tooled** chain
    /// producers (Miller/Baker/Scholar/Confectioner and the latent pool) feed from
    /// `run_producer_subsistence` and belong to the S7 capital-goods milestone, never
    /// re-entry. The vocation flip is between two **untooled** spatial roles whose
    /// econ value scale is identical (`production_specialty` is `None` for both
    /// `Consumer` and `Gatherer`), so it perturbs no resting quote and needs no scale
    /// regeneration — it only steers the next fast loop's `assign_idle_gatherer_tasks`.
    /// It mints nothing: a re-entrant feeds by gathering grain (the existing conserved
    /// node-regen source) and eating it (`subsistence_on_grain`).
    ///
    /// A no-op unless [`ChainConfig::productive_reentry`] is set AND raw grain is
    /// edible (so the gathered grain actually relieves hunger), so every existing
    /// run is byte-identical. Deterministic: slot-ordered, integer thresholds,
    /// nothing drawn.
    pub(super) fn run_productive_reentry(&mut self) {
        let Some(chain) = &self.chain else {
            return;
        };
        if !chain.productive_reentry {
            return;
        }
        let grain = chain.content.grain();
        // Without an edible-grain fallback the gathered grain would not feed anyone,
        // so re-entry would relabel without provisioning — stay inert.
        if self.known.subsistence != Some(grain) {
            return;
        }
        let h_in = chain.reentry_hunger_in;
        let h_out = chain.reentry_hunger_out;
        // Single canonical edible node: `node_for_good` resolves the lowest-id node
        // yielding grain, and the shipped frontier seeds exactly one grain node, so
        // `grain_node`/`on_grain` below are unambiguous. A future config that seeded two
        // grain-yielding nodes would read a gatherer home-assigned to the second as "not
        // on grain" and re-point it to the first (still edible, but it abandons its home
        // node) — revisit this resolution and `on_grain` before adding such configs.
        let Some(grain_node) = self.node_for_good(grain) else {
            return;
        };
        // S13: with spatial households on, a lineage member is spatial and may itself
        // re-enter grain gathering when hungry (carry rises → deposit), so it is no
        // longer skipped here. Off the flag the skip is exactly the pre-S13 gate
        // (`household.is_some()`), so every existing run is byte-identical.
        let spatial_active = self.spatial_households_active();
        for &slot in &self.live_colonist_slots {
            let colonist = &self.colonists[slot];
            // Pre-S13 lineage members are hearth-fed and skipped; the tooled chain
            // producers (latent or active Miller/Baker/Scholar/Confectioner) are the S7
            // path — skip both. This includes a formerly non-latent tool-holder that
            // adopted Miller/Baker earlier this same tick: its home role is still spatial,
            // but re-entry must not revert an active capital holder before market/
            // production. With spatial households on, a lineage member is no longer skipped.
            if (colonist.household.is_some() && !spatial_active)
                || colonist.latent.is_some()
                || matches!(
                    colonist.vocation,
                    Vocation::Miller | Vocation::Baker | Vocation::Scholar | Vocation::Confectioner
                )
            {
                continue;
            }
            // Re-enter only colonists whose HOME is an untooled spatial role: an idle
            // Consumer, a Gatherer, or a non-latent Unassigned (the spec's stranded
            // idle worker). Latent/seeded producers were already skipped above, so the
            // `Unassigned` arm never catches one of those; it stays for the non-latent
            // stranded case even though current generation produces no such colonist,
            // and is the home a fed re-entrant reverts to once relieved.
            if !matches!(
                colonist.home_vocation,
                Vocation::Consumer | Vocation::Gatherer | Vocation::Unassigned
            ) {
                continue;
            }
            let hunger = colonist.need.hunger;
            let on_grain =
                colonist.vocation == Vocation::Gatherer && colonist.node == Some(grain_node);
            let displaced =
                colonist.vocation != colonist.home_vocation || colonist.node != colonist.home_node;
            let (next_vocation, next_node) = if hunger >= h_in && !on_grain {
                // Hungry and not yet feeding on grain: adopt grain gathering.
                (Vocation::Gatherer, Some(grain_node))
            } else if hunger < h_out && displaced {
                if !self.reentry_revert_ready(colonist.id) {
                    continue;
                }
                // Fed re-entrant: revert to the home role it was displaced from.
                (colonist.home_vocation, colonist.home_node)
            } else {
                // In the hysteresis band, or already where it should be: hold.
                continue;
            };
            let colonist = &mut self.colonists[slot];
            if colonist.vocation != next_vocation || colonist.node != next_node {
                colonist.vocation = next_vocation;
                colonist.node = next_node;
            }
        }
    }
    /// CAPITAL FORMATION (S7.2 — producible capital goods). A gated, default-OFF
    /// `econ_tick` phase (after the scale regeneration, before role-choice) driving the
    /// **per-builder** project lifecycle: one builder, its OWN WOOD, its OWN labor.
    ///
    /// - **Advance + complete** every in-flight build by one labor unit (the builder's
    ///   own labor). On completion the durable tool credits the builder's own stock —
    ///   booked into `report.produced` (the produced side of the conserved build) —
    ///   and the formerly-non-latent builder is marked (observability). A build whose
    ///   builder has died is dropped: its WOOD was already consumed at the start tick.
    /// - **Start** a new build for each fed, non-latent colonist (in a survival/idle
    ///   role, holding no chain tool and no in-flight build) that holds enough saved
    ///   WOOD and whose entrepreneurial appraisal ([`capital_build_surplus`]) says the
    ///   tool's expected multi-period proceeds repay its build cost. The builder's own
    ///   WOOD is committed up front by [`start_project`] and booked into
    ///   `report.consumed_as_input` (the consumed side) — so the build conserves: WOOD
    ///   in at the start tick, the tool out at completion.
    ///
    /// Praxeological: each colonist decides for itself on its own value scale (hunger
    /// outranks building — a hungry colonist is skipped and gathers/feeds first), there
    /// is no global quota, no tool placement or transfer, and the WOOD + labor are the
    /// builder's own. Self-correcting: the appraisal is demand/price-driven, so once
    /// bread demand is met the per-run margin falls below the payback bar and no tool is
    /// built (the overinvestment guard).
    ///
    /// Returns `true` if any build completed this tick (so the caller regenerates the
    /// scales — the fresh tool-holder must carry its tool-anchor into the market step).
    /// A no-op unless [`ChainConfig::producible_capital`] is on and money has emerged,
    /// so every other run is byte-identical. Deterministic: slot-ordered, integer state.
    pub(super) fn run_capital_formation(
        &mut self,
        report: &mut EconTickReport,
        labor_used: &mut Vec<(AgentId, u32)>,
    ) -> bool {
        let Some(chain) = &self.chain else {
            return false;
        };
        if !chain.producible_capital {
            return false;
        }
        // Gate on the money phase: the build appraisal weighs realized money prices.
        if self.current_money_good().is_none() {
            return false;
        }
        // Pull content/knobs into owned locals so the `&self.chain` borrow is released
        // before the loops mutate `self.society`/`self.capital_builds`/`self.colonists`.
        let mill_recipe = chain.content.mill_recipe().clone();
        let bake_recipe = chain.content.bake_recipe().clone();
        let grain = chain.content.grain();
        let flour = chain.content.flour();
        let bread = chain.content.bread();
        let mill_good = chain.content.mill();
        let oven_good = chain.content.oven();
        let operating_cost = chain.operating_cost;
        let payback = chain.capital_payback_cycles;
        let wood_qty = chain.tool_build_wood;
        let build_labor = chain.tool_build_labor;
        let hunger_max = chain.capital_build_hunger_max;
        let per_agent = chain.per_agent_capital;
        let mortal_only = self.mortal_chain_producers_active();
        let tick = self.society.tick.0;

        let mut built = false;
        if per_agent {
            self.last_capital_decisions.clear();
        }

        // ---- 1. ADVANCE + COMPLETE in-flight builds (each its own labor).
        let mut finished: Vec<usize> = Vec::new();
        for bi in 0..self.capital_builds.len() {
            let builder = self.capital_builds[bi].builder;
            let slot = self.capital_builds[bi].slot;
            // Drop a build whose builder has died: its committed WOOD was already booked
            // `consumed_as_input` at the start tick, so the forfeit needs no further
            // booking (conservation already balanced — like an abandonment).
            let alive = self
                .colonist_slot_by_id
                .get(&builder)
                .is_some_and(|&s| self.colonists[s].alive);
            if !alive {
                finished.push(bi);
                continue;
            }
            // Advance with the builder's own labor (one unit per tick), then try to
            // complete it against the builder's own stock. A project already at its
            // required labor completes without an extra advance, so a one-labor build
            // started last tick is never charged N+1 units.
            {
                let build = &mut self.capital_builds[bi];
                if build.project.labor_advanced < build.template.required_labor
                    && advance_project(&mut build.project)
                {
                    labor_used.push((builder, 1));
                }
            }
            let tool = self.capital_builds[bi].project.output_good;
            let qty = self.capital_builds[bi].project.output_qty;
            let completed = match self.society.agents.get_mut(builder) {
                Some(agent) => {
                    let build = &mut self.capital_builds[bi];
                    complete_project_if_ready(&mut build.project, &build.template, &mut agent.stock)
                }
                None => false,
            };
            if completed {
                self.closure_emit(closure::ClosureEventKind::CapitalFormation {
                    agent: builder,
                    input: WOOD,
                    input_qty: 0,
                    tool,
                    tool_qty: qty,
                });
                *report.produced.entry(tool).or_insert(0) += u64::from(qty);
                self.tools_built = self.tools_built.saturating_add(u64::from(qty));
                self.record_mortal_capital_build_completion(slot, qty);
                // Tie the produced tool to its formerly-non-latent builder (test 6).
                if slot < self.colonists.len() && self.colonists[slot].latent.is_none() {
                    self.colonists[slot].acquired_tool = true;
                }
                built = true;
                finished.push(bi);
            }
        }
        for &bi in finished.iter().rev() {
            self.capital_builds.remove(bi);
        }
        if built {
            return true;
        }

        // ---- 2. START new builds. S10 (per_agent_capital): each eligible colonist runs
        // its OWN ordinal appraisal and any it accepts starts its own build — no global
        // stage choice, no first-eligible assignment, no single-in-flight gate (the
        // per-builder substrate is reused). Behind the gate; the S7 heuristic below is
        // byte-identical for every existing config.
        if per_agent {
            return self.start_per_agent_builds(
                report,
                labor_used,
                &PerAgentBuildParams {
                    mill_recipe: &mill_recipe,
                    bake_recipe: &bake_recipe,
                    grain,
                    flour,
                    bread,
                    mill_good,
                    oven_good,
                    operating_cost,
                    wood_qty,
                    build_labor,
                    hunger_max,
                    tick,
                },
            );
        }

        // ---- 2 (S7 heuristic). START a new build when a demand-anchored real-resource
        // investment appraisal clears. This is a settlement-level heuristic, NOT a
        // per-colonist ordinal-scale appraisal: the opportunity depends only on prices, so the
        // better-paying stage is appraised once (scalar margin x payback vs build cost);
        // it is then funded by the first eligible fed builder from its OWN WOOD + labor
        // (no tool placement, no quota). A fully individual ordinal appraisal is a
        // follow-on; here each eligible fed builder with enough WOOD can take the build.
        let wood_price = self
            .society
            .realized_price(WOOD)
            .map_or(operating_cost.max(1), |g| g.0);
        let flour_price = self.society.realized_price(flour);
        let grain_price = self.society.realized_price(grain);
        let bread_price = self.society.realized_price(bread);
        let appraisal = CapitalBuildAppraisal {
            operating_cost,
            wood_price,
            tool_build_wood: wood_qty,
            tool_build_labor: build_labor,
            payback_cycles: payback,
        };
        // Which tool to build is set by the chain's BOTTLENECK, anchored on the final
        // good's real demand — Menger's imputation in mechanism form: flour is worth
        // building a mill for only because bread is demanded and ovens turn flour into
        // it. Build only while BREAD is actually clearing (real demand for the chain's
        // output); when bread demand is met it stops clearing / its spread thins and
        // building stops — the demand-anchored brake. Given that demand, build the
        // scarcer stage: if the active bakers out-demand the active millers' flour
        // supply, the MILL is the bottleneck; otherwise flour is plentiful relative to
        // baking capacity, so an OVEN turns more of it into bread. This builds ovens
        // first (raising bread), pulls mills in only when bakers truly need flour, and
        // keeps the two stages balanced instead of flooding one — the naive
        // higher-margin-wins rule floods mills on a stale flour price and starves the
        // baker side. The chosen tool's own output must also have traded recently, and
        // its amortized margin must clear the payback bar.
        // The bottleneck is read from usable installed CAPACITY (the count of live
        // colonists HOLDING a tool), not active-producer counts and not whole-system
        // conserved totals: active counts loop (a just-built tool whose holder has not
        // yet adopted would read as zero capacity and drive an endless build of the same
        // stage), while a tool settled to the commons is conserved but inaccessible and
        // must not suppress replacement builds. Counting holders, not raw units, also
        // means a colonist that came to hold two tools of a kind (an inherited/transferred
        // stack) cannot overstate capacity: it still runs one vocation, one throughput.
        // The market clears one seller-side lot per producer per tick, so the practical
        // throughput ratio is one live mill holder to one live oven holder even when the
        // milling recipe emits a multi-unit flour batch.
        let held_mills = self.live_colonist_holder_count(mill_good);
        let held_ovens = self.live_colonist_holder_count(oven_good);
        let active_millers = self.living_count(Vocation::Miller) as u64;
        let active_bakers = self.living_count(Vocation::Baker) as u64;
        let oven_capacity = held_ovens;
        let mill_capacity = held_mills;
        // Utilization guard against idle-tool overbuild: only add a tool of a kind while
        // the kind already in the colony is close to fully employed — held tools at most
        // the active producers plus a small slack. The slack absorbs the emergent chain's
        // tick-to-tick adoption churn (a producer that briefly de-adopts still holds a
        // productive tool), so building is not stalled by a transient dip, while idle
        // tools cannot accumulate without bound: built capital tracks the active producer
        // count, the structural half of the overinvestment guard.
        let bread_signal = self.good_traded_within(bread, CAPITAL_BUILD_RECENCY)
            || (held_ovens == 0 && bread_price.is_some());
        let flour_signal = self.good_traded_within(flour, CAPITAL_BUILD_RECENCY)
            || (held_mills == 0 && flour_price.is_some());
        let choice: Option<(GoodId, ProjectTemplateId)> = if !bread_signal {
            // Build only while the chain's FINAL good (bread) is actually clearing —
            // real demand for the chain's output. When bread demand is met it stops
            // clearing / its spread thins and building stops (the demand brake). If a
            // stage has collapsed to ZERO usable capacity, a prior realized price is
            // enough to restart replacement building; no observed price still means no
            // appraisal basis.
            None
        } else if oven_capacity < mill_capacity {
            // Flour-milling capacity outruns baking capacity: add an OVEN to turn the
            // surplus flour into bread — unless idle ovens already sit unemployed.
            (held_ovens <= active_bakers.saturating_add(CAPITAL_IDLE_SLACK))
                .then(|| capital_build_surplus(&bake_recipe, bread_price, flour_price, &appraisal))
                .flatten()
                .map(|_| (oven_good, ProjectTemplateId::BuildOven))
        } else {
            // Baking capacity outruns milling: the bakers need more flour, so the
            // MILL is the bottleneck — gated on mills not already sitting idle, on
            // flour actually clearing (so the mill's output has a real buyer), and on
            // the milling spread paying.
            (held_mills <= active_millers.saturating_add(CAPITAL_IDLE_SLACK) && flour_signal)
                .then(|| capital_build_surplus(&mill_recipe, flour_price, grain_price, &appraisal))
                .flatten()
                .map(|_| (mill_good, ProjectTemplateId::BuildMill))
        };
        let Some((tool, template_id)) = choice else {
            if self.mortal_producer_inheritance_active() {
                self.producer_build_rejections = self.producer_build_rejections.saturating_add(1);
            }
            return built;
        };

        // Capital forms GRADUALLY: only one build is in flight at a time, so each new
        // tool is completed, adopted, and its price impact realized and re-appraised
        // before the next build starts — the entrepreneurial signal propagates and the
        // chain re-equilibrates, instead of a same-tick cluster of speculative idle
        // tools that whipsaws the intermediate price (the overinvestment guard). This is
        // pacing, not a quota: over the run the colony builds as many tools as the
        // demand-driven appraisal supports, and each builder still decides for itself.
        if !self.capital_builds.is_empty() {
            return built;
        }
        for idx in 0..self.live_colonist_slots.len() {
            let slot = self.live_colonist_slots[idx];
            let colonist = &self.colonists[slot];
            // Formerly-non-latent builders only (a seeded latent/producer already holds
            // a tool); only a fed colonist in a survival/idle role (not a producer).
            if mortal_only && colonist.lifespan.is_none() {
                continue;
            }
            if colonist.latent.is_some() {
                continue;
            }
            if !matches!(
                colonist.vocation,
                Vocation::Gatherer | Vocation::Consumer | Vocation::Unassigned
            ) {
                continue;
            }
            // Feed first: a hungry colonist gathers/feeds before investing in capital.
            if colonist.need.hunger > hunger_max {
                continue;
            }
            let id = colonist.id;
            // Skip a colonist that already has an in-flight build.
            if self.capital_builds.iter().any(|build| build.builder == id) {
                continue;
            }
            // Must hold no chain tool yet (else it is a producer/holder, not a builder)
            // and enough saved WOOD to fund the build from its OWN endowment.
            let can_fund = self.society.agents.get(id).is_some_and(|agent| {
                agent.stock.get(mill_good) == 0
                    && agent.stock.get(oven_good) == 0
                    && agent.stock.get(WOOD) >= wood_qty
            });
            if !can_fund {
                continue;
            }
            let template = match template_id {
                ProjectTemplateId::BuildOven => build_oven_template(tool, wood_qty, build_labor),
                _ => build_mill_template(tool, wood_qty, build_labor),
            };
            let pid = ProjectId(self.next_capital_project_id);
            // Commit the builder's own WOOD up front (booked consumed_as_input), then
            // advance one labor unit — mirroring the lab World BuildNet path (start then
            // advance). If that satisfies the labor requirement, complete immediately
            // so required_labor counts exact contributed units, not an extra wait tick.
            let started = match self.society.agents.get_mut(id) {
                Some(agent) => start_project(&template, &mut agent.stock, pid, Tick(tick)),
                None => None,
            };
            if let Some(mut project) = started {
                self.closure_emit(closure::ClosureEventKind::CapitalFormation {
                    agent: id,
                    input: WOOD,
                    input_qty: wood_qty,
                    tool: project.output_good,
                    tool_qty: 0,
                });
                *report.consumed_as_input.entry(WOOD).or_insert(0) += u64::from(wood_qty);
                if project.labor_advanced < template.required_labor && advance_project(&mut project)
                {
                    labor_used.push((id, 1));
                }
                self.next_capital_project_id = self.next_capital_project_id.wrapping_add(1);
                let completed = match self.society.agents.get_mut(id) {
                    Some(agent) => {
                        complete_project_if_ready(&mut project, &template, &mut agent.stock)
                    }
                    None => false,
                };
                if completed {
                    let qty = project.output_qty;
                    self.closure_emit(closure::ClosureEventKind::CapitalFormation {
                        agent: id,
                        input: WOOD,
                        input_qty: 0,
                        tool: project.output_good,
                        tool_qty: qty,
                    });
                    *report.produced.entry(project.output_good).or_insert(0) += u64::from(qty);
                    self.tools_built = self.tools_built.saturating_add(u64::from(qty));
                    self.record_mortal_capital_build_completion(slot, qty);
                    self.colonists[slot].acquired_tool = true;
                    built = true;
                } else {
                    self.capital_builds.push(CapitalBuild {
                        builder: id,
                        slot,
                        template,
                        project,
                    });
                }
                // One new build per tick (the gradual-accumulation pacing above).
                break;
            }
        }

        built
    }
    pub(super) fn run_earned_provisioning_transfers(&mut self) -> Vec<AgentId> {
        if !self.earned_provisioning_active() {
            return Vec::new();
        }
        let Some(bread) = self.provenance_bread_good() else {
            return Vec::new();
        };
        let price = self.society.realized_price(bread).map_or(1, |g| g.0.max(1));
        let producers = self.producer_house_producers();
        let mut funded_bid_members = Vec::new();
        for (producer, household) in producers {
            let members = self.producer_house_members(household);
            for member in members {
                if member == producer || !self.has_unprovided_now_bread_want(member, bread) {
                    continue;
                }
                let member_gold = self.free_agent_gold(member).0;
                let gap = price.saturating_sub(member_gold);
                if gap == 0 {
                    continue;
                }
                let amount = Gold(gap).min(self.free_agent_gold(producer));
                if amount == Gold::ZERO {
                    continue;
                }
                let gold_before = self.total_gold();
                if self.society.transfer_gold(producer, member, amount) {
                    // DH.a rule 4: bucket-preserving gold-only provisioning transfer.
                    self.closure_note_gold_transfer(producer, member, amount);
                    let (_, endowed, untracked) =
                        self.earned_provisioning_transfer_gold_provenance(producer, member, amount);
                    self.earned_provisioning.stats.endowment_funded_provisioning = self
                        .earned_provisioning
                        .stats
                        .endowment_funded_provisioning
                        .saturating_add(endowed)
                        .saturating_add(untracked);
                    self.earned_provisioning.stats.provisioning_transfers = self
                        .earned_provisioning
                        .stats
                        .provisioning_transfers
                        .saturating_add(1);
                    self.earned_provisioning.stats.provisioning_gold = self
                        .earned_provisioning
                        .stats
                        .provisioning_gold
                        .saturating_add(amount);
                    debug_assert_eq!(
                        gold_before,
                        self.total_gold(),
                        "earned provisioning must conserve total GOLD"
                    );
                    funded_bid_members.push(member);
                }
            }
        }
        funded_bid_members
    }
    pub(super) fn run_producer_stock_provisioning_control(&mut self) {
        if !self.producer_stock_provisioning_control_active() {
            return;
        }
        let Some(bread) = self.provenance_bread_good() else {
            return;
        };
        let producers = self.producer_house_producers();
        for (producer, household) in producers {
            let members = self.producer_house_members(household);
            for member in members {
                if member == producer || !self.has_unprovided_now_bread_want(member, bread) {
                    continue;
                }
                if self.stock_of_id(producer, bread) == 0 {
                    break;
                }
                if !self.society.debit_stock(producer, bread, 1) {
                    continue;
                }
                if self.society.credit_stock(member, bread, 1) {
                    if self.bread_provenance_active() && Some(bread) == self.provenance_bread_good()
                    {
                        self.bread_provenance.transfer(producer, member, 1);
                    }
                    if self.acquisition_ledger_active()
                        && Some(bread) == self.acquisition_food_good()
                    {
                        self.acquisition.transfer_preserve(producer, member, 1);
                    }
                } else {
                    let credited_back = self.society.credit_stock(producer, bread, 1);
                    debug_assert!(credited_back, "stock provisioning rollback must fit");
                }
            }
        }
    }
    pub(super) fn run_birth_stock_sufficiency_control(&mut self) -> Vec<usize> {
        if !self.birth_stock_control_active() {
            return Vec::new();
        }
        // The recurring control: ordinary donors (no producer exclusion), no origin flag.
        self.inject_birth_stock(false).0
    }
    /// C3R.e (impl-67): the A1 one-shot ignition — the SAME conserved injection machinery, fired
    /// ONCE at `birth_stock_ignition_at` behind its OWN latch (independent of the SufficiencyControl
    /// mode, which the recurring path gates on). Donors are restricted to NON-producer households
    /// and every moved unit is origin-flagged. `ignition_injected_qty` records the dose — the
    /// detector for under-dosing (`< 24 → IgnitionShortfall`), since the driver's shortfall counter
    /// alone cannot see an ineligible household that receives nothing.
    pub(super) fn run_birth_stock_ignition(&mut self) {
        let Some(at) = self
            .chain
            .as_ref()
            .and_then(|chain| chain.birth_stock_ignition_at)
        else {
            return;
        };
        if self.econ_tick != at {
            return;
        }
        let (_injected, injected_qty) = self.inject_birth_stock(true);
        self.ignition_injected_qty = injected_qty;
    }
    pub(super) fn run_earned_provisioning_market_attribution(&mut self, spot_trades_start: usize) {
        if !self.earned_provisioning_ledger_active() {
            return;
        }
        let Some(bread) = self.provenance_bread_good() else {
            return;
        };
        let trades: Vec<_> = self.society.trades[spot_trades_start..].to_vec();
        for trade in trades {
            let Some(payment) = gold_mul_qty(trade.price, trade.qty) else {
                continue;
            };
            let (buyer_lots, untracked) = self.debit_earned_provisioning_lots(trade.buyer, payment);
            let Some(seller_household) = self.colonist_household(trade.seller) else {
                continue;
            };
            if !self.is_producer_household(seller_household) {
                continue;
            }
            let buyer_household = self.colonist_household(trade.buyer);
            if trade.good != bread {
                if buyer_household == Some(seller_household) {
                    assert_eq!(
                        untracked,
                        Gold::ZERO,
                        "intra-household sale payment must be fully tracked by the earned-provisioning ledger"
                    );
                    self.credit_earned_provisioning_lots(trade.seller, buyer_lots);
                } else {
                    // Disclosed, class-tracked: a producer-class buyer means this revenue is
                    // internal producer-to-producer recirculation (e.g. a Miller's flour sold
                    // to a Baker) — it funds provisioning as genuinely earned income for the
                    // FIFO, but must stay visible to the accounting-loop reading rather than
                    // bypass the bread-only class split.
                    let producer_class_buyer =
                        buyer_household.is_some_and(|h| self.is_producer_household(h));
                    if producer_class_buyer {
                        self.earned_provisioning
                            .stats
                            .non_bread_producer_class_earned = self
                            .earned_provisioning
                            .stats
                            .non_bread_producer_class_earned
                            .saturating_add(payment);
                    } else {
                        self.earned_provisioning.stats.non_bread_external_earned = self
                            .earned_provisioning
                            .stats
                            .non_bread_external_earned
                            .saturating_add(payment);
                    }
                    self.credit_earned_provisioning_lot(
                        trade.seller,
                        EarnedGoldLot {
                            source: EarnedGoldSource::Earned,
                            amount: payment,
                        },
                    );
                }
                continue;
            }
            if buyer_household == Some(seller_household) {
                assert_eq!(
                    untracked,
                    Gold::ZERO,
                    "intra-household sale payment must be fully tracked by the earned-provisioning ledger"
                );
                self.credit_earned_provisioning_lots(trade.seller, buyer_lots);
                self.earned_provisioning.stats.intra_household_sales = self
                    .earned_provisioning
                    .stats
                    .intra_household_sales
                    .saturating_add(payment);
                self.earned_provisioning.stats.intra_household_bread_trades = self
                    .earned_provisioning
                    .stats
                    .intra_household_bread_trades
                    .saturating_add(1);
                continue;
            }

            let class = self.earned_buyer_class(trade.buyer, buyer_household);
            match class {
                EarnedBuyerClass::ImmortalConsumer => {
                    self.earned_provisioning.stats.from_immortal_consumers = self
                        .earned_provisioning
                        .stats
                        .from_immortal_consumers
                        .saturating_add(payment);
                    self.earned_provisioning.stats.genuine_external_revenue = self
                        .earned_provisioning
                        .stats
                        .genuine_external_revenue
                        .saturating_add(payment);
                    self.earned_provisioning.stats.genuine_external_bread_trades = self
                        .earned_provisioning
                        .stats
                        .genuine_external_bread_trades
                        .saturating_add(1);
                }
                EarnedBuyerClass::Gatherer => {
                    self.earned_provisioning.stats.from_gatherers = self
                        .earned_provisioning
                        .stats
                        .from_gatherers
                        .saturating_add(payment);
                    self.earned_provisioning.stats.genuine_external_revenue = self
                        .earned_provisioning
                        .stats
                        .genuine_external_revenue
                        .saturating_add(payment);
                    self.earned_provisioning.stats.genuine_external_bread_trades = self
                        .earned_provisioning
                        .stats
                        .genuine_external_bread_trades
                        .saturating_add(1);
                }
                EarnedBuyerClass::Lineage => {
                    self.earned_provisioning.stats.from_lineage = self
                        .earned_provisioning
                        .stats
                        .from_lineage
                        .saturating_add(payment);
                    self.earned_provisioning.stats.genuine_external_revenue = self
                        .earned_provisioning
                        .stats
                        .genuine_external_revenue
                        .saturating_add(payment);
                    self.earned_provisioning.stats.genuine_external_bread_trades = self
                        .earned_provisioning
                        .stats
                        .genuine_external_bread_trades
                        .saturating_add(1);
                }
                EarnedBuyerClass::OtherProducerHousehold => {
                    self.earned_provisioning
                        .stats
                        .from_other_producer_households = self
                        .earned_provisioning
                        .stats
                        .from_other_producer_households
                        .saturating_add(payment);
                }
            }
            self.earned_provisioning.stats.external_earned_revenue = self
                .earned_provisioning
                .stats
                .external_earned_revenue
                .saturating_add(payment);
            self.earned_provisioning.stats.external_bread_trades = self
                .earned_provisioning
                .stats
                .external_bread_trades
                .saturating_add(1);
            let current = self
                .earned_provisioning
                .per_seller_external
                .get(&trade.seller)
                .copied()
                .unwrap_or(Gold::ZERO);
            self.earned_provisioning
                .per_seller_external
                .insert(trade.seller, current.saturating_add(payment));
            self.credit_earned_provisioning_lot(
                trade.seller,
                EarnedGoldLot {
                    source: EarnedGoldSource::Earned,
                    amount: payment,
                },
            );
        }
        let stats = self.earned_provisioning.stats;
        let split = stats
            .from_immortal_consumers
            .saturating_add(stats.from_gatherers)
            .saturating_add(stats.from_lineage)
            .saturating_add(stats.from_other_producer_households);
        debug_assert_eq!(
            split, stats.external_earned_revenue,
            "earned-provisioning class split must equal total external producer-house revenue"
        );
        debug_assert!(
            stats.genuine_external_revenue <= stats.external_earned_revenue,
            "genuine external revenue must be a subset of external earned revenue"
        );
    }
    pub(super) fn run_commitment_norm_imitation(&mut self) {
        if !self.commitment_norm_spread_active() {
            return;
        }
        if self.abandonable_norm_active() {
            self.run_abandonable_norm_imitation();
            return;
        }
        let Some(chain) = self.chain.as_ref() else {
            return;
        };
        let period = chain.imitation_period;
        let window = chain.imitation_window;
        let margin = chain.imitation_margin_bps;
        let radius = chain.imitation_radius;
        let max_models = chain.imitation_max_models;
        let food_target = chain.food_window_target;
        let no_imitation = chain.no_imitation;
        let random_imitation = chain.random_imitation;
        let salt_in_score = chain.salt_in_score;
        if no_imitation || period == 0 || window == 0 || self.econ_tick == 0 {
            return;
        }
        if !self.econ_tick.is_multiple_of(period) {
            return;
        }

        let live = self.live_colonist_slots.clone();
        let exchange_pos = self
            .world
            .stockpile(self.exchange)
            .map(|stockpile| stockpile.pos);
        let adopters_at_start: BTreeSet<AgentId> = live
            .iter()
            .filter_map(|&slot| {
                let colonist = &self.colonists[slot];
                (colonist.alive && colonist.adopts_commitment_norm).then_some(colonist.id)
            })
            .collect();
        let mut adoptions: Vec<(usize, CommitmentNormCopyRow)> = Vec::new();

        for &slot in &live {
            let colonist = &self.colonists[slot];
            if !colonist.alive || colonist.adopts_commitment_norm {
                continue;
            }
            let Some(own_score) =
                self.commitment_norm_score(slot, window, food_target, salt_in_score)
            else {
                continue;
            };
            let models = self.commitment_norm_observation_set(
                slot,
                &live,
                exchange_pos,
                window,
                radius,
                max_models,
            );
            if models.is_empty() {
                continue;
            }
            let chosen = if random_imitation {
                let best = models
                    .iter()
                    .filter_map(|&model_slot| {
                        self.commitment_norm_score(model_slot, window, food_target, salt_in_score)
                            .map(|score| (model_slot, score))
                    })
                    .max_by_key(|&(model_slot, score)| {
                        (
                            score.total_bps,
                            std::cmp::Reverse(self.colonists[model_slot].id.0),
                        )
                    })
                    .filter(|&(_, score)| {
                        score.total_bps.saturating_sub(own_score.total_bps) >= margin
                    });
                let draw = deterministic_mix64(
                    COMMITMENT_NORM_RANDOM_SALT
                        ^ self.econ_tick.rotate_left(17)
                        ^ colonist.id.0.rotate_left(7),
                );
                let idx = (draw as usize) % models.len();
                let model_slot = models[idx];
                best.filter(|&(best_slot, _)| best_slot == model_slot)
            } else {
                models
                    .iter()
                    .filter_map(|&model_slot| {
                        self.commitment_norm_score(model_slot, window, food_target, salt_in_score)
                            .map(|score| (model_slot, score))
                    })
                    .max_by_key(|&(model_slot, score)| {
                        (
                            score.total_bps,
                            std::cmp::Reverse(self.colonists[model_slot].id.0),
                        )
                    })
                    .filter(|&(_, score)| {
                        score.total_bps.saturating_sub(own_score.total_bps) >= margin
                    })
            };
            let Some((model_slot, model_score)) = chosen else {
                continue;
            };
            let model = &self.colonists[model_slot];
            if !adopters_at_start.contains(&model.id) {
                continue;
            }
            let driver = commitment_norm_copy_driver(own_score, model_score, salt_in_score);
            adoptions.push((
                slot,
                CommitmentNormCopyRow {
                    tick: self.econ_tick,
                    copier: colonist.id.0,
                    model: model.id.0,
                    copied_norm_bit: true,
                    driver,
                    copier_score_bps: own_score.total_bps,
                    model_score_bps: model_score.total_bps,
                    positive_pre_copy_advantage: model_score.total_bps > own_score.total_bps,
                    adopter_share_gap_bps: 0,
                    group_imitation: false,
                    aligned_group_adoption_pre_core: false,
                },
            ));
        }

        for (slot, row) in adoptions {
            let id = self.colonists[slot].id;
            if !self.colonists[slot].adopts_commitment_norm {
                self.colonists[slot].adopts_commitment_norm = true;
                self.commitment_norm_imitation_adopters.insert(id);
                self.commitment_norm_copy_events.push(row);
            }
        }
    }
    pub(super) fn run_group_payoff_imitation(&mut self) {
        let Some(chain) = self.chain.as_ref() else {
            return;
        };
        let period = chain.imitation_period;
        let window = chain.imitation_window;
        let margin = chain.imitation_margin_bps;
        let radius = chain.imitation_radius;
        let max_models = chain.imitation_max_models;
        let food_target = chain.food_window_target;
        let no_imitation = chain.no_imitation;
        let random_imitation = chain.random_imitation;
        let salt_in_score = chain.salt_in_score;
        if no_imitation || period == 0 || window == 0 || self.econ_tick == 0 {
            return;
        }
        if !self.econ_tick.is_multiple_of(period) {
            return;
        }

        let live = self.live_colonist_slots.clone();
        let exchange_stockpile_pos = self
            .world
            .stockpile(self.exchange)
            .map(|stockpile| stockpile.pos);
        let exchange_pos = exchange_stockpile_pos.unwrap_or(Pos::new(0, 0));
        let committed_core_count = self.live_committed_count();
        let mut copies: Vec<(usize, bool, CommitmentNormCopyRow)> = Vec::new();

        for &slot in &live {
            let (copier_id, current_norm_bit) = {
                let colonist = &self.colonists[slot];
                if !colonist.alive {
                    continue;
                }
                (colonist.id, colonist.adopts_commitment_norm)
            };
            let Some(own_group) = self.commitment_norm_group_candidate(
                slot,
                &live,
                window,
                food_target,
                salt_in_score,
                radius,
                exchange_pos,
            ) else {
                continue;
            };
            let models = self.commitment_norm_observation_set(
                slot,
                &live,
                exchange_stockpile_pos,
                window,
                radius,
                max_models,
            );
            if models.is_empty() {
                continue;
            }
            let observed_groups: Vec<CommitmentNormGroupCandidate> = models
                .iter()
                .filter_map(|&model_slot| {
                    self.commitment_norm_group_candidate(
                        model_slot,
                        &live,
                        window,
                        food_target,
                        salt_in_score,
                        radius,
                        exchange_pos,
                    )
                })
                .collect();
            if observed_groups.is_empty() {
                continue;
            }

            let mut all_groups = Vec::with_capacity(observed_groups.len() + 1);
            all_groups.push(own_group.clone());
            all_groups.extend(observed_groups.iter().cloned());
            self.record_commitment_norm_group_covariance(&all_groups);

            let chosen = if random_imitation {
                let draw = deterministic_mix64(
                    COMMITMENT_NORM_RANDOM_SALT
                        ^ self.econ_tick.rotate_left(17)
                        ^ copier_id.0.rotate_left(7),
                );
                Some(observed_groups[(draw as usize) % observed_groups.len()].clone())
            } else {
                all_groups
                    .iter()
                    .max_by_key(|group| {
                        (group.score.total_bps, std::cmp::Reverse(group.center_id.0))
                    })
                    .filter(|group| {
                        group
                            .score
                            .total_bps
                            .saturating_sub(own_group.score.total_bps)
                            >= margin
                    })
                    .cloned()
            };
            let Some(best_group) = chosen else {
                continue;
            };
            let adopter_share_gap_bps =
                best_group.adopter_share_bps as i64 - own_group.adopter_share_bps as i64;
            let copied_norm_bit =
                if adopter_share_gap_bps >= COMMITMENT_NORM_ADOPTER_SHARE_GAP_BPS as i64 {
                    true
                } else if adopter_share_gap_bps <= -(COMMITMENT_NORM_ADOPTER_SHARE_GAP_BPS as i64) {
                    false
                } else {
                    continue;
                };
            let driver =
                commitment_norm_copy_driver(own_group.score, best_group.score, salt_in_score);
            copies.push((
                slot,
                copied_norm_bit,
                CommitmentNormCopyRow {
                    tick: self.econ_tick,
                    copier: copier_id.0,
                    model: best_group.center_id.0,
                    copied_norm_bit,
                    driver,
                    copier_score_bps: own_group.score.total_bps,
                    model_score_bps: best_group.score.total_bps,
                    positive_pre_copy_advantage: best_group.score.total_bps
                        > own_group.score.total_bps,
                    adopter_share_gap_bps,
                    group_imitation: true,
                    aligned_group_adoption_pre_core: !current_norm_bit
                        && copied_norm_bit
                        && best_group.score.total_bps > own_group.score.total_bps
                        && adopter_share_gap_bps > 0
                        && committed_core_count < ABANDONABLE_NORM_CORE_MARGIN,
                },
            ));
        }

        for (slot, copied_norm_bit, row) in copies {
            if self.stage_or_apply_commitment_norm_bit(slot, copied_norm_bit) {
                self.commitment_norm_copy_events.push(row);
            }
        }
    }
    pub(super) fn run_abandonable_norm_imitation(&mut self) {
        if self.group_payoff_imitation_active() {
            self.run_group_payoff_imitation();
            return;
        }
        let Some(chain) = self.chain.as_ref() else {
            return;
        };
        let period = chain.imitation_period;
        let window = chain.imitation_window;
        let margin = chain.imitation_margin_bps;
        let radius = chain.imitation_radius;
        let max_models = chain.imitation_max_models;
        let food_target = chain.food_window_target;
        let no_imitation = chain.no_imitation;
        let random_imitation = chain.random_imitation;
        let salt_in_score = chain.salt_in_score;
        if no_imitation || period == 0 || window == 0 || self.econ_tick == 0 {
            return;
        }
        if !self.econ_tick.is_multiple_of(period) {
            return;
        }

        let live = self.live_colonist_slots.clone();
        let exchange_pos = self
            .world
            .stockpile(self.exchange)
            .map(|stockpile| stockpile.pos);
        let mut copies: Vec<(usize, bool, CommitmentNormCopyRow)> = Vec::new();

        for &slot in &live {
            let colonist = &self.colonists[slot];
            if !colonist.alive {
                continue;
            }
            let models = self.commitment_norm_observation_set(
                slot,
                &live,
                exchange_pos,
                window,
                radius,
                max_models,
            );
            if models.is_empty() {
                continue;
            }

            let chosen = if random_imitation {
                // Gate the outcome-blind null on the SAME score-history warm-up as the scored path
                // (the copier's own score must be ready), so the matched anti-drift null copies on
                // the same cadence and stays comparable to the headline — the model is still drawn
                // uniformly, ignoring score and institution.
                let Some(own_score) =
                    self.commitment_norm_score(slot, window, food_target, salt_in_score)
                else {
                    continue;
                };
                let draw = deterministic_mix64(
                    COMMITMENT_NORM_RANDOM_SALT
                        ^ self.econ_tick.rotate_left(17)
                        ^ colonist.id.0.rotate_left(7),
                );
                let model_slot = models[(draw as usize) % models.len()];
                let model_score = self
                    .commitment_norm_score(model_slot, window, food_target, salt_in_score)
                    .unwrap_or_default();
                Some((model_slot, own_score, model_score))
            } else {
                let Some(own_score) =
                    self.commitment_norm_score(slot, window, food_target, salt_in_score)
                else {
                    continue;
                };
                models
                    .iter()
                    .filter_map(|&model_slot| {
                        self.commitment_norm_score(model_slot, window, food_target, salt_in_score)
                            .map(|score| (model_slot, score))
                    })
                    .max_by_key(|&(model_slot, score)| {
                        (
                            score.total_bps,
                            std::cmp::Reverse(self.colonists[model_slot].id.0),
                        )
                    })
                    .filter(|&(_, score)| {
                        score.total_bps.saturating_sub(own_score.total_bps) >= margin
                    })
                    .map(|(model_slot, model_score)| (model_slot, own_score, model_score))
            };
            let Some((model_slot, own_score, model_score)) = chosen else {
                continue;
            };
            let model = &self.colonists[model_slot];
            let copied_norm_bit = model.adopts_commitment_norm;
            let driver = commitment_norm_copy_driver(own_score, model_score, salt_in_score);
            copies.push((
                slot,
                copied_norm_bit,
                CommitmentNormCopyRow {
                    tick: self.econ_tick,
                    copier: colonist.id.0,
                    model: model.id.0,
                    copied_norm_bit,
                    driver,
                    copier_score_bps: own_score.total_bps,
                    model_score_bps: model_score.total_bps,
                    positive_pre_copy_advantage: model_score.total_bps > own_score.total_bps,
                    adopter_share_gap_bps: 0,
                    group_imitation: false,
                    aligned_group_adoption_pre_core: false,
                },
            ));
        }

        for (slot, copied_norm_bit, row) in copies {
            if self.stage_or_apply_commitment_norm_bit(slot, copied_norm_bit) {
                self.commitment_norm_copy_events.push(row);
            }
        }
    }
    /// S16: the produced-bread provenance market pass — runs right after `society.step()`,
    /// in the within-step order (the consume pass eats BEFORE the market clears). Sinks this
    /// tick's market-consume bread (produced-first), then for each bread trade transfers the
    /// drawn produced origin from seller to buyer and, for a bread→MEDIUM trade, attributes
    /// the produced vs minted split. Pre-promotion bread moves on the barter tape;
    /// post-promotion bread moves on the spot tape. Returns the consumption-log cursor so
    /// the later own-use consume pass is not re-counted. A no-op (returns 0) off the path.
    pub(super) fn run_bread_provenance_market(
        &mut self,
        barter_trades_start: usize,
        spot_trades_start: usize,
        was_pre_promotion: bool,
    ) -> usize {
        if !self.bread_provenance_active() {
            return 0;
        }
        let Some(bread) = self.provenance_bread_good() else {
            return 0;
        };
        let medium = self.barter_medium.map(|(good, _)| good);
        // S22c: clear this tick's cultivation-proceeds scratch up front so the post-promotion
        // bread→SALT spot sales below refill it; drained by `update_cultivation_returns`. Inert
        // (and the scratch stays empty) off the profit-driven-retention path.
        let retention = self.profit_driven_retention_active();
        if retention {
            self.cultivation_proceeds_scratch.clear();
        }
        // The whole consumption log so far is this tick's MARKET consume (cleared at the
        // step's start; the own-use consume has not run yet). Sink its bread, produced-first.
        let market_consume: Vec<(AgentId, u64)> = self
            .society
            .consumption_log_last_tick()
            .iter()
            .filter(|&&(_, good, _)| good == bread)
            .map(|&(agent, _, qty)| (agent, u64::from(qty)))
            .collect();
        let cursor = self.society.consumption_log_last_tick().len();
        for (agent, qty) in market_consume {
            self.bread_provenance.sink(agent, qty);
        }
        // This tick's bread trades (seller = the bread giver). Transfer the produced origin
        // to the buyer for EVERY bread trade (so a resold loaf keeps its origin), and
        // attribute the produced/minted split for a bread→MEDIUM trade.
        // The per-unit spot price (S22c) rides alongside as `Option<u64>`: `None` for a barter
        // trade (no gold price), `Some(price)` for a spot trade — so the post-promotion bread→SALT
        // proceeds can be credited to the producing seller below.
        let mut bread_trades: Vec<BreadTradeRow> = self.society.barter_trades
            [barter_trades_start..]
            .iter()
            .filter_map(|trade| {
                if trade.a_gives == bread {
                    Some(BreadTradeRow {
                        seller: trade.a,
                        buyer: trade.b,
                        other_good: Some(trade.b_gives),
                        qty: u64::from(trade.qty),
                        spot_price: None,
                    })
                } else if trade.b_gives == bread {
                    Some(BreadTradeRow {
                        seller: trade.b,
                        buyer: trade.a,
                        other_good: Some(trade.a_gives),
                        qty: u64::from(trade.qty),
                        spot_price: None,
                    })
                } else {
                    None
                }
            })
            .collect();
        let spot_medium = self.society.current_money_good();
        bread_trades.extend(
            self.society.trades[spot_trades_start..]
                .iter()
                .filter_map(|trade| {
                    (trade.good == bread).then_some(BreadTradeRow {
                        seller: trade.seller,
                        buyer: trade.buyer,
                        other_good: spot_medium,
                        qty: u64::from(trade.qty),
                        spot_price: Some(trade.price.0),
                    })
                }),
        );
        let tick = self.econ_tick;
        // S22c: credit the producing seller's realized proceeds only on POST-PROMOTION bread spot
        // sales for SALT (the spot tape with SALT as money), and only for the OWN-cultivated share
        // of the drawn lots (`lot.producer == seller`) — resold/minted bread is ignored.
        let credit_proceeds = retention && spot_medium == Some(SALT);
        for row in bread_trades {
            let drawn_lots = self
                .bread_provenance
                .transfer(row.seller, row.buyer, row.qty);
            if row.other_good == medium {
                self.bread_provenance.attribute_medium_sale(
                    &drawn_lots,
                    row.qty,
                    was_pre_promotion,
                    tick,
                );
            }
            let owner_own_qty: u64 = drawn_lots
                .iter()
                .filter(|lot| lot.producer == row.seller)
                .map(|lot| lot.qty)
                .sum();
            if owner_own_qty > 0 && self.current_or_ever_landowner(row.seller) {
                *self
                    .owner_surplus_sold_before_death
                    .entry(row.seller)
                    .or_insert(0) += owner_own_qty;
                let seller_age = self
                    .slot_for_id(row.seller)
                    .map_or(0, |slot| self.colonists[slot].age);
                let cohort = (seller_age / 10) * 10;
                *self
                    .buyer_purchases_by_owner_age_cohort
                    .entry(cohort)
                    .or_insert(0) += owner_own_qty;
                if !self.private_land_agent_holds_any_plot(row.buyer) {
                    self.owner_seller_attributed_bought = self
                        .owner_seller_attributed_bought
                        .saturating_add(owner_own_qty);
                }
            }
            if let Some(price) = row.spot_price {
                if credit_proceeds {
                    let own_qty: u64 = drawn_lots
                        .iter()
                        .filter(|lot| lot.producer == row.seller)
                        .map(|lot| lot.qty)
                        .sum();
                    if own_qty > 0 {
                        let proceeds = price.saturating_mul(own_qty);
                        *self
                            .cultivation_proceeds_scratch
                            .entry(row.seller)
                            .or_insert(0) += proceeds;
                    }
                }
                if self.wage_labor_active()
                    && spot_medium == Some(SALT)
                    && self.current_or_ever_landowner(row.seller)
                {
                    let own_qty: u64 = drawn_lots
                        .iter()
                        .filter(|lot| lot.producer == row.seller)
                        .map(|lot| lot.qty)
                        .sum();
                    if own_qty > 0 {
                        let proceeds = price.saturating_mul(own_qty);
                        self.credit_wage_retained_earnings_from_sale(row.seller, Gold(proceeds));
                    }
                }
            }
        }
        cursor
    }
    /// S16: sink this tick's OWN-USE cultivation bread consume (the log tail past the market
    /// pass's `cursor`), produced-first. Runs right after `run_own_use_cultivation`.
    pub(super) fn run_bread_provenance_own_use(&mut self, cursor: usize) {
        if !self.bread_provenance_active() {
            return;
        }
        let Some(bread) = self.provenance_bread_good() else {
            return;
        };
        let own_use: Vec<(AgentId, u64)> = self
            .society
            .consumption_log_last_tick()
            .iter()
            .skip(cursor)
            .filter(|&&(_, good, _)| good == bread)
            .map(|&(agent, _, qty)| (agent, u64::from(qty)))
            .collect();
        for (agent, qty) in own_use {
            self.bread_provenance.sink(agent, qty);
        }
    }
    /// S21d.1: the acquisition-ledger MARKET pass — runs right after `society.step()`, in the
    /// same within-step order as [`Self::run_bread_provenance_market`] (the consume pass eats
    /// BEFORE the market clears). First debit this tick's market-consume bread FIFO (booked by
    /// the channel it arrived through), then for each bread trade transfer the seller's units to
    /// the buyer as `Bought`. Pre-promotion bread moves on the barter tape; post-promotion on
    /// the spot tape. Returns the consumption-log cursor so the later own-use consume is not
    /// re-counted. A no-op (returns 0) off the path.
    pub(super) fn run_acquisition_market(
        &mut self,
        barter_trades_start: usize,
        spot_trades_start: usize,
    ) -> usize {
        if !self.acquisition_ledger_active() {
            return 0;
        }
        let Some(food) = self.acquisition_food_good() else {
            return 0;
        };
        // The whole consumption log so far is this tick's MARKET consume (cleared at the step's
        // start; the own-use consume has not run yet). Debit it FIFO, oldest channel first.
        let market_consume: Vec<(AgentId, u64)> = self
            .society
            .consumption_log_last_tick()
            .iter()
            .filter(|&&(_, good, _)| good == food)
            .map(|&(agent, _, qty)| (agent, u64::from(qty)))
            .collect();
        let cursor = self.society.consumption_log_last_tick().len();
        // S21d.2a: the producers whose buy → eat → bid bootstrap the microtrace follows.
        let producers = self.active_producer_ids();
        let tick = self.econ_tick;
        let mut producer_eaters: BTreeSet<AgentId> = BTreeSet::new();
        for (agent, qty) in market_consume {
            self.acquisition.consume(agent, qty);
            if producers.contains(&agent) {
                producer_eaters.insert(agent);
            }
        }
        for agent in producer_eaters {
            self.bootstrap_trace.observe_food_eat(agent, tick);
        }
        // This tick's bread trades (seller = the bread giver) — the buyer acquires `Bought`.
        // The 4th tuple element flags whether the bread was sold FOR SALT (the counterparty
        // good is SALT in a pre-promotion barter, or — post-promotion — a spot sale once SALT
        // is the money good): the S21h.0 hard invariant tallies the `SeededMinted` share of
        // exactly those sales.
        let money_is_salt = self.society.current_money_good() == Some(SALT);
        // DH.b (impl-69): under the closed marker every settled SPOT bread trade stamps its
        // fresh `Bought` lots with its globally-derivable trade id (the index into
        // `society.trades` — the same id the DH.a gold split records). A barter bread trade has
        // no spot identity (`None`) — unreachable on the closed base (no barter overlay), and if
        // one ever fired there the identity-less `Bought` lot would fail the live lot audit.
        let identity_on = self.closure_active();
        let mut bread_trades: Vec<(AgentId, AgentId, u64, bool, Option<u64>)> =
            self.society.barter_trades[barter_trades_start..]
                .iter()
                .filter_map(|trade| {
                    if trade.a_gives == food {
                        Some((
                            trade.a,
                            trade.b,
                            u64::from(trade.qty),
                            trade.b_gives == SALT,
                            None,
                        ))
                    } else if trade.b_gives == food {
                        Some((
                            trade.b,
                            trade.a,
                            u64::from(trade.qty),
                            trade.a_gives == SALT,
                            None,
                        ))
                    } else {
                        None
                    }
                })
                .collect();
        bread_trades.extend(
            self.society.trades[spot_trades_start..]
                .iter()
                .enumerate()
                .filter_map(|(offset, trade)| {
                    (trade.good == food).then_some((
                        trade.seller,
                        trade.buyer,
                        u64::from(trade.qty),
                        money_is_salt,
                        identity_on.then_some(spot_trades_start as u64 + offset as u64),
                    ))
                }),
        );
        let mut producer_buyers: BTreeSet<AgentId> = BTreeSet::new();
        for (seller, buyer, qty, received_salt, identity) in bread_trades {
            let (drawn, fresh_bought) = self
                .acquisition
                .transfer_as_bought_identified(seller, buyer, qty, identity);
            if received_salt {
                self.seeded_minted_bread_sold_for_salt = self
                    .seeded_minted_bread_sold_for_salt
                    .saturating_add(drawn[FoodChannel::SeededMinted.index()]);
            }
            // DH.b (R4-1): capture the purchase-credit-seam fact for the same-tick validation
            // against the settled-trade record (buyer / good / aggregate quantity).
            if let Some(trade_id) = identity {
                self.burden
                    .pending_purchase_credits
                    .push(burden::PendingPurchaseCredit {
                        trade_id,
                        buyer,
                        good: food,
                        credited: fresh_bought,
                    });
            }
            if producers.contains(&buyer) {
                producer_buyers.insert(buyer);
            }
        }
        for buyer in producer_buyers {
            self.bootstrap_trace.observe_food_buy(buyer, tick);
        }
        cursor
    }
    /// S21d.1: debit the cultivators' OWN-USE bread consume (the consumed-log tail past the
    /// market pass's `cursor`) FIFO — the own-use eating sink. Runs right after the own-use
    /// cultivation phase. A no-op off the path (and inert here — cultivation is off in the probe).
    pub(super) fn run_acquisition_own_use(&mut self, cursor: usize) {
        if !self.acquisition_ledger_active() {
            return;
        }
        let Some(food) = self.acquisition_food_good() else {
            return;
        };
        let own_use: Vec<(AgentId, u64)> = self
            .society
            .consumption_log_last_tick()
            .iter()
            .skip(cursor)
            .filter(|&&(_, good, _)| good == food)
            .map(|&(agent, _, qty)| (agent, u64::from(qty)))
            .collect();
        for (agent, qty) in own_use {
            self.acquisition.consume(agent, qty);
        }
    }
    /// S18: the multi-good money instrumentation pass — runs right after the bread
    /// provenance market pass, reading THIS tick's barter trades (the suffix past
    /// `barter_trades_start`). Two concerns: (1) the **pending-indirect-SALT round-trip
    /// ledger** (Codex P1c) — traced for ANY emergent barter medium, so it works on S9 too,
    /// where SALT actually round-trips; (2) the **WOOD↔medium leg** (the WOOD provenance
    /// bound), accumulated only on the multi-good path. Runtime-only (not digested); a no-op
    /// for a settlement with no barter medium.
    pub(super) fn run_multigood_instrumentation(
        &mut self,
        barter_trades_start: usize,
        spot_trades_start: usize,
        was_pre_promotion: bool,
    ) {
        let Some((medium, _)) = self.barter_medium else {
            return;
        };
        let multigood = self.multigood_money_active();
        for index in barter_trades_start..self.society.barter_trades.len() {
            let trade = &self.society.barter_trades[index];
            let (a, b, a_gives, b_gives, a_reason, b_reason, qty) = (
                trade.a,
                trade.b,
                trade.a_gives,
                trade.b_gives,
                trade.a_reason,
                trade.b_reason,
                u64::from(trade.qty),
            );
            // The round-trip ledger: trace each side's use of the medium (accept-as-means vs
            // spend-on-target). Tracks SALT actually intermediating, not just net stock.
            observe_round_trip_side(
                &mut self.multigood,
                a,
                a_gives,
                b_gives,
                a_reason,
                medium,
                qty,
            );
            observe_round_trip_side(
                &mut self.multigood,
                b,
                b_gives,
                a_gives,
                b_reason,
                medium,
                qty,
            );
            // The WOOD leg (multi-good only): a WOOD↔medium swap. With every WOOD buffer + the
            // mint zeroed, the WOOD here was gathered (bounded by `wood_gathered`) — the
            // provenance proof for the traded WOOD.
            if multigood
                && ((a_gives == WOOD && b_gives == medium)
                    || (a_gives == medium && b_gives == WOOD))
            {
                self.multigood.wood_for_salt = self.multigood.wood_for_salt.saturating_add(qty);
                if was_pre_promotion {
                    self.multigood.pre_promotion_wood_for_salt = self
                        .multigood
                        .pre_promotion_wood_for_salt
                        .saturating_add(qty);
                }
            }
        }
        // Post-promotion the barter medium IS the money good (the promotion promotes SALT
        // itself) and the market clears on the spot tape. Two reads off each spot trade:
        // (1) round-trip — a buyer acquiring its earmarked target with that money completes
        //     the means role as money, so decrement pending on each spot purchase of a target
        //     (a no-op for a buyer with nothing earmarked), else a real monetization would
        //     read as hoarding;
        // (2) the WOOD leg — a WOOD-for-money spot sale (the money good being the barter
        //     medium) is the post-promotion continuation of the WOOD↔medium leg, so fold it
        //     into the cumulative total, exactly as the bread ledger folds post-promotion
        //     spot bread→medium sales. The pre-promotion share is frozen, so this only grows
        //     the cumulative figure (spot trades clear only post-promotion).
        let spot_medium = self.society.current_money_good();
        for index in spot_trades_start..self.society.trades.len() {
            let (buyer, good, qty) = {
                let trade = &self.society.trades[index];
                (trade.buyer, trade.good, u64::from(trade.qty))
            };
            self.multigood.spend_on_target(buyer, good, qty);
            if multigood && good == WOOD && spot_medium == Some(medium) {
                self.multigood.wood_for_salt = self.multigood.wood_for_salt.saturating_add(qty);
                if was_pre_promotion {
                    self.multigood.pre_promotion_wood_for_salt = self
                        .multigood
                        .pre_promotion_wood_for_salt
                        .saturating_add(qty);
                }
            }
        }
    }
}
