//! S11 — entrepreneurial uncertainty + profit/loss selection (the DoD suite).
//!
//! The flagship `frontier_coemergent_strong_entrepreneurial` scenario composes the S10
//! originary colony (money EMERGES, capital forms by a per-agent ordinal choice) with
//! **per-agent fallible forecasts**: every entrepreneurial appraisal (role-choice adopt,
//! per-agent capital build, project input-bid) weighs its OUTPUT-revenue estimate against
//! the colonist's OWN grounded forecast — its adaptive `PriceBelief` tilted by the
//! heritable `forecast_bias_bps` — instead of the shared last realized price. The market
//! still clears at the REAL price, so a wrong forecast bears the profit/loss through
//! CAPITAL accumulation (not mortality — `hunger_critical` stays disabled). These are the
//! named acceptance tests from `docs/impl-entrepreneurial-uncertainty.md`.

use sim::{
    capital_build_outcome_with_forecast, CapitalDeclineReason, CultureParams, Gold, GoodId,
    Settlement, SettlementConfig, FORECAST_BIAS_NEUTRAL_BPS, SALT,
};

/// The flagship scenario: S10 originary + per-agent fallible forecasts.
fn entrepreneurial() -> SettlementConfig {
    SettlementConfig::frontier_coemergent_strong_entrepreneurial()
}

/// A CONTROLLED negative-NPV variant of the flagship: a raised operating cost shrinks the
/// chain's per-run margin so that at the REAL realized price building a tool does not pay
/// (an accurate forecaster declines), and a UNIFORM forecast bias (jitter 0) so the colony
/// is cleanly all-accurate or all-optimist — the falsifiability isolate for test 3.
fn negative_npv_colony(forecast_bias_base_bps: u16) -> SettlementConfig {
    let mut cfg = entrepreneurial();
    cfg.forecast_bias_base_bps = forecast_bias_base_bps;
    cfg.forecast_bias_jitter_bps = 0;
    if let Some(chain) = cfg.chain.as_mut() {
        chain.operating_cost = 3;
    }
    cfg
}

/// The chain's bread (final-good) id.
fn bread_good(cfg: &SettlementConfig) -> GoodId {
    cfg.chain.as_ref().expect("chain").content.bread()
}

#[test]
fn entrepreneurial_run_is_deterministic() {
    // Acceptance 1: same (seed, config) → byte-identical canonical_bytes AND digest; a
    // different seed diverges. The new state in the determinism surface — the per-colonist
    // forecast bias, the per-belief `observed` flag, and the flag itself — are all gated
    // into the digest.
    let cfg = entrepreneurial();
    let mut a = Settlement::generate(0xC0FFEE, &cfg);
    let mut b = Settlement::generate(0xC0FFEE, &cfg);
    a.run(1600);
    b.run(1600);
    assert_eq!(
        a.canonical_bytes(),
        b.canonical_bytes(),
        "same (seed, config) must be byte-identical with entrepreneurial forecasts on"
    );
    assert_eq!(a.digest(), b.digest());

    let mut c = Settlement::generate(0xBADF00D, &cfg);
    c.run(1600);
    assert_ne!(a.digest(), c.digest(), "the seed must matter");
}

#[test]
fn forecasts_are_heterogeneous_and_feed_decisions() {
    // Acceptance 2: two colonists IDENTICAL in everything but `forecast_bias_bps` reach
    // DIFFERENT build decisions on the SAME market state — the over-optimist adopts the
    // build the accurate forecaster declines — and the realized clearing price is an
    // unchanged INPUT (the forecast moves no price). Patient time preference is held fixed
    // so the only thing that flips the answer is the forecast.
    //
    // Market state: realized output price 3 (× qty 1) against input 2 (× qty 1) + operating
    // 1 → margin 0 at the realized price (a NON-positive-margin build). The accurate
    // forecaster (×1.0) sees margin 0 and DECLINES; the over-optimist (×2.0) forecasts 6,
    // sees margin 3 > 0, and — on its deep patient savings ladder — ACCEPTS.
    let patient_tp = 2_000;
    let accurate =
        CultureParams::new_with_forecast_bias(patient_tp, 3_000, FORECAST_BIAS_NEUTRAL_BPS);
    let optimist = CultureParams::new_with_forecast_bias(patient_tp, 3_000, 20_000);
    let realized_output_price = 3;

    let accurate_outcome = capital_build_outcome_with_forecast(
        accurate,
        SALT,
        0,
        6,
        realized_output_price,
        1,
        2,
        1,
        6,
        4,
        1,
    );
    let optimist_outcome = capital_build_outcome_with_forecast(
        optimist,
        SALT,
        0,
        6,
        realized_output_price,
        1,
        2,
        1,
        6,
        4,
        1,
    );

    assert!(
        !accurate_outcome.accepted,
        "the accurate forecaster declines the negative-margin build (reason {:?})",
        accurate_outcome.reason
    );
    assert_eq!(
        accurate_outcome.reason,
        CapitalDeclineReason::NonPositiveMargin,
        "the accurate forecaster declines because the build does not pay at the REAL price"
    );
    assert!(
        optimist_outcome.accepted,
        "the over-optimist forecasts an inflated revenue and accepts the SAME build"
    );

    // The realized clearing price is forecast-independent — it was the unchanged input to
    // BOTH appraisals (the forecast only tilts the agent's OWN estimate of it).
    let neutral_outcome = capital_build_outcome_with_forecast(
        accurate,
        SALT,
        0,
        6,
        realized_output_price,
        1,
        2,
        1,
        6,
        4,
        1,
    );
    assert_eq!(
        neutral_outcome.accepted, accurate_outcome.accepted,
        "the appraisal is a pure function of (market state, culture) — deterministic"
    );
}

