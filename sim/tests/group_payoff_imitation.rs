//! S24c acceptance suite: S24b abandonable commitment-norm imitation scored on local
//! group welfare, with copy direction following the welfare-selected group's adopter-share
//! gradient. The verdict is classified and printed; milestone success is not asserted.

use std::collections::{BTreeMap, BTreeSet};
use std::sync::OnceLock;

use econ::good::SALT;
use sim::{settlement::CommitmentNormCopyDriver, Settlement, SettlementConfig};

#[path = "support/mod.rs"]
mod support;
use support::*;

const PERSIST_FRACTION: f64 = 0.5;
const PERSIST_COHORT: usize = 4;
const FINAL_WINDOW: usize = 200;
const MATERIAL_BUYER_FOOD: u64 = 4;
const MIN_BUYER_COHORT: usize = 2;
const ADOPTER_SHARE_MIN: f64 = 0.15;
const ADOPTER_SHARE_MAX: f64 = 0.6;
const MIN_NONSEED_ADOPTERS: usize = 4;
const MIN_NONSEED_COMMITS: usize = 4;
const CORE_MARGIN: usize = PERSIST_COHORT;
const COMMITMENT_SEED_SHARE_BPS_DEFAULT: u16 = 1_500;
const IMITATION_PERIOD_DEFAULT: u64 = 24;
const IMITATION_MARGIN_BPS_DEFAULT: u64 = 1_500;
const IMITATION_RADIUS_DEFAULT: u16 = 1;
const GROUP_MIN_SIZE: usize = 3;
const ADOPTER_SHARE_GAP_BPS: u64 = 1_000;
const LOW_SEED_SHARE_BPS: u16 = 500;
const HIGH_SEED_SHARE_BPS: u16 = 3_000;
const TINY_MARGIN_BPS: u64 = 1;
const HUGE_MARGIN_BPS: u64 = 40_000;
const SMALL_GROUP_RADIUS: u16 = 0;
// Anchor positions (node/home_node, not literal instantaneous world position — see
// `commitment_norm_seed_anchor_pos`) resolve to a handful of discrete economic-role
// clusters, not a continuum, so a radius has to clear the gap BETWEEN two clusters before
// it changes any membership; a gap of 2 or less would collapse into the same handful of
// values as `IMITATION_RADIUS_DEFAULT` for any world this colony's node layout could
// plausibly produce. 4 is comfortably past that (still "local": far short of the world
// width), so the axis is a real bracket, not a same-value repeat of the default.
const LARGE_GROUP_RADIUS: u16 = 4;

