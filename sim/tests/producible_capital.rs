//! S7.3 — producible capital goods (the DoD acceptance suite).
//!
//! The `capital` scenario composes the scaling economy with the two S7 gates:
//! tool-acquisition eligibility (S7.1 — a colonist that HOLDS a mill/oven is admitted
//! to the adoption appraisal) and per-agent capital formation (S7.2 — a fed colonist
//! invests its own WOOD + labor to BUILD a mill/oven). The honest bar: under sustained
//! unmet bread demand the colony BUILDS new producers, so bread output tracks demand
//! instead of flat-lining at the seeded tool count — with NO planner tool placement and
//! no runaway over-building (capital formation stops when demand is met), and
//! conservation holding every tick across each build.
//!
//! These are the eight named acceptance tests from `docs/impl-producible-capital.md`.

use sim::{GoodId, Settlement, SettlementConfig, Vocation, WOOD};

/// The capital economy (both S7 gates on, a larger colony with unmet bread demand).
fn capital() -> SettlementConfig {
    SettlementConfig::frontier_capital()
}

/// The no-build control on the SAME colony as `capital`: both S7 gates off, so the
/// tooled chain is hard-capped at the seeded tool count (the S6 economy at scale).
fn capital_control() -> SettlementConfig {
    let mut cfg = SettlementConfig::frontier_capital();
    let chain = cfg.chain.as_mut().expect("chain");
    chain.tool_acquisition_eligibility = false;
    chain.producible_capital = false;
    cfg
}

/// S7.1 in isolation: the capital colony with the eligibility gate ON but the
/// autonomous build phase OFF, so a HANDED tool's adoption is observed without
/// autonomous builds interfering.
fn eligibility_only() -> SettlementConfig {
    let mut cfg = SettlementConfig::frontier_capital();
    cfg.chain.as_mut().expect("chain").producible_capital = false;
    cfg
}

fn chain_goods(cfg: &SettlementConfig) -> (GoodId, GoodId, GoodId, GoodId, GoodId) {
    let content = cfg.chain.as_ref().expect("chain").content.clone();
    (
        content.grain(),
        content.flour(),
        content.bread(),
        content.mill(),
        content.oven(),
    )
}

fn tools(s: &Settlement, mill: GoodId, oven: GoodId) -> u64 {
    s.whole_system_total(mill) + s.whole_system_total(oven)
}

fn living(s: &Settlement) -> usize {
    (0..s.population()).filter(|&i| s.is_alive(i)).count()
}

/// The first living, non-latent, non-producer spatial colonist (a gatherer/consumer).
/// Deterministic, so the same index is picked across runs of the same config.
fn pick_non_latent(s: &Settlement) -> usize {
    (0..s.population())
        .find(|&i| {
            s.is_alive(i)
                && !s.is_tool_acquisition_eligible(i)
                && matches!(
                    s.vocation_of(i),
                    Some(Vocation::Gatherer) | Some(Vocation::Consumer)
                )
        })
        .expect("a non-latent, non-producer spatial colonist")
}

#[test]
fn capital_run_is_deterministic() {
    // Acceptance 1: same (seed, config) → byte-identical canonical_bytes AND digest;
    // a different seed must diverge (the new build state + steering knobs are in the
    // determinism surface).
    let config = capital();
    let mut a = Settlement::generate(0xC0FFEE, &config);
    let mut b = Settlement::generate(0xC0FFEE, &config);
    a.run(800);
    b.run(800);
    assert_eq!(
        a.canonical_bytes(),
        b.canonical_bytes(),
        "same (seed, config) must be byte-identical with producible capital on"
    );
    assert_eq!(a.digest(), b.digest());

    let mut c = Settlement::generate(0xBADF00D, &config);
    c.run(800);
    assert_ne!(a.digest(), c.digest(), "the seed must matter");
}

