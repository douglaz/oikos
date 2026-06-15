//! G8c-2 acceptance suite — tender policies (the acceptance levers).
//!
//! G8c-1 gave the game the Austrian credit cycle. G8c-2 adds the **tender policies**
//! the lab built across M11–M17 — explicit rules for *which media must be accepted* on
//! each settlement surface (spot exchange, public debt, and **labor wages**) — as sim
//! policy levers, routed through econ's **unchanged** tender machinery
//! (`PublicSpotTender` / `LaborWageTender` / `PublicDebtTender` and their
//! `accepted_media()`, set by the `SetXTender` events). G8c-2 adds **no** tender logic
//! to econ; it only routes each settlement surface through its tender policy.
//!
//! The headline is **wage tender × the G8c-1 credit cycle** (the lab's M17 result, now
//! in the spatial cycle):
//!
//! - `wage-tender-cycle` (fiat wages legal tender): the fiat-credit employers can pay
//!   fiat wages → the fiat credit reaches workers → the boom→stop→bust transmits.
//! - `wage-refusal-cycle` (specie-only wages): the **same** fiat-credit issuance is
//!   **inert** — the employers cannot pay fiat wages, the credit never enters the real
//!   economy, and no boom and no bust form. The control is the proof the wage surface
//!   is the transmission valve.
//!
//! Tender gates **composition** (which medium settles a surface), never **totals** (no
//! money created or destroyed): a refused medium cannot settle its surface even if
//! held, the active medium does, and whole-system conservation holds under every
//! policy. The other surfaces (spot/debt) wire as the same lever on econ's
//! fiat-displacement benches (M11/M12). Magnitudes are SIGN only (cycle fires vs inert)
//! plus exact conservation; determinism is byte-identical per `(seed, config)`.

use econ::good::Gold;
use sim::{
    BankRepaymentTender, CycleConfig, CycleKind, IssuerRepaymentTender, LaborWageTender,
    PublicDebtTender, PublicSpotTender, Settlement, SettlementConfig, TenderPolicy,
    TraderEndowment,
};

const SEED: u64 = 0xC0FFEE;

/// The displacement benches' specie base (M6 emerged-gold bridge gold), fixed across
/// every tender policy — the conservation anchor.
const BENCH_SPECIE_BASE: u64 = 16;
/// The fiat the displacement issuer prints — held by the first receivers whether or
/// not the surface tender lets it settle (so the bench's broad money is specie + fiat).
const BENCH_FIAT_PRINTED: u64 = 8;

fn run(config: SettlementConfig, ticks: u64) -> Settlement {
    let mut s = Settlement::generate(SEED, &config);
    s.run(ticks);
    s
}

/// 1. Same `(seed, config)` → a byte-identical run. Integer state, econ's `Rng`
///    consumed only at generation, nothing drawn in the loop — the tender run is a
///    deterministic function of the run for the headline cycle config and for the spot
///    and debt benches alike (the canonical bytes carry the active tender timeline).
#[test]
fn tender_run_is_deterministic() {
    for config in [
        SettlementConfig::wage_tender_cycle(),
        SettlementConfig::wage_refusal_cycle(),
        SettlementConfig::spot_tender_bench(PublicSpotTender::SpecieOnly),
        SettlementConfig::spot_tender_bench(PublicSpotTender::FiatAndSpecie),
        SettlementConfig::debt_tender_bench(PublicDebtTender::SpecieOnly),
        SettlementConfig::debt_tender_bench(PublicDebtTender::FiatAndSpecie),
        SettlementConfig::bank_repayment_tender_bench(BankRepaymentTender::SpecieOnly),
        SettlementConfig::bank_repayment_tender_bench(BankRepaymentTender::BankClaimsAndSpecie),
        SettlementConfig::issuer_repayment_tender_bench(IssuerRepaymentTender::FiatRefused),
        SettlementConfig::issuer_repayment_tender_bench(IssuerRepaymentTender::FiatOnly),
    ] {
        let mut a = Settlement::generate(SEED, &config);
        let mut b = Settlement::generate(SEED, &config);
        a.run(80);
        b.run(80);
        assert_eq!(
            a.canonical_bytes(),
            b.canonical_bytes(),
            "a tender run must be byte-identical for the same seed + config"
        );
        assert_eq!(a.digest(), b.digest());
    }

    // The headline run actually exercised the cycle (a deterministic boom→bust, not an
    // inert run), and the wage policy is the legal-tender choice that transmits it.
    let headline = run(SettlementConfig::wage_tender_cycle(), 80);
    assert!(headline.is_cycle() && headline.is_m3());
    assert_eq!(headline.labor_wage_tender(), LaborWageTender::FiatAndSpecie);
    assert!(
        headline.cycle_fired(),
        "the deterministic headline run must actually cross the boom and the bust"
    );
}

