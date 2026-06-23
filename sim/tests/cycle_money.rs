//! S19 — imperfect-double-coincidence money in a produced 3-good cycle.
//!
//! The cycle is an artificial closed input loop, not a terminal-consumer economy:
//! role A consumes Z and produces X, B consumes X and produces Y, C consumes Y and
//! produces Z. The produced goods are wanted only as producer inputs through
//! `Horizon::Next`; there is no consumption taste for X/Y/Z. A survival hearth keeps
//! FOOD/WOOD off-market so the exchange topology is isolated.

use std::collections::BTreeSet;

use econ::agent::{AgentId, Want, WantKind};
use econ::good::{GoodId, Horizon, FOOD, SALT, WOOD};
use econ::money::MengerianConfig;
use sim::{BarterConfig, ChainConfig, Settlement, SettlementConfig, Vocation};

const S19_1_TICKS: u64 = 10;

#[derive(Clone, Copy)]
struct CycleGoods {
    x: GoodId,
    y: GoodId,
    z: GoodId,
}

fn cycle_goods(cfg: &SettlementConfig) -> CycleGoods {
    let (x, y, z) = cfg
        .chain
        .as_ref()
        .expect("cycle chain")
        .content
        .cycle_goods()
        .expect("cycle goods");
    CycleGoods { x, y, z }
}

fn s19_1_cycle_config() -> SettlementConfig {
    let mut cfg = SettlementConfig::viable();
    cfg.width = 4;
    cfg.height = 1;
    cfg.nodes.clear();
    cfg.gatherers = 0;
    cfg.consumers = 0;
    cfg.starting_gold_gatherer = 0;
    cfg.starting_gold_consumer = 0;
    cfg.gatherer_food_buffer = 0;
    cfg.gatherer_wood_buffer = 0;
    cfg.consumer_food_buffer = 0;
    cfg.consumer_wood_endowment = 0;
    cfg.demography = None;
    cfg.m3 = false;

    let mut chain = ChainConfig::three_good_cycle();
    chain.project_input_bids = true;
    chain.producer_subsistence = 4;
    cfg.chain = Some(chain);

    let goods = cycle_goods(&cfg);
    cfg.barter = Some(BarterConfig {
        menger: MengerianConfig {
            candidate_goods: vec![goods.x, goods.y, goods.z, SALT],
            ..MengerianConfig::default()
        },
        medium_good: SALT,
        medium_want_qty: 0,
        gatherer_medium_endowment: 0,
        consumer_medium_endowment: 0,
        salt_direct_use_qty: 0,
        salt_direct_use_period: 0,
    });
    cfg
}

fn run_with_conservation(seed: u64, cfg: &SettlementConfig, ticks: u64) -> Settlement {
    let mut s = Settlement::generate(seed, cfg);
    for tick in 0..ticks {
        let report = s.econ_tick();
        assert!(report.conserves(), "conservation broke at tick {tick}");
    }
    s
}

fn agent_for_vocation(s: &Settlement, vocation: Vocation) -> AgentId {
    (0..s.population())
        .find(|&i| s.is_alive(i) && s.vocation_of(i) == Some(vocation))
        .and_then(|i| s.colonist_id(i))
        .expect("live cycle producer")
}

fn agent_wants_next(s: &Settlement, id: AgentId, good: GoodId) -> bool {
    s.society()
        .agents
        .get(id)
        .expect("agent")
        .scale
        .iter()
        .any(|w: &Want| w.kind == WantKind::Good(good) && matches!(w.horizon, Horizon::Next))
}

fn cycle_barter_trade_count(s: &Settlement, goods: CycleGoods) -> usize {
    let cycle = BTreeSet::from([goods.x, goods.y, goods.z]);
    s.society()
        .barter_trades
        .iter()
        .filter(|trade| cycle.contains(&trade.a_gives) && cycle.contains(&trade.b_gives))
        .count()
}