#[test]
fn tool_acquisition_makes_a_colonist_eligible() {
    // Acceptance 2 (the keystone): a colonist that comes to hold a mill adopts Miller
    // and PRODUCES flour within N ticks; with S7.1 OFF it never does (proving the gate
    // relaxation, not a relabel).
    let (_grain, flour, _bread, mill, _oven) = chain_goods(&eligibility_only());

    // S7.1 ON: hand a non-latent colonist a mill mid-run; it adopts and mills flour.
    let on = eligibility_only();
    let mut s = Settlement::generate(7, &on);
    s.run(400);
    let idx = pick_non_latent(&s);
    let id = s.colonist_id(idx).expect("id");
    assert!(s.society_mut().credit_stock(id, mill, 1), "mill credited");
    assert!(
        s.is_tool_acquisition_eligible(idx),
        "holding the mill must make the non-latent colonist eligible"
    );
    let mut flour_made = 0u64;
    let mut adopted = false;
    for _ in 0..200 {
        let report = s.econ_tick();
        flour_made += report.produced_of(flour);
        if s.vocation_of(idx) == Some(Vocation::Miller) {
            adopted = true;
        }
    }
    assert!(
        adopted,
        "the eligible tool-holder must adopt Miller within N ticks"
    );
    assert!(
        flour_made > 0,
        "the adopted tool-holder must actually produce flour, got {flour_made}"
    );

    // S7.1 OFF: the same handed mill never makes the colonist a producer.
    let off = capital_control();
    let mut t = Settlement::generate(7, &off);
    t.run(400);
    let off_idx = pick_non_latent(&t);
    let off_id = t.colonist_id(off_idx).expect("id");
    let voc_before = t.vocation_of(off_idx);
    assert!(t.society_mut().credit_stock(off_id, mill, 1));
    assert!(
        !t.is_tool_acquisition_eligible(off_idx),
        "with S7.1 off, holding a mill must not make a non-latent colonist eligible"
    );
    t.run(200);
    assert_eq!(
        t.vocation_of(off_idx),
        voc_before,
        "with S7.1 off a handed mill must never turn a non-latent colonist into a producer"
    );
}

#[test]
fn acquired_tool_is_not_sold_before_adoption() {
    // Acceptance 3 (the phase-order/anchor guard): a colonist that gains a tool does
    // NOT sell it on the next market step before adopting — the whole-system tool count
    // does not drop, the would-be producer still holds it, and it adopts.
    let (_grain, _flour, _bread, mill, oven) = chain_goods(&eligibility_only());
    let cfg = eligibility_only();
    let mut s = Settlement::generate(7, &cfg);
    s.run(400);
    let idx = pick_non_latent(&s);
    let id = s.colonist_id(idx).expect("id");
    let tools_before = tools(&s, mill, oven);
    assert!(s.society_mut().credit_stock(id, mill, 1), "mill credited");

    // Step through several market clears. The tool count must never fall below the
    // handed level (the anchor protects the just-acquired capital from sale), and the
    // would-be producer must keep holding its mill.
    let mut adopted = false;
    for _ in 0..120 {
        s.econ_tick();
        assert!(
            tools(&s, mill, oven) > tools_before,
            "the acquired tool must not be sold (tool count dropped below the handed level)"
        );
        assert!(
            s.society().agents.get(id).expect("agent").stock.get(mill) >= 1,
            "the would-be producer must keep holding its mill through the market step"
        );
        if s.vocation_of(idx) == Some(Vocation::Miller) {
            adopted = true;
        }
    }
    assert!(
        adopted,
        "the protected tool-holder must go on to adopt Miller"
    );
}