fn persist_threshold() -> u32 {
    (PERSIST_FRACTION * FINAL_WINDOW as f64).ceil() as u32
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
enum Verdict {
    SeedDies,
    MoneyFailure,
    ConservationBroken,
    Cull,
    GroupSignalVacuous,
    NormDiesBack,
    UniversalCommitmentRePin,
    GroupDriftNotSelection,
    SeedClusterOnly,
    SpreadWithoutOccupation,
    CleanInstitutionSpread,
    WealthProxySelection,
}

#[derive(Clone, Debug)]
struct Metrics {
    seed: u64,
    group_on: bool,
    conserved: bool,
    bread_minted_max: u64,
    extinct: bool,
    provenance_clean: bool,
    promoted: bool,
    adoption_invariant: bool,
    seed_adopters: usize,
    seed_alive_final: usize,
    final_adopters: usize,
    nonseed_adopters: usize,
    nonseed_commits: usize,
    nonseed_renewed: usize,
    nonseed_core: usize,
    persistent_committed_cohort: usize,
    adopter_core: usize,
    all_persistent_renewed: bool,
    final_buyer_cohort: usize,
    post_promotion_bought: u64,
    positive_group_copy_advantages: usize,
    aligned_group_adoptions: usize,
    copy_events: usize,
    group_copy_events: usize,
    generic_copy_drivers: usize,
    salt_copy_drivers: usize,
    score_purity_guard: bool,
    group_score_purity_guard: bool,
    welfare_adopter_covariance: f64,
    covariance_samples: u64,
    adoptions: u64,
    abandonments: u64,
    final_window_flips: u64,
    final_window_share_variance: f64,
    living: usize,
    starvation: u64,
}

impl Metrics {
    fn adopter_share(&self) -> f64 {
        let denom = self.living.max(1) as f64;
        self.final_adopters as f64 / denom
    }

    fn bounded_adoption_clause(&self) -> bool {
        (ADOPTER_SHARE_MIN..=ADOPTER_SHARE_MAX).contains(&self.adopter_share())
    }

    fn core_clause(&self) -> bool {
        self.persistent_committed_cohort >= PERSIST_COHORT && self.all_persistent_renewed
    }

    fn buyer_clause(&self) -> bool {
        self.final_buyer_cohort >= MIN_BUYER_COHORT
            && self.post_promotion_bought >= MATERIAL_BOUGHT_FLOOR
    }

    fn bounded_two_tier_success(&self) -> bool {
        self.bounded_adoption_clause() && self.core_clause() && self.buyer_clause()
    }

    fn nonseed_clause(&self) -> bool {
        self.nonseed_adopters >= MIN_NONSEED_ADOPTERS
            && self.nonseed_commits >= MIN_NONSEED_COMMITS
            && self.nonseed_renewed >= 1
            && self.nonseed_core >= 1
    }

    fn guard_clause(&self) -> bool {
        self.conserved
            && self.bread_minted_max == 0
            && !self.extinct
            && self.provenance_clean
            && self.promoted
            && self.adoption_invariant
            && self.score_purity_guard
            && self.group_score_purity_guard
            && self.salt_copy_drivers == 0
    }

    // A POSITIVE, ALIGNED group-payoff signal — required for SUCCESS: the mechanism fired,
    // found welfare advantages, AND at least one aligned false→true adoption occurred (best
    // group had higher welfare AND higher adopter share) with positive welfare↔adopter covariance.
    fn group_signal_clause(&self) -> bool {
        self.positive_group_copy_advantages > 0
            && self.aligned_group_adoptions > 0
            && self.welfare_adopter_covariance > 0.0
            && self.covariance_samples > 0
            && self.generic_copy_drivers > 0
            && self.score_purity_guard
            && self.group_score_purity_guard
            && self.salt_copy_drivers == 0
    }

    // Was a MEANINGFUL group signal even OBSERVED? (Codex review-of-results): `GroupSignalVacuous`
    // must mean "no signal to select on" — no positive group-copy advantages / no samples — NOT
    // "the signal fired but selected away". A run with positive_group_copy_advantages>0 and
    // samples>0 DID produce a group signal; if it then dies back (adverse or non-adoption-aligned),
    // that is `NormDiesBack`, not vacuous. A negative covariance is an ADVERSE signal, still present.
    fn group_signal_present(&self) -> bool {
        self.positive_group_copy_advantages > 0 && self.covariance_samples > 0
    }

    fn selection_clause(&self, no_imitation: &[Metrics], random: &[Metrics]) -> bool {
        let no_core = no_imitation.iter().all(|m| !m.core_clause());
        let Some(random) = random.iter().find(|r| r.seed == self.seed) else {
            return false;
        };
        self.persistent_committed_cohort >= random.persistent_committed_cohort + CORE_MARGIN
            && !random.bounded_two_tier_success()
            && no_core
            && self.group_signal_clause()
    }

    fn all_success_clauses(
        &self,
        no_imitation: &[Metrics],
        random: &[Metrics],
        individual: &[Metrics],
        unprofitable: &[Metrics],
    ) -> bool {
        let individual_dies_back = individual
            .iter()
            .all(|m| m.adopter_share() < ADOPTER_SHARE_MIN && !m.core_clause());
        let unprofitable_killed = unprofitable.iter().all(|m| {
            m.welfare_adopter_covariance <= 0.0
                && m.aligned_group_adoptions == 0
                && m.nonseed_adopters < MIN_NONSEED_ADOPTERS
                && !m.core_clause()
        });
        self.bounded_adoption_clause()
            && self.core_clause()
            && self.buyer_clause()
            && self.selection_clause(no_imitation, random)
            && self.nonseed_clause()
            && self.guard_clause()
            && individual_dies_back
            && unprofitable_killed
    }

    fn verdict(
        &self,
        no_imitation: &[Metrics],
        random: &[Metrics],
        individual: &[Metrics],
        unprofitable: &[Metrics],
    ) -> Verdict {
        if self.seed_adopters == 0 || self.seed_alive_final == 0 {
            return Verdict::SeedDies;
        }
        if !self.promoted {
            return Verdict::MoneyFailure;
        }
        if !self.conserved
            || self.bread_minted_max > 0
            || !self.provenance_clean
            || !self.adoption_invariant
            || !self.score_purity_guard
            || !self.group_score_purity_guard
        {
            return Verdict::ConservationBroken;
        }
        if self.extinct {
            return Verdict::Cull;
        }
        if !self.group_signal_present() {
            return Verdict::GroupSignalVacuous;
        }
        if self.adopter_share() < ADOPTER_SHARE_MIN && !self.core_clause() {
            return Verdict::NormDiesBack;
        }
        if self.adopter_share() > ADOPTER_SHARE_MAX && !self.buyer_clause() {
            return Verdict::UniversalCommitmentRePin;
        }
        if let Some(r) = random.iter().find(|r| r.seed == self.seed) {
            if self.persistent_committed_cohort < r.persistent_committed_cohort + CORE_MARGIN
                || r.bounded_two_tier_success()
            {
                return Verdict::GroupDriftNotSelection;
            }
        }
        if self.core_clause() && !self.nonseed_clause() {
            return Verdict::SeedClusterOnly;
        }
        if !self.core_clause() {
            return Verdict::SpreadWithoutOccupation;
        }
        if self.all_success_clauses(no_imitation, random, individual, unprofitable) {
            Verdict::CleanInstitutionSpread
        } else {
            Verdict::SpreadWithoutOccupation
        }
    }

    fn line(
        &self,
        no_imitation: &[Metrics],
        random: &[Metrics],
        individual: &[Metrics],
        unprofitable: &[Metrics],
    ) -> String {
        format!(
            "{:?} | seed={} final={} nonseed={} share={:.2} | nonseed_commit={} \
             renewed={} nonseed_core={} | core={} adopter_core={} all_renewed={} buyers={} \
             post_bought={} | group_copy(events={} pos_adv={} aligned={} generic={} salt={} \
             purity={} group_purity={} cov={:.2} cov_n={}) | flips(adopt={} abandon={} fw={} \
             var={:.4}) | promoted={} clean={} conserved={} minted={} invariant={} extinct={} \
             living={} starv={}",
            self.verdict(no_imitation, random, individual, unprofitable),
            self.seed_adopters,
            self.final_adopters,
            self.nonseed_adopters,
            self.adopter_share(),
            self.nonseed_commits,
            self.nonseed_renewed,
            self.nonseed_core,
            self.persistent_committed_cohort,
            self.adopter_core,
            self.all_persistent_renewed,
            self.final_buyer_cohort,
            self.post_promotion_bought,
            self.group_copy_events,
            self.positive_group_copy_advantages,
            self.aligned_group_adoptions,
            self.generic_copy_drivers,
            self.salt_copy_drivers,
            self.score_purity_guard,
            self.group_score_purity_guard,
            self.welfare_adopter_covariance,
            self.covariance_samples,
            self.adoptions,
            self.abandonments,
            self.final_window_flips,
            self.final_window_share_variance,
            self.promoted,
            self.provenance_clean,
            self.conserved,
            self.bread_minted_max,
            self.adoption_invariant,
            self.extinct,
            self.living,
            self.starvation,
        )
    }
}

fn headline_cfg() -> SettlementConfig {
    SettlementConfig::frontier_group_payoff_imitation()
}

fn individual_score_control_cfg() -> SettlementConfig {
    SettlementConfig::frontier_abandonable_norm()
}

fn no_imitation_cfg() -> SettlementConfig {
    let mut cfg = headline_cfg();
    if let Some(chain) = cfg.chain.as_mut() {
        chain.no_imitation = true;
    }
    cfg
}

fn random_group_imitation_cfg() -> SettlementConfig {
    let mut cfg = headline_cfg();
    if let Some(chain) = cfg.chain.as_mut() {
        chain.random_imitation = true;
    }
    cfg
}

fn no_seed_cfg() -> SettlementConfig {
    let mut cfg = headline_cfg();
    if let Some(chain) = cfg.chain.as_mut() {
        chain.commitment_seed_share_bps = 0;
    }
    cfg
}

fn unprofitable_commitment_cfg() -> SettlementConfig {
    let mut cfg = headline_cfg();
    if let Some(chain) = cfg.chain.as_mut() {
        chain.commitment_term = 1;
    }
    cfg
}

fn salt_in_score_cfg() -> SettlementConfig {
    let mut cfg = headline_cfg();
    if let Some(chain) = cfg.chain.as_mut() {
        chain.salt_in_score = true;
    }
    cfg
}

fn seed_cluster_only_cfg() -> SettlementConfig {
    let mut cfg = headline_cfg();
    if let Some(chain) = cfg.chain.as_mut() {
        chain.no_imitation = true;
        chain.commitment_seed_share_bps = HIGH_SEED_SHARE_BPS;
    }
    cfg
}

fn live_adopter_ids(s: &Settlement) -> BTreeSet<u64> {
    (0..s.population())
        .filter(|&i| s.is_alive(i) && s.adopts_commitment_norm_of(i))
        .filter_map(|i| s.colonist_id(i).map(|id| id.0))
        .collect()
}

fn live_norm_bits(s: &Settlement) -> BTreeMap<u64, bool> {
    (0..s.population())
        .filter(|&i| s.is_alive(i))
        .filter_map(|i| {
            s.colonist_id(i)
                .map(|id| (id.0, s.adopts_commitment_norm_of(i)))
        })
        .collect()
}

fn variance(xs: &[f64]) -> f64 {
    if xs.is_empty() {
        return 0.0;
    }
    let mean = xs.iter().sum::<f64>() / xs.len() as f64;
    xs.iter()
        .map(|x| {
            let d = x - mean;
            d * d
        })
        .sum::<f64>()
        / xs.len() as f64
}

fn run_metrics(seed: u64, cfg: &SettlementConfig, ticks: u64) -> Metrics {
    let mut s = Settlement::generate(seed, cfg);
    let group_on = s.group_payoff_imitation_on();
    let bread = s.bread_good();
    let seed_ids: BTreeSet<u64> = s.commitment_norm_seed_adopter_ids().into_iter().collect();

    let mut conserved = true;
    let mut bread_minted_max = 0u64;
    let mut promoted = false;
    let mut bought_at_promotion = None;
    let final_window_start = ticks.saturating_sub(FINAL_WINDOW as u64);
    let mut fw_persist: BTreeMap<u64, u32> = BTreeMap::new();
    let mut buyer_samples = Vec::with_capacity(ticks as usize);
    let mut share_samples = Vec::with_capacity(FINAL_WINDOW);
    let mut final_window_flips = 0u64;
    let mut adoption_invariant = true;

    for t in 0..ticks {
        let before = live_norm_bits(&s);
        let report = s.econ_tick();
        conserved &= report.conserves();
        if let Some(bread) = bread {
            bread_minted_max = bread_minted_max.max(report.endowment_of(bread));
        }
        if !promoted && s.current_money_good() == Some(SALT) {
            promoted = true;
            bought_at_promotion = Some(s.acquisition_consumed_by_channel().bought);
        }

        let after = live_norm_bits(&s);
        let flip_rows = s.commitment_norm_flip_events();
        let copy_rows = s.commitment_norm_copy_events();
        for (&id, &from) in &before {
            let Some(&to) = after.get(&id) else { continue };
            if from != to {
                let recorded = flip_rows.iter().any(|row| {
                    row.tick == t && row.agent == id && row.from == from && row.to == to
                });
                let copied = copy_rows.iter().any(|row| {
                    row.copier == id
                        && row.copied_norm_bit == to
                        && row.tick <= t
                        && (!group_on || row.group_imitation)
                });
                adoption_invariant &= recorded && copied;
                if t >= final_window_start {
                    final_window_flips = final_window_flips.saturating_add(1);
                }
            }
        }

        let mut buyers = 0usize;
        let mut live_adopters = 0usize;
        for i in 0..s.population() {
            if !s.is_alive(i) {
                continue;
            }
            if s.adopts_commitment_norm_of(i) {
                live_adopters += 1;
            }
            if !s.adopts_commitment_norm_of(i) && s.is_committed(i) {
                adoption_invariant = false;
            }
            let Some(id) = s.colonist_id(i) else { continue };
            if t >= final_window_start && (s.is_cultivating(i) || s.is_committed(i)) {
                *fw_persist.entry(id.0).or_insert(0) += 1;
            }
            if !s.adopts_commitment_norm_of(i)
                && !s.is_committed(i)
                && s.bought_food_of(i) >= MATERIAL_BUYER_FOOD
            {
                buyers += 1;
            }
        }
        if t >= final_window_start {
            let denom = living(&s).max(1) as f64;
            share_samples.push(live_adopters as f64 / denom);
        }
        buyer_samples.push(buyers);
    }

    let committed_ever: BTreeSet<u64> = s.commitment_committed_ids().into_iter().collect();
    let final_adopter_ids = live_adopter_ids(&s);
    let nonseed_adopters = final_adopter_ids.difference(&seed_ids).count();

    let id_to_index: BTreeMap<u64, usize> = (0..s.population())
        .filter_map(|i| s.colonist_id(i).map(|id| (id.0, i)))
        .collect();
    let mut persistent_committed_ids = BTreeSet::new();
    let mut all_persistent_renewed = true;
    let mut nonseed_renewed = 0usize;
    for (&id, &i) in &id_to_index {
        if !seed_ids.contains(&id) && s.commitment_renewals_of(i) > 0 {
            nonseed_renewed += 1;
        }
    }
    for (&id, &count) in &fw_persist {
        if !committed_ever.contains(&id) {
            continue;
        }
        let renewals = id_to_index
            .get(&id)
            .map_or(0, |&i| s.commitment_renewals_of(i));
        if count >= persist_threshold() {
            persistent_committed_ids.insert(id);
            if renewals == 0 {
                all_persistent_renewed = false;
            }
        }
    }

    let consumed = s.acquisition_consumed_by_channel();
    let (_pp, pp_minted) = s.pre_promotion_bread_for_salt_by_provenance();
    let provenance_clean = s.seeded_minted_bread_sold_for_salt() == 0 && pp_minted == 0;
    let post_promotion_bought = bought_at_promotion
        .map(|at| consumed.bought.saturating_sub(at))
        .unwrap_or(0);
    let copy_events = s.commitment_norm_copy_events();
    let group_copy_events = copy_events.iter().filter(|row| row.group_imitation).count();
    let generic_copy_drivers = copy_events
        .iter()
        .filter(|row| {
            matches!(
                row.driver,
                CommitmentNormCopyDriver::Alive
                    | CommitmentNormCopyDriver::HungerRelief
                    | CommitmentNormCopyDriver::FoodConsumed
            )
        })
        .count();
    let salt_copy_drivers = copy_events
        .iter()
        .filter(|row| row.driver == CommitmentNormCopyDriver::SaltStock)
        .count();

    Metrics {
        seed,
        group_on,
        conserved,
        bread_minted_max,
        extinct: living(&s) == 0,
        provenance_clean,
        promoted: s.current_money_good() == Some(SALT),
        adoption_invariant,
        seed_adopters: seed_ids.len(),
        seed_alive_final: (0..s.population())
            .filter(|&i| {
                s.is_alive(i) && s.colonist_id(i).is_some_and(|id| seed_ids.contains(&id.0))
            })
            .count(),
        final_adopters: final_adopter_ids.len(),
        nonseed_adopters,
        nonseed_commits: committed_ever.difference(&seed_ids).count(),
        nonseed_renewed,
        nonseed_core: persistent_committed_ids.difference(&seed_ids).count(),
        persistent_committed_cohort: persistent_committed_ids.len(),
        adopter_core: persistent_committed_ids
            .intersection(&final_adopter_ids)
            .count(),
        all_persistent_renewed,
        final_buyer_cohort: buyer_samples[buyer_samples.len().saturating_sub(FINAL_WINDOW)..]
            .iter()
            .copied()
            .max()
            .unwrap_or(0),
        post_promotion_bought,
        positive_group_copy_advantages: copy_events
            .iter()
            .filter(|row| row.group_imitation && row.positive_pre_copy_advantage)
            .count(),
        aligned_group_adoptions: s.commitment_norm_aligned_group_adoptions(),
        copy_events: copy_events.len(),
        group_copy_events,
        generic_copy_drivers,
        salt_copy_drivers,
        score_purity_guard: s.commitment_norm_score_purity_guard(),
        group_score_purity_guard: s.commitment_norm_group_score_purity_guard(),
        welfare_adopter_covariance: s.commitment_norm_group_welfare_adopter_covariance(),
        covariance_samples: s.commitment_norm_group_covariance_observations(),
        adoptions: s.commitment_norm_adoptions(),
        abandonments: s.commitment_norm_abandonments(),
        final_window_flips,
        final_window_share_variance: variance(&share_samples),
        living: living(&s),
        starvation: s.starvation_deaths_total(),
    }
}

fn run_batch(jobs: Vec<(u64, SettlementConfig)>) -> Vec<Metrics> {
    let workers = 8usize.min(jobs.len().max(1));
    let mut buckets: Vec<Vec<(usize, u64, SettlementConfig)>> = vec![Vec::new(); workers];
    for (i, (seed, cfg)) in jobs.into_iter().enumerate() {
        buckets[i % workers].push((i, seed, cfg));
    }
    let mut computed: Vec<(usize, Metrics)> = std::thread::scope(|scope| {
        let handles: Vec<_> = buckets
            .into_iter()
            .map(|bucket| {
                scope.spawn(move || {
                    bucket
                        .into_iter()
                        .map(|(i, seed, cfg)| (i, run_metrics(seed, &cfg, PROBE_TICKS)))
                        .collect::<Vec<_>>()
                })
            })
            .collect();
        handles
            .into_iter()
            .flat_map(|h| h.join().expect("worker thread"))
            .collect()
    });
    computed.sort_by_key(|(i, _)| *i);
    computed.into_iter().map(|(_, metrics)| metrics).collect()
}

fn run_all(cfg: SettlementConfig) -> Vec<Metrics> {
    run_batch(SEEDS.iter().map(|&seed| (seed, cfg.clone())).collect())
}

fn headline_metrics() -> &'static [Metrics] {
    static METRICS: OnceLock<Vec<Metrics>> = OnceLock::new();
    METRICS.get_or_init(|| run_all(headline_cfg()))
}

