//! Commitment-norm machinery.
//!
//! The voluntary-cultivation commitment norm and its spread/imitation dynamics —
//! seeding, group scoring, adoption/abandonment observation, and the purity guards.
//! Extracted verbatim from `mod.rs` (pure code motion): the `impl Settlement` methods
//! move into the block below; the seed-time free functions become `pub(super)` and are
//! re-imported by the parent via `use commitment_norm::*` so all call sites are unchanged.

use super::*;

impl Settlement {
    pub(super) fn commitment_norm_seed_anchor_pos(
        &self,
        colonist: &Colonist,
        exchange_pos: Pos,
    ) -> Pos {
        colonist
            .home_node
            .or(colonist.node)
            .and_then(|node| self.world.node(node).map(|node| node.pos))
            .or_else(|| self.world.agent_pos(colonist.id))
            .unwrap_or(exchange_pos)
    }
    pub(super) fn commitment_norm_spread_active(&self) -> bool {
        self.chain
            .as_ref()
            .is_some_and(chain_runtime_commitment_norm_spread_active)
    }
    pub(super) fn fixed_commitment_norm_prevalence(&self) -> Option<f64> {
        self.chain
            .as_ref()
            .and_then(|chain| chain.fixed_commitment_norm_prevalence)
            .map(|prevalence| prevalence.clamp(0.0, 1.0))
    }
    pub(super) fn fixed_commitment_norm_active(&self) -> bool {
        self.chain.as_ref().is_some_and(|chain| {
            chain.fixed_commitment_norm_prevalence.is_some()
                && chain_runtime_voluntary_cultivation_commitment_active(chain)
        })
    }
    pub(super) fn commitment_norm_gate_active(&self) -> bool {
        self.commitment_norm_spread_active() || self.fixed_commitment_norm_active()
    }
    pub(super) fn commitment_norm_observation_set(
        &self,
        slot: usize,
        live: &[usize],
        exchange_pos: Option<Pos>,
        _window: u64,
        radius: u16,
        max_models: u16,
    ) -> Vec<usize> {
        let own = &self.colonists[slot];
        let own_pos = self.world.agent_pos(own.id);
        let mut models: Vec<(u32, u64, usize)> = Vec::new();
        for &model_slot in live {
            if model_slot == slot {
                continue;
            }
            let model = &self.colonists[model_slot];
            if !model.alive {
                continue;
            }
            let model_pos = self.world.agent_pos(model.id);
            let manhattan = match (own_pos, model_pos) {
                (Some(a), Some(b)) => a.manhattan(b),
                _ => u32::MAX,
            };
            let within_radius = manhattan <= u32::from(radius);
            let market_link =
                exchange_pos.is_some_and(|pos| own_pos == Some(pos) && model_pos == Some(pos));
            if within_radius || market_link {
                models.push((manhattan, model.id.0, model_slot));
            }
        }
        models.sort_unstable_by_key(|&(distance, id, _)| (distance, id));
        models.truncate(usize::from(max_models));
        models.into_iter().map(|(_, _, slot)| slot).collect()
    }
    #[allow(clippy::too_many_arguments)]
    pub(super) fn commitment_norm_group_candidate(
        &self,
        center_slot: usize,
        live: &[usize],
        window: u64,
        food_target: u64,
        include_salt: bool,
        radius: u16,
        exchange_pos: Pos,
    ) -> Option<CommitmentNormGroupCandidate> {
        let center = self.colonists.get(center_slot)?;
        if !center.alive {
            return None;
        }
        let members = self.commitment_norm_group_members(center_slot, live, radius, exchange_pos);
        if members.len() < COMMITMENT_NORM_GROUP_MIN_SIZE {
            return None;
        }
        let score =
            self.commitment_norm_group_score(&members, window, food_target, include_salt)?;
        let adopter_share_bps = self.commitment_norm_group_adopter_share_bps(&members);
        Some(CommitmentNormGroupCandidate {
            center_id: center.id,
            score,
            adopter_share_bps,
        })
    }
    /// A group's members are drawn by Manhattan radius around the center's ECONOMIC anchor
    /// position (its assigned resource node, falling back to its literal world position, then
    /// the exchange), the same anchor `init_commitment_norm_cluster_seed` ranks the clustered
    /// seed by (`commitment_norm_seed_anchor_pos`) — not the center's instantaneous world
    /// position. Every colonist starts (and idle colonists remain) at the shared exchange
    /// tile, and gatherers/cultivators complete their haul-and-return cycle well inside one
    /// `imitation_period`, so a literal-position radius sampled at an imitation checkpoint
    /// collapses every group to the whole colony (no group can ever out-score another). The
    /// anchor is the colonist's persistent economic role (which resource it works), so it gives
    /// GROUP_RADIUS genuine neighbourhoods to distinguish without moving anyone or reading any
    /// adopter/committer/vocation field (a node id is a location, not an institution signal).
    pub(super) fn commitment_norm_group_members(
        &self,
        center_slot: usize,
        live: &[usize],
        radius: u16,
        exchange_pos: Pos,
    ) -> Vec<usize> {
        let Some(center) = self.colonists.get(center_slot) else {
            return Vec::new();
        };
        let center_pos = self.commitment_norm_seed_anchor_pos(center, exchange_pos);
        let mut members: Vec<(u64, usize)> = Vec::new();
        for &slot in live {
            let colonist = &self.colonists[slot];
            if !colonist.alive {
                continue;
            }
            let pos = self.commitment_norm_seed_anchor_pos(colonist, exchange_pos);
            if center_pos.manhattan(pos) <= u32::from(radius) {
                members.push((colonist.id.0, slot));
            }
        }
        members.sort_unstable_by_key(|&(id, _)| id);
        members.into_iter().map(|(_, slot)| slot).collect()
    }
    pub(super) fn commitment_norm_group_score(
        &self,
        members: &[usize],
        window: u64,
        food_target: u64,
        include_salt: bool,
    ) -> Option<CommitmentNormScore> {
        if members.len() < COMMITMENT_NORM_GROUP_MIN_SIZE {
            return None;
        }
        let mut alive_bps = 0u64;
        let mut hunger_bps = 0u64;
        let mut food_bps = 0u64;
        let mut salt_bps = 0u64;
        let mut scored = 0u64;
        for &slot in members {
            let Some(score) = self.commitment_norm_score(slot, window, food_target, include_salt)
            else {
                continue;
            };
            scored = scored.saturating_add(1);
            alive_bps = alive_bps.saturating_add(score.alive_bps);
            hunger_bps = hunger_bps.saturating_add(score.hunger_bps);
            food_bps = food_bps.saturating_add(score.food_bps);
            salt_bps = salt_bps.saturating_add(score.salt_bps);
        }
        if scored < COMMITMENT_NORM_GROUP_MIN_SIZE as u64 {
            return None;
        }
        let count = scored;
        let alive_bps = alive_bps / count;
        let hunger_bps = hunger_bps / count;
        let food_bps = food_bps / count;
        let salt_bps = salt_bps / count;
        let total_bps = COMMITMENT_NORM_ALIVE_WEIGHT
            .saturating_mul(alive_bps)
            .saturating_add(COMMITMENT_NORM_HUNGER_WEIGHT.saturating_mul(hunger_bps))
            .saturating_add(COMMITMENT_NORM_FOOD_WEIGHT.saturating_mul(food_bps))
            .saturating_add(COMMITMENT_NORM_SALT_WEIGHT.saturating_mul(salt_bps));
        Some(CommitmentNormScore {
            alive_bps,
            hunger_bps,
            food_bps,
            salt_bps,
            total_bps,
        })
    }
    pub(super) fn commitment_norm_group_adopter_share_bps(&self, members: &[usize]) -> u64 {
        if members.is_empty() {
            return 0;
        }
        let adopters = members
            .iter()
            .filter(|&&slot| self.colonists[slot].adopts_commitment_norm)
            .count() as u64;
        adopters.saturating_mul(COMMITMENT_NORM_SCORE_BPS) / members.len() as u64
    }
    pub(super) fn commitment_norm_score(
        &self,
        slot: usize,
        window: u64,
        food_target: u64,
        include_salt: bool,
    ) -> Option<CommitmentNormScore> {
        let colonist = self.colonists.get(slot)?;
        let window_len = usize::try_from(window).ok()?;
        if window_len == 0 || colonist.commitment_norm_observations.len() < window_len {
            return None;
        }
        let observations: Vec<CommitmentNormObservation> = colonist
            .commitment_norm_observations
            .iter()
            .rev()
            .take(window_len)
            .copied()
            .collect();
        if observations.len() < window_len {
            return None;
        }
        let need_max = u64::from(self.dynamics.need_max.max(1));
        let hunger_sum: u64 = observations.iter().map(|obs| u64::from(obs.hunger)).sum();
        let mean_hunger_bps = hunger_sum.saturating_mul(COMMITMENT_NORM_SCORE_BPS)
            / (need_max.saturating_mul(observations.len() as u64));
        let hunger_bps = COMMITMENT_NORM_SCORE_BPS
            .saturating_sub(mean_hunger_bps)
            .min(COMMITMENT_NORM_SCORE_BPS);
        let food_sum: u64 = observations
            .iter()
            .map(|obs| u64::from(obs.food_consumed))
            .sum();
        let food_bps = if food_target == 0 {
            0
        } else {
            food_sum
                .saturating_mul(COMMITMENT_NORM_SCORE_BPS)
                .checked_div(food_target)
                .unwrap_or(0)
                .min(COMMITMENT_NORM_SCORE_BPS)
        };
        let salt_bps = if include_salt && food_target > 0 {
            let salt_sum: u64 = observations.iter().map(|obs| obs.salt_stock).sum();
            salt_sum
                .saturating_mul(COMMITMENT_NORM_SCORE_BPS)
                .checked_div(food_target)
                .unwrap_or(0)
                .min(COMMITMENT_NORM_SCORE_BPS)
        } else {
            0
        };
        let alive_bps = if colonist.alive {
            COMMITMENT_NORM_SCORE_BPS
        } else {
            0
        };
        let total_bps = COMMITMENT_NORM_ALIVE_WEIGHT
            .saturating_mul(alive_bps)
            .saturating_add(COMMITMENT_NORM_HUNGER_WEIGHT.saturating_mul(hunger_bps))
            .saturating_add(COMMITMENT_NORM_FOOD_WEIGHT.saturating_mul(food_bps))
            .saturating_add(COMMITMENT_NORM_SALT_WEIGHT.saturating_mul(salt_bps));
        Some(CommitmentNormScore {
            alive_bps,
            hunger_bps,
            food_bps,
            salt_bps,
            total_bps,
        })
    }
    pub fn commitment_norm_spread_on(&self) -> bool {
        self.commitment_norm_spread_active()
    }
    pub fn fixed_commitment_norm_prevalence_config(&self) -> Option<f64> {
        self.fixed_commitment_norm_prevalence()
    }
    pub fn fixed_commitment_norm_on(&self) -> bool {
        self.fixed_commitment_norm_active()
    }
    pub fn commitment_norm_seed_adopter_ids(&self) -> Vec<u64> {
        self.colonists
            .iter()
            .filter(|c| c.commitment_norm_seed_adopter)
            .map(|c| c.id.0)
            .collect()
    }
    pub fn commitment_norm_adopter_ids(&self) -> Vec<u64> {
        self.colonists
            .iter()
            .filter(|c| c.adopts_commitment_norm)
            .map(|c| c.id.0)
            .collect()
    }
    pub fn commitment_norm_imitation_adopter_ids(&self) -> Vec<u64> {
        self.commitment_norm_imitation_adopters
            .iter()
            .map(|id| id.0)
            .collect()
    }
    pub fn commitment_norm_copy_events(&self) -> Vec<CommitmentNormCopyRow> {
        self.commitment_norm_copy_events.clone()
    }
    pub fn commitment_norm_flip_events(&self) -> Vec<CommitmentNormFlipRow> {
        self.commitment_norm_flip_events.clone()
    }
    pub fn commitment_norm_adoptions(&self) -> u64 {
        self.commitment_norm_adoptions
    }
    pub fn commitment_norm_abandonments(&self) -> u64 {
        self.commitment_norm_abandonments
    }
    pub fn commitment_norm_positive_copy_advantages(&self) -> usize {
        self.commitment_norm_copy_events
            .iter()
            .filter(|row| row.positive_pre_copy_advantage)
            .count()
    }
    pub fn commitment_norm_aligned_group_adoptions(&self) -> usize {
        self.commitment_norm_copy_events
            .iter()
            .filter(|row| row.aligned_group_adoption_pre_core)
            .count()
    }
    pub fn commitment_norm_group_welfare_adopter_covariance(&self) -> f64 {
        if self.commitment_norm_group_covariance_count == 0 {
            return 0.0;
        }
        self.commitment_norm_group_covariance_sum as f64
            / self.commitment_norm_group_covariance_count as f64
    }
    pub fn commitment_norm_group_covariance_observations(&self) -> u64 {
        self.commitment_norm_group_covariance_count
    }
    /// Empirically exercises [`Self::commitment_norm_group_score`] itself (not a re-check of the
    /// individual-score driver classifier): score a real live group, flip every member's
    /// adopter/committer/vocation identity, rescore the SAME membership, and confirm the score is
    /// bit-for-bit unchanged — proving the group aggregation reads none of those fields — then
    /// revert the flip. Diagnostic-only: never called from an economic phase.
    pub fn commitment_norm_group_score_purity_guard(&mut self) -> bool {
        let Some(chain) = self.chain.as_ref() else {
            return true;
        };
        let window = chain.imitation_window;
        let food_target = chain.food_window_target;
        let salt_in_score = chain.salt_in_score;
        let members = self.live_colonist_slots.clone();
        if members.len() < COMMITMENT_NORM_GROUP_MIN_SIZE {
            return true;
        }
        let Some(before) =
            self.commitment_norm_group_score(&members, window, food_target, salt_in_score)
        else {
            return true;
        };
        let saved: Vec<(bool, u16, Vocation)> = members
            .iter()
            .map(|&slot| {
                let c = &self.colonists[slot];
                (c.adopts_commitment_norm, c.commitment_remaining, c.vocation)
            })
            .collect();
        for (&slot, &(adopts, remaining, _)) in members.iter().zip(saved.iter()) {
            let c = &mut self.colonists[slot];
            c.adopts_commitment_norm = !adopts;
            c.commitment_remaining = if remaining == 0 { 1 } else { 0 };
            c.vocation = Vocation::Unassigned;
        }
        let after = self.commitment_norm_group_score(&members, window, food_target, salt_in_score);
        for (&slot, &(adopts, remaining, vocation)) in members.iter().zip(saved.iter()) {
            let c = &mut self.colonists[slot];
            c.adopts_commitment_norm = adopts;
            c.commitment_remaining = remaining;
            c.vocation = vocation;
        }
        after == Some(before)
    }
    pub fn commitment_norm_score_purity_guard(&self) -> bool {
        let generic = CommitmentNormScore {
            alive_bps: COMMITMENT_NORM_SCORE_BPS,
            hunger_bps: COMMITMENT_NORM_SCORE_BPS / 2,
            food_bps: COMMITMENT_NORM_SCORE_BPS / 3,
            salt_bps: 0,
            total_bps: COMMITMENT_NORM_ALIVE_WEIGHT * COMMITMENT_NORM_SCORE_BPS
                + COMMITMENT_NORM_HUNGER_WEIGHT * (COMMITMENT_NORM_SCORE_BPS / 2)
                + COMMITMENT_NORM_FOOD_WEIGHT * (COMMITMENT_NORM_SCORE_BPS / 3),
        };
        commitment_norm_copy_driver(generic, generic, false) == CommitmentNormCopyDriver::None
    }
}

