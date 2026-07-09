//! C3R.a -- mortal chain producers, no succession.
//!
//! This suite keeps the build boundary honest: the flag only makes the chain's
//! producer reservoir mortal and closes producer formation to mortal agents. It
//! deliberately does not require self-repair; the verdict test prints the outcome.

use sim::{Era, EraDetector, GoodId, Settlement, SettlementConfig, Vocation};

const SEEDS: [u64; 5] = [3, 7, 11, 19, 23];
const RUN_TICKS: u64 = 1_200;
const FINAL_WINDOW: u64 = 120;
const THIN_POOL_MAX: usize = 1;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Verdict {
    BaseUnviable,
    ReservoirOpen,
    ConservationBroken,
    RegistryBroken,
    ChainCollapsesOnProducerDeath,
    CollapseFromThinMortalPool,
    ChainSelfRepairsWithoutSuccession,
    ChainRunsMortalAndBuilds,
}

#[derive(Clone, Debug)]
struct Trace {
    seed: u64,
    conserved: bool,
    registry_ok: bool,
    mortal_deaths: u64,
    immortal_producer_max: usize,
    final_min_millers: usize,
    final_min_bakers: usize,
    final_max_producers: usize,
    max_consecutive_both_stages: u64,
    bread_output: u64,
    current_era: Era,
    peak_era: Era,
    role_readoptions: u64,
    mortal_capital_builds: u64,
    mortal_builder_adopter_pool: usize,
}

fn chain_goods(cfg: &SettlementConfig) -> (GoodId, GoodId, GoodId) {
    let content = cfg.chain.as_ref().expect("chain").content.clone();
    (content.flour(), content.bread(), content.oven())
}

fn frontier_mortal_producers_flag_off() -> SettlementConfig {
    let mut cfg = SettlementConfig::frontier_mortal_producers();
    cfg.chain.as_mut().expect("chain").mortal_chain_producers = false;
    cfg
}

fn run_observing(seed: u64, cfg: &SettlementConfig, ticks: u64) -> (Settlement, EraDetector, bool) {
    let mut s = Settlement::generate(seed, cfg);
    let mut detector = EraDetector::new();
    let mut conserved = true;
    for _ in 0..ticks {
        let report = s.econ_tick();
        conserved &= report.conserves();
        detector.observe(&s);
    }
    (s, detector, conserved)
}

fn trace_run(seed: u64, cfg: &SettlementConfig) -> Trace {
    let (_flour, bread, _oven) = chain_goods(cfg);
    let mut s = Settlement::generate(seed, cfg);
    let mut detector = EraDetector::new();
    let mut conserved = true;
    let mut bread_output = 0u64;
    let mut immortal_producer_max = s.immortal_producer_count();
    let mut final_min_millers = usize::MAX;
    let mut final_min_bakers = usize::MAX;
    let mut final_max_producers = 0usize;
    let mut both_stage_streak = 0u64;
    let mut max_consecutive_both_stages = 0u64;

    for tick in 0..RUN_TICKS {
        let report = s.econ_tick();
        conserved &= report.conserves();
        bread_output = bread_output.saturating_add(report.produced_of(bread));
        detector.observe(&s);
        immortal_producer_max = immortal_producer_max.max(s.immortal_producer_count());
        let millers = s.living_count(Vocation::Miller);
        let bakers = s.living_count(Vocation::Baker);

        if millers > 0 && bakers > 0 {
            both_stage_streak = both_stage_streak.saturating_add(1);
            max_consecutive_both_stages = max_consecutive_both_stages.max(both_stage_streak);
        } else {
            both_stage_streak = 0;
        }

        if tick + FINAL_WINDOW >= RUN_TICKS {
            final_min_millers = final_min_millers.min(millers);
            final_min_bakers = final_min_bakers.min(bakers);
            final_max_producers = final_max_producers.max(millers + bakers);
        }
    }

    Trace {
        seed,
        conserved,
        registry_ok: s.private_land_registry_invariant_holds(),
        mortal_deaths: s.mortal_producer_old_age_deaths(),
        immortal_producer_max,
        final_min_millers,
        final_min_bakers,
        final_max_producers,
        max_consecutive_both_stages,
        bread_output,
        current_era: detector.current_era(),
        peak_era: detector.peak_era(),
        role_readoptions: s.role_readoptions(),
        mortal_capital_builds: s.mortal_capital_builds(),
        mortal_builder_adopter_pool: s.mortal_builder_adopter_pool(),
    }
}

