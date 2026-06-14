//! G4b acceptance suite — demography: births, aging, households, culture inheritance.
//!
//! G4a gave the engine real death (runtime removal, estate, cache reconciliation).
//! G4b completes demography: colonists **age**, **die of old age** (via G4a's
//! removal path), are **born** into **households** when the household can support
//! them, and children **inherit** their parents' [`CultureParams`] with bounded,
//! deterministic mutation — so time preference drifts under selection across
//! generations. This is the first milestone where the population is not a fixed
//! cast.
//!
//! These pin the mechanism + the curated demonstrations (the multi-seed
//! stability/selection STUDIES are deferred, per `docs/impl-g4b.md`):
//! - the run is deterministic with births and deaths (test 1);
//! - `Society::add_agent` reconciles every cache so a newborn participates (test 2);
//! - the population sustains in a band — births ≈ deaths, no extinction/blowup (test 3);
//! - an aged colonist dies through `remove_agent`, its estate settling (test 4);
//! - a child's culture is its parent's, mutated within the bound, deterministically (test 5);
//! - an estate routes to the household heirs, the commons only if extinct (test 6);
//! - a patient lineage out-accumulates a present-biased one — SIGN only (test 7);
//! - the econ market is unperturbed and the no-demography path is byte-identical (test 8).

use econ::good::{Gold, GOLD};
use sim::{
    DemographyConfig, HouseholdSpec, Region, RegionConfig, Route, Settlement, SettlementConfig,
    FOOD, WOOD,
};

/// The curated two-lineage config (patient household 0, present-biased household 1).
fn lineages() -> SettlementConfig {
    SettlementConfig::lineages()
}

/// The patient lineage's per-field culture mutation bound at birth (for test 5).
fn mutation_delta() -> u16 {
    DemographyConfig::lineages().mutation_delta_bps
}

/// Step until the first econ tick that records a birth, asserting whole-system
/// conservation every tick. Returns that tick index, or `None` within `max_ticks`.
fn run_to_first_birth(s: &mut Settlement, max_ticks: u64) -> Option<u64> {
    for t in 0..max_ticks {
        let report = s.econ_tick();
        assert!(report.conserves(), "conservation broke at econ tick {t}");
        if report.births > 0 {
            return Some(t);
        }
    }
    None
}

/// 1. `demography_run_is_deterministic` — same `(seed, config)` → byte-identical run
///    with births and deaths (deterministic mutation; nothing drawn in the loop).
#[test]
fn demography_run_is_deterministic() {
    let cfg = lineages();
    let mut a = Settlement::generate(7, &cfg);
    let mut b = Settlement::generate(7, &cfg);
    a.run(160);
    b.run(160);

    // The run actually exercised births AND old-age deaths (otherwise vacuous).
    assert!(a.births_total() > 0, "the run must include births");
    assert!(
        a.old_age_deaths_total() > 0,
        "the run must include old-age deaths"
    );

    assert_eq!(
        a.canonical_bytes(),
        b.canonical_bytes(),
        "a demography run diverged"
    );
    assert_eq!(a.digest(), b.digest());
    assert_eq!(a.births_total(), b.births_total());
    assert_eq!(a.old_age_deaths_total(), b.old_age_deaths_total());
    for h in 0..a.household_count() {
        assert_eq!(
            a.lineage_gold(h),
            b.lineage_gold(h),
            "lineage gold diverged"
        );
        assert_eq!(a.lineage_living_count(h), b.lineage_living_count(h));
    }

    // Tick-by-tick lockstep: the digest matches at every tick through the births
    // and deaths (the rebuild order is fixed, the mutation is hashed, not drawn).
    let mut x = Settlement::generate(7, &cfg);
    let mut y = Settlement::generate(7, &cfg);
    for tick in 0..160 {
        x.econ_tick();
        y.econ_tick();
        assert_eq!(
            x.digest(),
            y.digest(),
            "a demography run drifted at econ tick {tick}"
        );
    }
}

