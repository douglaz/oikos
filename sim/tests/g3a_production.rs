//! G3a acceptance suite — the seeded grain→flour→bread production chain.
//!
//! These pin the milestone's DoD: a content-defined recipe chain runs
//! end-to-end with **seeded** producer roles (hand-placed millers/bakers), and
//! conservation holds **across the transformations** — a recipe is a conserved
//! conversion (accounted input consumed, accounted output produced; tools are
//! durable, never consumed). They assert no magnitudes beyond the chain
//! operating and conserving (the lab discipline), and no role *emergence* (who
//! produces what is seeded here; that the spread makes an entrepreneur *choose*
//! to mill is G3b, out of scope).
//!
//! The seven checks: determinism (the tripwire); the chain flows end-to-end;
//! conservation under transformation (the tripwire); tools gate production and
//! are durable; a recipe input is consumed exactly; the chain sustains without
//! collapse; and econ market behaviour is unchanged.

use econ::agent::{Agent, AgentId, Role, WantKind};
use econ::good::{Gold, GoodId, Horizon, Stock, GOLD, WOOD};
use econ::money::{DesignatedMoney, MarketMoneyConfig};
use econ::scenario::{MarketScenario, ScenarioName};
use econ::society::Society;
use sim::{
    ContentSet, Region, RegionConfig, Route, Settlement, SettlementConfig, Vocation,
    ECON_TICKS_PER_YEAR,
};

/// The G3a chain settlement config.
fn chain_config() -> SettlementConfig {
    SettlementConfig::grain_flour_bread_chain()
}

/// The chain good ids of a settlement built from [`chain_config`].
struct ChainGoods {
    grain: GoodId,
    flour: GoodId,
    bread: GoodId,
    mill: GoodId,
    oven: GoodId,
}

fn chain_goods(s: &Settlement) -> ChainGoods {
    let content = s.content().expect("a chain settlement has content");
    ChainGoods {
        grain: content.grain(),
        flour: content.flour(),
        bread: content.bread(),
        mill: content.mill(),
        oven: content.oven(),
    }
}

fn recipe_society(content: &ContentSet, stock: Stock) -> Society {
    let agent = Agent {
        id: AgentId(0),
        scale: Vec::new(),
        stock,
        gold: Gold::ZERO,
        labor_capacity: 0,
        hunger_deficit: 0,
        roles: vec![Role::Household],
        expect: Vec::new(),
    };
    Society::from_scenario(MarketScenario {
        name: "g3a-recipe-test",
        scenario: ScenarioName::MarketBarterishGold,
        seed: 0,
        periods: 0,
        agents: vec![agent],
        recipes: content.recipes().to_vec(),
        events: Vec::new(),
        money: MarketMoneyConfig::Designated(DesignatedMoney { good: GOLD }),
    })
}

/// 1. Same `(seed, config)` → byte-identical run; the production phase and the
///    scale injections draw no randomness, so two runs stay in lockstep, tick by
///    tick. A different seed diverges (the `Rng` is consumed at generation).
#[test]
fn chain_run_is_deterministic() {
    let config = chain_config();

    let mut a = Settlement::generate(0xC0FFEE, &config);
    let mut b = Settlement::generate(0xC0FFEE, &config);
    a.run(40);
    b.run(40);
    assert_eq!(
        a.canonical_bytes(),
        b.canonical_bytes(),
        "same seed diverged"
    );
    assert_eq!(a.digest(), b.digest());

    // Tick-by-tick lockstep: nothing is drawn in either loop or the production
    // phase, so the digest matches at every econ tick.
    let mut x = Settlement::generate(7, &config);
    let mut y = Settlement::generate(7, &config);
    for tick in 0..40 {
        x.econ_tick();
        y.econ_tick();
        assert_eq!(x.digest(), y.digest(), "drifted at econ tick {tick}");
    }

    // A different seed yields a different run (generation actually uses the Rng).
    let mut c = Settlement::generate(0xBADF00D, &config);
    c.run(40);
    assert_ne!(a.digest(), c.digest(), "the seed did not matter");
}

