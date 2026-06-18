//! S6.3 — provisioning at scale via productive re-entry (the DoD acceptance suite).
//!
//! The `scaling` scenario composes the endogenous economy with the gated
//! productive-re-entry phase (S6.1) and its hysteresis (S6.2), on a larger/growing
//! colony. The honest bar: no colonist is permanently stranded at high hunger — a
//! hungry, unprovisioned colonist takes up edible gathering and feeds itself — while
//! the S5 bread chain is NOT regressed and the WOOD supply is not starved.
//!
//! These are the eight named acceptance tests from
//! `docs/impl-provisioning-at-scale.md`.

use econ::scenario::{builtin_market_scenario, ScenarioName};
use sim::{GoodId, Settlement, SettlementConfig, Society, Vocation, WOOD};

fn scaling() -> SettlementConfig {
    SettlementConfig::frontier_endogenous_scaling()
}

fn endogenous() -> SettlementConfig {
    SettlementConfig::frontier_endogenous()
}

/// The S6 entry threshold of a config (the hunger level at/above which a colonist
/// is considered high-hunger and re-entry fires).
fn h_in(config: &SettlementConfig) -> u16 {
    config.chain.as_ref().expect("chain").reentry_hunger_in
}

fn grain_bread(config: &SettlementConfig) -> (GoodId, GoodId, GoodId) {
    let content = config.chain.as_ref().expect("chain").content.clone();
    (content.grain(), content.flour(), content.bread())
}

/// The living colonist count.
fn living(s: &Settlement) -> usize {
    (0..s.population()).filter(|&i| s.is_alive(i)).count()
}

/// (mean, p95, max, chronically-hungry count) over the living roster, where
/// "chronic" is hunger ≥ `threshold`.
fn hunger_stats(s: &Settlement, threshold: u16) -> (u64, u16, u16, usize) {
    let mut h: Vec<u16> = (0..s.population())
        .filter(|&i| s.is_alive(i))
        .filter_map(|i| s.need_of(i).map(|n| n.hunger))
        .collect();
    h.sort_unstable();
    if h.is_empty() {
        return (0, 0, 0, 0);
    }
    let mean = h.iter().map(|&x| u64::from(x)).sum::<u64>() / h.len() as u64;
    let p95 = h[(h.len() * 95 / 100).min(h.len() - 1)];
    let max = *h.last().unwrap();
    let chronic = h.iter().filter(|&&x| x >= threshold).count();
    (mean, p95, max, chronic)
}

#[test]
fn re_entry_run_is_deterministic() {
    // Acceptance 1: same (seed, config) → byte-identical canonical_bytes AND digest;
    // a different seed must diverge (the seed matters).
    let config = scaling();
    let mut a = Settlement::generate(0xC0FFEE, &config);
    let mut b = Settlement::generate(0xC0FFEE, &config);
    a.run(800);
    b.run(800);
    assert_eq!(
        a.canonical_bytes(),
        b.canonical_bytes(),
        "same (seed, config) must be byte-identical under the re-entry phase"
    );
    assert_eq!(a.digest(), b.digest());

    let mut c = Settlement::generate(0xBADF00D, &config);
    c.run(800);
    assert_ne!(a.digest(), c.digest(), "the seed must matter");
}

