//! M19 emergence-conditions study acceptance tests.
//!
//! These pin the M19 instrument: the M18 baseline reproduces EXACTLY at an
//! explicit skew 0 (since M20 the DEFAULT skew is
//! `DEFAULT_DEMAND_BREADTH_SKEW_BPS` per the accepted study recommendation,
//! and the M20 default corpus has its own golden), the `demand_breadth_skew_bps`
//! axis consumes zero RNG draws at skew 0, the `--vary` sweep is deterministic
//! and parser-disciplined, and the two quality aggregates are well-formed
//! (empty statistics stay empty). They assert NO emergence-rate target.

use econ::agent::{Want, WantKind};
use econ::emergence::{
    parse_emergence_axis, render_emergence, render_emergence_sweep, run_emergence_corpus,
    run_emergence_corpus_tuned, run_emergence_sweep, EmergenceFormat, EmergenceKey,
    EmergenceSweepAxis, DEGENERATE_CONTROL_WORLDS,
};
use econ::good::{GoodId, Horizon, Stock, CLOTH, FOOD, GOLD, SALT, WOOD};
use econ::money::MarketMoneyConfig;
use econ::worldgen::{
    generate_world, generate_world_tuned, EmergenceTuning, GeneratedWorld, WorldEnvelope,
    WorldFamily,
};

const BASELINE_SEED: u64 = 18;
const BASELINE_WORLDS: u32 = 200;

/// Pinned FNV-1a/64 hash over the skew-0 200-world per-world CSV rows. This is
/// the RNG-stream tripwire: if it moves, the generator changed and every M18
/// baseline world changed with it. Reached via an explicit skew-0 override
/// since M20 moved the default.
const M18_BASELINE_PERWORLD_FNV1A: u64 = 0x8d55_0e81_49cc_0c2e;

/// Pinned FNV-1a/64 hash over the M20 DEFAULT 200-world per-world CSV rows
/// (skew = `DEFAULT_DEMAND_BREADTH_SKEW_BPS`). Pinned at M20 adoption.
const M20_BASELINE_PERWORLD_FNV1A: u64 = 0xf62a_0aa2_10d1_cf0e;

fn skew_zero() -> EmergenceTuning {
    EmergenceTuning {
        demand_breadth_skew_bps: Some(0),
        ..EmergenceTuning::default()
    }
}

fn envelope() -> WorldEnvelope {
    WorldEnvelope::default()
}

fn fnv1a64(bytes: &[u8]) -> u64 {
    let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
    for byte in bytes {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
    }
    hash
}

/// The per-world CSV rows (between the header and the first blank line).
fn per_world_rows(csv: &str) -> String {
    let mut rows = String::new();
    for line in csv.lines().skip(1) {
        if line.is_empty() {
            break;
        }
        rows.push_str(line);
        rows.push('\n');
    }
    rows
}

/// Per-agent endowment + want-scale projection. `Agent` is not `PartialEq`, but
/// `Stock` and `Want` are, so this captures everything worldgen randomizes.
fn endowments_and_wants(world: &GeneratedWorld) -> Vec<(Stock, Vec<Want>)> {
    world
        .scenario
        .agents
        .iter()
        .map(|agent| (agent.stock.clone(), agent.scale.clone()))
        .collect()
}

fn endowments_and_near_wants(world: &GeneratedWorld) -> Vec<(Stock, Vec<Want>)> {
    world
        .scenario
        .agents
        .iter()
        .map(|agent| {
            (
                agent.stock.clone(),
                agent
                    .scale
                    .iter()
                    .filter(|want| !matches!(want.horizon, Horizon::Later(_)))
                    .cloned()
                    .collect(),
            )
        })
        .collect()
}

fn later_want_count(world: &GeneratedWorld) -> usize {
    world
        .scenario
        .agents
        .iter()
        .flat_map(|agent| agent.scale.iter())
        .filter(|want| matches!(want.horizon, Horizon::Later(_)))
        .count()
}

/// All future (`Later`-horizon) wants in agent order. Used to assert the skew
/// axis leaves the main RNG stream untouched: future wants are drawn from the
/// main stream, so they must be byte-identical across skew levels for a seed.
fn later_wants(world: &GeneratedWorld) -> Vec<Want> {
    world
        .scenario
        .agents
        .iter()
        .flat_map(|agent| agent.scale.iter())
        .filter(|want| matches!(want.horizon, Horizon::Later(_)))
        .cloned()
        .collect()
}

