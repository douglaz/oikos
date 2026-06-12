use econ::agent::{Agent, AgentId, Want, WantKind};
use econ::good::{Gold, GoodId, Horizon, Stock, GOLD};
use econ::ledger::BankId;
use econ::money::Regime;
use econ::purpose::CreditSource;
use econ::record::{BankAuditRecord, M3Record, RedemptionAuditRecord, RedemptionOutcome};
use econ::scenario::{builtin_market_scenario, emerged_gold_bridge_agents, ScenarioName};
use econ::shadow::credit_disabled_scenario;
use econ::society::Society;

const BRIDGE_GOLD: Gold = Gold(16);
const RUN_TICK: u64 = 4;

#[derive(Clone, Debug, PartialEq, Eq)]
struct SeedRow {
    agent: AgentId,
    gold: Gold,
    stock: Vec<(GoodId, u32)>,
    scale: Vec<(WantKind, Horizon, u32)>,
}

#[test]
fn m10_scenarios_preserve_m6_bridge_prefix() {
    let bridge = emerged_gold_bridge_agents();
    let expected = seed_rows(&bridge);

    assert_eq!(bridge_gold(&bridge), BRIDGE_GOLD);
    assert!(bridge.iter().all(|agent| agent.stock.get(GOLD) == 0));

    for name in m10_scenarios() {
        let scenario = builtin_market_scenario(name);
        let prefix = scenario
            .agents
            .iter()
            .take(bridge.len())
            .cloned()
            .collect::<Vec<_>>();

        assert_eq!(
            seed_rows(&prefix),
            expected,
            "{name:?} bridge prefix changed"
        );
        assert_eq!(
            bridge_gold(&prefix),
            BRIDGE_GOLD,
            "{name:?} bridge gold changed"
        );
        assert!(
            prefix.iter().all(|agent| agent.stock.get(GOLD) == 0),
            "{name:?} bridge prefix should carry no stock gold"
        );
        assert!(
            scenario
                .agents
                .iter()
                .skip(bridge.len())
                .all(|agent| agent.id.0 > 10),
            "{name:?} add-on ids must not collide with the bridge"
        );
    }
}

#[test]
fn m10_scenarios_have_bank_no_issuer_no_fiat() {
    for name in m10_scenarios() {
        let scenario = builtin_market_scenario(name);
        let mut society = Society::from_scenario(scenario);
        assert_eq!(society.banks.len(), 1);
        assert_eq!(society.banks[0].reserves, Gold(2));
        assert!(society.issuers.is_empty());

        let periods = society_periods(name);
        society.run(periods);

        assert!(society.m3_records.iter().all(|record| {
            record.public_fiat == Gold::ZERO
                && record.fiat_credit_issued == Gold::ZERO
                && record.fiat_fiscal_issued == Gold::ZERO
        }));
    }
}

#[test]
fn redemption_run_matches_m9_prefix_until_run_tick() {
    let control = run_with_audits(ScenarioName::EmergedGoldReserveLeashControl);
    let redemption = run_with_audits(ScenarioName::EmergedGoldRedemptionRun);

    assert_eq!(
        m3_prefix(&control.m3_records, RUN_TICK),
        m3_prefix(&redemption.m3_records, RUN_TICK)
    );
    assert_eq!(
        bank_prefix(&control.bank_audit, RUN_TICK),
        bank_prefix(&redemption.bank_audit, RUN_TICK)
    );
}

#[test]
fn redemption_run_exhausts_reserves_under_convertibility() {
    let society = run_with_audits(ScenarioName::EmergedGoldRedemptionRun);
    let pre_run = record_at(&society.m3_records, RUN_TICK - 1);
    let final_record = record_at(&society.m3_records, RUN_TICK);
    let final_bank = bank_at(&society.bank_audit, RUN_TICK);

    assert_eq!(sum_requested(&society.redemption_audit), Gold(4));
    assert_eq!(sum_honored(&society.redemption_audit), Gold(2));
    assert_eq!(sum_failed(&society.redemption_audit), Gold(2));
    assert!(society.redemption_audit.iter().any(|row| matches!(
        row.outcome,
        RedemptionOutcome::PartiallyHonored | RedemptionOutcome::ReserveExhausted
    )));
    assert_eq!(final_bank.reserves, Gold::ZERO);
    assert_eq!(final_record.bank_reserves, Gold::ZERO);
    assert_eq!(final_record.demand_claims, Gold(2));
    assert_eq!(
        final_record.public_specie,
        pre_run.public_specie.saturating_add(Gold(2))
    );
    assert_eq!(
        final_record
            .public_specie
            .saturating_add(final_record.bank_reserves),
        pre_run.public_specie.saturating_add(pre_run.bank_reserves)
    );
}

