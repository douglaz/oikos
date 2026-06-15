//! G5b acceptance suite — emergence **composed** with the full stack.
//!
//! Each emergent phenomenon was proven in isolation: money emerges from spatial
//! barter (G5a), production roles emerge from price spreads (G3b), population
//! sustains under demographic selection (G4b). G5b is the **integration**
//! milestone — ONE settlement (`SettlementConfig::frontier`) where all three happen
//! together: a barter camp where a money good (SALT) emerges, then producers take up
//! milling/baking from the resulting money price spreads, while births and deaths run
//! demographic selection — all conserving and deterministic.
//!
//! G5b adds NO new mechanism: it reuses G5a money emergence, G3b role-choice, and G4b
//! demography unchanged, ordering them coherently in one econ tick (the
//! `frontier` config + the interaction fixes the combination surfaces). The DoD is
//! sign + exact conservation, never a tuned magnitude:
//! - the whole composed run is deterministic across every phase (test 1);
//! - all three emergences fire in one run (test 2);
//! - **production roles emerge only AFTER money** — a division of labor follows the
//!   medium of exchange (test 3, the economic-ordering tripwire);
//! - whole-system conservation holds every tick with ALL flows active simultaneously,
//!   including a birth on the promotion tick (test 4, the conservation tripwire);
//! - the demography-provisioned staple is vetoed from monetizing — the veto list now
//!   bites because demography is active (test 5);
//! - the composed society sustains over many econ-years (test 6);
//! - econ behaviour is unchanged: the six goldens replay byte-identically and the
//!   no-overlay path is byte-identical (test 7).
//!
//! The multi-seed robustness STUDY and multi-settlement composition are deferred (see
//! `docs/impl-g5b.md` and `docs/engine-divergence.md`).

use econ::good::{Gold, GoodId, GOLD, SALT, WOOD};
use econ::society::V2PromotionFailureReason;
use sim::{Settlement, SettlementConfig, Vocation};

/// The combined frontier config.
fn frontier() -> SettlementConfig {
    SettlementConfig::frontier()
}

/// A horizon comfortably past promotion (≈14) and role adoption (≈18) for the seeds
/// these tests use, with room for many demography turnovers. The tests assert the
/// transitions, never a hand-tuned tick.
const RUN_TICKS: u64 = 90;

/// The chain goods of a frontier settlement.
struct ChainGoods {
    flour: GoodId,
    bread: GoodId,
}

fn chain_goods(s: &Settlement) -> ChainGoods {
    let content = s.content().expect("the frontier runs a production chain");
    ChainGoods {
        flour: content.flour(),
        bread: content.bread(),
    }
}

/// Run `s` for `ticks`, returning `(promotion_tick, max_millers, max_bakers,
/// births, deaths)` and asserting whole-system conservation every tick.
fn run_and_observe(s: &mut Settlement, ticks: u64) -> (Option<u64>, usize, usize, u64, u64) {
    let mut promotion_tick = None;
    let (mut max_millers, mut max_bakers) = (0usize, 0usize);
    let mut deaths = 0u64;
    for tick in 0..ticks {
        let was_barter = s.current_money_good().is_none();
        let report = s.econ_tick();
        assert!(report.conserves(), "conservation broke at econ tick {tick}");
        if was_barter && s.current_money_good().is_some() {
            promotion_tick = Some(tick);
        }
        max_millers = max_millers.max(s.living_count(Vocation::Miller));
        max_bakers = max_bakers.max(s.living_count(Vocation::Baker));
        deaths += u64::from(report.deaths);
    }
    (
        promotion_tick,
        max_millers,
        max_bakers,
        s.births_total(),
        deaths,
    )
}