fn individual_metrics() -> &'static [Metrics] {
    static METRICS: OnceLock<Vec<Metrics>> = OnceLock::new();
    METRICS.get_or_init(|| run_all(individual_score_control_cfg()))
}

fn no_imitation_metrics() -> &'static [Metrics] {
    static METRICS: OnceLock<Vec<Metrics>> = OnceLock::new();
    METRICS.get_or_init(|| run_all(no_imitation_cfg()))
}

fn random_metrics() -> &'static [Metrics] {
    static METRICS: OnceLock<Vec<Metrics>> = OnceLock::new();
    METRICS.get_or_init(|| run_all(random_group_imitation_cfg()))
}

fn no_seed_metrics() -> &'static [Metrics] {
    static METRICS: OnceLock<Vec<Metrics>> = OnceLock::new();
    METRICS.get_or_init(|| run_all(no_seed_cfg()))
}

fn unprofitable_metrics() -> &'static [Metrics] {
    static METRICS: OnceLock<Vec<Metrics>> = OnceLock::new();
    METRICS.get_or_init(|| run_all(unprofitable_commitment_cfg()))
}

fn salt_metrics() -> &'static [Metrics] {
    static METRICS: OnceLock<Vec<Metrics>> = OnceLock::new();
    METRICS.get_or_init(|| run_all(salt_in_score_cfg()))
}

