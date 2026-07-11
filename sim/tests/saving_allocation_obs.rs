//! C3R.e-obs (impl-66) -- the allocation-contest instrumentation.
//!
//! A PURE-OBSERVATION slice: for every saving quote-opportunity that does not Fill it
//! assigns exactly one §2 outcome, aggregates into the five loss families, and prints the
//! per-seed diagnosis. NOTHING here changes behavior — the hard guards below pin that:
//! the digest differs from OFF by exactly the two-byte tag-32 emission, the obs flag is
//! byte-inert with the motive off, the family shares partition the unfilled opportunities,
//! and the obs-ON headline reproduces the landed C3R.d metrics AND verdict.
//!
//! The verdict is NOT a printed label: the oracle runs all four C3R.d cells through the SAME
//! real classifier (`common::classify`, shared with the C3R.d suite) and `assert_eq!`s the
//! computed verdict enum against the landed expectation (impl-66 repair §1). The opportunity
//! denominator is cross-checked by an INDEPENDENT eligible-opportunity recount off the
//! attribution snapshot, not only the family-sum tautology (repair §3).

mod common;

use common::{classify, trace, trace_with_config, Cell, Verdict};
use sim::{SavingAllocationObs, Settlement, SettlementConfig};

const SEEDS: [u64; 5] = [3, 7, 11, 19, 23];
const RUN_TICKS: u64 = 1_600;
// The same-seed C3R.c no-motive birth anchor (the identity/reference cell).
const EXPECTED_NO_MOTIVE_BIRTHS: [u64; 5] = [2, 3, 5, 2, 1];

/// The landed C3R.d headline facts pinned as the behavioral oracle: obs ON must reproduce
/// each `(producer_births, attributable_purchases, reached_four)` triple AND the verdict the
/// REAL classifier computes from the four-cell grid.
struct Pinned {
    seed: u64,
    verdict: Verdict,
    births: u64,
    attributable: u64,
    reached_four: usize,
}

const ORACLE: [Pinned; 5] = [
    Pinned {
        seed: 3,
        verdict: Verdict::BirthsResumeStructureStillDies,
        births: 3,
        attributable: 3,
        reached_four: 3,
    },
    Pinned {
        seed: 7,
        verdict: Verdict::BaseUnviable,
        births: 0,
        attributable: 5,
        reached_four: 0,
    },
    Pinned {
        seed: 11,
        verdict: Verdict::BirthStockRaceLost,
        births: 0,
        attributable: 7,
        reached_four: 0,
    },
    Pinned {
        seed: 19,
        verdict: Verdict::StockReachedBirthsStillBlocked,
        births: 1,
        attributable: 6,
        reached_four: 1,
    },
    Pinned {
        seed: 23,
        verdict: Verdict::StockReachedBirthsStillBlocked,
        births: 1,
        attributable: 3,
        reached_four: 1,
    },
];

/// The C3R.d headline cell with the allocation-contest observation ON.
fn headline_obs_on() -> SettlementConfig {
    let mut cfg = SettlementConfig::frontier_mortal_producers_saving();
    cfg.chain.as_mut().expect("chain").saving_allocation_obs = true;
    cfg
}

/// The identical headline cell with the observation OFF (the digest-delta baseline).
fn headline_obs_off() -> SettlementConfig {
    SettlementConfig::frontier_mortal_producers_saving()
}

/// The no-motive earned reference cell — the obs gate is inactive here (no motive), so
/// the instrumentation is inert. Kept only to anchor the C3R.c reference facts.
fn reference_cell() -> SettlementConfig {
    SettlementConfig::frontier_mortal_producers_earned()
}