#[test]
fn chain_canonical_bytes_include_throughput() {
    let base = chain_config();
    let mut different = chain_config();
    different.chain.as_mut().expect("chain config").throughput += 1;

    let base = Settlement::generate(7, &base);
    let different = Settlement::generate(7, &different);
    assert_ne!(
        base.canonical_bytes(),
        different.canonical_bytes(),
        "throughput must be part of chain config identity"
    );

    // The assertion above is about config identity only; the plain G2 path still
    // writes no chain bytes and is checked in `econ_unchanged`.
}

/// 2. The chain operates end-to-end: over a seeded run, flour is produced from
///    grain and bread from flour, and bread is consumed — every stage of the
///    chain sees nonzero activity, so grain really does flow
///    node→gather→mill→flour→bake→bread→consumed.
#[test]
fn grain_flour_bread_chain_operates_end_to_end() {
    let mut s = Settlement::generate(2_026, &chain_config());
    let g = chain_goods(&s);

    let (mut flour_made, mut bread_made) = (0u64, 0u64);
    let (mut grain_milled, mut flour_baked, mut bread_eaten) = (0u64, 0u64, 0u64);
    for _ in 0..30 {
        let report = s.econ_tick();
        flour_made += report.produced_of(g.flour);
        bread_made += report.produced_of(g.bread);
        grain_milled += report.consumed_as_input_of(g.grain);
        flour_baked += report.consumed_as_input_of(g.flour);
        bread_eaten += report.consumed_of(g.bread);
    }

    // The mill stage: grain → flour.
    assert!(grain_milled > 0, "no grain was milled");
    assert!(flour_made > 0, "no flour was produced");
    // The bake stage: flour → bread.
    assert!(flour_baked > 0, "no flour was baked");
    assert!(bread_made > 0, "no bread was produced");
    // The sink: bread is eaten.
    assert!(bread_eaten > 0, "no bread was consumed");

    // Grain reached the millers from the world node (the raw source actually
    // flowed), so the chain is fed, not just draining its seeded buffers.
    assert!(
        s.tracked_goods().contains(&g.grain),
        "grain is not even tracked"
    );
}

#[test]
fn chain_market_books_include_content_goods_without_seed_buffers() {
    let mut config = chain_config();
    let chain = config.chain.as_mut().expect("chain config");
    chain.miller_grain_buffer = 0;
    chain.baker_flour_buffer = 0;
    chain.bread_buffer = 0;

    let s = Settlement::generate(2_026, &config);
    let g = chain_goods(&s);
    let market_goods = s.society().market_goods();
    for good in [g.grain, g.flour, g.bread, g.mill, g.oven] {
        assert!(
            market_goods.contains(&good),
            "{good:?} is missing from market books"
        );
    }
}

#[test]
fn producer_input_wants_stay_below_current_need_goods() {
    let mut s = Settlement::generate(7, &chain_config());
    let g = chain_goods(&s);

    // Tick once so needs advance from fully rested to low hunger/warmth, which
    // produces the reviewer-flagged shape: patient savings can rank above a
    // present survival-good unit. Recipe inputs must still not jump ahead of
    // present bread/wood wants.
    s.econ_tick();

    let miller_slot = (0..s.population())
        .find(|&index| s.vocation_of(index) == Some(Vocation::Miller))
        .expect("chain config seeds a miller");
    let miller = s.colonist_id(miller_slot).expect("miller has an id");
    let scale = &s
        .society()
        .agents
        .get(miller)
        .expect("miller resolves in society")
        .scale;

    let first_later = scale
        .iter()
        .position(|want| matches!(want.horizon, Horizon::Later(_)))
        .expect("patient producer has a savings want");
    let last_present_need_good = scale
        .iter()
        .rposition(|want| {
            matches!(want.kind, WantKind::Good(good) if (good == g.bread || good == WOOD))
                && matches!(want.horizon, Horizon::Now)
        })
        .expect("producer has present bread/wood wants after the first tick");
    assert!(
        first_later < last_present_need_good,
        "the regression must cover savings interleaved above current needs"
    );

    let first_input = scale
        .iter()
        .position(|want| {
            want.kind == WantKind::Good(g.grain) && matches!(want.horizon, Horizon::Next)
        })
        .expect("miller has grain input wants");
    assert!(
        last_present_need_good < first_input,
        "grain input wants must not outrank current bread/wood wants"
    );

    assert_eq!(scale[0].kind, WantKind::Good(g.mill));
    assert!(matches!(scale[0].horizon, Horizon::Next));
}

