use econ::agent::{Agent, AgentId, Role, Want, WantKind};
use econ::expect::PriceBelief;
use econ::good::{Gold, GoodId, Horizon, Stock, FOOD, GOLD, NET, SALT, WOOD};
use econ::market::{Order, OrderBook, OrderSide, Reservations};
use econ::money::{DesignatedMoney, MarketMoneyConfig};
use econ::project::builtin_recipes;
use econ::record::MarketRecord;
use econ::scenario::{builtin_market_scenario, MarketScenario, ScenarioName};
use econ::society::Society;

const M1_SCENARIOS: [ScenarioName; 3] = [
    ScenarioName::MarketBarterishGold,
    ScenarioName::MarketPriceDiscovery,
    ScenarioName::MarketNoMutualBenefit,
];

fn run(name: ScenarioName) -> Society {
    let scenario = builtin_market_scenario(name);
    let periods = scenario.periods;
    let mut society = Society::from_scenario(scenario);
    society.run(periods);
    society
}

#[test]
fn market_gold_is_conserved() {
    for name in M1_SCENARIOS {
        let scenario = builtin_market_scenario(name);
        let initial_gold = scenario
            .agents
            .iter()
            .fold(Gold::ZERO, |total, agent| total.saturating_add(agent.gold));
        let periods = scenario.periods;
        let mut society = Society::from_scenario(scenario);
        for _ in 0..periods {
            society.step();
            assert_eq!(society.total_gold(), initial_gold);
            assert_eq!(society.records.last().unwrap().total_gold, initial_gold);
            for agent in &society.agents {
                assert!(society.reservations.reserved_gold(agent.id) <= agent.gold);
            }
        }
    }
}

#[test]
fn market_goods_are_conserved_except_production_consumption() {
    for name in M1_SCENARIOS {
        let scenario = builtin_market_scenario(name);
        let initial_food = total_stock(&scenario.agents, FOOD);
        let initial_wood = total_stock(&scenario.agents, WOOD);
        let initial_net = total_stock(&scenario.agents, NET);
        let periods = scenario.periods;
        let mut society = Society::from_scenario(scenario);
        for _ in 0..periods {
            society.step();
            assert_eq!(society.total_stock(FOOD), initial_food);
            assert_eq!(society.total_stock(WOOD), initial_wood);
            assert_eq!(society.total_stock(NET), initial_net);
        }
    }
}

#[test]
fn market_price_is_resting_order_limit() {
    let mut agents = vec![agent(1, Gold(10), 0), agent(2, Gold(0), 2)];
    let mut reservations = Reservations::new(&agents, 3);
    let mut book = OrderBook::new(FOOD);
    let ask = order(2, OrderSide::Ask, Gold(5), 1, 1);
    assert!(reservations.reserve_order(&agents, &ask));
    assert!(book
        .add_order(ask, 0, &mut agents, &mut reservations)
        .is_empty());
    let bid = order(1, OrderSide::Bid, Gold(7), 1, 2);
    assert!(reservations.reserve_order(&agents, &bid));
    let trades = book.add_order(bid, 0, &mut agents, &mut reservations);
    assert_eq!(trades[0].price, Gold(5));

    let mut agents = vec![agent(1, Gold(10), 0), agent(2, Gold(0), 2)];
    let mut reservations = Reservations::new(&agents, 3);
    let mut book = OrderBook::new(FOOD);
    let bid = order(1, OrderSide::Bid, Gold(7), 1, 1);
    assert!(reservations.reserve_order(&agents, &bid));
    assert!(book
        .add_order(bid, 0, &mut agents, &mut reservations)
        .is_empty());
    let ask = order(2, OrderSide::Ask, Gold(5), 1, 2);
    assert!(reservations.reserve_order(&agents, &ask));
    let trades = book.add_order(ask, 0, &mut agents, &mut reservations);
    assert_eq!(trades[0].price, Gold(7));
}

