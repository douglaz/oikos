//! DH.b-obs (impl-70) — the birth-gate-stock diagnostic (`docs/impl-birth-gate-obs.md`).
//!
//! A PURE-OBSERVATION slice on the exact DH.b 60-cell grid, run ONCE under observation:
//! decompose WHY no producer-household member ever holds q≥3 free loaves at the atomic
//! single-member birth gate. The oracle proves the DH.b golden reproduces byte-identically under
//! observation (verdicts, synthesis, `reproductive_burden_cells.txt`), derived through the SAME
//! shared `run_burden_grid` harness; the hard guards (GateDecisionMismatch==0, q=4 non-vacuity,
//! the independent recount equality set, the WindowStart→gate replay reconciliation) FAIL the
//! suite; the share decomposition is PRINTED, never asserted.

use std::collections::BTreeMap;
use std::fmt::Write as _;
use std::time::Instant;

use sim::{
    birth_gate_replay_reconciles, classify_birth_gate_opportunity, run_burden_grid, AgentId,
    BirthGateEvent, BirthGateOutcome, BirthGateRawOpportunity, BirthGateReport, BirthGateReportRow,
    BirthGateState, BirthGateTally, BirthStockSavingMode, BurdenSavingArm, BurdenSynthesis,
    ClosureClass, EventCause, Settlement, SettlementConfig, BURDEN_PRODUCER_CLASSES, BURDEN_QS,
    BURDEN_SEEDS,
};

const CELL_AUDIT_GOLDEN: &str = include_str!("goldens/reproductive_burden_cells.txt");

// ===========================================================================================
// The shared-harness behavioral oracle (asserted) + the printed decomposition — ONE execution
// ===========================================================================================

