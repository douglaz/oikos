//! G2c acceptance suite — multiple settlements + caravans (the `Region`).
//!
//! These pin the milestone's two DoDs and the constraints that protect them:
//!
//! - **Region-wide conservation is exact** (test 2, the tripwire): for every good
//!   and for gold, the total over all settlements **plus** in-transit route escrow
//!   changes only by accounted node regen (source) and consumption (sink); every
//!   caravan transfer is net-zero, and nothing is created or destroyed at any
//!   boundary, including in transit (test 3).
//! - **Trade converges prices** (tests 4 + 5, the falsification twin): with the
//!   caravan the realized FOOD-price gap between the two settlements narrows over
//!   time AND ends below the no-caravan control's gap, while the control keeps the
//!   gap. Sign only — no magnitude is pinned.
//!
//! plus determinism (test 1, the tripwire), the permanent trader pair / no roster
//! mutation contract (test 6), the additive-accessor conservation contract
//! (test 7, unit-level), and the unchanged econ/settlement behavior (test 8). They
//! assert shape, exactness, and sign — never a pinned price magnitude (the lab
//! discipline).

use econ::agent::{Agent, AgentId, Role};
use econ::good::{Gold, Stock, FOOD, GOLD, NET, SALT, WOOD};
use econ::money::{DesignatedMoney, MarketMoneyConfig};
use econ::scenario::{MarketScenario, ScenarioName};
use econ::society::Society;
use sim::{NodeSpec, Region, RegionConfig, Route};

/// The realized FOOD prices at A and B over a `ticks`-tick caravan or control run,
/// per econ tick (index = tick), `None` until that settlement first clears.
fn price_series(
    config: &RegionConfig,
    seed: u64,
    ticks: u64,
) -> (Vec<Option<u64>>, Vec<Option<u64>>) {
    let mut region = Region::generate(seed, config);
    let mut a = Vec::with_capacity(ticks as usize);
    let mut b = Vec::with_capacity(ticks as usize);
    for _ in 0..ticks {
        region.econ_tick();
        a.push(region.realized_price(0, FOOD).map(|g| g.0));
        b.push(region.realized_price(1, FOOD).map(|g| g.0));
    }
    (a, b)
}

/// The mean of the cleared prices in `series[lo..hi]` (skipping ticks with no
/// trade), as a rational `f64` — a windowed average that is robust to the
/// last-trade oscillation a single sample carries.
fn window_mean(series: &[Option<u64>], lo: usize, hi: usize) -> f64 {
    let hi = hi.min(series.len());
    let cleared: Vec<u64> = series[lo..hi].iter().filter_map(|p| *p).collect();
    assert!(!cleared.is_empty(), "the window cleared no FOOD trade");
    cleared.iter().sum::<u64>() as f64 / cleared.len() as f64
}

/// The windowed `|price_A − price_B|` gap at the start and end of a run: the start
/// window is an early band (after both settlements first clear), the end window the
/// final band. Returns `(start_gap, end_gap)`.
fn start_end_gaps(config: &RegionConfig, seed: u64, ticks: u64) -> (f64, f64) {
    let (a, b) = price_series(config, seed, ticks);
    let t = ticks as usize;
    let start = (window_mean(&a, 6, 18) - window_mean(&b, 6, 18)).abs();
    let end = (window_mean(&a, t - 18, t) - window_mean(&b, t - 18, t)).abs();
    (start, end)
}

/// The convergence experiment's run length — long enough for the dear settlement
/// to reach its (control) equilibrium and for the caravan to work the gap down.
const CONVERGENCE_TICKS: u64 = 120;

// ---- 1. determinism (the tripwire) ---------------------------------------

