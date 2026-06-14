//! G4a acceptance suite — real death: arena free, estate, cache reconciliation.
//!
//! Every milestone since G0b deferred ONE piece: actually removing an agent from a
//! running `Society`. G1 tombstoned the dead (froze them in place); G4a lands the
//! engine-integration core of demography. When a colonist starves it is removed for
//! real — its **estate settles** to the settlement commons (a conserved sink the sim
//! owns), its **arena slot is freed**, and every external `Society` cache that
//! referenced it **reconciles** — with whole-system conservation preserved and the
//! engine continuing correctly afterward.
//!
//! These pin the contract and its tripwires:
//! - the freed slot is reusable (test 1);
//! - the estate (gold + econ stock + world-carried escrow) settles to the commons,
//!   conserving (test 2);
//! - no cache dangles a reference to a freed/relocated slot (test 3);
//! - the dead colonist never trades again and is gone from activation (test 4);
//! - the reconciliation is deterministic (test 5);
//! - a death does not corrupt survivors (test 6);
//! - the no-death path stays byte-identical (test 7);
//! - the engine is otherwise unperturbed (test 8).
//!
//! Scope is real death ONLY: no births, aging, households, or culture inheritance
//! (those are G4b — the estate settles to the commons, not to heirs).

use econ::agent::{Agent, AgentId, Role};
use econ::good::{Gold, GoodId, Stock, FOOD, GOLD, WOOD};
use sim::{NodeSpec, Settlement, SettlementConfig};
use world::Pos;

/// The viable no-death digest (60 ticks, seed `0xC0FFEE`) — the byte-identical
/// tripwire for test 7. A no-death run NEVER frees an agent, so it must reproduce
/// the pre-G4a (tombstone-era) bytes exactly. If this constant moves, the free +
/// reconciliation machinery leaked into the no-free hot path; fix that, do not
/// re-pin (see `docs/impl-g4a.md`).
const VIABLE_NO_DEATH_DIGEST: u64 = 0xa174_8567_db1c_4341;

/// A marginal-supply settlement that produces a **partial die-off with survivors**:
/// only two gatherers feed six consumers from a far, slow node, so the consumers
/// starve while the gatherers (eating their own harvest) survive and keep the FOOD
/// market trading. A single FOOD node and a closed gold balance (inherited from
/// `viable`) keep conservation exact.
fn dieoff_config() -> SettlementConfig {
    let mut cfg = SettlementConfig::viable();
    cfg.gatherers = 2;
    cfg.consumers = 6;
    cfg.consumer_food_buffer = 3;
    cfg.nodes = vec![NodeSpec {
        good: FOOD,
        pos: Pos::new(10, 0),
        stock: 4_000,
        regen: 4,
        cap: 4_000,
    }];
    cfg
}

/// A minimal econ agent for probing arena slot reuse (test 1).
fn fresh_agent() -> Agent {
    Agent {
        id: AgentId(0),
        scale: Vec::new(),
        stock: Stock::new(WOOD.0),
        gold: Gold::ZERO,
        labor_capacity: 0,
        hunger_deficit: 0,
        roles: vec![Role::Household],
        expect: Vec::new(),
    }
}

/// Step `s` until the first econ tick that records a death, returning that tick
/// index, asserting whole-system conservation at every tick along the way. Returns
/// `None` if no death occurred within `max_ticks`.
fn run_to_first_death(s: &mut Settlement, max_ticks: u64) -> Option<u64> {
    for t in 0..max_ticks {
        let report = s.econ_tick();
        assert!(report.conserves(), "conservation broke at econ tick {t}");
        if report.deaths > 0 {
            return Some(t);
        }
    }
    None
}

