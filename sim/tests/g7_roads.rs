//! G7 acceptance suite — roads: infrastructure cuts trip cost (the [`Region`]).
//!
//! G2c proved a caravan converges two settlements' prices; G7 adds a **road** — a
//! public-works project the community builds from real labor that, once complete,
//! **cuts the route's transit cost**, so caravans cycle faster and the price gap
//! converges faster. These pin the milestone's DoDs and the constraints that
//! protect them:
//!
//! - **The road is community labor, conserved** (tests 2 + 5): colonists contribute
//!   labor to the road project (the reused G3 project-labor path), the build
//!   consumes its declared conserved materials (accounted) and creates **no** good,
//!   and whole-system conservation holds across the build.
//! - **The road accelerates convergence, the control proves it** (tests 3 + 4, the
//!   falsification twin): with the road the FOOD-price gap is tighter at a fixed
//!   horizon than the no-road control, which keeps a wider gap. Sign only — no
//!   magnitude is pinned.
//! - **The effect is a one-way route transit cut** (test 6): once built the reduced
//!   transit stays; it never flaps.
//!
//! plus determinism (test 1, the tripwire — including the road completion tick) and
//! the unchanged econ behavior (test 7: the road is region-level and game-only, so a
//! settlement's economy is byte-identical until the route transit changes; the six
//! econ goldens are the workspace suite's job). They assert shape, exactness, and
//! sign — never a pinned price magnitude (the lab discipline).

use econ::good::{FOOD, GOLD, WOOD};
use sim::{Region, RegionConfig, RoadPlan, Route};

/// The convergence experiment's run length. The road completes early (tick 9), so
/// most of the horizon runs on the cut transit; the comparison window is the final
/// band, by which the road has converged and the slow no-road control has not.
const CONVERGENCE_TICKS: u64 = 70;
/// The early band the start gap is measured over (after both settlements clear).
const START_WINDOW: (usize, usize) = (6, 18);
/// The late band the convergence gap is compared over.
const END_WINDOW: (usize, usize) = (50, 70);
/// The convergence seed (G2c's, so the two suites are directly comparable).
const SEED: u64 = 20_260_614;

/// The realized FOOD-price gap `|price_A − price_B|` per econ tick over a run, `None`
/// until both settlements have cleared a FOOD trade — the convergence observable.
fn gap_series(config: &RegionConfig, seed: u64, ticks: u64) -> Vec<Option<u64>> {
    let mut region = Region::generate(seed, config);
    let mut gaps = Vec::with_capacity(ticks as usize);
    for _ in 0..ticks {
        region.econ_tick();
        gaps.push(region.price_gap(FOOD));
    }
    gaps
}

/// The mean of the cleared gaps in `series[lo..hi]` (skipping ticks with no trade) —
/// a windowed average robust to the last-trade oscillation a single sample carries.
fn window_mean_gap(series: &[Option<u64>], (lo, hi): (usize, usize)) -> f64 {
    let hi = hi.min(series.len());
    let cleared: Vec<u64> = series[lo..hi].iter().filter_map(|g| *g).collect();
    assert!(!cleared.is_empty(), "the window cleared no FOOD trade");
    cleared.iter().sum::<u64>() as f64 / cleared.len() as f64
}

fn legacy_no_road_canonical_len(region: &Region) -> usize {
    let settlements_len: usize = region
        .settlements()
        .iter()
        .map(|settlement| 4 + settlement.canonical_bytes().len())
        .sum();
    // econ_tick + settlement count + settlement blobs + config knobs +
    // caravan phase/counter/escrow. No G7 road tag is present for `road: None`.
    8 + 4 + settlements_len + 2 + 4 + 4 + 4 + 1 + 1 + 4 + 4 + 8
}

// ---- 1. determinism (the tripwire) ---------------------------------------

