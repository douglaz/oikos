//! Land-market subsystem.
//!
//! Private-land tenure, the land market (pricing, listings, carrying costs, matching,
//! rent), and secure-land inheritance/succession. Extracted verbatim from `mod.rs` (pure
//! code motion): the `impl Settlement` methods move into the block below (module-private
//! ones become pub(super) — the exact scope they had inside `settlement`); the three
//! plot-layout free functions become pub(super) and are re-imported by the parent via
//! `use land_market::*`. The land value types stay in `mod.rs`.

use super::*;

impl Settlement {
    pub(super) fn private_land_harvest_task_node(task: Task) -> Option<NodeId> {
        match task {
            Task::GoHarvest(node, _) | Task::GoHarvestWithRoom(node, _, _) => Some(node),
            _ => None,
        }
    }
    pub(super) fn private_land_task_with_node(task: Task, node: NodeId) -> Task {
        match task {
            Task::GoHarvest(_, want) => Task::GoHarvest(node, want),
            Task::GoHarvestWithRoom(_, want, room) => Task::GoHarvestWithRoom(node, want, room),
            other => other,
        }
    }
    pub(super) fn private_land_plot_has_stock(&self, node: NodeId) -> bool {
        self.world
            .node(node)
            .is_some_and(|plot| plot.stock > 0 && self.land_plots.contains_key(&node))
    }
    pub(super) fn private_land_agent_distance(&self, agent: AgentId, node: NodeId) -> Option<u32> {
        let from = self.world.agent_pos(agent)?;
        let to = self.world.node(node)?.pos;
        Some(self.world.grid_distance(from, to))
    }
    pub(super) fn private_land_plot_quality(&self, node: NodeId) -> Option<LandPlotQuality> {
        let plot = self.world.node(node)?;
        let exchange = self.world.stockpile(self.exchange)?.pos;
        Some(LandPlotQuality {
            regen: plot.regen_per_tick,
            cap: plot.cap,
            distance: self.world.grid_distance(exchange, plot.pos),
        })
    }
    pub(super) fn private_land_viable_marginal_node(&self, node: NodeId) -> bool {
        // §2a is a pure floor test (unowned + reachable + regen/cap ≥ floors). It is NOT pinned to
        // the marginal-cap literal: a future cap sweep must keep working, and an unowned good plot
        // legitimately counts as viable homesteadable land too. The shared consts can't drift.
        self.private_land_plot_quality(node).is_some_and(|quality| {
            quality.regen >= LAND_VIABLE_REGEN_FLOOR && quality.cap >= LAND_VIABLE_CAP_FLOOR
        })
    }
    pub(super) fn private_land_has_better_stayer(
        &self,
        agent: AgentId,
        quality: LandPlotQuality,
    ) -> bool {
        self.land_plots.iter().any(|(&node, record)| {
            Self::private_land_record_holders(record)
                .into_iter()
                .any(|owner| owner != agent)
                && self.private_land_plot_quality(node).is_some_and(|other| {
                    other.regen > quality.regen
                        || other.cap > quality.cap
                        || other.distance < quality.distance
                })
        })
    }
    pub(super) fn private_land_live_agent(&self, agent: AgentId) -> bool {
        self.colonist_slot_by_id
            .get(&agent)
            .is_some_and(|&slot| self.colonists[slot].alive)
    }
    pub(super) fn private_land_record_has_holding(record: &LandPlotRecord) -> bool {
        record.owner.is_some() || !record.shares.is_empty()
    }
    pub(super) fn private_land_record_agent_holds(record: &LandPlotRecord, agent: AgentId) -> bool {
        record.owner == Some(agent) || record.shares.contains_key(&agent)
    }
    pub(super) fn private_land_record_claimable(record: &LandPlotRecord) -> bool {
        record.owner.is_none()
            && record.shares.is_empty()
            && record.reserved_for.is_none()
            && record.stranded_regen == 0
            && record.stranded_cap == 0
    }
    pub(super) fn private_land_record_primary_holder(record: &LandPlotRecord) -> Option<AgentId> {
        record
            .owner
            .or_else(|| record.shares.keys().next().copied())
    }
    pub(super) fn private_land_record_holders(record: &LandPlotRecord) -> Vec<AgentId> {
        let mut owners = BTreeSet::new();
        if let Some(owner) = record.owner {
            owners.insert(owner);
        }
        owners.extend(record.shares.keys().copied());
        owners.into_iter().collect()
    }
    pub(super) fn private_land_record_held_by_live_other(
        &self,
        record: &LandPlotRecord,
        agent: AgentId,
    ) -> bool {
        if Self::private_land_record_agent_holds(record, agent) {
            return false;
        }
        Self::private_land_record_holders(record)
            .into_iter()
            .any(|owner| owner != agent && self.private_land_live_agent(owner))
    }
    pub(super) fn private_land_harvest_room_for(
        &self,
        agent: AgentId,
        node: NodeId,
        requested: u32,
    ) -> u32 {
        if !self.secure_partible_land_active() {
            return requested;
        }
        let Some(record) = self.land_plots.get(&node) else {
            return requested;
        };
        let Some(share) = record.shares.get(&agent) else {
            return requested;
        };
        requested.min(share.available)
    }
    pub(super) fn private_land_share_capacity(
        &self,
        record: &LandPlotRecord,
        node: NodeId,
        agent: AgentId,
    ) -> Option<(u32, u32)> {
        if let Some(share) = record.shares.get(&agent) {
            return Some((share.regen, share.cap));
        }
        if record.owner == Some(agent) {
            let quality = self.private_land_plot_quality(node)?;
            return Some((quality.regen, quality.cap));
        }
        None
    }
    pub(super) fn private_land_replenish_partible_shares(&mut self) {
        if !self.secure_partible_land_active() {
            return;
        }
        for record in self.land_plots.values_mut() {
            for share in record.shares.values_mut() {
                share.available = share.available.saturating_add(share.regen).min(share.cap);
            }
        }
    }
    pub(super) fn private_land_debit_partible_share(
        &mut self,
        node: NodeId,
        agent: AgentId,
        moved: u32,
    ) {
        if !self.secure_partible_land_active() || moved == 0 {
            return;
        }
        if let Some(share) = self
            .land_plots
            .get_mut(&node)
            .and_then(|record| record.shares.get_mut(&agent))
        {
            share.available = share.available.saturating_sub(moved);
        }
    }
    pub(super) fn private_land_target_for_agent(
        &self,
        agent: AgentId,
        reserved_unowned: &BTreeSet<NodeId>,
    ) -> Option<NodeId> {
        if !self.private_land_tenure_active() {
            return self.grain_node();
        }
        let harvest_gate = self.private_land_harvest_gate_active();
        let rank = |this: &Self, node: NodeId| {
            this.private_land_agent_distance(agent, node)
                .map(|distance| (distance, node.0))
        };
        if !harvest_gate {
            return self
                .land_plots
                .keys()
                .copied()
                .filter(|&node| self.private_land_plot_has_stock(node))
                .filter_map(|node| rank(self, node).map(|key| (key, node)))
                .min_by_key(|(key, _)| *key)
                .map(|(_, node)| node);
        }

        let own = self
            .land_plots
            .iter()
            .filter(|&(&node, record)| {
                self.private_land_plot_has_stock(node)
                    && !self.share_plot_reserved_against_owner(agent, node)
                    && !self.in_kind_plot_reserved_against_owner(agent, node)
                    && (Self::private_land_record_agent_holds(record, agent)
                        || record.reserved_for == Some(agent))
                    && self.private_land_harvest_room_for(agent, node, 1) > 0
            })
            .filter_map(|(&node, _)| rank(self, node).map(|key| (key, node)))
            .min_by_key(|(key, _)| *key)
            .map(|(_, node)| node);
        if own.is_some() {
            return own;
        }

        if self.land_market_active() && self.current_money_good() == Some(SALT) {
            return None;
        }

        if !self.mortal_landowner_claim_eligible(agent) {
            return None;
        }

        self.land_plots
            .iter()
            .filter(|&(&node, record)| {
                self.private_land_plot_has_stock(node)
                    && Self::private_land_record_claimable(record)
                    && !reserved_unowned.contains(&node)
            })
            .filter_map(|(&node, _)| rank(self, node).map(|key| (key, node)))
            .min_by_key(|(key, _)| *key)
            .map(|(_, node)| node)
    }
    pub(super) fn private_land_validate_harvest_tasks(&mut self) {
        if !self.private_land_tenure_active() || !self.private_land_harvest_gate_active() {
            return;
        }

        let mut ids: Vec<AgentId> = self
            .live_colonist_slots
            .iter()
            .map(|&slot| self.colonists[slot].id)
            .collect();
        ids.sort();

        // Post-promotion under the land market the homestead path is closed (§3.2/§3.4): an unowned
        // plot is no longer free-claimable, only bought. A would-be homesteader heading to an unowned
        // plot must be rerouted off it BEFORE `world.tick`, else it harvests the grain for free this
        // tick (the post-claim `continue` in `apply_worked_events` would only suppress the title, not
        // the extraction) and bypasses the market entry constraint.
        let homestead_closed = self.land_market_active() && self.current_money_good() == Some(SALT);

        let mut invalid = Vec::new();
        let mut targeters: BTreeMap<NodeId, Vec<(AgentId, Task)>> = BTreeMap::new();
        for id in ids {
            let Some(task) = self.world.agent_task(id) else {
                continue;
            };
            let Some(node) = Self::private_land_harvest_task_node(task) else {
                continue;
            };
            let Some(record) = self.land_plots.get(&node).cloned() else {
                continue;
            };
            if self.share_plot_reserved_against_owner(id, node) {
                self.share_reservation_collision =
                    self.share_reservation_collision.saturating_add(1);
                invalid.push((id, task));
                continue;
            }
            if self.in_kind_plot_reserved_against_owner(id, node) {
                self.in_kind_reservation_collision =
                    self.in_kind_reservation_collision.saturating_add(1);
                invalid.push((id, task));
                continue;
            }
            let share_worker_admitted = self.share_worker_admitted_to(id, node, &record)
                || self.in_kind_worker_admitted_to(id, node, &record);
            let owned_by_other_live = self.private_land_record_held_by_live_other(&record, id);
            let reserved_by_other = record.reserved_for.is_some_and(|owner| owner != id);
            if !share_worker_admitted && (owned_by_other_live || reserved_by_other) {
                // A true exclusion: this agent is denied a plot held/reserved by another live owner.
                // Counted separately from the stampede reroutes below so §4 non-vacuity can require
                // ownership to actually gate harvest, not merely thin out unowned-plot contenders.
                self.land_owner_gate_denials_total += 1;
                invalid.push((id, task));
            } else if Self::private_land_record_claimable(&record) {
                if homestead_closed || !self.mortal_landowner_claim_eligible(id) {
                    // Reroute the would-be homesteader to its own plot if it holds one, else to Idle
                    // (`private_land_target_for_agent` returns `None` for a post-promotion non-owner).
                    invalid.push((id, task));
                } else {
                    targeters.entry(node).or_default().push((id, task));
                }
            }
        }

        let mut reserved = BTreeSet::new();
        let mut reroute = invalid;
        for (node, mut contenders) in targeters {
            contenders.sort_by_key(|(id, _)| {
                (
                    self.private_land_agent_distance(*id, node)
                        .unwrap_or(u32::MAX),
                    id.0,
                )
            });
            if !contenders.is_empty() {
                reserved.insert(node);
            }
            for (id, task) in contenders.into_iter().skip(1) {
                reroute.push((id, task));
            }
        }

        reroute.sort_by_key(|(id, _)| id.0);
        for (id, task) in reroute {
            self.land_harvest_denials_total += 1;
            if let Some(node) = self.private_land_target_for_agent(id, &reserved) {
                if self
                    .land_plots
                    .get(&node)
                    .is_some_and(Self::private_land_record_claimable)
                {
                    reserved.insert(node);
                }
                self.world
                    .assign_task(id, Self::private_land_task_with_node(task, node));
            } else {
                self.world.assign_task(id, Task::Idle);
            }
        }
    }
    pub(super) fn private_land_harvest_snapshot(&self) -> Vec<(AgentId, Task, u32)> {
        if !self.private_land_tenure_active() {
            return Vec::new();
        }
        let Some(grain) = self.chain.as_ref().map(|chain| chain.content.grain()) else {
            return Vec::new();
        };
        self.live_colonist_slots
            .iter()
            .filter_map(|&slot| {
                let id = self.colonists[slot].id;
                let task = self.world.agent_task(id)?;
                let node = Self::private_land_harvest_task_node(task)?;
                self.land_plots.contains_key(&node).then_some((
                    id,
                    task,
                    self.world.agent_carry(id, grain),
                ))
            })
            .collect()
    }
    pub(super) fn private_land_worked_events(
        &self,
        before: &[(AgentId, Task, u32)],
    ) -> Vec<WorkedLandEvent> {
        if before.is_empty() {
            return Vec::new();
        }
        let Some(grain) = self.chain.as_ref().map(|chain| chain.content.grain()) else {
            return Vec::new();
        };
        before
            .iter()
            .filter_map(|&(agent, task, prev_carry)| {
                let node = Self::private_land_harvest_task_node(task)?;
                let now = self.world.agent_carry(agent, grain);
                (now > prev_carry).then_some(WorkedLandEvent {
                    agent,
                    node,
                    moved: now - prev_carry,
                })
            })
            .collect()
    }
    pub(super) fn private_land_apply_worked_events(&mut self, events: &[WorkedLandEvent]) {
        if events.is_empty() || !self.private_land_tenure_active() {
            return;
        }
        for event in events {
            debug_assert!(event.moved > 0);
            *self.land_plot_harvest_totals.entry(event.node).or_insert(0) += u64::from(event.moved);
            if self.land_market_active() {
                *self
                    .land_market_yield_this_tick
                    .entry(event.node)
                    .or_insert(0) += event.moved;
            }
            let carried_share_contract_id = self
                .share_contract_for_worker(event.agent)
                .filter(|contract| contract.node == event.node)
                .map(|contract| contract.id);
            let carried_in_kind_contract_id = self
                .in_kind_contract_for_worker(event.agent)
                .filter(|contract| contract.node == event.node)
                .map(|contract| contract.id);
            if let Some(&slot) = self.colonist_slot_by_id.get(&event.agent) {
                if self.colonists[slot].alive {
                    self.colonists[slot].carried_grain_source = Some(event.node);
                    self.colonists[slot].carried_share_contract_id = carried_share_contract_id;
                    self.colonists[slot].carried_in_kind_contract_id = carried_in_kind_contract_id;
                }
            }
            let Some(record) = self.land_plots.get(&event.node).cloned() else {
                continue;
            };
            if self.share_plot_reserved_against_owner(event.agent, event.node) {
                self.share_reservation_collision =
                    self.share_reservation_collision.saturating_add(1);
                continue;
            }
            if self.in_kind_plot_reserved_against_owner(event.agent, event.node) {
                self.in_kind_reservation_collision =
                    self.in_kind_reservation_collision.saturating_add(1);
                continue;
            }
            let share_worker_admitted =
                self.share_worker_admitted_to(event.agent, event.node, &record)
                    || self.in_kind_worker_admitted_to(event.agent, event.node, &record);
            // C1R latent-composition guard (review P3): the owner-exclusion above blocks only
            // `contract.owner`; on a PARTIBLE config a co-holder in `record.shares` would still
            // be admitted to a share-reserved plot. Inert on the impartible C1R base — trip
            // loudly if the composition is ever built.
            #[cfg(debug_assertions)]
            if let Some(contract) = self.share_contract_for_node(event.node) {
                debug_assert!(
                    contract.worker == event.agent
                        || !Self::private_land_record_agent_holds(&record, event.agent),
                    "C1R: a partible co-holder drew from a share-reserved plot — the \
                     owner-exclusion guard covers only the title owner"
                );
            }
            if Self::private_land_record_has_holding(&record) {
                if share_worker_admitted {
                    // A share worker may harvest the owner's reserved plot but never acquires title.
                } else if !Self::private_land_record_agent_holds(&record, event.agent) {
                    // A non-owner pulled grain from a plot HELD by another. Impossible under the
                    // headline (validation reroutes non-owners off owned plots before world.tick);
                    // > 0 only when `harvest_gate` is off — the load-bearing proof the gate bites.
                    self.land_nonowner_harvest_of_owned_total += 1;
                } else {
                    self.private_land_debit_partible_share(event.node, event.agent, event.moved);
                }
                // Already owned (by this agent or another) — no (re)claim.
                continue;
            }
            if record
                .reserved_for
                .is_some_and(|reserved| reserved != event.agent)
            {
                continue;
            }
            if record.stranded_regen > 0 || record.stranded_cap > 0 {
                continue;
            }
            if self.land_market_active() && self.current_money_good() == Some(SALT) {
                // Post-promotion the homestead claim path is closed. Under the headline
                // (`harvest_gate` on) `validate_harvest_tasks` already rerouted non-owners off
                // unowned plots, so this is the defense-in-depth net; under `non_excludable_title`
                // (gate off) validation is skipped, so this is the load-bearing suppression that
                // keeps a free harvest from minting post-promotion title.
                continue;
            }
            if !self.mortal_landowner_claim_eligible(event.agent) {
                continue;
            }

            let was_non_owner = !self
                .land_plots
                .values()
                .any(|record| Self::private_land_record_agent_holds(record, event.agent));
            let viable_marginal = self.private_land_viable_marginal_node(event.node);
            let quality = self.private_land_plot_quality(event.node);
            let lapsed_reentry_worse = quality.is_some_and(|q| {
                self.land_lapsed_losses.contains_key(&event.agent)
                    && self.private_land_has_better_stayer(event.agent, q)
            });
            let prior_lost = self.land_lost_prior_owners.remove(&event.node);
            let partible_active = self.secure_partible_land_active();

            if let Some(record) = self.land_plots.get_mut(&event.node) {
                if partible_active {
                    if let Some(quality) = quality {
                        record.owner = None;
                        record.shares.insert(
                            event.agent,
                            LandShare {
                                regen: quality.regen,
                                cap: quality.cap,
                                available: quality.cap.saturating_sub(event.moved),
                            },
                        );
                    }
                } else {
                    record.owner = Some(event.agent);
                }
                record.idle_counter = 0;
                record.reserved_for = None;
            }
            self.record_landowner_acquired(event.agent);
            self.land_claims_total += 1;
            if self.land_market_active() {
                let history = self
                    .land_market_title_history
                    .entry(event.agent)
                    .or_default();
                history.ever_owned = true;
                history.current = Some(LandTitleSource::OriginalClaim);
            }
            // Non-vacuity wants the exact mechanic: a plot LOST ON IDLE then re-homesteaded by a
            // DIFFERENT agent. A plot vacated by a heirless death (cause `Death`) is excluded — it
            // is not the loss-on-idle the spec demands the mechanism demonstrate.
            if let Some((prior, LandLossCause::Idle)) = prior_lost {
                if prior != event.agent {
                    self.land_reclaims_by_other_total += 1;
                }
            }
            if was_non_owner && viable_marginal {
                self.land_marginal_nonowner_claims_total += 1;
            }
            if lapsed_reentry_worse {
                self.land_lapsed_reentry_worse_total += 1;
                self.land_lapsed_losses.remove(&event.agent);
            }
        }
    }
    pub(super) fn private_land_advance_idle_counters(&mut self, events: &[WorkedLandEvent]) {
        if !self.private_land_tenure_active() {
            return;
        }
        self.private_land_replenish_partible_shares();
        let land_market = self.land_market_active();
        let forfeit_on_idle = self.private_land_forfeit_on_idle_active();
        if !forfeit_on_idle && !land_market {
            return;
        }
        let Some(limit) = self.chain.as_ref().map(|chain| chain.land_idle_limit) else {
            return;
        };
        if limit == 0 && forfeit_on_idle {
            return;
        }

        // Engagement is tracked per (node, agent): a plot's idle clock resets only when its OWN
        // owner engages it. Under the `non_excludable_deed` control (harvest_gate = false) a
        // non-owner may harvest/carry-from/target an owned plot, but that is NOT the owner tending
        // their tenure, so it must not keep an absentee owner's plot alive (the forfeiture rule is
        // owner engagement). Under the headline (harvest_gate on) non-owners are rerouted off owned
        // plots, so only the owner ever appears here and this is behaviour-identical.
        let mut engaged_by: BTreeMap<NodeId, BTreeSet<AgentId>> = BTreeMap::new();
        for event in events {
            engaged_by
                .entry(event.node)
                .or_default()
                .insert(event.agent);
        }
        for &slot in &self.live_colonist_slots {
            let colonist = &self.colonists[slot];
            if let Some(node) = colonist.carried_grain_source {
                engaged_by.entry(node).or_default().insert(colonist.id);
            }
            if let Some(task) = self.world.agent_task(colonist.id) {
                if let Some(node) = Self::private_land_harvest_task_node(task) {
                    engaged_by.entry(node).or_default().insert(colonist.id);
                }
            }
        }

        let reclaim_reserved = self
            .chain
            .as_ref()
            .is_some_and(|chain| chain.reclaim_reserved_for_prior_owner);
        let mut forfeits = Vec::new();
        for (&node, record) in &mut self.land_plots {
            let Some(owner) = record.owner else {
                record.idle_counter = 0;
                continue;
            };
            let owner_engaged = engaged_by
                .get(&node)
                .is_some_and(|agents| agents.contains(&owner));
            if owner_engaged {
                record.idle_counter = 0;
                continue;
            }
            record.idle_counter = record.idle_counter.saturating_add(1);
            if forfeit_on_idle && record.idle_counter >= limit {
                forfeits.push((node, owner));
            }
        }

        for (node, owner) in forfeits {
            let quality = self.private_land_plot_quality(node);
            if let Some(record) = self.land_plots.get_mut(&node) {
                record.owner = None;
                record.idle_counter = 0;
                record.reserved_for = reclaim_reserved.then_some(owner);
            }
            self.land_idle_losses_total += 1;
            self.land_lost_prior_owners
                .insert(node, (owner, LandLossCause::Idle));
            if let Some(quality) = quality {
                self.land_lapsed_losses.insert(owner, quality);
            }
        }
    }
    pub(super) fn land_market_initial_price(&self, node: NodeId) -> u64 {
        let rent = self
            .private_land_plot_quality(node)
            .map_or(1, Self::land_quality_prior_rent);
        self.land_market_price_from_rent(rent)
    }
    pub(super) fn land_market_price_from_rent(&self, rent: u64) -> u64 {
        let factor = self
            .chain
            .as_ref()
            .map_or(LAND_PRICE_CAP_FACTOR_DEFAULT, |chain| {
                chain.land_price_cap_factor
            });
        if factor == 0 {
            0
        } else {
            factor.saturating_mul(rent).max(LAND_PRICE_MIN)
        }
    }
    pub(super) fn land_market_rent_basis(&self, node: NodeId) -> u64 {
        let prior = self
            .private_land_plot_quality(node)
            .map_or(1, Self::land_quality_prior_rent);
        let Some(state) = self.land_market_plots.get(&node) else {
            return prior;
        };
        let realized_ticks = state
            .yield_history
            .iter()
            .filter(|entry| entry.qty > 0)
            .count();
        if realized_ticks < LAND_MIN_RENT_HISTORY {
            return prior;
        }
        let sum: u64 = state
            .yield_history
            .iter()
            .map(|entry| u64::from(entry.qty))
            .sum();
        let periods = u64::try_from(state.yield_history.len()).unwrap_or(1).max(1);
        ((sum + periods / 2) / periods).max(1)
    }
    pub(super) fn land_market_base_price(&self, node: NodeId) -> u64 {
        self.land_market_price_from_rent(self.land_market_rent_basis(node))
    }
    pub(super) fn land_market_local_sale_mean(&self, node: NodeId) -> Option<u64> {
        let origin = self.world.node(node)?.pos;
        let mut sales: Vec<((u32, u32), u64)> = self
            .land_market_plots
            .iter()
            .filter_map(|(&other_node, state)| {
                let price = state.last_sale_price?;
                let pos = self.world.node(other_node)?.pos;
                let distance = self.world.grid_distance(origin, pos);
                Some(((distance, other_node.0), price))
            })
            .collect();
        sales.sort_by_key(|(key, _)| *key);
        let mut sum = 0u64;
        let mut count = 0u64;
        for (_, price) in sales.into_iter().take(LAND_SALE_HISTORY_K) {
            sum = sum.saturating_add(price);
            count += 1;
        }
        (count > 0).then(|| (sum + count / 2) / count)
    }
    pub(super) fn land_market_listed_price(&self, node: NodeId) -> u64 {
        let factor = self
            .chain
            .as_ref()
            .map_or(LAND_PRICE_CAP_FACTOR_DEFAULT, |chain| {
                chain.land_price_cap_factor
            });
        if factor == 0 {
            return 0;
        }
        let base = self.land_market_base_price(node);
        let blended = if let Some(mean) = self.land_market_local_sale_mean(node) {
            let base_weight = 10_000u64.saturating_sub(LAND_SALE_HISTORY_WEIGHT_BPS);
            (base
                .saturating_mul(base_weight)
                .saturating_add(mean.saturating_mul(LAND_SALE_HISTORY_WEIGHT_BPS))
                .saturating_add(5_000))
                / 10_000
        } else {
            base
        };
        let upper = LAND_PRICE_MIN.max(base.saturating_mul(4));
        blended.clamp(LAND_PRICE_MIN, upper)
    }
    pub(super) fn land_market_discounted_ask(&self, price: u64) -> u64 {
        if self
            .chain
            .as_ref()
            .is_some_and(|chain| chain.land_price_cap_factor == 0)
        {
            return 0;
        }
        let discounted =
            price.saturating_mul(10_000u64.saturating_sub(LAND_FORECLOSE_DISCOUNT_BPS)) / 10_000;
        discounted.max(LAND_PRICE_MIN)
    }
    pub(super) fn land_market_idle_list_fast_ticks() -> u16 {
        u16::try_from(u64::from(LAND_LIST_IDLE).saturating_mul(FAST_TICKS_PER_ECON_TICK))
            .unwrap_or(u16::MAX)
    }
    pub(super) fn land_market_finalize_rent_tick(&mut self) {
        if !self.land_market_active() {
            self.land_market_yield_this_tick.clear();
            return;
        }
        let nodes: Vec<NodeId> = self.land_plots.keys().copied().collect();
        for node in nodes {
            let qty = self.land_market_yield_this_tick.remove(&node).unwrap_or(0);
            let initial_price = self.land_market_initial_price(node);
            let state = self
                .land_market_plots
                .entry(node)
                .or_insert_with(|| LandMarketPlotState::new(initial_price));
            state.yield_history.push_back(LandYieldTick {
                tick: self.econ_tick,
                qty,
            });
            while state
                .yield_history
                .front()
                .is_some_and(|entry| entry.tick + LAND_RENT_WINDOW <= self.econ_tick)
            {
                state.yield_history.pop_front();
            }
        }
        self.land_market_yield_this_tick.clear();
    }
    pub(super) fn land_market_free_salt(&self, agent: AgentId) -> Gold {
        self.society.free_gold_after_all_reserves(agent)
    }
    pub(super) fn land_market_debit_fee(&mut self, agent: AgentId, amount: Gold) -> bool {
        if amount == Gold::ZERO {
            return true;
        }
        if self.land_market_free_salt(agent) < amount {
            return false;
        }
        if let Some(money_system) = self.society.money_system.as_mut() {
            if money_system.debit_specie(agent, amount).is_err() {
                return false;
            }
            money_system.reconcile_agent_cache(self.society.agents.as_mut_slice());
        } else if let Some(holder) = self.society.agents.get_mut(agent) {
            let Some(next) = holder.gold.checked_sub(amount) else {
                return false;
            };
            holder.gold = next;
        } else {
            return false;
        }
        self.land_fee_pool_salt = self.land_fee_pool_salt.saturating_add(amount);
        true
    }
    pub(super) fn land_market_charge_carrying_costs(&mut self) {
        if self.econ_tick == 0 || !self.econ_tick.is_multiple_of(LAND_CARRYING_PERIOD) {
            return;
        }
        let cost = Gold(
            self.chain
                .as_ref()
                .map_or(LAND_CARRYING_COST_DEFAULT, |chain| chain.land_carrying_cost),
        );
        if cost == Gold::ZERO {
            return;
        }
        let owners: Vec<(NodeId, AgentId)> = self
            .land_plots
            .iter()
            .filter_map(|(&node, record)| record.owner.map(|owner| (node, owner)))
            .collect();
        // Defense-in-depth (spec §3.2/§4): the carrying sweep is post-money-gated by
        // `run_land_market`'s early return, so this loop never runs pre-promotion in practice. If
        // that gate were ever removed, record each pre-promotion charge here (mirroring the trade
        // counter in `land_market_match`) so the `pre_money_forbidden` guard catches the regression
        // instead of asserting a tautology.
        let post_money = self.current_money_good() == Some(SALT);
        for (node, owner) in owners {
            if !post_money {
                self.land_market_pre_promotion_charges =
                    self.land_market_pre_promotion_charges.saturating_add(1);
            }
            if self.land_market_debit_fee(owner, cost) {
                self.land_market_carrying_paid_total =
                    self.land_market_carrying_paid_total.saturating_add(cost.0);
                let history = self.land_market_title_history.entry(owner).or_default();
                history.last_carrying_paid_tick = Some(self.econ_tick);
                if let Some(state) = self.land_market_plots.get_mut(&node) {
                    if state
                        .listing
                        .is_some_and(|listing| listing.kind == LandListingKind::Foreclosure)
                    {
                        state.listing = None;
                    }
                }
            } else {
                let current_price = self.land_market_listed_price(node);
                let ask = self
                    .land_market_plots
                    .get(&node)
                    .and_then(|state| state.listing)
                    .filter(|listing| listing.kind == LandListingKind::Foreclosure)
                    .map_or_else(
                        || self.land_market_discounted_ask(current_price),
                        |listing| {
                            let basis = listing.ask.min(current_price);
                            self.land_market_discounted_ask(basis)
                        },
                    );
                let state = self
                    .land_market_plots
                    .entry(node)
                    .or_insert_with(|| LandMarketPlotState::new(current_price));
                state.price = current_price;
                state.listing = Some(LandListing {
                    ask,
                    kind: LandListingKind::Foreclosure,
                });
                self.land_market_foreclosure_listings_total = self
                    .land_market_foreclosure_listings_total
                    .saturating_add(1);
            }
        }
    }
    pub(super) fn land_market_prepare_listings(&mut self) {
        let nodes: Vec<NodeId> = self.land_plots.keys().copied().collect();
        let idle_list_fast_ticks = Self::land_market_idle_list_fast_ticks();
        let cost = Gold(
            self.chain
                .as_ref()
                .map_or(LAND_CARRYING_COST_DEFAULT, |chain| chain.land_carrying_cost),
        );
        for node in nodes {
            let owner = self.land_plots.get(&node).and_then(|record| record.owner);
            let current_price = self.land_market_listed_price(node);
            self.land_market_plots
                .entry(node)
                .or_insert_with(|| LandMarketPlotState::new(current_price))
                .price = current_price;
            let Some(owner) = owner else {
                if let Some(state) = self.land_market_plots.get_mut(&node) {
                    state.listing = None;
                }
                continue;
            };
            if !self.private_land_live_agent(owner) {
                if let Some(state) = self.land_market_plots.get_mut(&node) {
                    state.listing = None;
                }
                continue;
            }
            let foreclosure_listing = self
                .land_market_plots
                .get(&node)
                .and_then(|state| state.listing)
                .is_some_and(|listing| listing.kind == LandListingKind::Foreclosure);
            if foreclosure_listing
                && cost > Gold::ZERO
                && self.land_market_free_salt(owner) >= cost
                && self.land_market_debit_fee(owner, cost)
            {
                self.land_market_carrying_paid_total =
                    self.land_market_carrying_paid_total.saturating_add(cost.0);
                let history = self.land_market_title_history.entry(owner).or_default();
                history.last_carrying_paid_tick = Some(self.econ_tick);
                if let Some(state) = self.land_market_plots.get_mut(&node) {
                    state.listing = None;
                }
            }
            let foreclosure_listing = self
                .land_market_plots
                .get(&node)
                .and_then(|state| state.listing)
                .is_some_and(|listing| listing.kind == LandListingKind::Foreclosure);
            let Some(state) = self.land_market_plots.get_mut(&node) else {
                continue;
            };
            if foreclosure_listing {
                continue;
            }
            let idle = self
                .land_plots
                .get(&node)
                .is_some_and(|record| record.idle_counter >= idle_list_fast_ticks);
            if idle {
                state.listing = Some(LandListing {
                    ask: current_price,
                    kind: LandListingKind::Idle,
                });
            } else if state
                .listing
                .is_some_and(|listing| listing.kind == LandListingKind::Idle)
            {
                state.listing = None;
            }
        }
    }
    pub(super) fn land_market_agent_owns_plot(&self, agent: AgentId) -> bool {
        self.land_plots
            .values()
            .any(|record| Self::private_land_record_agent_holds(record, agent))
    }
    pub(super) fn land_market_buyer_eligible(&self, slot: usize) -> bool {
        let colonist = &self.colonists[slot];
        if !colonist.alive || self.land_market_agent_owns_plot(colonist.id) {
            return false;
        }
        if !matches!(
            colonist.vocation,
            Vocation::Consumer | Vocation::Gatherer | Vocation::Unassigned
        ) {
            return false;
        }
        if self.world.agent_status(colonist.id).is_none() {
            return false;
        }
        // Eligibility is strictly agent-local "cultivating-or-attempting" (spec §3.4): a buyer must
        // be working its own plot, have cultivation stock in flight, be under cultivation pressure,
        // or be hungry enough to enter. It must NOT be admitted merely because some other plot is
        // listed — a global-listing clause would let any passive Consumer/Gatherer with spare SALT
        // bid, decoupling the buyer pool / priced-out trace from genuine re-entry pressure.
        let attempting = colonist.cultivating
            || colonist.cultivation_stock_pending
            || colonist.cultivate_pressure > 0
            || self
                .chain
                .as_ref()
                .is_some_and(|chain| colonist.need.hunger >= chain.cultivate_hunger_in);
        if !attempting {
            return false;
        }
        self.land_market_free_salt(colonist.id) > Gold::ZERO
            || self
                .chain
                .as_ref()
                .is_some_and(|chain| chain.land_price_cap_factor == 0)
    }
    pub(super) fn private_land_has_comparable_or_better_stayer(
        &self,
        agent: AgentId,
        priced_out_node: NodeId,
        quality: LandPlotQuality,
    ) -> Vec<AgentId> {
        self.land_plots
            .iter()
            .flat_map(|(&node, record)| {
                if node == priced_out_node {
                    return Vec::new();
                }
                let owners = Self::private_land_record_holders(record);
                // For the hysteresis trace, "comparable-or-better" is productive land quality.
                // Distance already enters the plot's rent/price and the buyer's nearest-plot
                // selection; requiring another plot to be at least as close would make the best
                // located good plot incomparable by construction and erase real re-buy pressure.
                let comparable = self
                    .private_land_plot_quality(node)
                    .is_some_and(|other| other.regen >= quality.regen && other.cap >= quality.cap);
                if !comparable {
                    return Vec::new();
                }
                owners
                    .into_iter()
                    .filter(|&owner| owner != agent && self.private_land_live_agent(owner))
                    .collect::<Vec<_>>()
            })
            .collect()
    }
    /// Records one budget-hysteresis priced-out event for `agent` against `node`, returning whether
    /// it counted. It counts only when a live, non-self stayer retains land comparable-or-better than
    /// `node` (§3.6), so feeding it an incidental marginal miss with no comparable stayer is a no-op.
    pub(super) fn land_market_record_priced_out(&mut self, agent: AgentId, node: NodeId) -> bool {
        let Some(quality) = self.private_land_plot_quality(node) else {
            return false;
        };
        let stayers = self.private_land_has_comparable_or_better_stayer(agent, node, quality);
        if stayers.is_empty() {
            return false;
        }
        self.land_market_priced_out_total = self.land_market_priced_out_total.saturating_add(1);
        if self
            .land_market_title_history
            .get(&agent)
            .is_some_and(|history| history.ever_sold)
        {
            self.land_market_lapsed_priced_out_total =
                self.land_market_lapsed_priced_out_total.saturating_add(1);
        }
        for stayer in stayers {
            let history = self.land_market_title_history.entry(stayer).or_default();
            history.retained_through_priced_out = true;
        }
        true
    }
    pub(super) fn land_market_match(&mut self) {
        #[derive(Clone, Copy)]
        struct LandBid {
            buyer: AgentId,
            node: NodeId,
            bid: u64,
            reservation: u64,
            salt: u64,
        }

        let mut asks: Vec<(u64, NodeId, AgentId, LandListingKind)> = self
            .land_plots
            .iter()
            .filter_map(|(&node, record)| {
                let owner = record.owner?;
                let listing = self.land_market_plots.get(&node)?.listing?;
                Some((listing.ask, node, owner, listing.kind))
            })
            .collect();
        asks.sort_by_key(|&(ask, node, _, _)| (ask, node.0));
        if asks.is_empty() {
            return;
        }

        let listed: Vec<(NodeId, u64)> =
            asks.iter().map(|&(ask, node, _, _)| (node, ask)).collect();
        let mut bids = Vec::new();
        let live = self.live_colonist_slots.clone();
        for slot in live {
            if !self.land_market_buyer_eligible(slot) {
                continue;
            }
            let buyer = self.colonists[slot].id;
            let salt = self.land_market_free_salt(buyer).0;
            let mut affordable = Vec::new();
            let mut priced_out_candidates = Vec::new();
            for &(node, ask) in &listed {
                let reservation = self.land_market_base_price(node);
                if reservation < ask {
                    continue;
                }
                let distance = self
                    .private_land_agent_distance(buyer, node)
                    .unwrap_or(u32::MAX);
                let candidate = ((distance, ask, node.0), node, ask, reservation);
                if salt >= ask {
                    affordable.push(candidate);
                } else {
                    priced_out_candidates.push(candidate);
                }
            }
            affordable.sort_by_key(|(key, _, _, _)| *key);
            priced_out_candidates.sort_by_key(|(key, _, _, _)| *key);

            // §3.6 budget-hysteresis trace, decoupled from the bid: a buyer that can afford a cheap
            // marginal listing still bids on it, but if it is ALSO budget-outbid on a comparable-or-
            // better listed plot (its fundamental reservation clears the ask, its SALT on hand does
            // not), that is exactly the "can only re-buy worse land" hysteresis the metric must
            // capture — and recording must NOT be gated on which plot it ends up bidding on. Rank the
            // out-of-budget listings best-land-first so a marginal miss never masks a comparable one;
            // `record_priced_out` itself requires a live stayer holding comparable land.
            let mut priced_out_ranked: Vec<NodeId> = priced_out_candidates
                .iter()
                .map(|&(_, node, _, _)| node)
                .collect();
            priced_out_ranked.sort_by_key(|&node| {
                let quality = self.private_land_plot_quality(node);
                let regen = quality.map_or(0, |q| q.regen);
                let cap = quality.map_or(0, |q| q.cap);
                let distance = self
                    .private_land_agent_distance(buyer, node)
                    .unwrap_or(u32::MAX);
                (
                    std::cmp::Reverse(regen),
                    std::cmp::Reverse(cap),
                    distance,
                    node.0,
                )
            });
            for node in priced_out_ranked {
                if self.land_market_record_priced_out(buyer, node) {
                    break;
                }
            }

            let selected = affordable
                .first()
                .copied()
                .or_else(|| priced_out_candidates.first().copied());
            let Some((_, node, ask, reservation)) = selected else {
                continue;
            };
            let bid = salt.min(reservation);
            if bid < ask {
                self.land_market_ask_bid_gap_sum = self
                    .land_market_ask_bid_gap_sum
                    .saturating_add(ask.saturating_sub(bid));
                self.land_market_ask_bid_gap_count =
                    self.land_market_ask_bid_gap_count.saturating_add(1);
            }
            bids.push(LandBid {
                buyer,
                node,
                bid,
                reservation,
                salt,
            });
        }

        let mut bids_by_node: BTreeMap<NodeId, Vec<LandBid>> = BTreeMap::new();
        for bid in bids {
            bids_by_node.entry(bid.node).or_default().push(bid);
        }
        for bids in bids_by_node.values_mut() {
            bids.sort_by(|a, b| b.bid.cmp(&a.bid).then(a.buyer.0.cmp(&b.buyer.0)));
        }

        let mut sold_buyers = BTreeSet::new();
        let mut sold_nodes = BTreeSet::new();
        for (ask, node, seller, kind) in asks {
            if sold_nodes.contains(&node) {
                continue;
            }
            let Some(candidates) = bids_by_node.get(&node) else {
                continue;
            };
            let Some(bid) = candidates
                .iter()
                .find(|bid| !sold_buyers.contains(&bid.buyer) && bid.bid >= ask)
                .copied()
            else {
                continue;
            };
            if bid.reservation < ask || bid.salt < ask {
                continue;
            }
            let moved = ask == 0 || self.move_money_conserved(bid.buyer, seller, Gold(ask));
            if !moved {
                continue;
            }
            if let Some(record) = self.land_plots.get_mut(&node) {
                record.owner = Some(bid.buyer);
                record.idle_counter = 0;
                record.reserved_for = None;
            }
            let rent = self.land_market_rent_basis(node);
            if let Some(state) = self.land_market_plots.get_mut(&node) {
                state.price = ask;
                state.listing = None;
                state.last_sale_price = Some(ask);
                state.last_sale_tick = Some(self.econ_tick);
            }
            // `land_lapsed_losses` is the S23a idle-forfeiture reclaim ledger; it is read only on the
            // homestead-claim path, which is closed post-promotion under the land market, so a market
            // seller is never "lapsed" in that sense — do not write it here.
            let quality = self.private_land_plot_quality(node);
            {
                let history = self.land_market_title_history.entry(seller).or_default();
                history.ever_owned = true;
                history.ever_sold = true;
                history.current = None;
                if kind == LandListingKind::Foreclosure {
                    history.foreclosed_out = true;
                }
            }
            {
                let history = self.land_market_title_history.entry(bid.buyer).or_default();
                history.ever_owned = true;
                history.ever_bought = true;
                history.current = Some(LandTitleSource::Bought);
            }
            let good_plot = quality.is_some_and(|q| q.regen >= LAND_GOOD_REGEN);
            self.land_market_sales.push(LandSaleRecord {
                tick: self.econ_tick,
                node,
                buyer: bid.buyer,
                seller,
                price: ask,
                rent,
                good_plot,
                foreclosure: kind == LandListingKind::Foreclosure,
            });
            self.land_market_trade_count = self.land_market_trade_count.saturating_add(1);
            if self.current_money_good() != Some(SALT) {
                self.land_market_pre_promotion_trade_count =
                    self.land_market_pre_promotion_trade_count.saturating_add(1);
            }
            sold_buyers.insert(bid.buyer);
            sold_nodes.insert(node);
        }
    }
    pub(super) fn run_land_market(&mut self) {
        if !self.land_market_active() {
            return;
        }
        if self.current_money_good() != Some(SALT) {
            return;
        }
        self.land_market_charge_carrying_costs();
        self.land_market_prepare_listings();
        self.land_market_match();
        debug_assert!(
            self.private_land_registry_invariant_holds(),
            "land-market sweep must preserve the finite plot registry"
        );
    }
    pub(super) fn private_land_heir_eligible(&self, heir: AgentId) -> bool {
        let Some(slot) = self.slot_for_id(heir) else {
            return false;
        };
        let colonist = &self.colonists[slot];
        colonist.alive
            && self.world.agent_status(heir).is_some()
            && matches!(
                colonist.vocation,
                Vocation::Consumer | Vocation::Gatherer | Vocation::Unassigned
            )
    }
    pub(super) fn secure_land_live_agent(&self, agent: AgentId) -> bool {
        self.slot_for_id(agent)
            .is_some_and(|slot| self.colonists[slot].alive)
            && self.society.agents.get(agent).is_some()
    }
    pub(super) fn secure_land_live_children(&self, owner: AgentId) -> Vec<AgentId> {
        let household = self.colonist_household(owner);
        let mut children: Vec<(std::cmp::Reverse<u64>, AgentId)> = self
            .live_colonist_slots
            .iter()
            .filter_map(|&slot| {
                let colonist = &self.colonists[slot];
                (colonist.parent == Some(owner)
                    && colonist.household == household
                    && self.secure_land_live_agent(colonist.id))
                .then_some((std::cmp::Reverse(colonist.age), colonist.id))
            })
            .collect();
        children.sort_by_key(|&(age, id)| (age, id.0));
        children.into_iter().map(|(_, id)| id).collect()
    }
    pub(super) fn secure_land_same_household_kin(&self, owner: AgentId) -> Option<AgentId> {
        let household = self.colonist_household(owner)?;
        let mut kin: Vec<(std::cmp::Reverse<u64>, AgentId)> = self
            .live_colonist_slots
            .iter()
            .filter_map(|&slot| {
                let colonist = &self.colonists[slot];
                (colonist.id != owner
                    && colonist.household == Some(household)
                    && self.secure_land_live_agent(colonist.id))
                .then_some((std::cmp::Reverse(colonist.age), colonist.id))
            })
            .collect();
        kin.sort_by_key(|&(age, id)| (age, id.0));
        kin.into_iter().map(|(_, id)| id).next()
    }
    pub(super) fn private_land_agent_holds_any_plot(&self, agent: AgentId) -> bool {
        self.land_plots
            .values()
            .any(|record| Self::private_land_record_agent_holds(record, agent))
    }
    pub(super) fn secure_land_household_distance(&self, owner: AgentId, candidate: AgentId) -> u32 {
        match (
            self.colonist_household(owner),
            self.colonist_household(candidate),
        ) {
            (Some(a), Some(b)) => u32::try_from(a.abs_diff(b)).unwrap_or(u32::MAX),
            (None, None) => 0,
            _ => u32::MAX / 2,
        }
    }
    pub(super) fn secure_land_spatial_distance(&self, owner: AgentId, candidate: AgentId) -> u32 {
        let Some(candidate_pos) = self.world.agent_pos(candidate) else {
            return u32::MAX;
        };
        let owner_pos = self.world.agent_pos(owner).or_else(|| {
            self.world
                .stockpile(self.exchange)
                .map(|stockpile| stockpile.pos)
        });
        owner_pos.map_or(u32::MAX, |pos| self.world.grid_distance(pos, candidate_pos))
    }
    pub(super) fn secure_land_colony_next_of_kin(&self, owner: AgentId) -> Option<AgentId> {
        self.live_colonist_slots
            .iter()
            .filter_map(|&slot| {
                let candidate = self.colonists[slot].id;
                (candidate != owner
                    && self.secure_land_live_agent(candidate)
                    && (!self.mortal_landowner_demography_active()
                        || self.mortal_landowner_reproductive_actor(candidate))
                    && !self.private_land_agent_holds_any_plot(candidate))
                .then_some((
                    self.secure_land_household_distance(owner, candidate),
                    self.secure_land_spatial_distance(owner, candidate),
                    candidate.0,
                    candidate,
                ))
            })
            .min_by_key(|&(household, spatial, id, _)| (household, spatial, id))
            .map(|(_, _, _, candidate)| candidate)
    }
    pub(super) fn secure_land_universal_heir_for(&self, owner: AgentId) -> Option<AgentId> {
        // Branch order: live children → same-household kin → household heir → colony next-of-kin.
        // The first three branches are household-scoped *by construction* (`live_children`,
        // `same_household_kin`, and `heir_for` all filter to the owner's own household), so under
        // the S23d flag they can only ever return a fellow lineage-household member — never the
        // immortal roster. Only `colony_next_of_kin` ranks candidates *across* households and can
        // reach a non-lineage/immortal agent, which is why the mortal-landowner reproductive-actor
        // gate lives there alone (see `secure_land_colony_next_of_kin`). Do not "symmetrise" by
        // adding the gate to branches 1–3 (redundant) or by dropping it from branch 4 (a leak).
        self.secure_land_live_children(owner)
            .into_iter()
            .next()
            .or_else(|| self.secure_land_same_household_kin(owner))
            .or_else(|| {
                self.heir_for(owner)
                    .filter(|&heir| self.secure_land_live_agent(heir))
            })
            .or_else(|| self.secure_land_colony_next_of_kin(owner))
    }
    pub(super) fn secure_land_partible_coheirs_for(&self, owner: AgentId) -> Vec<AgentId> {
        let children = self.secure_land_live_children(owner);
        if !children.is_empty() {
            return children;
        }
        self.secure_land_universal_heir_for(owner)
            .into_iter()
            .collect()
    }
    pub(super) fn secure_land_log_inheritance(
        &mut self,
        deceased: AgentId,
        heir: Option<AgentId>,
        plot: NodeId,
        regime: InheritanceRegime,
        pre: (u32, u32),
        post: (u32, u32),
    ) {
        self.secure_land_inheritance_events
            .push(SecureLandInheritanceRow {
                tick: self.econ_tick,
                deceased: deceased.0,
                heir: heir.map(|id| id.0),
                plot: plot.0,
                regime,
                pre_regen: pre.0,
                pre_cap: pre.1,
                post_regen: post.0,
                post_cap: post.1,
            });
    }
    pub(super) fn secure_land_split_effective_capacity(
        regen: u32,
        cap: u32,
        available: u32,
        heirs: &[AgentId],
    ) -> Vec<(AgentId, LandShare)> {
        if heirs.is_empty() {
            return Vec::new();
        }
        let n = u32::try_from(heirs.len()).unwrap_or(u32::MAX).max(1);
        let regen_base = regen / n;
        let regen_extra = regen % n;
        let cap_base = cap / n;
        let cap_extra = cap % n;
        // Split the deceased holder's REMAINING harvest availability alongside the
        // capacity, so a depleted share is inherited depleted rather than silently refilled
        // to a fresh full cap. Resetting `available` to `share_cap` would let a co-heir
        // re-harvest a share the deceased had already spent this regen cycle, drawing node
        // stock that should stay for the surviving co-owners. Clamped to `cap` so each
        // co-heir's `available <= cap` invariant holds.
        let available = available.min(cap);
        let avail_base = available / n;
        let avail_extra = available % n;
        heirs
            .iter()
            .enumerate()
            .map(|(idx, &heir)| {
                let idx = u32::try_from(idx).unwrap_or(u32::MAX);
                let share_regen = regen_base + u32::from(idx < regen_extra);
                let share_cap = cap_base + u32::from(idx < cap_extra);
                let share_available = avail_base + u32::from(idx < avail_extra);
                (
                    heir,
                    LandShare {
                        regen: share_regen,
                        cap: share_cap,
                        available: share_available,
                    },
                )
            })
            .collect()
    }
    pub(super) fn secure_land_add_partible_share(
        record: &mut LandPlotRecord,
        heir: AgentId,
        share: LandShare,
    ) {
        let entry = record.shares.entry(heir).or_default();
        entry.regen = entry.regen.saturating_add(share.regen);
        entry.cap = entry.cap.saturating_add(share.cap);
        entry.available = entry
            .available
            .saturating_add(share.available)
            .min(entry.cap);
    }
    /// S23a: whether private land tenure is active this tick. It composes strictly on S22a
    /// endogenous cultivation entry, so flag-only misconfigurations on older substrates are inert.
    pub(super) fn private_land_tenure_active(&self) -> bool {
        self.chain
            .as_ref()
            .is_some_and(chain_runtime_private_land_tenure_active)
    }
    pub(super) fn secure_land_tenure_active(&self) -> bool {
        self.chain
            .as_ref()
            .is_some_and(chain_runtime_secure_land_tenure_active)
    }
    pub(super) fn private_land_harvest_gate_active(&self) -> bool {
        self.secure_land_tenure_active()
            || self.chain.as_ref().is_some_and(|chain| chain.harvest_gate)
    }
    pub(super) fn private_land_forfeit_on_idle_active(&self) -> bool {
        self.chain.as_ref().is_some_and(|chain| {
            chain.forfeit_on_idle && !chain_runtime_secure_land_tenure_active(chain)
        }) && !self.land_market_active()
    }
    /// S23b: whether the post-money land-market institution is active as a behavior surface. The
    /// flag composes strictly on private land tenure; a flag-only config on an older substrate is
    /// inert and omitted from the digest.
    pub(super) fn land_market_active(&self) -> bool {
        self.chain
            .as_ref()
            .is_some_and(chain_runtime_land_market_active)
    }
    pub fn private_land_plot_count(&self) -> usize {
        self.land_plots.len()
    }
    pub fn private_land_grid_width(&self) -> u16 {
        self.world.grid().width()
    }
    pub fn private_land_claims_total(&self) -> u64 {
        self.land_claims_total
    }
    pub fn private_land_idle_losses_total(&self) -> u64 {
        self.land_idle_losses_total
    }
    pub fn private_land_harvest_denials_total(&self) -> u64 {
        self.land_harvest_denials_total
    }
    /// Reroutes off a plot already HELD by another (vs stampede losers on unowned land). ~Always 0
    /// by design (see the field doc): contention is resolved while plots are still unowned. Kept as
    /// a diagnostic; [`Self::private_land_nonowner_harvest_of_owned_total`] is the gate proof.
    pub fn private_land_owner_gate_denials_total(&self) -> u64 {
        self.land_owner_gate_denials_total
    }
    /// Harvests of a HELD plot by a non-owner. Zero under the headline (owner-exclusive harvest
    /// holds) and positive only under the `non_excludable_deed` control. The §4 non-vacuity gate
    /// reads this pair to prove ownership actually gates harvest rather than merely recording title.
    pub fn private_land_nonowner_harvest_of_owned_total(&self) -> u64 {
        self.land_nonowner_harvest_of_owned_total
    }
    pub fn private_land_reclaims_by_other_total(&self) -> u64 {
        self.land_reclaims_by_other_total
    }
    pub fn private_land_marginal_nonowner_claims_total(&self) -> u64 {
        self.land_marginal_nonowner_claims_total
    }
    pub fn private_land_lapsed_reentry_worse_total(&self) -> u64 {
        self.land_lapsed_reentry_worse_total
    }
    pub fn private_land_owner_ids(&self) -> Vec<u64> {
        self.land_plots
            .values()
            .flat_map(Self::private_land_record_holders)
            .map(|owner| owner.0)
            .collect::<BTreeSet<_>>()
            .into_iter()
            .collect()
    }
    pub fn secure_land_owner_old_age_deaths_total(&self) -> u64 {
        self.secure_land_owner_old_age_deaths_total
    }
    pub fn secure_land_inherit_eligible_owner_deaths_total(&self) -> u64 {
        self.secure_land_inherit_eligible_owner_deaths_total
    }
    pub fn private_land_owner_identity_rows(&self) -> Vec<MortalLandownerOwnerRow> {
        self.land_plots
            .values()
            .flat_map(Self::private_land_record_holders)
            .collect::<BTreeSet<_>>()
            .into_iter()
            .map(|owner| {
                let slot = self.colonist_slot_by_id.get(&owner).copied();
                let colonist = slot.and_then(|slot| self.colonists.get(slot));
                let household = colonist.and_then(|c| c.household);
                let lifespan = colonist.and_then(|c| c.lifespan);
                let reproduction_eligible = self.mortal_landowner_reproductive_actor(owner);
                MortalLandownerOwnerRow {
                    owner: owner.0,
                    lifespan,
                    household,
                    lineage_id: household,
                    reproduction_eligible,
                    in_birth_kinship_graph: self.demography.is_some() && household.is_some(),
                    born_in_sim: colonist.is_some_and(|c| c.parent.is_some()),
                }
            })
            .collect()
    }
    pub fn private_land_viable_marginal_plots(&self) -> usize {
        self.land_plots
            .iter()
            .filter(|&(&node, record)| {
                Self::private_land_record_claimable(record)
                    && self.private_land_viable_marginal_node(node)
            })
            .count()
    }
    pub fn private_land_plot_summaries(&self) -> Vec<(u32, Option<u64>, u16, u32, u32, u32)> {
        self.land_plots
            .iter()
            .filter_map(|(&node, record)| {
                let quality = self.private_land_plot_quality(node)?;
                Some((
                    node.0,
                    Self::private_land_record_primary_holder(record).map(|owner| owner.0),
                    record.idle_counter,
                    quality.regen,
                    quality.cap,
                    quality.distance,
                ))
            })
            .collect()
    }
    pub fn secure_land_tenure_on(&self) -> bool {
        self.secure_land_tenure_active()
    }
    pub fn secure_land_inheritance_regime(&self) -> Option<InheritanceRegime> {
        self.chain
            .as_ref()
            .filter(|chain| chain_runtime_secure_land_tenure_active(chain))
            .map(|chain| chain.inheritance_regime)
    }
    pub fn secure_land_inheritance_events(&self) -> Vec<SecureLandInheritanceRow> {
        self.secure_land_inheritance_events.clone()
    }
    pub fn secure_land_stranded_shares_total(&self) -> u64 {
        self.secure_land_stranded_shares_total
    }
    pub fn private_land_share_summaries(&self) -> Vec<SecureLandShareRow> {
        self.land_plots
            .iter()
            .flat_map(|(&node, record)| {
                record.shares.iter().map(move |(&owner, share)| {
                    (node.0, owner.0, share.regen, share.cap, share.available)
                })
            })
            .collect()
    }
    pub fn private_land_stranded_capacity_summaries(&self) -> Vec<(u32, u32, u32)> {
        self.land_plots
            .iter()
            .filter_map(|(&node, record)| {
                (record.stranded_regen > 0 || record.stranded_cap > 0).then_some((
                    node.0,
                    record.stranded_regen,
                    record.stranded_cap,
                ))
            })
            .collect()
    }
    pub fn private_land_plot_harvest_totals(&self) -> Vec<(u32, u64)> {
        self.land_plot_harvest_totals
            .iter()
            .map(|(&node, &qty)| (node.0, qty))
            .collect()
    }
    pub fn private_land_effective_capacity_by_owner(&self) -> Vec<(u64, u32, u32)> {
        let mut totals: BTreeMap<AgentId, (u32, u32)> = BTreeMap::new();
        for (&node, record) in &self.land_plots {
            for owner in Self::private_land_record_holders(record) {
                if let Some((regen, cap)) = self.private_land_share_capacity(record, node, owner) {
                    let entry = totals.entry(owner).or_insert((0, 0));
                    entry.0 = entry.0.saturating_add(regen);
                    entry.1 = entry.1.saturating_add(cap);
                }
            }
        }
        totals
            .into_iter()
            .map(|(owner, (regen, cap))| (owner.0, regen, cap))
            .collect()
    }
    pub fn private_land_owner_grain_share_bps(&self) -> u64 {
        let owners: BTreeSet<AgentId> = self
            .land_plots
            .values()
            .flat_map(Self::private_land_record_holders)
            .collect();
        let total: u64 = self.cultivation_grain_harvested.values().copied().sum();
        if total == 0 {
            return 0;
        }
        let owner_total: u64 = self
            .cultivation_grain_harvested
            .iter()
            .filter(|(agent, _)| owners.contains(agent))
            .map(|(_, qty)| *qty)
            .sum();
        owner_total.saturating_mul(10_000) / total
    }
    pub fn land_market_trades_total(&self) -> u64 {
        self.land_market_trade_count
    }
    pub fn land_market_pre_promotion_trades_total(&self) -> u64 {
        self.land_market_pre_promotion_trade_count
    }
    pub fn land_market_pre_promotion_charges_total(&self) -> u64 {
        self.land_market_pre_promotion_charges
    }
    pub fn land_market_fee_pool_salt(&self) -> u64 {
        self.land_fee_pool_salt.0
    }
    pub fn land_market_carrying_paid_total(&self) -> u64 {
        self.land_market_carrying_paid_total
    }
    pub fn land_market_foreclosure_listings_total(&self) -> u64 {
        self.land_market_foreclosure_listings_total
    }
    pub fn land_market_priced_out_total(&self) -> u64 {
        self.land_market_priced_out_total
    }
    pub fn land_market_lapsed_priced_out_total(&self) -> u64 {
        self.land_market_lapsed_priced_out_total
    }
    pub fn land_market_ask_bid_gap_mean(&self) -> Option<u64> {
        (self.land_market_ask_bid_gap_count > 0).then(|| {
            (self.land_market_ask_bid_gap_sum + self.land_market_ask_bid_gap_count / 2)
                / self.land_market_ask_bid_gap_count
        })
    }
    pub fn land_market_sale_rows(&self) -> Vec<LandMarketSaleRow> {
        self.land_market_sales
            .iter()
            .map(|sale| {
                (
                    sale.tick,
                    sale.node.0,
                    sale.buyer.0,
                    sale.seller.0,
                    sale.price,
                    sale.rent,
                    sale.good_plot,
                    sale.foreclosure,
                )
            })
            .collect()
    }
    pub fn land_market_affordable_listed_plots_for_nonowners(&self) -> usize {
        if !self.land_market_active() || self.current_money_good() != Some(SALT) {
            return 0;
        }
        let listed: Vec<(NodeId, u64)> = self
            .land_market_plots
            .iter()
            .filter_map(|(&node, state)| state.listing.map(|listing| (node, listing.ask)))
            .collect();
        if listed.is_empty() {
            return 0;
        }
        let mut affordable = BTreeSet::new();
        for &slot in &self.live_colonist_slots {
            if !self.land_market_buyer_eligible(slot) {
                continue;
            }
            let buyer = self.colonists[slot].id;
            let salt = self.land_market_free_salt(buyer).0;
            for &(node, ask) in &listed {
                if salt >= ask && self.land_market_base_price(node) >= ask {
                    affordable.insert(node);
                }
            }
        }
        affordable.len()
    }
    pub fn land_market_title_share_counts(&self) -> (usize, usize, usize, usize) {
        let mut original = 0usize;
        let mut inherited = 0usize;
        let mut bought = 0usize;
        for record in self.land_plots.values() {
            let Some(owner) = record.owner else {
                continue;
            };
            match self
                .land_market_title_history
                .get(&owner)
                .and_then(|history| history.current)
            {
                Some(LandTitleSource::OriginalClaim) => original += 1,
                Some(LandTitleSource::Inherited) => inherited += 1,
                Some(LandTitleSource::Bought) => bought += 1,
                None => {}
            }
        }
        let foreclosed = self
            .land_market_title_history
            .values()
            .filter(|history| history.foreclosed_out)
            .count();
        (original, inherited, bought, foreclosed)
    }
    pub fn land_market_agent_market_stabilized(&self, id: u64, final_start: u64) -> bool {
        let id = AgentId(id);
        self.land_market_title_history
            .get(&id)
            .is_some_and(|history| {
                history.ever_bought
                    || history.retained_through_priced_out
                    || history
                        .last_carrying_paid_tick
                        .is_some_and(|tick| tick >= final_start)
            })
    }
    pub fn private_land_registry_invariant_holds(&self) -> bool {
        if !self.private_land_tenure_active() {
            return true;
        }
        let Some(grain) = self.chain.as_ref().map(|chain| chain.content.grain()) else {
            return false;
        };
        if self.land_market_active() {
            if self.land_market_plots.len() != self.land_plots.len() {
                return false;
            }
            if !self
                .land_market_plots
                .keys()
                .all(|node| self.land_plots.contains_key(node))
            {
                return false;
            }
        }
        if self.in_kind_wage_active()
            && self
                .in_kind_contracts
                .iter()
                .any(|contract| self.share_contract_for_node(contract.node).is_some())
        {
            return false;
        }
        for (&node, record) in &self.land_plots {
            if !self.world.node(node).is_some_and(|plot| plot.good == grain) {
                return false;
            }
            if record.owner.is_some() && !record.shares.is_empty() {
                return false;
            }
            if record
                .owner
                .is_some_and(|owner| !self.private_land_live_agent(owner))
            {
                return false;
            }
            if record
                .shares
                .keys()
                .any(|&owner| !self.private_land_live_agent(owner))
            {
                return false;
            }
            if record
                .reserved_for
                .is_some_and(|owner| !self.private_land_live_agent(owner))
            {
                return false;
            }
            if (!record.shares.is_empty() || record.stranded_regen > 0 || record.stranded_cap > 0)
                && !self.private_land_plot_quality(node).is_some_and(|quality| {
                    let share_regen: u32 = record
                        .shares
                        .values()
                        .map(|share| share.regen)
                        .sum::<u32>()
                        .saturating_add(record.stranded_regen);
                    let share_cap: u32 = record
                        .shares
                        .values()
                        .map(|share| share.cap)
                        .sum::<u32>()
                        .saturating_add(record.stranded_cap);
                    let available_ok = record
                        .shares
                        .values()
                        .all(|share| share.available <= share.cap);
                    available_ok && share_regen == quality.regen && share_cap == quality.cap
                })
            {
                return false;
            }
            if self.land_market_active()
                && self
                    .land_market_plots
                    .get(&node)
                    .and_then(|state| state.listing)
                    .is_some()
                && record.owner.is_none()
            {
                return false;
            }
        }
        let harvest_gate = self.private_land_harvest_gate_active();
        let mut unowned_target_counts: BTreeMap<NodeId, usize> = BTreeMap::new();
        for &slot in &self.live_colonist_slots {
            let colonist = &self.colonists[slot];
            if let Some(source) = colonist.carried_grain_source {
                if !self.land_plots.contains_key(&source) {
                    return false;
                }
                let carry = self.world.agent_carry(colonist.id, grain);
                let pending = self
                    .pending_deposits
                    .get(&(colonist.id, grain))
                    .copied()
                    .unwrap_or(0);
                if carry == 0 && pending == 0 {
                    return false;
                }
            } else if colonist.carried_share_contract_id.is_some() {
                return false;
            }
            if let Some(task) = self.world.agent_task(colonist.id) {
                if let Some(node) = Self::private_land_harvest_task_node(task) {
                    if harvest_gate
                        && self
                            .land_plots
                            .get(&node)
                            .is_some_and(Self::private_land_record_claimable)
                    {
                        *unowned_target_counts.entry(node).or_insert(0) += 1;
                    }
                }
            }
        }
        unowned_target_counts.values().all(|&count| count <= 1)
    }
}

