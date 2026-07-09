//! C3R.b -- capital inheritance for mortal chain producers.
//!
//! The suite keeps the mechanism falsifiable: the inheritance cell, tool-denied
//! control, and C3R.a baseline are run side by side; the verdict ladder is printed,
//! while hard guards assert only invariants and attribution preconditions.

use sim::{Era, EraDetector, GoodId, Settlement, SettlementConfig, Vocation};

const SEEDS: [u64; 5] = [3, 7, 11, 19, 23];
const RUN_TICKS: u64 = 1_600;
const FINAL_WINDOW: u64 = 160;
const PRODUCER_HOUSEHOLDS: usize = 6;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Scenario {
    InheritanceCell,
    Control,
    MortalProducers,
    FlagOffHeritable,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Verdict {
    BaseUnviable,
    ReservoirOpen,
    ControlDidNotCollapse,
    ConservationBroken,
    RegistryBroken,
    InheritanceInert,
    ChainInheritsButStillCollapses,
    ChainPersistsUnderInheritance,
}

#[derive(Clone, Debug)]
struct Trace {
    seed: u64,
    cell: Scenario,
    conserved: bool,
    money_ok: bool,
    registry_ok: bool,
    mortal_deaths: u64,
    immortal_producer_max: usize,
    final_min_millers: usize,
    final_min_bakers: usize,
    living_millers: usize,
    living_bakers: usize,
    max_consecutive_both_stages: u64,
    bread_output: u64,
    producer_tool_inheritances: u64,
    heirless_producer_deaths: u64,
    heir_tool_adoptions: u64,
    role_readoptions: u64,
    mortal_capital_builds: u64,
    mortal_builder_adopter_pool: usize,
    producer_house_newborns: usize,
    producer_house_live_max: usize,
    current_era: Era,
    peak_era: Era,
}

fn chain_goods(cfg: &SettlementConfig) -> (GoodId, GoodId, GoodId) {
    let content = cfg.chain.as_ref().expect("chain").content.clone();
    (content.flour(), content.bread(), content.oven())
}

fn inheritance_cell() -> SettlementConfig {
    SettlementConfig::frontier_mortal_producers_heritable()
}

fn control_cell() -> SettlementConfig {
    let mut cfg = SettlementConfig::frontier_mortal_producers_heritable();
    cfg.chain
        .as_mut()
        .expect("chain")
        .mortal_producer_tool_inheritance = false;
    cfg
}

fn mortal_producers_baseline() -> SettlementConfig {
    SettlementConfig::frontier_mortal_producers()
}

fn flag_off_heritable_base() -> SettlementConfig {
    let mut cfg = SettlementConfig::frontier_mortal_producers_heritable();
    let chain = cfg.chain.as_mut().expect("chain");
    chain.mortal_chain_producers = false;
    chain.mortal_producer_inheritance = false;
    cfg
}

fn producer_house_start(s: &Settlement) -> Option<usize> {
    (s.household_count() >= PRODUCER_HOUSEHOLDS + 2)
        .then(|| s.household_count() - PRODUCER_HOUSEHOLDS)
}

fn producer_house_member_count(s: &Settlement) -> usize {
    let Some(start) = producer_house_start(s) else {
        return 0;
    };
    (0..s.population())
        .filter(|&idx| {
            s.is_alive(idx)
                && s.household_of(idx)
                    .is_some_and(|household| household >= start)
        })
        .count()
}

fn producer_house_newborn_count(s: &Settlement) -> usize {
    let Some(start) = producer_house_start(s) else {
        return 0;
    };
    (0..s.population())
        .filter(|&idx| {
            s.parent_of(idx).is_some()
                && s.household_of(idx)
                    .is_some_and(|household| household >= start)
        })
        .count()
}

fn trace_run(seed: u64, cell: Scenario, cfg: &SettlementConfig) -> Trace {
    let (_flour, bread, _oven) = chain_goods(cfg);
    let mut s = Settlement::generate(seed, cfg);
    let mut detector = EraDetector::new();
    let mut conserved = true;
    let mut money_ok = true;
    let mut bread_output = 0u64;
    let mut immortal_producer_max = s.immortal_producer_count();
    let mut final_min_millers = usize::MAX;
    let mut final_min_bakers = usize::MAX;
    let mut both_stage_streak = 0u64;
    let mut max_consecutive_both_stages = 0u64;
    let mut producer_house_live_max = producer_house_member_count(&s);

    for tick in 0..RUN_TICKS {
        let report = s.econ_tick();
        conserved &= report.conserves();
        money_ok &= report.money_conserves();
        bread_output = bread_output.saturating_add(report.produced_of(bread));
        detector.observe(&s);
        immortal_producer_max = immortal_producer_max.max(s.immortal_producer_count());
        producer_house_live_max = producer_house_live_max.max(producer_house_member_count(&s));

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
        }
    }

    Trace {
        seed,
        cell,
        conserved,
        money_ok,
        registry_ok: s.private_land_registry_invariant_holds(),
        mortal_deaths: s.mortal_producer_old_age_deaths(),
        immortal_producer_max,
        final_min_millers,
        final_min_bakers,
        living_millers: s.living_count(Vocation::Miller),
        living_bakers: s.living_count(Vocation::Baker),
        max_consecutive_both_stages,
        bread_output,
        producer_tool_inheritances: s.producer_tool_inheritances(),
        heirless_producer_deaths: s.heirless_producer_deaths(),
        heir_tool_adoptions: s.heir_tool_adoptions(),
        role_readoptions: s.role_readoptions(),
        mortal_capital_builds: s.mortal_capital_builds(),
        mortal_builder_adopter_pool: s.mortal_builder_adopter_pool(),
        producer_house_newborns: producer_house_newborn_count(&s),
        producer_house_live_max,
        current_era: detector.current_era(),
        peak_era: detector.peak_era(),
    }
}