/// 2. `wage-tender-cycle` (fiat wages legal tender): the G8c-1 credit cycle fires.
///    Cheap fiat credit opens a positive shadow gap, the boom over-invests in the
///    roundabout structure, credit stops, and the bust abandons the malinvestment and
///    consumes capital — and the fiat reaches workers as wages (the transmission).
///    MEASURED; sign only.
#[test]
fn fiat_wages_transmit_the_cycle() {
    let cycle = run(SettlementConfig::wage_tender_cycle(), 80);

    // The fiat-credit issuance reaches workers as fiat wages — the transmission valve
    // is open under legal-tender wages.
    assert_eq!(cycle.labor_wage_tender(), LaborWageTender::FiatAndSpecie);
    assert!(
        cycle.wage_fiat_settled() > Gold::ZERO,
        "fiat wages must settle when wages are legal tender"
    );

    // The cycle fires: gap > 0, boom, structure rose above the shadow baseline, bust,
    // capital consumed — the full G8c-1 shape.
    assert!(
        cycle.max_shadow_gap_bps() > 0,
        "fiat credit must open a positive shadow gap"
    );
    assert!(
        cycle.structure_rose_above_shadow(),
        "the boom must lengthen the roundabout structure above the shadow baseline"
    );
    assert!(
        cycle.boom_projects_started() > 0,
        "the boom must start roundabout projects"
    );
    assert!(
        cycle.bust_abandoned_projects() > 0,
        "the bust must abandon the malinvested projects"
    );
    assert!(
        cycle.capital_consumed() > 0,
        "the bust must consume real capital"
    );
    assert!(cycle.cycle_fired(), "the cycle fired");
}

/// 3. `wage-refusal-cycle` (specie-only wages): the **same** fiat-credit issuance is
///    INERT — no boom, no bust, no capital consumed. The fiat-credit employers cannot
///    pay fiat wages (the wage surface refuses fiat), so the credit never enters the
///    real economy. Paired with test 2, this shows the wage surface is the
///    transmission valve: the *only* difference is the wage tender. (Sign only.)
#[test]
fn specie_only_wages_render_credit_inert() {
    let refusal = run(SettlementConfig::wage_refusal_cycle(), 80);

    // The wage surface refuses fiat: no fiat wage ever settles, even though the
    // fiat-credit employers hold the fiat the issuer extended to them.
    assert_eq!(refusal.labor_wage_tender(), LaborWageTender::SpecieOnly);
    assert_eq!(
        refusal.wage_fiat_settled(),
        Gold::ZERO,
        "specie-only wages must refuse every fiat wage"
    );
    assert_eq!(
        refusal.wage_specie_settled(),
        Gold::ZERO,
        "with no specie wage offered either, no wage settles at all (the lab's M17)"
    );

    // The SAME issuance happened — the credit was extended, it is just inert. This is
    // what makes the control a falsification: the issuance is identical, only the wage
    // gate differs.
    assert!(
        refusal.credit_ever_circulated(),
        "the fiat credit must still be extended (the same issuance) — only inert"
    );

    // No cycle: no boom, no bust, no capital consumed — the credit never transmits.
    assert!(
        !refusal.structure_rose_above_shadow(),
        "the refusal control's structure never rises above the shadow baseline"
    );
    assert_eq!(
        refusal.boom_projects_started(),
        0,
        "specie-only wages start no boom projects"
    );
    assert_eq!(
        refusal.bust_abandoned_projects(),
        0,
        "specie-only wages abandon nothing (no boom to bust)"
    );
    assert_eq!(
        refusal.capital_consumed(),
        0,
        "specie-only wages consume no capital"
    );
    assert!(
        !refusal.cycle_fired(),
        "the cycle is inert under specie-only wages"
    );

    // The twin fired from the same economy — the wage tender is the only difference.
    let transmit = run(SettlementConfig::wage_tender_cycle(), 80);
    assert!(
        transmit.cycle_fired() && !refusal.cycle_fired(),
        "the cycle fires under fiat wages and is inert under specie-only wages — the \
         wage surface is the transmission valve"
    );
}

