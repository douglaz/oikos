use econ::agent::{Agent, AgentId, Want, WantKind};
use econ::good::{Gold, GoodId, Horizon, Stock, GOLD};
use econ::ledger::{BankId, IssuerId};
use econ::money::{Regime, ReserveRatioBps};
use econ::purpose::CreditSource;
use econ::record::M3Record;
use econ::scenario::{
    builtin_market_scenario, emerged_gold_bridge_agents, EventKind, ScenarioKind, ScenarioName,
};
use econ::shadow::{credit_disabled_scenario, run_credit_disabled_shadow};
use econ::society::{run_m3_with_shadow, Society};
use econ::sweep::{apply_sweep_values, SweepKey};

const BRIDGE_GOLD: Gold = Gold(16);

#[derive(Clone, Debug, PartialEq, Eq)]
struct SeedRow {
    agent: AgentId,
    gold: Gold,
    stock: Vec<(GoodId, u32)>,
    scale: Vec<(WantKind, Horizon, u32)>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct BoomBustEvidence {
    credit_tick: u64,
    gap_tick: u64,
    structure_tick: u64,
    stop_tick: u64,
    bust_tick: u64,
    consumed_tick: u64,
    bad_debt_or_retirement_tick: u64,
}

#[test]
fn m8_credit_scenarios_preserve_m6_bridge_prefix() {
    let bridge = emerged_gold_bridge_agents();
    let expected = seed_rows(&bridge);

    assert_eq!(bridge_gold(&bridge), BRIDGE_GOLD);
    assert!(bridge.iter().all(|agent| agent.stock.get(GOLD) == 0));

    for name in m8_scenarios() {
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
fn m8_credit_scenarios_report_addon_specie_honestly() {
    for (name, expected_addon) in [
        (ScenarioName::EmergedGoldFractionalReserve, Gold(39)),
        (ScenarioName::EmergedGoldFiatCreditExpansion, Gold(78)),
    ] {
        let scenario = builtin_market_scenario(name);
        let total_initial_gold = total_agent_gold(&scenario.agents);
        let addon_specie = addon_gold(&scenario.agents);

        assert_eq!(total_initial_gold, BRIDGE_GOLD.saturating_add(addon_specie));
        assert_eq!(
            addon_specie, expected_addon,
            "{name:?} add-on specie split should be explicit"
        );
        assert!(addon_specie > Gold::ZERO);
    }
}

#[test]
fn emerged_gold_fractional_reserve_starts_sound_then_switches_to_fractional() {
    let scenario = builtin_market_scenario(ScenarioName::EmergedGoldFractionalReserve);
    assert_eq!(scenario.scenario.kind(), ScenarioKind::MarketM3);
    assert_eq!(scenario.scenario.regime(), Regime::SoundGold);
    assert!(scenario.events.iter().any(|event| matches!(
        &event.kind,
        EventKind::SetRegime(Regime::FractionalConvertible)
    )));
    assert!(scenario.events.iter().any(|event| matches!(
        &event.kind,
        EventKind::SetReserveRatio {
            bank: BankId(1),
            ratio: ReserveRatioBps(0),
        }
    )));
    assert!(scenario.events.iter().any(|event| matches!(
        &event.kind,
        EventKind::SetBankCreditPolicy {
            bank: BankId(1),
            policy,
        } if policy.enabled
    )));

    let mut society = Society::from_scenario(scenario);
    assert_eq!(society.regime(), Regime::SoundGold);
    assert_eq!(society.banks.len(), 1);
    assert!(society.issuers.is_empty());

    society.step();

    let first = society.m3_records.first().expect("tick 0 M3 record");
    assert_eq!(first.m2.tick, 0);
    assert_eq!(first.regime, Regime::FractionalConvertible);
}

#[test]
fn emerged_gold_fractional_reserve_issues_fiduciary_claims() {
    let society = run_m3_with_shadow(builtin_market_scenario(
        ScenarioName::EmergedGoldFractionalReserve,
    ));

    assert!(society
        .loan_trades
        .iter()
        .any(|trade| matches!(trade.funding, CreditSource::BankFiduciary(BankId(1)))));
    assert!(society
        .m3_records
        .iter()
        .any(|record| record.bank_credit_issued > Gold::ZERO));
    assert!(society
        .m3_records
        .iter()
        .any(|record| record.demand_claims > Gold::ZERO));
    assert!(society
        .m3_records
        .iter()
        .any(|record| record.fiduciary > Gold::ZERO));
    assert!(society
        .m3_records
        .iter()
        .all(|record| record.public_fiat == Gold::ZERO));
    assert!(society.m3_records.iter().all(|record| {
        record.tms == record.public_specie.saturating_add(record.demand_claims)
            && record.fiduciary <= record.demand_claims
    }));
}

#[test]
fn emerged_gold_fractional_reserve_opens_shadow_gap_on_bank_credit() {
    let society = run_m3_with_shadow(builtin_market_scenario(
        ScenarioName::EmergedGoldFractionalReserve,
    ));

    let active_positive_gap_ticks = society
        .m3_records
        .iter()
        .filter(|record| {
            record.bank_credit_issued > Gold::ZERO && record.shadow_rate_gap_bps.unwrap_or(0) > 0
        })
        .count();
    assert!(
        active_positive_gap_ticks >= 2,
        "expected at least two active bank-credit ticks with positive shadow gap"
    );

    let control = run_m3_with_shadow(builtin_market_scenario(
        ScenarioName::EmergedGoldSoundControl,
    ));
    assert!(!has_sustained_positive_shadow_gap(&control.m3_records));
}

#[test]
fn emerged_gold_fractional_bank_credit_first_receivers_are_tagged() {
    let society = run_m3_with_shadow(builtin_market_scenario(
        ScenarioName::EmergedGoldFractionalReserve,
    ));

    let bank_receipts = society
        .cantillon_receipts
        .iter()
        .filter(|receipt| matches!(receipt.source, CreditSource::BankFiduciary(_)))
        .collect::<Vec<_>>();
    assert!(!bank_receipts.is_empty());
    assert!(bank_receipts
        .iter()
        .all(|receipt| receipt.amount > Gold::ZERO));
}

#[test]
fn emerged_gold_fiat_credit_starts_sound_then_switches_to_fiat() {
    let scenario = builtin_market_scenario(ScenarioName::EmergedGoldFiatCreditExpansion);
    assert_eq!(scenario.scenario.kind(), ScenarioKind::MarketM3);
    assert_eq!(scenario.scenario.regime(), Regime::SoundGold);
    assert!(scenario
        .events
        .iter()
        .any(|event| matches!(&event.kind, EventKind::SetRegime(Regime::Fiat))));
    assert!(scenario.events.iter().any(|event| matches!(
        &event.kind,
        EventKind::SetIssuerPolicy {
            issuer: IssuerId(1),
            policy,
        } if policy.credit_enabled
            && !policy.fiscal_enabled
            && policy.max_fiscal_issue_per_tick == Gold::ZERO
    )));
    assert!(scenario
        .events
        .iter()
        .all(|event| !matches!(&event.kind, EventKind::FiatPrint { .. })));

    let mut society = Society::from_scenario(scenario);
    assert_eq!(society.regime(), Regime::SoundGold);
    assert!(society.banks.is_empty());
    assert_eq!(society.issuers.len(), 1);

    society.step();

    let first = society.m3_records.first().expect("tick 0 M3 record");
    assert_eq!(first.m2.tick, 0);
    assert_eq!(first.regime, Regime::Fiat);
}

#[test]
fn emerged_gold_fiat_credit_is_credit_not_fiscal_printing() {
    let scenario = builtin_market_scenario(ScenarioName::EmergedGoldFiatCreditExpansion);
    let initial_gold = total_agent_gold(&scenario.agents);
    let society = run_m3_with_shadow(scenario);

    assert!(society
        .loan_trades
        .iter()
        .any(|trade| matches!(trade.funding, CreditSource::FiatCredit(IssuerId(1)))));
    assert!(society
        .m3_records
        .iter()
        .any(|record| record.fiat_credit_issued > Gold::ZERO));
    assert!(society
        .m3_records
        .iter()
        .any(|record| record.public_fiat > Gold::ZERO));
    assert!(society.m3_records.iter().all(|record| {
        record.fiat_fiscal_issued == Gold::ZERO
            && record.demand_claims == Gold::ZERO
            && record.fiduciary == Gold::ZERO
            && record.tms == record.public_specie.saturating_add(record.public_fiat)
            && record.public_specie.saturating_add(record.bank_reserves) == initial_gold
    }));
}

#[test]
fn emerged_gold_fiat_credit_first_receivers_are_tagged() {
    let society = run_m3_with_shadow(builtin_market_scenario(
        ScenarioName::EmergedGoldFiatCreditExpansion,
    ));

    let fiat_credit_receipts = society
        .cantillon_receipts
        .iter()
        .filter(|receipt| matches!(receipt.source, CreditSource::FiatCredit(_)))
        .collect::<Vec<_>>();
    assert!(!fiat_credit_receipts.is_empty());
    assert!(fiat_credit_receipts
        .iter()
        .all(|receipt| receipt.amount > Gold::ZERO));
}

#[test]
fn emerged_gold_fiat_credit_expands_then_stops_then_busts() {
    let evidence = fiat_credit_boom_bust_evidence(ScenarioName::EmergedGoldFiatCreditExpansion);

    assert!(evidence.credit_tick <= evidence.gap_tick);
    assert!(evidence.gap_tick <= evidence.structure_tick);
    assert!(evidence.structure_tick < evidence.stop_tick);
    assert!(evidence.stop_tick < evidence.bust_tick);
    assert!(evidence.bust_tick <= evidence.consumed_tick);
    assert!(evidence.stop_tick <= evidence.bad_debt_or_retirement_tick);
}

#[test]
fn emerged_gold_fiat_credit_shadow_disables_issuer_credit() {
    let scenario = builtin_market_scenario(ScenarioName::EmergedGoldFiatCreditExpansion);
    assert!(scenario
        .events
        .iter()
        .all(|event| !matches!(&event.kind, EventKind::FiatPrint { .. })));

    let neutralized = credit_disabled_scenario(&scenario);
    assert!(neutralized
        .events
        .iter()
        .all(|event| !matches!(&event.kind, EventKind::FiatPrint { .. })));
    assert!(neutralized.events.iter().any(|event| matches!(
        &event.kind,
        EventKind::SetIssuerPolicy {
            issuer: IssuerId(1),
            policy,
        } if !policy.credit_enabled
            && !policy.fiscal_enabled
            && policy.max_credit_issue_per_tick == Gold::ZERO
            && policy.max_fiscal_issue_per_tick == Gold::ZERO
    )));
    assert!(neutralized.events.iter().all(|event| match &event.kind {
        EventKind::SetIssuerPolicy { policy, .. } =>
            !policy.credit_enabled && !policy.fiscal_enabled,
        _ => true,
    }));

    let periods = neutralized.periods;
    let mut society = Society::from_scenario(neutralized);
    society.run(periods);

    assert!(society.m3_records.iter().all(|record| {
        record.fiat_credit_issued == Gold::ZERO
            && record.fiat_fiscal_issued == Gold::ZERO
            && record.fiat_loan_trades == 0
    }));
    assert!(society
        .loan_trades
        .iter()
        .all(|trade| !matches!(trade.funding, CreditSource::FiatCredit(_))));
}

#[test]
fn emerged_gold_fiat_credit_no_bust_without_credit() {
    let mut scenario = builtin_market_scenario(ScenarioName::EmergedGoldFiatCreditExpansion);
    apply_sweep_values(&mut scenario, &[(SweepKey::IssuerCreditPerTick, 0)]).unwrap();

    let society = run_m3_with_shadow(scenario);

    assert!(society
        .m3_records
        .iter()
        .all(|record| record.fiat_credit_issued == Gold::ZERO && record.fiat_loan_trades == 0));
    assert_eq!(sum_busts(&society.m3_records), 0);
    assert_eq!(sum_labor_consumed(&society.m3_records), 0);
    assert_eq!(sum_goods_consumed(&society.m3_records), 0);
}

fn m8_scenarios() -> [ScenarioName; 2] {
    [
        ScenarioName::EmergedGoldFractionalReserve,
        ScenarioName::EmergedGoldFiatCreditExpansion,
    ]
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

fn addon_gold(agents: &[Agent]) -> Gold {
    total_agent_gold(agents).saturating_sub(BRIDGE_GOLD)
}

fn fiat_credit_boom_bust_evidence(name: ScenarioName) -> BoomBustEvidence {
    let scenario = builtin_market_scenario(name);
    let stop_tick = scenario
        .events
        .iter()
        .find_map(|event| match &event.kind {
            EventKind::StopIssuerCredit { .. } => Some(event.tick.0),
            _ => None,
        })
        .expect("fiat-credit scenario has a stop event");
    let shadow = run_credit_disabled_shadow(&scenario);
    let society = run_m3_with_shadow(scenario);
    let records = &society.m3_records;

    let credit_tick = records
        .iter()
        .find(|record| record.fiat_credit_issued > Gold::ZERO)
        .map(|record| record.m2.tick)
        .expect("created credit rises");
    let gap_tick = records
        .iter()
        .find(|record| record.shadow_rate_gap_bps.unwrap_or(0) > 0)
        .map(|record| record.m2.tick)
        .expect("shadow gap opens");
    assert!(
        gap_tick < stop_tick,
        "shadow gap should open before credit stops"
    );

    let structure_tick = records
        .iter()
        .enumerate()
        .find(|(index, record)| {
            record.m2.tick < stop_tick
                && record.m2.structure_length_ticks_x100
                    > shadow
                        .structure_length_ticks_x100
                        .get(*index)
                        .copied()
                        .unwrap_or(0)
        })
        .map(|(_, record)| record.m2.tick)
        .expect("boom structure rises above shadow baseline");
    let bust_tick = records
        .iter()
        .find(|record| record.m2.tick > stop_tick && record.bust_abandoned_projects > 0)
        .map(|record| record.m2.tick)
        .expect("long project is abandoned after stop");
    let consumed_tick = records
        .iter()
        .find(|record| {
            record.m2.tick >= bust_tick
                && (record.m2.capital_labor_consumed > 0 || record.m2.capital_goods_consumed > 0)
        })
        .map(|record| record.m2.tick)
        .expect("abandonment consumes real capital");
    let bad_debt_or_retirement_tick = records
        .iter()
        .find(|record| {
            record.m2.tick >= stop_tick
                && (record.m2.debts_defaulted > 0 || record.credit_retired > Gold::ZERO)
        })
        .map(|record| record.m2.tick)
        .expect("stopped credit ends in repayment retirement or bad debt");

    assert!(records
        .iter()
        .filter(|record| record.m2.tick >= stop_tick)
        .all(|record| record.fiat_credit_issued == Gold::ZERO));

    BoomBustEvidence {
        credit_tick,
        gap_tick,
        structure_tick,
        stop_tick,
        bust_tick,
        consumed_tick,
        bad_debt_or_retirement_tick,
    }
}

fn has_sustained_positive_shadow_gap(records: &[M3Record]) -> bool {
    let mut run = 0u32;
    for record in records {
        if record.shadow_rate_gap_bps.unwrap_or(0) > 0 {
            run = run.saturating_add(1);
            if run >= 2 {
                return true;
            }
        } else {
            run = 0;
        }
    }
    false
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
