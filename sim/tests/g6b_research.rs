//! G6b acceptance suite — **research & tech tiers** (capabilities are earned by
//! research, not unlocked by a timer).
//!
//! G6a names the era a society has *earned*; G6b lets it *advance its capabilities*:
//! a **scholar** vocation produces **Knowledge** from labor, and crossing a Knowledge
//! threshold **unlocks a higher tech tier** — a recipe gated `enabled: false` until
//! then. The milestone proves the MECHANISM for ONE tier unlock (tier 1 → tier 2)
//! with seeded scholars, plus a **control** (no scholars → no unlock). The DoD is
//! research-driven progression, not a tuned magnitude:
//!
//! - the run is deterministic — same `(seed, config)` → byte-identical, including the
//!   Knowledge accumulation and the unlock tick (test 1);
//! - scholar labor accumulates Knowledge, it crosses the threshold, the tier-2 recipe
//!   flips enabled at a definite tick, and afterward the tier-2 good is produced —
//!   impossible before (test 2);
//! - the no-scholars control never accumulates Knowledge, so the tier never unlocks
//!   and the tier-2 good is never produced (test 3, the falsification twin);
//! - good INPUTS to research are conserved-consumed and accounted; whole-system goods
//!   conservation holds; Knowledge is reported on its own non-conserved line, OUTSIDE
//!   the goods ledger (test 4, the tripwire);
//! - before the unlock the tier-2 recipe cannot run even while a producer holds its
//!   inputs (test 5, the tier gate);
//! - the unlock is one-way — once unlocked it never flaps (test 6);
//! - econ behaviour is unchanged — the conformance scenarios replay byte-identically
//!   (test 7).
//!
//! Multi-tier trees, knowledge diffusion via trade, building-defs, and scholar-role
//! emergence are deferred (see `docs/engine-divergence.md`). The full-workspace
//! `cargo test`, `cargo clippy --workspace --all-targets -- -D warnings`, and
//! `cargo fmt --check` run outside this file.

use econ::good::GoodId;
use sim::{EconTickReport, Settlement, SettlementConfig, Vocation};

/// A horizon comfortably past the seeded threshold's unlock (≈tick 4 for these
/// configs) with many ticks of tier-2 production afterward. The tests assert the
/// unlock *happens* and *is earned by research*, never a hand-tuned tick.
const RUN_TICKS: u64 = 48;

/// The witness seed the deterministic tests pin.
const SEED: u64 = 6_206;

/// The research config and its no-scholars control.
fn research() -> SettlementConfig {
    SettlementConfig::research()
}
fn control() -> SettlementConfig {
    SettlementConfig::research_control()
}
fn research_with_threshold(threshold: u64) -> SettlementConfig {
    let mut config = research();
    config
        .chain
        .as_mut()
        .expect("research config carries a chain")
        .tier2_threshold = threshold;
    config
}

/// The Knowledge good id (interned, but NOT a conserved good).
fn knowledge_good(s: &Settlement) -> GoodId {
    s.content()
        .expect("a research settlement has content")
        .knowledge()
        .expect("the research content interns Knowledge")
}

/// The tier-2 (pastry) good id — a conserved, tracked good.
fn pastry_good(s: &Settlement) -> GoodId {
    s.content()
        .expect("a research settlement has content")
        .pastry()
        .expect("the research content interns pastry")
}

/// The grain good id — the conserved research (and milling) input.
fn grain_good(s: &Settlement) -> GoodId {
    s.content()
        .expect("a research settlement has content")
        .grain()
}

