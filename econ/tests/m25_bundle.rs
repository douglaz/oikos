use econ::agent::{Agent, AgentId, Role, Want, WantKind};
use econ::bundle::{appraise_project_bundle, ProjectBundleCandidate, ProjectBundleEndowment};
use econ::capital::ProjectLineId;
use econ::good::{Gold, Horizon, Stock, GOLD};
use econ::purpose::ProjectPlanId;
use econ::scenario::{builtin_market_scenario, ScenarioName};
use econ::society::Society;
use econ::timemarket::DebtState;

fn run(name: ScenarioName) -> Society {
    let scenario = builtin_market_scenario(name);
    let periods = scenario.periods;
    let mut society = Society::from_scenario(scenario);
    society.run(periods);
    society
}

#[test]
fn borrow_to_build_uses_project_purpose_commodity_credit() {
    let scenario = builtin_market_scenario(ScenarioName::BorrowToBuild);
    let initial_gold = total_agent_gold(&scenario.agents);
    let mut society = Society::from_scenario(scenario);
    let mut saw_project_trade = false;

    for _ in 0..society_tick_periods(ScenarioName::BorrowToBuild) {
        let before = agent_gold_snapshot(&society);
        let loan_start = society.loan_trades.len();
        society.step();
        assert_eq!(society.total_gold(), initial_gold);
        for trade in &society.loan_trades[loan_start..] {
            if trade.purpose.project_plan().is_none() {
                continue;
            }
            let lender = trade.lender.agent().expect("M2 loan lender is an agent");
            saw_project_trade = true;
            assert_eq!(
                gold_for(&society, lender),
                gold_in(&before, lender).saturating_sub(trade.present)
            );
            assert_eq!(
                gold_for(&society, trade.borrower),
                gold_in(&before, trade.borrower).saturating_add(trade.present)
            );
        }
    }

    assert!(saw_project_trade);
}

#[test]
fn cash_poor_capitalist_does_not_self_fund() {
    let scenario = builtin_market_scenario(ScenarioName::BorrowToBuild);
    let capitalist = capitalist(&scenario.agents);
    let initial_gold = gold_in_agents(&scenario.agents, capitalist);
    let society = run(ScenarioName::BorrowToBuild);
    let project = society
        .m2_projects
        .iter()
        .find(|project| project.owner == capitalist)
        .expect("capitalist project");

    assert!(
        project
            .advanced_gold
            .saturating_add(project.input_cost_basis)
            > initial_gold
    );
    assert!(society
        .loan_trades
        .iter()
        .any(|trade| { trade.borrower == capitalist && trade.purpose.project_plan().is_some() }));
}

#[test]
fn project_loan_precedes_project_labor() {
    let society = run(ScenarioName::BorrowToBuild);
    let trade = society
        .loan_trades
        .iter()
        .find(|trade| trade.purpose.project_plan().is_some())
        .expect("project loan trade");
    let project = trade.project.expect("loan linked to project");
    let labor_tick = society
        .labor_trades
        .iter()
        .filter(|trade| trade.project == project)
        .map(|trade| trade.tick)
        .min()
        .expect("project labor trade");

    assert!(trade.tick < labor_tick);
}

#[test]
fn project_debt_settles_after_actual_sale() {
    let scenario = builtin_market_scenario(ScenarioName::BorrowToBuild);
    let owner = capitalist(&scenario.agents);
    let society = run(ScenarioName::BorrowToBuild);
    let first_sale_tick = society
        .trades
        .iter()
        .filter(|trade| trade.seller == owner)
        .map(|trade| trade.tick)
        .min()
        .expect("project output sale");
    let settled_tick = society
        .m2_records
        .iter()
        .find(|record| record.project_debts_settled > 0)
        .map(|record| record.tick)
        .expect("project debt settlement");

    assert!(first_sale_tick < settled_tick);
    assert!(society
        .debts
        .iter()
        .filter(|debt| debt.purpose.project_plan().is_some())
        .all(|debt| debt.state == DebtState::Settled));
    assert!(society
        .m2_records
        .iter()
        .all(|record| record.project_debts_defaulted == 0));
}

