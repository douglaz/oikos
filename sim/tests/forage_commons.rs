//! S14.1 — FORAGE as a real **capped commons**.
//!
//! Behind `ChainConfig::forage_commons` (default `None`), the FORAGE node stops
//! being S12's `0/0/0` marker (which credited a fixed `forage_yield` per completed
//! forage task, independent of forager count) and becomes a real depleting
//! `world::ResourceNode`. Foragers harvest it through the existing GoHarvest haul
//! cycle (harvest → carry → deposit → transfer), so per-capita yield **falls** as
//! the foraging population grows — the carrying capacity. Node regen is the only
//! source, so conservation is untouched, and the deposit attribution is widened to
//! cover foragers (a Consumer/Unassigned/lineage forager, not only a Gatherer) so
//! the foraged FORAGE actually transfers to econ stock.
//!
//! The DoD here: N foragers draw ≈ the regen budget (NOT N × yield), per-capita
//! falls as N rises, a spatial lineage **Consumer**'s foraged FORAGE is attributed,
//! transferred, and relieves hunger, the run conserves, and with the commons flag
//! off the S12 fixed-credit path is byte-identical.

use econ::good::GoodId;
use sim::{ForageCommons, Settlement, SettlementConfig, Vocation, FAST_TICKS_PER_ECON_TICK};

fn forage_good(cfg: &SettlementConfig) -> GoodId {
    cfg.chain
        .as_ref()
        .expect("the provisioned chain")
        .content
        .forage()
        .expect("own-labor subsistence interns a forage good")
}

/// Immortal **consumer-foragers**, no demography: consumers never age out and never
/// starve (`hunger_critical` is disabled on the frontier), and they forage when
/// hungry — so the foraging population is exactly `consumers`, a clean control for
/// the per-capita sweep. Built from the S12 own-labor flagship with the chain
/// producers/buffers stripped and a [`ForageCommons`] attached.
fn consumer_commons(consumers: u16, stock: u32, regen: u32, cap: u32) -> SettlementConfig {
    let mut cfg = SettlementConfig::frontier_coemergent_strong_provisioned();
    cfg.gatherers = 0;
    cfg.consumers = consumers;
    cfg.demography = None;
    let chain = cfg.chain.as_mut().expect("chain");
    chain.millers = 0;
    chain.bakers = 0;
    chain.latent_millers = 0;
    chain.latent_bakers = 0;
    chain.bread_buffer = 0;
    chain.consumer_staple_buffer = 0;
    chain.latent_flour_seed = 0;
    chain.forage_commons = Some(ForageCommons { stock, regen, cap });
    cfg
}

/// (total FORAGE transferred to econ, total regen, total produced, max living hunger)
/// over `ticks`, asserting whole-system conservation every tick.
fn forage_flow(cfg: &SettlementConfig, ticks: u64) -> (u64, u64, u64, u16) {
    let fg = forage_good(cfg);
    let mut s = Settlement::generate(1, cfg);
    let (mut transferred, mut regen, mut produced) = (0u64, 0u64, 0u64);
    for tick in 0..ticks {
        let r = s.econ_tick();
        assert!(r.conserves(), "conservation broke at tick {tick}");
        transferred += r.transferred_of(fg);
        regen += r.regen_of(fg);
        produced += r.produced_of(fg);
    }
    (transferred, regen, produced, s.max_living_hunger())
}

#[test]
fn commons_is_a_real_node_not_a_marker() {
    // With the commons set, the FORAGE node carries the configured stock/regen/cap (a
    // real depleting ResourceNode). With it off (the S12 path), the node is a 0/0/0
    // marker — proving the mode switch lands on the node, not just the credit.
    let on = consumer_commons(4, 30, 5, 60);
    let s_on = Settlement::generate(1, &on);
    let node_on = s_on
        .forage_node_id()
        .and_then(|id| s_on.world().node(id))
        .expect("the commons forage node exists");
    assert_eq!(
        (node_on.stock, node_on.regen_per_tick, node_on.cap),
        (30, 5, 60)
    );

    let mut off = consumer_commons(4, 30, 5, 60);
    off.chain.as_mut().unwrap().forage_commons = None;
    let s_off = Settlement::generate(1, &off);
    let node_off = s_off
        .forage_node_id()
        .and_then(|id| s_off.world().node(id))
        .expect("the S12 marker forage node still exists");
    assert_eq!(
        (node_off.stock, node_off.regen_per_tick, node_off.cap),
        (0, 0, 0),
        "with the commons off the FORAGE node stays the S12 0/0/0 marker"
    );
}

