use econ::agent::{Agent, AgentId, Role, Want, WantKind};
use econ::bank::Bank;
use econ::cantillon::CantillonRoute;
use econ::good::{Gold, GoodId, Horizon, Stock, GOLD};
use econ::issuer::Issuer;
use econ::ledger::{BankId, MoneySystem};
use econ::money::{BankRepaymentTender, IssuerRepaymentTender, PublicDebtTender, PublicSpotTender};
use econ::project::Tick;
use econ::purpose::{CreditLender, CreditSource, DebtPurpose};
use econ::record::{DebtPaymentAuditRecord, DebtPaymentState};
use econ::scenario::{
    builtin_market_scenario, emerged_gold_bridge_agents, Event, EventKind, ScenarioName,
};
use econ::shadow::credit_disabled_scenario;
use econ::society::Society;
use econ::timemarket::{
    settle_due_debts_m3, DebtContract, DebtId, DebtSettlementM3Context, DebtState,
};

const BRIDGE_GOLD: Gold = Gold(16);
const FIRST_RECEIVERS: [AgentId; 2] = [AgentId(7), AgentId(8)];

#[derive(Clone, Debug, PartialEq, Eq)]
struct SeedRow {
    agent: AgentId,
    gold: Gold,
    stock: Vec<(GoodId, u32)>,
    scale: Vec<(WantKind, Horizon, u32)>,
}

#[test]
fn m12_scenarios_preserve_m6_bridge_prefix() {
    let bridge = emerged_gold_bridge_agents();
    let expected = seed_rows(&bridge);

    assert_eq!(bridge_gold(&bridge), BRIDGE_GOLD);
    assert!(bridge.iter().all(|agent| agent.stock.get(GOLD) == 0));

    for name in m12_scenarios() {
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
    }
}

#[test]
fn m12_scenarios_share_same_fiat_print_and_seeded_debt() {
    let refusal = builtin_market_scenario(ScenarioName::EmergedGoldFiatDebtRefusalControl);
    let legal = builtin_market_scenario(ScenarioName::EmergedGoldFiatDebtLegalTender);

    assert_eq!(fiat_print(&refusal.events), fiat_print(&legal.events));
    assert_eq!(
        fiat_print(&refusal.events),
        (Gold(8), CantillonRoute::Agents(FIRST_RECEIVERS.to_vec()))
    );
    assert!(matches!(
        refusal.events[3].kind,
        EventKind::SetPublicSpotTender(PublicSpotTender::SpecieOnly)
    ));
    assert!(matches!(
        legal.events[3].kind,
        EventKind::SetPublicSpotTender(PublicSpotTender::SpecieOnly)
    ));
    assert!(matches!(
        refusal.events[4].kind,
        EventKind::SetPublicDebtTender(PublicDebtTender::SpecieOnly)
    ));
    assert!(matches!(
        legal.events[4].kind,
        EventKind::SetPublicDebtTender(PublicDebtTender::FiatAndSpecie)
    ));
    assert_eq!(seeded_debt(&refusal.events), seeded_debt(&legal.events));
    assert_eq!(
        seeded_debt(&refusal.events),
        (
            AgentId(1),
            AgentId(7),
            Gold(4),
            Gold(4),
            Tick(0),
            DebtPurpose::Consumption
        )
    );
}

#[test]
fn refusal_control_discharges_debt_with_specie_only() {
    let society = run(ScenarioName::EmergedGoldFiatDebtRefusalControl);
    let row = only_debt_row(&society.debt_payment_audit);

    assert_eq!(row.debt, 1);
    assert_eq!(row.from, AgentId(7));
    assert_eq!(row.to, AgentId(1));
    assert_eq!(row.owed, Gold(4));
    assert_eq!(row.paid, Gold(4));
    assert_eq!(row.remaining, Gold::ZERO);
    assert_eq!(row.public_fiat, Gold::ZERO);
    assert_eq!(row.demand_claims, Gold::ZERO);
    assert_eq!(row.public_specie, Gold(4));
    assert_eq!(row.tender, PublicDebtTender::SpecieOnly);
    assert_eq!(row.state, DebtPaymentState::Settled);
    assert!(society.payment_audit.iter().all(|row| {
        row.public_fiat == Gold::ZERO && row.tender == PublicSpotTender::SpecieOnly
    }));
}