/// 1. A starving colonist dies; its `AgentId` resolves `None` after, its arena slot
///    is freed, and the slot is reusable — a subsequent insert reuses it with a
///    bumped generation, so the stale id stays `None`.
#[test]
fn death_frees_the_arena_slot() {
    let mut s = Settlement::generate(1, &SettlementConfig::starved_hauler());
    let hauler = s.colonist_id(0).unwrap();
    assert!(
        s.society().agents.get(hauler).is_some(),
        "the hauler resolves while alive"
    );

    assert!(
        run_to_first_death(&mut s, 40).is_some(),
        "the starved hauler must die"
    );
    assert!(!s.is_alive(0), "the hauler is marked dead");

    // Real removal: the dead id no longer resolves in the arena.
    assert!(
        s.society().agents.get(hauler).is_none(),
        "the dead colonist's id resolves to None"
    );
    assert_eq!(
        s.world().agent_status(hauler),
        None,
        "the dead colonist's spatial agent is removed"
    );

    // The slot is reusable: a fresh insert takes the freed numeric index with a
    // bumped generation, and the stale id stays None even after reuse.
    let reused = s.society_mut().agents.insert(fresh_agent());
    assert_eq!(
        reused.index(),
        hauler.index(),
        "the freed numeric slot is reused"
    );
    assert!(
        reused.generation() > hauler.generation(),
        "reuse bumps the slot generation"
    );
    assert!(
        s.society().agents.get(hauler).is_none(),
        "the stale id stays None after the slot is reused"
    );
    assert!(
        s.society().agents.get(reused).is_some(),
        "the reused id resolves to the new agent"
    );
}

/// 2. The dead colonist's gold + econ stock and any world-carried escrow move to the
///    commons exactly; whole-system conservation holds across the death (nothing
///    created or destroyed).
#[test]
fn estate_settles_to_commons_conserving() {
    let mut s = Settlement::generate(1, &SettlementConfig::starved_hauler());
    let goods: Vec<GoodId> = s.tracked_goods().to_vec();

    // The commons starts empty (no death has occurred).
    assert_eq!(s.commons_gold(), Gold::ZERO, "commons gold starts empty");
    for &g in &goods {
        assert_eq!(s.commons_stock_of(g), 0, "commons stock starts empty");
    }
    let gold_before = s.total_gold();

    // Run through the death; conservation holds at every tick (the per-good
    // whole-system ledger, including the commons, balances — `run_to_first_death`
    // asserts `report.conserves()`).
    assert!(
        run_to_first_death(&mut s, 40).is_some(),
        "the starved hauler must die"
    );

    // Gold is a closed balance (no source/sink): the dead colonist's gold left the
    // society for the commons, so the whole-system total is exactly unchanged.
    assert_eq!(
        s.total_gold(),
        gold_before,
        "gold is conserved across the death (society + commons)"
    );
    assert!(
        s.commons_gold() > Gold::ZERO,
        "the dead colonist's gold settled to the commons"
    );

    // Its carried FOOD escrow (world) plus any econ FOOD stock settled to the
    // commons too — physical goods conserved, not destroyed.
    assert!(
        s.commons_stock_of(FOOD) > 0,
        "the dead colonist's FOOD estate settled to the commons"
    );

    // The conservation receipt keeps balancing for several more ticks after the
    // death (the commons is a stable sink, never a leak).
    for _ in 0..8 {
        let report = s.econ_tick();
        assert!(report.conserves(), "post-death conservation broke");
    }
}

/// 3. After a death, no cache dangles a reference to the freed or a relocated slot:
///    the dead id is forgotten from the reservation caches and resolves `None`,
///    every live agent's reservation stays within its holdings, and the next econ
///    ticks run correctly (no panic, no stale order matching, conservation holds).
#[test]
fn caches_reconcile_no_dangling_reference() {
    let mut s = Settlement::generate(1, &dieoff_config());
    let population = s.living_total();

    assert!(
        run_to_first_death(&mut s, 40).is_some(),
        "the marginal settlement must record a death"
    );
    assert!(s.living_total() < population, "a colonist must have died");

    // Every dead colonist's id is freed and forgotten from the reservation cache.
    for index in 0..population {
        if !s.is_alive(index) {
            let dead = s.colonist_id(index).unwrap();
            assert!(
                s.society().agents.get(dead).is_none(),
                "a dead colonist resolves to None"
            );
            assert_eq!(
                s.society().reservations.reserved_gold(dead),
                Gold::ZERO,
                "a dead colonist's reservation is forgotten"
            );
        }
    }

    // No live agent's reservation exceeds its holdings — a dangling/relocated entry
    // would over- or under-commit a survivor. The next ticks run without panic and
    // keep conserving (no stale order from a freed slot ever matches).
    for _ in 0..12 {
        for agent in s.society().agents.iter() {
            assert!(
                s.society().reservations.reserved_gold(agent.id) <= agent.gold,
                "a live agent's reserved gold exceeds its balance after a death"
            );
        }
        let report = s.econ_tick();
        assert!(
            report.conserves(),
            "a tick after a death broke conservation"
        );
    }
}

