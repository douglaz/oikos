use econ::agent::{Agent, AgentId, Role, Want, WantKind};
use econ::bank::Bank;
use econ::good::{Gold, GoodId, Horizon, Stock, GOLD};
use econ::issuer::{Issuer, IssuerPolicy};
use econ::ledger::{BankId, IssuerId, MoneySystem};
use econ::money::{
    BankRepaymentTender, IssuerRepaymentTender, PublicDebtTender, Regime, ReserveRatioBps,
};
use econ::project::Tick;
use econ::purpose::{CreditLender, CreditSource, DebtPurpose, ProjectPlanId};
use econ::record::{
    BankRepaymentAuditRecord, DebtPaymentAuditRecord, DebtPaymentState, IssuerRepaymentAuditRecord,
    M3Record,
};
use econ::scenario::{
    builtin_market_scenario, emerged_gold_bridge_agents, EventKind, ScenarioName,
};
use econ::shadow::credit_disabled_scenario;
use econ::society::Society;
use econ::timemarket::{
    settle_due_debts_m3, DebtContract, DebtId, DebtSettlementM3Context, DebtState, LoanTrade,
};

const BRIDGE_GOLD: Gold = Gold(16);
const REPAYMENT_TICK: u64 = 13;
const REPAYMENT_DEBT: u64 = 10;
const REPAYMENT_ISSUER: IssuerId = IssuerId(1);
const REPAYMENT_BORROWER: AgentId = AgentId(212);
const CLAIM_BANK: BankId = BankId(1);

#[derive(Clone, Debug, PartialEq, Eq)]
struct SeedRow {
    agent: AgentId,
    gold: Gold,
    stock: Vec<(GoodId, u32)>,
    scale: Vec<(WantKind, Horizon, u32)>,
}

#[test]
fn m16_scenarios_preserve_m6_bridge_prefix() {
    let bridge = emerged_gold_bridge_agents();
    let expected = seed_rows(&bridge);
    let base = builtin_market_scenario(ScenarioName::EmergedGoldFiatCreditExpansion);

    assert_eq!(bridge_gold(&bridge), BRIDGE_GOLD);
    assert!(bridge.iter().all(|agent| agent.stock.get(GOLD) == 0));

    for name in m16_scenarios() {
        let scenario = builtin_market_scenario(name);
        let prefix = scenario
            .agents
            .iter()
            .take(bridge.len())
            .cloned()
            .collect::<Vec<_>>();

        assert_eq!(seed_rows(&prefix), expected, "{name:?} bridge prefix");
        assert_eq!(bridge_gold(&prefix), BRIDGE_GOLD, "{name:?} bridge gold");
        assert!(prefix.iter().all(|agent| agent.stock.get(GOLD) == 0));
        assert_eq!(seed_rows(&scenario.agents), seed_rows(&base.agents));
    }
}

#[test]
fn m16_scenarios_match_fiat_credit_prefix_until_tick_13() {
    let base = run_with_audits(ScenarioName::EmergedGoldFiatCreditExpansion);

    for name in m16_scenarios() {
        let society = run_with_audits(name);

        assert_eq!(
            m3_prefix(&base.m3_records, REPAYMENT_TICK),
            m3_prefix(&society.m3_records, REPAYMENT_TICK),
            "{name:?} M3 prefix"
        );
        assert_eq!(
            issuer_repayment_prefix(&base.issuer_repayment_audit, REPAYMENT_TICK),
            issuer_repayment_prefix(&society.issuer_repayment_audit, REPAYMENT_TICK),
            "{name:?} issuer repayment prefix"
        );
        assert_eq!(
            loan_prefix(&base.loan_trades, REPAYMENT_TICK),
            loan_prefix(&society.loan_trades, REPAYMENT_TICK),
            "{name:?} loan prefix"
        );
        assert!(society
            .m3_records
            .iter()
            .all(|record| record.fiat_fiscal_issued == Gold::ZERO));
    }
}

