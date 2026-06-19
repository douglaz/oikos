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
    capital_build_outcome_with_forecast, CapitalDeclineReason, CultureParams, Settlement,
    SettlementConfig, FORECAST_BIAS_NEUTRAL_BPS, SALT,
};

/// The flagship scenario: S10 originary + per-agent fallible forecasts.
fn entrepreneurial() -> SettlementConfig {
    SettlementConfig::frontier_coemergent_strong_entrepreneurial()
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

// Acceptance tests 3–7 are added with their slices (S11.2 — profit/loss selection;
// S11.3 — shock → discoordination → recovery).