#[test]
fn demography_digest_includes_future_steering_knobs() {
    let base = lineages();
    let mut different_birth_interval = lineages();
    different_birth_interval
        .demography
        .as_mut()
        .expect("lineages has demography")
        .birth_interval += 1;
    let mut different_provision = lineages();
    different_provision
        .demography
        .as_mut()
        .expect("lineages has demography")
        .households[0]
        .food_provision += 1;

    let base = Settlement::generate(7, &base);
    let changed_birth = Settlement::generate(7, &different_birth_interval);
    let changed_provision = Settlement::generate(7, &different_provision);

    assert_ne!(
        base.canonical_bytes(),
        changed_birth.canonical_bytes(),
        "birth cadence config must enter the deterministic state"
    );
    assert_ne!(
        base.canonical_bytes(),
        changed_provision.canonical_bytes(),
        "household provision config must enter the deterministic state"
    );
}

/// 2. `add_agent_reconciles_caches` — a birth's new colonist appears in the activation
///    set and participates (bids/asks) the next tick; no cache omits it; its endowment
///    is a conserved transfer (not a mint).
#[test]
fn add_agent_reconciles_caches() {
    let cfg = lineages();
    let mut s = Settlement::generate(7, &cfg);

    let founders = s.population();
    let gold_before = s.total_gold();
    let birth_tick = run_to_first_birth(&mut s, 40).expect("a household must birth");

    // A birth created at least one new colonist beyond the founders.
    assert!(s.population() > founders, "a newborn must be added");
    // Conservation: the endowment is a TRANSFER (parent → child), so the closed gold
    // total is unchanged across the birth tick — add_agent minted nothing.
    assert_eq!(
        s.total_gold(),
        gold_before,
        "the birth's endowment is a transfer, not a mint"
    );

    // Every newborn (a colonist beyond the founder roster) resolves in the arena, is
    // in the live activation set the engine iterates, and holds no stale reservation —
    // proof that add_agent reconciled the arena, agent_order, and the reservations.
    let newborns: Vec<_> = (founders..s.population())
        .filter(|&i| s.is_alive(i))
        .map(|i| s.colonist_id(i).unwrap())
        .collect();
    assert!(!newborns.is_empty(), "at least one newborn is alive");
    for &id in &newborns {
        assert!(
            s.society().agents.get(id).is_some(),
            "a newborn resolves in the arena"
        );
        assert!(
            s.society().agents.iter().any(|a| a.id == id),
            "a newborn is in the live activation set"
        );
        assert_eq!(
            s.society().reservations.reserved_gold(id),
            Gold::ZERO,
            "a fresh newborn holds no stale reservation"
        );
    }

    // Participation: within a few more ticks a newborn actually bids/asks — it appears
    // as a buyer or seller in the trade tape (only an agent the caches reconciled into
    // the market can post an order that clears).
    let mut traded = false;
    for _ in 0..12 {
        let trades_start = s.society().trades.len();
        let report = s.econ_tick();
        assert!(report.conserves(), "a post-birth tick broke conservation");
        for trade in &s.society().trades[trades_start..] {
            if newborns.contains(&trade.buyer) || newborns.contains(&trade.seller) {
                traded = true;
            }
        }
    }
    assert!(
        traded,
        "a newborn never traded — a cache that add_agent missed would keep it out of the market"
    );
    assert!(birth_tick < 40);
}

