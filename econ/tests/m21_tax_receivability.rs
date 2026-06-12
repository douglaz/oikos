//! M21 tax-receivability acceptance tests (impl-23.md).
//!
//! A tax is a `DebtContract` with `principal = ZERO` whose lender is the
//! issuer; the existing payables view (`agent_debt_views`) is the demand
//! mechanism. Settlement is gated by `TaxReceivability`, never the credit
//! tenders, and tax receipts NEVER move `credit_retired` or
//! `fiat_credit_outstanding`.
//!
//! Known Seam (declared, not engineered around): the payable-accounting labor
//! pull is AMOUNT-based; media enter only at settlement. The headline worker
//! works to cover the levy amount and is paid in fiat solely because the
//! employer holds nothing else and the ledger debit order is fiat-first — not
//! through any media-aware planning.

use econ::agent::{Agent, AgentId, Role, Want, WantKind};
use econ::bank::Bank;
use econ::good::{Gold, Horizon, Stock, GOLD};
use econ::issuer::{Issuer, IssuerPolicy};
use econ::ledger::{BankId, IssuerId, MoneySystem};
use econ::money::{BankRepaymentTender, IssuerRepaymentTender, PublicDebtTender, TaxReceivability};
use econ::project::Tick;
use econ::purpose::{CreditLender, CreditSource, DebtPurpose};
use econ::record::{DebtPaymentState, TaxAuditRecord};
use econ::scenario::{builtin_market_scenario, Event, EventKind, ScenarioName};
use econ::shadow::credit_disabled_scenario;
use econ::society::Society;
use econ::timemarket::{
    settle_due_debts_m3, DebtContract, DebtId, DebtSettlementM3Context, DebtSettlementSummary,
    DebtState,
};

const ISSUER: IssuerId = IssuerId(1);

// --- helpers -------------------------------------------------------------

fn agent(id: u32) -> Agent {
    Agent {
        id: AgentId(id),
        scale: vec![Want {
            kind: WantKind::Good(GOLD),
            horizon: Horizon::Later(4),
            qty: 1,
            satisfied: false,
        }],
        stock: Stock::new(3),
        gold: Gold::ZERO,
        labor_capacity: 0,
        hunger_deficit: 0,
        roles: vec![Role::Trader],
        expect: Vec::new(),
    }
}

fn fresh_issuer(id: IssuerId) -> Issuer {
    Issuer {
        id,
        fiat_issued: Gold::ZERO,
        fiat_retired: Gold::ZERO,
        fiat_credit_outstanding: Gold::ZERO,
        policy: IssuerPolicy::default(),
        taxes_levied: Gold::ZERO,
        tax_receipts_fiat: Gold::ZERO,
        tax_receipts_specie: Gold::ZERO,
        taxes_defaulted: Gold::ZERO,
    }
}

fn tax_debt(id: u64, borrower: AgentId, due: Gold, due_tick: Tick) -> DebtContract {
    DebtContract {
        id: DebtId(id),
        lender: CreditLender::Issuer(ISSUER),
        borrower,
        opened_tick: Tick(0),
        due_tick,
        principal: Gold::ZERO,
        due,
        paid: Gold::ZERO,
        state: DebtState::Open,
        purpose: DebtPurpose::TaxLiability,
        funding: CreditSource::Tax(ISSUER),
    }
}

/// Settles due debts under a tax-receivability policy with no banks, returning
/// the summary and the tax audit rows it produced.
fn settle_tax(
    agents: &mut [Agent],
    debts: &mut [DebtContract],
    money: &mut MoneySystem,
    issuers: &mut [Issuer],
    receivability: TaxReceivability,
    tick: Tick,
) -> (DebtSettlementSummary, Vec<TaxAuditRecord>) {
    let mut banks: Vec<Bank> = Vec::new();
    let mut debt_payment_audit = Vec::new();
    let mut bank_repayment_audit = Vec::new();
    let mut issuer_repayment_audit = Vec::new();
    let mut tax_audit = Vec::new();
    let summary = settle_due_debts_m3(DebtSettlementM3Context {
        agents,
        debts,
        tick,
        money_system: money,
        banks: &mut banks,
        issuers,
        public_debt_tender: PublicDebtTender::ParAll,
        bank_repayment_tender: BankRepaymentTender::ParAll,
        issuer_repayment_tender: IssuerRepaymentTender::FiatOnly,
        tax_receivability: receivability,
        debt_payment_audit: &mut debt_payment_audit,
        bank_repayment_audit: &mut bank_repayment_audit,
        issuer_repayment_audit: &mut issuer_repayment_audit,
        tax_audit: &mut tax_audit,
    });
    (summary, tax_audit)
}

