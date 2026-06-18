//! S5 — the endogenous specialization economy (the DoD acceptance suite).
//!
//! The grain→flour→bread division of labor emerges atop a household/subsistence
//! base and SUSTAINS on real market trade, with NO chain-specific global placement
//! of food or inputs (`subsistence_advance`, `input_advance`, and the per-tick
//! capital-advance loan are all OFF). Local/household allocation — the demography
//! hearth, each producer's own subsistence hearth, and the edible-grain fallback —
//! is allowed and is not scaffolding.
//!
//! These are the sharpened acceptance tests from
//! `docs/impl-endogenous-specialization.md`.

use econ::scenario::{builtin_market_scenario, ScenarioName};
use sim::{Settlement, SettlementConfig, Society, Vocation};

fn endogenous() -> SettlementConfig {
    SettlementConfig::frontier_endogenous()
}

#[test]
fn endogenous_run_is_deterministic() {
    let config = endogenous();
    let mut a = Settlement::generate(0xC0FFEE, &config);
    let mut b = Settlement::generate(0xC0FFEE, &config);
    a.run(800);
    b.run(800);
    assert_eq!(
        a.canonical_bytes(),
        b.canonical_bytes(),
        "same (seed, config) must be byte-identical"
    );
    assert_eq!(a.digest(), b.digest());

    let mut c = Settlement::generate(0xBADF00D, &config);
    c.run(800);
    assert_ne!(a.digest(), c.digest(), "the seed must matter");
}

#[test]
fn inputs_acquired_by_market_trade() {
    // THE clean metric. With every chain-specific global-placement phase OFF, after
    // tick 300 there must exist real `Society::trades` records where an active
    // Miller/Baker bought its recipe input (grain/flour) from a DIFFERENT seller,
    // the acquired input is LATER consumed as a recipe input, and NO transfer or
    // placement counter moved those units (gold moving alone is insufficient).
    let config = endogenous();
    let chain = config.chain.as_ref().expect("chain");
    // No chain-specific global placement is in play.
    assert!(
        !chain.input_advance && !chain.subsistence_advance && !chain.capital_advance,
        "the endogenous DoD must run with all global placement OFF"
    );
    let content = chain.content.clone();
    let grain = content.grain();
    let flour = content.flour();

    let mut settlement = Settlement::generate(1, &config);

    // A qualifying input trade by an active producer, recorded as it happens (so
    // the buyer's vocation is the one it traded under — role-choice runs at the
    // top of the next tick).
    let mut producer_input_trades = 0u64;
    let mut grain_consumed_as_input = 0u64;
    let mut flour_consumed_as_input = 0u64;
    let mut seen = 0usize;
    for tick in 0..400u64 {
        let report = settlement.econ_tick();
        // No placement/minting of the chain inputs ever: the demography and
        // producer hearths mint only the staple + WOOD, never grain or flour, so a
        // producer's grain/flour can only have arrived by a market Trade.
        assert_eq!(
            report.endowment_of(grain),
            0,
            "no hearth should ever mint grain (tick {tick})"
        );
        assert_eq!(
            report.endowment_of(flour),
            0,
            "no hearth should ever mint flour (tick {tick})"
        );
        // Count input consumption only in the SAME tail window the qualifying
        // trades fall in (tick >= 300). By then the seeded cold-start buffers are
        // long gone and grain/flour are never minted (asserted above), so any input
        // consumed here must have arrived through a market Trade — proving the
        // tail-bought units are the ones being transformed, not pre-seeded stock.
        if tick >= 300 {
            grain_consumed_as_input += report.consumed_as_input_of(grain);
            flour_consumed_as_input += report.consumed_as_input_of(flour);
        }

        let trades = &settlement.society().trades;
        for trade in &trades[seen..] {
            if trade.tick < 300 || trade.buyer == trade.seller {
                continue;
            }
            let bought_input = (trade.good == grain
                && matches!(
                    settlement.vocation_of_id(trade.buyer),
                    Some(Vocation::Miller)
                ))
                || (trade.good == flour
                    && matches!(
                        settlement.vocation_of_id(trade.buyer),
                        Some(Vocation::Baker)
                    ));
            if bought_input {
                producer_input_trades += 1;
            }
        }
        seen = trades.len();
    }

    assert!(
        producer_input_trades > 0,
        "after tick 300 an active producer must acquire its input through a real \
         order-book Trade from a different seller, got {producer_input_trades}"
    );
    // The TAIL-acquired inputs are actually transformed (consumed as recipe inputs
    // after tick 300, when only market-sourced units exist).
    assert!(
        grain_consumed_as_input > 0 && flour_consumed_as_input > 0,
        "the market-bought inputs must be consumed as recipe inputs after tick 300 \
         (grain milled, flour baked), got grain={grain_consumed_as_input} \
         flour={flour_consumed_as_input}"
    );
}