fn candidates_of(world: &GeneratedWorld) -> Vec<GoodId> {
    let MarketMoneyConfig::Emergent(config) = &world.scenario.money else {
        panic!("generated world must use emergent money");
    };
    config.candidate_goods.clone()
}

// 1. The pinned golden over the default 200-world rows PLUS the exact M18
//    semantic calibration. This is the test that makes "no default changed"
//    mechanical.
#[test]
fn skew_zero_reproduces_m18_baseline() {
    let (outcomes, summary) = run_emergence_corpus_tuned(
        BASELINE_SEED,
        BASELINE_WORLDS,
        WorldFamily::Random,
        &envelope(),
        &skew_zero(),
    );

    let csv = render_emergence(&outcomes, &summary, EmergenceFormat::Csv);
    let rows = per_world_rows(&csv);
    assert_eq!(rows.lines().count(), BASELINE_WORLDS as usize);
    assert_eq!(
        fnv1a64(rows.as_bytes()),
        M18_BASELINE_PERWORLD_FNV1A,
        "the M18 baseline worlds changed: the skew-0 200-world per-world rows no \
         longer hash to the pinned golden, so an RNG draw shifted the generator"
    );

    // Semantic M18 calibration (rate, promotions, exact winner histogram).
    assert_eq!(summary.worlds, 200);
    assert_eq!(summary.in_envelope, 200);
    assert_eq!(summary.promoted_in_envelope, 21);
    assert_eq!(summary.emergence_rate_bps, 1_050);
    assert_eq!(summary.distinct_winners, 5);
    assert_eq!(
        summary.winner_histogram,
        vec![(GOLD, 9), (FOOD, 5), (WOOD, 4), (SALT, 1), (CLOTH, 2)]
    );
    assert_eq!(summary.promotion_tick_median, Some(4));
    assert_eq!(summary.promotion_tick_p90, Some(6));
    assert_eq!(summary.degenerate_promotions, 0);
    assert_eq!(summary.conservation_failures, 0);
}

// 1b (M20). The DEFAULT corpus runs at the adopted envelope
// (skew = DEFAULT_DEMAND_BREADTH_SKEW_BPS) and is pinned by its own golden
// plus the semantic calibration recorded at adoption.
#[test]
fn default_run_matches_m20_baseline() {
    let (outcomes, summary) = run_emergence_corpus(
        BASELINE_SEED,
        BASELINE_WORLDS,
        WorldFamily::Random,
        &envelope(),
    );

    let csv = render_emergence(&outcomes, &summary, EmergenceFormat::Csv);
    let rows = per_world_rows(&csv);
    assert_eq!(rows.lines().count(), BASELINE_WORLDS as usize);
    assert_eq!(
        fnv1a64(rows.as_bytes()),
        M20_BASELINE_PERWORLD_FNV1A,
        "the M20 default worlds changed: the default 200-world per-world rows no \
         longer hash to the pinned golden"
    );

    // Semantic M20 calibration (recorded at envelope adoption).
    assert_eq!(summary.worlds, 200);
    assert_eq!(summary.in_envelope, 200);
    assert_eq!(summary.promoted_in_envelope, 45);
    assert_eq!(summary.emergence_rate_bps, 2_250);
    assert_eq!(summary.distinct_winners, 5);
    assert_eq!(summary.promotion_tick_median, Some(3));
    assert_eq!(summary.promotion_tick_p90, Some(4));
    assert_eq!(summary.money_use_share_bps, Some(10_000));
    assert_eq!(summary.winner_share_median_bps, Some(5_000));
    assert_eq!(summary.degenerate_promotions, 0);
    assert_eq!(summary.conservation_failures, 0);
}

