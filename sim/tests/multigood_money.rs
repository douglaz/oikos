//! S18 — money from a produced MULTI-GOOD economy (the DoD acceptance suite).
//!
//! `frontier_multigood` builds a real division of labor with TWO produced/gathered goods and
//! role-separated cross-demand: bread CULTIVATORS (lineages, sell surplus bread, want WOOD) ⇄
//! WOODCUTTERS (non-lineage gatherers pinned to the WOOD node, sell WOOD, want bread) ⇄
//! SALT-anchor consumers (hold SALT, buy both). Mints off (food AND WOOD), WOOD
//! provenance-clean (every buffer + the mint zeroed), `min_indirect_target_goods = 2`,
//! mortality off. The question: does money emerge from a produced multi-good economy?
//!
//! THE OUTCOME — **PRINCIPLED FAILURE** (the anticipated one — `multigood_money_finding`),
//! robust across the WOOD-flow / role-count sweep: SALT does NOT promote. The two-good
//! complementary division of labor is a **perfect double coincidence of wants** — the
//! cultivators want exactly what the woodcutters produce (WOOD) and the woodcutters want
//! exactly what the cultivators produce (bread) — so the two roles barter bread↔WOOD
//! DIRECTLY and no medium is needed. WOOD (the most-gathered good) becomes the rejected
//! provisional saleability leader; SALT, with only its heterogeneous direct-use anchor, never
//! leads, so its by-target indirect breadth is EMPTY (not `{bread, WOOD}`) and the traced
//! round-trip is `0/0` — SALT never even begins to intermediate. This DEEPENS the S16
//! finding: it is not just hunger-stress — money emerges to bridge the ABSENCE of a double
//! coincidence (S9's ≥3-good economy promotes SALT), and a two-good complementary economy is
//! precisely the case where it does not. The instrumentation proves the negative (by-target
//! breadth empty, round-trip 0, WOOD provenance-clean) and the controls bracket it (S9
//! promotes; dropping the WOOD market or the SALT anchor keeps no-promotion). The finding is
//! NOT rescued by minting or by inventing a want.

use econ::good::{GoodId, WOOD};
use sim::{Settlement, SettlementConfig, Vocation};

const RUN_TICKS: u64 = 2000;

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

/// The acceptances accrued by `good` as a money candidate (the direct-acceptance saleability
/// the provisional-leader rule reads).
fn acceptances(s: &Settlement, good: GoodId) -> u64 {
    s.emergence_acceptances()
        .into_iter()
        .find(|c| c.good == good)
        .map(|c| c.acceptances)
        .unwrap_or(0)
}

// ---- 1. determinism ------------------------------------------------------

#[test]
fn multigood_run_is_deterministic() {
    // Byte-identical `(seed, config)`: the run is a fixed, reproducible trajectory. The
    // runtime-only instrumentation (the WOOD source bound + the round-trip ledger) is NOT
    // digested, so it cannot perturb the identity.
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

    // The seed matters (founder cultures + woodcutter cultures are drawn from it).
    let mut c = Settlement::generate(2, &cfg);
    c.run(RUN_TICKS);
    assert_ne!(a.digest(), c.digest(), "the seed must change the run");
}

// ---- 2. role separation / no preemption ----------------------------------

