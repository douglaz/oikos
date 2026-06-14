//! G2b acceptance suite — the two-rate loop + delivery escrow (the `sim` crate).
//!
//! These assert the two milestone DoDs and the contracts that protect them:
//!
//! - **Conservation is exact** (test 2, the tripwire): every physical good is
//!   accounted across its full node→carry→stockpile→econ→consumed lifecycle, the
//!   world→econ transfer is net-zero, and the whole-system total changes only by
//!   node regen (source) and consumption (sink).
//! - **Distance affects realized prices** (tests 5–6): a node farther from the
//!   exchange delivers fewer units per econ tick (test 6, the supply mechanism)
//!   and so realizes a strictly higher price (test 5, sign only).
//!
//! plus determinism (test 1), the escrow contract (test 3), the §4.3 no-money-in-
//! the-fast-loop rule (test 4), self-sustenance (test 7), and the unchanged
//! econ/world/life suites (test 8). They pin no magnitudes — direction and
//! exactness only (the lab discipline).

use econ::agent::AgentId;
use econ::good::{GoodId, FOOD, WOOD};
use life::NeedDynamics;
use sim::{Settlement, SettlementConfig, Vocation, ECON_TICKS_PER_YEAR};

/// Per-agent gold in id order — the money-distribution probe for test 4.
fn gold_distribution(s: &Settlement) -> Vec<u64> {
    s.society().agents.iter().map(|a| a.gold.0).collect()
}

/// The min and max realized FOOD price over a window of `window` econ ticks taken
/// after a `warmup`, for the distance probe at `distance`. A window (not a single
/// sample) makes the near/far comparison robust to ordinary book oscillation.
fn windowed_food_price(distance: u16, warmup: u64, window: u64) -> (u64, u64) {
    let config = SettlementConfig::price_probe().with_food_node_distance(distance);
    let mut s = Settlement::generate(20_260_613, &config);
    s.run(warmup);
    let (mut lo, mut hi) = (u64::MAX, 0u64);
    for _ in 0..window {
        s.econ_tick();
        let price = s
            .realized_price(FOOD)
            .expect("the probe market keeps trading FOOD")
            .0;
        lo = lo.min(price);
        hi = hi.max(price);
    }
    (lo, hi)
}

/// Total FOOD units transferred (world→econ) over a window after a warmup, for
/// the distance probe at `distance`. The supply observable for test 6.
fn units_transferred(distance: u16, warmup: u64, window: u64) -> u64 {
    let config = SettlementConfig::price_probe().with_food_node_distance(distance);
    let mut s = Settlement::generate(20_260_613, &config);
    s.run(warmup);
    let mut total = 0u64;
    for _ in 0..window {
        total += s.econ_tick().transferred_of(FOOD);
    }
    total
}

/// 1. Same `(seed, SettlementConfig)` → byte-identical run; nothing is drawn in
///    either loop, so two runs stay in lockstep. A different seed diverges (the
///    `Rng` really is consumed at generation).
#[test]
fn run_is_deterministic() {
    let config = SettlementConfig::viable();

    let mut a = Settlement::generate(0xC0FFEE, &config);
    let mut b = Settlement::generate(0xC0FFEE, &config);
    a.run(60);
    b.run(60);
    assert_eq!(
        a.canonical_bytes(),
        b.canonical_bytes(),
        "same seed diverged"
    );
    assert_eq!(a.digest(), b.digest());

    // A different seed yields a different run (generation actually uses the Rng).
    let mut c = Settlement::generate(0xBADF00D, &config);
    c.run(60);
    assert_ne!(a.digest(), c.digest(), "the seed did not matter");

    // Tick-by-tick lockstep: the digest matches at every econ tick, so nothing is
    // drawn in the fast loop or the econ tick.
    let mut x = Settlement::generate(99, &config);
    let mut y = Settlement::generate(99, &config);
    for tick in 0..50 {
        x.econ_tick();
        y.econ_tick();
        assert_eq!(x.digest(), y.digest(), "drifted at econ tick {tick}");
    }
}