/// 4. The freed colonist never bids/asks/works again and is absent from activation:
///    its id never reappears among the live agents the engine iterates, and it holds
///    no reservation, across many ticks after its death.
#[test]
fn dead_colonist_places_no_orders_and_is_not_activated() {
    let mut s = Settlement::generate(1, &dieoff_config());
    let population = s.living_total();

    assert!(
        run_to_first_death(&mut s, 40).is_some(),
        "the marginal settlement must record a death"
    );

    // Collect the dead ids.
    let dead_ids: Vec<AgentId> = (0..population)
        .filter(|&i| !s.is_alive(i))
        .map(|i| s.colonist_id(i).unwrap())
        .collect();
    assert!(!dead_ids.is_empty(), "at least one colonist must have died");

    for _ in 0..16 {
        s.econ_tick();
        for &dead in &dead_ids {
            // Absent from activation: the engine only iterates live arena agents.
            assert!(
                s.society().agents.iter().all(|a| a.id != dead),
                "a dead colonist reappeared among the live agents"
            );
            // Resolves to None and holds no spot reservation (posts no orders).
            assert!(s.society().agents.get(dead).is_none());
            assert_eq!(s.world().agent_status(dead), None);
            assert_eq!(
                s.society().reservations.reserved_gold(dead),
                Gold::ZERO,
                "a dead colonist holds a reservation"
            );
        }
    }
}

/// 5. A run with deaths is byte-identical across two invocations: the reconciliation
///    rebuild order is fixed, nothing is drawn in the loops.
#[test]
fn reconciliation_is_deterministic() {
    let cfg = dieoff_config();

    let mut a = Settlement::generate(7, &cfg);
    let mut b = Settlement::generate(7, &cfg);
    a.run(40);
    b.run(40);

    // The run actually exercised death (otherwise the test would be vacuous).
    assert!(
        a.living_total() < dieoff_config_population(&cfg),
        "the determinism run must include at least one death"
    );

    assert_eq!(
        a.canonical_bytes(),
        b.canonical_bytes(),
        "a death run diverged"
    );
    assert_eq!(a.digest(), b.digest());
    assert_eq!(a.commons_gold(), b.commons_gold(), "commons gold diverged");
    for &g in a.tracked_goods() {
        assert_eq!(
            a.commons_stock_of(g),
            b.commons_stock_of(g),
            "commons stock diverged for a good"
        );
    }

    // Tick-by-tick lockstep through the deaths: the digest matches at every tick.
    let mut x = Settlement::generate(7, &cfg);
    let mut y = Settlement::generate(7, &cfg);
    for tick in 0..40 {
        x.econ_tick();
        y.econ_tick();
        assert_eq!(
            x.digest(),
            y.digest(),
            "a death run drifted at econ tick {tick}"
        );
    }
}

fn dieoff_config_population(cfg: &SettlementConfig) -> usize {
    usize::from(cfg.gatherers) + usize::from(cfg.consumers)
}

/// 6. A death does not corrupt survivors: gold stays a conserved closed balance, no
///    survivor's reservation exceeds its holdings, and the market keeps clearing
///    (a survivor completes trades after a death).
#[test]
fn survivors_unaffected_by_a_death() {
    let mut s = Settlement::generate(1, &dieoff_config());
    let population = s.living_total();
    let gold_total = s.total_gold();

    // Run to the first death, asserting the closed gold balance is conserved every
    // tick (a corrupted survivor balance would show here).
    let mut first_death = None;
    for t in 0..40 {
        let report = s.econ_tick();
        assert!(report.conserves(), "conservation broke at tick {t}");
        assert_eq!(
            s.total_gold(),
            gold_total,
            "gold (society + commons) is not conserved across tick {t}"
        );
        if report.deaths > 0 && first_death.is_none() {
            first_death = Some(t);
            break;
        }
    }
    let first_death = first_death.expect("a death must occur");

    // Survivors remain (not everyone died at the death tick).
    assert!(s.living_total() >= 1, "no survivor remains after the death");
    assert!(s.living_total() < population, "no death actually occurred");

    // After the death: survivors' reservations stay within their holdings, gold
    // stays conserved, and the market keeps clearing FOOD (survivors complete
    // trades the death did not interrupt).
    let mut saw_trade_after_death = false;
    for _ in 0..(40 - first_death) {
        let report = s.econ_tick();
        assert!(report.conserves(), "post-death conservation broke");
        assert_eq!(
            s.total_gold(),
            gold_total,
            "a death corrupted the conserved gold total"
        );
        for agent in s.society().agents.iter() {
            assert!(
                s.society().reservations.reserved_gold(agent.id) <= agent.gold,
                "a survivor's reserved gold exceeds its balance after a death"
            );
        }
        if report.transferred_of(FOOD) > 0 {
            saw_trade_after_death = true;
        }
    }
    assert!(
        saw_trade_after_death,
        "the survivors' market stopped clearing after a death"
    );
}