#[test]
fn no_colonist_is_permanently_stranded() {
    // Acceptance 2 (THE clean metric): over the tail, EVERY living colonist that is
    // high-hunger (hunger ≥ H_in) for K consecutive sampled ticks must be ACTUALLY
    // obtaining food — on the edible grain node while holding/gathering grain, or on
    // a hearth (a lineage member, or a chain producer fed by its own subsistence
    // floor). A WOOD gatherer pinned at hunger 12 is STILL stranded, not a pass.
    let config = scaling();
    let threshold = h_in(&config);
    let (grain, _flour, _bread) = grain_bread(&config);
    let mut s = Settlement::generate(1, &config);
    let grain_node = s.grain_node();

    s.run(1200);

    // Sample the last 400 ticks every 25 ticks (16 samples). For each colonist index
    // record, per sample where it is alive: is it high-hunger, and is it "feeding"
    // (on the grain node holding grain, or hearth-fed / a producer)?
    const STEP: u64 = 25;
    const SAMPLES: usize = 16;
    const K: usize = 3; // consecutive high-hunger samples (~75 ticks) = "permanent"

    // Per colonist index: a list of (high_hunger, feeding) for each sample it lived.
    let mut hi_run = vec![0usize; 0]; // current consecutive high-hunger streak
    let mut hi_run_unfed = vec![0usize; 0]; // streak where it was high-hunger AND not feeding
    let mut grain_seen = vec![false; 0]; // ever held/carried grain while a grain gatherer

    let ensure = |v: &mut Vec<usize>, n: usize| {
        if v.len() < n {
            v.resize(n, 0);
        }
    };
    let ensure_b = |v: &mut Vec<bool>, n: usize| {
        if v.len() < n {
            v.resize(n, false);
        }
    };

    let mut worst_unfed_streak = 0usize;
    let mut offender = String::new();

    for _ in 0..SAMPLES {
        for _ in 0..STEP {
            s.econ_tick();
        }
        let pop = s.population();
        ensure(&mut hi_run, pop);
        ensure(&mut hi_run_unfed, pop);
        ensure_b(&mut grain_seen, pop);
        for i in 0..pop {
            if !s.is_alive(i) {
                hi_run[i] = 0;
                hi_run_unfed[i] = 0;
                continue;
            }
            let hunger = s.need_of(i).map(|n| n.hunger).unwrap_or(0);
            let on_grain =
                s.vocation_of(i) == Some(Vocation::Gatherer) && s.node_of(i) == grain_node;
            let grain_held = s.stock_of(i, grain) + u64::from(s.carry_of(i, grain));
            if on_grain && grain_held > 0 {
                grain_seen[i] = true;
            }
            let hearth_fed = s.household_of(i).is_some()
                || matches!(
                    s.vocation_of(i),
                    Some(Vocation::Miller) | Some(Vocation::Baker)
                );
            // "feeding": on the edible node and actually holding/gathering grain, or
            // fed by a hearth. A relabeled grain gatherer that never holds grain does
            // NOT count.
            let feeding = hearth_fed || (on_grain && grain_seen[i]);
            if hunger >= threshold {
                hi_run[i] += 1;
                hi_run_unfed[i] = if feeding { 0 } else { hi_run_unfed[i] + 1 };
            } else {
                hi_run[i] = 0;
                hi_run_unfed[i] = 0;
            }
            if hi_run_unfed[i] > worst_unfed_streak {
                worst_unfed_streak = hi_run_unfed[i];
                offender = format!(
                    "idx {i}: hunger={hunger} voc={:?} node==grain={} grainHeld={grain_held} \
                     household={:?}",
                    s.vocation_of(i),
                    on_grain,
                    s.household_of(i)
                );
            }
        }
    }

    assert!(
        worst_unfed_streak < K,
        "a colonist was high-hunger (>= {threshold}) and NOT obtaining food for \
         {worst_unfed_streak} consecutive samples (>= {K} means permanently stranded): \
         {offender}"
    );
}

#[test]
fn hunger_tail_is_reduced_not_moved() {
    // Acceptance 3: vs the endogenous baseline, the tail is FIXED, not relocated —
    // p95, max (max_living_hunger) AND the count of chronically-hungry colonists all
    // drop, and stay non-drifting first-vs-last tail window.
    let tail_stats = |config: &SettlementConfig| -> ((u16, u16, usize), (u16, u16, usize)) {
        let threshold = h_in(config);
        let mut s = Settlement::generate(1, config);
        // First tail window stats at tick 1000, last at tick 1600.
        s.run(1000);
        let (_, p95_a, _, chronic_a) = hunger_stats(&s, threshold);
        let max_a = s.max_living_hunger();
        s.run(600);
        let (_, p95_b, _, chronic_b) = hunger_stats(&s, threshold);
        let max_b = s.max_living_hunger();
        ((p95_a, max_a, chronic_a), (p95_b, max_b, chronic_b))
    };

    let base_threshold = h_in(&endogenous());
    let (base_first, base_last) = tail_stats(&endogenous());
    let (scal_first, scal_last) = tail_stats(&scaling());

    // The endogenous baseline has a real stranded tail to beat (p95 and max pinned at
    // the hunger ceiling, several chronically-hungry colonists).
    assert!(
        base_last.0 >= base_threshold && base_last.1 >= base_threshold && base_last.2 > 0,
        "the endogenous baseline must have a stranded tail to fix, got {base_last:?}"
    );

    // Scaling reduces ALL THREE versus the baseline (worst tail window each).
    let base_p95 = base_first.0.max(base_last.0);
    let base_max = base_first.1.max(base_last.1);
    let base_chronic = base_first.2.max(base_last.2);
    let scal_p95 = scal_first.0.max(scal_last.0);
    let scal_max = scal_first.1.max(scal_last.1);
    let scal_chronic = scal_first.2.max(scal_last.2);
    assert!(
        scal_p95 < base_p95,
        "tail p95 must drop, got scaling {scal_p95} vs baseline {base_p95}"
    );
    assert!(
        scal_max < base_max,
        "tail max must drop, got scaling {scal_max} vs baseline {base_max}"
    );
    assert!(
        scal_chronic < base_chronic,
        "the chronically-hungry count must drop, got scaling {scal_chronic} vs baseline \
         {base_chronic}"
    );

    // And the scaling tail is non-drifting first-vs-last window: it does not creep
    // back up toward the baseline.
    assert!(
        scal_last.0 <= scal_first.0 + 4 && scal_last.1 <= scal_first.1 + 4,
        "the scaling tail must not drift upward, got first {scal_first:?} -> last {scal_last:?}"
    );
}

