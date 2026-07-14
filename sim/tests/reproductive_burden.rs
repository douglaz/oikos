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

use std::collections::{BTreeMap, BTreeSet};
use std::time::Instant;

use sim::{
    build_burden_windows, burden_dh_c_gate, burden_motive_effect, burden_nonmonotone_pairs,
    burden_start_tick, classify_birth_funding, classify_burden_cell, synthesize_burden_grid,
    AgentId, BirthStockSavingMode, BurdenBirthObs, BurdenCellInput, BurdenCellResult,
    BurdenCellVerdict, BurdenSavingArm, BurdenSynthesis, BurdenWindowObs, ClosureClass, GoodId,
    Settlement, SettlementConfig, BURDEN_QS, BURDEN_RUN_TICKS, BURDEN_SEEDS,
};

const PRODUCER_FOUNDERS: usize = 6;
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

struct CellRun {
    verdict: BurdenCellVerdict,
    founder_deaths: Vec<(AgentId, u64)>,
    start: u64,
    windows: Vec<BurdenWindowObs>,
    births: usize,
    guard_failures: Vec<String>,
}

/// Run one cell and classify it. Every hard-guard violation is collected (and later printed as
/// `PreconditionInvalid { guard }` + asserted by the caller).
fn run_cell(q: u32, arm: BurdenSavingArm, seed: u64) -> CellRun {
    let cfg = cell_config(q, arm);
    let mut s = Settlement::generate(seed, &cfg);
    let mut guards: Vec<String> = Vec::new();

    // Founders (R1-5): the generation-time closure-registry snapshot of the six initial
    // Miller/Baker AgentIds (NOT HouseholdSpec::founders, which is 0 for these households).
    let founders: BTreeMap<AgentId, ClosureClass> = s
        .closure_registry()
        .iter()
        .filter(|(_, &class)| matches!(class, ClosureClass::Miller | ClosureClass::Baker))
        .map(|(&id, &class)| (id, class))
        .collect();
    if founders.len() != PRODUCER_FOUNDERS {
        guards.push(format!(
            "founder snapshot: want {PRODUCER_FOUNDERS} producer founders, got {}",
            founders.len()
        ));
    }
    let (mill_good, oven_good): (GoodId, GoodId) = {
        let chain = cfg.chain.as_ref().expect("chain");
        (chain.content.mill(), chain.content.oven())
    };

    let mut founder_alive: BTreeMap<AgentId, bool> =
        founders.keys().map(|&id| (id, true)).collect();
    let mut founder_deaths: Vec<(AgentId, u64)> = Vec::new();
    // Fixed household classes: a registered id's class must never change (the closure-registry
    // invariant, per tick, over every living actor).
    let mut fixed_class: BTreeMap<AgentId, ClosureClass> = BTreeMap::new();
    let mut continuity_by_tick: Vec<[bool; 2]> = Vec::with_capacity(BURDEN_RUN_TICKS as usize);
    let mut conserved = true;
    let mut money_ok = true;

    for tick in 0..BURDEN_RUN_TICKS {
        let report = s.econ_tick();
        conserved &= report.conserves();
        money_ok &= report.money_conserves();

        let living = s.burden_living_snapshot();
        let born: BTreeSet<AgentId> = s.burden_birth_events().iter().map(|b| b.child).collect();
        let mut continuity = [false; 2];
        for &(id, class, has_lifespan) in &living {
            // Registry invariant: every living actor maps to a class, and the class is fixed.
            let Some(class) = class else {
                guards.push(format!(
                    "registry invariant: living {id:?} has no class (t={tick})"
                ));
                continue;
            };
            match fixed_class.get(&id) {
                Some(&prior) if prior != class => guards.push(format!(
                    "registry invariant: {id:?} class changed {prior:?}→{class:?} (t={tick})"
                )),
                None => {
                    fixed_class.insert(id, class);
                }
                _ => {}
            }
            // Zero immortal producers: no lifespan-less agent maps to a producer class.
            let producer = matches!(class, ClosureClass::Miller | ClosureClass::Baker);
            if producer && !has_lifespan {
                guards.push(format!(
                    "immortal producer {id:?} in class {class:?} (t={tick})"
                ));
            }
            // Criterion-1 sampling: a living NONFOUNDER class member backed by a birth event.
            if producer && !founders.contains_key(&id) && born.contains(&id) {
                let slot = match class {
                    ClosureClass::Miller => 0,
                    _ => 1,
                };
                continuity[slot] = true;
            }
        }
        continuity_by_tick.push(continuity);

        for (&fid, alive) in founder_alive.iter_mut() {
            if *alive && !living.iter().any(|&(id, _, _)| id == fid) {
                *alive = false;
                founder_deaths.push((fid, tick));
            }
        }

        // Zero intervention/support-origin stock, and the per-tick corruption audits.
        if s.acquisition_intervention_held() != 0 {
            guards.push(format!("intervention-origin stock held (t={tick})"));
        }
        for v in s.burden_lot_audit() {
            guards.push(format!("lot audit (t={tick}): {v}"));
        }
    }
    if !conserved {
        guards.push("conservation failed".to_string());
    }
    if !money_ok {
        guards.push("money conservation failed".to_string());
    }
    for v in s.burden_seam_violations() {
        guards.push(format!("seam violation: {v}"));
    }
    if s.closure_event_tape()
        .iter()
        .any(|e| matches!(e.kind, sim::ClosureEventKind::BSupportCredit { .. }))
    {
        guards.push("support-origin credit on the closed base".to_string());
    }

    // The saving TARGET equals q, per cell (§6): the On arm's motive emits Next-horizon wants
    // (q per eligible member per tick — nonzero whenever q>0, since the founders are alive and
    // below cap early), the Off arm and the q=0 no-op emit none.
    let wants = s.birth_stock_wants_emitted();
    match (arm, q) {
        (BurdenSavingArm::On, q) if q > 0 => {
            if wants == 0 {
                guards.push(format!("saving arm On with q={q} emitted zero wants"));
            }
        }
        _ => {
            if wants != 0 {
                guards.push(format!("arm {arm:?} q={q} emitted {wants} wants (want 0)"));
            }
        }
    }

    // Every founder dead before `start` (the formula makes this equivalent to: every founder
    // died within the run).
    if founder_deaths.len() != founders.len() {
        guards.push(format!(
            "founder extinction: {}/{} founders died within the run",
            founder_deaths.len(),
            founders.len()
        ));
    }

    // Funding-attribution completeness (§3.4, multiplicity-aware — R3-3).
    let births = s.burden_birth_events();
    let funding = s.burden_birth_funding_records();
    let birth_ids: BTreeSet<AgentId> = births.iter().map(|b| b.child).collect();
    let funding_ids: BTreeSet<AgentId> = funding.iter().map(|f| f.child).collect();
    if birth_ids.len() != births.len() {
        guards.push("duplicate BirthOccurred child id".to_string());
    }
    if funding_ids.len() != funding.len() {
        guards.push("duplicate funding-record child id".to_string());
    }
    if births.len() != funding.len() || birth_ids != funding_ids {
        guards.push(format!(
            "stream completeness: {} BirthOccurred vs {} funding records",
            births.len(),
            funding.len()
        ));
    }
    for f in funding {
        if f.q != q {
            guards.push(format!("funding record q={} in a q={q} cell", f.q));
        }
        let drawn: u64 = f.lots.iter().map(|l| l.qty).sum();
        if q > 0 && drawn != u64::from(q) {
            guards.push(format!(
                "funding record for {:?} drew {drawn} units, want q={q}",
                f.child
            ));
        }
        if classify_birth_funding(&f.lots, s.burden_trade_records()).unverifiable {
            guards.push(format!(
                "funding record for {:?} is unverifiable live telemetry",
                f.child
            ));
        }
    }

    let last_death = founder_deaths.iter().map(|&(_, t)| t).max().unwrap_or(0);
    let start = burden_start_tick(last_death);
    let windows = build_burden_windows(
        start,
        BURDEN_RUN_TICKS,
        &continuity_by_tick,
        s.burden_stage_executions(),
    );

    let verdict = if guards.is_empty() {
        let input = BurdenCellInput {
            q,
            mill_good,
            oven_good,
            windows: windows.clone(),
            inheritances: s.burden_tool_inheritances().to_vec(),
            adoptions: s.burden_role_adoptions().to_vec(),
            executions: s.burden_stage_executions().to_vec(),
            births: births
                .iter()
                .map(|b| BurdenBirthObs {
                    tick: b.tick,
                    child: b.child,
                    funding: {
                        let f = funding
                            .iter()
                            .find(|f| f.child == b.child)
                            .expect("completeness guard passed");
                        classify_birth_funding(&f.lots, s.burden_trade_records())
                    },
                })
                .collect(),
        };
        classify_burden_cell(&input)
    } else {
        BurdenCellVerdict::PreconditionInvalid {
            guard: guards.join("; "),
        }
    };

    CellRun {
        verdict,
        founder_deaths,
        start,
        windows,
        births: births.len(),
        guard_failures: guards,
    }
}