#[test]
fn suspended_redemption_refuses_same_requests() {
    let society = run_with_audits(ScenarioName::EmergedGoldSuspendedRedemption);
    let final_record = record_at(&society.m3_records, RUN_TICK);
    let final_bank = bank_at(&society.bank_audit, RUN_TICK);

    assert_eq!(sum_requested(&society.redemption_audit), Gold(4));
    assert_eq!(sum_honored(&society.redemption_audit), Gold::ZERO);
    assert_eq!(sum_failed(&society.redemption_audit), Gold(4));
    assert!(society
        .redemption_audit
        .iter()
        .all(|row| row.outcome == RedemptionOutcome::Suspended));
    assert_eq!(final_bank.reserves, Gold(2));
    assert_eq!(final_record.bank_reserves, Gold(2));
    assert_eq!(final_record.demand_claims, Gold(4));
    assert_eq!(final_record.regime, Regime::SuspendedConvertibility);
    assert!(!final_bank.policy_enabled);
}

#[test]
fn redemption_and_suspension_diverge_only_at_run_tick() {
    let run = run_with_audits(ScenarioName::EmergedGoldRedemptionRun);
    let suspended = run_with_audits(ScenarioName::EmergedGoldSuspendedRedemption);

    assert_eq!(
        m3_prefix(&run.m3_records, RUN_TICK),
        m3_prefix(&suspended.m3_records, RUN_TICK)
    );
    assert_eq!(
        bank_prefix(&run.bank_audit, RUN_TICK),
        bank_prefix(&suspended.bank_audit, RUN_TICK)
    );

    let run_tick = record_at(&run.m3_records, RUN_TICK);
    let suspended_tick = record_at(&suspended.m3_records, RUN_TICK);
    assert!(run_tick.public_specie > suspended_tick.public_specie);
    assert!(run_tick.bank_reserves < suspended_tick.bank_reserves);
    assert!(run_tick.demand_claims < suspended_tick.demand_claims);
}

#[test]
fn redemption_shadow_has_no_claims_to_redeem() {
    for name in m10_scenarios() {
        let scenario = credit_disabled_scenario(&builtin_market_scenario(name));
        let periods = scenario.periods;
        let mut society = Society::from_scenario(scenario);

        society.run(periods);

        assert!(society.m3_records.iter().all(|record| {
            record.bank_credit_issued == Gold::ZERO
                && record.bank_loan_trades == 0
                && record.demand_claims == Gold::ZERO
                && record.fiduciary == Gold::ZERO
        }));
        assert!(society
            .loan_trades
            .iter()
            .all(|trade| !matches!(trade.funding, CreditSource::BankFiduciary(_))));
        assert!(society
            .redemption_audit
            .iter()
            .all(|row| row.honored == Gold::ZERO));
        if name == ScenarioName::EmergedGoldSuspendedRedemption {
            assert!(society.m3_records.iter().any(|record| {
                record.m2.tick == RUN_TICK && record.regime == Regime::SuspendedConvertibility
            }));
        }
    }
}

fn m10_scenarios() -> [ScenarioName; 2] {
    [
        ScenarioName::EmergedGoldRedemptionRun,
        ScenarioName::EmergedGoldSuspendedRedemption,
    ]
}

fn society_periods(name: ScenarioName) -> u64 {
    builtin_market_scenario(name).periods
}

fn run_with_audits(name: ScenarioName) -> Society {
    let scenario = builtin_market_scenario(name);
    let periods = scenario.periods;
    let mut society = Society::from_scenario(scenario);
    society.enable_bank_audit();
    society.enable_money_audit();
    for tick in 0..periods {
        society.step();
        assert!(
            society.money_ledgers_reconcile(),
            "{name:?} ledgers failed to reconcile at tick {tick}"
        );
    }
    society
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
        .find(|record| record.tick == tick && record.bank == BankId(1))
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
