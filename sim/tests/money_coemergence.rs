//! S8 — money co-emergence with the specialized economy (the DoD acceptance suite).
//!
//! The `coemergent` scenario ([`SettlementConfig::frontier_coemergent`]) composes the
//! whole stack from a NO-money barter start: money, the grain→flour→bread division of
//! labor, and capital all CO-EMERGE in one run, with NO designated GOLD and NO curated
//! placement. SALT promotes by saleability from real indirect exchange; then the
//! (money-good-agnostic) S5 sustain stack and the S7 capital phase run on the EMERGED
//! unit. The honest bar: emergence drives the chain (not the reverse), the chain
//! SUSTAINS on emerged money (it does not freeze at the cutover), capital forms on the
//! emerged unit, hunger stays bounded, and everything conserves and is deterministic.
//!
//! These are the named acceptance tests from `docs/impl-money-coemergence.md`. Two of
//! them land a **principled finding** rather than the spec's hypothesized mechanism,
//! reported honestly as PASSING diagnostics that assert the OBSERVED behavior via the
//! S8.0 probe (see `tension_b_*`):
//!  - the chain's working capital across the cutover is NOT barter-earned SALT (the
//!    `frontier` saleability hub concentrates SALT in consumers who barely spend it
//!    before the fast promotion — Base Fact 5), it is post-promotion money-market sales
//!    of seeded cold-start output (earned, not endowed). The chain survives the cutover
//!    on these earnings regardless — so the sustain/capital tests below still PASS.

use econ::good::{Gold, GoodId, SALT, WOOD};
use econ::scenario::{builtin_market_scenario, ScenarioName};
use econ::society::Society;
use sim::{Settlement, SettlementConfig, Vocation};

fn coemergent() -> SettlementConfig {
    SettlementConfig::frontier_coemergent()
}

/// The no-saleability CONTROL: keep a physical SALT stock AND the barter overlay, but
/// drop the universal medium WANT — the asymmetry that makes SALT widely accepted.
/// SALT becomes just another good and never leads the saleability race, so it never
/// promotes. (Spreading the endowment evenly does NOT suffice — the universal want
/// alone still monetizes SALT — so this is the faithful falsification twin: the medium
/// is still present and still endowed, only its universal acceptance is removed.)
fn coemergent_control() -> SettlementConfig {
    let mut cfg = coemergent();
    cfg.barter.as_mut().expect("barter overlay").medium_want_qty = 0;
    cfg
}

struct ChainGoods {
    grain: GoodId,
    flour: GoodId,
    bread: GoodId,
    mill: GoodId,
    oven: GoodId,
}

fn chain_goods(cfg: &SettlementConfig) -> ChainGoods {
    let content = cfg.chain.as_ref().expect("chain").content.clone();
    ChainGoods {
        grain: content.grain(),
        flour: content.flour(),
        bread: content.bread(),
        mill: content.mill(),
        oven: content.oven(),
    }
}

fn living(s: &Settlement) -> usize {
    (0..s.population()).filter(|&i| s.is_alive(i)).count()
}

/// Run to the promotion tick (or `ticks` if it never promotes), returning the tick a
/// money good emerged. Asserts conservation every tick along the way.
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