fn seed_cluster_only_metrics() -> &'static [Metrics] {
    static METRICS: OnceLock<Vec<Metrics>> = OnceLock::new();
    METRICS.get_or_init(|| run_all(seed_cluster_only_cfg()))
}

fn assert_guards(m: &Metrics, ctx: &str) {
    assert!(m.conserved, "[{ctx}] conservation broke (seed {})", m.seed);
    assert_eq!(
        m.bread_minted_max, 0,
        "[{ctx}] bread minted (seed {})",
        m.seed
    );
    assert!(!m.extinct, "[{ctx}] colony extinct (seed {})", m.seed);
    assert!(
        m.provenance_clean,
        "[{ctx}] provenance disqualified (seed {})",
        m.seed
    );
    assert!(m.promoted, "[{ctx}] SALT did not promote (seed {})", m.seed);
    assert!(
        m.adoption_invariant,
        "[{ctx}] adoption invariant failed (seed {})",
        m.seed
    );
    assert!(
        m.score_purity_guard && m.group_score_purity_guard,
        "[{ctx}] score-purity guard failed (seed {})",
        m.seed
    );
}

#[test]
#[allow(clippy::assertions_on_constants)]
fn coherence_constants_are_well_formed() {
    assert!(COMMITMENT_SEED_SHARE_BPS_DEFAULT > 0);
    assert!(COMMITMENT_SEED_SHARE_BPS_DEFAULT < 10_000);
    assert!(IMITATION_PERIOD_DEFAULT > 0);
    assert!(IMITATION_MARGIN_BPS_DEFAULT > 0);
    assert_eq!(IMITATION_RADIUS_DEFAULT, 1);
    assert_eq!(GROUP_MIN_SIZE, 3);
    assert_eq!(ADOPTER_SHARE_GAP_BPS, 1_000);
    assert!(ADOPTER_SHARE_MIN > 0.0);
    assert!(ADOPTER_SHARE_MIN < ADOPTER_SHARE_MAX);
    assert!(ADOPTER_SHARE_MAX < 1.0);
    assert_eq!(CORE_MARGIN, PERSIST_COHORT);
    assert!(MIN_NONSEED_ADOPTERS > 0);
    assert!(MIN_NONSEED_COMMITS > 0);
    assert!(persist_threshold() as usize <= FINAL_WINDOW);
}

