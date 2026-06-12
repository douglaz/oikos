use econ::agent::AgentId;
use econ::cantillon::CantillonRoute;
use econ::good::Gold;
use econ::ledger::IssuerId;
use econ::money::Regime;
use econ::project::Tick;
use econ::purpose::CreditSource;
use econ::scenario::{builtin_market_scenario, Event, EventKind, ScenarioName};
use econ::society::Society;

const M3_SCENARIOS: &[ScenarioName] = &[
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
    // M21: only the fiat-receivable tax scenarios are listed here. The
    // specie-only control (EmergedGoldTaxSpecieControl) intentionally vaults
    // specie into the issuer, moving it out of public circulation, which the
    // `public_specie + bank_reserves == commodity_base` equality below forbids;
    // it is covered by the vault-aware reconciliation sweep in m3_invariants.
    ScenarioName::EmergedGoldTaxFiatUnpayableDefaults,
    ScenarioName::EmergedGoldTaxDrivesFiatLabor,
    ScenarioName::EmergedGoldNoTaxIdleControl,
];

#[test]
fn money_ledgers_reconcile_every_tick() {
    for &name in M3_SCENARIOS {
        let scenario = builtin_market_scenario(name);
        let initial_gold = scenario
            .agents
            .iter()
            .fold(Gold::ZERO, |total, agent| total.saturating_add(agent.gold));
        let periods = scenario.periods;
        let mut society = Society::from_scenario(scenario);
        let initial_bank_reserves = society.banks.iter().fold(Gold::ZERO, |total, bank| {
            total.saturating_add(bank.reserves)
        });
        let commodity_base = initial_gold.saturating_add(initial_bank_reserves);

        for tick in 0..periods {
            society.step();
            assert!(
                society.money_ledgers_reconcile(),
                "M3 ledger failed to reconcile for {name:?} at tick {tick}"
            );
            assert_eq!(society.m2_records.len(), 0);
            assert_eq!(society.m3_records.len(), usize::try_from(tick + 1).unwrap());

            let record = society.m3_records.last().expect("M3 record per tick");
            assert_eq!(record.regime, expected_regime_for_tick(name, tick));
            assert_eq!(
                record.public_specie.saturating_add(record.bank_reserves),
                commodity_base
            );
            if matches!(
                name,
                ScenarioName::FiatCreditExpansion
                    | ScenarioName::FiatFiscalCantillon
                    | ScenarioName::CantillonIsolation
                    | ScenarioName::EmergedGoldFiatDisplacement
                    | ScenarioName::EmergedGoldFiatRefusalControl
                    | ScenarioName::EmergedGoldFiatLegalTender
                    | ScenarioName::EmergedGoldFiatDebtRefusalControl
                    | ScenarioName::EmergedGoldFiatDebtLegalTender
                    | ScenarioName::EmergedGoldFiatCreditExpansion
                    | ScenarioName::EmergedGoldFiatWageRefusalControl
                    | ScenarioName::EmergedGoldFiatWageLegalTender
                    | ScenarioName::EmergedGoldIssuerRepaymentFiatRefusalControl
                    | ScenarioName::EmergedGoldIssuerRepaymentFiatTender
                    | ScenarioName::EmergedGoldTaxFiatUnpayableDefaults
                    | ScenarioName::EmergedGoldTaxDrivesFiatLabor
                    | ScenarioName::EmergedGoldNoTaxIdleControl
            ) {
                assert_eq!(
                    record.tms,
                    record
                        .public_specie
                        .saturating_add(record.public_fiat)
                        .saturating_add(record.demand_claims)
                );
                assert_eq!(record.demand_claims, Gold::ZERO);
                assert_eq!(record.bank_reserves, Gold::ZERO);
                assert_eq!(record.fiduciary, Gold::ZERO);
                assert_eq!(record.time_deposits, Gold::ZERO);
                assert_eq!(record.m2.total_gold, commodity_base);
                continue;
            }
            assert_eq!(record.public_fiat, Gold::ZERO);
            assert_eq!(record.time_deposits, Gold::ZERO);
            if matches!(
                name,
                ScenarioName::FractionalReserve
                    | ScenarioName::SuspensionOfConvertibility
                    | ScenarioName::EmergedGoldFractionalReserve
                    | ScenarioName::EmergedGoldReserveLeashControl
                    | ScenarioName::EmergedGoldSuspensionOfConvertibility
                    | ScenarioName::EmergedGoldRedemptionRun
                    | ScenarioName::EmergedGoldSuspendedRedemption
                    | ScenarioName::EmergedGoldBankClaimDebtRefusalControl
                    | ScenarioName::EmergedGoldBankClaimDebtLegalTender
                    | ScenarioName::EmergedGoldBankClaimSpotRefusalControl
                    | ScenarioName::EmergedGoldBankClaimSpotLegalTender
                    | ScenarioName::EmergedGoldBankLoanRepaymentRefusalControl
                    | ScenarioName::EmergedGoldBankLoanRepaymentClaimTender
            ) {
                assert_eq!(
                    record.tms,
                    record.public_specie.saturating_add(record.demand_claims)
                );
                assert_eq!(record.m2.total_gold, commodity_base);
                assert!(record.fiduciary <= record.demand_claims);
            } else {
                assert_eq!(record.demand_claims, Gold::ZERO);
                assert_eq!(record.bank_reserves, Gold::ZERO);
                assert_eq!(record.fiduciary, Gold::ZERO);
                assert_eq!(record.tms, initial_gold);
                assert_eq!(record.m2.total_gold, commodity_base);
            }
        }
    }
}