#[test]
fn market_no_self_trades() {
    let mut agents = vec![agent(1, Gold(10), 2)];
    let mut reservations = Reservations::new(&agents, 3);
    let mut book = OrderBook::new(FOOD);
    let ask = order(1, OrderSide::Ask, Gold(3), 1, 1);
    assert!(reservations.reserve_order(&agents, &ask));
    book.add_order(ask, 0, &mut agents, &mut reservations);
    let bid = order(1, OrderSide::Bid, Gold(5), 1, 2);
    assert!(reservations.reserve_order(&agents, &bid));
    let trades = book.add_order(bid, 0, &mut agents, &mut reservations);

    assert!(trades.is_empty());
}

#[test]
fn trade_rejects_seller_gold_overflow_without_releasing_reserves() {
    let mut agents = vec![agent(1, Gold(10), 0), agent(2, Gold(u64::MAX), 1)];
    let mut reservations = Reservations::new(&agents, 3);
    let mut book = OrderBook::new(FOOD);
    let ask = order(2, OrderSide::Ask, Gold(1), 1, 1);
    assert!(reservations.reserve_order(&agents, &ask));
    assert!(book
        .add_order(ask, 0, &mut agents, &mut reservations)
        .is_empty());

    let bid = order(1, OrderSide::Bid, Gold(1), 1, 2);
    assert!(reservations.reserve_order(&agents, &bid));
    let trades = book.add_order(bid, 0, &mut agents, &mut reservations);

    assert!(trades.is_empty());
    assert_eq!(agents[0].gold, Gold(10));
    assert_eq!(agents[1].gold, Gold(u64::MAX));
    assert_eq!(reservations.reserved_gold(AgentId(1)), Gold(1));
    assert_eq!(reservations.reserved_stock(AgentId(2), FOOD), 1);
    assert_eq!(book.live_order_counts(), (1, 1));
}

#[test]
fn market_barterish_gold_trades_food_and_wood() {
    let society = run(ScenarioName::MarketBarterishGold);

    assert!(society.records.iter().any(|record| record.food_volume > 0));
    assert!(society.records.iter().any(|record| record.wood_volume > 0));
    assert_eq!(society.total_gold(), Gold(19));
}

#[test]
fn market_no_mutual_benefit_has_zero_trades() {
    let society = run(ScenarioName::MarketNoMutualBenefit);

    assert_eq!(society.trades.len(), 0);
}

#[test]
fn market_record_tracks_custom_goods_generically() {
    let custom = GoodId(4);
    let mut seller_stock = Stock::new(custom.0);
    seller_stock.add(custom, 1);
    let buyer_stock = Stock::new(custom.0);
    let mut buyer_expect = vec![PriceBelief::new(Gold::ZERO, Gold(1)); usize::from(custom.0) + 1];
    buyer_expect[usize::from(custom.0)] = PriceBelief::new(Gold(1), Gold(1));
    let mut seller_expect = buyer_expect.clone();
    seller_expect[usize::from(custom.0)] = PriceBelief::new(Gold(1), Gold(1));
    let scenario = MarketScenario {
        name: "custom-good-record",
        scenario: ScenarioName::MarketPriceDiscovery,
        seed: 1,
        periods: 1,
        agents: vec![
            Agent {
                id: AgentId(1),
                scale: wants(&[(WantKind::Good(GOLD), Horizon::Later(1), 1)]),
                stock: seller_stock,
                gold: Gold::ZERO,
                labor_capacity: 0,
                hunger_deficit: 0,
                roles: vec![Role::Trader],
                expect: seller_expect,
            },
            Agent {
                id: AgentId(2),
                scale: wants(&[
                    (WantKind::Good(GOLD), Horizon::Later(1), 1),
                    (WantKind::Good(custom), Horizon::Next, 1),
                ]),
                stock: buyer_stock,
                gold: Gold(3),
                labor_capacity: 0,
                hunger_deficit: 0,
                roles: vec![Role::Trader],
                expect: buyer_expect,
            },
        ],
        recipes: Vec::new(),
        events: Vec::new(),
        money: MarketMoneyConfig::Designated(DesignatedMoney { good: GOLD }),
    };
    let mut society = Society::from_scenario(scenario);

    society.step();

    let record = society.records.last().unwrap();
    assert_eq!(record.good_volumes, vec![(custom, 1)]);
    assert_eq!(record.last_prices, vec![(custom, Gold(1))]);
    assert_eq!(record.food_volume, 0);
}

