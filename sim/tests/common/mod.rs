//! Shared C3R.d birth-stock-saving classifier machinery (impl-66 repair §1).
//!
//! Extracted VERBATIM (behavior-preserving) from `tests/birth_stock_saving.rs` so the C3R.d
//! suite and the C3R.e-obs acceptance suite classify every cell through the SAME real
//! classifier — no re-derivation, no predetermined verdict labels. Both suites include this
//! file with the standard `mod common;` pattern. The `Cell`/`Verdict`/`ReferenceVerdict`
//! enums, the `Trace`/`FinalWindow`/`GoldTrend` records, `config`, `trace`, `classify`,
//! `reference_verdict`, and their helpers are the move; `trace_with_config` is the additive
//! seam that lets a caller run a cell with a MUTATED config (e.g. the Headline cell with the
//! allocation-obs flag ON — behaviorally inert, so it reproduces the landed verdict).
//!
//! Every accessor it calls on [`Settlement`] is read-only, so tracing a cell perturbs the run
//! not at all and adds no engine code.
#![allow(dead_code)]

use sim::content::{BREAD_PER_BAKE, FLOUR_PER_BAKE};
use sim::{
    BirthStockSavingMode, EarnedProvisioningStats, GoodId, Settlement, SettlementConfig, Vocation,
};