#[test]
fn goldens_unchanged() {
    // Acceptance 8 (the gated, additive seam): with `entrepreneurial_forecasts` OFF the
    // appraisals read the raw realized price and the new forecast-bias / `observed` state
    // is never serialized. Reconstruct the S10 originary base from the entrepreneurial
    // config by reverting ONLY the flag — it must be byte-identical to
    // `frontier_coemergent_strong_originary`, proving the flagship differs from its base by
    // nothing but the entrepreneurial seam. (The S5–S10 scenarios + the six econ + the
    // g5a/g5b/coemergence goldens staying byte-identical is enforced by the workspace
    // suites — originary_interest / producible_capital / money_coemergence /
    // strong_bar_emergence — plus the canonical_bytes_include_forecast_bias /
    // canonical_bytes_include_entrepreneurial_flag_and_belief_observed regressions in
    // settlement.rs.)
    let originary = SettlementConfig::frontier_coemergent_strong_originary();

    let mut reverted = entrepreneurial();
    {
        let c = reverted.chain.as_mut().expect("chain");
        assert!(
            c.entrepreneurial_forecasts,
            "the flagship config must have entrepreneurial forecasts on"
        );
        c.entrepreneurial_forecasts = false;
    }

    let mut a = Settlement::generate(5, &originary);
    let mut b = Settlement::generate(5, &reverted);
    a.run(700);
    b.run(700);
    assert_eq!(
        a.canonical_bytes(),
        b.canonical_bytes(),
        "the entrepreneurial flagship must differ from the originary base ONLY by the \
         forecast flag — with it reverted the run is byte-identical"
    );
    assert_eq!(a.digest(), b.digest());

    // And the originary base itself keeps the flag off (no accidental opt-in).
    assert!(
        !originary
            .chain
            .as_ref()
            .expect("chain")
            .entrepreneurial_forecasts,
        "the S10 originary base must keep entrepreneurial forecasts OFF (byte-identical to pre-S11)"
    );
}