#[test]
fn legal_tender_discharges_debt_with_fiat_before_specie() {
    let society = run(ScenarioName::EmergedGoldFiatDebtLegalTender);
    let row = only_debt_row(&society.debt_payment_audit);

    assert_eq!(row.debt, 1);
    assert_eq!(row.from, AgentId(7));
    assert_eq!(row.to, AgentId(1));
    assert_eq!(row.owed, Gold(4));
    assert_eq!(row.paid, Gold(4));
    assert_eq!(row.remaining, Gold::ZERO);
    assert_eq!(row.public_fiat, Gold(4));
    assert_eq!(row.demand_claims, Gold::ZERO);
    assert_eq!(row.public_specie, Gold::ZERO);
    assert_eq!(row.tender, PublicDebtTender::FiatAndSpecie);
    assert_eq!(row.state, DebtPaymentState::Settled);
    assert!(society.payment_audit.iter().all(|row| {
        row.public_fiat == Gold::ZERO && row.tender == PublicSpotTender::SpecieOnly
    }));
}

#[test]
fn legal_tender_differs_from_refusal_only_by_debt_discharge_policy() {
    let refusal_scenario = builtin_market_scenario(ScenarioName::EmergedGoldFiatDebtRefusalControl);
    let legal_scenario = builtin_market_scenario(ScenarioName::EmergedGoldFiatDebtLegalTender);
    let bridge_len = emerged_gold_bridge_agents().len();

    assert_eq!(
        seed_rows(&refusal_scenario.agents[..bridge_len]),
        seed_rows(&legal_scenario.agents[..bridge_len])
    );
    assert_eq!(
        fiat_print(&refusal_scenario.events),
        fiat_print(&legal_scenario.events)
    );
    assert_eq!(
        spot_tender(&refusal_scenario.events),
        spot_tender(&legal_scenario.events)
    );
    assert_eq!(
        seeded_debt(&refusal_scenario.events),
        seeded_debt(&legal_scenario.events)
    );

    let refusal = run(ScenarioName::EmergedGoldFiatDebtRefusalControl);
    let legal = run(ScenarioName::EmergedGoldFiatDebtLegalTender);
    let refusal_final = refusal.m3_records.last().expect("refusal records");
    let legal_final = legal.m3_records.last().expect("legal records");
    let refusal_row = only_debt_row(&refusal.debt_payment_audit);
    let legal_row = only_debt_row(&legal.debt_payment_audit);

    assert_eq!(refusal.public_spot_tender, PublicSpotTender::SpecieOnly);
    assert_eq!(legal.public_spot_tender, PublicSpotTender::SpecieOnly);
    assert_eq!(refusal_final.public_fiat, Gold(8));
    assert_eq!(legal_final.public_fiat, Gold(8));
    assert_eq!(refusal_final.demand_claims, Gold::ZERO);
    assert_eq!(legal_final.demand_claims, Gold::ZERO);
    assert_eq!(refusal_row.public_specie, Gold(4));
    assert_eq!(refusal_row.public_fiat, Gold::ZERO);
    assert_eq!(legal_row.public_specie, Gold::ZERO);
    assert_eq!(legal_row.public_fiat, Gold(4));
}