#[test]
fn birth_gate_obs_oracle_and_decomposition() {
    let wall = Instant::now();
    // ONE execution under observation (the flag set; behavior is inert).
    let grid = run_burden_grid(true);

    // ---- The DH.b behavioral oracle: byte-identical golden, synthesis, paired table, derived
    // (never printed as constants) from the SAME single execution.
    let cell_audit = format!("{}\n", grid.audit_rows.join("\n"));
    assert_eq!(
        cell_audit, CELL_AUDIT_GOLDEN,
        "the DH.b 60-cell golden must reproduce byte-identically under observation"
    );
    assert_eq!(grid.cells.len(), 60, "exactly 60 cells");
    assert!(
        !grid.any_guard_failure,
        "no DH.b hard-guard failure under observation"
    );
    assert!(
        !matches!(grid.synthesis, BurdenSynthesis::InvalidGrid),
        "the DH.b synthesis must not be InvalidGrid"
    );
    // The landed DH.b synthesis rung (CostlessOnlyReplacement — the q>0 cells never clear the
    // wall; q=0 is the costless control).
    assert!(
        matches!(grid.synthesis, BurdenSynthesis::CostlessOnlyReplacement),
        "the landed DH.b synthesis is CostlessOnlyReplacement, got {:?}",
        grid.synthesis
    );

    let opportunities_by_cell = grid
        .opportunities_by_cell
        .as_ref()
        .expect("observe=true yields Some(opportunities)");
    let recount_by_cell = grid
        .recount_by_cell
        .as_ref()
        .expect("observe=true yields Some(recount)");

    // ---- The report (the SOLE reducer): classify every raw opportunity exactly once.
    let rows: Vec<BirthGateReportRow<'_>> = opportunities_by_cell
        .iter()
        .flat_map(|(&(q, arm, seed), list)| {
            list.iter().map(move |opportunity| BirthGateReportRow {
                q,
                arm,
                seed,
                opportunity,
            })
        })
        .collect();
    let report = BirthGateReport::from_traces(&rows);

    // ---- HARD guard: GateDecisionMismatch is zero grid-wide (a suite failure, not a result).
    assert_eq!(
        report.global.gate_decision_mismatch, 0,
        "GateDecisionMismatch must be zero grid-wide (the observer/gate contradiction)"
    );

    // ---- HARD guard: the WindowStart→gate replay reconciles with the authoritative gate state
    // for EVERY opportunity.
    for list in opportunities_by_cell.values() {
        for opportunity in list {
            assert!(
                birth_gate_replay_reconciles(opportunity),
                "the WindowStart→gate replay must reconcile with the gate state \
                 (tick {}, household {})",
                opportunity.tick,
                opportunity.household
            );
        }
    }

    // ---- HARD guard: totality — passes + failures = opportunities, grid-wide and per cell.
    assert_eq!(
        report.global.passes + report.global.failures,
        report.global.opportunities,
        "passes + failures must equal opportunities"
    );

    // ---- HARD guard: Miller + Baker = global, COMPONENT-WISE (opportunities, passes, failures,
    // every scientific outcome, and each crossing cause), every raw opportunity resolving to
    // exactly one closure class.
    let miller = report
        .by_type
        .get(&ClosureClass::Miller)
        .cloned()
        .unwrap_or_default();
    let baker = report
        .by_type
        .get(&ClosureClass::Baker)
        .cloned()
        .unwrap_or_default();
    assert_component_wise_sum(&miller, &baker, &report.global);

    // ---- HARD guard: machine-enforced q=4 non-vacuity — every q=4 cell has
    // independent_opportunities > 0 and independent_failures == independent_opportunities.
    for (&(q, arm, seed), obs) in recount_by_cell {
        if q != 4 {
            continue;
        }
        let (r_opp, _r_pass, r_fail, _by_type) = recount_tally(obs);
        assert!(
            r_opp > 0,
            "q=4 non-vacuity: cell (q={q}, {arm:?}, seed={seed}) must have >0 independent \
             opportunities"
        );
        assert_eq!(
            r_fail, r_opp,
            "q=4 non-vacuity: cell (q={q}, {arm:?}, seed={seed}) — every otherwise-eligible \
             opportunity must fail the endowment gate"
        );
    }

    // ---- HARD guard: the INDEPENDENT denominator recount, per cell. Replays the gates from the
    // pre-run_births snapshot (never reading the observer or birth_block_* counters); the full
    // component-wise equality set + Miller + Baker = global (component-wise).
    for (&(q, arm, seed), obs) in recount_by_cell {
        let (r_opp, r_pass, r_fail, r_by_type) = recount_tally(obs);
        // The observer tally for this cell (classified independently of the recount).
        let opportunities = &opportunities_by_cell[&(q, arm, seed)];
        let cell_report = BirthGateReport::from_traces(
            &opportunities
                .iter()
                .map(|opportunity| BirthGateReportRow {
                    q,
                    arm,
                    seed,
                    opportunity,
                })
                .collect::<Vec<_>>(),
        );
        assert_eq!(
            r_opp, cell_report.global.opportunities,
            "recount opportunities == observer opportunities (q={q}, {arm:?}, seed={seed})"
        );
        assert_eq!(
            r_pass, cell_report.global.passes,
            "recount passes == observer passes (q={q}, {arm:?}, seed={seed})"
        );
        assert_eq!(
            r_fail, cell_report.global.failures,
            "recount failures == observer failures (q={q}, {arm:?}, seed={seed})"
        );
        assert_eq!(
            r_pass, obs.births,
            "recount passes == births delta (q={q}, {arm:?}, seed={seed})"
        );
        assert_eq!(
            r_fail, obs.birth_block_endowment,
            "recount failures == birth_block_endowment delta (q={q}, {arm:?}, seed={seed})"
        );
        // Miller + Baker = global (component-wise) for the recount opportunities.
        let miller_opp = r_by_type.get(&ClosureClass::Miller).copied().unwrap_or(0);
        let baker_opp = r_by_type.get(&ClosureClass::Baker).copied().unwrap_or(0);
        assert_eq!(
            miller_opp + baker_opp,
            r_opp,
            "recount Miller + Baker == global opportunities (q={q}, {arm:?}, seed={seed})"
        );
    }

    // ---- The decomposition — PRINTED, never asserted.
    print_decomposition(&report);
    println!(
        "\nDH.b-obs complete: 60 cells executed ONCE under observation in {:.1}s \
         (~the DH.b wall clock + the trace overhead, not doubled).",
        grid.wall_secs
    );
    println!(
        "suite wall clock (incl. the join + report): {:.1}s",
        wall.elapsed().as_secs_f64()
    );
}