#[test]
fn m16_accelerates_existing_issuer_debt_only() {
    let base = run(ScenarioName::EmergedGoldFiatCreditExpansion);
    let base_scenario = builtin_market_scenario(ScenarioName::EmergedGoldFiatCreditExpansion);
    let debt = debt_by_id(&base.debts, DebtId(REPAYMENT_DEBT));

    assert_eq!(debt.lender, CreditLender::Issuer(REPAYMENT_ISSUER));
    assert_eq!(debt.borrower, REPAYMENT_BORROWER);
    assert_eq!(debt.principal, Gold(1));
    assert_eq!(debt.due, Gold(1));
    assert_eq!(debt.funding, CreditSource::FiatCredit(REPAYMENT_ISSUER));
    assert!(matches!(
        debt.purpose,
        DebtPurpose::ProjectFunding {
            plan: ProjectPlanId(7),
            ..
        }
    ));

    for (name, tender) in [
        (
            ScenarioName::EmergedGoldIssuerRepaymentFiatRefusalControl,
            IssuerRepaymentTender::FiatRefused,
        ),
        (
            ScenarioName::EmergedGoldIssuerRepaymentFiatTender,
            IssuerRepaymentTender::FiatOnly,
        ),
    ] {
        let scenario = builtin_market_scenario(name);
        assert_eq!(scenario.seed, base_scenario.seed);
        assert_eq!(
            seed_rows(&scenario.agents),
            seed_rows(&base_scenario.agents)
        );
        assert_eq!(
            event_signature(&scenario.events[..base_scenario.events.len()]),
            event_signature(&base_scenario.events)
        );
        let extra_events = &scenario.events[base_scenario.events.len()..];
        let expected_extra_events = if tender == IssuerRepaymentTender::FiatRefused {
            3
        } else {
            2
        };
        assert_eq!(extra_events.len(), expected_extra_events);
        assert!(matches!(
            extra_events[0].kind,
            EventKind::SetIssuerRepaymentTender(policy) if policy == tender
        ));
        assert!(matches!(
            extra_events[1].kind,
            EventKind::SetDebtDueTick {
                debt: DebtId(REPAYMENT_DEBT),
                due_tick: Tick(REPAYMENT_TICK),
            }
        ));
        if tender == IssuerRepaymentTender::FiatRefused {
            assert!(matches!(
                extra_events[2].kind,
                EventKind::SetIssuerRepaymentTender(IssuerRepaymentTender::FiatOnly)
            ));
            assert_eq!(extra_events[2].tick, Tick(REPAYMENT_TICK + 1));
        }
        assert!(scenario.events.iter().all(|event| !matches!(
            event.kind,
            EventKind::SeedCommodityDebt { .. }
                | EventKind::FiatPrint { .. }
                | EventKind::SeedStock { .. }
        )));
    }
}

#[test]
fn borrower_212_holds_fiat_before_m16_repayment() {
    let mut society = Society::from_scenario(builtin_market_scenario(
        ScenarioName::EmergedGoldFiatCreditExpansion,
    ));
    let mut tick_12 = None;
    let mut tick_13 = None;

    for tick in 0..=REPAYMENT_TICK {
        society.step();
        if tick == 12 || tick == REPAYMENT_TICK {
            let money = society.money_system.as_ref().expect("M3 money system");
            let debt = debt_by_id(&society.debts, DebtId(REPAYMENT_DEBT));
            let snapshot = money
                .balance_snapshot(REPAYMENT_BORROWER)
                .expect("borrower money balance");
            assert_eq!(snapshot.public_specie, Gold::ZERO);
            assert_eq!(debt.state, DebtState::Open);
            assert_eq!(debt.due_tick, Tick(15));
            if tick == 12 {
                tick_12 = Some(snapshot.public_fiat);
            } else {
                tick_13 = Some(snapshot.public_fiat);
            }
        }
    }

    assert_eq!(tick_12, Some(Gold(1)));
    assert_eq!(tick_13, Some(Gold(1)));
}