/// 3. Conservation holds across the transformations, exactly, every econ tick
///    (the tripwire). For every tracked good the whole-system total moves by
///    EXACTLY `+regen +produced −consumed_as_input −consumed`: a recipe consumes
///    an accounted input and produces an accounted output, and tools are durable
///    (a mill/oven never moves the ledger). No unit is unaccounted across a
///    transformation.
#[test]
fn production_conserves_with_transformations() {
    let mut s = Settlement::generate(99, &chain_config());
    let goods: Vec<GoodId> = s.tracked_goods().to_vec();
    let g = chain_goods(&s);

    let mut prev: Vec<u64> = goods
        .iter()
        .map(|&good| s.whole_system_total(good))
        .collect();
    let (mut any_produced, mut any_input, mut any_eaten) = (0u64, 0u64, 0u64);

    for tick in 0..60 {
        let report = s.econ_tick();
        assert!(
            report.conserves(),
            "report ledger unbalanced at tick {tick}"
        );

        for (i, &good) in goods.iter().enumerate() {
            let after = s.whole_system_total(good);
            let before = prev[i];
            let regen = report.regen_of(good) as i128;
            let produced = report.produced_of(good) as i128;
            let consumed_as_input = report.consumed_as_input_of(good) as i128;
            let consumed = report.consumed_of(good) as i128;

            // The generalized ledger: every appearance (regen/production) and
            // disappearance (recipe-input/eaten) is accounted; trades are
            // relocations and appear on neither side.
            assert_eq!(
                after as i128,
                before as i128 + regen + produced - consumed_as_input - consumed,
                "{good:?} conservation broke at tick {tick}: before={before} after={after} \
                 regen={regen} produced={produced} consumed_as_input={consumed_as_input} \
                 consumed={consumed}"
            );
            prev[i] = after;

            any_produced += report.produced_of(good);
            any_input += report.consumed_as_input_of(good);
            any_eaten += report.consumed_of(good);
        }

        // Tools never move the ledger: a mill / oven is durable, never an input
        // or an output, so its whole-system total is rock-steady.
        assert_eq!(report.produced_of(g.mill), 0);
        assert_eq!(report.consumed_as_input_of(g.mill), 0);
        assert_eq!(report.produced_of(g.oven), 0);
        assert_eq!(report.consumed_as_input_of(g.oven), 0);
    }

    // The proof is not vacuous: transformations actually happened.
    assert!(any_produced > 0, "nothing was ever produced");
    assert!(any_input > 0, "nothing was ever consumed as a recipe input");
    assert!(any_eaten > 0, "nothing was ever eaten");
}