pub(super) fn commitment_norm_seed_cluster_center(seed: u64, width: u16, height: u16) -> Pos {
    let width = u64::from(width.max(1));
    let height = u64::from(height.max(1));
    let x = deterministic_mix64(seed ^ COMMITMENT_NORM_CLUSTER_CENTER_SALT) % width;
    let y =
        deterministic_mix64(seed.rotate_left(23) ^ COMMITMENT_NORM_CLUSTER_CENTER_SALT) % height;
    Pos::new(
        u16::try_from(x).expect("cluster x is bounded by grid width"),
        u16::try_from(y).expect("cluster y is bounded by grid height"),
    )
}
pub(super) fn commitment_norm_seeded(seed: u64, id: AgentId, share_bps: u16) -> bool {
    let draw = deterministic_mix64(
        seed ^ COMMITMENT_NORM_SEED_SALT ^ id.0.wrapping_mul(0x9e37_79b9_7f4a_7c15),
    ) % COMMITMENT_NORM_SCORE_BPS;
    draw < u64::from(share_bps)
}
pub(super) fn fixed_commitment_norm_seeded(seed: u64, id: AgentId, prevalence: f64) -> bool {
    let threshold = (prevalence.clamp(0.0, 1.0) * COMMITMENT_NORM_SCORE_BPS as f64).round() as u64;
    if threshold == 0 {
        return false;
    }
    if threshold >= COMMITMENT_NORM_SCORE_BPS {
        return true;
    }
    let draw = deterministic_mix64(
        seed ^ COMMITMENT_NORM_SEED_SALT ^ id.0.wrapping_mul(0x9e37_79b9_7f4a_7c15),
    ) % COMMITMENT_NORM_SCORE_BPS;
    draw < threshold
}
pub(super) fn commitment_norm_copy_driver(
    copier: CommitmentNormScore,
    model: CommitmentNormScore,
    include_salt: bool,
) -> CommitmentNormCopyDriver {
    let mut best = (
        model
            .alive_bps
            .saturating_sub(copier.alive_bps)
            .saturating_mul(COMMITMENT_NORM_ALIVE_WEIGHT),
        CommitmentNormCopyDriver::Alive,
    );
    let hunger = (
        model
            .hunger_bps
            .saturating_sub(copier.hunger_bps)
            .saturating_mul(COMMITMENT_NORM_HUNGER_WEIGHT),
        CommitmentNormCopyDriver::HungerRelief,
    );
    if hunger.0 > best.0 {
        best = hunger;
    }
    let food = (
        model
            .food_bps
            .saturating_sub(copier.food_bps)
            .saturating_mul(COMMITMENT_NORM_FOOD_WEIGHT),
        CommitmentNormCopyDriver::FoodConsumed,
    );
    if food.0 > best.0 {
        best = food;
    }
    let salt = (
        if include_salt {
            model
                .salt_bps
                .saturating_sub(copier.salt_bps)
                .saturating_mul(COMMITMENT_NORM_SALT_WEIGHT)
        } else {
            0
        },
        CommitmentNormCopyDriver::SaltStock,
    );
    if salt.0 > best.0 {
        best = salt;
    }
    if best.0 == 0 {
        CommitmentNormCopyDriver::None
    } else {
        best.1
    }
}