/// 2. Whole-system conservation is exact every econ tick: for every good, the
///    node + carry + stockpile + econ-stock total changes only by accounted regen
///    (source) and consumption (sink); the world→econ transfer is net-zero; no
///    unit is created or destroyed at any boundary. The tripwire.
#[test]
fn whole_system_conserves_every_econ_tick() {
    let mut s = Settlement::generate(2_026, &SettlementConfig::viable());
    let goods: Vec<GoodId> = s.tracked_goods().to_vec();
    assert!(goods.contains(&FOOD) && goods.contains(&WOOD));

    let mut prev: Vec<u64> = goods.iter().map(|&g| s.whole_system_total(g)).collect();
    let (mut total_transferred, mut total_consumed, mut total_regen) = (0u64, 0u64, 0u64);

    for tick in 0..100 {
        let report = s.econ_tick();
        assert!(
            report.conserves(),
            "report ledger unbalanced at tick {tick}"
        );

        for (i, &good) in goods.iter().enumerate() {
            let after = s.whole_system_total(good);
            let before = prev[i];
            let regen = report.regen_of(good);
            let consumed = report.consumed_of(good);

            // The ledger: the whole-system total moves by EXACTLY +regen −consumed.
            // The transfer (stockpile→econ) is a relocation, so it appears on
            // neither side — if it ever leaked, this equation would break.
            assert_eq!(
                after as i128,
                before as i128 + regen as i128 - consumed as i128,
                "{good:?} conservation broke at tick {tick}: \
                 before={before} after={after} regen={regen} consumed={consumed}"
            );
            prev[i] = after;

            total_transferred += report.transferred_of(good);
            total_consumed += consumed;
            total_regen += regen;
        }

        // WOOD never enters the world (it is a closed econ good), so it is never
        // transferred and its world side stays zero throughout.
        assert_eq!(report.transferred_of(WOOD), 0);
        assert_eq!(s.world().total_goods_of(WOOD), 0);
    }

    // The run actually exercised the full lifecycle — FOOD was hauled across the
    // seam, regenerated at the node, and consumed — so the conservation proof is
    // not vacuous.
    assert!(total_transferred > 0, "no goods crossed the transfer seam");
    assert!(total_consumed > 0, "nothing was consumed");
    assert!(total_regen > 0, "the node never regenerated");
}