/// Hard guard: the ENTIRE digest footprint of the observation is the ON-only two-byte
/// `[32, 1]` tag emission. ON is exactly two bytes longer than OFF, and removing that
/// emission yields the OFF stream byte-for-byte — proving pure observation (no wants, no
/// transfers, no behavior) both at generation and after stepping.
#[test]
fn obs_digest_is_off_plus_the_single_tag32_emission() {
    let seed = SEEDS[0];
    for ticks in [0u64, 200] {
        let mut on = Settlement::generate(seed, &headline_obs_on());
        let mut off = Settlement::generate(seed, &headline_obs_off());
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
            .expect("ON and OFF must differ at the tag-32 emission");
        assert_eq!(
            &on_bytes[split..split + 2],
            &[32u8, 1u8],
            "the sole digest delta must be the [32, 1] tag-32 emission ({ticks} ticks)"
        );
        assert_eq!(
            &on_bytes[split + 2..],
            &off_bytes[split..],
            "removing the [32, 1] emission must yield the OFF bytes byte-for-byte ({ticks} ticks)"
        );
    }
}

/// Hard guard: the obs flag is gated behind the motive (mode 1). With the motive OFF the
/// gate is inactive, so setting the flag emits no tag and is byte-identical to the base —
/// no prior base can shift under a flag-off (canonicalized ON-only).
#[test]
fn obs_flag_is_byte_inert_when_the_motive_is_off() {
    let seed = SEEDS[0];
    let base = Settlement::generate(seed, &reference_cell());
    let mut flagged = reference_cell();
    flagged.chain.as_mut().expect("chain").saving_allocation_obs = true;
    let flagged = Settlement::generate(seed, &flagged);
    assert_eq!(
        base.canonical_bytes(),
        flagged.canonical_bytes(),
        "the obs flag with the motive off must be byte-identical (the gate is inactive)"
    );
}