#[test]
fn two_clean_surplus_goods_no_preemption() {
    // The role separation is clean enough that the lowest-good-id preemption
    // (`post_first_direct_barter_offer` offers WOOD (id 2) before bread (id 9)) never
    // suppresses a producer's primary surplus: WOOD is offered ONLY by the woodcutter role
    // (so no cultivator ever holds WOOD to preempt its bread), and BOTH bread and WOOD reach
    // the barter book with a substantial, sustained volume. The woodcutters are on the WOOD
    // node, not grain.
    let cfg = SettlementConfig::frontier_multigood();
    let bread = bread_good(&cfg);
    let s = run(&cfg, RUN_TICKS);

    // Both produced goods reach the book — neither is preempted into silence.
    assert!(
        s.trade_volume_of(bread) > 100,
        "bread must reach the barter book with a material volume"
    );
    assert!(
        s.trade_volume_of(WOOD) > 100,
        "WOOD must reach the barter book with a material volume"
    );

    // WOOD is offered ONLY by the woodcutter role (non-lineage Gatherers). The crux: no
    // LINEAGE cultivator ever offers WOOD, so a cultivator never holds both surplus classes
    // and its bread is never preempted by a lower-good-id WOOD offer.
    let wood_givers = s.barter_givers_of(WOOD);
    assert!(!wood_givers.is_empty(), "the WOOD market must have sellers");
    for giver in &wood_givers {
        let index = (0..s.population())
            .find(|&i| s.colonist_id(i) == Some(*giver))
            .expect("a live WOOD giver maps to a generation index");
        assert_eq!(
            s.vocation_of(index),
            Some(Vocation::Gatherer),
            "every WOOD seller must be a woodcutter (a Gatherer), not a cultivator"
        );
        assert_eq!(
            s.household_of(index),
            None,
            "every WOOD seller must be a NON-lineage woodcutter (the role-separation guard)"
        );
    }

    // The woodcutters are pinned to the WOOD node (Codex P1b), not the grain node.
    let wood_node = s.wood_node().expect("a WOOD node");
    let grain_node = s.grain_node().expect("a grain node");
    assert_ne!(wood_node, grain_node, "WOOD and grain are distinct nodes");
    let mut woodcutters = 0;
    for i in 0..s.population() {
        if s.is_alive(i)
            && s.vocation_of(i) == Some(Vocation::Gatherer)
            && s.household_of(i).is_none()
        {
            woodcutters += 1;
            assert_eq!(
                s.node_of(i),
                Some(wood_node),
                "a woodcutter must harvest the WOOD node, not grain"
            );
        }
    }
    assert!(woodcutters > 0, "the colony must field woodcutters");
}

// ---- 3. the finding ------------------------------------------------------

#[test]
fn multigood_money_finding() {
    // THE FINDING. In the produced two-good economy SALT does NOT promote — the role-separated
    // cross-demand is a PERFECT DOUBLE COINCIDENCE OF WANTS (cultivators want exactly what the
    // woodcutters produce and vice versa), so the two roles barter bread↔WOOD DIRECTLY and a
    // medium is superfluous. The characterized reason, by the instrumentation:
    let cfg = SettlementConfig::frontier_multigood();
    let salt = salt_good(&cfg);
    let bread = bread_good(&cfg);
    let s = run(&cfg, RUN_TICKS);

    // (i) No promotion: no money good emerges.
    assert!(
        s.promoted_at_tick().is_none(),
        "SALT must NOT promote against the two-good complementary division of labor"
    );
    assert!(s.current_money_good().is_none(), "no money good emerges");

    // (ii) The provisional-leader trace: a PRODUCED/gathered good out-leads SALT. WOOD (the
    // most-gathered good) dominates the direct-acceptance saleability; SALT, with only its
    // heterogeneous anchor, is barely accepted — so it never becomes the provisional leader.
    let salt_acc = acceptances(&s, salt);
    let wood_acc = acceptances(&s, WOOD);
    let bread_acc = acceptances(&s, bread);
    assert!(
        wood_acc > salt_acc && bread_acc > salt_acc,
        "both produced goods (WOOD {wood_acc}, bread {bread_acc}) must out-accept SALT {salt_acc}"
    );
    assert!(
        wood_acc > 10 * salt_acc.max(1),
        "WOOD must DOMINATE the saleability — the rejected provisional leader, not SALT"
    );

    // (iii) The by-target indirect breadth is EMPTY: SALT never leads, so no agent posts an
    // `IndirectFor{...}` offer accepting SALT — the two-sided produced breadth {bread, WOOD}
    // the strong-bar gate requires never forms.
    let targets = s.indirect_target_goods(salt);
    assert!(
        !(targets.contains(&bread) && targets.contains(&WOOD)),
        "the two-sided breadth {{bread, WOOD}} must NOT form (it never even leads): {targets:?}"
    );
    assert!(
        targets.is_empty(),
        "SALT accrues ZERO indirect target goods (it is never the provisional leader): {targets:?}"
    );

    // (iv) The traced round-trip is 0/0: SALT is never even accepted as a means, so it never
    // begins to intermediate (the strongest form of "means role incomplete").
    let (spent, accepted) = s.salt_round_trip();
    assert_eq!(
        (spent, accepted),
        (0, 0),
        "SALT never round-trips because it is never accepted IndirectFor a target"
    );

    // (v) The mechanism: bread and WOOD are traded for EACH OTHER, not for SALT — the direct
    // double coincidence. The bread→medium volume is identically zero.
    assert_eq!(
        s.bread_for_salt_volume(),
        0,
        "bread is bartered for WOOD directly, never for SALT (the double coincidence)"
    );
    assert!(
        s.trade_volume_of(WOOD) > 0 && s.trade_volume_of(bread) > 0,
        "yet both produced goods ARE traded — directly, against each other"
    );
}