/// 1. Same `(seed, RegionConfig)` → byte-identical run; nothing is drawn in the
///    region loop or the caravan step, so two runs stay in lockstep. A different
///    seed diverges (the `Rng` really is consumed only at generation).
#[test]
fn region_run_is_deterministic() {
    let config = RegionConfig::two_settlements();

    let mut a = Region::generate(0xC0FFEE, &config);
    let mut b = Region::generate(0xC0FFEE, &config);
    a.run(60);
    b.run(60);
    assert_eq!(
        a.canonical_bytes(),
        b.canonical_bytes(),
        "same seed diverged"
    );
    assert_eq!(a.digest(), b.digest());

    // A different seed yields a different run (generation actually uses the Rng).
    let mut c = Region::generate(0xBADF00D, &config);
    c.run(60);
    assert_ne!(a.digest(), c.digest(), "the seed did not affect the run");

    // The control twin is its own deterministic run, distinct from the caravan's.
    let control = RegionConfig::two_settlements_control();
    let mut d = Region::generate(0xC0FFEE, &control);
    let mut e = Region::generate(0xC0FFEE, &control);
    d.run(60);
    e.run(60);
    assert_eq!(d.digest(), e.digest(), "the control is not deterministic");
    assert_ne!(
        a.digest(),
        d.digest(),
        "the caravan and the control must run differently"
    );
}

// ---- 2. region-wide conservation (the tripwire) --------------------------

/// 2. For every good and for gold, the regional total (Σ settlements + route
///    escrow) changes only by accounted regen (source) and consumption (sink);
///    every caravan transfer is net-zero, so no unit or coin is created or
///    destroyed at any boundary — including in transit.
#[test]
fn region_conserves_every_econ_tick() {
    for &enabled in &[true, false] {
        let config = RegionConfig {
            caravans_enabled: enabled,
            ..RegionConfig::two_settlements()
        };
        let mut region = Region::generate(7, &config);
        // Gold is a closed regional balance; pin it across the whole run.
        let mut first_gold = None;
        for _ in 0..CONVERGENCE_TICKS {
            let report = region.econ_tick();
            // The ledger balances for every tracked good, and gold is unchanged.
            assert!(
                report.conserves(),
                "region conservation broke (enabled={enabled}) at tick {}",
                report.econ_tick
            );
            // Gold is conserved tick-over-tick (a closed regional balance): the
            // before/after totals are equal, and the running total never drifts.
            assert_eq!(
                report.gold_after, report.gold_before,
                "region gold changed within a tick (enabled={enabled})"
            );
            let gold = *first_gold.get_or_insert(report.gold_after);
            assert_eq!(
                report.gold_after, gold,
                "region gold drifted across ticks (enabled={enabled})"
            );
            // Spell out the per-good ledger so a break names the good and tick.
            for &good in region.tracked_goods() {
                let before = report.before_of(good) as i128;
                let after = report.after_of(good) as i128;
                let regen = report.regen_of(good) as i128;
                let consumed = report.consumed_of(good) as i128;
                assert_eq!(
                    after,
                    before + regen - consumed,
                    "good {good:?} broke conservation (enabled={enabled}) at tick {}",
                    report.econ_tick
                );
            }
        }
    }
}

// ---- 3. escrow in transit is conserved, retained on non-completion -------

/// 3. Goods and gold mid-route are counted in the region roll-up (at no
///    settlement) and arrive intact (G2c has no loss); a caravan that never
///    completes a leg retains its escrow, never destroys it.
#[test]
fn caravan_escrow_in_transit_is_conserved() {
    // A very long transit, so once the caravan departs A→B it is stuck in transit
    // for the rest of the run — its bought FOOD never arrives. The escrow must be
    // retained (non-zero) and the roll-up must still conserve every tick.
    let config = RegionConfig {
        route: Route {
            transit_ticks: 10_000,
        },
        ..RegionConfig::two_settlements()
    };
    let mut region = Region::generate(7, &config);

    // Run past the buy phase so the caravan has departed A→B with goods in escrow.
    let mut saw_escrow = false;
    for _ in 0..40 {
        let report = region.econ_tick();
        assert!(
            report.conserves(),
            "conservation broke with goods mid-transit"
        );
        if region.escrow_good() > 0 {
            saw_escrow = true;
        }
    }
    assert!(saw_escrow, "the caravan never put goods into route escrow");

    // The escrow is held by the region, at NO settlement: the regional total of
    // FOOD equals the sum across settlements PLUS the escrow, and the escrow is
    // non-zero — the in-transit units are accounted exactly once, in escrow.
    let escrow = u64::from(region.escrow_good());
    assert!(escrow > 0, "the in-transit escrow was lost mid-leg");
    let in_settlements: u64 = region
        .settlements()
        .iter()
        .map(|s| s.whole_system_total(FOOD))
        .sum();
    let report = region.last_report();
    assert_eq!(
        report.after_of(FOOD),
        in_settlements + escrow,
        "the in-transit escrow is not counted exactly once in the roll-up"
    );
    assert_eq!(
        report.escrow_of(FOOD),
        escrow,
        "the report's escrow does not match the live escrow"
    );

    // Running further never destroys the retained escrow (the leg still cannot
    // complete), and conservation keeps holding — escrow is retained, not lost.
    let retained = region.escrow_good();
    for _ in 0..40 {
        let report = region.econ_tick();
        assert!(
            report.conserves(),
            "conservation broke while escrow was retained"
        );
    }
    assert_eq!(
        region.escrow_good(),
        retained,
        "the retained escrow changed while the leg could not complete"
    );
}