pub(super) fn private_land_marginal_start_x(good_plots: u16) -> u32 {
    let after_good =
        u32::from(LAND_GOOD_START_X) + u32::from(good_plots) + LAND_GOOD_TO_MARGINAL_GAP;
    LAND_MARGINAL_START_X.max(after_good)
}
pub(super) fn private_land_marginal_x(good_plots: u16, marginal_index: u16) -> Option<u16> {
    let x = private_land_marginal_start_x(good_plots)
        .checked_add(LAND_MARGINAL_SPACING.checked_mul(u32::from(marginal_index))?)?;
    u16::try_from(x).ok()
}
pub(super) fn private_land_layout_width(good_plots: u16, marginal_plots: u16) -> Option<u16> {
    let last_good_x = if good_plots > 0 {
        u32::from(LAND_GOOD_START_X) + u32::from(good_plots) - 1
    } else {
        0
    };
    let last_marginal_x = if marginal_plots > 0 {
        u32::from(private_land_marginal_x(good_plots, marginal_plots - 1)?)
    } else {
        0
    };
    let farthest_plot_x = last_good_x.max(last_marginal_x);
    let width = u32::from(LAND_LAYOUT_MIN_WIDTH).max(
        farthest_plot_x
            .checked_add(LAND_LAYOUT_MARGIN)?
            .checked_add(1)?,
    );
    u16::try_from(width).ok()
}