fn run(name: ScenarioName) -> Society {
    let scenario = builtin_market_scenario(name);
    let periods = scenario.periods;
    let mut society = Society::from_scenario(scenario);
    society.run(periods);
    society
}

fn money(society: &Society) -> &MoneySystem {
    society
        .money_system
        .as_ref()
        .expect("M3 society has a money system")
}

fn public_specie(society: &Society, agent: AgentId) -> Gold {
    money(society)
        .balance_snapshot(agent)
        .map(|balance| balance.public_specie)
        .unwrap_or(Gold::ZERO)
}

fn issuer_of(society: &Society, id: IssuerId) -> &Issuer {
    society
        .issuers
        .iter()
        .find(|issuer| issuer.id == id)
        .expect("issuer present")
}

fn event_kinds(name: ScenarioName) -> Vec<EventKind> {
    builtin_market_scenario(name)
        .events
        .iter()
        .map(|event| event.kind.clone())
        .collect()
}

// --- tests ---------------------------------------------------------------

/// 1. `LevyTax` produces a zero-principal debt with the right lender, due,
///    purpose, and funding, moves no money, and the liability appears in the
///    taxed agent's payables view (the open debts where it is borrower).
#[test]
fn levy_creates_a_liability_and_a_payable() {
    let mut society = Society::from_scenario(builtin_market_scenario(
        ScenarioName::EmergedGoldTaxSpecieControl,
    ));
    // One tick: the tick-0 levy seeds the debt; its due tick (2) has not yet
    // arrived, so it is still open and nothing has been collected.
    society.run(1);

    let taxed = AgentId(50);
    let payables: Vec<&DebtContract> = society
        .debts
        .iter()
        .filter(|debt| debt.borrower == taxed && debt.is_open())
        .collect();
    assert_eq!(payables.len(), 1, "exactly one open tax payable");
    let debt = payables[0];
    assert_eq!(debt.lender, CreditLender::Issuer(ISSUER));
    assert_eq!(debt.principal, Gold::ZERO, "a tax lends nothing");
    assert_eq!(debt.due, Gold(2));
    assert_eq!(debt.purpose, DebtPurpose::TaxLiability);
    assert_eq!(debt.funding, CreditSource::Tax(ISSUER));
    assert_eq!(debt.state, DebtState::Open);

    // No money moved at levy time: the holder keeps its full specie endowment.
    assert_eq!(public_specie(&society, taxed), Gold(3));
    assert_eq!(money(&society).public_fiat(taxed), Gold::ZERO);
    assert_eq!(issuer_of(&society, ISSUER).taxes_levied, Gold(2));
    assert!(society.tax_audit.is_empty(), "nothing due yet");
}

/// 2. A levy with no issuer in the scenario is a no-op: no debt, no panic.
#[test]
fn levy_without_issuer_is_a_noop() {
    // EmergedGoldSoundControl has no issuer (no fiat-issuer start).
    let mut scenario = builtin_market_scenario(ScenarioName::EmergedGoldSoundControl);
    assert!(
        Society::from_scenario(scenario.clone()).issuers.is_empty(),
        "the sound-control scenario starts with no issuer"
    );
    scenario.events.push(Event {
        tick: Tick(0),
        kind: EventKind::LevyTax {
            agent: AgentId(7),
            amount: Gold(2),
            due_tick: Tick(2),
        },
    });
    let periods = scenario.periods;
    let mut society = Society::from_scenario(scenario);
    society.run(periods);

    assert!(society.debts.is_empty(), "no issuer => no tax debt");
    assert!(society.tax_audit.is_empty());
}