#[test]
fn provisioning_tracks_population() {
    // Acceptance 4: with the larger/growing colony, per-capita food EATEN (grain +
    // bread) and the hunger tail stay bounded as population rises across tail windows.
    let config = scaling();
    let threshold = h_in(&config);
    let (grain, _flour, bread) = grain_bread(&config);
    let mut s = Settlement::generate(1, &config);

    // Three equal 200-tick tail windows: [1000,1200), [1200,1400), [1400,1600).
    let mut window_food = [0u64; 3];
    let mut window_pop = [0usize; 3];
    let mut window_p95_sum = [0u64; 3];
    let mut window_samples = [0usize; 3];
    for tick in 0..1600u64 {
        let report = s.econ_tick();
        let w = if (1000..1200).contains(&tick) {
            Some(0)
        } else if (1200..1400).contains(&tick) {
            Some(1)
        } else if (1400..1600).contains(&tick) {
            Some(2)
        } else {
            None
        };
        if let Some(w) = w {
            window_food[w] += report.consumed_of(grain) + report.consumed_of(bread);
            window_pop[w] += living(&s);
            let (_, p95, _, _) = hunger_stats(&s, threshold);
            window_p95_sum[w] += u64::from(p95);
            window_samples[w] += 1;
        }
    }

    // Population over the scaling tail is genuinely larger than the endogenous
    // plateau (the colony grew), so provisioning is exercised at scale.
    let mut base = Settlement::generate(1, &endogenous());
    base.run(1600);
    let scaling_pop = living(&s);
    assert!(
        scaling_pop > living(&base),
        "the scaling colony must grow past the endogenous plateau, got {scaling_pop} vs {}",
        living(&base)
    );

    // Per-capita food eaten is positive and non-drifting across the three tail
    // windows (provisioning keeps pace; no managed decline, no runaway).
    let per_capita: Vec<u64> = (0..3)
        .map(|w| {
            let avg_pop = (window_pop[w] / window_samples[w].max(1)).max(1) as u64;
            window_food[w] / avg_pop
        })
        .collect();
    for (w, &pc) in per_capita.iter().enumerate() {
        assert!(
            pc > 0,
            "tail window {w} must still feed the colony per capita, got {pc} ({per_capita:?})"
        );
    }
    let first = per_capita[0].max(1);
    let last = per_capita[2].max(1);
    assert!(
        last * 4 >= first && first * 4 >= last,
        "per-capita food eaten must not drift (within 4x across tail windows), got {per_capita:?}"
    );

    // The hunger tail stays bounded and non-drifting as the population rises: the
    // window-average p95 stays at or below the chronic threshold (a transient
    // per-tick spike as a freshly-hungry colonist is re-pointed is churn, not
    // stranding — Test 2 proves no one is PERMANENTLY stranded).
    let window_p95: Vec<u64> = (0..3)
        .map(|w| window_p95_sum[w] / window_samples[w].max(1) as u64)
        .collect();
    for (w, &p95) in window_p95.iter().enumerate() {
        assert!(
            p95 <= u64::from(threshold),
            "the window-average hunger p95 must stay bounded (<= H_in {threshold}) as \
             population rises, got {p95} in window {w} ({window_p95:?})"
        );
    }
    assert!(
        window_p95[2] <= window_p95[0] + 4,
        "the hunger tail must not drift upward across tail windows, got {window_p95:?}"
    );
}