// 2. Two identical `--vary` runs are byte-identical (CSV and table).
#[test]
fn emergence_sweep_is_deterministic() {
    let axes = vec![
        EmergenceSweepAxis {
            key: EmergenceKey::DemandBreadthSkewBps,
            values: vec![0, 5000],
        },
        EmergenceSweepAxis {
            key: EmergenceKey::PromotionThresholdBps,
            values: vec![2500, 4500],
        },
    ];
    let a = run_emergence_sweep(BASELINE_SEED, 40, &envelope(), &axes).unwrap();
    let b = run_emergence_sweep(BASELINE_SEED, 40, &envelope(), &axes).unwrap();
    assert_eq!(a, b);
    assert_eq!(
        render_emergence_sweep(&axes, &a, EmergenceFormat::Csv),
        render_emergence_sweep(&axes, &b, EmergenceFormat::Csv),
    );
    assert_eq!(
        render_emergence_sweep(&axes, &a, EmergenceFormat::Table),
        render_emergence_sweep(&axes, &b, EmergenceFormat::Table),
    );
}

// 3. Unknown keys, empty value lists, and non-integer values are rejected,
//    mirroring the M4 `sweep` parser; the driver rejects empty axes + dup keys.
#[test]
fn sweep_rejects_unknown_keys() {
    assert!(EmergenceKey::parse("not-a-key").is_none());
    assert_eq!(
        parse_emergence_axis("not-a-key=1,2"),
        Err("unknown emergence sweep key 'not-a-key'".to_string())
    );

    // Empty value lists (both the trailing-`=` and the empty-element forms).
    assert_eq!(
        parse_emergence_axis("periods="),
        Err("empty emergence sweep value list".to_string())
    );
    assert_eq!(
        parse_emergence_axis("periods=4,,8"),
        Err("empty emergence sweep value list".to_string())
    );

    // Non-integer and signed values.
    assert_eq!(
        parse_emergence_axis("periods=4,x"),
        Err("invalid emergence sweep value".to_string())
    );
    assert_eq!(
        parse_emergence_axis("periods=-1"),
        Err("emergence sweep values must be unsigned integers".to_string())
    );

    // Missing `=`.
    assert_eq!(
        parse_emergence_axis("periods"),
        Err("invalid --vary".to_string())
    );

    // Driver-level: empty axis values and duplicate keys.
    assert!(run_emergence_sweep(
        BASELINE_SEED,
        4,
        &envelope(),
        &[EmergenceSweepAxis {
            key: EmergenceKey::Periods,
            values: vec![],
        }],
    )
    .is_err());
    assert_eq!(
        run_emergence_sweep(
            BASELINE_SEED,
            4,
            &envelope(),
            &[
                EmergenceSweepAxis {
                    key: EmergenceKey::Periods,
                    values: vec![40],
                },
                EmergenceSweepAxis {
                    key: EmergenceKey::Periods,
                    values: vec![80],
                },
            ],
        ),
        Err("duplicate emergence sweep key 'periods'".to_string())
    );
}

// 4. Combo rows cover the Cartesian product, in deterministic order (axis 0
//    outermost), each paired with the degenerate control.
#[test]
fn combo_rows_cover_the_cartesian_product() {
    let axes = vec![
        EmergenceSweepAxis {
            key: EmergenceKey::DemandBreadthSkewBps,
            values: vec![0, 5000, 7500],
        },
        EmergenceSweepAxis {
            key: EmergenceKey::LeadMarginBps,
            values: vec![500, 1500],
        },
    ];
    let rows = run_emergence_sweep(BASELINE_SEED, 20, &envelope(), &axes).unwrap();
    assert_eq!(rows.len(), 3 * 2);

    assert_eq!(
        rows[0].variables,
        vec![
            (EmergenceKey::DemandBreadthSkewBps, 0),
            (EmergenceKey::LeadMarginBps, 500),
        ]
    );
    assert_eq!(
        rows[1].variables,
        vec![
            (EmergenceKey::DemandBreadthSkewBps, 0),
            (EmergenceKey::LeadMarginBps, 1500),
        ]
    );
    assert_eq!(
        rows[5].variables,
        vec![
            (EmergenceKey::DemandBreadthSkewBps, 7500),
            (EmergenceKey::LeadMarginBps, 1500),
        ]
    );

    for row in &rows {
        assert_eq!(row.worlds, 20);
        // The paired degenerate control (fact 6) is always reported.
        assert_eq!(row.degenerate_promotions, 0);
    }
}

