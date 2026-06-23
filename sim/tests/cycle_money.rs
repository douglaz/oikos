//! S19 — imperfect-double-coincidence money in a produced 3-good cycle.
//!
//! The cycle is an artificial closed input loop, not a terminal-consumer economy:
//! role A consumes Z and produces X, B consumes X and produces Y, C consumes Y and
//! produces Z. The produced goods are wanted only as producer inputs through
//! `Horizon::Next`; there is no consumption taste for X/Y/Z. A survival hearth keeps
//! FOOD/WOOD off-market so the exchange topology is isolated.

use std::collections::BTreeSet;

use econ::agent::{AgentId, Want, WantKind};
use econ::barter::BarterReason;
use econ::good::{GoodId, Horizon, FOOD, SALT, WOOD};
use econ::money::MengerianConfig;
use sim::{
    BarterConfig, ChainConfig, DirectIndirectAcceptances, Settlement, SettlementConfig, Vocation,
};

const S19_1_TICKS: u64 = 10;
const S19_2_TICKS: u64 = 600;
const SWEEP_PERIODS: [u16; 5] = [8, 6, 4, 3, 2];
const SWEEP_SEEDS: [u64; 3] = [1, 7, 19];

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
    chain.producer_subsistence = 1;
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
        cycle_producer_medium_endowment: 0,
        salt_direct_use_qty: 0,
        salt_direct_use_period: 0,
    });
    cfg
}

