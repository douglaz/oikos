//! G1 acceptance suite — needs → wants (the `life` crate).
//!
//! Mechanism-only and pre-spatial (game-spec §11): these assert scale-generation
//! *properties* and non-collapse, never balance numbers. The first test that
//! pins a specific price or count is out of scope (G2+ tuning).

use econ::agent::{AgentId, WantKind};
use econ::good::{Horizon, FOOD, WOOD};
use life::{
    regenerate_scale, Camp, CampEnv, CultureParams, KnownGoods, NeedDynamics, NeedState,
    TICKS_PER_YEAR,
};

/// A tiny pure LCG so the property tests sweep many cases deterministically
/// without pulling in any RNG dependency.
struct Lcg(u64);

impl Lcg {
    fn next(&mut self) -> u64 {
        self.0 = self
            .0
            .wrapping_mul(6_364_136_223_846_793_005)
            .wrapping_add(1_442_695_040_888_963_407);
        self.0 >> 16
    }

    fn need(&mut self, max: u16) -> u16 {
        (self.next() % u64::from(max + 1)) as u16
    }

    fn bps(&mut self) -> u16 {
        (self.next() % 10_001) as u16
    }
}

fn culture(time_preference_bps: u16, leisure_weight_bps: u16) -> CultureParams {
    CultureParams::new(time_preference_bps, leisure_weight_bps)
}

/// Rank (index) of the first want of `kind`, or `usize::MAX` when the kind is
/// absent. Treating "absent" as the lowest possible rank keeps the monotonicity
/// comparison well-defined: gaining a want (MAX → some index) is never a drop.
fn rank_of(scale: &[econ::agent::Want], kind: WantKind) -> usize {
    scale
        .iter()
        .position(|want| want.kind == kind)
        .unwrap_or(usize::MAX)
}

/// 1. Increasing any one need's depletion never lowers that need's want rank.
#[test]
fn scale_is_satiation_monotone() {
    let known = KnownGoods::lab_default();
    let mut lcg = Lcg(0x5EED_1234);
    let need_max = 12u16;

    for _ in 0..4_000 {
        let base = NeedState::new(
            lcg.need(need_max - 1),
            lcg.need(need_max - 1),
            lcg.need(need_max - 1),
        );
        let cul = culture(lcg.bps(), lcg.bps());

        // hunger ↔ FOOD
        let lower = regenerate_scale(&base, &cul, &known);
        let mut hungrier = base;
        hungrier.hunger += 1;
        let higher = regenerate_scale(&hungrier, &cul, &known);
        assert!(
            rank_of(&higher, WantKind::Good(FOOD)) <= rank_of(&lower, WantKind::Good(FOOD)),
            "raising hunger lowered the food rank: {base:?} {cul:?}"
        );

        // warmth ↔ WOOD
        let mut colder = base;
        colder.warmth += 1;
        let higher = regenerate_scale(&colder, &cul, &known);
        assert!(
            rank_of(&higher, WantKind::Good(WOOD)) <= rank_of(&lower, WantKind::Good(WOOD)),
            "raising warmth lowered the wood rank: {base:?} {cul:?}"
        );

        // rest ↔ Leisure
        let mut wearier = base;
        wearier.rest += 1;
        let higher = regenerate_scale(&wearier, &cul, &known);
        assert!(
            rank_of(&higher, WantKind::Leisure) <= rank_of(&lower, WantKind::Leisure),
            "raising rest deficit lowered the leisure rank: {base:?} {cul:?}"
        );
    }
}

/// 2. Every generated scale contains a Leisure want, at every need configuration.
#[test]
fn scale_always_contains_leisure() {
    let known = KnownGoods::lab_default();
    let mut lcg = Lcg(0xA11CE);
    for _ in 0..4_000 {
        let needs = NeedState::new(lcg.need(12), lcg.need(12), lcg.need(12));
        let cul = culture(lcg.bps(), lcg.bps());
        let scale = regenerate_scale(&needs, &cul, &known);
        assert!(
            scale.iter().any(|want| want.kind == WantKind::Leisure),
            "scale has no leisure want: {needs:?} {cul:?}"
        );
    }
}