#[test]
fn issuer_repayment_refusal_defaults_without_retiring_fiat() {
    let refusal = run_through_tick(ScenarioName::EmergedGoldIssuerRepaymentFiatRefusalControl);
    let tender = run_through_tick(ScenarioName::EmergedGoldIssuerRepaymentFiatTender);
    let row = proof_row(&refusal.issuer_repayment_audit);
    let money = refusal.money_system.as_ref().expect("M3 money system");

    assert_eq!(row.tender, IssuerRepaymentTender::FiatRefused);
    assert_eq!(row.state, DebtPaymentState::Defaulted);
    assert_eq!(row.owed, Gold(1));
    assert_eq!(row.paid, Gold::ZERO);
    assert_eq!(row.remaining, Gold(1));
    assert_eq!(row.public_fiat, Gold::ZERO);
    assert_eq!(row.public_specie, Gold::ZERO);
    assert_eq!(row.credit_retired, Gold::ZERO);
    assert_eq!(money.public_fiat(REPAYMENT_BORROWER), Gold(1));
    assert_eq!(
        issuer(&refusal.issuers, REPAYMENT_ISSUER).fiat_credit_outstanding,
        issuer(&tender.issuers, REPAYMENT_ISSUER)
            .fiat_credit_outstanding
            .saturating_add(Gold(1))
    );
}

#[test]
fn issuer_repayment_refusal_resets_before_later_ladder_debts() {
    let refusal = run(ScenarioName::EmergedGoldIssuerRepaymentFiatRefusalControl);
    let proof = proof_row(&refusal.issuer_repayment_audit);
    let later = issuer_repayment_row(&refusal.issuer_repayment_audit, 15, 11, AgentId(214));

    assert_eq!(proof.tender, IssuerRepaymentTender::FiatRefused);
    assert_eq!(proof.state, DebtPaymentState::Defaulted);
    assert_eq!(later.tender, IssuerRepaymentTender::FiatOnly);
    assert_eq!(later.state, DebtPaymentState::Settled);
    assert_eq!(later.paid, Gold(1));
    assert_eq!(later.credit_retired, Gold(1));
}

#[test]
fn issuer_repayment_fiat_tender_retires_returned_fiat() {
    let refusal = run_through_tick(ScenarioName::EmergedGoldIssuerRepaymentFiatRefusalControl);
    let tender = run_through_tick(ScenarioName::EmergedGoldIssuerRepaymentFiatTender);
    let row = proof_row(&tender.issuer_repayment_audit);
    let tender_money = tender.money_system.as_ref().expect("M3 money system");
    let refusal_tick = record_at(&refusal.m3_records, REPAYMENT_TICK);
    let tender_tick = record_at(&tender.m3_records, REPAYMENT_TICK);
    let refusal_issuer = issuer(&refusal.issuers, REPAYMENT_ISSUER);
    let tender_issuer = issuer(&tender.issuers, REPAYMENT_ISSUER);

    assert_eq!(row.tender, IssuerRepaymentTender::FiatOnly);
    assert_eq!(row.state, DebtPaymentState::Settled);
    assert_eq!(row.owed, Gold(1));
    assert_eq!(row.paid, Gold(1));
    assert_eq!(row.remaining, Gold::ZERO);
    assert_eq!(row.public_fiat, Gold(1));
    assert_eq!(row.public_specie, Gold::ZERO);
    assert_eq!(row.credit_retired, Gold(1));
    assert_eq!(tender_money.public_fiat(REPAYMENT_BORROWER), Gold::ZERO);
    assert_eq!(
        tender_issuer
            .fiat_credit_outstanding
            .saturating_add(Gold(1)),
        refusal_issuer.fiat_credit_outstanding
    );
    assert_eq!(
        tender_issuer.fiat_retired,
        refusal_issuer.fiat_retired.saturating_add(Gold(1))
    );
    assert_eq!(
        tender_tick.public_fiat.saturating_add(Gold(1)),
        refusal_tick.public_fiat
    );
    assert_eq!(tender_tick.tms.saturating_add(Gold(1)), refusal_tick.tms);
    assert_eq!(tender_tick.public_specie, refusal_tick.public_specie);
    assert_eq!(
        tender_tick
            .public_specie
            .saturating_add(tender_tick.bank_reserves),
        refusal_tick
            .public_specie
            .saturating_add(refusal_tick.bank_reserves)
    );
}

