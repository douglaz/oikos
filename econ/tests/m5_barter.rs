use econ::good::{Gold, SALT};
use econ::record::V2Phase;
use econ::scenario::{builtin_market_scenario, ScenarioName};
use econ::society::Society;

#[test]
fn dynamic_money_good_is_not_consumed_as_stock() {
    let scenario = builtin_market_scenario(ScenarioName::MengerSaltMoney);
    let mut society = Society::from_scenario(scenario);

    society.run(4);
    assert_eq!(society.current_money_good(), Some(SALT));
    assert_eq!(society.total_stock(SALT), 0);
    assert_eq!(society.total_money_balance(), Gold(16));

    society.run(3);
    assert_eq!(society.total_stock(SALT), 0);
    assert_eq!(society.total_money_balance(), Gold(16));
    assert!(society
        .v2_records
        .iter()
        .filter(|record| record.phase == V2Phase::Money)
        .all(|record| record.total_money_units == Gold(16)));
}