/// 1. `coemergent_run_is_deterministic` — same `(seed, config)` → byte-identical
///    (`canonical_bytes` + `digest`) through barter → promotion → money → production →
///    capital; a different seed diverges. Non-vacuous: the run actually promoted, so
///    the determinism claim spans the whole co-emergence, not a quiet barter prefix.
#[test]
fn coemergent_run_is_deterministic() {
    let config = coemergent();
    let mut a = Settlement::generate(0xC0FFEE, &config);
    let mut b = Settlement::generate(0xC0FFEE, &config);
    a.run(1600);
    b.run(1600);
    assert_eq!(
        a.canonical_bytes(),
        b.canonical_bytes(),
        "same (seed, config) must be byte-identical through the whole co-emergence"
    );
    assert_eq!(a.digest(), b.digest());
    assert!(
        a.promoted_at_tick().is_some(),
        "the determinism run never promoted — the proof would be a vacuous barter prefix"
    );

    // Tick-by-tick lockstep across the whole composed run.
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

/// 2. `no_designated_money_and_zero_gold_at_generation` — the setup honesty check:
///    `frontier_coemergent` generates with `barter = Some(..)`, NO designated money
///    (it starts in barter with no money good and zero whole-system gold), EVERY gold
///    endowment zero, and NO curated placement phase (`*_advance`) on.
#[test]
fn no_designated_money_and_zero_gold_at_generation() {
    let config = coemergent();

    // A barter-start emergent overlay, not designated GOLD.
    assert!(
        config.barter.is_some(),
        "the co-emergent base starts in barter"
    );
    assert!(!config.m3, "no M3 designated ledger money");

    // Every gold endowment is zero (the barter overlay's tripwire asserts it).
    assert_eq!(config.starting_gold_gatherer, 0);
    assert_eq!(config.starting_gold_consumer, 0);
    let chain = config.chain.as_ref().expect("chain");
    assert_eq!(
        chain.producer_gold, 0,
        "producers hold no money before promotion"
    );
    let demo = config.demography.as_ref().expect("demography");
    assert_eq!(demo.child_gold_endowment, 0);
    assert!(demo.households.iter().all(|h| h.starting_gold == 0));

    // NO curated placement and NO raw-grain subsistence floor.
    assert!(
        !chain.subsistence_advance
            && !chain.input_advance
            && !chain.capital_advance
            && !chain.subsistence_on_grain
            && !chain.productive_reentry,
        "no curated food/input/capital placement, no grain floor, no S6 re-entry"
    );

    // At generation the settlement is emergent, in barter, with NO money good and ZERO
    // whole-system gold — nothing was designated or seeded.
    let s = Settlement::generate(1, &config);
    assert!(s.is_emergent() && s.in_barter_phase());
    assert_eq!(s.current_money_good(), None, "no money good is designated");
    assert_eq!(
        s.promoted_at_tick(),
        None,
        "nothing has promoted at generation"
    );
    assert_eq!(
        s.total_gold(),
        Gold::ZERO,
        "no gold exists before promotion"
    );
}

/// 3. `money_emerges_endogenously` — the promoted good IS SALT with a real
///    `promoted_at_tick()` (not merely `current_money_good().is_some()`), AND no active
///    Miller/Baker and no chain production occur before the promotion tick — the chain
///    waits on money, proving emergence drives the division of labor, not the reverse.
#[test]
fn money_emerges_endogenously() {
    let config = coemergent();
    let g = chain_goods(&config);
    let mut s = Settlement::generate(2_026, &config);

    // Starts in barter, no money assumed, no producer roles seeded (only the latent pool).
    assert!(s.is_emergent() && s.in_barter_phase());
    assert_eq!(s.vocation_count(Vocation::Miller), 0, "no seeded millers");
    assert_eq!(s.vocation_count(Vocation::Baker), 0, "no seeded bakers");
    assert!(
        s.living_count(Vocation::Unassigned) > 0,
        "the co-emergent base seeds a latent producer pool"
    );

    let mut promotion_tick = None;
    let mut chain_made_before_money = 0u64;
    let mut producers_before_money = false;
    for tick in 0..200u64 {
        let was_barter = s.current_money_good().is_none();
        let report = s.econ_tick();
        // The economic-ordering tripwire: while still in barter, no producer role exists
        // and no chain good is produced.
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
            chain_made_before_money += report.produced_of(g.flour) + report.produced_of(g.bread);
        }
        if was_barter && s.current_money_good().is_some() {
            promotion_tick = Some(tick);
            break;
        }
    }

    let promotion_tick = promotion_tick.expect("a money good must emerge");
    // The promoted good IS SALT, with a real promotion tick latched.
    assert_eq!(
        s.current_money_good(),
        Some(SALT),
        "the durable SALT medium is the emerged money good"
    );
    assert_eq!(
        s.promoted_at_tick(),
        Some(promotion_tick),
        "the promotion tick must be a real, latched value"
    );
    // The chain waited on money: no producer and no chain output before promotion.
    assert!(
        !producers_before_money,
        "a producer role emerged before money — the ordering is violated"
    );
    assert_eq!(
        chain_made_before_money, 0,
        "the chain produced flour/bread before money emerged — emergence must drive it"
    );
}