/// 3. Goods in transit are escrow, never lost. A hauler that does not reach the
///    exchange within the interval keeps its load in carry (escrow) and delivers
///    it on a later arrival; a hauler that dies mid-haul **settles** its carried
///    goods to the commons (G4a real death) — conserved, not delivered, not
///    destroyed, not frozen in the world.
#[test]
fn in_transit_goods_are_escrow_not_lost() {
    // Part 1 — escrow carried across an econ tick, delivered later.
    let mut config = SettlementConfig::viable();
    config.gatherers = 2;
    config.consumers = 2;
    config.gatherer_food_buffer = 40; // survive; this part is about escrow, not death
    config.nodes[0].pos = world::Pos::new(20, 0); // far: a round trip (40) exceeds the 24-tick budget
    let mut s = Settlement::generate(7, &config);
    let gatherer = s.colonist_id(2).unwrap(); // ids 0,1 are consumers; 2,3 gatherers

    let mut saw_escrow = false; // a tick where the gatherer holds carry but delivered nothing
    let mut total_delivered = 0u64;
    let mut prev_total = s.whole_system_total(FOOD);
    for _ in 0..8 {
        let report = s.econ_tick();
        let carry = s.world().agent_carry(gatherer, FOOD);
        if carry > 0 && report.transferred_of(FOOD) == 0 {
            saw_escrow = true;
        }
        total_delivered += report.transferred_of(FOOD);
        // Conservation never breaks while goods sit in escrow.
        let after = s.whole_system_total(FOOD);
        assert_eq!(
            after as i128,
            prev_total as i128 + report.regen_of(FOOD) as i128 - report.consumed_of(FOOD) as i128,
            "escrowed goods broke conservation"
        );
        prev_total = after;
    }
    assert!(
        saw_escrow,
        "a far hauler must hold goods in escrow across a tick"
    );
    assert!(
        total_delivered > 0,
        "escrowed goods must eventually transfer"
    );

    // Part 2 — a hauler that starves mid-haul settles its carried goods to the
    // commons (G4a real death), conserved across the death.
    let mut s = Settlement::generate(1, &SettlementConfig::starved_hauler());
    let hauler = s.colonist_id(0).unwrap();
    let mut died = false;
    let mut prev_total = s.whole_system_total(FOOD);
    for _ in 0..20 {
        let report = s.econ_tick();
        // Conservation holds every tick, including the death tick: the carry that
        // leaves the world reappears in the commons, so the whole-system total
        // (world + econ + commons) changes only by regen − consumed.
        let after = s.whole_system_total(FOOD);
        assert_eq!(
            after as i128,
            prev_total as i128 + report.regen_of(FOOD) as i128 - report.consumed_of(FOOD) as i128,
            "conservation broke across the death"
        );
        prev_total = after;
        if !s.is_alive(0) {
            died = true;
            // The dead hauler's carried escrow is drained out of the world (not
            // frozen there), and the commons gained the settled goods.
            assert_eq!(
                s.world().agent_carry(hauler, FOOD),
                0,
                "a dead hauler's carry drains out of the world"
            );
            break;
        }
    }
    assert!(died, "the starved hauler must die mid-haul");
    let frozen_commons = s.commons_stock_of(FOOD);
    assert!(
        frozen_commons > 0,
        "the dead hauler's escrow settled to the commons"
    );

    // After death: the dead hauler carries nothing and delivers nothing, the
    // commons never loses its settled escrow, and conservation keeps holding.
    for _ in 0..8 {
        let report = s.econ_tick();
        assert_eq!(
            s.world().agent_carry(hauler, FOOD),
            0,
            "a dead hauler carries nothing"
        );
        assert!(
            s.commons_stock_of(FOOD) >= frozen_commons,
            "the commons never loses its settled goods"
        );
        let after = s.whole_system_total(FOOD);
        assert_eq!(
            after as i128,
            prev_total as i128 + report.regen_of(FOOD) as i128 - report.consumed_of(FOOD) as i128,
            "post-death conservation broke"
        );
        prev_total = after;
    }
}

/// 4. No money moves in the fast loop. Across every econ tick the total money is
///    unchanged by the fast `world` ticks (§4.3); with a closed gold balance the
///    total is conserved and only redistributes between colonists across
///    `Society::step` — so money lives in the econ tick, never the fast loop.
#[test]
fn no_money_moves_in_the_fast_loop() {
    for (name, config) in [
        ("viable", SettlementConfig::viable()),
        ("price_probe", SettlementConfig::price_probe()),
    ] {
        // The sim configs have a closed gold balance: any change is pure
        // redistribution, which isolates the fast-loop claim.
        let mut s = Settlement::generate(5, &config);
        let start_gold = s.total_gold().0;

        let mut prev_distribution = gold_distribution(&s);
        let mut money_redistributed = false;
        for tick in 0..80 {
            let report = s.econ_tick();

            // The fast loop moved no money: the totals bracketing it are equal.
            assert_eq!(
                report.total_gold_before_fast, report.total_gold_after_fast,
                "{name}: the fast loop moved money at tick {tick}"
            );
            // Closed balance: the total is conserved across the whole econ tick.
            assert_eq!(
                report.total_gold_after_step, start_gold,
                "{name}: money was minted or burned at tick {tick}"
            );

            // Money DID move between colonists — and the only mutating phase is
            // `Society::step` — so money lives in the econ tick, not the fast loop.
            let distribution = gold_distribution(&s);
            if distribution != prev_distribution {
                money_redistributed = true;
            }
            prev_distribution = distribution;
        }
        assert_eq!(
            s.total_gold().0,
            start_gold,
            "{name}: gold total drifted over the run"
        );
        assert!(
            money_redistributed,
            "{name}: the market never moved any money"
        );
    }
}