fn trace_mortal(seed: u64) -> Trace {
    let cfg = SettlementConfig::frontier_mortal_producers();
    trace_run(seed, &cfg)
}

// Collapse-vs-self-repair is measured off the Capital-*stage* trigger
// (`both_stages_staffed` sustained over the final window), NOT the `EraDetector`'s
// Capital *rung*. That is deliberate and load-bearing: `frontier_capital` is a
// designated-GOLD economy (a money good from tick 0, `barter_trade_count() == 0`), so
// the spatial `Barter` rung (`barter_trade_count() >= 4`) never clears and the ladder
// is stranded at `Forager` — the immortal control included. Even forcing the barter
// floor to zero, the `Specialist` rung (20% producer share) is met by only one of the
// five seeds, so the era rung reaches `Capital` for <1/5 seeds and cannot discriminate
// collapse from self-repair here. `both_stages_staffed` is exactly the measured signal
// the `Capital` rung itself reads (era.rs), so reading it directly is the faithful
// operationalization; `current_era`/`peak_era` are captured as diagnostic context only.
fn classify(trace: &Trace) -> Verdict {
    if trace.mortal_deaths == 0 {
        return Verdict::BaseUnviable;
    }
    if trace.immortal_producer_max > 0 {
        return Verdict::ReservoirOpen;
    }
    if !trace.conserved {
        return Verdict::ConservationBroken;
    }
    if !trace.registry_ok {
        return Verdict::RegistryBroken;
    }

    let stages_staffed = trace.final_min_millers > 0 && trace.final_min_bakers > 0;
    if stages_staffed && trace.mortal_capital_builds > 0 {
        return Verdict::ChainRunsMortalAndBuilds;
    }
    if stages_staffed {
        return Verdict::ChainSelfRepairsWithoutSuccession;
    }

    if trace.mortal_builder_adopter_pool <= THIN_POOL_MAX {
        Verdict::CollapseFromThinMortalPool
    } else {
        Verdict::ChainCollapsesOnProducerDeath
    }
}

#[test]
fn flag_off_actual_base_runs_immortal_chain() {
    let cfg = frontier_mortal_producers_flag_off();
    for seed in SEEDS {
        let trace = trace_run(seed, &cfg);
        assert!(
            trace.conserved,
            "flag-off actual base broke conservation seed={seed}"
        );
        assert_eq!(
            trace.mortal_deaths, 0,
            "flag-off actual base must not count mortal producer deaths seed={seed}"
        );
        assert!(
            trace.immortal_producer_max > 0,
            "flag-off actual base should retain immortal producers seed={seed}"
        );
        assert!(
            trace.bread_output > 0,
            "flag-off actual base must run the bread chain seed={seed}"
        );
        // Base viability = the immortal chain reaches the Capital *stage* (both stages
        // staffed for at least the hysteresis window). Asserted off the stage trigger,
        // not the `EraDetector` rung, which is stranded at `Forager` on this
        // designated-money base — see the note on `classify` for the full reasoning.
        assert!(
            trace.max_consecutive_both_stages >= EraDetector::new().window(),
            "flag-off actual base must sustain both chain stages long enough to meet the Capital-stage trigger seed={seed}, max_both_stage_streak={} final_min=({},{}) era={:?}/{:?}",
            trace.max_consecutive_both_stages,
            trace.final_min_millers,
            trace.final_min_bakers,
            trace.current_era,
            trace.peak_era
        );
    }
}

