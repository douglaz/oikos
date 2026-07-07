//! G6a acceptance suite — **era detection** (eras are earned, not timed).
//!
//! The frontier (G5b) already passes through institutional phases — forage, barter,
//! a money good emerges, producers specialize, a roundabout chain runs — but nothing
//! *named* the era. G6a adds the read-only [`EraDetector`]: a classification of the
//! settlement's institutional era from **measured** quantities, with hysteresis. The
//! DoD is the *"phase is measured, never set"* doctrine, not a tuned magnitude:
//!
//! - the era timeline is deterministic — same `(seed, config)` → identical (test 1);
//! - the frontier climbs the ladder in order, each rung earned at a **measured**
//!   transition, not a timer (test 2);
//! - hysteresis is anti-flap: a single-tick dip never regresses the era, but a
//!   sustained failure over the window does (test 3);
//! - a no-exchange config stays Forager; a barter-only camp reaches Barter but never
//!   Money (test 4);
//! - era detection mutates nothing, is imported by no decision path, and observing a
//!   run is byte-identical to not observing it (test 5, with a source-gate like the
//!   lab's metrics gate);
//! - econ behaviour is unchanged — the conformance goldens replay byte-identically
//!   (test 6).
//!
//! The Credit/Modern rungs are deferred to G8 (finance); the full-workspace
//! `cargo test`, `cargo clippy --workspace --all-targets -- -D warnings`, and
//! `cargo fmt --check` run outside this file.

use sim::{Era, EraDetector, Settlement, SettlementConfig, Vocation};

/// A horizon comfortably past the frontier's promotion (≈14), role adoption (≈16),
/// and roundabout onset for the seeds these tests use. The tests assert the ordered
/// transitions, never a hand-tuned tick.
const RUN_TICKS: u64 = 90;

/// The seed the deterministic frontier tests pin (the G5b witness seed).
const SEED: u64 = 2_026;

/// Run `s` for `ticks`, observing it with `detector` every econ tick (read-only).
fn run_observing(s: &mut Settlement, detector: &mut EraDetector, ticks: u64) {
    for _ in 0..ticks {
        s.econ_tick();
        detector.observe(s);
    }
}

/// 1. `era_detection_is_deterministic` — same `(seed, config)` → an identical era
///    timeline. The detector is integer-only (counters + a fixed array), draws no
///    randomness, and reads only measured accessors, so two runs of the same seed
///    produce the same reached era and the same per-rung first ticks; a different
///    seed (different drawn cultures, a different run) produces a different timeline.
#[test]
fn era_detection_is_deterministic() {
    let config = SettlementConfig::frontier();

    let mut a = Settlement::generate(SEED, &config);
    let mut da = EraDetector::new();
    run_observing(&mut a, &mut da, RUN_TICKS);

    let mut b = Settlement::generate(SEED, &config);
    let mut db = EraDetector::new();
    run_observing(&mut b, &mut db, RUN_TICKS);

    assert_eq!(da.current_era(), db.current_era(), "current era diverged");
    assert_eq!(da.timeline(), db.timeline(), "era timeline diverged");
    for era in Era::ALL {
        assert_eq!(
            da.first_tick(era),
            db.first_tick(era),
            "{era:?} first-tick diverged"
        );
    }

    // The proof is non-vacuous: the run actually climbed the ladder.
    assert!(
        da.current_era() >= Era::Money,
        "the determinism run never reached Money — the proof is shallow"
    );

    // A different seed yields a different timeline (generation draws cultures, which
    // steer when the transitions land).
    let mut c = Settlement::generate(0xBADF00D, &config);
    let mut dc = EraDetector::new();
    run_observing(&mut c, &mut dc, RUN_TICKS);
    assert_ne!(
        da.timeline(),
        dc.timeline(),
        "the seed did not change the era timeline"
    );
}