/// 3. `population_sustains_without_collapse` — a viable config runs many econ-years with
///    births ≈ deaths: the population stays in a band (no extinction, no unbounded
///    blowup), deterministically. Smoke/sign, not a tuned number.
#[test]
fn population_sustains_without_collapse() {
    let cfg = lineages();
    let mut s = Settlement::generate(0xC0FFEE, &cfg);

    // The structural blowup bound: a household never births past its size cap, so the
    // whole colony is capped at households × max_household_size.
    let demo = DemographyConfig::lineages();
    let cap = demo.households.len() * usize::from(demo.max_household_size);

    let mut min_after_warmup = usize::MAX;
    let mut max_pop = 0;
    for t in 0..300 {
        let report = s.econ_tick();
        assert!(report.conserves(), "conservation broke at econ tick {t}");
        let pop = s.living_total();
        assert!(pop >= 1, "the colony went extinct at tick {t}");
        assert!(pop <= cap, "the population blew past the cap at tick {t}");
        max_pop = max_pop.max(pop);
        if t >= 60 {
            min_after_warmup = min_after_warmup.min(pop);
            // No lineage goes extinct: both households keep at least one member.
            for h in 0..s.household_count() {
                assert!(
                    s.lineage_living_count(h) >= 1,
                    "lineage {h} went extinct at tick {t}"
                );
            }
        }
    }

    // Births and deaths both happened and roughly balanced (the population stayed in a
    // band rather than running away or collapsing). Sign/smoke, not a tuned number.
    assert!(s.births_total() > 0, "no births occurred");
    assert!(s.old_age_deaths_total() > 0, "no old-age deaths occurred");
    assert!(min_after_warmup >= 1, "the colony collapsed after warmup");
    assert!(max_pop <= cap);

    // Deterministic: the same seed reproduces the band exactly.
    let mut twin = Settlement::generate(0xC0FFEE, &cfg);
    twin.run(300);
    assert_eq!(
        s.digest(),
        twin.digest(),
        "the sustaining run is not deterministic"
    );
}

/// 4. `old_age_death_routes_through_removal` — an aged colonist dies via `remove_agent`;
///    its slot frees (its id resolves `None`) and its estate settles (to the household);
///    conservation holds across the death.
#[test]
fn old_age_death_routes_through_removal() {
    let cfg = lineages();
    let mut s = Settlement::generate(7, &cfg);

    let gold_total = s.total_gold();
    // Step until the first old-age death, capturing the colonist that died this tick.
    let mut died: Option<usize> = None;
    for t in 0..200 {
        let alive_before: Vec<bool> = (0..s.population()).map(|i| s.is_alive(i)).collect();
        let old_age_before = s.old_age_deaths_total();
        let report = s.econ_tick();
        assert!(report.conserves(), "conservation broke at econ tick {t}");
        // Gold is a closed balance: a death (estate → heir or commons) conserves it.
        assert_eq!(
            s.total_gold(),
            gold_total,
            "a death broke gold conservation at tick {t}"
        );
        if s.old_age_deaths_total() > old_age_before {
            died = (0..alive_before.len()).find(|&i| alive_before[i] && !s.is_alive(i));
            break;
        }
    }
    let died = died.expect("an old-age death must occur");

    // It died of OLD AGE (its age reached its lifespan), not starvation.
    assert!(
        s.age_of(died).unwrap() >= s.lifespan_of(died).unwrap(),
        "the colonist reached its lifespan"
    );
    // Real removal: its id resolves to None (the arena slot was freed via remove_agent).
    let dead_id = s.colonist_id(died).unwrap();
    assert!(
        s.society().agents.get(dead_id).is_none(),
        "the dead colonist's id resolves to None (the slot was freed)"
    );
    // Its estate settled to the household (a survivor inherited it): the lineage is not
    // extinct, so the commons stays empty — the estate went to heirs, not the commons.
    assert_eq!(
        s.commons_gold(),
        Gold::ZERO,
        "a non-extinct lineage's estate routes to heirs, not the commons"
    );

    // The run continues conserving for several more ticks after the death.
    for _ in 0..12 {
        let report = s.econ_tick();
        assert!(report.conserves(), "post-death conservation broke");
        assert_eq!(s.total_gold(), gold_total);
    }
}