/// 4. Tender gates **composition**, never **totals**. Across the spot and debt
///    surfaces, a refused medium does NOT settle that surface (even though it is
///    *held*), the active medium does, and the settlement's total broad money is
///    unchanged by the policy — only which medium settled flips.
#[test]
fn tender_gates_media_not_totals() {
    // --- the spot surface (M11) ---
    let spot_refused = run(
        SettlementConfig::spot_tender_bench(PublicSpotTender::SpecieOnly),
        12,
    );
    let spot_legal = run(
        SettlementConfig::spot_tender_bench(PublicSpotTender::FiatAndSpecie),
        12,
    );

    // The refused medium (fiat) is HELD — the issuer printed it to the first receivers
    // — yet none of it settles the spot surface; specie settles instead.
    let refused_composition = spot_refused
        .money_composition()
        .expect("a finance settlement has an M3 composition");
    assert_eq!(
        refused_composition.public_fiat.0, BENCH_FIAT_PRINTED,
        "the refused fiat is still held (printed, not destroyed)"
    );
    assert_eq!(
        spot_refused.spot_fiat_settled(),
        Gold::ZERO,
        "specie-only spot tender refuses the held fiat at the spot surface"
    );
    assert!(
        spot_refused.spot_specie_settled() > Gold::ZERO,
        "the active medium (specie) settles the spot surface"
    );
    // The active medium settles in fiat when fiat is legal tender.
    assert!(
        spot_legal.spot_fiat_settled() > Gold::ZERO,
        "fiat-and-specie spot tender lets the held fiat settle the spot surface"
    );
    // Totals unchanged: only the composition (which medium settled) flipped.
    assert_eq!(
        spot_refused.total_broad_money(),
        spot_legal.total_broad_money(),
        "the spot tender changes composition, never the total broad money"
    );
    assert_eq!(
        spot_legal.total_broad_money().0,
        BENCH_SPECIE_BASE + BENCH_FIAT_PRINTED,
        "broad money is exactly the specie base + the printed fiat under either policy"
    );

    // --- the debt surface (M12) ---
    let debt_refused = run(
        SettlementConfig::debt_tender_bench(PublicDebtTender::SpecieOnly),
        12,
    );
    let debt_legal = run(
        SettlementConfig::debt_tender_bench(PublicDebtTender::FiatAndSpecie),
        12,
    );
    assert_eq!(
        debt_refused.debt_fiat_settled(),
        Gold::ZERO,
        "specie-only debt tender refuses fiat for the debt surface"
    );
    assert!(
        debt_refused.debt_specie_settled() > Gold::ZERO,
        "the active medium (specie) discharges the debt"
    );
    assert!(
        debt_legal.debt_fiat_settled() > Gold::ZERO,
        "fiat-and-specie debt tender lets fiat discharge the debt"
    );
    assert_eq!(
        debt_refused.total_broad_money(),
        debt_legal.total_broad_money(),
        "the debt tender changes composition, never the total broad money"
    );
}