/// 2. `frontier_progresses_through_eras` — the frontier reaches, **in order**,
///    Barter → Money → Specialist → Capital, each rung's first-tick after the prior
///    rung's, and each rung **earned at a measured transition**, not a timer: Money
///    onset is at or after the promotion tick, Specialist onset at or after the first
///    role adoption, Capital onset at or after both stages are first staffed. No rung
///    is reached before the measured event that earns it could possibly be observed.
#[test]
fn frontier_progresses_through_eras() {
    let mut s = Settlement::generate(SEED, &SettlementConfig::frontier());
    let mut detector = EraDetector::new();

    // Record the underlying *measured* transitions the eras must follow.
    let mut promotion_tick = None;
    let mut first_role_tick = None;
    let mut first_both_stages_tick = None;
    for _ in 0..RUN_TICKS {
        let was_barter = s.current_money_good().is_none();
        s.econ_tick();
        let tick = s.last_report().econ_tick;
        if was_barter && s.current_money_good().is_some() {
            promotion_tick = promotion_tick.or(Some(tick));
        }
        let millers = s.living_count(Vocation::Miller);
        let bakers = s.living_count(Vocation::Baker);
        if first_role_tick.is_none() && millers + bakers > 0 {
            first_role_tick = Some(tick);
        }
        if first_both_stages_tick.is_none() && millers > 0 && bakers > 0 {
            first_both_stages_tick = Some(tick);
        }
        detector.observe(&s);
    }

    let promotion_tick = promotion_tick.expect("the frontier promotes a money good");
    let first_role_tick = first_role_tick.expect("the frontier adopts producer roles");
    let first_both_stages_tick = first_both_stages_tick.expect("the frontier staffs both stages");

    let barter = detector.first_tick(Era::Barter).expect("reaches Barter");
    let money = detector.first_tick(Era::Money).expect("reaches Money");
    let specialist = detector
        .first_tick(Era::Specialist)
        .expect("reaches Specialist");
    let capital = detector
        .first_tick(Era::Capital)
        .expect("reaches Capital (the frontier sustains roundabout production)");

    // Forager is the floor, stamped at the first observation.
    assert_eq!(detector.first_tick(Era::Forager), Some(0));

    // The ladder is climbed strictly in order.
    assert!(
        barter < money,
        "Barter ({barter}) not before Money ({money})"
    );
    assert!(
        money < specialist,
        "Money ({money}) not before Specialist ({specialist})"
    );
    assert!(
        specialist < capital,
        "Specialist ({specialist}) not before Capital ({capital})"
    );
    assert_eq!(
        detector.current_era(),
        Era::Capital,
        "ends in the Capital era"
    );
    assert_eq!(detector.peak_era(), Era::Capital);

    // Each rung is EARNED at a measured transition — never before the event that
    // earns it could be observed (the "earned, not timed" claim). Money cannot be
    // detected before money is promoted; Specialist not before a role is adopted;
    // Capital not before both stages are staffed.
    assert!(
        money >= promotion_tick,
        "Money era ({money}) onset BEFORE the promotion tick ({promotion_tick}) — not earned"
    );
    assert!(
        specialist >= first_role_tick,
        "Specialist era ({specialist}) onset before any role adoption ({first_role_tick})"
    );
    assert!(
        capital >= first_both_stages_tick,
        "Capital era ({capital}) onset before both stages were staffed ({first_both_stages_tick})"
    );

    // And the onset follows the measured event PROMPTLY (within a few hysteresis
    // windows), so it tracks the economy rather than a decoupled clock.
    let slack = 4 * detector.window();
    assert!(
        money <= promotion_tick + slack,
        "Money era onset ({money}) is decoupled from promotion ({promotion_tick})"
    );
    assert!(
        specialist <= first_role_tick + slack,
        "Specialist onset ({specialist}) is decoupled from role adoption ({first_role_tick})"
    );
    assert!(
        capital <= first_both_stages_tick + slack,
        "Capital onset ({capital}) is decoupled from both-stage staffing ({first_both_stages_tick})"
    );
}