#[test]
fn tag_27_is_on_only_and_splits_mortal_config() {
    let seed = SEEDS[0];

    let off_cfg = SettlementConfig::frontier_capital();
    let explicit_off_cfg = frontier_mortal_producers_flag_off();
    let off = Settlement::generate(seed, &off_cfg);
    let explicit_off = Settlement::generate(seed, &explicit_off_cfg);
    assert_eq!(
        off.canonical_bytes(),
        explicit_off.canonical_bytes(),
        "flag-off frontier_capital bytes must remain unchanged"
    );

    let on = Settlement::generate(seed, &SettlementConfig::frontier_mortal_producers());
    assert_ne!(
        off.canonical_bytes(),
        on.canonical_bytes(),
        "active mortal_chain_producers must split the canonical stream"
    );

    let inert_cfg = SettlementConfig::grain_flour_bread_chain();
    let mut inert_flag_cfg = inert_cfg.clone();
    inert_flag_cfg
        .chain
        .as_mut()
        .expect("chain")
        .mortal_chain_producers = true;
    let inert = Settlement::generate(seed, &inert_cfg);
    let inert_flag = Settlement::generate(seed, &inert_flag_cfg);
    assert_eq!(
        inert.canonical_bytes(),
        inert_flag.canonical_bytes(),
        "mortal_chain_producers is inert without demography, so tag 27 must be ON-only"
    );
}

#[test]
fn classifier_can_report_repair_on_designated_money_base() {
    let repaired = Trace {
        seed: 0,
        conserved: true,
        registry_ok: true,
        mortal_deaths: 1,
        immortal_producer_max: 0,
        final_min_millers: 1,
        final_min_bakers: 1,
        final_max_producers: 2,
        max_consecutive_both_stages: FINAL_WINDOW,
        bread_output: 1,
        current_era: Era::Forager,
        peak_era: Era::Forager,
        role_readoptions: 1,
        mortal_capital_builds: 0,
        mortal_builder_adopter_pool: 2,
    };
    assert_eq!(
        classify(&repaired),
        Verdict::ChainSelfRepairsWithoutSuccession
    );

    let mut built = repaired.clone();
    built.mortal_capital_builds = 1;
    assert_eq!(classify(&built), Verdict::ChainRunsMortalAndBuilds);

    let mut unstaffed = built;
    unstaffed.final_min_bakers = 0;
    unstaffed.final_max_producers = 1;
    assert_eq!(classify(&unstaffed), Verdict::ChainCollapsesOnProducerDeath);
}

#[test]
fn mortal_seeded_producers_die_and_reservoir_stays_closed() {
    for seed in SEEDS {
        let cfg = SettlementConfig::frontier_mortal_producers();
        let s0 = Settlement::generate(seed, &cfg);
        let control = Settlement::generate(seed, &SettlementConfig::frontier_capital());
        assert_eq!(
            s0.household_count(),
            control.household_count(),
            "producer mortality must not add households seed={seed}"
        );
        let mortal_latent_producers = (0..s0.population())
            .filter(|&idx| {
                s0.household_of(idx).is_none()
                    && s0.lifespan_of(idx).is_some()
                    && s0.is_tool_acquisition_eligible(idx)
            })
            .count();
        assert_eq!(
            mortal_latent_producers, 6,
            "the six seeded latent Mill/Bake producers must be lifespan-only mortals seed={seed}"
        );

        let (s, _detector, conserved) = run_observing(seed, &cfg, RUN_TICKS);
        assert!(
            conserved,
            "mortal producer run broke conservation seed={seed}"
        );
        assert_eq!(
            s.immortal_producer_count(),
            0,
            "mortal producer reservoir reopened seed={seed}"
        );
        assert!(
            s.mortal_producer_old_age_deaths() > 0,
            "mortal seeded producers must die of old age seed={seed}"
        );
    }
}

#[test]
fn mortal_producer_verdicts_are_observed_not_asserted() {
    for seed in SEEDS {
        let trace = trace_mortal(seed);
        let verdict = classify(&trace);
        println!(
            "C3R.a seed={} verdict={:?} deaths={} immortal_max={} final_min=({},{}) \
             final_max_producers={} both_stage_streak={} era={:?}/{:?} bread={} readopts={} mortal_builds={} \
             pool={} conserved={} registry={}",
            trace.seed,
            verdict,
            trace.mortal_deaths,
            trace.immortal_producer_max,
            trace.final_min_millers,
            trace.final_min_bakers,
            trace.final_max_producers,
            trace.max_consecutive_both_stages,
            trace.current_era,
            trace.peak_era,
            trace.bread_output,
            trace.role_readoptions,
            trace.mortal_capital_builds,
            trace.mortal_builder_adopter_pool,
            trace.conserved,
            trace.registry_ok
        );
    }
}
