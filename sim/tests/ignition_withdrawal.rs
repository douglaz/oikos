//! C3R.e (impl-67) — Ignition and Withdrawal: the keystone's endgame.
//!
//! Can a FINITE intervention put the trapped mortal producer economy into a state that survives
//! the intervention's removal? Three finite interventions —
//! - **A1** one-shot conserved redistribution at tick 50 (`birth_stock_ignition_at`),
//! - **A2** generation-time additive producer-house endowment (`producer_house_starting_staple`),
//! - **B** the landed staple support pair (producer `food_provision = 1` AND
//!   `producer_subsistence = 4`) withdrawn at tick 400 (`producer_support_until_tick`) —
//!
//! are run on two disclosed regimes ({current}; {durable} = producer `wood_provision = 0` +
//! `gatherers = 48`), measured against a six-criterion hysteresis bar over ABSOLUTE 160-tick window
//! grids. The load-bearing new mechanism is a LAUNDER-PROOF intervention-origin flag on ledger lots
//! (see `AcquisitionLedger`); eligibility begins only once the GLOBAL intervention-origin held == 0.
//!
//! Everything (the verdict + all six criteria per window) is PRINTED, never asserted — the honest
//! default is relapse and the experiment is not tuned toward `HysteresisHolds`. The asserted hard
//! guards are invariants only: conservation/money/registry/reservoir, the drawn-lot funding sum, and
//! the landed C3R.d anchor (the {current} no-ignition cell reproduces the earned NoMotiveReference
//! births `[2, 3, 5, 2, 1]`). "Multiple equilibria / big push" language appears ONLY on the
//! `HysteresisHolds` print path.
//!
//! Run: `cargo test -p sim --test ignition_withdrawal -- --nocapture`.

use sim::content::{BREAD_PER_BAKE, FLOUR_PER_BAKE};
use sim::{
    classify_closure, ClosureTickAgg, ClosureVerdict, ClosureWindow, GoodId, Settlement,
    SettlementConfig, Vocation,
};

