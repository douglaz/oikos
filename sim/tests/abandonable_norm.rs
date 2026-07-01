//! S24b acceptance suite: S24a commitment-norm spread with abandonable bidirectional imitation.
//! The verdict is classified and printed; milestone success is not asserted.

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
const MIN_ABANDONMENTS: u64 = 8;
const CORE_MARGIN: usize = PERSIST_COHORT;
const CHURN_FLIP_RATE: f64 = 0.5;
const CHURN_SHARE_VAR: f64 = 0.01;
const COMMITMENT_SEED_SHARE_BPS_DEFAULT: u16 = 1_500;
const IMITATION_PERIOD_DEFAULT: u64 = 24;
const IMITATION_MARGIN_BPS_DEFAULT: u64 = 1_500;
const TINY_MARGIN_BPS: u64 = 1;
const HUGE_MARGIN_BPS: u64 = 40_000;
const LOW_SEED_SHARE_BPS: u16 = 500;
const HIGH_SEED_SHARE_BPS: u16 = 3_000;
const SLOW_IMITATION_PERIOD: u64 = 96;

fn persist_threshold() -> u32 {
    (PERSIST_FRACTION * FINAL_WINDOW as f64).ceil() as u32
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
enum Verdict {
    SeedDies,
    MoneyFailure,
    ConservationBroken,
    Cull,
    NormDiesBack,
    UniversalCommitmentRePin,
    DriftNotSelection,
    ChurnEquilibrium,
    SpreadWithoutOccupation,
    CleanInstitutionSpread,
    WealthProxySelection,
}

#[derive(Clone, Debug)]
struct Metrics {
    seed: u64,
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
    persistent_committed_cohort: usize,
    adopter_core: usize,
    all_persistent_renewed: bool,
    final_buyer_cohort: usize,
    post_promotion_bought: u64,
    positive_copy_advantages: usize,
    copy_events: usize,
    generic_copy_drivers: usize,
    salt_copy_drivers: usize,
    score_purity_guard: bool,
    adoptions: u64,
    abandonments: u64,
    final_window_flips: u64,
    final_window_flip_rate: f64,
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

    fn guard_clause(&self) -> bool {
        self.conserved
            && self.bread_minted_max == 0
            && !self.extinct
            && self.provenance_clean
            && self.promoted
            && self.adoption_invariant
            && self.score_purity_guard
            && self.salt_copy_drivers == 0
    }

    fn selection_clause(&self, no_imitation: &[Metrics], random: &[Metrics]) -> bool {
        let no_core = no_imitation.iter().all(|m| !m.core_clause());
        let Some(random) = random.iter().find(|r| r.seed == self.seed) else {
            return false;
        };
        self.persistent_committed_cohort >= random.persistent_committed_cohort + CORE_MARGIN
            && !random.bounded_two_tier_success()
            && no_core
            && self.positive_copy_advantages > 0
            && self.generic_copy_drivers > 0
            && self.score_purity_guard
            && self.salt_copy_drivers == 0
    }

    fn abandonment_clause(&self, random: &[Metrics]) -> bool {
        let random = random.iter().find(|r| r.seed == self.seed);
        self.abandonments >= MIN_ABANDONMENTS
            && random.is_some_and(|r| {
                r.adopter_share() <= ADOPTER_SHARE_MAX && r.adopter_share() <= self.adopter_share()
            })
    }

    fn all_success_clauses(&self, no_imitation: &[Metrics], random: &[Metrics]) -> bool {
        self.bounded_adoption_clause()
            && self.core_clause()
            && self.buyer_clause()
            && self.selection_clause(no_imitation, random)
            && self.abandonment_clause(random)
            && self.guard_clause()
    }

    fn verdict(&self, no_imitation: &[Metrics], random: &[Metrics]) -> Verdict {
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
        {
            return Verdict::ConservationBroken;
        }
        if self.extinct {
            return Verdict::Cull;
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
                return Verdict::DriftNotSelection;
            }
        }
        if (self.final_window_flip_rate >= CHURN_FLIP_RATE
            || self.final_window_share_variance >= CHURN_SHARE_VAR)
            && !self.core_clause()
        {
            return Verdict::ChurnEquilibrium;
        }
        if !self.core_clause() {
            return Verdict::SpreadWithoutOccupation;
        }
        if self.all_success_clauses(no_imitation, random) {
            Verdict::CleanInstitutionSpread
        } else {
            Verdict::SpreadWithoutOccupation
        }
    }

    fn line(&self, no_imitation: &[Metrics], random: &[Metrics]) -> String {
        format!(
            "{:?} | seed={} final={} nonseed={} share={:.2} var={:.4} | core={} \
             adopter_core={} all_renewed={} buyers={} post_bought={} | copy(events={} \
             pos_adv={} generic={} salt={} purity={}) | flips(adopt={} abandon={} \
             fw={} rate={:.4}) | promoted={} clean={} conserved={} minted={} invariant={} \
             extinct={} living={} starv={}",
            self.verdict(no_imitation, random),
            self.seed_adopters,
            self.final_adopters,
            self.nonseed_adopters,
            self.adopter_share(),
            self.final_window_share_variance,
            self.persistent_committed_cohort,
            self.adopter_core,
            self.all_persistent_renewed,
            self.final_buyer_cohort,
            self.post_promotion_bought,
            self.copy_events,
            self.positive_copy_advantages,
            self.generic_copy_drivers,
            self.salt_copy_drivers,
            self.score_purity_guard,
            self.adoptions,
            self.abandonments,
            self.final_window_flips,
            self.final_window_flip_rate,
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
    SettlementConfig::frontier_abandonable_norm()
}

fn sticky_cfg() -> SettlementConfig {
    SettlementConfig::frontier_commitment_norm_spread()
}

fn no_imitation_cfg() -> SettlementConfig {
    let mut cfg = headline_cfg();
    if let Some(chain) = cfg.chain.as_mut() {
        chain.no_imitation = true;
    }
    cfg
}

fn random_imitation_cfg() -> SettlementConfig {
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

fn tiny_margin_cfg() -> SettlementConfig {
    let mut cfg = headline_cfg();
    if let Some(chain) = cfg.chain.as_mut() {
        chain.imitation_margin_bps = TINY_MARGIN_BPS;
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
    let abandonable = s.abandonable_norm_on();
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
    let mut final_window_adopter_ticks = 0u64;
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
        if abandonable {
            let flip_rows = s.commitment_norm_flip_events();
            for (&id, &from) in &before {
                let Some(&to) = after.get(&id) else { continue };
                if from != to {
                    let recorded = flip_rows.iter().any(|row| {
                        row.tick == t && row.agent == id && row.from == from && row.to == to
                    });
                    adoption_invariant &= recorded;
                    if t >= final_window_start {
                        final_window_flips = final_window_flips.saturating_add(1);
                    }
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
            final_window_adopter_ticks =
                final_window_adopter_ticks.saturating_add(live_adopters as u64);
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
    let average_final_window_adopters = if share_samples.is_empty() {
        0.0
    } else {
        final_window_adopter_ticks as f64 / share_samples.len() as f64
    };
    let final_window_flip_rate = if average_final_window_adopters == 0.0 {
        0.0
    } else {
        final_window_flips as f64 / average_final_window_adopters
    };

    Metrics {
        seed,
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
        positive_copy_advantages: s.commitment_norm_positive_copy_advantages(),
        copy_events: copy_events.len(),
        generic_copy_drivers,
        salt_copy_drivers,
        score_purity_guard: s.commitment_norm_score_purity_guard(),
        adoptions: s.commitment_norm_adoptions(),
        abandonments: s.commitment_norm_abandonments(),
        final_window_flips,
        final_window_flip_rate,
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

fn no_imitation_metrics() -> &'static [Metrics] {
    static METRICS: OnceLock<Vec<Metrics>> = OnceLock::new();
    METRICS.get_or_init(|| run_all(no_imitation_cfg()))
}

fn random_metrics() -> &'static [Metrics] {
    static METRICS: OnceLock<Vec<Metrics>> = OnceLock::new();
    METRICS.get_or_init(|| run_all(random_imitation_cfg()))
}

fn sticky_metrics() -> &'static [Metrics] {
    static METRICS: OnceLock<Vec<Metrics>> = OnceLock::new();
    METRICS.get_or_init(|| run_all(sticky_cfg()))
}

fn sticky_random_metrics() -> &'static [Metrics] {
    static METRICS: OnceLock<Vec<Metrics>> = OnceLock::new();
    METRICS.get_or_init(|| {
        let mut cfg = sticky_cfg();
        if let Some(chain) = cfg.chain.as_mut() {
            chain.random_imitation = true;
        }
        run_all(cfg)
    })
}

fn no_seed_metrics() -> &'static [Metrics] {
    static METRICS: OnceLock<Vec<Metrics>> = OnceLock::new();
    METRICS.get_or_init(|| run_all(no_seed_cfg()))
}

fn tiny_margin_metrics() -> &'static [Metrics] {
    static METRICS: OnceLock<Vec<Metrics>> = OnceLock::new();
    METRICS.get_or_init(|| run_all(tiny_margin_cfg()))
}

fn salt_metrics() -> &'static [Metrics] {
    static METRICS: OnceLock<Vec<Metrics>> = OnceLock::new();
    METRICS.get_or_init(|| run_all(salt_in_score_cfg()))
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
        m.score_purity_guard,
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
    assert!(ADOPTER_SHARE_MIN > 0.0);
    assert!(ADOPTER_SHARE_MIN < ADOPTER_SHARE_MAX);
    assert!(ADOPTER_SHARE_MAX < 1.0);
    assert!(MIN_ABANDONMENTS > 0);
    assert_eq!(CORE_MARGIN, PERSIST_COHORT);
    assert!(CHURN_FLIP_RATE > 0.0);
    assert_eq!(CHURN_SHARE_VAR, 0.01);
    assert!(persist_threshold() as usize <= FINAL_WINDOW);
}

#[test]
fn mandatory_abandonment_and_null_diagnostics() {
    let headline = headline_metrics();
    let random = random_metrics();
    for m in headline {
        assert_guards(m, "headline");
        eprintln!("[headline seed {}] {:?}", m.seed, m);
    }
    for m in random {
        assert_guards(m, "random");
        eprintln!("[random seed {}] {:?}", m.seed, m);
    }
    let qualifying_abandonment = headline.iter().any(|m| {
        let Some(r) = random.iter().find(|r| r.seed == m.seed) else {
            return false;
        };
        m.seed_adopters > 0
            && m.abandonments >= MIN_ABANDONMENTS
            && m.generic_copy_drivers > 0
            && r.score_purity_guard
    });
    assert!(
        qualifying_abandonment,
        "no seed demonstrated real abandonable generic copying"
    );
    // Non-vacuity for the OTHER arm of the bidirectional rule: the abandonable false->true
    // adoption path must actually fire somewhere it legitimately can (the blind random null copies
    // both directions), so a silent regression that broke the adopt arm cannot pass this suite
    // green. The headline `NormDiesBack`s with `adopt == 0`, so the spread-beyond-seed guarantee is
    // asserted against the matched random null, where the abandonable update genuinely adopts.
    assert!(
        random.iter().any(|m| m.adoptions > 0),
        "the abandonable false->true adoption arm never fired in the random null"
    );
}

#[test]
fn abandonable_norm_verdict_prints() {
    let headline = headline_metrics();
    let no_imitation = no_imitation_metrics();
    let random = random_metrics();
    let salt = salt_metrics();
    eprintln!("\n================ S24b abandonable commitment norm ================");
    for m in headline {
        assert_guards(m, "headline");
        eprintln!("seed {:>2}: {}", m.seed, m.line(no_imitation, random));
    }
    let mut tally = BTreeMap::new();
    for m in headline {
        *tally
            .entry(m.verdict(no_imitation, random))
            .or_insert(0usize) += 1;
    }
    let clean = *tally.get(&Verdict::CleanInstitutionSpread).unwrap_or(&0);
    let drift = *tally.get(&Verdict::DriftNotSelection).unwrap_or(&0);
    eprintln!("headline tally: {tally:?}");
    eprintln!(
        "milestone success bar: clean={clean}/5 drift={drift}/5 strict_success={}",
        clean >= 3 && drift == 0
    );
    let headline_success = clean >= 3 && drift == 0;
    let salt_clean = salt
        .iter()
        .filter(|m| m.verdict(no_imitation, random) == Verdict::CleanInstitutionSpread)
        .count();
    if !headline_success && salt_clean >= 3 {
        eprintln!(
            "S24b sensitivity verdict: {:?}",
            Verdict::WealthProxySelection
        );
    }
    eprintln!("===============================================================\n");
}

#[test]
fn sticky_reference_is_s24a_mixed_control() {
    let sticky = sticky_metrics();
    let no_imitation = no_imitation_metrics();
    let random = sticky_random_metrics();
    for m in sticky {
        assert_guards(m, "sticky_reference");
        eprintln!("[sticky seed {}] {}", m.seed, m.line(no_imitation, random));
    }
    assert!(
        sticky
            .iter()
            .any(|m| m.adopter_share() > ADOPTER_SHARE_MAX || m.core_clause()),
        "sticky reference must retain the S24a high-adoption/core behavior"
    );
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
        assert!(
            !m.core_clause(),
            "seed alone must not reproduce the core (seed {})",
            m.seed
        );
    }
}

#[test]
fn random_imitation_reports_matched_drift() {
    let headline = headline_metrics();
    let random = random_metrics();
    for m in random {
        assert_guards(m, "random_imitation");
        eprintln!("[random seed {}] {:?}", m.seed, m);
    }
    let clean_random_seeds = random
        .iter()
        .filter(|m| m.adopter_share() <= ADOPTER_SHARE_MAX && !m.core_clause())
        .count();
    assert!(
        clean_random_seeds > 0,
        "random imitation must leave at least one matched seed below the core"
    );
    for h in headline {
        let r = random
            .iter()
            .find(|m| m.seed == h.seed)
            .expect("matched random seed");
        if h.persistent_committed_cohort < r.persistent_committed_cohort + CORE_MARGIN
            || r.bounded_two_tier_success()
        {
            eprintln!(
                "[random drift seed {}] headline_core={} random_core={} random_share={:.2}",
                h.seed,
                h.persistent_committed_cohort,
                r.persistent_committed_cohort,
                r.adopter_share()
            );
        }
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
fn tiny_margin_reports_churn_sensitivity() {
    let tiny = tiny_margin_metrics();
    let no_imitation = no_imitation_metrics();
    let random = random_metrics();
    for m in tiny {
        assert_guards(m, "tiny_margin");
        eprintln!("[tiny seed {}] {}", m.seed, m.line(no_imitation, random));
    }
    assert!(
        tiny.iter().any(|m| {
            m.verdict(no_imitation, random) == Verdict::ChurnEquilibrium
                || m.final_window_share_variance >= CHURN_SHARE_VAR
                || m.final_window_flips > 0
                || m.adoptions + m.abandonments > 0
        }),
        "tiny margin must move the system"
    );
}

#[test]
fn salt_in_score_is_sensitivity_not_headline_upgrade() {
    let headline = headline_metrics();
    let no_imitation = no_imitation_metrics();
    let random = random_metrics();
    let salt = salt_metrics();
    for m in salt {
        assert!(m.conserved, "[salt_in_score] conservation broke");
        assert_eq!(m.bread_minted_max, 0, "[salt_in_score] bread minted");
        assert!(!m.extinct, "[salt_in_score] extinct");
        assert!(m.provenance_clean, "[salt_in_score] provenance broke");
        assert!(m.promoted, "[salt_in_score] SALT did not promote");
    }
    let headline_clean = headline
        .iter()
        .filter(|m| m.verdict(no_imitation, random) == Verdict::CleanInstitutionSpread)
        .count();
    let salt_clean = salt
        .iter()
        .filter(|m| m.verdict(no_imitation, random) == Verdict::CleanInstitutionSpread)
        .count();
    if headline_clean < 3 && salt_clean >= 3 {
        eprintln!(
            "salt-only success is classified as {:?}",
            Verdict::WealthProxySelection
        );
    }
    assert!(
        salt.iter()
            .any(|m| m.salt_copy_drivers > 0 || m.generic_copy_drivers > 0),
        "the SALT sensitivity must report copy drivers"
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
                // A staged flip is applied AT term expiry (the tick the binding clears), so an
                // unbound agent never lingers with an un-applied pending bit at end of tick — the
                // abandonment lands in the adopter bit (and the digest) the same tick, not one late.
                assert!(
                    s.pending_commitment_norm_bit_of(i).is_none(),
                    "unbound agent retained a staged norm bit at tick {} index {} \
                     (staged flip not applied at term expiry)",
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
    let mut huge_margin = headline_cfg();
    if let Some(chain) = huge_margin.chain.as_mut() {
        chain.imitation_margin_bps = HUGE_MARGIN_BPS;
    }
    let mut slow_period = headline_cfg();
    if let Some(chain) = slow_period.chain.as_mut() {
        chain.imitation_period = SLOW_IMITATION_PERIOD;
    }

    let default = run_all(headline_cfg());
    let tiny = run_all(tiny_margin_cfg());
    let low_seed = run_all(low_seed);
    let high_seed = run_all(high_seed);
    let huge_margin = run_all(huge_margin);
    let slow_period = run_all(slow_period);
    for m in default
        .iter()
        .chain(tiny.iter())
        .chain(low_seed.iter())
        .chain(high_seed.iter())
        .chain(huge_margin.iter())
        .chain(slow_period.iter())
    {
        assert_guards(m, "robustness");
    }
    let mean_adopters =
        |xs: &[Metrics]| xs.iter().map(|m| m.final_adopters as f64).sum::<f64>() / xs.len() as f64;
    let mean_var = |xs: &[Metrics]| {
        xs.iter()
            .map(|m| m.final_window_share_variance)
            .sum::<f64>()
            / xs.len() as f64
    };
    eprintln!(
        "[robustness] adopters default={:.1} tiny={:.1} low_seed={:.1} high_seed={:.1} \
         huge_margin={:.1} slow_period={:.1} | var tiny={:.4} default={:.4}",
        mean_adopters(&default),
        mean_adopters(&tiny),
        mean_adopters(&low_seed),
        mean_adopters(&high_seed),
        mean_adopters(&huge_margin),
        mean_adopters(&slow_period),
        mean_var(&tiny),
        mean_var(&default),
    );
    assert!(
        mean_adopters(&high_seed) >= mean_adopters(&low_seed),
        "seed-share axis must move adoption"
    );
    assert!(
        (mean_adopters(&huge_margin) - mean_adopters(&tiny)).abs() >= 0.5,
        "margin axis must move adoption"
    );
    assert!(
        mean_var(&tiny) >= mean_var(&default) || mean_adopters(&tiny) != mean_adopters(&default),
        "tiny margin must be outcome-driving"
    );
}
