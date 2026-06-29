//! S24a acceptance suite: endogenous spread of the S22f commitment norm by local imitation of
//! observed generic success. The verdict is classified and printed; it is not asserted as SUCCESS.

use std::collections::{BTreeMap, BTreeSet};

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
const ADOPTER_SHARE_MAX: f64 = 0.6;
const SPREAD_FACTOR: usize = 3;
const MIN_NONSEED_ADOPTERS: usize = 4;
const MIN_NONSEED_COMMITS: usize = 4;
const COMMITMENT_SEED_SHARE_BPS_DEFAULT: u16 = 1_500;
const IMITATION_PERIOD_DEFAULT: u64 = 24;
const IMITATION_MARGIN_BPS_DEFAULT: u64 = 1_500;
const TOO_LARGE_SEED_SHARE_BPS: u16 = 6_000;
const TOO_LARGE_MARGIN_BPS: u64 = 40_000;

fn persist_threshold() -> u32 {
    (PERSIST_FRACTION * FINAL_WINDOW as f64).ceil() as u32
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
enum Verdict {
    SeedDies,
    MoneyFailure,
    ConservationBroken,
    Cull,
    SeedOnlyNoSpread,
    UniversalCommitmentRePin,
    DriftNotSelection,
    WealthProxySelection,
    SpreadWithoutOccupation,
    InstitutionSpreadSuccess,
}

#[derive(Clone, Debug)]
struct Metrics {
    seed: u64,
    conserved: bool,
    bread_minted_max: u64,
    extinct: bool,
    provenance_clean: bool,
    promoted: bool,
    seed_adopters: usize,
    seed_alive_final: usize,
    final_adopters: usize,
    nonseed_adopters: usize,
    nonseed_commits: usize,
    nonseed_renewed: usize,
    persistent_committed_cohort: usize,
    all_persistent_renewed: bool,
    final_buyer_cohort: usize,
    post_promotion_bought: u64,
    positive_copy_advantages: usize,
    copy_events: usize,
    generic_copy_drivers: usize,
    salt_copy_drivers: usize,
    score_purity_guard: bool,
    living: usize,
    starvation: u64,
}

impl Metrics {
    fn adopter_share(&self) -> f64 {
        let denom = self.living.max(1) as f64;
        self.final_adopters as f64 / denom
    }

    fn spread_clause(&self) -> bool {
        self.seed_adopters > 0
            && self.final_adopters >= SPREAD_FACTOR * self.seed_adopters
            && self.nonseed_adopters >= MIN_NONSEED_ADOPTERS
    }

    fn nonseed_commit_clause(&self) -> bool {
        self.nonseed_commits >= MIN_NONSEED_COMMITS && self.nonseed_renewed >= 1
    }

    fn core_clause(&self) -> bool {
        self.persistent_committed_cohort >= PERSIST_COHORT && self.all_persistent_renewed
    }

    fn buyer_clause(&self) -> bool {
        self.final_buyer_cohort >= MIN_BUYER_COHORT
            && self.post_promotion_bought >= MATERIAL_BOUGHT_FLOOR
    }

    fn bounded_clause(&self) -> bool {
        self.adopter_share() <= ADOPTER_SHARE_MAX && self.buyer_clause()
    }

    fn guard_clause(&self) -> bool {
        self.conserved
            && self.bread_minted_max == 0
            && !self.extinct
            && self.provenance_clean
            && self.promoted
    }

    fn selection_clause(&self, no_imitation: &[Metrics], random: &[Metrics]) -> bool {
        let no_core = no_imitation.iter().all(|m| !m.core_clause());
        // PER-SEED drift check (Codex review-of-results P1): this seed's OWN matched
        // random-imitation run must not reproduce the spread+core. An aggregate
        // all-seeds check masks per-seed drift — under sticky adoption, outcome-blind
        // random copying ratchets the bit up and forms a core on most seeds, so a
        // headline "success" whose matched random also forms a core is drift, not
        // selection.
        let random_reproduces_core = random
            .iter()
            .find(|r| r.seed == self.seed)
            .is_some_and(|r| r.spread_clause() && r.core_clause());
        no_core
            && !random_reproduces_core
            && self.positive_copy_advantages > 0
            && self.generic_copy_drivers > 0
            && self.score_purity_guard
    }

    fn all_success_clauses(&self, no_imitation: &[Metrics], random: &[Metrics]) -> bool {
        self.spread_clause()
            && self.nonseed_commit_clause()
            && self.core_clause()
            && self.buyer_clause()
            && self.bounded_clause()
            && self.guard_clause()
            && self.selection_clause(no_imitation, random)
    }

    fn verdict(&self, no_imitation: &[Metrics], random: &[Metrics]) -> Verdict {
        if self.seed_adopters == 0 || self.seed_alive_final == 0 {
            return Verdict::SeedDies;
        }
        if !self.promoted {
            return Verdict::MoneyFailure;
        }
        if !self.conserved || self.bread_minted_max > 0 || !self.provenance_clean {
            return Verdict::ConservationBroken;
        }
        if self.extinct {
            return Verdict::Cull;
        }
        if !self.spread_clause() {
            return Verdict::SeedOnlyNoSpread;
        }
        if self.adopter_share() > ADOPTER_SHARE_MAX || !self.buyer_clause() {
            return Verdict::UniversalCommitmentRePin;
        }
        if random
            .iter()
            .find(|r| r.seed == self.seed)
            .is_some_and(|r| r.spread_clause() && r.core_clause())
        {
            return Verdict::DriftNotSelection;
        }
        if !self.core_clause() {
            return Verdict::SpreadWithoutOccupation;
        }
        if self.all_success_clauses(no_imitation, random) {
            Verdict::InstitutionSpreadSuccess
        } else {
            Verdict::SpreadWithoutOccupation
        }
    }

    fn line(&self, no_imitation: &[Metrics], random: &[Metrics]) -> String {
        format!(
            "{:?} | seed={} final={} nonseed={} share={:.2} | nonseed_commit={} renewed={} | \
             core={} all_renewed={} buyers={} post_bought={} | copy(events={} pos_adv={} \
             generic={} salt={} purity={}) | promoted={} clean={} conserved={} minted={} \
             extinct={} living={} starv={}",
            self.verdict(no_imitation, random),
            self.seed_adopters,
            self.final_adopters,
            self.nonseed_adopters,
            self.adopter_share(),
            self.nonseed_commits,
            self.nonseed_renewed,
            self.persistent_committed_cohort,
            self.all_persistent_renewed,
            self.final_buyer_cohort,
            self.post_promotion_bought,
            self.copy_events,
            self.positive_copy_advantages,
            self.generic_copy_drivers,
            self.salt_copy_drivers,
            self.score_purity_guard,
            self.promoted,
            self.provenance_clean,
            self.conserved,
            self.bread_minted_max,
            self.extinct,
            self.living,
            self.starvation,
        )
    }
}

fn norm_cfg() -> SettlementConfig {
    SettlementConfig::frontier_commitment_norm_spread()
}

fn no_imitation_cfg() -> SettlementConfig {
    let mut cfg = norm_cfg();
    if let Some(chain) = cfg.chain.as_mut() {
        chain.no_imitation = true;
    }
    cfg
}

fn random_imitation_cfg() -> SettlementConfig {
    let mut cfg = norm_cfg();
    if let Some(chain) = cfg.chain.as_mut() {
        chain.random_imitation = true;
    }
    cfg
}

fn no_seed_cfg() -> SettlementConfig {
    let mut cfg = norm_cfg();
    if let Some(chain) = cfg.chain.as_mut() {
        chain.commitment_seed_share_bps = 0;
    }
    cfg
}

fn unprofitable_seed_cfg() -> SettlementConfig {
    let mut cfg = norm_cfg();
    if let Some(chain) = cfg.chain.as_mut() {
        chain.commitment_term = 1;
    }
    cfg
}

fn salt_in_score_cfg() -> SettlementConfig {
    let mut cfg = norm_cfg();
    if let Some(chain) = cfg.chain.as_mut() {
        chain.salt_in_score = true;
    }
    cfg
}

fn run_metrics(seed: u64, cfg: &SettlementConfig, ticks: u64) -> Metrics {
    let mut s = Settlement::generate(seed, cfg);
    let bread = s.bread_good();
    let seed_ids: BTreeSet<u64> = s.commitment_norm_seed_adopter_ids().into_iter().collect();

    let mut conserved = true;
    let mut bread_minted_max = 0u64;
    let mut promoted = false;
    let mut bought_at_promotion = None;
    let final_window_start = ticks.saturating_sub(FINAL_WINDOW as u64);
    let mut fw_persist: BTreeMap<u64, u32> = BTreeMap::new();
    let mut buyer_samples = Vec::with_capacity(ticks as usize);

    for t in 0..ticks {
        let report = s.econ_tick();
        conserved &= report.conserves();
        if let Some(bread) = bread {
            bread_minted_max = bread_minted_max.max(report.endowment_of(bread));
        }
        if !promoted && s.current_money_good() == Some(SALT) {
            promoted = true;
            bought_at_promotion = Some(s.acquisition_consumed_by_channel().bought);
        }
        let mut buyers = 0usize;
        for i in 0..s.population() {
            if !s.is_alive(i) {
                continue;
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
        buyer_samples.push(buyers);
    }

    let committed_ever: BTreeSet<u64> = s.commitment_committed_ids().into_iter().collect();
    let uptake: BTreeSet<u64> = s.commitment_uptake().keys().copied().collect();
    let final_adopters: BTreeSet<u64> = s.commitment_norm_adopter_ids().into_iter().collect();
    let nonseed_adopters = final_adopters.difference(&seed_ids).count();
    let nonseed_commits = uptake.difference(&seed_ids).count();

    let id_to_index: BTreeMap<u64, usize> = (0..s.population())
        .filter_map(|i| s.colonist_id(i).map(|id| (id.0, i)))
        .collect();
    let mut persistent_committed_cohort = 0usize;
    let mut all_persistent_renewed = true;
    let mut nonseed_renewed_ids = BTreeSet::new();
    for (&id, &count) in &fw_persist {
        if !committed_ever.contains(&id) {
            continue;
        }
        let renewals = id_to_index
            .get(&id)
            .map_or(0, |&i| s.commitment_renewals_of(i));
        if renewals > 0 && !seed_ids.contains(&id) {
            nonseed_renewed_ids.insert(id);
        }
        if count >= persist_threshold() {
            persistent_committed_cohort += 1;
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

    Metrics {
        seed,
        conserved,
        bread_minted_max,
        extinct: living(&s) == 0,
        provenance_clean,
        promoted: s.current_money_good() == Some(SALT),
        seed_adopters: seed_ids.len(),
        seed_alive_final: (0..s.population())
            .filter(|&i| {
                s.is_alive(i) && s.colonist_id(i).is_some_and(|id| seed_ids.contains(&id.0))
            })
            .count(),
        final_adopters: final_adopters.len(),
        nonseed_adopters,
        nonseed_commits,
        nonseed_renewed: nonseed_renewed_ids.len(),
        persistent_committed_cohort,
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
}

#[test]
#[allow(clippy::assertions_on_constants)] // intentional compile-time config-invariant sanity checks
fn coherence_constants_are_well_formed() {
    assert!(COMMITMENT_SEED_SHARE_BPS_DEFAULT > 0);
    assert!(COMMITMENT_SEED_SHARE_BPS_DEFAULT < 10_000);
    assert!(IMITATION_PERIOD_DEFAULT > 0);
    assert!(IMITATION_MARGIN_BPS_DEFAULT > 0);
    assert!(TOO_LARGE_SEED_SHARE_BPS as f64 / 10_000.0 >= ADOPTER_SHARE_MAX);
    assert!(TOO_LARGE_MARGIN_BPS >= 4 * 10_000);
    assert!(persist_threshold() as usize <= FINAL_WINDOW);
}

#[test]
fn mandatory_non_vacuity_and_anti_oracle() {
    let headline = run_all(norm_cfg());
    let no_imitation = run_all(no_imitation_cfg());
    let random = run_all(random_imitation_cfg());
    for m in &headline {
        assert_guards(m, "non_vacuity");
        assert!(
            m.seed_adopters > 0,
            "seed must be nonzero (seed {})",
            m.seed
        );
        assert!(
            m.seed_adopters < m.final_adopters,
            "final adopters must exceed the seed (seed {}: seed={} final={})",
            m.seed,
            m.seed_adopters,
            m.final_adopters
        );
        assert!(
            m.nonseed_adopters >= MIN_NONSEED_ADOPTERS,
            "non-seed adopters must be material (seed {}: {})",
            m.seed,
            m.nonseed_adopters
        );
        assert!(
            m.nonseed_commits >= MIN_NONSEED_COMMITS,
            "non-seed adopters must enter real commitments (seed {}: {})",
            m.seed,
            m.nonseed_commits
        );
        assert!(
            m.generic_copy_drivers > 0,
            "a generic observable must drive at least one copy (seed {})",
            m.seed
        );
        assert!(
            m.score_purity_guard,
            "score purity guard failed (seed {})",
            m.seed
        );
        eprintln!(
            "[headline seed {}] {}",
            m.seed,
            m.line(&no_imitation, &random)
        );
    }
}

#[test]
fn commitment_norm_verdict_prints() {
    let headline = run_all(norm_cfg());
    let no_imitation = run_all(no_imitation_cfg());
    let random = run_all(random_imitation_cfg());
    let salt = run_all(salt_in_score_cfg());
    eprintln!("\n================ S24a commitment norm spread ================");
    for m in &headline {
        eprintln!("seed {:>2}: {}", m.seed, m.line(&no_imitation, &random));
    }
    let headline_success = headline
        .iter()
        .all(|m| m.verdict(&no_imitation, &random) == Verdict::InstitutionSpreadSuccess);
    let salt_success = salt
        .iter()
        .all(|m| m.verdict(&no_imitation, &random) == Verdict::InstitutionSpreadSuccess);
    if !headline_success && salt_success {
        eprintln!(
            "S24a sensitivity verdict: {:?}",
            Verdict::WealthProxySelection
        );
    }
    let mut tally = BTreeMap::new();
    for m in &headline {
        *tally
            .entry(m.verdict(&no_imitation, &random))
            .or_insert(0usize) += 1;
    }
    eprintln!("headline tally: {tally:?}");
    eprintln!("============================================================\n");
}

#[test]
fn global_commitment_on_positive_control_can_form_core() {
    let ms = run_all(SettlementConfig::frontier_voluntary_commitment());
    for m in &ms {
        assert_guards(m, "global_commitment_on");
    }
    assert!(
        ms.iter().any(Metrics::core_clause),
        "the supplied S22f positive control must be able to form a committed core"
    );
}

#[test]
fn no_imitation_seed_only_no_core() {
    for m in run_all(no_imitation_cfg()) {
        assert_guards(&m, "no_imitation");
        assert_eq!(
            m.final_adopters, m.seed_adopters,
            "without imitation the bit must remain seed-only (seed {})",
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
fn random_imitation_drift_is_seed_dependent() {
    // Codex review-of-results P1: under STICKY adoption, outcome-blind random copying
    // ratchets the norm bit up and reproduces the committed core on SOME seeds (here
    // 3/5), so the drift null is contaminated. This is reported honestly: random must
    // not reproduce the core on EVERY seed (else the whole result is pure drift), and
    // the PER-SEED verdict (selection_clause / DriftNotSelection) downgrades any
    // headline seed whose matched random run also forms a core. The per-seed random
    // cores are printed so the contamination is visible.
    let ms = run_all(random_imitation_cfg());
    for m in &ms {
        assert_guards(m, "random_imitation");
        eprintln!("[random seed {}] {:?}", m.seed, m);
    }
    assert!(
        !ms.iter().all(|m| m.spread_clause() && m.core_clause()),
        "random imitation must not reproduce spread+core on EVERY seed (else pure drift)"
    );
    let drift_seeds: Vec<u64> = ms
        .iter()
        .filter(|m| m.spread_clause() && m.core_clause())
        .map(|m| m.seed)
        .collect();
    eprintln!("[random] drift-contaminated seeds (random forms a core): {drift_seeds:?}");
}

#[test]
fn no_seed_norm_never_appears() {
    for m in run_all(no_seed_cfg()) {
        assert_guards(&m, "no_seed");
        assert_eq!(
            m.seed_adopters, 0,
            "no_seed starts with no seed (seed {})",
            m.seed
        );
        assert_eq!(
            m.final_adopters, 0,
            "no_seed must not invent the norm (seed {})",
            m.seed
        );
        assert_eq!(m.nonseed_commits, 0, "no_seed cannot commit via the norm");
    }
}

#[test]
fn unprofitable_seed_spreads_bit_via_food_advantage_but_forms_no_core() {
    // Codex review-of-results P1: the spec intended unprofitable_seed (commitment_term=1,
    // a non-binding commitment that confers no stickiness benefit) to remove BOTH the
    // advantage AND the spread. The data shows otherwise and we report it honestly: even
    // a 1-tick commitment lets an adopter cultivate and eat its own bread once, so adopters
    // still hold a transient GENERIC food/hunger advantage and the norm BIT still spreads —
    // but the non-binding institution forms NO committed occupation core. So spread of the
    // bit alone is NOT institution selection: the target occupation requires a binding term.
    for m in run_all(unprofitable_seed_cfg()) {
        assert_guards(&m, "unprofitable_seed");
        eprintln!("[unprofitable seed {}] {:?}", m.seed, m);
        assert!(
            !m.core_clause(),
            "term=1 (non-binding) must not form the committed occupation core (seed {})",
            m.seed
        );
    }
}

#[test]
fn salt_in_score_is_sensitivity_not_headline_upgrade() {
    let headline = run_all(norm_cfg());
    let no_imitation = run_all(no_imitation_cfg());
    let random = run_all(random_imitation_cfg());
    let salt = run_all(salt_in_score_cfg());
    let headline_success = headline
        .iter()
        .all(|m| m.verdict(&no_imitation, &random) == Verdict::InstitutionSpreadSuccess);
    let salt_success = salt
        .iter()
        .all(|m| m.verdict(&no_imitation, &random) == Verdict::InstitutionSpreadSuccess);
    for m in &salt {
        assert_guards(m, "salt_in_score");
    }
    if !headline_success && salt_success {
        eprintln!(
            "salt-only success is classified as {:?}",
            Verdict::WealthProxySelection
        );
    }
    assert!(
        !(!headline_success && salt_success)
            || salt
                .iter()
                .any(|m| m.salt_copy_drivers > 0 || m.generic_copy_drivers > 0),
        "the SALT sensitivity must report copy drivers"
    );
}

#[test]
fn non_adopters_cannot_commit() {
    let mut s = Settlement::generate(3, &norm_cfg());
    for _ in 0..PROBE_TICKS {
        s.econ_tick();
        for i in 0..s.population() {
            assert!(
                s.adopts_commitment_norm_of(i) || !s.is_committed(i),
                "non-adopter committed at tick {} index {}",
                s.econ_tick_count(),
                i
            );
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
    let mut low_seed = norm_cfg();
    if let Some(chain) = low_seed.chain.as_mut() {
        chain.commitment_seed_share_bps = 500;
    }
    let mut high_seed = norm_cfg();
    if let Some(chain) = high_seed.chain.as_mut() {
        chain.commitment_seed_share_bps = TOO_LARGE_SEED_SHARE_BPS;
    }
    let mut high_margin = norm_cfg();
    if let Some(chain) = high_margin.chain.as_mut() {
        chain.imitation_margin_bps = TOO_LARGE_MARGIN_BPS;
    }
    let mut slow_period = norm_cfg();
    if let Some(chain) = slow_period.chain.as_mut() {
        chain.imitation_period = 96;
    }

    let default = run_all(norm_cfg());
    let low_seed = run_all(low_seed);
    let high_seed = run_all(high_seed);
    let high_margin = run_all(high_margin);
    let slow_period = run_all(slow_period);
    for m in default
        .iter()
        .chain(low_seed.iter())
        .chain(high_seed.iter())
        .chain(high_margin.iter())
        .chain(slow_period.iter())
    {
        assert_guards(m, "robustness");
    }
    let mean_adopters =
        |xs: &[Metrics]| xs.iter().map(|m| m.final_adopters as f64).sum::<f64>() / xs.len() as f64;
    eprintln!(
        "[robustness] adopters default={:.1} low_seed={:.1} high_seed={:.1} \
         high_margin={:.1} slow_period={:.1}",
        mean_adopters(&default),
        mean_adopters(&low_seed),
        mean_adopters(&high_seed),
        mean_adopters(&high_margin),
        mean_adopters(&slow_period),
    );
    assert!(
        mean_adopters(&high_seed) > mean_adopters(&low_seed),
        "seed-share axis must move adoption"
    );
    assert!(
        mean_adopters(&high_margin) < mean_adopters(&default),
        "margin axis must suppress adoption"
    );
}
