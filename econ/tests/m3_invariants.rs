use econ::good::Gold;
use econ::metrics::{
    aggregate_project_forecast_error, basket_relative_price_dispersion, tms_growth_variance,
};
use econ::money::Regime;
use econ::purpose::CreditSource;
use econ::record::M3Record;
use econ::scenario::{builtin_market_scenario, EventKind, ScenarioName};
use econ::shadow::run_credit_disabled_shadow;
use econ::society::run_m3_with_shadow;

const FNV_OFFSET_BASIS: u64 = 0xcbf2_9ce4_8422_2325;
const FNV_PRIME: u64 = 0x0000_0100_0000_01b3;

#[test]
fn money_ledgers_reconcile_every_tick() {
    for name in [
        ScenarioName::SoundMoney100Pct,
        ScenarioName::CommodityCreditNeutral,
        ScenarioName::FractionalReserve,
        ScenarioName::SuspensionOfConvertibility,
        ScenarioName::FiatCreditExpansion,
        ScenarioName::FiatFiscalCantillon,
        ScenarioName::CantillonIsolation,
        ScenarioName::EmergedGoldSoundControl,
        ScenarioName::EmergedGoldFiatDisplacement,
        ScenarioName::EmergedGoldFiatRefusalControl,
        ScenarioName::EmergedGoldFiatLegalTender,
        ScenarioName::EmergedGoldFiatDebtRefusalControl,
        ScenarioName::EmergedGoldFiatDebtLegalTender,
        ScenarioName::EmergedGoldFractionalReserve,
        ScenarioName::EmergedGoldFiatCreditExpansion,
        ScenarioName::EmergedGoldFiatWageRefusalControl,
        ScenarioName::EmergedGoldFiatWageLegalTender,
        ScenarioName::EmergedGoldIssuerRepaymentFiatRefusalControl,
        ScenarioName::EmergedGoldIssuerRepaymentFiatTender,
        ScenarioName::EmergedGoldReserveLeashControl,
        ScenarioName::EmergedGoldSuspensionOfConvertibility,
        ScenarioName::EmergedGoldRedemptionRun,
        ScenarioName::EmergedGoldSuspendedRedemption,
        ScenarioName::EmergedGoldBankClaimDebtRefusalControl,
        ScenarioName::EmergedGoldBankClaimDebtLegalTender,
        ScenarioName::EmergedGoldBankClaimSpotRefusalControl,
        ScenarioName::EmergedGoldBankClaimSpotLegalTender,
        ScenarioName::EmergedGoldBankLoanRepaymentRefusalControl,
        ScenarioName::EmergedGoldBankLoanRepaymentClaimTender,
        ScenarioName::EmergedGoldTaxSpecieControl,
        ScenarioName::EmergedGoldTaxFiatUnpayableDefaults,
        ScenarioName::EmergedGoldTaxDrivesFiatLabor,
        ScenarioName::EmergedGoldNoTaxIdleControl,
    ] {
        let scenario = builtin_market_scenario(name);
        let society = run_m3_with_shadow(scenario);
        assert!(society.money_ledgers_reconcile());
        assert!(
            !society.m3_records.is_empty(),
            "{name:?} should produce M3 records"
        );

        let stayed_sound_gold = society
            .m3_records
            .iter()
            .all(|record| record.regime == Regime::SoundGold);
        for record in &society.m3_records {
            assert_eq!(
                record.tms,
                record
                    .public_specie
                    .saturating_add(record.public_fiat)
                    .saturating_add(record.demand_claims)
            );
            assert!(record.fiduciary <= record.demand_claims);
            if stayed_sound_gold {
                assert_eq!(record.public_fiat, Gold::ZERO);
            }
        }
    }
}