/// Assert `miller + baker == global` component-wise over every tally field the report exposes.
fn assert_component_wise_sum(
    miller: &BirthGateTally,
    baker: &BirthGateTally,
    global: &BirthGateTally,
) {
    assert_eq!(
        miller.opportunities + baker.opportunities,
        global.opportunities,
        "opportunities: Miller + Baker == global"
    );
    assert_eq!(miller.passes + baker.passes, global.passes, "passes");
    assert_eq!(
        miller.failures + baker.failures,
        global.failures,
        "failures"
    );
    assert_eq!(
        miller.gate_decision_mismatch + baker.gate_decision_mismatch,
        global.gate_decision_mismatch,
        "gate_decision_mismatch"
    );
    assert_eq!(
        miller.member_drained + baker.member_drained,
        global.member_drained,
        "member_drained"
    );
    assert_eq!(
        miller.split_at_gate + baker.split_at_gate,
        global.split_at_gate,
        "split_at_gate"
    );
    assert_eq!(
        miller.household_drained + baker.household_drained,
        global.household_drained,
        "household_drained"
    );
    assert_eq!(
        miller.never_reached_q + baker.never_reached_q,
        global.never_reached_q,
        "never_reached_q"
    );
    // Every crossing-cause total: Miller + Baker == global.
    let mut causes: std::collections::BTreeSet<EventCause> =
        global.crossing_causes.keys().copied().collect();
    causes.extend(miller.crossing_causes.keys());
    causes.extend(baker.crossing_causes.keys());
    for cause in causes {
        let m = miller.crossing_causes.get(&cause).copied().unwrap_or(0);
        let b = baker.crossing_causes.get(&cause).copied().unwrap_or(0);
        let g = global.crossing_causes.get(&cause).copied().unwrap_or(0);
        assert_eq!(
            m + b,
            g,
            "crossing cause {cause:?}: Miller + Baker == global"
        );
    }
}

/// Recount tallies over one cell's independent snapshots: `(opportunities, passes, failures,
/// opportunities-by-producer-type)`. Reads NEITHER the observer NOR any birth_block_* counter.
fn recount_tally(obs: &sim::BirthGateCellObs) -> (u64, u64, u64, BTreeMap<ClosureClass, u64>) {
    let mut opp = 0u64;
    let mut pass = 0u64;
    let mut fail = 0u64;
    let mut by_type: BTreeMap<ClosureClass, u64> = BTreeMap::new();
    for snap in &obs.recount {
        if snap.reached_endowment_gate() {
            opp += 1;
            *by_type.entry(snap.producer_type).or_insert(0) += 1;
            if snap.gate_passed() {
                pass += 1;
            } else {
                fail += 1;
            }
        }
    }
    (opp, pass, fail, by_type)
}

fn print_decomposition(report: &BirthGateReport) {
    print!("{}", render_decomposition(report));
}

fn render_decomposition(report: &BirthGateReport) -> String {
    let mut rendered = String::new();
    writeln!(
        rendered,
        "DH.b-obs birth-gate-stock decomposition — printed, never asserted:"
    )
    .expect("writing to a String is infallible");
    writeln!(
        rendered,
        "  GLOBAL: opps={} passes={} failures={} | member_drained={} split_at_gate={} \
         household_drained={} never_reached_q={}",
        report.global.opportunities,
        report.global.passes,
        report.global.failures,
        report.global.member_drained,
        report.global.split_at_gate,
        report.global.household_drained,
        report.global.never_reached_q,
    )
    .expect("writing to a String is infallible");
    writeln!(
        rendered,
        "  crossing causes: {:?}",
        report.global.crossing_causes
    )
    .expect("writing to a String is infallible");
    writeln!(
        rendered,
        "  household_peak distribution (failures): {:?}",
        report.global.peak_hist
    )
    .expect("writing to a String is infallible");
    writeln!(
        rendered,
        "  gap_to_q distribution (failures): {:?}",
        report.global.gap_hist
    )
    .expect("writing to a String is infallible");
    for producer_type in BURDEN_PRODUCER_CLASSES {
        let tally = report
            .by_type
            .get(&producer_type)
            .cloned()
            .unwrap_or_default();
        writeln!(
            rendered,
            "  {producer_type:?}: opps={} passes={} failures={} | member_drained={} \
             split={} household_drained={} never_reached_q={} crossing={:?}",
            tally.opportunities,
            tally.passes,
            tally.failures,
            tally.member_drained,
            tally.split_at_gate,
            tally.household_drained,
            tally.never_reached_q,
            tally.crossing_causes,
        )
        .expect("writing to a String is infallible");
    }
    // Per (q, arm, producer_type) stratum — the headline q∈{1,2}→q≥3 contrast, arms NOT merged.
    writeln!(
        rendered,
        "  per (q, arm, producer_type) stratum [raw counts always; NA for zero denominators]:"
    )
    .expect("writing to a String is infallible");
    for q in BURDEN_QS {
        for arm in [BurdenSavingArm::Off, BurdenSavingArm::On] {
            for producer_type in BURDEN_PRODUCER_CLASSES {
                let tally = report
                    .by_stratum
                    .get(&(q, arm, producer_type))
                    .cloned()
                    .unwrap_or_default();
                let never = tally.share(tally.never_reached_q);
                let member = tally.share(tally.member_drained);
                let split = tally.share(tally.split_at_gate);
                let household = tally.share(tally.household_drained);
                writeln!(
                    rendered,
                    "    STRATUM q={q} {arm:?} {producer_type:?}: opps={} passes={} failures={} => \
                     never_reached={never} member_drained={member} split={split} \
                     household_drained={household}",
                    tally.opportunities, tally.passes, tally.failures,
                )
                .expect("writing to a String is infallible");
            }
        }
    }

    writeln!(
        rendered,
        "  per (q, arm, seed, producer_type) cell [raw counts always; NA for zero denominators]:"
    )
    .expect("writing to a String is infallible");
    for q in BURDEN_QS {
        for arm in [BurdenSavingArm::Off, BurdenSavingArm::On] {
            for seed in BURDEN_SEEDS {
                for producer_type in BURDEN_PRODUCER_CLASSES {
                    let tally = report
                        .by_cell
                        .get(&(q, arm, seed, producer_type))
                        .cloned()
                        .unwrap_or_default();
                    let never = tally.share(tally.never_reached_q);
                    let member = tally.share(tally.member_drained);
                    let split = tally.share(tally.split_at_gate);
                    let household = tally.share(tally.household_drained);
                    writeln!(
                        rendered,
                        "    CELL q={q} {arm:?} seed={seed} {producer_type:?}: opps={} passes={} \
                         failures={} => never_reached={never} member_drained={member} split={split} \
                         household_drained={household}",
                        tally.opportunities, tally.passes, tally.failures,
                    )
                    .expect("writing to a String is infallible");
                }
            }
        }
    }

    rendered
}