/// 1. Same `(seed, RegionConfig)` → byte-identical run, including the road
///    completion tick and the transit change; nothing is drawn in the region loop,
///    the caravan step, or the road step. A different seed diverges; the no-road
///    control is its own deterministic run, distinct from the road's.
#[test]
fn roads_run_is_deterministic() {
    let config = RegionConfig::roads();

    let mut a = Region::generate(0xC0FFEE, &config);
    let mut b = Region::generate(0xC0FFEE, &config);
    a.run(CONVERGENCE_TICKS);
    b.run(CONVERGENCE_TICKS);
    assert_eq!(
        a.canonical_bytes(),
        b.canonical_bytes(),
        "same seed diverged"
    );
    assert_eq!(a.digest(), b.digest());

    // The road completion tick and the cut transit are part of the deterministic
    // state — identical across the two runs, and the road really did complete.
    assert_eq!(a.road_completed_at(), b.road_completed_at());
    assert!(
        a.road_completed_at().is_some(),
        "the road never completed over the run"
    );
    assert_eq!(a.route_transit_ticks(), b.route_transit_ticks());
    assert!(a.road_complete() && a.route_transit_ticks() == 8);

    // A different seed yields a different run (generation actually uses the Rng).
    let mut c = Region::generate(0xBADF00D, &config);
    c.run(CONVERGENCE_TICKS);
    assert_ne!(a.digest(), c.digest(), "the seed did not affect the run");

    // The no-road control is its own deterministic run, distinct from the road's.
    let control = RegionConfig::roads_control();
    let mut d = Region::generate(0xC0FFEE, &control);
    let mut e = Region::generate(0xC0FFEE, &control);
    d.run(CONVERGENCE_TICKS);
    e.run(CONVERGENCE_TICKS);
    assert_eq!(d.digest(), e.digest(), "the control is not deterministic");
    assert_ne!(
        a.digest(),
        d.digest(),
        "the road and the no-road control must run differently"
    );
}

#[test]
fn no_road_canonical_bytes_omit_g7_state() {
    let mut region = Region::generate(SEED, &RegionConfig::two_settlements());
    assert!(!region.has_road());
    assert_eq!(
        region.canonical_bytes().len(),
        legacy_no_road_canonical_len(&region),
        "a G2c no-road region must not emit a G7 road-state sentinel"
    );

    region.run(12);
    assert_eq!(
        region.canonical_bytes().len(),
        legacy_no_road_canonical_len(&region),
        "a running G2c no-road region must keep its pre-G7 canonical shape"
    );

    let control = Region::generate(SEED, &RegionConfig::roads_control());
    assert!(!control.has_road());
    assert_eq!(
        control.canonical_bytes().len(),
        legacy_no_road_canonical_len(&control),
        "the G7 no-road control must not emit a road-state sentinel"
    );
}

// ---- 2. the road is built from labor -------------------------------------

/// 2. The road completes only after enough labor is contributed; the contributed
///    labor is a conserved expenditure (accounted on its own line, and its conserved
///    materials accounted as inputs); and the route's transit drops exactly at
///    completion.
#[test]
fn road_is_built_from_labor() {
    let config = RegionConfig::roads();
    let labor_cost = config.road.expect("roads has a road").labor_cost;
    let mut region = Region::generate(SEED, &config);

    let unbuilt_transit = region.route_transit_ticks();
    let mut labor_reported = 0u64;
    let mut materials_reported = 0u64;
    let mut completion_tick = None;

    for _ in 0..CONVERGENCE_TICKS {
        let was_complete = region.road_complete();
        let report = region.econ_tick();
        labor_reported += report.road_labor;
        materials_reported += report.consumed_as_input_of(WOOD);

        if !was_complete {
            // While building, the route transit is unchanged — the cut has not fired.
            // (Checked on the pre-completion ticks; the completion tick itself applies
            // the cut, asserted below.)
            if !region.road_complete() {
                assert_eq!(
                    region.route_transit_ticks(),
                    unbuilt_transit,
                    "the transit dropped before the road completed at tick {}",
                    report.econ_tick
                );
                assert!(
                    region.road_labor_advanced().unwrap() < labor_cost,
                    "the road's labor reached the cost without completing"
                );
            } else {
                // This is the completion tick: the contributed labor met the cost and
                // the transit cut fired this very tick.
                completion_tick = Some(report.econ_tick);
                assert!(
                    region.road_labor_advanced().unwrap() >= labor_cost,
                    "the road completed before its labor cost was met"
                );
                assert!(
                    region.route_transit_ticks() < unbuilt_transit,
                    "the transit did not drop at completion"
                );
            }
        }
    }

    let completion_tick = completion_tick.expect("the road completed within the horizon");
    assert_eq!(region.road_completed_at(), Some(completion_tick));
    // The labor is accounted: the per-tick `road_labor` line sums to the labor the
    // project advanced (clamped to the cost) — a conserved expenditure, not a good.
    assert_eq!(
        labor_reported,
        u64::from(region.road_labor_advanced().unwrap())
    );
    assert_eq!(labor_reported, u64::from(labor_cost));
    // Its conserved materials are accounted as inputs (material_per_labor == 1 here,
    // so exactly one WOOD per labor unit), and the build consumed them all.
    assert_eq!(materials_reported, u64::from(labor_cost));
}

