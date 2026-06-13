use std::{fs, path::Path};

use econ::agent::{Agent, AgentId, Role, Want, WantKind};
use econ::capital::{M2Project, M2ProjectId, M2ProjectState, ProjectLineId, ProjectOutputLot};
use econ::good::{Gold, Horizon, FOOD, GOLD};
use econ::metrics::cumulative_project_profit;
use econ::project::Tick;
use econ::record::M2Record;
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
fn time_market_basic_clears_and_settles() {
    let scenario = builtin_market_scenario(ScenarioName::TimeMarketBasic);
    let initial_gold = total_agent_gold(&scenario.agents);
    let periods = scenario.periods;
    let mut society = Society::from_scenario(scenario);

    for _ in 0..periods {
        society.step();
        assert_eq!(society.total_gold(), initial_gold);
        assert_eq!(society.m2_records.last().unwrap().total_gold, initial_gold);
    }

    assert!(!society.loan_trades.is_empty());
    assert!(society
        .debts
        .iter()
        .any(|debt| { debt.state == econ::timemarket::DebtState::Settled }));
}

#[test]
fn commodity_credit_transfers_existing_gold_only() {
    let scenario = builtin_market_scenario(ScenarioName::TimeMarketBasic);
    let initial_gold = total_agent_gold(&scenario.agents);
    let mut society = Society::from_scenario(scenario);
    let before = agent_gold_snapshot(&society);

    society.step();

    let trade = society.loan_trades.first().expect("loan trade");
    let lender = trade.lender.agent().expect("M2 loan lender is an agent");
    assert_eq!(
        gold_for(&society, lender),
        gold_in(&before, lender).saturating_sub(trade.present)
    );
    assert_eq!(
        gold_for(&society, trade.borrower),
        gold_in(&before, trade.borrower).saturating_add(trade.present)
    );
    assert_eq!(society.total_gold(), initial_gold);
}

#[test]
fn roundabout_capital_forms_matures_and_sells() {
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

    assert!(!society.loan_trades.is_empty());
    assert!(society
        .loan_trades
        .iter()
        .all(|trade| !capitalist_ids.contains(&trade.borrower)));
    assert!(!society.labor_trades.is_empty());
    assert!(society
        .m2_records
        .iter()
        .any(|record| record.sold_projects > 0));
    assert!(society
        .m2_records
        .iter()
        .any(|record| record.project_revenue > Gold::ZERO));
    assert!(society
        .m2_projects
        .iter()
        .any(|project| project.state == M2ProjectState::Sold));
    assert!(society
        .project_output_lots
        .iter()
        .all(|lot| lot.qty_remaining == 0));
    assert!(society
        .m2_records
        .iter()
        .any(|record| record.project_profit > 0));
    assert!(society
        .debts
        .iter()
        .all(|debt| debt.state != DebtState::Defaulted));
}

#[test]
fn sold_input_project_profit_subtracts_input_cost_basis() {
    let project = M2Project {
        id: M2ProjectId(1),
        owner: AgentId(1),
        line: ProjectLineId(99),
        state: M2ProjectState::Sold,
        started_at: Tick(0),
        maturity: Tick(1),
        labor_advanced: 1,
        input_goods_committed: Vec::new(),
        input_cost_basis: Gold(6),
        advanced_gold: Gold(3),
        expected_revenue: Gold(12),
        output_good: FOOD,
        output_qty: 1,
        salvage_bps: 5000,
    };
    let lot = ProjectOutputLot {
        project: M2ProjectId(1),
        owner: AgentId(1),
        good: FOOD,
        qty_remaining: 0,
        proceeds: Gold(12),
    };

    assert_eq!(cumulative_project_profit(&[project], &[lot]), 3);
}

#[test]
fn m2_step_path_uses_scenario_kind_not_display_name() {
    let mut renamed_m2 = builtin_market_scenario(ScenarioName::RoundaboutCapital);
    renamed_m2.name = "renamed-roundabout";
    let mut m2_society = Society::from_scenario(renamed_m2);
    m2_society.step();

    assert_eq!(m2_society.m2_records.len(), 1);

    let mut m1_named_like_m2 = builtin_market_scenario(ScenarioName::MarketPriceDiscovery);
    m1_named_like_m2.name = "roundabout-capital";
    let mut m1_society = Society::from_scenario(m1_named_like_m2);
    m1_society.step();

    assert!(m1_society.m2_records.is_empty());
    assert_eq!(m1_society.records.len(), 1);
}