// ---- 4. provenance: the traded goods are produced/gathered, not minted ---

#[test]
fn the_traded_goods_are_gathered_not_minted() {
    // Even though SALT does not monetize, the goods that reach the market are provenance-clean:
    // bread is PRODUCED (cultivated) and WOOD is GATHERED — neither is minted or seeded.
    let cfg = SettlementConfig::frontier_multigood();
    let bread = bread_good(&cfg);

    // Nothing is seeded holding WOOD or bread at generation (every buffer zeroed).
    let s0 = Settlement::generate(1, &cfg);
    let init_wood: u64 = (0..s0.population()).map(|i| s0.stock_of(i, WOOD)).sum();
    let init_bread: u64 = (0..s0.population()).map(|i| s0.stock_of(i, bread)).sum();
    assert_eq!(init_wood, 0, "no WOOD is seeded (every WOOD buffer zeroed)");
    assert_eq!(
        init_bread, 0,
        "no bread is seeded (the bread buffers absent)"
    );

    // Run, asserting no minting of either good on any tick.
    let mut s = Settlement::generate(1, &cfg);
    for tick in 0..RUN_TICKS {
        let report = s.econ_tick();
        assert_eq!(
            report.endowment_of(WOOD),
            0,
            "WOOD must never be minted (wood_provision = 0, mint off) at tick {tick}"
        );
        assert_eq!(
            report.endowment_of(bread),
            0,
            "bread must never be minted (own-labor path, mint off) at tick {tick}"
        );
    }

    // WOOD enters the economy ONLY by node-gathering, and the traded WOOD→medium leg is
    // bounded by that gather (the WOOD provenance bound, Codex P1a).
    assert!(
        s.wood_gathered_total() > 0,
        "WOOD must enter the economy by node-gathering"
    );
    assert!(
        s.pre_promotion_wood_for_salt_volume() <= s.wood_gathered_total(),
        "the traded WOOD→medium volume cannot exceed the WOOD gathered (provenance bound)"
    );
    assert_eq!(
        s.wood_for_salt_volume(),
        s.pre_promotion_wood_for_salt_volume(),
        "with no promotion in the finding, every WOOD→medium trade is pre-promotion"
    );
    assert!(
        s.trade_volume_of(WOOD) > 0,
        "the gathered WOOD reaches a real market"
    );

    // Bread is produced (cultivated): the provenance ledger credited produced bread, and the
    // minted bread→medium contribution is provably zero.
    let (credited, _sunk) = s.produced_bread_credited_and_sunk();
    assert!(credited > 0, "bread is produced by cultivation");
    let (_produced, minted) = s.bread_for_salt_volume_by_provenance();
    assert_eq!(minted, 0, "the minted bread→medium contribution is zero");
}

// ---- 5. the round-trip ledger (the means-role guard) ---------------------