const SEEDS: [u64; 5] = [3, 7, 11, 19, 23];
const RUN_TICKS: u64 = 1_600;
/// The persistence window (≈ 6 producer lifetimes, mean lifespan ≈ 27).
const N: u64 = 160;
/// The required consecutive-eligible-window streak for `HysteresisHolds`.
const M: usize = 5;
const PRODUCER_HOUSEHOLDS: usize = 6;
const FLOW_RUNS_MIN_BREAD: u64 = 100;
/// The A1 ignition tick (T).
const IGNITION_TICK: u64 = 50;
/// The B support-withdrawal tick (W).
const SUPPORT_UNTIL: u64 = 400;
/// The full A1 ignition dose = 6 producer households × the 4-loaf `child_food_endowment`.
const IGNITION_DOSE: u64 = 24;
/// The child food endowment funding one birth (frontier pin).
const CHILD_FOOD_ENDOWMENT: u64 = 4;
/// The A2 additive per-producer-subject staple endowment.
const A2_ENDOWMENT: u32 = 4;
/// The B cushion (`producer_subsistence`) staple leg.
const CUSHION: u32 = 4;
/// The B producer-house `food_provision` hearth.
const HEARTH_FOOD: u32 = 1;
/// The {durable} regime gatherer count (double the base 24 — a minority, not swept).
const DURABLE_GATHERERS: u16 = 48;
/// The landed C3R.d NoMotiveReference producer births (the earned trap), pinned as an executable
/// anchor: the {current} no-ignition cell must reproduce it (the acquisition ledger is behaviorally
/// inert).
const EXPECTED_NO_MOTIVE_BIRTHS: [u64; 5] = [2, 3, 5, 2, 1];

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Regime {
    Durable,
    Current,
    /// DH.a (impl-68): the closed circulation — the durable stack MINUS the endowed non-producing
    /// surround (`consumers = 0`, the 2 legacy lineage households removed), instrumented by the
    /// closure ledger. Re-poses the ignition question on a genuinely closed regime.
    Closed,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Intervention {
    BWithdrawn,
    A1Redistribution,
    A2Additive,
    BNeverWithdrawn,
    NoIgnition,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Cell {
    regime: Regime,
    intervention: Intervention,
}

impl Cell {
    fn label(self) -> String {
        format!("{{{:?}, {:?}}}", self.regime, self.intervention)
    }
    /// Is this an INTERVENTION cell (the §2 ladder applies) or a control (reference outcomes)?
    fn is_intervention(self) -> bool {
        matches!(
            self.intervention,
            Intervention::BWithdrawn | Intervention::A1Redistribution | Intervention::A2Additive
        )
    }
}

/// The 7-cell spine (§6).
const CELLS: [Cell; 7] = [
    Cell {
        regime: Regime::Durable,
        intervention: Intervention::BWithdrawn,
    },
    Cell {
        regime: Regime::Durable,
        intervention: Intervention::A1Redistribution,
    },
    Cell {
        regime: Regime::Durable,
        intervention: Intervention::A2Additive,
    },
    Cell {
        regime: Regime::Durable,
        intervention: Intervention::BNeverWithdrawn,
    },
    Cell {
        regime: Regime::Durable,
        intervention: Intervention::NoIgnition,
    },
    Cell {
        regime: Regime::Current,
        intervention: Intervention::BWithdrawn,
    },
    Cell {
        regime: Regime::Current,
        intervention: Intervention::NoIgnition,
    },
];

/// DH.a (impl-68): the 5 Closed cells, appended after the landed 7 in the landed intervention order
/// (BWithdrawn, A1, A2, BNeverWithdrawn, NoIgnition). Closure is evaluated on the Closed NoIgnition
/// trace; every Closed cell prints the closure preamble before the unchanged ladder verdict.
const CLOSED_CELLS: [Cell; 5] = [
    Cell {
        regime: Regime::Closed,
        intervention: Intervention::BWithdrawn,
    },
    Cell {
        regime: Regime::Closed,
        intervention: Intervention::A1Redistribution,
    },
    Cell {
        regime: Regime::Closed,
        intervention: Intervention::A2Additive,
    },
    Cell {
        regime: Regime::Closed,
        intervention: Intervention::BNeverWithdrawn,
    },
    Cell {
        regime: Regime::Closed,
        intervention: Intervention::NoIgnition,
    },
];

/// Build the cell's config from the C3R.d earned base (the trap) + the acquisition ledger, the
/// disclosed regime, and the finite intervention. Every pin is verbatim; none is searched.
fn config(cell: Cell) -> SettlementConfig {
    // The trap: the C3R.d NoMotiveReference (earned, no saving motive, no support). The ledger is
    // enabled in EVERY C3R.e cell (§1.4) — runtime-only, behaviorally inert. DH.a: the {closed}
    // regime is the durable stack MINUS the endowed surround (`frontier_closed_circulation`, which
    // already pins gatherers=48 + producer wood_provision=0 and drops the consumers/lineage).
    let mut cfg = match cell.regime {
        Regime::Closed => SettlementConfig::frontier_closed_circulation(),
        _ => SettlementConfig::frontier_mortal_producers_earned(),
    };
    cfg.chain.as_mut().expect("chain").acquisition_ledger = true;

    // The disclosed {durable} regime: retire the producer WOOD mint (producers buy warmth out of
    // bread revenue) and pin gatherers at 48 (double the base 24, a single value — classify not
    // tune). {current} keeps the landed regime; {closed} already applied both in its constructor.
    if cell.regime == Regime::Durable {
        cfg.gatherers = DURABLE_GATHERERS;
        let demo = cfg.demography.as_mut().expect("demography");
        let start = demo
            .households
            .len()
            .checked_sub(PRODUCER_HOUSEHOLDS)
            .expect("producer households appended");
        for household in &mut demo.households[start..] {
            household.wood_provision = 0;
        }
    }

    // The finite intervention.
    match cell.intervention {
        Intervention::NoIgnition => {}
        Intervention::A1Redistribution => {
            cfg.chain.as_mut().expect("chain").birth_stock_ignition_at = Some(IGNITION_TICK);
        }
        Intervention::A2Additive => {
            cfg.chain
                .as_mut()
                .expect("chain")
                .producer_house_starting_staple = A2_ENDOWMENT;
        }
        Intervention::BWithdrawn => set_support(&mut cfg, Some(SUPPORT_UNTIL)),
        // Never withdrawn within the run: support delivered through the last tick (`econ_tick <
        // RUN_TICKS` is always true), the support-on control (§4.4).
        Intervention::BNeverWithdrawn => set_support(&mut cfg, Some(RUN_TICKS)),
    }
    cfg
}

/// Restore the exact landed C3R.b viable support pair on the producer houses — `food_provision = 1`
/// AND the `producer_subsistence = 4` cushion — gated by `producer_support_until_tick`. The
/// cushion's WOOD leg is disabled for the whole run by the engine (a `Some` gate), constant across
/// eras.
fn set_support(cfg: &mut SettlementConfig, until: Option<u64>) {
    {
        let chain = cfg.chain.as_mut().expect("chain");
        chain.producer_subsistence = CUSHION;
        chain.producer_support_until_tick = until;
    }
    let demo = cfg.demography.as_mut().expect("demography");
    let start = demo
        .households
        .len()
        .checked_sub(PRODUCER_HOUSEHOLDS)
        .expect("producer households appended");
    for household in &mut demo.households[start..] {
        household.food_provision = HEARTH_FOOD;
    }
}

fn chain_goods(cfg: &SettlementConfig) -> (GoodId, GoodId) {
    let content = &cfg.chain.as_ref().expect("chain").content;
    (content.flour(), content.bread())
}

fn positive_bake_spread(s: &Settlement, bread: GoodId, flour: GoodId) -> bool {
    let (Some(bread_price), Some(flour_price)) = (s.realized_price(bread), s.realized_price(flour))
    else {
        return false;
    };
    u128::from(bread_price.0) * u128::from(BREAD_PER_BAKE)
        > u128::from(flour_price.0) * u128::from(FLOUR_PER_BAKE)
}

fn tick_bread_trades(s: &Settlement, bread: GoodId, spot_start: usize, barter_start: usize) -> u64 {
    let spot = s.society().trades[spot_start..]
        .iter()
        .filter(|trade| trade.good == bread)
        .count() as u64;
    let barter = s.society().barter_trades[barter_start..]
        .iter()
        .filter(|trade| trade.a_gives == bread || trade.b_gives == bread)
        .count() as u64;
    spot.saturating_add(barter)
}

/// One tick's structure/flow observations (the `WindowTrace.observe` inputs, per tick).
#[derive(Clone, Copy)]
struct TickSample {
    millers: usize,
    bakers: usize,
    produced: u64,
    trades: u64,
    price: Option<u64>,
    spread: bool,
}

/// One driven cell: the per-tick structure samples plus the cumulative-after-tick counter series
/// the six criteria window-diff, and the whole-run hard-guard flags.
struct CellRun {
    seed: u64,
    cell: Cell,
    samples: Vec<TickSample>,
    /// Intervention-origin held AFTER each tick (the GLOBAL exhaustion read).
    held: Vec<u64>,
    /// Cumulative producer-house births after each tick.
    births: Vec<u64>,
    /// … cumulative producer-house birth funding from non-market channels (SeededMinted + Foraged +
    /// Commons) after each tick — window-diffed for criterion iii.
    funded_nonmarket: Vec<u64>,
    /// … from intervention-origin lots.
    funded_intervention: Vec<u64>,
    /// Cumulative genuine external revenue (all-goods leg = genuine + non-bread external).
    genuine_rev: Vec<u64>,
    nonbread_earned: Vec<u64>,
    genuine_trades: Vec<u64>,
    from_gatherers: Vec<u64>,
    conserved: bool,
    money_ok: bool,
    registry_ok: bool,
    immortal_max: usize,
    /// Cumulative producer member starvations — a clean trap baseline is fed (`== 0`); a base that
    /// starves is unviable as an experiment (`BaseUnviable`).
    member_starvations: u64,
    ignition_injected_qty: u64,
    ignition_gates: [u64; 6],
    final_births: u64,
    funded_market_total: u64,
    funded_nonmarket_total: u64,
    /// DH.a: the closure ledger's per-tick aggregates (one per tick), captured for Closed cells so
    /// the preamble can window them; empty for the landed Durable/Current cells.
    closure_ticks: Vec<ClosureTickAgg>,
}

fn run_cell_seed(seed: u64, cell: Cell) -> CellRun {
    let cfg = config(cell);
    let (flour, bread) = chain_goods(&cfg);
    let mut s = Settlement::generate(seed, &cfg);

    let len = RUN_TICKS as usize;
    let mut samples = Vec::with_capacity(len);
    let mut held = Vec::with_capacity(len);
    let mut births = Vec::with_capacity(len);
    let mut funded_market = Vec::with_capacity(len);
    let mut funded_nonmarket = Vec::with_capacity(len);
    let mut funded_intervention = Vec::with_capacity(len);
    let mut genuine_rev = Vec::with_capacity(len);
    let mut nonbread_earned = Vec::with_capacity(len);
    let mut genuine_trades = Vec::with_capacity(len);
    let mut from_gatherers = Vec::with_capacity(len);

    let mut conserved = true;
    let mut money_ok = true;
    let mut immortal_max = 0usize;

    for _tick in 0..RUN_TICKS {
        let spot_start = s.society().trades.len();
        let barter_start = s.society().barter_trades.len();
        let report = s.econ_tick();
        conserved &= report.conserves();
        money_ok &= report.money_conserves();
        immortal_max = immortal_max.max(s.immortal_producer_count());

        let millers = s.living_count(Vocation::Miller);
        let bakers = s.living_count(Vocation::Baker);
        let produced = report.produced_of(bread);
        let trades = tick_bread_trades(&s, bread, spot_start, barter_start);
        // The realized price is recorded only on a trade tick (mirrors `WindowTrace::observe`).
        let price = if trades > 0 {
            s.realized_price(bread).map(|price| price.0)
        } else {
            None
        };
        let spread = positive_bake_spread(&s, bread, flour);
        samples.push(TickSample {
            millers,
            bakers,
            produced,
            trades,
            price,
            spread,
        });

        held.push(s.acquisition_intervention_held());
        births.push(s.producer_house_births());
        let funded = s.producer_birth_funded_by_channel();
        funded_market.push(funded.bought.saturating_add(funded.self_produced));
        funded_nonmarket.push(
            funded
                .seeded_minted
                .saturating_add(funded.foraged)
                .saturating_add(funded.commons),
        );
        funded_intervention.push(s.producer_birth_funded_intervention());
        let stats = s.earned_provisioning_stats();
        genuine_rev.push(stats.genuine_external_revenue.0);
        nonbread_earned.push(stats.non_bread_external_earned.0);
        genuine_trades.push(stats.genuine_external_bread_trades);
        from_gatherers.push(stats.from_gatherers.0);
    }

    let funded_market_total = *funded_market.last().unwrap_or(&0);
    let funded_nonmarket_total = *funded_nonmarket.last().unwrap_or(&0);
    // DH.a: capture the closure ledger's per-tick aggregates for Closed cells only.
    let closure_ticks = if cell.regime == Regime::Closed {
        s.closure_tick_aggregates().to_vec()
    } else {
        Vec::new()
    };
    CellRun {
        seed,
        cell,
        samples,
        held,
        births,
        funded_nonmarket,
        funded_intervention,
        genuine_rev,
        nonbread_earned,
        genuine_trades,
        from_gatherers,
        conserved,
        money_ok,
        registry_ok: s.private_land_registry_invariant_holds(),
        immortal_max,
        member_starvations: s.earned_provisioning_stats().member_starvations,
        ignition_injected_qty: s.ignition_injected_qty(),
        ignition_gates: s.ignition_gate_decomposition(),
        final_births: s.producer_house_births(),
        funded_market_total,
        funded_nonmarket_total,
        closure_ticks,
    }
}

/// A derived view of one absolute grid window `[start, start+160)` over a `CellRun`.
#[derive(Clone, Copy)]
struct WindowView {
    ticks: u64,
    min_millers: usize,
    min_bakers: usize,
    staffed: u64,
    output: u64,
    price: Option<u64>,
    spread: bool,
    held_entering: u64,
    d_births: u64,
    d_nonmarket: u64,
    d_intervention: u64,
    d_genuine_rev: u64,
    d_nonbread: u64,
    d_genuine_trades: u64,
    d_from_gatherers: u64,
}

impl WindowView {
    fn structure_runs(self) -> bool {
        self.ticks > 0 && self.min_millers > 0 && self.min_bakers > 0
    }

    fn flow_runs(self) -> bool {
        self.structure_runs()
            && self.price.is_some_and(|price| price != 1)
            && self.spread
            && self.output >= FLOW_RUNS_MIN_BREAD
            && self.output.saturating_mul(10) >= self.staffed
    }

    /// iii — MarketFundedBirths: ≥1 producer birth, none funded by an intervention-origin lot, and
    /// every drawn lot ∈ {Bought, SelfProduced} (no SeededMinted/Foraged/Commons funding).
    fn crit_iii(self) -> bool {
        self.d_births >= 1 && self.d_intervention == 0 && self.d_nonmarket == 0
    }

    /// iv — StructureAndFlow: the landed structure + flow bars AND the all-goods revenue delta > 0.
    fn crit_iv(self) -> bool {
        self.structure_runs()
            && self.flow_runs()
            && self.d_genuine_rev.saturating_add(self.d_nonbread) > 0
    }

    /// v — ActiveExternalDemand: Δ genuine bread trades > 0 AND Δ from-gatherers revenue > 0.
    fn crit_v(self) -> bool {
        self.d_genuine_trades > 0 && self.d_from_gatherers > 0
    }
}

fn delta(series: &[u64], start: u64, end: u64) -> u64 {
    let hi = series[(end - 1) as usize];
    let lo = if start == 0 {
        0
    } else {
        series[(start - 1) as usize]
    };
    hi.saturating_sub(lo)
}

impl CellRun {
    /// Derive the window view over `[start, start+160)`. Requires the window to fit within the run.
    fn window(&self, start: u64) -> WindowView {
        let end = start + N;
        debug_assert!(end <= RUN_TICKS);
        let mut min_millers = usize::MAX;
        let mut min_bakers = usize::MAX;
        let mut staffed = 0u64;
        let mut output = 0u64;
        let mut price = None;
        let mut spread = false;
        let mut ticks = 0u64;
        for t in start..end {
            let sample = self.samples[t as usize];
            ticks += 1;
            min_millers = min_millers.min(sample.millers);
            min_bakers = min_bakers.min(sample.bakers);
            if sample.millers > 0 && sample.bakers > 0 {
                staffed += 1;
            }
            output = output.saturating_add(sample.produced);
            if sample.trades > 0 {
                price = sample.price;
            }
            spread |= sample.spread;
        }
        // The held ENTERING the window (its state at the boundary). For `start == 0` the
        // generation-time endowment is already live, so we read after the tick-0 sweep.
        let held_entering = self.held[(start.max(1) - 1) as usize];
        WindowView {
            ticks,
            min_millers,
            min_bakers,
            staffed,
            output,
            price,
            spread,
            held_entering,
            d_births: delta(&self.births, start, end),
            d_nonmarket: delta(&self.funded_nonmarket, start, end),
            d_intervention: delta(&self.funded_intervention, start, end),
            d_genuine_rev: delta(&self.genuine_rev, start, end),
            d_nonbread: delta(&self.nonbread_earned, start, end),
            d_genuine_trades: delta(&self.genuine_trades, start, end),
            d_from_gatherers: delta(&self.from_gatherers, start, end),
        }
    }

    /// The last tick with any intervention-origin held (the exhaustion frontier). Held is monotone
    /// to zero (interventions are finite one-shots), so exhaustion = 1 + this. `0` if none ever.
    fn exhaustion_tick(&self) -> u64 {
        self.held
            .iter()
            .rposition(|&h| h > 0)
            .map_or(0, |idx| idx as u64 + 1)
    }
}

/// The measurement grid window START ticks for an intervention (absolute, tail-dropped).
fn measurement_starts(intervention: Intervention) -> Vec<u64> {
    let base = match intervention {
        Intervention::A1Redistribution => IGNITION_TICK, // [50 + k·160)
        Intervention::A2Additive | Intervention::NoIgnition => 0, // [k·160)
        Intervention::BWithdrawn | Intervention::BNeverWithdrawn => SUPPORT_UNTIL, // [400 + k·160)
    };
    let mut starts = Vec::new();
    let mut k = 0u64;
    loop {
        let start = base + k * N;
        if start + N > RUN_TICKS {
            break;
        }
        starts.push(start);
        k += 1;
    }
    starts
}

/// The strict lower bound a window start must EXCEED to be eligible (the vacuous-exhaustion hole):
/// A1 → `> 50` (its global origin-held is 0 BEFORE the shot fires); A2 and B → NO floor (the spec's
/// `start ≥ 0` / grid-start semantics — the debt-repair fix of the result-neutral `start > 0` mismatch).
fn eligible_start_exclusive(intervention: Intervention) -> Option<u64> {
    match intervention {
        Intervention::A1Redistribution => Some(IGNITION_TICK),
        _ => None,
    }
}

/// The OBSERVATION-grid window starts (for the non-vacuous `IgnitionNeverIgnites`).
fn observation_starts(intervention: Intervention, exhaustion: u64) -> Vec<u64> {
    match intervention {
        // B support-era observation grid (round-3): the measurement grid begins at 400, so the
        // support era needs its own grid. [0,160), [160,320); [320,400) dropped.
        Intervention::BWithdrawn | Intervention::BNeverWithdrawn => vec![0, N],
        // A1/A2 observation = the measurement-grid windows fully BEFORE exhaustion.
        _ => measurement_starts(intervention)
            .into_iter()
            .filter(|&start| start + N <= exhaustion)
            .collect(),
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
enum Verdict {
    // Preconditions (the verbatim quartet + the two executable additions).
    BaseUnviable,
    ReservoirOpen,
    ConservationBroken,
    RegistryBroken,
    IgnitionShortfall,
    SupportOnControlFails,
    // The ordered outcomes.
    HysteresisHolds {
        first_window: u64,
        windows_survived: usize,
    },
    IgnitionNeverIgnites,
    ResidualNeverExhausted,
    RegimeUntrapsWithoutIgnition {
        window: u64,
    },
    ExternalDemandDiedFirst {
        window: u64,
    },
    IgnitesThenRelapses {
        relapse_window: u64,
    },
    UnclassifiedMixed {
        all_pass_but_short: Option<usize>,
    },
    // Control reference outcomes.
    SupportOnControlViable,
    MatchedReferenceTrapped,
    MatchedReferenceUntrapped,
}

/// Does the reference cell pass structure in ANY window of the intervention cell's measurement grid
/// (the same absolute intervals)? Feeds criterion vi and the RegimeUntraps rung.
fn reference_passes_structure_at(reference: &CellRun, start: u64) -> bool {
    reference.window(start).structure_runs()
}

/// Classify one intervention cell against its same-seed matched no-ignition reference and the
/// same-regime support-on control (durable only). Returns the verdict and the per-window criteria
/// for printing.
fn classify_intervention(
    run: &CellRun,
    reference: &CellRun,
    support_on: Option<&CellRun>,
) -> (Verdict, Vec<WindowReport>) {
    let intervention = run.cell.intervention;

    // ---- Preconditions (the verbatim quartet, then the two executable additions).
    // BaseUnviable: the matched trap baseline (same-regime no-ignition) must be a FED trap — if it
    // starves, there is no clean trap to escape and no withdrawal verdict is meaningful.
    if reference.member_starvations > 0 {
        return (Verdict::BaseUnviable, Vec::new());
    }
    if run.immortal_max > 0 {
        return (Verdict::ReservoirOpen, Vec::new());
    }
    if !run.conserved || !run.money_ok {
        return (Verdict::ConservationBroken, Vec::new());
    }
    if !run.registry_ok {
        return (Verdict::RegistryBroken, Vec::new());
    }
    // A1: the DOSE is the detector — an under-dose disqualifies rather than silently under-igniting.
    if intervention == Intervention::A1Redistribution && run.ignition_injected_qty < IGNITION_DOSE {
        return (Verdict::IgnitionShortfall, Vec::new());
    }
    // The executable support-on era guard (durable intervention cells): if even PERMANENT support
    // fails structure in EVERY [400,1600) grid window on this regime, the era is unviable and every
    // same-regime withdrawal verdict is void.
    if let Some(support) = support_on {
        let support_viable = measurement_starts(Intervention::BNeverWithdrawn)
            .into_iter()
            .any(|start| support.window(start).structure_runs());
        if !support_viable {
            return (Verdict::SupportOnControlFails, Vec::new());
        }
    }

    // ---- The window grid, eligibility, and per-window criteria.
    let exhaustion = run.exhaustion_tick();
    let start_floor = eligible_start_exclusive(intervention);
    let mut reports = Vec::new();
    let mut eligible: Vec<WindowView> = Vec::new();
    for start in measurement_starts(intervention) {
        let w = run.window(start);
        let is_eligible = start_floor.is_none_or(|f| start > f) && w.held_entering == 0;
        let vi = !reference_passes_structure_at(reference, start); // matched cell fails structure
        reports.push(WindowReport {
            start,
            eligible: is_eligible,
            structure: w.structure_runs(),
            flow: w.flow_runs(),
            crit_iii: w.crit_iii(),
            crit_iv: w.crit_iv(),
            crit_v: w.crit_v(),
            crit_vi: vi,
            reference_structure: !vi,
            held_entering: w.held_entering,
            d_births: w.d_births,
        });
        if is_eligible {
            eligible.push(w);
        }
    }

    // A window passes iff iii ∧ iv ∧ v ∧ vi (i–ii hold by eligibility). vi indexes align with
    // `eligible` because both walk the measurement grid in order.
    let passes: Vec<bool> = reports
        .iter()
        .filter(|r| r.eligible)
        .map(|r| r.crit_iii && r.crit_iv && r.crit_v && r.crit_vi)
        .collect();
    let eligible_reports: Vec<&WindowReport> = reports.iter().filter(|r| r.eligible).collect();

    // ---- The ordered ladder (HysteresisHolds checked FIRST).
    if let Some(k) = first_streak(&passes, M) {
        let survived = passes[k..].iter().take_while(|&&p| p).count();
        return (
            Verdict::HysteresisHolds {
                first_window: eligible_reports[k].start,
                windows_survived: survived,
            },
            reports,
        );
    }

    // IgnitionNeverIgnites (non-vacuous): ≥1 eligible window AND ≥1 full observation-grid window,
    // all structure-dead. Requiring an eligible window keeps it from pre-empting ResidualNever…
    let observation = observation_starts(intervention, exhaustion);
    let observation_full: Vec<u64> = observation
        .into_iter()
        .filter(|&start| start + N <= RUN_TICKS)
        .collect();
    let obs_all_dead = !observation_full.is_empty()
        && observation_full
            .iter()
            .all(|&start| !run.window(start).structure_runs());
    if !eligible.is_empty() && obs_all_dead {
        return (Verdict::IgnitionNeverIgnites, reports);
    }

    // ResidualNeverExhausted: no eligible window at all (withdrawal UNDETERMINED, not relapse).
    if eligible.is_empty() {
        return (Verdict::ResidualNeverExhausted, reports);
    }

    // RegimeUntrapsWithoutIgnition: ¬vi(w) for some eligible w (the matched cell passed structure
    // in the same absolute interval) — persistence is not attributable to ignition history.
    if let Some(r) = eligible_reports.iter().find(|r| !r.crit_vi) {
        return (
            Verdict::RegimeUntrapsWithoutIgnition { window: r.start },
            reports,
        );
    }
    // From here vi holds for every eligible window.

    // ExternalDemandDiedFirst: the earliest ¬v window is at or before the earliest ¬(iii∧iv) window
    // (or no v-alive eligible window exists at all) — UNDETERMINED, precedes relapse.
    let first_fail = eligible_reports
        .iter()
        .position(|r| !(r.crit_iii && r.crit_iv));
    let first_v_dead = eligible_reports.iter().position(|r| !r.crit_v);
    let any_v_alive = eligible_reports.iter().any(|r| r.crit_v);
    if let Some(vdead) = first_v_dead {
        let edd = !any_v_alive || first_fail.is_some_and(|ff| vdead <= ff);
        if edd {
            return (
                Verdict::ExternalDemandDiedFirst {
                    window: eligible_reports[vdead].start,
                },
                reports,
            );
        }
    }

    // IgnitesThenRelapses (the honest default): the earliest eligible ¬(iii∧iv) window where v ∧ vi
    // held for EVERY eligible u ≤ w.
    if let Some(ff) = first_fail {
        let held_through = eligible_reports[..=ff]
            .iter()
            .all(|r| r.crit_v && r.crit_vi);
        if held_through {
            return (
                Verdict::IgnitesThenRelapses {
                    relapse_window: eligible_reports[ff].start,
                },
                reports,
            );
        }
    }

    // UnclassifiedMixed — the exact terminal catch-all. Name the all-passing-but-<5 case.
    let all_pass_but_short = if !passes.is_empty() && passes.iter().all(|&p| p) {
        Some(passes.len())
    } else {
        None
    };
    (Verdict::UnclassifiedMixed { all_pass_but_short }, reports)
}

/// The earliest index `k` at which `flags[k..k+len]` are all `true`, else `None`.
fn first_streak(flags: &[bool], len: usize) -> Option<usize> {
    if len == 0 || flags.len() < len {
        return None;
    }
    (0..=flags.len() - len).find(|&k| flags[k..k + len].iter().all(|&p| p))
}

/// One eligible-or-observation window's printed criteria.
#[derive(Clone, Copy)]
struct WindowReport {
    start: u64,
    eligible: bool,
    structure: bool,
    flow: bool,
    crit_iii: bool,
    crit_iv: bool,
    crit_v: bool,
    crit_vi: bool,
    reference_structure: bool,
    held_entering: u64,
    d_births: u64,
}

/// A control cell's reference outcome (§6).
fn classify_control(run: &CellRun) -> Verdict {
    match run.cell.intervention {
        Intervention::BNeverWithdrawn => {
            let viable = measurement_starts(Intervention::BNeverWithdrawn)
                .into_iter()
                .any(|start| run.window(start).structure_runs());
            if viable {
                Verdict::SupportOnControlViable
            } else {
                Verdict::SupportOnControlFails
            }
        }
        // NoIgnition: trapped iff it never passes structure over the whole-run [k·160) grid.
        _ => {
            let untrapped = measurement_starts(Intervention::NoIgnition)
                .into_iter()
                .any(|start| run.window(start).structure_runs());
            if untrapped {
                Verdict::MatchedReferenceUntrapped
            } else {
                Verdict::MatchedReferenceTrapped
            }
        }
    }
}

/// The matched no-ignition reference cell for an intervention cell (same regime, same seed).
fn matched_reference(cell: Cell) -> Cell {
    Cell {
        regime: cell.regime,
        intervention: Intervention::NoIgnition,
    }
}

/// The same-regime support-on control cell (durable and closed regimes — §3.4 extends it to Closed
/// identically). `Current` keeps the landed regime, which has no support-on control.
fn support_on_control(cell: Cell) -> Option<Cell> {
    matches!(cell.regime, Regime::Durable | Regime::Closed).then_some(Cell {
        regime: cell.regime,
        intervention: Intervention::BNeverWithdrawn,
    })
}

fn assert_hard_guards(run: &CellRun) {
    assert!(
        run.conserved,
        "conservation broke: {} seed {}",
        run.cell.label(),
        run.seed
    );
    assert!(
        run.money_ok,
        "money conservation broke: {} seed {}",
        run.cell.label(),
        run.seed
    );
    assert!(
        run.registry_ok,
        "private-land registry invariant broke: {} seed {}",
        run.cell.label(),
        run.seed
    );
    assert_eq!(
        run.immortal_max,
        0,
        "reservoir opened (an immortal producer): {} seed {}",
        run.cell.label(),
        run.seed
    );
    // The drawn-lot funding sum invariant (§6 / Slice B DoD): every producer-house birth draws
    // exactly `child_food_endowment` funding units, so the per-channel tally sums to
    // `4 × producer_births`. Holds whenever conservation holds (the ledger mirrors stock).
    assert_eq!(
        run.funded_market_total + run.funded_nonmarket_total,
        CHILD_FOOD_ENDOWMENT * run.final_births,
        "drawn-lot funding sum must equal 4 × producer births: {} seed {}",
        run.cell.label(),
        run.seed
    );
}

/// Render the per-window criteria block as a `String` (the canonical renderer, §3.4). The bytes
/// are byte-identical to the pre-DH.a `print_windows` `println!` text — validated against the
/// committed golden captured from the unmodified base, never against the renderer itself.
fn render_windows(reports: &[WindowReport]) -> String {
    use std::fmt::Write as _;
    let mut out = String::new();
    for r in reports {
        let tag = if r.eligible { "ELIGIBLE" } else { "obs/skip" };
        let _ = writeln!(
            out,
            "      w@{:>4} {tag} held_in={:>3} births={} | iii={} iv={} v={} vi={} \
             | structure={} flow={} ref_structure={}",
            r.start,
            r.held_entering,
            r.d_births,
            r.crit_iii as u8,
            r.crit_iv as u8,
            r.crit_v as u8,
            r.crit_vi as u8,
            r.structure as u8,
            r.flow as u8,
            r.reference_structure as u8,
        );
    }
    out
}

/// Render one INTERVENTION cell's report block (verdict line, the A1 gate decomposition, the
/// HysteresisHolds banner, then the per-window criteria) as a `String`, byte-identical to the
/// pre-DH.a `println!` sequence.
fn render_intervention_cell(
    seed: u64,
    cell: Cell,
    run: &CellRun,
    verdict: &Verdict,
    reports: &[WindowReport],
) -> String {
    use std::fmt::Write as _;
    let survivors: usize = reports.iter().filter(|r| r.eligible).count();
    let mut out = String::new();
    let _ = writeln!(
        out,
        "  {} seed {seed}: verdict={:?}  (ignition_dose={}, eligible_windows={})",
        cell.label(),
        verdict,
        run.ignition_injected_qty,
        survivors,
    );
    if matches!(cell.intervention, Intervention::A1Redistribution) {
        let [interval, extinct, cap, hunger, at_target, donor] = run.ignition_gates;
        let _ = writeln!(
            out,
            "      ignition gates: interval={interval} extinct={extinct} cap={cap} \
             hunger={hunger} at_target={at_target} donor_shortfall={donor}"
        );
    }
    if let Verdict::HysteresisHolds {
        first_window,
        windows_survived,
    } = verdict
    {
        // The ONLY rung permitted multiple-equilibria / big-push language.
        let _ = writeln!(
            out,
            "    HYSTERESIS HOLDS — a big push separated two equilibria of the same \
             economy: {windows_survived} consecutive post-withdrawal windows survive \
             from w@{first_window} (history, not parameters)."
        );
    }
    out.push_str(&render_windows(reports));
    out
}

/// Render one CONTROL cell's reference-outcome line as a `String`, byte-identical to the pre-DH.a
/// `println!`.
fn render_control_cell(seed: u64, cell: Cell, verdict: &Verdict) -> String {
    format!("  {} seed {seed}: control={verdict:?}\n", cell.label())
}

/// DH.a (§3.3): window the closure ledger's per-tick aggregates into the oracle's absolute 160-tick
/// grid ([0,160), [160,320), … to the horizon). `start == 0` is the BOOTSTRAP window (printed but
/// excluded from `classify_closure`; passing it is impossible by construction — all gold begins
/// endowed). Per-class fields are indexed by [`ClosureClass::index`] (`[Gatherer, Miller, Baker]`).
fn closure_all_windows(ticks: &[ClosureTickAgg]) -> Vec<ClosureWindow> {
    let mut windows = Vec::new();
    let mut start = 0u64;
    while start + N <= RUN_TICKS {
        let mut w = ClosureWindow {
            start,
            present: [true; 3],
            own_sale_consideration: [0; 3],
            purchase_consideration: [0; 3],
            endowed_purchase_debits: [0; 3],
            endowed_physical_debits: [0; 3],
            commons_drain: 0,
            commons_goods_drain: 0,
            wage_escrow_gold: 0,
            land_fee_pool_salt: 0,
        };
        for t in start..start + N {
            let Some(agg) = ticks.get(t as usize) else {
                w.present = [false; 3];
                continue;
            };
            for c in 0..3 {
                if !agg.living[c] {
                    w.present[c] = false;
                }
                w.own_sale_consideration[c] =
                    w.own_sale_consideration[c].saturating_add(agg.own_sale_consideration[c]);
                w.purchase_consideration[c] =
                    w.purchase_consideration[c].saturating_add(agg.purchase_consideration[c]);
                w.endowed_purchase_debits[c] =
                    w.endowed_purchase_debits[c].saturating_add(agg.endowed_purchase_debits[c]);
                w.endowed_physical_debits[c] =
                    w.endowed_physical_debits[c].saturating_add(agg.endowed_physical_debits[c]);
            }
            w.commons_drain = w.commons_drain.saturating_add(agg.commons_drain);
            w.commons_goods_drain = w
                .commons_goods_drain
                .saturating_add(agg.commons_goods_drain);
            w.wage_escrow_gold = w.wage_escrow_gold.max(agg.wage_escrow_gold);
            w.land_fee_pool_salt = w.land_fee_pool_salt.max(agg.land_fee_pool_salt);
        }
        windows.push(w);
        start += N;
    }
    windows
}

/// The classified (post-bootstrap) subset of the absolute grid — the input to `classify_closure`.
fn closure_classified(windows: &[ClosureWindow]) -> Vec<ClosureWindow> {
    windows.iter().filter(|w| w.start >= N).cloned().collect()
}

/// Render one closure window's CC0–CC3 diagnostics (§3.3).
fn render_closure_window(w: &ClosureWindow) -> String {
    let tag = if w.start >= N {
        "classified"
    } else {
        "BOOTSTRAP "
    };
    format!(
        "      closure w@{:>4} [{tag}] CC0 G={} M={} B={} | CC1 own[{} {} {}] buy[{} {} {}] \
         | CC2 epd[{} {} {}] eph[{} {} {}] | CC3 drain={} goods={} esc={} fee={}\n",
        w.start,
        w.present[0] as u8,
        w.present[1] as u8,
        w.present[2] as u8,
        w.own_sale_consideration[0],
        w.own_sale_consideration[1],
        w.own_sale_consideration[2],
        w.purchase_consideration[0],
        w.purchase_consideration[1],
        w.purchase_consideration[2],
        w.endowed_purchase_debits[0],
        w.endowed_purchase_debits[1],
        w.endowed_purchase_debits[2],
        w.endowed_physical_debits[0],
        w.endowed_physical_debits[1],
        w.endowed_physical_debits[2],
        w.commons_drain,
        w.commons_goods_drain,
        w.wage_escrow_gold,
        w.land_fee_pool_salt,
    )
}

/// Render the closure preamble for a Closed cell (§3.4): this cell's own per-window CC0–CC3
/// diagnostics (bootstrap included, marked) then the base-trace (Closed NoIgnition) closure verdict.
/// Printed BEFORE the unchanged ladder verdict.
fn render_closure_preamble(
    cell_windows: &[ClosureWindow],
    base_verdict: &ClosureVerdict,
) -> String {
    use std::fmt::Write as _;
    let mut out = String::new();
    for w in cell_windows {
        out.push_str(&render_closure_window(w));
    }
    let _ = writeln!(
        out,
        "      closure verdict (base NoIgnition): {base_verdict:?}"
    );
    out
}

#[test]
fn ignition_and_withdrawal_diagnosis() {
    println!(
        "C3R.e ignition-and-withdrawal — seeds={SEEDS:?}, ticks={RUN_TICKS}, window N={N}, streak M={M}"
    );
    println!(
        "  the honest default is relapse; verdicts + all six criteria are PRINTED, never asserted."
    );

    let mut total_runs = 0usize;
    // The canonical renderer's existing-cell projection (§3.4): each existing cell's report
    // block, concatenated in the seeds-outermost × landed-`CELLS`-order loop, asserted
    // byte-identical against the pre-DH.a golden captured from the unmodified base.
    let mut existing_projection = String::new();
    for &seed in &SEEDS {
        // Drive the landed seven cells and the five DH.a Closed cells for this seed (references are
        // DERIVED views — no extra runs). Seeds-outermost, landed order then Closed order.
        let runs: Vec<CellRun> = CELLS
            .iter()
            .chain(CLOSED_CELLS.iter())
            .map(|&cell| run_cell_seed(seed, cell))
            .collect();
        // Hard guards (invariants only) — asserted on every run.
        for run in &runs {
            assert_hard_guards(run);
            total_runs += 1;
        }

        // The landed C3R.d anchor: the {current} no-ignition cell reproduces the earned
        // NoMotiveReference births (the ledger is behaviorally inert).
        let current_noignition = runs
            .iter()
            .find(|r| {
                r.cell.regime == Regime::Current && r.cell.intervention == Intervention::NoIgnition
            })
            .expect("the current no-ignition cell exists");
        let seed_index = SEEDS.iter().position(|&s| s == seed).expect("seed listed");
        assert_eq!(
            current_noignition.final_births, EXPECTED_NO_MOTIVE_BIRTHS[seed_index],
            "the {{current, NoIgnition}} anchor must reproduce the landed earned NoMotiveReference \
             births at seed {seed}",
        );

        let lookup = |cell: Cell| runs.iter().find(|r| r.cell == cell).expect("cell driven");

        println!("--- seed {seed} ---");
        for &cell in &CELLS {
            let run = lookup(cell);
            let block = if cell.is_intervention() {
                let reference = lookup(matched_reference(cell));
                let support = support_on_control(cell).map(lookup);
                let (verdict, reports) = classify_intervention(run, reference, support);
                render_intervention_cell(seed, cell, run, &verdict, &reports)
            } else {
                let verdict = classify_control(run);
                render_control_cell(seed, cell, &verdict)
            };
            print!("{block}");
            existing_projection.push_str(&block);
        }

        // DH.a: the Closed cells. Closure is evaluated on the Closed NoIgnition trace (§3.3); every
        // Closed cell prints its own per-window CC0–CC3 diagnostics + that base verdict, THEN the
        // unchanged ladder verdict. The Closed cells are NOT part of the pre-DH.a golden projection.
        let closed_base = lookup(Cell {
            regime: Regime::Closed,
            intervention: Intervention::NoIgnition,
        });
        let base_windows = closure_all_windows(&closed_base.closure_ticks);
        let base_verdict = classify_closure(&closure_classified(&base_windows));
        for &cell in &CLOSED_CELLS {
            let run = lookup(cell);
            let cell_windows = closure_all_windows(&run.closure_ticks);
            let mut block = render_closure_preamble(&cell_windows, &base_verdict);
            block.push_str(&if cell.is_intervention() {
                let reference = lookup(matched_reference(cell));
                let support = support_on_control(cell).map(lookup);
                let (verdict, reports) = classify_intervention(run, reference, support);
                render_intervention_cell(seed, cell, run, &verdict, &reports)
            } else {
                let verdict = classify_control(run);
                render_control_cell(seed, cell, &verdict)
            });
            print!("{block}");
        }
    }
    // §3.4 step 2: the renderer's existing-cell projection equals the golden captured from the
    // unmodified base, byte-for-byte. The renderer is validated against master's output here,
    // never against itself.
    assert_eq!(
        existing_projection,
        include_str!("goldens/ignition_withdrawal_pre_dh_a.txt"),
        "the existing seven-cell projection must reproduce the pre-DH.a golden byte-for-byte"
    );
    let all_cells = CELLS.len() + CLOSED_CELLS.len();
    println!(
        "C3R.e / DH.a complete: {total_runs} runs ({all_cells} cells × {} seeds).",
        SEEDS.len()
    );
    assert_eq!(total_runs, all_cells * SEEDS.len());
}