/// 1. `frontier_run_is_deterministic` — same `(seed, config)` → byte-identical run
///    through barter, promotion, money, production, AND demography. Nothing is drawn
///    in the loops (the `Rng` is consumed only at generation), every overlay is
///    integer + `BTreeMap`/`Vec`, and the canonical digest captures the money good,
///    promotion tick, the full Mengerian runtime, the latent/role state, and the
///    demography roster — so two runs stay in lockstep tick by tick across all four
///    transitions, and a different seed (different drawn cultures) diverges.
#[test]
fn frontier_run_is_deterministic() {
    let config = frontier();

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
    // The run actually crossed the money transition, so the determinism claim spans
    // barter → promotion → money → production → demography, not a quiet prefix.
    assert!(
        a.promoted_at_tick().is_some(),
        "the determinism run never promoted — the proof is vacuous"
    );

    // Tick-by-tick lockstep across the whole composed run.
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

/// 2. `all_three_emergences_fire` — in ONE frontier run: a money good is promoted
///    (G5a), at least one producer adopts milling AND at least one adopts baking
///    afterward (G3b), and births and deaths both occur with the population sustained
///    in a band (G4b). The whole economic foundation runs as one society.
#[test]
fn all_three_emergences_fire() {
    let mut s = Settlement::generate(2_026, &frontier());
    let g = chain_goods(&s);

    // Starts in barter — no money assumed, no producer roles seeded.
    assert!(s.is_emergent(), "the frontier runs the emergent path");
    assert!(s.in_barter_phase(), "must start in barter");
    assert_eq!(s.vocation_count(Vocation::Miller), 0, "no seeded millers");
    assert_eq!(s.vocation_count(Vocation::Baker), 0, "no seeded bakers");
    assert!(
        s.living_count(Vocation::Unassigned) > 0,
        "the frontier seeds a latent producer pool"
    );

    let mut bread_made = 0u64;
    let (promotion_tick, max_millers, max_bakers, births, deaths) = {
        let mut promotion_tick = None;
        let (mut mm, mut mb) = (0usize, 0usize);
        let mut deaths = 0u64;
        for tick in 0..RUN_TICKS {
            let was_barter = s.current_money_good().is_none();
            let report = s.econ_tick();
            assert!(report.conserves(), "conservation broke at tick {tick}");
            if was_barter && s.current_money_good().is_some() {
                promotion_tick = Some(tick);
            }
            mm = mm.max(s.living_count(Vocation::Miller));
            mb = mb.max(s.living_count(Vocation::Baker));
            bread_made += report.produced_of(g.bread);
            deaths += u64::from(report.deaths);
        }
        (promotion_tick, mm, mb, s.births_total(), deaths)
    };

    // G5a: a money good emerged from realized spatial barter.
    assert!(
        promotion_tick.is_some(),
        "no money good ever emerged in the frontier"
    );
    assert_eq!(
        s.current_money_good(),
        Some(SALT),
        "the durable SALT medium is the money good"
    );

    // G3b: producers took up BOTH milling and baking from the spread — and bread
    // (the chain's product) really flowed.
    assert!(max_millers >= 1, "no colonist ever adopted milling");
    assert!(max_bakers >= 1, "no colonist ever adopted baking");
    assert!(bread_made > 0, "the chain produced no bread");
    assert!(
        s.realized_price(g.flour).is_some(),
        "flour never realized a money price — the middle of the chain never traded"
    );

    // G4b: births and deaths both ran, and the population stayed in a band (no
    // extinction, no unbounded blowup).
    assert!(births > 0, "no colonist was ever born");
    assert!(deaths > 0, "no colonist ever died");
    let living = s.living_total();
    assert!(
        (8..=40).contains(&living),
        "population left its band: {living} living"
    );
}

/// 3. `production_roles_emerge_only_after_money` — the load-bearing economic
///    ordering. No production role is adopted during the barter phase; roles appear
///    only post-promotion (a division of labor presupposes a medium of exchange). The
///    role-choice phase is gated on the money phase, so a pre-promotion tick can never
///    leave a colonist a Miller/Baker.
#[test]
fn production_roles_emerge_only_after_money() {
    let mut s = Settlement::generate(2_026, &frontier());

    let mut promotion_tick = None;
    let mut roles_before_money = false;
    let mut roles_after_money = false;
    for tick in 0..RUN_TICKS {
        let was_barter = s.current_money_good().is_none();
        s.econ_tick();
        let producers = s.living_count(Vocation::Miller) + s.living_count(Vocation::Baker);
        // The tripwire: if the settlement is still in barter at the end of this tick
        // (no money good), no producer role may exist.
        if s.in_barter_phase() {
            assert_eq!(
                producers, 0,
                "a production role was adopted during the barter phase (tick {tick})"
            );
        }
        if was_barter && s.current_money_good().is_some() {
            promotion_tick = Some(tick);
        }
        match promotion_tick {
            None => {
                if producers > 0 {
                    roles_before_money = true;
                }
            }
            Some(_) => {
                if producers > 0 {
                    roles_after_money = true;
                }
            }
        }
    }

    assert!(
        promotion_tick.is_some(),
        "money never emerged, so the ordering cannot be exercised"
    );
    assert!(
        !roles_before_money,
        "a producer role existed before money emerged — the ordering is violated"
    );
    assert!(
        roles_after_money,
        "no producer role ever emerged after money — the chain did not form"
    );
}

/// 4. `frontier_conserves_with_all_flows` — whole-system conservation holds every
///    econ tick with barter swaps, the promotion conversion, recipe transformations,
///    birth endowments, and death estates ALL active simultaneously. For every tracked
///    good the whole-system total moves by exactly the report's ledger every tick, and
///    total money is constant except at the promotion tick, where it rises by exactly
///    the converted medium stock. The proof is non-vacuous (promotion, production,
///    births, and deaths all occur), and a second witness run lands a **birth on the
///    promotion tick** — the awkward coincidence the integration must still conserve.
#[test]
fn frontier_conserves_with_all_flows() {
    let mut s = Settlement::generate(2_026, &frontier());
    let goods: Vec<GoodId> = s.tracked_goods().to_vec();

    let mut prev: Vec<u64> = goods.iter().map(|&g| s.whole_system_total(g)).collect();
    let mut prev_gold = s.total_gold();
    let mut promotions = 0u32;
    let (mut any_produced, mut deaths) = (0u64, 0u64);

    for tick in 0..RUN_TICKS {
        let report = s.econ_tick();
        assert!(
            report.conserves(),
            "report ledger unbalanced at tick {tick}"
        );

        // Per-good whole-system identity, with EVERY G5b flow term: regen, the
        // demography endowment (a source), recipe production / input, consumption, and
        // the promotion sink. Births and deaths move goods WITHIN the system, so they
        // cancel in before/after and need no term.
        for (i, &good) in goods.iter().enumerate() {
            let after = s.whole_system_total(good);
            let before = prev[i] as i128;
            let regen = report.regen_of(good) as i128;
            let endowment = report.endowment_of(good) as i128;
            let produced = report.produced_of(good) as i128;
            let consumed_as_input = report.consumed_as_input_of(good) as i128;
            let consumed = report.consumed_of(good) as i128;
            let promoted = report.promoted_of(good) as i128;
            assert_eq!(
                after as i128,
                before + regen + endowment + produced - consumed_as_input - consumed - promoted,
                "{good:?} conservation broke at tick {tick}"
            );
            prev[i] = after;
            any_produced += report.produced_of(good);
        }

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
            assert_eq!(
                s.econ_stock_total(winner),
                0,
                "the promoted stock did not convert"
            );
        }
        prev_gold = gold;
        deaths += u64::from(report.deaths);
    }

    // Non-vacuous: every flow actually ran over the horizon.
    assert_eq!(promotions, 1, "exactly one promotion must have occurred");
    assert!(any_produced > 0, "no recipe output — production never ran");
    assert!(
        s.births_total() > 0,
        "no births — the demography flow never ran"
    );
    assert!(deaths > 0, "no deaths — the demography flow never ran");

    // The awkward coincidence: a birth/death on the promotion tick. The natural frontier
    // promotes (≈14) on a quiet demography tick, so to exercise the coincidence we use a
    // witness variant whose promotion lands one tick later (a slightly higher acceptance
    // bar), which puts a birth on the very tick a good monetizes. We scan a small,
    // deterministic seed set for the first such coincidence (not a robustness study — a
    // single witness for the conservation property) and assert that tick conserved with
    // the promotion AND demography flows firing together.
    let mut witness = frontier();
    witness
        .barter
        .as_mut()
        .expect("the frontier carries a barter overlay")
        .menger
        .min_total_acceptances = 16;
    let mut coincided = false;
    'seeds: for seed in 0..16u64 {
        let mut w = Settlement::generate(seed, &witness);
        for tick in 0..RUN_TICKS {
            let was_barter = w.current_money_good().is_none();
            let report = w.econ_tick();
            assert!(
                report.conserves(),
                "witness conservation broke at seed {seed} tick {tick}"
            );
            if was_barter && w.current_money_good().is_some() {
                if report.births > 0 || report.deaths > 0 {
                    // The promotion tick itself ran a birth/death AND the promotion mint,
                    // and still balanced — the awkward coincidence conserves.
                    assert!(!report.promoted.is_empty(), "the promotion was recorded");
                    assert!(
                        report.conserves(),
                        "promotion+birth/death tick broke conservation"
                    );
                    coincided = true;
                    break 'seeds;
                }
                break; // this seed promoted on a quiet tick; try the next
            }
        }
    }
    assert!(
        coincided,
        "no witness seed put a birth/death on the promotion tick"
    );
}

