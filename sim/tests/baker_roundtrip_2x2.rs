//! C3R.h cut 2: Baker round-trip telemetry and the base/L2/L1/L1+L2 experiment.
//!
//! **Measured result — `EITHER_SUFFICES` with a negative interaction (5/5 seeds).**
//! The v1 acceptance ("baker-origin bread SOLD ≥ N") was MISCALIBRATED: on this base bread
//! is the food *staple* (`bread_is_staple`, `mod.rs:1025`), so a functioning chain's output
//! is EATEN, not sold for gold — measuring gold-sales of a staple made every arm look like a
//! null. The right lens is: does the chain sustainably FEED the colony (staff + produce) and
//! stay SOLVENT (the baker class does not bleed its gold to zero)?
//!
//! Under that lens (verified with a 4,000-tick solvency probe):
//! - **base**: bakers collapse (`living_bakers = 0`) — fails.
//! - **L2** (`stale_input_price_fix`): 9 bakers, ~12,000 loaves/run, bread eaten; the baker
//!   class runs gold-LEAN but SOLVENT — its gold FLOORS at a low positive steady state
//!   (~10-220) and never depletes; production holds to the end. Passes.
//! - **L1** (retire mints + raw-grain subsistence): 3 bakers, ~4,000 loaves, and now bread
//!   actually SELLS (round trip cash-POSITIVE, +948..+1,781). Passes.
//! - **L1+L2**: COLLAPSES (`living_bakers = 0`, 27 loaves) — a real NEGATIVE INTERACTION.
//!
//! So EITHER lever alone makes the mortal-producer chain function and sustain; combining
//! them collapses it. Cut 1's `STALE-PRICE-SUFFICES` reading survives as one half of EITHER.
//! The gold POOLS in the millers (~4,000) while bakers run lean — a monetary-distribution
//! feature (bread eaten not sold back), reported, not a sustainability failure.
//!
//! The assertions pin EITHER + the negative interaction. An economy change that makes a
//! single lever stop sustaining, or that lets L1+L2 sustain, will FAIL this suite and force
//! a re-read of `docs/impl-final-stage-demand.md`.

use sim::settlement::BakerRoundTrip;
use sim::{Settlement, SettlementConfig, Vocation};

const SEEDS: [u64; 5] = [3, 7, 11, 19, 23];
const RUN_TICKS: u64 = 1_600;
const FINAL_WINDOW: u64 = 160;
/// A functioning baker produces sustainably: this many loaves in the final window.
/// L2 makes ~1,800; a collapsed stage (base, L1+L2) makes ~0.
const PRODUCE_FLOOR: u64 = 300;
/// Long-horizon solvency check: 1.5× the base run. The "loss-making" L2 baker's gold FLOORS
/// well before this (steady by ~tick 2,000 in the 4,000-tick probe), so a positive balance
/// here proves it does not deplete rather than merely "was positive at 1,600".
const SOLVENCY_TICKS: u64 = 2_400;
const MORTAL_SMOKE_SEED: u64 = 3;
const MORTAL_SMOKE_TICKS: u64 = 400;
/// The pinned base gives every household six producer houses at the tail.
const PRODUCER_HOUSES: usize = 6;

/// `l1` retires food mints and raw-grain subsistence; `l2` fixes stale input prices.
#[derive(Clone, Copy, PartialEq, Eq)]
struct Arm {
    label: &'static str,
    l1: bool,
    l2: bool,
}

const BASE: Arm = Arm {
    label: "base",
    l1: false,
    l2: false,
};
const L2: Arm = Arm {
    label: "L2",
    l1: false,
    l2: true,
};
const L1: Arm = Arm {
    label: "L1",
    l1: true,
    l2: false,
};
const L1L2: Arm = Arm {
    label: "L1+L2",
    l1: true,
    l2: true,
};

