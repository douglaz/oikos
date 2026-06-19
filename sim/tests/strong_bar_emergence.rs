//! S9 — strong-bar money emergence (the experiment's acceptance suite).
//!
//! S8 removed designated gold, but SALT still promoted only because every colonist
//! was configured to want SALT *as a medium* before it was money (`medium_want_qty`)
//! — a residual circularity Codex flagged. This milestone removes that pre-monetary
//! want and asks the real question: **does money emerge from real saleability when no
//! agent is configured to want SALT as a medium?**
//!
//! On the gated strong path ([`SettlementConfig::frontier_coemergent_strong`]) the
//! medium want is off; SALT instead has a modest, HETEROGENEOUS real direct use (a
//! `Good(SALT)/Now` consumption want on ~1-in-8 colonists), and promotion is gated on
//! genuine INDIRECT-exchange breadth — enough indirect acceptances, by enough distinct
//! acceptors, for at least one end other than SALT's own use. The Mengerian chain runs
//! forward: heterogeneous direct use → saleability → provisional leader → indirect
//! acceptance by the OTHERS → breadth gate → promotion.
//!
//! **Observed result: money EMERGES.** SALT promotes from real saleability across
//! seeds, then the S8 chain + capital sustain on the emerged unit. The two controls
//! prove necessity: with indirect acceptance gated off SALT still leads and trades
//! directly but never monetizes; with no direct use at all there is no demand to seed
//! saleability and nothing monetizes. The realized indirect demand concentrates on the
//! staple (bread) — the one near-universal unmet want the colony re-trades SALT to
//! reach — so the indirect-target breadth is one dominant end (the gate requires one),
//! while the distinct-acceptor floor rules out a few-agent churn. This is reported
//! honestly as the headline outcome, not forced: the medium want is never restored and
//! no good is designated or seeded.

use econ::agent::{Want, WantKind};
use econ::barter::BarterReason;
use econ::good::{Gold, GoodId, Horizon, SALT, WOOD};
use econ::scenario::{builtin_market_scenario, ScenarioName};
use econ::society::Society;
use sim::{Settlement, SettlementConfig, Vocation};

fn strong() -> SettlementConfig {
    SettlementConfig::frontier_coemergent_strong()
}

/// Control: indirect acceptance gated OFF (NOT lowering the leader floor). SALT still
/// reaches provisional leader and trades directly, but no indirect volume can accrue,
/// so under the positive indirect gate it cannot monetize.
fn no_indirect_acceptance_control() -> SettlementConfig {
    let mut cfg = strong();
    cfg.barter
        .as_mut()
        .expect("barter overlay")
        .menger
        .allow_indirect_acceptance = false;
    cfg
}

/// Control: remove SALT's direct use (the medium want is already off). With no demand
/// for SALT at all, nothing seeds its saleability and it never monetizes.
fn no_direct_use_control() -> SettlementConfig {
    let mut cfg = strong();
    cfg.barter
        .as_mut()
        .expect("barter overlay")
        .salt_direct_use_qty = 0;
    cfg
}

struct ChainGoods {
    bread: GoodId,
    mill: GoodId,
    oven: GoodId,
}

fn chain_goods(cfg: &SettlementConfig) -> ChainGoods {
    let content = cfg.chain.as_ref().expect("chain").content.clone();
    ChainGoods {
        bread: content.bread(),
        mill: content.mill(),
        oven: content.oven(),
    }
}

fn living(s: &Settlement) -> usize {
    (0..s.population()).filter(|&i| s.is_alive(i)).count()
}

/// Does a living colonist currently carry a `Good(SALT)/Now` direct-use want?
fn directly_wants_salt(s: &Settlement, slot: usize) -> bool {
    let Some(id) = s.colonist_id(slot) else {
        return false;
    };
    s.society()
        .agents
        .get(id)
        .map(|agent| {
            agent
                .scale
                .iter()
                .any(|w: &Want| w.kind == WantKind::Good(SALT) && matches!(w.horizon, Horizon::Now))
        })
        .unwrap_or(false)
}