/// 5. `PublicSpotTender` / `PublicDebtTender` and the bank/issuer-repayment tenders
///    each enforce their refusal-vs-acceptance on their surface — the lab's M11-M16
///    results, now reachable as sim config levers routed through econ's unchanged
///    tender machinery. The active policy reads back from the live society (the viewer
///    surfaces it).
#[test]
fn spot_and_debt_tenders_gate_their_surfaces() {
    // The spot surface (M11): refusal settles in specie, legal tender settles in fiat —
    // the same trades, a different settlement medium.
    let spot_refused = run(
        SettlementConfig::spot_tender_bench(PublicSpotTender::SpecieOnly),
        12,
    );
    let spot_legal = run(
        SettlementConfig::spot_tender_bench(PublicSpotTender::FiatAndSpecie),
        12,
    );
    assert_eq!(
        spot_refused.public_spot_tender(),
        PublicSpotTender::SpecieOnly
    );
    assert_eq!(
        spot_legal.public_spot_tender(),
        PublicSpotTender::FiatAndSpecie
    );
    assert!(spot_refused.is_tender_bench() && spot_legal.is_tender_bench());
    // Refusal: fiat refused (zero), specie settles.
    assert_eq!(spot_refused.spot_fiat_settled(), Gold::ZERO);
    assert!(spot_refused.spot_specie_settled() > Gold::ZERO);
    // Acceptance: fiat settles, specie does not (the buyers spend the cheaper fiat).
    assert!(spot_legal.spot_fiat_settled() > Gold::ZERO);
    assert_eq!(spot_legal.spot_specie_settled(), Gold::ZERO);

    // The debt surface (M12): the same refusal-vs-acceptance, on debt discharge.
    let debt_refused = run(
        SettlementConfig::debt_tender_bench(PublicDebtTender::SpecieOnly),
        12,
    );
    let debt_legal = run(
        SettlementConfig::debt_tender_bench(PublicDebtTender::FiatAndSpecie),
        12,
    );
    assert_eq!(
        debt_refused.public_debt_tender(),
        PublicDebtTender::SpecieOnly
    );
    assert_eq!(
        debt_legal.public_debt_tender(),
        PublicDebtTender::FiatAndSpecie
    );
    assert_eq!(debt_refused.debt_fiat_settled(), Gold::ZERO);
    assert!(debt_refused.debt_specie_settled() > Gold::ZERO);
    assert!(debt_legal.debt_fiat_settled() > Gold::ZERO);
    assert_eq!(debt_legal.debt_specie_settled(), Gold::ZERO);

    // Bank-loan repayment (M15): the borrower holds a bank claim. SpecieOnly refuses
    // it; BankClaimsAndSpecie accepts it and retires the bank credit.
    let bank_refused = run(
        SettlementConfig::bank_repayment_tender_bench(BankRepaymentTender::SpecieOnly),
        5,
    );
    let bank_legal = run(
        SettlementConfig::bank_repayment_tender_bench(BankRepaymentTender::BankClaimsAndSpecie),
        5,
    );
    assert_eq!(
        bank_refused.bank_repayment_tender(),
        BankRepaymentTender::SpecieOnly
    );
    assert_eq!(
        bank_legal.bank_repayment_tender(),
        BankRepaymentTender::BankClaimsAndSpecie
    );
    assert_eq!(bank_refused.bank_repayment_claims_settled(), Gold::ZERO);
    assert_eq!(bank_refused.bank_repayment_credit_retired(), Gold::ZERO);
    assert!(
        bank_legal.bank_repayment_claims_settled() > Gold::ZERO,
        "bank-claim tender accepts the held claim for bank repayment"
    );
    assert!(
        bank_legal.bank_repayment_credit_retired() > Gold::ZERO,
        "accepted bank-claim repayment retires bank credit through econ"
    );
    assert_eq!(bank_legal.bank_repayment_fiat_settled(), Gold::ZERO);
    assert_eq!(bank_legal.bank_repayment_specie_settled(), Gold::ZERO);

    // Issuer-credit repayment (M16): the borrower holds fiat. FiatRefused refuses it;
    // FiatOnly accepts it and retires issuer credit.
    let issuer_refused = run(
        SettlementConfig::issuer_repayment_tender_bench(IssuerRepaymentTender::FiatRefused),
        14,
    );
    let issuer_legal = run(
        SettlementConfig::issuer_repayment_tender_bench(IssuerRepaymentTender::FiatOnly),
        14,
    );
    assert_eq!(
        issuer_refused.issuer_repayment_tender(),
        IssuerRepaymentTender::FiatRefused
    );
    assert_eq!(
        issuer_legal.issuer_repayment_tender(),
        IssuerRepaymentTender::FiatOnly
    );
    assert_eq!(issuer_refused.issuer_repayment_fiat_settled(), Gold::ZERO);
    assert_eq!(issuer_refused.issuer_repayment_credit_retired(), Gold::ZERO);
    assert!(
        issuer_legal.issuer_repayment_fiat_settled() > Gold::ZERO,
        "fiat-only issuer repayment accepts returned fiat"
    );
    assert!(
        issuer_legal.issuer_repayment_credit_retired() > Gold::ZERO,
        "accepted issuer repayment retires issuer credit through econ"
    );
    assert_eq!(issuer_legal.issuer_repayment_specie_settled(), Gold::ZERO);
}