#[test]
fn redemption_run_exposes_reserve_shortfall_without_fiat() {
    let society = run_m3_with_shadow(builtin_market_scenario(
        ScenarioName::EmergedGoldRedemptionRun,
    ));

    let requested = sum_redemption_requested(&society.redemption_audit);
    let honored = sum_redemption_honored(&society.redemption_audit);
    let failed = sum_redemption_failed(&society.redemption_audit);
    let final_record = society.m3_records.last().expect("redemption run records");

    assert_eq!(requested, Gold(4));
    assert_eq!(honored, Gold(2));
    assert_eq!(failed, Gold(2));
    assert_eq!(final_record.public_fiat, Gold::ZERO);
    assert_eq!(final_record.fiat_credit_issued, Gold::ZERO);
    assert_eq!(final_record.fiat_fiscal_issued, Gold::ZERO);
    assert_eq!(final_record.bank_reserves, Gold::ZERO);
    assert_eq!(final_record.demand_claims, Gold(2));
    assert!(final_record.demand_claims > final_record.bank_reserves);
}

#[test]
fn sound_money_has_no_cycle() {
    let society = run_m3_with_shadow(builtin_market_scenario(ScenarioName::SoundMoney100Pct));

    assert!(society
        .m3_records
        .iter()
        .all(|record| record.fiduciary == Gold::ZERO));
    assert!(society
        .m3_records
        .iter()
        .all(|record| record.public_fiat == Gold::ZERO));
    assert!(society
        .m3_records
        .iter()
        .all(|record| record.m2.capital_labor_consumed == 0));
    assert!(society
        .m3_records
        .iter()
        .all(|record| record.m2.capital_goods_consumed == 0));
    assert!(!has_sustained_positive_shadow_gap(&society.m3_records));
    assert_eq!(sum_busts(&society.m3_records), 0);
}

#[test]
fn commodity_credit_is_cycle_neutral() {
    let scenario = builtin_market_scenario(ScenarioName::CommodityCreditNeutral);
    let society = run_m3_with_shadow(scenario);

    assert!(society
        .loan_trades
        .iter()
        .any(|trade| matches!(trade.funding, CreditSource::Commodity)));
    assert!(society
        .m3_records
        .iter()
        .all(|record| record.bank_credit_issued == Gold::ZERO));
    assert!(society
        .m3_records
        .iter()
        .all(|record| record.fiat_credit_issued == Gold::ZERO));
    assert!(society
        .m3_records
        .iter()
        .all(|record| record.fiduciary == Gold::ZERO));
    assert!(!has_sustained_positive_shadow_gap(&society.m3_records));
    assert_eq!(sum_busts(&society.m3_records), 0);
}

#[test]
fn fiduciary_expansion_opens_a_rate_gap() {
    assert_fiduciary_expansion_opens_a_rate_gap(ScenarioName::FractionalReserve);
}

#[test]
fn emerged_gold_fiduciary_expansion_opens_a_rate_gap() {
    assert_fiduciary_expansion_opens_a_rate_gap(ScenarioName::EmergedGoldFractionalReserve);
}

#[test]
fn cantillon_early_receivers_gain_real_wealth_vs_late() {
    let scenario = builtin_market_scenario(ScenarioName::CantillonIsolation);
    let society = run_m3_with_shadow(scenario);

    let first_tick = society
        .cantillon_receipts
        .iter()
        .map(|receipt| receipt.tick.0)
        .min()
        .expect("receipt tape should have early receivers");
    let mut early_receivers = society
        .cantillon_receipts
        .iter()
        .filter(|receipt| receipt.tick.0 == first_tick)
        .map(|receipt| receipt.agent)
        .collect::<Vec<_>>();
    early_receivers.sort();
    early_receivers.dedup();
    assert!(!early_receivers.is_empty());
    assert!(society.trades.iter().any(|trade| {
        trade.tick >= first_tick && early_receivers.binary_search(&trade.buyer).is_ok()
    }));

    let final_record = society.m3_records.last().expect("M3 records exist");
    assert!(final_record.early_receiver_wealth_delta > final_record.late_receiver_wealth_delta);
    assert!(final_record.early_receiver_wealth_delta > 0);
}