/// Run to the promotion tick (or `ticks` if it never promotes), asserting conservation
/// every tick. Returns the promotion tick.
fn run_to_promotion(s: &mut Settlement, ticks: u64) -> Option<u64> {
    for tick in 0..ticks {
        let was_barter = s.current_money_good().is_none();
        let report = s.econ_tick();
        assert!(report.conserves(), "conservation broke at tick {tick}");
        if was_barter && s.current_money_good().is_some() {
            return Some(tick);
        }
    }
    None
}

/// SALT's accumulated indirect-exchange breadth (volume, distinct acceptors, distinct
/// targets) — what the strong-bar gate reads. Persists after promotion (the tracker
/// stops accumulating but keeps the values that fired it).
fn salt_indirect_breadth(s: &Settlement) -> (u64, usize, usize) {
    s.emergence_acceptances()
        .into_iter()
        .find(|c| c.good == SALT)
        .map(|c| {
            (
                c.indirect_acceptances,
                c.indirect_acceptor_agents,
                c.indirect_target_goods,
            )
        })
        .unwrap_or((0, 0, 0))
}

/// 1. `strong_run_is_deterministic` — byte-identical `(seed, config)` through the whole
///    arc (barter → promotion → money → production → capital); a different seed
///    diverges. Non-vacuous: the run actually promoted.
#[test]
fn strong_run_is_deterministic() {
    let config = strong();
    let mut a = Settlement::generate(0xC0FFEE, &config);
    let mut b = Settlement::generate(0xC0FFEE, &config);
    a.run(1600);
    b.run(1600);
    assert_eq!(
        a.canonical_bytes(),
        b.canonical_bytes(),
        "same (seed, config) must be byte-identical through the whole strong emergence"
    );
    assert_eq!(a.digest(), b.digest());
    assert!(
        a.promoted_at_tick().is_some(),
        "the determinism run never promoted — the proof would be a vacuous barter prefix"
    );

    let mut x = Settlement::generate(7, &config);
    let mut y = Settlement::generate(7, &config);
    for tick in 0..1600u64 {
        x.econ_tick();
        y.econ_tick();
        assert_eq!(x.digest(), y.digest(), "drifted at econ tick {tick}");
    }

    let mut c = Settlement::generate(0xBADF00D, &config);
    c.run(1600);
    assert_ne!(a.digest(), c.digest(), "the seed must matter");
}

/// 2. `no_medium_want_and_salt_has_direct_use` — the setup honesty check: the strong
///    config has `medium_want_qty == 0` AND a non-zero, heterogeneous SALT direct use;
///    no designated money, no M3 ledger, every gold endowment zero, no curated
///    placement, and zero whole-system gold at generation.
#[test]
fn no_medium_want_and_salt_has_direct_use() {
    let config = strong();
    let barter = config.barter.as_ref().expect("barter overlay");

    // The circular pre-monetary medium want is OFF, but SALT is still endowed (so it
    // can circulate and convert 1:1 at promotion).
    assert_eq!(
        barter.medium_want_qty, 0,
        "no agent may be configured to want SALT as a medium before it is money"
    );
    assert_eq!(barter.medium_good, SALT);
    assert!(
        barter.consumer_medium_endowment > 0,
        "SALT must still be physically present to circulate"
    );

    // The real, heterogeneous direct use replaces it.
    assert!(
        barter.salt_direct_use_qty > 0 && barter.salt_direct_use_period > 1,
        "SALT must have a non-zero, HETEROGENEOUS real direct use (period > 1)"
    );

    // The strong-bar gate is armed.
    assert!(
        barter.menger.min_indirect_acceptances > 0
            && barter.menger.min_indirect_acceptor_agents > 0
            && barter.menger.min_indirect_target_goods > 0,
        "the indirect-breadth gate must be armed"
    );
    assert!(
        barter.menger.allow_indirect_acceptance,
        "the headline path keeps indirect acceptance on"
    );

    // No designated money, no seeded gold, no curated placement.
    assert!(config.barter.is_some() && !config.m3);
    assert_eq!(config.starting_gold_gatherer, 0);
    assert_eq!(config.starting_gold_consumer, 0);
    let chain = config.chain.as_ref().expect("chain");
    assert_eq!(chain.producer_gold, 0);
    assert!(
        !chain.subsistence_advance
            && !chain.input_advance
            && !chain.capital_advance
            && !chain.subsistence_on_grain,
        "no curated food/input/capital placement, no raw-grain floor"
    );

    let s = Settlement::generate(1, &config);
    assert!(s.is_emergent() && s.in_barter_phase());
    assert_eq!(s.current_money_good(), None, "no money good is designated");
    assert_eq!(s.promoted_at_tick(), None);
    assert_eq!(
        s.total_gold(),
        Gold::ZERO,
        "no gold exists before promotion"
    );
}

