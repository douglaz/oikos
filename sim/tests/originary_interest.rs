//! S10 — per-agent intertemporal capital choice / originary interest (the DoD suite).
//!
//! The flagship `frontier_coemergent_strong_originary` scenario composes the strong-bar
//! co-emergent colony (money EMERGES from real saleability) with **per-agent capital
//! choice**: capital forms through a per-colonist ORDINAL decision — each eligible
//! colonist appraises, on its own value scale, whether committing present WOOD + forgone
//! leisure to build a durable mill/oven whose recipe-margin receipt stream provisions one
//! of its own future-money savings wants is worth it — instead of S7's settlement-level
//! planner. Capital formation then tracks each colonist's `time_preference_bps`: patient
//! colonists invest in the roundabout tooled chain, present-biased ones do not — with NO
//! cardinal discount (originary interest expressed ordinally via the multi-horizon savings
//! ladder). These are the named acceptance tests from `docs/impl-originary-interest.md`.

use sim::{
    capital_build_outcome_for_culture, savings_ladder_depth, CapitalDeclineReason, CultureParams,
    GoodId, Settlement, SettlementConfig, Vocation, SALT, WOOD,
};

/// The flagship scenario: strong-bar co-emergence + per-agent capital choice.
fn originary() -> SettlementConfig {
    SettlementConfig::frontier_coemergent_strong_originary()
}

/// The chain's (mill, oven) tool goods.
fn tool_goods(cfg: &SettlementConfig) -> (GoodId, GoodId) {
    let content = cfg.chain.as_ref().expect("chain").content.clone();
    (content.mill(), content.oven())
}

fn tools(s: &Settlement, mill: GoodId, oven: GoodId) -> u64 {
    s.whole_system_total(mill) + s.whole_system_total(oven)
}

/// The flagship with every colony time preference forced to `tp` — a uniformly patient
/// (low) or present-biased (high) colony, holding everything else (incl. the SALT
/// emergence machinery) fixed.
fn with_colony_time_preference(mut cfg: SettlementConfig, tp: u16) -> SettlementConfig {
    cfg.gatherer_time_preference_base_bps = tp;
    cfg.consumer_time_preference_base_bps = tp;
    if let Some(demo) = cfg.demography.as_mut() {
        for household in &mut demo.households {
            household.time_preference_base_bps = tp;
        }
    }
    cfg
}

#[test]
fn originary_run_is_deterministic() {
    // Acceptance 1: same (seed, config) → byte-identical canonical_bytes AND digest; a
    // different seed must diverge (the per-agent build state + the per_agent_capital gate
    // + the multi-horizon ladder are all in the determinism surface).
    let cfg = originary();
    let mut a = Settlement::generate(0xC0FFEE, &cfg);
    let mut b = Settlement::generate(0xC0FFEE, &cfg);
    a.run(1600);
    b.run(1600);
    assert_eq!(
        a.canonical_bytes(),
        b.canonical_bytes(),
        "same (seed, config) must be byte-identical with per-agent capital on"
    );
    assert_eq!(a.digest(), b.digest());

    let mut c = Settlement::generate(0xBADF00D, &cfg);
    c.run(1600);
    assert_ne!(a.digest(), c.digest(), "the seed must matter");
}

#[test]
fn capital_decision_is_per_agent_not_planned() {
    // Acceptance 2: with per_agent_capital on there is NO settlement-level stage choice or
    // builder assignment. Via the per-agent decision diagnostic, prove that on some tick a
    // LATER-slot colonist accepted while an EARLIER-slot eligible colonist DECLINED ON ITS
    // OWN SCALE (a scale-specific reason — its savings were satiated/too-shallow, or the
    // present cost outranked the future gain) — so the builder is chosen by its own
    // appraisal, not slot-order-first. And tools are funded from the builder's OWN WOOD.
    let cfg = originary();
    let wood_cost = u64::from(cfg.chain.as_ref().expect("chain").tool_build_wood);
    let mut s = Settlement::generate(1, &cfg);

    let mut saw_later_accept_after_earlier_own_decline = false;
    let mut saw_own_wood_spent = false;
    let mut prev_wood: Vec<u64> = (0..s.population()).map(|i| s.stock_of(i, WOOD)).collect();
    for _ in 0..1200u64 {
        s.econ_tick();
        let decisions = s.last_capital_decisions();
        // The earliest slot at which a colonist declined ON ITS OWN SCALE this tick.
        let earliest_own_decline = decisions
            .iter()
            .filter(|d| {
                !d.accepted
                    && matches!(
                        d.reason,
                        CapitalDeclineReason::NoFutureProvision
                            | CapitalDeclineReason::PresentCostOutranks
                    )
            })
            .map(|d| d.slot)
            .min();
        if let Some(decline_slot) = earliest_own_decline {
            if decisions
                .iter()
                .any(|d| d.accepted && d.slot > decline_slot)
            {
                saw_later_accept_after_earlier_own_decline = true;
            }
        }
        // Some builder spent its OWN WOOD (its deposited WOOD fell by at least the build
        // cost between ticks) — the WOOD came from its endowment, not a placement.
        if prev_wood.len() < s.population() {
            prev_wood.resize(s.population(), 0);
        }
        for (i, prev) in prev_wood.iter_mut().enumerate() {
            let now = s.stock_of(i, WOOD);
            if prev.saturating_sub(now) >= wood_cost {
                saw_own_wood_spent = true;
            }
            *prev = now;
        }
    }

    assert!(
        s.tools_built() > 0,
        "the per-agent build path must fire at least once"
    );
    assert!(
        saw_later_accept_after_earlier_own_decline,
        "an EARLIER-eligible colonist must decline on its OWN scale while a LATER one \
         accepts — proving per-colonist appraisal, not slot-order-first assignment"
    );
    assert!(
        saw_own_wood_spent,
        "a builder must pay the WOOD from its OWN deposited endowment"
    );
    assert!(
        (0..s.population()).any(|i| s.acquired_tool_of(i)),
        "a formerly-non-latent colonist must be the one that built a tool"
    );
}