/// 1. `research_run_is_deterministic` — same `(seed, config)` → byte-identical run,
///    Knowledge accumulation and the unlock tick included. Nothing is drawn in the
///    loops (the `Rng` is consumed only at generation), so two runs stay in lockstep;
///    a different seed diverges. The proof is non-vacuous: the run really crossed the
///    threshold and unlocked tier 2.
#[test]
fn research_run_is_deterministic() {
    let config = research();

    let mut a = Settlement::generate(SEED, &config);
    let mut b = Settlement::generate(SEED, &config);
    a.run(RUN_TICKS);
    b.run(RUN_TICKS);
    assert_eq!(
        a.canonical_bytes(),
        b.canonical_bytes(),
        "same seed diverged"
    );
    assert_eq!(a.digest(), b.digest());

    // Non-vacuous: research actually accumulated Knowledge and unlocked the tier.
    assert!(
        a.knowledge() > 0,
        "no Knowledge accumulated — proof is shallow"
    );
    assert_eq!(
        a.current_tier(),
        2,
        "tier 2 never unlocked — proof is shallow"
    );
    assert!(a.tier2_unlocked_at().is_some());

    // A different seed yields a different run (generation draws cultures, which steer
    // the market and so the byte-state), but the unlock is just as deterministic.
    let mut c = Settlement::generate(0xD1FF, &config);
    let mut d = Settlement::generate(0xD1FF, &config);
    c.run(RUN_TICKS);
    d.run(RUN_TICKS);
    assert_eq!(c.canonical_bytes(), d.canonical_bytes());
    assert_ne!(
        a.canonical_bytes(),
        c.canonical_bytes(),
        "the seed did not change the run"
    );
}

/// Unit: the Knowledge threshold is future-steering canonical state even before
/// either settlement unlocks. Otherwise two byte-identical pre-unlock states can
/// diverge on the next ticks solely because one crosses the threshold earlier.
#[test]
fn research_threshold_is_part_of_canonical_state() {
    let lower = Settlement::generate(SEED, &research_with_threshold(20));
    let higher = Settlement::generate(SEED, &research_with_threshold(21));

    assert_eq!(lower.knowledge(), 0);
    assert_eq!(higher.knowledge(), 0);
    assert_eq!(lower.tier2_unlocked_at(), None);
    assert_eq!(higher.tier2_unlocked_at(), None);
    assert!(!lower.tier2_recipe_enabled());
    assert!(!higher.tier2_recipe_enabled());
    assert_ne!(
        lower.canonical_bytes(),
        higher.canonical_bytes(),
        "research thresholds that steer future unlock ticks must not digest equal"
    );
    assert_ne!(lower.digest(), higher.digest());
}

/// 2. `research_unlocks_tier_two` — scholar labor accumulates Knowledge, it crosses
///    the threshold, the tier-2 recipe flips to enabled at a DEFINITE tick, and
///    afterward the tier-2 good (pastry) is produced — which was impossible before.
#[test]
fn research_unlocks_tier_two() {
    let mut s = Settlement::generate(SEED, &research());
    let pastry = pastry_good(&s);
    let threshold = s.tier2_threshold();
    assert!(
        threshold > 0,
        "the research config must set a tier-2 threshold"
    );

    // Tier 1, recipe gated, no Knowledge at generation.
    assert_eq!(s.current_tier(), 1);
    assert!(!s.tier2_recipe_enabled());
    assert_eq!(s.knowledge(), 0);

    let mut unlock_tick = None;
    let mut pastry_before_unlock = 0u64;
    let mut pastry_after_unlock = 0u64;
    let mut prev_knowledge = 0u64;
    for _ in 0..RUN_TICKS {
        let report = s.econ_tick();
        // Knowledge is monotonic (an accumulator).
        assert!(
            s.knowledge() >= prev_knowledge,
            "Knowledge decreased — it must be a monotonic accumulator"
        );
        prev_knowledge = s.knowledge();

        let produced = report.produced_of(pastry);
        match s.tier2_unlocked_at() {
            None => {
                pastry_before_unlock += produced;
            }
            Some(tick) => {
                unlock_tick.get_or_insert(tick);
                pastry_after_unlock += produced;
            }
        }
    }

    let unlock_tick = unlock_tick.expect("research crossed the threshold and unlocked tier 2");
    // The unlock is earned: Knowledge had to reach the threshold to flip the gate.
    assert!(
        s.knowledge() >= threshold,
        "tier 2 unlocked at {unlock_tick} but Knowledge ({}) never reached the threshold ({threshold})",
        s.knowledge()
    );
    assert_eq!(s.current_tier(), 2);
    assert!(
        s.tier2_recipe_enabled(),
        "the tier-2 recipe is enabled after unlock"
    );

    // The tier-2 good was IMPOSSIBLE before the unlock and is produced after it.
    assert_eq!(
        pastry_before_unlock, 0,
        "the tier-2 good was produced before the unlock — the gate leaked"
    );
    assert!(
        pastry_after_unlock > 0,
        "no tier-2 good produced after the unlock — the unlock is vacuous"
    );
    // And it really accumulated in the whole system (a higher-order good now exists).
    assert!(s.whole_system_total(pastry) > 0);
}