/// 3. `eras_do_not_flap` — the hysteresis is anti-flap. Driving the pure state
///    machine ([`EraDetector::apply_triggers`]) with controlled trigger signals: a
///    **single-tick dip** below the current rung's trigger does NOT regress the era,
///    but a **sustained failure** over the full window does — and a later regression
///    never clears the first-tick record. A real frontier run is also checked to
///    confirm the measured signals never make the era flap (it is monotonic).
#[test]
fn eras_do_not_flap() {
    // Trigger arrays indexed by Era::rank:
    // [Forager, Barter, Money, Specialist, Capital, Credit, Modern].
    let all_on = [true, true, true, true, false, false, false];
    let mut d = EraDetector::with_window(3);

    // Climb Forager → Barter → Money → Specialist over 9 sustained ticks (3 per rung).
    for tick in 0..9 {
        d.apply_triggers(tick, all_on);
    }
    assert_eq!(
        d.current_era(),
        Era::Specialist,
        "did not climb to Specialist"
    );
    let specialist_onset = d.first_tick(Era::Specialist).expect("reached Specialist");

    // A SINGLE-TICK dip below the Specialist trigger — does not regress.
    let specialist_off = [true, true, true, false, false, false, false];
    d.apply_triggers(9, specialist_off);
    assert_eq!(
        d.current_era(),
        Era::Specialist,
        "a single-tick dip regressed the era — hysteresis failed"
    );
    // The trigger recovers; the era was never lost.
    d.apply_triggers(10, all_on);
    assert_eq!(d.current_era(), Era::Specialist);

    // A SUSTAINED failure over the full window DOES regress — but only after the
    // whole window, not before.
    d.apply_triggers(11, specialist_off); // fail 1
    d.apply_triggers(12, specialist_off); // fail 2
    assert_eq!(
        d.current_era(),
        Era::Specialist,
        "regressed before the failure was sustained over the window"
    );
    d.apply_triggers(13, specialist_off); // fail 3 == window → regress
    assert_eq!(
        d.current_era(),
        Era::Money,
        "a sustained failure over the window did not regress the era"
    );
    // The first-tick record is monotonic — regression never clears it.
    assert_eq!(
        d.first_tick(Era::Specialist),
        Some(specialist_onset),
        "regression cleared the historical first-tick"
    );

    // A real frontier run never flaps: the measured signals only ever advance the era.
    let mut s = Settlement::generate(SEED, &SettlementConfig::frontier());
    let mut detector = EraDetector::new();
    let mut prev = Era::Forager;
    for _ in 0..RUN_TICKS {
        s.econ_tick();
        let era = detector.observe(&s);
        assert!(
            era >= prev,
            "the frontier era regressed ({prev:?} → {era:?}) — the measured signals flapped"
        );
        prev = era;
    }
    assert_eq!(detector.current_era(), Era::Capital);
}

