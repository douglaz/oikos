//! M18 emergence-robustness harness acceptance tests.
//!
//! These pin the determinism, seed-purity, true-negative, conservation,
//! winner-diversity, anchor-reproduction, classification, and baseline-
//! calibration properties of the `worldgen` + `emergence` instrument. They
//! assert NOTHING about whether the emergence rate is high — the rate is a
//! measurement, recorded as the baseline.

use econ::agent::WantKind;
use econ::emergence::{
    promotion_facts, render_emergence, run_emergence_corpus, run_emergence_corpus_tuned,
    run_generated_world, EmergenceFormat,
};
use econ::good::{GoodId, GOLD, SALT};
use econ::scenario::{builtin_market_scenario, ScenarioName};
use econ::society::Society;
use econ::worldgen::{
    generate_world, EmergenceTuning, WorldClass, WorldEnvelope, WorldFamily, GOOD_POOL,
};

const BASELINE_SEED: u64 = 18;
const BASELINE_WORLDS: u32 = 200;

fn envelope() -> WorldEnvelope {
    WorldEnvelope::default()
}

// 1. Two runs of (worlds=50, seed=18, random) produce byte-identical CSV.
#[test]
fn emergence_corpus_is_deterministic() {
    let (first, summary_a) = run_emergence_corpus(18, 50, WorldFamily::Random, &envelope());
    let (second, summary_b) = run_emergence_corpus(18, 50, WorldFamily::Random, &envelope());

    let csv_a = render_emergence(&first, &summary_a, EmergenceFormat::Csv);
    let csv_b = render_emergence(&second, &summary_b, EmergenceFormat::Csv);

    assert_eq!(csv_a, csv_b);
    assert_eq!(first, second);
    assert_eq!(summary_a, summary_b);
}

// 2. Regenerating world i alone equals world i extracted from the batch.
#[test]
fn worldgen_is_seed_pure() {
    let (outcomes, _summary) =
        run_emergence_corpus(BASELINE_SEED, 40, WorldFamily::Random, &envelope());

    for &index in &[0u32, 1, 7, 13, 29, 39] {
        let batch = outcomes[index as usize];

        // World i, generated and run entirely in isolation.
        let (world, society) =
            run_generated_world(BASELINE_SEED, index, WorldFamily::Random, &envelope());
        let facts = promotion_facts(&society);

        assert_eq!(world.world_seed, batch.world_seed);
        assert_eq!(world.profile.population, batch.population);
        assert_eq!(world.profile.candidate_goods, batch.candidate_goods);
        assert_eq!(world.profile.surplus_goods, batch.surplus_goods);
        assert_eq!(facts.promoted, batch.promoted);
        assert_eq!(facts.winner, batch.winner);
        assert_eq!(facts.promotion_tick, batch.promotion_tick);
        assert_eq!(facts.money_units, batch.money_units);
        assert_eq!(facts.barter_trades, batch.barter_trades);
    }
}

// 3. The generator privileges no good: every pool good appears as an endowment
//    and as a want with nonzero, non-dominant frequency. Bounds pinned for the
//    seed-18 / 200-world corpus (uniform expectation over six goods ~1667 bps).
#[test]
fn generator_designates_no_winner() {
    let env = envelope();
    let mut endow_slots = [0u64; 7];
    let mut want_slots = [0u64; 7];

    for index in 0..BASELINE_WORLDS {
        let world = generate_world(BASELINE_SEED, index, WorldFamily::Random, &env);
        for agent in &world.scenario.agents {
            for good in agent.stock.positive_goods() {
                endow_slots[usize::from(good.0)] += 1;
            }
            for want in &agent.scale {
                if let WantKind::Good(good) = want.kind {
                    want_slots[usize::from(good.0)] += 1;
                }
            }
        }
    }

    let endow_total: u64 = GOOD_POOL
        .iter()
        .map(|g| endow_slots[usize::from(g.0)])
        .sum();
    let want_total: u64 = GOOD_POOL.iter().map(|g| want_slots[usize::from(g.0)]).sum();
    assert!(endow_total > 0 && want_total > 0);

    for good in &GOOD_POOL {
        let i = usize::from(good.0);
        assert!(endow_slots[i] > 0, "good {good:?} never endowed");
        assert!(want_slots[i] > 0, "good {good:?} never wanted");

        let endow_share = endow_slots[i] * 10_000 / endow_total;
        let want_share = want_slots[i] * 10_000 / want_total;
        // Neither structurally zero nor dominant: a privileged good would blow
        // far past 2500 bps; a starved good would fall under 1000 bps.
        assert!(
            (1_000..=2_500).contains(&endow_share),
            "good {good:?} endowment share {endow_share} bps outside [1000, 2500]"
        );
        assert!(
            (1_000..=2_500).contains(&want_share),
            "good {good:?} want share {want_share} bps outside [1000, 2500]"
        );
    }
}