#[test]
fn levy_with_ambiguous_issuers_is_a_noop() {
    let mut society = Society::from_scenario(builtin_market_scenario(
        ScenarioName::EmergedGoldTaxSpecieControl,
    ));
    society.issuers.push(fresh_issuer(IssuerId(2)));
    society.run(1);

    assert!(
        society.debts.is_empty(),
        "LevyTax has no issuer field, so multi-issuer taxes are ambiguous"
    );
    assert!(society
        .issuers
        .iter()
        .all(|issuer| issuer.taxes_levied == Gold::ZERO));
    assert!(society.tax_audit.is_empty());
}

/// 3. The specie-only control settles into the issuer vault: public specie and
///    TMS fall by the levy, the conserved commodity base is unchanged, and no
///    fiat moves.
#[test]
fn specie_tax_settles_into_the_vault() {
    let society = run(ScenarioName::EmergedGoldTaxSpecieControl);

    // Proof row.
    assert_eq!(society.tax_audit.len(), 1);
    let row = &society.tax_audit[0];
    assert_eq!(row.tick, 2);
    assert_eq!(row.agent, AgentId(50));
    assert_eq!(row.issuer, ISSUER);
    assert_eq!(row.owed, Gold(2));
    assert_eq!(row.paid, Gold(2));
    assert_eq!(row.paid_specie, Gold(2));
    assert_eq!(row.paid_fiat, Gold::ZERO);
    assert_eq!(row.receivability, TaxReceivability::SpecieOnly);
    assert_eq!(row.state, DebtPaymentState::Settled);

    // Conservation across the due tick: public specie and TMS fall by exactly
    // the levy; the commodity base (which includes the vault) is conserved.
    let before = &society.m3_records[1];
    let after = &society.m3_records[2];
    assert_eq!(
        after.public_specie,
        before.public_specie.saturating_sub(Gold(2))
    );
    assert_eq!(after.tms, before.tms.saturating_sub(Gold(2)));
    assert_eq!(after.m2.total_gold, before.m2.total_gold, "commodity base");
    assert_eq!(after.public_fiat, Gold::ZERO, "no fiat ever moves");

    // Vault credited via the issuer's specie-receipt counter.
    let issuer = issuer_of(&society, ISSUER);
    assert_eq!(issuer.tax_receipts_specie, Gold(2));
    assert_eq!(issuer.tax_receipts_fiat, Gold::ZERO);
    assert!(society.money_ledgers_reconcile());
}

/// 4. A fiat-only levy on a wealthy but fiat-less subject defaults: it is
///    receivability, not ability to pay, that gates discharge. No money moves
///    and the audit row shows paid 0 against positive specie holdings.
#[test]
fn fiat_tax_on_the_fiatless_defaults_despite_wealth() {
    let society = run(ScenarioName::EmergedGoldTaxFiatUnpayableDefaults);

    assert_eq!(society.tax_audit.len(), 1);
    let row = &society.tax_audit[0];
    assert_eq!(row.agent, AgentId(50));
    assert_eq!(row.owed, Gold(2));
    assert_eq!(row.paid, Gold::ZERO);
    assert_eq!(row.remaining, Gold(2));
    assert_eq!(row.paid_fiat, Gold::ZERO);
    assert_eq!(row.paid_specie, Gold::ZERO);
    assert_eq!(row.receivability, TaxReceivability::FiatOnly);
    assert_eq!(row.state, DebtPaymentState::Defaulted);

    // The subject is wealthy in specie and still defaulted; its specie is
    // untouched and it never held any fiat.
    assert_eq!(public_specie(&society, AgentId(50)), Gold(5));
    assert_eq!(money(&society).public_fiat(AgentId(50)), Gold::ZERO);

    let issuer = issuer_of(&society, ISSUER);
    assert_eq!(issuer.taxes_defaulted, Gold(2));
    assert_eq!(issuer.tax_receipts_fiat, Gold::ZERO);
    assert_eq!(issuer.tax_receipts_specie, Gold::ZERO);
}