#[test]
fn decomposition_renders_every_stratum_and_cell_including_empty_ones() {
    let populated = BirthGateTally {
        opportunities: 1,
        failures: 1,
        never_reached_q: 1,
        ..BirthGateTally::default()
    };
    let mut report = BirthGateReport::default();
    report.by_stratum.insert(
        (4, BurdenSavingArm::Off, ClosureClass::Miller),
        populated.clone(),
    );
    report.by_cell.insert(
        (4, BurdenSavingArm::Off, 3, ClosureClass::Miller),
        populated,
    );

    let rendered = render_decomposition(&report);
    assert_eq!(
        rendered
            .lines()
            .filter(|line| line.trim_start().starts_with("STRATUM "))
            .count(),
        BURDEN_QS.len() * 2 * BURDEN_PRODUCER_CLASSES.len()
    );
    assert_eq!(
        rendered
            .lines()
            .filter(|line| line.trim_start().starts_with("CELL "))
            .count(),
        BURDEN_QS.len() * 2 * BURDEN_SEEDS.len() * BURDEN_PRODUCER_CLASSES.len()
    );
    assert!(rendered
        .contains("STRATUM q=4 Off Miller: opps=1 passes=0 failures=1 => never_reached=1.000"));
    assert!(rendered
        .contains("CELL q=4 Off seed=3 Miller: opps=1 passes=0 failures=1 => never_reached=1.000"));
    assert!(rendered.contains(
        "CELL q=8 On seed=23 Baker: opps=0 passes=0 failures=0 => \
         never_reached=NA (denominator=0)"
    ));
}

// ===========================================================================================
// The classifier table tests (all branches, precedence, crossing event, baseline, mismatch)
// ===========================================================================================

fn a(n: u64) -> AgentId {
    AgentId(n)
}

fn ev(cause: EventCause, updates: &[(AgentId, u32)]) -> BirthGateEvent {
    BirthGateEvent {
        cause,
        updates: updates.to_vec(),
    }
}

fn classify(
    window_start: &[(AgentId, u32)],
    events: &[BirthGateEvent],
    gate_members: &[(AgentId, u32)],
    recorded_pass: bool,
    q: u32,
) -> BirthGateOutcome {
    classify_birth_gate_opportunity(
        window_start,
        events,
        &BirthGateState {
            members: gate_members.to_vec(),
            recorded_pass,
        },
        q,
    )
    .outcome
}