#[test]
fn capital_is_built_under_demand_and_conserves() {
    // Acceptance 4: under sustained unmet bread demand a per-agent BuildMill/BuildOven
    // completes — the whole-system tool count rises, produced_of(tool) > 0, the WOOD is
    // booked to consumed_as_input, and conservation holds EVERY tick across the build.
    let cfg = capital();
    let (_grain, _flour, _bread, mill, oven) = chain_goods(&cfg);
    let mut s = Settlement::generate(1, &cfg);
    let tools_before = tools(&s, mill, oven);

    let mut wood_consumed_as_input = 0u64;
    let mut tool_produced = 0u64;
    for tick in 0..1200u64 {
        let report = s.econ_tick();
        assert!(
            report.conserves(),
            "whole-system conservation must hold every tick across a tool build, broke at {tick}"
        );
        // WOOD is consumed_as_input ONLY by a capital build (no recipe consumes WOOD).
        wood_consumed_as_input += report.consumed_as_input_of(WOOD);
        tool_produced += report.produced_of(mill) + report.produced_of(oven);
    }

    assert!(
        s.tools_built() > 0,
        "a builder must complete at least one tool under unmet demand, got {}",
        s.tools_built()
    );
    assert!(
        tool_produced > 0,
        "produced_of(tool) must be > 0 across the build, got {tool_produced}"
    );
    assert!(
        wood_consumed_as_input > 0,
        "the build must book its WOOD to consumed_as_input, got {wood_consumed_as_input}"
    );
    assert!(
        tools(&s, mill, oven) > tools_before,
        "whole-system tool count must rise above the seeded count"
    );
}

#[test]
fn building_is_individual_not_planned() {
    // Acceptance 5: the build is a per-colonist appraisal paid from the builder's OWN
    // endowment — no planner quota, no tool placement/transfer. Catch a colonist the
    // tick its build completes and verify (a) it was non-latent (a formerly-non-latent
    // adopter, marked), (b) it spent its OWN WOOD (its WOOD stock fell by the build cost
    // before completion), and (c) the whole-system tool count rose by exactly the builds
    // completed — nothing minted that was not built.
    let cfg = capital();
    let (_grain, _flour, _bread, mill, oven) = chain_goods(&cfg);
    let wood_cost = cfg.chain.as_ref().expect("chain").tool_build_wood;
    let mut s = Settlement::generate(1, &cfg);
    let seeded_tools = tools(&s, mill, oven);

    // Track each builder's WOOD at the moment a build starts (its WOOD drops), and that
    // the whole-system tool count only ever rises by builds it actually completed.
    let mut saw_own_wood_spent = false;
    let mut prev_built = s.tools_built();
    let mut prev_tools = seeded_tools;
    let mut prev_wood: Vec<u64> = (0..s.population()).map(|i| s.stock_of(i, WOOD)).collect();
    for _ in 0..1200u64 {
        s.econ_tick();
        // A completed build raises the whole-system tool count by exactly the number of
        // tools built this tick (one durable tool per completion) — nothing appears that
        // was not built (no placement/transfer minting).
        let built_now = s.tools_built() - prev_built;
        let tools_now = tools(&s, mill, oven);
        assert_eq!(
            tools_now,
            prev_tools + built_now,
            "the whole-system tool count must rise by exactly the tools built (no placement)"
        );
        // Some builder spent its OWN WOOD on a build (its deposited WOOD fell by at
        // least the build cost between ticks) — the WOOD came from its endowment.
        if prev_wood.len() < s.population() {
            prev_wood.resize(s.population(), 0);
        }
        for (i, prev) in prev_wood.iter_mut().enumerate() {
            let now = s.stock_of(i, WOOD);
            if prev.saturating_sub(now) >= u64::from(wood_cost) {
                saw_own_wood_spent = true;
            }
            *prev = now;
        }
        prev_built = s.tools_built();
        prev_tools = tools_now;
    }

    assert!(
        s.tools_built() > 0,
        "at least one tool must be built (the per-agent path must fire)"
    );
    assert!(
        saw_own_wood_spent,
        "a builder must pay the WOOD from its OWN deposited endowment"
    );
    let built_adopter = (0..s.population()).any(|i| s.acquired_tool_of(i));
    assert!(
        built_adopter,
        "a formerly-non-latent colonist must be the one that built a tool (not a seeded producer)"
    );
}