/// 5. The headline: a fiat-only tax pulls the worker into fiat-wage labor; the
///    wage settles in fiat, the tax settles in fiat, the fiat returns to the
///    issuer (TMS falls by the levy), and no spot trade settles fiat anywhere.
#[test]
fn tax_drives_fiat_labor() {
    let society = run(ScenarioName::EmergedGoldTaxDrivesFiatLabor);
    let worker = AgentId(61);

    // A labor trade exists for the worker and the wage settled in fiat.
    let worker_trades: Vec<_> = society
        .labor_trades
        .iter()
        .filter(|trade| trade.worker == worker)
        .collect();
    assert_eq!(worker_trades.len(), 1, "the worker works exactly once");
    let wage_row = society
        .wage_payment_audit
        .iter()
        .find(|row| row.worker == worker)
        .expect("the wage was paid");
    assert!(wage_row.public_fiat > Gold::ZERO, "the wage is fiat");
    assert_eq!(wage_row.public_specie, Gold::ZERO);

    // The tax settled in fiat for exactly the levy.
    assert_eq!(society.tax_audit.len(), 1);
    let row = &society.tax_audit[0];
    assert_eq!(row.agent, worker);
    assert_eq!(row.paid, Gold(2));
    assert_eq!(row.paid_fiat, Gold(2));
    assert_eq!(row.paid_specie, Gold::ZERO);
    assert_eq!(row.state, DebtPaymentState::Settled);

    // The fiat returns to the issuer: receipts and TMS move by exactly the levy.
    let issuer = issuer_of(&society, ISSUER);
    assert_eq!(issuer.tax_receipts_fiat, Gold(2));
    let before = &society.m3_records[usize::try_from(row.tick).unwrap() - 1];
    let after = &society.m3_records[usize::try_from(row.tick).unwrap()];
    assert_eq!(after.tms, before.tms.saturating_sub(Gold(2)));
    assert_eq!(
        after.public_specie, before.public_specie,
        "specie untouched"
    );

    // No spot trade in the run settled fiat (spot tender is specie-only).
    assert!(
        society
            .payment_audit
            .iter()
            .all(|payment| payment.public_fiat == Gold::ZERO),
        "spot markets refuse fiat outright"
    );

    // Concerns2 Option-A: tax receipts never contract created credit.
    assert!(society
        .m3_records
        .iter()
        .all(|record| record.credit_retired == Gold::ZERO));
}

/// 6. The falsification twin: with the levy removed and NOTHING else changed,
///    the worker never works, holds no fiat, and the employer's fiat sits idle.
///    Pinned against the headline as a paired falsification.
#[test]
fn no_tax_no_labor() {
    let society = run(ScenarioName::EmergedGoldNoTaxIdleControl);
    let worker = AgentId(61);
    let employer = AgentId(60);

    assert!(
        society
            .labor_trades
            .iter()
            .all(|trade| trade.worker != worker),
        "no labor trades for the worker"
    );
    assert!(society.tax_audit.is_empty());
    assert_eq!(money(&society).public_fiat(worker), Gold::ZERO);
    // The employer's printed fiat sits idle: it still holds the full issue.
    assert_eq!(money(&society).public_fiat(employer), Gold(8));

    // The twin's event list is the headline's MINUS exactly the LevyTax event.
    let headline = event_kinds(ScenarioName::EmergedGoldTaxDrivesFiatLabor);
    let twin = event_kinds(ScenarioName::EmergedGoldNoTaxIdleControl);
    let headline_without_levy: Vec<EventKind> = headline
        .iter()
        .filter(|kind| !matches!(kind, EventKind::LevyTax { .. }))
        .cloned()
        .collect();
    assert_eq!(headline_without_levy, twin);
    assert_eq!(headline.len(), twin.len() + 1);
    assert!(headline
        .iter()
        .any(|kind| matches!(kind, EventKind::LevyTax { .. })));
    assert!(twin
        .iter()
        .all(|kind| !matches!(kind, EventKind::LevyTax { .. })));
}