/// 6. Whole-system conservation holds under **every** tender policy — the policy
///    changes composition only, never totals. The M3 ledger reconciles every tick on
///    every config; the displacement benches hold the specie base and the broad money
///    fixed whichever medium settles; and the cycle conserves the specie base with the
///    fiat base an exact `issued − retired` identity (a default retires/books, never
///    leaks).
#[test]
fn tender_conserves() {
    // The benches: the specie base and the broad money are fixed across the policy —
    // tender flips which medium settles, never the total. The ledger reconciles every
    // tick.
    for config in [
        SettlementConfig::spot_tender_bench(PublicSpotTender::SpecieOnly),
        SettlementConfig::spot_tender_bench(PublicSpotTender::FiatAndSpecie),
        SettlementConfig::debt_tender_bench(PublicDebtTender::SpecieOnly),
        SettlementConfig::debt_tender_bench(PublicDebtTender::FiatAndSpecie),
    ] {
        let mut s = Settlement::generate(SEED, &config);
        for t in 0..12 {
            s.econ_tick();
            assert!(
                s.society().money_ledgers_reconcile(),
                "the M3 ledger must reconcile every tick on a bench at tick {t}"
            );
            let composition = s
                .money_composition()
                .expect("a finance settlement has an M3 composition");
            assert_eq!(
                composition.public_specie.0 + composition.bank_reserves.0,
                BENCH_SPECIE_BASE,
                "the bench specie base is fixed across the tender policy at tick {t}"
            );
            assert_eq!(
                s.total_broad_money().0,
                BENCH_SPECIE_BASE + BENCH_FIAT_PRINTED,
                "the bench broad money is fixed across the tender policy at tick {t}"
            );
        }
    }

    // The repayment benches also reconcile every tick. Their accepted side retires
    // credit (as econ's repayment machinery specifies), but the tender policy itself
    // still only gates the repayment medium.
    for (config, ticks) in [
        (
            SettlementConfig::bank_repayment_tender_bench(BankRepaymentTender::SpecieOnly),
            5,
        ),
        (
            SettlementConfig::bank_repayment_tender_bench(BankRepaymentTender::BankClaimsAndSpecie),
            5,
        ),
        (
            SettlementConfig::issuer_repayment_tender_bench(IssuerRepaymentTender::FiatRefused),
            14,
        ),
        (
            SettlementConfig::issuer_repayment_tender_bench(IssuerRepaymentTender::FiatOnly),
            14,
        ),
    ] {
        let mut s = Settlement::generate(SEED, &config);
        for t in 0..ticks {
            s.econ_tick();
            assert!(
                s.society().money_ledgers_reconcile(),
                "the M3 ledger must reconcile every tick on a repayment bench at tick {t}"
            );
        }
    }

    // The cycle (both wage policies): the specie base is fixed (fiat is credit, not
    // minted specie), the fiat base is exactly issued − retired, and the ledger
    // reconciles every tick — whether the cycle transmits or is inert.
    for config in [
        SettlementConfig::wage_tender_cycle(),
        SettlementConfig::wage_refusal_cycle(),
    ] {
        let mut s = Settlement::generate(SEED, &config);
        let initial = s
            .money_composition()
            .expect("a finance settlement has an M3 composition");
        let specie_base = initial.public_specie.0 + initial.bank_reserves.0;
        for t in 0..80 {
            s.econ_tick();
            assert!(
                s.society().money_ledgers_reconcile(),
                "the M3 ledger must reconcile every tick on the cycle at tick {t}"
            );
            let composition = s
                .money_composition()
                .expect("a finance settlement has an M3 composition");
            assert_eq!(
                composition.public_specie.0 + composition.bank_reserves.0,
                specie_base,
                "the cycle specie base is fixed across the tender policy at tick {t}"
            );
            assert_eq!(
                s.fiat_base(),
                composition.public_fiat,
                "the fiat base (issued − retired) equals the outstanding fiat at tick {t}"
            );
        }
        assert_eq!(
            s.fiat_base(),
            s.fiat_issued().saturating_sub(s.fiat_retired()),
            "the fiat base is exactly issued − retired"
        );
    }
}

