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
    let mut cfg = SettlementConfig::frontier_cycle();
    cfg.barter
        .as_mut()
        .expect("barter overlay")
        .menger
        .multi_offer_medium = true;
    cfg
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
fn two_lane_cycle_round_trips_bilaterally() {
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
