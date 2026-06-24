use econ::agent::{Agent, AgentId, Role, Want, WantKind};
use econ::barter::BarterTrade;
use econ::expect::PriceBelief;
use econ::good::{Gold, GoodId, Horizon, Stock, CLOTH, FOOD, ORE, SALT, WOOD};
use econ::marketability::{GoodMarketability, MarketabilityConfig};
use econ::menger::SaleabilitySnapshot;
use econ::money::MarketMoneyConfig;
use econ::record::V2Record;
use econ::scenario::{builtin_market_scenario, MarketScenario, ScenarioName};
use econ::society::Society;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct CandidateCounts {
    acceptances: u64,
    direct_acceptances: u64,
    indirect_acceptances: u64,
}

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

fn marketability_scenario(durability_aware: bool, salt_bad: bool) -> MarketScenario {
    let mut scenario = builtin_market_scenario(ScenarioName::MengerMarketabilityDurability);
    let MarketMoneyConfig::Emergent(config) = &mut scenario.money else {
        panic!("marketability scenario must use emergent money");
    };
    config.durability_aware_acceptance = durability_aware;
    if salt_bad {
        config.marketability.goods.insert(
            SALT,
            GoodMarketability {
                decay_bps: 10_000,
                carry_cost: 1,
            },
        );
    }
    scenario
}

fn run_marketability(durability_aware: bool, salt_bad: bool) -> Society {
    let scenario = marketability_scenario(durability_aware, salt_bad);
    let periods = scenario.periods;
    let mut society = Society::from_scenario(scenario);
    society.run(periods);
    society
}

fn candidate_counts(society: &Society, good: GoodId) -> CandidateCounts {
    let candidate = society
        .emergence()
        .expect("emergent state")
        .tracker()
        .candidate_saleability()
        .find(|candidate| candidate.good == good)
        .expect("candidate good is tracked");
    CandidateCounts {
        acceptances: candidate.acceptances,
        direct_acceptances: candidate
            .acceptances
            .saturating_sub(candidate.indirect_acceptances),
        indirect_acceptances: candidate.indirect_acceptances,
    }
}

fn saleability_leader(society: &Society) -> GoodId {
    society
        .emergence()
        .expect("emergent state")
        .tracker()
        .leader_shares()
        .expect("the controlled scenario has a leader")
        .good
}

fn run_trace(
    scenario: MarketScenario,
) -> (Vec<V2Record>, Vec<BarterTrade>, Vec<SaleabilitySnapshot>) {
    let periods = scenario.periods;
    let mut society = Society::from_scenario(scenario);
    society.run(periods);
    (
        society.v2_records,
        society.barter_trades,
        society.saleability_snapshots,
    )
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

#[test]
fn durability_aware_run_is_deterministic() {
    let first = run_trace(marketability_scenario(true, false));
    let second = run_trace(marketability_scenario(true, false));

    assert_eq!(first, second);
}

#[test]
fn lever_off_reproduces_necessity_dominance() {
    let off = run_marketability(false, false);

    assert_eq!(saleability_leader(&off), FOOD);
    assert_ne!(saleability_leader(&off), SALT);
    assert!(
        candidate_counts(&off, FOOD).acceptances > candidate_counts(&off, SALT).acceptances,
        "with the lever off, the wanted necessity should lead the acceptance-share race"
    );
}

#[test]
fn the_necessity_still_trades_directly_and_the_drop_is_indirect() {
    let off = run_marketability(false, false);
    let on = run_marketability(true, false);
    let off_food = candidate_counts(&off, FOOD);
    let on_food = candidate_counts(&on, FOOD);

    assert!(
        on_food.direct_acceptances >= 4,
        "the necessity must retain material direct acceptance volume: {on_food:?}"
    );
    assert_eq!(
        off_food.direct_acceptances, on_food.direct_acceptances,
        "the durability lever must not remove direct necessity demand"
    );
    assert!(
        off_food.indirect_acceptances > on_food.indirect_acceptances,
        "the demotion must be a drop in indirect-as-means FOOD acceptance: off={off_food:?} on={on_food:?}"
    );
}

#[test]
fn marketability_finding() {
    let off = run_marketability(false, false);
    let on = run_marketability(true, false);
    let off_food = candidate_counts(&off, FOOD);
    let on_food = candidate_counts(&on, FOOD);
    let on_salt = candidate_counts(&on, SALT);

    assert_eq!(saleability_leader(&off), FOOD);
    assert_eq!(
        saleability_leader(&on),
        FOOD,
        "marketability finding: declining perishable FOOD as a medium drops indirect FOOD volume, \
         but direct necessity volume still dominates pure acceptance-share saleability"
    );
    assert!(off_food.indirect_acceptances > on_food.indirect_acceptances);
    assert!(
        on_food.direct_acceptances > on_salt.acceptances,
        "the direct necessity volume remains larger than all SALT acceptances: FOOD={on_food:?} SALT={on_salt:?}"
    );
}

#[test]
fn it_is_marketability_not_taste() {
    let scenario = marketability_scenario(true, false);
    let food_wanters = scenario
        .agents
        .iter()
        .filter(|agent| {
            agent
                .scale
                .iter()
                .any(|want| want.kind == WantKind::Good(FOOD))
        })
        .count();
    let salt_wanters = scenario
        .agents
        .iter()
        .filter(|agent| {
            agent
                .scale
                .iter()
                .any(|want| want.kind == WantKind::Good(SALT))
        })
        .count();
    let salt_bad = run_marketability(true, true);

    assert!(
        salt_wanters < food_wanters,
        "SALT must not be configured as a universal medium want"
    );
    assert_eq!(
        saleability_leader(&salt_bad),
        FOOD,
        "making SALT physically bad must not reveal a hidden SALT preference"
    );
}

#[test]
fn goldens_unchanged() {
    for name in [ScenarioName::MengerSaltMoney, ScenarioName::MengerGoldMoney] {
        let base = builtin_market_scenario(name);
        let mut explicit_empty = base.clone();
        let MarketMoneyConfig::Emergent(config) = &mut explicit_empty.money else {
            panic!("Mengerian scenario must use emergent money");
        };
        config.durability_aware_acceptance = false;
        config.marketability = MarketabilityConfig::default();

        assert_eq!(
            run_trace(base),
            run_trace(explicit_empty),
            "an explicit empty/off marketability config must be byte-identical for {name:?}"
        );
    }
}
