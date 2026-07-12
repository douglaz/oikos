//! DH.a (impl-68) — the closed circulation: constructor identity, base assertions, and the ON-only
//! digest tag 34. The ledger/reducer/`classify_closure` verification lives alongside (added below as
//! the ledger lands); the ignition/withdrawal oracle re-run lives in `ignition_withdrawal.rs`.
//!
//! See `docs/impl-closed-circulation.md`.

use sim::{Settlement, SettlementConfig, GOLD};

const PRODUCER_HOUSEHOLDS: usize = 6;

/// The `{durable}` regime exactly as the `ignition_withdrawal` oracle builds it, then the §3.7 DH.a
/// edit list — built INDEPENDENTLY of `frontier_closed_circulation()` so the identity test pins the
/// constructor to EXACTLY these edits (any other differing field fails).
fn durable_stack_with_dh_a_edits() -> SettlementConfig {
    let mut cfg = SettlementConfig::frontier_mortal_producers_earned();
    cfg.gatherers = 48;
    {
        let demo = cfg.demography.as_mut().expect("demography");
        let start = demo.households.len() - PRODUCER_HOUSEHOLDS;
        for household in &mut demo.households[start..] {
            household.wood_provision = 0;
        }
    }
    cfg.consumers = 0;
    cfg.starting_gold_consumer = 0;
    cfg.consumer_wood_endowment = 0;
    {
        let demo = cfg.demography.as_mut().expect("demography");
        let start = demo.households.len() - PRODUCER_HOUSEHOLDS;
        demo.households.drain(..start);
    }
    cfg.closed_circulation = true;
    cfg
}

/// §3.7 — the config-identity test.
#[test]
fn frontier_closed_circulation_is_the_durable_stack_plus_exactly_the_edit_list() {
    assert_eq!(
        SettlementConfig::frontier_closed_circulation(),
        durable_stack_with_dh_a_edits(),
        "frontier_closed_circulation() must equal the durable stack with EXACTLY the §3.7 edits"
    );
}

/// §3.6 — the base assertions on the constructor + generation + a stepped run.
#[test]
fn base_assertions() {
    let cfg = SettlementConfig::frontier_closed_circulation();

    // The emergent-money machinery is off; GOLD is designated from generation.
    assert!(
        cfg.barter.is_none(),
        "barter must be None (no money-emergence path)"
    );
    assert!(!cfg.m3, "m3 ledger money must be off");
    assert!(
        cfg.bank.is_none()
            && cfg.cycle.is_none()
            && cfg.tender_bench.is_none()
            && cfg.tax.is_none(),
        "bank/cycle/tender/tax finance surfaces must all be absent"
    );

    let chain = cfg.chain.as_ref().expect("chain");
    assert!(!chain.wage_labor, "wage-labor mode inactive");
    assert!(!chain.land_market, "land market inactive");
    assert!(!chain.capital_advance, "capital_advance off");
    assert!(!chain.input_advance, "input_advance off");
    assert!(!chain.subsistence_advance, "subsistence_advance off");

    assert_eq!(cfg.consumers, 0, "consumers == 0");
    assert_eq!(cfg.starting_gold_consumer, 0, "starting_gold_consumer == 0");
    assert_eq!(
        cfg.consumer_wood_endowment, 0,
        "consumer_wood_endowment == 0"
    );
    assert_eq!(cfg.gatherers, 48, "gatherers == 48");
    assert!(cfg.closed_circulation, "closed_circulation == true");

    let demo = cfg.demography.as_ref().expect("demography");
    assert_eq!(
        demo.households.len(),
        PRODUCER_HOUSEHOLDS,
        "household list length == 6 (the 6 producer specs; the 2 lineage households removed)"
    );
    for household in &demo.households {
        assert_eq!(
            household.food_provision, 0,
            "every retained spec food_provision == 0"
        );
        assert_eq!(
            household.wood_provision, 0,
            "every retained spec wood_provision == 0"
        );
    }

    // Generation succeeds on the subtracted population (consumers=0, lineage removed): the money
    // good is GOLD, and the CC3 runtime sinks are 0 at every window boundary over a full run.
    let mut s = Settlement::generate(3, &cfg);
    assert_eq!(
        s.current_money_good(),
        Some(GOLD),
        "designated money good is GOLD from generation"
    );
    for tick in 0..1_600u64 {
        assert_eq!(
            s.wage_escrow_gold(),
            0,
            "wage_escrow_gold stays 0 (tick {tick})"
        );
        assert_eq!(
            s.land_market_fee_pool_salt(),
            0,
            "land_fee_pool_salt stays 0 (tick {tick})"
        );
        s.econ_tick();
    }
    assert_eq!(s.wage_escrow_gold(), 0);
    assert_eq!(s.land_market_fee_pool_salt(), 0);
}

