use econ::barter::BarterReason;
use econ::good::{Gold, SALT};
use econ::record::V2Phase;
use econ::report::{render_barter_tape, render_saleability_tape, render_v2, OutputFormat};
use econ::scenario::{builtin_market_scenario, ScenarioName};
use econ::society::{Society, SocietyBuildError};

fn run_menger() -> Society {
    let scenario = builtin_market_scenario(ScenarioName::MengerSaltMoney);
    let periods = scenario.periods;
    let mut society = Society::from_scenario(scenario);
    society.run(periods);
    society
}

#[test]
fn menger_scenario_promotes_salt_once() {
    let society = run_menger();
    let promotions = society
        .v2_records
        .iter()
        .filter(|record| record.promoted_this_tick)
        .collect::<Vec<_>>();

    assert_eq!(promotions.len(), 1);
    assert_eq!(promotions[0].tick, 3);
    assert_eq!(promotions[0].money_good, Some(SALT));
    assert_eq!(promotions[0].phase, V2Phase::Barter);
}

#[test]
fn promotion_moves_money_good_stock_into_money_balances() {
    let society = run_menger();
    let promotion = society
        .v2_records
        .iter()
        .find(|record| record.promoted_this_tick)
        .expect("salt promotes");

    assert_eq!(promotion.total_money_units, Gold(16));
    assert_eq!(society.total_money_balance(), Gold(16));
    assert_eq!(society.total_stock(SALT), 0);
}

#[test]
fn menger_scenario_rejects_preexisting_money_balances() {
    let mut scenario = builtin_market_scenario(ScenarioName::MengerSaltMoney);
    scenario.agents[0].gold = Gold(1);

    assert!(matches!(
        Society::try_from_scenario(scenario),
        Err(SocietyBuildError::V2InitialMoneyBalance)
    ));
}

#[test]
fn promotion_removes_money_good_from_market_goods() {
    let society = run_menger();

    assert_eq!(society.current_money_good(), Some(SALT));
    assert!(!society.market_goods().contains(&SALT));
}

#[test]
fn promotion_cancels_barter_and_money_good_quotes() {
    let scenario = builtin_market_scenario(ScenarioName::MengerSaltMoney);
    let mut society = Society::from_scenario(scenario);

    society.run(3);
    assert!(society.live_barter_offer_count() > 0);
    society.step();

    assert_eq!(society.current_money_good(), Some(SALT));
    assert_eq!(society.live_barter_offer_count(), 0);
    assert_eq!(society.live_spot_quote_count_for_good(SALT), 0);
}

#[test]
fn after_promotion_spot_trades_use_emerged_money() {
    let society = run_menger();
    let promoted_at = society
        .v2_records
        .iter()
        .find(|record| record.promoted_this_tick)
        .map(|record| record.tick)
        .expect("salt promotes");

    assert!(society.trades.iter().any(|trade| trade.tick > promoted_at));
    assert!(society
        .trades
        .iter()
        .all(|trade| trade.tick > promoted_at && trade.good != SALT));
}

#[test]
fn post_promotion_total_money_units_are_conserved() {
    let society = run_menger();
    let promoted_at = society
        .v2_records
        .iter()
        .find(|record| record.promoted_this_tick)
        .expect("salt promotes");
    let total = promoted_at.total_money_units;

    for record in society
        .v2_records
        .iter()
        .filter(|record| record.phase == V2Phase::Money)
    {
        assert_eq!(record.total_money_units, total);
    }
}

#[test]
fn menger_money_emergence_is_deterministic() {
    let first = run_menger();
    let second = run_menger();

    assert_eq!(first.v2_records, second.v2_records);
    assert_eq!(first.barter_trades, second.barter_trades);
    assert_eq!(first.saleability_snapshots, second.saleability_snapshots);
}

#[test]
fn menger_csv_has_stable_header() {
    let society = run_menger();
    let output = render_v2(&society.v2_records, OutputFormat::Csv);

    assert_eq!(
        output.lines().next(),
        Some("tick,phase,money_good,promoted_this_tick,barter_trades,spot_trades,candidate_good,candidate_share_bps,runner_up_share_bps,total_money_units,bid_count,ask_count,expired_orders")
    );
}

#[test]
fn barter_tape_has_stable_header() {
    let society = run_menger();
    let output = render_barter_tape(&society.barter_trades);

    assert_eq!(
        output.lines().next(),
        Some("tick,a,b,a_gives,b_gives,qty,a_reason,b_reason")
    );
}

#[test]
fn saleability_tape_has_stable_header() {
    let society = run_menger();
    let output = render_saleability_tape(&society.saleability_snapshots);

    assert_eq!(
        output.lines().next(),
        Some("tick,good,acceptances,acceptance_share_bps,acceptor_agents,counterpart_goods,eligible,winner")
    );
}

#[test]
fn salt_final_saleability_snapshot_beats_runner_up() {
    let society = run_menger();
    let promotion = society
        .v2_records
        .iter()
        .find(|record| record.promoted_this_tick)
        .expect("salt promotes");

    assert_eq!(promotion.candidate_good, Some(SALT));
    assert_eq!(promotion.candidate_share_bps, Some(5_000));
    assert_eq!(promotion.runner_up_share_bps, Some(2_222));
}

#[test]
fn v2_records_blank_tied_and_post_money_candidates() {
    let society = run_menger();
    let tied_opening = society
        .v2_records
        .iter()
        .find(|record| record.tick == 0)
        .expect("opening V2 record");

    assert_eq!(tied_opening.candidate_good, None);
    assert_eq!(tied_opening.candidate_share_bps, None);
    assert_eq!(tied_opening.runner_up_share_bps, None);

    for record in society
        .v2_records
        .iter()
        .filter(|record| record.phase == V2Phase::Money)
    {
        assert_eq!(record.candidate_good, None);
        assert_eq!(record.candidate_share_bps, None);
        assert_eq!(record.runner_up_share_bps, None);
    }
}

#[test]
fn connector_accepts_salt_indirectly_once_provisional_leader_exists() {
    let scenario = builtin_market_scenario(ScenarioName::MengerSaltMoney);
    let mut society = Society::from_scenario(scenario);

    society.run(2);
    assert!(society.barter_trades.iter().all(|trade| !matches!(
        trade.a_reason,
        BarterReason::IndirectFor { .. }
    ) && !matches!(
        trade.b_reason,
        BarterReason::IndirectFor { .. }
    )));

    society.step();

    assert!(society.barter_trades.iter().any(|trade| {
        trade.tick == 2
            && trade.b.0 == 6
            && matches!(trade.b_reason, BarterReason::IndirectFor { .. })
    }));
}

#[test]
fn connector_indirect_offer_requires_unsatisfied_final_want() {
    let mut scenario = builtin_market_scenario(ScenarioName::MengerSaltMoney);
    let connector = scenario
        .agents
        .iter_mut()
        .find(|agent| agent.id.0 == 6)
        .expect("connector exists");
    connector.scale.clear();

    let periods = scenario.periods;
    let mut society = Society::from_scenario(scenario);
    society.run(periods);

    assert!(society.barter_trades.iter().all(|trade| {
        !matches!(trade.a_reason, BarterReason::IndirectFor { .. })
            && !matches!(trade.b_reason, BarterReason::IndirectFor { .. })
    }));
}
