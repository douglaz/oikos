//! G8c-1 acceptance suite — fiat, the regime ladder, and the credit cycle.
//!
//! This is the climax of the economic engine: the **Austrian business cycle**, in the
//! colony game, on econ's **unchanged** ABCT/regime/shadow machinery. G8a put the sim
//! on M3 ledger money; G8b added banks and fiduciary credit. G8c-1 adds **fiat** and
//! the **regime ladder** (`SoundGold → FractionalConvertible → SuspendedConvertibility
//! → Fiat`) and demonstrates the cycle the lab proved: cheap credit drives the market
//! rate **below** the credit-disabled shadow natural rate (a measured **gap**),
//! capitalists over-invest in the long roundabout project (the **boom**), credit
//! **stops**, the rate reasserts, the malinvested project is abandoned, and **capital
//! is consumed** (the **bust**) — against a **sound-money control** that shows no gap
//! and no cycle.
//!
//! The reuse is total: the regime ladder (`SetRegime`), fiat issuance
//! (`SetIssuerPolicy`/`StopIssuerCredit`), the boom/bust/abandonment/capital-
//! consumption records, and the `run_credit_disabled_shadow` counterfactual are all
//! econ's, UNCHANGED. G8c-1 only routes the sim's regime/issuance into a finance
//! settlement (no spatial colony) and reads the measured signals back — so the six
//! econ conformance goldens stay byte-identical and every prior G1–G8b test is green.
//!
//! Magnitudes are SIGN/direction only (a positive gap vs ≈ 0; abandonments and
//! capital-consumed positive vs zero) — the lab's direction-not-magnitude discipline.
//! Conservation is EXACT (the M3 ledger reconciles; fiat base = issued − retired).
//! Scope is fiat + the regime ladder + the cycle + the control; tender/tax (G8c-2) and
//! the multi-seed robustness study are NOT here.

use econ::good::Gold;
use econ::money::Regime;
use sim::{Era, EraDetector, Settlement, SettlementConfig, TraderEndowment};

const SEED: u64 = 0xC0FFEE;

/// Run a fresh credit-cycle settlement `ticks` ticks.
fn credit_cycle(ticks: u64) -> Settlement {
    let mut s = Settlement::generate(SEED, &SettlementConfig::credit_cycle());
    s.run(ticks);
    s
}

/// Run a fresh sound-money control `ticks` ticks.
fn sound_money(ticks: u64) -> Settlement {
    let mut s = Settlement::generate(SEED, &SettlementConfig::sound_money());
    s.run(ticks);
    s
}

/// Observe a settlement with a fresh era detector for `ticks` ticks, returning the
/// detector so the climb can be inspected.
fn observe(config: SettlementConfig, ticks: u64) -> EraDetector {
    let mut s = Settlement::generate(SEED, &config);
    let mut detector = EraDetector::new();
    for _ in 0..ticks {
        s.econ_tick();
        detector.observe(&s);
    }
    detector
}

/// Run two fresh credit-cycle settlements in lockstep until live project funding
/// state exists. The pair should be byte-identical before a test mutates one side.
fn live_project_cycle_pair() -> (Settlement, Settlement) {
    let config = SettlementConfig::credit_cycle();
    let mut a = Settlement::generate(SEED, &config);
    let mut b = Settlement::generate(SEED, &config);
    for _ in 0..80 {
        a.econ_tick();
        b.econ_tick();
        let society = a.society();
        if !society.m2_projects.is_empty()
            && !society.debts.is_empty()
            && !society.project_funding_plans.is_empty()
        {
            assert_eq!(
                a.canonical_bytes(),
                b.canonical_bytes(),
                "lockstep cycle twins must match before mutation"
            );
            return (a, b);
        }
    }
    panic!("the credit cycle never reached live M2 project/debt/funding state");
}

