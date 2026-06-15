//! G8c-3 acceptance suite — tax receivability (the state's counter-lever).
//!
//! G8c-2 gave the player the *private* acceptance levers (tender policies): when the
//! labor market refuses fiat wages, fiat credit is inert — the boom never transmits.
//! G8c-3 adds the *state's* counter-lever — **tax receivability** (the lab's M21,
//! chartalist) — as a sim policy on that same settlement, routed through econ's
//! **unchanged** tax machinery (`apply_levy_tax`, `settle_due_debts_m3` gated by
//! [`TaxReceivability`], the `SetTaxReceivability` / `LevyTax` events, and the issuer tax
//! accounts). G8c-3 adds **no** tax logic to econ; it only routes the levy/receivability
//! in.
//!
//! The headline ties straight back to G8c-2: in a settlement whose **wages are
//! specie-only** (fiat credit inert, no private fiat demand), a **fiat-receivable** tax
//! compels fiat demand through the **fiscal** channel (`tax_receipts_fiat > 0`) — fiat
//! circulates via tax where the labor market refused it. The **control** is the same
//! settlement with a **specie-receivable** tax (`tax_receipts_specie > 0`,
//! `tax_receipts_fiat == 0`): no compelled fiat demand. The two configs differ in
//! **exactly** the receivability, so the compelled fiat demand is isolated to that gate,
//! not the levy or the spatial dynamics.
//!
//! Tax is **fiscal, not credit**: receipts move into the issuer's tax accounts and never
//! touch `credit_retired` / `fiat_credit_outstanding`. The levy is a zero-principal
//! `DebtContract` owed to the single state issuer. Conservation is exact: a levy is
//! either received (into the issuer) or **defaulted** (unmet by rule) — never created or
//! destroyed. Magnitudes are SIGN only; determinism is byte-identical per `(seed,
//! config)`.

use econ::agent::AgentId;
use econ::good::Gold;
use econ::ledger::MoneySystem;
use econ::purpose::{CreditLender, CreditSource, DebtPurpose};
use econ::timemarket::DebtState;
use sim::{LaborWageTender, Settlement, SettlementConfig, TaxReceivability};

const SEED: u64 = 0xC0FFEE;

/// The tick the counter-lever levy comes due (mirrors `settlement::TAX_DUE_TICK`): inside
/// the cycle's fiat-outstanding window and before any loan repayment retires credit.
const TAX_DUE_TICK: usize = 8;

/// The fiat-holding capitalist the counter-lever levies (mirrors
/// `settlement::TAX_FIAT_HOLDER`) — under specie-only wages it holds the issuer's fiat
/// idle.
const FIAT_HOLDER: AgentId = AgentId(200);
/// The specie-holding trader the same levy targets (mirrors
/// `settlement::TAX_SPECIE_HOLDER`).
const SPECIE_HOLDER: AgentId = AgentId(100);

fn run(config: SettlementConfig, ticks: u64) -> Settlement {
    let mut s = Settlement::generate(SEED, &config);
    s.run(ticks);
    s
}

fn money(s: &Settlement) -> &MoneySystem {
    s.society()
        .money_system
        .as_ref()
        .expect("a finance settlement has an M3 money system")
}

fn specie_held(s: &Settlement, agent: AgentId) -> Gold {
    money(s)
        .balance_snapshot(agent)
        .map(|balance| balance.public_specie)
        .unwrap_or(Gold::ZERO)
}

fn fiat_held(s: &Settlement, agent: AgentId) -> Gold {
    money(s).public_fiat(agent)
}

/// The settlement state of the tax debt owed by `agent` (each counter-lever target has
/// exactly one).
fn tax_debt_state(s: &Settlement, agent: AgentId) -> DebtState {
    s.society()
        .debts
        .iter()
        .find(|debt| debt.purpose == DebtPurpose::TaxLiability && debt.borrower == agent)
        .map(|debt| debt.state)
        .expect("a tax liability for the agent")
}

/// 1. Same `(seed, config)` → a byte-identical run through the levy and the settlement.
///    Integer state, econ's `Rng` consumed only at generation, nothing drawn in the loop
///    — the tax run is a deterministic function of the run for both the fiat headline and
///    the specie control (the canonical bytes carry the receivability + the issuer tax
///    accounts). The headline run actually levied and settled (not an inert no-op).
#[test]
fn tax_run_is_deterministic() {
    for config in [
        SettlementConfig::tax_in_fiat(),
        SettlementConfig::tax_in_specie(),
    ] {
        let mut a = Settlement::generate(SEED, &config);
        let mut b = Settlement::generate(SEED, &config);
        a.run(80);
        b.run(80);
        assert_eq!(
            a.canonical_bytes(),
            b.canonical_bytes(),
            "a tax run must be byte-identical for the same seed + config"
        );
        assert_eq!(a.digest(), b.digest());
    }

    // The headline run is a real levy + settlement: the tax is fiat-receivable, the levy
    // seeded, and it actually settled in fiat (the deterministic bytes carry it).
    let headline = run(SettlementConfig::tax_in_fiat(), 80);
    assert!(headline.is_tax() && headline.is_cycle());
    assert_eq!(headline.tax_receivability(), TaxReceivability::FiatOnly);
    assert!(
        headline.taxes_levied() > Gold::ZERO,
        "the state levy must seed the liability"
    );
    assert!(
        headline.tax_receipts_fiat() > Gold::ZERO,
        "the deterministic headline run must actually settle the tax in fiat"
    );
}

