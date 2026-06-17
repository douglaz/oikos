//! S3 — working-capital persistence (real retained earnings, no planner loan).
//!
//! The producer's working capital is REAL saved money it keeps across ticks, not
//! a per-tick planner top-up. The endogenous economy runs with the revolving
//! capital-advance loan OFF (`capital_advance = false`): a producer funds its
//! input bids from earnings it retained, and the demand-responsive restock gate
//! (it buys input only as it clears output) keeps it from draining its purse by
//! over-producing into a saturated market. These tests prove the producers hold a
//! persistent, non-trivial cash balance over a long horizon with no loan, and that
//! production sustains on that retained capital.

use sim::{Settlement, SettlementConfig, Vocation};

fn producer_gold(s: &Settlement) -> u64 {
    s.gold_by_vocation()
        .iter()
        .filter(|(v, _)| matches!(v, Vocation::Miller | Vocation::Baker))
        .map(|(_, g)| g)
        .sum()
}

#[test]
fn endogenous_economy_uses_no_planner_loan() {
    // The endogenous DoD config turns the revolving capital-advance loan OFF, so
    // working capital can only be real retained earnings (the milestone's contract:
    // "no curated advances" would be false with a per-tick loan).
    let config = SettlementConfig::frontier_endogenous();
    let chain = config.chain.as_ref().expect("chain");
    assert!(
        !chain.capital_advance,
        "the endogenous economy must not use the per-tick planner loan"
    );
    assert!(
        !chain.subsistence_advance && !chain.input_advance,
        "the endogenous economy must not use global food/input placement"
    );
}

#[test]
fn producers_retain_working_capital_over_a_long_horizon() {
    let config = SettlementConfig::frontier_endogenous();
    let bread = config.chain.as_ref().unwrap().content.bread();
    let mut settlement = Settlement::generate(1, &config);

    // Producers are seeded with a little working capital; with no loan and no
    // sweep, they must still hold a non-trivial cash balance hundreds of ticks
    // later — funded entirely by retained earnings from selling their output.
    let mut min_late_gold = u64::MAX;
    let mut late_bread = 0u64;
    for tick in 0..900u64 {
        let report = settlement.econ_tick();
        assert!(report.conserves(), "must conserve (tick {tick})");
        if tick >= 600 {
            min_late_gold = min_late_gold.min(producer_gold(&settlement));
            late_bread += report.produced_of(bread);
        }
    }

    assert!(
        min_late_gold > 0,
        "producers should retain working capital across the run (never swept to \
         zero), but the late-window minimum was {min_late_gold}"
    );
    assert!(
        late_bread > 0,
        "production should sustain on retained capital past tick 600, got \
         {late_bread} bread"
    );
}

#[test]
fn retained_capital_outperforms_the_drained_baseline() {
    // A real differential isolating the LOCAL subsistence base (and the working
    // capital it frees up) as the load-bearing piece. Both arms run the SAME
    // endogenous config; the only difference is the local producer-subsistence
    // floor:
    //
    //   - TREATMENT (`producer_subsistence = 4`): each producer's own household
    //     hearth feeds it the staple + WOOD, so its purse frees ENTIRELY for recipe
    //     inputs. It retains a persistent working-capital balance across the whole
    //     tail and the chain keeps producing.
    //   - BASELINE (`producer_subsistence = 0`): with no local hearth, the producer
    //     must spend its purse on its own hunger and warmth, drains cash-light, and
    //     the chain stalls — it ends the run swept to zero.
    //
    // The treatment must keep strictly more working capital in producer hands than
    // the drained baseline (and must itself never be swept to zero), and production
    // must sustain in the treatment while it stalls in the baseline.
    fn run_and_measure(producer_subsistence: u32) -> (u64, u64) {
        let mut config = SettlementConfig::frontier_endogenous();
        config.chain.as_mut().expect("chain").producer_subsistence = producer_subsistence;
        let bread = config.chain.as_ref().expect("chain").content.bread();
        let mut s = Settlement::generate(1, &config);
        let mut min_late_gold = u64::MAX;
        let mut late_bread = 0u64;
        for tick in 0..900u64 {
            let report = s.econ_tick();
            assert!(
                report.conserves(),
                "must conserve (producer_subsistence={producer_subsistence}, tick {tick})"
            );
            if tick >= 600 {
                min_late_gold = min_late_gold.min(producer_gold(&s));
                late_bread += report.produced_of(bread);
            }
        }
        (min_late_gold, late_bread)
    }

    let (treatment_gold, treatment_bread) = run_and_measure(4);
    let (baseline_gold, baseline_bread) = run_and_measure(0);

    assert!(
        treatment_gold > baseline_gold,
        "the local subsistence base must keep more working capital in producer \
         hands than the drained baseline: treatment late-min={treatment_gold}, \
         baseline late-min={baseline_gold}"
    );
    assert!(
        treatment_gold > 0,
        "the endogenous economy must never sweep producers to zero working capital, \
         but the treatment late-window minimum was {treatment_gold}"
    );
    assert!(
        treatment_bread > baseline_bread,
        "production must sustain on the retained capital but stall in the drained \
         baseline: treatment={treatment_bread} bread, baseline={baseline_bread} bread"
    );
}
