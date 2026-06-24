use econ::agent::AgentId;
use econ::barter::{BarterReason, BarterTrade};
use econ::good::{GoodId, CLOTH, FOOD, ORE, SALT, WOOD};
use econ::menger::{MengerianEmergence, SaleabilitySnapshot};
use econ::money::{MarketMoneyConfig, MengerianConfig};
use econ::record::V2Record;
use econ::scenario::{builtin_market_scenario, MarketScenario, ScenarioName};
use econ::society::Society;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct CandidateCounts {
    acceptances: u64,
    direct_acceptances: u64,
    direct_acceptors: usize,
    indirect_acceptances: u64,
}

fn two_layer_scenario(enabled: bool) -> MarketScenario {
    let mut scenario = builtin_market_scenario(ScenarioName::MengerTwoLayerSaleability);
    let MarketMoneyConfig::Emergent(config) = &mut scenario.money else {
        panic!("two-layer scenario must use emergent money");
    };
    config.two_layer_saleability = enabled;
    if !enabled {
        config.min_direct_use_acceptors = 0;
    }
    scenario
}

fn run(scenario: MarketScenario) -> Society {
    let periods = scenario.periods;
    let mut society = Society::from_scenario(scenario);
    society.run(periods);
    society
}

fn run_trace(
    scenario: MarketScenario,
) -> (Vec<V2Record>, Vec<BarterTrade>, Vec<SaleabilitySnapshot>) {
    let society = run(scenario);
    (
        society.v2_records,
        society.barter_trades,
        society.saleability_snapshots,
    )
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
        direct_acceptances: candidate.direct_acceptances,
        direct_acceptors: candidate.direct_acceptor_agents.len(),
        indirect_acceptances: candidate.indirect_acceptances,
    }
}

fn total_saleability_leader(society: &Society) -> GoodId {
    society
        .emergence()
        .expect("emergent state")
        .tracker()
        .leader_shares()
        .expect("the controlled scenario has a total leader")
        .good
}

fn medium_saleability_leader(society: &Society) -> GoodId {
    society
        .emergence()
        .expect("emergent state")
        .medium_leader_shares()
        .expect("the controlled scenario has a medium leader")
        .good
}

#[test]
fn two_layer_off_reproduces_the_finding() {
    let off = run(two_layer_scenario(false));

    assert_eq!(total_saleability_leader(&off), FOOD);
    assert_ne!(total_saleability_leader(&off), SALT);
    assert!(
        candidate_counts(&off, FOOD).direct_acceptances > candidate_counts(&off, SALT).acceptances,
        "the universal necessity should still dominate total/direct acceptance volume"
    );
}

#[test]
fn two_layer_promotes_the_medium() {
    let on = run(two_layer_scenario(true));
    let food = candidate_counts(&on, FOOD);
    let salt = candidate_counts(&on, SALT);

    assert_eq!(total_saleability_leader(&on), FOOD);
    assert_eq!(medium_saleability_leader(&on), SALT);
    assert_eq!(
        food.indirect_acceptances, 0,
        "perishable FOOD should not accrue accepted IndirectFor trades"
    );
    assert!(
        salt.indirect_acceptances > 0,
        "durable SALT should accrue accepted IndirectFor trades"
    );
}

#[test]
fn the_commodity_promotes_to_money() {
    let on = run(two_layer_scenario(true));

    assert_eq!(on.current_money_good(), Some(SALT));
}

#[test]
fn discovery_was_open_not_preselected() {
    let on = run(two_layer_scenario(true));
    let config = on.emergence().expect("emergent state").config();
    let food = candidate_counts(&on, FOOD);
    let salt = candidate_counts(&on, SALT);

    assert!(config.two_layer_saleability);
    assert!(food.direct_acceptors >= usize::from(config.min_direct_use_acceptors));
    assert!(salt.direct_acceptors >= usize::from(config.min_direct_use_acceptors));
    assert_eq!(
        food.indirect_acceptances, 0,
        "the bad direct-use candidate was eligible to be considered but rejected as a medium"
    );
    assert!(
        salt.indirect_acceptances > 0,
        "the durable direct-use candidate won by accepted medium trades"
    );
}

#[test]
fn eligibility_floor_is_non_circular() {
    let config = MengerianConfig {
        candidate_goods: vec![SALT, FOOD],
        min_total_acceptances: 1,
        promotion_threshold_bps: 1,
        lead_margin_bps: 1,
        min_acceptor_agents: 0,
        min_counterpart_goods: 0,
        stability_ticks: 1,
        indirect_min_acceptance_share_bps: 1,
        min_indirect_acceptances: 1,
        min_indirect_acceptor_agents: 1,
        min_indirect_target_goods: 1,
        allow_indirect_acceptance: true,
        multi_offer_medium: true,
        durability_aware_acceptance: false,
        two_layer_saleability: true,
        min_direct_use_acceptors: 2,
        marketability: Default::default(),
    };
    let mut emergence = MengerianEmergence::new(config);

    emergence.observe_trade(&indirect_trade(1, 2, WOOD, SALT, CLOTH));
    emergence.observe_trade(&indirect_trade(3, 4, ORE, SALT, WOOD));

    assert_eq!(emergence.medium_leader_shares().expect("leader").good, SALT);
    assert_eq!(emergence.end_tick(1), None);
    assert_eq!(emergence.current_money_good(), None);

    emergence.observe_trade(&direct_trade(5, 6, WOOD, SALT));
    assert_eq!(
        emergence.end_tick(2),
        None,
        "one direct acceptor is still below the direct-use floor"
    );

    emergence.observe_trade(&direct_trade(7, 8, ORE, SALT));
    assert_eq!(emergence.end_tick(3), Some(SALT));
}

#[test]
fn lever_off_is_byte_identical() {
    let off = two_layer_scenario(false);
    let mut explicit_off = off.clone();
    let MarketMoneyConfig::Emergent(config) = &mut explicit_off.money else {
        panic!("two-layer scenario must use emergent money");
    };
    config.two_layer_saleability = false;
    config.min_direct_use_acceptors = 99;

    assert_eq!(
        run_trace(off),
        run_trace(explicit_off),
        "direct-use floor changes must be inert while the two-layer flag is off"
    );
}

#[test]
fn two_layer_run_is_deterministic() {
    let first = run_trace(two_layer_scenario(true));
    let second = run_trace(two_layer_scenario(true));

    assert_eq!(first, second);
}

fn direct_trade(a: u32, b: u32, a_gives: GoodId, b_gives: GoodId) -> BarterTrade {
    BarterTrade {
        tick: 0,
        a: AgentId(u64::from(a)),
        b: AgentId(u64::from(b)),
        a_gives,
        b_gives,
        qty: 1,
        a_reason: BarterReason::DirectWant,
        b_reason: BarterReason::DirectWant,
    }
}

fn indirect_trade(
    a: u32,
    b: u32,
    a_gives: GoodId,
    b_gives: GoodId,
    a_target: GoodId,
) -> BarterTrade {
    BarterTrade {
        tick: 0,
        a: AgentId(u64::from(a)),
        b: AgentId(u64::from(b)),
        a_gives,
        b_gives,
        qty: 1,
        a_reason: BarterReason::IndirectFor { target: a_target },
        b_reason: BarterReason::DirectWant,
    }
}
