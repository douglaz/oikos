//! C3R.d -- saving ahead of need: the birth-stock motive.
//!
//! Outcomes are classified and printed, never promoted by assertion. Assertions
//! cover only the landed references and hard conservation/registry/mode guards.
//!
//! The classifier machinery (`Cell`/`Verdict`/`ReferenceVerdict`, `Trace`, `config`, `trace`,
//! `classify`, `reference_verdict`, and their helpers) lives in `tests/common/mod.rs` so the
//! C3R.e-obs acceptance suite classifies through the SAME real classifier (impl-66 repair §1).

mod common;

use common::{
    assert_ledger_split, classify, config, hard_invariants, print_trace, reference_verdict, trace,
    Cell, ReferenceVerdict, RUN_TICKS,
};
use sim::{BirthStockSavingMode, Settlement, SettlementConfig};

const SEEDS: [u64; 5] = [3, 7, 11, 19, 23];
const EXPECTED_NO_MOTIVE_BIRTHS: [u64; 5] = [2, 3, 5, 2, 1];
// Exact landed C3R.b viable-cell facts. Seed 7 is an honest precondition null:
// the classifier must return BaseUnviable rather than silently treating it as a
// headline result or tuning the reference away from the landed cell.
const EXPECTED_MINT_ON_STRUCTURE: [bool; 5] = [true, false, true, true, true];

#[test]
fn old_bases_are_byte_identical_and_tag_31_modes_split() {
    for (name, cfg, expected_base_digest) in [
        (
            "frontier",
            SettlementConfig::frontier(),
            0x5c0b_c654_ac51_6376,
        ),
        (
            "frontier_capital",
            SettlementConfig::frontier_capital(),
            0x2f9d_c5c1_1364_b05e,
        ),
        (
            "frontier_mortal_producers",
            SettlementConfig::frontier_mortal_producers(),
            0x9b1b_2b43_7d04_eb93,
        ),
        (
            "frontier_mortal_producers_heritable",
            SettlementConfig::frontier_mortal_producers_heritable(),
            0x0b27_39bd_e8ec_7bde,
        ),
        (
            "frontier_mortal_producers_earned",
            SettlementConfig::frontier_mortal_producers_earned(),
            0x98b5_64d3_0a7e_9070,
        ),
    ] {
        let base = Settlement::generate(SEEDS[0], &cfg);
        assert_eq!(
            base.digest(),
            expected_base_digest,
            "old base {name} diverged from its b15f5d6 canonical digest"
        );
        let mut explicit_off = cfg.clone();
        let chain = explicit_off.chain.as_mut().expect("chain");
        chain.birth_stock_saving = false;
        chain.birth_stock_saving_mode = BirthStockSavingMode::Off;
        assert_eq!(
            base.canonical_bytes(),
            Settlement::generate(SEEDS[0], &explicit_off).canonical_bytes(),
            "old base {name} changed with tag 31 off"
        );
    }
    let motive = Settlement::generate(SEEDS[0], &config(Cell::Headline)).canonical_bytes();
    let control =
        Settlement::generate(SEEDS[0], &config(Cell::SufficiencyControl)).canonical_bytes();
    assert!(motive.windows(3).any(|bytes| bytes == [31, 1, 1]));
    assert!(control.windows(3).any(|bytes| bytes == [31, 0, 2]));
    assert_ne!(motive, control);
}

#[test]
fn birth_stock_cells_print_total_verdicts_without_asserting_success() {
    println!(
        "C3R.d grid seeds={SEEDS:?} ticks={RUN_TICKS} lineage_surround={{3}}; verdicts are observations"
    );
    for (seed_index, seed) in SEEDS.into_iter().enumerate() {
        let reference = trace(seed, Cell::NoMotiveReference);
        let mint_on = trace(seed, Cell::MintOnReference);
        let control = trace(seed, Cell::SufficiencyControl);
        let headline = trace(seed, Cell::Headline);

        for trace in [&reference, &mint_on, &control, &headline] {
            assert!(hard_invariants(trace), "hard invariant failed: {trace:?}");
            assert_ledger_split(trace);
        }
        assert_eq!(
            reference.producer_births, EXPECTED_NO_MOTIVE_BIRTHS[seed_index],
            "the same-seed C3R.c birth anchor drifted: {reference:?}"
        );
        assert_eq!(
            reference_verdict(&reference),
            ReferenceVerdict::FedButChildless,
            "the C3R.c reference verdict drifted: {reference:?}"
        );
        assert_eq!(
            mint_on.final_window.structure_runs(),
            EXPECTED_MINT_ON_STRUCTURE[seed_index],
            "the exact per-seed C3R.b mint-on structure fact drifted: {mint_on:?}"
        );
        assert_eq!(control.wants, 0, "control mode must disable the motive");
        assert_eq!(
            control.producer_hearth_food, 0,
            "control must not mint bread"
        );
        assert_eq!(
            control.injections_completed as usize, control.injection_records,
            "every completed injection has an immediate result"
        );

        let verdict = classify(&headline, &control, &reference, &mint_on);
        print_trace(&reference, verdict);
        print_trace(&mint_on, verdict);
        print_trace(&control, verdict);
        print_trace(&headline, verdict);
    }
}