#[test]
fn market_posts_in_agent_id_order_not_vector_order() {
    let scenario = MarketScenario {
        name: "agent-id-order",
        scenario: ScenarioName::MarketPriceDiscovery,
        seed: 1,
        periods: 1,
        agents: vec![
            market_test_agent(
                2,
                Gold(5),
                stock_with(0, 0, 0),
                wants(&[
                    (WantKind::Good(GOLD), Horizon::Later(1), 2),
                    (WantKind::Good(FOOD), Horizon::Next, 1),
                ]),
                Gold(3),
            ),
            market_test_agent(
                1,
                Gold(0),
                stock_with(1, 0, 0),
                wants(&[(WantKind::Good(GOLD), Horizon::Later(1), 1)]),
                Gold(1),
            ),
        ],
        recipes: builtin_recipes(),
        events: Vec::new(),
        money: MarketMoneyConfig::Designated(DesignatedMoney { good: GOLD }),
    };
    let mut society = Society::from_scenario(scenario);

    society.step();

    assert_eq!(society.trades.len(), 1);
    assert_eq!(society.trades[0].seller, AgentId(1));
    assert_eq!(society.trades[0].price, Gold(1));
}

#[test]
fn unchanged_shaded_limit_preserves_resting_time_priority() {
    let scenario = MarketScenario {
        name: "stable-time-priority",
        scenario: ScenarioName::MarketPriceDiscovery,
        seed: 1,
        periods: 2,
        agents: vec![
            market_test_agent(
                1,
                Gold(0),
                stock_with(1, 0, 0),
                wants(&[(WantKind::Good(GOLD), Horizon::Later(1), 1)]),
                Gold(2),
            ),
            market_test_agent(
                2,
                Gold(0),
                stock_with(1, 0, 0),
                wants(&[(WantKind::Good(GOLD), Horizon::Later(1), 1)]),
                Gold(2),
            ),
            market_test_agent(
                3,
                Gold(0),
                stock_with(1, 0, 0),
                wants(&[(WantKind::Good(GOLD), Horizon::Later(1), 1)]),
                Gold(1),
            ),
            market_test_agent(
                4,
                Gold(10),
                stock_with(0, 0, 0),
                wants(&[
                    (WantKind::Good(GOLD), Horizon::Later(1), 5),
                    (WantKind::Good(FOOD), Horizon::Next, 2),
                ]),
                Gold(3),
            ),
        ],
        recipes: builtin_recipes(),
        events: Vec::new(),
        money: MarketMoneyConfig::Designated(DesignatedMoney { good: GOLD }),
    };
    let mut society = Society::from_scenario(scenario);

    society.step();
    society.step();

    assert_eq!(society.trades.len(), 2);
    assert_eq!(society.trades[0].seller, AgentId(1));
    assert_eq!(society.trades[1].seller, AgentId(2));
}

#[test]
fn society_direct_labor_respects_leisure_rank() {
    let scenario = MarketScenario {
        name: "leisure-before-work",
        scenario: ScenarioName::MarketPriceDiscovery,
        seed: 1,
        periods: 1,
        agents: vec![market_test_agent(
            1,
            Gold::ZERO,
            stock_with(1, 0, 0),
            wants(&[
                (WantKind::Good(FOOD), Horizon::Now, 1),
                (WantKind::Leisure, Horizon::Now, 1),
                (WantKind::Good(FOOD), Horizon::Next, 1),
            ]),
            Gold(1),
        )],
        recipes: builtin_recipes(),
        events: Vec::new(),
        money: MarketMoneyConfig::Designated(DesignatedMoney { good: GOLD }),
    };
    let mut society = Society::from_scenario(scenario);
    society.agents[0].labor_capacity = 1;

    society.step();

    assert_eq!(society.agents[0].stock.get(FOOD), 0);
}

