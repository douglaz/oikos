//! C3R.h (impl-73) L2: replace stale realized input prices in role choice with
//! fresh non-self reservation asks. The measurement reports, but does not gate on,
//! bread output and baker survival.

use sim::settlement::RoleChoiceDiag;
use sim::{Settlement, SettlementConfig, Vocation};

const SEEDS: [u64; 5] = [3, 7, 11, 19, 23];
const RUN_TICKS: u64 = 1_600;

fn config() -> SettlementConfig {
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
    chain.mortal_chain_producers = false;
    chain.mortal_producer_inheritance = false;
    cfg
}

fn config_with_l2(enabled: bool) -> SettlementConfig {
    let mut cfg = config();
    cfg.chain.as_mut().expect("chain").stale_input_price_fix = enabled;
    cfg
}

#[test]
fn canonical_bytes_include_stale_input_price_fix() {
    let default_off_cfg = config();
    let mut explicit_off_cfg = default_off_cfg.clone();
    explicit_off_cfg
        .chain
        .as_mut()
        .expect("chain")
        .stale_input_price_fix = false;
    let mut on_cfg = default_off_cfg.clone();
    on_cfg.chain.as_mut().expect("chain").stale_input_price_fix = true;

    let default_off = Settlement::generate(SEEDS[0], &default_off_cfg);
    let explicit_off = Settlement::generate(SEEDS[0], &explicit_off_cfg);
    let on = Settlement::generate(SEEDS[0], &on_cfg);

    assert_eq!(
        default_off.canonical_bytes(),
        explicit_off.canonical_bytes(),
        "an explicit stale_input_price_fix=false must keep the default byte stream identical"
    );
    assert_ne!(
        default_off.canonical_bytes(),
        on.canonical_bytes(),
        "the stale-input-price fix must split the canonical digest when on"
    );

    // Distinct tags prevent two active behavior flags from aliasing.
    let mut tool_only = SettlementConfig::emergent_chain();
    tool_only
        .chain
        .as_mut()
        .expect("chain")
        .tool_acquisition_eligibility = true;
    let mut l2_only = SettlementConfig::emergent_chain();
    l2_only.chain.as_mut().expect("chain").stale_input_price_fix = true;
    assert_ne!(
        Settlement::generate(SEEDS[0], &tool_only).canonical_bytes(),
        Settlement::generate(SEEDS[0], &l2_only).canonical_bytes(),
        "tool-eligibility-only and L2-only configs must not alias"
    );

    // The marker is omitted when role choice has no candidates.
    let inert_off = SettlementConfig::grain_flour_bread_chain();
    let mut inert_on = inert_off.clone();
    inert_on
        .chain
        .as_mut()
        .expect("chain")
        .stale_input_price_fix = true;
    assert_eq!(
        Settlement::generate(SEEDS[0], &inert_off).canonical_bytes(),
        Settlement::generate(SEEDS[0], &inert_on).canonical_bytes(),
        "an inert L2 flag must not split canonical bytes"
    );
}

#[test]
fn flag_off_is_byte_identical() {
    const TICKS: u64 = 300;
    let baseline_cfg = config();

    for seed in SEEDS {
        let mut baseline = Settlement::generate(seed, &baseline_cfg);
        baseline.run(TICKS);

        let mut explicit_off = Settlement::generate(seed, &config_with_l2(false));
        explicit_off.run(TICKS);

        assert_eq!(
            baseline.canonical_bytes(),
            explicit_off.canonical_bytes(),
            "seed {seed}: a flag-off run must be byte-identical to the default-field run"
        );
    }
}

struct Arm {
    diag: RoleChoiceDiag,
    bread_produced: u64,
    living_bakers: usize,
}

/// The C3R.h bucket invariants the new `InputPriceAbsent` reason documents
/// (`sim/src/settlement/mod.rs`): every appraisal lands in exactly one bucket, and the
/// new bucket is silent with the flag off — so the pre-C3R.h four-way partition that
/// `baker_role_diagnostic.rs` pins still holds for every flag-off run. Without these, a
/// double-observe or a dropped `continue` in the new branch would pass unnoticed.
fn assert_partitions(diag: RoleChoiceDiag, stale_input_price_fix: bool) {
    for histogram in [diag.mill, diag.bake] {
        assert_eq!(
            histogram.attempts,
            histogram.price_absent
                + histogram.input_price_absent
                + histogram.margin_nonpositive
                + histogram.ordinal_decline
                + histogram.accepts,
            "reason buckets must partition every appraisal: {histogram:?}"
        );
        if !stale_input_price_fix {
            assert_eq!(
                histogram.input_price_absent, 0,
                "InputPriceAbsent is unreachable with the flag off: {histogram:?}"
            );
        }
    }
}

fn run_arm(seed: u64, stale_input_price_fix: bool) -> Arm {
    let cfg = config_with_l2(stale_input_price_fix);
    let bread = cfg.chain.as_ref().expect("chain").content.bread();
    let mut settlement = Settlement::generate(seed, &cfg);
    let mut bread_produced = 0u64;
    for _ in 0..RUN_TICKS {
        bread_produced = bread_produced.saturating_add(settlement.econ_tick().produced_of(bread));
    }
    let diag = settlement.role_choice_diag();
    assert_partitions(diag, stale_input_price_fix);
    Arm {
        diag,
        bread_produced,
        living_bakers: settlement.living_count(Vocation::Baker),
    }
}

#[test]
fn l2_base_vs_fix_measurement() {
    let mut flipped_to_accepts = 0usize;
    let mut histograms_differ = 0usize;

    for seed in SEEDS {
        let base = run_arm(seed, false);
        let l2 = run_arm(seed, true);

        println!(
            "C3R.h L2 seed={seed} arm=base bake={:?} bread_produced={} living_bakers={}",
            base.diag.bake, base.bread_produced, base.living_bakers,
        );
        println!(
            "C3R.h L2 seed={seed} arm=l2 bake={:?} bread_produced={} living_bakers={}",
            l2.diag.bake, l2.bread_produced, l2.living_bakers,
        );

        if l2.diag.bake.accepts > base.diag.bake.accepts
            && l2.diag.bake.margin_nonpositive < base.diag.bake.margin_nonpositive
        {
            flipped_to_accepts += 1;
        }
        if l2.diag.bake != base.diag.bake {
            histograms_differ += 1;
        }
    }

    assert!(
        flipped_to_accepts > 0,
        "L2 must flip at least one base MarginNonpositive bake appraisal to Accepts on ≥1 seed \
         (otherwise the flag is vacuous)"
    );
    assert!(
        histograms_differ > 0,
        "base and L2 role-choice histograms must differ on ≥1 seed"
    );
}