/// 1. Same `(seed, config)` → a byte-identical run through the boom, the stop, and the
///    bust. Integer state, the econ `Rng` consumed only at generation, nothing drawn
///    in the loop — the cycle is a deterministic function of the run, and the canonical
///    bytes carry the regime rung, the fiat base, and the per-tick ABCT records.
#[test]
fn cycle_run_is_deterministic() {
    let config = SettlementConfig::credit_cycle();
    let mut a = Settlement::generate(SEED, &config);
    let mut b = Settlement::generate(SEED, &config);
    a.run(80);
    b.run(80);
    assert_eq!(
        a.canonical_bytes(),
        b.canonical_bytes(),
        "a credit-cycle run must be byte-identical for the same seed + config"
    );
    assert_eq!(a.digest(), b.digest());

    // The run is a finance settlement that actually exercised the cycle — determinism
    // over a live boom→stop→bust, not over an inert run.
    assert!(a.is_cycle() && a.is_m3());
    assert!(
        a.boom_projects_started() > 0 && a.bust_abandoned_projects() > 0,
        "the deterministic run must actually cross the boom and the bust"
    );
}

/// 2. Under fiat credit the market rate falls **below** the credit-disabled shadow
///    natural rate: the gap is `> 0` during the boom. Sign only. The sound-money
///    control, with no credit expansion, shows no positive gap — pairing this with the
///    control isolates the gap to credit.
#[test]
fn fiat_credit_opens_a_shadow_gap() {
    let cycle = credit_cycle(80);

    // The shadow gap = shadow natural rate − market rate. Cheap credit pulls the
    // market rate below the natural rate, so the gap opens positive somewhere in the
    // boom. MEASURED from the M3 records + the credit-disabled shadow replay.
    assert!(
        cycle.max_shadow_gap_bps() > 0,
        "fiat credit must open a positive shadow gap (market rate below the natural rate)"
    );
    let gaps = cycle
        .shadow_gap_bps()
        .expect("a cycle settlement has a shadow gap series");
    assert!(
        gaps.iter().any(|gap| gap.is_some_and(|bps| bps > 0)),
        "at least one boom tick must carry a positive gap"
    );

    // The regime descended the ladder to Fiat and the issuer extended fiat-credit.
    assert_eq!(
        cycle.regime(),
        Regime::Fiat,
        "the regime ladder descends to Fiat"
    );
    assert!(
        cycle.credit_ever_circulated(),
        "the cycle must extend created credit"
    );

    // The control opens no positive gap (paired falsification).
    let control = sound_money(80);
    assert_eq!(
        control.max_shadow_gap_bps(),
        0,
        "the sound-money control must open no positive gap"
    );
}

/// 3. The boom → stop → bust: cheap credit lengthens the roundabout structure above the
///    credit-disabled shadow baseline (the **boom**), credit **stops**, the rate
///    reasserts, the malinvested project is abandoned (`abandonments > 0`), and capital
///    is consumed (`capital_consumed > 0`). All MEASURED, reusing econ's M2/M3
///    abandonment + capital-consumption machinery.
#[test]
fn expansion_then_stop_busts_and_consumes_capital() {
    let cycle = credit_cycle(80);

    // The boom: the roundabout structure lengthens above the shadow baseline.
    assert!(
        cycle.structure_rose_above_shadow(),
        "the boom must lengthen the roundabout structure above the shadow baseline"
    );
    assert!(
        cycle.boom_projects_started() > 0,
        "the boom must start roundabout projects"
    );

    // The bust: malinvested projects are abandoned when credit stops.
    assert!(
        cycle.bust_abandoned_projects() > 0,
        "the bust must abandon the malinvested projects"
    );
    // Capital is consumed (the labor + non-salvaged goods embodied in those projects).
    assert!(
        cycle.capital_consumed() > 0,
        "the bust must consume real capital"
    );
}

