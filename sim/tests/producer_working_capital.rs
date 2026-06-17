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
fn threshold_spoilage_raises_production_and_conserves() {
    // Codex's carrying-cost fix, in its working form: a THRESHOLD spoilage (only
    // hoards above a free-storage floor decay, so working/fresh stock is exempt)
    // raises total production over the capital-advance baseline and keeps the
    // colony well-fed longer. (Naive flat spoilage instead collapsed production
    // by rotting the bootstrap; the threshold is what makes carrying cost help.)
    // It does NOT yet achieve sustained production — the residual blocker is the
    // value-scale ordering (hungry producers won't buy inputs) — but it is a
    // clear, conserved improvement.
    let with_spoilage = bread_made_over(SettlementConfig::frontier_spoilage(), 300);
    let baseline = bread_made_over(SettlementConfig::frontier_capital_advance(), 300);
    assert!(
        with_spoilage > baseline,
        "threshold spoilage should raise production over capital-advance, \
         got with_spoilage={with_spoilage}, baseline={baseline}"
    );

    // Spoilage is a real sink — whole-system conservation must still hold.
    let mut settlement = Settlement::generate(1, &SettlementConfig::frontier_spoilage());
    for _ in 0..200 {
        assert!(
            settlement.econ_tick().conserves(),
            "spoilage must conserve (the sink is accounted in report.spoiled)"
        );
    }
}

#[test]
fn stock_and_gold_trace_at_the_halt() {
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
fn in_kind_advance_feeds_producers_and_conserves() {
    // The in-kind subsistence advance: hungry producers are fed staple food in
    // kind (from the provisioned lineages), which removes the chronic-hunger
    // halt — the colony stays well-fed long-horizon — while conservation holds
    // (the food is a transfer, not a mint). This is a welfare win; it does not
    // by itself make the production chain self-sustaining (a fed producer's money
    // is then reserved for its savings want over the low-ranked input want).
    let config = SettlementConfig::frontier_in_kind();
    let bread = config.chain.as_ref().expect("chain").content.bread();
    let mut settlement = Settlement::generate(1, &config);
    for _ in 0..200 {
        assert!(
            settlement.econ_tick().conserves(),
            "the in-kind advance is a transfer and must conserve"
        );
    }
    // The mechanism: producers actually receive staple food in kind.
    let producer_bread: u64 = settlement
        .stock_by_vocation(bread)
        .iter()
        .filter(|(voc, _)| matches!(voc, Vocation::Miller | Vocation::Baker))
        .map(|(_, qty)| *qty)
        .sum();
    assert!(
        producer_bread > 0,
        "the in-kind advance should leave producers holding staple food, got {producer_bread}"
    );
}

#[test]
fn input_advance_conserves_but_triggers_satiation_de_adoption() {
    // The in-kind INPUT advance (a capitalist buys producers' inputs in kind and
    // places them) conserves, but it does NOT make the chain self-sustain: by
    // boosting output it makes producers EARN, which fills their bounded savings
    // want, so role-choice de-adopts them and the chain collapses (worse than the
    // in-kind FOOD advance alone). This locks the conservation of the new
    // transfer and documents the satiation/de-adoption wall (producers gone by a
    // few hundred ticks) — the deepest remaining blocker.
    let config = SettlementConfig::frontier_input_advance();
    let mut settlement = Settlement::generate(1, &config);
    for _ in 0..300 {
        assert!(
            settlement.econ_tick().conserves(),
            "in-kind input advance (money cap→seller, input seller→producer) must conserve"
        );
    }
    let producers =
        settlement.living_count(Vocation::Miller) + settlement.living_count(Vocation::Baker);
    assert_eq!(
        producers, 0,
        "input advance over-feeds producer earnings → savings satiate → de-adoption; \
         expected producers gone by tick 300, got {producers}"
    );
}

#[test]
fn live_order_trace_at_the_halt() {
    // Codex's decisive instrument: reconstruct the live BID/ASK intent for grain
    // across the halt (the reservation orders each agent WOULD post), to tell
    // apart the four candidate gates — (1) miller posts no grain bid, (2) miller
    // bids but no gatherer asks, (3) both post but don't cross, (4) inputs held
    // but recipe fails. Observational (run with --nocapture).
    let config = SettlementConfig::frontier_capital_advance();
    let grain = config.chain.as_ref().expect("chain").content.grain();
    let mut settlement = Settlement::generate(1, &config);
    for tick in 1..=350 {
        let report = settlement.econ_tick();
        if tick >= 240 && tick % 20 == 0 {
            let stats: Vec<_> = settlement
                .order_stats_by_vocation(grain)
                .into_iter()
                .filter(|s| s.bidders > 0 || s.askers > 0)
                .map(|s| {
                    format!(
                        "{:?}{{bid {}@{:?}, ask {}@{:?}}}",
                        s.vocation, s.bidders, s.best_bid, s.askers, s.best_ask
                    )
                })
                .collect();
            eprintln!(
                "t={tick:<4} grain.input={} grain bid/ask: [{}]",
                report.consumed_as_input_of(grain),
                stats.join(", ")
            );
        }
    }

    // Lock the producer-side-gate signature at the halt: the grain market has
    // SELLERS (gatherers post asks) but NO BUYERS (zero grain bidders across all
    // vocations). The would-be buyer (the miller) posts no bid and the
    // money-holding consumers don't want grain — so grain never trades and the
    // chain is input-starved. (Market-time instrumentation further showed even a
    // loan-funded miller posts no grain bid — its money is reserved for its own
    // unmet bread want — which is why a money-only advance can't fix this and an
    // in-kind advance is needed.)
    let grain_orders = settlement.order_stats_by_vocation(grain);
    let askers: usize = grain_orders.iter().map(|s| s.askers).sum();
    let bidders: usize = grain_orders.iter().map(|s| s.bidders).sum();
    assert!(
        askers > 0 && bidders == 0,
        "halt: grain should have sellers and no buyers, got askers={askers} bidders={bidders}"
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
