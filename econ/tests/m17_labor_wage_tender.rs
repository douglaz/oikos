use econ::agent::{Agent, AgentId, Role, Want, WantKind};
use econ::capital::{dry_fish_short_line, start_project, M2Project, M2ProjectId};
use econ::expect::PriceBelief;
use econ::factor::{
    FactorSide, LaborBook, LaborMarketView, LaborOrder, LaborReservations, LaborTrade,
};
use econ::good::{Gold, GoodId, Horizon, Stock, FOOD, GOLD};
use econ::ledger::{BankId, MoneySystem};
use econ::market::{Order, OrderBook, OrderSide, Reservations};
use econ::money::{
    BankRepaymentTender, IssuerRepaymentTender, LaborWageTender, PublicDebtTender, PublicSpotTender,
};
use econ::project::Tick;
use econ::purpose::CreditSource;
use econ::record::{M3Record, WagePaymentAuditRecord};
use econ::report::{
    render_labor_tape, render_loan_tape, render_money_tape, render_wage_payment_tape,
};
use econ::scenario::{
    builtin_market_scenario, emerged_gold_bridge_agents, EventKind, ScenarioName,
};
use econ::shadow::credit_disabled_scenario;
use econ::society::Society;
use econ::timemarket::DebtState;

const BRIDGE_GOLD: Gold = Gold(16);
const WAGE_TAPE_HEADER: &str =
    "tick,project,employer,worker,wage,qty,amount,public_fiat,demand_claims,public_specie,tender";
const EXPECTED_LEGAL_WAGE_TAPE: &str = "\
tick,project,employer,worker,wage,qty,amount,public_fiat,demand_claims,public_specie,tender
1,1,200,201,1,1,1,1,0,0,fiat-and-specie
1,2,202,203,1,1,1,1,0,0,fiat-and-specie
1,3,204,205,1,1,1,1,0,0,fiat-and-specie
2,4,206,207,1,1,1,1,0,0,fiat-and-specie
2,5,208,209,1,1,1,1,0,0,fiat-and-specie
2,6,210,211,1,1,1,1,0,0,fiat-and-specie
";

#[derive(Clone, Debug, PartialEq, Eq)]
struct SeedRow {
    agent: AgentId,
    gold: Gold,
    stock: Vec<(GoodId, u32)>,
    scale: Vec<(WantKind, Horizon, u32)>,
}

#[derive(Clone, Debug)]
struct WageCase {
    trades: Vec<LaborTrade>,
    audit: Vec<WagePaymentAuditRecord>,
    money: MoneySystem,
}