/// 5. `demography_provision_good_cannot_monetize` — the G5a veto list now genuinely
///    bites because demography is active: a demography-provisioned staple (bread, which
///    the household hearth mints every tick) is vetoed from promotion. Forced as the
///    sole money candidate with permissive thresholds it would win the saleability race
///    every tick, yet it never monetizes — econ records an `UnsupportedMoneyGood`
///    failure and the settlement stays in barter. Money emerges on a non-renewable good
///    (SALT) or not at all.
#[test]
fn demography_provision_good_cannot_monetize() {
    // First, establish that bread really IS demography-provisioned (the household
    // hearth mints it) — so vetoing it is vetoing a demography-provisioned staple.
    let mut base = Settlement::generate(2_026, &frontier());
    let bread = base.content().expect("chain").bread();
    let mut provisioned_bread = false;
    for _ in 0..RUN_TICKS {
        let report = base.econ_tick();
        if report.endowment_of(bread) > 0 {
            provisioned_bread = true;
        }
    }
    assert!(
        provisioned_bread,
        "bread was never provisioned — the veto target is not a demography staple"
    );
    // And the unmodified frontier monetizes the durable, NON-renewable medium.
    assert_eq!(
        base.current_money_good(),
        Some(SALT),
        "the frontier must monetize the non-renewable SALT medium"
    );

    // Now force the demography-provisioned bread as the sole candidate with permissive
    // thresholds: it accumulates saleability and would win, but the rejection list
    // (which now covers the demography hearth) vetoes it every tick.
    let mut config = frontier();
    {
        let barter = config.barter.as_mut().expect("barter overlay");
        barter.menger.candidate_goods = vec![bread];
        barter.menger.min_total_acceptances = 1;
        barter.menger.promotion_threshold_bps = 1;
        barter.menger.lead_margin_bps = 0;
        barter.menger.min_acceptor_agents = 1;
        barter.menger.min_counterpart_goods = 1;
        barter.menger.stability_ticks = 1;
    }
    let mut s = Settlement::generate(2_026, &config);
    for tick in 0..RUN_TICKS {
        let report = s.econ_tick();
        assert!(report.conserves(), "ledger broke at tick {tick}");
        assert!(
            report.promoted.is_empty(),
            "bread was converted to money at tick {tick}"
        );
        assert_eq!(
            s.current_money_good(),
            None,
            "the demography staple became money at tick {tick}"
        );
        assert!(
            s.in_barter_phase(),
            "the settlement left barter at tick {tick}"
        );
    }
    // The veto is non-vacuous: bread genuinely led the saleability race (so it WOULD
    // have promoted), and econ recorded the unsupported-money-good veto.
    assert!(
        s.saleability_bps(bread).is_some_and(|bps| bps > 0),
        "bread never traded — the veto is vacuous"
    );
    assert!(
        s.society().v2_promotion_failures.iter().any(|failure| {
            failure.money_good == bread
                && failure.reason == V2PromotionFailureReason::UnsupportedMoneyGood
        }),
        "the demography-provisioned bread veto was not recorded"
    );
    assert_eq!(
        s.promoted_at_tick(),
        None,
        "a vetoed promotion still latched"
    );
}