#[test]
fn issuer_repayment_tender_is_independent_from_public_debt_and_bank_repayment_tenders() {
    let mut agents = vec![agent(1, Gold::ZERO), agent(2, Gold::ZERO)];
    let mut money = MoneySystem::from_agents(&agents);
    money.credit_fiat(AgentId(1), Gold(2)).unwrap();
    money
        .issue_demand_claim(CLAIM_BANK, AgentId(1), Gold(1), Gold::ZERO)
        .unwrap();
    money.reconcile_agent_cache(&mut agents);
    let mut banks = vec![bank_with_claim_debt(Gold(1), Gold(1))];
    let mut issuers = vec![issuer_with_credit(Gold(1))];
    let mut debts = vec![
        issuer_debt(DebtId(1), Gold(1)),
        DebtContract {
            id: DebtId(2),
            lender: CreditLender::Agent(AgentId(2)),
            borrower: AgentId(1),
            opened_tick: Tick(0),
            due_tick: Tick(1),
            principal: Gold(1),
            due: Gold(1),
            paid: Gold::ZERO,
            state: DebtState::Open,
            purpose: DebtPurpose::Consumption,
            funding: CreditSource::Commodity,
        },
        DebtContract {
            id: DebtId(3),
            lender: CreditLender::Bank(CLAIM_BANK),
            borrower: AgentId(1),
            opened_tick: Tick(0),
            due_tick: Tick(1),
            principal: Gold(1),
            due: Gold(1),
            paid: Gold::ZERO,
            state: DebtState::Open,
            purpose: DebtPurpose::Consumption,
            funding: CreditSource::BankFiduciary(CLAIM_BANK),
        },
    ];
    let mut debt_payment_audit = Vec::new();
    let mut bank_repayment_audit = Vec::new();
    let mut issuer_repayment_audit = Vec::new();

    settle_due_debts_m3(DebtSettlementM3Context {
        agents: &mut agents,
        debts: &mut debts,
        tick: Tick(1),
        money_system: &mut money,
        banks: &mut banks,
        issuers: &mut issuers,
        public_debt_tender: PublicDebtTender::SpecieOnly,
        bank_repayment_tender: BankRepaymentTender::BankClaimsAndSpecie,
        issuer_repayment_tender: IssuerRepaymentTender::FiatOnly,
        debt_payment_audit: &mut debt_payment_audit,
        bank_repayment_audit: &mut bank_repayment_audit,
        issuer_repayment_audit: &mut issuer_repayment_audit,
        tax_receivability: econ::money::TaxReceivability::SpecieOnly,
        tax_audit: &mut Vec::new(),
    });

    let issuer_row = only_issuer_repayment_row(&issuer_repayment_audit);
    assert_eq!(issuer_row.state, DebtPaymentState::Settled);
    assert_eq!(issuer_row.public_fiat, Gold(1));
    assert_eq!(issuer_row.credit_retired, Gold(1));

    let public_row = only_debt_row(&debt_payment_audit);
    assert_eq!(public_row.state, DebtPaymentState::Defaulted);
    assert_eq!(public_row.public_fiat, Gold::ZERO);
    assert_eq!(public_row.paid, Gold::ZERO);

    let bank_row = only_bank_repayment_row(&bank_repayment_audit);
    assert_eq!(bank_row.state, DebtPaymentState::Settled);
    assert_eq!(bank_row.demand_claims, Gold(1));
    assert_eq!(bank_row.credit_retired, Gold(1));
    assert_eq!(money.public_fiat(AgentId(1)), Gold(1));
}