/// §3.5 — the ON-only, injective digest tag 34: its ENTIRE digest footprint is the two-byte
/// `[34, 1]` emission. Flipping only the marker on the closed topology adds exactly those two bytes
/// (and the observation-only ledger it enables shifts nothing else), both at generation and after
/// stepping.
#[test]
fn digest_tag34_is_off_plus_the_single_marker_emission() {
    let seed = 3;
    let on_cfg = SettlementConfig::frontier_closed_circulation();
    let mut off_cfg = on_cfg.clone();
    off_cfg.closed_circulation = false;

    for ticks in [0u64, 200] {
        let mut on = Settlement::generate(seed, &on_cfg);
        let mut off = Settlement::generate(seed, &off_cfg);
        for _ in 0..ticks {
            on.econ_tick();
            off.econ_tick();
        }
        let on_bytes = on.canonical_bytes();
        let off_bytes = off.canonical_bytes();
        assert_eq!(
            on_bytes.len(),
            off_bytes.len() + 2,
            "ON canonical bytes must be exactly two longer than OFF ({ticks} ticks)"
        );
        let split = on_bytes
            .iter()
            .zip(&off_bytes)
            .position(|(a, b)| a != b)
            .expect("ON and OFF must differ at the tag-34 emission");
        assert_eq!(
            &on_bytes[split..split + 2],
            &[34u8, 1u8],
            "the sole digest delta must be the [34, 1] tag-34 emission ({ticks} ticks)"
        );
        assert_eq!(
            &on_bytes[split + 2..],
            &off_bytes[split..],
            "removing the [34, 1] emission must yield the OFF bytes byte-for-byte ({ticks} ticks)"
        );
    }
}

// ===========================================================================================
// DH.a verification battery (§6 criterion 4): the ledger/preamble are observation-only, the
// raw-tape recount, the bootstrap exclusion, the disjointness split, and the total mapping.
// ===========================================================================================

use sim::{
    classify_closure, closure_classified_windows, closure_windows, AgentId, ClosureClass,
    ClosureCriterion, ClosureDebitFamily, ClosureEventKind, ClosurePhysicalEvent, ClosureSaleSplit,
    ClosureTickAgg, ClosureVerdict, ClosureWindow, Gold, GoodId,
};
use std::collections::BTreeMap;

const SEED3: u64 = 3;
const RUN_TICKS: u64 = 1_600;
const N: u64 = 160;

// §3.3 P2-1: the mandatory observation-inertness check moved to a LIBRARY unit-test module
// (`sim::settlement::closure::inertness_tests`) because its force-disable hook is now `#[cfg(test)]`
// and thus invisible to this integration-test build. See `closure.rs`.

/// §3.1: the total ClosureClass mapping — every living agent maps to exactly one class at every
/// window boundary; every non-household colonist is a Gatherer.
#[test]
fn closure_class_mapping_is_total() {
    let cfg = SettlementConfig::frontier_closed_circulation();
    let mut s = Settlement::generate(SEED3, &cfg);
    for _ in 0..RUN_TICKS {
        s.econ_tick();
    }
    // Every registered agent maps to exactly one class; the registry is non-empty.
    let registry = s.closure_registry();
    assert!(
        !registry.is_empty(),
        "the registry must cover the founding population"
    );
    for (&id, &class) in registry {
        assert_eq!(
            s.closure_class_of(id),
            Some(class),
            "the class mapping must be single-valued for {id:?}"
        );
    }
}