#[test]
fn salt_round_trips_not_hoarded() {
    // The traced round-trip ledger is the means-role guard. On the multi-good flagship SALT is
    // never even accepted as a means (the finding), so the round-trip is `0/0` — the means
    // role never begins. To prove the GUARD itself discriminates (it is not vacuously zero),
    // run it on a REAL medium: the S9 strong-bar economy, where SALT IS accepted IndirectFor a
    // target and DOES monetize.
    let multigood = run(&SettlementConfig::frontier_multigood(), RUN_TICKS);
    assert_eq!(
        multigood.salt_round_trip(),
        (0, 0),
        "in the finding SALT is never accepted as a means, so it never round-trips"
    );

    let s9 = SettlementConfig::frontier_coemergent_strong();

    // Pre-promotion (the hoarding WINDOW): SALT is accepted IndirectFor a target, but the
    // barter round-trip stays ~0 — the acceptor uses a lower-good-id surplus to reach the
    // target and HOARDS the SALT (the gate counts acceptance at receipt, the Codex concern).
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
    let early_targets = early.indirect_target_goods(salt_good(&s9));
    let mut pending_total = 0;
    for index in 0..early.population() {
        for &target in &early_targets {
            pending_total += early.pending_indirect_salt(index, target);
        }
    }
    assert!(
        pending_total > 0,
        "accepted-as-means SALT must remain visible in the pending ledger during the hoarding window"
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

// ---- 6. the by-target breadth accessor (S18.2) ---------------------------

#[test]
fn by_target_breadth_accessor_surfaces_membership() {
    // The by-target accessor surfaces the `IndirectFor{target}` MEMBERSHIP (the `&[GoodId]`)
    // the strong-bar gate counts but the emergence probe collapses to a count. On a real
    // medium (S9) it returns the actual target set; on the multi-good finding it is empty.
    let s9 = run(&SettlementConfig::frontier_coemergent_strong(), 600);
    let s9_salt = salt_good(&SettlementConfig::frontier_coemergent_strong());
    let s9_targets = s9.indirect_target_goods(s9_salt);
    assert!(
        !s9_targets.is_empty(),
        "on a real medium the by-target accessor returns SALT's indirect target set: {s9_targets:?}"
    );

    let mg = run(&SettlementConfig::frontier_multigood(), RUN_TICKS);
    let mg_salt = salt_good(&SettlementConfig::frontier_multigood());
    assert!(
        mg.indirect_target_goods(mg_salt).is_empty(),
        "on the finding SALT never leads, so its indirect target set is empty"
    );
}

// ---- 7. conservation -----------------------------------------------------

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

// ---- 8. controls that bracket the finding --------------------------------

#[test]
fn controls_close_the_finding() {
    // The controls isolate "the perfect double coincidence is what suppresses SALT".

    // (contrast) The S9 strong-bar economy — ≥3 traded goods (grain, WOOD, bread) with NO
    // clean double coincidence — DOES promote SALT. The gate/mechanism works; the multi-good
    // failure is structural (the double coincidence), not a broken gate.
    let s9 = run(&SettlementConfig::frontier_coemergent_strong(), 1200);
    assert_eq!(
        s9.current_money_good(),
        Some(salt_good(&SettlementConfig::frontier_coemergent_strong())),
        "the strong-bar mechanism monetizes SALT when there is no clean double coincidence"
    );

    // (a) No WOOD market (drop the woodcutters): the economy collapses to the S16 single
    // produced good (bread) — no WOOD is traded and SALT still does not promote.
    let mut no_wood = SettlementConfig::frontier_multigood();
    no_wood.gatherers = 0;
    let s_no_wood = run(&no_wood, RUN_TICKS);
    assert!(
        s_no_wood.promoted_at_tick().is_none(),
        "with no WOOD market the economy is S16 single-good — SALT still does not promote"
    );
    assert_eq!(
        s_no_wood.trade_volume_of(WOOD),
        0,
        "dropping the woodcutters removes the WOOD market entirely"
    );

    // (c) No SALT direct-use anchor: with nothing directly wanting SALT it has even less
    // saleability and never leads — SALT does not promote.
    let mut no_anchor = SettlementConfig::frontier_multigood();
    if let Some(barter) = no_anchor.barter.as_mut() {
        barter.salt_direct_use_qty = 0;
        barter.salt_direct_use_period = 0;
    }
    let s_no_anchor = run(&no_anchor, RUN_TICKS);
    assert!(
        s_no_anchor.promoted_at_tick().is_none(),
        "without the SALT anchor SALT does not lead and does not promote"
    );

    // (b) Indirect acceptance disabled: no agent posts an IndirectFor offer, so no indirect
    // breadth can accrue — SALT does not promote.
    let mut no_indirect = SettlementConfig::frontier_multigood();
    if let Some(barter) = no_indirect.barter.as_mut() {
        barter.menger.allow_indirect_acceptance = false;
    }
    let s_no_indirect = run(&no_indirect, RUN_TICKS);
    assert!(
        s_no_indirect.promoted_at_tick().is_none(),
        "with indirect acceptance disabled SALT cannot accrue breadth and does not promote"
    );

    // (d) No role separation: collapse one producer group by seeding the WOOD sellers
    // with a large bread surplus too. Restore a non-consuming medium-holding want in this
    // control only, so the collapsed producers have an observable reason to sell while the
    // SALT-rich buyers still retain stock. Those same agents now hold WOOD (lower good id)
    // and bread, so the one-offer-per-agent book offers WOOD first and their bread never
    // gets a clean surplus-offer lane. The result is no two-sided indirect breadth and no
    // promotion.
    let mut no_roles = SettlementConfig::frontier_multigood();
    if let Some(chain) = no_roles.chain.as_mut() {
        chain.bread_buffer = 5_000;
    }
    if let Some(barter) = no_roles.barter.as_mut() {
        barter.medium_want_qty = 1;
    }
    let no_role_bread = bread_good(&no_roles);
    let s_no_roles_initial = Settlement::generate(1, &no_roles);
    let seeded_woodcutters = (0..s_no_roles_initial.population())
        .filter(|&index| {
            s_no_roles_initial.vocation_of(index) == Some(Vocation::Gatherer)
                && s_no_roles_initial.household_of(index).is_none()
                && s_no_roles_initial.stock_of(index, no_role_bread) > 0
        })
        .count();
    assert!(
        seeded_woodcutters > 0,
        "the collapsed-role control must seed bread on the WOOD-seller group"
    );
    let s_no_roles = run(&no_roles, RUN_TICKS);
    assert!(
        s_no_roles.promoted_at_tick().is_none(),
        "without role separation SALT still does not promote"
    );
    let no_role_wood_givers = s_no_roles.barter_givers_of(WOOD);
    assert!(
        !no_role_wood_givers.is_empty(),
        "the collapsed-role control must be non-vacuous: collapsed producers do sell WOOD"
    );
    let no_role_bread_givers = s_no_roles.barter_givers_of(no_role_bread);
    for giver in &no_role_wood_givers {
        let index = (0..s_no_roles.population())
            .find(|&i| s_no_roles.colonist_id(i) == Some(*giver))
            .expect("a live WOOD giver maps to a generation index");
        if s_no_roles.vocation_of(index) == Some(Vocation::Gatherer)
            && s_no_roles.household_of(index).is_none()
        {
            assert!(
                !no_role_bread_givers.contains(giver),
                "a collapsed WOOD seller that also held bread must have its bread preempted by WOOD"
            );
        }
    }
    let no_role_targets = s_no_roles.indirect_target_goods(salt_good(&no_roles));
    assert!(
        !(no_role_targets.contains(&no_role_bread) && no_role_targets.contains(&WOOD)),
        "without role separation the two-sided indirect breadth must not form: {no_role_targets:?}"
    );
}

// ---- 9. the gate holds: existing goldens are byte-identical --------------

#[test]
fn goldens_unchanged() {
    // With the S18 scenario absent, the additive + gated changes leave every existing
    // identity untouched. The `multigood_money` flag emits its canonical marker only when
    // active (covered by the settlement unit test `canonical_bytes_include_multigood_money`),
    // and the runtime-only instrumentation is excluded from `canonical_bytes` (covered by
    // `canonical_bytes_exclude_multigood_instrumentation`). The cross-scenario golden digests
    // (the `lineages` + `g4a_death` tripwires, the S5–S17 + econ + emergence goldens) live in
    // their own suites (`forage_carrying_capacity`, `g4a_death`, the econ/emergence tests) and
    // stay green; this test pins the two key demographic/death tripwires directly.
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
    // The g4a_death no-death golden (the live-starvation tripwire family).
    let mut viable = Settlement::generate(0xC0FFEE, &SettlementConfig::viable());
    viable.run(60);
    assert_eq!(
        viable.digest(),
        0xa174_8567_db1c_4341,
        "the g4a_death no-death golden must be byte-identical"
    );
}