#[test]
fn roundabout_capital_remains_m2_self_funded() {
    let society = run(ScenarioName::RoundaboutCapital);
    let capitalist_ids = society
        .agents
        .iter()
        .filter(|agent| {
            agent
                .roles
                .iter()
                .any(|role| matches!(role, Role::Capitalist))
        })
        .map(|agent| agent.id)
        .collect::<Vec<_>>();

    assert!(society.loan_trades.iter().all(|trade| {
        trade.purpose.project_plan().is_none() || !capitalist_ids.contains(&trade.borrower)
    }));
}

#[test]
fn sound_money_100pct_still_conserves_gold() {
    let scenario = builtin_market_scenario(ScenarioName::SoundMoney100Pct);
    let initial_gold = total_agent_gold(&scenario.agents);
    let periods = scenario.periods;
    let mut society = Society::from_scenario(scenario);

    for _ in 0..periods {
        society.step();
        assert_eq!(society.total_gold(), initial_gold);
        assert_eq!(society.m2_records.last().unwrap().total_gold, initial_gold);
    }
}

#[test]
fn bundle_appraisal_does_not_call_present_value() {
    let agent = Agent {
        id: AgentId(1),
        scale: vec![
            want(WantKind::Good(GOLD), Horizon::Now),
            want(WantKind::Good(GOLD), Horizon::Later(7)),
            want(WantKind::Good(GOLD), Horizon::Later(7)),
            want(WantKind::Good(GOLD), Horizon::Later(7)),
        ],
        stock: Stock::new(3),
        gold: Gold::ZERO,
        labor_capacity: 0,
        hunger_deficit: 0,
        roles: vec![Role::Capitalist],
        expect: Vec::new(),
    };
    let schedule = agent.derive_agio_schedule(&[], econ::project::Tick(0));
    assert!(schedule.max_future_due_for_borrowing(Gold(1), 7).is_some());
    assert!(schedule.lending.is_empty());
    assert_eq!(schedule.present_value(Gold(4), 7), None);

    let candidate = ProjectBundleCandidate {
        owner: agent.id,
        line: ProjectLineId(3),
        present_advance: Gold(1),
        expected_revenue: Gold(4),
        input_cost_basis: Gold::ZERO,
        required_labor: 1,
        project_period: 2,
        loan_horizon: 7,
        input_goods: Vec::new(),
    };
    let endowment = ProjectBundleEndowment {
        scale: &agent.scale,
        stock: &agent.stock,
        gold: agent.gold,
        receivables: &[],
        payables: &[],
        tick: econ::project::Tick(0),
    };

    assert!(appraise_project_bundle(&endowment, &candidate, ProjectPlanId(99)).is_some());
}

fn society_tick_periods(name: ScenarioName) -> u64 {
    builtin_market_scenario(name).periods
}

fn want(kind: WantKind, horizon: Horizon) -> Want {
    Want {
        kind,
        horizon,
        qty: 1,
        satisfied: false,
    }
}

fn capitalist(agents: &[Agent]) -> AgentId {
    agents
        .iter()
        .find(|agent| {
            agent
                .roles
                .iter()
                .any(|role| matches!(role, Role::Capitalist))
        })
        .map(|agent| agent.id)
        .expect("scenario has a capitalist")
}

fn total_agent_gold(agents: &[Agent]) -> Gold {
    agents
        .iter()
        .fold(Gold::ZERO, |total, agent| total.saturating_add(agent.gold))
}

fn agent_gold_snapshot(society: &Society) -> Vec<(AgentId, Gold)> {
    society
        .agents
        .iter()
        .map(|agent| (agent.id, agent.gold))
        .collect()
}

fn gold_in(snapshot: &[(AgentId, Gold)], agent: AgentId) -> Gold {
    snapshot
        .iter()
        .find(|(entry, _)| *entry == agent)
        .map(|(_, gold)| *gold)
        .unwrap_or(Gold::ZERO)
}

fn gold_in_agents(agents: &[Agent], agent: AgentId) -> Gold {
    agents
        .iter()
        .find(|entry| entry.id == agent)
        .map(|entry| entry.gold)
        .unwrap_or(Gold::ZERO)
}

fn gold_for(society: &Society, agent: AgentId) -> Gold {
    society
        .agents
        .iter()
        .find(|entry| entry.id == agent)
        .map(|entry| entry.gold)
        .unwrap_or(Gold::ZERO)
}