/// 4. The sound-money control — the falsification twin. `SoundGold`, no fiat, no credit
///    expansion: the gap stays ≈ 0, no boom forms, nothing is abandoned, and no capital
///    is consumed. Paired with tests 2–3 (same agents, same roundabout project line,
///    only the credit/regime differs), this isolates the cycle to credit expansion — if
///    the control busts, the cycle is not coming from credit.
#[test]
fn sound_money_control_has_no_cycle() {
    let control = sound_money(80);

    // The control stays on sound money: SoundGold, no fiat ever issued.
    assert_eq!(
        control.regime(),
        Regime::SoundGold,
        "the control stays SoundGold"
    );
    assert_eq!(
        control.fiat_base(),
        Gold::ZERO,
        "the control issues no fiat"
    );
    assert!(
        !control.credit_ever_circulated(),
        "the control extends no created credit"
    );

    // No cycle: no gap, no boom, no bust, no capital consumed.
    assert_eq!(control.max_shadow_gap_bps(), 0, "the control opens no gap");
    assert!(
        !control.structure_rose_above_shadow(),
        "the control's structure never rises above the shadow baseline"
    );
    assert_eq!(
        control.boom_projects_started(),
        0,
        "the control starts no boom projects"
    );
    assert_eq!(
        control.bust_abandoned_projects(),
        0,
        "the control abandons nothing"
    );
    assert_eq!(
        control.capital_consumed(),
        0,
        "the control consumes no capital"
    );

    // The twin: the credit cycle DID bust from the same agents and the same project
    // line. The credit/regime is the only difference, so it is the cause.
    let cycle = credit_cycle(80);
    assert!(
        cycle.bust_abandoned_projects() > 0 && cycle.capital_consumed() > 0,
        "the credit twin busts from the same economy — the cycle comes from credit, \
         not the production/spatial dynamics"
    );
}

/// 5. Fiat issuance/retirement **conserves**: the fiat base = issued − retired equals
///    the outstanding fiat circulating, the specie base is unchanged across the cycle
///    (fiat is credit, not minted specie), and the M3 ledger reconciles every tick — a
///    default changes the money stock by rule (retirement/booking), never by a leak.
#[test]
fn fiat_conserves() {
    let mut s = Settlement::generate(SEED, &SettlementConfig::credit_cycle());
    let initial = s
        .money_composition()
        .expect("a finance settlement has an M3 composition");
    // The specie base: public specie + bank reserves, fixed across the whole cycle.
    let specie_base = initial.public_specie.0 + initial.bank_reserves.0;

    let mut crossed_fiat = false;
    for t in 0..80 {
        s.econ_tick();
        // The M3 ledger's own conservation holds every tick (specie/fiat/claims
        // reconcile, balances map to live agents).
        assert!(
            s.society().money_ledgers_reconcile(),
            "the M3 ledger must reconcile across the cycle at econ tick {t}"
        );
        let composition = s
            .money_composition()
            .expect("a finance settlement has an M3 composition");
        // Specie is conserved: fiat issuance is credit, never minted specie.
        assert_eq!(
            composition.public_specie.0 + composition.bank_reserves.0,
            specie_base,
            "the specie base changed at econ tick {t} — fiat is credit, not minted specie"
        );
        // The fiat base identity: outstanding fiat = issued − retired.
        assert_eq!(
            s.fiat_base(),
            composition.public_fiat,
            "fiat base (issued − retired) must equal the outstanding circulating fiat at tick {t}"
        );
        // Broad money is exactly specie + fiat (no claims/fiduciary on the fiat-credit
        // path) — the TMS identity.
        assert_eq!(
            composition.tms().0,
            composition.public_specie.0 + composition.public_fiat.0,
            "broad money must be specie + fiat at econ tick {t}"
        );
        if s.fiat_base() > Gold::ZERO {
            crossed_fiat = true;
        }
    }
    assert!(
        crossed_fiat,
        "the run must actually issue fiat (else conservation proves nothing)"
    );
    // Retirement is real: the run repaid some fiat credit (issued strictly exceeds the
    // outstanding base by the retired amount).
    assert!(
        s.fiat_issued() >= s.fiat_base(),
        "issued fiat is never less than the outstanding base"
    );
    assert_eq!(
        s.fiat_base(),
        s.fiat_issued().saturating_sub(s.fiat_retired()),
        "the fiat base is exactly issued − retired"
    );
}