#[test]
fn mandatory_non_vacuity_group_signal_and_anti_drift() {
    let headline = headline_metrics();
    let random = random_metrics();
    for m in headline {
        assert_guards(m, "headline");
        assert!(m.group_on, "headline must activate group-payoff imitation");
        assert!(
            m.seed_adopters > 0 && m.adopter_share() < 1.0,
            "headline seed is not a real minority (seed {})",
            m.seed
        );
        assert!(
            m.covariance_samples > 0,
            "headline did not exercise group observations (seed {})",
            m.seed
        );
        eprintln!("[headline seed {}] {:?}", m.seed, m);
    }
    for m in random {
        assert_guards(m, "random_group_imitation");
        assert!(
            !m.bounded_two_tier_success(),
            "random group null reached bounded two-tier success (seed {})",
            m.seed
        );
    }
    if !headline
        .iter()
        .any(|m| m.nonseed_adopters >= MIN_NONSEED_ADOPTERS)
    {
        eprintln!("headline produced no material non-seed adoption");
    }
    if !headline
        .iter()
        .any(|m| m.nonseed_commits >= MIN_NONSEED_COMMITS)
    {
        eprintln!("headline produced no material non-seed commitment");
    }
    if !headline
        .iter()
        .any(|m| m.positive_group_copy_advantages > 0)
    {
        eprintln!("headline saw no positive pre-copy group-welfare advantage");
    }
    if !headline.iter().any(|m| m.generic_copy_drivers > 0) {
        eprintln!("headline produced no generic copy driver");
    }
    for h in headline {
        let r = random
            .iter()
            .find(|m| m.seed == h.seed)
            .expect("matched random seed");
        assert!(
            !r.core_clause()
                || h.persistent_committed_cohort >= r.persistent_committed_cohort + CORE_MARGIN,
            "matched random group null reached the core without the required margin (seed {})",
            h.seed
        );
    }
}