#[test]
fn live_ask_reserve_is_not_consumed_next_tick() {
    let mut stock = Stock::new(3);
    stock.add(FOOD, 1);
    let scenario = MarketScenario {
        name: "reserved-ask-consumption",
        scenario: ScenarioName::MarketPriceDiscovery,
        seed: 1,
        periods: 2,
        agents: vec![Agent {
            id: AgentId(1),
            scale: vec![
                Want {
                    kind: WantKind::Good(econ::good::GOLD),
                    horizon: Horizon::Later(1),
                    qty: 1,
                    satisfied: false,
                },
                Want {
                    kind: WantKind::Good(FOOD),
                    horizon: Horizon::Next,
                    qty: 1,
                    satisfied: false,
                },
            ],
            stock,
            gold: Gold::ZERO,
            labor_capacity: 0,
            hunger_deficit: 0,
            roles: vec![Role::Trader],
            expect: Vec::new(),
        }],
        recipes: builtin_recipes(),
        events: Vec::new(),
        money: MarketMoneyConfig::Designated(DesignatedMoney {
            good: econ::good::GOLD,
        }),
    };
    let mut society = Society::from_scenario(scenario);
    society.step();
    assert_eq!(society.reservations.reserved_stock(AgentId(1), FOOD), 1);

    society.agents[0].scale[1].horizon = Horizon::Now;
    society.step();

    assert_eq!(society.agents[0].stock.get(FOOD), 1);
    assert_eq!(society.reservations.reserved_stock(AgentId(1), FOOD), 1);
}

#[test]
fn live_bid_reserved_gold_does_not_satisfy_gold_wants_next_tick() {
    let scenario = MarketScenario {
        name: "reserved-bid-gold-satisfaction",
        scenario: ScenarioName::MarketPriceDiscovery,
        seed: 1,
        periods: 2,
        agents: vec![market_test_agent(
            1,
            Gold(1),
            stock_with(0, 0, 0),
            wants(&[
                (WantKind::Good(FOOD), Horizon::Next, 1),
                (WantKind::Good(GOLD), Horizon::Later(1), 1),
            ]),
            Gold(1),
        )],
        recipes: builtin_recipes(),
        events: Vec::new(),
        money: MarketMoneyConfig::Designated(DesignatedMoney { good: GOLD }),
    };
    let mut society = Society::from_scenario(scenario);

    society.step();
    assert_eq!(society.reservations.reserved_gold(AgentId(1)), Gold(1));
    society.step();

    assert_eq!(society.agents[0].gold, Gold(1));
    assert_eq!(society.reservations.reserved_gold(AgentId(1)), Gold(1));
    assert!(!society.agents[0].scale[1].satisfied);
}

#[test]
fn overreserved_live_bid_is_canceled_before_it_can_rest() {
    let scenario = MarketScenario {
        name: "overreserved-live-bid",
        scenario: ScenarioName::MarketPriceDiscovery,
        seed: 1,
        periods: 2,
        agents: vec![market_test_agent(
            1,
            Gold(1),
            stock_with(0, 0, 0),
            wants(&[(WantKind::Good(FOOD), Horizon::Next, 1)]),
            Gold(1),
        )],
        recipes: builtin_recipes(),
        events: Vec::new(),
        money: MarketMoneyConfig::Designated(DesignatedMoney { good: GOLD }),
    };
    let mut society = Society::from_scenario(scenario);

    society.step();
    assert_eq!(society.reservations.reserved_gold(AgentId(1)), Gold(1));
    assert_eq!(live_orders(&society), (1, 0));

    society.reservations.gold[0] = Gold(2);
    society.step();

    assert!(society.reservations.reserved_gold(AgentId(1)) <= society.agents[0].gold);
    assert_eq!(live_orders(&society), (0, 0));
}