#[test]
fn specialization_sustains() {
    // bread.made > 0 through tick 800 AND tick 1600 — the chain is still producing
    // at both horizons, not collapsed after the cold start.
    let config = endogenous();
    let bread = config.chain.as_ref().unwrap().content.bread();
    let mut settlement = Settlement::generate(1, &config);

    let mut made_700_800 = 0u64;
    let mut made_1500_1600 = 0u64;
    for tick in 0..1600u64 {
        let produced = settlement.econ_tick().produced_of(bread);
        if (700..800).contains(&tick) {
            made_700_800 += produced;
        }
        if (1500..1600).contains(&tick) {
            made_1500_1600 += produced;
        }
    }
    assert!(
        made_700_800 > 0,
        "the chain should still be producing bread approaching tick 800, got \
         {made_700_800}"
    );
    assert!(
        made_1500_1600 > 0,
        "the chain should still be producing bread approaching tick 1600, got \
         {made_1500_1600}"
    );
}

#[test]
fn hunger_and_provisioning_are_stationary() {
    // Over tail windows: population trend, bread produced PER CAPITA, and hunger
    // mean/p95 are bounded and non-drifting (distinguishing a stable economy from a
    // managed decline), and no hidden drawdown of initial food buffers.
    let config = endogenous();
    let content = config.chain.as_ref().unwrap().content.clone();
    let (grain, bread) = (content.grain(), content.bread());
    let mut s = Settlement::generate(1, &config);

    let living =
        |s: &Settlement| -> usize { (0..s.population()).filter(|&i| s.is_alive(i)).count() };
    let hunger_stats = |s: &Settlement| -> (u64, u16) {
        let mut h: Vec<u16> = (0..s.population())
            .filter(|&i| s.is_alive(i))
            .filter_map(|i| s.need_of(i).map(|n| n.hunger))
            .collect();
        h.sort_unstable();
        if h.is_empty() {
            return (0, 0);
        }
        let mean = h.iter().map(|&x| u64::from(x)).sum::<u64>() / h.len() as u64;
        let p95 = h[(h.len() * 95 / 100).min(h.len() - 1)];
        (mean, p95)
    };

    // Three equal 300-tick tail windows: [700,1000), [1000,1300), [1300,1600).
    let mut window_bread = [0u64; 3];
    let mut samples: Vec<(usize, u64, u16, u64, u64)> = Vec::new(); // pop, hMean, hP95, sysBread, sysGrain
    for tick in 0..1600u64 {
        let produced = s.econ_tick().produced_of(bread);
        if (700..1000).contains(&tick) {
            window_bread[0] += produced;
        } else if (1000..1300).contains(&tick) {
            window_bread[1] += produced;
        } else if (1300..1600).contains(&tick) {
            window_bread[2] += produced;
        }
        if matches!(tick + 1, 800 | 1200 | 1600) {
            let (hm, hp) = hunger_stats(&s);
            samples.push((
                living(&s),
                hm,
                hp,
                s.whole_system_total(bread),
                s.whole_system_total(grain),
            ));
        }
    }

    // Population bounded across the tail (no collapse, no explosion).
    for (pop, ..) in &samples {
        assert!(
            (10..=50).contains(pop),
            "population should stay bounded, got {pop} in {samples:?}"
        );
    }
    // Population non-drifting: first vs last tail sample within a tight band.
    let pop_first = samples.first().unwrap().0 as i64;
    let pop_last = samples.last().unwrap().0 as i64;
    assert!(
        (pop_first - pop_last).abs() <= pop_first / 2,
        "population should not drift, got {pop_first} -> {pop_last}"
    );

    // Per-capita bread bounded and non-drifting across the three tail windows.
    let pop = samples.last().unwrap().0.max(1) as u64;
    for (i, &b) in window_bread.iter().enumerate() {
        let per_cap = b / pop;
        assert!(
            per_cap > 0,
            "tail window {i} should still produce bread per capita, got {per_cap} \
             (window={b}, pop={pop})"
        );
    }
    let w_first = window_bread[0].max(1);
    let w_last = window_bread[2].max(1);
    assert!(
        w_last * 4 >= w_first && w_first * 4 >= w_last,
        "per-capita bread should not drift (within 4x across tail windows), got \
         {window_bread:?}"
    );

    // Hunger mean/p95 bounded and non-drifting. (samples tuple is
    // (pop, hungerMean, hungerP95, sysBread, sysGrain).)
    let (_, m_first, _, _, _) = samples[0];
    let (_, m_last, p_last, _, _) = *samples.last().unwrap();
    // The colony is well-fed in the mean (live tail mean ~3, far below the ~8
    // chronic-hunger level), while the worst-off 5% run hotter (p95 ~12) — honest
    // tail inequality from the churning non-lineage producers. Bound both so the
    // test enforces a genuinely fed colony, not merely a non-exploding one.
    assert!(
        m_last <= 7 && p_last <= 13,
        "hunger should stay bounded (mean well below chronic ~8; p95 tail capped), \
         got mean={m_last} p95={p_last}"
    );
    assert!(
        (m_first as i64 - m_last as i64).abs() <= 3,
        "hunger mean should not drift, got {m_first} -> {m_last}"
    );

    // No hidden drawdown of initial food buffers: the system food stock at the end
    // is not a fraction of its mid-run level (it is bounded by spoilage, not
    // drained to nothing — the colony lives on production + regen, not a buffer).
    let (_, _, _, sys_bread_mid, sys_grain_mid) = samples[0];
    let (_, _, _, sys_bread_end, sys_grain_end) = *samples.last().unwrap();
    assert!(
        sys_bread_end * 4 >= sys_bread_mid && sys_grain_end * 2 >= sys_grain_mid,
        "no hidden food drawdown: end food should not collapse vs mid-run, got \
         bread {sys_bread_mid}->{sys_bread_end} grain {sys_grain_mid}->{sys_grain_end}"
    );
}