/// The 60-cell audit: serial, seeds outermost, q ascending, Off-then-On (§3). Scientific
/// outcomes print AND are machine-bound against the committed audit-table golden; the hard
/// guards additionally fail the suite on violation.
#[test]
fn reproductive_burden_sixty_cell_audit() {
    let wall = Instant::now();
    let mut cells: Vec<BurdenCellResult> = Vec::new();
    let mut births_by_cell: BTreeMap<(u32, BurdenSavingArm, u64), usize> = BTreeMap::new();
    let mut audit_rows: Vec<String> = Vec::new();
    let mut any_guard_failure = false;

    for &seed in &BURDEN_SEEDS {
        for &q in &BURDEN_QS {
            for arm in [BurdenSavingArm::Off, BurdenSavingArm::On] {
                let run = run_cell(q, arm, seed);
                println!(
                    "cell seed={seed} q={q} arm={arm:?}: start={} windows={} births={} \
                     founder_deaths={:?}",
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
                    any_guard_failure = true;
                    println!(
                        "  PreconditionInvalid {{ guard: {:?} }}",
                        run.guard_failures
                    );
                }
                println!("  verdict[rung {}]: {:?}", run.verdict.rung(), run.verdict);
                births_by_cell.insert((q, arm, seed), run.births);
                audit_rows.push(format!(
                    "audit_cell|seed={seed}|q={q}|arm={arm:?}|births={}|rung={}|verdict={:?}",
                    run.births,
                    run.verdict.rung(),
                    run.verdict
                ));
                cells.push(BurdenCellResult {
                    q,
                    arm,
                    seed,
                    verdict: run.verdict,
                });
            }
        }
    }

    // Stable, machine-bound evidence for every published cell figure (P2-4): unlike the
    // diagnostic prose above, each row binds births and verdict in one fixed-format record and
    // the committed golden makes drift a test failure.
    let cell_audit = format!("{}\n", audit_rows.join("\n"));
    println!("\nstructured per-cell audit:\n{cell_audit}");
    assert_eq!(
        cell_audit, CELL_AUDIT_GOLDEN,
        "the published 60-cell verdict+birth figures changed"
    );

    // Compact paired evidence for the rung-level motive null (P1-1). Deltas are On minus Off;
    // all six burdens print, including the q=0 reachability control.
    println!("paired births table (delta = On - Off):");
    for &q in &BURDEN_QS {
        let off: Vec<usize> = BURDEN_SEEDS
            .iter()
            .map(|&seed| births_by_cell[&(q, BurdenSavingArm::Off, seed)])
            .collect();
        let on: Vec<usize> = BURDEN_SEEDS
            .iter()
            .map(|&seed| births_by_cell[&(q, BurdenSavingArm::On, seed)])
            .collect();
        let delta: Vec<i64> = on
            .iter()
            .zip(&off)
            .map(|(&on, &off)| on as i64 - off as i64)
            .collect();
        println!(
            "paired_births|q={q}|seeds={BURDEN_SEEDS:?}|off={off:?}|on={on:?}|delta={delta:?}"
        );
    }

    // The full per-arm per-seed rung table (always printed, R1-3).
    println!("\nper-arm per-seed rung table (rows q/arm, columns seeds {BURDEN_SEEDS:?}):");
    for &q in &BURDEN_QS {
        for arm in [BurdenSavingArm::Off, BurdenSavingArm::On] {
            let rungs: Vec<u8> = BURDEN_SEEDS
                .iter()
                .map(|&seed| {
                    cells
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

    let synthesis = synthesize_burden_grid(&cells);
    println!("\nsynthesis: {synthesis:?}");
    println!("motive_effect: {:?}", burden_motive_effect(&cells));
    let nonmonotone = burden_nonmonotone_pairs(&cells);
    println!("nonmonotone: {nonmonotone:?}");
    assert!(
        nonmonotone.iter().all(|&(_, lo, hi)| lo != 0 && hi != 0),
        "no nonmonotone tuple contains q=0 (R4-2)"
    );
    let gate = burden_dh_c_gate(&cells);
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
        wall.elapsed().as_secs_f64(),
        cells.len()
    );

    // The hard-guard discipline: exactly 60 cells ran, all guard-valid.
    assert_eq!(cells.len(), 60, "exactly 60 cells");
    assert!(
        !any_guard_failure
            && !matches!(synthesis, BurdenSynthesis::InvalidGrid)
            && cells
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