#[test]
fn issuer_repayment_never_debits_specie_or_claims() {
    let (_, money, _, debts, audit) = settle_issuer_case(
        IssuerRepaymentTender::FiatOnly,
        Gold(1),
        Gold::ZERO,
        Gold::ZERO,
        Gold(1),
        Gold(1),
    );
    let row = only_issuer_repayment_row(&audit);
    assert_eq!(row.state, DebtPaymentState::Defaulted);
    assert_eq!(row.public_specie, Gold::ZERO);
    assert_eq!(money.snapshot().public_specie, Gold(1));
    assert_eq!(money.base.issuer_gold_vault, Gold::ZERO);
    assert_eq!(debts[0].state, DebtState::Defaulted);

    let (_, money, _, _, audit) = settle_issuer_case(
        IssuerRepaymentTender::FiatOnly,
        Gold(1),
        Gold::ZERO,
        Gold(1),
        Gold::ZERO,
        Gold(1),
    );
    let row = only_issuer_repayment_row(&audit);
    assert_eq!(row.state, DebtPaymentState::Defaulted);
    assert_eq!(row.public_fiat, Gold::ZERO);
    assert_eq!(money.demand_claim_on(AgentId(1), CLAIM_BANK), Gold(1));

    let (_, money, _, _, audit) = settle_issuer_case(
        IssuerRepaymentTender::FiatOnly,
        Gold(1),
        Gold(1),
        Gold::ZERO,
        Gold(1),
        Gold::ZERO,
    );
    let row = only_issuer_repayment_row(&audit);
    assert_eq!(row.state, DebtPaymentState::Settled);
    assert_eq!(row.public_fiat, Gold(1));
    assert_eq!(row.public_specie, Gold::ZERO);
    assert_eq!(money.public_fiat(AgentId(1)), Gold::ZERO);
    assert_eq!(
        money
            .balance_snapshot(AgentId(1))
            .expect("borrower balance")
            .public_specie,
        Gold(1)
    );

    let (_, money, _, _, audit) = settle_issuer_case(
        IssuerRepaymentTender::FiatOnly,
        Gold(1),
        Gold(1),
        Gold(1),
        Gold::ZERO,
        Gold::ZERO,
    );
    let row = only_issuer_repayment_row(&audit);
    assert_eq!(row.state, DebtPaymentState::Settled);
    assert_eq!(row.public_fiat, Gold(1));
    assert_eq!(money.public_fiat(AgentId(1)), Gold::ZERO);
    assert_eq!(money.demand_claim_on(AgentId(1), CLAIM_BANK), Gold(1));
}

#[test]
fn issuer_repayment_partial_fiat_retirement_is_audited() {
    let (summary, money, issuers, debts, audit) = settle_issuer_case(
        IssuerRepaymentTender::FiatOnly,
        Gold(2),
        Gold(1),
        Gold::ZERO,
        Gold::ZERO,
        Gold(1),
    );
    let row = only_issuer_repayment_row(&audit);

    assert_eq!(summary.defaulted, 1);
    assert_eq!(summary.paid, Gold(1));
    assert_eq!(summary.credit_retired, Gold(1));
    assert_eq!(row.paid, Gold(1));
    assert_eq!(row.remaining, Gold(1));
    assert_eq!(row.credit_retired, Gold(1));
    assert_eq!(row.state, DebtPaymentState::Defaulted);
    assert_eq!(money.public_fiat(AgentId(1)), Gold::ZERO);
    assert_eq!(money.snapshot().public_fiat, Gold(1));
    assert_eq!(issuers[0].fiat_credit_outstanding, Gold(1));
    assert_eq!(debts[0].state, DebtState::Defaulted);
}

#[test]
fn issuer_repayment_lender_mismatch_defaults_with_zero_paid_audit_row() {
    let mut agents = vec![agent(1, Gold::ZERO), agent(2, Gold::ZERO)];
    let mut money = MoneySystem::from_agents(&agents);
    money.credit_fiat(AgentId(1), Gold(1)).unwrap();
    money.reconcile_agent_cache(&mut agents);
    let mut banks: Vec<Bank> = Vec::new();
    let mut issuers = vec![issuer_with_credit(Gold(1))];
    let mut debts = vec![DebtContract {
        lender: CreditLender::Agent(AgentId(2)),
        ..issuer_debt(DebtId(1), Gold(1))
    }];
    let mut debt_payment_audit = Vec::new();
    let mut bank_repayment_audit = Vec::new();
    let mut issuer_repayment_audit = Vec::new();

    let summary = settle_due_debts_m3(DebtSettlementM3Context {
        agents: &mut agents,
        debts: &mut debts,
        tick: Tick(1),
        money_system: &mut money,
        banks: &mut banks,
        issuers: &mut issuers,
        public_debt_tender: PublicDebtTender::ParAll,
        bank_repayment_tender: BankRepaymentTender::ParAll,
        issuer_repayment_tender: IssuerRepaymentTender::FiatOnly,
        debt_payment_audit: &mut debt_payment_audit,
        bank_repayment_audit: &mut bank_repayment_audit,
        issuer_repayment_audit: &mut issuer_repayment_audit,
        tax_receivability: econ::money::TaxReceivability::SpecieOnly,
        tax_audit: &mut Vec::new(),
    });

    assert_eq!(summary.defaulted, 1);
    assert_eq!(summary.paid, Gold::ZERO);
    assert_eq!(summary.unpaid, Gold(1));
    assert_eq!(debts[0].state, DebtState::Defaulted);
    let row = only_issuer_repayment_row(&issuer_repayment_audit);
    assert_eq!(row.state, DebtPaymentState::Defaulted);
    assert_eq!(row.owed, Gold(1));
    assert_eq!(row.paid, Gold::ZERO);
    assert_eq!(row.remaining, Gold(1));
    assert_eq!(row.public_fiat, Gold::ZERO);
    assert_eq!(row.public_specie, Gold::ZERO);
    assert_eq!(row.credit_retired, Gold::ZERO);
    assert_eq!(money.public_fiat(AgentId(1)), Gold(1));
    assert_eq!(issuers[0].fiat_credit_outstanding, Gold(1));
    assert_eq!(issuers[0].fiat_retired, Gold::ZERO);
}