/// 2. `tax-in-fiat` (a fiat-receivable tax under specie-only wages): the tax settles in
///    fiat (`tax_receipts_fiat > 0`) — fiat circulates through the **fiscal** channel
///    where the **labor** channel refused it (`wage_fiat_settled == 0`). The chartalist
///    counter-lever: the state compels what the market would not. (Sign only.)
#[test]
fn fiat_tax_compels_fiat_demand() {
    let tax = run(SettlementConfig::tax_in_fiat(), 80);

    assert_eq!(tax.tax_receivability(), TaxReceivability::FiatOnly);
    assert!(
        tax.tax_receipts_fiat() > Gold::ZERO,
        "a fiat-receivable tax must compel fiat through the fiscal channel"
    );

    // The labor market refused fiat: specie-only wages, no fiat wage ever settled — yet
    // fiat moved through the fiscal channel. That contrast is the whole headline.
    assert_eq!(tax.labor_wage_tender(), LaborWageTender::SpecieOnly);
    assert_eq!(
        tax.wage_fiat_settled(),
        Gold::ZERO,
        "the labor market refused fiat (specie-only wages)"
    );

    // The credit cycle itself stays inert (the fiscal channel, not a transmitted boom,
    // moved the fiat): the fiat-credit issuance never reached the real economy.
    assert!(
        !tax.cycle_fired(),
        "the cycle is inert under specie-only wages; the fiscal channel is what moved fiat"
    );
}

/// 3. `tax-in-specie` (a specie-receivable tax, the control): `tax_receipts_specie > 0`,
///    `tax_receipts_fiat == 0`. Paired with test 2 — and with the *only* difference being
///    the receivability (the levy set is identical) — this isolates the compelled fiat
///    demand to the receivability policy (the counter-lever, not the levy or the spatial
///    dynamics).
#[test]
fn specie_tax_compels_no_fiat_demand() {
    let control = run(SettlementConfig::tax_in_specie(), 80);

    assert_eq!(control.tax_receivability(), TaxReceivability::SpecieOnly);
    assert!(
        control.tax_receipts_specie() > Gold::ZERO,
        "a specie-receivable tax settles in specie"
    );
    assert_eq!(
        control.tax_receipts_fiat(),
        Gold::ZERO,
        "the specie-receivable control compels NO fiat demand"
    );

    // The falsification twin: the headline compels fiat, the control compels none — and
    // the levy is identical (same total levied), so the difference is the gate alone.
    let headline = run(SettlementConfig::tax_in_fiat(), 80);
    assert!(
        headline.tax_receipts_fiat() > Gold::ZERO && control.tax_receipts_fiat() == Gold::ZERO,
        "the receivability policy alone decides whether the tax compels fiat demand"
    );
    assert_eq!(
        headline.taxes_levied(),
        control.taxes_levied(),
        "the levy is identical across the twin — only the receivability differs"
    );
}

/// 4. Tax is **fiscal, not credit**: the tax levy/receipt does NOT move `credit_retired`
///    or `fiat_credit_outstanding` (econ's M21 fact 2). The tax settles with positive
///    receipts, yet through the levy's due tick `credit_retired` is zero (the cycle's own
///    loan repayments come later), so the receipt is not credit retirement; and the levy
///    is a zero-principal liability owed to the single issuer, funded as tax — not credit.
#[test]
fn tax_is_fiscal_not_credit() {
    for (config, settled_in_fiat) in [
        (SettlementConfig::tax_in_fiat(), true),
        (SettlementConfig::tax_in_specie(), false),
    ] {
        let tax = run(config, 80);

        // The fiscal channel is active (receipts in the configured medium)...
        if settled_in_fiat {
            assert!(tax.tax_receipts_fiat() > Gold::ZERO);
        } else {
            assert!(tax.tax_receipts_specie() > Gold::ZERO);
        }

        // ...but it booked NO credit retirement: through the levy's due tick,
        // `credit_retired` is zero. A tax receipt moves `fiat_retired` / the specie
        // vault, never the credit aggregates.
        let records = &tax.society().m3_records;
        for (tick, record) in records.iter().enumerate().take(TAX_DUE_TICK + 1) {
            assert_eq!(
                record.credit_retired,
                Gold::ZERO,
                "the tax receipt must not retire credit (tick {tick})"
            );
        }

        // The levy is a zero-principal liability owed to the single state issuer, funded
        // as tax — structurally not a credit extension.
        let tax_debts: Vec<_> = tax
            .society()
            .debts
            .iter()
            .filter(|debt| debt.purpose == DebtPurpose::TaxLiability)
            .collect();
        assert_eq!(
            tax_debts.len(),
            2,
            "the counter-lever seeds exactly two levies"
        );
        for debt in tax_debts {
            assert_eq!(debt.principal, Gold::ZERO, "a tax lends nothing");
            assert!(
                matches!(debt.lender, CreditLender::Issuer(_)),
                "a tax is owed to the issuer"
            );
            assert!(
                matches!(debt.funding, CreditSource::Tax(_)),
                "a tax is funded as tax, not credit"
            );
        }
    }
}