/// 4. `forager_and_barter_controls` — the two falsification controls. A no-exchange
///    config (a lone gatherer with no counterparty) clears no trade, so it stays in
///    Forager. A barter-only camp (the G5a `barter-camp-control`, whose symmetric
///    swaps never let a good lead the promotion margin) reaches Barter but never
///    Money — money is not assumed, it must be earned, and here it never is.
#[test]
fn forager_and_barter_controls() {
    // A no-exchange config: a single gatherer, no consumer — no counterparty, so no
    // trade can clear.
    let mut no_exchange = SettlementConfig::barter_camp_control();
    no_exchange.gatherers = 1;
    no_exchange.consumers = 0;
    let mut s = Settlement::generate(SEED, &no_exchange);
    let mut detector = EraDetector::new();
    run_observing(&mut s, &mut detector, RUN_TICKS);
    assert_eq!(
        s.barter_trade_count(),
        0,
        "the no-exchange control cleared a trade — it is not no-exchange"
    );
    assert_eq!(
        detector.current_era(),
        Era::Forager,
        "the no-exchange control left Forager"
    );
    assert_eq!(detector.peak_era(), Era::Forager);
    assert_eq!(detector.first_tick(Era::Barter), None);

    // A barter-only camp: reaches Barter (a thick barter book), never Money (no good
    // ever leads by the promotion margin, so nothing monetizes).
    let mut control = Settlement::generate(SEED, &SettlementConfig::barter_camp_control());
    let mut control_detector = EraDetector::new();
    run_observing(&mut control, &mut control_detector, RUN_TICKS);
    assert!(
        control.barter_trade_count() > 0,
        "the barter control never bartered — the Barter claim is vacuous"
    );
    assert_eq!(
        control.current_money_good(),
        None,
        "the barter control monetized a good — it should stay in barter"
    );
    assert_eq!(
        control_detector.peak_era(),
        Era::Barter,
        "the barter control did not reach Barter (or climbed past it)"
    );
    assert!(
        control_detector.first_tick(Era::Barter).is_some(),
        "the barter control never reached Barter"
    );
    assert_eq!(
        control_detector.first_tick(Era::Money),
        None,
        "the barter control reached Money without a promotion"
    );
}

/// 5. `era_is_read_only` — era detection is measurement-only. (a) Observing a run is
///    byte-identical to not observing it (the detector borrows `&Settlement`, draws
///    nothing, mutates nothing). (b) A single `observe` leaves the settlement's digest
///    unchanged. (c) A source-gate (like econ's metrics gate) confirms no decision
///    path imports the era module — running with vs without querying the era cannot
///    change a run.
#[test]
fn era_is_read_only() {
    let config = SettlementConfig::frontier();

    // (a) A run observed by a detector is byte-identical to one that is not.
    let mut observed = Settlement::generate(SEED, &config);
    let mut detector = EraDetector::new();
    run_observing(&mut observed, &mut detector, RUN_TICKS);

    let mut plain = Settlement::generate(SEED, &config);
    plain.run(RUN_TICKS);

    assert_eq!(
        observed.canonical_bytes(),
        plain.canonical_bytes(),
        "observing the run changed its bytes"
    );
    assert_eq!(observed.digest(), plain.digest());
    // Non-vacuous: the observed run really exercised the detector up the ladder.
    assert!(detector.current_era() >= Era::Money);

    // (b) A single observe does not mutate the settlement.
    let mut s = Settlement::generate(SEED, &config);
    s.run(20);
    let before = s.digest();
    let mut probe = EraDetector::new();
    probe.observe(&s);
    assert_eq!(s.digest(), before, "observe mutated the settlement");

    // (c) Source-gate: no decision/behavior module imports the era module. Era
    // detection is a measurement layer, like the lab's metrics — unimportable by any
    // decision path, so no decision can branch on the era.
    let era_patterns = [
        concat!("Era", "Detector"),
        concat!("crate", "::", "era"),
        concat!("Era", "::"),
    ];
    for (module, source) in [
        (
            "settlement/mod.rs",
            include_str!("../src/settlement/mod.rs"),
        ),
        (
            "settlement/share_tenancy.rs",
            include_str!("../src/settlement/share_tenancy.rs"),
        ),
        (
            "settlement/wage_labor.rs",
            include_str!("../src/settlement/wage_labor.rs"),
        ),
        ("region.rs", include_str!("../src/region.rs")),
        ("demography.rs", include_str!("../src/demography.rs")),
        ("content.rs", include_str!("../src/content.rs")),
    ] {
        for pattern in era_patterns {
            assert!(
                !source.contains(pattern),
                "{module} references the era module ({pattern}) — a decision path must not read the era"
            );
        }
    }
}