/// 3. No input produces an empty scale.
#[test]
fn scale_is_never_empty() {
    let known = KnownGoods::lab_default();
    let mut lcg = Lcg(0xE3E3);
    // Fully satisfied is the hardest case (no present consumption wants).
    assert!(
        !regenerate_scale(&NeedState::rested(), &CultureParams::lab_default(), &known).is_empty()
    );
    for _ in 0..4_000 {
        let needs = NeedState::new(lcg.need(12), lcg.need(12), lcg.need(12));
        let cul = culture(lcg.bps(), lcg.bps());
        assert!(!regenerate_scale(&needs, &cul, &known).is_empty());
    }
}

/// 4. Identical inputs → identical output (run twice, byte-equal).
#[test]
fn scale_generation_is_deterministic() {
    let known = KnownGoods::lab_default();
    let mut lcg = Lcg(0xD37E);
    for _ in 0..4_000 {
        let needs = NeedState::new(lcg.need(12), lcg.need(12), lcg.need(12));
        let cul = culture(lcg.bps(), lcg.bps());
        let a = regenerate_scale(&needs, &cul, &known);
        let b = regenerate_scale(&needs, &cul, &known);
        assert_eq!(a, b, "non-deterministic for {needs:?} {cul:?}");
    }
}

/// 5. A need with capacity for multiple units lists them at strictly descending
///    ranks (later units rank below earlier ones; no cardinal number used).
#[test]
fn diminishing_marginal_utility_is_positional() {
    let known = KnownGoods::lab_default();
    // A deep hunger deficit emits several present FOOD units.
    let scale = regenerate_scale(
        &NeedState::new(4, 0, 0),
        &CultureParams::lab_default(),
        &known,
    );
    let food_now_ranks: Vec<usize> = scale
        .iter()
        .enumerate()
        .filter(|(_, want)| want.kind == WantKind::Good(FOOD) && want.horizon == Horizon::Now)
        .map(|(index, _)| index)
        .collect();
    assert!(
        food_now_ranks.len() >= 2,
        "a deep deficit must emit multiple marginal units"
    );
    for pair in food_now_ranks.windows(2) {
        assert!(
            pair[0] < pair[1],
            "successive marginal units must rank strictly lower"
        );
    }
}

/// 6. Raising `time_preference_bps` moves `Later` wants down relative to `Now`
///    (monotone), holding needs fixed.
#[test]
fn time_preference_orders_horizons() {
    let known = KnownGoods::lab_default();
    let needs = NeedState::new(3, 3, 2);
    let mut previous = 0usize;
    for &tpb in &[0u16, 2_000, 4_000, 6_000, 8_000, 10_000] {
        let scale = regenerate_scale(&needs, &culture(tpb, 3_000), &known);
        let first_later = scale
            .iter()
            .position(|want| matches!(want.horizon, Horizon::Later(_)))
            .expect("a future-provisioning want is always present");
        assert!(
            first_later >= previous,
            "raising time preference must not raise a Later want's rank (tpb={tpb})"
        );
        previous = first_later;
    }
    // And it strictly sinks across the full range.
    let patient = regenerate_scale(&needs, &culture(0, 3_000), &known);
    let present = regenerate_scale(&needs, &culture(10_000, 3_000), &known);
    let patient_later = patient
        .iter()
        .position(|w| matches!(w.horizon, Horizon::Later(_)))
        .unwrap();
    let present_later = present
        .iter()
        .position(|w| matches!(w.horizon, Horizon::Later(_)))
        .unwrap();
    assert!(present_later > patient_later);
}

/// 7. At low rest-depletion Leisure ranks below goods (the colonist will work to
///    provision); at high rest-depletion Leisure outranks goods (it rests). The
///    emergent labor-supply proof, at the scale level.
#[test]
fn rested_colonist_works_exhausted_colonist_rests() {
    let known = KnownGoods::lab_default();
    let cul = CultureParams::lab_default();

    // Rested but hungry/cold: leisure ranks below the goods → it works.
    let working = regenerate_scale(&NeedState::new(5, 5, 0), &cul, &known);
    let leisure = rank_of(&working, WantKind::Leisure);
    assert!(leisure > rank_of(&working, WantKind::Good(FOOD)));
    assert!(leisure > rank_of(&working, WantKind::Good(WOOD)));

    // Exhausted and barely hungry/cold: leisure outranks the goods → it rests.
    let resting = regenerate_scale(&NeedState::new(1, 1, 10), &cul, &known);
    let leisure = rank_of(&resting, WantKind::Leisure);
    assert!(leisure < rank_of(&resting, WantKind::Good(FOOD)));
    assert!(leisure < rank_of(&resting, WantKind::Good(WOOD)));
}