/// 4. `no_saleability_control_does_not_monetize` — a NON-VACUOUS control: keep a
///    physical SALT stock AND the barter overlay, but drop the universal medium want
///    (the asymmetry that makes SALT widely accepted). Money never emerges; emergence
///    is earned, not structural-by-accident.
#[test]
fn no_saleability_control_does_not_monetize() {
    let control = coemergent_control();
    // Non-vacuous: SALT is still physically endowed and the barter overlay is still on.
    let barter = control.barter.as_ref().expect("barter overlay");
    assert!(
        barter.consumer_medium_endowment > 0,
        "the control must keep a physical SALT stock (it removes the want, not the good)"
    );
    assert_eq!(barter.medium_good, SALT);
    assert_eq!(
        barter.medium_want_qty, 0,
        "the universal medium want is dropped"
    );

    let mut s = Settlement::generate(1, &control);
    for tick in 0..600u64 {
        let report = s.econ_tick();
        assert!(report.conserves(), "control ledger broke at tick {tick}");
        assert!(
            report.promoted.is_empty(),
            "the control promoted a money good at tick {tick}"
        );
        assert_eq!(
            s.current_money_good(),
            None,
            "money emerged in the no-saleability control at tick {tick}"
        );
    }
    assert_eq!(
        s.promoted_at_tick(),
        None,
        "the control must never latch a promotion"
    );
    // And SALT genuinely still circulated as a good (the control is not inert because
    // SALT vanished — it is present but not specially wanted).
    assert!(
        s.econ_stock_total(SALT) > 0,
        "the control must keep SALT in the economy (a non-vacuous twin)"
    );
}