/// 3. `no_scholars_control_never_unlocks` — the falsification twin. With the scholars
///    removed, Knowledge never accumulates, so the tier-2 recipe stays disabled and
///    the tier-2 good is never produced — even though the confectioner (the would-be
///    producer) is present and holds its flour input throughout. Paired with test 2
///    this shows research, not time, drives the unlock.
#[test]
fn no_scholars_control_never_unlocks() {
    let mut s = Settlement::generate(SEED, &control());
    let pastry = pastry_good(&s);

    // The control has NO scholars but DOES keep the confectioner (the would-be
    // producer) — so any unlock could only come from research, which is absent.
    assert_eq!(
        s.vocation_count(Vocation::Scholar),
        0,
        "the control has no scholars"
    );
    assert!(
        s.vocation_count(Vocation::Confectioner) > 0,
        "the control must keep the confectioner — else the gate test is vacuous"
    );

    let mut total_pastry = 0u64;
    for _ in 0..RUN_TICKS {
        let report = s.econ_tick();
        total_pastry += report.produced_of(pastry);
        // The invariants hold every single tick, not just at the end.
        assert_eq!(s.knowledge(), 0, "Knowledge accumulated with no scholars");
        assert!(
            !s.tier2_recipe_enabled(),
            "the tier-2 recipe enabled with no research"
        );
        assert_eq!(s.current_tier(), 1, "the tier advanced with no research");
    }

    assert_eq!(s.knowledge(), 0);
    assert_eq!(
        s.tier2_unlocked_at(),
        None,
        "tier 2 unlocked without any research"
    );
    assert_eq!(
        total_pastry, 0,
        "the tier-2 good was produced with no unlock"
    );
    assert_eq!(s.whole_system_total(pastry), 0);
}

/// 4. `research_inputs_conserve` — the conservation tripwire. Good INPUTS to research
///    (grain) are conserved-consumed and accounted in `consumed_as_input`, so
///    whole-system goods conservation holds every tick. Knowledge, by contrast, is
///    reported on its OWN non-conserved line (`knowledge_produced`): it is never a
///    tracked good, never in the `produced` ledger, and the per-tick line sums exactly
///    to the accumulated counter.
#[test]
fn research_inputs_conserve() {
    let mut s = Settlement::generate(SEED, &research());
    let knowledge = knowledge_good(&s);
    let grain = grain_good(&s);

    // Knowledge is interned (so the recipe names it) but is NOT a conserved good.
    assert!(
        !s.tracked_goods().contains(&knowledge),
        "Knowledge must be outside the goods-conservation ledger"
    );

    let mut grain_consumed_as_input = 0u64;
    let mut knowledge_line_total = 0u64;
    for _ in 0..RUN_TICKS {
        let report = s.econ_tick();
        // Whole-system goods conservation holds for every tracked good, every tick.
        assert!(
            report.conserves(),
            "whole-system conservation broke at tick {}",
            report.econ_tick
        );
        // Knowledge never appears in the conserved goods `produced` ledger — it lives
        // on its own non-conserved line.
        assert_eq!(
            report.produced_of(knowledge),
            0,
            "Knowledge leaked into the conserved-goods produced ledger"
        );
        grain_consumed_as_input += report.consumed_as_input_of(grain);
        knowledge_line_total += report.knowledge_produced();
    }

    // Research (and milling) really consumed conserved grain — accounted, not lost.
    assert!(
        grain_consumed_as_input > 0,
        "no grain consumed as a recipe input — conservation claim is vacuous"
    );
    // The non-conserved Knowledge line sums EXACTLY to the accumulated counter.
    assert!(
        knowledge_line_total > 0,
        "no Knowledge produced — proof is shallow"
    );
    assert_eq!(
        knowledge_line_total,
        s.knowledge(),
        "the per-tick knowledge_produced line does not sum to the accumulator"
    );
}

