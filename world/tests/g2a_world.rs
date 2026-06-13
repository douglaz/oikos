//! G2a acceptance suite — the `world` crate (spatial substrate).
//!
//! These assert the spatial substrate's *properties*: deterministic generation
//! and ticking, deterministic BFS movement around obstacles, and the
//! conservation/capacity ledger invariants. They reach for **no** economic
//! concept — no prices, money, wants, or trades — because those are G2b, not
//! here (the milestone boundary in `docs/impl-g2a.md`).
//!
//! Determinism (tests 1–2) and conservation (tests 6–8) are the two contracts;
//! pathfinding (tests 3–5) and the distance accessor (test 10) are the genuine
//! mechanics G2b will build on.

use world::grid::{Grid, Pos};
use world::node::{NodeId, ResourceNode};
use world::path::shortest_path;
use world::stockpile::{Stockpile, StockpileId};
use world::world::{AgentStatus, PlacementError, Task, World, WorldGen};
use world::{AgentId, GoodId};

use econ::good::{FOOD, WOOD};

/// Drive one agent (speed 1) to completion, recording its position after each
/// tick. Returns the visited tiles (excluding the start), capped to avoid a
/// runaway loop on a non-terminating layout.
fn walk_path(world: &mut World, agent: AgentId, max_ticks: usize) -> Vec<Pos> {
    let mut visited = Vec::new();
    for _ in 0..max_ticks {
        let before = world.agent_pos(agent).unwrap();
        world.tick();
        let after = world.agent_pos(agent).unwrap();
        if after != before {
            visited.push(after);
        }
        // Stop once the agent is idle (arrived) or stuck (blocked).
        if matches!(
            world.agent_status(agent),
            Some(AgentStatus::Idle) | Some(AgentStatus::Blocked)
        ) {
            break;
        }
    }
    visited
}

/// 1. Same `(seed, WorldGen)` → byte-identical world (two independent runs).
#[test]
fn world_generation_is_deterministic() {
    let gen = WorldGen::demo();
    let a = World::generate(0xC0FFEE, &gen);
    let b = World::generate(0xC0FFEE, &gen);
    assert_eq!(
        a.canonical_bytes(),
        b.canonical_bytes(),
        "same seed produced different worlds"
    );
    assert_eq!(a.digest(), b.digest());

    // A different seed produces a different world (the generation actually uses
    // the RNG — this is not a constant).
    let c = World::generate(0xBADF00D, &gen);
    assert_ne!(a.canonical_bytes(), c.canonical_bytes());
}

/// 2. Same world + same task assignments → identical state after N ticks across
///    two runs. The tick loop draws no RNG, so this is byte-exact.
#[test]
fn tick_is_deterministic() {
    fn scripted_run(seed: u64) -> u64 {
        let mut world = World::generate(seed, &WorldGen::demo());
        let agents = world.agent_ids();
        // Phase A: every agent harvests from node 0.
        for &agent in &agents {
            world.assign_task(agent, Task::GoHarvest(NodeId(0), 5));
        }
        world.run(20);
        // Phase B: every agent deposits into stockpile 0.
        for &agent in &agents {
            world.assign_task(agent, Task::GoDeposit(StockpileId(0)));
        }
        world.run(20);
        world.digest()
    }

    assert_eq!(scripted_run(2026), scripted_run(2026));
}