fn persists(trace: &Trace) -> bool {
    trace.final_min_millers > 0 && trace.final_min_bakers > 0
}

fn reached_capital_stage(trace: &Trace) -> bool {
    trace.max_consecutive_both_stages >= EraDetector::new().window()
}

fn classify(cell: &Trace, control_collapsed: bool, flag_off_viable: bool) -> Verdict {
    if cell.mortal_deaths == 0 || !flag_off_viable {
        return Verdict::BaseUnviable;
    }
    if cell.immortal_producer_max > 0 {
        return Verdict::ReservoirOpen;
    }
    if !control_collapsed {
        return Verdict::ControlDidNotCollapse;
    }
    if !cell.conserved || !cell.money_ok {
        return Verdict::ConservationBroken;
    }
    if !cell.registry_ok {
        return Verdict::RegistryBroken;
    }
    if cell.producer_tool_inheritances == 0 {
        return Verdict::InheritanceInert;
    }
    if persists(cell) {
        Verdict::ChainPersistsUnderInheritance
    } else {
        Verdict::ChainInheritsButStillCollapses
    }
}

fn print_trace(trace: &Trace, verdict: Option<Verdict>) {
    println!(
        "C3R.b seed={} cell={:?} verdict={:?} deaths={} immortal_max={} \
         final_min=({},{}) living=({},{}) both_stage_streak={} era={:?}/{:?} bread={} \
         inheritances={} heirless={} heir_adoptions={} readopts={} mortal_builds={} pool={} \
         producer_house_newborns={} producer_house_live_max={} conserved={} money={} registry={}",
        trace.seed,
        trace.cell,
        verdict,
        trace.mortal_deaths,
        trace.immortal_producer_max,
        trace.final_min_millers,
        trace.final_min_bakers,
        trace.living_millers,
        trace.living_bakers,
        trace.max_consecutive_both_stages,
        trace.current_era,
        trace.peak_era,
        trace.bread_output,
        trace.producer_tool_inheritances,
        trace.heirless_producer_deaths,
        trace.heir_tool_adoptions,
        trace.role_readoptions,
        trace.mortal_capital_builds,
        trace.mortal_builder_adopter_pool,
        trace.producer_house_newborns,
        trace.producer_house_live_max,
        trace.conserved,
        trace.money_ok,
        trace.registry_ok
    );
}

#[test]
fn tag_28_is_on_only_and_splits_heritable_config() {
    let seed = SEEDS[0];

    let frontier = Settlement::generate(seed, &SettlementConfig::frontier());
    let mut frontier_knobs = SettlementConfig::frontier();
    {
        let chain = frontier_knobs.chain.as_mut().expect("chain");
        chain.mortal_producer_inheritance = true;
        chain.mortal_producer_tool_inheritance = false;
        chain.producer_house_cap = 99;
    }
    assert_eq!(
        frontier.canonical_bytes(),
        Settlement::generate(seed, &frontier_knobs).canonical_bytes(),
        "tag 28 must stay absent when the C3R.b gate is inert"
    );

    let capital = Settlement::generate(seed, &SettlementConfig::frontier_capital());
    let mut explicit_off = SettlementConfig::frontier_mortal_producers();
    {
        let chain = explicit_off.chain.as_mut().expect("chain");
        chain.mortal_chain_producers = false;
        chain.mortal_producer_inheritance = false;
        chain.mortal_producer_tool_inheritance = false;
        chain.producer_house_cap = 99;
    }
    assert_eq!(
        capital.canonical_bytes(),
        Settlement::generate(seed, &explicit_off).canonical_bytes(),
        "new C3R.b knobs must not perturb frontier_capital / C3R.a flag-off bytes"
    );

    let on = Settlement::generate(seed, &inheritance_cell());
    let mut off_cfg = inheritance_cell();
    off_cfg
        .chain
        .as_mut()
        .expect("chain")
        .mortal_producer_inheritance = false;
    let off = Settlement::generate(seed, &off_cfg);
    assert_ne!(
        on.canonical_bytes(),
        off.canonical_bytes(),
        "active mortal_producer_inheritance must split the canonical stream"
    );

    let denied = Settlement::generate(seed, &control_cell());
    assert_ne!(
        on.canonical_bytes(),
        denied.canonical_bytes(),
        "the tool-inheritance switch must be part of tag 28"
    );

    let mut cap_cfg = inheritance_cell();
    cap_cfg.chain.as_mut().expect("chain").producer_house_cap = 3;
    assert_ne!(
        on.canonical_bytes(),
        Settlement::generate(seed, &cap_cfg).canonical_bytes(),
        "the producer-house cap must be part of tag 28"
    );
}