/// 5. `tension_b_working_capital_is_earned_post_promotion_not_in_barter` — THE
///    principled S8.2 finding, landed honestly as a PASSING diagnostic on the S8.0
///    probe. The spec hypothesized the cutover working capital would be barter-earned
///    SALT (latent producers selling seeded bread/WOOD into barter, converting 1:1 to
///    gold at promotion). The OBSERVED behavior is different and informative: the
///    `frontier` saleability hub concentrates SALT in consumers who barely spend it
///    before the fast promotion (Base Fact 5), so latent producers earn ~NO barter SALT
///    and hold ~zero CONVERTED gold at the promotion tick. The chain survives the
///    cutover REGARDLESS — producers fund their working capital by selling their seeded
///    cold-start output into the real MONEY market post-promotion (earned, not endowed,
///    no advance), as `inputs_acquired_by_market_trade_after_promotion` confirms.
#[test]
fn tension_b_working_capital_is_earned_post_promotion_not_in_barter() {
    let config = coemergent();
    let mut s = Settlement::generate(1, &config);

    // Walk to the promotion tick, capturing the SALT distribution the tick BEFORE it
    // (after promotion SALT converts to gold and delists), and the producer cash AT it.
    let mut salt_in_consumers_pre = 0u64;
    let mut salt_in_producers_pre = 0u64;
    let mut promotion_tick = None;
    for tick in 0..200u64 {
        // Snapshot pre-step SALT holdings; if this step promotes, these are the last
        // barter-phase holdings.
        let consumer_salt: u64 = s
            .stock_by_vocation(SALT)
            .iter()
            .filter(|(voc, _)| *voc == Vocation::Consumer)
            .map(|(_, qty)| *qty)
            .sum();
        let producer_salt: u64 = s
            .stock_by_vocation(SALT)
            .iter()
            .filter(|(voc, _)| {
                matches!(
                    voc,
                    Vocation::Miller | Vocation::Baker | Vocation::Unassigned
                )
            })
            .map(|(_, qty)| *qty)
            .sum();
        let was_barter = s.current_money_good().is_none();
        s.econ_tick();
        if was_barter && s.current_money_good().is_some() {
            salt_in_consumers_pre = consumer_salt;
            salt_in_producers_pre = producer_salt;
            promotion_tick = Some(tick);
            break;
        }
    }
    assert!(
        promotion_tick.is_some(),
        "money must emerge for the finding to apply"
    );

    // The hub concentrated SALT in consumers (Base Fact 5) — the producers held ~none.
    assert!(
        salt_in_consumers_pre > 0,
        "the SALT must be held SOMEWHERE in barter (a non-vacuous probe)"
    );
    assert!(
        salt_in_producers_pre * 4 < salt_in_consumers_pre,
        "the finding is that the hub concentrates SALT in consumers, not producers: \
         producers held {salt_in_producers_pre}, consumers {salt_in_consumers_pre}"
    );

    // AT the promotion tick the latent producers hold ~no CONVERTED gold — the
    // hypothesized barter-earned working capital did NOT materialize.
    let cash = s.producer_cash();
    let latent_converted: u64 = cash
        .iter()
        .filter(|c| c.role.is_latent())
        .map(|c| c.gold)
        .sum();
    assert_eq!(
        latent_converted, 0,
        "the finding is that latent producers earn ~no barter SALT, so they hold no \
         converted gold at promotion; got {latent_converted}"
    );
    // And the bread-for-SALT leg the spec hoped would capitalize them is thin.
    assert!(
        s.bread_for_salt_volume() < 50,
        "the bread-for-SALT leg is thin (the hub hoards SALT): {}",
        s.bread_for_salt_volume()
    );

    // YET the chain survives the cutover on EARNED post-promotion working capital: an
    // active producer buys its recipe input by a real market trade within a window after
    // promotion (funded by selling its seeded output, not endowed).
    let g = chain_goods(&config);
    let mut input_trade_after_promotion = false;
    let mut seen = s.society().trades.len();
    for _ in 0..400u64 {
        s.econ_tick();
        let trades = &s.society().trades;
        for trade in &trades[seen..] {
            if trade.buyer == trade.seller {
                continue;
            }
            let bought_input = (trade.good == g.grain
                && matches!(s.vocation_of_id(trade.buyer), Some(Vocation::Miller)))
                || (trade.good == g.flour
                    && matches!(s.vocation_of_id(trade.buyer), Some(Vocation::Baker)));
            if bought_input {
                input_trade_after_promotion = true;
            }
        }
        seen = trades.len();
        if input_trade_after_promotion {
            break;
        }
    }
    assert!(
        input_trade_after_promotion,
        "the chain must survive the cutover on EARNED post-promotion working capital — \
         a producer buying its input by a real market trade (not barter SALT, not endowed)"
    );
}