#[test]
fn fiat_fiscal_injection_does_not_force_bust() {
    let scenario = builtin_market_scenario(ScenarioName::FiatFiscalCantillon);
    let society = run_m3_with_shadow(scenario);
    let first = society.m3_records.first().expect("M3 records exist");
    let last = society.m3_records.last().expect("M3 records exist");

    assert!(last.public_fiat > first.public_fiat);
    assert!(last.tms > first.tms);
    assert!(society
        .m3_records
        .iter()
        .all(|record| record.bank_loan_trades == 0 && record.fiat_loan_trades == 0));
    assert!(!has_sustained_positive_shadow_gap(&society.m3_records));
    assert_eq!(sum_busts(&society.m3_records), 0);
    assert_eq!(sum_labor_consumed(&society.m3_records), 0);
    assert_eq!(sum_goods_consumed(&society.m3_records), 0);
}

#[test]
fn calculation_degrades_under_fiat_credit() {
    let fiat = run_m3_with_shadow(builtin_market_scenario(ScenarioName::FiatCreditExpansion));
    let sound = run_m3_with_shadow(builtin_market_scenario(ScenarioName::SoundMoney100Pct));

    let fiat_error = aggregate_project_forecast_error(&fiat.m2_projects, &fiat.project_output_lots)
        .expect("fiat forecast error should fit in u128");
    let sound_error =
        aggregate_project_forecast_error(&sound.m2_projects, &sound.project_output_lots)
            .expect("sound forecast error should fit in u128");

    assert!(fiat_error > sound_error);
}

#[test]
fn hard_vs_soft_money() {
    let gold = run_m3_with_shadow(builtin_market_scenario(ScenarioName::SoundMoney100Pct));
    let fiat = run_m3_with_shadow(builtin_market_scenario(ScenarioName::FiatFiscalCantillon));

    let fiat_tms_variance =
        tms_growth_variance(&fiat.m3_records).expect("fiat TMS variance should fit in u128");
    let gold_tms_variance =
        tms_growth_variance(&gold.m3_records).expect("gold TMS variance should fit in u128");
    let fiat_price_dispersion = basket_relative_price_dispersion(&fiat.trades)
        .expect("fiat price dispersion should fit in u128");
    let gold_price_dispersion = basket_relative_price_dispersion(&gold.trades)
        .expect("gold price dispersion should fit in u128");

    assert!(fiat_tms_variance > gold_tms_variance);
    assert!(fiat_price_dispersion > gold_price_dispersion);
}

#[test]
fn expansion_then_stop_busts_and_consumes_capital() {
    assert_fiat_credit_expansion_then_stop_busts_and_consumes_capital(
        ScenarioName::FiatCreditExpansion,
    );
}

#[test]
fn emerged_gold_fiat_credit_expands_stops_and_busts() {
    assert_fiat_credit_expansion_then_stop_busts_and_consumes_capital(
        ScenarioName::EmergedGoldFiatCreditExpansion,
    );
}

#[test]
fn fractional_then_suspension_widens_gap() {
    let scenario = builtin_market_scenario(ScenarioName::SuspensionOfConvertibility);
    let suspension_tick = scenario
        .events
        .iter()
        .find_map(|event| match event.kind {
            EventKind::SetRegime(Regime::SuspendedConvertibility) => Some(event.tick.0),
            _ => None,
        })
        .expect("suspension event exists");
    let society = run_m3_with_shadow(scenario);

    let before_peak = peak_shadow_gap(&society.m3_records, |tick| tick < suspension_tick);
    let after_peak = peak_shadow_gap(&society.m3_records, |tick| tick >= suspension_tick);
    assert!(
        after_peak > before_peak,
        "suspension should widen the peak shadow gap"
    );

    let before_capacity = society
        .m3_records
        .iter()
        .filter(|record| record.m2.tick < suspension_tick)
        .map(|record| record.bank_credit_issued.0)
        .max()
        .unwrap_or(0);
    let after_capacity = society
        .m3_records
        .iter()
        .filter(|record| record.m2.tick >= suspension_tick)
        .map(|record| record.bank_credit_issued.0)
        .max()
        .unwrap_or(0);
    assert!(after_capacity > before_capacity);
}