// ---- 3. the road accelerates convergence (sign only) ---------------------

/// 3. With the road, the realized FOOD-price gap at the late window is smaller than
///    at the start (it narrows) AND smaller than the no-road control's late gap (it
///    converges faster). Sign only — no magnitude is pinned.
#[test]
fn road_accelerates_convergence() {
    let road_gaps = gap_series(&RegionConfig::roads(), SEED, CONVERGENCE_TICKS);
    let control_gaps = gap_series(&RegionConfig::roads_control(), SEED, CONVERGENCE_TICKS);

    let road_start = window_mean_gap(&road_gaps, START_WINDOW);
    let road_end = window_mean_gap(&road_gaps, END_WINDOW);
    let control_end = window_mean_gap(&control_gaps, END_WINDOW);

    // The gap narrows over time with the road...
    assert!(
        road_end < road_start,
        "the road did not narrow the gap: start={road_start:.2} end={road_end:.2}"
    );
    // ...and ends below where the no-road control leaves it (the road — not the
    // caravan, which the control also has — is what converges it faster).
    assert!(
        road_end < control_end,
        "the road end gap ({road_end:.2}) is not below the no-road control end gap ({control_end:.2})"
    );
}

// ---- 4. the no-road control converges slower (the twin) ------------------

/// 4. The falsification twin: the same region WITHOUT the road converges slower /
///    to a wider gap. Paired with test 3, it shows the road — not time or the
///    caravan alone — is the cause of the faster convergence. If both converged
///    identically the road would not be cutting transit.
#[test]
fn no_road_control_converges_slower() {
    let control_gaps = gap_series(&RegionConfig::roads_control(), SEED, CONVERGENCE_TICKS);
    let road_gaps = gap_series(&RegionConfig::roads(), SEED, CONVERGENCE_TICKS);

    let control_start = window_mean_gap(&control_gaps, START_WINDOW);
    let control_end = window_mean_gap(&control_gaps, END_WINDOW);
    let road_end = window_mean_gap(&road_gaps, END_WINDOW);

    // The control keeps a clearly-positive gap to the late window — it does not
    // converge the way the road does (its slow caravan barely closes it).
    assert!(
        control_end > 1.0,
        "the control gap unexpectedly converged: start={control_start:.2} end={control_end:.2}"
    );
    // The control's late gap is clearly wider than the road's — the road, not the
    // shared caravan, is what closes it.
    assert!(
        control_end > road_end + 1.0,
        "the no-road control did not keep the gap relative to the road: control_end={control_end:.2} road_end={road_end:.2}"
    );
}

// ---- 5. building the road conserves --------------------------------------

/// 5. Building the road creates no good and destroys none beyond the labor/inputs
///    spent: whole-system (region-wide) conservation holds every tick across the
///    build, the road produces no good, and the conserved materials it consumes are
///    exactly accounted and drawn from the road fund (which empties to zero).
#[test]
fn road_build_conserves() {
    let config = RegionConfig::roads();
    let plan = config.road.expect("roads has a road");
    let mut region = Region::generate(SEED, &config);

    // Gold is a closed regional balance; pin it across the whole run.
    let mut first_gold = None;
    let mut total_materials = 0u64;
    for _ in 0..CONVERGENCE_TICKS {
        let report = region.econ_tick();
        // The whole-system ledger balances for every good, and gold is unchanged.
        assert!(
            report.conserves(),
            "region conservation broke across the build at tick {}",
            report.econ_tick
        );
        assert_eq!(
            report.gold_after, report.gold_before,
            "region gold changed within a tick at tick {}",
            report.econ_tick
        );
        let gold = *first_gold.get_or_insert(report.gold_after);
        assert_eq!(report.gold_after, gold, "region gold drifted across ticks");

        // The road creates NO good — the region's `produced` line is empty every
        // tick (a plain settlement produces nothing, and a road produces nothing).
        assert_eq!(
            report.produced.values().copied().sum::<u64>(),
            0,
            "the road build created a good at tick {}",
            report.econ_tick
        );
        total_materials += report.consumed_as_input_of(WOOD);
    }

    // The road consumed exactly its declared materials — no more, no fewer — and the
    // road fund emptied to zero (the materials were spent into the build, not lost or
    // created). material_per_labor == 1, so the total equals the labor cost.
    let expected = u64::from(plan.labor_cost) * u64::from(plan.material_per_labor);
    assert_eq!(
        total_materials, expected,
        "the road did not consume exactly its declared materials"
    );
    assert_eq!(
        region.road_fund_of(WOOD),
        0,
        "the road fund was not drawn down to exactly the materials spent"
    );
}