#[test]
fn classifier_gate_passed_and_baseline_at_q() {
    // A member holds q at the baseline (event 0) and holds it at the gate → GatePassed.
    let outcome = classify(
        &[(a(1), 4), (a(2), 0)],
        &[],
        &[(a(1), 4), (a(2), 0)],
        true,
        4,
    );
    assert_eq!(outcome, BirthGateOutcome::GatePassed);
}

#[test]
fn classifier_gate_decision_mismatch_both_directions() {
    // recorded PASS, recomputed FAIL (no member ≥ q at the gate) → mismatch.
    let m1 = classify(&[(a(1), 2)], &[], &[(a(1), 2)], true, 4);
    assert_eq!(
        m1,
        BirthGateOutcome::GateDecisionMismatch {
            recorded: true,
            recomputed: false
        }
    );
    // recorded FAIL, recomputed PASS (a member ≥ q at the gate) → mismatch.
    let m2 = classify(&[(a(1), 4)], &[], &[(a(1), 4)], false, 4);
    assert_eq!(
        m2,
        BirthGateOutcome::GateDecisionMismatch {
            recorded: false,
            recomputed: true
        }
    );
}

#[test]
fn classifier_member_drained_by_consumption() {
    // A member reaches q at a Production then Consumption drops it below q → MemberDrained, crossing
    // = the Consumption event.
    let events = [
        ev(EventCause::Production, &[(a(1), 4)]),
        ev(EventCause::Consumption, &[(a(1), 1)]),
    ];
    let outcome = classify(
        &[(a(1), 0), (a(2), 0)],
        &events,
        &[(a(1), 1), (a(2), 0)],
        false,
        4,
    );
    assert_eq!(
        outcome,
        BirthGateOutcome::MemberDrainedBeforeGate {
            crossing: EventCause::Consumption,
            member: a(1),
        }
    );
}

#[test]
fn classifier_member_drained_focal_tie_rule() {
    // Two members both reach q then drain; the focal member is the highest peak, then earliest
    // at-or-above-q event, then lowest AgentId. Member 2 peaks at 6 (> member 1's 4) → focal = 2.
    let events = [
        ev(EventCause::Production, &[(a(1), 4)]),
        ev(EventCause::Production, &[(a(2), 6)]),
        ev(EventCause::AskChange, &[(a(1), 0)]),
        ev(EventCause::Consumption, &[(a(2), 1)]),
    ];
    let outcome = classify(
        &[(a(1), 0), (a(2), 0)],
        &events,
        &[(a(1), 0), (a(2), 1)],
        false,
        4,
    );
    assert_eq!(
        outcome,
        BirthGateOutcome::MemberDrainedBeforeGate {
            crossing: EventCause::Consumption,
            member: a(2),
        }
    );
}

#[test]
fn classifier_member_drained_by_ask_change_only() {
    // A member reaches q at the baseline and an AskChange (sale reservation) is the ONLY drop —
    // crossing = AskChange (the sale-lock candidate lever).
    let events = [ev(EventCause::AskChange, &[(a(1), 1)])];
    let outcome = classify(&[(a(1), 4)], &events, &[(a(1), 1)], false, 4);
    assert_eq!(
        outcome,
        BirthGateOutcome::MemberDrainedBeforeGate {
            crossing: EventCause::AskChange,
            member: a(1),
        }
    );
}

#[test]
fn classifier_split_at_gate() {
    // No single member ever reaches q, but the household TOTAL is ≥ q at the gate.
    let outcome = classify(
        &[(a(1), 2), (a(2), 2)],
        &[],
        &[(a(1), 2), (a(2), 2)],
        false,
        4,
    );
    assert_eq!(outcome, BirthGateOutcome::SplitAtGate);
}

#[test]
fn classifier_saturates_household_totals() {
    let classified = classify_birth_gate_opportunity(
        &[(a(1), u32::MAX - 1), (a(2), 2)],
        &[],
        &BirthGateState {
            members: vec![(a(1), u32::MAX - 1), (a(2), 2)],
            recorded_pass: false,
        },
        u32::MAX,
    );
    assert_eq!(classified.outcome, BirthGateOutcome::SplitAtGate);
    assert_eq!(classified.household_peak, u32::MAX);
    assert_eq!(classified.gap_to_q, 0);
    assert_eq!(classified.gate_household_total, u32::MAX);
}