/// 3. An agent reaches a reachable target in exactly the expected step count for
///    a known open-grid layout (the count is pinned).
#[test]
fn movement_reaches_reachable_targets() {
    // GoTo: distance 5 on an open row, speed 1 → exactly 5 ticks.
    let mut world = World::new(Grid::new(8, 1));
    let agent = world.add_agent(Pos::new(0, 0), 10, 1).unwrap();
    world.assign_task(agent, Task::GoTo(Pos::new(5, 0)));
    for _ in 0..4 {
        world.tick();
    }
    assert_ne!(
        world.agent_pos(agent),
        Some(Pos::new(5, 0)),
        "arrived too early"
    );
    world.tick(); // 5th tick
    assert_eq!(world.agent_pos(agent), Some(Pos::new(5, 0)));
    assert_eq!(world.agent_status(agent), Some(AgentStatus::Idle));

    // GoHarvest: distance 3, speed 1 → arrives and harvests on the same (3rd)
    // tick per the pinned same-tick arrival rule.
    let mut world = World::new(Grid::new(6, 6));
    let agent = world.add_agent(Pos::new(0, 0), 10, 1).unwrap();
    let node = world
        .add_node(ResourceNode::new(Pos::new(3, 0), FOOD, 10, 0, 10))
        .unwrap();
    world.assign_task(agent, Task::GoHarvest(node, 4));
    world.tick();
    world.tick();
    assert_eq!(
        world.agent_carry(agent, FOOD),
        0,
        "harvested before arrival"
    );
    let report = world.tick(); // 3rd tick: arrival + harvest
    assert_eq!(world.agent_pos(agent), Some(Pos::new(3, 0)));
    assert_eq!(world.agent_carry(agent, FOOD), 4);
    assert_eq!(world.node(node).unwrap().stock, 6);
    assert_eq!(report.harvested, 4);
}

/// 4. With an obstacle wall and a gap, the agent routes through the gap; the path
///    is the fixed-tie-break shortest path and is reproducible.
#[test]
fn pathfinding_avoids_impassable_and_is_deterministic() {
    // 5x5; vertical wall at x=2 for rows 0..=3, leaving (2,4) as the only gap.
    let mut grid = Grid::new(5, 5);
    for y in 0..4 {
        grid.set_impassable(Pos::new(2, y));
    }
    let start = Pos::new(0, 0);
    let goal = Pos::new(4, 0);
    let expected = shortest_path(&grid, start, goal).expect("reachable via the gap");
    assert_eq!(
        expected.len(),
        12,
        "shortest detour through the gap is 12 steps"
    );
    assert!(expected.contains(&Pos::new(2, 4)), "must cross the gap");

    let run = |grid: &Grid| {
        let mut world = World::new(grid.clone());
        let agent = world.add_agent(start, 10, 1).unwrap();
        world.assign_task(agent, Task::GoTo(goal));
        walk_path(&mut world, agent, 50)
    };

    let taken = run(&grid);
    // The agent walks exactly the fixed-tie-break shortest path (speed 1 → one
    // tile per tick), never steps onto an impassable tile, and arrives.
    assert_eq!(
        taken, expected,
        "agent did not follow the BFS shortest path"
    );
    for tile in &taken {
        assert!(
            grid.is_passable(*tile),
            "agent stepped onto impassable {tile:?}"
        );
    }
    assert_eq!(*taken.last().unwrap(), goal);
    // Reproducible: a second identical run produces the same walk.
    assert_eq!(run(&grid), taken);
}

/// 5. A fully walled-off target leaves the agent put with `Blocked`,
///    deterministically, and never panics.
#[test]
fn unreachable_target_blocks_without_panic() {
    // (2,2) is passable but enclosed by impassable tiles on all four sides.
    let mut grid = Grid::new(5, 5);
    for wall in [
        Pos::new(1, 2),
        Pos::new(3, 2),
        Pos::new(2, 1),
        Pos::new(2, 3),
    ] {
        grid.set_impassable(wall);
    }

    // A GoTo into the pocket: accepted (in bounds) but never reachable.
    let mut world = World::new(grid.clone());
    let agent = world.add_agent(Pos::new(0, 0), 10, 1).unwrap();
    assert!(world.assign_task(agent, Task::GoTo(Pos::new(2, 2))));
    for _ in 0..10 {
        world.tick();
        assert_eq!(
            world.agent_pos(agent),
            Some(Pos::new(0, 0)),
            "blocked agent moved"
        );
        assert_eq!(world.agent_status(agent), Some(AgentStatus::Blocked));
    }

    // A GoHarvest of a node parked in the pocket blocks too, and harvests nothing.
    let mut world = World::new(grid);
    let agent = world.add_agent(Pos::new(0, 0), 10, 1).unwrap();
    let node = world
        .add_node(ResourceNode::new(Pos::new(2, 2), FOOD, 9, 0, 9))
        .unwrap();
    world.assign_task(agent, Task::GoHarvest(node, 5));
    let report = world.run(10);
    assert!(world.agent_blocked(agent));
    assert_eq!(world.agent_carry(agent, FOOD), 0);
    assert_eq!(
        world.node(node).unwrap().stock,
        9,
        "blocked harvest moved goods"
    );
    assert_eq!(report.harvested, 0);
}