#[test]
fn m16_shadow_preserves_policy_but_has_no_issuer_debt_to_repay() {
    let scenario =
        builtin_market_scenario(ScenarioName::EmergedGoldIssuerRepaymentFiatRefusalControl);
    let shadow = credit_disabled_scenario(&scenario);

    assert!(shadow.events.iter().any(|event| matches!(
        event.kind,
        EventKind::SetIssuerRepaymentTender(IssuerRepaymentTender::FiatRefused)
    )));
    assert!(shadow.events.iter().any(|event| matches!(
        event.kind,
        EventKind::SetIssuerRepaymentTender(IssuerRepaymentTender::FiatOnly)
    )));
    assert!(shadow.events.iter().any(|event| matches!(
        event.kind,
        EventKind::SetDebtDueTick {
            debt: DebtId(REPAYMENT_DEBT),
            due_tick: Tick(REPAYMENT_TICK),
        }
    )));

    let periods = shadow.periods;
    let mut society = Society::from_scenario(shadow);
    society.run(periods);

    assert_eq!(
        society.issuer_repayment_tender,
        IssuerRepaymentTender::FiatOnly
    );
    assert!(society
        .debts
        .iter()
        .all(|debt| debt.id != DebtId(REPAYMENT_DEBT)
            || debt.funding != CreditSource::FiatCredit(REPAYMENT_ISSUER)));
    assert!(society
        .issuer_repayment_audit
        .iter()
        .all(|row| !(row.tick == REPAYMENT_TICK && row.debt == REPAYMENT_DEBT)));
    assert_eq!(
        society
            .issuer_repayment_audit
            .iter()
            .fold(Gold::ZERO, |total, row| total.saturating_add(row.paid)),
        Gold::ZERO
    );
}

fn m16_scenarios() -> [ScenarioName; 2] {
    [
        ScenarioName::EmergedGoldIssuerRepaymentFiatRefusalControl,
        ScenarioName::EmergedGoldIssuerRepaymentFiatTender,
    ]
}

fn run(name: ScenarioName) -> Society {
    let scenario = builtin_market_scenario(name);
    let periods = scenario.periods;
    let mut society = Society::from_scenario(scenario);
    society.run(periods);
    society
}

fn run_with_audits(name: ScenarioName) -> Society {
    let scenario = builtin_market_scenario(name);
    let periods = scenario.periods;
    let mut society = Society::from_scenario(scenario);
    for tick in 0..periods {
        society.step();
        assert!(
            society.money_ledgers_reconcile(),
            "{name:?} ledgers failed to reconcile at tick {tick}"
        );
    }
    society
}

fn run_through_tick(name: ScenarioName) -> Society {
    let mut society = Society::from_scenario(builtin_market_scenario(name));
    for _ in 0..=REPAYMENT_TICK {
        society.step();
    }
    society
}