/// §3.2 R6-3: the InitialHolding / A2FrontLoad disjointness — for a Closed A2 config, the two events
/// sum ONCE to the real generated holding per agent per good, and the A2 component appears only on a
/// producer-subject's staple balance.
#[test]
fn initial_holding_and_a2_frontload_are_disjoint() {
    // A Closed A2 config: the durable-closed stack with the additive producer-house staple.
    let mut cfg = SettlementConfig::frontier_closed_circulation();
    cfg.chain
        .as_mut()
        .expect("chain")
        .producer_house_starting_staple = 4;
    let s = Settlement::generate(SEED3, &cfg);

    // Sum InitialHolding + A2FrontLoad per (agent, good) from the tape.
    let mut init: BTreeMap<(AgentId, GoodId), u32> = BTreeMap::new();
    let mut a2: BTreeMap<(AgentId, GoodId), u32> = BTreeMap::new();
    for ev in s.closure_event_tape() {
        match ev.kind {
            ClosureEventKind::InitialHolding { agent, good, qty } => {
                *init.entry((agent, good)).or_default() += qty;
            }
            ClosureEventKind::A2FrontLoad { agent, good, qty } => {
                *a2.entry((agent, good)).or_default() += qty;
            }
            _ => {}
        }
    }
    // Every A2 component is nonzero and each key appears at most once (disjoint from InitialHolding
    // by construction — they are separate events that SUM to the holding, never the same units).
    let mut any_a2 = false;
    for (&key, &qty) in &a2 {
        assert!(
            qty > 0,
            "an A2FrontLoad event must carry the A2 component, not 0"
        );
        assert!(
            init.contains_key(&key),
            "the A2 balance must also have an InitialHolding"
        );
        any_a2 = true;
    }
    assert!(
        any_a2,
        "the A2 config must front-load at least one producer-subject staple balance"
    );

    // The two sum to the real generated holding for every physical balance.
    let keys: std::collections::BTreeSet<(AgentId, GoodId)> =
        init.keys().chain(a2.keys()).copied().collect();
    for (agent, good) in keys {
        let shadow = init.get(&(agent, good)).copied().unwrap_or(0)
            + a2.get(&(agent, good)).copied().unwrap_or(0);
        let real = s
            .society()
            .agents
            .get(agent)
            .map_or(0, |a| a.stock.get(good));
        assert_eq!(
            shadow, real,
            "InitialHolding + A2FrontLoad must sum ONCE to the real holding for {agent:?} {good:?}"
        );
    }
}

/// §3.3 R6-4 / P1-3: the PIPELINE-level bootstrap-exclusion test — driven through the SAME shared
/// pipeline (`closure_windows` → `closure_classified_windows` → `classify_closure`) the oracle uses,
/// not a duplicate. The bootstrap window [0,160) is printed (present in the absolute grid) but the
/// classifier input begins at [160,320); an endowed debit in a later window still fails CC2.
#[test]
fn bootstrap_window_is_printed_but_excluded_from_classification() {
    let cfg = SettlementConfig::frontier_closed_circulation();
    let mut s = Settlement::generate(SEED3, &cfg);
    for _ in 0..RUN_TICKS {
        s.econ_tick();
    }
    // THE real pipeline (shared with the oracle, P1-3): window aggregation → bootstrap filter.
    let windows = closure_windows(s.closure_tick_aggregates());
    assert_eq!(
        windows[0].start, 0,
        "the bootstrap window [0,160) is printed (present in the absolute grid)"
    );
    let classified = closure_classified_windows(&windows);
    assert_eq!(
        classified[0].start, N,
        "the classifier input begins at [160,320), not the bootstrap window"
    );
    assert!(
        classified.iter().all(|w| w.start >= N),
        "no bootstrap window survives the filter"
    );
    // An endowed physical debit injected into a later CLASSIFIED window still fails CC2 (the
    // exclusion is of the bootstrap window ONLY, never of genuine later leaks).
    let mut leaky = pass_window(320);
    leaky.endowed_physical_debits[ClosureClass::Gatherer.index()] = 5;
    assert_eq!(
        classify_closure(&[pass_window(160), leaky]),
        ClosureVerdict::ClosureLeaks {
            first_window: 320,
            criterion: ClosureCriterion::Cc2,
            class: Some(ClosureClass::Gatherer),
        }
    );
}