#[test]
fn emerged_gold_suspension_widens_gap_against_reserve_leash_control() {
    let control_scenario = builtin_market_scenario(ScenarioName::EmergedGoldReserveLeashControl);
    let suspension_scenario =
        builtin_market_scenario(ScenarioName::EmergedGoldSuspensionOfConvertibility);
    assert_eq!(control_scenario.seed, suspension_scenario.seed);
    assert_eq!(control_scenario.periods, suspension_scenario.periods);

    let control = run_m3_with_shadow(control_scenario);
    let suspension = run_m3_with_shadow(suspension_scenario);

    let control_pre_records = control
        .m3_records
        .iter()
        .filter(|record| record.m2.tick < 4)
        .collect::<Vec<_>>();
    let suspension_pre_records = suspension
        .m3_records
        .iter()
        .filter(|record| record.m2.tick < 4)
        .collect::<Vec<_>>();
    assert_eq!(control_pre_records, suspension_pre_records);

    let control_final = control.m3_records.last().expect("control records");
    let suspension_final = suspension.m3_records.last().expect("suspension records");
    assert!(suspension_final.demand_claims > control_final.demand_claims);
    assert!(suspension_final.fiduciary > control_final.fiduciary);

    let control_post_peak = peak_shadow_gap(&control.m3_records, |tick| tick >= 4);
    let suspension_post_peak = peak_shadow_gap(&suspension.m3_records, |tick| tick >= 4);
    assert!(
        suspension_post_peak > control_post_peak,
        "emerged-gold suspension should widen the post-suspension gap"
    );
}

#[test]
fn golden_m3_series_is_stable() {
    // Updated when credit-channel Cantillon receipts were added (concerns2.md B1): the
    // fiat-credit boom's first receivers now populate early/late receiver wealth deltas,
    // which this hash covers. The rest of the M3 series is byte-identical.
    const EXPECTED: u64 = 0x27c2_820c_7db0_795a;
    let mut scenario = builtin_market_scenario(ScenarioName::FiatCreditExpansion);
    scenario.seed = 31;
    let society = run_m3_with_shadow(scenario);

    let actual = hash_m3_series(&society.m3_records);
    assert_eq!(actual, EXPECTED, "actual golden hash: {actual:#018x}");
}

fn assert_fiduciary_expansion_opens_a_rate_gap(name: ScenarioName) {
    let scenario = builtin_market_scenario(name);
    let society = run_m3_with_shadow(scenario);

    assert!(society
        .loan_trades
        .iter()
        .any(|trade| matches!(trade.funding, CreditSource::BankFiduciary(_))));
    assert!(society
        .m3_records
        .iter()
        .any(|record| record.demand_claims > Gold::ZERO));
    assert!(society
        .m3_records
        .iter()
        .any(|record| record.fiduciary > Gold::ZERO));

    let active_positive_gap_ticks = society
        .m3_records
        .iter()
        .filter(|record| {
            record.bank_credit_issued > Gold::ZERO && record.shadow_rate_gap_bps.unwrap_or(0) > 0
        })
        .count();
    assert!(
        active_positive_gap_ticks >= 2,
        "{name:?} should have at least two active bank-credit ticks with positive shadow gap"
    );
}

fn assert_fiat_credit_expansion_then_stop_busts_and_consumes_capital(name: ScenarioName) {
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

    assert!(credit_tick <= gap_tick);
    assert!(gap_tick <= structure_tick);
    assert!(structure_tick < stop_tick);
    assert!(stop_tick < bust_tick);
    assert!(bust_tick <= consumed_tick);
    assert!(stop_tick <= bad_debt_or_retirement_tick);
    assert!(records
        .iter()
        .filter(|record| record.m2.tick >= stop_tick)
        .all(|record| record.fiat_credit_issued == Gold::ZERO));
}