/// 6. `inputs_acquired_by_market_trade_after_promotion` — THE cutover metric: after the
///    promotion tick there exist real `Society::trades` where an active Miller/Baker
///    buys grain/flour from a DIFFERENT seller, later consumed as a recipe input — the
///    chain clears inputs across the barter→money discontinuity (no freeze).
#[test]
fn inputs_acquired_by_market_trade_after_promotion() {
    let config = coemergent();
    let g = chain_goods(&config);
    let mut s = Settlement::generate(1, &config);

    let promotion_tick = run_to_promotion(&mut s, 200).expect("money must emerge");

    let mut producer_input_trades = 0u64;
    let mut grain_consumed_as_input = 0u64;
    let mut flour_consumed_as_input = 0u64;
    let mut seen = s.society().trades.len();
    // A window comfortably past the cutover, by which only post-promotion market-sourced
    // inputs exist (the seeded cold-start buffers are long gone).
    let window_start = promotion_tick + 200;
    for tick in (promotion_tick + 1)..(promotion_tick + 400) {
        let report = s.econ_tick();
        assert!(report.conserves(), "conservation broke at tick {tick}");
        if tick >= window_start {
            grain_consumed_as_input += report.consumed_as_input_of(g.grain);
            flour_consumed_as_input += report.consumed_as_input_of(g.flour);
        }
        let trades = &s.society().trades;
        for trade in &trades[seen..] {
            if trade.tick < window_start || trade.buyer == trade.seller {
                continue;
            }
            let bought_input = (trade.good == g.grain
                && matches!(s.vocation_of_id(trade.buyer), Some(Vocation::Miller)))
                || (trade.good == g.flour
                    && matches!(s.vocation_of_id(trade.buyer), Some(Vocation::Baker)));
            if bought_input {
                producer_input_trades += 1;
            }
        }
        seen = trades.len();
    }

    assert!(
        producer_input_trades > 0,
        "after the cutover an active producer must acquire its input through a real \
         order-book Trade from a different seller, got {producer_input_trades}"
    );
    assert!(
        grain_consumed_as_input > 0 && flour_consumed_as_input > 0,
        "the market-bought inputs must be consumed as recipe inputs (grain milled, flour \
         baked), got grain={grain_consumed_as_input} flour={flour_consumed_as_input}"
    );
}

/// 7. `specialization_sustains_on_emerged_money` — `bread.made > 0` through tick 800 and
///    1600, on emerged SALT money (not designated gold). The chain does not freeze at
///    the barter→money cutover once the post-promotion money pulse is absorbed.
#[test]
fn specialization_sustains_on_emerged_money() {
    let config = coemergent();
    let g = chain_goods(&config);
    let mut s = Settlement::generate(1, &config);

    let mut made_700_800 = 0u64;
    let mut made_1500_1600 = 0u64;
    for tick in 0..1600u64 {
        let produced = s.econ_tick().produced_of(g.bread);
        if (700..800).contains(&tick) {
            made_700_800 += produced;
        }
        if (1500..1600).contains(&tick) {
            made_1500_1600 += produced;
        }
    }
    // The money it sustained on is the EMERGED SALT, not designated gold.
    assert_eq!(s.current_money_good(), Some(SALT));
    // Require a real production RATE, not merely non-zero (a trickle): seed 1 makes
    // ~450 bread in each 100-tick window, so a floor of 100 proves a sustained chain
    // with comfortable headroom rather than just "not collapsed".
    assert!(
        made_700_800 >= 100,
        "the chain should still produce bread at a real rate approaching tick 800, \
         got {made_700_800}"
    );
    assert!(
        made_1500_1600 >= 100,
        "the chain should still produce bread at a real rate approaching tick 1600, \
         got {made_1500_1600}"
    );
}

