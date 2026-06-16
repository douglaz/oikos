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

fn bread_made_over(config: SettlementConfig, ticks: u32) -> u64 {
    let mut settlement = Settlement::generate(1, &config);
    let bread = config
        .chain
        .as_ref()
        .map(|c| c.content.bread())
        .expect("chain");
    let mut total = 0;
    for _ in 0..ticks {
        let report = settlement.econ_tick();
        total += report.produced_of(bread);
    }
    total
}

#[test]
fn repaid_capital_advance_sustains_roles_and_raises_production() {
    // The faithful capital advance is a REVOLVING loan: working capital is
    // advanced to cashless producers before the market and repaid from their
    // sales after it, so they stay cash-light and their future-money want stays
    // UNMET — role-choice keeps them adopted (an unrepaid gift, by contrast,
    // satisfies the want and gets them de-adopted, suppressing production). This
    // locks the two effects: the loan raises total production above the millisats
    // baseline, and the producers are still in role late in the run.
    let with_advance = bread_made_over(SettlementConfig::frontier_capital_advance(), 300);
    let baseline = bread_made_over(SettlementConfig::frontier_millisats(1_000), 300);
    assert!(
        with_advance > baseline,
        "revolving capital advance should raise production above baseline, \
         got with_advance={with_advance}, baseline={baseline}"
    );

    let mut settlement = Settlement::generate(1, &SettlementConfig::frontier_capital_advance());
    for _ in 0..200 {
        settlement.econ_tick();
    }
    let producers =
        settlement.living_count(Vocation::Miller) + settlement.living_count(Vocation::Baker);
    assert!(
        producers > 0,
        "the revolving advance should keep producers adopted at tick 200, got {producers}"
    );
}

#[test]
fn market_gate_trace_at_the_halt() {
    // Observational diagnostic (run with --nocapture): trace the input market
    // across the ~tick-300 production halt of the revolving-loan colony. The
    // question (per the Codex read): is it SUPPLY withdrawal (raw grain piles up
    // with gatherers/savers who won't sell while millers hold none), DEMAND
    // collapse (consumers sit on bread and post no bids), or a recipe/stock bug?
    let config = SettlementConfig::frontier_capital_advance();
    let content = config.chain.as_ref().expect("chain").content.clone();
    let grain = content.grain();
    let flour = content.flour();
    let bread = content.bread();
    let mut settlement = Settlement::generate(1, &config);
    for tick in 1..=350 {
        let report = settlement.econ_tick();
        if tick >= 240 && tick % 10 == 0 {
            eprintln!(
                "t={tick:<4} bread.made={} grain.input={} flour.input={}\n  grain.stock={:?}\n  flour.stock={:?}\n  bread.stock={:?}\n  gold={:?}",
                report.produced_of(bread),
                report.consumed_as_input_of(grain),
                report.consumed_as_input_of(flour),
                settlement.stock_by_vocation(grain),
                settlement.stock_by_vocation(flour),
                settlement.stock_by_vocation(bread),
                settlement.gold_by_vocation(),
            );
        }
    }

    // Lock the halt signature: grain piles up with the gatherers (the sellers)
    // and never reaches the millers, and consumers hoard the bread + money while
    // producers hold neither. The chain dies from input starvation driven by a
    // satiated, withdrawn consumer class — not lack of producer working capital.
    let grain_stock = settlement.stock_by_vocation(grain);
    let grain_of = |voc| {
        grain_stock
            .iter()
            .find(|(v, _)| *v == voc)
            .map_or(0, |(_, q)| *q)
    };
    assert!(
        grain_of(Vocation::Gatherer) > 1_000 && grain_of(Vocation::Miller) == 0,
        "halt signature: grain should pile with gatherers and not reach millers, got gatherer={} miller={}",
        grain_of(Vocation::Gatherer),
        grain_of(Vocation::Miller),
    );
    let bread_stock = settlement.stock_by_vocation(bread);
    let consumer_bread = bread_stock
        .iter()
        .find(|(v, _)| *v == Vocation::Consumer)
        .map_or(0, |(_, q)| *q);
    assert!(
        consumer_bread > 1_000,
        "halt signature: consumers should hoard bread while producers starve, got {consumer_bread}"
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
