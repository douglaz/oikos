//! G5a acceptance suite — money **emerges** from spatial barter.
//!
//! Every sim settlement before G5a runs on econ's designated GOLD market: money
//! is assumed. G5a removes the assumption. A curated **barter camp** starts with
//! no designated money; colonists haul FOOD and WOOD from two nodes and barter
//! goods-for-goods at the exchange, demanding a durable SALT medium. The realized
//! spatial barter feeds econ's reused `SaleabilityTracker`, and when the lab's
//! Mengerian `winner` rule fires, SALT is **promoted**: its econ stock converts
//! to money units (the lab's conserved promotion) and from the next tick trade is
//! money-priced (the existing G2b market).
//!
//! The DoD is a **mechanism + falsification twin**: money emerges in
//! `barter-camp` (tests 2, 5) and does **not** in the symmetric `barter-camp-control`
//! (test 3) — the trade structure (a saleable medium), not luck, is what
//! monetizes. The promotion routes through econ's reused rule (test 6), the whole
//! run is deterministic across the phase transition (test 1), conservation is
//! exact across barter → promotion → money (test 4), and the econ goldens are
//! untouched (test 7). The multi-seed spatial robustness STUDY and composition
//! with production/demography (G5b) are deferred — this is the mechanism slice.

use econ::good::{Gold, GoodId, FOOD, SALT, WOOD};
use econ::money::MengerianConfig;
use econ::society::V2PromotionFailureReason;
use sim::{Settlement, SettlementConfig, Vocation};

fn camp() -> SettlementConfig {
    SettlementConfig::barter_camp()
}

fn control() -> SettlementConfig {
    SettlementConfig::barter_camp_control()
}

/// The tick the barter camp promotes for the seeds these tests use. The run is
/// deterministic, so this is a fixed tick (test 1), reached well inside the
/// horizon — but the tests assert the *transition*, never a hand-tuned magnitude.
const RUN_TICKS: u64 = 40;

/// 1. `barter_camp_run_is_deterministic` — same `(seed, config)` → byte-identical
///    run through the barter, promotion, and money-priced phases. Nothing is drawn
///    in the loops (the `Rng` is consumed only at generation), the barter/saleability/
///    promotion machinery is integer + `BTreeMap`/`Vec`, and the canonical digest
///    captures the money good, promotion tick, and the FULL Mengerian emergence
///    runtime (the saleability tracker's accumulated per-candidate acceptances and
///    acceptor/counterpart sets, plus the promotion-timing latch) — so two runs
///    stay in lockstep tick by tick across the phase transition, and a different
///    seed (different drawn cultures) diverges.
#[test]
fn barter_camp_run_is_deterministic() {
    let config = camp();

    let mut a = Settlement::generate(0xC0FFEE, &config);
    let mut b = Settlement::generate(0xC0FFEE, &config);
    a.run(RUN_TICKS);
    b.run(RUN_TICKS);
    assert_eq!(
        a.canonical_bytes(),
        b.canonical_bytes(),
        "same seed diverged"
    );
    assert_eq!(a.digest(), b.digest());
    // The run actually crossed the phase transition (so the determinism claim
    // spans barter → promotion → money, not just a quiet barter prefix).
    assert!(
        a.promoted_at_tick().is_some(),
        "the determinism run never promoted"
    );

    // Tick-by-tick lockstep across the whole transition.
    let mut x = Settlement::generate(7, &config);
    let mut y = Settlement::generate(7, &config);
    for tick in 0..RUN_TICKS {
        x.econ_tick();
        y.econ_tick();
        assert_eq!(x.digest(), y.digest(), "drifted at econ tick {tick}");
    }

    // A different seed yields a different run (generation draws cultures from the
    // `Rng`, which steer the dynamics).
    let mut c = Settlement::generate(0xBADF00D, &config);
    c.run(RUN_TICKS);
    assert_ne!(a.digest(), c.digest(), "the seed did not matter");
}