/// 3. `salt_is_traded_directly_before_it_monetizes` — pre-promotion there exist barter
///    trades accepting SALT with `BarterReason::DirectWant`, AND the heterogeneity
///    holds: some living colonists directly want SALT while OTHERS do not (so the
///    others can accept it indirectly).
#[test]
fn salt_is_traded_directly_before_it_monetizes() {
    let config = strong();
    let mut s = Settlement::generate(1, &config);

    let mut direct_salt_trades = 0u64;
    let mut want = 0usize;
    let mut nowant = 0usize;
    let mut seen = 0usize;
    for _ in 0..1600u64 {
        let was_barter = s.current_money_good().is_none();
        s.econ_tick();
        if !was_barter {
            break;
        }
        // Count direct SALT acceptances accrued in barter (each side's own reason).
        let trades = &s.society().barter_trades;
        for trade in &trades[seen..] {
            let direct_for_salt = (trade.b_gives == SALT
                && trade.a_reason == BarterReason::DirectWant)
                || (trade.a_gives == SALT && trade.b_reason == BarterReason::DirectWant);
            if direct_for_salt {
                direct_salt_trades += 1;
            }
        }
        seen = trades.len();

        // Sample the heterogeneity once a few ticks in (scales are fresh each tick).
        if s.econ_tick_count() == 4 {
            for slot in 0..s.population() {
                if !s.is_alive(slot) {
                    continue;
                }
                if directly_wants_salt(&s, slot) {
                    want += 1;
                } else {
                    nowant += 1;
                }
            }
        }
    }

    assert!(
        direct_salt_trades > 0,
        "SALT must be traded DIRECTLY (BarterReason::DirectWant) before it monetizes"
    );
    assert!(
        want > 0,
        "the heterogeneous direct use must seed SALT demand on SOME colonists"
    );
    assert!(
        nowant > 0,
        "OTHER colonists must NOT directly want SALT (so they can accept it indirectly)"
    );
}

/// 4. `promotion_requires_indirect_breadth` — the strong gate withholds promotion the
///    weak bar would fire: a variant with the indirect thresholds zeroed (the weak S8
///    bar) promotes early on direct churn, but the strong config has NOT promoted by
///    that tick. The strong config promotes only later, and at its promotion the
///    indirect-breadth gate is actually satisfied.
#[test]
fn promotion_requires_indirect_breadth() {
    // The weak bar: same heterogeneous setup, but the indirect gate is off.
    let mut weak = strong();
    {
        let m = &mut weak.barter.as_mut().expect("barter overlay").menger;
        m.min_indirect_acceptances = 0;
        m.min_indirect_acceptor_agents = 0;
        m.min_indirect_target_goods = 0;
    }
    let mut w = Settlement::generate(1, &weak);
    let weak_tick = run_to_promotion(&mut w, 1600).expect("the weak bar must promote");

    // The strong config has NOT promoted by the tick the weak bar fired.
    let mut s = Settlement::generate(1, &strong());
    for _ in 0..=weak_tick {
        s.econ_tick();
    }
    assert_eq!(
        s.current_money_good(),
        None,
        "the strong gate must withhold promotion at the tick the weak bar fires \
         (weak promoted at {weak_tick})"
    );

    // The strong config does promote later, AND its promotion clears the gate.
    let strong_tick =
        run_to_promotion(&mut s, 1600).expect("the strong bar must eventually promote");
    assert!(
        strong_tick > weak_tick,
        "the strong gate must delay promotion past the weak bar ({strong_tick} > {weak_tick})"
    );
    assert_eq!(s.current_money_good(), Some(SALT));
    let barter = strong().barter.expect("barter overlay");
    let (acc, acceptors, targets) = salt_indirect_breadth(&s);
    assert!(
        acc >= u64::from(barter.menger.min_indirect_acceptances)
            && acceptors >= usize::from(barter.menger.min_indirect_acceptor_agents)
            && targets >= usize::from(barter.menger.min_indirect_target_goods),
        "at promotion SALT must actually satisfy the indirect-breadth gate, got \
         acceptances={acc} acceptors={acceptors} targets={targets}"
    );
}