/// 6. The Credit and Modern era rungs unlock: the G6a detector reaches **Credit** when
///    institutionally-created credit circulates and **Modern** when state fiat is the
///    marginal medium — both MEASURED, with the same hysteresis as every other rung.
///    The sound-money control never reaches them (the falsification).
#[test]
fn credit_and_modern_eras_unlock() {
    let detector = observe(SettlementConfig::credit_cycle(), 80);
    // The cycle climbs the full ladder up to Modern.
    assert_eq!(
        detector.current_era(),
        Era::Modern,
        "the credit cycle must reach the Modern era (state fiat the marginal medium)"
    );
    // Credit unlocks before Modern (created credit circulates before fiat dominates the
    // margin), and the rungs are earned in ladder order.
    let credit_tick = detector
        .first_tick(Era::Credit)
        .expect("the cycle reaches the Credit era");
    let modern_tick = detector
        .first_tick(Era::Modern)
        .expect("the cycle reaches the Modern era");
    let capital_tick = detector
        .first_tick(Era::Capital)
        .expect("the cycle reaches the Capital era first");
    assert!(
        capital_tick < credit_tick && credit_tick < modern_tick,
        "the eras must be earned in ladder order: capital < credit < modern"
    );

    // Hysteresis: each rung is entered only after the trigger holds for a sustained
    // window, so consecutive rungs are at least a window apart (no single-tick flap up
    // the ladder).
    assert!(
        modern_tick - credit_tick >= detector.window(),
        "Modern must be earned a sustained window after Credit (hysteresis)"
    );

    // The control never reaches the finance rungs — no credit, no fiat.
    let control = observe(SettlementConfig::sound_money(), 80);
    assert!(
        control.current_era() < Era::Credit,
        "the sound-money control must not reach Credit or Modern"
    );
    assert_eq!(control.first_tick(Era::Credit), None);
    assert_eq!(control.first_tick(Era::Modern), None);
}

/// 7. `econ_unchanged` — the cycle additions are inert for every non-finance settlement
///    (a spatial run is byte-identical to a twin, and surfaces no cycle), and a cycle
///    run keeps econ's invariants (the M3 ledger reconciles every tick). The six econ
///    goldens staying byte-identical and the full workspace suite + `cargo clippy
///    --workspace --all-targets -- -D warnings` + `cargo fmt --check` are the real gate;
///    this checks the local seam.
#[test]
fn econ_unchanged() {
    // A non-finance settlement surfaces no cycle — the finance path is skipped.
    let plain = Settlement::generate(7, &SettlementConfig::m3_settlement());
    assert!(!plain.is_cycle(), "the M3 settlement runs no credit cycle");
    assert_eq!(plain.regime(), Regime::SoundGold);
    let m1 = Settlement::generate(7, &SettlementConfig::viable());
    assert!(
        !m1.is_cycle(),
        "the closed-GOLD M1 settlement runs no credit cycle"
    );

    // The non-finance M3 settlement is byte-identical to a twin — the G8c-1 additions
    // are present but unexercised.
    let mut a = Settlement::generate(42, &SettlementConfig::m3_settlement());
    let mut b = Settlement::generate(42, &SettlementConfig::m3_settlement());
    a.run(40);
    b.run(40);
    assert_eq!(a.canonical_bytes(), b.canonical_bytes());

    // A cycle run keeps the econ invariants tick over tick: the M3 ledger reconciles.
    let mut cycle = Settlement::generate(3, &SettlementConfig::credit_cycle());
    for _ in 0..120 {
        cycle.econ_tick();
        assert!(
            cycle.society().money_ledgers_reconcile(),
            "the M3 ledger reconciles every tick across the cycle"
        );
    }
}

// ---- unit tests -------------------------------------------------------------

/// A credit-cycle settlement requires the M3 ledger.
#[test]
#[should_panic(expected = "requires the M3 ledger")]
fn cycle_requires_m3_ledger() {
    let mut config = SettlementConfig::credit_cycle();
    config.m3 = false;
    let _ = Settlement::generate(1, &config);
}

/// A credit-cycle settlement is a finance settlement with no spatial colony — a
/// caller cannot bolt a spatial colony onto it.
#[test]
#[should_panic(expected = "no spatial colony")]
fn cycle_rejects_spatial_colony() {
    let mut config = SettlementConfig::credit_cycle();
    config.gatherers = 4;
    let _ = Settlement::generate(1, &config);
}

/// A credit-cycle settlement is mutually exclusive with the bank overlay (a finance
/// settlement routes credit through the issuer/regime, not a sim-chartered bank).
#[test]
#[should_panic(expected = "no spatial overlay")]
fn cycle_rejects_bank_overlay() {
    let mut config = SettlementConfig::credit_cycle();
    config.bank = SettlementConfig::bank().bank;
    let _ = Settlement::generate(1, &config);
}

