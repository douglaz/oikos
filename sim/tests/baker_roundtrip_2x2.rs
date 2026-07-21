//! C3R.h cut 2: Baker round-trip telemetry and the base/L2/L1/L1+L2 experiment.
//!
//! **Measured result — `BakerProducesButDoesNotSell` (suite `DEEPER_WALL`, 5/5 seeds).**
//! L2 (`stale_input_price_fix`) sustains nine bakers and ~12,000 cumulative loaves, but
//! Baker-class bread *sales* stay at 46-59 loaves over the whole 1,600-tick run and at
//! **zero** across the final window. Cut 1's leading reading "STALE-PRICE-SUFFICES" was
//! measured on cumulative *production*; this cut measures *sales* and **falsifies** it.
//! Production is not the bottleneck — clearing is.
//!
//! The realized cash flows sharpen it further. L2 does not merely fail to clear, it runs
//! the baker stage at a **loss**: ~4,100 gold of flour bought against ~900 of bread sold,
//! a round trip near **−3,200** on every seed. L1 stays cash-*positive* (+948 to +1,781),
//! while base is positive on four seeds but enters the same high-output, cash-negative
//! regime as L2 on seed 3 (13,068 loaves, −3,807). So L2 buys the input and bakes, and
//! the output never converts back to money. Retiring the food floor (L1) does not rescue
//! clearing either, and the combined L1+L2 arm collapses the baker stage outright
//! (`living_bakers = 0`, 27 loaves).
//!
//! The assertions below therefore pin the NULL, in the same style as `WageMarketVacuous`
//! and `ChainCollapsesOnProducerDeath`. They are written so that an economy change which
//! actually makes baker-origin bread clear will FAIL this suite and force a re-read.

use sim::settlement::BakerRoundTrip;
use sim::{Settlement, SettlementConfig, Vocation};

const SEEDS: [u64; 5] = [3, 7, 11, 19, 23];
const RUN_TICKS: u64 = 1_600;
const FINAL_WINDOW: u64 = 160;
/// Pre-declared falsifiable floor for "substantial" baker-origin sales.
const SUBSTANTIAL: u64 = 300;
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
    // (`mod.rs:3670`) disables starvation colony-wide, not just for the chain producers,
    // so a hunger BOUND is vacuous here: `hunger` clamps at `need_max` and nothing dies of
    // it. The window max is reported as evidence, not asserted; `mortal_l2_smoke` below is
    // where a reachable starvation ceiling is actually exercised.
    chain.mortal_chain_producers = false;
    chain.mortal_producer_inheritance = false;

    if arm.l1 {
        chain.retire_food_mints = true;
        chain.subsistence_on_grain = false;
    }
    chain.stale_input_price_fix = arm.l2;
    cfg
}

struct ArmResult {
    /// Whole-run counters. Every derived acceptance metric is computed from these so the
    /// numerator and denominator share one scope.
    acc: BakerRoundTrip,
    /// Final-window delta of the same counters, reported as supporting evidence.
    window_acc: BakerRoundTrip,
    living_bakers: usize,
    window_max_hunger: u16,
    nonlineage_survivors: usize,
    /// Whether recurring bread MINTS are live on this arm (`!retire_food_mints`). When
    /// true, seller-vocation attribution is origin-contaminated — see
    /// [`ArmResult::baker_origin_bread_sold`].
    mints_active: bool,
}

impl ArmResult {
    /// Whole-run UPPER BOUND on Baker-origin bread sales.
    ///
    /// `bread_units_produced` is Baker-only by construction (it is the bake-phase delta).
    /// `bread_units_sold` is seller-vocation attribution off the spot tape, so on the
    /// mint-bearing arms (`mints_active`) a Baker can also be selling loaves it never
    /// baked: `run_producer_subsistence` (`phases.rs:972`) and
    /// `deliver_demography_provisions` (`demography.rs:1098`) both mint `known.hunger`,
    /// which on this designated-gold chain IS bread (`generation.rs:90`), and both mints
    /// are live unless `retire_food_mints` is set — i.e. on `base` and `L2`.
    ///
    /// The contamination is therefore strictly UPWARD on the sales term, so it can only
    /// make an arm look BETTER than the truth. It cannot manufacture the null this suite
    /// asserts; it could only hide a pass. The L1 arms retire both mints and so carry no
    /// contamination at all — and they fail too, which is the contamination-free control.
    fn baker_origin_bread_sold(&self) -> u64 {
        self.acc.bread_units_produced.min(self.acc.bread_units_sold)
    }

    /// Executed cash only; `operating_cost` is an imputed appraisal threshold with no
    /// payment site (`mod.rs:1019`), so it is NOT debited here.
    fn realized_round_trip(&self) -> i64 {
        self.acc.bread_gold_earned as i64 - self.acc.flour_gold_spent as i64
    }

    /// Report Baker sales beyond Baker production as resale.
    fn resale(&self) -> bool {
        self.acc.bread_units_sold > self.acc.bread_units_produced
    }

