use econ::agent::{Agent, AgentId, Want, WantKind};
use econ::good::{Gold, GoodId, Horizon, Stock, GOLD};
use econ::ledger::BankId;
use econ::money::{Regime, ReserveRatioBps};
use econ::purpose::CreditSource;
use econ::record::{BankAuditRecord, M3Record};
use econ::scenario::{
    builtin_market_scenario, emerged_gold_bridge_agents, EventKind, MarketScenario, ScenarioKind,
    ScenarioName,
};
use econ::shadow::{credit_disabled_scenario, run_credit_disabled_shadow};
use econ::society::Society;
use econ::sweep::{apply_sweep_values, SweepKey};

const BRIDGE_GOLD: Gold = Gold(16);
const M9_CONVERTIBLE_CAP: Gold = Gold(4);
const SUSPENSION_TICK: u64 = 4;

#[derive(Clone, Debug, PartialEq, Eq)]
struct SeedRow {
    agent: AgentId,
    gold: Gold,
    stock: Vec<(GoodId, u32)>,
    scale: Vec<(WantKind, Horizon, u32)>,
}

#[test]
fn m9_scenarios_preserve_m6_bridge_prefix() {
    let bridge = emerged_gold_bridge_agents();
    let expected = seed_rows(&bridge);

    assert_eq!(bridge_gold(&bridge), BRIDGE_GOLD);
    assert!(bridge.iter().all(|agent| agent.stock.get(GOLD) == 0));

    for name in m9_scenarios() {
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
fn m9_scenarios_account_bank_reserve_specie_separately() {
    for name in m9_scenarios() {
        let scenario = builtin_market_scenario(name);
        let total_agent_gold = total_agent_gold(&scenario.agents);
        let society = Society::from_scenario(scenario);
        let initial_bank_reserves = society.banks.iter().fold(Gold::ZERO, |total, bank| {
            total.saturating_add(bank.reserves)
        });
        let money_system = society
            .money_system
            .as_ref()
            .expect("M9 scenario has a money system");
        let snapshot = money_system.snapshot();

        assert_eq!(society.banks.len(), 1);
        assert!(initial_bank_reserves > Gold::ZERO);
        assert!(society.issuers.is_empty());
        assert_eq!(snapshot.public_fiat, Gold::ZERO);
        assert_eq!(snapshot.demand_claims, Gold::ZERO);
        assert_eq!(snapshot.bank_reserves, initial_bank_reserves);
        assert_eq!(snapshot.public_specie, total_agent_gold);
        assert_eq!(
            snapshot
                .public_specie
                .saturating_add(snapshot.bank_reserves),
            money_system.base.commodity_base
        );
        assert_eq!(snapshot.tms(), total_agent_gold);
    }
}

#[test]
fn reserve_leash_control_starts_sound_then_fractional() {
    let scenario = builtin_market_scenario(ScenarioName::EmergedGoldReserveLeashControl);
    assert_eq!(scenario.scenario.kind(), ScenarioKind::MarketM3);
    assert_eq!(scenario.scenario.regime(), Regime::SoundGold);
    assert!(scenario.events.iter().any(|event| matches!(
        &event.kind,
        EventKind::SetRegime(Regime::FractionalConvertible)
    )));
    assert!(scenario.events.iter().all(|event| !matches!(
        &event.kind,
        EventKind::SetRegime(Regime::SuspendedConvertibility)
            | EventKind::SetBankConvertibility {
                convertible: false,
                ..
            }
    )));

    let mut society = Society::from_scenario(scenario);
    assert_eq!(society.regime(), Regime::SoundGold);

    society.step();

    let first = society.m3_records.first().expect("tick 0 M3 record");
    assert_eq!(first.m2.tick, 0);
    assert_eq!(first.regime, Regime::FractionalConvertible);
    assert!(society.banks.iter().all(|bank| bank.convertible));
}

#[test]
fn reserve_leash_control_obeys_convertible_capacity() {
    let society = run_with_bank_audit(builtin_market_scenario(
        ScenarioName::EmergedGoldReserveLeashControl,
    ));

    assert_convertible_rows_obey_cap(&society.bank_audit);
    assert!(society
        .loan_trades
        .iter()
        .any(|trade| matches!(trade.funding, CreditSource::BankFiduciary(BankId(1)))));
    assert!(society
        .m3_records
        .iter()
        .any(|record| record.bank_credit_issued > Gold::ZERO));

    let final_row = society.bank_audit.last().expect("bank audit row");
    assert_eq!(convertible_cap(final_row), M9_CONVERTIBLE_CAP);
    assert_eq!(final_row.demand_deposits, M9_CONVERTIBLE_CAP);
}

#[test]
fn reserve_leash_control_has_no_fiat() {
    let scenario = builtin_market_scenario(ScenarioName::EmergedGoldReserveLeashControl);
    assert!(scenario
        .events
        .iter()
        .all(|event| !matches!(&event.kind, EventKind::FiatPrint { .. })));
    let society = run_with_bank_audit(scenario);

    assert!(society.issuers.is_empty());
    assert!(society.m3_records.iter().all(no_fiat_record));
}

#[test]
fn emerged_gold_suspension_starts_sound_then_fractional_then_suspended() {
    let scenario = builtin_market_scenario(ScenarioName::EmergedGoldSuspensionOfConvertibility);
    assert_eq!(scenario.scenario.kind(), ScenarioKind::MarketM3);
    assert_eq!(scenario.scenario.regime(), Regime::SoundGold);

    let periods = scenario.periods;
    let mut society = Society::from_scenario(scenario);
    society.enable_bank_audit();
    assert_eq!(society.regime(), Regime::SoundGold);
    society.run(periods);

    assert!(society
        .m3_records
        .iter()
        .filter(|record| record.m2.tick < SUSPENSION_TICK)
        .all(|record| record.regime == Regime::FractionalConvertible));
    assert!(society
        .m3_records
        .iter()
        .filter(|record| record.m2.tick >= SUSPENSION_TICK)
        .all(|record| record.regime == Regime::SuspendedConvertibility));
    assert!(society
        .bank_audit
        .iter()
        .filter(|row| row.tick < SUSPENSION_TICK)
        .all(|row| row.convertible));
    assert!(society
        .bank_audit
        .iter()
        .filter(|row| row.tick >= SUSPENSION_TICK)
        .all(|row| !row.convertible));
}

#[test]
fn emerged_gold_suspension_expands_beyond_control_after_suspension() {
    let control_scenario = builtin_market_scenario(ScenarioName::EmergedGoldReserveLeashControl);
    let suspension_scenario =
        builtin_market_scenario(ScenarioName::EmergedGoldSuspensionOfConvertibility);
    assert_eq!(control_scenario.seed, suspension_scenario.seed);
    assert_eq!(control_scenario.periods, suspension_scenario.periods);

    let control = run_with_bank_audit_and_shadow(control_scenario);
    let suspension = run_with_bank_audit_and_shadow(suspension_scenario);

    let control_pre_records = control
        .m3_records
        .iter()
        .filter(|record| record.m2.tick < SUSPENSION_TICK)
        .collect::<Vec<_>>();
    let suspension_pre_records = suspension
        .m3_records
        .iter()
        .filter(|record| record.m2.tick < SUSPENSION_TICK)
        .collect::<Vec<_>>();
    assert_eq!(control_pre_records, suspension_pre_records);

    let control_pre_audit = control
        .bank_audit
        .iter()
        .filter(|row| row.tick < SUSPENSION_TICK)
        .collect::<Vec<_>>();
    let suspension_pre_audit = suspension
        .bank_audit
        .iter()
        .filter(|row| row.tick < SUSPENSION_TICK)
        .collect::<Vec<_>>();
    assert_eq!(control_pre_audit, suspension_pre_audit);

    let control_final = control.m3_records.last().expect("control records");
    let suspension_final = suspension.m3_records.last().expect("suspension records");

    assert!(suspension_final.m2.tick >= SUSPENSION_TICK);
    assert!(suspension_final.demand_claims > control_final.demand_claims);
    assert!(suspension_final.fiduciary > control_final.fiduciary);
    assert!(
        sum_bank_credit(&suspension.m3_records, |tick| tick >= SUSPENSION_TICK)
            > sum_bank_credit(&control.m3_records, |tick| tick >= SUSPENSION_TICK)
    );
    assert!(
        peak_shadow_gap(&suspension.m3_records, |tick| tick >= SUSPENSION_TICK)
            > peak_shadow_gap(&control.m3_records, |tick| tick >= SUSPENSION_TICK)
    );
}

#[test]
fn emerged_gold_suspension_cuts_the_reserve_leash() {
    let society = run_with_bank_audit(builtin_market_scenario(
        ScenarioName::EmergedGoldSuspensionOfConvertibility,
    ));

    for row in society
        .bank_audit
        .iter()
        .filter(|row| row.tick < SUSPENSION_TICK)
    {
        assert!(
            row.demand_deposits <= convertible_cap(row),
            "pre-suspension row exceeded the convertible cap: {row:?}"
        );
    }

    let mut exceeded_after_suspension = false;
    for row in &society.bank_audit {
        if row.demand_deposits > convertible_cap(row) {
            assert!(row.tick >= SUSPENSION_TICK);
            assert!(!row.convertible);
            exceeded_after_suspension = true;
        }
    }
    assert!(exceeded_after_suspension);
}

#[test]
fn emerged_gold_suspension_has_no_fiat_or_issuer() {
    let scenario = builtin_market_scenario(ScenarioName::EmergedGoldSuspensionOfConvertibility);
    assert!(scenario
        .events
        .iter()
        .all(|event| !matches!(&event.kind, EventKind::FiatPrint { .. })));
    let society = run_with_bank_audit(scenario);

    assert!(society.issuers.is_empty());
    assert!(society.m3_records.iter().all(no_fiat_record));
}

#[test]
fn emerged_gold_suspension_shadow_disables_bank_credit() {
    let scenario = builtin_market_scenario(ScenarioName::EmergedGoldSuspensionOfConvertibility);
    let neutralized = credit_disabled_scenario(&scenario);
    let periods = neutralized.periods;
    let mut society = Society::from_scenario(neutralized);
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
    assert!(society.m3_records.iter().any(|record| {
        record.m2.tick >= SUSPENSION_TICK && record.regime == Regime::SuspendedConvertibility
    }));
}

#[test]
fn emerged_gold_suspension_no_credit_no_widening() {
    let mut scenario = builtin_market_scenario(ScenarioName::EmergedGoldSuspensionOfConvertibility);
    apply_sweep_values(&mut scenario, &[(SweepKey::BankCreditPerTick, 0)]).unwrap();

    let shadow = run_credit_disabled_shadow(&scenario);
    let society = run_with_bank_audit_and_shadow(scenario);

    assert_eq!(shadow.natural_rate_bps.len(), society.m3_records.len());
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
    assert!(peak_shadow_gap(&society.m3_records, |tick| tick >= SUSPENSION_TICK) <= 0);
    assert_eq!(sum_busts(&society.m3_records), 0);
    assert_eq!(sum_labor_consumed(&society.m3_records), 0);
    assert_eq!(sum_goods_consumed(&society.m3_records), 0);
    assert!(society
        .m3_records
        .iter()
        .all(|record| record.m2.capital_gold_loss == Gold::ZERO));
}

fn m9_scenarios() -> [ScenarioName; 2] {
    [
        ScenarioName::EmergedGoldReserveLeashControl,
        ScenarioName::EmergedGoldSuspensionOfConvertibility,
    ]
}

fn run_with_bank_audit(scenario: MarketScenario) -> Society {
    let periods = scenario.periods;
    let mut society = Society::from_scenario(scenario);
    society.enable_bank_audit();
    society.run(periods);
    society
}

fn run_with_bank_audit_and_shadow(scenario: MarketScenario) -> Society {
    let shadow = run_credit_disabled_shadow(&scenario);
    let mut society = run_with_bank_audit(scenario);
    society.attach_shadow(&shadow);
    society
}

fn assert_convertible_rows_obey_cap(rows: &[BankAuditRecord]) {
    let mut saw_convertible_row = false;
    for row in rows
        .iter()
        .filter(|row| row.convertible && row.reserve_ratio_bps.0 > 0)
    {
        saw_convertible_row = true;
        assert!(
            row.demand_deposits <= convertible_cap(row),
            "convertible row exceeded reserve-ratio capacity: {row:?}"
        );
    }
    assert!(saw_convertible_row);
}

fn convertible_cap(row: &BankAuditRecord) -> Gold {
    if row.reserve_ratio_bps == ReserveRatioBps(0) {
        return Gold(u64::MAX);
    }
    let cap =
        u128::from(row.reserves.0).saturating_mul(10_000) / u128::from(row.reserve_ratio_bps.0);
    Gold(u64::try_from(cap).unwrap_or(u64::MAX))
}

fn no_fiat_record(record: &M3Record) -> bool {
    record.public_fiat == Gold::ZERO
        && record.fiat_credit_issued == Gold::ZERO
        && record.fiat_fiscal_issued == Gold::ZERO
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

fn total_agent_gold(agents: &[Agent]) -> Gold {
    agents
        .iter()
        .fold(Gold::ZERO, |total, agent| total.saturating_add(agent.gold))
}

fn bridge_gold(agents: &[Agent]) -> Gold {
    agents
        .iter()
        .filter(|agent| agent.id.0 <= 10)
        .fold(Gold::ZERO, |total, agent| total.saturating_add(agent.gold))
}

fn sum_bank_credit(records: &[M3Record], tick_filter: impl Fn(u64) -> bool) -> Gold {
    records
        .iter()
        .filter(|record| tick_filter(record.m2.tick))
        .fold(Gold::ZERO, |total, record| {
            total.saturating_add(record.bank_credit_issued)
        })
}

fn peak_shadow_gap(records: &[M3Record], tick_filter: impl Fn(u64) -> bool) -> i64 {
    records
        .iter()
        .filter(|record| tick_filter(record.m2.tick))
        .filter_map(|record| record.shadow_rate_gap_bps)
        .max()
        .unwrap_or(0)
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

fn sum_goods_consumed(records: &[M3Record]) -> u32 {
    records.iter().fold(0u32, |sum, record| {
        sum.saturating_add(record.m2.capital_goods_consumed)
    })
}
