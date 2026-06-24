//! S20 — two-lane bilateral medium clearing in the produced 3-good cycle.

use std::collections::BTreeSet;

use econ::barter::BarterReason;
use econ::good::{GoodId, FOOD, SALT};
use econ::marketability::{GoodMarketability, MarketabilityConfig};
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
    let cycle = BTreeSet::from([goods.x, goods.y, goods.z]);
    let promotion_tick = s.promoted_at_tick().unwrap_or(u64::MAX);
    let mut mediated_pairs = 0usize;
    let mut mediated_targets = BTreeSet::new();

    for trade in s
        .society()
        .barter_trades
        .iter()
        .filter(|trade| trade.tick <= promotion_tick)
    {
        match (trade.a_gives, trade.b_gives, trade.a_reason, trade.b_reason) {
            (SALT, received, BarterReason::DirectWant, BarterReason::IndirectFor { target })
                if cycle.contains(&received) =>
            {
                mediated_pairs += 1;
                mediated_targets.insert(target);
            }
            (given, SALT, BarterReason::IndirectFor { target }, BarterReason::DirectWant)
                if cycle.contains(&given) =>
            {
                mediated_pairs += 1;
                mediated_targets.insert(target);
            }
            _ => {}
        }
    }

    assert_eq!(
        pre_promotion_cycle_barter_without_salt(&s, goods),
        0,
        "pre-promotion cycle-good acquisitions must have SALT on one side"
    );
    assert!(
        mediated_pairs > 0,
        "pre-promotion clearing must include pairwise SALT-mediated barter"
    );
    assert!(
        mediated_targets.contains(&goods.x)
            && mediated_targets.contains(&goods.y)
            && mediated_targets.contains(&goods.z),
        "SALT-mediated pairs must span all cycle targets, got {mediated_targets:?}"
    );
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
fn canonical_bytes_include_durability_aware_marketability() {
    let base = Settlement::generate(7, &SettlementConfig::frontier_cycle());

    let mut explicit_empty_cfg = SettlementConfig::frontier_cycle();
    let empty_menger = &mut explicit_empty_cfg
        .barter
        .as_mut()
        .expect("barter overlay")
        .menger;
    empty_menger.durability_aware_acceptance = false;
    empty_menger.marketability = MarketabilityConfig::default();
    let explicit_empty = Settlement::generate(7, &explicit_empty_cfg);

    let mut active_cfg = SettlementConfig::frontier_cycle();
    let active_menger = &mut active_cfg.barter.as_mut().expect("barter overlay").menger;
    active_menger.durability_aware_acceptance = true;
    active_menger.marketability = MarketabilityConfig {
        hold_horizon: 1,
        ..MarketabilityConfig::default()
    }
    .with_good(
        FOOD,
        GoodMarketability {
            decay_bps: 10_000,
            carry_cost: 0,
        },
    );
    let active = Settlement::generate(7, &active_cfg);

    // A marketability table populated while the lever is OFF is behaviour-inert:
    // the agent gate never reads it, so future behaviour is identical to the
    // empty default and the digest must match (mirroring the `multi_offer_medium`
    // "appended only when ON" identity invariant).
    let mut inert_table_cfg = SettlementConfig::frontier_cycle();
    inert_table_cfg
        .barter
        .as_mut()
        .expect("barter overlay")
        .menger
        .marketability = MarketabilityConfig::default().with_good(
        SALT,
        GoodMarketability {
            decay_bps: 0,
            carry_cost: 1,
        },
    );
    let inert_table = Settlement::generate(7, &inert_table_cfg);

    assert_eq!(base.canonical_bytes(), explicit_empty.canonical_bytes());
    assert_ne!(base.canonical_bytes(), active.canonical_bytes());
    assert_eq!(
        base.canonical_bytes(),
        inert_table.canonical_bytes(),
        "a flag-off marketability table has no future behaviour, so it must not split the digest"
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

// ---------------------------------------------------------------------------
// Robustness appendix (S20-R): the S20 promotion is not a single-seed / single-
// parameter artifact. Each sweep CLASSIFIES every cell honestly; a fragility
// (a knife-edge that only passes at the shipped value) would FAIL these and is
// itself a finding to record, not something to hide.
// ---------------------------------------------------------------------------

const ROBUST_TICKS: u64 = 1_200;

/// Run the cleared two-lane cycle with `mutate` applied to the config; return
/// (salt_led, promoted, promotion_tick).
fn promotes_with(
    seed: u64,
    mutate: impl FnOnce(&mut SettlementConfig),
    ticks: u64,
) -> (bool, bool, Option<u64>) {
    let mut cfg = two_lane_cycle_config();
    mutate(&mut cfg);
    let (s, salt_led) = run_with_trace(seed, &cfg, ticks);
    (
        salt_led,
        s.current_money_good() == Some(SALT),
        s.promoted_at_tick(),
    )
}

#[test]
fn s20_promotes_across_seeds() {
    // Multi-seed promotion: SALT promotes regardless of the RNG seed (not seed-1 luck).
    let seeds = [1u64, 7, 19, 23, 42];
    let mut ticks_seen = Vec::new();
    for seed in seeds {
        let (led, promoted, at) = promotes_with(seed, |_| {}, ROBUST_TICKS);
        assert!(led, "seed {seed}: SALT must lead");
        assert!(
            promoted,
            "seed {seed}: SALT must promote (multi-seed robustness)"
        );
        ticks_seen.push((seed, at));
    }
    // Sanity: every seed recorded a promotion tick within the horizon.
    assert!(
        ticks_seen.iter().all(|(_, at)| at.is_some()),
        "every seed must promote within {ROBUST_TICKS} ticks: {ticks_seen:?}"
    );
}

#[test]
fn s20_promotes_across_seed_sizes() {
    // SALT producer seed sweep: promotion is a BAND, not a knife-edge at the shipped 12.
    // (Below the bootstrap minimum the ring legitimately can't turn — that is not fragility;
    // the claim is robustness AT AND ABOVE the shipped seed.)
    let sizes = [12u32, 18, 24, 36, 48];
    for endow in sizes {
        let (led, promoted, _) = promotes_with(
            1,
            |c| {
                c.barter
                    .as_mut()
                    .expect("barter")
                    .cycle_producer_medium_endowment = endow
            },
            ROBUST_TICKS,
        );
        assert!(led, "endow {endow}: SALT must lead");
        assert!(
            promoted,
            "endow {endow}: SALT must promote across the seed-size band (shipped is 12)"
        );
    }
}

#[test]
fn s20_promotes_across_anchor_densities() {
    // salt_direct_use period sweep on the TWO-LANE (ON) path. Denser anchors (lower period)
    // give more distinct SALT acceptors. Classify every cell; assert the shipped period 4 and
    // the denser ones promote. Sparser anchors may fail the acceptor floor even with two-lane
    // clearing (a real S19-style result, not a bug) — recorded, not hidden.
    let periods = [2u16, 3, 4, 6, 8];
    let mut promoted_periods = Vec::new();
    for period in periods {
        let (_led, promoted, _) = promotes_with(
            1,
            |c| c.barter.as_mut().expect("barter").salt_direct_use_period = period,
            ROBUST_TICKS,
        );
        if promoted {
            promoted_periods.push(period);
        }
    }
    // The shipped period (4) and the denser anchors (2, 3) must promote — clearing is fixed,
    // and a denser anchor clears the acceptor floor.
    for required in [2u16, 3, 4] {
        assert!(
            promoted_periods.contains(&required),
            "period {required} must promote on the two-lane path; promoted: {promoted_periods:?}"
        );
    }
}