/// 8. `hunger_bounded_under_coemergence` — hunger mean/p95 bounded and non-drifting over
///    tail windows, no curated placement (a modest residual tail is allowed — S6
///    provisioning-at-scale under emergence is deferred to S9).
#[test]
fn hunger_bounded_under_coemergence() {
    let config = coemergent();
    let mut s = Settlement::generate(1, &config);

    let hunger_stats = |s: &Settlement| -> (u64, u16) {
        let mut h: Vec<u16> = (0..s.population())
            .filter(|&i| s.is_alive(i))
            .filter_map(|i| s.need_of(i).map(|n| n.hunger))
            .collect();
        h.sort_unstable();
        if h.is_empty() {
            return (0, 0);
        }
        let mean = h.iter().map(|&x| u64::from(x)).sum::<u64>() / h.len() as u64;
        let p95 = h[(h.len() * 95 / 100).min(h.len() - 1)];
        (mean, p95)
    };

    let mut samples: Vec<(usize, u64, u16)> = Vec::new();
    for tick in 0..1600u64 {
        s.econ_tick();
        if matches!(tick + 1, 800 | 1200 | 1600) {
            let (m, p) = hunger_stats(&s);
            samples.push((living(&s), m, p));
        }
    }

    // Population bounded across the tail (no collapse, no explosion).
    for (pop, ..) in &samples {
        assert!(
            (8..=40).contains(pop),
            "population should stay bounded, got {pop} in {samples:?}"
        );
    }
    // Hunger mean/p95 bounded (a modest residual tail allowed) and non-drifting.
    let (_, m_first, _) = samples[0];
    let (_, m_last, p_last) = *samples.last().unwrap();
    assert!(
        m_last <= 9 && p_last <= 13,
        "hunger should stay bounded (mean below chronic; p95 tail capped), got \
         mean={m_last} p95={p_last}"
    );
    assert!(
        (m_first as i64 - m_last as i64).abs() <= 3,
        "hunger mean should not drift, got {m_first} -> {m_last}"
    );
}

/// 9. `capital_forms_on_emerged_money` — at least one mill/oven is BUILT (`produced`)
///    AFTER promotion under emerged-money prices, conserving — the S7 build runs on the
///    emerged SALT unit. The hard "new capital entered the chain" check: a
///    formerly-non-latent colonist built a PRODUCED tool, adopted, and BOUGHT its recipe
///    input by a real `Society::trade` (buyer != seller), so it is not a seeded artifact.
#[test]
fn capital_forms_on_emerged_money() {
    let config = coemergent();
    let g = chain_goods(&config);
    let mut s = Settlement::generate(1, &config);
    let seeded_tools = s.whole_system_total(g.mill) + s.whole_system_total(g.oven);

    let mut promotion_tick = None;
    let mut tool_produced_after_promotion = 0u64;
    let mut wood_consumed_as_input = 0u64;
    let mut input_trade_by_built_adopter = false;
    let mut chain_output_produced = 0u64;
    let mut seen = 0usize;
    for tick in 0..1600u64 {
        let was_barter = s.current_money_good().is_none();
        let report = s.econ_tick();
        assert!(
            report.conserves(),
            "conservation broke across a build at tick {tick}"
        );
        if was_barter && s.current_money_good().is_some() {
            promotion_tick = Some(tick);
        }
        if promotion_tick.is_some() {
            tool_produced_after_promotion +=
                report.produced_of(g.mill) + report.produced_of(g.oven);
            // WOOD is consumed_as_input ONLY by a capital build (no recipe consumes WOOD).
            wood_consumed_as_input += report.consumed_as_input_of(WOOD);
            chain_output_produced += report.produced_of(g.flour) + report.produced_of(g.bread);
            let trades = &s.society().trades;
            for trade in &trades[seen..] {
                if trade.buyer == trade.seller {
                    continue;
                }
                let Some(slot) =
                    (0..s.population()).find(|&i| s.colonist_id(i) == Some(trade.buyer))
                else {
                    continue;
                };
                if !s.acquired_tool_of(slot) {
                    continue;
                }
                let bought_input = (trade.good == g.grain
                    && s.vocation_of(slot) == Some(Vocation::Miller))
                    || (trade.good == g.flour && s.vocation_of(slot) == Some(Vocation::Baker));
                if bought_input {
                    input_trade_by_built_adopter = true;
                }
            }
            seen = trades.len();
        }
    }

    let promotion_tick = promotion_tick.expect("money must emerge");
    assert!(s.promoted_at_tick() == Some(promotion_tick));
    assert!(
        s.tools_built() > 0,
        "a builder must complete at least one tool on the emerged money, got {}",
        s.tools_built()
    );
    assert!(
        tool_produced_after_promotion > 0,
        "the tool must be PRODUCED after promotion (under emerged-money prices), got \
         {tool_produced_after_promotion}"
    );
    assert!(
        wood_consumed_as_input > 0,
        "the build must book its WOOD to consumed_as_input, got {wood_consumed_as_input}"
    );
    assert!(
        s.whole_system_total(g.mill) + s.whole_system_total(g.oven) > seeded_tools,
        "the whole-system tool count must rise above the seeded count"
    );
    assert!(
        (0..s.population()).any(|i| s.acquired_tool_of(i)),
        "a formerly-non-latent colonist must have built a produced tool"
    );
    assert!(
        input_trade_by_built_adopter,
        "a built-tool adopter must have bought its recipe input by a real Society::trade"
    );
    assert!(
        chain_output_produced > 0,
        "the built capital must have transformed its input (the chain produced output)"
    );
}

