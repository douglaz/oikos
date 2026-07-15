//! DH.b (impl-69) — the reproductive-burden robustness audit (`docs/impl-reproductive-burden.md`).
//!
//! The 60-cell grid: on the EXACT closed base (`frontier_closed_circulation()`), sweep the birth
//! burden q∈{0,1,2,3,4,8} × the two-field saving arm {Off, On(Motive)} × the landed seeds
//! [3,7,11,19,23] — serial, seeds outermost, q ascending, Off-then-On — scored by the
//! succession-survival oracle (W=36, M=5, RUN_TICKS=1600, start = 36×ceil((last_founder_death+1)/36)).
//!
//! Hard guards (conservation, the registry invariant, founder extinction before `start`, zero
//! immortal producers, zero intervention/support-origin stock, funding-attribution completeness,
//! instrumentation-corruption audits) print `PreconditionInvalid { guard }` and FAIL the suite.
//! Scientific outcomes are printed, never asserted. The DH.c gate is REPORTED only — a q=4 arm at
//! rung 6+/7 on 5/5 seeds authorizes a FUTURE rerun, never executed here.

use sim::{
    burden_dh_c_gate, burden_motive_effect, burden_nonmonotone_pairs, run_burden_grid,
    BirthStockSavingMode, BurdenCellVerdict, BurdenSavingArm, BurdenSynthesis, Settlement,
    SettlementConfig, BURDEN_QS, BURDEN_SEEDS,
};

const CELL_AUDIT_GOLDEN: &str = include_str!("goldens/reproductive_burden_cells.txt");

/// The per-cell config: `frontier_closed_circulation()` + EXACTLY {child_food_endowment=q, the
/// two-field saving arm} and nothing else (§2). q=4 stays canonical in every constructor — the
/// sweep lives only here, in the test.
fn cell_config(q: u32, arm: BurdenSavingArm) -> SettlementConfig {
    let mut cfg = SettlementConfig::frontier_closed_circulation();
    cfg.demography
        .as_mut()
        .expect("the closed base carries demography")
        .child_food_endowment = q;
    if arm == BurdenSavingArm::On {
        let chain = cfg.chain.as_mut().expect("the closed base carries a chain");
        chain.birth_stock_saving = true;
        chain.birth_stock_saving_mode = BirthStockSavingMode::Motive;
    }
    // SufficiencyControl is unreachable in every cell (§2).
    assert_ne!(
        cfg.chain.as_ref().expect("chain").birth_stock_saving_mode,
        BirthStockSavingMode::SufficiencyControl
    );
    cfg
}

/// The 60-cell audit: serial, seeds outermost, q ascending, Off-then-On (§3). Scientific
/// outcomes print AND are machine-bound against the committed audit-table golden; the hard
/// guards additionally fail the suite on violation.
#[test]
fn reproductive_burden_sixty_cell_audit() {
    // The shared harness (§4a) — the SAME real pipeline the DH.b-obs suite drives, with observation
    // OFF here so this golden is unchanged.
    let grid = run_burden_grid(false);

    for run in &grid.cell_runs {
        println!(
            "cell seed={} q={} arm={:?}: start={} windows={} births={} founder_deaths={:?}",
            run.seed,
            run.q,
            run.arm,
            run.start,
            run.windows.len(),
            run.births,
            run.founder_deaths
        );
        for window in &run.windows {
            println!(
                "  window start={} continuity={:?} flow={:?}",
                window.start, window.continuity, window.flow
            );
        }
        if !run.guard_failures.is_empty() {
            println!(
                "  PreconditionInvalid {{ guard: {:?} }}",
                run.guard_failures
            );
        }
        println!("  verdict[rung {}]: {:?}", run.verdict.rung(), run.verdict);
    }

    // Stable, machine-bound evidence for every published cell figure (P2-4): unlike the
    // diagnostic prose above, each row binds births and verdict in one fixed-format record and
    // the committed golden makes drift a test failure.
    let cell_audit = format!("{}\n", grid.audit_rows.join("\n"));
    println!("\nstructured per-cell audit:\n{cell_audit}");
    assert_eq!(
        cell_audit, CELL_AUDIT_GOLDEN,
        "the published 60-cell verdict+birth figures changed"
    );

    // Compact paired evidence for the rung-level motive null (P1-1). Deltas are On minus Off;
    // all six burdens print, including the q=0 reachability control.
    println!("paired births table (delta = On - Off):");
    for line in &grid.paired_table {
        println!("{line}");
    }

    // The full per-arm per-seed rung table (always printed, R1-3).
    println!("\nper-arm per-seed rung table (rows q/arm, columns seeds {BURDEN_SEEDS:?}):");
    for &q in &BURDEN_QS {
        for arm in [BurdenSavingArm::Off, BurdenSavingArm::On] {
            let rungs: Vec<u8> = BURDEN_SEEDS
                .iter()
                .map(|&seed| {
                    grid.cells
                        .iter()
                        .find(|c| c.q == q && c.arm == arm && c.seed == seed)
                        .expect("complete grid")
                        .verdict
                        .rung()
                })
                .collect();
            println!("  q={q} {arm:?}: {rungs:?}");
        }
    }

    println!("\nsynthesis: {:?}", grid.synthesis);
    println!("motive_effect: {:?}", burden_motive_effect(&grid.cells));
    let nonmonotone = burden_nonmonotone_pairs(&grid.cells);
    println!("nonmonotone: {nonmonotone:?}");
    assert!(
        nonmonotone.iter().all(|&(_, lo, hi)| lo != 0 && hi != 0),
        "no nonmonotone tuple contains q=0 (R4-2)"
    );
    let gate = burden_dh_c_gate(&grid.cells);
    if gate.is_empty() {
        println!(
            "DH.c gate: NOT AUTHORIZED (no q=4 arm at rung 6+/7 on 5/5 seeds; a q=8-only pass \
             cannot authorize)"
        );
    } else {
        println!(
            "DH.c gate: a FUTURE DH.c grand-oracle rerun is AUTHORIZED by the q=4 arm(s) \
             {gate:?} — reported only, never executed here"
        );
    }
    println!(
        "wall clock: {:.1}s for {} cells",
        grid.wall_secs,
        grid.cells.len()
    );

    // The hard-guard discipline: exactly 60 cells ran, all guard-valid.
    assert_eq!(grid.cells.len(), 60, "exactly 60 cells");
    assert!(
        !grid.any_guard_failure
            && !matches!(grid.synthesis, BurdenSynthesis::InvalidGrid)
            && grid
                .cells
                .iter()
                .all(|c| { !matches!(c.verdict, BurdenCellVerdict::PreconditionInvalid { .. }) }),
        "hard-guard failure: see the PreconditionInvalid lines above"
    );
}