/// 2. `money_emerges_from_spatial_barter` — in `barter-camp` a money good is
///    promoted from realized spatial barter: `current_money_good` transitions from
///    `None` to the winner at a definite tick, the barter that drove it was real
///    (a nonzero realized barter volume preceded promotion), the emerged good is a
///    durable medium (NOT a gathered node good — the world never mints it), and
///    afterward trade is money-priced.
#[test]
fn money_emerges_from_spatial_barter() {
    let mut s = Settlement::generate(2_026, &camp());

    // Starts in barter — no money assumed.
    assert!(
        s.is_emergent(),
        "the barter camp must run the emergent path"
    );
    assert!(s.in_barter_phase(), "must start in barter");
    assert_eq!(s.current_money_good(), None, "no money at tick 0");

    let mut promotion_tick = None;
    let mut barter_before_promotion = 0usize;
    for tick in 0..RUN_TICKS {
        let was_barter = s.current_money_good().is_none();
        s.econ_tick();
        if was_barter && s.current_money_good().is_some() {
            promotion_tick = Some(tick);
            barter_before_promotion = s.barter_trade_count();
            break;
        }
    }

    let promotion_tick = promotion_tick.expect("no money good ever emerged in the barter camp");
    // The promotion was driven by REALIZED SPATIAL BARTER, not seeded — barter
    // trades cleared before the money good emerged.
    assert!(
        barter_before_promotion > 0,
        "money emerged with no realized barter behind it"
    );
    // The emerged good is the durable SALT medium, never a gathered node good (the
    // world keeps minting node goods; a money good the world re-mints would break
    // the conserved promotion).
    let money = s
        .current_money_good()
        .expect("a money good is set after promotion");
    assert_eq!(money, SALT, "the emergent medium (SALT) is the money good");
    assert_eq!(
        s.promoted_at_tick(),
        Some(promotion_tick),
        "the recorded promotion tick matches the observed transition"
    );
    assert!(
        !s.tracked_goods().contains(&money) || s.econ_stock_total(money) == 0,
        "the promoted medium's stock fully converted to money"
    );

    // Afterward trade is money-priced: run on and a realized money price appears.
    s.run(RUN_TICKS - promotion_tick);
    assert!(
        s.realized_price(FOOD).is_some(),
        "no money price realized after promotion — trade is not money-priced"
    );
}

/// 3. `no_surplus_control_does_not_monetize` — the falsification twin. With the
///    medium's supply removed (no SALT endowed) and so no saleability differential,
///    the SAME emergence machinery runs over the SAME FOOD/WOOD barter every tick
///    but NO good is ever promoted and the settlement stays in barter. Paired with
///    test 2 this isolates the cause: the saleable medium, not luck.
#[test]
fn no_surplus_control_does_not_monetize() {
    let mut s = Settlement::generate(2_026, &control());
    assert!(
        s.is_emergent(),
        "the control must run the SAME emergent machinery"
    );

    for tick in 0..RUN_TICKS {
        s.econ_tick();
        assert_eq!(
            s.current_money_good(),
            None,
            "a good monetized in the no-surplus control at tick {tick}"
        );
        assert!(
            s.in_barter_phase(),
            "the control left barter at tick {tick}"
        );
    }
    assert_eq!(
        s.promoted_at_tick(),
        None,
        "the control promoted a money good"
    );

    // The control is NOT vacuous: the same FOOD/WOOD barter genuinely ran and was
    // observed by the (reused) tracker — it simply never produced a winner because
    // the reciprocal FOOD-for-WOOD trade structure is symmetric.
    assert!(
        s.barter_trade_count() > 0,
        "the control never bartered — the falsification is vacuous"
    );
    let food = s
        .saleability_bps(FOOD)
        .expect("the tracker observed FOOD barter");
    let wood = s
        .saleability_bps(WOOD)
        .expect("the tracker observed WOOD barter");
    assert!(
        food > 0 && wood > 0,
        "the control's tracker saw no realized barter to judge"
    );
    // No money price ever forms — the settlement never reaches the money market.
    assert!(
        s.realized_price(FOOD).is_none(),
        "the control realized a money price despite never monetizing"
    );
}