#[test]
fn classifier_household_drained() {
    // No single member ever reaches q; the total reaches q (2+2) then a member's sale drops the
    // total below q at the gate → HouseholdDrained, crossing = the reducing event.
    let events = [ev(EventCause::SettledTradeSell, &[(a(1), 0)])];
    let outcome = classify(
        &[(a(1), 2), (a(2), 2)],
        &events,
        &[(a(1), 0), (a(2), 2)],
        false,
        4,
    );
    assert_eq!(
        outcome,
        BirthGateOutcome::HouseholdDrainedBeforeGate {
            crossing: EventCause::SettledTradeSell,
        }
    );
}

#[test]
fn classifier_never_reached_q() {
    // The household total never reaches q at any event.
    let events = [ev(EventCause::Consumption, &[(a(1), 0)])];
    let outcome = classify(
        &[(a(1), 1), (a(2), 1)],
        &events,
        &[(a(1), 0), (a(2), 1)],
        false,
        4,
    );
    assert_eq!(outcome, BirthGateOutcome::NeverReachedQ);
}

#[test]
fn classifier_same_household_transfer_conserves_total() {
    // A same-household SettledTrade (seller→buyer, both members) moves one loaf WITHIN the
    // household as ONE atomic group: no phantom seller-dip/buyer-peak, the total is conserved, and
    // since neither member ever reaches q alone, this is SplitAtGate (total 4 at the gate).
    let events = [ev(EventCause::SettledTradeSell, &[(a(1), 1), (a(2), 3)])];
    let outcome = classify(
        &[(a(1), 2), (a(2), 2)],
        &events,
        &[(a(1), 1), (a(2), 3)],
        false,
        4,
    );
    assert_eq!(outcome, BirthGateOutcome::SplitAtGate);
}

#[test]
fn classifier_window_with_a_buy_and_a_sell() {
    // A member buys up to q (SettledTrade buy raises free), then sells it (AskChange) before the
    // gate → MemberDrained, crossing = AskChange.
    let events = [
        ev(EventCause::SettledTradeSell, &[(a(1), 4)]), // the member is the BUYER leg here (free up)
        ev(EventCause::AskChange, &[(a(1), 0)]),
    ];
    let outcome = classify(&[(a(1), 0)], &events, &[(a(1), 0)], false, 4);
    assert_eq!(
        outcome,
        BirthGateOutcome::MemberDrainedBeforeGate {
            crossing: EventCause::AskChange,
            member: a(1),
        }
    );
}

#[test]
fn replay_reconciliation_requires_the_exact_member_map() {
    let opportunity = |window_start, gate_members| BirthGateRawOpportunity {
        tick: 0,
        household: 0,
        producer_type: ClosureClass::Miller,
        q: 4,
        window_start,
        events: vec![],
        gate: BirthGateState {
            members: gate_members,
            recorded_pass: false,
        },
    };

    assert!(birth_gate_replay_reconciles(&opportunity(
        vec![(a(1), 1), (a(2), 0)],
        vec![(a(2), 0), (a(1), 1)],
    )));
    assert!(!birth_gate_replay_reconciles(&opportunity(
        vec![(a(1), 1)],
        vec![(a(1), 1), (a(2), 0)],
    )));
    assert!(!birth_gate_replay_reconciles(&opportunity(
        vec![(a(1), 1), (a(2), 0)],
        vec![(a(1), 1)],
    )));
}

// ===========================================================================================
// The plumbing test: moving one opportunity's trace moves exactly the corresponding bucket
// ===========================================================================================

#[test]
fn plumbing_moving_a_trace_moves_exactly_one_bucket() {
    let base = BirthGateRawOpportunity {
        tick: 0,
        household: 0,
        producer_type: ClosureClass::Baker,
        q: 4,
        window_start: vec![(a(1), 1), (a(2), 1)],
        events: vec![],
        gate: BirthGateState {
            members: vec![(a(1), 1), (a(2), 1)],
            recorded_pass: false,
        },
    };
    let rows = [BirthGateReportRow {
        q: 4,
        arm: BurdenSavingArm::Off,
        seed: 3,
        opportunity: &base,
    }];
    let before = BirthGateReport::from_traces(&rows);
    // The base opportunity: total 2 < 4, never reached → NeverReachedQ.
    assert_eq!(before.global.never_reached_q, 1);
    assert_eq!(before.global.split_at_gate, 0);

    // Mutate the trace so the household total reaches q at the gate (SplitAtGate). Exactly one
    // bucket must move: never_reached_q 1→0, split_at_gate 0→1.
    let mutated = BirthGateRawOpportunity {
        window_start: vec![(a(1), 2), (a(2), 2)],
        gate: BirthGateState {
            members: vec![(a(1), 2), (a(2), 2)],
            recorded_pass: false,
        },
        ..base.clone()
    };
    let rows = [BirthGateReportRow {
        q: 4,
        arm: BurdenSavingArm::Off,
        seed: 3,
        opportunity: &mutated,
    }];
    let after = BirthGateReport::from_traces(&rows);
    assert_eq!(after.global.never_reached_q, 0, "never_reached_q must drop");
    assert_eq!(after.global.split_at_gate, 1, "split_at_gate must rise");
    assert_eq!(after.global.opportunities, before.global.opportunities);
    assert_eq!(after.global.failures, before.global.failures);
}