#[test]
fn optimist_overbuilds_and_ends_poorer() {
    // Acceptance 3 — THE clean selection microtest (the falsifiability tripwire), a
    // controlled NEGATIVE-NPV opportunity, tested two ways.

    // (a) THE DECISION on the negative-NPV opportunity: building is unprofitable at the
    // real price (margin 0 → the accurate forecaster DECLINES) but appears profitable at an
    // inflated forecast (the over-optimist's ×2.0 revenue → it ACCEPTS). Deterministic,
    // signed, isolates the mechanism: only `forecast_bias_bps` differs.
    let accurate = CultureParams::new_with_forecast_bias(2_000, 3_000, FORECAST_BIAS_NEUTRAL_BPS);
    let optimist = CultureParams::new_with_forecast_bias(2_000, 3_000, 20_000);
    let accurate_decision =
        capital_build_outcome_with_forecast(accurate, SALT, 0, 6, 3, 1, 2, 1, 6, 4, 1);
    let optimist_decision =
        capital_build_outcome_with_forecast(optimist, SALT, 0, 6, 3, 1, 2, 1, 6, 4, 1);
    assert!(
        !accurate_decision.accepted,
        "the accurate forecaster declines the build that does not pay at the REAL price"
    );
    assert_eq!(
        accurate_decision.reason,
        CapitalDeclineReason::NonPositiveMargin
    );
    assert!(
        optimist_decision.accepted,
        "the over-optimist accepts the SAME build on its inflated forecast"
    );

    // (b) THE REALIZED OUTCOME on a live, conserved run: an all-accurate colony and an
    // all-optimist colony on the SAME controlled negative-NPV chain. The accurate colony
    // declines and PRESERVES (it tools up minimally and keeps its WOOD/gold); the optimist
    // colony OVERBUILDS, realizes the real (lower) proceeds against its inflated forecast,
    // and ends STRICTLY LOWER on the `agent_capital` balance sheet (gold + WOOD-at-realized
    // + tools-at-realized-liquidation, which is 0 — so a sunk-WOOD loss cannot hide in idle
    // tools). Deterministic and signed.
    let accurate_cfg = negative_npv_colony(FORECAST_BIAS_NEUTRAL_BPS);
    let optimist_cfg = negative_npv_colony(20_000);
    let mut accurate_run = Settlement::generate(1, &accurate_cfg);
    let mut optimist_run = Settlement::generate(1, &optimist_cfg);
    accurate_run.run(1200);
    optimist_run.run(1200);

    let accurate_tools = accurate_run.tools_built();
    let optimist_tools = optimist_run.tools_built();
    let accurate_capital = accurate_run.total_agent_capital();
    let optimist_capital = optimist_run.total_agent_capital();

    // Money emerges in both (the forecast bias does not gate SALT promotion).
    assert!(
        accurate_run.promoted_at_tick().is_some() && optimist_run.promoted_at_tick().is_some(),
        "money must emerge in both colonies"
    );
    // The accurate colony declines/preserves: it builds FAR fewer tools than the optimist.
    assert!(
        optimist_tools > accurate_tools,
        "the over-optimist must OVERBUILD relative to the accurate forecaster \
         (optimist {optimist_tools} vs accurate {accurate_tools})"
    );
    assert!(
        accurate_tools * 2 <= optimist_tools,
        "the accurate forecaster must build MATERIALLY less on the negative-NPV chain \
         (accurate {accurate_tools} vs optimist {optimist_tools})"
    );
    // The selection bites: the optimist ends STRICTLY LOWER on the balance sheet.
    assert!(
        optimist_capital < accurate_capital,
        "the over-optimist must end STRICTLY LOWER on agent_capital — profit/loss selection \
         through capital (optimist {optimist_capital} vs accurate {accurate_capital})"
    );
}

#[test]
fn forecasts_can_be_wrong() {
    // Acceptance 4: forecasting under uncertainty, not clairvoyance. There exist live
    // decisions where an agent's forecast MATERIALLY differs from the realized price (an
    // over-optimist standing above it), AND beliefs ADAPT toward realized over time
    // (`observe()` is live), so the bias is a standing over-shoot on top of a tracking
    // belief — not a permanent delusion and not perfect foresight.
    let cfg = entrepreneurial();
    let bread = bread_good(&cfg);
    let mut s = Settlement::generate(1, &cfg);

    // Snapshot every colonist's bread belief, then run on — beliefs that update from
    // observed trades must MOVE (observe() is live, so the forecast tracks the level even
    // as the bias holds it off-center).
    s.run(100);
    let pop = s.population();
    let belief_snapshot: Vec<Option<Gold>> =
        (0..pop).map(|i| s.belief_expected_of(i, bread)).collect();

    let mut saw_material_forecast_gap = false;
    let mut saw_optimist_above_realized = false;
    let mut saw_belief_adapt = false;
    for _ in 0..400u64 {
        s.econ_tick();
        for (i, snap) in belief_snapshot.iter().enumerate() {
            if !s.belief_observed_of(i, bread) {
                continue;
            }
            if s.belief_expected_of(i, bread) != *snap {
                saw_belief_adapt = true;
            }
            let Some(realized) = s.realized_price(bread) else {
                continue;
            };
            let Some(forecast) = s.forecast_price_for_good(i, bread) else {
                continue;
            };
            // A material gap between an agent's forecast and the realized price.
            if forecast.0.abs_diff(realized.0) * 5 >= realized.0 {
                saw_material_forecast_gap = true;
            }
            // An over-optimist (bias > neutral) forecasts ABOVE the realized price.
            if s.forecast_bias_of(i).unwrap_or(FORECAST_BIAS_NEUTRAL_BPS)
                > FORECAST_BIAS_NEUTRAL_BPS
                && forecast.0 > realized.0
            {
                saw_optimist_above_realized = true;
            }
        }
    }

    assert!(
        saw_material_forecast_gap,
        "some live forecast must materially differ from the realized price (forecasts can be wrong)"
    );
    assert!(
        saw_optimist_above_realized,
        "an over-optimist must systematically forecast ABOVE the realized price"
    );
    assert!(
        saw_belief_adapt,
        "beliefs must ADAPT toward realized over time (observe() is live)"
    );
}