/// 4. `barter_and_promotion_conserve` — every barter swap is a conserved
///    relocation and the promotion converts the winning good's stock to money units
///    EXACTLY (the lab's conserved promotion), so whole-system conservation holds
///    across the phase transition. For every tracked good the whole-system total
///    moves by exactly the report's ledger every tick; total money is constant
///    EXCEPT at the promotion tick, where it rises by precisely the units of the
///    promoted good that left the physical ledger. The proof is non-vacuous (a
///    promotion really occurred).
#[test]
fn barter_and_promotion_conserve() {
    let mut s = Settlement::generate(2_026, &camp());
    let goods: Vec<GoodId> = s.tracked_goods().to_vec();

    let mut prev: Vec<u64> = goods.iter().map(|&g| s.whole_system_total(g)).collect();
    let mut prev_gold = s.total_gold();
    let mut promotions = 0u32;

    for tick in 0..RUN_TICKS {
        let report = s.econ_tick();
        assert!(
            report.conserves(),
            "report ledger unbalanced at tick {tick}"
        );

        // Per-good whole-system identity (now including the G5a promotion sink).
        for (i, &good) in goods.iter().enumerate() {
            let after = s.whole_system_total(good);
            let before = prev[i] as i128;
            let regen = report.regen_of(good) as i128;
            let consumed = report.consumed_of(good) as i128;
            let promoted = report.promoted_of(good) as i128;
            assert_eq!(
                after as i128,
                before + regen - consumed - promoted,
                "{good:?} conservation broke at tick {tick}"
            );
            prev[i] = after;
        }

        // Money conservation: a barter swap relocates goods and never mints money,
        // so total gold is constant — except on the promotion tick, where it rises
        // by EXACTLY the promoted good's converted stock (good→money, 1-for-1).
        let gold = s.total_gold();
        let minted: u64 = report.promoted.values().sum();
        assert_eq!(
            gold.0,
            prev_gold.0 + minted,
            "money conservation broke at tick {tick} (minted {minted})"
        );
        if minted > 0 {
            promotions += 1;
            // The promotion converted the winning good's whole-system stock to
            // money exactly: the units that left the physical ledger equal the
            // money minted, and the good's stock is now zero.
            let (&winner, &units) = report
                .promoted
                .iter()
                .next()
                .expect("a promotion records its good");
            assert_eq!(units, minted, "more than one good promoted at once");
            assert_eq!(
                s.econ_stock_total(winner),
                0,
                "the promoted good's stock did not fully convert"
            );
        }
        prev_gold = gold;
    }

    assert_eq!(
        promotions, 1,
        "the conservation proof is vacuous — no single promotion occurred"
    );
}

/// 5. `promotion_transitions_to_money_market` — after promotion the settlement runs
///    the existing G2b money-priced market: realized money prices appear and the
///    barter book goes quiescent (no further barter clears, no live barter offers).
#[test]
fn promotion_transitions_to_money_market() {
    let mut s = Settlement::generate(2_026, &camp());

    // Advance to just past promotion.
    let mut promotion_tick = None;
    for tick in 0..RUN_TICKS {
        let was_barter = s.current_money_good().is_none();
        s.econ_tick();
        if was_barter && s.current_money_good().is_some() {
            promotion_tick = Some(tick);
            break;
        }
    }
    assert!(promotion_tick.is_some(), "the camp never promoted");

    let barter_at_promotion = s.barter_trade_count();
    let spot_at_promotion = s.society().trades.len();

    // Run the money-priced phase.
    s.run(20);

    // Money prices realize (the spot market clears).
    assert!(
        s.realized_price(FOOD).is_some() || s.realized_price(WOOD).is_some(),
        "no money price realized after promotion"
    );
    assert!(
        s.society().trades.len() > spot_at_promotion,
        "the money market cleared no trades after promotion"
    );
    // The barter book is quiescent: no further barter clears and no offer rests.
    assert_eq!(
        s.barter_trade_count(),
        barter_at_promotion,
        "barter kept clearing after the money good emerged"
    );
    assert_eq!(
        s.society().live_barter_offer_count(),
        0,
        "the barter book still holds offers after promotion"
    );
}

