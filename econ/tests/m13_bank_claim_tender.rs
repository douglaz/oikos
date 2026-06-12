use econ::agent::{Agent, AgentId, Role, Want, WantKind};
use econ::bank::Bank;
use econ::good::{Gold, GoodId, Horizon, Stock, GOLD};
use econ::issuer::Issuer;
use econ::ledger::{BankId, MoneySystem};
use econ::money::{BankRepaymentTender, IssuerRepaymentTender, PublicDebtTender, PublicSpotTender};
use econ::project::Tick;
use econ::purpose::{CreditLender, CreditSource, DebtPurpose};
use econ::record::{
    BankAuditRecord, DebtPaymentAuditRecord, DebtPaymentState, M3Record, RedemptionAuditRecord,
    RedemptionOutcome,
};
use econ::scenario::{
    builtin_market_scenario, emerged_gold_bridge_agents, Event, EventKind, ScenarioName,
};
use econ::shadow::credit_disabled_scenario;
use econ::society::Society;
use econ::timemarket::{
    settle_due_debts_m3, DebtContract, DebtId, DebtSettlementM3Context, DebtState,
};

const BRIDGE_GOLD: Gold = Gold(16);
const CLAIM_DEBT_TICK: u64 = 4;
const CLAIM_BANK: BankId = BankId(1);
const CLAIM_BORROWER: AgentId = AgentId(121);
const CLAIM_LENDER: AgentId = AgentId(1);

#[derive(Clone, Debug, PartialEq, Eq)]
struct SeedRow {
    agent: AgentId,
    gold: Gold,
    stock: Vec<(GoodId, u32)>,
    scale: Vec<(WantKind, Horizon, u32)>,
}

#[test]
fn bank_claims_and_specie_debt_tender_uses_claims_before_specie_and_never_fiat() {
    let mut agents = vec![agent(1, Gold(2)), agent(2, Gold::ZERO)];
    let mut money = MoneySystem::from_agents(&agents);
    money.credit_fiat(AgentId(1), Gold(5)).unwrap();
    money
        .issue_demand_claim(CLAIM_BANK, AgentId(1), Gold(1), Gold::ZERO)
        .unwrap();
    money.reconcile_agent_cache(&mut agents);
    let mut debts = vec![DebtContract {
        id: DebtId(1),
        lender: CreditLender::Agent(AgentId(2)),
        borrower: AgentId(1),
        opened_tick: Tick(0),
        due_tick: Tick(1),
        principal: Gold(2),
        due: Gold(2),
        paid: Gold::ZERO,
        state: DebtState::Open,
        purpose: DebtPurpose::Consumption,
        funding: CreditSource::Commodity,
    }];
    let mut banks: Vec<Bank> = Vec::new();
    let mut issuers: Vec<Issuer> = Vec::new();
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
        public_debt_tender: PublicDebtTender::BankClaimsAndSpecie,
        bank_repayment_tender: BankRepaymentTender::ParAll,
        issuer_repayment_tender: IssuerRepaymentTender::FiatOnly,
        debt_payment_audit: &mut debt_payment_audit,
        bank_repayment_audit: &mut bank_repayment_audit,
        issuer_repayment_audit: &mut issuer_repayment_audit,
        tax_receivability: econ::money::TaxReceivability::SpecieOnly,
        tax_audit: &mut Vec::new(),
    });

    assert_eq!(summary.settled, 1);
    assert_eq!(debts[0].state, DebtState::Settled);
    let row = only_debt_row(&debt_payment_audit);
    assert_eq!(row.public_fiat, Gold::ZERO);
    assert_eq!(row.demand_claims, Gold(1));
    assert_eq!(row.public_specie, Gold(1));
    assert_eq!(row.tender, PublicDebtTender::BankClaimsAndSpecie);
    assert_eq!(money.public_fiat(AgentId(1)), Gold(5));
}

#[test]
fn m13_scenarios_preserve_m6_bridge_prefix() {
    let bridge = emerged_gold_bridge_agents();
    let expected = seed_rows(&bridge);

    assert_eq!(bridge_gold(&bridge), BRIDGE_GOLD);
    assert!(bridge.iter().all(|agent| agent.stock.get(GOLD) == 0));

    for name in m13_scenarios() {
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
        assert!(scenario
            .agents
            .iter()
            .skip(bridge.len())
            .all(|agent| agent.id.0 > 10));
    }
}