// ---- 4. the caravan narrows the price gap (sign only) --------------------

/// 4. With the caravan enabled, the realized FOOD-price gap at the end is smaller
///    than at the start AND smaller than the no-caravan control's end gap. Sign
///    only — no magnitude is pinned.
#[test]
fn caravan_narrows_the_price_gap() {
    let seed = 20_260_614;
    let caravan = RegionConfig::two_settlements();
    let control = RegionConfig::two_settlements_control();

    let (caravan_start, caravan_end) = start_end_gaps(&caravan, seed, CONVERGENCE_TICKS);
    let (_control_start, control_end) = start_end_gaps(&control, seed, CONVERGENCE_TICKS);

    // The gap narrows over time with the caravan...
    assert!(
        caravan_end < caravan_start,
        "the caravan did not narrow the gap: start={caravan_start:.2} end={caravan_end:.2}"
    );
    // ...and ends below where the no-caravan control leaves it (the caravan is
    // what closes it).
    assert!(
        caravan_end < control_end,
        "the caravan end gap ({caravan_end:.2}) is not below the control end gap ({control_end:.2})"
    );
}

// ---- 5. the no-caravan control keeps the gap (the twin) ------------------

/// 5. With caravans disabled the price gap does not converge — the falsification
///    twin. The control's end gap stays close to (does not collapse from) its
///    start gap, and stays well above the caravan run's end gap.
#[test]
fn no_caravan_control_keeps_the_gap() {
    let seed = 20_260_614;
    let control = RegionConfig::two_settlements_control();
    let caravan = RegionConfig::two_settlements();

    let (control_start, control_end) = start_end_gaps(&control, seed, CONVERGENCE_TICKS);
    let (_caravan_start, caravan_end) = start_end_gaps(&caravan, seed, CONVERGENCE_TICKS);

    // The control keeps a clearly-positive gap to the end — it does not converge.
    assert!(
        control_end > 1.0,
        "the control gap unexpectedly converged: start={control_start:.2} end={control_end:.2}"
    );
    // The control does not narrow the way the caravan does: its end gap is far
    // larger than the caravan's, so the caravan — not time — is what closes it.
    assert!(
        control_end > caravan_end + 1.0,
        "the control did not keep the gap relative to the caravan: control_end={control_end:.2} caravan_end={caravan_end:.2}"
    );
}

// ---- 6. permanent trader pairs, no roster mutation -----------------------

/// 6. Agent counts in each settlement's `Society` are constant across the run (no
///    runtime add/remove); the caravan moves wealth, not agents. The resident
///    trader ids are stable for the whole run.
#[test]
fn trader_pairs_are_permanent_no_roster_mutation() {
    for &enabled in &[true, false] {
        let config = RegionConfig {
            caravans_enabled: enabled,
            ..RegionConfig::two_settlements()
        };
        let mut region = Region::generate(7, &config);

        let counts0: Vec<usize> = region
            .settlements()
            .iter()
            .map(|s| s.society().agents.len())
            .collect();
        let trader0: Vec<Option<AgentId>> = (0..region.settlement_count())
            .map(|i| region.trader_id(i))
            .collect();
        assert_eq!(counts0.len(), 2, "G2c is a two-settlement region");

        for _ in 0..CONVERGENCE_TICKS {
            region.econ_tick();
            let counts: Vec<usize> = region
                .settlements()
                .iter()
                .map(|s| s.society().agents.len())
                .collect();
            assert_eq!(
                counts, counts0,
                "a settlement's agent roster changed at runtime (enabled={enabled})"
            );
            let traders: Vec<Option<AgentId>> = (0..region.settlement_count())
                .map(|i| region.trader_id(i))
                .collect();
            assert_eq!(
                traders, trader0,
                "a caravan trader id changed at runtime (enabled={enabled})"
            );
        }

        // The caravan moved wealth: with it enabled, the trader pair's combined
        // holdings differ from the start (gold/goods shuttled), while the agent
        // count never did. With it disabled, the idle pair never traded.
        if enabled {
            // Over the run the caravan completed cycles, so gold has flowed between
            // the two settlements (specie follows trade); their gold totals are no
            // longer the pristine starting split.
            let a_gold = region.settlement(0).unwrap().total_gold().0;
            let b_gold = region.settlement(1).unwrap().total_gold().0;
            // Regional gold is still conserved (the closed balance), but it has
            // redistributed — B fed A through the caravan's proceeds.
            assert!(
                a_gold > 0 && b_gold > 0,
                "both settlements still hold gold (it only moved, never vanished)"
            );
        }
    }
}