#[test]
fn filled_trade_cancels_other_quotes_that_lost_reservation() {
    let scenario = MarketScenario {
        name: "post-fill-revalidation",
        scenario: ScenarioName::MarketPriceDiscovery,
        seed: 1,
        periods: 1,
        agents: vec![
            market_test_agent(
                1,
                Gold(0),
                stock_with(1, 1, 0),
                wants(&[(WantKind::Good(GOLD), Horizon::Later(1), 1)]),
                Gold(1),
            ),
            market_test_agent(
                2,
                Gold(2),
                stock_with(0, 0, 0),
                wants(&[
                    (WantKind::Good(GOLD), Horizon::Later(1), 1),
                    (WantKind::Good(FOOD), Horizon::Next, 1),
                ]),
                Gold(1),
            ),
            market_test_agent(
                3,
                Gold(2),
                stock_with(0, 0, 0),
                wants(&[
                    (WantKind::Good(GOLD), Horizon::Later(1), 1),
                    (WantKind::Good(WOOD), Horizon::Next, 1),
                ]),
                Gold(1),
            ),
        ],
        recipes: builtin_recipes(),
        events: Vec::new(),
        money: MarketMoneyConfig::Designated(DesignatedMoney { good: GOLD }),
    };
    let mut society = Society::from_scenario(scenario);

    society.step();

    assert_eq!(society.trades.len(), 1);
    assert_eq!(society.trades[0].good, FOOD);
    assert_eq!(society.agents[0].stock.get(WOOD), 1);
    assert_eq!(society.reservations.reserved_stock(AgentId(1), WOOD), 0);
}

#[test]
fn designated_non_gold_money_protects_money_wants_when_bidding() {
    let scenario = MarketScenario {
        name: "designated-salt-money",
        scenario: ScenarioName::MarketPriceDiscovery,
        seed: 1,
        periods: 1,
        agents: vec![
            market_test_agent(
                1,
                Gold(2),
                stock_with(0, 0, 0),
                wants(&[
                    (WantKind::Good(SALT), Horizon::Now, 1),
                    (WantKind::Good(FOOD), Horizon::Next, 1),
                ]),
                Gold(3),
            ),
            market_test_agent(
                2,
                Gold::ZERO,
                stock_with(1, 0, 0),
                wants(&[(WantKind::Good(SALT), Horizon::Later(1), 1)]),
                Gold(1),
            ),
        ],
        recipes: builtin_recipes(),
        events: Vec::new(),
        money: MarketMoneyConfig::Designated(DesignatedMoney { good: SALT }),
    };
    let mut society = Society::from_scenario(scenario);

    society.step();

    assert!(society.books.iter().all(|book| book.good != SALT));
    assert_eq!(society.trades.len(), 1);
    assert_eq!(society.trades[0].good, FOOD);
    assert_eq!(society.trades[0].price, Gold(1));
    assert_eq!(society.agents[0].gold, Gold(1));
    assert!(society.agents[0].scale[0].satisfied);
}

#[test]
fn initial_designated_money_stock_moves_to_money_balance() {
    let mut stock = Stock::new(6);
    stock.add(SALT, 2);
    let scenario = MarketScenario {
        name: "designated-salt-stock",
        scenario: ScenarioName::MarketPriceDiscovery,
        seed: 1,
        periods: 1,
        agents: vec![Agent {
            id: AgentId(1),
            scale: vec![Want {
                kind: WantKind::Good(SALT),
                horizon: Horizon::Now,
                qty: 2,
                satisfied: false,
            }],
            stock,
            gold: Gold::ZERO,
            labor_capacity: 0,
            hunger_deficit: 0,
            roles: vec![Role::Trader],
            expect: Vec::new(),
        }],
        recipes: Vec::new(),
        events: Vec::new(),
        money: MarketMoneyConfig::Designated(DesignatedMoney { good: SALT }),
    };

    let mut society = Society::from_scenario(scenario);

    assert_eq!(society.agents[0].gold, Gold(2));
    assert_eq!(society.agents[0].stock.get(SALT), 0);
    society.step();
    assert!(society.agents[0].scale[0].satisfied);
}