// 5. For a config-side swept key, world i is identical (population, candidates,
//    endowments, wants) across combos — only the generated config differs. For
//    an envelope-side key, the non-swept profile fields (population, candidate
//    count, candidate set) still match.
#[test]
fn paired_worlds_differ_only_by_swept_params() {
    let env = envelope();

    // Config-side: worldgen RNG is untouched.
    let lo = EmergenceTuning {
        promotion_threshold_bps: Some(2500),
        ..Default::default()
    };
    let hi = EmergenceTuning {
        promotion_threshold_bps: Some(4500),
        ..Default::default()
    };
    for index in 0..12u32 {
        let a = generate_world_tuned(BASELINE_SEED, index, WorldFamily::Random, &env, &lo);
        let b = generate_world_tuned(BASELINE_SEED, index, WorldFamily::Random, &env, &hi);
        assert_eq!(a.profile, b.profile);
        assert_eq!(endowments_and_wants(&a), endowments_and_wants(&b));

        let MarketMoneyConfig::Emergent(ca) = &a.scenario.money else {
            panic!("emergent money expected");
        };
        let MarketMoneyConfig::Emergent(cb) = &b.scenario.money else {
            panic!("emergent money expected");
        };
        assert_eq!(ca.candidate_goods, cb.candidate_goods);
        assert_eq!(ca.promotion_threshold_bps, 2500);
        assert_eq!(cb.promotion_threshold_bps, 4500);
    }

    // Envelope-side (skew): the skew target and redirect rolls are drawn from an
    // ISOLATED stream, so the entire MAIN generation stream is untouched. Every
    // paired world therefore shares population, candidates, endowments, want
    // counts, and future (Later) wants across skew levels — only the *value* of a
    // redirected near-want differs. This is the paired-comparison guarantee the
    // H3 dose-response relies on.
    let s0 = EmergenceTuning {
        demand_breadth_skew_bps: Some(0),
        ..Default::default()
    };
    let s5 = EmergenceTuning {
        demand_breadth_skew_bps: Some(5000),
        ..Default::default()
    };
    for index in 0..12u32 {
        let a = generate_world_tuned(BASELINE_SEED, index, WorldFamily::Random, &env, &s0);
        let b = generate_world_tuned(BASELINE_SEED, index, WorldFamily::Random, &env, &s5);
        assert_eq!(a.profile.population, b.profile.population);
        assert_eq!(a.profile.candidate_goods, b.profile.candidate_goods);
        assert_eq!(candidates_of(&a), candidates_of(&b));
        assert_eq!(
            a.scenario.agents.len(),
            b.scenario.agents.len(),
            "population diverged at world {index}"
        );
        for (ag_a, ag_b) in a.scenario.agents.iter().zip(&b.scenario.agents) {
            assert_eq!(
                ag_a.stock, ag_b.stock,
                "endowment diverged at world {index}"
            );
            assert_eq!(
                ag_a.scale.len(),
                ag_b.scale.len(),
                "want count diverged at world {index}"
            );
        }
        assert_eq!(
            later_wants(&a),
            later_wants(&b),
            "future wants diverged at world {index}: skew touched the main stream"
        );
    }

    // Envelope-side (future wants): explicit sweeps are paired against the
    // pinned envelope stream, so endowments and near wants stay identical while
    // only Later wants are removed/added. The explicit default value still
    // matches the no-override M18 path exactly.
    let f0 = EmergenceTuning {
        future_want_share_bps: Some(0),
        ..Default::default()
    };
    let f3 = EmergenceTuning {
        future_want_share_bps: Some(3000),
        ..Default::default()
    };
    let f9 = EmergenceTuning {
        future_want_share_bps: Some(9000),
        ..Default::default()
    };
    for index in 0..12u32 {
        let default = generate_world(BASELINE_SEED, index, WorldFamily::Random, &env);
        let low = generate_world_tuned(BASELINE_SEED, index, WorldFamily::Random, &env, &f0);
        let pinned = generate_world_tuned(BASELINE_SEED, index, WorldFamily::Random, &env, &f3);
        let high = generate_world_tuned(BASELINE_SEED, index, WorldFamily::Random, &env, &f9);

        assert_eq!(
            endowments_and_wants(&default),
            endowments_and_wants(&pinned)
        );
        assert_eq!(
            endowments_and_near_wants(&low),
            endowments_and_near_wants(&pinned)
        );
        assert_eq!(
            endowments_and_near_wants(&pinned),
            endowments_and_near_wants(&high)
        );
        assert!(later_want_count(&low) <= later_want_count(&pinned));
        assert!(later_want_count(&pinned) <= later_want_count(&high));
    }
}

