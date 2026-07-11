//! C3R.e-obs (impl-66) -- the allocation-contest instrumentation.
//!
//! A PURE-OBSERVATION slice: for every saving quote-opportunity that does not Fill it
//! assigns exactly one §2 outcome, aggregates into the five loss families, and prints the
//! per-seed diagnosis. NOTHING here changes behavior — the hard guards below pin that:
//! the digest differs from OFF by exactly the two-byte tag-32 emission, the obs flag is
//! byte-inert with the motive off, the family shares partition the unfilled opportunities,
//! and the obs-ON headline reproduces the landed C3R.d metrics. Diagnosis is PRINTED,
//! never asserted (the lever selection is the NEXT milestone's, not this one's).

use sim::{SavingAllocationObs, Settlement, SettlementConfig};

const SEEDS: [u64; 5] = [3, 7, 11, 19, 23];
const RUN_TICKS: u64 = 1_600;
// The same-seed C3R.c no-motive birth anchor (the identity/reference cell).
const EXPECTED_NO_MOTIVE_BIRTHS: [u64; 5] = [2, 3, 5, 2, 1];

/// The landed C3R.d headline facts pinned as the behavioral oracle: obs ON must reproduce
/// each `(producer_births, attributable_purchases, reached_four)` triple and its verdict.
struct Pinned {
    seed: u64,
    verdict: &'static str,
    births: u64,
    attributable: u64,
    reached_four: usize,
}

const ORACLE: [Pinned; 5] = [
    Pinned {
        seed: 3,
        verdict: "BirthsResumeStructureStillDies",
        births: 3,
        attributable: 3,
        reached_four: 3,
    },
    Pinned {
        seed: 7,
        verdict: "BaseUnviable",
        births: 0,
        attributable: 5,
        reached_four: 0,
    },
    Pinned {
        seed: 11,
        verdict: "BirthStockRaceLost",
        births: 0,
        attributable: 7,
        reached_four: 0,
    },
    Pinned {
        seed: 19,
        verdict: "StockReachedBirthsStillBlocked",
        births: 1,
        attributable: 6,
        reached_four: 1,
    },
    Pinned {
        seed: 23,
        verdict: "StockReachedBirthsStillBlocked",
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

fn run(seed: u64, cfg: &SettlementConfig) -> Settlement {
    let mut s = Settlement::generate(seed, cfg);
    for _ in 0..RUN_TICKS {
        s.econ_tick();
    }
    s
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

/// The pinned five-seed oracle + the totality invariant + the printed diagnosis. The
/// oracle runs the two budgeted cells (headline obs-ON + the no-motive reference) over all
/// five seeds = 16,000 settlement ticks.
#[test]
fn obs_reproduces_the_landed_c3rd_headline_and_prints_the_diagnosis() {
    println!(
        "C3R.e-obs allocation-contest diagnosis (obs ON) — seeds={SEEDS:?}, ticks={RUN_TICKS}"
    );
    for (index, pinned) in ORACLE.iter().enumerate() {
        let headline = run(pinned.seed, &headline_obs_on());
        let reference = run(pinned.seed, &reference_cell());

        // INERTNESS: obs ON reproduces the exact landed C3R.d headline metrics, and the
        // reference cell reproduces the C3R.c no-motive birth anchor. Together these
        // reproduce the classification inputs (and thus the landed verdict) with zero
        // behavioral drift.
        assert_eq!(
            headline.producer_house_births(),
            pinned.births,
            "seed {}: obs ON changed producer_births",
            pinned.seed
        );
        assert_eq!(
            headline.birth_stock_attributable_purchases(),
            pinned.attributable,
            "seed {}: obs ON changed attributable purchases",
            pinned.seed
        );
        assert_eq!(
            headline.birth_stock_reached_four_count(),
            pinned.reached_four,
            "seed {}: obs ON changed reached_four",
            pinned.seed
        );
        assert_eq!(
            reference.producer_house_births(),
            EXPECTED_NO_MOTIVE_BIRTHS[index],
            "seed {}: the no-motive reference anchor drifted",
            pinned.seed
        );

        // TOTALITY (a bookkeeping invariant, not a result): every unfilled opportunity got
        // exactly one outcome, so the five families partition `unfilled()` exactly, and the
        // CompetitiveLoss bases partition the competitive-loss total.
        let obs = headline.saving_allocation_obs_report();
        // `filled` counts distinct eligible buyer-tick opportunities, while attributable
        // purchases counts units. A carried unit may fill before the member's quote turn and
        // a newly posted unit may fill later in the same pass, so quantity can exceed the
        // opportunity count even though both use the same eligibility snapshot.
        assert!(
            obs.filled <= headline.birth_stock_attributable_purchases(),
            "seed {}: distinct filled opportunities cannot exceed purchased quantity",
            pinned.seed
        );
        assert!(
            obs.filled + obs.unfilled() > 0,
            "seed {}: the allocation oracle must observe a non-empty opportunity domain",
            pinned.seed
        );
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
        print_diagnosis(pinned, obs);
    }
}

fn print_diagnosis(pinned: &Pinned, obs: &SavingAllocationObs) {
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
        "seed {:>2} [{}]: filled={} unfilled={} no_spot_pass_ticks={} drops={}",
        pinned.seed, pinned.verdict, obs.filled, unfilled, obs.no_spot_pass_ticks, obs.drops
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