#[test]
fn direct_recipes_produce_designated_money_good_as_balance() {
    let scenario = MarketScenario {
        name: "food-money-no-stock-production",
        scenario: ScenarioName::MarketPriceDiscovery,
        seed: 1,
        periods: 1,
        agents: vec![Agent {
            id: AgentId(1),
            scale: vec![Want {
                kind: WantKind::Good(FOOD),
                horizon: Horizon::Now,
                qty: 1,
                satisfied: false,
            }],
            stock: Stock::new(3),
            gold: Gold::ZERO,
            labor_capacity: 1,
            hunger_deficit: 0,
            roles: vec![Role::Household],
            expect: Vec::new(),
        }],
        recipes: builtin_recipes(),
        events: Vec::new(),
        money: MarketMoneyConfig::Designated(DesignatedMoney { good: FOOD }),
    };
    let mut society = Society::from_scenario(scenario);

    society.step();

    assert_eq!(society.agents[0].gold, Gold(2));
    assert_eq!(society.agents[0].stock.get(FOOD), 0);
    assert!(society.agents[0].scale[0].satisfied);
}

#[test]
fn direct_recipes_can_produce_designated_money_for_later_wants() {
    let scenario = MarketScenario {
        name: "food-money-later-production",
        scenario: ScenarioName::MarketPriceDiscovery,
        seed: 1,
        periods: 1,
        agents: vec![Agent {
            id: AgentId(1),
            scale: vec![Want {
                kind: WantKind::Good(FOOD),
                horizon: Horizon::Later(1),
                qty: 1,
                satisfied: false,
            }],
            stock: Stock::new(3),
            gold: Gold::ZERO,
            labor_capacity: 1,
            hunger_deficit: 0,
            roles: vec![Role::Household],
            expect: Vec::new(),
        }],
        recipes: builtin_recipes(),
        events: Vec::new(),
        money: MarketMoneyConfig::Designated(DesignatedMoney { good: FOOD }),
    };
    let mut society = Society::from_scenario(scenario);

    society.step();

    assert_eq!(society.agents[0].gold, Gold(2));
    assert_eq!(society.agents[0].stock.get(FOOD), 0);
    assert!(society.agents[0].scale[0].satisfied);
}

#[test]
fn stock_only_custom_goods_get_market_books() {
    let custom = GoodId(9);
    let mut stock = Stock::new(custom.0);
    stock.add(custom, 1);
    let scenario = MarketScenario {
        name: "stock-only-custom-good",
        scenario: ScenarioName::MarketPriceDiscovery,
        seed: 1,
        periods: 1,
        agents: vec![market_test_agent(
            1,
            Gold::ZERO,
            stock,
            wants(&[(WantKind::Good(GOLD), Horizon::Later(1), 1)]),
            Gold(1),
        )],
        recipes: Vec::new(),
        events: Vec::new(),
        money: MarketMoneyConfig::Designated(DesignatedMoney { good: GOLD }),
    };
    let society = Society::from_scenario(scenario);

    assert!(society.books.iter().any(|book| book.good == custom));
}

#[test]
fn market_runs_are_deterministic() {
    let a = run(ScenarioName::MarketPriceDiscovery);
    let b = run(ScenarioName::MarketPriceDiscovery);

    assert_eq!(a.records, b.records);
    assert_eq!(a.trades, b.trades);
}

#[test]
fn golden_m1_market_series_is_stable() {
    let society = run(ScenarioName::MarketPriceDiscovery);

    assert_eq!(
        fnv1a_market(&society.records, &society.trades),
        8_534_293_163_604_412_536
    );
}

#[test]
fn price_discovery_clears_at_least_five_trades() {
    let society = run(ScenarioName::MarketPriceDiscovery);

    assert!(
        society.trades.len() >= 5,
        "price discovery should clear at least 5 trades, got {}",
        society.trades.len()
    );
    assert!(society.trades.iter().all(|t| t.good == FOOD));
}

fn total_stock(agents: &[Agent], good: GoodId) -> u32 {
    agents.iter().map(|agent| agent.stock.get(good)).sum()
}

fn live_orders(society: &Society) -> (u32, u32) {
    society
        .books
        .iter()
        .map(|book| book.live_order_counts())
        .fold((0, 0), |(bid_total, ask_total), (bids, asks)| {
            (bid_total + bids, ask_total + asks)
        })
}