/// 6. Harvesting moves units node→carry exactly: node stock down by the amount,
///    carry up by the amount, and the conservation total is unchanged by harvest
///    alone (regen set to 0 so nothing else moves the total).
#[test]
fn harvest_conserves_goods() {
    let mut world = World::new(Grid::new(4, 1));
    let agent = world.add_agent(Pos::new(0, 0), 10, 4).unwrap();
    let node = world
        .add_node(ResourceNode::new(Pos::new(2, 0), FOOD, 7, 0, 7))
        .unwrap();
    let total_before = world.total_goods();
    let food_before = world.total_goods_of(FOOD);
    assert_eq!(total_before, 7);

    world.assign_task(agent, Task::GoHarvest(node, 3));
    let report = world.tick();

    assert_eq!(report.harvested, 3);
    assert_eq!(world.node(node).unwrap().stock, 4, "node down by exactly 3");
    assert_eq!(world.agent_carry(agent, FOOD), 3, "carry up by exactly 3");
    assert_eq!(
        world.total_goods(),
        total_before,
        "harvest changed the total"
    );
    assert_eq!(world.total_goods_of(FOOD), food_before);
    assert_eq!(report.net, 0, "no regen → no net change");
}

/// 7. Depositing into a stockpile clamps to capacity; overflow stays carried;
///    nothing is destroyed; the totals balance.
#[test]
fn deposit_respects_capacity_and_conserves() {
    let mut world = World::new(Grid::new(3, 1));
    let agent = world.add_agent(Pos::new(0, 0), 10, 5).unwrap();
    let node = world
        .add_node(ResourceNode::new(Pos::new(1, 0), FOOD, 7, 0, 7))
        .unwrap();
    let sp = world
        .add_stockpile(Stockpile::new(Pos::new(2, 0), 4))
        .unwrap();
    let total_before = world.total_goods();

    world.assign_task(agent, Task::GoHarvest(node, 7));
    world.tick();
    assert_eq!(world.agent_carry(agent, FOOD), 7);

    world.assign_task(agent, Task::GoDeposit(sp));
    let report = world.tick();

    assert_eq!(report.deposited, 4, "only the capacity was accepted");
    assert_eq!(world.stockpile_get(sp, FOOD), 4);
    assert_eq!(world.agent_carry(agent, FOOD), 3, "overflow stays carried");
    // Capacity full: a second deposit accepts nothing, destroys nothing.
    world.assign_task(agent, Task::GoDeposit(sp));
    let report = world.tick();
    assert_eq!(report.deposited, 0);
    assert_eq!(world.agent_carry(agent, FOOD), 3);
    assert_eq!(
        world.total_goods(),
        total_before,
        "deposit changed the total"
    );
}

/// 8. Node regen is the sole creator of goods, clamped to `cap`, and the
///    per-tick report's `regenerated` exactly equals the increase. With
///    `regen_per_tick = 0` the world good total is invariant across ticks even
///    while agents harvest and deposit.
#[test]
fn regen_is_the_only_source_and_is_accounted() {
    // (a) A regen node: every tick, report.regenerated == the stock increase,
    //     and the world total rises by exactly that.
    let mut world = World::new(Grid::new(2, 1));
    let node = world
        .add_node(ResourceNode::new(Pos::new(0, 0), WOOD, 0, 3, 7))
        .unwrap();
    for _ in 0..5 {
        let before_stock = world.node(node).unwrap().stock;
        let before_total = world.total_goods();
        let report = world.tick();
        let after_stock = world.node(node).unwrap().stock;
        let increase = u64::from(after_stock - before_stock);
        assert_eq!(
            report.regenerated, increase,
            "regen report != stock increase"
        );
        assert_eq!(report.net, increase as i64);
        assert_eq!(world.total_goods(), before_total + increase);
        assert!(after_stock <= 7, "regen overshot the cap");
    }
    assert_eq!(
        world.node(node).unwrap().stock,
        7,
        "regen settles at the cap"
    );

    // (b) A regen-free world: movement and hauling never change the total.
    let mut world = World::new(Grid::new(5, 1));
    let agent = world.add_agent(Pos::new(0, 0), 10, 2).unwrap();
    let node = world
        .add_node(ResourceNode::new(Pos::new(2, 0), FOOD, 8, 0, 8))
        .unwrap();
    let sp = world
        .add_stockpile(Stockpile::new(Pos::new(4, 0), 100))
        .unwrap();
    let invariant = world.total_goods();
    world.assign_task(agent, Task::GoHarvest(node, 8));
    for tick in 0..12 {
        world.tick();
        if tick == 5 {
            world.assign_task(agent, Task::GoDeposit(sp));
        }
        assert_eq!(world.total_goods(), invariant, "regen-free total drifted");
        assert_eq!(world.last_report().regenerated, 0);
    }
}