pub const RUN_TICKS: u64 = 1_600;
pub const FINAL_WINDOW: u64 = 160;
pub const PRODUCER_HOUSEHOLDS: usize = 6;
pub const FLOW_RUNS_MIN_BREAD: u64 = 100;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Cell {
    Headline,
    SufficiencyControl,
    NoMotiveReference,
    MintOnReference,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ReferenceVerdict {
    FedButChildless,
    Drifted,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Verdict {
    BaseUnviable,
    ReservoirOpen,
    ConservationBroken,
    RegistryBroken,
    SufficiencyControlInconclusive,
    BirthGateNotSoleBlocker,
    SavingMotiveInert,
    BirthStockRaceLost,
    StockReachedBirthsStillBlocked,
    BirthsResumeStructureStillDies,
    ContinuityRestored,
    UnclassifiedMixed,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct GoldTrend {
    pub consumers: u64,
    pub gatherers: u64,
    pub millers: u64,
    pub bakers: u64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FinalWindow {
    pub ticks: u64,
    pub min_millers: usize,
    pub min_bakers: usize,
    pub staffed_ticks: u64,
    pub bread_output: u64,
    pub bread_trades: u64,
    pub bread_price: Option<u64>,
    pub spread_positive: bool,
}

impl Default for FinalWindow {
    fn default() -> Self {
        Self {
            ticks: 0,
            min_millers: usize::MAX,
            min_bakers: usize::MAX,
            staffed_ticks: 0,
            bread_output: 0,
            bread_trades: 0,
            bread_price: None,
            spread_positive: false,
        }
    }
}

impl FinalWindow {
    pub fn structure_runs(self) -> bool {
        self.ticks > 0 && self.min_millers > 0 && self.min_bakers > 0
    }

    pub fn flow_runs(self) -> bool {
        self.structure_runs()
            && self.bread_price.is_some_and(|price| price != 1)
            && self.spread_positive
            && self.bread_output >= FLOW_RUNS_MIN_BREAD
            && self.bread_output.saturating_mul(10) >= self.staffed_ticks
    }
}

#[derive(Clone, Debug)]
pub struct Trace {
    pub seed: u64,
    pub cell: Cell,
    pub conserved: bool,
    pub money_ok: bool,
    pub registry_ok: bool,
    pub immortal_producer_max: usize,
    pub producer_births: u64,
    pub births_by_household: Vec<u64>,
    pub producer_deaths: u64,
    pub producer_starvations: u64,
    pub producer_hearth_food: u64,
    pub wants: u64,
    pub attributable_purchases: u64,
    pub reached_four: usize,
    pub held_max: u32,
    pub held_at_death: u32,
    pub eligible_opportunities: u64,
    pub injections_completed: u64,
    pub source_shortfalls: u64,
    pub injection_records: usize,
    pub failed_injected_births: usize,
    pub birth_block_interval: u64,
    pub birth_block_size_cap: u64,
    pub birth_block_hunger_ceiling: u64,
    pub birth_block_endowment: u64,
    pub final_window: FinalWindow,
    pub stats: EarnedProvisioningStats,
    pub gold_start: GoldTrend,
    pub gold_mid: GoldTrend,
    pub gold_final: GoldTrend,
}

pub fn config(cell: Cell) -> SettlementConfig {
    match cell {
        Cell::Headline => SettlementConfig::frontier_mortal_producers_saving(),
        Cell::SufficiencyControl => {
            let mut cfg = SettlementConfig::frontier_mortal_producers_earned();
            cfg.chain.as_mut().expect("chain").birth_stock_saving_mode =
                BirthStockSavingMode::SufficiencyControl;
            cfg
        }
        Cell::NoMotiveReference => SettlementConfig::frontier_mortal_producers_earned(),
        Cell::MintOnReference => {
            let mut cfg = SettlementConfig::frontier_mortal_producers_heritable();
            let demo = cfg.demography.as_mut().expect("demography");
            let start = demo
                .households
                .len()
                .checked_sub(PRODUCER_HOUSEHOLDS)
                .expect("producer households appended");
            for household in &mut demo.households[start..] {
                household.food_provision = 1;
            }
            cfg.chain.as_mut().expect("chain").producer_house_cap = 2;
            cfg
        }
    }
}

fn chain_goods(cfg: &SettlementConfig) -> (GoodId, GoodId) {
    let content = &cfg.chain.as_ref().expect("chain").content;
    (content.flour(), content.bread())
}

fn producer_house_start(s: &Settlement) -> usize {
    s.household_count()
        .checked_sub(PRODUCER_HOUSEHOLDS)
        .expect("producer households appended")
}

fn gold_trend(s: &Settlement) -> GoldTrend {
    let mut trend = GoldTrend::default();
    for (vocation, gold) in s.gold_by_vocation() {
        match vocation {
            Vocation::Consumer => trend.consumers = trend.consumers.saturating_add(gold),
            Vocation::Gatherer => trend.gatherers = trend.gatherers.saturating_add(gold),
            Vocation::Miller => trend.millers = trend.millers.saturating_add(gold),
            Vocation::Baker => trend.bakers = trend.bakers.saturating_add(gold),
            _ => {}
        }
    }
    trend
}

fn positive_bake_spread(s: &Settlement, bread: GoodId, flour: GoodId) -> bool {
    let (Some(bread_price), Some(flour_price)) = (s.realized_price(bread), s.realized_price(flour))
    else {
        return false;
    };
    u128::from(bread_price.0) * u128::from(BREAD_PER_BAKE)
        > u128::from(flour_price.0) * u128::from(FLOUR_PER_BAKE)
}

/// Trace the given `cell` on `seed` with its canonical [`config`]. The C3R.d suite's entry.
pub fn trace(seed: u64, cell: Cell) -> Trace {
    trace_with_config(seed, cell, config(cell)).0
}

/// Trace `cell` on `seed` with an explicit (possibly MUTATED) `cfg`, returning both the
/// classified [`Trace`] and the driven [`Settlement`] so a caller can also read the run's
/// runtime-only observers (e.g. the allocation-obs report). The metrics are read-only
/// accessors, so a config that only toggles a behaviorally-inert observation flag reproduces
/// the same `Trace` and thus the same [`classify`] verdict.
pub fn trace_with_config(seed: u64, cell: Cell, cfg: SettlementConfig) -> (Trace, Settlement) {
    let (flour, bread) = chain_goods(&cfg);
    let mut s = Settlement::generate(seed, &cfg);
    let gold_start = gold_trend(&s);
    let mut gold_mid = gold_start;
    let mut conserved = true;
    let mut money_ok = true;
    let mut immortal_producer_max = 0usize;
    let mut final_window = FinalWindow::default();
    for tick in 0..RUN_TICKS {
        let spot_start = s.society().trades.len();
        let barter_start = s.society().barter_trades.len();
        let report = s.econ_tick();
        conserved &= report.conserves();
        money_ok &= report.money_conserves();
        immortal_producer_max = immortal_producer_max.max(s.immortal_producer_count());
        if tick + 1 == RUN_TICKS / 2 {
            gold_mid = gold_trend(&s);
        }
        if tick + FINAL_WINDOW >= RUN_TICKS {
            let millers = s.living_count(Vocation::Miller);
            let bakers = s.living_count(Vocation::Baker);
            final_window.ticks = final_window.ticks.saturating_add(1);
            final_window.min_millers = final_window.min_millers.min(millers);
            final_window.min_bakers = final_window.min_bakers.min(bakers);
            if millers > 0 && bakers > 0 {
                final_window.staffed_ticks = final_window.staffed_ticks.saturating_add(1);
            }
            final_window.bread_output = final_window
                .bread_output
                .saturating_add(report.produced_of(bread));
            let spot_trades = s.society().trades[spot_start..]
                .iter()
                .filter(|trade| trade.good == bread)
                .count() as u64;
            let barter_trades = s.society().barter_trades[barter_start..]
                .iter()
                .filter(|trade| trade.a_gives == bread || trade.b_gives == bread)
                .count() as u64;
            let trades = spot_trades.saturating_add(barter_trades);
            final_window.bread_trades = final_window.bread_trades.saturating_add(trades);
            if trades > 0 {
                final_window.bread_price = s.realized_price(bread).map(|price| price.0);
            }
            final_window.spread_positive |= positive_bake_spread(&s, bread, flour);
        }
    }
    let producer_start = producer_house_start(&s);
    let births_by_household = s.births_by_household()[producer_start..].to_vec();
    let stats = s.earned_provisioning_stats();
    let trace = Trace {
        seed,
        cell,
        conserved,
        money_ok,
        registry_ok: s.private_land_registry_invariant_holds(),
        immortal_producer_max,
        producer_births: s.producer_house_births(),
        births_by_household,
        producer_deaths: s.producer_house_deaths(),
        producer_starvations: stats.member_starvations,
        producer_hearth_food: s.producer_house_hearth_food_minted(),
        wants: s.birth_stock_wants_emitted(),
        attributable_purchases: s.birth_stock_attributable_purchases(),
        reached_four: s.birth_stock_reached_four_count(),
        held_max: s.birth_stock_held_max(),
        held_at_death: s.birth_stock_held_at_death(),
        eligible_opportunities: s.birth_stock_eligible_opportunities(),
        injections_completed: s.birth_stock_injections_completed(),
        source_shortfalls: s.birth_stock_source_shortfalls(),
        injection_records: s.birth_stock_injection_records().len(),
        failed_injected_births: s
            .birth_stock_injection_records()
            .iter()
            .filter(|record| !record.birth_succeeded)
            .count(),
        birth_block_interval: s.birth_block_interval(),
        birth_block_size_cap: s.birth_block_size_cap(),
        birth_block_hunger_ceiling: s.birth_block_hunger_ceiling(),
        birth_block_endowment: s.birth_block_endowment(),
        final_window,
        stats,
        gold_start,
        gold_mid,
        gold_final: gold_trend(&s),
    };
    (trace, s)
}

pub fn reference_verdict(trace: &Trace) -> ReferenceVerdict {
    if trace.producer_starvations == 0
        && trace.producer_births <= 5
        && !trace.final_window.structure_runs()
    {
        ReferenceVerdict::FedButChildless
    } else {
        ReferenceVerdict::Drifted
    }
}

pub fn hard_invariants(trace: &Trace) -> bool {
    trace.conserved && trace.money_ok && trace.registry_ok && trace.immortal_producer_max == 0
}

pub fn classify(headline: &Trace, control: &Trace, reference: &Trace, mint_on: &Trace) -> Verdict {
    if reference_verdict(reference) != ReferenceVerdict::FedButChildless
        || !mint_on.final_window.structure_runs()
    {
        return Verdict::BaseUnviable;
    }
    if [headline, control, reference, mint_on]
        .iter()
        .any(|trace| trace.immortal_producer_max > 0)
    {
        return Verdict::ReservoirOpen;
    }
    if [headline, control, reference, mint_on]
        .iter()
        .any(|trace| !trace.conserved || !trace.money_ok)
    {
        return Verdict::ConservationBroken;
    }
    if [headline, control, reference, mint_on]
        .iter()
        .any(|trace| !trace.registry_ok)
    {
        return Verdict::RegistryBroken;
    }
    if control.injections_completed == 0 {
        return Verdict::SufficiencyControlInconclusive;
    }
    if control.failed_injected_births > 0 {
        return Verdict::BirthGateNotSoleBlocker;
    }
    if headline.wants == 0 || headline.attributable_purchases == 0 {
        return Verdict::SavingMotiveInert;
    }
    if headline.attributable_purchases > 0
        && headline.reached_four == 0
        && headline.producer_births <= reference.producer_births
    {
        return Verdict::BirthStockRaceLost;
    }
    if headline.reached_four > 0 && headline.producer_births <= reference.producer_births {
        return Verdict::StockReachedBirthsStillBlocked;
    }
    if headline.producer_births > reference.producer_births
        && !headline.final_window.structure_runs()
    {
        return Verdict::BirthsResumeStructureStillDies;
    }
    if headline.producer_births > reference.producer_births
        && headline.final_window.structure_runs()
    {
        return Verdict::ContinuityRestored;
    }
    Verdict::UnclassifiedMixed
}

pub fn assert_ledger_split(trace: &Trace) {
    let stats = trace.stats;
    let split = stats
        .from_immortal_consumers
        .saturating_add(stats.from_gatherers)
        .saturating_add(stats.from_lineage)
        .saturating_add(stats.from_other_producer_households);
    assert_eq!(split, stats.external_earned_revenue, "{trace:?}");
    assert!(stats.genuine_external_revenue <= stats.external_earned_revenue);
}

pub fn print_trace(trace: &Trace, verdict: Verdict) {
    println!(
        "C3R.d seed={} cell={:?} verdict={:?} conserved={} money={} registry={} immortal={} \
         births={} per_house={:?} deaths={} starvations={} producer_hearth_food={} \
         wants={} attributable={} reached_four={} held_max={} held_at_death={} \
         eligible={} injections={} shortfalls={} records={} failed_injected_births={} \
         blocks=(interval:{} cap:{} hunger:{} endowment:{}) final_min=({},{}) \
         final_output={} final_trades={} final_price={:?} structure={} flow={} \
         ledger=(external:{} genuine:{} intra:{} non_bread_ext:{} non_bread_prod:{} provisioning:{}) \
         gold_start={:?} gold_mid={:?} gold_final={:?}",
        trace.seed,
        trace.cell,
        verdict,
        trace.conserved,
        trace.money_ok,
        trace.registry_ok,
        trace.immortal_producer_max,
        trace.producer_births,
        trace.births_by_household,
        trace.producer_deaths,
        trace.producer_starvations,
        trace.producer_hearth_food,
        trace.wants,
        trace.attributable_purchases,
        trace.reached_four,
        trace.held_max,
        trace.held_at_death,
        trace.eligible_opportunities,
        trace.injections_completed,
        trace.source_shortfalls,
        trace.injection_records,
        trace.failed_injected_births,
        trace.birth_block_interval,
        trace.birth_block_size_cap,
        trace.birth_block_hunger_ceiling,
        trace.birth_block_endowment,
        trace.final_window.min_millers,
        trace.final_window.min_bakers,
        trace.final_window.bread_output,
        trace.final_window.bread_trades,
        trace.final_window.bread_price,
        trace.final_window.structure_runs(),
        trace.final_window.flow_runs(),
        trace.stats.external_earned_revenue.0,
        trace.stats.genuine_external_revenue.0,
        trace.stats.intra_household_sales.0,
        trace.stats.non_bread_external_earned.0,
        trace.stats.non_bread_producer_class_earned.0,
        trace.stats.provisioning_gold.0,
        trace.gold_start,
        trace.gold_mid,
        trace.gold_final,
    );
}