fn expected_regime_for_tick(name: ScenarioName, tick: u64) -> Regime {
    match name {
        ScenarioName::SuspensionOfConvertibility if tick >= 4 => Regime::SuspendedConvertibility,
        ScenarioName::EmergedGoldSuspensionOfConvertibility if tick >= 4 => {
            Regime::SuspendedConvertibility
        }
        ScenarioName::EmergedGoldSuspendedRedemption if tick >= 4 => {
            Regime::SuspendedConvertibility
        }
        ScenarioName::EmergedGoldBankClaimDebtRefusalControl if tick >= 4 => {
            Regime::SuspendedConvertibility
        }
        ScenarioName::EmergedGoldBankClaimDebtLegalTender if tick >= 4 => {
            Regime::SuspendedConvertibility
        }
        ScenarioName::EmergedGoldBankClaimSpotRefusalControl if tick >= 4 => {
            Regime::SuspendedConvertibility
        }
        ScenarioName::EmergedGoldBankClaimSpotLegalTender if tick >= 4 => {
            Regime::SuspendedConvertibility
        }
        ScenarioName::EmergedGoldBankLoanRepaymentRefusalControl if tick >= 4 => {
            Regime::SuspendedConvertibility
        }
        ScenarioName::EmergedGoldBankLoanRepaymentClaimTender if tick >= 4 => {
            Regime::SuspendedConvertibility
        }
        ScenarioName::SuspensionOfConvertibility => Regime::FractionalConvertible,
        ScenarioName::EmergedGoldSuspensionOfConvertibility
        | ScenarioName::EmergedGoldSuspendedRedemption
        | ScenarioName::EmergedGoldBankClaimDebtRefusalControl
        | ScenarioName::EmergedGoldBankClaimDebtLegalTender
        | ScenarioName::EmergedGoldBankClaimSpotRefusalControl
        | ScenarioName::EmergedGoldBankClaimSpotLegalTender
        | ScenarioName::EmergedGoldBankLoanRepaymentRefusalControl
        | ScenarioName::EmergedGoldBankLoanRepaymentClaimTender
        | ScenarioName::EmergedGoldRedemptionRun
        | ScenarioName::FractionalReserve
        | ScenarioName::EmergedGoldFractionalReserve
        | ScenarioName::EmergedGoldReserveLeashControl => Regime::FractionalConvertible,
        ScenarioName::FiatCreditExpansion
        | ScenarioName::FiatFiscalCantillon
        | ScenarioName::CantillonIsolation
        | ScenarioName::EmergedGoldFiatDisplacement
        | ScenarioName::EmergedGoldFiatRefusalControl
        | ScenarioName::EmergedGoldFiatLegalTender
        | ScenarioName::EmergedGoldFiatDebtRefusalControl
        | ScenarioName::EmergedGoldFiatDebtLegalTender
        | ScenarioName::EmergedGoldFiatCreditExpansion
        | ScenarioName::EmergedGoldFiatWageRefusalControl
        | ScenarioName::EmergedGoldFiatWageLegalTender
        | ScenarioName::EmergedGoldIssuerRepaymentFiatRefusalControl
        | ScenarioName::EmergedGoldIssuerRepaymentFiatTender
        | ScenarioName::EmergedGoldTaxFiatUnpayableDefaults
        | ScenarioName::EmergedGoldTaxDrivesFiatLabor
        | ScenarioName::EmergedGoldNoTaxIdleControl => Regime::Fiat,
        ScenarioName::CommodityCreditNeutral | ScenarioName::EmergedGoldSoundControl => {
            Regime::SoundGold
        }
        _ => unreachable!("only M3 scenarios are listed"),
    }
}