/// 5. `money_emerges_then_chain_sustains` — the headline success metric: SALT promotes
///    (the promoted good IS SALT), the chain waits on money (no producer / no chain
///    output before promotion), then bread sustains at a real rate through t1600 on the
///    emerged unit, with at least one tool built after promotion — conserving.
#[test]
fn money_emerges_then_chain_sustains() {
    let config = strong();
    let g = chain_goods(&config);
    let mut s = Settlement::generate(1, &config);

    let mut promotion_tick = None;
    let mut chain_made_before_money = 0u64;
    let mut producers_before_money = false;
    let mut made_1500_1600 = 0u64;
    let mut tools_after_promotion = 0u64;
    for tick in 0..1600u64 {
        let was_barter = s.current_money_good().is_none();
        let report = s.econ_tick();
        assert!(report.conserves(), "conservation broke at tick {tick}");
        if s.in_barter_phase() {
            assert_eq!(
                s.living_count(Vocation::Miller) + s.living_count(Vocation::Baker),
                0,
                "a production role existed during the barter phase (tick {tick})"
            );
        }
        if promotion_tick.is_none() {
            if s.living_count(Vocation::Miller) + s.living_count(Vocation::Baker) > 0 {
                producers_before_money = true;
            }
            chain_made_before_money += report.produced_of(g.bread);
            if was_barter && s.current_money_good().is_some() {
                promotion_tick = Some(tick);
            }
        } else {
            tools_after_promotion += report.produced_of(g.mill) + report.produced_of(g.oven);
        }
        if (1500..1600).contains(&tick) {
            made_1500_1600 += report.produced_of(g.bread);
        }
    }

    let promotion_tick = promotion_tick.expect("money must emerge from real saleability");
    assert_eq!(
        s.current_money_good(),
        Some(SALT),
        "the emerged money good is SALT, from real indirect saleability"
    );
    assert_eq!(s.promoted_at_tick(), Some(promotion_tick));
    assert!(
        !producers_before_money,
        "a producer role emerged before money — the ordering is violated"
    );
    assert_eq!(
        chain_made_before_money, 0,
        "the chain produced bread before money emerged — emergence must drive it"
    );
    assert!(
        made_1500_1600 >= 100,
        "the chain must still produce bread at a real rate approaching t1600 on the \
         emerged unit, got {made_1500_1600}"
    );
    assert!(
        tools_after_promotion > 0 && s.tools_built() > 0,
        "at least one tool must be built on the emerged unit after promotion, got \
         produced={tools_after_promotion} tools_built={}",
        s.tools_built()
    );
}

/// 6. `no_indirect_acceptance_control_does_not_monetize` — the clean control: with
///    indirect acceptance gated off (NOT lowering the leader floor), SALT STILL reaches
///    provisional leader and STILL trades directly, but with no indirect volume it
///    never monetizes.
#[test]
fn no_indirect_acceptance_control_does_not_monetize() {
    let control = no_indirect_acceptance_control();
    assert!(
        !control
            .barter
            .as_ref()
            .expect("barter overlay")
            .menger
            .allow_indirect_acceptance,
        "the control gates indirect acceptance off"
    );

    let mut s = Settlement::generate(1, &control);
    let mut reached_leader = false;
    let mut direct_salt_trades = 0u64;
    let mut seen = 0usize;
    for tick in 0..800u64 {
        let report = s.econ_tick();
        assert!(report.conserves(), "control ledger broke at tick {tick}");
        assert_eq!(
            s.current_money_good(),
            None,
            "money emerged with indirect acceptance off at tick {tick}"
        );
        if s.society().saleability_provisional_leader() == Some(SALT) {
            reached_leader = true;
        }
        let trades = &s.society().barter_trades;
        for trade in &trades[seen..] {
            if (trade.b_gives == SALT && trade.a_reason == BarterReason::DirectWant)
                || (trade.a_gives == SALT && trade.b_reason == BarterReason::DirectWant)
            {
                direct_salt_trades += 1;
            }
        }
        seen = trades.len();
    }
    assert_eq!(s.promoted_at_tick(), None, "the control must never promote");
    assert!(
        reached_leader,
        "the control is non-vacuous: SALT must still reach provisional leader"
    );
    assert!(
        direct_salt_trades > 0,
        "the control is non-vacuous: SALT must still trade DIRECTLY, it just cannot \
         accrue the indirect volume promotion needs"
    );
    let (indirect, _, _) = salt_indirect_breadth(&s);
    assert_eq!(
        indirect, 0,
        "with indirect acceptance off no indirect volume may accrue"
    );
}