#[test]
fn m13_scenarios_match_suspended_redemption_prefix() {
    let base = run_with_audits(ScenarioName::EmergedGoldSuspendedRedemption);

    for name in m13_scenarios() {
        let society = run_with_audits(name);

        assert_eq!(
            m3_prefix(&base.m3_records, CLAIM_DEBT_TICK),
            m3_prefix(&society.m3_records, CLAIM_DEBT_TICK),
            "{name:?} M3 prefix"
        );
        assert_eq!(
            bank_prefix(&base.bank_audit, CLAIM_DEBT_TICK),
            bank_prefix(&society.bank_audit, CLAIM_DEBT_TICK),
            "{name:?} bank prefix"
        );
        assert!(society.m3_records.iter().all(|record| {
            record.public_fiat == Gold::ZERO
                && record.fiat_credit_issued == Gold::ZERO
                && record.fiat_fiscal_issued == Gold::ZERO
        }));
    }
}

#[test]
fn m13_redemption_is_suspended_before_debt_payment() {
    for name in m13_scenarios() {
        let society = run_with_audits(name);
        let final_bank = bank_at(&society.bank_audit, CLAIM_DEBT_TICK);

        assert_eq!(sum_requested(&society.redemption_audit), Gold(4));
        assert_eq!(sum_honored(&society.redemption_audit), Gold::ZERO);
        assert_eq!(sum_failed(&society.redemption_audit), Gold(4));
        assert!(society
            .redemption_audit
            .iter()
            .all(|row| row.outcome == RedemptionOutcome::Suspended));
        assert_eq!(final_bank.reserves, Gold(2));
        assert!(!final_bank.convertible);
        assert_eq!(
            seeded_debt_row(&society.debt_payment_audit).tick,
            CLAIM_DEBT_TICK
        );
    }
}

#[test]
fn claim_debt_refusal_defaults_without_specie() {
    let society = run(ScenarioName::EmergedGoldBankClaimDebtRefusalControl);
    let row = seeded_debt_row(&society.debt_payment_audit);

    assert_eq!(row.tender, PublicDebtTender::SpecieOnly);
    assert_eq!(row.paid, Gold::ZERO);
    assert_eq!(row.remaining, Gold(1));
    assert_eq!(row.demand_claims, Gold::ZERO);
    assert_eq!(row.public_specie, Gold::ZERO);
    assert_eq!(row.state, DebtPaymentState::Defaulted);
}

#[test]
fn claim_debt_legal_tender_spends_unredeemable_claim() {
    let society = run(ScenarioName::EmergedGoldBankClaimDebtLegalTender);
    let row = seeded_debt_row(&society.debt_payment_audit);

    assert_eq!(row.tender, PublicDebtTender::BankClaimsAndSpecie);
    assert_eq!(row.paid, Gold(1));
    assert_eq!(row.remaining, Gold::ZERO);
    assert_eq!(row.demand_claims, Gold(1));
    assert_eq!(row.public_fiat, Gold::ZERO);
    assert_eq!(row.public_specie, Gold::ZERO);
    assert_eq!(row.state, DebtPaymentState::Settled);
}

#[test]
fn claim_legal_tender_transfers_claim_without_redemption() {
    let base = run_with_audits(ScenarioName::EmergedGoldSuspendedRedemption);
    let legal = run_with_audits(ScenarioName::EmergedGoldBankClaimDebtLegalTender);
    let base_tick = record_at(&base.m3_records, CLAIM_DEBT_TICK);
    let legal_tick = record_at(&legal.m3_records, CLAIM_DEBT_TICK);
    let legal_bank = bank_at(&legal.bank_audit, CLAIM_DEBT_TICK);
    let base_money = base.money_system.as_ref().expect("base M3 money system");
    let legal_money = legal.money_system.as_ref().expect("legal M3 money system");

    assert_eq!(legal_tick.demand_claims, Gold(4));
    assert_eq!(legal_tick.bank_reserves, Gold(2));
    assert_eq!(legal_bank.reserves, Gold(2));
    assert_eq!(legal_tick.public_fiat, Gold::ZERO);
    assert_eq!(
        legal_tick.tms,
        legal_tick
            .public_specie
            .saturating_add(legal_tick.demand_claims)
    );
    assert_eq!(legal_tick.demand_claims, base_tick.demand_claims);
    assert_eq!(legal_tick.bank_reserves, base_tick.bank_reserves);
    assert_eq!(legal_tick.public_specie, base_tick.public_specie);
    assert_eq!(legal_tick.tms, base_tick.tms);
    assert_eq!(
        base_money.demand_claim_on(CLAIM_BORROWER, CLAIM_BANK),
        Gold(1)
    );
    assert_eq!(
        legal_money.demand_claim_on(CLAIM_BORROWER, CLAIM_BANK),
        Gold::ZERO
    );
    assert_eq!(
        base_money.demand_claim_on(CLAIM_LENDER, CLAIM_BANK),
        Gold::ZERO
    );
    assert_eq!(
        legal_money.demand_claim_on(CLAIM_LENDER, CLAIM_BANK),
        Gold(1)
    );
}

