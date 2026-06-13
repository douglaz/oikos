use econ::agent::{Agent, AgentId, Role, Want, WantKind};
use econ::bank::Bank;
use econ::good::{Gold, GoodId, Horizon, Stock, GOLD};
use econ::issuer::Issuer;
use econ::ledger::{BankId, MoneySystem};
use econ::money::{BankRepaymentTender, IssuerRepaymentTender, PublicDebtTender};
use econ::project::Tick;
use econ::purpose::{CreditLender, CreditSource, DebtPurpose};
use econ::record::{
    BankAuditRecord, BankRepaymentAuditRecord, DebtPaymentAuditRecord, DebtPaymentState, M3Record,
    RedemptionAuditRecord, RedemptionOutcome,
};
use econ::scenario::{
    builtin_market_scenario, emerged_gold_bridge_agents, EventKind, ScenarioName,
};
use econ::shadow::credit_disabled_scenario;
use econ::society::Society;
use econ::timemarket::{
    settle_due_debts_m3, DebtContract, DebtId, DebtSettlementM3Context, DebtState,
};

const BRIDGE_GOLD: Gold = Gold(16);
const REPAYMENT_TICK: u64 = 4;
const REPAYMENT_DEBT: u64 = 4;
const REPAYMENT_BANK: BankId = BankId(1);
const REPAYMENT_BORROWER: AgentId = AgentId(124);

#[derive(Clone, Debug, PartialEq, Eq)]
struct SeedRow {
    agent: AgentId,
    gold: Gold,
    stock: Vec<(GoodId, u32)>,
    scale: Vec<(WantKind, Horizon, u32)>,
}