/// The pinned five-seed oracle + the totality invariant + the printed diagnosis. The oracle
/// runs the FOUR C3R.d cells (Headline obs-ON, SufficiencyControl, NoMotiveReference,
/// MintOnReference) through the SAME real classifier the C3R.d suite uses, and `assert_eq!`s
/// the COMPUTED verdict against the landed expectation — no display-only label. The obs-ON
/// headline reproduces the landed C3R.d `(births, attributable, reached_four)` metrics with
/// zero behavioral drift.
#[test]
fn obs_reproduces_the_landed_c3rd_headline_verdict_and_prints_the_diagnosis() {
    println!(
        "C3R.e-obs allocation-contest diagnosis (obs ON) — seeds={SEEDS:?}, ticks={RUN_TICKS}"
    );
    for (index, pinned) in ORACLE.iter().enumerate() {
        // The four C3R.d cells, exactly as the C3R.d suite forms them — except the Headline
        // runs with the allocation-obs flag ON (behaviorally inert, so the metrics/verdict
        // are unchanged). `trace_with_config` also hands back the driven settlement so we can
        // read its runtime-only obs report.
        let reference = trace(pinned.seed, Cell::NoMotiveReference);
        let mint_on = trace(pinned.seed, Cell::MintOnReference);
        let control = trace(pinned.seed, Cell::SufficiencyControl);
        let (headline, headline_settlement) =
            trace_with_config(pinned.seed, Cell::Headline, headline_obs_on());

        // ORACLE (repair §1): the verdict is COMPUTED by the real classifier and asserted,
        // never a printed constant. If obs ON perturbed any classification input the verdict
        // would drift and this fails.
        let verdict = classify(&headline, &control, &reference, &mint_on);
        assert_eq!(
            verdict, pinned.verdict,
            "seed {}: the real classifier's verdict drifted from the landed C3R.d cell",
            pinned.seed
        );

        // The exact landed metric pins are KEPT — they are the classifier's own inputs, so
        // pinning them and the verdict together closes the underdetermination the display-only
        // label left open.
        assert_eq!(
            headline.producer_births, pinned.births,
            "seed {}: obs ON changed producer_births",
            pinned.seed
        );
        assert_eq!(
            headline.attributable_purchases, pinned.attributable,
            "seed {}: obs ON changed attributable purchases",
            pinned.seed
        );
        assert_eq!(
            headline.reached_four, pinned.reached_four,
            "seed {}: obs ON changed reached_four",
            pinned.seed
        );
        assert_eq!(
            reference.producer_births, EXPECTED_NO_MOTIVE_BIRTHS[index],
            "seed {}: the no-motive reference anchor drifted",
            pinned.seed
        );

        let obs = headline_settlement.saving_allocation_obs_report();
        // `filled` counts distinct eligible buyer-tick opportunities, while attributable
        // purchases counts units. A carried unit may fill before the member's quote turn and
        // a newly posted unit may fill later in the same pass, so quantity can exceed the
        // opportunity count even though both use the same eligibility snapshot.
        assert!(
            obs.filled <= headline.attributable_purchases,
            "seed {}: distinct filled opportunities cannot exceed purchased quantity",
            pinned.seed
        );
        assert!(
            obs.filled + obs.unfilled() > 0,
            "seed {}: the allocation oracle must observe a non-empty opportunity domain",
            pinned.seed
        );

        // TOTALITY (a bookkeeping invariant, not a result): every unfilled opportunity got
        // exactly one outcome, so the five families partition `unfilled()` exactly, and the
        // CompetitiveLoss bases partition the competitive-loss total.
        let family_sum = obs.offer_scarcity()
            + obs.allocation_priority()
            + obs.microstructure_loss()
            + obs.gold_bind()
            + obs.residual();
        assert_eq!(
            family_sum,
            obs.unfilled(),
            "seed {}: the five families must partition the unfilled opportunities",
            pinned.seed
        );
        assert_eq!(
            obs.competitive_loss_total(),
            obs.allocation_priority() + obs.microstructure_loss() + obs.post_exit_loss(),
            "seed {}: the CompetitiveLoss bases must partition the competitive-loss total",
            pinned.seed
        );
        let phases = [
            obs.death_phase,
            obs.pre_market_phase,
            obs.market_phase,
            obs.production_own_use_phase,
            obs.birth_phase,
            obs.end_of_tick_phase,
        ];
        for phase in phases {
            assert_eq!(
                phase.physical_delta,
                phase.attributed_delta + phase.within_phase_ambiguous,
                "seed {}: each stock phase must reconcile before ambiguity",
                pinned.seed
            );
        }
        assert_eq!(
            phases.iter().map(|phase| phase.physical_delta).sum::<i64>(),
            obs.phys_net_delta,
            "seed {}: pinned phase seams must partition the measured physical delta",
            pinned.seed
        );
        assert_eq!(
            phases
                .iter()
                .map(|phase| phase.within_phase_ambiguous)
                .sum::<i64>(),
            obs.phys_within_phase_ambiguous,
            "seed {}: only reconciled phase residuals may be reported ambiguous",
            pinned.seed
        );

        // DIAGNOSIS: printed, NEVER asserted.
        print_diagnosis(pinned, verdict, obs);
    }
}

/// Repair §3: the INDEPENDENT eligible-opportunity count, driven per tick over one 1,600-tick
/// headline obs-ON run. After each tick, this reads the public accessor for the C3R.d
/// attribution snapshot captured at the actual pre-market seam. It adds that set's size only
/// when the observation report gained a supply row for the same tick, proving the tick opened
/// a money-priced spot pass (`PassStart`). Neither the snapshot accessor nor this accumulator
/// reads the observation outcome counters whose denominator it checks.
#[test]
fn independent_eligible_opportunity_count_via_attribution_snapshot() {
    let seed = ORACLE[0].seed;
    let mut s = Settlement::generate(seed, &headline_obs_on());
    let mut independent = 0u64;
    let mut spot_pass_ticks = 0u64;

    for _ in 0..RUN_TICKS {
        let tick = s.econ_tick_count();
        let supply_rows_before = s.saving_allocation_obs_report().supply_series.len();
        s.econ_tick();

        let obs = s.saving_allocation_obs_report();
        let opened_spot_pass = obs.supply_series.len() == supply_rows_before + 1
            && obs.supply_series.last().is_some_and(|row| row.tick == tick);
        if opened_spot_pass {
            independent += s.birth_stock_attribution_members().len() as u64;
            spot_pass_ticks += 1;
        }
    }

    let obs = s.saving_allocation_obs_report();
    let opportunities = obs.filled + obs.unfilled();
    assert!(independent > 0, "the accessor recount must be non-vacuous");
    assert_eq!(
        independent, opportunities,
        "seed {seed}: the independent pre-market eligible-opportunity recount must equal \
         filled + unfilled"
    );
    println!(
        "seed {seed:>2}: opportunities={opportunities} \
         independent(pre-market accessor)={independent} spot_pass_ticks={spot_pass_ticks}"
    );
}