/// 8. A camp of 50 run five "years" (60 ticks/year → 300 ticks): no panic;
///    living count stays > 0 throughout; the camp settles into a sustainable
///    equilibrium where the viable endowment holds survivors' needs strictly off
///    the ceiling; deterministic across two runs. Smoke test only — survival and
///    boundedness, not prices or counts.
///
/// Note on the bootstrap: a *closed* viable camp (woodcutters can only obtain
/// food by selling wood to foragers, who can also cut their own) may lose some
/// colonists in the first year while the market discovers prices, then settles.
/// The non-collapse property asserted here is that it SETTLES — the survivors
/// stay alive and bounded indefinitely — not that nobody ever starves. Pinning a
/// survivor count or price would be G2+ balance tuning, out of scope (game-spec
/// §11).
#[test]
fn camp_of_50_does_not_collapse() {
    let env = CampEnv::viable();
    let dynamics = NeedDynamics::lab_default();
    // One year to bootstrap, then assert the steady state over the remaining four.
    let warmup = TICKS_PER_YEAR as usize;

    let mut camp = Camp::generate(20_260_613, 50, &env);
    let mut settled_living: Option<usize> = None;
    for tick in 0..5 * warmup {
        camp.step();

        // Survival: the camp never collapses to zero living colonists.
        assert!(camp.living_count() > 0, "camp collapsed to zero living");

        if tick == warmup {
            settled_living = Some(camp.living_count());
        }
        if let Some(settled) = settled_living {
            // Endowment-driven boundedness (NOT the clamp): `NeedState::advance`
            // clamps every need AT `need_max`, so `<= need_max` is vacuous — a
            // survivor pegged at the ceiling would satisfy it. Asserting a strict
            // `< need_max` once the camp has settled actually proves the viable
            // endowment keeps survivors off the ceiling. (During the bootstrap,
            // before `warmup`, living colonists do briefly reach `need_max`, so
            // this would fail if applied too early — exactly why it is meaningful.)
            assert!(
                camp.max_living_need() < dynamics.need_max,
                "a settled colonist's need pegged at the ceiling: {} (tick {tick})",
                camp.max_living_need()
            );
            // The settled camp does not keep bleeding out: no further deaths once
            // it reaches equilibrium (asserts stability, not a specific count).
            assert_eq!(
                camp.living_count(),
                settled,
                "the settled camp kept losing colonists (tick {tick})"
            );
        }
    }
    assert!(
        settled_living.is_some(),
        "the camp must reach its settled equilibrium within the run"
    );

    // Determinism: a second run from the same seed matches.
    let mut twin = Camp::generate(20_260_613, 50, &env);
    twin.run(5 * warmup as u64);
    assert_eq!(camp.digest(), twin.digest(), "camp is not deterministic");
}

/// 9. A colonist cut off from FOOD crosses the death threshold, is tombstoned
///    (marked dead, posts no orders, excluded from living count); the arena slot
///    is NOT freed and total conservation still balances (frozen holdings
///    included).
#[test]
fn starvation_kills_via_tombstone() {
    let env = CampEnv::starved();
    let mut camp = Camp::generate(3, 6, &env);
    let population = camp.population();
    let initial_gold = camp.total_gold();

    let mut observed_death = false;
    for _ in 0..80 {
        camp.step();

        // Gold is conserved every tick — including any tombstoned colonist's
        // frozen balance, since its slot is never freed.
        assert_eq!(
            camp.total_gold(),
            initial_gold,
            "conservation broke (frozen holdings must stay counted)"
        );
        // The arena is never shrunk: a tombstone is not an arena free.
        assert_eq!(
            camp.society().agents.len(),
            population,
            "tombstone must not free the arena slot"
        );

        if camp.living_count() < population {
            observed_death = true;
            // Every dead colonist has an empty scale (so it posts no orders) and
            // is excluded from the living count.
            for index in 0..population {
                if !camp.is_alive(index) {
                    let dead = camp
                        .society()
                        .agents
                        .get(AgentId(index as u64))
                        .expect("tombstoned colonist still resolves in the arena");
                    assert!(dead.scale.is_empty(), "a dead colonist must post no orders");
                }
            }
        }
    }

    assert!(observed_death, "a food-starved colonist must die");
    assert!(
        camp.living_count() < population,
        "living count must exclude the dead"
    );
}