/// P1-3: the SHARED pipeline's window aggregation folds CC0 correctly — a class that is present at
/// SOME per-tick samples of a window but not ALL of them disappears mid-window, so that window fails
/// CC0 (present == false). Drives `closure_windows` (the real aggregation the oracle uses) on an
/// injected per-tick aggregate sequence: Miller lives every tick EXCEPT one sample inside [160,320),
/// so [0,160) has Miller present but [160,320) does not → `ClosureStructureAbsent` at 160 for Miller.
#[test]
fn pipeline_fails_cc0_when_a_class_disappears_mid_window() {
    let miller = ClosureClass::Miller.index();
    // Two full windows' worth of ticks (2 * N). Every class lives at every sample, and every class
    // trades every tick, so absent the injection the grid would hold.
    let mut ticks: Vec<ClosureTickAgg> = (0..2 * N)
        .map(|t| ClosureTickAgg {
            tick: t,
            living: [true; 3],
            own_sale_consideration: [1; 3],
            purchase_consideration: [1; 3],
            ..Default::default()
        })
        .collect();
    // Miller disappears for exactly ONE sample inside the SECOND window [160,320) — present at the
    // other 159 samples. Living "at EVERY sample" is required, so the whole window fails CC0.
    let mid = (N + N / 2) as usize;
    ticks[mid].living[miller] = false;
    let windows = closure_windows(&ticks);
    assert_eq!(windows.len(), 2, "two full 160-tick windows");
    assert!(
        windows[0].present[miller],
        "[0,160): Miller present at every sample"
    );
    assert!(
        !windows[1].present[miller],
        "[160,320): a single missing sample makes Miller absent for the whole window"
    );
    // Through the classifier (all windows classified here — the injected grid has no bootstrap
    // meaning) the mid-window disappearance is a CC0 structure failure at that window.
    assert_eq!(
        classify_closure(&windows),
        ClosureVerdict::ClosureStructureAbsent {
            first_window: N,
            class: ClosureClass::Miller,
        }
    );
}

/// §3.3 R6-1 / P1-2: the preregistered seed-value-3 raw-tape recount, made INDEPENDENT and COMPLETE.
/// An independent reference reducer — built SOLELY from the raw `ClosurePhysicalEvent` tape + the
/// actor→class registry, sharing with production ONLY the raw event types, the registry data, and
/// `Gold::mul_qty` (its class/family index mappings are TEST-LOCAL, never the production
/// `ClosureClass::index()`/`ClosureDebitFamily::index()` helpers) — must byte-match the production
/// ledger on EVERY tape-derived per-window result: EVERY sale's explicit bucket decomposition, the
/// per-tick own-production/purchase consideration, endowed_physical_debits by class AND event
/// family, gross commons_goods_drain, and the origin inventories at EVERY 160-tick window boundary
/// (not only the final one). Monetary gold-bucket fields stay outside — no raw gold events ride the
/// tape.
#[test]
fn seed3_raw_tape_recount_matches_the_production_ledger() {
    let cfg = SettlementConfig::frontier_closed_circulation();
    let mut s = Settlement::generate(SEED3, &cfg);
    for _ in 0..RUN_TICKS {
        s.econ_tick();
    }

    // The independent reference reducer over the raw tape + registry.
    let reference = RefReducer::replay(s.closure_event_tape(), s.closure_registry(), RUN_TICKS);

    // (a) Per-tick tape-derived aggregates.
    let production = s.closure_tick_aggregates();
    for (t, prod) in production.iter().enumerate() {
        let refa = reference
            .ticks
            .get(&(t as u64))
            .cloned()
            .unwrap_or_default();
        assert_eq!(
            prod.own_sale_consideration, refa.own_sale,
            "own-production sale consideration mismatch at tick {t}"
        );
        assert_eq!(
            prod.purchase_consideration, refa.purchase,
            "purchase consideration mismatch at tick {t}"
        );
        assert_eq!(
            prod.endowed_physical_debits, refa.endowed_physical_debits,
            "endowed_physical_debits mismatch at tick {t}"
        );
        assert_eq!(
            prod.endowed_physical_debits_by_family, refa.endowed_by_family,
            "endowed_physical_debits by (class, family) mismatch at tick {t}"
        );
        assert_eq!(
            prod.commons_goods_drain, refa.commons_goods_drain,
            "gross commons_goods_drain mismatch at tick {t}"
        );
    }

    // (b) EVERY sale's explicit bucket decomposition (not just the aggregate families).
    let prod_sales = s.closure_sale_splits();
    assert!(
        !prod_sales.is_empty(),
        "the seed-3 run must settle at least one sale to recount"
    );
    assert_eq!(
        prod_sales.len(),
        reference.sale_splits.len(),
        "the reducers must decompose the SAME number of sales"
    );
    for (prod_split, ref_split) in prod_sales.iter().zip(&reference.sale_splits) {
        assert_eq!(
            *prod_split, *ref_split,
            "per-sale origin decomposition mismatch (tick {}, trade {})",
            prod_split.tick, prod_split.trade_id
        );
    }

    // (c) Origin inventories at EVERY 160-tick window boundary (not only the final one).
    let prod_boundaries = s.closure_boundary_inventories();
    assert_eq!(
        prod_boundaries.len(),
        (RUN_TICKS / N) as usize,
        "one boundary snapshot per window"
    );
    for (start, prod_inv) in prod_boundaries {
        let ref_inv = reference
            .boundaries
            .get(start)
            .expect("the reference must snapshot the same window boundaries");
        assert_eq!(
            prod_inv, ref_inv,
            "boundary origin inventory mismatch at window w@{start}"
        );
    }
    // The final boundary equals the live inventory snapshot (a cross-check of the accessor).
    assert_eq!(
        s.closure_inventory_snapshot(),
        reference.positive_inventory(),
        "the reducers must agree on the final origin inventory"
    );
}