#[test]
fn heritable_producers_reproduce_and_reservoir_stays_closed() {
    for seed in SEEDS {
        let trace = trace_run(seed, Scenario::InheritanceCell, &inheritance_cell());
        print_trace(&trace, None);
        assert!(
            trace.conserved,
            "inheritance cell broke conservation seed={seed}"
        );
        assert!(
            trace.money_ok,
            "inheritance cell broke money invariant seed={seed}"
        );
        assert!(trace.registry_ok, "registry invariant failed seed={seed}");
        assert_eq!(
            trace.immortal_producer_max, 0,
            "mortal producer reservoir reopened seed={seed}"
        );
        assert!(
            trace.mortal_deaths > 0,
            "mortal producers must die of old age seed={seed}"
        );
        assert!(
            trace.producer_house_newborns > 0,
            "producer households must bear heirs seed={seed}"
        );
        assert!(
            trace.producer_house_live_max <= PRODUCER_HOUSEHOLDS * 2,
            "producer-house pool must stay bounded seed={seed} max={}",
            trace.producer_house_live_max
        );
    }
}

#[test]
fn flag_off_heritable_base_reaches_capital_stage() {
    for seed in SEEDS {
        let trace = trace_run(seed, Scenario::FlagOffHeritable, &flag_off_heritable_base());
        print_trace(&trace, None);
        assert!(
            trace.conserved,
            "flag-off base broke conservation seed={seed}"
        );
        assert!(
            trace.money_ok,
            "flag-off base broke money invariant seed={seed}"
        );
        assert!(
            reached_capital_stage(&trace),
            "flag-off heritable base must reach Capital-stage staffing seed={seed}"
        );
    }
}

#[test]
fn triad_classifies_inheritance_against_collapsing_controls() {
    for seed in SEEDS {
        let baseline = trace_run(
            seed,
            Scenario::MortalProducers,
            &mortal_producers_baseline(),
        );
        let control = trace_run(seed, Scenario::Control, &control_cell());
        let cell = trace_run(seed, Scenario::InheritanceCell, &inheritance_cell());
        let flag_off = trace_run(seed, Scenario::FlagOffHeritable, &flag_off_heritable_base());

        assert!(
            baseline.conserved,
            "baseline conservation failed seed={seed}"
        );
        assert!(control.conserved, "control conservation failed seed={seed}");
        assert!(
            cell.conserved,
            "inheritance conservation failed seed={seed}"
        );
        assert!(
            baseline.money_ok,
            "baseline money invariant failed seed={seed}"
        );
        assert!(
            control.money_ok,
            "control money invariant failed seed={seed}"
        );
        assert!(
            cell.money_ok,
            "inheritance money invariant failed seed={seed}"
        );
        assert!(baseline.registry_ok, "baseline registry failed seed={seed}");
        assert!(control.registry_ok, "control registry failed seed={seed}");
        assert!(cell.registry_ok, "inheritance registry failed seed={seed}");
        assert_eq!(
            baseline.immortal_producer_max, 0,
            "baseline reservoir opened seed={seed}"
        );
        assert_eq!(
            control.immortal_producer_max, 0,
            "control reservoir opened seed={seed}"
        );
        assert_eq!(
            cell.immortal_producer_max, 0,
            "inheritance reservoir opened seed={seed}"
        );
        assert!(
            baseline.mortal_deaths > 0,
            "baseline is vacuous seed={seed}"
        );
        assert!(control.mortal_deaths > 0, "control is vacuous seed={seed}");
        assert!(cell.mortal_deaths > 0, "inheritance is vacuous seed={seed}");

        assert!(
            !persists(&baseline),
            "C3R.a mortal producer baseline must collapse seed={seed}"
        );
        assert!(
            !persists(&control),
            "tool-inheritance-denied control must collapse for attribution seed={seed}"
        );

        let verdict = classify(&cell, !persists(&control), reached_capital_stage(&flag_off));
        print_trace(&baseline, None);
        print_trace(&control, None);
        print_trace(&cell, Some(verdict));

        assert_ne!(
            verdict,
            Verdict::BaseUnviable,
            "flag-off base/death precondition failed seed={seed}"
        );
        assert_ne!(
            verdict,
            Verdict::ReservoirOpen,
            "inheritance cell reopened immortal producer reservoir seed={seed}"
        );
        assert_ne!(
            verdict,
            Verdict::ControlDidNotCollapse,
            "tool-denied control did not collapse seed={seed}"
        );
        assert_ne!(
            verdict,
            Verdict::ConservationBroken,
            "inheritance cell broke conservation seed={seed}"
        );
        assert_ne!(
            verdict,
            Verdict::RegistryBroken,
            "inheritance cell broke registry seed={seed}"
        );
    }
}