/// 5. `tier_gate_blocks_pre_unlock` — before the unlock, the tier-2 recipe is
///    `enabled: false` and cannot run, so NO tier-2 good is produced — even though the
///    confectioner holds its flour input the whole time (the gate, not a missing
///    input, is what blocks production).
#[test]
fn tier_gate_blocks_pre_unlock() {
    let mut s = Settlement::generate(SEED, &research());
    let pastry = pastry_good(&s);
    let flour = s.content().unwrap().flour();

    // The confectioner holds flour from generation (a would-be producer with inputs).
    let confectioner = (0..s.population())
        .find(|&i| s.vocation_of(i) == Some(Vocation::Confectioner))
        .expect("the research config seeds a confectioner");
    let confectioner_id = s.colonist_id(confectioner).unwrap();
    let holds_flour = |s: &Settlement| {
        s.society()
            .agents
            .get(confectioner_id)
            .map_or(0, |a| a.stock.get(flour))
    };
    assert!(
        holds_flour(&s) > 0,
        "the confectioner must hold its flour input"
    );

    let mut saw_gate_block = false;
    for _ in 0..RUN_TICKS {
        let before_unlock = s.tier2_unlocked_at().is_none();
        let report = s.econ_tick();
        if before_unlock {
            // While gated: recipe disabled, no pastry produced — yet flour is on hand.
            assert!(!s.tier2_recipe_enabled() || s.tier2_unlocked_at() == Some(report.econ_tick));
            if s.tier2_unlocked_at().is_none() {
                assert_eq!(
                    report.produced_of(pastry),
                    0,
                    "the tier-2 recipe ran while gated (tick {})",
                    report.econ_tick
                );
                assert!(
                    holds_flour(&s) > 0,
                    "the confectioner ran out of its (unused) flour input while gated"
                );
                saw_gate_block = true;
            }
        }
    }

    assert!(
        saw_gate_block,
        "the run never exercised the pre-unlock gate"
    );
    // And after the unlock the gate is open and the good flows (paired with the block).
    assert!(s.tier2_recipe_enabled());
    assert!(s.whole_system_total(pastry) > 0);
}

/// 6. `unlock_is_one_way` — once tier 2 unlocks it stays unlocked: the unlock tick
///    never changes, the recipe never re-disables, and the tier never regresses — even
///    after research stops (the scholars exhaust their grain buffer). No flapping.
#[test]
fn unlock_is_one_way() {
    let mut s = Settlement::generate(SEED, &research());

    // Run until the tier unlocks, recording the unlock tick.
    let mut ticks_run = 0u64;
    while s.tier2_unlocked_at().is_none() && ticks_run < RUN_TICKS {
        s.econ_tick();
        ticks_run += 1;
    }
    let unlock_tick = s
        .tier2_unlocked_at()
        .expect("tier 2 unlocked within the horizon");

    // Run a long tail well past the point research could still matter; the unlock
    // never moves and the tier never regresses, tick by tick.
    let mut prev_knowledge = s.knowledge();
    for _ in 0..RUN_TICKS {
        s.econ_tick();
        assert_eq!(
            s.tier2_unlocked_at(),
            Some(unlock_tick),
            "the unlock tick changed — the unlock is not one-way"
        );
        assert!(
            s.tier2_recipe_enabled(),
            "the tier-2 recipe re-disabled — it flapped"
        );
        assert_eq!(s.current_tier(), 2, "the tier regressed");
        // Knowledge is monotonic across the tail too (never un-accumulates).
        assert!(s.knowledge() >= prev_knowledge);
        prev_knowledge = s.knowledge();
    }
}