/// §3.2: the market's authoritative phase order is consumption before settled trades. The raw tape
/// must preserve that order so a seller that consumes and sells the same good debits the correct
/// origin buckets before the sale split is priced.
#[test]
fn market_consumption_precedes_settled_trades_on_the_raw_tape() {
    let cfg = SettlementConfig::frontier_closed_circulation();
    let mut s = Settlement::generate(SEED3, &cfg);
    for _ in 0..N {
        s.econ_tick();
    }

    let mut exercised = false;
    for tick in 0..N {
        let events: Vec<_> = s
            .closure_event_tape()
            .iter()
            .filter(|event| event.tick == tick)
            .collect();
        let first_consumption = events.iter().find_map(|event| {
            matches!(event.kind, ClosureEventKind::Consumption { .. }).then_some(event.order)
        });
        let first_trade = events.iter().find_map(|event| {
            matches!(event.kind, ClosureEventKind::SettledSpotTrade { .. }).then_some(event.order)
        });
        if let (Some(consumption), Some(trade)) = (first_consumption, first_trade) {
            exercised = true;
            assert!(
                consumption < trade,
                "market consumption must precede settled trades on the raw tape at tick {tick}"
            );
        }
    }
    assert!(
        exercised,
        "the seed-3 bootstrap must exercise a tick containing both consumption and trade"
    );
}

// ---- helpers ----

/// A window that passes CC0–CC3 for every required class (mirrors the classify unit tests).
fn pass_window(start: u64) -> ClosureWindow {
    ClosureWindow {
        start,
        present: [true; 3],
        own_sale_consideration: [1, 1, 1],
        purchase_consideration: [1, 1, 1],
        endowed_purchase_debits: [0; 3],
        endowed_physical_debits: [0; 3],
        commons_drain: 0,
        commons_goods_drain: 0,
        wage_escrow_gold: 0,
        land_fee_pool_salt: 0,
    }
}

/// A per-agent per-good origin-inventory snapshot `(endowed, acquired, own_produced)`, matching the
/// production `closure_inventory_snapshot` shape (defined test-locally so the recount shares no type
/// helper beyond the raw vocabulary).
type OriginInventory = BTreeMap<AgentId, BTreeMap<GoodId, (u32, u32, u32)>>;

/// One tick's tape-derived aggregates, reproduced independently from the raw tape.
#[derive(Clone, Default, PartialEq, Eq)]
struct RefTickAgg {
    own_sale: [u64; 3],
    purchase: [u64; 3],
    endowed_physical_debits: [u64; 3],
    endowed_by_family: [[u64; 5]; 3],
    commons_goods_drain: u64,
}