#[test]
fn fiscal_cantillon_redistributes_without_forced_bust_or_credit_expansion() {
    for name in [
        ScenarioName::FiatFiscalCantillon,
        ScenarioName::CantillonIsolation,
    ] {
        let scenario = builtin_market_scenario(name);
        let periods = scenario.periods;
        let mut society = Society::from_scenario(scenario);

        society.run(periods);

        assert!(society
            .cantillon_receipts
            .iter()
            .any(|receipt| matches!(receipt.source, CreditSource::FiatFiscal(_))));
        assert!(society
            .m3_records
            .iter()
            .any(|record| record.fiat_fiscal_issued > Gold::ZERO));
        assert!(society
            .m3_records
            .iter()
            .any(|record| record.public_fiat > Gold::ZERO && record.tms > Gold::ZERO));
        assert!(society.m3_records.iter().all(|record| {
            record.bank_credit_issued == Gold::ZERO
                && record.fiat_credit_issued == Gold::ZERO
                && record.bank_loan_trades == 0
                && record.fiat_loan_trades == 0
                && record.m2.capital_labor_consumed == 0
                && record.m2.capital_goods_consumed == 0
                && record.m2.abandoned_projects == 0
        }));

        let final_record = society.m3_records.last().expect("scenario records");
        assert!(
            final_record.early_receiver_wealth_delta > final_record.late_receiver_wealth_delta,
            "{name:?} early receiver delta should exceed late/non-receiver delta"
        );
    }
}

#[test]
fn fiat_fiscal_cantillon_first_receivers_spend_before_later_receivers() {
    let scenario = builtin_market_scenario(ScenarioName::FiatFiscalCantillon);
    let periods = scenario.periods;
    let mut society = Society::from_scenario(scenario);

    society.run(periods);

    let first_tick = society
        .cantillon_receipts
        .iter()
        .map(|receipt| receipt.tick.0)
        .min()
        .expect("receipt tape");
    let mut early = society
        .cantillon_receipts
        .iter()
        .filter(|receipt| receipt.tick.0 == first_tick)
        .map(|receipt| receipt.agent)
        .collect::<Vec<_>>();
    early.sort();
    early.dedup();
    let mut late = society
        .cantillon_receipts
        .iter()
        .filter(|receipt| receipt.tick.0 > first_tick && !early.contains(&receipt.agent))
        .map(|receipt| receipt.agent)
        .collect::<Vec<_>>();
    late.sort();
    late.dedup();

    let early_spend_tick = society
        .trades
        .iter()
        .filter(|trade| early.contains(&trade.buyer))
        .map(|trade| trade.tick)
        .min()
        .expect("early receivers spend");
    let late_spend_tick = society
        .trades
        .iter()
        .filter(|trade| late.contains(&trade.buyer))
        .map(|trade| trade.tick)
        .min()
        .expect("later receivers spend");

    assert!(early_spend_tick < late_spend_tick);
}