/// 5. Whole-system conservation across levy → settlement: every levied unit is either
///    received (into the issuer, in the receivable medium) or **defaulted** (unmet by
///    rule, never a leak). The M3 ledger reconciles every tick, the fiat base is exactly
///    issued − retired, and `levied == receipts_fiat + receipts_specie + defaulted`.
#[test]
fn tax_settlement_conserves() {
    for config in [
        SettlementConfig::tax_in_fiat(),
        SettlementConfig::tax_in_specie(),
    ] {
        let mut s = Settlement::generate(SEED, &config);
        for t in 0..80 {
            s.econ_tick();
            assert!(
                s.society().money_ledgers_reconcile(),
                "the M3 ledger must reconcile every tick under the tax at tick {t}"
            );
        }

        // Every levied unit is accounted for: received or defaulted, never created or
        // destroyed. A default is unmet-by-rule, not a leak.
        assert_eq!(
            s.taxes_levied(),
            s.tax_receipts_fiat()
                .saturating_add(s.tax_receipts_specie())
                .saturating_add(s.taxes_defaulted()),
            "levied == received (fiat + specie) + defaulted"
        );
        assert!(
            s.taxes_levied() > Gold::ZERO && s.taxes_defaulted() > Gold::ZERO,
            "the counter-lever exercised both a settlement and a by-rule default"
        );

        // The fiat base stays the exact issued − retired identity (a fiat tax receipt is
        // honest money contraction, booked into `fiat_retired`).
        assert_eq!(
            s.fiat_base(),
            s.fiat_issued().saturating_sub(s.fiat_retired()),
            "the fiat base is exactly issued − retired"
        );
    }
}

/// 6. The receivability gate decides the tax surface: a medium not in the active
///    `TaxReceivability` cannot discharge the tax **even if held**; the receivable medium
///    does (the M21 gate, in the sim). Under a fiat-receivable tax the specie-holder
///    defaults though it holds specie; under a specie-receivable tax the fiat-holder
///    defaults though it holds fiat.
#[test]
fn tax_receivability_gates_the_tax_surface() {
    // A short horizon: past the levy's due tick (the debts have settled/defaulted) but
    // before the cycle's loans unwind, so the refused-but-held medium is still on hand.
    let ticks = 12;

    // Fiat-receivable: the held fiat discharges; the held specie is refused (defaults).
    let fiat = run(SettlementConfig::tax_in_fiat(), ticks);
    assert_eq!(
        tax_debt_state(&fiat, FIAT_HOLDER),
        DebtState::Settled,
        "the held fiat discharges the fiat tax"
    );
    assert_eq!(
        tax_debt_state(&fiat, SPECIE_HOLDER),
        DebtState::Defaulted,
        "specie cannot discharge a fiat-receivable tax"
    );
    assert!(
        specie_held(&fiat, SPECIE_HOLDER) > Gold::ZERO,
        "the refused medium (specie) was held — it is the gate, not lack of funds"
    );

    // Specie-receivable: the mirror — the held specie discharges; the held fiat defaults.
    let specie = run(SettlementConfig::tax_in_specie(), ticks);
    assert_eq!(
        tax_debt_state(&specie, SPECIE_HOLDER),
        DebtState::Settled,
        "the held specie discharges the specie tax"
    );
    assert_eq!(
        tax_debt_state(&specie, FIAT_HOLDER),
        DebtState::Defaulted,
        "fiat cannot discharge a specie-receivable tax"
    );
    assert!(
        fiat_held(&specie, FIAT_HOLDER) > Gold::ZERO,
        "the refused medium (fiat) was held — it is the gate, not lack of funds"
    );
}