// 4. (worlds=50, degenerate): zero promotions, zero money units, every world
//    classified and reported.
#[test]
fn degenerate_worlds_never_promote() {
    let (outcomes, summary) = run_emergence_corpus(18, 50, WorldFamily::Degenerate, &envelope());

    assert_eq!(outcomes.len(), 50);
    assert_eq!(summary.degenerate, 50);
    assert_eq!(summary.degenerate_promotions, 0);
    assert_eq!(summary.promoted_in_envelope, 0);

    for outcome in &outcomes {
        assert_eq!(outcome.family, WorldFamily::Degenerate);
        assert!(!outcome.promoted, "degenerate world promoted");
        assert_eq!(outcome.money_units, 0);
        assert_eq!(outcome.winner, None);
        assert_eq!(outcome.class, WorldClass::OutOfEnvelope);
        assert_eq!(outcome.barter_trades, 0);
    }
}

// 5. Every promoted world in the default corpus conserves money and has
//    money_units > 0.
#[test]
fn promoted_worlds_conserve_money() {
    let (outcomes, summary) = run_emergence_corpus(
        BASELINE_SEED,
        BASELINE_WORLDS,
        WorldFamily::Random,
        &envelope(),
    );

    assert_eq!(summary.conservation_failures, 0);
    let promoted: Vec<_> = outcomes.iter().filter(|o| o.promoted).collect();
    assert!(
        !promoted.is_empty(),
        "expected some promotions in the corpus"
    );
    for outcome in promoted {
        assert!(
            outcome.money_conserved,
            "promoted world failed conservation"
        );
        assert!(
            outcome.money_units > 0,
            "promoted world has zero money units"
        );
        assert!(outcome.winner.is_some());
        assert!(outcome.promotion_tick.is_some());
    }
}

// 6. The default 200-world corpus yields >= 2 distinct winner goods.
#[test]
fn winners_are_diverse() {
    let (_outcomes, summary) = run_emergence_corpus(
        BASELINE_SEED,
        BASELINE_WORLDS,
        WorldFamily::Random,
        &envelope(),
    );

    // Pinned lower bound from the seed-18 calibration (observed: 5 distinct
    // winners). Treated as a floor, not an equality, to stay a robust guard.
    assert!(
        summary.distinct_winners >= 2,
        "expected >= 2 distinct winners, got {}",
        summary.distinct_winners
    );
    assert!(summary.distinct_winners >= 5, "calibration floor regressed");
}

// 7. The outcome extractor reproduces the known hand-built anchors.
#[test]
fn harness_reproduces_salt_and_gold_anchors() {
    let salt = anchor_facts(ScenarioName::MengerSaltMoney);
    assert!(salt.promoted);
    assert_eq!(salt.winner, Some(SALT));
    assert_eq!(salt.promotion_tick, Some(3));
    assert_eq!(salt.money_units, 16);
    assert_eq!(salt.winner_share_bps, 5_000);
    assert_eq!(salt.runner_up_share_bps, 2_222);
    assert!(salt.money_conserved);

    let gold = anchor_facts(ScenarioName::MengerGoldMoney);
    assert!(gold.promoted);
    assert_eq!(gold.winner, Some(GOLD));
    assert_eq!(gold.promotion_tick, Some(3));
    assert_eq!(gold.money_units, 16);
    assert!(gold.money_conserved);
}

fn anchor_facts(name: ScenarioName) -> econ::emergence::PromotionFacts {
    let scenario = builtin_market_scenario(name);
    let periods = scenario.periods;
    let mut society = Society::from_scenario(scenario);
    society.run(periods);
    promotion_facts(&society)
}

// 8. In promoted worlds with nonzero post-promotion spot trades, those trades
//    price in the promoted good (and never trade the promoted good itself).
#[test]
fn post_promotion_trades_settle_in_winner() {
    let (outcomes, _summary) = run_emergence_corpus(
        BASELINE_SEED,
        BASELINE_WORLDS,
        WorldFamily::Random,
        &envelope(),
    );

    let mut checked = 0u32;
    for outcome in &outcomes {
        if !outcome.promoted || outcome.post_promotion_spot_trades == 0 {
            continue;
        }
        let winner = outcome.winner.expect("promoted world has a winner");
        let (_world, society) = run_generated_world(
            BASELINE_SEED,
            outcome.world_index,
            WorldFamily::Random,
            &envelope(),
        );
        let post: Vec<_> = society
            .trades
            .iter()
            .filter(|trade| Some(trade.tick) > outcome.promotion_tick)
            .collect();
        assert!(!post.is_empty());
        assert!(
            post.iter().all(|trade| trade.good != winner),
            "post-promotion trade exchanged the money good itself"
        );
        assert!(
            post.iter().all(|trade| trade.price.0 > 0),
            "post-promotion trade priced at zero money units"
        );
        assert!(!society.market_goods().contains(&winner));
        checked += 1;
    }
    assert!(checked > 0, "expected promoted worlds with spot trades");
}