/// The pinned immortal-producer control, identical to cut 1's base.
fn config(arm: Arm) -> SettlementConfig {
    let mut cfg = SettlementConfig::frontier_mortal_producers_heritable();
    let households = &mut cfg
        .demography
        .as_mut()
        .expect("heritable demography")
        .households;
    let producer_start = households
        .len()
        .checked_sub(PRODUCER_HOUSES)
        .expect("producer houses");
    for house in &mut households[producer_start..] {
        house.food_provision = 0;
    }

    let chain = cfg.chain.as_mut().expect("chain");
    chain.producer_house_cap = 2;
    chain.mortal_producer_tool_inheritance = true;
    // The pinned immortal control. Its inherited `hunger_critical = need_max + 1`
    // (`mod.rs:3670`) disables starvation colony-wide, so a hunger BOUND is vacuous here:
    // `hunger` clamps at `need_max` and nothing dies of it. The window max is reported as
    // evidence, not asserted; `mortal_l2_smoke` below exercises a reachable ceiling.
    chain.mortal_chain_producers = false;
    chain.mortal_producer_inheritance = false;

    if arm.l1 {
        chain.retire_food_mints = true;
        chain.subsistence_on_grain = false;
    }
    chain.stale_input_price_fix = arm.l2;
    cfg
}

/// Total gold held by the currently-Baker colonist class — the solvency signal. The baker
/// class buys flour and (mostly) eats its bread, so this drifts DOWN under L2; the question
/// is whether it FLOORS above zero (solvent) or depletes to zero (the chain stalls).
fn baker_class_gold(s: &Settlement) -> u64 {
    let mut gold = 0u64;
    for idx in 0..s.population() {
        if s.is_alive(idx) && s.vocation_of(idx) == Some(Vocation::Baker) {
            if let Some(id) = s.colonist_id(idx) {
                if let Some(agent) = s.society().agents.get(id) {
                    gold = gold.saturating_add(agent.gold.0);
                }
            }
        }
    }
    gold
}

struct ArmResult {
    /// Whole-run counters. Reported as evidence of the monetary flows (esp. the L2 loss and
    /// the L1 cash-positive clearing); acceptance is on solvency, not on these.
    acc: BakerRoundTrip,
    living_bakers: usize,
    /// Bread produced by Bakers in the final window — the "still producing sustainably" leg.
    window_bread_produced: u64,
    /// Baker-class gold at end of run — the "solvent, not depleted" leg.
    baker_gold_end: u64,
    window_max_hunger: u16,
    nonlineage_survivors: usize,
    mints_active: bool,
}

impl ArmResult {
    /// Executed cash only; `operating_cost` is an imputed appraisal threshold with no
    /// payment site (`mod.rs:1019`), so it is NOT debited here. Negative under L2 (bread
    /// eaten, not sold); positive under L1 (bread sold).
    fn realized_round_trip(&self) -> i64 {
        self.acc.bread_gold_earned as i64 - self.acc.flour_gold_spent as i64
    }

    /// The pre-declared acceptance predicate: the chain FUNCTIONS (bakers staff and keep
    /// producing) and stays SOLVENT (baker-class gold does not deplete to zero). NOT gated
    /// on bread SALES — bread is the eaten staple here, so a functioning chain need not sell
    /// it for gold (that was the v1 miscalibration). Solvency across the run is confirmed
    /// separately at `SOLVENCY_TICKS` by `l2_baker_class_stays_solvent`.
    fn passes(&self) -> bool {
        self.living_bakers > 0
            && self.window_bread_produced >= PRODUCE_FLOOR
            && self.baker_gold_end > 0
    }
}

fn accumulator_delta(end: BakerRoundTrip, start: BakerRoundTrip) -> BakerRoundTrip {
    BakerRoundTrip {
        flour_gold_spent: end
            .flour_gold_spent
            .checked_sub(start.flour_gold_spent)
            .expect("flour spend accumulator is monotonic"),
        bread_gold_earned: end
            .bread_gold_earned
            .checked_sub(start.bread_gold_earned)
            .expect("bread earn accumulator is monotonic"),
        bread_units_sold: end
            .bread_units_sold
            .checked_sub(start.bread_units_sold)
            .expect("bread sold accumulator is monotonic"),
        bread_units_produced: end
            .bread_units_produced
            .checked_sub(start.bread_units_produced)
            .expect("bread produced accumulator is monotonic"),
    }
}