#[test]
fn m15_scenarios_preserve_m6_bridge_prefix() {
    let bridge = emerged_gold_bridge_agents();
    let expected = seed_rows(&bridge);
    let base = builtin_market_scenario(ScenarioName::EmergedGoldSuspendedRedemption);

    assert_eq!(bridge_gold(&bridge), BRIDGE_GOLD);
    assert!(bridge.iter().all(|agent| agent.stock.get(GOLD) == 0));

    for name in m15_scenarios() {
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
fn m15_scenarios_match_suspended_redemption_prefix_until_tick_4() {
    let base = run_with_audits(ScenarioName::EmergedGoldSuspendedRedemption);

    for name in m15_scenarios() {
        let society = run_with_audits(name);

        assert_eq!(
            m3_prefix(&base.m3_records, REPAYMENT_TICK),
            m3_prefix(&society.m3_records, REPAYMENT_TICK),
            "{name:?} M3 prefix"
        );
        assert_eq!(
            bank_prefix(&base.bank_audit, REPAYMENT_TICK),
            bank_prefix(&society.bank_audit, REPAYMENT_TICK),
            "{name:?} bank prefix"
        );
        assert_eq!(
            bank_repayment_prefix(&base.bank_repayment_audit, REPAYMENT_TICK),
            bank_repayment_prefix(&society.bank_repayment_audit, REPAYMENT_TICK),
            "{name:?} bank repayment prefix"
        );
        assert!(society.m3_records.iter().all(|record| {
            record.public_fiat == Gold::ZERO
                && record.fiat_credit_issued == Gold::ZERO
                && record.fiat_fiscal_issued == Gold::ZERO
        }));
    }
}

#[test]
fn m15_accelerates_existing_bank_debt_only() {
    let base = run(ScenarioName::EmergedGoldSuspendedRedemption);
    let debt = debt_by_id(&base.debts, DebtId(REPAYMENT_DEBT));

    assert_eq!(debt.lender, CreditLender::Bank(REPAYMENT_BANK));
    assert_eq!(debt.borrower, REPAYMENT_BORROWER);
    assert_eq!(debt.principal, Gold(1));
    assert_eq!(debt.due, Gold(1));
    assert_eq!(debt.funding, CreditSource::BankFiduciary(REPAYMENT_BANK));

    for name in m15_scenarios() {
        let scenario = builtin_market_scenario(name);
        assert!(scenario.events.iter().any(|event| matches!(
            event.kind,
            EventKind::SetDebtDueTick {
                debt: DebtId(REPAYMENT_DEBT),
                due_tick: Tick(REPAYMENT_TICK),
            }
        )));
        assert!(scenario
            .events
            .iter()
            .all(|event| !matches!(event.kind, EventKind::SeedCommodityDebt { .. })));
        assert!(scenario
            .events
            .iter()
            .all(|event| !matches!(event.kind, EventKind::SeedStock { .. })));

        let settled = run(name);
        let changed = debt_by_id(&settled.debts, DebtId(REPAYMENT_DEBT));
        assert_eq!(changed.lender, debt.lender);
        assert_eq!(changed.borrower, debt.borrower);
        assert_eq!(changed.principal, debt.principal);
        assert_eq!(changed.due, debt.due);
        assert_eq!(changed.purpose, debt.purpose);
        assert_eq!(changed.funding, debt.funding);
        assert_eq!(changed.due_tick, Tick(REPAYMENT_TICK));
    }
}

#[test]
fn redemption_is_suspended_before_bank_repayment() {
    for name in m15_scenarios() {
        let society = run_with_audits(name);
        let final_bank = bank_at(&society.bank_audit, REPAYMENT_TICK);
        let proof = proof_row(&society.bank_repayment_audit);

        assert_eq!(sum_requested(&society.redemption_audit), Gold(4));
        assert_eq!(sum_honored(&society.redemption_audit), Gold::ZERO);
        assert_eq!(sum_failed(&society.redemption_audit), Gold(4));
        assert!(society
            .redemption_audit
            .iter()
            .all(|row| row.outcome == RedemptionOutcome::Suspended));
        assert!(society.redemption_audit.iter().any(|row| {
            row.tick == REPAYMENT_TICK
                && row.agent == REPAYMENT_BORROWER
                && row.requested == Gold(1)
                && row.honored == Gold::ZERO
                && row.failed == Gold(1)
                && row.outcome == RedemptionOutcome::Suspended
        }));
        assert_eq!(proof.tick, REPAYMENT_TICK);
        assert!(!final_bank.convertible);
    }
}

#[test]
fn bank_repayment_refusal_defaults_without_spending_claim() {
    let society = run(ScenarioName::EmergedGoldBankLoanRepaymentRefusalControl);
    let row = proof_row(&society.bank_repayment_audit);

    assert_eq!(row.tender, BankRepaymentTender::SpecieOnly);
    assert_eq!(row.state, DebtPaymentState::Defaulted);
    assert_eq!(row.owed, Gold(1));
    assert_eq!(row.paid, Gold::ZERO);
    assert_eq!(row.remaining, Gold(1));
    assert_eq!(row.public_fiat, Gold::ZERO);
    assert_eq!(row.demand_claims, Gold::ZERO);
    assert_eq!(row.public_specie, Gold::ZERO);
    assert_eq!(row.credit_retired, Gold::ZERO);
}

#[test]
fn bank_repayment_refusal_retains_claim_without_same_tick_public_redeployment() {
    let refusal = run_with_audits(ScenarioName::EmergedGoldBankLoanRepaymentRefusalControl);
    let claim = run_with_audits(ScenarioName::EmergedGoldBankLoanRepaymentClaimTender);

    assert_eq!(
        refusal
            .money_system
            .as_ref()
            .expect("M3 money")
            .demand_claim_on(REPAYMENT_BORROWER, REPAYMENT_BANK),
        Gold(1)
    );
    assert!(!refusal
        .labor_trades
        .iter()
        .any(|trade| trade.tick == REPAYMENT_TICK && trade.employer == REPAYMENT_BORROWER));
    assert!(!refusal.loan_trades.iter().any(|trade| {
        trade.tick == REPAYMENT_TICK
            && (trade.lender == CreditLender::Agent(REPAYMENT_BORROWER)
                || trade.borrower == REPAYMENT_BORROWER)
    }));
    assert_eq!(tick_labor_signature(&refusal), tick_labor_signature(&claim));
    assert_eq!(
        record_at(&refusal.m3_records, REPAYMENT_TICK).boom_projects_started,
        record_at(&claim.m3_records, REPAYMENT_TICK).boom_projects_started
    );
    assert_eq!(
        record_at(&refusal.m3_records, REPAYMENT_TICK)
            .m2
            .loan_trades,
        record_at(&claim.m3_records, REPAYMENT_TICK).m2.loan_trades
    );
}

#[test]
fn bank_repayment_claim_tender_retires_unredeemable_claim() {
    let society = run(ScenarioName::EmergedGoldBankLoanRepaymentClaimTender);
    let row = proof_row(&society.bank_repayment_audit);

    assert_eq!(row.tender, BankRepaymentTender::BankClaimsAndSpecie);
    assert_eq!(row.state, DebtPaymentState::Settled);
    assert_eq!(row.owed, Gold(1));
    assert_eq!(row.paid, Gold(1));
    assert_eq!(row.remaining, Gold::ZERO);
    assert_eq!(row.public_fiat, Gold::ZERO);
    assert_eq!(row.demand_claims, Gold(1));
    assert_eq!(row.public_specie, Gold::ZERO);
    assert_eq!(row.credit_retired, Gold(1));
    assert_eq!(
        society
            .money_system
            .as_ref()
            .expect("M3 money")
            .demand_claim_on(REPAYMENT_BORROWER, REPAYMENT_BANK),
        Gold::ZERO
    );
}

#[test]
fn bank_repayment_claim_tender_contracts_credit_without_reserve_move() {
    let refusal = run_with_audits(ScenarioName::EmergedGoldBankLoanRepaymentRefusalControl);
    let claim = run_with_audits(ScenarioName::EmergedGoldBankLoanRepaymentClaimTender);
    let refusal_tick = record_at(&refusal.m3_records, REPAYMENT_TICK);
    let claim_tick = record_at(&claim.m3_records, REPAYMENT_TICK);
    let refusal_bank = bank_at(&refusal.bank_audit, REPAYMENT_TICK);
    let claim_bank = bank_at(&claim.bank_audit, REPAYMENT_TICK);

    assert_eq!(
        claim_tick.demand_claims.saturating_add(Gold(1)),
        refusal_tick.demand_claims
    );
    assert_eq!(
        claim_tick.fiduciary.saturating_add(Gold(1)),
        refusal_tick.fiduciary
    );
    assert_eq!(claim_tick.tms.saturating_add(Gold(1)), refusal_tick.tms);
    assert_eq!(
        claim_bank.loans_outstanding.saturating_add(Gold(1)),
        refusal_bank.loans_outstanding
    );
    assert_eq!(
        claim_bank.demand_deposits.saturating_add(Gold(1)),
        refusal_bank.demand_deposits
    );
    assert_eq!(
        claim_bank.fiduciary_issued.saturating_add(Gold(1)),
        refusal_bank.fiduciary_issued
    );
    assert_eq!(claim_bank.reserves, refusal_bank.reserves);
    assert_eq!(claim_tick.public_fiat, Gold::ZERO);
    assert_eq!(refusal_tick.public_fiat, Gold::ZERO);

    let (_, money, _, _, audit) = settle_bank_case(
        BankRepaymentTender::BankClaimsAndSpecie,
        Gold(1),
        Gold::ZERO,
        Gold(1),
        Gold(2),
    );
    let row = only_bank_repayment_row(&audit);
    assert_eq!(row.demand_claims, Gold(1));
    assert_eq!(row.public_specie, Gold::ZERO);
    assert_eq!(money.snapshot().public_specie, Gold(2));
}

#[test]
fn bank_repayment_tender_is_independent_from_public_debt_tender() {
    let mut agents = vec![agent(1, Gold::ZERO), agent(2, Gold::ZERO)];
    let mut money = MoneySystem::from_agents(&agents);
    money
        .issue_demand_claim(REPAYMENT_BANK, AgentId(1), Gold(1), Gold::ZERO)
        .unwrap();
    money.reconcile_agent_cache(&mut agents);
    let mut banks = vec![bank_with_claim_debt(Gold(1), Gold(1))];
    let mut issuers: Vec<Issuer> = Vec::new();
    let mut debts = vec![
        DebtContract {
            id: DebtId(1),
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
            id: DebtId(2),
            lender: CreditLender::Bank(REPAYMENT_BANK),
            borrower: AgentId(1),
            opened_tick: Tick(0),
            due_tick: Tick(1),
            principal: Gold(1),
            due: Gold(1),
            paid: Gold::ZERO,
            state: DebtState::Open,
            purpose: DebtPurpose::Consumption,
            funding: CreditSource::BankFiduciary(REPAYMENT_BANK),
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

    let public_row = only_debt_row(&debt_payment_audit);
    assert_eq!(public_row.state, DebtPaymentState::Defaulted);
    assert_eq!(public_row.demand_claims, Gold::ZERO);
    assert_eq!(public_row.paid, Gold::ZERO);

    let bank_row = only_bank_repayment_row(&bank_repayment_audit);
    assert_eq!(bank_row.state, DebtPaymentState::Settled);
    assert_eq!(bank_row.demand_claims, Gold(1));
    assert_eq!(bank_row.credit_retired, Gold(1));
}

#[test]
fn bank_repayment_tender_filters_fiat_and_claims_correctly() {
    let (_, _, _, _, specie_only) = settle_bank_case(
        BankRepaymentTender::SpecieOnly,
        Gold(2),
        Gold(1),
        Gold(1),
        Gold(1),
    );
    let row = only_bank_repayment_row(&specie_only);
    assert_eq!(row.public_fiat, Gold::ZERO);
    assert_eq!(row.demand_claims, Gold::ZERO);
    assert_eq!(row.public_specie, Gold(1));
    assert_eq!(row.state, DebtPaymentState::Defaulted);

    let (_, money, _, _, fiat_and_specie) = settle_bank_case(
        BankRepaymentTender::FiatAndSpecie,
        Gold(2),
        Gold(1),
        Gold(1),
        Gold(1),
    );
    let row = only_bank_repayment_row(&fiat_and_specie);
    assert_eq!(row.public_fiat, Gold(1));
    assert_eq!(row.demand_claims, Gold::ZERO);
    assert_eq!(row.public_specie, Gold(1));
    assert_eq!(money.demand_claim_on(AgentId(1), REPAYMENT_BANK), Gold(1));

    let (_, money, _, _, claims_and_specie) = settle_bank_case(
        BankRepaymentTender::BankClaimsAndSpecie,
        Gold(2),
        Gold(1),
        Gold(1),
        Gold(1),
    );
    let row = only_bank_repayment_row(&claims_and_specie);
    assert_eq!(row.public_fiat, Gold::ZERO);
    assert_eq!(row.demand_claims, Gold(1));
    assert_eq!(row.public_specie, Gold(1));
    assert_eq!(money.public_fiat(AgentId(1)), Gold(1));

    let (_, _, _, _, par_all) = settle_bank_case(
        BankRepaymentTender::ParAll,
        Gold(3),
        Gold(1),
        Gold(1),
        Gold(1),
    );
    let row = only_bank_repayment_row(&par_all);
    assert_eq!(row.public_fiat, Gold(1));
    assert_eq!(row.demand_claims, Gold(1));
    assert_eq!(row.public_specie, Gold(1));
}

#[test]
fn m15_shadow_preserves_policy_but_has_no_bank_debt_to_repay() {
    let scenario = builtin_market_scenario(ScenarioName::EmergedGoldBankLoanRepaymentClaimTender);
    let shadow = credit_disabled_scenario(&scenario);

    assert!(shadow.events.iter().any(|event| matches!(
        event.kind,
        EventKind::SetBankRepaymentTender(BankRepaymentTender::BankClaimsAndSpecie)
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
        society.bank_repayment_tender,
        BankRepaymentTender::BankClaimsAndSpecie
    );
    assert!(society
        .debts
        .iter()
        .all(|debt| debt.id != DebtId(REPAYMENT_DEBT)
            || debt.funding != CreditSource::BankFiduciary(REPAYMENT_BANK)));
    assert!(society
        .bank_repayment_audit
        .iter()
        .all(|row| !(row.tick == REPAYMENT_TICK && row.debt == REPAYMENT_DEBT)));
    assert_eq!(
        society
            .bank_repayment_audit
            .iter()
            .fold(Gold::ZERO, |total, row| total
                .saturating_add(row.demand_claims)),
        Gold::ZERO
    );
}

fn m15_scenarios() -> [ScenarioName; 2] {
    [
        ScenarioName::EmergedGoldBankLoanRepaymentRefusalControl,
        ScenarioName::EmergedGoldBankLoanRepaymentClaimTender,
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
    society.enable_bank_audit();
    for tick in 0..periods {
        society.step();
        assert!(
            society.money_ledgers_reconcile(),
            "{name:?} ledgers failed to reconcile at tick {tick}"
        );
    }
    society
}

fn settle_bank_case(
    tender: BankRepaymentTender,
    due: Gold,
    fiat: Gold,
    claim: Gold,
    specie: Gold,
) -> (
    econ::timemarket::DebtSettlementSummary,
    MoneySystem,
    Vec<Bank>,
    Vec<DebtContract>,
    Vec<BankRepaymentAuditRecord>,
) {
    let mut agents = vec![agent(1, specie)];
    let mut money = MoneySystem::from_agents(&agents);
    if fiat > Gold::ZERO {
        money.credit_fiat(AgentId(1), fiat).unwrap();
    }
    if claim > Gold::ZERO {
        money
            .issue_demand_claim(REPAYMENT_BANK, AgentId(1), claim, Gold::ZERO)
            .unwrap();
    }
    money.reconcile_agent_cache(&mut agents);
    let mut banks = vec![bank_with_claim_debt(due, claim)];
    let mut issuers: Vec<Issuer> = Vec::new();
    let mut debts = vec![DebtContract {
        id: DebtId(1),
        lender: CreditLender::Bank(REPAYMENT_BANK),
        borrower: AgentId(1),
        opened_tick: Tick(0),
        due_tick: Tick(1),
        principal: due,
        due,
        paid: Gold::ZERO,
        state: DebtState::Open,
        purpose: DebtPurpose::Consumption,
        funding: CreditSource::BankFiduciary(REPAYMENT_BANK),
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
        bank_repayment_tender: tender,
        issuer_repayment_tender: IssuerRepaymentTender::FiatOnly,
        debt_payment_audit: &mut debt_payment_audit,
        bank_repayment_audit: &mut bank_repayment_audit,
        issuer_repayment_audit: &mut issuer_repayment_audit,
        tax_receivability: econ::money::TaxReceivability::SpecieOnly,
        tax_audit: &mut Vec::new(),
    });

    (summary, money, banks, debts, bank_repayment_audit)
}

fn bank_with_claim_debt(loans_outstanding: Gold, demand_deposits: Gold) -> Bank {
    Bank {
        id: REPAYMENT_BANK,
        name: "test bank",
        reserves: Gold::ZERO,
        demand_deposits,
        time_deposits: Gold::ZERO,
        loans_outstanding,
        fiduciary_issued: demand_deposits,
        reserve_ratio_bps: econ::money::ReserveRatioBps(0),
        convertible: true,
        policy: econ::bank::BankPolicy::default(),
    }
}

fn proof_row(records: &[BankRepaymentAuditRecord]) -> &BankRepaymentAuditRecord {
    records
        .iter()
        .find(|row| {
            row.tick == REPAYMENT_TICK
                && row.debt == REPAYMENT_DEBT
                && row.borrower == REPAYMENT_BORROWER
                && row.bank == REPAYMENT_BANK
        })
        .expect("M15 proof row")
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

fn m3_prefix(records: &[M3Record], before_tick: u64) -> Vec<M3Record> {
    records
        .iter()
        .filter(|record| record.m2.tick < before_tick)
        .cloned()
        .collect()
}

fn bank_prefix(records: &[BankAuditRecord], before_tick: u64) -> Vec<BankAuditRecord> {
    records
        .iter()
        .filter(|record| record.tick < before_tick)
        .cloned()
        .collect()
}

fn bank_repayment_prefix(
    records: &[BankRepaymentAuditRecord],
    before_tick: u64,
) -> Vec<BankRepaymentAuditRecord> {
    records
        .iter()
        .filter(|record| record.tick < before_tick)
        .cloned()
        .collect()
}

fn record_at(records: &[M3Record], tick: u64) -> &M3Record {
    records
        .iter()
        .find(|record| record.m2.tick == tick)
        .expect("M3 record for tick")
}

fn bank_at(records: &[BankAuditRecord], tick: u64) -> &BankAuditRecord {
    records
        .iter()
        .find(|record| record.tick == tick && record.bank == REPAYMENT_BANK)
        .expect("bank record for tick")
}

fn tick_labor_signature(society: &Society) -> Vec<(AgentId, AgentId, Gold, u32)> {
    society
        .labor_trades
        .iter()
        .filter(|trade| trade.tick == REPAYMENT_TICK)
        .map(|trade| (trade.employer, trade.worker, trade.wage, trade.qty))
        .collect()
}

fn sum_requested(records: &[RedemptionAuditRecord]) -> Gold {
    records.iter().fold(Gold::ZERO, |sum, record| {
        sum.saturating_add(record.requested)
    })
}

fn sum_honored(records: &[RedemptionAuditRecord]) -> Gold {
    records
        .iter()
        .fold(Gold::ZERO, |sum, record| sum.saturating_add(record.honored))
}

fn sum_failed(records: &[RedemptionAuditRecord]) -> Gold {
    records
        .iter()
        .fold(Gold::ZERO, |sum, record| sum.saturating_add(record.failed))
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
        id: AgentId(u64::from(id)),
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