#[test]
fn spot_tender_remains_specie_only() {
    for name in m13_scenarios() {
        let scenario = builtin_market_scenario(name);
        assert!(scenario.events.iter().any(|event| {
            event.tick == Tick(CLAIM_DEBT_TICK)
                && matches!(
                    event.kind,
                    EventKind::SetPublicSpotTender(PublicSpotTender::SpecieOnly)
                )
        }));

        let society = run(name);
        assert_eq!(society.public_spot_tender, PublicSpotTender::SpecieOnly);
        assert!(society
            .payment_audit
            .iter()
            .all(|row| row.demand_claims == Gold::ZERO));
    }
}

#[test]
fn m13_shadow_preserves_policy_but_has_no_claim_to_tender() {
    let scenario = builtin_market_scenario(ScenarioName::EmergedGoldBankClaimDebtLegalTender);
    let shadow = credit_disabled_scenario(&scenario);

    assert!(shadow.events.iter().any(|event| matches!(
        event.kind,
        EventKind::SetPublicDebtTender(PublicDebtTender::BankClaimsAndSpecie)
    )));
    assert_eq!(seeded_debt(&shadow.events), seeded_debt(&scenario.events));

    let periods = shadow.periods;
    let mut society = Society::from_scenario(shadow);
    society.run(periods);

    assert_eq!(
        society.public_debt_tender,
        PublicDebtTender::BankClaimsAndSpecie
    );
    assert!(society
        .m3_records
        .iter()
        .all(|record| record.demand_claims == Gold::ZERO));
    let row = seeded_debt_row(&society.debt_payment_audit);
    assert_eq!(row.demand_claims, Gold::ZERO);
    assert_eq!(row.public_fiat, Gold::ZERO);
    assert_eq!(row.public_specie, Gold::ZERO);
    assert_eq!(row.paid, Gold::ZERO);
}

fn m13_scenarios() -> [ScenarioName; 2] {
    [
        ScenarioName::EmergedGoldBankClaimDebtRefusalControl,
        ScenarioName::EmergedGoldBankClaimDebtLegalTender,
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

fn only_debt_row(records: &[DebtPaymentAuditRecord]) -> &DebtPaymentAuditRecord {
    assert_eq!(records.len(), 1);
    &records[0]
}

fn seeded_debt_row(records: &[DebtPaymentAuditRecord]) -> &DebtPaymentAuditRecord {
    records
        .iter()
        .find(|row| {
            row.tick == CLAIM_DEBT_TICK
                && row.from == CLAIM_BORROWER
                && row.to == CLAIM_LENDER
                && row.owed == Gold(1)
        })
        .expect("seeded M13 debt row")
}

fn seeded_debt(events: &[Event]) -> (AgentId, AgentId, Gold, Gold, Tick, DebtPurpose) {
    events
        .iter()
        .find_map(|event| match &event.kind {
            EventKind::SeedCommodityDebt {
                lender,
                borrower,
                principal,
                due,
                due_tick,
                purpose,
            } => Some((
                *lender,
                *borrower,
                *principal,
                *due,
                *due_tick,
                purpose.clone(),
            )),
            _ => None,
        })
        .expect("seeded debt event")
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

fn record_at(records: &[M3Record], tick: u64) -> &M3Record {
    records
        .iter()
        .find(|record| record.m2.tick == tick)
        .expect("M3 record for tick")
}

fn bank_at(records: &[BankAuditRecord], tick: u64) -> &BankAuditRecord {
    records
        .iter()
        .find(|record| record.tick == tick && record.bank == CLAIM_BANK)
        .expect("bank record for tick")
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