/// 5. `child_inherits_mutated_culture` — a child's `CultureParams` equal a parent's
///    within the bounded mutation delta, and the mutation is deterministic (same birth
///    → same child params).
#[test]
fn child_inherits_mutated_culture() {
    let cfg = lineages();
    let founders = DemographyConfig::lineages().founder_count();
    let mut s = Settlement::generate(7, &cfg);
    run_to_first_birth(&mut s, 40).expect("a household must birth");

    // The first newborn descends from a FOUNDER of its household (no founder has died
    // by the first birth), so its culture is that parent's, mutated within the bound.
    let child_index = founders; // the first colonist appended after the founders
    let child = s.culture_of(child_index).expect("the first newborn exists");
    let household = s
        .household_of(child_index)
        .expect("a newborn has a household");
    let delta = i32::from(mutation_delta());

    let within_bound = (0..founders).any(|i| {
        if s.household_of(i) != Some(household) {
            return false;
        }
        let parent = s.culture_of(i).unwrap();
        let dtp = i32::from(child.time_preference_bps) - i32::from(parent.time_preference_bps);
        let dlw = i32::from(child.leisure_weight_bps) - i32::from(parent.leisure_weight_bps);
        dtp.abs() <= delta && dlw.abs() <= delta
    });
    assert!(
        within_bound,
        "the child's culture must be a founder parent's, mutated within {delta} bps"
    );

    // Determinism: the same seed reproduces the identical child culture.
    let mut twin = Settlement::generate(7, &cfg);
    run_to_first_birth(&mut twin, 40).expect("the twin must birth");
    assert_eq!(
        twin.culture_of(child_index),
        Some(child),
        "the same birth must yield the same child culture"
    );
}

/// 6. `estate_routes_to_household_then_commons` — a death's estate goes to the household
///    heirs; if the household is extinct it falls back to the commons; conserved either
///    way.
#[test]
fn estate_routes_to_household_then_commons() {
    // --- Heir path: in the two-lineage config a member dies with survivors, so the
    // estate routes to a living heir and the commons stays empty (conserved). ---
    let cfg = lineages();
    let mut s = Settlement::generate(7, &cfg);
    let gold_total = s.total_gold();
    let mut saw_old_age_death = false;
    for _ in 0..160 {
        let report = s.econ_tick();
        assert!(report.conserves());
        assert_eq!(
            s.total_gold(),
            gold_total,
            "an estate transfer broke conservation"
        );
        if s.old_age_deaths_total() > 0 {
            saw_old_age_death = true;
        }
    }
    assert!(saw_old_age_death, "a member must have died");
    // Neither lineage went extinct, so every estate went to heirs — the commons is empty.
    assert!(s.lineage_living_count(0) >= 1 && s.lineage_living_count(1) >= 1);
    assert_eq!(
        s.commons_gold(),
        Gold::ZERO,
        "estates routed to heirs leave the commons empty"
    );

    // --- Commons fallback: a single-member household that cannot reproduce goes
    // extinct when its lone founder dies of old age → its estate falls back to the
    // commons, conserved. ---
    let mut single = lineages();
    let starting_gold = 20;
    single.demography = Some(DemographyConfig {
        households: vec![HouseholdSpec {
            founders: 1,
            time_preference_base_bps: 500,
            food_provision: 2,
            wood_provision: 3,
            starting_gold,
            starting_food: 6,
            starting_wood: 4,
        }],
        // A one-member size cap means the household can never birth a replacement.
        max_household_size: 1,
        ..DemographyConfig::lineages()
    });
    let mut s = Settlement::generate(1, &single);
    let gold_total = s.total_gold();
    let mut extinct = false;
    for _ in 0..200 {
        let report = s.econ_tick();
        assert!(report.conserves());
        assert_eq!(
            s.total_gold(),
            gold_total,
            "the commons fallback broke conservation"
        );
        if s.lineage_living_count(0) == 0 {
            extinct = true;
            break;
        }
    }
    assert!(
        extinct,
        "the lone founder must die of old age, leaving the lineage extinct"
    );
    assert!(
        s.commons_gold() > Gold::ZERO,
        "an extinct lineage's estate falls back to the commons"
    );
    // Conserved either way: the closed gold total never moved.
    assert_eq!(s.total_gold(), gold_total);
}

