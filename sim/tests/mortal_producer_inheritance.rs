//! C3R.b v2 -- capital inheritance for mortal chain producers.
//!
//! The suite classifies rather than tunes: it sweeps the producer-house hearth
//! subsidy and house cap on the landed v1 inheritance mechanism, prints the
//! structure/flow split, and asserts only invariants.

use sim::{Era, EraDetector, GoodId, Settlement, SettlementConfig, Vocation};

const SEEDS: [u64; 5] = [3, 7, 11, 19, 23];
const RUN_TICKS: u64 = 1_600;
const FINAL_WINDOW: u64 = 160;
const PRODUCER_HOUSEHOLDS: usize = 6;
const FOOD_SWEEP: [u32; 4] = [0, 1, 2, 3];
const CAP_SWEEP: [u8; 3] = [1, 2, 3];
const VIABLE_SUBSIDY_SLICE: u32 = 1;
const DEFAULT_CAP_SLICE: u8 = 2;
const SUBSIDY_FLOOD_BREAD_FLOOR: u64 = 1;
const FLOW_RUNS_MIN_BREAD: u64 = 100;
const FLOW_RUNS_MIN_PER_STAFFED_TICK: f64 = 0.10;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Scenario {
    InheritanceCell,
    Control,
    MortalProducers,
    FlagOffHeritable,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum StructureVerdict {
    BaseUnviable,
    ReservoirOpen,
    ConservationBroken,
    RegistryBroken,
    SubsidyFloodsChainDies,
    StructurePersistsUnderInheritance,
    StructureDoesNotPersist,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum FlowVerdict {
    NotEvaluated,
    FlowCapped,
    FlowRuns,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Case {
    cell: Scenario,
    food_provision: Option<u32>,
    house_cap: Option<u8>,
}

#[derive(Clone, Debug)]
struct Trace {
    seed: u64,
    cell: Scenario,
    food_provision: Option<u32>,
    house_cap: Option<u8>,
    conserved: bool,
    money_ok: bool,
    registry_ok: bool,
    mortal_deaths: u64,
    immortal_producer_max: usize,
    final_min_millers: usize,
    final_min_bakers: usize,
    living_millers: usize,
    living_bakers: usize,
    both_stage_staffed_ticks: u64,
    max_both_stage_streak: u64,
    producer_role_ticks: u64,
    producer_bread_output: u64,
    bread_consumed: u64,
    bread_bought: u64,
    late_bread_price: Option<u64>,
    late_bread_trade_count: u64,
    producer_house_hearth_food_minted: u64,
    non_producer_hearth_food_minted: u64,
    producer_house_births: u64,
    producer_house_deaths: u64,
    producer_house_person_ticks: u64,
    producer_mean_tenure: f64,
    producer_tool_inheritances: u64,
    heirless_producer_deaths: u64,
    heir_tool_adoptions: u64,
    role_readoptions: u64,
    mortal_capital_builds: u64,
    mortal_builder_adopter_pool: usize,
    producer_house_live_max: usize,
    recipe_pay_rejections: u64,
    build_rejections: u64,
    adoption_rejections: u64,
    current_era: Era,
    peak_era: Era,
}

fn chain_goods(cfg: &SettlementConfig) -> (GoodId, GoodId, GoodId) {
    let content = cfg.chain.as_ref().expect("chain").content.clone();
    (content.flour(), content.bread(), content.oven())
}

fn mutate_producer_house_food(cfg: &mut SettlementConfig, food_provision: u32) {
    let demo = cfg
        .demography
        .as_mut()
        .expect("heritable base has demography");
    let start = demo
        .households
        .len()
        .checked_sub(PRODUCER_HOUSEHOLDS)
        .expect("heritable base appends producer households");
    for spec in &mut demo.households[start..] {
        spec.food_provision = food_provision;
    }
}

fn heritable_cell(food_provision: u32, house_cap: u8, tool_inheritance: bool) -> SettlementConfig {
    let mut cfg = SettlementConfig::frontier_mortal_producers_heritable();
    mutate_producer_house_food(&mut cfg, food_provision);
    let chain = cfg.chain.as_mut().expect("chain");
    chain.producer_house_cap = house_cap;
    chain.mortal_producer_tool_inheritance = tool_inheritance;
    cfg
}

fn case_config(case: Case) -> SettlementConfig {
    match case.cell {
        Scenario::InheritanceCell => heritable_cell(
            case.food_provision.expect("inheritance food axis"),
            case.house_cap.expect("inheritance cap axis"),
            true,
        ),
        Scenario::Control => heritable_cell(
            case.food_provision.expect("control food axis"),
            case.house_cap.expect("control cap axis"),
            false,
        ),
        Scenario::MortalProducers => SettlementConfig::frontier_mortal_producers(),
        Scenario::FlagOffHeritable => {
            let mut cfg = SettlementConfig::frontier_mortal_producers_heritable();
            let chain = cfg.chain.as_mut().expect("chain");
            chain.mortal_chain_producers = false;
            chain.mortal_producer_inheritance = false;
            cfg
        }
    }
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

fn tick_bread_trade_count(
    s: &Settlement,
    bread: GoodId,
    spot_start: usize,
    barter_start: usize,
) -> u64 {
    let spot = s.society().trades[spot_start..]
        .iter()
        .filter(|trade| trade.good == bread)
        .count() as u64;
    let barter = s.society().barter_trades[barter_start..]
        .iter()
        .filter(|trade| trade.a_gives == bread || trade.b_gives == bread)
        .count() as u64;
    spot + barter
}

fn trace_run(seed: u64, case: Case, cfg: &SettlementConfig) -> Trace {
    let (_flour, bread, _oven) = chain_goods(cfg);
    let mut s = Settlement::generate(seed, cfg);
    let mut detector = EraDetector::new();
    let mut conserved = true;
    let mut money_ok = true;
    let mut producer_bread_output = 0u64;
    let mut bread_consumed = 0u64;
    let mut immortal_producer_max = s.immortal_producer_count();
    let mut final_min_millers = usize::MAX;
    let mut final_min_bakers = usize::MAX;
    let mut both_stage_staffed_ticks = 0u64;
    let mut max_both_stage_streak = 0u64;
    let mut cur_both_stage_streak = 0u64;
    let mut producer_role_ticks = 0u64;
    let mut producer_house_live_max = producer_house_member_count(&s);
    let mut late_bread_price = None;
    let mut late_bread_trade_count = 0u64;

    for tick in 0..RUN_TICKS {
        let spot_start = s.society().trades.len();
        let barter_start = s.society().barter_trades.len();
        let report = s.econ_tick();
        conserved &= report.conserves();
        money_ok &= report.money_conserves();
        let produced = report.produced_of(bread);
        producer_bread_output = producer_bread_output.saturating_add(produced);
        bread_consumed = bread_consumed.saturating_add(report.consumed_of(bread));
        detector.observe(&s);
        immortal_producer_max = immortal_producer_max.max(s.immortal_producer_count());
        producer_house_live_max = producer_house_live_max.max(producer_house_member_count(&s));

        let millers = s.living_count(Vocation::Miller);
        let bakers = s.living_count(Vocation::Baker);
        if millers > 0 && bakers > 0 {
            both_stage_staffed_ticks = both_stage_staffed_ticks.saturating_add(1);
            cur_both_stage_streak = cur_both_stage_streak.saturating_add(1);
            max_both_stage_streak = max_both_stage_streak.max(cur_both_stage_streak);
        } else {
            cur_both_stage_streak = 0;
        }
        producer_role_ticks = producer_role_ticks.saturating_add((millers + bakers) as u64);

        if tick + FINAL_WINDOW >= RUN_TICKS {
            final_min_millers = final_min_millers.min(millers);
            final_min_bakers = final_min_bakers.min(bakers);
            let tick_bread_trades = tick_bread_trade_count(&s, bread, spot_start, barter_start);
            late_bread_trade_count = late_bread_trade_count.saturating_add(tick_bread_trades);
            if tick_bread_trades > 0 {
                late_bread_price = s.realized_price(bread).map(|price| price.0);
            }
        }
    }

    let producer_house_deaths = s.producer_house_deaths();
    let producer_house_person_ticks = s.producer_house_person_ticks();
    // Mean producer-house member tenure: person-ticks accrued by producer-house
    // members over the run, divided by producer-house deaths. Both quantities are
    // producer-house-scoped, so the ratio is population-consistent (unlike dividing
    // colony-wide `producer_role_ticks`, which also counts non-producer-household
    // S7 adopters that never die into `producer_house_deaths`).
    let producer_mean_tenure = if producer_house_deaths > 0 {
        producer_house_person_ticks as f64 / producer_house_deaths as f64
    } else {
        0.0
    };

    Trace {
        seed,
        cell: case.cell,
        food_provision: case.food_provision,
        house_cap: case.house_cap,
        conserved,
        money_ok,
        registry_ok: s.private_land_registry_invariant_holds(),
        mortal_deaths: s.mortal_producer_old_age_deaths(),
        immortal_producer_max,
        final_min_millers,
        final_min_bakers,
        living_millers: s.living_count(Vocation::Miller),
        living_bakers: s.living_count(Vocation::Baker),
        both_stage_staffed_ticks,
        max_both_stage_streak,
        producer_role_ticks,
        producer_bread_output,
        bread_consumed,
        bread_bought: s.trade_volume_of(bread),
        late_bread_price,
        late_bread_trade_count,
        producer_house_hearth_food_minted: s.producer_house_hearth_food_minted(),
        non_producer_hearth_food_minted: s.non_producer_hearth_food_minted(),
        producer_house_births: s.producer_house_births(),
        producer_house_deaths,
        producer_house_person_ticks,
        producer_mean_tenure,
        producer_tool_inheritances: s.producer_tool_inheritances(),
        heirless_producer_deaths: s.heirless_producer_deaths(),
        heir_tool_adoptions: s.heir_tool_adoptions(),
        role_readoptions: s.role_readoptions(),
        mortal_capital_builds: s.mortal_capital_builds(),
        mortal_builder_adopter_pool: s.mortal_builder_adopter_pool(),
        producer_house_live_max,
        recipe_pay_rejections: s.producer_recipe_pay_rejections(),
        build_rejections: s.producer_build_rejections(),
        adoption_rejections: s.producer_adoption_rejections(),
        current_era: detector.current_era(),
        peak_era: detector.peak_era(),
    }
}

fn persists(trace: &Trace) -> bool {
    trace.final_min_millers > 0 && trace.final_min_bakers > 0
}

fn reached_capital_stage(trace: &Trace) -> bool {
    // Sustained (consecutive) joint staffing, not a cumulative tally: the base is
    // "viable" only if both stages held together for at least a hysteresis window,
    // mirroring how `EraDetector` enters a rung. A cumulative count would pass on
    // scattered single-tick staffing that never sustains a chain.
    trace.max_both_stage_streak >= EraDetector::new().window()
}

fn classify(trace: &Trace) -> (StructureVerdict, FlowVerdict) {
    if trace.mortal_deaths == 0 && !matches!(trace.cell, Scenario::FlagOffHeritable) {
        return (StructureVerdict::BaseUnviable, FlowVerdict::NotEvaluated);
    }
    if trace.immortal_producer_max > 0 && !matches!(trace.cell, Scenario::FlagOffHeritable) {
        return (StructureVerdict::ReservoirOpen, FlowVerdict::NotEvaluated);
    }
    if !trace.conserved || !trace.money_ok {
        return (
            StructureVerdict::ConservationBroken,
            FlowVerdict::NotEvaluated,
        );
    }
    if !trace.registry_ok {
        return (StructureVerdict::RegistryBroken, FlowVerdict::NotEvaluated);
    }
    if trace.producer_house_hearth_food_minted > 0
        && trace.producer_bread_output <= SUBSIDY_FLOOD_BREAD_FLOOR
    {
        return (
            StructureVerdict::SubsidyFloodsChainDies,
            FlowVerdict::NotEvaluated,
        );
    }
    if !persists(trace) {
        return (
            StructureVerdict::StructureDoesNotPersist,
            FlowVerdict::NotEvaluated,
        );
    }

    let per_staffed_tick = bread_per_staffed_tick(trace);
    let flow = if trace.producer_bread_output >= FLOW_RUNS_MIN_BREAD
        && per_staffed_tick >= FLOW_RUNS_MIN_PER_STAFFED_TICK
    {
        FlowVerdict::FlowRuns
    } else {
        FlowVerdict::FlowCapped
    };
    (StructureVerdict::StructurePersistsUnderInheritance, flow)
}

fn bread_per_staffed_tick(trace: &Trace) -> f64 {
    if trace.both_stage_staffed_ticks == 0 {
        0.0
    } else {
        trace.producer_bread_output as f64 / trace.both_stage_staffed_ticks as f64
    }
}

fn print_trace(trace: &Trace, structure: StructureVerdict, flow: FlowVerdict) {
    println!(
        "C3R.b.v2 seed={} cell={:?} food={:?} cap={:?} structure={:?} flow={:?} \
         deaths={} immortal_max={} final_min=({},{}) living=({},{}) era={:?}/{:?} \
         bread_producer={} bread_consumed={} bread_bought={} \
         late_bread_price={:?} late_bread_trades={} both_stage_staffed_ticks={} \
         max_both_stage_streak={} producer_role_ticks={} bread_per_staffed_tick={:.4} \
         producer_hearth_food={} \
         nonproducer_hearth_food={} producer_births={} producer_deaths={} \
         producer_person_ticks={} producer_mean_tenure={:.2} inheritances={} heirless={} \
         heir_adoptions={} readopts={} mortal_builds={} pool={} producer_live_max={} \
         recipe_pay_rejects={} build_rejects={} adoption_rejects={} conserved={} money={} registry={}",
        trace.seed,
        trace.cell,
        trace.food_provision,
        trace.house_cap,
        structure,
        flow,
        trace.mortal_deaths,
        trace.immortal_producer_max,
        trace.final_min_millers,
        trace.final_min_bakers,
        trace.living_millers,
        trace.living_bakers,
        trace.current_era,
        trace.peak_era,
        trace.producer_bread_output,
        trace.bread_consumed,
        trace.bread_bought,
        trace.late_bread_price,
        trace.late_bread_trade_count,
        trace.both_stage_staffed_ticks,
        trace.max_both_stage_streak,
        trace.producer_role_ticks,
        bread_per_staffed_tick(trace),
        trace.producer_house_hearth_food_minted,
        trace.non_producer_hearth_food_minted,
        trace.producer_house_births,
        trace.producer_house_deaths,
        trace.producer_house_person_ticks,
        trace.producer_mean_tenure,
        trace.producer_tool_inheritances,
        trace.heirless_producer_deaths,
        trace.heir_tool_adoptions,
        trace.role_readoptions,
        trace.mortal_capital_builds,
        trace.mortal_builder_adopter_pool,
        trace.producer_house_live_max,
        trace.recipe_pay_rejections,
        trace.build_rejections,
        trace.adoption_rejections,
        trace.conserved,
        trace.money_ok,
        trace.registry_ok
    );
}

fn assert_invariants(trace: &Trace) {
    assert!(trace.conserved, "conservation failed: {trace:?}");
    assert!(trace.money_ok, "money invariant failed: {trace:?}");
    assert!(trace.registry_ok, "registry invariant failed: {trace:?}");
    if !matches!(trace.cell, Scenario::FlagOffHeritable) {
        assert_eq!(
            trace.immortal_producer_max, 0,
            "mortal producer reservoir opened: {trace:?}"
        );
    }
    if matches!(
        trace.cell,
        Scenario::MortalProducers | Scenario::Control | Scenario::InheritanceCell
    ) && trace.food_provision != Some(0)
    {
        assert!(
            trace.mortal_deaths > 0,
            "expected mortal producer old-age deaths: {trace:?}"
        );
    }
}

fn push_unique(cases: &mut Vec<Case>, case: Case) {
    if !cases.contains(&case) {
        cases.push(case);
    }
}

fn informative_cases() -> Vec<Case> {
    let mut cases = Vec::new();
    push_unique(
        &mut cases,
        Case {
            cell: Scenario::MortalProducers,
            food_provision: None,
            house_cap: None,
        },
    );
    push_unique(
        &mut cases,
        Case {
            cell: Scenario::FlagOffHeritable,
            food_provision: None,
            house_cap: None,
        },
    );

    for food in FOOD_SWEEP {
        for cell in [Scenario::Control, Scenario::InheritanceCell] {
            push_unique(
                &mut cases,
                Case {
                    cell,
                    food_provision: Some(food),
                    house_cap: Some(DEFAULT_CAP_SLICE),
                },
            );
        }
    }
    for cap in CAP_SWEEP {
        for cell in [Scenario::Control, Scenario::InheritanceCell] {
            push_unique(
                &mut cases,
                Case {
                    cell,
                    food_provision: Some(VIABLE_SUBSIDY_SLICE),
                    house_cap: Some(cap),
                },
            );
        }
    }
    cases
}

#[test]
fn old_bases_are_byte_identical_and_tag_28_splits_heritable_config() {
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
        "C3R.b knobs must be inert on the old frontier base"
    );

    let capital = Settlement::generate(seed, &SettlementConfig::frontier_capital());
    let mut capital_knobs = SettlementConfig::frontier_capital();
    {
        let chain = capital_knobs.chain.as_mut().expect("chain");
        chain.mortal_producer_inheritance = true;
        chain.mortal_producer_tool_inheritance = false;
        chain.producer_house_cap = 99;
    }
    assert_eq!(
        capital.canonical_bytes(),
        Settlement::generate(seed, &capital_knobs).canonical_bytes(),
        "C3R.b knobs must be inert on the old frontier_capital base"
    );

    let mortal = Settlement::generate(seed, &SettlementConfig::frontier_mortal_producers());
    let mut mortal_knobs = SettlementConfig::frontier_mortal_producers();
    {
        let chain = mortal_knobs.chain.as_mut().expect("chain");
        chain.mortal_producer_tool_inheritance = false;
        chain.producer_house_cap = 99;
    }
    assert_eq!(
        mortal.canonical_bytes(),
        Settlement::generate(seed, &mortal_knobs).canonical_bytes(),
        "C3R.b tool/cap knobs must be inert on the old C3R.a mortal base"
    );

    let on = Settlement::generate(seed, &heritable_cell(1, 2, true));
    let mut off_cfg = heritable_cell(1, 2, true);
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

    let denied = Settlement::generate(seed, &heritable_cell(1, 2, false));
    assert_ne!(
        on.canonical_bytes(),
        denied.canonical_bytes(),
        "the tool-inheritance switch must be part of tag 28"
    );

    let cap3 = Settlement::generate(seed, &heritable_cell(1, 3, true));
    assert_ne!(
        on.canonical_bytes(),
        cap3.canonical_bytes(),
        "the producer-house cap must be part of tag 28"
    );

    let food2 = Settlement::generate(seed, &heritable_cell(2, 2, true));
    assert_ne!(
        on.canonical_bytes(),
        food2.canonical_bytes(),
        "producer-house food provision rides demography bytes, not tag 28"
    );
}

#[test]
fn v2_sweep_prints_split_classification_without_verdict_assertions() {
    let cases = informative_cases();
    println!(
        "C3R.b.v2 running informative slice: full food sweep {:?} for Control/InheritanceCell at cap {}; cap sweep {:?} at food {}; MortalProducers and FlagOffHeritable once. Dropped for tractability: repeated baselines across axis cells and Control/InheritanceCell cells where cap != {} and food != {}.",
        FOOD_SWEEP,
        DEFAULT_CAP_SLICE,
        CAP_SWEEP,
        VIABLE_SUBSIDY_SLICE,
        DEFAULT_CAP_SLICE,
        VIABLE_SUBSIDY_SLICE
    );
    for seed in SEEDS {
        for case in cases.iter().copied() {
            let cfg = case_config(case);
            let trace = trace_run(seed, case, &cfg);
            let (structure, flow) = classify(&trace);
            print_trace(&trace, structure, flow);
            assert_invariants(&trace);
            if matches!(case.cell, Scenario::FlagOffHeritable) {
                assert!(
                    reached_capital_stage(&trace),
                    "flag-off heritable base should still reach the capital-stage staffing window"
                );
            }
        }
    }
}
