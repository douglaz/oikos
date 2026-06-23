//! S20 — two-lane bilateral medium clearing in the produced 3-good cycle.

use std::collections::BTreeSet;

use econ::barter::BarterReason;
use econ::good::{GoodId, SALT};
use sim::{DirectIndirectAcceptances, Settlement, SettlementConfig};

const S20_TICKS: u64 = 600;

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

fn two_lane_cycle_config() -> SettlementConfig {
    SettlementConfig::frontier_cycle_cleared()
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

fn run_with_trace(seed: u64, cfg: &SettlementConfig, ticks: u64) -> (Settlement, bool) {
    let mut s = Settlement::generate(seed, cfg);
    let mut salt_led = false;
    for tick in 0..ticks {
        let was_pre_promotion = s.current_money_good().is_none();
        let report = s.econ_tick();
        assert!(report.conserves(), "conservation broke at tick {tick}");
        if was_pre_promotion && s.saleability_leader() == Some(SALT) {
            salt_led = true;
        }
    }
    (s, salt_led)
}

fn pre_promotion_cycle_barter_without_salt(s: &Settlement, goods: CycleGoods) -> usize {
    let cycle = BTreeSet::from([goods.x, goods.y, goods.z]);
    let promotion_tick = s.promoted_at_tick().unwrap_or(u64::MAX);
    s.society()
        .barter_trades
        .iter()
        .filter(|trade| trade.tick <= promotion_tick)
        .filter(|trade| cycle.contains(&trade.a_gives) || cycle.contains(&trade.b_gives))
        .filter(|trade| trade.a_gives != SALT && trade.b_gives != SALT)
        .count()
}

#[test]
fn salt_round_trips_bilaterally() {
    let cfg = two_lane_cycle_config();
    let goods = cycle_goods(&cfg);
    let (s, salt_led) = run_with_trace(7, &cfg, S20_TICKS);
    let (spent, accepted) = s.salt_round_trip();
    let targets = s.indirect_target_goods(SALT);
    let salt = acceptance_split(&s, SALT);

    assert!(salt_led, "SALT must first become the saleability leader");
    assert!(
        accepted > 0 && spent > 0,
        "SALT must be accepted indirectly and later spent on the target: \
         round_trip=(spent {spent}, accepted {accepted}), salt={salt:?}, targets={targets:?}"
    );
    assert!(
        contains_all_targets(&targets, goods),
        "SALT indirect breadth must span all cycle targets, got {targets:?}"
    );
    assert_eq!(
        pre_promotion_cycle_barter_without_salt(&s, goods),
        0,
        "cycle inputs must clear through SALT, not direct X/Y/Z barter"
    );
}

#[test]
fn salt_promotes_from_the_cleared_cycle() {
    let cfg = two_lane_cycle_config();
    let goods = cycle_goods(&cfg);
    let barter = cfg.barter.as_ref().expect("barter overlay");
    let mut s = Settlement::generate(7, &cfg);
    let mut salt_led = false;
    let mut promotion_tick = None;
    let mut post_promotion_production = 0u64;

    for tick in 0..S20_TICKS {
        if s.current_money_good().is_none() && s.saleability_leader() == Some(SALT) {
            salt_led = true;
        }
        let was_pre_promotion = s.current_money_good().is_none();
        let report = s.econ_tick();
        assert!(report.conserves(), "conservation broke at tick {tick}");
        if was_pre_promotion && s.current_money_good().is_none() {
            salt_led |= s.saleability_leader() == Some(SALT);
        }
        if was_pre_promotion && s.current_money_good() == Some(SALT) {
            promotion_tick = Some(tick);
        }
        if s.current_money_good() == Some(SALT) {
            post_promotion_production += report.produced_of(goods.x)
                + report.produced_of(goods.y)
                + report.produced_of(goods.z);
        }
    }

    let targets = s.indirect_target_goods(SALT);
    assert!(salt_led, "SALT must lead before it promotes");
    assert_eq!(promotion_tick, s.promoted_at_tick());
    assert_eq!(s.current_money_good(), Some(SALT));
    assert_eq!(barter.medium_good, SALT);
    assert_eq!(barter.medium_want_qty, 0);
    assert!(
        contains_all_targets(&targets, goods),
        "SALT must bridge all three cycle inputs, got {targets:?}"
    );
    assert!(
        post_promotion_production > 0,
        "cycle production must continue after promotion"
    );
}

#[test]
fn flag_off_preserves_s19_deadlock() {
    let cfg = SettlementConfig::frontier_cycle();
    let (s, salt_led) = run_with_trace(7, &cfg, S20_TICKS);

    assert!(salt_led);
    assert_eq!(s.current_money_good(), None);
    assert_eq!(s.indirect_target_goods(SALT), Vec::<GoodId>::new());
    assert_eq!(s.salt_round_trip(), (0, 0));
}

#[test]
fn two_lane_run_is_deterministic() {
    let cfg = two_lane_cycle_config();
    let mut a = Settlement::generate(0xC0FFEE, &cfg);
    let mut b = Settlement::generate(0xC0FFEE, &cfg);
    a.run(S20_TICKS);
    b.run(S20_TICKS);

    assert_eq!(a.canonical_bytes(), b.canonical_bytes());
    assert_eq!(a.digest(), b.digest());
}

#[test]
fn two_lane_conserves() {
    let cfg = two_lane_cycle_config();
    let goods = cycle_goods(&cfg);
    let mut s = Settlement::generate(7, &cfg);
    let mut any_produced = false;
    let mut any_indirect = false;

    for tick in 0..S20_TICKS {
        let report = s.econ_tick();
        assert!(report.conserves(), "conservation broke at tick {tick}");
        any_produced |=
            report.produced_of(goods.x) + report.produced_of(goods.y) + report.produced_of(goods.z)
                > 0;
        any_indirect |= acceptance_split(&s, SALT).indirect > 0;
        assert_eq!(report.produced_of(SALT), 0);
        assert_eq!(report.consumed_as_input_of(SALT), 0);
    }

    assert!(any_produced, "the cycle must remain materially productive");
    assert!(
        any_indirect,
        "the run must include indirect SALT acceptances"
    );
}

#[test]
fn money_does_the_work_not_a_ring_matcher() {
    let cfg = two_lane_cycle_config();
    let goods = cycle_goods(&cfg);
    let (s, _) = run_with_trace(11, &cfg, S20_TICKS);

    assert_eq!(
        pre_promotion_cycle_barter_without_salt(&s, goods),
        0,
        "pre-promotion cycle-good acquisitions must have SALT on one side"
    );
    assert!(s.society().barter_trades.iter().all(|trade| {
        trade.a != trade.b
            && (matches!(trade.a_reason, BarterReason::DirectWant)
                || matches!(trade.a_reason, BarterReason::IndirectFor { .. }))
            && (matches!(trade.b_reason, BarterReason::DirectWant)
                || matches!(trade.b_reason, BarterReason::IndirectFor { .. }))
    }));
}

#[test]
fn controls_prove_money_is_load_bearing() {
    let run = |cfg: &SettlementConfig| run_with_trace(7, cfg, S20_TICKS).0;

    let flag_off = run(&SettlementConfig::frontier_cycle());
    assert_eq!(flag_off.current_money_good(), None);
    assert_eq!(flag_off.salt_round_trip(), (0, 0));
    assert_eq!(flag_off.indirect_target_goods(SALT), Vec::<GoodId>::new());

    let mut no_seed = two_lane_cycle_config();
    no_seed
        .barter
        .as_mut()
        .expect("barter overlay")
        .cycle_producer_medium_endowment = 0;
    let no_seed = run(&no_seed);
    assert_eq!(no_seed.current_money_good(), None);
    assert_eq!(acceptance_split(&no_seed, SALT).total, 0);
    assert_eq!(no_seed.salt_round_trip(), (0, 0));

    let mut no_indirect = two_lane_cycle_config();
    no_indirect
        .barter
        .as_mut()
        .expect("barter overlay")
        .menger
        .allow_indirect_acceptance = false;
    let no_indirect = run(&no_indirect);
    assert_eq!(no_indirect.current_money_good(), None);
    assert_eq!(acceptance_split(&no_indirect, SALT).indirect, 0);
    assert_eq!(no_indirect.salt_round_trip(), (0, 0));

    let mut no_salt_candidate = two_lane_cycle_config();
    no_salt_candidate
        .barter
        .as_mut()
        .expect("barter overlay")
        .menger
        .candidate_goods
        .retain(|good| *good != SALT);
    let no_salt_candidate = run(&no_salt_candidate);
    assert_ne!(no_salt_candidate.current_money_good(), Some(SALT));
    assert_eq!(
        no_salt_candidate.indirect_target_goods(SALT),
        Vec::<GoodId>::new()
    );
    assert_eq!(no_salt_candidate.salt_round_trip(), (0, 0));
}

#[test]
fn canonical_bytes_include_multi_offer_medium() {
    let off = Settlement::generate(7, &SettlementConfig::frontier_cycle());
    let mut explicit_off_cfg = SettlementConfig::frontier_cycle();
    explicit_off_cfg
        .barter
        .as_mut()
        .expect("barter overlay")
        .menger
        .multi_offer_medium = false;
    let explicit_off = Settlement::generate(7, &explicit_off_cfg);
    let on = Settlement::generate(7, &SettlementConfig::frontier_cycle_cleared());

    assert_eq!(off.canonical_bytes(), explicit_off.canonical_bytes());
    assert_ne!(
        off.canonical_bytes(),
        on.canonical_bytes(),
        "the flag-on scenario must have a distinct canonical identity"
    );
}

#[test]
fn goldens_unchanged() {
    let digest = |cfg: &SettlementConfig, ticks: u64| {
        let mut s = Settlement::generate(1, cfg);
        s.run(ticks);
        s.digest()
    };

    assert_eq!(
        digest(&SettlementConfig::lineages(), 300),
        0x2335e13c809749fc
    );
    assert_eq!(
        digest(&SettlementConfig::lineages(), 800),
        0x3ffd78e50842d934
    );
    assert_eq!(
        digest(&SettlementConfig::frontier(), 300),
        0xcc83bf2669f0980d
    );
    assert_eq!(
        digest(&SettlementConfig::frontier_cultivation(), 300),
        0xd8cfd0b2e9674373
    );

    let mut viable = Settlement::generate(0xC0FFEE, &SettlementConfig::viable());
    viable.run(60);
    assert_eq!(viable.digest(), 0xa174_8567_db1c_4341);
}