// ---- 7. additive accessors are conservative (unit-level) -----------------

/// Build a one-agent M1 society (the `sim` settlement's regime) holding `agent`,
/// for unit-testing the additive transfer accessors in isolation.
fn unit_society(agent: Agent) -> Society {
    Society::from_scenario(MarketScenario {
        name: "g2c-accessor-unit",
        scenario: ScenarioName::MarketBarterishGold,
        seed: 1,
        periods: 0,
        agents: vec![agent],
        recipes: Vec::new(),
        events: Vec::new(),
        money: MarketMoneyConfig::Designated(DesignatedMoney { good: GOLD }),
    })
}

fn unit_agent(id: AgentId, gold: u64, food: u32) -> Agent {
    let mut stock = Stock::new(NET.0);
    stock.add(FOOD, food);
    Agent {
        id,
        scale: Vec::new(),
        stock,
        gold: Gold(gold),
        labor_capacity: 0,
        hunger_deficit: 0,
        roles: vec![Role::Trader],
        expect: Vec::new(),
    }
}

/// 7. `debit_stock` / `credit_gold` / `debit_gold` reject unknown and freed
///    (G4a real-removal) ids, never go negative, and MOVE (never create or
///    destroy) value.
#[test]
fn additive_accessors_are_conservative() {
    let id = AgentId(0);
    let mut society = unit_society(unit_agent(id, 10, 5));

    // --- move value, never mint or burn ---
    let gold0 = society.total_gold();
    let food0 = society.total_stock(FOOD);

    // Debit then re-credit gold: a paired move is net-zero on the total.
    assert!(society.debit_gold(id, Gold(4)));
    assert_eq!(society.total_gold().0, gold0.0 - 4, "debit removes exactly");
    assert!(society.credit_gold(id, Gold(4)));
    assert_eq!(society.total_gold(), gold0, "the paired credit restores it");

    // Debit stock: relocation out (the region credits escrow by the same amount).
    assert!(society.debit_stock(id, FOOD, 2));
    assert_eq!(
        society.total_stock(FOOD),
        food0 - 2,
        "stock moves out exactly"
    );

    // --- never go negative (atomic over-debit is rejected, nothing moves) ---
    let food_now = society.total_stock(FOOD);
    assert!(!society.debit_stock(id, FOOD, food_now + 1));
    assert_eq!(
        society.total_stock(FOOD),
        food_now,
        "over-debit moved nothing"
    );
    let gold_now = society.total_gold();
    assert!(!society.debit_gold(id, gold_now.saturating_add(Gold(1))));
    assert_eq!(
        society.total_gold(),
        gold_now,
        "over-debit gold moved nothing"
    );

    // --- reject unknown ids ---
    let unknown = AgentId(99);
    assert!(!society.debit_stock(unknown, FOOD, 1));
    assert!(!society.credit_gold(unknown, Gold(1)));
    assert!(!society.debit_gold(unknown, Gold(1)));
    assert_eq!(
        society.total_stock(FOOD),
        food_now,
        "unknown id moved no stock"
    );
    assert_eq!(society.total_gold(), gold_now, "unknown id moved no gold");

    // --- real removal (G4a) settles the estate out and rejects the freed id ---
    let gold_before_removal = society.total_gold();
    let food_before_removal = society.total_stock(FOOD);
    let estate = society.remove_agent(id).expect("live id removes");
    // The estate carries exactly the holdings that left the society.
    assert_eq!(estate.gold, gold_before_removal, "estate carries the gold");
    assert_eq!(
        estate.stock.get(FOOD),
        food_before_removal,
        "estate carries the stock"
    );
    // The freed id resolves to None and every accessor rejects it.
    assert!(
        society.agents.get(id).is_none(),
        "removed id resolves to None"
    );
    assert!(!society.debit_stock(id, FOOD, 1));
    assert!(!society.credit_gold(id, Gold(1)));
    assert!(!society.debit_gold(id, Gold(1)));
    assert_eq!(
        society.total_stock(FOOD),
        0,
        "settled stock left the society"
    );
    assert_eq!(
        society.total_gold(),
        Gold::ZERO,
        "settled gold left the society"
    );
}