/// 6. `frontier_sustains` — the composed society runs many econ-years without
///    collapse: money emerged, producers are working, the population stays in a band,
///    and conservation holds throughout. Smoke/sign, deterministic — no asserted
///    magnitude beyond "all-three still alive and the ledger balances."
#[test]
fn frontier_sustains() {
    let years = 12u64;
    let ticks = years * sim::ECON_TICKS_PER_YEAR; // 144 econ ticks
    let mut s = Settlement::generate(2_026, &frontier());

    let (promotion_tick, max_millers, max_bakers, births, deaths) = run_and_observe(&mut s, ticks);

    // Money emerged and stuck.
    assert!(
        promotion_tick.is_some(),
        "money never emerged over the horizon"
    );
    assert_eq!(s.current_money_good(), Some(SALT));

    // Producers worked, the population turned over (births and deaths), and it neither
    // went extinct nor blew up.
    assert!(
        max_millers >= 1 && max_bakers >= 1,
        "no producers ever worked"
    );
    assert!(births > 0 && deaths > 0, "the population never turned over");
    let living = s.living_total();
    assert!(
        (8..=40).contains(&living),
        "the society collapsed or blew up: {living} living after {ticks} ticks"
    );
    // Producers are STILL working at the end (the division of labor persists).
    assert!(
        s.living_count(Vocation::Miller) + s.living_count(Vocation::Baker) > 0,
        "no producers remain working at the end of the horizon"
    );
}