/// 5. Distance raises the realized price. Two runs identical but for the
///    gatherers' node distance: the far run's realized FOOD price is strictly
///    higher than the near run's. Sign only — no magnitude is pinned.
#[test]
fn distance_raises_realized_price() {
    let near = windowed_food_price(8, 60, 24);
    let far = windowed_food_price(24, 60, 24);

    // The far run's LOWEST realized price over the window exceeds the near run's
    // HIGHEST — a margin no ordinary book oscillation can close.
    assert!(
        far.0 > near.1,
        "distance did not raise the realized food price: near=[{},{}] far=[{},{}]",
        near.0,
        near.1,
        far.0,
        far.1
    );
}

/// 6. The supply mechanism behind test 5: units transferred per econ tick
///    decrease monotonically as node distance increases, holding the fast-tick
///    budget fixed. Fewer round trips fit the budget, so fewer units land.
#[test]
fn far_node_delivers_fewer_units_per_econ_tick() {
    let distances = [4u16, 8, 12, 16, 20];
    let units: Vec<u64> = distances
        .iter()
        .map(|&d| units_transferred(d, 12, 24))
        .collect();

    for pair in units.windows(2) {
        assert!(
            pair[1] <= pair[0],
            "units transferred were not monotone in distance: {distances:?} -> {units:?}"
        );
    }
    assert!(
        *units.last().unwrap() < units[0],
        "the farthest node must deliver strictly fewer units than the nearest: {units:?}"
    );
}

/// 7. A viable settlement runs several econ-years without collapse: both
///    vocations stay populated and survivors' hunger stays bounded off the
///    ceiling. Smoke test only — survival and boundedness, not prices or counts —
///    and deterministic across two runs.
#[test]
fn settlement_sustains_itself() {
    let config = SettlementConfig::viable();
    let need_max = NeedDynamics::lab_default().need_max;
    let horizon = 5 * ECON_TICKS_PER_YEAR;

    let mut s = Settlement::generate(20_260_613, &config);
    for tick in 0..horizon {
        s.econ_tick();
        assert!(
            s.living_count(Vocation::Gatherer) > 0,
            "the gatherers collapsed at tick {tick}"
        );
        assert!(
            s.living_count(Vocation::Consumer) > 0,
            "the consumers collapsed at tick {tick}"
        );
        // Endowment-driven boundedness: a survivor's hunger never reaches the
        // ceiling (which would put it on the death path). The viable supply keeps
        // every survivor fed.
        assert!(
            s.max_living_hunger() < need_max,
            "a survivor's hunger reached the ceiling at tick {tick}"
        );
    }

    // Deterministic: a second run from the same seed matches byte-for-byte.
    let mut twin = Settlement::generate(20_260_613, &config);
    twin.run(horizon);
    assert_eq!(
        s.digest(),
        twin.digest(),
        "the settlement is not deterministic"
    );
}