#[test]
fn m12_shadow_strips_fiat_print_but_preserves_debt_policy_and_seeded_debt() {
    let scenario = builtin_market_scenario(ScenarioName::EmergedGoldFiatDebtLegalTender);
    let shadow = credit_disabled_scenario(&scenario);

    assert!(shadow
        .events
        .iter()
        .all(|event| !matches!(event.kind, EventKind::FiatPrint { .. })));
    assert!(shadow.events.iter().any(|event| matches!(
        &event.kind,
        EventKind::SetPublicDebtTender(PublicDebtTender::FiatAndSpecie)
    )));
    assert_eq!(seeded_debt(&shadow.events), seeded_debt(&scenario.events));

    let periods = shadow.periods;
    let mut society = Society::from_scenario(shadow);
    society.run(periods);

    assert_eq!(society.public_debt_tender, PublicDebtTender::FiatAndSpecie);
    assert_eq!(sum_debt_fiat(&society.debt_payment_audit), Gold::ZERO);
}

#[test]
fn par_all_commodity_debt_settlement_stays_legacy() {
    let mut agents = vec![agent(1, Gold(5)), agent(2, Gold::ZERO)];
    let mut money = MoneySystem::from_agents(&agents);
    money.credit_fiat(AgentId(1), Gold(2)).unwrap();
    money
        .issue_demand_claim(BankId(1), AgentId(1), Gold(3), Gold::ZERO)
        .unwrap();
    money.reconcile_agent_cache(&mut agents);
    let mut debts = vec![DebtContract {
        id: DebtId(1),
        lender: CreditLender::Agent(AgentId(2)),
        borrower: AgentId(1),
        opened_tick: Tick(0),
        due_tick: Tick(1),
        principal: Gold(9),
        due: Gold(9),
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
        public_debt_tender: PublicDebtTender::ParAll,
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
    assert_eq!(row.public_fiat, Gold(2));
    assert_eq!(row.demand_claims, Gold(3));
    assert_eq!(row.public_specie, Gold(4));
    assert_eq!(row.tender, PublicDebtTender::ParAll);
}

fn m12_scenarios() -> [ScenarioName; 2] {
    [
        ScenarioName::EmergedGoldFiatDebtRefusalControl,
        ScenarioName::EmergedGoldFiatDebtLegalTender,
    ]
}

fn run(name: ScenarioName) -> Society {
    let scenario = builtin_market_scenario(name);
    let periods = scenario.periods;
    let mut society = Society::from_scenario(scenario);
    society.run(periods);
    society
}

fn only_debt_row(records: &[DebtPaymentAuditRecord]) -> &DebtPaymentAuditRecord {
    assert_eq!(records.len(), 1);
    &records[0]
}

fn seed_rows(agents: &[Agent]) -> Vec<SeedRow> {
    agents
        .iter()
        .map(|agent| SeedRow {
            agent: agent.id,
            gold: agent.gold,
            stock: agent
                .stock
                .positive_goods()
                .map(|good| (good, agent.stock.get(good)))
                .collect(),
            scale: agent
                .scale
                .iter()
                .map(|want| (want.kind, want.horizon, want.qty))
                .collect(),
        })
        .collect()
}

fn bridge_gold(agents: &[Agent]) -> Gold {
    agents
        .iter()
        .fold(Gold::ZERO, |total, agent| total.saturating_add(agent.gold))
}

fn fiat_print(events: &[Event]) -> (Gold, CantillonRoute) {
    events
        .iter()
        .find_map(|event| match &event.kind {
            EventKind::FiatPrint { amount, route, .. } => Some((*amount, route.clone())),
            _ => None,
        })
        .expect("fiat print event")
}

fn spot_tender(events: &[Event]) -> PublicSpotTender {
    events
        .iter()
        .find_map(|event| match &event.kind {
            EventKind::SetPublicSpotTender(tender) => Some(*tender),
            _ => None,
        })
        .expect("spot tender event")
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

fn sum_debt_fiat(records: &[DebtPaymentAuditRecord]) -> Gold {
    records.iter().fold(Gold::ZERO, |total, record| {
        total.saturating_add(record.public_fiat)
    })
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