#[test]
fn higher_time_preference_forms_less_capital() {
    // Acceptance 3 — THE falsifiable bar, tested two ways.

    // (a) MICROTEST: two otherwise-identical colonists differing ONLY in
    // time_preference_bps. The patient one ACCEPTS the build, the present-biased one
    // REJECTS it — strict and deterministic, isolating the horizon-depth formula. If this
    // does not hold, the decision is not reading the ordinal scale and the milestone
    // failed its purpose.
    assert!(
        savings_ladder_depth(2_000) >= 2,
        "a patient colonist's savings ladder must reach a deep horizon"
    );
    assert_eq!(
        savings_ladder_depth(8_000),
        1,
        "a present-biased colonist's ladder must be the shallow base only"
    );
    // Same rested state, same WOOD on hand (6), same zero savings balance, same recipe
    // proceeds (output 10 × qty 1 − input 2 × qty 1 − operating 1 = 7 per run), same build
    // cost (6 WOOD, 4 labor) — only time preference differs.
    let patient = capital_build_outcome_for_culture(
        CultureParams::new(2_000, 3_000),
        SALT,
        0,
        6,
        10,
        1,
        2,
        1,
        6,
        4,
        1,
    );
    let present_biased = capital_build_outcome_for_culture(
        CultureParams::new(8_000, 3_000),
        SALT,
        0,
        6,
        10,
        1,
        2,
        1,
        6,
        4,
        1,
    );
    assert!(
        patient.accepted,
        "the patient colonist must accept the build on its own deep ladder"
    );
    assert!(
        !present_biased.accepted,
        "the present-biased colonist must decline — its shallow ladder is unreachable"
    );
    assert_eq!(
        present_biased.reason,
        CapitalDeclineReason::NoFutureProvision,
        "the present-biased decline is because the tool's receipts reach no savings want"
    );

    // (b) LIVE AGGREGATE: a present-biased colony forms materially less / non-more capital
    // than a patient one over the full run (NOT strict per-step monotonicity). Money still
    // EMERGES in both (the savings ladder does not gate SALT promotion), so the difference
    // is the capital response to time preference, not a money-emergence artifact.
    let cfg = originary();
    let (mill, oven) = tool_goods(&cfg);
    let measure = |tp: u16| {
        let mut s = Settlement::generate(1, &with_colony_time_preference(cfg.clone(), tp));
        s.run(1600);
        (s.tools_built(), tools(&s, mill, oven), s.promoted_at_tick())
    };
    let (patient_built, patient_tools, patient_promo) = measure(400);
    let (biased_built, biased_tools, biased_promo) = measure(8_000);

    assert!(
        patient_promo.is_some() && biased_promo.is_some(),
        "money must emerge in both colonies (patient {patient_promo:?}, biased {biased_promo:?})"
    );
    assert!(
        patient_built > 0 && patient_tools > 0,
        "the patient colony must form capital, got {patient_built} built / {patient_tools} tools"
    );
    assert!(
        biased_built < patient_built,
        "a present-biased colony must form less capital (biased {biased_built} built vs \
         patient {patient_built})"
    );
    assert!(
        biased_tools <= patient_tools / 2,
        "the present-biased colony's capital must be MATERIALLY less (biased {biased_tools} \
         tools vs patient {patient_tools})"
    );
}