#[test]
fn labor_supply_is_not_forced() {
    let mut scenario = builtin_market_scenario(ScenarioName::RoundaboutCapital);
    scenario.periods = 4;
    for agent in &mut scenario.agents {
        if agent.labor_capacity > 0 {
            agent.scale = vec![
                want(WantKind::Leisure, Horizon::Now),
                want(WantKind::Good(GOLD), Horizon::Now),
            ];
        }
    }
    let mut society = Society::from_scenario(scenario);

    society.run(4);

    assert!(society.labor_trades.is_empty());
}

#[test]
fn project_bids_do_not_read_metrics() {
    let manifest_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
    let files = [
        "src/agent.rs",
        "src/capital.rs",
        "src/factor.rs",
        "src/timemarket.rs",
        "src/bundle.rs",
    ];
    let forbidden = [
        "metrics::",
        "natural_rate",
        "market_rate",
        "rate_gap",
        "structure_length",
    ];
    let mut matches = Vec::new();

    for file in files {
        let source = fs::read_to_string(manifest_dir.join(file)).expect("source file is readable");
        for (line_index, line) in source.lines().enumerate() {
            if forbidden.iter().any(|needle| line.contains(needle)) {
                matches.push(format!("{file}:{}:{line}", line_index + 1));
            }
        }
    }

    assert!(matches.is_empty(), "{}", matches.join("\n"));
}

#[test]
fn sound_money_has_no_sustained_positive_gap() {
    let society = run(ScenarioName::SoundMoney100Pct);
    let mut streak = 0;

    for record in &society.m2_records {
        let active = record.loan_trades > 0 || record.natural_rate_proxy_bps.is_some();
        if active && record.rate_gap_bps.unwrap_or(0) > 0 {
            streak += 1;
        } else if active {
            streak = 0;
        }
        assert!(streak < 3);
    }
}

#[test]
fn sound_money_has_no_credit_cycle_bust() {
    let society = run(ScenarioName::SoundMoney100Pct);

    assert!(society
        .m2_records
        .iter()
        .all(|record| record.abandoned_projects == 0));
    assert!(society
        .m2_records
        .iter()
        .all(|record| record.capital_labor_consumed == 0));
    assert!(society
        .m2_records
        .iter()
        .all(|record| record.capital_goods_consumed == 0));
    assert!(society
        .m2_records
        .iter()
        .all(|record| record.debts_defaulted == 0));
    assert!(society
        .m2_records
        .iter()
        .any(|record| record.sold_projects > 0));
    assert!(society
        .m2_records
        .last()
        .is_some_and(|record| record.active_projects == 0 && record.sold_projects > 0));
}

#[test]
fn abandoned_m2_project_records_capital_loss() {
    let mut scenario = builtin_market_scenario(ScenarioName::RoundaboutCapital);
    scenario.periods = 1;
    let owner = scenario
        .agents
        .iter()
        .find(|agent| {
            agent
                .roles
                .iter()
                .any(|role| matches!(role, econ::agent::Role::Capitalist))
        })
        .map(|agent| agent.id)
        .expect("roundabout scenario has a capitalist");
    let mut society = Society::from_scenario(scenario);
    society.m2_projects.push(M2Project {
        id: M2ProjectId(99),
        owner,
        line: ProjectLineId(2),
        state: M2ProjectState::Forming,
        started_at: Tick(0),
        maturity: Tick(0),
        labor_advanced: 1,
        input_goods_committed: Vec::new(),
        input_cost_basis: Gold::ZERO,
        advanced_gold: Gold(2),
        expected_revenue: Gold(1),
        output_good: FOOD,
        output_qty: 9,
        salvage_bps: 5000,
    });

    society.step();
    let record = society.m2_records.last().expect("M2 record");

    assert!(record.abandoned_projects >= 1);
    assert!(record.capital_labor_consumed >= 1);
    assert!(record.capital_gold_loss >= Gold(2));
}

#[test]
fn m2_runs_are_deterministic() {
    let a = run(ScenarioName::SoundMoney100Pct);
    let b = run(ScenarioName::SoundMoney100Pct);

    assert_eq!(a.m2_records, b.m2_records);
    assert_eq!(a.trades, b.trades);
    assert_eq!(a.loan_trades, b.loan_trades);
    assert_eq!(a.labor_trades, b.labor_trades);
}

#[test]
fn golden_m2_sound_money_series_is_stable() {
    let society = run(ScenarioName::SoundMoney100Pct);

    assert_eq!(fnv1a_m2(&society), 10_330_696_908_468_943_666);
}