/// 6. `emergence_reuses_the_lab_rule` — the promotion decision routes through econ's
///    reused `MengerianEmergence` (the V2 path inside `Society::step`), not a
///    sim-local reimplementation, and the adopted M20 envelope/config is used. The
///    sim adds NO emergence rule: it only wires realized spatial barter into the
///    tracker and lets the lab's `winner` rule fire.
#[test]
fn emergence_reuses_the_lab_rule() {
    let config = camp();
    // The camp's Mengerian envelope is the adopted M20 default thresholds, only the
    // candidate goods name this camp's tradeable set — reused from econ, no
    // sim-local thresholds.
    let envelope = config
        .barter
        .as_ref()
        .expect("the camp carries a barter overlay")
        .menger
        .clone();
    let m20 = MengerianConfig::default();
    assert_eq!(envelope.min_total_acceptances, m20.min_total_acceptances);
    assert_eq!(
        envelope.promotion_threshold_bps,
        m20.promotion_threshold_bps
    );
    assert_eq!(envelope.lead_margin_bps, m20.lead_margin_bps);
    assert_eq!(envelope.min_acceptor_agents, m20.min_acceptor_agents);
    assert_eq!(envelope.min_counterpart_goods, m20.min_counterpart_goods);
    assert_eq!(envelope.stability_ticks, m20.stability_ticks);
    assert_eq!(
        envelope.indirect_min_acceptance_share_bps,
        m20.indirect_min_acceptance_share_bps
    );

    let mut s = Settlement::generate(2_026, &camp());
    // The running settlement drives econ's emergence state object (the V2 path),
    // and the config it runs under is exactly the adopted envelope above.
    assert!(
        s.society().emergence().is_some(),
        "the settlement does not run econ's MengerianEmergence"
    );
    assert_eq!(
        s.mengerian_config(),
        Some(&envelope),
        "the running emergence uses a config other than the reused envelope"
    );

    s.run(RUN_TICKS);
    let emergence = s.society().emergence().expect("emergent settlement");
    let money = s.current_money_good().expect("a money good emerged");
    // The good econ's reused machinery promoted is the saleability winner — the
    // most-saleable good by the lab's rule. `tracker().winner(config)` is the very
    // function the V2 step calls; replaying it on the realized tally reproduces the
    // promoted good (the tracker keeps the pre-promotion tally), proving the
    // decision is the lab's, not a sim re-implementation.
    assert_eq!(
        emergence.tracker().winner(emergence.config()),
        Some(money),
        "the promoted good is not the lab winner rule's choice"
    );
}

/// 7. `econ_unchanged` — the engine's conformance scenarios still replay
///    byte-identically (the six econ goldens are untouched: G5a's only econ edits
///    are additive accessors and a consumption-log capture in `step_v2` that is
///    inert unless the log is enabled, which the goldens never do), and a plain G2b
///    settlement is byte-identical with or without the (defaulted-`None`) barter
///    overlay. The full `cargo test --workspace`, `cargo clippy -- -D warnings`, and
///    `cargo fmt --check` run outside this test.
#[test]
fn econ_unchanged() {
    use econ::scenario::{builtin_market_scenario, ScenarioName};
    use econ::society::Society;

    for name in [
        ScenarioName::MarketBarterishGold,
        ScenarioName::MarketPriceDiscovery,
        ScenarioName::MengerSaltMoney,
        ScenarioName::MengerGoldMoney,
    ] {
        let scenario = builtin_market_scenario(name);
        let periods = scenario.periods;

        let mut first = Society::from_scenario(scenario);
        let total_gold = first.total_gold();
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
        if matches!(name, ScenarioName::MarketBarterishGold) {
            assert_eq!(
                first.total_gold(),
                total_gold,
                "{name:?} broke gold conservation"
            );
        }
    }

    // A plain settlement is byte-identical to one with an explicitly-absent barter
    // overlay — the additive G5a state never moves a non-emergent digest.
    let plain = Settlement::generate(7, &SettlementConfig::viable());
    let mut explicit = SettlementConfig::viable();
    explicit.barter = None;
    let explicit = Settlement::generate(7, &explicit);
    assert_eq!(plain.digest(), explicit.digest());
    assert!(!plain.is_emergent(), "a viable settlement is not emergent");
}

/// Unit: the control is the barter camp with the medium's supply removed — the SAME
/// roster, nodes, and reused emergence envelope, only the SALT endowment (and so the
/// near "hold the medium" demand it supports) differs. This is what makes test 2 /
/// test 3 a clean falsification twin: identical machinery and raw input supply, the
/// saleable medium's presence the only causal difference.
#[test]
fn control_is_the_camp_without_the_medium() {
    let camp = SettlementConfig::barter_camp();
    let control = SettlementConfig::barter_camp_control();

    // Same world and roster.
    assert_eq!(camp.nodes, control.nodes, "the raw nodes must match");
    assert_eq!(camp.gatherers, control.gatherers);
    assert_eq!(camp.consumers, control.consumers);

    let cb = camp.barter.as_ref().expect("camp barter overlay");
    let kb = control.barter.as_ref().expect("control barter overlay");
    // The reused emergence envelope and the medium identity are identical — the
    // control runs the SAME machinery over the SAME candidate goods.
    assert_eq!(cb.menger, kb.menger, "the emergence envelope must match");
    assert_eq!(cb.medium_good, kb.medium_good);

    // The only causal difference: the camp supplies the medium, the control does not.
    assert!(
        cb.gatherer_medium_endowment + cb.consumer_medium_endowment > 0,
        "the camp must endow the circulating medium"
    );
    assert_eq!(
        kb.gatherer_medium_endowment + kb.consumer_medium_endowment,
        0,
        "the control must endow NO medium (no saleability differential)"
    );
}