fn agent(id: u32, gold: Gold, food: u32) -> Agent {
    let mut stock = Stock::new(3);
    stock.add(FOOD, food);
    Agent {
        id: AgentId(id),
        scale: vec![Want {
            kind: WantKind::Good(FOOD),
            horizon: Horizon::Next,
            qty: 1,
            satisfied: false,
        }],
        stock,
        gold,
        labor_capacity: 0,
        hunger_deficit: 0,
        roles: vec![Role::Trader],
        expect: Vec::new(),
    }
}

fn stock_with(food: u32, wood: u32, nets: u32) -> Stock {
    let mut stock = Stock::new(3);
    stock.add(FOOD, food);
    stock.add(WOOD, wood);
    stock.add(NET, nets);
    stock
}

fn wants(entries: &[(WantKind, Horizon, usize)]) -> Vec<Want> {
    let mut scale = Vec::new();
    for (kind, horizon, count) in entries {
        for _ in 0..*count {
            scale.push(Want {
                kind: *kind,
                horizon: *horizon,
                qty: 1,
                satisfied: false,
            });
        }
    }
    scale
}

fn market_test_agent(
    id: u32,
    gold: Gold,
    stock: Stock,
    scale: Vec<Want>,
    food_expected: Gold,
) -> Agent {
    let mut expect = vec![PriceBelief::new(Gold::ZERO, Gold(1)); 4];
    expect[usize::from(FOOD.0)] = PriceBelief::new(food_expected, Gold(1));
    Agent {
        id: AgentId(id),
        scale,
        stock,
        gold,
        labor_capacity: 0,
        hunger_deficit: 0,
        roles: vec![Role::Trader],
        expect,
    }
}

fn order(agent: u32, side: OrderSide, limit: Gold, qty: u32, seq: u64) -> Order {
    Order {
        agent: AgentId(agent),
        side,
        good: FOOD,
        limit,
        qty,
        seq,
        expires_tick: 3,
    }
}

fn fnv1a_market(records: &[MarketRecord], trades: &[econ::market::Trade]) -> u64 {
    let mut hash = 0xcbf2_9ce4_8422_2325;
    for record in records {
        hash_u64(&mut hash, record.tick);
        hash_u64(&mut hash, record.total_gold.0);
        hash_u32(&mut hash, record.trades);
        hash_u32(&mut hash, record.food_volume);
        hash_u32(&mut hash, record.wood_volume);
        hash_u32(&mut hash, record.net_volume);
        hash_option_gold(&mut hash, record.last_food_price);
        hash_option_gold(&mut hash, record.last_wood_price);
        hash_option_gold(&mut hash, record.last_net_price);
        hash_u32(&mut hash, record.bid_count);
        hash_u32(&mut hash, record.ask_count);
        hash_u32(&mut hash, record.expired_orders);
    }
    for trade in trades {
        hash_u64(&mut hash, trade.tick);
        hash_u16(&mut hash, trade.good.0);
        hash_u32(&mut hash, trade.buyer.0);
        hash_u32(&mut hash, trade.seller.0);
        hash_u64(&mut hash, trade.price.0);
        hash_u32(&mut hash, trade.qty);
    }
    hash
}

fn hash_option_gold(hash: &mut u64, value: Option<Gold>) {
    match value {
        Some(gold) => {
            hash_u8(hash, 1);
            hash_u64(hash, gold.0);
        }
        None => hash_u8(hash, 0),
    }
}

fn hash_u8(hash: &mut u64, value: u8) {
    *hash ^= u64::from(value);
    *hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
}

fn hash_u16(hash: &mut u64, value: u16) {
    for byte in value.to_le_bytes() {
        hash_u8(hash, byte);
    }
}

fn hash_u32(hash: &mut u64, value: u32) {
    for byte in value.to_le_bytes() {
        hash_u8(hash, byte);
    }
}

fn hash_u64(hash: &mut u64, value: u64) {
    for byte in value.to_le_bytes() {
        hash_u8(hash, byte);
    }
}