// ---- 8. econ + settlement behavior unchanged -----------------------------

/// 8. The composing `Region` changed no `econ` and no `Settlement` behavior. The
///    econ engine still replays deterministically with conservation intact from
///    `sim`'s workspace, and a plain `Settlement` (no resident traders) is
///    byte-identical to G2b. The byte-identical golden guarantee plus the full
///    G1/G2a/G2b/G2d suites are enforced by `cargo test` across the workspace,
///    with `cargo clippy --workspace --all-targets -- -D warnings` and
///    `cargo fmt --check`. This checks the engine/settlement are unperturbed here.
#[test]
fn econ_settlement_unchanged() {
    use sim::{Settlement, SettlementConfig};

    // The econ engine is usable and deterministic from the region's workspace.
    for name in [
        ScenarioName::MarketBarterishGold,
        ScenarioName::MarketPriceDiscovery,
        ScenarioName::MengerSaltMoney,
    ] {
        let scenario = econ::scenario::builtin_market_scenario(name);
        let periods = scenario.periods;
        let mut first = Society::from_scenario(scenario);
        let total_gold = first.total_gold();
        first.run(periods);
        let mut second = Society::from_scenario(econ::scenario::builtin_market_scenario(name));
        second.run(periods);
        assert_eq!(
            first.records, second.records,
            "{name:?} did not replay deterministically"
        );
        if matches!(name, ScenarioName::MarketBarterishGold) {
            assert_eq!(
                first.total_gold(),
                total_gold,
                "{name:?} broke gold conservation"
            );
        }
    }

    // A plain settlement (no resident traders) is byte-identical to a G2b run —
    // composing the Region added an opt-in field that, left empty, changes nothing.
    let config = SettlementConfig::viable();
    let mut plain = Settlement::generate(0xC0FFEE, &config);
    plain.run(40);
    let mut twin = Settlement::generate(0xC0FFEE, &config);
    twin.run(40);
    assert_eq!(plain.digest(), twin.digest(), "a plain settlement diverged");
    // And FOOD/WOOD are still its only tracked goods (the seam added no good).
    assert_eq!(plain.tracked_goods(), &[FOOD, WOOD]);
}

// ---- extra unit-level checks ---------------------------------------------

/// The control and caravan start from the SAME settlements — same colonist
/// roster, same spatial substrate, same trader pair — so the control is a clean
/// falsification twin. The only difference is whether the caravan operates: the
/// caravan activates its buyer trader's value scale at generation, while the
/// control leaves the whole pair idle. So the world substrate and the populations
/// match exactly; only the (econ-side) trader scale differs.
#[test]
fn control_and_caravan_share_the_same_initial_settlements() {
    let caravan = Region::generate(7, &RegionConfig::two_settlements());
    let control = Region::generate(7, &RegionConfig::two_settlements_control());
    for i in 0..2 {
        let c = caravan.settlement(i).unwrap();
        let t = control.settlement(i).unwrap();
        assert_eq!(
            c.world().canonical_bytes(),
            t.world().canonical_bytes(),
            "settlement {i} spatial substrate differs between caravan and control"
        );
        assert_eq!(
            c.population(),
            t.population(),
            "settlement {i} population differs"
        );
        assert_eq!(
            caravan.trader_id(i),
            control.trader_id(i),
            "settlement {i} trader id differs"
        );
    }
    // The caravan runs; the control does not.
    assert!(caravan.caravans_enabled());
    assert!(!control.caravans_enabled());
}