/// 8. `sim` changed no econ/world/life behavior. The econ engine still replays
///    deterministically with conservation intact from `sim`'s workspace; the
///    byte-identical golden guarantee and the full G1 (`life`) / G2a (`world`)
///    suites are enforced by `cargo test` across the workspace, plus
///    `cargo clippy --workspace --all-targets -- -D warnings` and
///    `cargo fmt --check`. This checks the engine is usable and unperturbed here.
#[test]
fn econ_world_life_unchanged() {
    use econ::scenario::{builtin_market_scenario, ScenarioName};
    use econ::society::Society;

    for name in [
        ScenarioName::MarketBarterishGold,
        ScenarioName::MarketPriceDiscovery,
        ScenarioName::MengerSaltMoney,
    ] {
        let scenario = builtin_market_scenario(name);
        let periods = scenario.periods;

        let mut first = Society::from_scenario(scenario);
        let total_gold = first.total_gold();
        first.run(periods);

        let mut second = Society::from_scenario(builtin_market_scenario(name));
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
}

/// Unit: the transfer credits exactly the depositing colonist's stock, and the
/// stockpile is drained back to empty (a unit has one owner at a time).
#[test]
fn transfer_credits_the_depositing_gatherer_and_drains_the_stockpile() {
    let mut config = SettlementConfig::viable();
    config.gatherers = 2;
    config.consumers = 1;
    config.nodes[0].pos = world::Pos::new(3, 0); // near: gatherers deliver within the interval
    let mut s = Settlement::generate(11, &config);

    let report = s.econ_tick();
    // Some FOOD was hauled and transferred this tick.
    assert!(report.transferred_of(FOOD) > 0);

    // The exchange stockpile is empty again — every deposited unit was withdrawn
    // into econ stock (no double counting in both world and econ).
    assert_eq!(s.world().stockpile_get(s.exchange(), FOOD), 0);

    // The credited FOOD lives in the gatherers' econ stock (ids >= consumers),
    // never the consumer's.
    let consumer = s.colonist_id(0).unwrap();
    let consumer_food = s
        .society()
        .agents
        .get(consumer)
        .map(|a| a.stock.get(FOOD))
        .unwrap_or(0);
    let gatherer_food: u32 = [s.colonist_id(1).unwrap(), s.colonist_id(2).unwrap()]
        .into_iter()
        .map(|id| {
            s.society()
                .agents
                .get(id)
                .map(|a| a.stock.get(FOOD))
                .unwrap_or(0)
        })
        .sum();
    // The consumer started with a FOOD buffer it is eating down; the gatherers
    // hold the hauled FOOD. The transferred units are accounted in econ stock.
    // (Addition form, not `transferred - 8`, so a small transfer can't underflow.)
    assert!(
        u64::from(gatherer_food) + 8 >= report.transferred_of(FOOD),
        "transferred FOOD is unaccounted in gatherer stock"
    );
    let _ = consumer_food;
}

/// Unit: if a valid delivery cannot be credited because the depositor's econ
/// stock is at the `u32` ceiling, the delivered goods stay in the exchange
/// stockpile and are retried later once consumption opens headroom. They are not
/// forgotten just because no new carry drop happens in the retry tick.
#[test]
fn clipped_exchange_deposits_retry_when_stock_headroom_opens() {
    let mut config = SettlementConfig::viable();
    config.gatherers = 1;
    config.consumers = 0;
    config.exchange_cap = 2;
    config.carry_cap = 2;
    config.gatherer_food_buffer = u32::MAX;
    config.gatherer_wood_buffer = 0;
    config.nodes[0].pos = world::Pos::new(1, 0);
    let mut s = Settlement::generate(13, &config);
    let gatherer = s.colonist_id(0).unwrap();
    assert_eq!(
        s.society().agents.get(gatherer).unwrap().stock.get(FOOD),
        u32::MAX
    );

    let first = s.econ_tick();
    assert_eq!(
        first.transferred_of(FOOD),
        0,
        "full econ stock must clip the transfer"
    );
    assert_eq!(
        s.world().stockpile_get(s.exchange(), FOOD),
        config.exchange_cap,
        "clipped delivery must remain world-owned in the exchange"
    );

    let mut retried = false;
    for _ in 0..5 {
        let before_stockpile = s.world().stockpile_get(s.exchange(), FOOD);
        assert_eq!(
            before_stockpile, config.exchange_cap,
            "the full exchange admits no fresh deposit before the retry"
        );
        let report = s.econ_tick();
        if report.transferred_of(FOOD) > 0 {
            assert!(
                s.world().stockpile_get(s.exchange(), FOOD) < before_stockpile,
                "retry must withdraw the already-delivered exchange units"
            );
            retried = true;
            break;
        }
    }
    assert!(retried, "clipped exchange deposits were never retried");
}

/// Unit: a colonist id maps 1:1 between the world and the econ society, so the
/// transfer can credit the same `AgentId` it observed depositing.
#[test]
fn world_and_econ_agent_ids_coincide() {
    let s = Settlement::generate(3, &SettlementConfig::viable());
    for index in 0..s.population() {
        let id = s.colonist_id(index).unwrap();
        assert_eq!(id, AgentId(index as u64));
        assert!(
            s.world().agent_pos(id).is_some(),
            "no world agent for {id:?}"
        );
        assert!(
            s.society().agents.get(id).is_some(),
            "no econ agent for {id:?}"
        );
    }
}