// 6. The skew axis at an explicit 0 consumes ZERO extra draws (test 1's M18
//    golden is the corpus-level proof), and the forked-stream design keeps
//    skew levels PAIRED: the M20 default (skewed) world and the skew-0 world
//    share the same seed, population, candidate subset, and endowments —
//    only the want boost and the recorded target differ.
#[test]
fn skew_zero_draws_nothing() {
    let env = envelope();
    let skew_zero = EmergenceTuning {
        demand_breadth_skew_bps: Some(0),
        ..Default::default()
    };
    for &index in &[0u32, 1, 5, 13, 29, 50, 99] {
        let default_world = generate_world(BASELINE_SEED, index, WorldFamily::Random, &env);
        let tuned =
            generate_world_tuned(BASELINE_SEED, index, WorldFamily::Random, &env, &skew_zero);
        assert_eq!(default_world.world_seed, tuned.world_seed);
        assert_eq!(
            default_world.profile.population, tuned.profile.population,
            "main-stream pairing broke: population differs across skew levels"
        );
        assert_eq!(
            default_world.profile.candidate_goods,
            tuned.profile.candidate_goods
        );
        let default_endowments: Vec<Stock> = endowments_and_wants(&default_world)
            .into_iter()
            .map(|(stock, _)| stock)
            .collect();
        let tuned_endowments: Vec<Stock> = endowments_and_wants(&tuned)
            .into_iter()
            .map(|(stock, _)| stock)
            .collect();
        assert_eq!(
            default_endowments, tuned_endowments,
            "main-stream pairing broke: endowments differ across skew levels"
        );
        assert_eq!(tuned.profile.skew_target, None);
        assert!(
            default_world.profile.skew_target.is_some(),
            "the M20 default must skew (DEFAULT_DEMAND_BREADTH_SKEW_BPS > 0)"
        );
    }
}

// 7. The skew target is per-world random, drawn from each world's own candidate
//    subset; across a corpus at least 3 distinct targets appear (no global
//    designation).
#[test]
fn skew_target_is_random_not_fixed() {
    let env = envelope();
    let tuning = EmergenceTuning {
        demand_breadth_skew_bps: Some(5000),
        ..Default::default()
    };
    let mut targets: Vec<GoodId> = Vec::new();
    for index in 0..40u32 {
        let world = generate_world_tuned(BASELINE_SEED, index, WorldFamily::Random, &env, &tuning);
        let target = world.profile.skew_target.expect("skew > 0 draws a target");
        assert!(
            candidates_of(&world).contains(&target),
            "skew target {target:?} not drawn from this world's candidate subset"
        );
        if !targets.contains(&target) {
            targets.push(target);
        }
    }
    assert!(
        targets.len() >= 3,
        "expected >= 3 distinct skew targets across the corpus, got {}",
        targets.len()
    );
}

// 8. Generator-level monotonicity: with the same seed and increasing positive
//    skew, the count of wants on the skew target is non-decreasing (more bps ->
//    a superset of redirected wants). The skew target and redirect rolls come
//    from an isolated stream seeded from the world+agent (not the bps), so across
//    skew levels the target is the same and only the redirect threshold rises.
#[test]
fn skew_raises_demand_breadth_monotonically() {
    let env = envelope();
    for index in [3u32, 8, 21, 37] {
        let mut last_count = 0usize;
        let mut last_target: Option<GoodId> = None;
        for bps in [1000u16, 4000, 7000, 10000] {
            let tuning = EmergenceTuning {
                demand_breadth_skew_bps: Some(bps),
                ..Default::default()
            };
            let world =
                generate_world_tuned(BASELINE_SEED, index, WorldFamily::Random, &env, &tuning);
            let target = world.profile.skew_target.expect("skew > 0 draws a target");
            if let Some(prev) = last_target {
                assert_eq!(
                    prev, target,
                    "skew target moved across skew levels for the same seed (index {index})"
                );
            }
            last_target = Some(target);

            let count = world
                .scenario
                .agents
                .iter()
                .flat_map(|agent| agent.scale.iter())
                .filter(|want| want.kind == WantKind::Good(target))
                .count();
            assert!(
                count >= last_count,
                "demand breadth for target fell at bps={bps}: {count} < {last_count} (index {index})"
            );
            last_count = count;
        }
        assert!(
            last_count > 0,
            "expected some wants on the target at maximum skew (index {index})"
        );
    }
}