/// 7. Tax receipts never move `credit_retired` or `fiat_credit_outstanding`,
///    across the specie and fiat-labor scenarios and a unit fiat settlement.
#[test]
fn tax_receipts_never_touch_credit_retired() {
    for name in [
        ScenarioName::EmergedGoldTaxSpecieControl,
        ScenarioName::EmergedGoldTaxDrivesFiatLabor,
    ] {
        let society = run(name);
        assert!(
            society
                .m3_records
                .iter()
                .all(|record| record.credit_retired == Gold::ZERO),
            "{name:?} credit_retired stayed at the base"
        );
        assert!(
            society
                .issuers
                .iter()
                .all(|issuer| issuer.fiat_credit_outstanding == Gold::ZERO),
            "{name:?} no created credit outstanding"
        );
    }

    // Unit fiat settlement: a fiat tax receipt bumps fiat_retired (the honest
    // money-contraction counter) but never fiat_credit_outstanding.
    let mut agents = vec![agent(1)];
    let mut money = MoneySystem::from_agents(&agents);
    money.credit_fiat(AgentId(1), Gold(2)).unwrap();
    money.reconcile_agent_cache(&mut agents);
    let mut issuers = vec![fresh_issuer(ISSUER)];
    let mut debts = vec![tax_debt(1, AgentId(1), Gold(2), Tick(1))];

    let (summary, audit) = settle_tax(
        &mut agents,
        &mut debts,
        &mut money,
        &mut issuers,
        TaxReceivability::FiatOnly,
        Tick(1),
    );

    assert_eq!(summary.credit_retired, Gold::ZERO);
    assert_eq!(audit[0].state, DebtPaymentState::Settled);
    assert_eq!(issuers[0].fiat_credit_outstanding, Gold::ZERO);
    assert_eq!(issuers[0].fiat_retired, Gold(2), "honest fiat contraction");
    assert_eq!(issuers[0].tax_receipts_fiat, Gold(2));
}

/// 8. A partial payment defaults with the unpaid remainder recorded.
#[test]
fn partial_payment_defaults_with_remainder() {
    let mut agents = vec![agent(1)];
    let mut money = MoneySystem::from_agents(&agents);
    money.credit_fiat(AgentId(1), Gold(1)).unwrap();
    money.reconcile_agent_cache(&mut agents);
    let mut issuers = vec![fresh_issuer(ISSUER)];
    let mut debts = vec![tax_debt(1, AgentId(1), Gold(2), Tick(1))];

    let (summary, audit) = settle_tax(
        &mut agents,
        &mut debts,
        &mut money,
        &mut issuers,
        TaxReceivability::FiatOnly,
        Tick(1),
    );

    assert_eq!(summary.defaulted, 1);
    assert_eq!(summary.paid, Gold(1));
    assert_eq!(debts[0].state, DebtState::Defaulted);
    assert_eq!(debts[0].paid, Gold(1));

    let row = &audit[0];
    assert_eq!(row.paid, Gold(1));
    assert_eq!(row.paid_fiat, Gold(1));
    assert_eq!(row.remaining, Gold(1));
    assert_eq!(row.state, DebtPaymentState::Defaulted);

    assert_eq!(issuers[0].tax_receipts_fiat, Gold(1));
    assert_eq!(issuers[0].taxes_defaulted, Gold(1));
}