/// A credit-cycle settlement is also mutually exclusive with resident traders: the
/// finance path builds its agents from econ's scenario cast, so spatial resident
/// traders must be rejected rather than silently dropped.
#[test]
#[should_panic(expected = "resident_traders")]
fn cycle_rejects_resident_traders_overlay() {
    let config = SettlementConfig::credit_cycle().with_resident_traders(vec![TraderEndowment {
        gold: 1,
        stock: Vec::new(),
    }]);
    let _ = Settlement::generate(1, &config);
}

/// The credit cycle and its sound-money control are a falsification twin: same kind of
/// settlement, opposite credit/regime outcomes.
#[test]
fn cycle_and_control_are_a_twin() {
    let cycle = credit_cycle(60);
    let control = sound_money(60);
    assert!(cycle.is_cycle() && control.is_cycle());
    assert_eq!(cycle.cycle_kind(), Some(sim::CycleKind::CreditCycle));
    assert_eq!(control.cycle_kind(), Some(sim::CycleKind::SoundMoney));
    // The cycle descends the regime ladder to Fiat and prints fiat; the control stays
    // SoundGold and prints none.
    assert_eq!(cycle.regime(), Regime::Fiat);
    assert!(cycle.fiat_base() > Gold::ZERO);
    assert_eq!(control.regime(), Regime::SoundGold);
    assert_eq!(control.fiat_base(), Gold::ZERO);
}

/// The finance overlay's kind and retained event schedule are future-steering
/// canonical state even before the first M3 record exists. The cycle and control are
/// live-state twins at generation, but their next ticks intentionally diverge.
#[test]
fn cycle_kind_and_policy_timeline_are_canonical_state() {
    let cycle = Settlement::generate(SEED, &SettlementConfig::credit_cycle());
    let control = Settlement::generate(SEED, &SettlementConfig::sound_money());

    assert_eq!(cycle.society().m3_records.len(), 0);
    assert_eq!(control.society().m3_records.len(), 0);
    assert_eq!(cycle.regime(), Regime::SoundGold);
    assert_eq!(control.regime(), Regime::SoundGold);
    assert_eq!(cycle.fiat_base(), Gold::ZERO);
    assert_eq!(control.fiat_base(), Gold::ZERO);

    assert_ne!(
        cycle.canonical_bytes(),
        control.canonical_bytes(),
        "future-divergent cycle/control configs must not collide before the first tick"
    );
    assert_ne!(cycle.digest(), control.digest());
}

/// The regime walked the full ladder rungs on the way to Fiat (the cycle records pass
/// through FractionalConvertible / SuspendedConvertibility before Fiat).
#[test]
fn regime_walks_the_ladder() {
    let cycle = credit_cycle(10);
    // The per-tick M3 records pass through the full ladder
    // (FractionalConvertible → SuspendedConvertibility → Fiat) before settling at Fiat.
    let regimes: Vec<Regime> = cycle
        .society()
        .m3_records
        .iter()
        .map(|record| record.regime)
        .collect();
    assert!(
        regimes.contains(&Regime::FractionalConvertible),
        "the ladder descends through FractionalConvertible"
    );
    assert!(
        regimes.contains(&Regime::SuspendedConvertibility),
        "the ladder descends through SuspendedConvertibility"
    );
    assert!(regimes.contains(&Regime::Fiat), "the ladder reaches Fiat");
    // The settlement's current regime is Fiat (the bottom of the ladder).
    assert_eq!(cycle.regime(), Regime::Fiat);
    // The shadow control never leaves SoundGold (the top of the ladder).
    let control = sound_money(10);
    assert_eq!(control.regime(), Regime::SoundGold);
    assert!(control
        .society()
        .m3_records
        .iter()
        .all(|record| record.regime == Regime::SoundGold));
}