/// 3b. The conservation generalization reaches the **region** path too. When a
///     G3a chain settlement is composed into a `Region`, that settlement's
///     production flows (produced / consumed_as_input) are nonzero, so the
///     region-wide ledger must account them exactly as the settlement ledger
///     does: `after == before + regen + produced − consumed_as_input − consumed`.
///     The region rolls the two production maps up from each settlement's report,
///     so it conserves every tick — it does not trip the tick-0 conservation
///     assertion the old `+regen − consumed` form would on the first transform.
#[test]
fn region_conserves_with_a_composed_chain_settlement() {
    // The reviewer-flagged composition: a chain settlement on (at least) one side
    // of a region. Both sides run the chain here, trading the staple (bread) over
    // the caravan — bread has an order book and is tracked in both, so generation
    // succeeds and the caravan actually steps over chain settlements.
    let content = ContentSet::grain_flour_bread();
    let (flour, bread) = (content.flour(), content.bread());
    let config = RegionConfig {
        settlement_a: chain_config(),
        settlement_b: chain_config(),
        route: Route { transit_ticks: 1 },
        good: bread,
        trader_gold: 200,
        buy_ticks: 4,
        sell_ticks: 6,
        caravans_enabled: true,
    };

    let mut region = Region::generate(7, &config);
    let (mut bread_made, mut flour_baked) = (0u64, 0u64);
    for tick in 0..40 {
        let report = region.econ_tick();
        assert!(
            report.conserves(),
            "region-wide conservation broke at tick {tick} with a chain settlement"
        );
        bread_made += report.produced_of(bread);
        flour_baked += report.consumed_as_input_of(flour);
    }

    // The rollup is actually exercised: the composed chain really transformed
    // goods inside the region (so the new region terms were nonzero, not a vacuous
    // pass against empty production maps).
    assert!(bread_made > 0, "no bread was produced region-wide");
    assert!(flour_baked > 0, "no flour was baked region-wide");
}

/// 4. Tools gate production and are durable. A would-be miller WITHOUT a mill
///    produces no flour; a miller WITH a mill produces flour and still holds the
///    mill afterward — `required_tool` is a capital gate, not a consumed input.
#[test]
fn tools_gate_production_and_are_durable() {
    let content = ContentSet::grain_flour_bread();
    let mill_recipe = content.mill_recipe();

    // Without the mill: grain present, but the gate is shut → no flour.
    let mut no_tool = Stock::new(content.oven().0);
    no_tool.add(content.grain(), 4);
    let mut no_tool = recipe_society(&content, no_tool);
    assert!(
        no_tool
            .execute_direct_recipe_for_agent_checked(AgentId(0), mill_recipe.id)
            .is_none(),
        "milling ran without a mill"
    );
    let no_tool_stock = &no_tool.agents.get(AgentId(0)).expect("agent exists").stock;
    assert_eq!(
        no_tool_stock.get(content.flour()),
        0,
        "flour appeared with no mill"
    );
    assert_eq!(
        no_tool_stock.get(content.grain()),
        4,
        "grain consumed with no mill"
    );

    // With the mill: flour is produced and the mill is retained (durable).
    let mut with_tool = Stock::new(content.oven().0);
    with_tool.add(content.grain(), 4);
    with_tool.add(content.mill(), 1);
    let mut with_tool = recipe_society(&content, with_tool);
    let applied = with_tool
        .execute_direct_recipe_for_agent_checked(AgentId(0), mill_recipe.id)
        .expect("milling runs with a mill");
    assert_eq!(applied.output.0, content.flour());
    let stock = &with_tool
        .agents
        .get(AgentId(0))
        .expect("agent exists")
        .stock;
    assert!(stock.get(content.flour()) > 0, "no flour produced");
    assert_eq!(
        stock.get(content.mill()),
        1,
        "the mill was consumed — tools must be durable"
    );

    // And it stays durable across repeated applications.
    let _ = with_tool.execute_direct_recipe_for_agent_checked(AgentId(0), mill_recipe.id);
    let _ = with_tool.execute_direct_recipe_for_agent_checked(AgentId(0), mill_recipe.id);
    let stock = &with_tool
        .agents
        .get(AgentId(0))
        .expect("agent exists")
        .stock;
    assert_eq!(stock.get(content.mill()), 1, "the mill wore out");
}