fn run_arm(seed: u64, arm: Arm) -> ArmResult {
    let cfg = config(arm);
    let mints_active = !cfg.chain.as_ref().expect("chain").retire_food_mints;
    let mut settlement = Settlement::generate(seed, &cfg);
    let window_start = RUN_TICKS - FINAL_WINDOW;
    let mut window_max_hunger = 0u16;
    let mut window_start_acc = BakerRoundTrip::default();
    for tick in 0..RUN_TICKS {
        if tick == window_start {
            window_start_acc = settlement.baker_round_trip();
        }
        settlement.econ_tick();
        if tick >= window_start {
            window_max_hunger = window_max_hunger.max(settlement.max_living_hunger());
        }
    }
    let nonlineage_survivors = (0..settlement.population())
        .filter(|&i| settlement.is_alive(i) && settlement.household_of(i).is_none())
        .count();
    let acc = settlement.baker_round_trip();
    let window_acc = accumulator_delta(acc, window_start_acc);
    ArmResult {
        window_bread_produced: window_acc.bread_units_produced,
        baker_gold_end: baker_class_gold(&settlement),
        acc,
        living_bakers: settlement.living_count(Vocation::Baker),
        window_max_hunger,
        nonlineage_survivors,
        mints_active,
    }
}

fn report(seed: u64, arm: Arm, r: &ArmResult) {
    println!(
        "C3R.h cut2 seed={seed} arm={} flour_gold_spent={} bread_gold_earned={} \
         bread_units_sold={} bread_units_produced={} realized_round_trip={} \
         window_bread_produced={} baker_gold_end={} living_bakers={} window_max_hunger={} \
         nonlineage_survivors={} mints_active={} passes={}",
        arm.label,
        r.acc.flour_gold_spent,
        r.acc.bread_gold_earned,
        r.acc.bread_units_sold,
        r.acc.bread_units_produced,
        r.realized_round_trip(),
        r.window_bread_produced,
        r.baker_gold_end,
        r.living_bakers,
        r.window_max_hunger,
        r.nonlineage_survivors,
        r.mints_active,
        r.passes(),
    );
}

/// Exclusive precedence tree, SINGLES first (the data is non-monotonic: L1+L2 < either
/// single, so a failing combined arm is a negative interaction, NOT a deeper wall).
/// `DEEPER_WALL` means nothing sustains.
fn outcome(l2: &ArmResult, l1: &ArmResult, l1l2: &ArmResult) -> &'static str {
    if l2.passes() && l1.passes() {
        "EITHER_SUFFICES"
    } else if l2.passes() {
        "STALE_PRICE_SUFFICES"
    } else if l1.passes() {
        "FOOD_FLOOR_RETIREMENT_SUFFICES"
    } else if l1l2.passes() {
        "BOTH_NEEDED"
    } else {
        "DEEPER_WALL"
    }
}

#[test]
fn canonical_bytes_excludes_baker_roundtrip() {
    const TICKS: u64 = 400;
    const AFTER: u64 = 50;
    let cfg = config(L2);

    let mut settlement = Settlement::generate(SEEDS[0], &cfg);
    settlement.run(TICKS);
    let populated = settlement.baker_round_trip();
    assert_ne!(
        populated,
        BakerRoundTrip::default(),
        "the accumulator must actually observe events before this test can prove anything"
    );

    let before = settlement.canonical_bytes();
    let _read = settlement.baker_round_trip();
    settlement.debug_perturb_baker_round_trip();
    let perturbed = settlement.baker_round_trip();
    assert!(
        perturbed.flour_gold_spent != populated.flour_gold_spent
            && perturbed.bread_gold_earned != populated.bread_gold_earned
            && perturbed.bread_units_sold != populated.bread_units_sold
            && perturbed.bread_units_produced != populated.bread_units_produced,
        "the perturbation must land on all four fields, otherwise the exclusion check is \
         vacuous for the untouched ones — got {perturbed:?} from {populated:?}"
    );
    assert_eq!(
        before,
        settlement.canonical_bytes(),
        "the Baker round-trip accumulator must be excluded from canonical_bytes"
    );

    let mut twin = Settlement::generate(SEEDS[0], &cfg);
    twin.run(TICKS);
    settlement.run(AFTER);
    twin.run(AFTER);
    assert_eq!(
        settlement.canonical_bytes(),
        twin.canonical_bytes(),
        "perturbed telemetry must not steer any later decision, price, or flow"
    );
}

