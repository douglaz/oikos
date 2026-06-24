use econ::agent::{Agent, AgentId, Role, Want, WantKind};
use econ::expect::PriceBelief;
use econ::good::{Gold, GoodId, Horizon, Stock, FOOD, SALT, WOOD};
use econ::marketability::{GoodMarketability, MarketabilityConfig};

fn want(good: GoodId) -> Want {
    Want {
        kind: WantKind::Good(good),
        horizon: Horizon::Next,
        qty: 1,
        satisfied: false,
    }
}

fn indirect_test_agent(target: GoodId, stock: Stock) -> Agent {
    Agent {
        id: AgentId(1),
        scale: vec![want(target)],
        stock,
        gold: Gold::ZERO,
        labor_capacity: 0,
        hunger_deficit: 0,
        roles: vec![Role::Trader],
        expect: vec![PriceBelief::new(Gold(1), Gold(1)); usize::from(SALT.0) + 1],
    }
}

#[test]
fn marketability_config_defaults_are_inert_and_readable() {
    let mut stock = Stock::new(SALT.0);
    stock.add(WOOD, 1);
    let agent = indirect_test_agent(FOOD, stock.clone());
    let empty = MarketabilityConfig::default();

    let blind = agent
        .would_accept_indirect_barter_swap_with_stock(&stock, WOOD, SALT, FOOD, 1, false, &empty);
    let empty_enabled = agent
        .would_accept_indirect_barter_swap_with_stock(&stock, WOOD, SALT, FOOD, 1, true, &empty);

    assert!(blind, "the baseline indirect barter should be acceptable");
    assert_eq!(
        blind, empty_enabled,
        "an enabled empty table treats every good as durable and low-carry"
    );

    let table = MarketabilityConfig::default().with_good(
        SALT,
        GoodMarketability {
            decay_bps: 25,
            carry_cost: 1,
        },
    );
    assert_eq!(
        table.good(SALT),
        GoodMarketability {
            decay_bps: 25,
            carry_cost: 1
        }
    );
    assert_eq!(table.good(FOOD), GoodMarketability::default());
}
