//! C3R.g Stage-1 Baker-role rejection telemetry.

use sim::settlement::RoleChoiceDiag;
use sim::{Settlement, SettlementConfig, Vocation};

const SEEDS: [u64; 5] = [3, 7, 11, 19, 23];
const FAILING_CONTROL_SEEDS: [u64; 4] = [7, 11, 19, 23];
const RUN_TICKS: u64 = 1_600;

fn config(immortal: bool) -> SettlementConfig {
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
    if immortal {
        chain.mortal_chain_producers = false;
        chain.mortal_producer_inheritance = false;
    }
    cfg
}

fn assert_exhaustive(diag: RoleChoiceDiag) {
    assert!(diag.bake.attempts > 0, "bake appraisals must be observed");
    for histogram in [diag.mill, diag.bake] {
        assert_eq!(
            histogram.attempts,
            histogram.price_absent
                + histogram.margin_nonpositive
                + histogram.ordinal_decline
                + histogram.accepts,
            "reason buckets must partition every appraisal: {histogram:?}"
        );
    }
}

#[test]
fn canonical_bytes_excludes_role_choice_diag() {
    let mut settlement = Settlement::generate(SEEDS[0], &config(false));
    settlement.run(300);
    let diag = settlement.role_choice_diag();
    assert!(diag.bake.attempts > 0);

    let before = settlement.canonical_bytes();
    let _ = settlement.role_choice_diag();
    assert_eq!(before, settlement.canonical_bytes());

    settlement.debug_perturb_role_choice_diag();
    assert_ne!(diag, settlement.role_choice_diag());
    assert_eq!(before, settlement.canonical_bytes());
}

#[test]
fn baker_role_reason_histogram() {
    for (immortal, cell) in [(false, "InheritanceCell"), (true, "FlagOffHeritable")] {
        let cfg = config(immortal);
        let bread = cfg.chain.as_ref().expect("chain").content.bread();
        for seed in SEEDS {
            let mut settlement = Settlement::generate(seed, &cfg);
            let mut bread_produced = 0u64;
            for _ in 0..RUN_TICKS {
                bread_produced =
                    bread_produced.saturating_add(settlement.econ_tick().produced_of(bread));
            }

            let diag = settlement.role_choice_diag();
            println!(
                "C3R.g seed={seed} cell={cell} mill={:?} bake={:?} \
                 baker_hold=({:?}..={:?}) bread_produced={bread_produced}",
                diag.mill, diag.bake, diag.baker_first_econ_tick, diag.baker_last_econ_tick,
            );
            assert_exhaustive(diag);
            if immortal && FAILING_CONTROL_SEEDS.contains(&seed) {
                assert_eq!(
                    settlement.living_count(Vocation::Baker),
                    0,
                    "failing immortal-control seed {seed} must end with no living Baker"
                );
            }
        }
    }
}