#[test]
fn shock_causes_discoordination_then_recovery() {
    // Acceptance 5: a SETTLEMENT-LEVEL chain shock — disable the BAKE stage over [A, B)
    // then re-enable. FIRST assert the shock actually changed chain output (bread collapses
    // in [A, B) — not a no-op), THEN show the temporary production disruption RECOVERS to
    // pre-shock bounds in the tail, with no planner correction (the chain re-coordinates on
    // its own — beliefs re-learn, role-choice re-adopts). Whole-system conservation holds
    // every tick across the shock (no goods are created or destroyed; production stops).
    let cfg = entrepreneurial();
    let bread = bread_good(&cfg);
    let mut s = Settlement::generate(1, &cfg);
    s.run(500); // warm up to a coordinated chain producing bread

    let window = 80u64;

    // Pre-shock baseline.
    let mut pre_bread = 0u64;
    for _ in 0..window {
        let r = s.econ_tick();
        assert!(r.conserves(), "pre-shock tick must conserve");
        pre_bread += r.produced_of(bread);
    }
    assert!(pre_bread > 0, "the chain must be producing bread pre-shock");

    // SHOCK over [A, B): disable the bake stage. No oven can fire, so bread production
    // stops — but the tick still conserves (the shock moves no goods).
    assert!(
        s.set_bake_stage_enabled(false),
        "the chain must carry a bake recipe to shock"
    );
    let mut shock_bread = 0u64;
    for _ in 0..window {
        let r = s.econ_tick();
        assert!(r.conserves(), "the shock must conserve every tick");
        shock_bread += r.produced_of(bread);
    }
    s.set_bake_stage_enabled(true);

    // FIRST: the shock ACTUALLY changed chain output — bread collapsed during [A, B). With
    // bake disabled no oven fires, so output is far below the pre-shock baseline (not a
    // no-op econ event that never reached the sim chain).
    assert!(
        shock_bread * 2 < pre_bread,
        "the bake-stage shock must collapse bread output in [A,B) (shock {shock_bread} vs \
         pre {pre_bread}) — proving it actually perturbs the chain, not a no-op"
    );

    // Let the colony re-coordinate with NO planner correction — only the re-enable above;
    // the chain re-forms through the ordinary econ-tick phases.
    s.run(200);

    // RECOVERY: bread output returns to pre-shock bounds in the tail.
    let mut tail_bread = 0u64;
    for _ in 0..window {
        let r = s.econ_tick();
        assert!(r.conserves(), "post-shock tick must conserve");
        tail_bread += r.produced_of(bread);
    }
    assert!(
        tail_bread * 2 >= pre_bread,
        "bread must RECOVER to pre-shock bounds in the tail (tail {tail_bread} vs \
         pre {pre_bread}) — temporary discoordination, then re-coordination"
    );
}

#[test]
fn selection_is_not_mortality() {
    // Acceptance 6: confirm NO starvation deaths occur across a full flagship run — selection
    // is through capital, not death (`hunger_critical` stays disabled at need_max+1). Any
    // deaths are old age (the G4b demography overlay), kept distinct from the separate
    // starvation milestone. Tracks the conservation identity: every colonist that left the
    // living roster is accounted for by an old-age death.
    let cfg = entrepreneurial();
    let mut s = Settlement::generate(3, &cfg);

    // Every death reported across the run (`report.deaths` = starvation + old age) must be
    // accounted for by an OLD-AGE death — so the starvation count is exactly zero.
    let mut total_deaths = 0u64;
    for _ in 0..1600u64 {
        let r = s.econ_tick();
        total_deaths += u64::from(r.deaths);
    }
    assert_eq!(
        total_deaths,
        s.old_age_deaths_total(),
        "every death must be OLD AGE — zero starvation deaths (total deaths {total_deaths} \
         must equal old-age deaths {}); selection is capital, not mortality",
        s.old_age_deaths_total()
    );
}

#[test]
fn entrepreneurial_conserves() {
    // Acceptance 7: whole-system conservation holds every tick across the flagship run —
    // forecasts move NO goods (only the decision changes; the real trade conserves as
    // always). The build that DOES happen books WOOD → consumed_as_input and the tool →
    // produced, exactly as in S10.
    let mut s = Settlement::generate(7, &entrepreneurial());
    for tick in 0..1300u64 {
        let report = s.econ_tick();
        assert!(
            report.conserves(),
            "whole-system conservation must hold every tick, broke at {tick}"
        );
    }
    assert!(
        s.tools_built() > 0,
        "builds must have happened during the conserved run (else the test is vacuous)"
    );
}