/// 7. `econ_unchanged` — the tender additions are inert for every non-finance
///    settlement (a spatial run is byte-identical to a twin and surfaces no tender
///    bench), and the default-tender cycle is byte-identical to its G8c-1 form. The six
///    econ goldens staying byte-identical and the full workspace suite + `cargo clippy
///    --workspace --all-targets -- -D warnings` + `cargo fmt --check` are the real gate;
///    this checks the local seam.
#[test]
fn econ_unchanged() {
    // A spatial settlement runs no tender bench and no cycle — the finance path is
    // skipped, and the active tenders read back as the econ defaults.
    let plain = Settlement::generate(7, &SettlementConfig::m3_settlement());
    assert!(!plain.is_tender_bench() && !plain.is_cycle());
    assert_eq!(plain.public_spot_tender(), PublicSpotTender::ParAll);
    assert_eq!(plain.labor_wage_tender(), LaborWageTender::ParAll);

    // A non-finance M3 settlement is byte-identical to a twin — the G8c-2 additions are
    // present but unexercised.
    let mut a = Settlement::generate(42, &SettlementConfig::m3_settlement());
    let mut b = Settlement::generate(42, &SettlementConfig::m3_settlement());
    a.run(40);
    b.run(40);
    assert_eq!(a.canonical_bytes(), b.canonical_bytes());

    // The default-tender credit cycle emits no tender events, so it is byte-identical
    // to the G8c-1 cycle: the wage-tender cycle (an explicit legal-tender choice) and
    // the plain cycle both fire, but the plain cycle's bytes carry no SetXTender event.
    let plain_cycle = Settlement::generate(SEED, &SettlementConfig::credit_cycle());
    assert_eq!(plain_cycle.labor_wage_tender(), LaborWageTender::ParAll);

    // A tender run keeps econ's invariants tick over tick: the M3 ledger reconciles.
    let mut bench = Settlement::generate(
        3,
        &SettlementConfig::spot_tender_bench(PublicSpotTender::SpecieOnly),
    );
    for _ in 0..12 {
        bench.econ_tick();
        assert!(
            bench.society().money_ledgers_reconcile(),
            "the M3 ledger reconciles every tick across a tender bench"
        );
    }
}

// ---- unit tests -------------------------------------------------------------

