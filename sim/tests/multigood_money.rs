//! S18 — money from a produced MULTI-GOOD economy.
//!
//! `frontier_multigood` re-adds the WOOD node S16 dropped and fields a WOODCUTTER role:
//! non-lineage `Gatherer`s pinned to the WOOD node (the `multigood_money` seam, not the
//! round-robin), who produce + sell WOOD and want bread/food — alongside the inherited bread
//! CULTIVATORS (lineages, sell surplus bread, want WOOD) and the SALT-anchor consumers. WOOD
//! is market-supplied AND provenance-clean: `wood_provision = 0` (no mint) and every initial
//! WOOD buffer zeroed, so traded WOOD can ONLY come from node-gathering.
//!
//! This file covers S18.1 (the woodcutter role + the WOOD provenance) and S18.2 (the
//! indirect-breadth instrumentation: the by-target accessor + the traced round-trip ledger).
//! The monetization DoD is the S18.3 acceptance suite.

use econ::good::{GoodId, WOOD};
use sim::{Settlement, SettlementConfig, Vocation};

const RUN_TICKS: u64 = 1500;

fn salt_good(cfg: &SettlementConfig) -> GoodId {
    cfg.barter.as_ref().expect("a barter overlay").medium_good
}

fn bread_good(cfg: &SettlementConfig) -> GoodId {
    cfg.chain.as_ref().expect("a chain").content.bread()
}

fn run(cfg: &SettlementConfig, ticks: u64) -> Settlement {
    let mut s = Settlement::generate(1, cfg);
    for _ in 0..ticks {
        s.econ_tick();
    }
    s
}

// ---- S18.1: the woodcutter role + a clean WOOD market --------------------

#[test]
fn multigood_run_is_deterministic() {
    // Byte-identical `(seed, config)`: a fixed, reproducible trajectory. The runtime-only
    // instrumentation (the WOOD source bound + the round-trip ledger) is NOT digested, so it
    // cannot perturb the identity.
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

// ---- S18.2: the indirect-breadth instrumentation -------------------------

#[test]
fn by_target_breadth_accessor_surfaces_membership() {
    // The by-target accessor surfaces the `IndirectFor{target}` MEMBERSHIP (the `&[GoodId]`)
    // the strong-bar gate counts but the emergence probe collapses to a count. On a real
    // medium (S9) it returns the actual target set; on the multi-good scenario (where SALT
    // never leads) it is empty.
    let s9_cfg = SettlementConfig::frontier_coemergent_strong();
    let s9 = run(&s9_cfg, 600);
    let s9_targets = s9.indirect_target_goods(salt_good(&s9_cfg));
    assert!(
        !s9_targets.is_empty(),
        "on a real medium the by-target accessor returns SALT's indirect target set: {s9_targets:?}"
    );

    let mg_cfg = SettlementConfig::frontier_multigood();
    let mg = run(&mg_cfg, RUN_TICKS);
    assert!(
        mg.indirect_target_goods(salt_good(&mg_cfg)).is_empty(),
        "on the multi-good scenario SALT never leads, so its indirect target set is empty"
    );
}

#[test]
fn salt_round_trips_not_hoarded() {
    // The traced round-trip ledger is the means-role guard. On the multi-good scenario SALT is
    // never even accepted as a means, so the round-trip is `0/0` — the means role never begins.
    // To prove the GUARD itself discriminates (it is not vacuously zero), run it on a REAL
    // medium: the S9 strong-bar economy, where SALT IS accepted IndirectFor a target.
    let multigood = run(&SettlementConfig::frontier_multigood(), RUN_TICKS);
    assert_eq!(
        multigood.salt_round_trip(),
        (0, 0),
        "in the multi-good scenario SALT is never accepted as a means, so it never round-trips"
    );

    let s9 = SettlementConfig::frontier_coemergent_strong();

    // Pre-promotion (the hoarding WINDOW): SALT is accepted IndirectFor a target, but the
    // round-trip stays ~0 — the acceptor uses a lower-good-id surplus to reach the target and
    // HOARDS the SALT (the gate counts acceptance at receipt, the Codex concern).
    let early = run(&s9, 400);
    let (early_spent, early_accepted) = early.salt_round_trip();
    assert!(
        early.promoted_at_tick().is_none(),
        "the early window is pre-promotion (the hoarding window)"
    );
    assert!(
        early_accepted > 0,
        "SALT IS accepted as a means on a real medium (accept-side volume > 0)"
    );
    assert_eq!(
        early_spent, 0,
        "pre-promotion the means role is incomplete — SALT is hoarded, not round-tripped"
    );
    assert_eq!(
        early.salt_round_trip_fraction_bps(),
        0,
        "the hoarding signature: accept-side volume > 0 while the round-trip fraction ~ 0"
    );

    // Run on to promotion: the means role then COMPLETES as money — the SALT accepted as a
    // means is spent acquiring its target, so the round-trip becomes material.
    let late = run(&s9, 1200);
    assert_eq!(
        late.current_money_good(),
        Some(salt_good(&s9)),
        "SALT monetizes on the real (no-double-coincidence) medium"
    );
    let (late_spent, late_accepted) = late.salt_round_trip();
    assert!(late_accepted > 0, "credits accrue on real indirect accepts");
    assert!(
        late_spent > 0,
        "the means role completes — SALT accepted as a means is later spent on its target"
    );
    assert!(
        late_spent <= late_accepted,
        "the round-trip can never spend more than was accepted as a means"
    );
}

#[test]
fn goldens_unchanged() {
    // The additive + gated changes leave every existing identity untouched. The
    // `multigood_money` flag emits its canonical marker only when active, and the runtime-only
    // instrumentation is excluded from `canonical_bytes` (both covered by the settlement unit
    // tests). The `lineages` + `g4a_death` tripwires must stay byte-identical.
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