fn print_diagnosis(pinned: &Pinned, verdict: Verdict, obs: &SavingAllocationObs) {
    let unfilled = obs.unfilled();
    let share = |count: u64| {
        if unfilled == 0 {
            0.0
        } else {
            count as f64 / unfilled as f64
        }
    };
    let offer = obs.offer_scarcity();
    let priority = obs.allocation_priority();
    let micro = obs.microstructure_loss();
    let gold = obs.gold_bind();
    let residual = obs.residual();
    // Exact >1/2 majority (ties at exactly 1/2 fall through to MixedDiagnosis).
    let diagnosis = if offer * 2 > unfilled {
        "OfferScarcityDominates"
    } else if priority * 2 > unfilled {
        "AllocationPriorityDominates"
    } else if micro * 2 > unfilled {
        "MicrostructureDominates"
    } else if gold * 2 > unfilled {
        "GoldBindDominates"
    } else {
        "MixedDiagnosis"
    };
    println!(
        "seed {:>2} [{:?}]: filled={} unfilled={} no_spot_pass_ticks={} drops={}",
        pinned.seed, verdict, obs.filled, unfilled, obs.no_spot_pass_ticks, obs.drops
    );
    println!(
        "    families: OfferScarcity={:.3} AllocationPriority={:.3} Microstructure={:.3} \
         GoldBind={:.3} Residual={:.3} => {}",
        share(offer),
        share(priority),
        share(micro),
        share(gold),
        share(residual),
        diagnosis
    );
    println!(
        "    atoms: no_bid={} self_ask={} no_ask={} priced_out={} comp_loss={} exec_residual={}",
        obs.no_bid_posted,
        obs.self_ask_only,
        obs.no_executable_ask_in_window,
        obs.all_asks_above_limit,
        obs.competitive_loss_total(),
        obs.execution_residual
    );
    println!(
        "    competitive-loss (basis × winner_intent): {:?}",
        obs.competitive_loss_matrix()
    );
    println!(
        "    phys staple: produced={} consumed={} net_delta={} within_phase_ambiguous={}",
        obs.phys_produced, obs.phys_consumed, obs.phys_net_delta, obs.phys_within_phase_ambiguous
    );
    println!(
        "    phases (physical,reserved,attributed,ambiguous): death={:?} pre_market={:?} \
         market={:?} production/own_use={:?} birth={:?} end={:?}",
        obs.death_phase,
        obs.pre_market_phase,
        obs.market_phase,
        obs.production_own_use_phase,
        obs.birth_phase,
        obs.end_of_tick_phase,
    );
    // §5 supply-side series: the offerable staple supply vs the asks actually posted, so a
    // reader can tell genuine offer scarcity (offerable ~ 0) from quote-generation failure
    // (offerable held, no ask posted) when reading the OfferScarcity family. Printed only.
    let supply = obs.supply_totals();
    println!(
        "    supply: spot_ticks={} offerable_sellers(member={},other={}) posted_asks={} \
         offerable_but_no_ask_ticks={}",
        supply.spot_ticks,
        supply.offerable_sellers_member,
        supply.offerable_sellers_other,
        supply.posted_asks,
        supply.offerable_but_no_ask_ticks,
    );
}