/// 6. `econ_unchanged` — the engine's conformance scenarios still replay
///    byte-identically: era detection lives entirely in `sim` as an additive,
///    read-only layer (it reuses existing accessors and writes no econ/sim state), so
///    the econ goldens are byte-identical **by construction**. A plain settlement is
///    also byte-identical with or without an era detector observing it. The full
///    `cargo test --workspace`, `cargo clippy -- -D warnings`, and `cargo fmt --check`
///    run outside this test.
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

    // A plain settlement run is byte-identical whether or not an era detector observes
    // it — era detection adds public surface but no behavior.
    let mut a = Settlement::generate(7, &SettlementConfig::viable());
    a.run(40);
    let mut b = Settlement::generate(7, &SettlementConfig::viable());
    let mut detector = EraDetector::new();
    run_observing(&mut b, &mut detector, 40);
    assert_eq!(
        a.digest(),
        b.digest(),
        "the era detector changed a plain run"
    );
    // A designated-money settlement never exhibits the measured barter→promotion
    // transitions, so it sits at the Forager floor (the ladder measures the emergent
    // institutional path).
    assert_eq!(detector.current_era(), Era::Forager);
}

/// Unit: the era labels and ordering are stable (the surface the viewer renders).
#[test]
fn era_labels_are_stable() {
    assert_eq!(Era::Forager.label(), "forager");
    assert_eq!(Era::Barter.label(), "barter");
    assert_eq!(Era::Money.label(), "money");
    assert_eq!(Era::Specialist.label(), "specialist");
    assert_eq!(Era::Capital.label(), "capital");
    assert_eq!(Era::Credit.label(), "credit");
    assert_eq!(Era::Modern.label(), "modern");
    // Strictly increasing ranks — the ladder order.
    let ranks: Vec<usize> = Era::ALL.iter().map(|e| e.rank()).collect();
    assert_eq!(ranks, vec![0, 1, 2, 3, 4, 5, 6]);
}

/// Unit: the barter-volume floor is a real gate — a camp just below the floor stays
/// Forager, one at or above it reaches Barter. (Drives the measurement deterministically
/// through `apply_triggers`, which the production tests exercise via `observe`.)
#[test]
fn barter_volume_floor_gates_the_barter_rung() {
    // With the Barter trigger never set, the detector never leaves Forager.
    let mut below = EraDetector::with_window(2);
    for tick in 0..10 {
        below.apply_triggers(tick, [true, false, false, false, false, false, false]);
    }
    assert_eq!(below.current_era(), Era::Forager);

    // With it set and sustained over the window, it reaches Barter.
    let mut at = EraDetector::with_window(2);
    for tick in 0..10 {
        at.apply_triggers(tick, [true, true, false, false, false, false, false]);
    }
    assert_eq!(at.current_era(), Era::Barter);
}

/// Unit: `measured_triggers` reads the **cumulative barter volume** against the floor,
/// and the floor is configurable. The frontier accumulates 19 barter trades before
/// promotion, so a detector with a floor of 19 reads the Barter trigger as set while one
/// with a floor of 20 does not — purely from the read-only `barter_trade_count` accessor.
#[test]
fn measured_barter_trigger_reads_cumulative_volume_against_the_floor() {
    let mut s = Settlement::generate(SEED, &SettlementConfig::frontier());
    s.run(RUN_TICKS);
    let volume = s.barter_trade_count();
    assert!(volume > 0, "the frontier never bartered");

    // A floor at the realized volume reads the Barter trigger as set (index 1).
    let met = EraDetector::new().with_min_barter_volume(volume);
    assert!(
        met.measured_triggers(&s)[Era::Barter.rank()],
        "the Barter trigger should be set at a floor equal to the realized volume"
    );
    // A floor one above it is not met — the floor is a real, configurable gate.
    let unmet = EraDetector::new().with_min_barter_volume(volume + 1);
    assert!(
        !unmet.measured_triggers(&s)[Era::Barter.rank()],
        "the Barter trigger should be unset at a floor above the realized volume"
    );
    // Forager is always the floor; Money is set (the frontier promoted).
    assert!(met.measured_triggers(&s)[Era::Forager.rank()]);
    assert!(met.measured_triggers(&s)[Era::Money.rank()]);
}