/// 7. `econ_unchanged` — the engine's conformance scenarios replay byte-identically:
///    G6b's research/tiers live entirely in `sim` (scholars, the Knowledge counter,
///    the per-settlement tier state) and the only econ touch is an additive accessor
///    (`set_recipe_enabled`) called by no engine path, so the econ goldens are
///    byte-identical by construction. A plain settlement is also byte-identical whether
///    or not a research overlay exists elsewhere.
#[test]
fn econ_unchanged() {
    use econ::scenario::{builtin_market_scenario, ScenarioName};
    use econ::society::Society;

    for name in [
        ScenarioName::MarketBarterishGold,
        ScenarioName::MarketPriceDiscovery,
        ScenarioName::MengerSaltMoney,
        ScenarioName::MengerGoldMoney,
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
        assert_eq!(
            first.v2_records, second.v2_records,
            "{name:?} V2 records diverged"
        );
        if matches!(name, ScenarioName::MarketBarterishGold) {
            assert_eq!(
                first.total_gold(),
                total_gold,
                "{name:?} broke gold conservation"
            );
        }
    }

    // The existing seeded chain (no research) is byte-identical run twice — the G6b
    // additions are gated on a research chain, so a plain chain config is untouched.
    let mut a = Settlement::generate(7, &SettlementConfig::grain_flour_bread_chain());
    let mut b = Settlement::generate(7, &SettlementConfig::grain_flour_bread_chain());
    a.run(40);
    b.run(40);
    assert_eq!(a.digest(), b.digest());
    // A plain chain is tier 1 with no research surface.
    assert!(!a.is_research());
    assert_eq!(a.current_tier(), 1);
    assert_eq!(a.knowledge(), 0);
}

/// Unit: at generation a research settlement is tier 1 with Knowledge zero and the
/// tier-2 recipe gated; the control is identical but seeds no scholars.
#[test]
fn research_settlement_starts_gated() {
    let s = Settlement::generate(SEED, &research());
    assert!(s.is_research());
    assert_eq!(s.current_tier(), 1);
    assert_eq!(s.knowledge(), 0);
    assert_eq!(s.tier2_unlocked_at(), None);
    assert!(!s.tier2_recipe_enabled());
    assert!(s.vocation_count(Vocation::Scholar) > 0);
    assert!(s.vocation_count(Vocation::Confectioner) > 0);

    let c = Settlement::generate(SEED, &control());
    assert!(c.is_research());
    assert_eq!(c.vocation_count(Vocation::Scholar), 0);
    assert!(c.vocation_count(Vocation::Confectioner) > 0);
}

/// Unit: the Knowledge accumulator lives OUTSIDE the goods ledger. The good is
/// interned (the research recipe names it) but is neither tracked for conservation
/// nor part of the content's conserved goods set, while the conserved tier-2 good
/// (pastry) and the durable tools ARE tracked.
#[test]
fn knowledge_is_outside_the_goods_ledger() {
    let s = Settlement::generate(SEED, &research());
    let content = s.content().unwrap();
    let knowledge = content.knowledge().unwrap();
    let pastry = content.pastry().unwrap();
    let library = content.library().unwrap();
    let atelier = content.atelier().unwrap();

    assert!(
        !s.tracked_goods().contains(&knowledge),
        "Knowledge must not be a tracked (conserved) good"
    );
    assert!(!content.goods().contains(&knowledge));
    // The tier-2 good and the durable research/tier-2 tools ARE conserved goods.
    assert!(s.tracked_goods().contains(&pastry));
    assert!(s.tracked_goods().contains(&library));
    assert!(s.tracked_goods().contains(&atelier));
}

/// Unit: the `EconTickReport` carries Knowledge on its own line and a default report
/// (a plain settlement) reports zero, with no goods-ledger entry for Knowledge.
#[test]
fn knowledge_produced_line_defaults_to_zero() {
    let report = EconTickReport::default();
    assert_eq!(report.knowledge_produced(), 0);
    assert!(report.conserves(), "an empty report trivially conserves");
}