/// TEST-LOCAL class index (independent of the production `ClosureClass::index`, P1-2). Shares only
/// the raw `ClosureClass` vocabulary (registry data); the mapping is re-derived here, so a production
/// index bug cannot propagate into the recount. Must agree with the production per-class array layout
/// `[Gatherer, Miller, Baker]` for the byte-match to hold — that agreement is exactly what is tested.
fn class_index(c: ClosureClass) -> usize {
    match c {
        ClosureClass::Gatherer => 0,
        ClosureClass::Miller => 1,
        ClosureClass::Baker => 2,
    }
}

/// TEST-LOCAL debit-family index (independent of the production `ClosureDebitFamily::index`, P1-2).
/// Re-derived from the raw `ClosureDebitFamily` vocabulary; must agree with the production
/// `[[u64;5];3]` family layout `[Sale, Consumption, RecipeInput, CapitalInput, Spoilage]`.
fn family_index(f: ClosureDebitFamily) -> usize {
    match f {
        ClosureDebitFamily::Sale => 0,
        ClosureDebitFamily::Consumption => 1,
        ClosureDebitFamily::RecipeInput => 2,
        ClosureDebitFamily::CapitalInput => 3,
        ClosureDebitFamily::Spoilage => 4,
    }
}

/// The INDEPENDENT reference reducer (P1-2): it re-implements the physical reduction from scratch —
/// its own bucket-consumption order, its own class/family index mappings (`class_index`/
/// `family_index`, never the production `.index()` helpers), and its own aggregation — consuming ONLY
/// the raw tape + the registry, plus `Gold::mul_qty` (the one shared arithmetic, §3.2). It records
/// EVERY sale's split and the origin inventory at EVERY 160-tick window boundary.
#[derive(Default)]
struct RefReducer {
    inv: BTreeMap<AgentId, BTreeMap<GoodId, [u32; 3]>>,
    ticks: BTreeMap<u64, RefTickAgg>,
    /// Every settled sale's decomposition, in tape order (compared per-sale against production).
    sale_splits: Vec<ClosureSaleSplit>,
    /// Origin inventory at each 160-tick window boundary (window start → per-agent per-good split).
    boundaries: BTreeMap<u64, OriginInventory>,
}

impl RefReducer {
    fn replay(
        tape: &[ClosurePhysicalEvent],
        registry: &BTreeMap<AgentId, ClosureClass>,
        run_ticks: u64,
    ) -> Self {
        let mut r = RefReducer::default();
        // Snapshot the origin inventory at the END of each 160-tick window: the tape is ordered by
        // (tick, order), so when the first event of a later window arrives, every event of the
        // windows before it has been applied — the boundary state (matching the production ledger's
        // finalize-tick snapshot at ticks 159, 319, …).
        let mut next_end = N;
        for ev in tape {
            while ev.tick >= next_end {
                let snap = r.positive_inventory();
                r.boundaries.insert(next_end - N, snap);
                next_end += N;
            }
            r.apply(ev, registry);
        }
        while next_end <= run_ticks {
            let snap = r.positive_inventory();
            r.boundaries.insert(next_end - N, snap);
            next_end += N;
        }
        r
    }

    fn class(registry: &BTreeMap<AgentId, ClosureClass>, agent: AgentId) -> Option<usize> {
        registry.get(&agent).copied().map(class_index)
    }

    /// Consume `qty` in the fixed order endowed → acquired → own_produced; returns `[e, a, o]`.
    fn debit(&mut self, agent: AgentId, good: GoodId, qty: u32) -> [u32; 3] {
        let b = self.inv.entry(agent).or_default().entry(good).or_default();
        let e = b[0].min(qty);
        b[0] -= e;
        let mut rest = qty - e;
        let a = b[1].min(rest);
        b[1] -= a;
        rest -= a;
        let o = b[2].min(rest);
        b[2] -= o;
        [e, a, o]
    }

    fn credit(&mut self, agent: AgentId, good: GoodId, qty: u32, slot: usize) {
        if qty == 0 {
            return;
        }
        self.inv.entry(agent).or_default().entry(good).or_default()[slot] += qty;
    }

