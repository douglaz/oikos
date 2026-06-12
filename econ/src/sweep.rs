//! Deterministic parameter sweeps over built-in M3 scenario knobs.

use crate::good::Gold;
use crate::ledger::IssuerId;
use crate::metrics::build_m4_records;
use crate::money::ReserveRatioBps;
use crate::record::M4Record;
use crate::scenario::{EventKind, MarketScenario};
use crate::society::run_m3_with_shadow;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SweepKey {
    IssuerCreditPerTick,
    BankCreditPerTick,
    ReserveRatioBps,
    FiscalPrint,
}

impl SweepKey {
    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "issuer-credit-per-tick" => Some(Self::IssuerCreditPerTick),
            "bank-credit-per-tick" => Some(Self::BankCreditPerTick),
            "reserve-ratio-bps" => Some(Self::ReserveRatioBps),
            "fiscal-print" => Some(Self::FiscalPrint),
            _ => None,
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::IssuerCreditPerTick => "issuer-credit-per-tick",
            Self::BankCreditPerTick => "bank-credit-per-tick",
            Self::ReserveRatioBps => "reserve-ratio-bps",
            Self::FiscalPrint => "fiscal-print",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SweepAxis {
    pub key: SweepKey,
    pub values: Vec<u64>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SweepRecord {
    pub scenario: &'static str,
    pub seed: u64,
    pub periods: u64,
    pub variables: Vec<(SweepKey, u64)>,
    pub final_tms: Gold,
    pub final_fiduciary: Gold,
    pub final_public_fiat: Gold,
    pub total_bank_credit_issued: Gold,
    pub total_fiat_credit_issued: Gold,
    pub total_fiat_fiscal_issued: Gold,
    pub total_credit_retired: Gold,
    pub max_shadow_rate_gap_bps: Option<i64>,
    pub max_structure_length_ticks_x100: u64,
    pub total_bust_abandoned_projects: u32,
    pub final_abandoned_projects: u32,
    pub final_debts_defaulted: u32,
    pub final_project_debts_defaulted: u32,
    pub final_capital_labor_consumed: u32,
    pub final_capital_goods_consumed: u32,
    pub final_real_wealth_gini_bps: Option<u32>,
    pub final_early_late_real_wealth_gap: i128,
    pub max_idle_labor_bps: Option<u32>,
    pub max_sector_price_dispersion_bps: Option<u64>,
}

pub fn apply_sweep_values(
    scenario: &mut MarketScenario,
    values: &[(SweepKey, u64)],
) -> Result<(), String> {
    for (key, value) in values {
        let mutated = match key {
            SweepKey::IssuerCreditPerTick => apply_issuer_credit_per_tick(scenario, *value),
            SweepKey::BankCreditPerTick => apply_bank_credit_per_tick(scenario, *value),
            SweepKey::ReserveRatioBps => apply_reserve_ratio_bps(scenario, *value)?,
            SweepKey::FiscalPrint => apply_fiscal_print(scenario, *value)?,
        };
        if !mutated {
            return Err(format!(
                "sweep key '{}' has no matching event in scenario '{}'",
                key.as_str(),
                scenario.name
            ));
        }
    }
    Ok(())
}

pub fn run_sweep(scenario: MarketScenario, axes: &[SweepAxis]) -> Result<Vec<SweepRecord>, String> {
    for (index, axis) in axes.iter().enumerate() {
        if axis.values.is_empty() {
            return Err(format!("sweep axis '{}' has no values", axis.key.as_str()));
        }
        if axes[..index].iter().any(|seen| seen.key == axis.key) {
            return Err(format!("duplicate sweep key '{}'", axis.key.as_str()));
        }
    }

    let mut records = Vec::new();
    let mut variables = Vec::new();
    run_sweep_axis(&scenario, axes, 0, &mut variables, &mut records)?;
    Ok(records)
}

fn run_sweep_axis(
    scenario: &MarketScenario,
    axes: &[SweepAxis],
    axis_index: usize,
    variables: &mut Vec<(SweepKey, u64)>,
    records: &mut Vec<SweepRecord>,
) -> Result<(), String> {
    if axis_index == axes.len() {
        let mut variant = scenario.clone();
        apply_sweep_values(&mut variant, variables)?;
        let society = run_m3_with_shadow(variant.clone());
        let m4_records = build_m4_records(&society)?;
        records.push(build_sweep_record(
            &variant,
            variables.clone(),
            &m4_records,
        )?);
        return Ok(());
    }

    let axis = &axes[axis_index];
    for value in &axis.values {
        variables.push((axis.key, *value));
        run_sweep_axis(scenario, axes, axis_index + 1, variables, records)?;
        variables.pop();
    }
    Ok(())
}

fn apply_issuer_credit_per_tick(scenario: &mut MarketScenario, value: u64) -> bool {
    let mut mutated = false;
    for event in &mut scenario.events {
        if let EventKind::SetIssuerPolicy { policy, .. } = &mut event.kind {
            if policy.credit_enabled {
                policy.max_credit_issue_per_tick = Gold(value);
                mutated = true;
            }
        }
    }
    mutated
}

fn apply_bank_credit_per_tick(scenario: &mut MarketScenario, value: u64) -> bool {
    let mut mutated = false;
    for event in &mut scenario.events {
        if let EventKind::SetBankCreditPolicy { policy, .. } = &mut event.kind {
            if policy.enabled {
                policy.max_new_fiduciary_per_tick = Gold(value);
                mutated = true;
            }
        }
    }
    mutated
}

fn apply_reserve_ratio_bps(scenario: &mut MarketScenario, value: u64) -> Result<bool, String> {
    let ratio = u16::try_from(value)
        .map_err(|_| "reserve-ratio-bps value overflows target type".to_string())?;
    let mut mutated = false;
    for event in &mut scenario.events {
        if let EventKind::SetReserveRatio {
            ratio: event_ratio, ..
        } = &mut event.kind
        {
            *event_ratio = ReserveRatioBps(ratio);
            mutated = true;
        }
    }
    Ok(mutated)
}

fn apply_fiscal_print(scenario: &mut MarketScenario, value: u64) -> Result<bool, String> {
    let amount = Gold(value);
    let required_caps = fiscal_print_required_caps(scenario, amount)?;
    if required_caps.is_empty() {
        return Ok(false);
    }
    if amount > Gold::ZERO {
        for required_cap in &required_caps {
            if !has_enabled_fiscal_policy_before_print(scenario, required_cap) {
                return Err(
                    "fiscal-print sweep has no enabled fiscal policy before a FiatPrint event"
                        .to_string(),
                );
            }
        }
    }

    for event in &mut scenario.events {
        if let EventKind::FiatPrint {
            amount: event_amount,
            ..
        } = &mut event.kind
        {
            *event_amount = amount;
        }
    }

    for event in &mut scenario.events {
        if let EventKind::SetIssuerPolicy { issuer, policy } = &mut event.kind {
            let required_cap = required_fiscal_cap(&required_caps, *issuer);
            if required_cap > Gold::ZERO
                && policy.fiscal_enabled
                && policy.max_fiscal_issue_per_tick < required_cap
            {
                policy.max_fiscal_issue_per_tick = required_cap;
            }
        }
    }

    Ok(true)
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct RequiredFiscalCap {
    issuer: IssuerId,
    tick: u64,
    amount: Gold,
    first_print_index: usize,
}

fn fiscal_print_required_caps(
    scenario: &MarketScenario,
    amount: Gold,
) -> Result<Vec<RequiredFiscalCap>, String> {
    let mut required_caps = Vec::new();
    for (index, event) in scenario.events.iter().enumerate() {
        if let EventKind::FiatPrint { issuer, .. } = &event.kind {
            add_required_fiscal_cap(&mut required_caps, *issuer, event.tick.0, amount, index)?;
        }
    }
    Ok(required_caps)
}

fn add_required_fiscal_cap(
    required_caps: &mut Vec<RequiredFiscalCap>,
    issuer: IssuerId,
    tick: u64,
    amount: Gold,
    print_index: usize,
) -> Result<(), String> {
    if let Some(required_cap) = required_caps
        .iter_mut()
        .find(|entry| entry.issuer == issuer && entry.tick == tick)
    {
        required_cap.amount = required_cap
            .amount
            .checked_add(amount)
            .ok_or_else(|| "fiscal-print cap overflowed".to_string())?;
        required_cap.first_print_index = required_cap.first_print_index.min(print_index);
        return Ok(());
    }
    required_caps.push(RequiredFiscalCap {
        issuer,
        tick,
        amount,
        first_print_index: print_index,
    });
    Ok(())
}

fn has_enabled_fiscal_policy_before_print(
    scenario: &MarketScenario,
    required_cap: &RequiredFiscalCap,
) -> bool {
    scenario.events.iter().enumerate().any(|(index, event)| {
        let policy_applies_before_print = event.tick.0 < required_cap.tick
            || (event.tick.0 == required_cap.tick && index < required_cap.first_print_index);
        if !policy_applies_before_print {
            return false;
        }
        matches!(
            &event.kind,
            EventKind::SetIssuerPolicy { issuer, policy }
                if *issuer == required_cap.issuer && policy.fiscal_enabled
        )
    })
}

fn required_fiscal_cap(required_caps: &[RequiredFiscalCap], issuer: IssuerId) -> Gold {
    required_caps
        .iter()
        .filter(|entry| entry.issuer == issuer)
        .map(|entry| entry.amount)
        .max()
        .unwrap_or(Gold::ZERO)
}

fn build_sweep_record(
    scenario: &MarketScenario,
    variables: Vec<(SweepKey, u64)>,
    records: &[M4Record],
) -> Result<SweepRecord, String> {
    let final_record = records.last();
    Ok(SweepRecord {
        scenario: scenario.name,
        seed: scenario.seed,
        periods: scenario.periods,
        variables,
        final_tms: final_record.map(|record| record.tms).unwrap_or(Gold::ZERO),
        final_fiduciary: final_record
            .map(|record| record.fiduciary)
            .unwrap_or(Gold::ZERO),
        final_public_fiat: final_record
            .map(|record| record.public_fiat)
            .unwrap_or(Gold::ZERO),
        total_bank_credit_issued: sum_gold(records, |record| record.bank_credit_issued)?,
        total_fiat_credit_issued: sum_gold(records, |record| record.fiat_credit_issued)?,
        total_fiat_fiscal_issued: sum_gold(records, |record| record.fiat_fiscal_issued)?,
        total_credit_retired: sum_gold(records, |record| record.credit_retired)?,
        max_shadow_rate_gap_bps: records
            .iter()
            .filter_map(|record| record.shadow_rate_gap_bps)
            .max(),
        max_structure_length_ticks_x100: records
            .iter()
            .map(|record| record.structure_length_ticks_x100)
            .max()
            .unwrap_or(0),
        total_bust_abandoned_projects: sum_u32(records, |record| record.bust_abandoned_projects)?,
        final_abandoned_projects: final_record
            .map(|record| record.abandoned_projects)
            .unwrap_or(0),
        final_debts_defaulted: final_record
            .map(|record| record.debts_defaulted)
            .unwrap_or(0),
        final_project_debts_defaulted: final_record
            .map(|record| record.project_debts_defaulted)
            .unwrap_or(0),
        final_capital_labor_consumed: final_record
            .map(|record| record.capital_labor_consumed)
            .unwrap_or(0),
        final_capital_goods_consumed: final_record
            .map(|record| record.capital_goods_consumed)
            .unwrap_or(0),
        final_real_wealth_gini_bps: final_record.and_then(|record| record.real_wealth_gini_bps),
        final_early_late_real_wealth_gap: final_record
            .map(|record| record.early_late_real_wealth_gap)
            .unwrap_or(0),
        max_idle_labor_bps: records
            .iter()
            .filter_map(|record| record.idle_labor_bps)
            .max(),
        max_sector_price_dispersion_bps: records
            .iter()
            .flat_map(|record| {
                [
                    record.tick_sector_price_dispersion_bps,
                    record.cumulative_sector_price_dispersion_bps,
                ]
            })
            .flatten()
            .max(),
    })
}

fn sum_gold(records: &[M4Record], value: impl Fn(&M4Record) -> Gold) -> Result<Gold, String> {
    records
        .iter()
        .map(value)
        .try_fold(Gold::ZERO, Gold::checked_add)
        .ok_or_else(|| "sweep gold total overflowed".to_string())
}

fn sum_u32(records: &[M4Record], value: impl Fn(&M4Record) -> u32) -> Result<u32, String> {
    records
        .iter()
        .map(value)
        .try_fold(0u32, u32::checked_add)
        .ok_or_else(|| "sweep count total overflowed".to_string())
}

#[cfg(test)]
mod tests {
    use super::{apply_sweep_values, run_sweep, SweepAxis, SweepKey};
    use crate::good::Gold;
    use crate::scenario::{builtin_market_scenario, EventKind, ScenarioName};

    #[test]
    fn sweep_rejects_key_without_matching_event() {
        let mut scenario = builtin_market_scenario(ScenarioName::FiatCreditExpansion);

        assert_eq!(
            apply_sweep_values(&mut scenario, &[(SweepKey::BankCreditPerTick, 1)]),
            Err(
                "sweep key 'bank-credit-per-tick' has no matching event in scenario 'fiat-credit-expansion'"
                    .to_string()
            )
        );
    }

    #[test]
    fn sweep_rejects_disabled_issuer_credit_policy() {
        let mut scenario = builtin_market_scenario(ScenarioName::FiatFiscalCantillon);

        assert_eq!(
            apply_sweep_values(&mut scenario, &[(SweepKey::IssuerCreditPerTick, 3)]),
            Err(
                "sweep key 'issuer-credit-per-tick' has no matching event in scenario 'fiat-fiscal-cantillon'"
                    .to_string()
            )
        );
    }

    #[test]
    fn sweep_rejects_disabled_bank_credit_policy() {
        let mut scenario = builtin_market_scenario(ScenarioName::FractionalReserve);
        for event in &mut scenario.events {
            if let EventKind::SetBankCreditPolicy { policy, .. } = &mut event.kind {
                policy.enabled = false;
            }
        }

        assert_eq!(
            apply_sweep_values(&mut scenario, &[(SweepKey::BankCreditPerTick, 3)]),
            Err(
                "sweep key 'bank-credit-per-tick' has no matching event in scenario 'fractional-reserve'"
                    .to_string()
            )
        );
    }

    #[test]
    fn bank_credit_sweep_updates_all_enabled_policies_for_every_value() {
        let mut zero = builtin_market_scenario(ScenarioName::EmergedGoldSuspensionOfConvertibility);
        apply_sweep_values(&mut zero, &[(SweepKey::BankCreditPerTick, 0)]).unwrap();
        assert_eq!(bank_credit_caps(&zero), vec![Gold::ZERO, Gold::ZERO]);

        let mut positive =
            builtin_market_scenario(ScenarioName::EmergedGoldSuspensionOfConvertibility);
        apply_sweep_values(&mut positive, &[(SweepKey::BankCreditPerTick, 2)]).unwrap();
        assert_eq!(bank_credit_caps(&positive), vec![Gold(2), Gold(2)]);
    }

    #[test]
    fn sweep_rejects_duplicate_axis() {
        let scenario = builtin_market_scenario(ScenarioName::FiatCreditExpansion);

        assert_eq!(
            run_sweep(
                scenario,
                &[
                    SweepAxis {
                        key: SweepKey::IssuerCreditPerTick,
                        values: vec![1],
                    },
                    SweepAxis {
                        key: SweepKey::IssuerCreditPerTick,
                        values: vec![2],
                    },
                ],
            ),
            Err("duplicate sweep key 'issuer-credit-per-tick'".to_string())
        );
    }

    #[test]
    fn fiscal_print_sweep_updates_print_amounts_and_policy_cap() {
        let mut scenario = builtin_market_scenario(ScenarioName::FiatFiscalCantillon);

        apply_sweep_values(&mut scenario, &[(SweepKey::FiscalPrint, 12)]).unwrap();

        let mut print_amounts = Vec::new();
        let mut fiscal_cap = Gold::ZERO;
        for event in &scenario.events {
            match &event.kind {
                EventKind::FiatPrint { amount, .. } => print_amounts.push(*amount),
                EventKind::SetIssuerPolicy { policy, .. } => {
                    fiscal_cap = fiscal_cap.max(policy.max_fiscal_issue_per_tick);
                }
                _ => {}
            }
        }

        assert_eq!(print_amounts, vec![Gold(12), Gold(12)]);
        assert_eq!(fiscal_cap, Gold(12));
    }

    #[test]
    fn fiscal_print_sweep_rejects_missing_enabled_fiscal_policy() {
        let mut scenario = builtin_market_scenario(ScenarioName::FiatFiscalCantillon);
        for event in &mut scenario.events {
            if let EventKind::SetIssuerPolicy { policy, .. } = &mut event.kind {
                policy.fiscal_enabled = false;
            }
        }

        assert_eq!(
            apply_sweep_values(&mut scenario, &[(SweepKey::FiscalPrint, 12)]),
            Err(
                "fiscal-print sweep has no enabled fiscal policy before a FiatPrint event"
                    .to_string()
            )
        );
    }

    #[test]
    fn fiscal_print_sweep_changes_fiat_fiscal_issued() {
        let scenario = builtin_market_scenario(ScenarioName::FiatFiscalCantillon);
        let records = run_sweep(
            scenario,
            &[SweepAxis {
                key: SweepKey::FiscalPrint,
                values: vec![0, 12],
            }],
        )
        .unwrap();

        assert_eq!(records.len(), 2);
        assert_eq!(records[0].total_fiat_fiscal_issued, Gold::ZERO);
        assert!(records[1].total_fiat_fiscal_issued > records[0].total_fiat_fiscal_issued);
    }

    #[test]
    fn sweep_outputs_all_cartesian_rows() {
        let scenario = builtin_market_scenario(ScenarioName::FractionalReserve);
        let records = run_sweep(
            scenario,
            &[
                SweepAxis {
                    key: SweepKey::BankCreditPerTick,
                    values: vec![0, 1],
                },
                SweepAxis {
                    key: SweepKey::ReserveRatioBps,
                    values: vec![0, 10_000, 20_000],
                },
            ],
        )
        .unwrap();

        assert_eq!(records.len(), 6);
        assert_eq!(
            records[0].variables,
            vec![
                (SweepKey::BankCreditPerTick, 0),
                (SweepKey::ReserveRatioBps, 0),
            ]
        );
        assert_eq!(
            records[5].variables,
            vec![
                (SweepKey::BankCreditPerTick, 1),
                (SweepKey::ReserveRatioBps, 20_000),
            ]
        );
    }

    #[test]
    fn sweep_issuer_credit_capacity_changes_fiat_credit_issued() {
        let scenario = builtin_market_scenario(ScenarioName::FiatCreditExpansion);
        let records = run_sweep(
            scenario,
            &[SweepAxis {
                key: SweepKey::IssuerCreditPerTick,
                values: vec![0, 3],
            }],
        )
        .unwrap();

        assert_eq!(records.len(), 2);
        assert_eq!(records[0].total_fiat_credit_issued, Gold::ZERO);
        assert!(records[1].total_fiat_credit_issued > records[0].total_fiat_credit_issued);
    }

    fn bank_credit_caps(scenario: &crate::scenario::MarketScenario) -> Vec<Gold> {
        scenario
            .events
            .iter()
            .filter_map(|event| match &event.kind {
                EventKind::SetBankCreditPolicy { policy, .. } if policy.enabled => {
                    Some(policy.max_new_fiduciary_per_tick)
                }
                _ => None,
            })
            .collect()
    }
}
