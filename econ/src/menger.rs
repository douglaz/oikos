//! Saleability tracking for Mengerian commodity-money emergence.

use crate::agent::AgentId;
use crate::barter::BarterTrade;
use crate::good::GoodId;
use crate::money::MengerianConfig;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SaleabilitySnapshot {
    pub tick: u64,
    pub good: GoodId,
    pub acceptances: u32,
    pub acceptance_share_bps: u16,
    pub acceptor_agents: u16,
    pub counterpart_goods: u16,
    pub eligible: bool,
    pub winner: bool,
}

impl Default for SaleabilitySnapshot {
    fn default() -> Self {
        Self {
            tick: 0,
            good: GoodId(0),
            acceptances: 0,
            acceptance_share_bps: 0,
            acceptor_agents: 0,
            counterpart_goods: 0,
            eligible: false,
            winner: false,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SaleabilityTracker {
    candidates: Vec<CandidateStats>,
    total_acceptances: u64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SaleabilityLeader {
    pub good: GoodId,
    pub share_bps: u16,
    pub runner_up_share_bps: u16,
    pub tied_best: bool,
}

/// A read-only view of one candidate good's accumulated saleability state: its
/// running acceptance count plus the **distinct** acceptor agents and counterpart
/// goods it has been traded against. Exposed (with the member lists, not just
/// their counts) so a caller serializing the tracker for a determinism digest can
/// capture the full future-behaviour identity — a later acceptance only advances
/// the eligibility counts when its acceptor/counterpart is new, so two trackers
/// with equal counts but different members can still diverge on a future tick.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CandidateSaleability<'a> {
    pub good: GoodId,
    pub acceptances: u64,
    pub acceptor_agents: &'a [AgentId],
    pub counterpart_goods: &'a [GoodId],
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct CandidateStats {
    good: GoodId,
    acceptances: u64,
    acceptor_agents: Vec<AgentId>,
    counterpart_goods: Vec<GoodId>,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
struct StableWinnerEvaluation {
    stable_winner: Option<GoodId>,
    stable_winner_ticks: u32,
    promotion_candidate: Option<GoodId>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MengerianEmergence {
    config: MengerianConfig,
    tracker: SaleabilityTracker,
    promoted_good: Option<GoodId>,
    stable_winner: Option<GoodId>,
    stable_winner_ticks: u32,
    promoted_at_tick: Option<u64>,
}

impl SaleabilityTracker {
    pub fn new(candidate_goods: Vec<GoodId>) -> Self {
        let mut candidates = candidate_goods;
        candidates.sort();
        candidates.dedup();
        Self {
            candidates: candidates
                .into_iter()
                .map(|good| CandidateStats {
                    good,
                    acceptances: 0,
                    acceptor_agents: Vec::new(),
                    counterpart_goods: Vec::new(),
                })
                .collect(),
            total_acceptances: 0,
        }
    }

    pub fn total_acceptances(&self) -> u64 {
        self.total_acceptances
    }

    /// The accumulated per-candidate saleability state, in the tracker's stored
    /// (sorted, deduped) candidate order. See [`CandidateSaleability`] for why the
    /// distinct-member lists — not just their counts — are exposed.
    pub fn candidate_saleability(&self) -> impl ExactSizeIterator<Item = CandidateSaleability<'_>> {
        self.candidates.iter().map(|stats| CandidateSaleability {
            good: stats.good,
            acceptances: stats.acceptances,
            acceptor_agents: &stats.acceptor_agents,
            counterpart_goods: &stats.counterpart_goods,
        })
    }

    pub fn observe_trade(&mut self, trade: &BarterTrade) {
        self.total_acceptances = self.total_acceptances.saturating_add(2);
        self.observe_acceptance(trade.b_gives, trade.a, trade.a_gives);
        self.observe_acceptance(trade.a_gives, trade.b, trade.b_gives);
    }

    pub fn acceptance_share_bps(&self, good: GoodId) -> Option<u16> {
        self.stats(good)
            .map(|stats| self.share_bps(stats.acceptances))
    }

    pub fn snapshots(&self, tick: u64, config: &MengerianConfig) -> Vec<SaleabilitySnapshot> {
        let winner = self.winner(config);
        self.candidates
            .iter()
            .map(|stats| {
                let share = self.share_bps(stats.acceptances);
                SaleabilitySnapshot {
                    tick,
                    good: stats.good,
                    acceptances: acceptances_to_u32(stats.acceptances),
                    acceptance_share_bps: share,
                    acceptor_agents: len_to_u16(stats.acceptor_agents.len()),
                    counterpart_goods: len_to_u16(stats.counterpart_goods.len()),
                    eligible: self.base_eligible(stats, share, config),
                    winner: winner == Some(stats.good),
                }
            })
            .collect()
    }

    pub fn winner(&self, config: &MengerianConfig) -> Option<GoodId> {
        let leader = self.leader_shares()?;
        if leader.tied_best || leader.share_bps <= leader.runner_up_share_bps {
            return None;
        }
        if leader.share_bps.saturating_sub(leader.runner_up_share_bps) < config.lead_margin_bps {
            return None;
        }
        let stats = self.stats(leader.good)?;
        if self.base_eligible(stats, leader.share_bps, config) {
            Some(leader.good)
        } else {
            None
        }
    }

    pub fn provisional_leader(&self, config: &MengerianConfig) -> Option<GoodId> {
        let leader = self.leader_shares()?;
        if leader.tied_best || leader.share_bps <= leader.runner_up_share_bps {
            return None;
        }
        let stats = self.stats(leader.good)?;
        if leader.share_bps < config.indirect_min_acceptance_share_bps
            || len_to_u16(stats.acceptor_agents.len()) < config.min_acceptor_agents
            || len_to_u16(stats.counterpart_goods.len()) < config.min_counterpart_goods
        {
            return None;
        }
        Some(leader.good)
    }

    fn observe_acceptance(&mut self, accepted: GoodId, acceptor: AgentId, counterpart: GoodId) {
        let Some(stats) = self
            .candidates
            .iter_mut()
            .find(|stats| stats.good == accepted)
        else {
            return;
        };
        stats.acceptances = stats.acceptances.saturating_add(1);
        insert_unique_sorted(&mut stats.acceptor_agents, acceptor);
        insert_unique_sorted(&mut stats.counterpart_goods, counterpart);
    }

    fn stats(&self, good: GoodId) -> Option<&CandidateStats> {
        self.candidates.iter().find(|stats| stats.good == good)
    }

    fn share_bps(&self, acceptances: u64) -> u16 {
        if self.total_acceptances == 0 {
            return 0;
        }
        let numerator = u128::from(acceptances).saturating_mul(10_000);
        let share = numerator / u128::from(self.total_acceptances);
        u16::try_from(share).unwrap_or(u16::MAX)
    }

    fn base_eligible(&self, stats: &CandidateStats, share: u16, config: &MengerianConfig) -> bool {
        self.total_acceptances >= u64::from(config.min_total_acceptances)
            && share >= config.promotion_threshold_bps
            && len_to_u16(stats.acceptor_agents.len()) >= config.min_acceptor_agents
            && len_to_u16(stats.counterpart_goods.len()) >= config.min_counterpart_goods
    }

    pub fn leader_shares(&self) -> Option<SaleabilityLeader> {
        let mut best: Option<(GoodId, u16)> = None;
        let mut runner_up_share = 0u16;
        let mut tied_best = false;

        for stats in &self.candidates {
            let share = self.share_bps(stats.acceptances);
            match best {
                None => {
                    best = Some((stats.good, share));
                }
                Some((_, best_share)) if share > best_share => {
                    runner_up_share = best_share.max(runner_up_share);
                    best = Some((stats.good, share));
                    tied_best = false;
                }
                Some((_, best_share)) if share == best_share => {
                    tied_best = true;
                    runner_up_share = runner_up_share.max(share);
                }
                Some(_) => {
                    runner_up_share = runner_up_share.max(share);
                }
            }
        }

        best.map(|(good, share)| SaleabilityLeader {
            good,
            share_bps: share,
            runner_up_share_bps: runner_up_share,
            tied_best,
        })
    }
}

impl MengerianEmergence {
    pub fn new(config: MengerianConfig) -> Self {
        let tracker = SaleabilityTracker::new(config.candidate_goods.clone());
        Self {
            config,
            tracker,
            promoted_good: None,
            stable_winner: None,
            stable_winner_ticks: 0,
            promoted_at_tick: None,
        }
    }

    pub fn observe_trade(&mut self, trade: &BarterTrade) {
        if self.promoted_good.is_none() {
            self.tracker.observe_trade(trade);
        }
    }

    pub fn snapshots(&self, tick: u64) -> Vec<SaleabilitySnapshot> {
        self.tracker.snapshots(tick, &self.config)
    }

    pub fn tracker(&self) -> &SaleabilityTracker {
        &self.tracker
    }

    /// The Mengerian config this emergence runs under — the adopted M20 envelope
    /// (candidate goods + promotion thresholds). Read-only; G5a asserts the
    /// spatial barter camp drives the lab's reused config, not a sim-local one.
    pub fn config(&self) -> &MengerianConfig {
        &self.config
    }

    pub fn leader_shares(&self) -> Option<SaleabilityLeader> {
        self.tracker.leader_shares()
    }

    pub fn current_money_good(&self) -> Option<GoodId> {
        self.promoted_good
    }

    pub fn promoted_at_tick(&self) -> Option<u64> {
        self.promoted_at_tick
    }

    pub fn saleability_bps(&self, good: GoodId) -> Option<u16> {
        self.tracker.acceptance_share_bps(good)
    }

    pub fn provisional_leader(&self) -> Option<GoodId> {
        self.tracker.provisional_leader(&self.config)
    }

    /// The good currently latched as the stable saleability winner (the lab's
    /// pre-promotion stability gate), or `None` if none leads stably. Promotion
    /// fires once this same good has held the lead for `config.stability_ticks`.
    /// Exposed so a caller can capture the promotion-timing state in a determinism
    /// digest: two barter states with identical holdings but a different latch
    /// promote on different future ticks.
    pub fn stable_winner(&self) -> Option<GoodId> {
        self.stable_winner
    }

    /// How many consecutive ticks the current [`Self::stable_winner`] has held the
    /// lead — the counter promotion fires on once it reaches
    /// `config.stability_ticks`. Part of the future-behaviour identity.
    pub fn stable_winner_ticks(&self) -> u32 {
        self.stable_winner_ticks
    }

    pub fn promotion_candidate_after_tick(&self) -> Option<GoodId> {
        if self.promoted_good.is_some() {
            return None;
        }
        self.evaluate_winner_after_tick().promotion_candidate
    }

    pub fn end_tick(&mut self, tick: u64) -> Option<GoodId> {
        if self.promoted_good.is_some() {
            return None;
        }
        let evaluation = self.evaluate_winner_after_tick();
        self.apply_winner_evaluation(evaluation);

        if let Some(good) = evaluation.promotion_candidate {
            self.promoted_good = Some(good);
            self.promoted_at_tick = Some(tick);
            return Some(good);
        }
        None
    }

    pub fn end_tick_without_promotion(&mut self) -> Option<GoodId> {
        if self.promoted_good.is_some() {
            return None;
        }
        let evaluation = self.evaluate_winner_after_tick();
        self.apply_winner_evaluation(evaluation);
        evaluation.promotion_candidate
    }

    fn evaluate_winner_after_tick(&self) -> StableWinnerEvaluation {
        let stable_winner = self.tracker.winner(&self.config);
        let stable_winner_ticks = if stable_winner != self.stable_winner {
            if stable_winner.is_some() {
                1
            } else {
                0
            }
        } else if stable_winner.is_some() {
            self.stable_winner_ticks.saturating_add(1)
        } else {
            0
        };

        let required_ticks = self.config.stability_ticks.max(1);
        let promotion_candidate = stable_winner.filter(|_| stable_winner_ticks >= required_ticks);
        StableWinnerEvaluation {
            stable_winner,
            stable_winner_ticks,
            promotion_candidate,
        }
    }

    fn apply_winner_evaluation(&mut self, evaluation: StableWinnerEvaluation) {
        self.stable_winner = evaluation.stable_winner;
        self.stable_winner_ticks = evaluation.stable_winner_ticks;
    }
}

fn len_to_u16(len: usize) -> u16 {
    u16::try_from(len).unwrap_or(u16::MAX)
}

fn acceptances_to_u32(acceptances: u64) -> u32 {
    u32::try_from(acceptances).unwrap_or(u32::MAX)
}

fn insert_unique_sorted<T: Ord>(items: &mut Vec<T>, item: T) {
    if let Err(index) = items.binary_search(&item) {
        items.insert(index, item);
    }
}

#[cfg(test)]
mod tests {
    use super::{MengerianEmergence, SaleabilityTracker};
    use crate::agent::AgentId;
    use crate::barter::{BarterReason, BarterTrade};
    use crate::good::{GoodId, CLOTH, FOOD, ORE, SALT, WOOD};
    use crate::money::MengerianConfig;

    #[test]
    fn saleability_counts_realized_acceptances_only() {
        let config = config(&[SALT]);
        let tracker = SaleabilityTracker::new(config.candidate_goods.clone());
        let snapshots = tracker.snapshots(0, &config);

        assert_eq!(snapshots.len(), 1);
        assert_eq!(snapshots[0].acceptances, 0);
        assert_eq!(snapshots[0].acceptance_share_bps, 0);
        assert_eq!(tracker.total_acceptances(), 0);
    }

    #[test]
    fn saleability_counts_both_sides_of_trade() {
        let config = config(&[FOOD, WOOD]);
        let mut tracker = SaleabilityTracker::new(config.candidate_goods.clone());

        tracker.observe_trade(&trade(1, 2, FOOD, WOOD));
        let snapshots = tracker.snapshots(1, &config);

        assert_eq!(snapshot(&snapshots, FOOD).acceptances, 1);
        assert_eq!(snapshot(&snapshots, FOOD).acceptance_share_bps, 5_000);
        assert_eq!(snapshot(&snapshots, WOOD).acceptances, 1);
        assert_eq!(snapshot(&snapshots, WOOD).acceptance_share_bps, 5_000);
        assert_eq!(tracker.total_acceptances(), 2);
    }

    #[test]
    fn saleability_requires_breadth_not_single_pair_churn() {
        let mut emergence = MengerianEmergence::new(MengerianConfig {
            candidate_goods: vec![SALT, FOOD],
            min_total_acceptances: 4,
            promotion_threshold_bps: 4_000,
            lead_margin_bps: 0,
            min_acceptor_agents: 2,
            min_counterpart_goods: 2,
            stability_ticks: 1,
            indirect_min_acceptance_share_bps: 3_000,
        });

        emergence.observe_trade(&trade(1, 2, FOOD, SALT));
        emergence.observe_trade(&trade(1, 2, FOOD, SALT));

        assert_eq!(emergence.end_tick(1), None);
        assert_eq!(emergence.current_money_good(), None);
    }

    #[test]
    fn saleability_requires_strict_lead() {
        let config = MengerianConfig {
            candidate_goods: vec![FOOD, WOOD],
            min_total_acceptances: 2,
            promotion_threshold_bps: 0,
            lead_margin_bps: 0,
            min_acceptor_agents: 0,
            min_counterpart_goods: 0,
            stability_ticks: 1,
            indirect_min_acceptance_share_bps: 0,
        };
        let mut tracker = SaleabilityTracker::new(config.candidate_goods.clone());

        tracker.observe_trade(&trade(1, 2, FOOD, WOOD));

        assert_eq!(tracker.winner(&config), None);
        assert!(tracker
            .snapshots(1, &config)
            .iter()
            .all(|snapshot| !snapshot.winner));
    }

    #[test]
    fn saleability_stability_ticks_delay_promotion() {
        let mut emergence = MengerianEmergence::new(MengerianConfig {
            candidate_goods: vec![SALT, FOOD],
            min_total_acceptances: 2,
            promotion_threshold_bps: 1_000,
            lead_margin_bps: 1,
            min_acceptor_agents: 1,
            min_counterpart_goods: 1,
            stability_ticks: 2,
            indirect_min_acceptance_share_bps: 1_000,
        });

        emergence.observe_trade(&trade(1, 2, ORE, SALT));

        assert_eq!(emergence.end_tick(1), None);
        assert_eq!(emergence.current_money_good(), None);
        assert_eq!(emergence.end_tick(2), Some(SALT));
        assert_eq!(emergence.current_money_good(), Some(SALT));
        assert_eq!(emergence.promoted_at_tick(), Some(2));
    }

    #[test]
    fn saleability_failed_promotion_tick_advances_stability_without_promoting() {
        let mut emergence = MengerianEmergence::new(MengerianConfig {
            candidate_goods: vec![SALT, FOOD],
            min_total_acceptances: 2,
            promotion_threshold_bps: 1_000,
            lead_margin_bps: 1,
            min_acceptor_agents: 1,
            min_counterpart_goods: 1,
            stability_ticks: 2,
            indirect_min_acceptance_share_bps: 1_000,
        });

        emergence.observe_trade(&trade(1, 2, ORE, SALT));

        assert_eq!(emergence.end_tick(1), None);
        assert_eq!(emergence.end_tick_without_promotion(), Some(SALT));
        assert_eq!(emergence.current_money_good(), None);
        assert_eq!(emergence.end_tick(3), Some(SALT));
        assert_eq!(emergence.promoted_at_tick(), Some(3));
    }

    #[test]
    fn candidate_saleability_exposes_accumulated_members() {
        let config = config(&[SALT, FOOD]);
        let mut tracker = SaleabilityTracker::new(config.candidate_goods.clone());

        // Two distinct acceptors take SALT, each against a different counterpart.
        tracker.observe_trade(&trade(1, 2, ORE, SALT));
        tracker.observe_trade(&trade(3, 4, CLOTH, SALT));

        let salt = tracker
            .candidate_saleability()
            .find(|c| c.good == SALT)
            .expect("SALT is a candidate");
        assert_eq!(salt.acceptances, 2);
        // SALT is the `b_gives` side of each trade, so its acceptor is the `a` agent
        // (agents 1 and 3); the DISTINCT acceptors are recorded sorted.
        assert_eq!(salt.acceptor_agents, &[AgentId(1), AgentId(3)]);
        // The DISTINCT counterpart goods SALT was traded against (CLOTH=5, ORE=6),
        // recorded sorted by id.
        assert_eq!(salt.counterpart_goods, &[CLOTH, ORE]);
        // The candidate view is in the tracker's stored (sorted) order.
        let goods: Vec<GoodId> = tracker.candidate_saleability().map(|c| c.good).collect();
        let mut sorted = goods.clone();
        sorted.sort();
        assert_eq!(goods, sorted);
    }

    #[test]
    fn stable_winner_latch_advances_then_promotes() {
        let mut emergence = MengerianEmergence::new(MengerianConfig {
            candidate_goods: vec![SALT, FOOD],
            min_total_acceptances: 2,
            promotion_threshold_bps: 1_000,
            lead_margin_bps: 1,
            min_acceptor_agents: 1,
            min_counterpart_goods: 1,
            stability_ticks: 2,
            indirect_min_acceptance_share_bps: 1_000,
        });

        // No barter yet: nothing latched.
        assert_eq!(emergence.stable_winner(), None);
        assert_eq!(emergence.stable_winner_ticks(), 0);

        emergence.observe_trade(&trade(1, 2, ORE, SALT));

        // Tick 1 latches SALT but the stability count is short of `stability_ticks`.
        assert_eq!(emergence.end_tick(1), None);
        assert_eq!(emergence.stable_winner(), Some(SALT));
        assert_eq!(emergence.stable_winner_ticks(), 1);

        // Tick 2 reaches the stability count and promotes; the latch is frozen.
        assert_eq!(emergence.end_tick(2), Some(SALT));
        assert_eq!(emergence.stable_winner(), Some(SALT));
        assert_eq!(emergence.stable_winner_ticks(), 2);
    }

    #[test]
    fn saleability_uses_integer_bps() {
        let config = config(&[SALT, FOOD]);
        let mut tracker = SaleabilityTracker::new(config.candidate_goods.clone());

        tracker.observe_trade(&trade(1, 2, ORE, SALT));
        tracker.observe_trade(&trade(3, 4, CLOTH, SALT));
        tracker.observe_trade(&trade(5, 6, ORE, CLOTH));

        assert_eq!(
            snapshot(&tracker.snapshots(1, &config), SALT).acceptances,
            2
        );
        assert_eq!(
            snapshot(&tracker.snapshots(1, &config), SALT).acceptance_share_bps,
            3_333
        );
    }

    #[test]
    fn saleability_internal_acceptance_count_exceeds_snapshot_width() {
        let config = config(&[SALT]);
        let mut tracker = SaleabilityTracker::new(config.candidate_goods.clone());
        tracker.total_acceptances = u64::from(u32::MAX) + 3;
        tracker.candidates[0].acceptances = u64::from(u32::MAX) + 1;

        let snapshots = tracker.snapshots(1, &config);
        let snapshot = snapshot(&snapshots, SALT);

        assert_eq!(tracker.total_acceptances(), u64::from(u32::MAX) + 3);
        assert_eq!(snapshot.acceptances, u32::MAX);
        assert_eq!(snapshot.acceptance_share_bps, 9_999);
    }

    fn config(candidates: &[GoodId]) -> MengerianConfig {
        MengerianConfig {
            candidate_goods: candidates.to_vec(),
            min_total_acceptances: 1,
            promotion_threshold_bps: 1,
            lead_margin_bps: 1,
            min_acceptor_agents: 1,
            min_counterpart_goods: 1,
            stability_ticks: 1,
            indirect_min_acceptance_share_bps: 1,
        }
    }

    fn trade(a: u32, b: u32, a_gives: GoodId, b_gives: GoodId) -> BarterTrade {
        BarterTrade {
            tick: 0,
            a: AgentId(u64::from(a)),
            b: AgentId(u64::from(b)),
            a_gives,
            b_gives,
            qty: 1,
            a_reason: BarterReason::DirectWant,
            b_reason: BarterReason::DirectWant,
        }
    }

    fn snapshot(
        snapshots: &[super::SaleabilitySnapshot],
        good: GoodId,
    ) -> &super::SaleabilitySnapshot {
        snapshots
            .iter()
            .find(|snapshot| snapshot.good == good)
            .expect("snapshot for good")
    }
}