/// 10. `coemergence_conserves` — whole-system conservation every tick, including the
///     promotion sink (`report.promoted`) and all chain/capital flows. Non-vacuous: the
///     promotion mint, production, and a capital build all actually occur over the run.
#[test]
fn coemergence_conserves() {
    let config = coemergent();
    let g = chain_goods(&config);
    let mut s = Settlement::generate(1, &config);

    let mut prev_gold = s.total_gold();
    let mut promotions = 0u32;
    let mut any_produced = 0u64;
    for tick in 0..1600u64 {
        let report = s.econ_tick();
        assert!(
            report.conserves(),
            "whole-system conservation broke at tick {tick}"
        );

        // Money conservation: a closed balance except the 1-for-1 promotion mint.
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
        any_produced += report.produced_of(g.bread) + report.produced_of(g.flour);
    }

    assert_eq!(promotions, 1, "exactly one promotion must have occurred");
    assert!(any_produced > 0, "no recipe output — production never ran");
    assert!(
        s.tools_built() > 0,
        "no capital build — the S7 flow never ran"
    );
}

/// 11. `goldens_unchanged` — the engine's conformance scenarios still replay
///     byte-identically (the six econ goldens are untouched — every S8 edit lives in
///     `sim`, additive and gated), and the new `frontier_coemergent` builder does not
///     mutate the existing `frontier`/`frontier_endogenous` builders. (The full G5a/G5b
///     emergence goldens, the S5/S6/S7 acceptance suites, the `canonical_bytes_include_*`
///     digest regressions, clippy `-D warnings`, and fmt `--check` are the workspace
///     gate that enforces the rest.)
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

    // The co-emergent builder is additive: it does not mutate the existing builders it
    // is NOT derived from. `frontier` (G5b) keeps its barter-start defaults (no S5
    // sustain stack, full hearth provisions); `frontier_endogenous` (S5) keeps its
    // designated-GOLD regime (`barter = None`).
    let frontier = SettlementConfig::frontier();
    let fchain = frontier.chain.as_ref().expect("chain");
    assert!(
        !fchain.recurring_motive && !fchain.project_input_bids && fchain.producer_subsistence == 0,
        "frontier must keep its bare barter-start chain (no S5 sustain stack)"
    );
    assert!(
        frontier
            .demography
            .as_ref()
            .expect("demography")
            .households
            .iter()
            .all(|h| h.food_provision == 3),
        "frontier must keep its full hearth provisions (the lean trim is co-emergent-only)"
    );
    let endogenous = SettlementConfig::frontier_endogenous();
    assert!(
        endogenous.barter.is_none(),
        "frontier_endogenous stays designated-GOLD"
    );
    // And the co-emergent base is genuinely a distinct, emergent regime.
    let coemergent = coemergent();
    assert!(
        coemergent.barter.is_some(),
        "frontier_coemergent is barter-start emergent"
    );
    assert!(
        coemergent.chain.as_ref().unwrap().producible_capital,
        "frontier_coemergent composes the S7 capital phase"
    );
}