#[test]
fn fiscal_issuer_cap_is_enforced_across_same_tick_prints() {
    let mut scenario = builtin_market_scenario(ScenarioName::CantillonIsolation);
    scenario.events.push(Event {
        tick: Tick(0),
        kind: EventKind::FiatPrint {
            issuer: IssuerId(1),
            amount: Gold(1),
            route: CantillonRoute::Agents(vec![AgentId(2)]),
        },
    });
    let periods = scenario.periods;
    let mut society = Society::from_scenario(scenario);

    society.run(periods);

    let first_record = society.m3_records.first().expect("M3 record");
    let final_record = society.m3_records.last().expect("M3 record");
    assert_eq!(first_record.fiat_fiscal_issued, Gold(4));
    assert_eq!(final_record.public_fiat, Gold(4));
    assert_eq!(society.cantillon_receipts.len(), 1);
}

#[test]
fn fractional_reserve_issues_fiduciary_claims() {
    let scenario = builtin_market_scenario(ScenarioName::FractionalReserve);
    let periods = scenario.periods;
    let initial_gold = scenario
        .agents
        .iter()
        .fold(Gold::ZERO, |total, agent| total.saturating_add(agent.gold));
    let mut society = Society::from_scenario(scenario);

    society.run(periods);

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
        .any(|record| record.bank_credit_issued > Gold::ZERO));
    assert!(society
        .m3_records
        .iter()
        .any(|record| record.bank_loan_trades > 0));
    assert!(society
        .m3_records
        .iter()
        .any(|record| record.m2.structure_length_ticks_x100 > 0));
    assert!(society.m3_records.iter().all(|record| {
        record.public_specie.saturating_add(record.bank_reserves) == initial_gold
            && record.tms == record.public_specie.saturating_add(record.demand_claims)
            && record.fiduciary <= record.demand_claims
    }));
}

#[test]
fn commodity_credit_neutral_has_no_fiduciary() {
    let scenario = builtin_market_scenario(ScenarioName::CommodityCreditNeutral);
    let periods = scenario.periods;
    let mut society = Society::from_scenario(scenario);

    society.run(periods);

    assert!(society
        .m3_records
        .iter()
        .all(|record| record.demand_claims == Gold::ZERO
            && record.fiduciary == Gold::ZERO
            && record.bank_credit_issued == Gold::ZERO
            && record.bank_loan_trades == 0));
}

#[test]
fn bank_credit_first_receiver_is_tagged_on_receipt_tape() {
    // Bank fiduciary loans clear in fractional-reserve; impl-05.md §5 requires the
    // borrowers who cross those bank lend orders to be tagged as first receivers.
    let scenario = builtin_market_scenario(ScenarioName::FractionalReserve);
    let periods = scenario.periods;
    let mut society = Society::from_scenario(scenario);
    society.run(periods);

    let bank_receipts: Vec<_> = society
        .cantillon_receipts
        .iter()
        .filter(|receipt| matches!(receipt.source, CreditSource::BankFiduciary(_)))
        .collect();
    assert!(
        !bank_receipts.is_empty(),
        "bank-credit first receivers must be tagged on the Cantillon receipt tape"
    );
    // amount is the loan principal (trade.present), strictly positive.
    assert!(bank_receipts
        .iter()
        .all(|receipt| receipt.amount > Gold::ZERO));
}

#[test]
fn fiat_credit_first_receiver_is_tagged_on_receipt_tape() {
    // Issuer fiat-credit lines clear during the boom in fiat-credit-expansion; their
    // borrowers must likewise be tagged as first receivers (impl-05.md §5).
    let scenario = builtin_market_scenario(ScenarioName::FiatCreditExpansion);
    let periods = scenario.periods;
    let mut society = Society::from_scenario(scenario);
    society.run(periods);

    let fiat_credit_receipts: Vec<_> = society
        .cantillon_receipts
        .iter()
        .filter(|receipt| matches!(receipt.source, CreditSource::FiatCredit(_)))
        .collect();
    assert!(
        !fiat_credit_receipts.is_empty(),
        "fiat-credit first receivers must be tagged on the Cantillon receipt tape"
    );
    assert!(fiat_credit_receipts
        .iter()
        .all(|receipt| receipt.amount > Gold::ZERO));
}
