//! S18.1 — the woodcutter role + a market WOOD supply (the second produced good).
//!
//! `frontier_multigood` re-adds the WOOD node S16 dropped and fields a WOODCUTTER role:
//! non-lineage `Gatherer`s pinned to the WOOD node (the `multigood_money` seam, not the
//! round-robin), who produce + sell WOOD and want bread/food — alongside the inherited bread
//! CULTIVATORS (lineages, sell surplus bread, want WOOD) and the SALT-anchor consumers. WOOD
//! is market-supplied AND provenance-clean: `wood_provision = 0` (no mint) and every initial
//! WOOD buffer zeroed, so traded WOOD can ONLY come from node-gathering. This slice pins the
//! role structure + the WOOD provenance; the monetization question is the S18.3 DoD.

use econ::good::{GoodId, WOOD};
use sim::{Settlement, SettlementConfig, Vocation};

const RUN_TICKS: u64 = 1500;

fn bread_good(cfg: &SettlementConfig) -> GoodId {
    cfg.chain.as_ref().expect("a chain").content.bread()
}

#[test]
fn multigood_run_is_deterministic() {
    // Byte-identical `(seed, config)`: a fixed, reproducible trajectory. The runtime-only WOOD
    // instrumentation is NOT digested, so it cannot perturb the identity.
    let cfg = SettlementConfig::frontier_multigood();
    let mut a = Settlement::generate(1, &cfg);
    let mut b = Settlement::generate(1, &cfg);
    a.run(RUN_TICKS);
    b.run(RUN_TICKS);
    assert_eq!(
        a.canonical_bytes(),
        b.canonical_bytes(),
        "the multi-good run must be byte-identical for the same (seed, config)"
    );
    assert_eq!(a.digest(), b.digest());
    let mut c = Settlement::generate(2, &cfg);
    c.run(RUN_TICKS);
    assert_ne!(a.digest(), c.digest(), "the seed must change the run");
}