#[test]
fn group_payoff_verdict_prints() {
    let headline = headline_metrics();
    let no_imitation = no_imitation_metrics();
    let random = random_metrics();
    let individual = individual_metrics();
    let unprofitable = unprofitable_metrics();
    let salt = salt_metrics();
    eprintln!("\n================ S24c group-payoff imitation ================");
    for m in headline {
        assert_guards(m, "headline");
        eprintln!(
            "seed {:>2}: {}",
            m.seed,
            m.line(no_imitation, random, individual, unprofitable)
        );
    }
    let mut tally = BTreeMap::new();
    for m in headline {
        *tally
            .entry(m.verdict(no_imitation, random, individual, unprofitable))
            .or_insert(0usize) += 1;
    }
    let clean = *tally.get(&Verdict::CleanInstitutionSpread).unwrap_or(&0);
    let drift = *tally.get(&Verdict::GroupDriftNotSelection).unwrap_or(&0);
    eprintln!("headline tally: {tally:?}");
    eprintln!(
        "milestone success bar: clean={clean}/5 drift={drift}/5 strict_success={}",
        clean >= 3 && drift == 0
    );
    let headline_success = clean >= 3 && drift == 0;
    let salt_clean = salt
        .iter()
        .filter(|m| {
            m.verdict(no_imitation, random, individual, unprofitable)
                == Verdict::CleanInstitutionSpread
        })
        .count();
    if !headline_success && salt_clean >= 3 {
        eprintln!(
            "S24c sensitivity verdict: {:?}",
            Verdict::WealthProxySelection
        );
    }
    eprintln!("============================================================\n");
}

#[test]
fn individual_score_control_reproduces_norm_dies_back() {
    let individual = individual_metrics();
    for m in individual {
        assert_guards(m, "individual_score_control");
        eprintln!("[individual seed {}] {:?}", m.seed, m);
        assert!(!m.group_on, "individual control must leave S24c flag off");
        assert!(
            m.adopter_share() < ADOPTER_SHARE_MIN && !m.core_clause(),
            "S24b individual-score control did not reproduce NormDiesBack (seed {})",
            m.seed
        );
    }
}

#[test]
fn random_group_imitation_null_does_not_reach_core() {
    let random = random_metrics();
    let headline = headline_metrics();
    for m in random {
        assert_guards(m, "random_group_imitation");
        eprintln!("[random seed {}] {:?}", m.seed, m);
        assert!(
            !m.bounded_two_tier_success(),
            "random group imitation reached bounded success (seed {})",
            m.seed
        );
    }
    for h in headline {
        let r = random
            .iter()
            .find(|m| m.seed == h.seed)
            .expect("matched random seed");
        eprintln!(
            "[random margin seed {}] headline_core={} random_core={} random_share={:.2}",
            h.seed,
            h.persistent_committed_cohort,
            r.persistent_committed_cohort,
            r.adopter_share()
        );
    }
}

#[test]
fn no_imitation_seed_only_no_core() {
    for m in no_imitation_metrics() {
        assert_guards(m, "no_imitation");
        assert_eq!(
            m.final_adopters, m.seed_alive_final,
            "without imitation the bit must remain seed-only among survivors (seed {})",
            m.seed
        );
        assert_eq!(m.nonseed_adopters, 0, "seed-only run spread the norm");
        assert!(
            !m.core_clause(),
            "seed alone must not reproduce the core (seed {})",
            m.seed
        );
    }
}

#[test]
fn no_seed_norm_never_appears() {
    for m in no_seed_metrics() {
        assert_guards(m, "no_seed");
        assert_eq!(m.seed_adopters, 0, "no_seed starts with no seed");
        assert_eq!(m.final_adopters, 0, "no_seed invented the norm");
        assert_eq!(m.adoptions, 0, "no_seed copied an absent norm");
        assert_eq!(m.abandonments, 0, "no_seed abandoned an absent norm");
    }
}

#[test]
fn unprofitable_commitment_kills_group_alignment() {
    let unprofitable = unprofitable_metrics();
    for m in unprofitable {
        assert_guards(m, "unprofitable_commitment");
        eprintln!("[unprofitable seed {}] {:?}", m.seed, m);
        assert_eq!(
            m.aligned_group_adoptions, 0,
            "unprofitable commitment left aligned group adoption (seed {})",
            m.seed
        );
        assert!(
            m.nonseed_adopters < MIN_NONSEED_ADOPTERS,
            "unprofitable commitment spread to non-seed adopters (seed {})",
            m.seed
        );
        assert!(
            !m.core_clause(),
            "unprofitable commitment formed a core (seed {})",
            m.seed
        );
    }
    // No copy is ever aligned (asserted above), so a lone seed's raw welfare/adopter-share
    // covariance is unsigned background noise across the observed groups, not a signal — it
    // flips sign seed to seed with no aligned copy behind it. Average across the seed panel
    // (the alignment classifier's own unit of evidence) rather than gating each noisy draw.
    let mean_covariance = unprofitable
        .iter()
        .map(|m| m.welfare_adopter_covariance)
        .sum::<f64>()
        / unprofitable.len() as f64;
    eprintln!("[unprofitable] mean welfare/adopter covariance = {mean_covariance:.2}");
    assert!(
        mean_covariance <= 0.0,
        "unprofitable commitment left positive mean welfare/adopter covariance ({mean_covariance:.2})"
    );
}