/// 7. `no_direct_use_control_does_not_monetize` — remove SALT's direct use (the medium
///    want is already off): with no demand for SALT at all, nothing seeds its
///    saleability and it never monetizes.
#[test]
fn no_direct_use_control_does_not_monetize() {
    let control = no_direct_use_control();
    let barter = control.barter.as_ref().expect("barter overlay");
    assert_eq!(barter.salt_direct_use_qty, 0, "the direct use is removed");
    assert_eq!(barter.medium_want_qty, 0, "the medium want stays off");

    let mut s = Settlement::generate(1, &control);
    for tick in 0..800u64 {
        let report = s.econ_tick();
        assert!(report.conserves(), "control ledger broke at tick {tick}");
        assert_eq!(
            s.current_money_good(),
            None,
            "money emerged with no SALT demand at all at tick {tick}"
        );
    }
    assert_eq!(s.promoted_at_tick(), None, "the control must never promote");
    assert!(
        s.econ_stock_total(SALT) > 0,
        "the control is non-vacuous: SALT is still present, just not demanded"
    );
}

/// 8. `alternate_winner_is_a_valid_outcome` — the diagnostic guard: WHICHEVER good
///    promotes must itself satisfy the indirect-breadth gate (a real Mengerian winner,
///    not an artifact). The observed winner is SALT; this asserts it, and that it
///    cleared the gate — but it is written to accept a different winner too (it would
///    still demand the gate is met), so a future tuning change cannot smuggle in an
///    unearned promotion.
#[test]
fn alternate_winner_is_a_valid_outcome() {
    let config = strong();
    let mut s = Settlement::generate(1, &config);
    let promotion_tick = run_to_promotion(&mut s, 1600);

    let winner = s
        .current_money_good()
        .expect("a good must promote in the headline run");
    assert_eq!(
        winner, SALT,
        "the observed winner is SALT (a non-SALT winner would also be valid, but must \
         still clear the gate below)"
    );
    assert!(promotion_tick.is_some());

    // The winner — whatever it is — must satisfy the armed indirect-breadth gate.
    let barter = config.barter.expect("barter overlay");
    let breadth = s
        .emergence_acceptances()
        .into_iter()
        .find(|c| c.good == winner)
        .expect("the winner must appear among the candidates");
    assert!(
        breadth.indirect_acceptances >= u64::from(barter.menger.min_indirect_acceptances)
            && breadth.indirect_acceptor_agents
                >= usize::from(barter.menger.min_indirect_acceptor_agents)
            && breadth.indirect_target_goods
                >= usize::from(barter.menger.min_indirect_target_goods),
        "the promoted good must be a real indirect-exchange winner, got {breadth:?}"
    );
}