fn settle_issuer_case(
    tender: IssuerRepaymentTender,
    due: Gold,
    borrower_fiat: Gold,
    borrower_claim: Gold,
    borrower_specie: Gold,
    overhang_fiat: Gold,
) -> (
    econ::timemarket::DebtSettlementSummary,
    MoneySystem,
    Vec<Issuer>,
    Vec<DebtContract>,
    Vec<IssuerRepaymentAuditRecord>,
) {
    let mut agents = vec![agent(1, borrower_specie), agent(2, Gold::ZERO)];
    let mut money = MoneySystem::from_agents(&agents);
    if borrower_fiat > Gold::ZERO {
        money.credit_fiat(AgentId(1), borrower_fiat).unwrap();
    }
    if overhang_fiat > Gold::ZERO {
        money.credit_fiat(AgentId(2), overhang_fiat).unwrap();
    }
    if borrower_claim > Gold::ZERO {
        money
            .issue_demand_claim(CLAIM_BANK, AgentId(1), borrower_claim, Gold::ZERO)
            .unwrap();
    }
    money.reconcile_agent_cache(&mut agents);
    let mut banks: Vec<Bank> = Vec::new();
    let mut issuers = vec![issuer_with_credit(due)];
    let mut debts = vec![issuer_debt(DebtId(1), due)];
    let mut debt_payment_audit = Vec::new();
    let mut bank_repayment_audit = Vec::new();
    let mut issuer_repayment_audit = Vec::new();

    let summary = settle_due_debts_m3(DebtSettlementM3Context {
        agents: &mut agents,
        debts: &mut debts,
        tick: Tick(1),
        money_system: &mut money,
        banks: &mut banks,
        issuers: &mut issuers,
        public_debt_tender: PublicDebtTender::ParAll,
        bank_repayment_tender: BankRepaymentTender::ParAll,
        issuer_repayment_tender: tender,
        debt_payment_audit: &mut debt_payment_audit,
        bank_repayment_audit: &mut bank_repayment_audit,
        issuer_repayment_audit: &mut issuer_repayment_audit,
        tax_receivability: econ::money::TaxReceivability::SpecieOnly,
        tax_audit: &mut Vec::new(),
    });

    (summary, money, issuers, debts, issuer_repayment_audit)
}

fn issuer_debt(id: DebtId, due: Gold) -> DebtContract {
    DebtContract {
        id,
        lender: CreditLender::Issuer(REPAYMENT_ISSUER),
        borrower: AgentId(1),
        opened_tick: Tick(0),
        due_tick: Tick(1),
        principal: due,
        due,
        paid: Gold::ZERO,
        state: DebtState::Open,
        purpose: DebtPurpose::Consumption,
        funding: CreditSource::FiatCredit(REPAYMENT_ISSUER),
    }
}

fn issuer_with_credit(amount: Gold) -> Issuer {
    let mut issuer = Issuer {
        id: REPAYMENT_ISSUER,
        fiat_issued: Gold::ZERO,
        fiat_retired: Gold::ZERO,
        fiat_credit_outstanding: Gold::ZERO,
        policy: IssuerPolicy {
            fiscal_enabled: false,
            credit_enabled: true,
            max_fiscal_issue_per_tick: Gold::ZERO,
            max_credit_issue_per_tick: Gold(10),
            loan_present: Gold(1),
            loan_horizon: 4,
            loan_future_due: Gold(1),
        },
        taxes_levied: Gold::ZERO,
        tax_receipts_fiat: Gold::ZERO,
        tax_receipts_specie: Gold::ZERO,
        taxes_defaulted: Gold::ZERO,
    };
    issuer.record_credit_loan(Regime::Fiat, amount).unwrap();
    issuer
}

fn bank_with_claim_debt(loans_outstanding: Gold, demand_deposits: Gold) -> Bank {
    Bank {
        id: CLAIM_BANK,
        name: "test bank",
        reserves: Gold::ZERO,
        demand_deposits,
        time_deposits: Gold::ZERO,
        loans_outstanding,
        fiduciary_issued: demand_deposits,
        reserve_ratio_bps: ReserveRatioBps(0),
        convertible: true,
        policy: econ::bank::BankPolicy::default(),
    }
}