#[test]
fn bread_chain_does_not_regress() {
    // Acceptance 5 (S5 preserved): with re-entry ON, bread.made stays > 0 through
    // tick 1600, Miller/Baker adoption does not collapse, and real grain/flour input
    // trades by active producers still occur after tick 300. Guards the
    // subsistence↔specialization tension: mass raw-grain eating must not gut bread
    // demand, and a grain glut must not de-adopt the millers.
    let config = scaling();
    let (grain, flour, bread) = grain_bread(&config);
    let mut s = Settlement::generate(1, &config);

    let mut bread_made_late = 0u64;
    let mut producer_input_trades = 0u64;
    let mut grain_consumed_as_input = 0u64;
    let mut flour_consumed_as_input = 0u64;
    let mut seen = 0usize;
    // Sample Miller/Baker adoption across the tail.
    let mut miller_present = 0usize;
    let mut baker_present = 0usize;
    let mut tail_samples = 0usize;

    for tick in 0..1600u64 {
        let report = s.econ_tick();
        if (1500..1600).contains(&tick) {
            bread_made_late += report.produced_of(bread);
        }
        if tick >= 300 {
            grain_consumed_as_input += report.consumed_as_input_of(grain);
            flour_consumed_as_input += report.consumed_as_input_of(flour);
        }
        let trades = &s.society().trades;
        for trade in &trades[seen..] {
            if trade.tick < 300 || trade.buyer == trade.seller {
                continue;
            }
            let bought_input = (trade.good == grain
                && matches!(s.vocation_of_id(trade.buyer), Some(Vocation::Miller)))
                || (trade.good == flour
                    && matches!(s.vocation_of_id(trade.buyer), Some(Vocation::Baker)));
            if bought_input {
                producer_input_trades += 1;
            }
        }
        seen = trades.len();
        if tick >= 1200 && tick % 20 == 0 {
            tail_samples += 1;
            if s.living_count(Vocation::Miller) > 0 {
                miller_present += 1;
            }
            if s.living_count(Vocation::Baker) > 0 {
                baker_present += 1;
            }
        }
    }

    assert!(
        bread_made_late > 0,
        "the chain must still produce bread approaching tick 1600 with re-entry on, got \
         {bread_made_late}"
    );
    // Adoption does not collapse: each role is adopted across a substantial fraction
    // of the tail (not permanently de-adopted by the grain glut / raw-grain eating).
    assert!(
        miller_present * 2 >= tail_samples,
        "Miller adoption must not collapse, present in {miller_present}/{tail_samples} tail samples"
    );
    assert!(
        baker_present * 2 >= tail_samples,
        "Baker adoption must not collapse, present in {baker_present}/{tail_samples} tail samples"
    );
    // Real market input trades by active producers still occur (the S5 clean metric).
    assert!(
        producer_input_trades > 0,
        "an active producer must still acquire its input through a real order-book trade \
         after tick 300, got {producer_input_trades}"
    );
    assert!(
        grain_consumed_as_input > 0 && flour_consumed_as_input > 0,
        "the bought inputs must still be transformed (grain milled, flour baked) after \
         tick 300, got grain={grain_consumed_as_input} flour={flour_consumed_as_input}"
    );
}