/// 7. `patient_lineage_outaccumulates_impatient` — on the curated two-lineage config the
///    patient household's lineage ends with more accumulated gold than the present-biased
///    one (SIGN only). The selection result.
#[test]
fn patient_lineage_outaccumulates_impatient() {
    let cfg = lineages();
    let mut s = Settlement::generate(7, &cfg);
    for _ in 0..250 {
        let report = s.econ_tick();
        assert!(report.conserves());
    }
    // Household 0 is the patient lineage (a high saving target → it keeps offering its
    // wood surplus and accumulates gold); household 1 is present-biased (it spends its
    // gold down buying warmth). The patient lineage ends richer — sign only.
    let patient = s.lineage_gold(0);
    let impatient = s.lineage_gold(1);
    assert!(
        patient > impatient,
        "the patient lineage ({patient}) must out-accumulate the present-biased one ({impatient})"
    );
}

/// A composed [`Region`] whose settlements run the demography overlay must roll the
/// per-member **provision** (`EconTickReport::endowment`) into the region-wide
/// conservation report — it is a source like node regen. Without that roll-up the
/// region would gain provisioned FOOD/WOOD each tick and trip the regional
/// `conserves()` invariant even though every settlement conserves locally. This pins
/// the regional accounting of the G4b household hearth (and that caravan transfers
/// stay net-zero alongside births and deaths).
#[test]
fn region_rolls_up_demography_endowment() {
    let config = RegionConfig {
        settlement_a: SettlementConfig::lineages(),
        settlement_b: SettlementConfig::lineages(),
        route: Route { transit_ticks: 1 },
        good: FOOD,
        trader_gold: 64,
        buy_ticks: 4,
        sell_ticks: 4,
        caravans_enabled: true,
    };
    let mut region = Region::generate(7, &config);

    let mut endowment_seen = 0u64;
    for t in 0..160 {
        let report = region.econ_tick();
        assert!(
            report.conserves(),
            "region-wide conservation broke at econ tick {t} (demography endowment unaccounted?)"
        );
        endowment_seen += report.endowment_of(FOOD) + report.endowment_of(WOOD);
    }
    // The new term was actually exercised — both lineages provision FOOD each tick and
    // the patient one provisions WOOD — so this is not a vacuous pass.
    assert!(
        endowment_seen > 0,
        "the region never rolled up a demography provision (the term is untested)"
    );
}

/// 8. `econ_unchanged` — the no-demography path is byte-identical (the demography
///    additions are inert when there is no overlay), and a demography run keeps the
///    econ invariants (gold conserved, reservations within holdings). The six econ
///    goldens staying byte-identical and the full workspace suite + `cargo clippy
///    --workspace --all-targets -- -D warnings` + `cargo fmt --check` are the real gate
///    (enforced across the workspace); this checks the local seam.
#[test]
fn econ_unchanged() {
    // A no-demography settlement is byte-identical to a twin — the engine still replays
    // deterministically with the G4b additions present but unexercised — and runs no
    // demography (no households, no births/deaths-of-old-age, an empty commons).
    let plain = SettlementConfig::viable();
    let mut a = Settlement::generate(42, &plain);
    let mut b = Settlement::generate(42, &plain);
    a.run(30);
    b.run(30);
    assert_eq!(a.canonical_bytes(), b.canonical_bytes());
    assert!(!a.is_demographic(), "viable runs no demography");
    assert_eq!(a.household_count(), 0);
    assert_eq!(a.births_total(), 0);
    assert_eq!(a.old_age_deaths_total(), 0);
    assert_eq!(
        a.commons_gold(),
        Gold::ZERO,
        "a no-demography run pools no estate"
    );

    // A demography run keeps the econ invariants tick over tick: the closed gold balance
    // is conserved (society + commons) and no live agent over-commits its holdings.
    let mut d = Settlement::generate(3, &lineages());
    let gold_total = d.total_gold();
    for _ in 0..120 {
        let report = d.econ_tick();
        assert!(report.conserves());
        assert_eq!(
            d.total_gold(),
            gold_total,
            "a birth/death broke gold conservation"
        );
        for agent in d.society().agents.iter() {
            assert!(
                d.society().reservations.reserved_gold(agent.id) <= agent.gold,
                "a live agent's reserved gold exceeds its balance"
            );
        }
    }
    // GOLD is money, never physical commons stock.
    assert_eq!(
        d.commons_stock_of(GOLD),
        0,
        "GOLD is money, not commons stock"
    );
}