#[test]
fn m17_scenarios_preserve_m6_bridge_prefix() {
    let bridge = emerged_gold_bridge_agents();
    let expected = seed_rows(&bridge);
    let base = builtin_market_scenario(ScenarioName::EmergedGoldFiatCreditExpansion);

    assert_eq!(bridge_gold(&bridge), BRIDGE_GOLD);
    assert!(bridge.iter().all(|agent| agent.stock.get(GOLD) == 0));

    for name in m17_scenarios() {
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
fn base_pays_every_wage_in_fiat() {
    let society = run(ScenarioName::EmergedGoldFiatCreditExpansion);

    assert_eq!(society.wage_payment_audit.len(), 6);
    for (index, row) in society.wage_payment_audit.iter().enumerate() {
        let pair = u32::try_from(index).unwrap();
        let expected_employer = AgentId(u64::from(200 + pair * 2));
        let expected_worker = AgentId(u64::from(201 + pair * 2));

        assert!(row.tick == 1 || row.tick == 2);
        assert_eq!(row.project, M2ProjectId(u64::try_from(index + 1).unwrap()));
        assert_eq!(row.employer, expected_employer);
        assert_eq!(row.worker, expected_worker);
        assert_eq!(row.wage, Gold(1));
        assert_eq!(row.qty, 1);
        assert_eq!(row.amount, Gold(1));
        assert_eq!(row.public_fiat, Gold(1));
        assert_eq!(row.demand_claims, Gold::ZERO);
        assert_eq!(row.public_specie, Gold::ZERO);
        assert_eq!(row.tender, LaborWageTender::ParAll);
    }
}

#[test]
fn legal_tender_run_is_byte_identical_to_base() {
    let base = run_with_money_audit(ScenarioName::EmergedGoldFiatCreditExpansion);
    let legal = run_with_money_audit(ScenarioName::EmergedGoldFiatWageLegalTender);

    assert_eq!(legal.m3_records, base.m3_records);
    assert_eq!(
        render_labor_tape(&legal.labor_trades),
        render_labor_tape(&base.labor_trades)
    );
    assert_eq!(
        render_loan_tape(&legal.loan_trades),
        render_loan_tape(&base.loan_trades)
    );
    assert_eq!(
        render_money_tape(&legal.money_audit),
        render_money_tape(&base.money_audit)
    );
    assert_eq!(
        render_wage_payment_tape(&legal.wage_payment_audit),
        EXPECTED_LEGAL_WAGE_TAPE
    );
}

#[test]
fn wage_refusal_blocks_every_fiat_wage() {
    let mut partial = Society::from_scenario(builtin_market_scenario(
        ScenarioName::EmergedGoldFiatWageRefusalControl,
    ));
    partial.step();
    partial.step();
    let resting_worker_asks = fiat_credit_workers()
        .iter()
        .filter_map(|worker| {
            partial
                .labor_book
                .live_order(*worker, FactorSide::Work)
                .map(|order| (*worker, order.seq))
        })
        .collect::<Vec<_>>();
    assert!(
        !resting_worker_asks.is_empty(),
        "worker asks should rest before TTL expiry"
    );
    for _ in 0..3 {
        partial.step();
    }
    assert!(resting_worker_asks.iter().all(|(worker, seq)| {
        partial
            .labor_book
            .live_order(*worker, FactorSide::Work)
            .map(|order| order.seq != *seq)
            .unwrap_or(true)
    }));

    let society = run(ScenarioName::EmergedGoldFiatWageRefusalControl);

    assert!(society.labor_trades.is_empty());
    assert!(society
        .m3_records
        .iter()
        .all(|record| record.m2.labor_trades == 0));
    assert!(society.wage_payment_audit.is_empty());
    assert_eq!(
        render_wage_payment_tape(&society.wage_payment_audit)
            .lines()
            .next(),
        Some(WAGE_TAPE_HEADER)
    );
    assert!(society
        .m2_projects
        .iter()
        .filter(|project| (200..=214).contains(&project.owner.0))
        .all(project_has_no_input_or_labor_debit));
}

#[test]
fn wage_refusal_severs_the_boom_bust_transmission() {
    let refusal = run(ScenarioName::EmergedGoldFiatWageRefusalControl);
    let legal = run(ScenarioName::EmergedGoldFiatWageLegalTender);
    let base = run(ScenarioName::EmergedGoldFiatCreditExpansion);

    assert!(refusal
        .m3_records
        .iter()
        .all(|record| record.m2.structure_length_ticks_x100 == 0));
    assert!(refusal
        .m3_records
        .iter()
        .all(|record| record.boom_projects_started == 0));
    assert_eq!(sum_busts(&refusal.m3_records), 0);
    assert_eq!(sum_labor_consumed(&refusal.m3_records), 0);
    assert_eq!(sum_goods_consumed(&refusal.m3_records), 0);
    assert_eq!(sum_fiat_credit_issued(&refusal.m3_records), Gold(8));

    assert_eq!(legal.m3_records, base.m3_records);
    assert_eq!(peak_structure(&legal.m3_records), 800);
    assert_eq!(sum_busts(&legal.m3_records), 6);
    assert_eq!(final_labor_consumed(&legal.m3_records), 6);
}

#[test]
fn wage_refusal_fiat_round_trips_to_issuer() {
    let refusal = run(ScenarioName::EmergedGoldFiatWageRefusalControl);
    let legal = run(ScenarioName::EmergedGoldFiatWageLegalTender);
    let issued = sum_fiat_credit_issued(&refusal.m3_records);
    let retired = sum_credit_retired(&refusal.m3_records);
    let final_row = refusal.m3_records.last().expect("refusal records");
    let initial_public_specie = refusal
        .m3_records
        .first()
        .expect("refusal records")
        .public_specie;
    let money = refusal.money_system.as_ref().expect("M3 money system");

    assert_eq!(issued, Gold(8));
    assert!(issued > Gold::ZERO);
    assert_eq!(retired, issued);
    assert_eq!(final_row.public_fiat, Gold::ZERO);
    assert_eq!(final_row.demand_claims, Gold::ZERO);
    assert_eq!(final_row.tms, final_row.public_specie);
    assert!(refusal
        .m3_records
        .iter()
        .all(|record| record.public_specie == initial_public_specie));
    assert_eq!(money.base.commodity_base, initial_public_specie);
    assert!(refusal
        .debts
        .iter()
        .filter(|debt| matches!(debt.funding, CreditSource::FiatCredit(_)))
        .all(|debt| debt.state == DebtState::Settled));
    assert_eq!(defaulted_fiat_credit_debts(&refusal), 0);

    let legal_issued = sum_fiat_credit_issued(&legal.m3_records);
    let legal_retired = sum_credit_retired(&legal.m3_records);
    assert!(legal_retired < legal_issued);
    assert!(defaulted_fiat_credit_debts(&legal) > 0);
}

#[test]
fn wage_settlement_draws_only_accepted_media() {
    let specie_only = wage_case(LaborWageTender::SpecieOnly, Gold(1), Gold(1), Gold::ZERO);
    assert_eq!(specie_only.trades.len(), 1);
    assert_eq!(specie_only.audit[0].public_specie, Gold(1));
    assert_eq!(specie_only.audit[0].public_fiat, Gold::ZERO);
    assert_eq!(specie_only.money.public_fiat(AgentId(1)), Gold(1));

    let refused = wage_case(LaborWageTender::SpecieOnly, Gold::ZERO, Gold(1), Gold::ZERO);
    assert!(refused.trades.is_empty());
    assert!(refused.audit.is_empty());
    assert_eq!(refused.money.public_fiat(AgentId(1)), Gold(1));

    let fiat_and_specie = wage_case(LaborWageTender::FiatAndSpecie, Gold::ZERO, Gold(1), Gold(1));
    assert_eq!(fiat_and_specie.trades.len(), 1);
    assert_eq!(fiat_and_specie.audit[0].public_fiat, Gold(1));
    assert_eq!(fiat_and_specie.audit[0].demand_claims, Gold::ZERO);
    assert_eq!(
        fiat_and_specie.money.demand_claim_on(AgentId(1), BankId(1)),
        Gold(1)
    );

    let claim_refused = wage_case(
        LaborWageTender::FiatAndSpecie,
        Gold::ZERO,
        Gold::ZERO,
        Gold(1),
    );
    assert!(claim_refused.trades.is_empty());
    assert!(claim_refused.audit.is_empty());
    assert_eq!(
        claim_refused.money.demand_claim_on(AgentId(1), BankId(1)),
        Gold(1)
    );

    let par_fiat_specie = wage_case(LaborWageTender::ParAll, Gold(1), Gold(1), Gold::ZERO);
    assert_eq!(par_fiat_specie.audit[0].public_fiat, Gold(1));
    assert_eq!(par_fiat_specie.audit[0].public_specie, Gold::ZERO);

    let par_fiat_only = wage_case(LaborWageTender::ParAll, Gold::ZERO, Gold(1), Gold::ZERO);
    assert_eq!(par_fiat_only.audit[0].public_fiat, Gold(1));

    let par_fiat_claim = wage_case(LaborWageTender::ParAll, Gold::ZERO, Gold(1), Gold(1));
    assert_eq!(par_fiat_claim.audit[0].public_fiat, Gold(1));
    assert_eq!(par_fiat_claim.audit[0].demand_claims, Gold::ZERO);

    let par_claim_only = wage_case(LaborWageTender::ParAll, Gold::ZERO, Gold::ZERO, Gold(1));
    assert_eq!(par_claim_only.audit[0].public_fiat, Gold::ZERO);
    assert_eq!(par_claim_only.audit[0].demand_claims, Gold(1));
    assert_eq!(
        par_claim_only.money.demand_claim_on(AgentId(2), BankId(1)),
        Gold(1)
    );
}

#[test]
fn wage_tender_is_independent_of_the_other_five_tenders() {
    let mut policy_surface = Society::from_scenario(builtin_market_scenario(
        ScenarioName::EmergedGoldFiatCreditExpansion,
    ));
    policy_surface.public_spot_tender = PublicSpotTender::SpecieOnly;
    policy_surface.public_debt_tender = PublicDebtTender::SpecieOnly;
    policy_surface.bank_repayment_tender = BankRepaymentTender::SpecieOnly;
    policy_surface.issuer_repayment_tender = IssuerRepaymentTender::FiatOnly;
    policy_surface.labor_wage_tender = LaborWageTender::FiatAndSpecie;

    let wage = wage_case(
        policy_surface.labor_wage_tender,
        Gold::ZERO,
        Gold(1),
        Gold::ZERO,
    );
    assert_eq!(wage.trades.len(), 1);
    assert_eq!(wage.audit[0].public_fiat, Gold(1));

    policy_surface.public_spot_tender = PublicSpotTender::FiatAndSpecie;
    policy_surface.labor_wage_tender = LaborWageTender::SpecieOnly;

    let refused_wage = wage_case(
        policy_surface.labor_wage_tender,
        Gold::ZERO,
        Gold(1),
        Gold::ZERO,
    );
    assert!(refused_wage.trades.is_empty());

    let spot_payment = fiat_spot_payment(policy_surface.public_spot_tender);
    assert_eq!(spot_payment.fiat, Gold(1));
    assert_eq!(spot_payment.specie, Gold::ZERO);
    assert_eq!(spot_payment.claims, Vec::new());
}

#[test]
fn wage_audit_rows_match_committed_composition() {
    let society = run(ScenarioName::EmergedGoldFiatWageLegalTender);

    for row in &society.wage_payment_audit {
        assert_eq!(row.amount, row.wage.mul_qty(row.qty).expect("amount fits"));
        assert_eq!(
            row.public_fiat
                .saturating_add(row.demand_claims)
                .saturating_add(row.public_specie),
            row.amount
        );
    }

    let m2 = run(ScenarioName::RoundaboutCapital);
    assert!(m2.wage_payment_audit.is_empty());
}

#[test]
fn m17_shadow_preserves_policy_with_no_wage_trades() {
    for (name, tender) in [
        (
            ScenarioName::EmergedGoldFiatWageRefusalControl,
            LaborWageTender::SpecieOnly,
        ),
        (
            ScenarioName::EmergedGoldFiatWageLegalTender,
            LaborWageTender::FiatAndSpecie,
        ),
    ] {
        let scenario = builtin_market_scenario(name);
        let shadow = credit_disabled_scenario(&scenario);
        assert!(shadow.events.iter().any(|event| matches!(
            event.kind,
            EventKind::SetLaborWageTender(policy) if policy == tender
        )));

        let periods = shadow.periods;
        let mut society = Society::from_scenario(shadow);
        society.run(periods);

        assert!(society.labor_trades.is_empty());
        assert!(society.wage_payment_audit.is_empty());
        assert!(society
            .m3_records
            .iter()
            .all(|record| record.m2.labor_trades == 0));
    }
}

fn m17_scenarios() -> [ScenarioName; 2] {
    [
        ScenarioName::EmergedGoldFiatWageRefusalControl,
        ScenarioName::EmergedGoldFiatWageLegalTender,
    ]
}

fn run(name: ScenarioName) -> Society {
    let scenario = builtin_market_scenario(name);
    let periods = scenario.periods;
    let mut society = Society::from_scenario(scenario);
    society.run(periods);
    society
}

fn run_with_money_audit(name: ScenarioName) -> Society {
    let scenario = builtin_market_scenario(name);
    let periods = scenario.periods;
    let mut society = Society::from_scenario(scenario);
    society.enable_money_audit();
    society.run(periods);
    society
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

fn fiat_credit_workers() -> [AgentId; 6] {
    [
        AgentId(201),
        AgentId(203),
        AgentId(205),
        AgentId(207),
        AgentId(209),
        AgentId(211),
    ]
}

fn project_has_no_input_or_labor_debit(project: &M2Project) -> bool {
    project.input_goods_committed.is_empty()
        && project.labor_advanced == 0
        && project.advanced_gold == Gold::ZERO
}

fn sum_fiat_credit_issued(records: &[M3Record]) -> Gold {
    records.iter().fold(Gold::ZERO, |sum, record| {
        sum.saturating_add(record.fiat_credit_issued)
    })
}

fn sum_credit_retired(records: &[M3Record]) -> Gold {
    records.iter().fold(Gold::ZERO, |sum, record| {
        sum.saturating_add(record.credit_retired)
    })
}

fn sum_busts(records: &[M3Record]) -> u32 {
    records.iter().fold(0u32, |sum, record| {
        sum.saturating_add(record.bust_abandoned_projects)
    })
}

fn sum_labor_consumed(records: &[M3Record]) -> u32 {
    records.iter().fold(0u32, |sum, record| {
        sum.saturating_add(record.m2.capital_labor_consumed)
    })
}

fn final_labor_consumed(records: &[M3Record]) -> u32 {
    records
        .last()
        .map(|record| record.m2.capital_labor_consumed)
        .unwrap_or(0)
}

fn sum_goods_consumed(records: &[M3Record]) -> u32 {
    records.iter().fold(0u32, |sum, record| {
        sum.saturating_add(record.m2.capital_goods_consumed)
    })
}

fn peak_structure(records: &[M3Record]) -> u64 {
    records
        .iter()
        .map(|record| record.m2.structure_length_ticks_x100)
        .max()
        .unwrap_or(0)
}

fn defaulted_fiat_credit_debts(society: &Society) -> usize {
    society
        .debts
        .iter()
        .filter(|debt| {
            matches!(debt.funding, CreditSource::FiatCredit(_))
                && debt.state == DebtState::Defaulted
        })
        .count()
}

fn wage_case(
    tender: LaborWageTender,
    employer_specie: Gold,
    employer_fiat: Gold,
    employer_claim: Gold,
) -> WageCase {
    let mut agents = vec![
        test_agent(1, employer_specie, 0, 0),
        test_agent(2, Gold::ZERO, 0, 1),
    ];
    let mut money = MoneySystem::from_agents(&agents);
    if employer_fiat > Gold::ZERO {
        money.credit_fiat(AgentId(1), employer_fiat).unwrap();
    }
    if employer_claim > Gold::ZERO {
        money
            .issue_demand_claim(BankId(1), AgentId(1), employer_claim, Gold::ZERO)
            .unwrap();
    }
    money.reconcile_agent_cache(&mut agents);

    let line = dry_fish_short_line();
    let mut projects = vec![start_project(
        AgentId(1),
        &line,
        M2ProjectId(1),
        Tick(0),
        Gold(4),
        Gold::ZERO,
    )];
    let lines = vec![line];
    let mut reservations = LaborReservations::new();
    let mut book = LaborBook::new();
    let mut audit = Vec::new();

    let trades = {
        let mut market = LaborMarketView {
            agents: &mut agents,
            reservations: &mut reservations,
            projects: &mut projects,
            lines: &lines,
            money_system: Some(&mut money),
            wage_media: tender.accepted_media(),
            wage_audit: Some(&mut audit),
            wage_tender: tender,
        };
        let work = labor_order(2, FactorSide::Work, 1, 1);
        assert!(market.reservations.reserve_order(market.agents, &work));
        assert!(book.add_order(work, None, 0, &mut market).is_empty());

        let hire = labor_order(1, FactorSide::Hire, 1, 2);
        assert!(market.reservations.reserve_order(market.agents, &hire));
        book.add_order(hire, Some(M2ProjectId(1)), 0, &mut market)
    };

    WageCase {
        trades,
        audit,
        money,
    }
}

fn fiat_spot_payment(tender: PublicSpotTender) -> econ::ledger::MoneyComposition {
    let mut agents = vec![
        test_agent(1, Gold::ZERO, 0, 0),
        test_agent(2, Gold::ZERO, 1, 0),
    ];
    let mut money = MoneySystem::from_agents(&agents);
    money.credit_fiat(AgentId(1), Gold(1)).unwrap();
    money.reconcile_agent_cache(&mut agents);

    let mut reservations = Reservations::new(&agents, FOOD.0);
    let mut book = OrderBook::new(FOOD);
    let ask = spot_order(2, OrderSide::Ask, Gold(1), 1);
    assert!(reservations.reserve_order(&agents, &ask));
    assert!(book
        .add_order_m3(
            ask,
            0,
            &mut agents,
            &mut reservations,
            &mut money,
            tender.accepted_media(),
        )
        .is_empty());

    let bid = spot_order(1, OrderSide::Bid, Gold(1), 2);
    assert!(reservations.reserve_order(&agents, &bid));
    let trades = book.add_order_m3(
        bid,
        0,
        &mut agents,
        &mut reservations,
        &mut money,
        tender.accepted_media(),
    );
    assert_eq!(trades.len(), 1);
    trades[0].payment.clone().expect("M3 payment composition")
}

fn labor_order(agent: u32, side: FactorSide, wage_limit: u64, seq: u64) -> LaborOrder {
    LaborOrder {
        agent: AgentId(u64::from(agent)),
        side,
        wage_limit: Gold(wage_limit),
        qty: 1,
        seq,
        expires_tick: 3,
    }
}

fn spot_order(agent: u32, side: OrderSide, limit: Gold, seq: u64) -> Order {
    Order {
        agent: AgentId(u64::from(agent)),
        side,
        good: FOOD,
        limit,
        qty: 1,
        seq,
        expires_tick: 3,
    }
}

fn test_agent(id: u32, gold: Gold, food: u32, labor_capacity: u32) -> Agent {
    let mut stock = Stock::new(FOOD.0);
    stock.add(FOOD, food);
    Agent {
        id: AgentId(u64::from(id)),
        scale: vec![Want {
            kind: WantKind::Good(GOLD),
            horizon: Horizon::Now,
            qty: 1,
            satisfied: false,
        }],
        stock,
        gold,
        labor_capacity,
        hunger_deficit: 0,
        roles: vec![Role::Trader],
        expect: vec![PriceBelief::new(Gold(1), Gold(1)); usize::from(FOOD.0) + 1],
    }
}