/// 9. Receivability gates media, not amounts: a fiat+specie+claim holder pays
///    only from accepted media under each policy, and bank claims are never
///    accepted.
#[test]
fn receivability_gates_media_not_amounts() {
    // owed exceeds fiat+specie so the (never-accepted) claim is the only thing
    // that could close the gap; it never does.
    for (policy, paid_fiat, paid_specie, state) in [
        (
            TaxReceivability::SpecieOnly,
            Gold::ZERO,
            Gold(2),
            DebtState::Defaulted,
        ),
        (
            TaxReceivability::FiatOnly,
            Gold(2),
            Gold::ZERO,
            DebtState::Defaulted,
        ),
        (
            TaxReceivability::FiatAndSpecie,
            Gold(2),
            Gold(2),
            DebtState::Defaulted,
        ),
    ] {
        let mut agents = vec![agent(1)];
        let mut money = MoneySystem::from_agents(&agents);
        money.credit_fiat(AgentId(1), Gold(2)).unwrap();
        money.credit_specie(AgentId(1), Gold(2)).unwrap();
        // A demand claim the tax must never touch (bank claims are non-tender).
        money
            .issue_demand_claim(BankId(1), AgentId(1), Gold(2), Gold::ZERO)
            .unwrap();
        money.reconcile_agent_cache(&mut agents);
        let mut issuers = vec![fresh_issuer(ISSUER)];
        let mut debts = vec![tax_debt(1, AgentId(1), Gold(6), Tick(1))];

        let (_summary, audit) = settle_tax(
            &mut agents,
            &mut debts,
            &mut money,
            &mut issuers,
            policy,
            Tick(1),
        );

        let row = &audit[0];
        assert_eq!(row.paid_fiat, paid_fiat, "{policy:?} fiat");
        assert_eq!(row.paid_specie, paid_specie, "{policy:?} specie");
        assert_eq!(debts[0].state, state, "{policy:?} state");
        // The claim is never drawn: the holder keeps its full Gold(2) claim,
        // so the remainder is at least the claim it could not tender.
        assert!(row.remaining >= Gold(2), "{policy:?} claim untouched");
    }

    // The media table itself never admits bank claims.
    for policy in [
        TaxReceivability::SpecieOnly,
        TaxReceivability::FiatOnly,
        TaxReceivability::FiatAndSpecie,
    ] {
        assert!(!policy.accepted_media().bank_claims, "{policy:?}");
    }
}

/// 10. Taxes are policy, not credit: the credit-disabled shadow preserves the
///     tax events and settles or defaults them identically.
#[test]
fn shadow_levies_the_same_taxes() {
    for name in [
        ScenarioName::EmergedGoldTaxSpecieControl,
        ScenarioName::EmergedGoldTaxFiatUnpayableDefaults,
    ] {
        let normal = run(name);

        let shadow_scenario = credit_disabled_scenario(&builtin_market_scenario(name));
        // The shadow preserves both tax events unchanged.
        assert!(shadow_scenario
            .events
            .iter()
            .any(|event| matches!(event.kind, EventKind::SetTaxReceivability(_))));
        assert!(shadow_scenario
            .events
            .iter()
            .any(|event| matches!(event.kind, EventKind::LevyTax { .. })));
        let periods = shadow_scenario.periods;
        let mut shadow = Society::from_scenario(shadow_scenario);
        shadow.run(periods);

        assert_eq!(
            normal.tax_audit, shadow.tax_audit,
            "{name:?} tax outcome is credit-independent"
        );
        // The shadow truly disabled created credit.
        assert!(shadow
            .m3_records
            .iter()
            .all(|record| record.fiat_credit_issued == Gold::ZERO
                && record.fiat_fiscal_issued == Gold::ZERO));
    }
}

/// 11. M0 ignores tax events entirely (no debts, no panic). Tape exclusivity
///     and the M3-only gate are covered by the CLI tests in `praxsim`.
#[test]
fn m0_ignores_tax_events() {
    use econ::scenario::builtin_scenario;
    use econ::sim::World;

    let base = builtin_scenario(ScenarioName::CrusoeSurvival);
    let mut scenario = base.clone();
    scenario.events.push(Event {
        tick: Tick(0),
        kind: EventKind::SetTaxReceivability(TaxReceivability::FiatOnly),
    });
    scenario.events.push(Event {
        tick: Tick(0),
        kind: EventKind::LevyTax {
            agent: AgentId(1),
            amount: Gold(2),
            due_tick: Tick(1),
        },
    });
    let periods = scenario.periods;
    let mut control = World::from_scenario(base);
    let mut taxed = World::from_scenario(scenario);
    control.run(periods);
    taxed.run(periods);

    // The tax events are inert at M0: the run is identical to the control.
    assert_eq!(control.records, taxed.records);
}