#[test]
fn commons_draw_is_bounded_by_regen_not_forager_count() {
    // The defining commons property: the total FORAGE drawn is bounded by the node's
    // regen budget (the only source), NOT by the forager count × a fixed yield. So
    // doubling the foragers past saturation does NOT double the draw. Booked as regen
    // (the node), never `produced` (no fixed credit).
    let ticks = 300u64;
    let regen = 1u32;
    let budget = u64::from(regen) * FAST_TICKS_PER_ECON_TICK * ticks; // initial stock 0

    let (few_t, few_regen, few_produced, _) =
        forage_flow(&consumer_commons(8, 0, regen, 40), ticks);
    let (many_t, _, many_produced, _) = forage_flow(&consumer_commons(48, 0, regen, 40), ticks);

    assert_eq!(
        few_produced, 0,
        "the commons books no fixed `produced` credit"
    );
    assert_eq!(
        many_produced, 0,
        "the commons books no fixed `produced` credit"
    );
    assert!(
        few_regen > 0,
        "the FORAGE node must actually regenerate (the source)"
    );
    assert!(
        few_t > 0,
        "foragers must draw FORAGE through the haul cycle"
    );

    assert!(
        many_t <= budget,
        "total draw {many_t} must be bounded by the regen budget {budget}"
    );
    // 48 foragers do NOT draw 6× what 8 draw: the commons saturates near the regen
    // budget. A fixed-credit path would scale ~linearly with the forager count.
    assert!(
        many_t < few_t * 3,
        "draw must saturate at the commons budget, not scale with foragers ({few_t} -> {many_t})"
    );
}

#[test]
fn per_capita_yield_falls_as_foragers_rise() {
    // Once the commons binds, per-capita FORAGE strictly falls as the foraging
    // population grows — the carrying-capacity pressure. Hunger climbs in step (the
    // preventive-check signal S14.3 exploits), with deaths impossible here (no
    // demography, hunger_critical disabled).
    let ticks = 300u64;
    let mut per_capita = Vec::new();
    let mut max_hunger = Vec::new();
    for &n in &[4u16, 8, 16, 32] {
        let (transferred, _, _, maxhung) = forage_flow(&consumer_commons(n, 0, 1, 40), ticks);
        per_capita.push(transferred / u64::from(n));
        max_hunger.push(maxhung);
    }
    for w in per_capita.windows(2) {
        assert!(
            w[1] < w[0],
            "per-capita FORAGE must fall as foragers rise: {per_capita:?}"
        );
    }
    assert!(
        max_hunger.last() >= max_hunger.first(),
        "hunger must rise (not fall) as the commons binds: {max_hunger:?}"
    );
}

/// A spatial-lineage commons: founders are spatial (so lineage Consumers forage) and
/// start with no staple buffer, so they forage the commons from the first hungry
/// tick. The forage is generous (4 founders, regen 4) so they stay fed.
fn spatial_lineage_commons() -> SettlementConfig {
    let mut cfg = SettlementConfig::frontier_coemergent_strong_provisioned();
    cfg.gatherers = 0;
    cfg.consumers = 0;
    {
        let chain = cfg.chain.as_mut().expect("chain");
        chain.millers = 0;
        chain.bakers = 0;
        chain.latent_millers = 0;
        chain.latent_bakers = 0;
        chain.bread_buffer = 0;
        chain.consumer_staple_buffer = 0;
        chain.latent_flour_seed = 0;
        chain.forage_commons = Some(ForageCommons {
            stock: 80,
            regen: 4,
            cap: 160,
        });
    }
    {
        let demo = cfg.demography.as_mut().expect("demography");
        demo.spatial_households = true;
        demo.max_household_size = 20;
        for h in &mut demo.households {
            h.founders = 2;
            h.starting_food = 0;
        }
    }
    cfg
}