#[test]
fn salt_in_score_is_sensitivity_not_headline_upgrade() {
    let no_imitation = no_imitation_metrics();
    let random = random_metrics();
    let individual = individual_metrics();
    let unprofitable = unprofitable_metrics();
    let headline = headline_metrics();
    let salt = salt_metrics();
    for m in salt {
        assert!(m.conserved, "[salt_in_score] conservation broke");
        assert_eq!(m.bread_minted_max, 0, "[salt_in_score] bread minted");
        assert!(!m.extinct, "[salt_in_score] extinct");
        assert!(m.provenance_clean, "[salt_in_score] provenance broke");
        assert!(m.promoted, "[salt_in_score] SALT did not promote");
        eprintln!("[salt seed {}] {:?}", m.seed, m);
    }
    let headline_clean = headline
        .iter()
        .filter(|m| {
            m.verdict(no_imitation, random, individual, unprofitable)
                == Verdict::CleanInstitutionSpread
        })
        .count();
    let salt_clean = salt
        .iter()
        .filter(|m| {
            m.verdict(no_imitation, random, individual, unprofitable)
                == Verdict::CleanInstitutionSpread
        })
        .count();
    if headline_clean < 3 && salt_clean >= 3 {
        eprintln!(
            "salt-only success is classified as {:?}",
            Verdict::WealthProxySelection
        );
    }
    assert!(
        salt.iter()
            .any(|m| m.copy_events > 0 || m.covariance_samples > 0),
        "the SALT sensitivity must exercise the group observation path"
    );
}

#[test]
fn seed_cluster_only_check_requires_nonseed_participation() {
    let no_imitation = no_imitation_metrics();
    let random = random_metrics();
    let individual = individual_metrics();
    let unprofitable = unprofitable_metrics();
    for m in seed_cluster_only_metrics() {
        assert_guards(m, "seed_cluster_only");
        eprintln!("[seed_cluster_only seed {}] {:?}", m.seed, m);
        assert_eq!(
            m.nonseed_adopters, 0,
            "seed-cluster-only control spread beyond the seed"
        );
        assert!(
            m.verdict(no_imitation, random, individual, unprofitable)
                != Verdict::CleanInstitutionSpread,
            "seed-cluster-only control counted as clean success"
        );
    }
}

fn classifier_fixture(seed: u64) -> Metrics {
    Metrics {
        seed,
        group_on: true,
        conserved: true,
        bread_minted_max: 0,
        extinct: false,
        provenance_clean: true,
        promoted: true,
        adoption_invariant: true,
        seed_adopters: 10,
        seed_alive_final: 10,
        final_adopters: 10,
        nonseed_adopters: 0,
        nonseed_commits: 0,
        nonseed_renewed: 0,
        nonseed_core: 0,
        persistent_committed_cohort: PERSIST_COHORT,
        adopter_core: PERSIST_COHORT,
        all_persistent_renewed: true,
        final_buyer_cohort: MIN_BUYER_COHORT,
        post_promotion_bought: MATERIAL_BOUGHT_FLOOR,
        positive_group_copy_advantages: 1,
        aligned_group_adoptions: 1,
        copy_events: 1,
        group_copy_events: 1,
        generic_copy_drivers: 1,
        salt_copy_drivers: 0,
        score_purity_guard: true,
        group_score_purity_guard: true,
        welfare_adopter_covariance: 1.0,
        covariance_samples: 1,
        adoptions: 0,
        abandonments: 0,
        final_window_flips: 0,
        final_window_share_variance: 0.0,
        living: 50,
        starvation: 0,
    }
}

#[test]
fn classifier_reaches_seed_cluster_only_branch() {
    let seed = 4242;
    let headline = classifier_fixture(seed);
    let random = Metrics {
        persistent_committed_cohort: 0,
        final_adopters: 0,
        ..classifier_fixture(seed)
    };
    assert_eq!(
        headline.verdict(&[], &[random], &[], &[]),
        Verdict::SeedClusterOnly,
        "a real group signal plus a seed-only core must classify as SeedClusterOnly before the final gate"
    );
}

#[test]
fn non_adopters_cannot_commit_and_abandonment_waits_for_expiry() {
    let mut s = Settlement::generate(3, &headline_cfg());
    for _ in 0..PROBE_TICKS {
        s.econ_tick();
        for i in 0..s.population() {
            assert!(
                s.adopts_commitment_norm_of(i) || !s.is_committed(i),
                "non-adopter committed at tick {} index {}",
                s.econ_tick_count(),
                i
            );
            if s.is_committed(i) {
                assert!(
                    s.pending_commitment_norm_bit_of(i).is_none() || s.adopts_commitment_norm_of(i),
                    "pending abandonment broke a binding term at tick {} index {}",
                    s.econ_tick_count(),
                    i
                );
            } else {
                assert!(
                    s.pending_commitment_norm_bit_of(i).is_none(),
                    "unbound agent retained a staged norm bit at tick {} index {}",
                    s.econ_tick_count(),
                    i
                );
            }
        }
    }
}

#[test]
fn goldens_unchanged() {
    let digest = |cfg: &SettlementConfig, ticks: u64| {
        let mut s = Settlement::generate(1, cfg);
        s.run(ticks);
        s.digest()
    };
    assert_eq!(
        digest(&SettlementConfig::lineages(), 300),
        0x2335e13c809749fc
    );
    assert_eq!(
        digest(&SettlementConfig::lineages(), 800),
        0x3ffd78e50842d934
    );
    assert_eq!(
        digest(&SettlementConfig::frontier(), 300),
        0xcc83bf2669f0980d
    );
    assert_eq!(
        digest(&SettlementConfig::frontier_spatial_households(), 300),
        0xf30e3ce9345a73b3
    );
    let mut viable = Settlement::generate(0xC0FFEE, &SettlementConfig::viable());
    viable.run(60);
    assert_eq!(viable.digest(), 0xa174_8567_db1c_4341);
}