    fn post_endowed(
        &mut self,
        tick: u64,
        class: Option<usize>,
        family: ClosureDebitFamily,
        e: u32,
    ) {
        if e == 0 {
            return;
        }
        if let Some(c) = class {
            let a = self.ticks.entry(tick).or_default();
            a.endowed_physical_debits[c] += u64::from(e);
            a.endowed_by_family[c][family_index(family)] += u64::from(e);
        }
    }

    fn apply(&mut self, ev: &ClosurePhysicalEvent, registry: &BTreeMap<AgentId, ClosureClass>) {
        let tick = ev.tick;
        match ev.kind {
            ClosureEventKind::InitialHolding { agent, good, qty }
            | ClosureEventKind::A2FrontLoad { agent, good, qty }
            | ClosureEventKind::BSupportCredit { agent, good, qty } => {
                self.credit(agent, good, qty, 0);
            }
            ClosureEventKind::GatherDeposit { agent, good, qty } => {
                self.credit(agent, good, qty, 2);
            }
            ClosureEventKind::SettledSpotTrade {
                seller,
                buyer,
                good,
                qty,
                price,
                trade_id,
            } => {
                let split = self.debit(seller, good, qty);
                self.credit(buyer, good, qty, 1);
                // EVERY sale's explicit decomposition, keyed to its trade id (P1-2).
                self.sale_splits.push(ClosureSaleSplit {
                    tick,
                    trade_id,
                    endowed: split[0],
                    acquired: split[1],
                    own_produced: split[2],
                });
                let sc = Self::class(registry, seller);
                let bc = Self::class(registry, buyer);
                if let Some(c) = sc {
                    let own = price.mul_qty(split[2]).unwrap_or(Gold::ZERO).0;
                    self.ticks.entry(tick).or_default().own_sale[c] += own;
                }
                if let Some(c) = bc {
                    let paid = price.mul_qty(qty).unwrap_or(Gold::ZERO).0;
                    self.ticks.entry(tick).or_default().purchase[c] += paid;
                }
                self.post_endowed(tick, sc, ClosureDebitFamily::Sale, split[0]);
            }
            ClosureEventKind::RecipeProduction {
                agent,
                input,
                input_qty,
                output,
                output_qty,
            } => {
                let split = self.debit(agent, input, input_qty);
                self.credit(agent, output, output_qty, 2);
                self.post_endowed(
                    tick,
                    Self::class(registry, agent),
                    ClosureDebitFamily::RecipeInput,
                    split[0],
                );
            }
            ClosureEventKind::CapitalFormation {
                agent,
                input,
                input_qty,
                tool,
                tool_qty,
            } => {
                let split = self.debit(agent, input, input_qty);
                self.credit(agent, tool, tool_qty, 2);
                self.post_endowed(
                    tick,
                    Self::class(registry, agent),
                    ClosureDebitFamily::CapitalInput,
                    split[0],
                );
            }
            ClosureEventKind::Consumption { agent, good, qty } => {
                let split = self.debit(agent, good, qty);
                self.post_endowed(
                    tick,
                    Self::class(registry, agent),
                    ClosureDebitFamily::Consumption,
                    split[0],
                );
            }
            ClosureEventKind::Spoilage { agent, good, qty } => {
                let split = self.debit(agent, good, qty);
                self.post_endowed(
                    tick,
                    Self::class(registry, agent),
                    ClosureDebitFamily::Spoilage,
                    split[0],
                );
            }
            ClosureEventKind::HouseholdTransfer {
                from,
                to,
                good,
                qty,
            } => {
                let split = self.debit(from, good, qty);
                self.credit(to, good, split[0], 0);
                self.credit(to, good, split[1], 1);
                self.credit(to, good, split[2], 2);
            }
            ClosureEventKind::EstateToCommons { agent, good, qty } => {
                self.debit(agent, good, qty);
                self.ticks.entry(tick).or_default().commons_goods_drain += u64::from(qty);
            }
        }
    }

    fn positive_inventory(&self) -> OriginInventory {
        let mut out: OriginInventory = BTreeMap::new();
        for (&agent, goods) in &self.inv {
            for (&good, b) in goods {
                if b[0] + b[1] + b[2] > 0 {
                    out.entry(agent)
                        .or_default()
                        .insert(good, (b[0], b[1], b[2]));
                }
            }
        }
        out
    }
}
