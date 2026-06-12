use econ::barter::BarterReason;
use econ::good::{good_name, Gold, GoodId, FOOD, GOLD, SALT};
use econ::record::V2Phase;
use econ::scenario::{builtin_market_scenario, ScenarioName};
use econ::society::Society;

fn run_menger_gold() -> Society {
    let scenario = builtin_market_scenario(ScenarioName::MengerGoldMoney);
    let periods = scenario.periods;
    let mut society = Society::from_scenario(scenario);
    society.run(periods);
    society
}

fn run_menger_salt() -> Society {
    let scenario = builtin_market_scenario(ScenarioName::MengerSaltMoney);
    let periods = scenario.periods;
    let mut society = Society::from_scenario(scenario);
    society.run(periods);
    society
}

fn total_stock_gold_and_money(society: &Society) -> Gold {
    society
        .total_money_balance()
        .saturating_add(Gold(u64::from(society.total_stock(GOLD))))
}

fn scenario_stock_gold(society: &Society) -> u32 {
    society
        .agents
        .iter()
        .map(|agent| agent.stock.get(GOLD))
        .sum()
}

fn promotion_tick(society: &Society) -> u64 {
    society
        .v2_records
        .iter()
        .find(|record| record.promoted_this_tick)
        .map(|record| record.tick)
        .expect("gold promotes")
}

fn snapshot_at(society: &Society, tick: u64, good: GoodId) -> (u32, u16, u16, u16, bool) {
    let snapshot = society
        .saleability_snapshots
        .iter()
        .find(|snapshot| snapshot.tick == tick && snapshot.good == good)
        .expect("saleability snapshot exists");

    (
        snapshot.acceptances,
        snapshot.acceptance_share_bps,
        snapshot.acceptor_agents,
        snapshot.counterpart_goods,
        snapshot.winner,
    )
}

#[test]
fn menger_gold_money_starts_with_physical_gold_stock() {
    let scenario = builtin_market_scenario(ScenarioName::MengerGoldMoney);

    assert!(scenario.agents.iter().all(|agent| agent.gold == Gold::ZERO));
    assert!(scenario
        .agents
        .iter()
        .any(|agent| agent.stock.get(GOLD) > 0));

    let society = Society::from_scenario(scenario);
    assert_eq!(society.current_money_good(), None);
    assert_eq!(society.total_money_balance(), Gold::ZERO);
    assert_eq!(scenario_stock_gold(&society), 16);
}

#[test]
fn menger_gold_money_reports_zero_money_before_promotion() {
    let scenario = builtin_market_scenario(ScenarioName::MengerGoldMoney);
    let mut society = Society::from_scenario(scenario);

    for expected_tick in 0..3 {
        society.step();
        let record = society.v2_records.last().expect("V2 record");
        assert_eq!(record.tick, expected_tick);
        assert_eq!(record.money_good, None);
        assert_eq!(record.total_money_units, Gold::ZERO);
        assert_eq!(society.current_money_good(), None);
        assert!(society.agents.iter().all(|agent| agent.gold == Gold::ZERO));
        assert!(society.total_stock(GOLD) > 0);
    }
}

#[test]
fn menger_gold_money_promotes_gold_once() {
    let society = run_menger_gold();
    let promotions = society
        .v2_records
        .iter()
        .filter(|record| record.promoted_this_tick)
        .collect::<Vec<_>>();

    assert_eq!(promotions.len(), 1);
    assert_eq!(promotions[0].tick, 3);
    assert_eq!(promotions[0].money_good, Some(GOLD));
    assert_eq!(promotions[0].candidate_good, Some(GOLD));
    assert!(society
        .v2_records
        .iter()
        .filter(|record| record.promoted_this_tick)
        .all(|record| record.money_good == Some(GOLD)));
}

#[test]
fn gold_promotion_moves_stock_gold_to_money_balances() {
    let scenario = builtin_market_scenario(ScenarioName::MengerGoldMoney);
    let mut society = Society::from_scenario(scenario);

    society.run(3);
    assert_eq!(society.current_money_good(), None);
    assert!(society.agents.iter().all(|agent| agent.gold == Gold::ZERO));
    let stock_before = society.total_stock(GOLD);
    assert_eq!(stock_before, 16);

    society.step();

    assert_eq!(society.current_money_good(), Some(GOLD));
    assert_eq!(society.total_money_balance(), Gold(u64::from(stock_before)));
    assert!(society.agents.iter().any(|agent| agent.gold > Gold::ZERO));
    assert!(society
        .agents
        .iter()
        .all(|agent| agent.stock.get(GOLD) == 0));
}