#[test]
fn baker_roundtrip_2x2() {
    let mut outcomes = Vec::new();
    let mut any_sales = false;

    for seed in SEEDS {
        let base = run_arm(seed, BASE);
        let l2 = run_arm(seed, L2);
        let l1 = run_arm(seed, L1);
        let l1l2 = run_arm(seed, L1L2);

        for (arm, result) in [(BASE, &base), (L2, &l2), (L1, &l1), (L1L2, &l1l2)] {
            report(seed, arm, result);
            any_sales |= result.acc.bread_units_sold > 0;
        }

        let seed_outcome = outcome(&l2, &l1, &l1l2);
        let negative_interaction = !l1l2.passes() && (l2.passes() || l1.passes());
        println!(
            "C3R.h cut2 seed={seed} outcome={seed_outcome} base_suffices={} \
             negative_interaction={negative_interaction}",
            base.passes(),
        );
        outcomes.push((seed, seed_outcome, base, l2, l1, l1l2, negative_interaction));
    }

    let first = outcomes[0].1;
    let suite = if outcomes.iter().all(|o| o.1 == first) {
        first
    } else {
        "MIXED_SEED"
    };
    println!("C3R.h cut2 suite_label={suite} finding=EitherSufficesNegativeInteraction");

    // Non-vacuity: the observer actually sees the tape.
    assert!(
        any_sales,
        "the Baker round-trip accumulator observed no bread sale on any arm — the observer \
         is broken, not the economy"
    );

    // Cut 1's L2 Baker-stage result promoted to an assertion (regression guard for the
    // landed `stale_input_price_fix`): the stage must STAFF on every seed.
    for (seed, _, _, l2, ..) in &outcomes {
        assert!(
            l2.living_bakers > 0,
            "seed {seed}: the L2 arm must sustain the baker stage (cut-1 regression)"
        );
    }

    // THE FINDING: EITHER lever alone makes the chain function + stay solvent, and combining
    // them collapses it. If a single lever stops sustaining, or L1+L2 starts sustaining, this
    // fails — re-read the printed table and fold the new result into the impl-73 doc.
    assert_eq!(
        suite, "EITHER_SUFFICES",
        "cut-2 result is EITHER_SUFFICES on all five seeds (L2 alone and L1 alone each \
         sustain a solvent chain); got suite_label={suite} — see the printed table above"
    );

    for (seed, _, base, l2, l1, l1l2, negative_interaction) in &outcomes {
        // The two single levers each pass; the base does not, and L1+L2 collapses.
        assert!(
            l2.passes() && l1.passes(),
            "seed {seed}: both single levers must sustain a solvent chain — L2 \
             (bakers={}, window_produced={}, baker_gold_end={}), L1 (bakers={}, \
             window_produced={}, baker_gold_end={})",
            l2.living_bakers,
            l2.window_bread_produced,
            l2.baker_gold_end,
            l1.living_bakers,
            l1.window_bread_produced,
            l1.baker_gold_end,
        );
        // NOTE: base is NOT asserted to fail — seed 3 is the one pre-viable seed whose base
        // already functions (`base_suffices` is printed per seed). EITHER_SUFFICES is about
        // the two single LEVERS each sustaining, which holds on all five seeds regardless.
        let _ = base;
        // The negative interaction: combining both levers collapses the stage.
        assert!(
            !l1l2.passes() && *negative_interaction,
            "seed {seed}: L1+L2 must collapse the negative interaction the finding names — \
             got bakers={}, window_produced={}, baker_gold_end={}",
            l1l2.living_bakers,
            l1l2.window_bread_produced,
            l1l2.baker_gold_end,
        );

        // Colony control: no arm "wins" by wiping out the demand side. (A hunger BOUND is
        // vacuous on this immortal base — see `config`.)
        for (label, arm) in [("base", base), ("L2", l2), ("L1", l1), ("L1+L2", l1l2)] {
            assert!(
                arm.nonlineage_survivors > 0,
                "seed {seed}: the {label} arm wiped out the non-lineage tail — the arm is \
                 economically destructive, not viable"
            );
        }
    }
}