#[test]
fn robustness_axes_are_outcome_driving() {
    let mut low_seed = headline_cfg();
    if let Some(chain) = low_seed.chain.as_mut() {
        chain.commitment_seed_share_bps = LOW_SEED_SHARE_BPS;
    }
    let mut high_seed = headline_cfg();
    if let Some(chain) = high_seed.chain.as_mut() {
        chain.commitment_seed_share_bps = HIGH_SEED_SHARE_BPS;
    }
    let mut tiny_margin = headline_cfg();
    if let Some(chain) = tiny_margin.chain.as_mut() {
        chain.imitation_margin_bps = TINY_MARGIN_BPS;
    }
    let mut huge_margin = headline_cfg();
    if let Some(chain) = huge_margin.chain.as_mut() {
        chain.imitation_margin_bps = HUGE_MARGIN_BPS;
    }
    let mut small_radius = headline_cfg();
    if let Some(chain) = small_radius.chain.as_mut() {
        chain.imitation_radius = SMALL_GROUP_RADIUS;
    }
    let mut large_radius = headline_cfg();
    if let Some(chain) = large_radius.chain.as_mut() {
        chain.imitation_radius = LARGE_GROUP_RADIUS;
    }

    let no_imitation = no_imitation_metrics();
    let random = random_metrics();
    let individual = individual_metrics();
    let unprofitable = unprofitable_metrics();
    let default = run_all(headline_cfg());
    let low_seed = run_all(low_seed);
    let high_seed = run_all(high_seed);
    let tiny_margin = run_all(tiny_margin);
    let huge_margin = run_all(huge_margin);
    let small_radius = run_all(small_radius);
    let large_radius = run_all(large_radius);
    for (label, xs) in [
        ("default", default.as_slice()),
        ("low_seed", low_seed.as_slice()),
        ("high_seed", high_seed.as_slice()),
        ("tiny_margin", tiny_margin.as_slice()),
        ("huge_margin", huge_margin.as_slice()),
        ("small_radius", small_radius.as_slice()),
        ("large_radius", large_radius.as_slice()),
    ] {
        for m in xs {
            assert_guards(m, label);
            eprintln!(
                "[robustness {label} seed {}] {}",
                m.seed,
                m.line(no_imitation, random, individual, unprofitable)
            );
        }
    }
    let mean =
        |xs: &[Metrics], f: fn(&Metrics) -> f64| xs.iter().map(f).sum::<f64>() / xs.len() as f64;
    let adopters = |m: &Metrics| m.final_adopters as f64;
    let core = |m: &Metrics| m.persistent_committed_cohort as f64;
    let copies = |m: &Metrics| m.group_copy_events as f64;
    let cov = |m: &Metrics| m.welfare_adopter_covariance;
    eprintln!(
        "[robustness means] adopters default={:.1} low_seed={:.1} high_seed={:.1} \
         tiny_margin={:.1} huge_margin={:.1} small_radius={:.1} large_radius={:.1} | \
         core default={:.1} large_radius={:.1} | copies default={:.1} huge_margin={:.1} | \
         cov default={:.2} large_radius={:.2}",
        mean(&default, adopters),
        mean(&low_seed, adopters),
        mean(&high_seed, adopters),
        mean(&tiny_margin, adopters),
        mean(&huge_margin, adopters),
        mean(&small_radius, adopters),
        mean(&large_radius, adopters),
        mean(&default, core),
        mean(&large_radius, core),
        mean(&default, copies),
        mean(&huge_margin, copies),
        mean(&default, cov),
        mean(&large_radius, cov),
    );
    let seed_adopters = |m: &Metrics| m.seed_adopters as f64;
    assert!(
        mean(&high_seed, seed_adopters) > mean(&low_seed, seed_adopters),
        "seed-share axis must change the seeded adopter count"
    );
    // Group-payoff copying can push the seeded minority to abandon (the group-level echo of
    // S24b's tragedy of imitation), so a larger seed does not guarantee a larger FINAL adopter
    // share — more seeded adopters means more of them get compared and copied away. The axis
    // still must be outcome-driving: it must materially move at least one downstream observable
    // (final adopters or how many group copies fire), just not necessarily upward.
    assert!(
        (mean(&high_seed, adopters) - mean(&low_seed, adopters)).abs() >= 0.5
            || (mean(&high_seed, copies) - mean(&low_seed, copies)).abs() >= 1.0,
        "seed-share axis must be outcome-driving"
    );
    let any_group_copy = [
        default.as_slice(),
        tiny_margin.as_slice(),
        huge_margin.as_slice(),
        small_radius.as_slice(),
        large_radius.as_slice(),
    ]
    .into_iter()
    .flatten()
    .any(|m| m.group_copy_events > 0);
    if any_group_copy {
        assert!(
            (mean(&tiny_margin, adopters) - mean(&huge_margin, adopters)).abs() >= 0.5
                || (mean(&tiny_margin, copies) - mean(&huge_margin, copies)).abs() >= 1.0,
            "margin axis must be outcome-driving once group copies fire"
        );
        assert!(
            (mean(&small_radius, adopters) - mean(&large_radius, adopters)).abs() >= 0.5
                || (mean(&small_radius, cov) - mean(&large_radius, cov)).abs() >= 1.0,
            "group-radius axis must be outcome-driving once group copies fire"
        );
    } else {
        eprintln!(
            "[robustness] margin/radius axes are inert because the classified regime is \
             GroupSignalVacuous: no group-gradient copy fired in any swept cell"
        );
    }
}