    /// The pre-declared acceptance predicate. Deliberately NOT widened with the hunger or
    /// survivor controls: those are reported separately and asserted separately, so the
    /// falsifiable criterion pinned before the run is the one that gets evaluated.
    fn passes(&self) -> bool {
        self.living_bakers > 0
            && self.baker_origin_bread_sold() >= SUBSTANTIAL
            && self.realized_round_trip() > 0
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
            .expect("bread earnings accumulator is monotonic"),
        bread_units_sold: end
            .bread_units_sold
            .checked_sub(start.bread_units_sold)
            .expect("bread sales accumulator is monotonic"),
        bread_units_produced: end
            .bread_units_produced
            .checked_sub(start.bread_units_produced)
            .expect("bread production accumulator is monotonic"),
    }
}

fn run_arm(seed: u64, arm: Arm) -> ArmResult {
    let cfg = config(arm);
    // Read the flag actually configured, not the arm label. Assertion 4b's mint-free
    // control is what carries the rejection of the seller-vocation attribution caveat,
    // so it must fail if `config` ever stops retiring the mints on an L1 arm.
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
    ArmResult {
        acc,
        window_acc: accumulator_delta(acc, window_start_acc),
        living_bakers: settlement.living_count(Vocation::Baker),
        window_max_hunger,
        nonlineage_survivors,
        mints_active,
    }
}

fn report(seed: u64, arm: Arm, r: &ArmResult) {
    println!(
        "C3R.h cut2 seed={seed} arm={} flour_gold_spent={} bread_gold_earned={} \
         bread_units_sold={} bread_units_produced={} baker_origin_bread_sold={} \
         realized_round_trip={} window_flour_gold_spent={} window_bread_gold_earned={} \
         window_bread_units_sold={} window_bread_units_produced={} living_bakers={} \
         window_max_hunger={} nonlineage_survivors={} mints_active={} resale={} passes={}",
        arm.label,
        r.acc.flour_gold_spent,
        r.acc.bread_gold_earned,
        r.acc.bread_units_sold,
        r.acc.bread_units_produced,
        r.baker_origin_bread_sold(),
        r.realized_round_trip(),
        r.window_acc.flour_gold_spent,
        r.window_acc.bread_gold_earned,
        r.window_acc.bread_units_sold,
        r.window_acc.bread_units_produced,
        r.living_bakers,
        r.window_max_hunger,
        r.nonlineage_survivors,
        r.mints_active,
        r.resale(),
        r.passes(),
    );
}