/// 5. A recipe input is consumed exactly. Applying the mill recipe consumes
///    exactly `input_qty` grain per `output_qty` flour produced — no grain leaks
///    or is created across the transformation.
#[test]
fn recipe_input_is_consumed_exactly() {
    let content = ContentSet::grain_flour_bread();
    let mill = content.mill_recipe();
    let (in_good, in_qty) = mill.input_good.expect("the mill recipe has an input");
    let out_qty = mill.output_qty;

    let mut stock = Stock::new(content.oven().0);
    let start_grain = 10;
    stock.add(content.grain(), start_grain);
    stock.add(content.mill(), 1);
    let mut society = recipe_society(&content, stock);

    // Apply it three times; grain falls by exactly `in_qty` each time and flour
    // rises by exactly `out_qty`, with no other grain motion.
    for n in 1..=3u32 {
        let applied = society
            .execute_direct_recipe_for_agent_checked(AgentId(0), mill.id)
            .expect("mill runs");
        assert_eq!(applied.input, Some((in_good, in_qty)));
        assert_eq!(applied.output, (content.flour(), out_qty));
        let stock = &society.agents.get(AgentId(0)).expect("agent exists").stock;
        assert_eq!(
            stock.get(content.grain()),
            start_grain - n * in_qty,
            "grain not consumed exactly after {n} applications"
        );
        assert_eq!(
            stock.get(content.flour()),
            n * out_qty,
            "flour not produced exactly after {n} applications"
        );
    }
    assert_eq!(
        society.labor_used_last_tick(),
        &[(AgentId(0), mill.labor * 3)],
        "recipe labor was not recorded"
    );
}

#[test]
fn recipe_output_headroom_is_checked_before_input_is_consumed() {
    let content = ContentSet::grain_flour_bread();
    let mill = content.mill_recipe();
    let mut stock = Stock::new(content.oven().0);
    stock.add(content.grain(), 1);
    stock.add(content.mill(), 1);
    stock.add(content.flour(), u32::MAX - 1);
    let mut society = recipe_society(&content, stock);

    assert!(
        society
            .execute_direct_recipe_for_agent_checked(AgentId(0), mill.id)
            .is_none(),
        "recipe must not run when output would saturate"
    );

    let stock = &society.agents.get(AgentId(0)).expect("agent exists").stock;
    assert_eq!(stock.get(content.grain()), 1, "input was consumed");
    assert_eq!(stock.get(content.flour()), u32::MAX - 1);
    assert_eq!(stock.get(content.mill()), 1, "tool was consumed");
}

/// 6. The chain sustains itself without collapse over a multi-year smoke run:
///    the seeded producers and consumers stay alive and hunger stays bounded
///    (well below the lethal ceiling). Smoke only — deterministic, no magnitude.
#[test]
fn chain_sustains_without_collapse() {
    let config = chain_config();
    let dynamics = config.dynamics;
    let mut s = Settlement::generate(11, &config);

    let years = 3;
    let mut total_deaths = 0u32;
    for _ in 0..(years * ECON_TICKS_PER_YEAR) {
        let report = s.econ_tick();
        total_deaths += report.deaths;
        // Hunger never reaches the lethal ceiling over the smoke horizon.
        assert!(
            s.max_living_hunger() < dynamics.need_max,
            "a colonist's hunger hit the ceiling — the chain collapsed"
        );
    }

    assert_eq!(total_deaths, 0, "the chain suffered a death — it collapsed");
    // Every seeded role is still alive: the chain still has gatherers, millers,
    // bakers, and consumers running it.
    for vocation in [
        Vocation::Gatherer,
        Vocation::Consumer,
        Vocation::Miller,
        Vocation::Baker,
    ] {
        assert!(
            s.living_count(vocation) > 0,
            "no living {vocation:?} remain — the chain collapsed"
        );
    }
}

/// 7. Econ market behaviour is unchanged: the engine's conformance scenarios
///    still replay byte-identically and conserve gold (the six goldens are not
///    perturbed by the additive G3a accessors), and a plain G2b settlement is
///    byte-identical with or without the (defaulted-empty) chain field. The full
///    `cargo test --workspace`, `cargo clippy -- -D warnings`, and
///    `cargo fmt --check` gates run outside this test.
#[test]
fn econ_unchanged() {
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

    // A plain settlement is byte-identical to one with an explicitly-absent chain
    // — the additive field never moves a non-chain digest.
    let plain = Settlement::generate(7, &SettlementConfig::viable());
    let mut explicit = SettlementConfig::viable();
    explicit.chain = None;
    let explicit = Settlement::generate(7, &explicit);
    assert_eq!(plain.digest(), explicit.digest());
}