/// 7. A run with NO deaths is byte-identical to the pre-G4a (tombstone-era) run: the
///    reconciliation never fires (deaths stay zero, the commons stays empty), and
///    the digest matches the pinned no-death tripwire. If this moves, the free-path
///    machinery leaked into the no-free hot path.
#[test]
fn no_death_path_is_byte_identical() {
    let config = SettlementConfig::viable();
    let mut s = Settlement::generate(0xC0FFEE, &config);

    for tick in 0..60 {
        let report = s.econ_tick();
        assert_eq!(
            report.deaths, 0,
            "the viable settlement died at tick {tick}"
        );
    }

    // The reconciliation NEVER fired: the commons is untouched, so the free + cache
    // reconciliation path is provably inert when no colonist dies.
    assert_eq!(
        s.commons_gold(),
        Gold::ZERO,
        "the commons must stay empty without a death"
    );
    for &g in s.tracked_goods() {
        assert_eq!(
            s.commons_stock_of(g),
            0,
            "the commons must stay empty without a death"
        );
    }

    // Byte-identical to the pre-G4a no-death run (the pinned tripwire) and stable
    // across reruns.
    assert_eq!(
        s.digest(),
        VIABLE_NO_DEATH_DIGEST,
        "the no-death path drifted — the free/reconcile machinery leaked into it"
    );
    let mut twin = Settlement::generate(0xC0FFEE, &config);
    twin.run(60);
    assert_eq!(
        s.digest(),
        twin.digest(),
        "the no-death run is not deterministic"
    );
}

/// 8. The engine is otherwise unperturbed: a plain settlement with no deaths is
///    byte-identical to its twin, and a settlement with deaths keeps the econ
///    invariants (gold conserved, reservations within holdings). The six econ
///    goldens staying byte-identical, the full G1/G2*/G3* suites, `cargo clippy
///    --workspace --all-targets -- -D warnings`, and `cargo fmt --check` are
///    enforced across the workspace (the real gate); this checks the local seam.
#[test]
fn econ_unchanged() {
    // A plain settlement is byte-identical to a twin — the engine still replays
    // deterministically with the G4a additions present but unexercised.
    let plain = SettlementConfig::viable();
    let mut s = Settlement::generate(42, &plain);
    let mut twin = Settlement::generate(42, &plain);
    s.run(30);
    twin.run(30);
    assert_eq!(s.canonical_bytes(), twin.canonical_bytes());
    assert_eq!(
        s.commons_gold(),
        Gold::ZERO,
        "a no-death run pools no estate"
    );

    // A settlement with deaths keeps the econ invariants tick over tick: the closed
    // gold balance is conserved (society + commons) and no agent over-commits.
    let mut d = Settlement::generate(3, &dieoff_config());
    let gold_total = d.total_gold();
    for _ in 0..40 {
        let report = d.econ_tick();
        assert!(report.conserves());
        assert_eq!(
            d.total_gold(),
            gold_total,
            "a death broke gold conservation"
        );
        for agent in d.society().agents.iter() {
            assert!(d.society().reservations.reserved_gold(agent.id) <= agent.gold);
        }
    }
    // A GOLD-stock guard: agents never hold the money good as physical stock here,
    // so the commons gold accounting never double-counts it.
    assert_eq!(
        d.commons_stock_of(GOLD),
        0,
        "GOLD is money, not commons stock"
    );
}