fn cycle_money_config(period: u16) -> SettlementConfig {
    let mut cfg = s19_1_cycle_config();
    let goods = cycle_goods(&cfg);
    let barter = cfg.barter.as_mut().expect("barter overlay");
    barter.cycle_producer_medium_endowment = 12;
    barter.salt_direct_use_qty = 1;
    barter.salt_direct_use_period = period;
    barter.menger = MengerianConfig {
        candidate_goods: vec![goods.x, goods.y, goods.z, SALT],
        min_indirect_acceptances: 12,
        min_indirect_acceptor_agents: 3,
        min_indirect_target_goods: 3,
        ..MengerianConfig::default()
    };
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

fn acceptance_split(s: &Settlement, good: GoodId) -> DirectIndirectAcceptances {
    s.direct_indirect_acceptances()
        .into_iter()
        .find(|c| c.good == good)
        .unwrap_or(DirectIndirectAcceptances {
            good,
            total: 0,
            direct: 0,
            indirect: 0,
        })
}

fn contains_all_targets(targets: &[GoodId], goods: CycleGoods) -> bool {
    targets.contains(&goods.x) && targets.contains(&goods.y) && targets.contains(&goods.z)
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum DensityClass {
    LeadsAndPromotes,
    LeadsButNoBreadth,
    NeverLeads,
}

#[derive(Clone, Debug)]
struct DensityCell {
    period: u16,
    seed: u64,
    class: DensityClass,
    salt_led: bool,
    promoted: Option<GoodId>,
    salt: DirectIndirectAcceptances,
    targets: Vec<GoodId>,
    round_trip: (u64, u64),
}

fn classify_density_cell(period: u16, seed: u64, ticks: u64) -> DensityCell {
    let cfg = cycle_money_config(period);
    let goods = cycle_goods(&cfg);
    let mut s = Settlement::generate(seed, &cfg);
    let mut salt_led = false;
    for tick in 0..ticks {
        let was_pre_promotion = s.current_money_good().is_none();
        let report = s.econ_tick();
        assert!(report.conserves(), "conservation broke at tick {tick}");
        if was_pre_promotion && s.saleability_leader() == Some(SALT) {
            salt_led = true;
        }
    }

    let promoted = s.current_money_good();
    let targets = s.indirect_target_goods(SALT);
    let class = if salt_led && promoted == Some(SALT) && contains_all_targets(&targets, goods) {
        DensityClass::LeadsAndPromotes
    } else if salt_led {
        DensityClass::LeadsButNoBreadth
    } else {
        DensityClass::NeverLeads
    };

    DensityCell {
        period,
        seed,
        class,
        salt_led,
        promoted,
        salt: acceptance_split(&s, SALT),
        targets,
        round_trip: s.salt_round_trip(),
    }
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

#[test]
fn salt_seed_is_neutral_and_on_cycle_producers() {
    let cfg = cycle_money_config(4);
    let barter = cfg.barter.as_ref().expect("barter overlay");
    assert_eq!(barter.medium_good, SALT);
    assert_eq!(barter.medium_want_qty, 0);
    assert_eq!(barter.gatherer_medium_endowment, 0);
    assert_eq!(barter.consumer_medium_endowment, 0);
    assert!(barter.cycle_producer_medium_endowment > 0);

    let s = Settlement::generate(1, &cfg);
    assert_eq!(s.current_money_good(), None);
    for i in 0..s.population() {
        match s.vocation_of(i) {
            Some(Vocation::CycleA | Vocation::CycleB | Vocation::CycleC) => assert_eq!(
                s.stock_of(i, SALT),
                u64::from(barter.cycle_producer_medium_endowment),
                "cycle producers must hold the neutral SALT commodity seed"
            ),
            _ => assert_eq!(s.stock_of(i, SALT), 0),
        }
    }
}

#[test]
fn direct_indirect_saleability_split_is_derived() {
    let cfg = cycle_money_config(4);
    let goods = cycle_goods(&cfg);
    let s = run_with_conservation(1, &cfg, S19_2_TICKS);

    let raw = s.emergence_acceptances();
    let derived = s.direct_indirect_acceptances();
    assert_eq!(raw.len(), derived.len());
    for candidate in raw {
        let split = derived
            .iter()
            .find(|split| split.good == candidate.good)
            .expect("derived split for candidate");
        assert_eq!(split.total, candidate.acceptances);
        assert_eq!(
            split.direct,
            candidate
                .acceptances
                .saturating_sub(candidate.indirect_acceptances)
        );
        assert_eq!(split.indirect, candidate.indirect_acceptances);
    }

    let salt = acceptance_split(&s, SALT);
    let x = acceptance_split(&s, goods.x);
    let y = acceptance_split(&s, goods.y);
    let z = acceptance_split(&s, goods.z);
    assert!(
        salt.total > 0,
        "SALT must accrue saleability from the producer seed plus direct-use anchor; \
         trades={}, live_offers={:?}, raw={:?}, salt_stock={}",
        s.society().barter_trades.len(),
        s.society().live_barter_offers(),
        s.emergence_acceptances(),
        (0..s.population())
            .map(|i| s.stock_of(i, SALT))
            .sum::<u64>()
    );

    if s.saleability_leader() == Some(SALT) {
        let salt_bps = s.saleability_bps(SALT).expect("SALT saleability");
        for good in [goods.x, goods.y, goods.z] {
            assert!(
                salt_bps >= s.saleability_bps(good).unwrap_or(0),
                "the reported leader must agree with by-good saleability"
            );
        }
    }

    assert_eq!(
        cycle_barter_trade_count(&s, goods),
        0,
        "cycle goods still must not clear directly against each other"
    );
    assert!(
        salt.direct >= x.direct.min(y.direct).min(z.direct),
        "SALT direct acceptances should be material relative to the broken cycle goods: \
         salt={salt:?}, x={x:?}, y={y:?}, z={z:?}"
    );
}

#[test]
fn cycle_money_finding_no_indirect_round_trip_forms() {
    let cfg = cycle_money_config(4);
    let s = run_with_conservation(7, &cfg, S19_2_TICKS);

    assert_eq!(s.saleability_leader(), Some(SALT));
    assert_eq!(s.current_money_good(), None);
    assert!(
        s.society()
            .live_barter_offers()
            .iter()
            .any(|offer| matches!(offer.reason, BarterReason::IndirectFor { .. })),
        "the book reaches indirect SALT offers before the deadlock"
    );
    assert_eq!(
        s.indirect_target_goods(SALT),
        Vec::<GoodId>::new(),
        "no indirect SALT trade clears, so the strong-bar breadth stays empty"
    );
    assert_eq!(
        s.salt_round_trip(),
        (0, 0),
        "with no accepted IndirectFor SALT, the round-trip ledger correctly stays empty"
    );
}

#[test]
fn anchor_density_sweep_classifies_the_outcome() {
    let mut cells = Vec::new();
    for period in SWEEP_PERIODS {
        for seed in SWEEP_SEEDS {
            cells.push(classify_density_cell(period, seed, S19_2_TICKS));
        }
    }
    for seed in SWEEP_SEEDS {
        cells.push(classify_density_cell(1, seed, S19_2_TICKS));
    }

    assert_eq!(cells.len(), (SWEEP_PERIODS.len() + 1) * SWEEP_SEEDS.len());
    assert!(
        cells
            .iter()
            .all(|cell| cell.salt.total >= cell.salt.indirect),
        "each density cell must report a sane direct/indirect split: {cells:?}"
    );
    assert!(
        cells
            .iter()
            .all(|cell| cell.promoted.is_none() || cell.salt_led),
        "promotion should not occur without a prior SALT lead in this sweep: {cells:?}"
    );

    let universal: Vec<_> = cells.iter().filter(|cell| cell.period == 1).collect();
    assert_eq!(universal.len(), SWEEP_SEEDS.len());
    assert!(
        universal
            .iter()
            .all(|cell| cell.class != DensityClass::LeadsAndPromotes
                && cell.targets.len() < 3
                && cell.round_trip.1 == 0),
        "the universal direct-use control must suppress indirect breadth: {universal:?}"
    );

    for period in SWEEP_PERIODS {
        for seed in SWEEP_SEEDS {
            assert!(
                cells
                    .iter()
                    .any(|cell| cell.period == period && cell.seed == seed),
                "missing density cell for period {period}, seed {seed}"
            );
        }
    }
}