// ---- 6. the road is one-way ----------------------------------------------

/// 6. Once built the transit reduction stays — no flapping: the route transit, the
///    completion tick, and the accumulated labor are all frozen after completion,
///    and no further labor is contributed (the project is one-way).
#[test]
fn road_is_one_way() {
    let config = RegionConfig::roads();
    let mut region = Region::generate(SEED, &config);

    // Run until the road completes.
    let mut ticks = 0;
    while !region.road_complete() && ticks < CONVERGENCE_TICKS {
        region.econ_tick();
        ticks += 1;
    }
    assert!(region.road_complete(), "the road never completed");

    let built_transit = region.route_transit_ticks();
    let built_at = region.road_completed_at();
    let built_labor = region.road_labor_advanced();
    assert!(built_transit < 20, "the transit was not cut on completion");

    // Run far past completion: the cut transit, the completion stamp, and the labor
    // never change again, and no further labor is ever contributed.
    for _ in 0..CONVERGENCE_TICKS {
        let report = region.econ_tick();
        assert_eq!(
            region.route_transit_ticks(),
            built_transit,
            "the built-road transit flapped at tick {}",
            report.econ_tick
        );
        assert_eq!(
            region.road_completed_at(),
            built_at,
            "the completion tick changed"
        );
        assert_eq!(
            region.road_labor_advanced(),
            built_labor,
            "labor was contributed to an already-built road"
        );
        assert_eq!(
            report.road_labor, 0,
            "an already-built road still drew labor at tick {}",
            report.econ_tick
        );
    }
}

// ---- 7. econ behavior is unchanged ---------------------------------------

/// 7. The road is region-level and game-only: it never touches a settlement's
///    economy. Until the route transit changes (after completion), a road region's
///    settlements are byte-identical to the no-road control's — the community-labor
///    public works perturbs no settlement econ. (The six econ goldens being
///    byte-identical and the whole prior suite green is the workspace `cargo test`'s
///    job; this is the in-suite proxy that the road is purely additive.)
#[test]
fn econ_unchanged() {
    let mut road = Region::generate(SEED, &RegionConfig::roads());
    let mut control = Region::generate(SEED, &RegionConfig::roads_control());

    // The road and the no-road control share the SAME settlements, caravan, route,
    // and seed — only the road differs. Before the road completes, the route transit
    // is identical in both, so the caravan does the same thing and the settlement
    // economies are byte-identical. (The road build itself only touches region-level
    // road state, never a settlement.)
    let completion = {
        let mut probe = Region::generate(SEED, &RegionConfig::roads());
        while !probe.road_complete() {
            probe.econ_tick();
        }
        probe.road_completed_at().expect("the road completes")
    };
    assert!(completion > 0, "the road completed before any tick ran");

    for tick in 0..completion {
        road.econ_tick();
        control.econ_tick();
        for i in 0..road.settlement_count() {
            assert_eq!(
                road.settlement(i).unwrap().canonical_bytes(),
                control.settlement(i).unwrap().canonical_bytes(),
                "the road build perturbed settlement {i}'s economy at tick {tick}"
            );
        }
    }

    // The no-road control never has road state: no transit cut, no road labor, no
    // material consumption — the road is genuinely opt-in.
    assert!(!control.has_road());
    assert_eq!(control.route_transit_ticks(), 20);
    assert_eq!(control.road_completed_at(), None);
}

// ---- unit tests ----------------------------------------------------------

/// A road must REDUCE the route transit — a `transit_after` not below the unbuilt
/// route is a misconfiguration and is rejected loudly at generation.
#[test]
#[should_panic(expected = "a road must REDUCE the route transit")]
fn road_that_does_not_reduce_transit_is_rejected() {
    let config = RegionConfig {
        route: Route { transit_ticks: 8 },
        road: Some(RoadPlan {
            labor_cost: 10,
            labor_per_colonist: 1,
            transit_after: 8, // not below the route transit → rejected
            material: WOOD,
            material_per_labor: 1,
        }),
        ..RegionConfig::roads()
    };
    let _ = Region::generate(SEED, &config);
}

/// Road materials are conserved physical inputs, never the closed regional GOLD
/// money ledger. Rejecting this at generation keeps road consumption out of the
/// money invariant's blind spot.
#[test]
#[should_panic(expected = "road material cannot be GOLD")]
fn road_material_cannot_be_gold() {
    let config = RegionConfig {
        road: Some(RoadPlan {
            material: GOLD,
            ..RegionConfig::roads().road.unwrap()
        }),
        ..RegionConfig::roads()
    };
    let _ = Region::generate(SEED, &config);
}

