use econ::agent::{Agent, AgentId, Role, Want, WantKind};
use econ::expect::PriceBelief;
use econ::good::{Gold, GoodId, Horizon, Stock, CLOTH, FOOD, ORE, SALT, WOOD};
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

#[test]
fn bad_medium_declined_good_medium_accepted() {
    let mut stock = Stock::new(ORE.0);
    stock.add(ORE, 1);
    let agent = indirect_test_agent(CLOTH, stock.clone());
    let config = MarketabilityConfig {
        hold_horizon: 1,
        ..MarketabilityConfig::default()
    }
    .with_good(
        SALT,
        GoodMarketability {
            decay_bps: 0,
            carry_cost: 0,
        },
    )
    .with_good(
        FOOD,
        GoodMarketability {
            decay_bps: 10_000,
            carry_cost: 0,
        },
    )
    .with_good(
        WOOD,
        GoodMarketability {
            decay_bps: 0,
            carry_cost: 1,
        },
    );

    assert!(
        agent.would_accept_indirect_barter_swap_with_stock(
            &stock, ORE, SALT, CLOTH, 1, true, &config
        ),
        "durable low-carry SALT should be accepted as a medium"
    );
    assert!(
        !agent.would_accept_indirect_barter_swap_with_stock(
            &stock, ORE, FOOD, CLOTH, 1, true, &config
        ),
        "perishable FOOD should be declined as a medium"
    );
    assert!(
        !agent.would_accept_indirect_barter_swap_with_stock(
            &stock, ORE, WOOD, CLOTH, 1, true, &config
        ),
        "durable high-carry WOOD should be declined as a medium"
    );

    for receive_good in [SALT, FOOD, WOOD] {
        assert!(
            agent.would_accept_indirect_barter_swap_with_stock(
                &stock,
                ORE,
                receive_good,
                CLOTH,
                1,
                false,
                &config
            ),
            "with the flag off, {receive_good:?} remains marketability-blind"
        );
    }
    assert_eq!(agent.scale, vec![want(CLOTH)]);
}