// 9. The quality aggregates are computed only over promoted worlds, and empty-
//    promotion combos emit EMPTY fields — never zeros pretending to be data.
#[test]
fn quality_aggregates_are_well_formed() {
    let env = envelope();

    // Promoted worlds present -> aggregates are Some and consistent with the rows.
    let (outcomes, summary) =
        run_emergence_corpus(BASELINE_SEED, BASELINE_WORLDS, WorldFamily::Random, &env);
    let promoted: Vec<_> = outcomes
        .iter()
        .filter(|o| o.promoted && o.class == econ::worldgen::WorldClass::InEnvelope)
        .collect();
    assert!(!promoted.is_empty());

    let with_use = promoted
        .iter()
        .filter(|o| o.post_promotion_spot_trades > 0)
        .count();
    let expected_use = (with_use as u64) * 10_000 / (promoted.len() as u64);
    assert_eq!(summary.money_use_share_bps, Some(expected_use as u32));
    assert!(summary.money_use_share_bps.unwrap() <= 10_000);
    assert!(summary.winner_share_median_bps.is_some());

    // No promotions (degenerate corpus) -> empty, NOT zero.
    let (_deg, deg_summary) = run_emergence_corpus(
        BASELINE_SEED,
        DEGENERATE_CONTROL_WORLDS,
        WorldFamily::Degenerate,
        &env,
    );
    assert_eq!(deg_summary.promoted_in_envelope, 0);
    assert_eq!(deg_summary.money_use_share_bps, None);
    assert_eq!(deg_summary.winner_share_median_bps, None);

    // A random combo with an unsatisfiable promotion threshold (100% share is
    // impossible when both sides of every trade are counted) -> 0 promotions ->
    // the combo row renders EMPTY quality fields, not "0".
    let axes = vec![EmergenceSweepAxis {
        key: EmergenceKey::PromotionThresholdBps,
        values: vec![10_000],
    }];
    let rows = run_emergence_sweep(BASELINE_SEED, 40, &env, &axes).unwrap();
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].promoted, 0);
    assert_eq!(rows[0].money_use_share_bps, None);
    assert_eq!(rows[0].winner_share_median_bps, None);

    let csv = render_emergence_sweep(&axes, &rows, EmergenceFormat::Csv);
    let data_row = csv.lines().nth(1).expect("one combo row");
    let fields: Vec<&str> = data_row.split(',').collect();
    assert_eq!(fields.len(), 12);
    assert_eq!(fields[3], "0", "promoted should be 0");
    assert_eq!(fields[8], "", "money_use_share_bps must be empty, not 0");
    assert_eq!(
        fields[9], "",
        "winner_share_median_bps must be empty, not 0"
    );
}

// 10. Existing suites untouched: the tuned code path with default tuning is
//     byte-identical to the untuned path (no instrument drift), and the
//     degenerate control still never promotes. (`cargo test`, the M3 golden,
//     and the M18 suite are the process-level guards.)
#[test]
fn default_tuning_matches_untuned_path() {
    let env = envelope();
    let (o1, s1) = run_emergence_corpus(BASELINE_SEED, 64, WorldFamily::Random, &env);
    let (o2, s2) = run_emergence_corpus_tuned(
        BASELINE_SEED,
        64,
        WorldFamily::Random,
        &env,
        &EmergenceTuning::default(),
    );
    assert_eq!(o1, o2);
    assert_eq!(s1, s2);
    assert_eq!(
        render_emergence(&o1, &s1, EmergenceFormat::Csv),
        render_emergence(&o2, &s2, EmergenceFormat::Csv),
    );

    let (_deg, deg_summary) = run_emergence_corpus_tuned(
        BASELINE_SEED,
        DEGENERATE_CONTROL_WORLDS,
        WorldFamily::Degenerate,
        &env,
        &EmergenceTuning::default(),
    );
    assert_eq!(deg_summary.degenerate_promotions, 0);
    assert_eq!(deg_summary.promoted_in_envelope, 0);
}