/// The solvency confirmation: over 1.5× the base horizon, the "loss-making" L2 baker class
/// does NOT deplete its gold to zero — it floors at a low positive steady state and keeps
/// producing. This is what makes L2 a real STALE_PRICE half of EITHER rather than a chain
/// that merely hadn't run out of gold yet at tick 1,600.
#[test]
fn l2_baker_class_stays_solvent() {
    for seed in SEEDS {
        let cfg = config(L2);
        let bread = cfg.chain.as_ref().expect("chain").content.bread();
        let mut settlement = Settlement::generate(seed, &cfg);
        let mut late_produced = 0u64;
        let late_start = SOLVENCY_TICKS - FINAL_WINDOW;
        for tick in 0..SOLVENCY_TICKS {
            let produced = settlement.econ_tick().produced_of(bread);
            if tick >= late_start {
                late_produced = late_produced.saturating_add(produced);
            }
        }
        let baker_gold = baker_class_gold(&settlement);
        println!(
            "C3R.h cut2 solvency seed={seed} ticks={SOLVENCY_TICKS} baker_gold={baker_gold} \
             late_produced={late_produced} living_bakers={}",
            settlement.living_count(Vocation::Baker),
        );
        assert!(
            baker_gold > 0,
            "seed {seed}: the L2 baker class depleted its gold to zero by tick \
             {SOLVENCY_TICKS} — insolvent, so STALE_PRICE does not truly sustain"
        );
        assert!(
            late_produced >= PRODUCE_FLOOR,
            "seed {seed}: the L2 baker class stopped producing by tick {SOLVENCY_TICKS} \
             (late_produced={late_produced}) — not a sustained chain"
        );
        assert!(
            settlement.living_count(Vocation::Baker) > 0,
            "seed {seed}: the L2 baker stage did not survive to tick {SOLVENCY_TICKS}"
        );
    }
}

/// Restore producer mortality and a reachable starvation ceiling for a real smoke.
#[test]
fn mortal_l2_smoke() {
    let mut cfg = config(L2);
    cfg.dynamics.hunger_critical = cfg.dynamics.need_max;
    let households = cfg
        .demography
        .as_ref()
        .expect("heritable demography")
        .households
        .len();
    let producer_start = households
        .checked_sub(PRODUCER_HOUSES)
        .expect("producer houses");
    let chain = cfg.chain.as_mut().expect("chain");
    chain.mortal_chain_producers = true;
    chain.mortal_producer_inheritance = true;

    let mut settlement = Settlement::generate(MORTAL_SMOKE_SEED, &cfg);
    let mut start_producer_lineage = 0usize;
    for idx in 0..settlement.population() {
        if settlement
            .household_of(idx)
            .is_some_and(|h| h >= producer_start)
        {
            start_producer_lineage += 1;
        }
    }
    for _ in 0..MORTAL_SMOKE_TICKS {
        settlement.econ_tick();
    }
    let lineage_survivors = (0..settlement.population())
        .filter(|&i| settlement.is_alive(i) && settlement.household_of(i).is_some())
        .count();
    let producer_lineage_survivors = (0..settlement.population())
        .filter(|&i| {
            settlement.is_alive(i)
                && settlement
                    .household_of(i)
                    .is_some_and(|h| h >= producer_start)
        })
        .count();
    println!(
        "C3R.h cut2 mortal_smoke seed={MORTAL_SMOKE_SEED} arm=L2 ticks={MORTAL_SMOKE_TICKS} \
         starvation_deaths_total={} start_producer_lineage={start_producer_lineage} \
         lineage_survivors={lineage_survivors} producer_lineage_survivors={producer_lineage_survivors}",
        settlement.starvation_deaths_total(),
    );
    // A real (reachable) starvation ceiling with mortal producers: the smoke just confirms
    // the run does not wipe the whole lineage tail under L2 with mortality on.
    assert!(
        lineage_survivors > 0,
        "the mortal L2 smoke wiped out every lineage member in {MORTAL_SMOKE_TICKS} ticks"
    );
}