/// A tender bench requires the M3 ledger (it settles on the M3 money system).
#[test]
#[should_panic(expected = "requires the M3 ledger")]
fn bench_requires_m3_ledger() {
    let mut config = SettlementConfig::spot_tender_bench(PublicSpotTender::SpecieOnly);
    config.m3 = false;
    let _ = Settlement::generate(1, &config);
}

/// A tender bench is a finance settlement with no spatial colony.
#[test]
#[should_panic(expected = "no spatial colony")]
fn bench_rejects_spatial_colony() {
    let mut config = SettlementConfig::spot_tender_bench(PublicSpotTender::SpecieOnly);
    config.gatherers = 4;
    let _ = Settlement::generate(1, &config);
}

/// The cycle and the tender bench are mutually exclusive finance overlays.
#[test]
#[should_panic(expected = "either the credit cycle or a tender bench")]
fn cycle_and_bench_are_mutually_exclusive() {
    let mut config = SettlementConfig::spot_tender_bench(PublicSpotTender::SpecieOnly);
    config.cycle = SettlementConfig::credit_cycle().cycle;
    let _ = Settlement::generate(1, &config);
}

/// A tender bench rejects resident traders (its agents come from econ's scenario cast).
#[test]
#[should_panic(expected = "resident_traders")]
fn bench_rejects_resident_traders_overlay() {
    let config = SettlementConfig::debt_tender_bench(PublicDebtTender::FiatAndSpecie)
        .with_resident_traders(vec![TraderEndowment {
            gold: 1,
            stock: Vec::new(),
        }]);
    let _ = Settlement::generate(1, &config);
}

/// The wage-tender cycle and the wage-refusal cycle are a falsification twin: the same
/// credit cycle, opposite wage tenders, opposite outcomes (fires vs inert). They differ
/// only in the wage tender, so their canonical bytes diverge from generation.
#[test]
fn wage_cycle_and_control_are_a_twin() {
    let transmit = Settlement::generate(SEED, &SettlementConfig::wage_tender_cycle());
    let refusal = Settlement::generate(SEED, &SettlementConfig::wage_refusal_cycle());
    assert!(transmit.is_cycle() && refusal.is_cycle());
    // The wage tender is a Tick(0) scenario event, so the live policy is still the
    // econ default until the first tick fires it (like the regime reading SoundGold at
    // generation) — but the retained policy timeline is already canonical state, so the
    // twin's bytes diverge from generation.
    assert_eq!(transmit.labor_wage_tender(), LaborWageTender::ParAll);
    assert_ne!(
        transmit.canonical_bytes(),
        refusal.canonical_bytes(),
        "the wage-tender twins must not collide before the first tick"
    );

    // After running, the live policies and the outcomes are the falsification twin:
    // the same credit cycle, opposite wage tenders, opposite outcomes.
    let mut transmit = transmit;
    let mut refusal = refusal;
    transmit.run(80);
    refusal.run(80);
    assert_eq!(transmit.labor_wage_tender(), LaborWageTender::FiatAndSpecie);
    assert_eq!(refusal.labor_wage_tender(), LaborWageTender::SpecieOnly);
    assert!(
        transmit.cycle_fired() && !refusal.cycle_fired(),
        "the only difference is the wage tender, and it decides whether the cycle fires"
    );
}

/// The default-tender credit cycle and the explicit wage-tender cycle BOTH fire (fiat
/// wages transmit under `ParAll` and `FiatAndSpecie` alike) — the legal-tender choice
/// is explicit policy, not a behavior change.
#[test]
fn default_and_legal_tender_wages_both_transmit() {
    let default = run(SettlementConfig::credit_cycle(), 80);
    let legal = run(SettlementConfig::wage_tender_cycle(), 80);
    assert_eq!(default.labor_wage_tender(), LaborWageTender::ParAll);
    assert_eq!(legal.labor_wage_tender(), LaborWageTender::FiatAndSpecie);
    assert!(
        default.cycle_fired() && legal.cycle_fired(),
        "both ParAll and FiatAndSpecie wages accept fiat, so both transmit the cycle"
    );
}