fn proof_row(records: &[IssuerRepaymentAuditRecord]) -> &IssuerRepaymentAuditRecord {
    issuer_repayment_row(records, REPAYMENT_TICK, REPAYMENT_DEBT, REPAYMENT_BORROWER)
}

fn issuer_repayment_row(
    records: &[IssuerRepaymentAuditRecord],
    tick: u64,
    debt: u64,
    borrower: AgentId,
) -> &IssuerRepaymentAuditRecord {
    records
        .iter()
        .find(|row| {
            row.tick == tick
                && row.debt == debt
                && row.borrower == borrower
                && row.issuer == REPAYMENT_ISSUER
        })
        .expect("issuer repayment row")
}

fn only_issuer_repayment_row(
    records: &[IssuerRepaymentAuditRecord],
) -> &IssuerRepaymentAuditRecord {
    assert_eq!(records.len(), 1);
    &records[0]
}

fn only_bank_repayment_row(records: &[BankRepaymentAuditRecord]) -> &BankRepaymentAuditRecord {
    assert_eq!(records.len(), 1);
    &records[0]
}

fn only_debt_row(records: &[DebtPaymentAuditRecord]) -> &DebtPaymentAuditRecord {
    assert_eq!(records.len(), 1);
    &records[0]
}

fn debt_by_id(debts: &[DebtContract], debt_id: DebtId) -> &DebtContract {
    debts
        .iter()
        .find(|debt| debt.id == debt_id)
        .expect("debt exists")
}

fn issuer(issuers: &[Issuer], issuer_id: IssuerId) -> &Issuer {
    issuers
        .iter()
        .find(|issuer| issuer.id == issuer_id)
        .expect("issuer exists")
}

fn record_at(records: &[M3Record], tick: u64) -> &M3Record {
    records
        .iter()
        .find(|record| record.m2.tick == tick)
        .expect("M3 record for tick")
}

fn m3_prefix(records: &[M3Record], before_tick: u64) -> Vec<M3Record> {
    records
        .iter()
        .filter(|record| record.m2.tick < before_tick)
        .cloned()
        .collect()
}

fn issuer_repayment_prefix(
    records: &[IssuerRepaymentAuditRecord],
    before_tick: u64,
) -> Vec<IssuerRepaymentAuditRecord> {
    records
        .iter()
        .filter(|record| record.tick < before_tick)
        .cloned()
        .collect()
}

fn loan_prefix(records: &[LoanTrade], before_tick: u64) -> Vec<LoanTrade> {
    records
        .iter()
        .filter(|record| record.tick < before_tick)
        .cloned()
        .collect()
}

fn event_signature(events: &[econ::scenario::Event]) -> Vec<(Tick, EventKind)> {
    events
        .iter()
        .map(|event| (event.tick, event.kind.clone()))
        .collect()
}

fn seed_rows(agents: &[Agent]) -> Vec<SeedRow> {
    let mut rows = agents
        .iter()
        .map(|agent| SeedRow {
            agent: agent.id,
            gold: agent.gold,
            stock: positive_stock(&agent.stock),
            scale: scale_signature(&agent.scale),
        })
        .collect::<Vec<_>>();
    rows.sort_by_key(|row| row.agent);
    rows
}

fn scale_signature(scale: &[Want]) -> Vec<(WantKind, Horizon, u32)> {
    scale
        .iter()
        .map(|want| (want.kind, want.horizon, want.qty))
        .collect()
}

fn positive_stock(stock: &Stock) -> Vec<(GoodId, u32)> {
    stock
        .positive_goods()
        .map(|good| (good, stock.get(good)))
        .collect()
}

fn bridge_gold(agents: &[Agent]) -> Gold {
    agents
        .iter()
        .filter(|agent| agent.id.0 <= 10)
        .fold(Gold::ZERO, |total, agent| total.saturating_add(agent.gold))
}

fn agent(id: u32, gold: Gold) -> Agent {
    Agent {
        id: AgentId(id),
        scale: vec![Want {
            kind: WantKind::Good(GOLD),
            horizon: Horizon::Later(4),
            qty: 1,
            satisfied: false,
        }],
        stock: Stock::new(3),
        gold,
        labor_capacity: 0,
        hunger_deficit: 0,
        roles: vec![Role::Trader],
        expect: Vec::new(),
    }
}
