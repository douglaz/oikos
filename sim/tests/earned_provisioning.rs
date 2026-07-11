//! C3R.c -- earned provisioning for mortal producer households.
//!
//! The suite classifies rather than tunes. Verdicts are printed as observations;
//! assertions stay on hard invariants, byte identity, and the mint-on C3R.b
//! reference precondition.

use sim::content::{BREAD_PER_BAKE, FLOUR_PER_BAKE};
use sim::{
    EarnedProvisioningStats, EraDetector, Gold, GoodId, Settlement, SettlementConfig, Vocation,
};

const SEEDS: [u64; 5] = [3, 7, 11, 19, 23];
const RUN_TICKS: u64 = 1_600;
const FINAL_WINDOW: u64 = 160;
const EARLY_START: u64 = 160;
const EARLY_END: u64 = 320;
const MID_START: u64 = 720;
const MID_END: u64 = 880;
const PRODUCER_HOUSEHOLDS: usize = 6;
const LINEAGE_SURROUND: [u32; 3] = [3, 1, 0];
const PRODUCER_HOUSE_CAP: u8 = 2;
const C3RB_REFERENCE_PRODUCER_FOOD: u32 = 1;
const FLOW_RUNS_MIN_BREAD: u64 = 100;
const FLOW_RUNS_MIN_PER_STAFFED_TICK: f64 = 0.10;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Cell {
    MintOnReference,
    EarnedHeadline,
    StockProvisioningControl,
    NoProvisioningControl,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Case {
    cell: Cell,
    lineage_food: Option<u32>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Verdict {
    BaseUnviable,
    ReservoirOpen,
    ConservationBroken,
    RegistryBroken,
    ColdStartGapBites,
    AccountingLoopOnly,
    EarnedIncomeInsufficient,
    SavingsFundBridgesGap,
    StructureAndFlowRun,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct BuyerGold {
    consumers: u64,
    gatherers: u64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct WindowTrace {
    ticks: u64,
    min_millers: usize,
    min_bakers: usize,
    staffed_ticks: u64,
    bread_output: u64,
    bread_trade_count: u64,
    bread_price: Option<u64>,
    spread_positive: bool,
}

impl Default for WindowTrace {
    fn default() -> Self {
        Self {
            ticks: 0,
            min_millers: usize::MAX,
            min_bakers: usize::MAX,
            staffed_ticks: 0,
            bread_output: 0,
            bread_trade_count: 0,
            bread_price: None,
            spread_positive: false,
        }
    }
}

impl WindowTrace {
    fn observe(
        &mut self,
        s: &Settlement,
        bread: GoodId,
        flour: GoodId,
        spot_start: usize,
        barter_start: usize,
        produced_bread: u64,
    ) {
        self.ticks = self.ticks.saturating_add(1);
        let millers = s.living_count(Vocation::Miller);
        let bakers = s.living_count(Vocation::Baker);
        self.min_millers = self.min_millers.min(millers);
        self.min_bakers = self.min_bakers.min(bakers);
        if millers > 0 && bakers > 0 {
            self.staffed_ticks = self.staffed_ticks.saturating_add(1);
        }
        self.bread_output = self.bread_output.saturating_add(produced_bread);
        let trades = tick_bread_trade_count(s, bread, spot_start, barter_start);
        self.bread_trade_count = self.bread_trade_count.saturating_add(trades);
        if trades > 0 {
            self.bread_price = s.realized_price(bread).map(|price| price.0);
        }
        self.spread_positive |= positive_bake_spread(s, bread, flour);
    }

    fn structure_runs(self) -> bool {
        self.ticks > 0 && self.min_millers > 0 && self.min_bakers > 0
    }

    fn bread_per_staffed_tick(self) -> f64 {
        if self.staffed_ticks == 0 {
            0.0
        } else {
            self.bread_output as f64 / self.staffed_ticks as f64
        }
    }

    fn flow_runs(self) -> bool {
        self.structure_runs()
            && matches!(self.bread_price, Some(price) if price != 1)
            && self.spread_positive
            && self.bread_output >= FLOW_RUNS_MIN_BREAD
            && self.bread_per_staffed_tick() >= FLOW_RUNS_MIN_PER_STAFFED_TICK
    }
}

#[derive(Clone, Debug)]
struct Trace {
    seed: u64,
    case: Case,
    conserved: bool,
    money_ok: bool,
    registry_ok: bool,
    immortal_producer_max: usize,
    old_age_deaths: u64,
    starvation_deaths: u64,
    producer_house_starvations: u64,
    producer_house_hearth_food_minted: u64,
    non_producer_hearth_food_minted: u64,
    producer_house_births: u64,
    producer_house_deaths: u64,
    producer_house_live_final: usize,
    producer_households_sustained: bool,
    max_both_stage_streak: u64,
    producer_bread_output: u64,
    bread_consumed: u64,
    bread_bought: u64,
    both_stage_staffed_ticks: u64,
    early: WindowTrace,
    mid: WindowTrace,
    final_window: WindowTrace,
    stats_start: EarnedProvisioningStats,
    stats_mid: EarnedProvisioningStats,
    stats_final: EarnedProvisioningStats,
    late_genuine_external_bread_trades: u64,
    late_genuine_external_revenue: Gold,
    buyer_gold_start: BuyerGold,
    buyer_gold_mid: BuyerGold,
    buyer_gold_final: BuyerGold,
}

fn chain_goods(cfg: &SettlementConfig) -> (GoodId, GoodId) {
    let content = &cfg.chain.as_ref().expect("chain").content;
    (content.flour(), content.bread())
}

fn producer_house_start_for_specs(cfg: &SettlementConfig) -> usize {
    cfg.demography
        .as_ref()
        .expect("demography")
        .households
        .len()
        .checked_sub(PRODUCER_HOUSEHOLDS)
        .expect("producer households appended")
}

fn set_producer_house_food(cfg: &mut SettlementConfig, food: u32) {
    let start = producer_house_start_for_specs(cfg);
    let demo = cfg.demography.as_mut().expect("demography");
    for spec in &mut demo.households[start..] {
        spec.food_provision = food;
    }
}

fn set_lineage_food(cfg: &mut SettlementConfig, food: u32) {
    let start = producer_house_start_for_specs(cfg);
    let demo = cfg.demography.as_mut().expect("demography");
    for spec in &mut demo.households[..start] {
        spec.food_provision = food;
    }
}

fn retire_producer_food_mints(cfg: &mut SettlementConfig) {
    cfg.chain.as_mut().expect("chain").producer_subsistence = 0;
    set_producer_house_food(cfg, 0);
}

fn pin_producer_cap(cfg: &mut SettlementConfig) {
    cfg.chain.as_mut().expect("chain").producer_house_cap = PRODUCER_HOUSE_CAP;
}

fn mint_on_reference_config() -> SettlementConfig {
    let mut cfg = SettlementConfig::frontier_mortal_producers_heritable();
    pin_producer_cap(&mut cfg);
    set_producer_house_food(&mut cfg, C3RB_REFERENCE_PRODUCER_FOOD);
    cfg
}

fn earned_config(lineage_food: u32) -> SettlementConfig {
    let mut cfg = SettlementConfig::frontier_mortal_producers_earned();
    pin_producer_cap(&mut cfg);
    set_lineage_food(&mut cfg, lineage_food);
    cfg
}

fn stock_control_config() -> SettlementConfig {
    let mut cfg = SettlementConfig::frontier_mortal_producers_heritable();
    pin_producer_cap(&mut cfg);
    retire_producer_food_mints(&mut cfg);
    cfg.chain
        .as_mut()
        .expect("chain")
        .producer_stock_provisioning_control = true;
    cfg
}

fn no_provisioning_config() -> SettlementConfig {
    let mut cfg = SettlementConfig::frontier_mortal_producers_heritable();
    pin_producer_cap(&mut cfg);
    retire_producer_food_mints(&mut cfg);
    cfg
}

fn case_config(case: Case) -> SettlementConfig {
    match case.cell {
        Cell::MintOnReference => mint_on_reference_config(),
        Cell::EarnedHeadline => earned_config(case.lineage_food.expect("headline lineage food")),
        Cell::StockProvisioningControl => stock_control_config(),
        Cell::NoProvisioningControl => no_provisioning_config(),
    }
}

fn cases() -> Vec<Case> {
    let mut cases = vec![
        Case {
            cell: Cell::MintOnReference,
            lineage_food: None,
        },
        Case {
            cell: Cell::StockProvisioningControl,
            lineage_food: None,
        },
        Case {
            cell: Cell::NoProvisioningControl,
            lineage_food: None,
        },
    ];
    for lineage_food in LINEAGE_SURROUND {
        cases.push(Case {
            cell: Cell::EarnedHeadline,
            lineage_food: Some(lineage_food),
        });
    }
    cases
}

fn producer_house_start(s: &Settlement) -> Option<usize> {
    (s.household_count() >= PRODUCER_HOUSEHOLDS).then(|| s.household_count() - PRODUCER_HOUSEHOLDS)
}

fn producer_house_living_counts(s: &Settlement) -> Vec<usize> {
    let Some(start) = producer_house_start(s) else {
        return Vec::new();
    };
    let mut counts = vec![0usize; PRODUCER_HOUSEHOLDS];
    for idx in 0..s.population() {
        if !s.is_alive(idx) {
            continue;
        }
        let Some(household) = s.household_of(idx) else {
            continue;
        };
        if let Some(offset) = household.checked_sub(start) {
            if let Some(count) = counts.get_mut(offset) {
                *count += 1;
            }
        }
    }
    counts
}

fn producer_house_member_count(s: &Settlement) -> usize {
    producer_house_living_counts(s).into_iter().sum()
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

fn positive_bake_spread(s: &Settlement, bread: GoodId, flour: GoodId) -> bool {
    let Some(bread_price) = s.realized_price(bread) else {
        return false;
    };
    let Some(flour_price) = s.realized_price(flour) else {
        return false;
    };
    u128::from(bread_price.0) * u128::from(BREAD_PER_BAKE)
        > u128::from(flour_price.0) * u128::from(FLOUR_PER_BAKE)
}

fn buyer_gold(s: &Settlement) -> BuyerGold {
    let mut gold = BuyerGold {
        consumers: 0,
        gatherers: 0,
    };
    for (vocation, amount) in s.gold_by_vocation() {
        match vocation {
            Vocation::Consumer => gold.consumers = gold.consumers.saturating_add(amount),
            Vocation::Gatherer => gold.gatherers = gold.gatherers.saturating_add(amount),
            _ => {}
        }
    }
    gold
}

fn trace_run(seed: u64, case: Case, cfg: &SettlementConfig) -> Trace {
    let (flour, bread) = chain_goods(cfg);
    let mut s = Settlement::generate(seed, cfg);
    let mut detector = EraDetector::new();
    let mut conserved = true;
    let mut money_ok = true;
    let mut immortal_producer_max = s.immortal_producer_count();
    let mut producer_bread_output = 0u64;
    let mut bread_consumed = 0u64;
    let mut both_stage_staffed_ticks = 0u64;
    let mut max_both_stage_streak = 0u64;
    let mut current_both_stage_streak = 0u64;
    let mut early = WindowTrace::default();
    let mut mid = WindowTrace::default();
    let mut final_window = WindowTrace::default();
    let stats_start = s.earned_provisioning_stats();
    let buyer_gold_start = buyer_gold(&s);
    let mut stats_mid = stats_start;
    let mut buyer_gold_mid = buyer_gold_start;
    let mut stats_before_late = stats_start;

    for tick in 0..RUN_TICKS {
        if tick + FINAL_WINDOW == RUN_TICKS {
            stats_before_late = s.earned_provisioning_stats();
        }

        let spot_start = s.society().trades.len();
        let barter_start = s.society().barter_trades.len();
        let report = s.econ_tick();
        conserved &= report.conserves();
        money_ok &= report.money_conserves();
        detector.observe(&s);

        let produced = report.produced_of(bread);
        producer_bread_output = producer_bread_output.saturating_add(produced);
        bread_consumed = bread_consumed.saturating_add(report.consumed_of(bread));
        immortal_producer_max = immortal_producer_max.max(s.immortal_producer_count());

        let millers = s.living_count(Vocation::Miller);
        let bakers = s.living_count(Vocation::Baker);
        if millers > 0 && bakers > 0 {
            both_stage_staffed_ticks = both_stage_staffed_ticks.saturating_add(1);
            current_both_stage_streak = current_both_stage_streak.saturating_add(1);
            max_both_stage_streak = max_both_stage_streak.max(current_both_stage_streak);
        } else {
            current_both_stage_streak = 0;
        }

        if (EARLY_START..EARLY_END).contains(&tick) {
            early.observe(&s, bread, flour, spot_start, barter_start, produced);
        }
        if (MID_START..MID_END).contains(&tick) {
            mid.observe(&s, bread, flour, spot_start, barter_start, produced);
        }
        if tick + FINAL_WINDOW >= RUN_TICKS {
            final_window.observe(&s, bread, flour, spot_start, barter_start, produced);
        }

        if tick + 1 == RUN_TICKS / 2 {
            stats_mid = s.earned_provisioning_stats();
            buyer_gold_mid = buyer_gold(&s);
        }
    }

    let stats_final = s.earned_provisioning_stats();
    let late_genuine_external_bread_trades = stats_final
        .genuine_external_bread_trades
        .saturating_sub(stats_before_late.genuine_external_bread_trades);
    let late_genuine_external_revenue = stats_final
        .genuine_external_revenue
        .saturating_sub(stats_before_late.genuine_external_revenue);
    let producer_counts = producer_house_living_counts(&s);
    let producer_households_sustained =
        !producer_counts.is_empty() && producer_counts.iter().all(|count| *count > 0);

    Trace {
        seed,
        case,
        conserved,
        money_ok,
        registry_ok: s.private_land_registry_invariant_holds(),
        immortal_producer_max,
        old_age_deaths: s.mortal_producer_old_age_deaths(),
        starvation_deaths: s.starvation_deaths_total(),
        producer_house_starvations: stats_final.member_starvations,
        producer_house_hearth_food_minted: s.producer_house_hearth_food_minted(),
        non_producer_hearth_food_minted: s.non_producer_hearth_food_minted(),
        producer_house_births: s.producer_house_births(),
        producer_house_deaths: s.producer_house_deaths(),
        producer_house_live_final: producer_house_member_count(&s),
        producer_households_sustained,
        max_both_stage_streak,
        producer_bread_output,
        bread_consumed,
        bread_bought: s.trade_volume_of(bread),
        both_stage_staffed_ticks,
        early,
        mid,
        final_window,
        stats_start,
        stats_mid,
        stats_final,
        late_genuine_external_bread_trades,
        late_genuine_external_revenue,
        buyer_gold_start,
        buyer_gold_mid,
        buyer_gold_final: buyer_gold(&s),
    }
}

fn reference_viable(trace: &Trace) -> bool {
    trace.conserved
        && trace.money_ok
        && trace.registry_ok
        && trace.immortal_producer_max == 0
        && trace.old_age_deaths > 0
        && trace.max_both_stage_streak >= EraDetector::new().window()
}

fn savings_bridge(trace: &Trace) -> bool {
    (trace.early.flow_runs() || trace.mid.flow_runs())
        && !trace.final_window.flow_runs()
        && trace.stats_final.endowment_funded_provisioning > Gold::ZERO
        && trace.late_genuine_external_bread_trades == 0
}

fn classify(trace: &Trace, base_viable: bool) -> Verdict {
    if !base_viable {
        return Verdict::BaseUnviable;
    }
    if trace.immortal_producer_max > 0 {
        return Verdict::ReservoirOpen;
    }
    if !trace.conserved || !trace.money_ok {
        return Verdict::ConservationBroken;
    }
    if !trace.registry_ok {
        return Verdict::RegistryBroken;
    }
    if !trace.final_window.structure_runs()
        && trace.stats_final.genuine_external_revenue == Gold::ZERO
        && trace.stats_final.genuine_external_bread_trades == 0
    {
        return Verdict::ColdStartGapBites;
    }
    if trace.stats_final.genuine_external_revenue == Gold::ZERO
        && trace.stats_final.from_other_producer_households > Gold::ZERO
        && trace.stats_final.from_other_producer_households
            >= trace
                .stats_final
                .external_earned_revenue
                .saturating_sub(trace.stats_final.from_other_producer_households)
    {
        return Verdict::AccountingLoopOnly;
    }
    let savings_bridge = savings_bridge(trace);
    if trace.stats_final.genuine_external_revenue > Gold::ZERO
        && !trace.final_window.flow_runs()
        && !savings_bridge
    {
        return Verdict::EarnedIncomeInsufficient;
    }
    if savings_bridge {
        return Verdict::SavingsFundBridgesGap;
    }
    if trace.final_window.flow_runs()
        && trace.stats_final.genuine_external_revenue >= trace.stats_final.provisioning_gold
        && trace.producer_households_sustained
    {
        return Verdict::StructureAndFlowRun;
    }
    Verdict::EarnedIncomeInsufficient
}

fn assert_ledger_split(trace: &Trace) {
    let stats = trace.stats_final;
    let split = stats
        .from_immortal_consumers
        .saturating_add(stats.from_gatherers)
        .saturating_add(stats.from_lineage)
        .saturating_add(stats.from_other_producer_households);
    assert_eq!(
        split, stats.external_earned_revenue,
        "earned provisioning buyer-class split drifted: {trace:?}"
    );
    assert!(
        stats.genuine_external_revenue <= stats.external_earned_revenue,
        "genuine external revenue must be a subset of external revenue: {trace:?}"
    );
}

fn assert_retired_mint_config(cfg: &SettlementConfig) {
    let chain = cfg.chain.as_ref().expect("chain");
    assert_eq!(chain.producer_subsistence, 0);
    let start = producer_house_start_for_specs(cfg);
    let demo = cfg.demography.as_ref().expect("demography");
    assert!(
        demo.households[start..]
            .iter()
            .all(|household| household.food_provision == 0),
        "producer-house food provisions must be retired"
    );
}

fn assert_hard_invariants(trace: &Trace) {
    assert!(trace.conserved, "conservation failed: {trace:?}");
    assert!(trace.money_ok, "money invariant failed: {trace:?}");
    assert!(trace.registry_ok, "registry invariant failed: {trace:?}");
    assert_eq!(
        trace.immortal_producer_max, 0,
        "mortal producer reservoir opened: {trace:?}"
    );
    assert_ledger_split(trace);
}

fn print_trace(trace: &Trace, verdict: Verdict) {
    let stats = trace.stats_final;
    println!(
        "C3R.c seed={} cell={:?} lineage_food={:?} verdict={:?} destructive_lineage_zero={} \
         conserved={} money={} registry={} immortal_max={} old_deaths={} starvation_deaths={} \
         producer_starvations={} producer_live_final={} producer_households_sustained={} \
         producer_births={} producer_deaths={} producer_hearth_food={} nonproducer_hearth_food={} \
         max_both_stage_streak={} both_stage_staffed_ticks={} producer_bread_output={} \
         bread_consumed={} bread_bought={} final_min=({},{}) final_flow={} \
         final_bread_output={} final_bread_price={:?} final_bread_trades={} \
         final_spread_positive={} final_bread_per_staffed={:.4} \
         early_flow={} mid_flow={} late_genuine_trades={} late_genuine_revenue={} \
         earned_total={} genuine={} from_consumers={} from_gatherers={} from_lineage={} \
         from_other_producers={} intra_household_sales={} non_bread_ext_earned={} non_bread_prod_earned={} external_trades={} genuine_trades={} \
         intra_trades={} provisioning_transfers={} provisioning_gold={} \
         endowment_funded_provisioning={} members_fed_by_purchase={} funded_but_unfilled={} \
         buyer_gold_start=({},{}) buyer_gold_mid=({},{}) buyer_gold_final=({},{}) \
         stats_start_genuine={} stats_mid_genuine={}",
        trace.seed,
        trace.case.cell,
        trace.case.lineage_food,
        verdict,
        trace.case.cell == Cell::EarnedHeadline && trace.case.lineage_food == Some(0),
        trace.conserved,
        trace.money_ok,
        trace.registry_ok,
        trace.immortal_producer_max,
        trace.old_age_deaths,
        trace.starvation_deaths,
        trace.producer_house_starvations,
        trace.producer_house_live_final,
        trace.producer_households_sustained,
        trace.producer_house_births,
        trace.producer_house_deaths,
        trace.producer_house_hearth_food_minted,
        trace.non_producer_hearth_food_minted,
        trace.max_both_stage_streak,
        trace.both_stage_staffed_ticks,
        trace.producer_bread_output,
        trace.bread_consumed,
        trace.bread_bought,
        trace.final_window.min_millers,
        trace.final_window.min_bakers,
        trace.final_window.flow_runs(),
        trace.final_window.bread_output,
        trace.final_window.bread_price,
        trace.final_window.bread_trade_count,
        trace.final_window.spread_positive,
        trace.final_window.bread_per_staffed_tick(),
        trace.early.flow_runs(),
        trace.mid.flow_runs(),
        trace.late_genuine_external_bread_trades,
        trace.late_genuine_external_revenue.0,
        stats.external_earned_revenue.0,
        stats.genuine_external_revenue.0,
        stats.from_immortal_consumers.0,
        stats.from_gatherers.0,
        stats.from_lineage.0,
        stats.from_other_producer_households.0,
        stats.intra_household_sales.0,
        stats.non_bread_external_earned.0,
        stats.non_bread_producer_class_earned.0,
        stats.external_bread_trades,
        stats.genuine_external_bread_trades,
        stats.intra_household_bread_trades,
        stats.provisioning_transfers,
        stats.provisioning_gold.0,
        stats.endowment_funded_provisioning.0,
        stats.members_fed_by_purchase,
        stats.funded_but_unfilled,
        trace.buyer_gold_start.consumers,
        trace.buyer_gold_start.gatherers,
        trace.buyer_gold_mid.consumers,
        trace.buyer_gold_mid.gatherers,
        trace.buyer_gold_final.consumers,
        trace.buyer_gold_final.gatherers,
        trace.stats_start.genuine_external_revenue.0,
        trace.stats_mid.genuine_external_revenue.0
    );
}

#[test]
fn old_bases_are_byte_identical_and_tag_29_splits_earned_config() {
    let seed = SEEDS[0];

    for (name, mut cfg) in [
        ("frontier", SettlementConfig::frontier()),
        ("frontier_capital", SettlementConfig::frontier_capital()),
        (
            "frontier_mortal_producers",
            SettlementConfig::frontier_mortal_producers(),
        ),
    ] {
        let base = Settlement::generate(seed, &cfg).canonical_bytes();
        let chain = cfg.chain.as_mut().expect("chain");
        chain.earned_provisioning = true;
        chain.producer_stock_provisioning_control = true;
        assert_eq!(
            base,
            Settlement::generate(seed, &cfg).canonical_bytes(),
            "C3R.c flags must be inert on old base {name}"
        );
    }

    let heritable = Settlement::generate(
        seed,
        &SettlementConfig::frontier_mortal_producers_heritable(),
    )
    .canonical_bytes();
    let mut explicit_off = SettlementConfig::frontier_mortal_producers_heritable();
    {
        let chain = explicit_off.chain.as_mut().expect("chain");
        chain.earned_provisioning = false;
        chain.producer_stock_provisioning_control = false;
    }
    assert_eq!(
        heritable,
        Settlement::generate(seed, &explicit_off).canonical_bytes(),
        "C3R.c default-off fields must leave the old heritable base byte-identical"
    );

    let on = Settlement::generate(seed, &SettlementConfig::frontier_mortal_producers_earned())
        .canonical_bytes();
    let mut off_cfg = SettlementConfig::frontier_mortal_producers_earned();
    off_cfg.chain.as_mut().expect("chain").earned_provisioning = false;
    let off = Settlement::generate(seed, &off_cfg).canonical_bytes();
    assert_ne!(
        on, off,
        "earned_provisioning must split the canonical stream under tag 29"
    );
    assert!(
        on.windows(2).any(|window| window == [29, 1]),
        "active earned_provisioning stream should include tag 29 followed by the flag byte"
    );
}

#[test]
fn earned_provisioning_cells_print_verdicts_without_asserting_success() {
    println!(
        "C3R.c running full required grid: cells=[mint-on reference, stock control, no-provisioning, earned lineage_food {:?}], seeds={:?}, ticks={}, producer_house_cap={}. The lineage_food=0 point is destructive/disclosure-only.",
        LINEAGE_SURROUND, SEEDS, RUN_TICKS, PRODUCER_HOUSE_CAP
    );

    for seed in SEEDS {
        let reference_case = Case {
            cell: Cell::MintOnReference,
            lineage_food: None,
        };
        let reference_cfg = case_config(reference_case);
        let reference_trace = trace_run(seed, reference_case, &reference_cfg);
        let base_viable = reference_viable(&reference_trace);
        let reference_verdict = classify(&reference_trace, base_viable);
        print_trace(&reference_trace, reference_verdict);
        assert_hard_invariants(&reference_trace);
        assert!(
            base_viable,
            "mint-on C3R.b reference did not reproduce the landed viable grid: {reference_trace:?}"
        );

        for case in cases()
            .into_iter()
            .filter(|case| case.cell != Cell::MintOnReference)
        {
            let cfg = case_config(case);
            assert_retired_mint_config(&cfg);
            let trace = trace_run(seed, case, &cfg);
            let verdict = classify(&trace, base_viable);
            print_trace(&trace, verdict);
            assert_hard_invariants(&trace);
            assert_eq!(
                trace.producer_house_hearth_food_minted, 0,
                "producer household food mint must be retired in C3R.c cells: {trace:?}"
            );
        }
    }
}
