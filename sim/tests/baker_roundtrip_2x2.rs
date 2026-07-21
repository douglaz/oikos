//! C3R.h cut 2: Baker round-trip telemetry and the base/L2/L1/L1+L2 experiment.

use sim::settlement::BakerRoundTrip;
use sim::{Settlement, SettlementConfig, Vocation};

const SEEDS: [u64; 5] = [3, 7, 11, 19, 23];
const RUN_TICKS: u64 = 1_600;
const FINAL_WINDOW: u64 = 160;
const SUBSTANTIAL: u64 = 300;
const MORTAL_SMOKE_SEED: u64 = 3;
const MORTAL_SMOKE_TICKS: u64 = 400;
const MORTAL_SMOKE_LIVING_FLOOR: usize = 1;

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
    let producer_start = households.len().checked_sub(6).expect("producer houses");
    for house in &mut households[producer_start..] {
        house.food_provision = 0;
    }

    let chain = cfg.chain.as_mut().expect("chain");
    chain.producer_house_cap = 2;
    chain.mortal_producer_tool_inheritance = true;
    // The pinned immortal control. Its inherited `hunger_critical = need_max + 1`
    // disables starvation colony-wide, not just for the chain producers.
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
    acc: BakerRoundTrip,
    /// Final-window delta; whole-run production remains in `acc`.
    window_acc: BakerRoundTrip,
    living_bakers: usize,
    window_max_hunger: u16,
    nonlineage_survivors: usize,
}

impl ArmResult {
    /// The task-defined class overlap, not a per-loaf provenance ledger.
    fn baker_origin_bread_sold(&self) -> u64 {
        self.acc
            .bread_units_produced
            .min(self.window_acc.bread_units_sold)
    }

    /// Executed cash only; `operating_cost` is imputed and is not debited.
    fn realized_round_trip(&self) -> i64 {
        self.window_acc.bread_gold_earned as i64 - self.window_acc.flour_gold_spent as i64
    }

    /// Report Baker sales beyond Baker production as resale.
    fn resale(&self) -> bool {
        self.acc.bread_units_sold > self.acc.bread_units_produced
    }

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
    }
}

fn report(seed: u64, arm: Arm, r: &ArmResult) {
    println!(
        "C3R.h cut2 seed={seed} arm={} flour_gold_spent={} bread_gold_earned={} \
         bread_units_sold={} bread_units_produced={} baker_origin_bread_sold={} \
         realized_round_trip={} window_flour_gold_spent={} window_bread_gold_earned={} \
         window_bread_units_sold={} window_bread_units_produced={} living_bakers={} \
         window_max_hunger={} \
         nonlineage_survivors={} resale={} passes={}",
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
    assert_ne!(
        settlement.baker_round_trip(),
        populated,
        "the perturbation must land, otherwise the exclusion check is vacuous"
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
        outcomes.push((seed, seed_outcome, l2));
    }

    let first = outcomes[0].1;
    let suite = if outcomes.iter().all(|&(_, o, _)| o == first) {
        first
    } else {
        "MIXED_SEED"
    };
    println!("C3R.h cut2 suite_label={suite}");

    // Assertion 2 — non-vacuity.
    assert!(
        any_sales,
        "the Baker round-trip accumulator observed no bread sale on any arm — the \
         observer is broken, not the economy"
    );

    // Assertion 3 — cut 1's L2 Baker-stage result promoted to an assertion.
    for (seed, _, l2) in &outcomes {
        assert!(
            l2.living_bakers > 0,
            "seed {seed}: the L2 arm must sustain the baker stage (cut-1 regression)"
        );
    }

    // Assertion 4 — STALE-PRICE-SUFFICES acceptance: every L2 arm must sell at least
    // SUBSTANTIAL Baker-origin bread with a strictly positive realized cash round-trip.
    //
    // If this fails, L2 does NOT suffice: the outcome is BOTH_NEEDED (or DEEPER_WALL).
    // Inspect the printed L1 and L1+L2 arms above to tell which — a failure here is a
    // real finding about the economy, not necessarily a defect in this suite. No
    // specific suite label is asserted beyond this; the truth table is the deliverable.
    for (seed, _, l2) in &outcomes {
        assert!(
            l2.passes(),
            "seed {seed}: the L2 arm must sell ≥{SUBSTANTIAL} units of baker-origin \
             bread with a positive realized round-trip — got sold={} (produced={}, \
             seller-attributed={}), round_trip={}, living_bakers={}",
            l2.baker_origin_bread_sold(),
            l2.acc.bread_units_produced,
            l2.acc.bread_units_sold,
            l2.realized_round_trip(),
            l2.living_bakers,
        );
    }
}

/// Restore producer mortality and a reachable starvation ceiling for a real smoke.
#[test]
fn mortal_l2_smoke() {
    let mut cfg = config(L2);
    cfg.dynamics.hunger_critical = cfg.dynamics.need_max;
    let chain = cfg.chain.as_mut().expect("chain");
    chain.mortal_chain_producers = true;
    chain.mortal_producer_inheritance = true;

    let mut settlement = Settlement::generate(MORTAL_SMOKE_SEED, &cfg);
    settlement.run(MORTAL_SMOKE_TICKS);
    let living = (0..settlement.population())
        .filter(|&i| settlement.is_alive(i))
        .count();
    let starvation_deaths = settlement.starvation_deaths_total();
    println!(
        "C3R.h cut2 mortal_smoke seed={MORTAL_SMOKE_SEED} arm=L2 \
         ticks={MORTAL_SMOKE_TICKS} starvation_deaths_total={starvation_deaths} \
         living={living}"
    );
    assert!(
        living >= MORTAL_SMOKE_LIVING_FLOOR,
        "mortal L2 smoke must retain at least {MORTAL_SMOKE_LIVING_FLOOR} living agent"
    );
    assert_eq!(
        starvation_deaths, 0,
        "mortal L2 smoke must not introduce starvation deaths"
    );
}