/// Exclusive precedence tree, with the combined arm evaluated first.
fn outcome(l2: &ArmResult, l1: &ArmResult, l1l2: &ArmResult) -> &'static str {
    if !l1l2.passes() {
        "DEEPER_WALL"
    } else if l2.passes() && l1.passes() {
        "EITHER"
    } else if l2.passes() {
        "STALE_PRICE_SUFFICES"
    } else if l1.passes() {
        "FOOD_FLOOR_RETIREMENT_SUFFICES"
    } else {
        "BOTH_NEEDED"
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
    // Every field must move, otherwise the tripwire only covers the ones that do.
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
        let negative_interaction = !l1l2.passes() && (base.passes() || l2.passes() || l1.passes());
        println!(
            "C3R.h cut2 seed={seed} outcome={seed_outcome} base_suffices={} \
             negative_interaction={negative_interaction}",
            base.passes(),
        );
        outcomes.push((seed, seed_outcome, base, l2, l1, l1l2));
    }

    let first = outcomes[0].1;
    let suite = if outcomes.iter().all(|o| o.1 == first) {
        first
    } else {
        "MIXED_SEED"
    };
    println!("C3R.h cut2 suite_label={suite} finding=BakerProducesButDoesNotSell");

    // Assertion 2 — non-vacuity: the observer actually sees the tape.
    assert!(
        any_sales,
        "the Baker round-trip accumulator observed no bread sale on any arm — the \
         observer is broken, not the economy"
    );

    // Assertion 3 — cut 1's L2 Baker-stage result promoted to an assertion. This is the
    // regression guard for the landed `stale_input_price_fix`: the stage must STAFF.
    for (seed, _, _, l2, ..) in &outcomes {
        assert!(
            l2.living_bakers > 0,
            "seed {seed}: the L2 arm must sustain the baker stage (cut-1 regression)"
        );
    }

    // Assertion 4 — THE FINDING, asserted rather than hoped for.
    //
    // Cut 1's "STALE-PRICE-SUFFICES" reading is FALSIFIED. L2 makes the baker stage staff
    // and produce at scale, and it still does not clear: no arm — not base, not L2, not
    // L1, not L1+L2 — reaches SUBSTANTIAL baker-origin SALES. The suite label is
    // DEEPER_WALL on every seed, so neither lever alone nor both together closes the loop,
    // and the wall is downstream of the role-choice appraisal cut 1 fixed.
    //
    // If this assertion ever fails, the economy has CHANGED and the null no longer holds:
    // re-read the printed truth table, re-derive the outcome, and fold the new result into
    // `docs/impl-final-stage-demand.md` rather than relaxing anything here.
    assert_eq!(
        suite, "DEEPER_WALL",
        "the measured cut-2 result is DEEPER_WALL on all five seeds (baker-origin bread \
         is produced but does not sell); got suite_label={suite} — see the printed \
         per-(arm, seed) table above"
    );

    for (seed, _, base, l2, l1, l1l2) in &outcomes {
        // 4a — the mechanism: L2 PRODUCES at scale but does NOT sell. Both halves matter;
        // asserting only the second would also pass on an arm that simply never baked.
        assert!(
            l2.acc.bread_units_produced >= SUBSTANTIAL,
            "seed {seed}: the L2 arm must still produce ≥{SUBSTANTIAL} loaves (cut-1 \
             regression) — got {}",
            l2.acc.bread_units_produced
        );
        assert!(
            l2.baker_origin_bread_sold() < SUBSTANTIAL,
            "seed {seed}: L2 baker-origin bread now SELLS (≥{SUBSTANTIAL}) — the cut-2 \
             null is broken and STALE-PRICE-SUFFICES may hold after all; got sold={} \
             (produced={}, seller-attributed={}), realized_round_trip={}",
            l2.baker_origin_bread_sold(),
            l2.acc.bread_units_produced,
            l2.acc.bread_units_sold,
            l2.realized_round_trip(),
        );

        // 4b — the contamination-free control. The L1 arms retire BOTH bread mints, so
        // their seller-vocation attribution carries no minted loaves, and they fail the
        // acceptance too. The null therefore does not rest on the contaminated arms.
        for (label, arm) in [("L1", l1), ("L1+L2", l1l2)] {
            assert!(
                !arm.mints_active,
                "seed {seed}: the {label} arm must retire the bread mints — otherwise it \
                 is not a contamination-free control"
            );
            assert!(
                !arm.passes(),
                "seed {seed}: the mint-free {label} arm now passes — the cut-2 null is \
                 broken; got sold={}, realized_round_trip={}, living_bakers={}",
                arm.baker_origin_bread_sold(),
                arm.realized_round_trip(),
                arm.living_bakers,
            );
        }

        // 4c — the colony control. An arm that "wins" by depopulating the demand side is
        // not a result; every arm must leave the non-lineage tail alive. (A hunger BOUND
        // would be vacuous on this immortal base — see `config`.)
        for (label, arm) in [("base", base), ("L2", l2), ("L1", l1), ("L1+L2", l1l2)] {
            assert!(
                arm.nonlineage_survivors > 0,
                "seed {seed}: the {label} arm wiped out the non-lineage tail — the arm is \
                 economically destructive, not viable"
            );
        }
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
    for i in 0..settlement.population() {
        if settlement.is_alive(i)
            && settlement
                .household_of(i)
                .is_some_and(|h| h >= producer_start)
        {
            start_producer_lineage += 1;
        }
    }
    settlement.run(MORTAL_SMOKE_TICKS);
    // Scope the survivor floor to the MORTAL LINEAGE (household members), not the whole
    // colony. A global living count is toothless here: the non-lineage tail is ~74 agents,
    // so it passes even with every household extinct.
    //
    // The producer-lineage count is measured and PRINTED but deliberately NOT asserted
    // above zero. It goes 6 → 0 within these 400 ticks, and that is the LANDED C3R.a null
    // `ChainCollapsesOnProducerDeath` (`sim/tests/mortal_producers.rs:164`,
    // `docs/impl-mortal-producers.md:115` — "after the first producer die-off the
    // milling/baking stage empties"), not something cut 2 introduced. Producer lifespan is
    // impl-71's problem. This smoke's job is non-regression for the L2 lever: no new
    // starvation deaths, and the colony's lineages do not collapse wholesale.
    let mut lineage_survivors = 0usize;
    let mut producer_lineage_survivors = 0usize;
    for i in 0..settlement.population() {
        if !settlement.is_alive(i) {
            continue;
        }
        if let Some(house) = settlement.household_of(i) {
            lineage_survivors += 1;
            if house >= producer_start {
                producer_lineage_survivors += 1;
            }
        }
    }
    let starvation_deaths = settlement.starvation_deaths_total();
    println!(
        "C3R.h cut2 mortal_smoke seed={MORTAL_SMOKE_SEED} arm=L2 \
         ticks={MORTAL_SMOKE_TICKS} starvation_deaths_total={starvation_deaths} \
         start_producer_lineage={start_producer_lineage} \
         lineage_survivors={lineage_survivors} \
         producer_lineage_survivors={producer_lineage_survivors}"
    );
    assert!(
        lineage_survivors > 0,
        "mortal L2 smoke: every household lineage went extinct within \
         {MORTAL_SMOKE_TICKS} ticks"
    );
    assert_eq!(
        starvation_deaths, 0,
        "mortal L2 smoke must not introduce starvation deaths"
    );
}
