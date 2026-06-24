use econ::agent::{AgentId, WantKind};
use econ::barter::{BarterReason, BarterTrade};
use econ::good::{GoodId, Horizon, CLOTH, FOOD, ORE, SALT, WOOD};
use econ::menger::{MengerianEmergence, SaleabilitySnapshot};
use econ::money::{MarketMoneyConfig, MengerianConfig};
use econ::record::V2Record;
use econ::scenario::{
    builtin_market_scenario, scale, v2_agent, v2_stock, MarketScenario, ScenarioName,
};
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

/// A controlled candidate-mode scenario built to exercise the S21c path-dependence:
/// FOOD and WOOD cross the direct-use floor at tick 1 (so candidate mode is active from
/// tick 2), while a below-floor good (ORE) needs a SECOND distinct direct acceptor that
/// can only act in candidate mode. ORE seller O1 holds 2 ORE and trades one per tick;
/// A1 acquires ORE at tick 1 (pre-candidate), A2 must acquire it later — and A2 holds and
/// wants only below-floor goods (CLOTH/ORE), so without the legacy direct-discovery lane
/// it posts no offer once candidates exist and ORE stalls at one acceptor.
fn late_crosser_scenario() -> MarketScenario {
    let mut scenario = builtin_market_scenario(ScenarioName::MengerTwoLayerSaleability);
    scenario.periods = 6;
    scenario.agents = vec![
        // FOOD/WOOD makers — both cross the floor at tick 1 (2 distinct acceptors each).
        v2_agent(
            1,
            v2_stock(0, 1, 0, 0, 0),
            scale(&[(WantKind::Good(FOOD), Horizon::Now, 1)]),
        ),
        v2_agent(
            2,
            v2_stock(0, 1, 0, 0, 0),
            scale(&[(WantKind::Good(FOOD), Horizon::Now, 1)]),
        ),
        v2_agent(
            3,
            v2_stock(1, 0, 0, 0, 0),
            scale(&[(WantKind::Good(WOOD), Horizon::Now, 1)]),
        ),
        v2_agent(
            4,
            v2_stock(1, 0, 0, 0, 0),
            scale(&[(WantKind::Good(WOOD), Horizon::Now, 1)]),
        ),
        // Below-floor ORE/CLOTH agents: O1 sells ORE one-per-tick; A1, A2 each want ORE.
        v2_agent(
            5,
            v2_stock(0, 0, 0, 0, 2),
            scale(&[(WantKind::Good(CLOTH), Horizon::Now, 1)]),
        ),
        v2_agent(
            6,
            v2_stock(0, 0, 0, 1, 0),
            scale(&[(WantKind::Good(ORE), Horizon::Now, 1)]),
        ),
        v2_agent(
            7,
            v2_stock(0, 0, 0, 1, 0),
            scale(&[(WantKind::Good(ORE), Horizon::Now, 1)]),
        ),
    ];
    let MarketMoneyConfig::Emergent(config) = &mut scenario.money else {
        panic!("two-layer scenario must use emergent money");
    };
    config.candidate_goods = vec![FOOD, WOOD, SALT, CLOTH, ORE];
    scenario
}

#[test]
fn late_floor_crosser_is_not_starved() {
    let society = run(late_crosser_scenario());
    let ore = candidate_counts(&society, ORE);
    let config = society.emergence().expect("emergent state").config();

    // FOOD crossed first and became a candidate, activating candidate mode.
    assert!(
        candidate_counts(&society, FOOD).direct_acceptors
            >= usize::from(config.min_direct_use_acceptors),
        "FOOD should cross the direct-use floor first"
    );
    // The below-floor good still reaches the floor LATE via the legacy direct-discovery
    // lane. Without S21c, candidate mode stops posting direct offers for below-floor goods
    // once a candidate exists, so ORE would stall at its single pre-candidate acceptor.
    assert!(
        ore.direct_acceptors >= usize::from(config.min_direct_use_acceptors),
        "the late floor-crosser must not be starved of direct discovery: ORE={ore:?}"
    );
    assert!(
        society
            .emergence()
            .expect("emergent state")
            .provisional_media_candidates()
            .contains(&ORE),
        "ORE should join the candidate set after crossing the direct-use floor late"
    );
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