#[test]
fn bread_scales_with_capital() {
    // Acceptance 6 (THE clean metric): vs the no-build control on the SAME growing
    // colony, the build-enabled run ends with MORE tools, MORE active producers, and
    // HIGHER (and non-declining per-capita) bread output — AND a formerly-non-latent
    // colonist built a PRODUCED tool, adopted, BOUGHT its input by a real Society::trade,
    // and transformed it (so "more tools + more bread" is not a seeded/placement artifact).
    let (grain, flour, bread, mill, oven) = chain_goods(&capital());

    // Run a config to tick 1600, returning (total bread, early bread, late bread,
    // tools, summed active producers over the tail, living count).
    let measure = |cfg: &SettlementConfig| {
        let bread = cfg.chain.as_ref().expect("chain").content.bread();
        let mut s = Settlement::generate(1, cfg);
        let mut total = 0u64;
        let mut early = 0u64;
        let mut late = 0u64;
        let mut producer_tail = 0u64;
        for tick in 0..1600u64 {
            let report = s.econ_tick();
            total += report.produced_of(bread);
            if (400..600).contains(&tick) {
                early += report.produced_of(bread);
            }
            if (1400..1600).contains(&tick) {
                late += report.produced_of(bread);
            }
            if tick >= 1000 && tick % 10 == 0 {
                producer_tail +=
                    (s.living_count(Vocation::Miller) + s.living_count(Vocation::Baker)) as u64;
            }
        }
        let pop = living(&s).max(1) as u64;
        (
            total,
            early,
            late,
            tools(&s, mill, oven),
            producer_tail,
            pop,
        )
    };

    let (cap_total, cap_early, cap_late, cap_tools, cap_producers, cap_pop) = measure(&capital());
    let (ctl_total, _ctl_early, _ctl_late, ctl_tools, ctl_producers, ctl_pop) =
        measure(&capital_control());

    // MORE tools, MORE active producers, HIGHER bread (total and per capita).
    assert!(
        cap_tools > ctl_tools,
        "capital must end with more tools, got {cap_tools} vs control {ctl_tools}"
    );
    assert!(
        cap_producers > ctl_producers,
        "capital must sustain more active producers over the tail, got {cap_producers} vs \
         control {ctl_producers}"
    );
    assert!(
        cap_total > ctl_total,
        "capital must produce more bread, got {cap_total} vs control {ctl_total}"
    );
    let cap_per_capita = cap_total / cap_pop;
    let ctl_per_capita = ctl_total / ctl_pop;
    assert!(
        cap_per_capita > ctl_per_capita,
        "capital must produce more bread per capita, got {cap_per_capita} vs control \
         {ctl_per_capita}"
    );
    // Bread output is non-declining across the run (late window >= early window).
    assert!(
        cap_late >= cap_early,
        "capital bread output must not decline (late {cap_late} vs early {cap_early})"
    );

    // The hard "new capital entered the chain" assertion: a formerly-non-latent colonist
    // built a PRODUCED tool, adopted, BOUGHT its recipe input by a real Society::trade
    // (buyer != seller), and transformed it (the chain produced its output).
    let mut s = Settlement::generate(1, &capital());
    let _ = (mill, oven);
    let mut input_trade_by_built_adopter = false;
    let mut chain_output_produced = 0u64;
    let mut seen_trades = 0usize;
    for _ in 0..1600u64 {
        let report = s.econ_tick();
        chain_output_produced += report.produced_of(flour) + report.produced_of(bread);
        let trades = &s.society().trades;
        for trade in &trades[seen_trades..] {
            if trade.buyer == trade.seller {
                continue;
            }
            let Some(slot) = (0..s.population()).find(|&i| s.colonist_id(i) == Some(trade.buyer))
            else {
                continue;
            };
            if !s.acquired_tool_of(slot) {
                continue;
            }
            let bought_input = (trade.good == grain
                && s.vocation_of(slot) == Some(Vocation::Miller))
                || (trade.good == flour && s.vocation_of(slot) == Some(Vocation::Baker));
            if bought_input {
                input_trade_by_built_adopter = true;
            }
        }
        seen_trades = trades.len();
    }
    assert!(
        (0..s.population()).any(|i| s.acquired_tool_of(i)),
        "a formerly-non-latent colonist must have built a produced tool"
    );
    assert!(
        input_trade_by_built_adopter,
        "a built-tool adopter must have bought its recipe input by a real Society::trade"
    );
    assert!(
        chain_output_produced > 0,
        "the built capital must have transformed its input (the chain produced output)"
    );
}