/// §5.6a — NO digest tag: the 12 arm configurations are pairwise distinguished by EXISTING
/// canonical state (q rides the demography bytes; the active motive emits tag 31), and each cell
/// differs from the landed base by EXACTLY the two swept fields (§7.4 — reverting them restores
/// the canonical config byte-for-byte).
#[test]
fn twelve_arm_configs_are_pairwise_distinguished_by_canonical_state() {
    let mut bytes: Vec<((u32, BurdenSavingArm), Vec<u8>)> = Vec::new();
    for &q in &BURDEN_QS {
        for arm in [BurdenSavingArm::Off, BurdenSavingArm::On] {
            let mut cfg = cell_config(q, arm);
            // Nothing-else-changed: reverting the two swept fields restores the exact base.
            cfg.demography
                .as_mut()
                .expect("demography")
                .child_food_endowment = 4;
            let chain = cfg.chain.as_mut().expect("chain");
            chain.birth_stock_saving = false;
            chain.birth_stock_saving_mode = BirthStockSavingMode::Off;
            assert_eq!(
                cfg,
                SettlementConfig::frontier_closed_circulation(),
                "cell (q={q}, {arm:?}) must differ from the base by exactly the two fields"
            );
            let s = Settlement::generate(3, &cell_config(q, arm));
            bytes.push(((q, arm), s.canonical_bytes()));
        }
    }
    assert_eq!(bytes.len(), 12);
    for i in 0..bytes.len() {
        for j in i + 1..bytes.len() {
            assert_ne!(
                bytes[i].1, bytes[j].1,
                "arms {:?} and {:?} must differ in canonical state",
                bytes[i].0, bytes[j].0
            );
        }
    }
}

/// §5.6b — the (q=4, Off) cell's config is byte-identical to the landed closed base: the sweep
/// changes NOTHING at the canonical point.
#[test]
fn canonical_cell_config_is_identical_to_the_landed_closed_base() {
    assert_eq!(
        cell_config(4, BurdenSavingArm::Off),
        SettlementConfig::frontier_closed_circulation(),
        "the (q=4, Off) cell IS the landed closed base"
    );
    // …and its generated settlement is byte-identical too.
    let cell = Settlement::generate(3, &cell_config(4, BurdenSavingArm::Off));
    let base = Settlement::generate(3, &SettlementConfig::frontier_closed_circulation());
    assert_eq!(cell.canonical_bytes(), base.canonical_bytes());
}

/// Flags-off inertness at the config seam: a NON-closed run (the C3R.d saving headline, which
/// shares the demography + saving fields) records NO burden telemetry — every hook gates
/// through `closure_active()`.
#[test]
fn burden_telemetry_is_inert_off_the_closed_marker() {
    let cfg = SettlementConfig::frontier_mortal_producers_saving();
    let mut s = Settlement::generate(3, &cfg);
    for _ in 0..200 {
        s.econ_tick();
    }
    assert!(s.burden_birth_events().is_empty());
    assert!(s.burden_birth_funding_records().is_empty());
    assert!(s.burden_tool_inheritances().is_empty());
    assert!(s.burden_role_adoptions().is_empty());
    assert!(s.burden_stage_executions().is_empty());
    assert!(s.burden_trade_records().is_empty());
    assert!(s.burden_seam_violations().is_empty());
    assert!(s.burden_lot_audit().is_empty());
}