#[test]
fn gold_removed_from_market_goods_after_promotion() {
    let scenario = builtin_market_scenario(ScenarioName::MengerGoldMoney);
    let mut society = Society::from_scenario(scenario);

    society.run(4);

    assert_eq!(society.current_money_good(), Some(GOLD));
    assert!(!society.market_goods().contains(&GOLD));
    assert!(society.books.iter().all(|book| book.good != GOLD));
    assert_eq!(society.live_spot_quote_count_for_good(GOLD), 0);
}

#[test]
fn post_promotion_gold_money_clears_spot_trades() {
    let society = run_menger_gold();
    let promoted_at = promotion_tick(&society);
    let post_promotion = society
        .trades
        .iter()
        .filter(|trade| trade.tick > promoted_at)
        .collect::<Vec<_>>();

    assert!(!post_promotion.is_empty());
    assert!(post_promotion.iter().all(|trade| trade.good != GOLD));
    assert!(post_promotion.iter().all(|trade| trade.price > Gold::ZERO));
}

#[test]
fn gold_total_conserved_across_menger_gold_money() {
    let scenario = builtin_market_scenario(ScenarioName::MengerGoldMoney);
    let periods = scenario.periods;
    let mut society = Society::from_scenario(scenario);
    let initial_total = total_stock_gold_and_money(&society);

    for _ in 0..periods {
        society.step();
        assert_eq!(total_stock_gold_and_money(&society), initial_total);
    }
}

#[test]
fn gold_saleability_snapshot_has_nonzero_runner_up() {
    let society = run_menger_gold();
    let promoted_at = promotion_tick(&society);

    assert_eq!(
        snapshot_at(&society, promoted_at, GOLD),
        (9, 5_000, 6, 4, true)
    );
    assert_eq!(
        snapshot_at(&society, promoted_at, FOOD),
        (4, 2_222, 2, 1, false)
    );
    assert_eq!(
        society
            .v2_records
            .iter()
            .find(|record| record.promoted_this_tick)
            .map(|record| record.runner_up_share_bps),
        Some(Some(2_222))
    );
}

#[test]
fn gold_is_accepted_indirectly_before_promotion() {
    let society = run_menger_gold();

    assert!(society.barter_trades.iter().any(|trade| {
        trade.tick < promotion_tick(&society)
            && ((trade.a_gives == GOLD
                && matches!(trade.b_reason, BarterReason::IndirectFor { .. }))
                || (trade.b_gives == GOLD
                    && matches!(trade.a_reason, BarterReason::IndirectFor { .. })))
    }));
}

#[test]
fn post_promotion_barter_tape_has_no_gold_after_promotion() {
    let society = run_menger_gold();
    let promoted_at = promotion_tick(&society);

    assert!(society.barter_trades.iter().all(|trade| {
        trade.tick <= promoted_at || (trade.a_gives != GOLD && trade.b_gives != GOLD)
    }));
}

#[test]
fn spot_tape_contains_no_gold_good_trades() {
    let society = run_menger_gold();

    assert!(society.trades.iter().all(|trade| trade.good != GOLD));
    assert!(society
        .trades
        .iter()
        .all(|trade| good_name(trade.good) != "gold"));
}

#[test]
fn menger_salt_money_still_promotes_salt_at_same_tick() {
    let society = run_menger_salt();
    let promotion = society
        .v2_records
        .iter()
        .find(|record| record.promoted_this_tick)
        .expect("salt promotes");

    assert_eq!(promotion.tick, 3);
    assert_eq!(promotion.money_good, Some(SALT));
    assert_eq!(promotion.phase, V2Phase::Barter);
}

#[test]
fn menger_salt_money_total_money_units_unchanged() {
    let society = run_menger_salt();

    assert_eq!(society.total_money_balance(), Gold(16));
    assert_eq!(
        society
            .v2_records
            .iter()
            .find(|record| record.promoted_this_tick)
            .map(|record| record.total_money_units),
        Some(Gold(16))
    );
}