/// Unit: a barter camp is a plain gatherer/consumer camp — no production chain and
/// no demography (the G5a mechanism slice). Composition with the full stack is G5b.
#[test]
fn barter_camp_is_a_plain_camp() {
    let config = SettlementConfig::barter_camp();
    assert!(
        config.chain.is_none(),
        "G5a is not composed with production"
    );
    assert!(
        config.demography.is_none(),
        "G5a is not composed with demography"
    );
    assert!(
        config.barter.is_some(),
        "the camp carries the barter overlay"
    );
    // Zero starting money: the camp holds no money before promotion (the econ V2
    // path requires zero initial money balances).
    assert_eq!(config.starting_gold_gatherer, 0);
    assert_eq!(config.starting_gold_consumer, 0);

    // The roster is only gatherers and consumers (no producer/latent vocations).
    let s = Settlement::generate(1, &config);
    for index in 0..s.population() {
        let vocation = s.vocation_of(index).expect("a colonist resolves");
        assert!(
            matches!(vocation, Vocation::Gatherer | Vocation::Consumer),
            "barter camp colonist {index} has a non-camp vocation {vocation:?}"
        );
    }
}

/// Unit: the conserved promotion is good→money 1-for-1 over the WHOLE roster. At the
/// promotion tick the report records exactly the units removed from physical stock,
/// and the gold minted across all colonists equals those units — no money created or
/// destroyed in the conversion.
#[test]
fn promotion_mints_money_one_for_one() {
    let mut s = Settlement::generate(2_026, &camp());
    let gold_before = s.total_gold();
    let salt_before = s.whole_system_total(SALT);

    let mut minted = None;
    for _ in 0..RUN_TICKS {
        let report = s.econ_tick();
        if !report.promoted.is_empty() {
            minted = Some(report.promoted_of(SALT));
            break;
        }
    }
    let minted = minted.expect("the camp promoted");

    assert!(minted > 0, "the promotion converted no stock");
    // Gold rose by exactly the converted SALT; SALT physical stock fell by the same.
    assert_eq!(
        s.total_gold(),
        Gold(gold_before.0 + minted),
        "gold minted does not equal the converted stock"
    );
    assert_eq!(
        s.whole_system_total(SALT),
        salt_before - minted,
        "the converted SALT did not leave the physical ledger 1-for-1"
    );
    assert_eq!(s.econ_stock_total(SALT), 0, "residual SALT after promotion");
}

/// Unit: G5a supports a spatial medium that is not regenerated by the world. The
/// tracker may observe gathered goods as candidates, but if a custom envelope would
/// actually promote one, the settlement must veto the transition and stay in
/// barter rather than let future node output create physical units of the money
/// good.
#[test]
fn gathered_good_promotion_is_rejected_without_panicking() {
    let mut config = camp();
    let barter = config.barter.as_mut().expect("barter overlay");
    barter.menger.candidate_goods = vec![FOOD];
    barter.menger.min_total_acceptances = 1;
    barter.menger.promotion_threshold_bps = 1;
    barter.menger.lead_margin_bps = 0;
    barter.menger.min_acceptor_agents = 1;
    barter.menger.min_counterpart_goods = 1;
    barter.menger.stability_ticks = 1;

    let mut s = Settlement::generate(2_026, &config);
    for tick in 0..RUN_TICKS {
        let report = s.econ_tick();
        assert!(report.conserves(), "ledger broke at tick {tick}");
        assert!(
            report.promoted.is_empty(),
            "a gathered good was converted to money at tick {tick}"
        );
        assert_eq!(
            s.current_money_good(),
            None,
            "a gathered good became money at tick {tick}"
        );
        assert!(
            s.in_barter_phase(),
            "the settlement left barter at tick {tick}"
        );
    }

    assert!(
        s.society().v2_promotion_failures.iter().any(|failure| {
            failure.money_good == FOOD
                && failure.reason == V2PromotionFailureReason::UnsupportedMoneyGood
        }),
        "the unsupported FOOD promotion was not recorded"
    );
    assert_eq!(
        s.promoted_at_tick(),
        None,
        "a vetoed gathered-good promotion still latched"
    );
}
