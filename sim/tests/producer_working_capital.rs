//! EXPERIMENT (producer-working-capital hypothesis).
//!
//! The long-horizon `frontier` colony stops producing bread ~15 ticks after
//! money emerges, even though grain keeps arriving and (under millisats) bread
//! prices climb — strong unmet demand, zero production. The hypothesis under
//! test: the chain producers (Miller/Baker) are drained of cash while the
//! gathering/consuming households accumulate it, so the chain halts for lack of
//! working capital to buy inputs — not money concentration, not divisibility.
//!
//! This test instruments per-vocation gold over a run and checks whether the
//! producers cash-starve relative to the savers. It runs the millisats variant
//! (money de-concentrated, so any starvation is a flow problem, not a
//! corner-the-supply artifact). Game-only; reads via `gold_by_vocation`, so the
//! econ conformance goldens are untouched.

use sim::{Settlement, SettlementConfig, Vocation};

fn gold_of(rows: &[(Vocation, u64)], wanted: &[Vocation]) -> u64 {
    rows.iter()
        .filter(|(voc, _)| wanted.contains(voc))
        .map(|(_, gold)| *gold)
        .sum()
}

#[test]
fn producers_cash_starve_while_savers_accumulate() {
    let config = SettlementConfig::frontier_millisats(1_000);
    let mut settlement = Settlement::generate(1, &config);

    let producers = [Vocation::Miller, Vocation::Baker];
    let savers = [Vocation::Gatherer, Vocation::Consumer];

    // Trajectory print (visible under `--nocapture`): watch the producer purse
    // drain as the saver households fill.
    for tick in 0..=250 {
        if tick % 25 == 0 {
            let rows = settlement.gold_by_vocation();
            eprintln!(
                "t={tick:<4} producers={:>10}  savers={:>10}  all={:?}",
                gold_of(&rows, &producers),
                gold_of(&rows, &savers),
                rows
            );
        }
        if tick < 250 {
            settlement.econ_tick();
        }
    }

    let rows = settlement.gold_by_vocation();
    let producer_gold = gold_of(&rows, &producers);
    let saver_gold = gold_of(&rows, &savers);

    // The hypothesis: by the time the chain has stalled, the producers hold
    // almost none of the money while the saving households hold the bulk. Use a
    // wide margin (savers hold >100x the producers) so the test asserts the
    // direction, not a magnitude.
    assert!(
        saver_gold > producer_gold.saturating_mul(100),
        "producer-working-capital hypothesis: expected savers >> producers, \
         got producers={producer_gold}, savers={saver_gold}, rows={rows:?}"
    );
}

#[test]
fn gold_by_vocation_conserves_against_total() {
    // The per-vocation gold sum (living colonists) plus commons must not exceed
    // the settlement's total gold — a sanity check on the diagnostic accessor.
    let config = SettlementConfig::frontier_millisats(1_000);
    let mut settlement = Settlement::generate(1, &config);
    for _ in 0..60 {
        settlement.econ_tick();
    }
    let by_voc: u64 = settlement
        .gold_by_vocation()
        .iter()
        .map(|(_, gold)| *gold)
        .sum();
    assert!(
        by_voc <= settlement.total_gold().0,
        "living per-vocation gold {by_voc} exceeds total gold {}",
        settlement.total_gold().0
    );
}