// ===========================================================================================
// Digest inertness — tag 35 ON-only, keyed on `configured`
// ===========================================================================================

/// The closed base with `birth_gate_obs` set to `on`.
fn closed_obs(on: bool) -> SettlementConfig {
    let mut cfg = SettlementConfig::frontier_closed_circulation();
    cfg.chain.as_mut().expect("chain").birth_gate_obs = on;
    cfg
}

#[test]
fn obs_digest_is_off_plus_the_single_tag35_emission() {
    let seed = BURDEN_SEEDS[0];
    for ticks in [0u64, 200] {
        let mut on = Settlement::generate(seed, &closed_obs(true));
        let mut off = Settlement::generate(seed, &closed_obs(false));
        for _ in 0..ticks {
            on.econ_tick();
            off.econ_tick();
        }
        let on_bytes = on.canonical_bytes();
        let off_bytes = off.canonical_bytes();
        assert_eq!(
            on_bytes.len(),
            off_bytes.len() + 2,
            "ON canonical bytes must be exactly two longer than OFF ({ticks} ticks)"
        );
        let split = on_bytes
            .iter()
            .zip(&off_bytes)
            .position(|(a, b)| a != b)
            .expect("ON and OFF must differ at the tag-35 emission");
        assert_eq!(
            &on_bytes[split..split + 2],
            &[35u8, 1u8],
            "the sole digest delta must be the [35, 1] tag-35 emission ({ticks} ticks)"
        );
        assert_eq!(
            &on_bytes[split + 2..],
            &off_bytes[split..],
            "removing the [35, 1] emission must yield the OFF bytes byte-for-byte ({ticks} ticks)"
        );
    }
}

/// The CONFIGURED-BUT-INACTIVE test: a marker-configured run on a NON-closed config
/// (`closure_active()` false) steps with identical behavior/reports, an EMPTY observer tape, and
/// canonical bytes = the flag-OFF bytes + `[35, 1]` (NOT literal equality — a configured run
/// always carries the tag).
#[test]
fn configured_but_inactive_is_off_plus_tag35_and_tape_empty() {
    let seed = BURDEN_SEEDS[0];
    // The C3R.d saving base (non-closed) carries demography — so `configured` holds — but
    // `closure_active()` is false, so `active` is false.
    let base = SettlementConfig::frontier_mortal_producers_saving();
    let mut configured = base.clone();
    configured.chain.as_mut().expect("chain").birth_gate_obs = true;

    let mut on = Settlement::generate(seed, &configured);
    let mut off = Settlement::generate(seed, &base);
    for _ in 0..200 {
        let report_on = on.econ_tick();
        let report_off = off.econ_tick();
        assert_eq!(
            report_on, report_off,
            "a configured-but-inactive run must step with identical EconTickReports"
        );
    }
    // The observer tape is EMPTY (no writes when inactive).
    assert!(
        on.birth_gate_opportunities().is_empty(),
        "configured-but-inactive must record no opportunities"
    );
    assert!(
        on.birth_gate_recount_snapshots().is_empty(),
        "configured-but-inactive must record no recount snapshots"
    );
    // Canonical bytes: ON = OFF + [35, 1] (NOT literal equality — the configured run carries the tag).
    let on_bytes = on.canonical_bytes();
    let off_bytes = off.canonical_bytes();
    assert_eq!(on_bytes.len(), off_bytes.len() + 2);
    let split = on_bytes
        .iter()
        .zip(&off_bytes)
        .position(|(a, b)| a != b)
        .expect("a configured run differs from OFF at the tag");
    assert_eq!(&on_bytes[split..split + 2], &[35u8, 1u8]);
    assert_eq!(&on_bytes[split + 2..], &off_bytes[split..]);
}

