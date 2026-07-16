//! Demography and lifecycle machinery.
//!
//! Death settlement and its bookkeeping (estate collection, heir selection,
//! estate-to-heirs/commons routing, landowner-lineage telemetry), land transfer on
//! death, aging/elderly removal, demography provisioning, birth caps, and
//! birth-stock transfer/injection. Extracted verbatim from `mod.rs` (pure code
//! motion) into this sibling `impl Settlement` block; the module-private methods
//! become `pub(super)` — the exact scope they already had inside `settlement`.
//! Generic cross-module helpers (`colonist_household`, `stock_of_id`) stay in
//! `mod.rs`.

use super::*;

impl Settlement {
    /// NEEDS phase: advance living colonists' needs from the last econ tick's
    /// realized consumption + labor, then apply starvation deaths as **real
    /// removal** (G4a) — settling each dead colonist's estate to the commons,
    /// freeing its arena slot, and removing it from the world. Returns the number of
    /// deaths. Deterministic: deaths are collected in generation order and settled
    /// in that order; nothing is drawn.
    pub(super) fn update_needs_and_remove_dead(
        &mut self,
        report: &mut EconTickReport,
        wage_labor_used: &mut Vec<(AgentId, u32)>,
    ) -> u32 {
        let live_slots = self.live_colonist_slots.clone();
        let mut intakes = vec![NeedIntake::default(); live_slots.len()];
        for &(agent, good, qty) in self.society.consumption_log_last_tick() {
            let Some(index) = self.slot_for_id(agent) else {
                continue;
            };
            let Ok(intake_index) = live_slots.binary_search(&index) else {
                continue;
            };
            if good == self.known.hunger || Some(good) == self.known.subsistence {
                // The preferred staple OR the directly-edible subsistence food
                // (e.g. raw grain) both reduce hunger. This is final
                // consumption (want satisfaction), not chain-input use, so
                // grain milled into flour is not counted here.
                intakes[intake_index].food_consumed =
                    intakes[intake_index].food_consumed.saturating_add(qty);
            } else if good == self.known.warmth {
                intakes[intake_index].wood_consumed =
                    intakes[intake_index].wood_consumed.saturating_add(qty);
            }
        }
        for &(agent, labor) in self.society.labor_used_last_tick() {
            let Some(index) = self.slot_for_id(agent) else {
                continue;
            };
            let Ok(intake_index) = live_slots.binary_search(&index) else {
                continue;
            };
            intakes[intake_index].labor_used =
                intakes[intake_index].labor_used.saturating_add(labor);
        }

        for (intake_index, &slot) in live_slots.iter().enumerate() {
            self.colonists[slot]
                .need
                .advance(&self.dynamics, intakes[intake_index]);
        }

        // Collect deaths first (immutable read of `dynamics`), then apply.
        let mut dying = Vec::new();
        for &slot in &live_slots {
            let colonist = &mut self.colonists[slot];
            if colonist.need.is_critical(&self.dynamics) {
                colonist.critical_streak = colonist.critical_streak.saturating_add(1);
            } else {
                colonist.critical_streak = 0;
            }
            if colonist.critical_streak >= self.dynamics.death_window {
                dying.push(colonist.id);
            }
        }
        // Settle each dying colonist's bank deposit before removal: redeem its demand
        // claims for specie (the deposit's mirror image) so it holds only specie and
        // settles through the unchanged G8a specie estate. A no-op without a bank, so
        // every pre-G8b death path is byte-identical. The underlying economy is viable
        // only over a bounded horizon — its consumers eventually starve once their
        // finite WOOD income runs out (with or without a bank) — so a depositing
        // colonist can reach the death window still holding claims; this withdraws them
        // with no econ change and no claim-estate routing (G8c). See
        // [`Self::liquidate_bank_deposit_on_death`].
        for &id in &dying {
            self.liquidate_bank_deposit_on_death(id);
        }
        // Every colonist that reached the starvation death window must now be settle-able.
        // A balance still holding demand claims or fiat has no conserved estate route yet
        // (claim/fiat estates land with the G8c tax/regime work); the deposit-withdrawal
        // above clears the only claim a shipped config produces, so this stays a fail-loud
        // backstop for any future claim/fiat holder the withdrawal cannot cover (e.g. a
        // claim beyond the bank's reserves), rather than silently dropping it from the
        // dying list and leaving an alive-but-permanently-critical colonist that never
        // settles. It is an assertion pass, not a filter — the `dying` set is unchanged
        // when every member is settle-able (every shipped case).
        for &id in &dying {
            assert!(
                self.society.can_remove_agent(id),
                "colonist {id:?} reached the starvation death window but cannot be \
                 settled (still holds demand claims or fiat the deposit-withdrawal \
                 could not cover, with no estate route until G8c); the dying -> \
                 settle path must stay complete for every shipped config"
            );
        }
        for &id in &dying {
            if let Some(slot) = self.slot_for_id(id) {
                self.mark_colonist_dead(slot);
            }
        }
        let mut deaths = 0;
        for id in dying {
            if self
                .colonist_household(id)
                .is_some_and(|household| self.is_producer_household(household))
            {
                self.earned_provisioning.stats.member_starvations = self
                    .earned_provisioning
                    .stats
                    .member_starvations
                    .saturating_add(1);
            }
            deaths += u32::from(self.settle_death(id, report, wage_labor_used));
        }
        // S17 — attribute the positive check: accumulate the starvation death count into
        // the runtime-only counter (mirrors `old_age_deaths_total` in `age_and_remove_elderly`,
        // but is NOT digested). A no-death tick adds zero, so a pre-S17 run reads the same.
        self.starvation_deaths_total = self
            .starvation_deaths_total
            .saturating_add(u64::from(deaths));
        deaths
    }
    /// Route a dead colonist's estate (G4a removal + G4b inheritance). A demography
    /// settlement routes to the household **heirs** (the commons only if the lineage
    /// is extinct); every pre-G4b settlement routes to the commons exactly as G4a.
    /// The dispatch keeps the no-demography path structurally unchanged, so the G4a
    /// suite and the conformance goldens are byte-identical.
    pub(super) fn settle_death(
        &mut self,
        id: AgentId,
        report: &mut EconTickReport,
        wage_labor_used: &mut Vec<(AgentId, u32)>,
    ) -> bool {
        self.record_owner_death_telemetry(id);
        self.settle_in_kind_wage_for_starvation_death(id);
        let pending_share_successions = self.settle_share_tenancy_for_death(id);
        self.transfer_private_land_on_death(id);
        self.finalize_share_contract_successions(pending_share_successions);
        if self.demography.is_some() {
            self.settle_estate_to_heirs(id, report, wage_labor_used)
        } else {
            self.settle_estate_to_commons(id, report, wage_labor_used)
        }
    }
    /// Remove `id` from the running settlement and collect its full estate — econ
    /// gold + stock (via [`Society::remove_agent`]), world-carried delivery escrow,
    /// and any stranded exchange-deposit escrow — returning the gold and a per-good
    /// map, and removing its world agent. The estate is collected but NOT yet routed;
    /// the caller settles it to the commons (G4a) or the household heirs (G4b). The
    /// order is the spec's (settle → cancel → free → reconcile, inside
    /// `remove_agent`; then drain world/exchange escrow), so wherever the estate goes
    /// the whole-system total is conserved. Deterministic: id-ordered, no RNG.
    pub(super) fn collect_estate(
        &mut self,
        id: AgentId,
        report: &mut EconTickReport,
        wage_labor_used: &mut Vec<(AgentId, u32)>,
    ) -> Option<(Gold, BTreeMap<GoodId, u64>)> {
        self.settle_wage_labor_for_death(id, report, wage_labor_used);
        let estate = self.society.remove_agent(id)?;
        let gold = estate.gold;
        let mut stock: BTreeMap<GoodId, u64> = BTreeMap::new();
        // Econ estate: the dead colonist's gold plus every physical good it held
        // (its stock is a subset of `self.goods`; GOLD is money, not stock).
        for &good in &self.goods {
            let qty = estate.stock.get(good);
            if qty > 0 {
                *stock.entry(good).or_insert(0) += u64::from(qty);
            }
        }
        // World-carried escrow: drain it out of the world (rather than freezing it in
        // place as the G1 tombstone did). A non-spatial householder (G4b) carries
        // nothing, so this is a no-op for it.
        for &good in &self.goods {
            let carried = self.world.agent_carry(id, good);
            if carried > 0 {
                let drained = self.world.withdraw_agent_carry(id, good, carried);
                *stock.entry(good).or_insert(0) += u64::from(drained);
            }
        }
        // Pending exchange-deposit escrow: units this colonist delivered to the
        // exchange stockpile but never had credited (its attribution still sitting in
        // `pending_deposits`) are part of its estate. Drain them out of the world's
        // exchange and drop the attribution — a conserved transfer that leaves no
        // entry keyed by the freed id for `transfer_pending_deposits` to retry against
        // forever. The withdraw mirrors the removed attribution unit-for-unit,
        // preserving the pending↔exchange invariant. Empty in the starvation/old-age
        // death models (the transfer credits a still-live depositor before it can
        // die; a householder never deposits), so this is a defensive settle.
        let stranded: Vec<(AgentId, GoodId)> = self
            .pending_deposits
            .keys()
            .copied()
            .filter(|(agent, _)| *agent == id)
            .collect();
        for key in stranded {
            let qty = self.pending_deposits.remove(&key).unwrap_or(0);
            if qty == 0 {
                continue;
            }
            let (_, good) = key;
            let drained = self.world.stockpile_withdraw(self.exchange, good, qty);
            debug_assert_eq!(
                drained, qty,
                "the exchange must hold every pending unit attributed to a dead depositor"
            );
            if drained > 0 {
                *stock.entry(good).or_insert(0) += u64::from(drained);
            }
        }
        // Remove its spatial body after draining carry so future world ticks do not
        // scan historical deaths. Non-spatial G4b householders were never in the
        // world, so this is a no-op for them.
        if let Some(remaining_carry) = self.world.remove_agent(id) {
            // The loop above drains every good in `self.goods`; this sweeps any residual
            // into the estate rather than dropping it in release builds (the assert pins
            // the invariant in debug). Conservation is enforced, never assumed.
            debug_assert!(
                remaining_carry.values().all(|&qty| qty == 0),
                "estate collection must drain carry before removing a world agent"
            );
            for (good, qty) in remaining_carry {
                if qty > 0 {
                    *stock.entry(good).or_insert(0) += u64::from(qty);
                }
            }
        }
        Some((gold, stock))
    }
    /// Settle a dead colonist's estate to the **commons** (G4a). A conserved transfer
    /// end to end: the gold and goods leave the society and the world for the commons,
    /// nothing created or destroyed. Deterministic: id-ordered, no RNG.
    pub(super) fn settle_estate_to_commons(
        &mut self,
        id: AgentId,
        report: &mut EconTickReport,
        wage_labor_used: &mut Vec<(AgentId, u32)>,
    ) -> bool {
        if !self.society.can_remove_agent(id) {
            return false;
        }
        if let Some(slot) = self.slot_for_id(id) {
            self.mark_colonist_dead(slot);
        }
        let Some((gold, stock)) = self.collect_estate(id, report, wage_labor_used) else {
            return false;
        };
        self.commons_gold = self.commons_gold.saturating_add(gold);
        self.earned_provisioning.buckets.remove(&id);
        let mut closure_goods = BTreeMap::new();
        for (good, qty) in stock {
            if qty > 0 {
                *self.commons_stock.entry(good).or_insert(0) += qty;
                closure_goods.insert(good, (0, qty));
            }
        }
        // S16: the dead colonist's produced bread leaves the living population for the
        // commons — a conserved sink for the provenance ledger.
        if self.bread_provenance_active() {
            self.bread_provenance.drop_to_sink(id);
        }
        // S21d.1: the dead colonist's tracked food leaves the living population for the
        // commons — a conserved sink for the acquisition ledger too.
        if self.acquisition_ledger_active() {
            self.acquisition.drop_to_sink(id);
        }
        self.record_estate_destination(id, EstateDestination::Commons, None, closure_goods);
        true
    }
    /// Settle a dead colonist's estate to the household **heirs** (G4b inheritance):
    /// credit the whole estate to a living member of the same household (the first
    /// surviving heir in colonist-insertion order), falling back to the **commons** if the lineage is extinct (no
    /// living member remains). Crediting a live heir is a conserved transfer *within*
    /// the society (the dead's holdings move to a survivor), and the commons fallback
    /// is the same conserved transfer G4a used — so whole-system conservation holds
    /// either way. Any unplaceable remainder (an heir at the `u32`/`u64` ceiling — never
    /// reached with these small quantities) routes to the commons rather than vanish.
    pub(super) fn settle_estate_to_heirs(
        &mut self,
        id: AgentId,
        report: &mut EconTickReport,
        wage_labor_used: &mut Vec<(AgentId, u32)>,
    ) -> bool {
        if !self.society.can_remove_agent(id) {
            return false;
        }
        let producer_subject = self.mortal_producer_inheritance_active()
            && self
                .slot_for_id(id)
                .is_some_and(|slot| self.mortal_chain_producer_subject(slot));
        self.record_producer_house_death(id);
        let producer_tools = self
            .chain
            .as_ref()
            .map(|chain| [chain.content.mill(), chain.content.oven()]);
        if let Some(slot) = self.slot_for_id(id) {
            self.mark_colonist_dead(slot);
        }
        let Some((gold, mut stock)) = self.collect_estate(id, report, wage_labor_used) else {
            return false;
        };
        let destination = self.heir_for(id).map(|heir| EstateDestination::Household {
            household: self.colonist_household(id).unwrap_or_default(),
            heir,
        });
        let producer_tool_units = producer_tools.map_or(0, |tools| {
            tools
                .iter()
                .map(|tool| stock.get(tool).copied().unwrap_or(0))
                .sum()
        });
        if producer_subject && destination.is_none() && producer_tool_units > 0 {
            self.heirless_producer_deaths = self.heirless_producer_deaths.saturating_add(1);
        }
        let forced_commons_producer_tools =
            if producer_subject && !self.mortal_producer_tool_inheritance_active() {
                producer_tools.map(|tools| {
                    tools.map(|tool| {
                        let qty = stock.remove(&tool).unwrap_or(0);
                        (tool, qty)
                    })
                })
            } else {
                None
            };
        // S22e: the plow estate-routing SWITCH (the genuinely new primitive). When the
        // endowed-cultivation-capital gate is active AND inheritance is OFF, FORCE any plows in the
        // estate to the commons even when the rest of the estate goes to the heir. Implemented as a
        // single stock-map partition BEFORE placement: the plow units are removed from `stock` here
        // (the {plows} partition) and placed to the commons once after the heir/commons routing
        // below, while {everything else} stays in `stock` and follows the existing routing — each
        // partition placed exactly once, a pure conserved transfer, never a mint. The plow good is
        // never the tracked bread, so the provenance / acquisition heir/commons split is unaffected.
        // When inheritance is ON (the default under the gate) or the gate is inactive, `stock` is
        // untouched and plows follow the existing heir path — goldens byte-identical off the gate.
        let plow_good = self.cultivation_tool_good();
        let forced_commons_plows: u64 = if self.endowed_cultivation_capital_active()
            && !self.cultivation_tool_inheritance_active()
        {
            plow_good.map_or(0, |plow| stock.remove(&plow).unwrap_or(0))
        } else {
            0
        };
        // The tracked bread good for BOTH stock-origin ledgers (S16 provenance + S21d.1
        // acquisition) — they classify the same good, so the heir/commons split is computed
        // once whenever either ledger is active.
        let tracked_bread = if self.bread_provenance_active() || self.acquisition_ledger_active() {
            self.provenance_bread_good()
        } else {
            None
        };
        let mut bread_placed_with_heir = 0u64;
        let mut bread_placed_with_commons = 0u64;
        let mut closure_goods = BTreeMap::new();
        let gold_heir = match destination {
            Some(EstateDestination::Household { heir, .. }) => {
                let gold_heir = if self.credit_estate_gold_to_heir(heir, gold) {
                    let (lots, untracked) = self.debit_earned_provisioning_lots(id, gold);
                    self.credit_earned_provisioning_lots(heir, lots);
                    self.credit_earned_provisioning_lot(
                        heir,
                        EarnedGoldLot {
                            source: EarnedGoldSource::Endowed,
                            amount: untracked,
                        },
                    );
                    Some(heir)
                } else {
                    // Defensive: an overflow at the heir, stale heir id, or future
                    // ledger-money estate routes the gold to the commons.
                    self.commons_gold = self.commons_gold.saturating_add(gold);
                    self.earned_provisioning.buckets.remove(&id);
                    None
                };
                for (good, qty) in stock {
                    if qty == 0 {
                        continue;
                    }
                    // Clamp the credit to the heir's remaining headroom so the
                    // saturating `Stock::add` can never silently drop goods: any amount
                    // the heir cannot hold (its stock would pass `u32::MAX`) routes to
                    // the commons instead of vanishing — the same clamp the provision
                    // path uses. Unreached with these small quantities, but conservation
                    // is load-bearing, so it is enforced here, never assumed.
                    let held = self
                        .society
                        .agents
                        .get(heir)
                        .map_or(0, |agent| agent.stock.get(good));
                    let headroom = u64::from(u32::MAX - held);
                    let credited = u32::try_from(qty.min(headroom)).unwrap_or(0);
                    let placed = if credited > 0 && self.society.credit_stock(heir, good, credited)
                    {
                        u64::from(credited)
                    } else {
                        0
                    };
                    if qty > placed {
                        *self.commons_stock.entry(good).or_insert(0) += qty - placed;
                    }
                    closure_goods.insert(good, (placed, qty - placed));
                    if Some(good) == tracked_bread {
                        bread_placed_with_heir += placed;
                        bread_placed_with_commons += qty - placed;
                    }
                    if producer_subject
                        && placed > 0
                        && producer_tools.is_some_and(|tools| tools.contains(&good))
                    {
                        self.producer_tool_inheritances =
                            self.producer_tool_inheritances.saturating_add(placed);
                        self.producer_tool_inheritors.insert((heir, good));
                        // DH.b (impl-69): the inheritance-identity succession event at the real
                        // estate seam — a producer subject's tool actually PLACED with a living
                        // heir. The class is the heir's fixed registry class (the deceased's is
                        // the same household class).
                        if self.closure_active() {
                            if let Some(class) = self.closure_class_of(heir) {
                                debug_assert_eq!(
                                    self.closure_class_of(id),
                                    Some(class),
                                    "heir and deceased share the fixed household class"
                                );
                                self.burden.inheritances.push(burden::BurdenToolInherited {
                                    tick: self.econ_tick,
                                    class,
                                    deceased: id,
                                    heir,
                                    tool: good,
                                });
                            }
                        }
                    }
                    // S22e (runtime diagnostic): a plow that lands with a LIVING heir is a real
                    // inheritance transfer (conserved, never a mint — the tool-stock total is
                    // unchanged). Record the count + the heir id so the non-vacuity test can confirm
                    // a post-founder-death plow transfer occurred. Not digested.
                    if Some(good) == plow_good && placed > 0 {
                        self.cultivation_tool_inherited_total =
                            self.cultivation_tool_inherited_total.saturating_add(placed);
                        self.cultivation_tool_inheritor_ids.insert(heir);
                    }
                }
                gold_heir
            }
            Some(EstateDestination::Commons) | None => {
                self.commons_gold = self.commons_gold.saturating_add(gold);
                self.earned_provisioning.buckets.remove(&id);
                for (good, qty) in stock {
                    if qty > 0 {
                        *self.commons_stock.entry(good).or_insert(0) += qty;
                        closure_goods.insert(good, (0, qty));
                    }
                    if Some(good) == tracked_bread {
                        bread_placed_with_commons += qty;
                    }
                }
                None
            }
        };
        self.earned_provisioning.buckets.remove(&id);
        // S22e: place the {plows} partition the inheritance switch forced to the commons (a
        // conserved transfer — the same sink the heirless commons fallback uses). Empty unless the
        // gate is active with inheritance OFF, so this is inert (and goldens byte-identical) on
        // every other path. Placed exactly once, after the {everything else} partition above.
        if forced_commons_plows > 0 {
            if let Some(plow) = plow_good {
                *self.commons_stock.entry(plow).or_insert(0) += forced_commons_plows;
                closure_goods.insert(plow, (0, forced_commons_plows));
            }
        }
        if let Some(forced_tools) = forced_commons_producer_tools {
            for (tool, qty) in forced_tools {
                if qty > 0 {
                    *self.commons_stock.entry(tool).or_insert(0) += qty;
                    closure_goods.insert(tool, (0, qty));
                }
            }
        }
        if self.current_or_ever_landowner(id) {
            self.inherited_stock_to_heirs = self
                .inherited_stock_to_heirs
                .saturating_add(bread_placed_with_heir);
        }
        // S16: route the dead colonist's produced bread with the bread units the estate
        // actually placed. Heir headroom can split physical bread between heir and commons,
        // so the produced-origin counter follows that same split.
        if self.bread_provenance_active() {
            match destination {
                Some(EstateDestination::Household { heir, .. }) => {
                    self.bread_provenance
                        .transfer(id, heir, bread_placed_with_heir);
                    self.bread_provenance.sink(id, bread_placed_with_commons);
                    let residual = self.bread_provenance.drop_to_sink(id);
                    debug_assert_eq!(
                        residual, 0,
                        "estate provenance routing must account for every produced bread unit"
                    );
                }
                _ => {
                    self.bread_provenance.drop_to_sink(id);
                }
            }
        }
        // S21d.1: route the dead colonist's tracked food the same way — an heir TRANSFER
        // (origin preserved), a commons SINK, and any residual dropped to the sink — so the
        // acquisition ledger conserves across the estate. The heir/commons split drains the
        // agent's whole tracked-food balance, so the residual drop must be zero (the same
        // localizing invariant `BreadProvenance` asserts on its parallel routing above).
        if self.acquisition_ledger_active() {
            match destination {
                Some(EstateDestination::Household { heir, .. }) => {
                    self.acquisition
                        .transfer_preserve(id, heir, bread_placed_with_heir);
                    self.acquisition.sink(id, bread_placed_with_commons);
                    let residual = self.acquisition.drop_to_sink(id);
                    debug_assert_eq!(
                        residual, 0,
                        "estate acquisition routing must account for every tracked-food unit"
                    );
                }
                _ => {
                    self.acquisition.drop_to_sink(id);
                }
            }
        }
        self.record_estate_destination(
            id,
            destination.unwrap_or(EstateDestination::Commons),
            gold_heir,
            closure_goods,
        );
        true
    }
    /// Credit already-collected estate gold to a live heir, on either money regime.
    /// [`Society::remove_agent`] has already removed the dead colonist's money from
    /// this same society — its `Agent.gold` in closed-GOLD M1, or its public specie
    /// drained out of the ledger in M3 (G8a) — so restoring it to a live household
    /// heir is a conserved in-settlement estate move. [`Society::credit_estate_gold`]
    /// handles every regime: it adds to `Agent.gold` in closed-GOLD M1 and in
    /// post-promotion emergent money, and re-credits ledger specie (returning
    /// `commodity_base` to its pre-death total) in M3. Returns `false` only on an
    /// overflow or stale heir, in which case the gold routes to the commons instead.
    pub(super) fn credit_estate_gold_to_heir(&mut self, heir: AgentId, gold: Gold) -> bool {
        self.society.credit_estate_gold(heir, gold)
    }
    pub(super) fn record_estate_destination(
        &mut self,
        id: AgentId,
        destination: EstateDestination,
        gold_heir: Option<AgentId>,
        goods: BTreeMap<GoodId, (u64, u64)>,
    ) {
        // DH.a: stash the actual heir/commons placements for post-death closure observation.
        if self.closure_active() {
            let heir = match destination {
                EstateDestination::Household { heir, .. } => Some(heir),
                EstateDestination::Commons => None,
            };
            self.closure.pending_estate.push((
                id,
                closure::ClosureEstateRouting {
                    gold_heir,
                    heir,
                    goods,
                },
            ));
        }
        if let Some(slot) = self.slot_for_id(id) {
            self.colonists[slot].estate_destination = Some(destination);
        }
    }
    /// The heir for a dead colonist's estate (G4b): the first **living** member of
    /// the dead colonist's household, in colonist-insertion order, that still resolves as a live econ agent, or
    /// `None` if the lineage is extinct (or the colonist has no household — a pre-G4b
    /// colonist, which therefore settles to the commons). The dead colonist is already
    /// marked `alive = false` before settlement, so it is never its own heir.
    pub(super) fn heir_for(&self, dead_id: AgentId) -> Option<AgentId> {
        let household = self
            .slot_for_id(dead_id)
            .and_then(|s| self.colonists[s].household)?;
        // Scan only the compact live roster: the dead colonist is marked dead — and so
        // already off `live_colonist_slots` — before settlement, so it is never its own
        // heir, and co-dying members (marked before any are settled) are excluded too.
        // `live_colonist_slots` is kept in slot order, so this yields the first
        // surviving household member in colonist-insertion order, the same colonist the
        // historical scan picked, without walking the full historical roster.
        self.live_colonist_slots
            .iter()
            .map(|&slot| &self.colonists[slot])
            .filter(|c| c.household == Some(household))
            .map(|c| c.id)
            .find(|&heir| self.society.agents.get(heir).is_some())
    }
    pub(super) fn record_landowner_acquired(&mut self, owner: AgentId) {
        self.ever_landowner_ids.insert(owner);
        if self.owner_first_claim_tick.contains_key(&owner) {
            return;
        }
        self.owner_first_claim_tick.insert(owner, self.econ_tick);
        if let Some(slot) = self.slot_for_id(owner) {
            self.owner_age_at_first_claim
                .insert(owner, self.colonists[slot].age);
        }
    }
    pub(super) fn current_or_ever_landowner(&self, owner: AgentId) -> bool {
        self.ever_landowner_ids.contains(&owner) || self.private_land_agent_holds_any_plot(owner)
    }
    pub(super) fn record_owner_death_telemetry(&mut self, owner: AgentId) {
        if !self.current_or_ever_landowner(owner) {
            return;
        }
        if let Some(first_tick) = self.owner_first_claim_tick.get(&owner).copied() {
            self.owner_tenure_before_death
                .push((owner, self.econ_tick.saturating_sub(first_tick)));
        }
        if let Some(bread) = self.provenance_bread_good() {
            self.owner_inventory_at_death
                .push((owner, self.stock_of_id(owner, bread)));
        }
    }
    pub(super) fn transfer_secure_private_land_on_death(&mut self, dead: AgentId) {
        let Some(regime) = self.chain.as_ref().map(|chain| chain.inheritance_regime) else {
            return;
        };
        let mut dead_owners: BTreeSet<AgentId> = self
            .land_plots
            .values()
            .flat_map(Self::private_land_record_holders)
            .filter(|&owner| !self.private_land_live_agent(owner))
            .collect();
        dead_owners.insert(dead);
        if dead_owners.is_empty() {
            return;
        }

        match regime {
            InheritanceRegime::Impartible => {
                let transfers: Vec<ImpartibleLandTransfer> = self
                    .land_plots
                    .iter()
                    .filter_map(|(&node, record)| {
                        let owner = record.owner?;
                        dead_owners.contains(&owner).then(|| {
                            let capacity = self
                                .private_land_share_capacity(record, node, owner)
                                .unwrap_or((0, 0));
                            (
                                node,
                                owner,
                                self.secure_land_universal_heir_for(owner),
                                capacity,
                            )
                        })
                    })
                    .collect();
                let mut lost = Vec::new();
                for (node, owner, heir, capacity) in transfers {
                    if let Some(record) = self.land_plots.get_mut(&node) {
                        if record.owner != Some(owner) {
                            continue;
                        }
                        record.owner = heir;
                        record.idle_counter = 0;
                        record.reserved_for = None;
                    }
                    if let Some(heir) = heir {
                        self.record_landowner_acquired(heir);
                    }
                    if heir.is_none() {
                        lost.push((node, owner));
                    }
                    self.secure_land_log_inheritance(
                        owner,
                        heir,
                        node,
                        regime,
                        capacity,
                        heir.map_or((0, 0), |_| capacity),
                    );
                    if self.land_market_active() {
                        let owner_history =
                            self.land_market_title_history.entry(owner).or_default();
                        owner_history.ever_owned = true;
                        owner_history.current = None;
                        if let Some(heir) = heir {
                            let heir_history =
                                self.land_market_title_history.entry(heir).or_default();
                            heir_history.ever_owned = true;
                            heir_history.current = Some(LandTitleSource::Inherited);
                        }
                    }
                }
                for (node, owner) in lost {
                    self.land_lost_prior_owners
                        .insert(node, (owner, LandLossCause::Death));
                    if let Some(quality) = self.private_land_plot_quality(node) {
                        self.land_lapsed_losses.insert(owner, quality);
                    }
                    if self.land_market_active() {
                        if let Some(state) = self.land_market_plots.get_mut(&node) {
                            state.listing = None;
                        }
                    }
                }
            }
            InheritanceRegime::Partible => {
                let mut transfers: Vec<PartibleLandTransfer> = Vec::new();
                for (&node, record) in &self.land_plots {
                    if let Some(owner) = record.owner {
                        if dead_owners.contains(&owner) {
                            let capacity = self
                                .private_land_share_capacity(record, node, owner)
                                .unwrap_or((0, 0));
                            transfers.push((
                                node,
                                owner,
                                self.secure_land_partible_coheirs_for(owner),
                                capacity,
                                // An atomic owner holds the whole plot with no per-share
                                // rationing, so the first division hands each co-heir its
                                // titled share at full availability (node stock still bounds
                                // the physical harvest).
                                capacity.1,
                                true,
                            ));
                        }
                    }
                    for (&owner, share) in &record.shares {
                        if dead_owners.contains(&owner) {
                            transfers.push((
                                node,
                                owner,
                                self.secure_land_partible_coheirs_for(owner),
                                (share.regen, share.cap),
                                // Carry the deceased share's remaining availability so a
                                // depleted share is inherited depleted, not refilled.
                                share.available,
                                false,
                            ));
                        }
                    }
                }

                for (node, owner, heirs, capacity, available, from_atomic_owner) in transfers {
                    let split = Self::secure_land_split_effective_capacity(
                        capacity.0, capacity.1, available, &heirs,
                    );
                    let mut stranded_added = 0u64;
                    if let Some(record) = self.land_plots.get_mut(&node) {
                        if from_atomic_owner && record.owner == Some(owner) {
                            record.owner = None;
                        }
                        record.shares.remove(&owner);
                        record.idle_counter = 0;
                        record.reserved_for = None;
                        if split.is_empty() {
                            if record.owner.is_none() && record.shares.is_empty() {
                                record.stranded_regen = 0;
                                record.stranded_cap = 0;
                            } else {
                                record.stranded_regen =
                                    record.stranded_regen.saturating_add(capacity.0);
                                record.stranded_cap =
                                    record.stranded_cap.saturating_add(capacity.1);
                                stranded_added = stranded_added.saturating_add(1);
                            }
                        } else {
                            for &(heir, share) in &split {
                                if share.regen < LAND_VIABLE_REGEN_FLOOR {
                                    record.stranded_regen =
                                        record.stranded_regen.saturating_add(share.regen);
                                    record.stranded_cap =
                                        record.stranded_cap.saturating_add(share.cap);
                                    stranded_added = stranded_added.saturating_add(1);
                                } else {
                                    Self::secure_land_add_partible_share(record, heir, share);
                                }
                            }
                        }
                    }
                    for &(heir, share) in &split {
                        if share.regen >= LAND_VIABLE_REGEN_FLOOR {
                            self.record_landowner_acquired(heir);
                        }
                    }
                    self.secure_land_stranded_shares_total = self
                        .secure_land_stranded_shares_total
                        .saturating_add(stranded_added);
                    if split.is_empty() {
                        self.secure_land_log_inheritance(
                            owner,
                            None,
                            node,
                            regime,
                            capacity,
                            (0, 0),
                        );
                    } else {
                        for (heir, share) in split {
                            let post = if share.regen < LAND_VIABLE_REGEN_FLOOR {
                                (0, 0)
                            } else {
                                (share.regen, share.cap)
                            };
                            self.secure_land_log_inheritance(
                                owner,
                                Some(heir),
                                node,
                                regime,
                                capacity,
                                post,
                            );
                        }
                    }
                }
            }
        }

        let dead_owners = dead_owners;
        // C1R: a dead share party's reservation is dissolved at ITS OWN death seam
        // (`settle_share_tenancy_for_death`), but the registry invariant is asserted inside
        // EVERY death settlement — with several same-tick deaths, a dead-but-not-yet-settled
        // share worker's reservation would trip the liveness clause mid-sequence. Clear any
        // dead agent's reservation here too, GATED on the share flag so the off path keeps
        // the base's exact dead-owner clearing (review P3: no ungated off-path change).
        let share_active = self.share_tenancy_active();
        let in_kind_active = self.in_kind_wage_active();
        let dead_reserved: BTreeSet<AgentId> = self
            .land_plots
            .values()
            .filter_map(|record| record.reserved_for)
            .filter(|&agent| {
                dead_owners.contains(&agent)
                    || (share_active && !self.private_land_live_agent(agent))
                    || (in_kind_active && !self.private_land_live_agent(agent))
            })
            .collect();
        for record in self.land_plots.values_mut() {
            if record
                .reserved_for
                .is_some_and(|agent| dead_reserved.contains(&agent))
            {
                record.reserved_for = None;
            }
        }
        debug_assert!(
            self.private_land_registry_invariant_holds(),
            "secure private land registry must settle dead owners"
        );
    }
    pub(super) fn transfer_private_land_on_death(&mut self, dead: AgentId) {
        if !self.private_land_tenure_active() {
            return;
        }
        if self.secure_land_tenure_active() {
            self.transfer_secure_private_land_on_death(dead);
            return;
        }
        let mut dead_owners: BTreeSet<AgentId> = self
            .land_plots
            .values()
            .filter_map(|record| record.owner)
            .filter(|&owner| !self.private_land_live_agent(owner))
            .collect();
        dead_owners.insert(dead);
        let heirs: BTreeMap<AgentId, Option<AgentId>> = dead_owners
            .iter()
            .map(|&owner| {
                let heir = self
                    .heir_for(owner)
                    .filter(|&candidate| self.private_land_heir_eligible(candidate));
                (owner, heir)
            })
            .collect();

        let land_market = self.land_market_active();
        // C1R: same share-gated dead-reservation clearing as the secure path above (the
        // liveness clause of the registry invariant is checked per death settlement).
        let share_active = self.share_tenancy_active();
        let in_kind_active = self.in_kind_wage_active();
        let dead_reserved: BTreeSet<AgentId> = self
            .land_plots
            .values()
            .filter_map(|record| record.reserved_for)
            .filter(|&agent| {
                heirs.contains_key(&agent)
                    || (share_active && !self.private_land_live_agent(agent))
                    || (in_kind_active && !self.private_land_live_agent(agent))
            })
            .collect();
        let mut inherited_titles = Vec::new();
        let mut acquired_titles = Vec::new();
        let mut cleared_market_titles = Vec::new();
        let mut lost = Vec::new();
        for (&node, record) in &mut self.land_plots {
            if let Some(owner) = record.owner {
                if let Some(&heir) = heirs.get(&owner) {
                    record.owner = heir;
                    record.idle_counter = 0;
                    record.reserved_for = None;
                    if land_market {
                        cleared_market_titles.push(owner);
                        if let Some(heir) = heir {
                            inherited_titles.push(heir);
                        }
                    }
                    if let Some(heir) = heir {
                        acquired_titles.push(heir);
                    }
                    if heir.is_none() {
                        lost.push((node, owner));
                    }
                }
            }
            if record
                .reserved_for
                .is_some_and(|agent| dead_reserved.contains(&agent))
            {
                record.reserved_for = None;
            }
        }
        for owner in cleared_market_titles {
            let owner_history = self.land_market_title_history.entry(owner).or_default();
            owner_history.ever_owned = true;
            owner_history.current = None;
        }
        for heir in inherited_titles {
            let heir_history = self.land_market_title_history.entry(heir).or_default();
            heir_history.ever_owned = true;
            heir_history.current = Some(LandTitleSource::Inherited);
        }
        for heir in acquired_titles {
            self.record_landowner_acquired(heir);
        }
        for (node, owner) in lost {
            // Tagged `Death` so the by-other reclaim counter never credits a heirless-death vacancy
            // as the idle-loss mechanic. (A dead owner can never re-enter, so its lapsed-quality
            // entry is inert — but it keeps the hysteresis trace's loss-set complete.)
            self.land_lost_prior_owners
                .insert(node, (owner, LandLossCause::Death));
            if let Some(quality) = self.private_land_plot_quality(node) {
                self.land_lapsed_losses.insert(owner, quality);
            }
            // A heirless death zeroes `record.owner`; under the land market the plot's market
            // listing must be cleared in the same settlement, or the registry invariant (no listing
            // on an unowned plot) trips before the next sweep clears it. Scope limit: post-promotion
            // the homestead-claim path is closed and unowned plots are not targeted, so a vacated
            // plot leaves the tradeable set as dead inventory rather than re-entering the market.
            // Empirically inert in this regime (inheritance keeps the finite plot set owned), so it
            // never shrinks supply here; surfaced explicitly rather than silently relied upon.
            if land_market {
                if let Some(state) = self.land_market_plots.get_mut(&node) {
                    state.listing = None;
                }
            }
        }
        debug_assert!(
            self.private_land_registry_invariant_holds(),
            "private land registry must not retain a dead owner after death settlement"
        );
    }
    /// AGING + OLD-AGE DEATH (G4b): advance each living householder's age by one econ
    /// tick and remove any that reach their deterministic `lifespan` via the G4a
    /// removal path, settling the estate to a household heir. Returns the old-age
    /// death count. A no-op without a demography overlay. Deterministic: ages and
    /// deaths are taken in slot order, the lifespan is a pure function of the
    /// colonist's seed, nothing is drawn.
    pub(super) fn age_and_remove_elderly(
        &mut self,
        report: &mut EconTickReport,
        wage_labor_used: &mut Vec<(AgentId, u32)>,
    ) -> u32 {
        if self.demography.is_none() {
            return 0;
        }
        let mut dying = Vec::new();
        let live_slots = self.live_colonist_slots.clone();
        for &slot in &live_slots {
            let colonist = &mut self.colonists[slot];
            let Some(lifespan) = colonist.lifespan else {
                continue;
            };
            colonist.age = colonist.age.saturating_add(1);
            if colonist.age >= lifespan {
                dying.push(colonist.id);
            }
        }
        let dying: Vec<_> = dying
            .into_iter()
            .filter(|&id| self.society.can_remove_agent(id))
            .collect();
        let mortal_producer_deaths = if self.mortal_chain_producers_active() {
            dying
                .iter()
                .filter(|&&id| {
                    self.slot_for_id(id)
                        .is_some_and(|slot| self.mortal_chain_producer_subject(slot))
                })
                .count() as u64
        } else {
            0
        };
        for &id in &dying {
            if let Some(slot) = self.slot_for_id(id) {
                self.mark_colonist_dead(slot);
            }
        }
        let secure_owner_death_snapshot: Vec<(AgentId, bool, bool)> =
            if self.secure_land_tenure_active() {
                dying
                    .iter()
                    .map(|&id| {
                        let owned_plot = self.private_land_agent_holds_any_plot(id);
                        let inherit_eligible =
                            owned_plot && self.secure_land_universal_heir_for(id).is_some();
                        (id, owned_plot, inherit_eligible)
                    })
                    .collect()
            } else {
                Vec::new()
            };
        let mut deaths = 0;
        for id in dying {
            self.record_owner_death_telemetry(id);
            // C1R death seam, old-age leg: starvation deaths route through `settle_death`,
            // which settles share tenancy BEFORE land transfer and estate routing (the dying
            // agent's society entry is still present, so a dead owner is credited its pending
            // (1 − s) contract-grain share and the estate carries it to the heir). This estate
            // path skipped that settle, so an owner's old-age death mid-contract left the
            // worker holding 100% of the pending contract grain. Inert unless share contracts
            // exist (empty-vec early return), so every non-share config is byte-unchanged.
            self.settle_in_kind_wage_for_death(id);
            let pending_share_successions = self.settle_share_tenancy_for_death(id);
            if let Some((_, true, inherit_eligible)) = secure_owner_death_snapshot
                .iter()
                .find(|&&(owner, _, _)| owner == id)
            {
                self.secure_land_owner_old_age_deaths_total = self
                    .secure_land_owner_old_age_deaths_total
                    .saturating_add(1);
                if *inherit_eligible {
                    self.secure_land_inherit_eligible_owner_deaths_total = self
                        .secure_land_inherit_eligible_owner_deaths_total
                        .saturating_add(1);
                }
            }
            // Old-age deaths must settle private land tenure too. Starvation deaths route through
            // `settle_death`, which calls `transfer_private_land_on_death` (reassign the plot to an
            // heir, clear a market listing on a heirless death); this estate path settles directly,
            // so without this an old-age death under the land market would orphan its plot to a dead
            // owner and break the registry invariant before the next death's scan lazily reclaims it.
            // Scoped to `land_market_active()`: S23a's idle-forfeiture path tolerates the lazy
            // cleanup and its `land_plots` records are digested, so changing the reassignment timing
            // there would shift the byte-identical S23a goldens.
            if self.land_market_active() || self.secure_land_tenure_active() {
                self.transfer_private_land_on_death(id);
            }
            self.finalize_share_contract_successions(pending_share_successions);
            deaths += u32::from(self.settle_estate_to_heirs(id, report, wage_labor_used));
        }
        self.old_age_deaths_total = self.old_age_deaths_total.saturating_add(u64::from(deaths));
        self.mortal_producer_old_age_deaths = self
            .mortal_producer_old_age_deaths
            .saturating_add(mortal_producer_deaths);
        deaths
    }
    /// PROVISION phase (G4b): deliver each living householder its household's
    /// renewable staple/WOOD hearth into econ stock, recording the total as a conserved
    /// source in `report.endowment`. A no-op without a demography overlay.
    /// Deterministic: slot order, no RNG. The provision keeps members fed (so deaths
    /// are old age, not starvation) and supplies the wood-surplus household its
    /// tradeable surplus. The staple is the settlement's hunger good
    /// ([`KnownGoods::hunger`]) — FOOD on a `lineages` colony, bread on the G5b
    /// frontier — so members are always provisioned the very good they eat.
    pub(super) fn deliver_demography_provisions(&mut self, report: &mut EconTickReport) {
        let Some(demo) = self.demography.clone() else {
            return;
        };
        let staple = self.known.hunger;
        // S12: own-labor subsistence retires the demographic FOOD mint (the food
        // scaffold) — only the WOOD/warmth provision stays an endowment (hunger-only
        // scope). The lineage then earns its food from the market (selling its WOOD
        // provision for the staple), exactly the retirement test 2 pins
        // (`endowment[staple] == 0`). S21d.0 retires the SAME demographic food mint
        // independent of forage (no FORAGE good interned) — the open-survival probe.
        let mint_food = !self.own_labor_subsistence_can_run() && !self.retire_food_mints();
        // Collect (id, household) first so the colonists borrow is released before the
        // society is mutated.
        let members: Vec<(AgentId, usize)> = self
            .live_colonist_slots
            .iter()
            .filter_map(|&slot| {
                let colonist = &self.colonists[slot];
                colonist.household.map(|h| (colonist.id, h))
            })
            .collect();
        for (id, h) in members {
            let spec = &demo.households[h];
            // C3R.e (impl-67): the B support-withdrawal gate — a producer household's food hearth
            // is withdrawn once `econ_tick >= producer_support_until_tick`. Lineage households and
            // every non-B config (`producer_support_until_tick == None`) are unaffected, so the run
            // is byte-identical off B.
            if mint_food && (!self.is_producer_household(h) || self.producer_support_active()) {
                // C3R.e (impl-67): a producer household's food hearth under a B cell is a support
                // mint — origin-flag it (only producer households under B ever reach here with a
                // non-zero `food_provision`; every lineage hearth stays plain).
                let support_mint =
                    self.is_producer_household(h) && self.producer_support_configured();
                self.deliver_demography_provision_unit(
                    id,
                    Some(h),
                    staple,
                    spec.food_provision,
                    support_mint,
                    report,
                );
            }
            // WOOD is not tracked food, so this never touches the acquisition ledger (never flagged).
            self.deliver_demography_provision_unit(
                id,
                Some(h),
                WOOD,
                spec.wood_provision,
                false,
                report,
            );
        }
    }
    pub(super) fn deliver_demography_provision_unit(
        &mut self,
        id: AgentId,
        household: Option<usize>,
        good: GoodId,
        provision: u32,
        // C3R.e (impl-67): stamp the credited tracked-food lot as INTERVENTION-ORIGIN — set only
        // for a B cell's producer-house support mint (the hearth staple / the cushion's staple
        // leg), so the withdrawn support inventory is exhaustion-tracked. `false` everywhere else
        // (every lineage hearth, every non-B config), so the ledger stays behaviour-identical.
        intervention: bool,
        report: &mut EconTickReport,
    ) {
        if provision == 0 {
            return;
        }
        let Some(held) = self
            .society
            .agents
            .get(id)
            .map(|agent| agent.stock.get(good))
        else {
            return;
        };
        let credited = provision.min(u32::MAX - held);
        if credited > 0 && self.society.credit_stock(id, good, credited) {
            // DH.a (P1-1): the runtime support-delivery seam. In the closed regime EVERY demographic
            // provision / producer-subsistence mint is a B-arm support credit (there is no lineage
            // hearth) — endowed, since support is not production. Emit at the real mint.
            self.closure_emit(closure::ClosureEventKind::BSupportCredit {
                agent: id,
                good,
                qty: credited,
            });
            *report.endowment.entry(good).or_insert(0) += u64::from(credited);
            if good == self.known.hunger {
                if household.is_some_and(|h| self.is_producer_household(h)) {
                    self.producer_house_hearth_food_minted = self
                        .producer_house_hearth_food_minted
                        .saturating_add(u64::from(credited));
                } else if household.is_some() {
                    self.non_producer_hearth_food_minted = self
                        .non_producer_hearth_food_minted
                        .saturating_add(u64::from(credited));
                }
            }
            // S21d.1: a hearth MINT of the tracked food (the demographic `food_provision`
            // or the producer staple floor) enters as the `SeededMinted` channel — the very
            // term the open-survival probe retires, so its only effect here is on the
            // mints-ON control. WOOD/other provisions are not tracked food, so no-op.
            // C3R.e (impl-67): a B cell's producer-house support mint enters as an
            // INTERVENTION-ORIGIN `SeededMinted` lot (resale-proof), so the withdrawn support is
            // exhaustion-tracked; every other mint stays plain, so the ledger is unchanged off B.
            if self.acquisition_ledger_active() && Some(good) == self.acquisition_food_good() {
                if intervention {
                    self.acquisition.credit_intervention(
                        id,
                        FoodChannel::SeededMinted,
                        u64::from(credited),
                    );
                } else {
                    self.acquisition
                        .credit(id, FoodChannel::SeededMinted, u64::from(credited));
                }
            }
        }
    }
    pub(super) fn birth_cap_for_household(&self, household: usize, lineage_cap: u16) -> usize {
        let producer_cap = self
            .producer_household_start()
            .filter(|&start| household >= start && household < start + MORTAL_PRODUCER_HOUSEHOLDS)
            .and_then(|_| self.chain.as_ref().map(|chain| chain.producer_house_cap))
            .unwrap_or(0);
        if producer_cap > 0 {
            usize::from(producer_cap)
        } else {
            usize::from(lineage_cap)
        }
    }
    /// C3R.e (impl-67): `ignition` re-flags the moved acquisition-ledger lots as
    /// intervention-origin (A1's one-shot injection); the recurring SufficiencyControl caller passes
    /// `false`, so its moved lots keep their ordinary origin and the run is byte-identical off A1.
    pub(super) fn transfer_birth_stock(
        &mut self,
        donor: AgentId,
        recipient: AgentId,
        staple: GoodId,
        qty: u32,
        ignition: bool,
    ) -> bool {
        let recipient_held = self
            .society
            .agents
            .get(recipient)
            .map(|agent| agent.stock.get(staple));
        if !self.society.debit_stock(donor, staple, qty) {
            return false;
        }
        let credited = self.society.credit_stock(recipient, staple, qty);
        let credited_qty = recipient_held
            .and_then(|before| {
                self.society
                    .agents
                    .get(recipient)
                    .map(|agent| agent.stock.get(staple).saturating_sub(before))
            })
            .unwrap_or(0);
        if !credited || credited_qty != qty {
            if credited_qty > 0 {
                let removed = self.society.debit_stock(recipient, staple, credited_qty);
                assert!(removed, "partial birth-stock credit rollback must fit");
            }
            let rolled_back = self.society.credit_stock(donor, staple, qty);
            assert!(
                rolled_back,
                "birth-stock injection rollback must fit the donor"
            );
            return false;
        }
        if Some(staple) == self.provenance_bread_good() {
            self.bread_provenance
                .transfer(donor, recipient, u64::from(qty));
        }
        if Some(staple) == self.acquisition_food_good() {
            if ignition {
                self.acquisition.transfer_preserve_as_intervention(
                    donor,
                    recipient,
                    u64::from(qty),
                );
            } else {
                self.acquisition
                    .transfer_preserve(donor, recipient, u64::from(qty));
            }
        }
        // DH.a: a committed staple move between existing agents — a bucket-preserving transfer.
        self.closure_note_staple_transfer(donor, recipient, staple, qty);
        true
    }
    /// C3R.e (impl-67): the shared birth-stock injection body, factored out of the recurring
    /// SufficiencyControl driver so the A1 one-shot can reuse it. With `ignition = false` this is
    /// byte-identical to the landed control (ordinary donors, no origin flag). With `ignition =
    /// true` the donor pool EXCLUDES producer households and every moved unit is origin-flagged.
    /// Returns the injected households and the total staple quantity moved (the ignition dose).
    pub(super) fn inject_birth_stock(&mut self, ignition: bool) -> (Vec<usize>, u64) {
        // C3R.e debt repair: the one-shot ignition records a per-household gate decomposition
        // (runtime-only; the RoR's disclosed verification gap — WHY the dose fell short).

        let Some((household_count, birth_interval, max_household_size, hunger_ceiling, target)) =
            self.demography.as_ref().map(|demo| {
                (
                    demo.households.len(),
                    demo.birth_interval,
                    demo.max_household_size,
                    demo.birth_hunger_ceiling,
                    demo.child_food_endowment,
                )
            })
        else {
            return (Vec::new(), 0);
        };
        let staple = self.known.hunger;
        let mut injected = Vec::new();
        let mut injected_qty = 0u64;
        let mut injected_recipients = BTreeSet::new();
        for household in 0..household_count {
            if !self.is_producer_household(household) {
                continue;
            }
            let next_eligible = self.households[household]
                .last_birth_tick
                .map_or(birth_interval, |tick| tick + birth_interval);
            if self.econ_tick < next_eligible {
                if ignition {
                    self.ignition_gate_blocked_interval =
                        self.ignition_gate_blocked_interval.saturating_add(1);
                }
                continue;
            }
            let member_slots: Vec<_> = self
                .live_colonist_slots
                .iter()
                .copied()
                .filter(|&slot| self.colonists[slot].household == Some(household))
                .collect();
            if member_slots.is_empty() {
                if ignition {
                    self.ignition_gate_extinct = self.ignition_gate_extinct.saturating_add(1);
                }
                continue;
            }
            if member_slots.len() >= self.birth_cap_for_household(household, max_household_size) {
                if ignition {
                    self.ignition_gate_blocked_cap =
                        self.ignition_gate_blocked_cap.saturating_add(1);
                }
                continue;
            }
            if !member_slots
                .iter()
                .all(|&slot| self.colonists[slot].need.hunger <= hunger_ceiling)
            {
                if ignition {
                    self.ignition_gate_blocked_hunger =
                        self.ignition_gate_blocked_hunger.saturating_add(1);
                }
                continue;
            }
            if member_slots.iter().any(|&slot| {
                self.society
                    .free_stock_after_all_reserves(self.colonists[slot].id, staple)
                    >= target
            }) {
                if ignition {
                    self.ignition_gate_suppressed_at_target =
                        self.ignition_gate_suppressed_at_target.saturating_add(1);
                }
                continue;
            }
            self.birth_stock_eligible_opportunities =
                self.birth_stock_eligible_opportunities.saturating_add(1);
            let recipient_slot = member_slots
                .iter()
                .copied()
                .max_by_key(|&slot| {
                    let id = self.colonists[slot].id;
                    (
                        self.society.free_gold_after_all_reserves(id).0,
                        std::cmp::Reverse(slot),
                    )
                })
                .expect("eligible household has a member");
            let donor = self
                .live_colonist_slots
                .iter()
                .copied()
                .filter(|&slot| {
                    // C3R.e A1: an ignition additionally excludes producer households from the
                    // donor pool (a pure redistribution FROM the non-producer surround).
                    self.colonists[slot].household != Some(household)
                        && !injected_recipients.contains(&self.colonists[slot].id)
                        && !(ignition
                            && self.colonists[slot]
                                .household
                                .is_some_and(|h| self.is_producer_household(h)))
                })
                .map(|slot| {
                    let id = self.colonists[slot].id;
                    (
                        self.society.free_stock_after_all_reserves(id, staple),
                        std::cmp::Reverse(slot),
                        id,
                    )
                })
                .max_by_key(|&(held, slot, _)| (held, slot));
            let Some((held, _, donor)) = donor.filter(|&(held, _, _)| held >= target) else {
                self.birth_stock_source_shortfalls =
                    self.birth_stock_source_shortfalls.saturating_add(1);
                if ignition {
                    self.ignition_gate_donor_shortfall =
                        self.ignition_gate_donor_shortfall.saturating_add(1);
                }
                continue;
            };
            debug_assert!(held >= target);
            let recipient = self.colonists[recipient_slot].id;
            if self.transfer_birth_stock(donor, recipient, staple, target, ignition) {
                self.birth_stock_injections_completed =
                    self.birth_stock_injections_completed.saturating_add(1);
                injected.push(household);
                injected_qty = injected_qty.saturating_add(u64::from(target));
                injected_recipients.insert(recipient);
            } else {
                self.birth_stock_source_shortfalls =
                    self.birth_stock_source_shortfalls.saturating_add(1);
            }
        }
        (injected, injected_qty)
    }
}