fn peak_shadow_gap(records: &[M3Record], tick_filter: impl Fn(u64) -> bool) -> i64 {
    records
        .iter()
        .filter(|record| tick_filter(record.m2.tick))
        .filter_map(|record| record.shadow_rate_gap_bps)
        .max()
        .unwrap_or(i64::MIN)
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

fn sum_redemption_requested(records: &[econ::record::RedemptionAuditRecord]) -> Gold {
    records.iter().fold(Gold::ZERO, |sum, record| {
        sum.saturating_add(record.requested)
    })
}

fn sum_redemption_honored(records: &[econ::record::RedemptionAuditRecord]) -> Gold {
    records
        .iter()
        .fold(Gold::ZERO, |sum, record| sum.saturating_add(record.honored))
}

fn sum_redemption_failed(records: &[econ::record::RedemptionAuditRecord]) -> Gold {
    records
        .iter()
        .fold(Gold::ZERO, |sum, record| sum.saturating_add(record.failed))
}

fn hash_m3_series(records: &[M3Record]) -> u64 {
    let mut hash = FNV_OFFSET_BASIS;
    for record in records {
        hash_u64(&mut hash, record.m2.tick);
        hash_u64(&mut hash, regime_code(record.regime));
        hash_gold(&mut hash, record.public_specie);
        hash_gold(&mut hash, record.public_fiat);
        hash_gold(&mut hash, record.demand_claims);
        hash_gold(&mut hash, record.bank_reserves);
        hash_gold(&mut hash, record.fiduciary);
        hash_gold(&mut hash, record.time_deposits);
        hash_gold(&mut hash, record.tms);
        hash_gold(&mut hash, record.bank_credit_issued);
        hash_gold(&mut hash, record.fiat_credit_issued);
        hash_gold(&mut hash, record.fiat_fiscal_issued);
        hash_gold(&mut hash, record.credit_retired);
        hash_u64(&mut hash, u64::from(record.bank_loan_trades));
        hash_u64(&mut hash, u64::from(record.fiat_loan_trades));
        hash_u64(&mut hash, u64::from(record.m2.loan_trades));
        hash_u64(&mut hash, u64::from(record.m2.project_loan_trades));
        hash_u64(&mut hash, u64::from(record.boom_projects_started));
        hash_u64(&mut hash, u64::from(record.bust_abandoned_projects));
        hash_u64(&mut hash, u64::from(record.m2.active_projects));
        hash_u64(&mut hash, u64::from(record.m2.waiting_projects));
        hash_u64(&mut hash, u64::from(record.m2.mature_projects));
        hash_u64(&mut hash, u64::from(record.m2.sold_projects));
        hash_u64(&mut hash, u64::from(record.m2.abandoned_projects));
        hash_u64(&mut hash, u64::from(record.m2.capital_labor_consumed));
        hash_u64(&mut hash, u64::from(record.m2.capital_goods_consumed));
        hash_gold(&mut hash, record.m2.project_revenue);
        hash_u64(&mut hash, u64::from(record.m2.debts_defaulted));
        hash_u64(&mut hash, u64::from(record.m2.project_debts_defaulted));
        hash_u64(&mut hash, record.m2.structure_length_ticks_x100);
        hash_i64_option(&mut hash, record.m2.market_rate_bps);
        hash_i64_option(&mut hash, record.shadow_natural_rate_bps);
        hash_i64_option(&mut hash, record.shadow_rate_gap_bps);
        hash_i64(&mut hash, record.early_receiver_wealth_delta);
        hash_i64(&mut hash, record.late_receiver_wealth_delta);
    }
    hash
}

fn regime_code(regime: Regime) -> u64 {
    match regime {
        Regime::SoundGold => 0,
        Regime::FractionalConvertible => 1,
        Regime::SuspendedConvertibility => 2,
        Regime::Fiat => 3,
    }
}

fn hash_gold(hash: &mut u64, value: Gold) {
    hash_u64(hash, value.0);
}

fn hash_i64_option(hash: &mut u64, value: Option<i64>) {
    match value {
        Some(value) => {
            hash_u64(hash, 1);
            hash_i64(hash, value);
        }
        None => hash_u64(hash, 0),
    }
}

fn hash_i64(hash: &mut u64, value: i64) {
    for byte in value.to_le_bytes() {
        hash_byte(hash, byte);
    }
}

fn hash_u64(hash: &mut u64, value: u64) {
    for byte in value.to_le_bytes() {
        hash_byte(hash, byte);
    }
}

fn hash_byte(hash: &mut u64, byte: u8) {
    *hash ^= u64::from(byte);
    *hash = hash.wrapping_mul(FNV_PRIME);
}