#[test]
fn capital_still_responds_to_demand() {
    // Acceptance 4: real-resource investment responds to demand on each colonist's scale
    // and STOPS — once savers' deep wants are met / the spread no longer pays, the
    // per-agent appraisal declines, so the tool count stabilizes (no unbounded
    // idle-tool overbuild) and whole-system WOOD is not drained by speculative building.
    let cfg = originary();
    let (mill, oven) = tool_goods(&cfg);
    let mut s = Settlement::generate(1, &cfg);

    let snap = |s: &Settlement| {
        (
            tools(s, mill, oven),
            s.whole_system_total(WOOD),
            s.tools_built(),
        )
    };
    s.run(800);
    let (tools_a, wood_a, built_a) = snap(&s);
    s.run(400);
    let (tools_b, wood_b, built_b) = snap(&s);
    s.run(400);
    let (tools_c, wood_c, built_c) = snap(&s);

    // The tool count is BOUNDED across the tail (it does not keep climbing): the last
    // window's growth is a small fraction of the count — building essentially stops once
    // the savers' demand for the roundabout chain is met.
    assert!(
        tools_c <= tools_a + tools_a / 2 + 3,
        "tools must stabilize across the tail, got {tools_a} -> {tools_b} -> {tools_c}"
    );
    // The build RATE does not accelerate in the tail (no runaway overbuild).
    assert!(
        built_c.saturating_sub(built_b) <= built_b.saturating_sub(built_a).max(3),
        "the build rate must not accelerate (overbuild): built {built_a} -> {built_b} -> {built_c}"
    );
    // WOOD is not drained by speculative building: the tail WOOD does not collapse.
    assert!(
        wood_c * 2 >= wood_a,
        "whole-system WOOD must not be drained by building, got {wood_a} -> {wood_b} -> {wood_c}"
    );
}

#[test]
fn chain_sustains_under_per_agent_capital() {
    // Acceptance 5: the flagship still emerges money, sustains bread to t1600, builds at
    // least one tool by individual choice, and conserves every tick.
    let cfg = originary();
    let bread = cfg.chain.as_ref().expect("chain").content.bread();
    let mut s = Settlement::generate(1, &cfg);
    let mut late_bread = 0u64;
    for tick in 0..1600u64 {
        let report = s.econ_tick();
        assert!(report.conserves(), "broke conservation at tick {tick}");
        if (1400..1600).contains(&tick) {
            late_bread += report.produced_of(bread);
        }
    }
    assert!(
        s.promoted_at_tick().is_some(),
        "money must emerge in the flagship run"
    );
    assert!(
        late_bread > 0,
        "bread must sustain to t1600 (late-window bread {late_bread})"
    );
    assert!(
        s.tools_built() >= 1,
        "at least one tool must be built by individual choice, got {}",
        s.tools_built()
    );
    // The built capital entered the chain: a formerly-non-latent colonist holds a tool and
    // active producers run the tooled chain.
    assert!(
        (0..s.population()).any(|i| s.acquired_tool_of(i)),
        "a formerly-non-latent colonist must have built a produced tool"
    );
    assert!(
        s.living_count(Vocation::Miller) + s.living_count(Vocation::Baker) > 0,
        "active producers must run the tooled chain at t1600"
    );
}

#[test]
fn originary_conserves() {
    // Acceptance 6: whole-system conservation holds every tick across the per-agent builds
    // (the build's WOOD → consumed_as_input, the tool → produced, exactly as in S7).
    let mut s = Settlement::generate(7, &originary());
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

#[test]
fn goldens_unchanged_per_agent_gated() {
    // Acceptance 7 (the gated, additive seam): with per_agent_capital OFF the build seam is
    // exactly S7's, and the multi-horizon savings ladder never activates. Reconstruct the
    // S9 strong-bar base from the originary config by reverting ONLY the S10 seam
    // (per_agent off) and the two build-knob tweaks — it must be byte-identical to
    // `frontier_coemergent_strong`, proving the originary scenario differs from the strong
    // base by nothing but the per-agent seam. (The S5–S9 scenarios + the six econ + the
    // g5a/g5b/coemergence goldens staying byte-identical is enforced by the workspace
    // suites — producible_capital / money_coemergence / strong_bar_emergence — plus the
    // canonical_bytes_include_per_agent_capital digest regression in settlement.rs.)
    let strong = SettlementConfig::frontier_coemergent_strong();
    let strong_chain = strong.chain.as_ref().expect("chain");
    let strong_wood = strong_chain.tool_build_wood;
    let strong_buffer = strong_chain.wood_buffer;

    let mut reverted = originary();
    {
        let c = reverted.chain.as_mut().expect("chain");
        assert!(
            c.per_agent_capital,
            "the originary config must have per-agent on"
        );
        c.per_agent_capital = false;
        c.tool_build_wood = strong_wood;
        c.wood_buffer = strong_buffer;
    }

    let mut a = Settlement::generate(5, &strong);
    let mut b = Settlement::generate(5, &reverted);
    a.run(700);
    b.run(700);
    assert_eq!(
        a.canonical_bytes(),
        b.canonical_bytes(),
        "the originary scenario must differ from the strong-bar base ONLY by the per-agent \
         seam and its build-knob tweaks — with those reverted it is byte-identical"
    );
    assert_eq!(a.digest(), b.digest());

    // And the strong-bar base itself keeps per_agent_capital off (no accidental opt-in).
    assert!(
        !strong_chain.per_agent_capital,
        "the S9 strong-bar base must keep per-agent capital OFF (byte-identical to pre-S10)"
    );
    // With per-agent off the per-agent decision diagnostic is never recorded.
    let mut off = Settlement::generate(1, &reverted);
    off.run(600);
    assert!(
        off.last_capital_decisions().is_empty(),
        "the per-agent decision diagnostic must be empty on the S7 heuristic path"
    );
}