#[test]
fn spatial_lineage_consumer_forages_transfers_and_is_fed() {
    // The deposit-attribution fix in action: a spatial lineage member is a Consumer,
    // yet its foraged FORAGE must be attributed and transferred into its OWN econ
    // stock (pre-S14 only Gatherers were attributed) — and that FORAGE relieves its
    // hunger. The debug `carry-delta == exchange-increase` assertion inside the fast
    // loop also guards the attribution here (a missed forager deposit would panic).
    let cfg = spatial_lineage_commons();
    let fg = forage_good(&cfg);
    let mut s = Settlement::generate(1, &cfg);
    // Watch the founders' lifetime (frontier lifespans are short, and without the S14.2
    // FORAGE child endowment births stall on bread, so the lineage cannot yet grow —
    // that is S14.2's job). Within it, a spatial lineage Consumer must forage the
    // commons, have the foraged FORAGE attributed + transferred into its OWN econ stock
    // (the fix), and be fed by it (hunger relieved well below the ceiling).
    let mut saw_attributed_forager = false;
    let mut saw_fed_forager = false;
    for tick in 0..30u64 {
        let r = s.econ_tick();
        assert!(r.conserves(), "conservation broke at tick {tick}");
        for i in 0..s.population() {
            if !s.is_alive(i) || s.household_of(i).is_none() {
                continue;
            }
            assert_eq!(
                s.vocation_of(i),
                Some(Vocation::Consumer),
                "lineage members are Consumers"
            );
            let id = s.colonist_id(i).expect("a live colonist has an id");
            let econ_forage = s.society().agents.get(id).map_or(0, |a| a.stock.get(fg));
            let hunger = s.need_of(i).map(|n| n.hunger).unwrap_or(u16::MAX);
            if econ_forage > 0 {
                // The foraged FORAGE was attributed to this lineage Consumer and
                // transferred into its OWN econ stock (pre-S14, only Gatherers were).
                saw_attributed_forager = true;
            }
            if econ_forage > 0 && hunger <= 4 {
                saw_fed_forager = true;
            }
        }
    }
    assert!(
        saw_attributed_forager,
        "a spatial lineage Consumer's foraged FORAGE must be attributed + transferred to its econ stock"
    );
    assert!(
        saw_fed_forager,
        "the foraged FORAGE must relieve the lineage Consumer's hunger"
    );
}

#[test]
fn commons_conserves_with_node_regen_the_only_source() {
    // Whole-system conservation every tick, with FORAGE created ONLY by node regen
    // (no `produced` credit, no minted food) — the relocations are harvest/deposit/
    // transfer and the only sink is consumption.
    let cfg = consumer_commons(12, 20, 2, 80);
    let fg = forage_good(&cfg);
    let mut s = Settlement::generate(3, &cfg);
    for tick in 0..400u64 {
        let r = s.econ_tick();
        assert!(r.conserves(), "conservation broke at tick {tick}");
        assert_eq!(
            r.produced_of(fg),
            0,
            "the commons mints no FORAGE via the fixed credit (tick {tick})"
        );
    }
}

#[test]
fn commons_off_is_byte_identical_to_the_s12_fixed_credit_path() {
    // The gating is real and in one place: a default-`None` commons run equals an
    // explicit-`None` run (the field is inert off), and turning the commons on changes
    // the bytes. The S12 fixed-credit forage path (the `provisioned` flagship) is the
    // untouched baseline — its builder is not mutated, so its golden is byte-identical.
    let run = |cfg: &SettlementConfig| {
        let mut s = Settlement::generate(1, cfg);
        s.run(250);
        (s.canonical_bytes(), s.digest())
    };

    let base = SettlementConfig::frontier_coemergent_strong_provisioned();
    let (base_bytes, _) = run(&base);
    let mut explicit_off = SettlementConfig::frontier_coemergent_strong_provisioned();
    explicit_off.chain.as_mut().unwrap().forage_commons = None;
    let (off_bytes, _) = run(&explicit_off);
    assert_eq!(
        base_bytes, off_bytes,
        "a commons-off run must be byte-identical to the S12 fixed-credit path"
    );

    let mut on = SettlementConfig::frontier_coemergent_strong_provisioned();
    on.chain.as_mut().unwrap().forage_commons = Some(ForageCommons {
        stock: 50,
        regen: 3,
        cap: 100,
    });
    let (on_bytes, _) = run(&on);
    assert_ne!(
        base_bytes, on_bytes,
        "turning the commons on must change the canonical bytes (the gating is real)"
    );
}