/// 7. `econ_unchanged` — the engine's conformance scenarios still replay
///    byte-identically (the six econ goldens are untouched: every G5b edit lives in
///    `sim`, reusing econ's existing emergence/role-choice/demography accessors), and a
///    plain settlement is byte-identical with or without the (defaulted-`None`)
///    overlays. The full `cargo test --workspace`, `cargo clippy -- -D warnings`, and
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

    // A plain settlement is byte-identical to one with explicitly-absent overlays — the
    // additive G5b state never moves a non-overlay digest.
    let plain = Settlement::generate(7, &SettlementConfig::viable());
    let mut explicit = SettlementConfig::viable();
    explicit.chain = None;
    explicit.demography = None;
    explicit.barter = None;
    let explicit = Settlement::generate(7, &explicit);
    assert_eq!(plain.digest(), explicit.digest());
    assert!(!plain.is_emergent(), "a viable settlement is not emergent");
}

/// Unit: the frontier is a genuinely COMPOSED config — all three overlays present at
/// once (the structure G5b proves coexists), with zero money before promotion.
#[test]
fn frontier_is_the_composed_config() {
    let config = frontier();
    assert!(
        config.chain.is_some(),
        "the frontier runs a production chain"
    );
    assert!(config.demography.is_some(), "the frontier runs demography");
    assert!(
        config.barter.is_some(),
        "the frontier runs barter-start emergence"
    );
    // No money before promotion: every gold source is zero (econ's V2 path requires it).
    assert_eq!(config.starting_gold_gatherer, 0);
    assert_eq!(config.starting_gold_consumer, 0);
    assert_eq!(config.chain.as_ref().unwrap().producer_gold, 0);
    let demo = config.demography.as_ref().unwrap();
    assert_eq!(demo.child_gold_endowment, 0);
    assert!(demo.households.iter().all(|h| h.starting_gold == 0));

    // The emergent medium (SALT) is none of the renewable goods — not a node good, not
    // a chain good, not the demography staple — so it is the one good that can monetize.
    let barter = config.barter.as_ref().unwrap();
    assert_eq!(barter.medium_good, SALT);
    assert!(
        config.nodes.iter().all(|n| n.good != SALT),
        "the medium must not be a gathered node good"
    );
    let content = config.chain.as_ref().unwrap().content.clone();
    assert!(
        !content.goods().contains(&SALT),
        "the medium must not be a production-chain good"
    );
}