/// The road fund is `Stock`-backed (`u32` quantities), so configs whose declared
/// material total cannot be represented are rejected loudly instead of saturating
/// and leaving a project that can never reach its labor cost.
#[test]
#[should_panic(expected = "road material total must fit in u32")]
fn oversized_road_material_total_is_rejected() {
    let config = RegionConfig {
        road: Some(RoadPlan {
            labor_cost: u32::MAX,
            material_per_labor: 2,
            ..RegionConfig::roads().road.unwrap()
        }),
        ..RegionConfig::roads()
    };
    let _ = Region::generate(SEED, &config);
}

/// The road material joins the region-wide conservation ledger (so its draw-down is
/// snapshotted and accounted), and the fund starts stocked with exactly the build's
/// total materials.
#[test]
fn road_material_is_tracked_and_funded() {
    let config = RegionConfig::roads();
    let plan = config.road.expect("roads has a road");
    let region = Region::generate(SEED, &config);

    assert!(
        region.tracked_goods().contains(&plan.material),
        "the road material is not tracked for conservation"
    );
    assert_eq!(
        region.road_fund_of(plan.material),
        u64::from(plan.labor_cost) * u64::from(plan.material_per_labor),
        "the road fund is not stocked with exactly the build's materials"
    );
    assert_eq!(region.road_labor_cost(), Some(plan.labor_cost));
    assert_eq!(region.road_labor_advanced(), Some(0));
    assert!(!region.road_complete());
}

/// A labor-only road (no material cost) still builds and cuts transit, and consumes
/// no goods at all — the optional-materials path the spec allows.
#[test]
fn labor_only_road_consumes_no_goods() {
    let config = RegionConfig {
        road: Some(RoadPlan {
            material_per_labor: 0,
            ..RegionConfig::roads().road.unwrap()
        }),
        ..RegionConfig::roads()
    };
    let mut region = Region::generate(SEED, &config);
    // The fund holds nothing for a labor-only road.
    assert_eq!(region.road_fund_of(WOOD), 0);

    let mut total_inputs = 0u64;
    for _ in 0..CONVERGENCE_TICKS {
        let report = region.econ_tick();
        assert!(report.conserves(), "a labor-only road broke conservation");
        total_inputs += report.consumed_as_input_of(WOOD);
    }
    assert!(region.road_complete(), "the labor-only road never built");
    assert_eq!(region.route_transit_ticks(), 8, "the transit was not cut");
    assert_eq!(total_inputs, 0, "a labor-only road consumed a good");
}

// ---- 3+4 across many seeds (the convergence sign is not a seed fluke) -----

/// The road-vs-control convergence sign is **robust across 40 seeds**, not a fluke
/// of the single convergence seed — for every one of 40 independent generations the
/// road's late gap narrows below its own start AND stays below the no-road control's
/// late gap (and the control keeps a clearly-positive gap). This is what backs the
/// `engine-divergence.md` "robust across 40 seeds" claim; it is the multi-seed
/// generalization of tests 3 + 4. Sign only — no magnitude is pinned.
#[test]
fn road_accelerates_convergence_across_seeds() {
    const SEED_COUNT: u64 = 40;
    for s in 0..SEED_COUNT {
        // A well-mixed sub-seed per iteration (the same golden-ratio stride the
        // region uses to fork its two settlements), so the 40 runs are independent.
        let seed = SEED.wrapping_add(s.wrapping_mul(0x9E37_79B9_7F4A_7C15));
        let road_gaps = gap_series(&RegionConfig::roads(), seed, CONVERGENCE_TICKS);
        let control_gaps = gap_series(&RegionConfig::roads_control(), seed, CONVERGENCE_TICKS);

        let road_start = window_mean_gap(&road_gaps, START_WINDOW);
        let road_end = window_mean_gap(&road_gaps, END_WINDOW);
        let control_end = window_mean_gap(&control_gaps, END_WINDOW);

        assert!(
            road_end < road_start,
            "seed #{s}: the road did not narrow the gap: start={road_start:.2} end={road_end:.2}"
        );
        assert!(
            road_end < control_end,
            "seed #{s}: the road end gap ({road_end:.2}) is not below the no-road control's ({control_end:.2})"
        );
        assert!(
            control_end > 1.0,
            "seed #{s}: the no-road control unexpectedly converged (end={control_end:.2})"
        );
    }
}