/// 7. `econ_unchanged` — the tax additions are inert for every settlement that levies no
///    tax (it surfaces no tax and is byte-identical to a twin), and a tax run keeps econ's
///    invariants tick over tick. The six econ goldens staying byte-identical and the full
///    workspace suite + `cargo clippy --workspace --all-targets -- -D warnings` + `cargo
///    fmt --check` are the real gate; this checks the local seam.
#[test]
fn econ_unchanged() {
    // A plain cycle and a spatial settlement levy no tax; the active receivability reads
    // back as the econ default and nothing is levied.
    let plain_cycle = Settlement::generate(SEED, &SettlementConfig::credit_cycle());
    assert!(!plain_cycle.is_tax());
    assert_eq!(plain_cycle.configured_tax_receivability(), None);
    assert_eq!(
        plain_cycle.tax_receivability(),
        TaxReceivability::SpecieOnly,
        "econ's default receivability"
    );
    assert_eq!(plain_cycle.taxes_levied(), Gold::ZERO);

    let spatial = Settlement::generate(7, &SettlementConfig::m3_settlement());
    assert!(!spatial.is_tax());

    // A non-tax cycle is byte-identical to a twin — the G8c-3 additions are present but
    // unexercised (the canonical tax block is omitted entirely for a non-tax settlement).
    let mut a = Settlement::generate(42, &SettlementConfig::wage_refusal_cycle());
    let mut b = Settlement::generate(42, &SettlementConfig::wage_refusal_cycle());
    a.run(40);
    b.run(40);
    assert_eq!(a.canonical_bytes(), b.canonical_bytes());

    // A tax run keeps econ's invariants tick over tick: the M3 ledger reconciles.
    let mut tax = Settlement::generate(3, &SettlementConfig::tax_in_fiat());
    for _ in 0..40 {
        tax.econ_tick();
        assert!(
            tax.society().money_ledgers_reconcile(),
            "the M3 ledger reconciles every tick under a tax"
        );
    }
}

// ---- unit tests -------------------------------------------------------------

/// The tax overlay rides on the credit cycle (the chartalist counter-lever to the wage
/// refusal); a tax config without the cycle is rejected at generation.
#[test]
#[should_panic(expected = "tax overlay requires the credit cycle")]
fn tax_overlay_requires_the_cycle() {
    let mut config = SettlementConfig::tax_in_fiat();
    config.cycle = None;
    let _ = Settlement::generate(SEED, &config);
}

/// The tax-in-fiat headline and the tax-in-specie control are a falsification twin: the
/// SAME levy set, opposite receivabilities, opposite receipt media. They differ in
/// exactly the receivability, so their canonical bytes diverge from generation (the
/// receivability is canonical policy state) but the levies are identical.
#[test]
fn tax_twin_differs_only_in_receivability() {
    let fiat = SettlementConfig::tax_in_fiat()
        .tax
        .expect("the headline has a tax overlay");
    let specie = SettlementConfig::tax_in_specie()
        .tax
        .expect("the control has a tax overlay");
    assert_ne!(
        fiat.receivability, specie.receivability,
        "the twin differs in the receivability"
    );
    assert_eq!(
        fiat.levies, specie.levies,
        "the twin levies the SAME set — only the receivability differs"
    );

    // The receivability is canonical policy state, so the bytes diverge even at
    // generation (before the Tick(0) event fires), like the wage-tender twin.
    let a = Settlement::generate(SEED, &SettlementConfig::tax_in_fiat());
    let b = Settlement::generate(SEED, &SettlementConfig::tax_in_specie());
    assert_ne!(
        a.canonical_bytes(),
        b.canonical_bytes(),
        "the receivability is canonical policy state — the twin must not collide"
    );
}

/// A tax settlement is the finance (credit-cycle) settlement, not a tender bench: the tax
/// overlay layers onto the cycle, never a bench surface.
#[test]
fn tax_settlement_is_the_cycle_not_a_bench() {
    let s = Settlement::generate(SEED, &SettlementConfig::tax_in_fiat());
    assert!(s.is_tax());
    assert!(s.is_cycle());
    assert!(!s.is_tender_bench());
    assert_eq!(s.bench_surface(), None);
}

/// The configured receivability is the policy the config chose; the active receivability
/// is still econ's default until the `Tick(0)` `SetTaxReceivability` event fires (like
/// the wage tender and the regime), then reads back the configured value.
#[test]
fn configured_and_active_receivability() {
    let mut s = Settlement::generate(SEED, &SettlementConfig::tax_in_fiat());
    assert_eq!(
        s.configured_tax_receivability(),
        Some(TaxReceivability::FiatOnly)
    );
    assert_eq!(
        s.tax_receivability(),
        TaxReceivability::SpecieOnly,
        "the Tick(0) event has not fired at generation"
    );
    s.run(80);
    assert_eq!(
        s.tax_receivability(),
        TaxReceivability::FiatOnly,
        "after running, the active receivability reads back the configured policy"
    );
}