/// The Credit rung is **path-independent**: G8c-1 measures it the same way wherever
/// institutionally-created credit arises, not just on the finance path. A spatial G8b
/// banked settlement whose chartered bank lends fiduciary credit reads the Credit
/// trigger as **set** (the documented chartered-bank-fiduciary signal), while its
/// 100%-reserve control — which lends no fiduciary — reads it **unset**. This pins the
/// measured trigger directly: the designated-money spatial ladder is gated at the
/// emergent Barter rung (a designated-money camp never barters), so neither config
/// climbs to Credit through the hysteresis — the trigger, not the climb, is what makes
/// the rung path-independent, and a future banked settlement that did climb to Capital
/// would advance to Credit from this same signal.
#[test]
fn chartered_bank_fiduciary_sets_the_credit_trigger() {
    // The fractional-reserve bank lends fiduciary credit into the M3 records.
    let mut fractional = Settlement::generate(7, &SettlementConfig::bank());
    fractional.run(60);
    assert!(
        fractional.credit_ever_circulated(),
        "the fractional bank must circulate chartered-bank fiduciary credit"
    );
    let detector = EraDetector::new();
    let fractional_triggers = detector.measured_triggers(&fractional);
    assert!(
        fractional_triggers[Era::Credit.rank()],
        "a chartered-bank settlement's Credit trigger must be set from its circulating fiduciary credit"
    );
    // Modern stays unset — a G8b bank issues no state fiat (only the credit cycle does).
    assert!(
        !fractional_triggers[Era::Modern.rank()],
        "a G8b bank issues no fiat, so the Modern trigger stays unset"
    );

    // The 100%-reserve control lends no fiduciary, so its Credit trigger stays unset —
    // the falsification twin (only the reserve ratio differs from `bank`).
    let mut full_reserve = Settlement::generate(7, &SettlementConfig::bank_full_reserve());
    full_reserve.run(60);
    assert!(
        !full_reserve.credit_ever_circulated(),
        "the 100%-reserve control lends no fiduciary credit"
    );
    assert!(
        !detector.measured_triggers(&full_reserve)[Era::Credit.rank()],
        "the 100%-reserve control's Credit trigger must stay unset (no created credit)"
    );

    // The bank-free emergent-chain frontier reads neither finance signal, so its Credit
    // and Modern triggers are zero — the spatial ladder still tops out at Capital and
    // every G6a measured timeline is byte-identical.
    let mut frontier = Settlement::generate(7, &SettlementConfig::frontier());
    frontier.run(60);
    let frontier_triggers = detector.measured_triggers(&frontier);
    assert!(
        !frontier_triggers[Era::Credit.rank()] && !frontier_triggers[Era::Modern.rank()],
        "the bank-free frontier reads no created credit or fiat — Credit/Modern stay unset"
    );
}

/// A finance settlement is byte-identical for the same `(seed, config)` even over a
/// short pre-boom window — determinism does not depend on the boom firing first.
#[test]
fn cycle_short_run_is_deterministic() {
    let config = SettlementConfig::credit_cycle();
    let mut a = Settlement::generate(99, &config);
    let mut b = Settlement::generate(99, &config);
    a.run(8);
    b.run(8);
    assert_eq!(a.canonical_bytes(), b.canonical_bytes());
}

/// Finance runs after the boom are driven by live M2 project/debt/funding state, not
/// just historical M3 summaries. The canonical digest must distinguish states that
/// share the same history but would step differently next.
#[test]
fn cycle_canonical_bytes_include_live_project_debt_state() {
    let (baseline, mut changed) = live_project_cycle_pair();
    {
        let project = &mut changed.society_mut().m2_projects[0];
        project.labor_advanced = project.labor_advanced.saturating_add(1);
    }
    assert_ne!(
        baseline.canonical_bytes(),
        changed.canonical_bytes(),
        "live M2 project state must be canonical"
    );

    let (baseline, mut changed) = live_project_cycle_pair();
    {
        let debt = &mut changed.society_mut().debts[0];
        debt.paid = Gold(debt.paid.0.saturating_add(1));
    }
    assert_ne!(
        baseline.canonical_bytes(),
        changed.canonical_bytes(),
        "live debt state must be canonical"
    );

    let (baseline, mut changed) = live_project_cycle_pair();
    {
        let plan = &mut changed.society_mut().project_funding_plans[0];
        plan.reserved_gold = Gold(plan.reserved_gold.0.saturating_add(1));
    }
    assert_ne!(
        baseline.canonical_bytes(),
        changed.canonical_bytes(),
        "live project-funding plan state must be canonical"
    );
}