/// The spot bench's surface and the debt bench's surface are distinct (the canonical
/// bytes carry the surface, so benches for different surfaces never collide).
#[test]
fn bench_surface_is_canonical() {
    let spot = Settlement::generate(
        SEED,
        &SettlementConfig::spot_tender_bench(PublicSpotTender::SpecieOnly),
    );
    let debt = Settlement::generate(
        SEED,
        &SettlementConfig::debt_tender_bench(PublicDebtTender::SpecieOnly),
    );
    let bank = Settlement::generate(
        SEED,
        &SettlementConfig::bank_repayment_tender_bench(BankRepaymentTender::SpecieOnly),
    );
    let issuer = Settlement::generate(
        SEED,
        &SettlementConfig::issuer_repayment_tender_bench(IssuerRepaymentTender::FiatRefused),
    );
    assert_eq!(spot.bench_surface(), Some(sim::BenchSurface::Spot));
    assert_eq!(debt.bench_surface(), Some(sim::BenchSurface::Debt));
    assert_eq!(bank.bench_surface(), Some(sim::BenchSurface::BankRepayment));
    assert_eq!(
        issuer.bench_surface(),
        Some(sim::BenchSurface::IssuerRepayment)
    );
    let bytes = [
        spot.canonical_bytes(),
        debt.canonical_bytes(),
        bank.canonical_bytes(),
        issuer.canonical_bytes(),
    ];
    for (left, left_bytes) in bytes.iter().enumerate() {
        for (right, right_bytes) in bytes.iter().enumerate().skip(left + 1) {
            assert_ne!(
                left_bytes, right_bytes,
                "bench surfaces {left} and {right} must not collide"
            );
        }
    }
}

/// The bank/issuer-repayment tenders wire as the **same** config lever as the spot,
/// wage, and debt surfaces: a `TenderPolicy` knob that routes through econ's unchanged
/// `SetXTender` mechanism. The repayment benches above prove their refusal-vs-acceptance
/// behavior; this cycle assertion proves the knobs also route on the cycle policy set.
#[test]
fn repayment_tenders_route_as_the_same_lever() {
    let mut config = SettlementConfig::credit_cycle();
    config.cycle = Some(CycleConfig {
        kind: CycleKind::CreditCycle,
        tender: TenderPolicy {
            bank_repayment: BankRepaymentTender::SpecieOnly,
            issuer_repayment: IssuerRepaymentTender::FiatRefused,
            ..TenderPolicy::default()
        },
    });

    // At generation the live policy is still the econ default (the Tick(0) events have
    // not fired yet) — exactly like the wage tender and the regime.
    let mut s = Settlement::generate(SEED, &config);
    assert_eq!(s.bank_repayment_tender(), BankRepaymentTender::ParAll);
    assert_eq!(s.issuer_repayment_tender(), IssuerRepaymentTender::FiatOnly);

    // After the first tick the events fire and the active policy reads back through the
    // live society — the lever routed through econ's unchanged tender machinery.
    for t in 0..20 {
        s.econ_tick();
        assert!(
            s.society().money_ledgers_reconcile(),
            "the repayment-tender lever must not break ledger reconciliation at tick {t}"
        );
    }
    assert_eq!(s.bank_repayment_tender(), BankRepaymentTender::SpecieOnly);
    assert_eq!(
        s.issuer_repayment_tender(),
        IssuerRepaymentTender::FiatRefused
    );

    // The non-default repayment knobs are part of the canonical policy timeline, so the
    // bytes diverge from the default-tender cycle even at generation.
    let plain = Settlement::generate(SEED, &SettlementConfig::credit_cycle());
    assert_ne!(
        Settlement::generate(SEED, &config).canonical_bytes(),
        plain.canonical_bytes(),
        "the repayment-tender lever is canonical policy state"
    );
}