#[test]
fn endogenous_conserves() {
    // Whole-system conservation every tick across all the new flows (override bids,
    // trades, the producer-subsistence hearth, spoilage, working capital).
    let config = endogenous();
    let mut settlement = Settlement::generate(1, &config);
    for tick in 0..1600u64 {
        assert!(
            settlement.econ_tick().conserves(),
            "whole-system conservation must hold every tick, broke at {tick}"
        );
    }
}

#[test]
fn econ_unchanged() {
    // The S1 disabled-hook regression at the econ boundary: running an econ
    // conformance scenario while touching the (empty) bid-override hook every tick
    // is byte-identical to never touching it — the additive, gated seam is inert
    // when no override is set. (The six byte-identical conformance goldens — econ
    // m5/m6/m7/m8/m9 — plus clippy `-D warnings` and `fmt --check` are the
    // workspace gate that enforces the rest of this property.)
    let run = |touch_hook: bool| {
        let mut society =
            Society::from_scenario(builtin_market_scenario(ScenarioName::MarketPriceDiscovery));
        for _ in 0..24 {
            if touch_hook {
                society.clear_bid_overrides();
            }
            society.step();
        }
        (society.records, society.trades)
    };
    let (plain_records, plain_trades) = run(false);
    let (hooked_records, hooked_trades) = run(true);
    assert_eq!(
        plain_records, hooked_records,
        "the unused override hook must not change econ market records"
    );
    assert_eq!(
        plain_trades, hooked_trades,
        "the unused override hook must not change econ trades"
    );
}