#[test]
fn woodcutters_supply_a_clean_wood_market() {
    // The role structure: WOODCUTTERS (non-lineage Gatherers on the WOOD node) hold a WOOD
    // surplus and run bread-short; bread CULTIVATORS (lineages) hold a bread surplus and run
    // WOOD-short. Each role's only surplus is its produced good. The WOOD they trade is
    // GATHERED (no mint, no buffer): nothing holds WOOD at generation, WOOD is never minted,
    // and the WOOD that circulates is bounded by the node→econ haul.
    let cfg = SettlementConfig::frontier_multigood();
    let bread = bread_good(&cfg);

    // Provenance-clean at generation: nothing is seeded holding WOOD or bread.
    let s0 = Settlement::generate(1, &cfg);
    let init_wood: u64 = (0..s0.population()).map(|i| s0.stock_of(i, WOOD)).sum();
    let init_bread: u64 = (0..s0.population()).map(|i| s0.stock_of(i, bread)).sum();
    assert_eq!(init_wood, 0, "no WOOD is seeded (every WOOD buffer zeroed)");
    assert_eq!(init_bread, 0, "no bread is seeded");

    // The woodcutters are pinned to the WOOD node, not grain.
    let wood_node = s0.wood_node().expect("a WOOD node");
    let grain_node = s0.grain_node().expect("a grain node");
    assert_ne!(wood_node, grain_node, "WOOD and grain are distinct nodes");
    let mut woodcutters = 0;
    for i in 0..s0.population() {
        if s0.is_alive(i)
            && s0.vocation_of(i) == Some(Vocation::Gatherer)
            && s0.household_of(i).is_none()
        {
            woodcutters += 1;
            assert_eq!(
                s0.node_of(i),
                Some(wood_node),
                "a woodcutter must be pinned to the WOOD node, not grain (the seam)"
            );
        }
    }
    assert!(woodcutters > 0, "the colony must field woodcutters");

    // Run, asserting WOOD is never minted, and watch the role holdings cross the clean states.
    let mut s = Settlement::generate(1, &cfg);
    let mut saw_cultivator_bread_no_wood = false;
    let mut saw_woodcutter_wood_no_bread = false;
    for _ in 0..RUN_TICKS {
        let report = s.econ_tick();
        assert_eq!(report.endowment_of(WOOD), 0, "WOOD must never be minted");
        for i in 0..s.population() {
            if !s.is_alive(i) {
                continue;
            }
            let (bread_held, wood_held) = (s.stock_of(i, bread), s.stock_of(i, WOOD));
            // A lineage cultivator with a bread surplus and zero WOOD (it must BUY WOOD).
            if s.household_of(i).is_some() && bread_held > 0 && wood_held == 0 {
                saw_cultivator_bread_no_wood = true;
            }
            // A woodcutter with a WOOD surplus and zero bread (it must BUY bread).
            if s.household_of(i).is_none()
                && s.vocation_of(i) == Some(Vocation::Gatherer)
                && wood_held > 0
                && bread_held == 0
            {
                saw_woodcutter_wood_no_bread = true;
            }
        }
    }
    assert!(
        saw_cultivator_bread_no_wood,
        "a cultivator must hold a bread surplus with zero WOOD (genuinely WOOD-short)"
    );
    assert!(
        saw_woodcutter_wood_no_bread,
        "a woodcutter must hold a WOOD surplus with zero bread (genuinely bread-short)"
    );

    // The WOOD that circulates is GATHERED: it entered the economy only by the node→econ
    // haul, and the traded WOOD→medium volume cannot exceed that gather (the provenance bound).
    assert!(
        s.wood_gathered_total() > 0,
        "WOOD must enter the economy by node-gathering"
    );
    assert!(
        s.trade_volume_of(WOOD) > 0,
        "the gathered WOOD reaches a real market"
    );
    assert!(
        s.pre_promotion_wood_for_salt_volume() <= s.wood_gathered_total(),
        "traded WOOD→medium cannot exceed the WOOD gathered (the provenance bound)"
    );
}

#[test]
fn multigood_conserves() {
    // Whole-system conservation every tick: the grain + WOOD nodes regen the sources, bread is
    // produced, WOOD is gathered, and NOTHING is minted (no food/WOOD endowment).
    let cfg = SettlementConfig::frontier_multigood();
    let bread = bread_good(&cfg);
    let mut s = Settlement::generate(1, &cfg);
    for tick in 0..RUN_TICKS {
        let report = s.econ_tick();
        assert!(
            report.conserves(),
            "whole-system conservation must hold at tick {tick}"
        );
        assert_eq!(
            report.endowment_of(WOOD) + report.endowment_of(bread),
            0,
            "no food/WOOD may be minted at tick {tick}"
        );
    }
}

#[test]
fn goldens_unchanged() {
    // The additive + gated changes leave every existing identity untouched. The
    // `multigood_money` flag emits its canonical marker only when active, and the runtime-only
    // WOOD instrumentation is excluded from `canonical_bytes` (both covered by the settlement
    // unit tests). The `lineages` + `g4a_death` tripwires must stay byte-identical.
    let digest = |cfg: &SettlementConfig, ticks: u64| {
        let mut s = Settlement::generate(1, cfg);
        s.run(ticks);
        s.digest()
    };
    assert_eq!(
        digest(&SettlementConfig::lineages(), 300),
        0x2335e13c809749fc,
        "the `lineages` demographic golden (the key tripwire) must be byte-identical"
    );
    assert_eq!(
        digest(&SettlementConfig::lineages(), 800),
        0x3ffd78e50842d934,
        "the long `lineages` run must be byte-identical"
    );
    let mut viable = Settlement::generate(0xC0FFEE, &SettlementConfig::viable());
    viable.run(60);
    assert_eq!(
        viable.digest(),
        0xa174_8567_db1c_4341,
        "the g4a_death no-death golden must be byte-identical"
    );
}