#[test]
fn no_overinvestment_in_capital() {
    // Acceptance 7: real-resource investment responds to demand and STOPS. Once bread
    // demand is met the per-run margin falls below the payback bar, so the tool count
    // and producer count stabilize (no unbounded idle-tool overbuild) and whole-system
    // WOOD is not drained by speculative building.
    let cfg = capital();
    let (_grain, _flour, _bread, mill, oven) = chain_goods(&cfg);
    let mut s = Settlement::generate(1, &cfg);

    // Snapshot tools/producers/WOOD at three tail boundaries.
    let snap = |s: &Settlement| {
        (
            tools(s, mill, oven),
            s.living_count(Vocation::Miller) + s.living_count(Vocation::Baker),
            s.whole_system_total(WOOD),
            s.tools_built(),
        )
    };
    s.run(800);
    let (tools_a, prod_a, wood_a, built_a) = snap(&s);
    s.run(400);
    let (tools_b, prod_b, wood_b, built_b) = snap(&s);
    s.run(400);
    let (tools_c, prod_c, wood_c, built_c) = snap(&s);

    // The tool count is BOUNDED across the tail (it does not keep climbing): the last
    // window's growth is no larger than a small fraction of the count — building has
    // essentially stopped once demand is met.
    assert!(
        tools_c <= tools_a + tools_a / 4 + 2,
        "tools must stabilize across the tail, got {tools_a} -> {tools_b} -> {tools_c}"
    );
    assert!(
        built_c.saturating_sub(built_b) <= built_b.saturating_sub(built_a).max(2),
        "the build RATE must not accelerate in the tail (overbuild), built {built_a} -> \
         {built_b} -> {built_c}"
    );
    // Active producers stay bounded (no runaway producer count).
    assert!(
        prod_c <= prod_a + prod_a / 2 + 3,
        "active producers must stabilize, got {prod_a} -> {prod_b} -> {prod_c}"
    );
    // WOOD is not drained by speculative building: the tail WOOD does not collapse.
    assert!(
        wood_c * 2 >= wood_a,
        "whole-system WOOD must not be drained by building, got {wood_a} -> {wood_b} -> {wood_c}"
    );
}

#[test]
fn s5_s6_unchanged() {
    // Acceptance 8: with S7 OFF the `endogenous` (S5) and `scaling` (S6) scenarios are
    // byte-identical regardless of the (unused) S7 knobs — the gated, additive seam is
    // inert. (The six econ conformance goldens, the endogenous_economy and
    // provisioning_at_scale suites, the canonical_bytes_include_* digest regressions in
    // settlement.rs, clippy `-D warnings`, and fmt `--check` are the workspace gate that
    // enforces the rest.)
    for base in [
        SettlementConfig::frontier_endogenous(),
        SettlementConfig::frontier_endogenous_scaling(),
    ] {
        assert!(
            !base.chain.as_ref().unwrap().tool_acquisition_eligibility
                && !base.chain.as_ref().unwrap().producible_capital,
            "the S5/S6 scenarios must keep both S7 gates off"
        );
        // The unused S7 knobs cannot steer a phase that never runs.
        let mut other = base.clone();
        {
            let c = other.chain.as_mut().unwrap();
            c.capital_payback_cycles = 1;
            c.tool_build_wood = 99;
            c.tool_build_labor = 99;
            c.capital_build_hunger_max = 99;
        }
        let mut a = Settlement::generate(0xC0FFEE, &base);
        let mut b = Settlement::generate(0xC0FFEE, &other);
        a.run(600);
        b.run(600);
        assert_eq!(
            a.canonical_bytes(),
            b.canonical_bytes(),
            "with S7 off the run must be byte-identical regardless of the unused build knobs"
        );
        assert_eq!(a.digest(), b.digest());
    }
}