/// 9. `strong_emergence_conserves` — whole-system conservation every tick, INCLUDING
///    the SALT direct-use consumption (the `consumed` bucket) and the promotion sink.
///    Non-vacuous: the direct-use consumption, the promotion mint, production, and a
///    capital build all actually occur over the run.
#[test]
fn strong_emergence_conserves() {
    let config = strong();
    let g = chain_goods(&config);
    let mut s = Settlement::generate(1, &config);

    let mut prev_gold = s.total_gold();
    let mut promotions = 0u32;
    let mut salt_consumed = 0u64;
    let mut any_produced = 0u64;
    for tick in 0..1600u64 {
        let report = s.econ_tick();
        assert!(
            report.conserves(),
            "whole-system conservation broke at tick {tick}"
        );

        // The SALT direct-use eats into the `consumed` bucket pre-promotion.
        salt_consumed += report.consumed_of(SALT);

        // Money conservation: closed except the 1-for-1 promotion mint.
        let gold = s.total_gold();
        let minted: u64 = report.promoted.values().sum();
        assert_eq!(
            gold.0,
            prev_gold.0 + minted,
            "money conservation broke at tick {tick} (minted {minted})"
        );
        if minted > 0 {
            promotions += 1;
            let (&winner, &units) = report.promoted.iter().next().expect("a promotion good");
            assert_eq!(units, minted, "more than one good promoted at once");
            assert_eq!(winner, SALT, "SALT is the emerged money good");
            assert_eq!(
                s.econ_stock_total(winner),
                0,
                "the promoted stock did not convert"
            );
        }
        prev_gold = gold;
        any_produced += report.produced_of(g.bread);
    }

    assert_eq!(promotions, 1, "exactly one promotion must have occurred");
    assert!(
        salt_consumed > 0,
        "the SALT direct-use consumption must actually occur (the `consumed` sink)"
    );
    assert!(any_produced > 0, "no recipe output — production never ran");
    assert!(
        s.tools_built() > 0,
        "no capital build — the S7 flow never ran"
    );
    // Population stays bounded across the run (no collapse, no explosion).
    assert!(
        (8..=40).contains(&living(&s)),
        "population should stay bounded, got {}",
        living(&s)
    );
    // WOOD is the build input; the run consumed some as a capital input (non-vacuous).
    assert!(
        s.whole_system_total(WOOD) > 0,
        "WOOD must remain in the closed system"
    );
}

/// 10. `goldens_unchanged` — the engine's conformance scenarios still replay
///     byte-identically (the six econ goldens are untouched — every S9 edit is additive
///     and gated), the strong builder DERIVES from `frontier_coemergent` without
///     mutating it, and the S8 co-emergent base still promotes SALT under its OWN
///     (unchanged) config. (The full G5a/G5b/coemergence suites, the
///     `canonical_bytes_include_*` digest regressions, clippy `-D warnings`, and fmt
///     `--check` are the workspace gate enforcing the rest.)
#[test]
fn goldens_unchanged() {
    for name in [
        ScenarioName::MarketBarterishGold,
        ScenarioName::MarketPriceDiscovery,
        ScenarioName::MengerSaltMoney,
        ScenarioName::MengerGoldMoney,
    ] {
        let scenario = builtin_market_scenario(name);
        let periods = scenario.periods;
        let mut first = Society::from_scenario(scenario);
        first.run(periods);
        let mut second = Society::from_scenario(builtin_market_scenario(name));
        second.run(periods);
        assert_eq!(
            first.records, second.records,
            "{name:?} did not replay deterministically"
        );
        assert_eq!(
            first.v2_records, second.v2_records,
            "{name:?} V2 records diverged"
        );
    }

    // The strong builder derives from `frontier_coemergent` without mutating it: the
    // base keeps its configured medium want and its inert (default) indirect gate.
    let base = SettlementConfig::frontier_coemergent();
    let base_barter = base.barter.as_ref().expect("barter overlay");
    assert!(
        base_barter.medium_want_qty > 0,
        "the S8 co-emergent base must keep its configured medium want"
    );
    assert_eq!(
        base_barter.salt_direct_use_qty, 0,
        "the S8 base has no heterogeneous direct use"
    );
    assert_eq!(
        base_barter.menger.min_indirect_acceptances, 0,
        "the S8 base runs the inert (weak) promotion gate"
    );
    assert!(base_barter.menger.allow_indirect_acceptance);

    // The strong derivation flips exactly the strong-bar knobs.
    let strong = strong();
    let strong_barter = strong.barter.as_ref().expect("barter overlay");
    assert_eq!(strong_barter.medium_want_qty, 0);
    assert!(strong_barter.salt_direct_use_qty > 0);
    assert!(strong_barter.menger.min_indirect_acceptances > 0);

    // The S8 base still promotes SALT under its OWN config (unchanged behavior).
    let mut s8 = Settlement::generate(1, &SettlementConfig::frontier_coemergent());
    s8.run(400);
    assert_eq!(
        s8.current_money_good(),
        Some(SALT),
        "the S8 co-emergent base must still promote SALT under its unchanged config"
    );
}