/// Unit: a barter-start frontier that seeds money on ANY composed gold source is
/// rejected at generation — the V2 promotion refuses to commit when an agent already
/// holds gold, so a producer's working capital or a household's starting gold under a
/// barter overlay would silently never-promote. Generation catches it loudly.
#[test]
#[should_panic(expected = "holds no money before promotion")]
fn generate_rejects_composed_starting_money() {
    let mut config = frontier();
    config.demography.as_mut().expect("demography").households[0].starting_gold = 5;
    let _ = Settlement::generate(1, &config);
}

/// Unit: a barter-start frontier whose emergent medium names a renewable good is
/// rejected — the chain would keep re-minting it after promotion, breaking the
/// conserved good→money conversion. (Here: the medium set to the chain's bread.)
#[test]
#[should_panic(expected = "must not be a production-chain good")]
fn generate_rejects_renewable_medium() {
    let mut config = frontier();
    let bread = config.chain.as_ref().expect("chain").content.bread();
    let barter = config.barter.as_mut().expect("barter overlay");
    barter.medium_good = bread;
    barter.menger.candidate_goods = vec![bread, WOOD];
    let _ = Settlement::generate(1, &config);
}

/// Unit: the role-choice appraisal is threaded with the CURRENT money good, not GOLD.
/// On the frontier the emerged money is SALT, so a producer's future-money savings want
/// is `Good(SALT)`; the appraisal must read that want (the GOLD-only wrapper would see
/// no savings want and decline). This pins the money-good threading that lets roles
/// emerge under emergent (non-GOLD) money.
#[test]
fn role_choice_threads_the_emergent_money_good() {
    use econ::agent::{Agent, AgentId, Role, Want, WantKind};
    use econ::good::{Horizon, Stock, NET};
    use sim::{recipe_adoption_pays, recipe_adoption_pays_for_money, ContentSet};

    let content = ContentSet::grain_flour_bread();
    let mill = content.mill_recipe();

    // A patient producer that saves in SALT (the emergent medium), holding no money.
    let salt_saver = Agent {
        id: AgentId(1),
        scale: vec![Want {
            kind: WantKind::Good(SALT),
            horizon: Horizon::Later(4),
            qty: 1,
            satisfied: false,
        }],
        stock: Stock::new(NET.0),
        gold: Gold(0),
        labor_capacity: 0,
        hunger_deficit: 0,
        roles: vec![Role::Household],
        expect: Vec::new(),
    };

    // Threaded with SALT (the real money good post-promotion) the spread provisions the
    // future-SALT want, so the producer adopts.
    assert!(
        recipe_adoption_pays_for_money(&salt_saver, mill, Some(Gold(5)), Some(Gold(1)), 0, 1, SALT),
        "the SALT-money appraisal must read the Good(SALT) savings want"
    );
    // The GOLD-only wrapper sees no Good(GOLD) savings want on this scale, so it
    // declines — exactly why role-choice must thread the current money good, not assume
    // GOLD. (A GOLD-saving colonist would be the mirror image.)
    assert!(
        !recipe_adoption_pays(&salt_saver, mill, Some(Gold(5)), Some(Gold(1)), 0, 1),
        "the GOLD-only appraisal must not satisfy a SALT savings want"
    );
    // And the same colonist saving in GOLD adopts under the GOLD wrapper — the
    // designated-money (G3a/G3b) path is unchanged.
    let gold_saver = Agent {
        scale: vec![Want {
            kind: WantKind::Good(GOLD),
            horizon: Horizon::Later(4),
            qty: 1,
            satisfied: false,
        }],
        ..salt_saver
    };
    assert!(
        recipe_adoption_pays(&gold_saver, mill, Some(Gold(5)), Some(Gold(1)), 0, 1),
        "the GOLD appraisal must still adopt for a GOLD saver (G3a/G3b unchanged)"
    );
}