#[test]
fn wood_supply_does_not_collapse() {
    // Acceptance 6: WOOD does not become the new tail. The S6.2 hysteresis returns
    // fed colonists to WOOD gathering, so WOOD gatherers stay active, WOOD is gathered
    // every tail window (non-drifting), and the whole-system WOOD stock does not
    // collapse across the tail.
    let config = scaling();
    let (_grain, _flour, _bread) = grain_bread(&config);
    let mut s = Settlement::generate(1, &config);
    let grain_node = s.grain_node();

    // WOOD gathered (node regen) per 200-tick tail window, and the live WOOD-gatherer
    // count + whole-system WOOD at the window boundaries.
    let mut window_wood_regen = [0u64; 3];
    let mut wood_held = [0u64; 4];
    let mut wood_gatherers = [0usize; 4];
    let boundary = |s: &Settlement, grain_node| {
        let wg = (0..s.population())
            .filter(|&i| {
                s.is_alive(i)
                    && s.vocation_of(i) == Some(Vocation::Gatherer)
                    && s.node_of(i) != grain_node
            })
            .count();
        (s.whole_system_total(WOOD), wg)
    };

    for tick in 0..1600u64 {
        if tick == 1000 {
            let (h, wg) = boundary(&s, grain_node);
            wood_held[0] = h;
            wood_gatherers[0] = wg;
        }
        let report = s.econ_tick();
        let w = if (1000..1200).contains(&tick) {
            Some(0)
        } else if (1200..1400).contains(&tick) {
            Some(1)
        } else if (1400..1600).contains(&tick) {
            Some(2)
        } else {
            None
        };
        if let Some(w) = w {
            window_wood_regen[w] += report.regen_of(WOOD);
        }
        if matches!(tick + 1, 1200 | 1400 | 1600) {
            let idx = ((tick + 1 - 1200) / 200 + 1) as usize;
            let (h, wg) = boundary(&s, grain_node);
            wood_held[idx] = h;
            wood_gatherers[idx] = wg;
        }
    }

    // WOOD gatherers stay active at every tail boundary (the hysteresis keeps them).
    for (i, &wg) in wood_gatherers.iter().enumerate() {
        assert!(
            wg > 0,
            "WOOD gatherers must stay active across the tail (boundary {i} had {wg}), \
             wood_gatherers={wood_gatherers:?}"
        );
    }
    // WOOD is gathered every tail window, and the gathering rate is non-drifting.
    for (w, &regen) in window_wood_regen.iter().enumerate() {
        assert!(
            regen > 0,
            "WOOD must keep being gathered in tail window {w}, got {regen} \
             ({window_wood_regen:?})"
        );
    }
    let first = window_wood_regen[0].max(1);
    let last = window_wood_regen[2].max(1);
    assert!(
        last * 4 >= first && first * 4 >= last,
        "WOOD gathering must not drift across tail windows, got {window_wood_regen:?}"
    );
    // The whole-system WOOD stock does not collapse across the tail.
    assert!(
        *wood_held.last().unwrap() * 2 >= wood_held[0],
        "whole-system WOOD must not collapse across the tail, got {wood_held:?}"
    );
}

#[test]
fn re_entry_conserves() {
    // Acceptance 7: whole-system conservation holds every tick across the new phase —
    // the re-entry flip mints nothing; gathering is the existing conserved node-regen
    // source.
    let config = scaling();
    let mut s = Settlement::generate(1, &config);
    for tick in 0..1600u64 {
        assert!(
            s.econ_tick().conserves(),
            "whole-system conservation must hold every tick with re-entry on, broke at {tick}"
        );
    }
}

#[test]
fn endogenous_unchanged() {
    // Acceptance 8: with the re-entry phase OFF, the `endogenous` scenario is
    // byte-identical to the pre-S6 economy. The phase is default-OFF there and the
    // gated, additive seam is inert: a run is byte-identical regardless of the
    // (unused) re-entry thresholds, and deterministic across reruns. (The six econ
    // conformance goldens, the full endogenous_economy.rs suite, clippy `-D warnings`
    // and `fmt --check` are the workspace gate that enforces the rest.)
    let off = endogenous();
    assert!(
        !off.chain.as_ref().unwrap().productive_reentry,
        "the endogenous scenario must keep productive re-entry OFF"
    );

    // The unused re-entry thresholds cannot steer a phase that never runs.
    let mut other = endogenous();
    {
        let c = other.chain.as_mut().unwrap();
        c.reentry_hunger_in = 1;
        c.reentry_hunger_out = 0;
    }
    let mut a = Settlement::generate(0xC0FFEE, &off);
    let mut b = Settlement::generate(0xC0FFEE, &other);
    a.run(800);
    b.run(800);
    assert_eq!(
        a.canonical_bytes(),
        b.canonical_bytes(),
        "with re-entry OFF the endogenous run must be byte-identical regardless of thresholds"
    );
    assert_eq!(a.digest(), b.digest());

    // The unused override seam at the econ boundary is inert too (the S1 tripwire):
    // touching the empty bid-override hook is byte-identical to never touching it.
    let run = |touch: bool| {
        let mut society =
            Society::from_scenario(builtin_market_scenario(ScenarioName::MarketPriceDiscovery));
        for _ in 0..24 {
            if touch {
                society.clear_bid_overrides();
            }
            society.step();
        }
        (society.records, society.trades)
    };
    let (plain_records, plain_trades) = run(false);
    let (hooked_records, hooked_trades) = run(true);
    assert_eq!(plain_records, hooked_records);
    assert_eq!(plain_trades, hooked_trades);
}