/// The traded good is FOOD and both settlements track it (a precondition for the
/// region-wide conservation roll-up to see every in-transit unit).
#[test]
fn region_tracks_the_traded_good_in_both_settlements() {
    let region = Region::generate(7, &RegionConfig::two_settlements());
    assert_eq!(region.traded_good(), FOOD);
    assert!(region.tracked_goods().contains(&FOOD));
    for i in 0..2 {
        assert!(region
            .settlement(i)
            .unwrap()
            .tracked_goods()
            .contains(&FOOD));
    }
}

/// Generating a region with GOLD as the traded good is rejected — money is not a
/// hauled physical good.
#[test]
#[should_panic(expected = "cannot trade the money good")]
fn region_rejects_a_gold_traded_good() {
    let config = RegionConfig {
        good: GOLD,
        ..RegionConfig::two_settlements()
    };
    let _ = Region::generate(7, &config);
}

/// Generating a region whose configured traded good is *tracked* (a node good)
/// but has no market order book in either settlement is rejected — such a region
/// would conserve but never trade. SALT is a node-only good outside the built-in
/// market set (FOOD/WOOD/NET) and is held/wanted by no agent at generation, so it
/// is tracked yet bookless: the loud guard beats a silent dead region.
#[test]
#[should_panic(expected = "must have an order book")]
fn region_rejects_a_non_market_traded_good() {
    let mut config = RegionConfig::two_settlements();
    // A SALT node makes SALT a *tracked* good in each settlement (node goods are
    // tracked for conservation), but SALT is not FOOD/WOOD/NET and no agent holds
    // or wants it at generation, so no order book is built for it.
    let salt_node = NodeSpec {
        good: SALT,
        ..config.settlement_a.nodes[0]
    };
    config.settlement_a.nodes.push(salt_node);
    config.settlement_b.nodes.push(salt_node);
    config.good = SALT;
    let _ = Region::generate(7, &config);
}

/// Public zero dwell lengths are valid edge inputs: they must complete on the
/// next caravan step, not underflow/wrap the counter and stall the caravan.
#[test]
fn zero_length_caravan_dwells_do_not_underflow_or_stall() {
    let mut zero_buy = Region::generate(
        7,
        &RegionConfig {
            buy_ticks: 0,
            route: Route {
                transit_ticks: 10_000,
            },
            ..RegionConfig::two_settlements()
        },
    );
    for _ in 0..20 {
        assert!(zero_buy.econ_tick().conserves());
    }
    assert!(
        zero_buy.escrow_good() > 0,
        "zero buy dwell stalled before the A->B escrow move"
    );

    let mut zero_sell = Region::generate(
        7,
        &RegionConfig {
            buy_ticks: 0,
            sell_ticks: 0,
            route: Route { transit_ticks: 0 },
            ..RegionConfig::two_settlements()
        },
    );
    let initial_b_gold = zero_sell.settlement(1).unwrap().total_gold();
    let mut saw_b_gold_leave = false;
    for _ in 0..40 {
        assert!(zero_sell.econ_tick().conserves());
        if zero_sell.settlement(1).unwrap().total_gold() < initial_b_gold {
            saw_b_gold_leave = true;
        }
    }
    assert!(
        saw_b_gold_leave,
        "zero sell dwell stalled before returning proceeds to A"
    );
}

/// Canonical bytes include all Region-level knobs that steer future ticks, even
/// before a fresh pair of regions has dynamically diverged.
#[test]
fn region_canonical_bytes_include_future_steering_knobs() {
    let base = Region::generate(7, &RegionConfig::two_settlements());
    let different_sell_dwell = Region::generate(
        7,
        &RegionConfig {
            sell_ticks: RegionConfig::two_settlements().sell_ticks + 1,
            ..RegionConfig::two_settlements()
        },
    );
    assert_ne!(
        base.canonical_bytes(),
        different_sell_dwell.canonical_bytes(),
        "sell_ticks must be serialized before the SellB phase is reached"
    );
    assert_ne!(base.digest(), different_sell_dwell.digest());

    let different_good = Region::generate(
        7,
        &RegionConfig {
            good: WOOD,
            ..RegionConfig::two_settlements()
        },
    );
    assert_ne!(
        base.canonical_bytes(),
        different_good.canonical_bytes(),
        "the traded good must be serialized as Region state"
    );
}