/// 10. Run a camp to a settled stretch where food trades clear at a flat price,
///     then cut the FOOD harvest; once the camp's buffers deplete the realized
///     FOOD price settles to a STRICTLY higher level and stays there (SIGN only,
///     no magnitude). The market responding to scarcity the needs created.
///
/// The assertion is deliberately a *persistent-elevation* one, not a transient
/// max: it samples a late post-shock window and requires the realized price to
/// sit above the settled pre-shock level for the whole window. A one-tick blip
/// from ordinary book fluctuation cannot clear a whole-window floor — only a
/// sustained scarcity response can — so this isolates the shock's effect rather
/// than catching noise (which a `max`-over-window comparison against a single
/// pre-shock sample would not).
#[test]
fn harvest_shock_raises_food_price() {
    let mut camp = Camp::generate(7, 40, &CampEnv::shockable());

    // Run to a pre-shock stretch and confirm the food price has SETTLED: it is
    // flat over the final pre-shock ticks, so `before` is a genuine settled
    // level rather than a sample taken mid-swing.
    camp.run(15);
    let before = camp
        .realized_food_price()
        .expect("food trades before the shock establish a price");
    for _ in 0..5 {
        camp.step();
        assert_eq!(
            camp.realized_food_price(),
            Some(before),
            "the pre-shock food price must be settled (flat) before supply is cut"
        );
    }

    // Cut the FOOD harvest, then let the buffers deplete and the book re-price.
    camp.set_food_flow(1);
    camp.run(110);

    // Sample a late window: the realized price must sit strictly above the
    // settled pre-shock level for EVERY tick of it (the window floor, not a peak).
    let mut window_floor = None;
    for _ in 0..20 {
        camp.step();
        if let Some(price) = camp.realized_food_price() {
            window_floor = Some(window_floor.map_or(price, |floor| price.min(floor)));
        }
    }
    let settled_after = window_floor.expect("food keeps trading after the shock");

    assert!(
        settled_after > before,
        "scarcity must raise the realized food price and keep it raised: \
         before={before:?} settled_after={settled_after:?}"
    );
    assert!(camp.living_count() > 0, "the camp must not vanish entirely");
}

/// 11. The econ engine still runs and replays deterministically from `life`'s
///     workspace, with conservation intact — `life` added no econ behavior
///     change. (The byte-identical golden guarantee is the econ conformance
///     suite itself, run by `cargo test` across the workspace; this checks the
///     engine is usable and unperturbed from here.)
#[test]
fn econ_goldens_unchanged() {
    use econ::scenario::{builtin_market_scenario, ScenarioName};
    use econ::society::Society;

    for name in [
        ScenarioName::MarketBarterishGold,
        ScenarioName::MarketPriceDiscovery,
        ScenarioName::MengerSaltMoney,
    ] {
        let scenario = builtin_market_scenario(name);
        let periods = scenario.periods;

        let mut first = Society::from_scenario(scenario);
        let total_gold = first.total_gold();
        first.run(periods);

        let mut second = Society::from_scenario(builtin_market_scenario(name));
        second.run(periods);

        // Deterministic replay: the per-tick market records match byte-for-byte.
        assert_eq!(
            first.records, second.records,
            "{name:?} did not replay deterministically"
        );
        // Designated-money scenarios conserve gold across the run.
        if matches!(name, ScenarioName::MarketBarterishGold) {
            assert_eq!(
                first.total_gold(),
                total_gold,
                "{name:?} broke gold conservation"
            );
        }
    }
}