/// 9. Constructing a node/stockpile/agent on an impassable tile (or off the
///    grid) is rejected with the right reason — never silently accepted.
#[test]
fn placement_on_impassable_is_rejected() {
    let mut grid = Grid::new(4, 4);
    grid.set_impassable(Pos::new(2, 2));
    let mut world = World::new(grid);

    assert_eq!(
        world.add_node(ResourceNode::new(Pos::new(2, 2), FOOD, 5, 0, 5)),
        Err(PlacementError::Impassable)
    );
    assert_eq!(
        world.add_stockpile(Stockpile::new(Pos::new(2, 2), 10)),
        Err(PlacementError::Impassable)
    );
    assert_eq!(
        world.add_agent(Pos::new(2, 2), 10, 1),
        Err(PlacementError::Impassable)
    );
    assert_eq!(
        world.add_node(ResourceNode::new(Pos::new(9, 9), FOOD, 5, 0, 5)),
        Err(PlacementError::OutOfBounds)
    );
    // A passable tile is accepted, and nothing was added on the rejected paths.
    assert!(world
        .add_node(ResourceNode::new(Pos::new(0, 0), FOOD, 5, 0, 5))
        .is_ok());
    assert_eq!(world.node_count(), 1);
    assert_eq!(world.stockpile_count(), 0);
}

/// 10. The travel/distance accessor grows monotonically with grid separation on
///     an open grid (the property G2b leans on for "distance affects price").
#[test]
fn distance_estimate_is_monotonic() {
    let world = World::new(Grid::new(12, 12));
    let origin = Pos::new(0, 0);

    // Sweep every tile; stepping one tile farther east (Manhattan +1) must
    // raise the travel estimate by exactly one (on an open grid the two
    // coincide), so the estimate is strictly monotone in separation.
    for y in 0..12u16 {
        for x in 0..11u16 {
            let near = Pos::new(x, y);
            let far = Pos::new(x + 1, y);
            let dn = world.grid_distance(origin, near);
            let df = world.grid_distance(origin, far);
            let tn = world.travel_estimate(origin, near).unwrap();
            let tf = world.travel_estimate(origin, far).unwrap();
            assert_eq!(tn, dn, "open-grid travel must equal Manhattan");
            assert_eq!(tf, df);
            assert_eq!(df, dn + 1);
            assert!(tn < tf, "travel estimate not monotone: {near:?} -> {far:?}");
        }
    }

    // A direct monotone chain along the diagonal.
    let mut last = 0u32;
    for step in 0..12u16 {
        let p = Pos::new(step, step);
        let est = world.travel_estimate(origin, p).unwrap();
        if step > 0 {
            assert!(est > last, "diagonal travel estimate must strictly grow");
        }
        last = est;
    }
}

/// 11. The econ engine still runs and replays deterministically from `world`'s
///     workspace, with conservation intact — `world` added no econ behavior
///     change (it does not depend on econ economic logic, and econ does not
///     depend on `world`). The byte-identical golden guarantee and the full G1
///     `life` suite are enforced by `cargo test` across the workspace, plus
///     `cargo clippy --workspace --all-targets -- -D warnings` and
///     `cargo fmt --check`; this checks the engine is usable and unperturbed
///     from here.
#[test]
fn econ_and_life_unchanged() {
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

    // The shared primitives `world` re-exports really are econ's types.
    let _: GoodId = FOOD;
    assert_eq!(World::new(Grid::new(1, 1)).total_goods(), 0);
}