fn want(kind: WantKind, horizon: Horizon) -> Want {
    Want {
        kind,
        horizon,
        qty: 1,
        satisfied: false,
    }
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

fn gold_for(society: &Society, agent: AgentId) -> Gold {
    society
        .agents
        .iter()
        .find(|entry| entry.id == agent)
        .map(|entry| entry.gold)
        .unwrap_or(Gold::ZERO)
}

fn fnv1a_m2(society: &Society) -> u64 {
    let mut hash = 0xcbf2_9ce4_8422_2325;
    for record in &society.m2_records {
        hash_record(&mut hash, record);
    }
    for trade in &society.trades {
        hash_u64(&mut hash, trade.tick);
        hash_u32(&mut hash, u32::from(trade.good.0));
        hash_u32(&mut hash, trade.buyer.index());
        hash_u32(&mut hash, trade.seller.index());
        hash_u64(&mut hash, trade.price.0);
        hash_u32(&mut hash, trade.qty);
    }
    for trade in &society.loan_trades {
        hash_u64(&mut hash, trade.tick);
        hash_u32(
            &mut hash,
            trade
                .lender
                .agent()
                .map(|agent| agent.index())
                .unwrap_or_default(),
        );
        hash_u32(&mut hash, trade.borrower.index());
        hash_u64(&mut hash, trade.present.0);
        hash_u64(&mut hash, trade.future_due.0);
        hash_u32(&mut hash, u32::from(trade.horizon));
        hash_u64(&mut hash, trade.debt.0);
        hash_loan_purpose(&mut hash, &trade.purpose);
        hash_option_u64(&mut hash, trade.project.map(|project| project.0));
    }
    for trade in &society.labor_trades {
        hash_u64(&mut hash, trade.tick);
        hash_u32(&mut hash, trade.employer.index());
        hash_u32(&mut hash, trade.worker.index());
        hash_u64(&mut hash, trade.wage.0);
        hash_u32(&mut hash, trade.qty);
        hash_u64(&mut hash, trade.project.0);
    }
    hash
}

fn hash_record(hash: &mut u64, record: &M2Record) {
    hash_u64(hash, record.tick);
    hash_u64(hash, record.total_gold.0);
    hash_u32(hash, record.spot_trades);
    hash_u32(hash, record.labor_trades);
    hash_u32(hash, record.loan_trades);
    hash_u32(hash, record.project_loan_trades);
    hash_u64(hash, record.project_borrowed_gold.0);
    hash_u32(hash, record.debts_open);
    hash_u32(hash, record.debts_settled);
    hash_u32(hash, record.debts_defaulted);
    hash_u32(hash, record.project_debts_open);
    hash_u32(hash, record.project_debts_settled);
    hash_u32(hash, record.project_debts_defaulted);
    hash_u64(hash, record.project_funding_reserved_gold.0);
    hash_u32(hash, record.active_projects);
    hash_u32(hash, record.waiting_projects);
    hash_u32(hash, record.mature_projects);
    hash_u32(hash, record.sold_projects);
    hash_u32(hash, record.abandoned_projects);
    hash_u32(hash, record.labor_advanced);
    hash_u64(hash, record.wages_paid.0);
    hash_u64(hash, record.project_revenue.0);
    hash_i64(hash, record.project_profit);
    hash_u32(hash, record.capital_labor_consumed);
    hash_u32(hash, record.capital_goods_consumed);
    hash_u64(hash, record.capital_gold_loss.0);
    hash_option_i64(hash, record.market_rate_bps);
    hash_option_i64(hash, record.natural_rate_proxy_bps);
    hash_option_i64(hash, record.rate_gap_bps);
    hash_u64(hash, record.structure_length_ticks_x100);
}

fn hash_option_i64(hash: &mut u64, value: Option<i64>) {
    match value {
        Some(value) => {
            hash_u32(hash, 1);
            hash_i64(hash, value);
        }
        None => hash_u32(hash, 0),
    }
}

fn hash_option_u64(hash: &mut u64, value: Option<u64>) {
    match value {
        Some(value) => {
            hash_u32(hash, 1);
            hash_u64(hash, value);
        }
        None => hash_u32(hash, 0),
    }
}

fn hash_loan_purpose(hash: &mut u64, purpose: &econ::purpose::LoanPurpose) {
    match purpose {
        econ::purpose::LoanPurpose::Consumption => hash_u32(hash, 0),
        econ::purpose::LoanPurpose::ProjectFunding(plan) => {
            hash_u32(hash, 1);
            hash_u64(hash, plan.0);
        }
    }
}

fn hash_i64(hash: &mut u64, value: i64) {
    for byte in value.to_le_bytes() {
        *hash ^= u64::from(byte);
        *hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
    }
}

fn hash_u32(hash: &mut u64, value: u32) {
    for byte in value.to_le_bytes() {
        *hash ^= u64::from(byte);
        *hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
    }
}

fn hash_u64(hash: &mut u64, value: u64) {
    for byte in value.to_le_bytes() {
        *hash ^= u64::from(byte);
        *hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
    }
}