#[test]
fn three_roles_produce_and_derive_input_demand() {
    let cfg = s19_1_cycle_config();
    let goods = cycle_goods(&cfg);
    let mut s = Settlement::generate(1, &cfg);

    let first = s.econ_tick();
    assert!(first.conserves());
    assert_eq!(first.produced_of(goods.x), 3);
    assert_eq!(first.produced_of(goods.y), 3);
    assert_eq!(first.produced_of(goods.z), 3);
    assert_eq!(first.consumed_as_input_of(goods.x), 3);
    assert_eq!(first.consumed_as_input_of(goods.y), 3);
    assert_eq!(first.consumed_as_input_of(goods.z), 3);

    let s = run_with_conservation(1, &cfg, S19_1_TICKS);
    let a = agent_for_vocation(&s, Vocation::CycleA);
    let b = agent_for_vocation(&s, Vocation::CycleB);
    let c = agent_for_vocation(&s, Vocation::CycleC);

    assert!(agent_wants_next(&s, a, goods.z), "A must demand Z as input");
    assert!(agent_wants_next(&s, b, goods.x), "B must demand X as input");
    assert!(agent_wants_next(&s, c, goods.y), "C must demand Y as input");
    assert!(
        s.society().live_barter_offer_count() > 0 || !s.society().barter_trades.is_empty(),
        "the derived input wants must reach the generic barter path before money exists"
    );
    assert!(
        s.current_money_good().is_none(),
        "the S19.1 substrate must stay pre-money"
    );
}

#[test]
fn no_pairwise_double_coincidence() {
    let cfg = s19_1_cycle_config();
    let goods = cycle_goods(&cfg);
    let s = run_with_conservation(2, &cfg, S19_1_TICKS);

    let a = agent_for_vocation(&s, Vocation::CycleA);
    let b = agent_for_vocation(&s, Vocation::CycleB);
    let c = agent_for_vocation(&s, Vocation::CycleC);

    assert!(
        !agent_wants_next(&s, c, goods.x),
        "C holds/produces A's input Z but must not want A's output X"
    );
    assert!(
        !agent_wants_next(&s, a, goods.y),
        "A holds/produces B's input X but must not want B's output Y"
    );
    assert!(
        !agent_wants_next(&s, b, goods.z),
        "B holds/produces C's input Y but must not want C's output Z"
    );
    assert_eq!(
        cycle_barter_trade_count(&s, goods),
        0,
        "X/Y/Z must not clear by direct pairwise coincidence"
    );
}

#[test]
fn survival_stays_off_market() {
    let cfg = s19_1_cycle_config();
    let s = run_with_conservation(3, &cfg, S19_1_TICKS);

    assert_eq!(s.trade_volume_of(FOOD), 0);
    assert_eq!(s.trade_volume_of(WOOD), 0);
    assert_eq!(
        s.society()
            .barter_trades
            .iter()
            .filter(|trade| {
                trade.a_gives == FOOD
                    || trade.b_gives == FOOD
                    || trade.a_gives == WOOD
                    || trade.b_gives == WOOD
            })
            .count(),
        0,
        "survival goods must not enter the barter tape"
    );
}

#[test]
fn cycle_conserves() {
    let cfg = s19_1_cycle_config();
    let goods = cycle_goods(&cfg);
    let mut s = Settlement::generate(4, &cfg);
    for tick in 0..40 {
        let report = s.econ_tick();
        assert!(report.conserves(), "conservation broke at tick {tick}");
        assert_eq!(
            report.produced_of(goods.x) + report.produced_of(goods.y) + report.produced_of(goods.z),
            report.consumed_as_input_of(goods.x)
                + report.consumed_as_input_of(goods.y)
                + report.consumed_as_input_of(goods.z),
            "cycle recipes must transform inputs into outputs one-for-one at tick {tick}"
        );
        assert_eq!(report.produced_of(SALT), 0);
        assert_eq!(report.consumed_as_input_of(SALT), 0);
    }
}