// 9. in_envelope + out_of_envelope == worlds; the rate denominator is the
//    in-envelope count.
#[test]
fn classification_partitions_the_corpus() {
    let (outcomes, summary) = run_emergence_corpus(
        BASELINE_SEED,
        BASELINE_WORLDS,
        WorldFamily::Random,
        &envelope(),
    );

    assert_eq!(
        summary.in_envelope + summary.out_of_envelope,
        summary.worlds
    );
    assert_eq!(summary.worlds, BASELINE_WORLDS);

    let in_envelope = outcomes
        .iter()
        .filter(|o| o.class == WorldClass::InEnvelope)
        .count() as u32;
    let promoted_in_envelope = outcomes
        .iter()
        .filter(|o| o.class == WorldClass::InEnvelope && o.promoted)
        .count() as u32;
    assert_eq!(summary.in_envelope, in_envelope);
    assert_eq!(summary.promoted_in_envelope, promoted_in_envelope);

    let expected_rate = if in_envelope == 0 {
        0
    } else {
        u64::from(promoted_in_envelope) * 10_000 / u64::from(in_envelope)
    };
    assert_eq!(u64::from(summary.emergence_rate_bps), expected_rate);
}

// 10. The M18 baseline calibration is pinned (the regression tripwire), and
//     the population-scaled config floors hold. Since M20 the harness DEFAULT
//     is the adopted skew envelope, so the M18 symmetric baseline is reached
//     via an explicit skew-0 override (the forked-stream design guarantees
//     identical worlds). The M3 golden and M5/M6 anchors are guarded by their
//     own suites; this pins the harness.
#[test]
fn baseline_calibration_is_pinned_at_skew_zero() {
    let skew_zero = EmergenceTuning {
        demand_breadth_skew_bps: Some(0),
        ..EmergenceTuning::default()
    };
    let (_outcomes, summary) = run_emergence_corpus_tuned(
        BASELINE_SEED,
        BASELINE_WORLDS,
        WorldFamily::Random,
        &envelope(),
        &skew_zero,
    );

    assert_eq!(summary.worlds, 200);
    assert_eq!(summary.in_envelope, 200);
    assert_eq!(summary.out_of_envelope, 0);
    assert_eq!(summary.degenerate, 0);
    assert_eq!(summary.promoted_in_envelope, 21);
    assert_eq!(summary.emergence_rate_bps, 1_050);
    assert_eq!(summary.distinct_winners, 5);
    assert_eq!(summary.promotion_tick_median, Some(4));
    assert_eq!(summary.promotion_tick_p90, Some(6));
    assert_eq!(summary.degenerate_promotions, 0);
    assert_eq!(summary.conservation_failures, 0);

    // Pinned scale-floor formulas, verified through the generated config.
    let env = envelope();
    for index in 0..16u32 {
        let world = generate_world(BASELINE_SEED, index, WorldFamily::Random, &env);
        let population = world.profile.population;
        let econ::money::MarketMoneyConfig::Emergent(config) = &world.scenario.money else {
            panic!("generated world must use emergent money");
        };
        assert_eq!(config.min_total_acceptances, u32::from(population).max(12));
        assert_eq!(
            config.min_acceptor_agents,
            u16::try_from((u32::from(population) * 3 / 10).max(3)).unwrap()
        );
    }
}

// Guard: the winner histogram counts only goods that actually won, in id order.
// The histogram is scoped to in-envelope promotions (it shares the rate's
// denominator); this `total_histogram == total_promoted` identity therefore holds
// exactly because the seed-18 baseline is 100% in-envelope (no out-of-envelope
// promotion exists to diverge the two counts).
#[test]
fn winner_histogram_matches_promotions() {
    let (outcomes, summary) = run_emergence_corpus(
        BASELINE_SEED,
        BASELINE_WORLDS,
        WorldFamily::Random,
        &envelope(),
    );

    let total_histogram: u32 = summary.winner_histogram.iter().map(|(_, n)| n).sum();
    let total_promoted = outcomes.iter().filter(|o| o.promoted).count() as u32;
    assert_eq!(total_histogram, total_promoted);

    // Histogram goods are unique and drawn from the pool.
    let mut seen: Vec<GoodId> = Vec::new();
    for (good, count) in &summary.winner_histogram {
        assert!(*count > 0);
        assert!(GOOD_POOL.contains(good));
        assert!(!seen.contains(good), "duplicate winner histogram entry");
        seen.push(*good);
    }
}