/// Flags-off byte-identity: the closed base without the obs flag is byte-identical to the landed
/// closed base (the obs adds no digest footprint when unconfigured).
#[test]
fn flags_off_closed_base_is_byte_identical() {
    let seed = BURDEN_SEEDS[0];
    let base = Settlement::generate(seed, &SettlementConfig::frontier_closed_circulation());
    let off = Settlement::generate(seed, &closed_obs(false));
    assert_eq!(
        base.canonical_bytes(),
        off.canonical_bytes(),
        "the flag-off closed base must be byte-identical to the landed base"
    );
}

/// A saving-arm (On) closed cell reproduces the golden's rung under observation — a spot check that
/// the motive arm is also pure-observation. (Full grid coverage is the oracle above.)
#[test]
fn saving_arm_cell_is_pure_observation() {
    let mut on = SettlementConfig::frontier_closed_circulation();
    {
        let demo = on.demography.as_mut().expect("demography");
        demo.child_food_endowment = 4;
    }
    {
        let chain = on.chain.as_mut().expect("chain");
        chain.birth_stock_saving = true;
        chain.birth_stock_saving_mode = BirthStockSavingMode::Motive;
    }
    let mut with_obs = on.clone();
    with_obs.chain.as_mut().expect("chain").birth_gate_obs = true;

    let mut a_run = Settlement::generate(BURDEN_SEEDS[0], &on);
    let mut b_run = Settlement::generate(BURDEN_SEEDS[0], &with_obs);
    for _ in 0..300 {
        assert_eq!(
            a_run.econ_tick(),
            b_run.econ_tick(),
            "the saving-arm cell must step identically with observation on"
        );
    }
}

// ===========================================================================================
// §6.6 LIVE-EMITTER (Settlement side) — the post-executor Production emission on a real run
// ===========================================================================================

/// The closed base with `birth_gate_obs` set and `child_food_endowment = q` (the grid's per-cell
/// sweep knob), for one real observed burden cell.
fn closed_obs_q(q: u32) -> SettlementConfig {
    let mut cfg = SettlementConfig::frontier_closed_circulation();
    cfg.demography
        .as_mut()
        .expect("the closed base carries demography")
        .child_food_endowment = q;
    cfg.chain.as_mut().expect("chain").birth_gate_obs = true;
    cfg
}

/// The Settlement-side `Production` emitter (mod.rs `run_production`) is the only staple-stock
/// event NOT owned by the Society tape, so the econ-layer live-emitter battery cannot reach it.
/// On a REAL closed-base run each successful `execute_direct_recipe_for_agent_checked` appends
/// exactly one single-member post-executor `Production` event into the joined tape, so captured
/// opportunities carry them.
///
/// The cell must be one where the producers actually mill/bake before extinction: at the canonical
/// q=4 the producer households collapse in the cold-start before ever executing a recipe (zero
/// `run_production` Mill/Bake executions across the whole run), so no Production event can fire —
/// that is the DH.b wall, not an emitter defect. A low-burden cell (q=1) keeps the producers alive
/// long enough to run the Mill/Bake chain AND reach the gate the same tick, so the emitter is
/// exercised on a real run. (Every Production *value* is corroborated per-event by the oracle's
/// replay reconciliation; this pins that the emitter fires with the one-member `push_production`
/// shape.)
#[test]
fn production_live_emitter_fires_on_a_real_closed_run() {
    let mut run = Settlement::generate(BURDEN_SEEDS[0], &closed_obs_q(1));
    // Step until a captured opportunity carries a Production event (the first producer household to
    // reach the gate already baked/milled that tick), bounded by the full DH.b run length.
    for _ in 0..1_600 {
        run.econ_tick();
        if run
            .birth_gate_opportunities()
            .iter()
            .any(|opp| opp.events.iter().any(|e| e.cause == EventCause::Production))
        {
            break;
        }
    }
    let mut production_events = 0usize;
    for opp in run.birth_gate_opportunities() {
        for event in &opp.events {
            if event.cause == EventCause::Production {
                production_events += 1;
                assert_eq!(
                    event.updates.len(),
                    1,
                    "each `push_production` emits exactly one single-member Production event: \
                     {event:?}"
                );
            }
        }
    }
    assert!(
        production_events > 0,
        "the Settlement-side Production emitter must fire on a real closed-base burden run"
    );
}
