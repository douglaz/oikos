//! Credit-disabled replay helpers for ABCT counterfactual metrics.

use crate::bank::BankPolicy;
use crate::good::Gold;
use crate::issuer::IssuerPolicy;
use crate::ledger::{BankId, IssuerId};
use crate::money::ReserveRatioBps;
use crate::project::Tick;
use crate::scenario::{Event, EventKind, MarketScenario};
use crate::society::{banks_for_scenario, issuers_for_scenario, Society};

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ShadowSeries {
    pub natural_rate_bps: Vec<Option<i64>>,
    pub structure_length_ticks_x100: Vec<u64>,
}

pub fn run_credit_disabled_shadow(scenario: &MarketScenario) -> ShadowSeries {
    let neutralized = credit_disabled_scenario(scenario);
    let periods = neutralized.periods;
    let mut society = Society::from_scenario(neutralized);
    society.run(periods);
    debug_assert!(
        society.m3_records.iter().all(|record| {
            record.bank_credit_issued == Gold::ZERO
                && record.fiat_credit_issued == Gold::ZERO
                && record.fiat_fiscal_issued == Gold::ZERO
                && record.bank_loan_trades == 0
                && record.fiat_loan_trades == 0
        }),
        "credit-disabled shadow emitted created credit"
    );
    ShadowSeries {
        natural_rate_bps: society
            .m3_records
            .iter()
            .map(|record| record.m2.natural_rate_proxy_bps)
            .collect(),
        structure_length_ticks_x100: society
            .m3_records
            .iter()
            .map(|record| record.m2.structure_length_ticks_x100)
            .collect(),
    }
}

pub fn credit_disabled_scenario(scenario: &MarketScenario) -> MarketScenario {
    let mut neutralized = scenario.clone();
    neutralized.events = Vec::new();

    for bank in shadow_bank_ids(scenario) {
        neutralized.events.push(Event {
            tick: Tick(0),
            kind: EventKind::SetReserveRatio {
                bank,
                ratio: ReserveRatioBps::FULL,
            },
        });
        neutralized.events.push(Event {
            tick: Tick(0),
            kind: EventKind::SetBankCreditPolicy {
                bank,
                policy: disabled_bank_policy(),
            },
        });
        neutralized.events.push(Event {
            tick: Tick(0),
            kind: EventKind::StopBankCredit { bank },
        });
    }

    for issuer in shadow_issuer_ids(scenario) {
        neutralized.events.push(Event {
            tick: Tick(0),
            kind: EventKind::SetIssuerPolicy {
                issuer,
                policy: disabled_issuer_policy(),
            },
        });
        neutralized.events.push(Event {
            tick: Tick(0),
            kind: EventKind::StopIssuerCredit { issuer },
        });
    }

    neutralized
        .events
        .extend(scenario.events.iter().filter_map(|event| {
            let kind = match event.kind.clone() {
                EventKind::FiatPrint { .. } => return None,
                EventKind::SetReserveRatio { bank, .. } => EventKind::SetReserveRatio {
                    bank,
                    ratio: ReserveRatioBps::FULL,
                },
                EventKind::SetBankCreditPolicy { bank, .. } => EventKind::SetBankCreditPolicy {
                    bank,
                    policy: disabled_bank_policy(),
                },
                EventKind::SetIssuerPolicy { issuer, .. } => EventKind::SetIssuerPolicy {
                    issuer,
                    policy: disabled_issuer_policy(),
                },
                other => other,
            };
            Some(Event {
                tick: event.tick,
                kind,
            })
        }));
    neutralized
}

fn disabled_bank_policy() -> BankPolicy {
    BankPolicy {
        max_new_fiduciary_per_tick: Gold::ZERO,
        loan_present: Gold::ZERO,
        loan_horizon: 0,
        loan_future_due: Gold::ZERO,
        enabled: false,
    }
}

fn disabled_issuer_policy() -> IssuerPolicy {
    IssuerPolicy {
        fiscal_enabled: false,
        credit_enabled: false,
        max_fiscal_issue_per_tick: Gold::ZERO,
        max_credit_issue_per_tick: Gold::ZERO,
        loan_present: Gold::ZERO,
        loan_horizon: 0,
        loan_future_due: Gold::ZERO,
    }
}

fn shadow_bank_ids(scenario: &MarketScenario) -> Vec<BankId> {
    let mut ids = banks_for_scenario(scenario.scenario)
        .iter()
        .map(|bank| bank.id)
        .collect::<Vec<_>>();
    for event in &scenario.events {
        match event.kind {
            EventKind::SetReserveRatio { bank, .. }
            | EventKind::SetBankConvertibility { bank, .. }
            | EventKind::SetBankCreditPolicy { bank, .. }
            | EventKind::StopBankCredit { bank }
            | EventKind::RedeemDemandClaims { bank, .. } => ids.push(bank),
            _ => {}
        }
    }
    ids.sort();
    ids.dedup();
    ids
}

fn shadow_issuer_ids(scenario: &MarketScenario) -> Vec<IssuerId> {
    let mut ids = issuers_for_scenario(scenario.scenario)
        .iter()
        .map(|issuer| issuer.id)
        .collect::<Vec<_>>();
    for event in &scenario.events {
        match event.kind {
            EventKind::FiatPrint { issuer, .. }
            | EventKind::SetIssuerPolicy { issuer, .. }
            | EventKind::StopIssuerCredit { issuer } => ids.push(issuer),
            _ => {}
        }
    }
    ids.sort();
    ids.dedup();
    ids
}

#[cfg(test)]
mod tests {
    use super::{credit_disabled_scenario, run_credit_disabled_shadow};
    use crate::bank::BankPolicy;
    use crate::cantillon::CantillonRoute;
    use crate::good::Gold;
    use crate::issuer::IssuerPolicy;
    use crate::ledger::{BankId, IssuerId};
    use crate::money::ReserveRatioBps;
    use crate::project::Tick;
    use crate::purpose::CreditSource;
    use crate::scenario::{builtin_market_scenario, Event, EventKind, ScenarioName};
    use crate::society::Society;
    use std::fs;
    use std::path::Path;

    #[test]
    fn shadow_disables_bank_and_fiat_credit() {
        for name in [
            ScenarioName::FractionalReserve,
            ScenarioName::SuspensionOfConvertibility,
            ScenarioName::FiatCreditExpansion,
            ScenarioName::FiatFiscalCantillon,
            ScenarioName::EmergedGoldFiatDisplacement,
            ScenarioName::EmergedGoldFractionalReserve,
            ScenarioName::EmergedGoldFiatCreditExpansion,
        ] {
            let scenario = credit_disabled_scenario(&builtin_market_scenario(name));
            let periods = scenario.periods;
            let mut society = Society::from_scenario(scenario);

            society.run(periods);

            assert!(society.m3_records.iter().all(|record| {
                record.bank_credit_issued == Gold::ZERO
                    && record.fiat_credit_issued == Gold::ZERO
                    && record.fiat_fiscal_issued == Gold::ZERO
                    && record.bank_loan_trades == 0
                    && record.fiat_loan_trades == 0
            }));
        }
    }

    #[test]
    fn shadow_preserves_commodity_credit() {
        let scenario = credit_disabled_scenario(&builtin_market_scenario(
            ScenarioName::CommodityCreditNeutral,
        ));
        let periods = scenario.periods;
        let mut society = Society::from_scenario(scenario);

        society.run(periods);

        assert!(society
            .loan_trades
            .iter()
            .any(|trade| matches!(trade.funding, CreditSource::Commodity)));
    }

    #[test]
    fn shadow_is_deterministic() {
        let scenario = builtin_market_scenario(ScenarioName::FiatCreditExpansion);

        let first = run_credit_disabled_shadow(&scenario);
        let second = run_credit_disabled_shadow(&scenario);

        assert_eq!(first, second);
    }

    #[test]
    fn shadow_neutralizes_all_configured_and_event_referenced_institutions() {
        let mut scenario = builtin_market_scenario(ScenarioName::FractionalReserve);
        scenario.events.extend([
            Event {
                tick: Tick(0),
                kind: EventKind::SetBankCreditPolicy {
                    bank: BankId(42),
                    policy: BankPolicy {
                        max_new_fiduciary_per_tick: Gold(5),
                        loan_present: Gold(2),
                        loan_horizon: 7,
                        loan_future_due: Gold(2),
                        enabled: true,
                    },
                },
            },
            Event {
                tick: Tick(0),
                kind: EventKind::SetIssuerPolicy {
                    issuer: IssuerId(7),
                    policy: IssuerPolicy {
                        fiscal_enabled: true,
                        credit_enabled: true,
                        max_fiscal_issue_per_tick: Gold(5),
                        max_credit_issue_per_tick: Gold(5),
                        loan_present: Gold(1),
                        loan_horizon: 7,
                        loan_future_due: Gold(1),
                    },
                },
            },
            Event {
                tick: Tick(0),
                kind: EventKind::FiatPrint {
                    issuer: IssuerId(7),
                    amount: Gold(1),
                    route: CantillonRoute::Helicopter,
                },
            },
        ]);

        let neutralized = credit_disabled_scenario(&scenario);

        for bank in [BankId(1), BankId(42)] {
            assert!(neutralized.events.iter().any(|event| matches!(
                event.kind,
                EventKind::SetReserveRatio {
                    bank: event_bank,
                    ratio: ReserveRatioBps::FULL,
                } if event_bank == bank
            )));
            assert!(neutralized.events.iter().any(|event| matches!(
                event.kind,
                EventKind::StopBankCredit { bank: event_bank } if event_bank == bank
            )));
        }
        assert!(neutralized.events.iter().any(|event| matches!(
            event.kind,
            EventKind::StopIssuerCredit {
                issuer: IssuerId(7),
            }
        )));
        assert!(neutralized
            .events
            .iter()
            .all(|event| !matches!(event.kind, EventKind::FiatPrint { .. })));
        assert!(neutralized.events.iter().all(|event| match event.kind {
            EventKind::SetBankCreditPolicy { policy, .. } =>
                !policy.enabled
                    && policy.max_new_fiduciary_per_tick == Gold::ZERO
                    && policy.loan_present == Gold::ZERO,
            EventKind::SetIssuerPolicy { policy, .. } =>
                !policy.fiscal_enabled
                    && !policy.credit_enabled
                    && policy.max_fiscal_issue_per_tick == Gold::ZERO
                    && policy.max_credit_issue_per_tick == Gold::ZERO,
            _ => true,
        }));
    }

    #[test]
    fn emerged_gold_fiat_displacement_shadow_removes_printing() {
        let scenario = builtin_market_scenario(ScenarioName::EmergedGoldFiatDisplacement);
        assert!(scenario
            .events
            .iter()
            .any(|event| matches!(event.kind, EventKind::FiatPrint { .. })));

        let neutralized = credit_disabled_scenario(&scenario);
        assert!(neutralized
            .events
            .iter()
            .all(|event| !matches!(event.kind, EventKind::FiatPrint { .. })));

        let periods = neutralized.periods;
        let mut society = Society::from_scenario(neutralized);
        society.run(periods);

        assert!(society.m3_records.iter().all(|record| {
            record.bank_credit_issued == Gold::ZERO
                && record.fiat_credit_issued == Gold::ZERO
                && record.fiat_fiscal_issued == Gold::ZERO
                && record.bank_loan_trades == 0
                && record.fiat_loan_trades == 0
        }));
    }

    #[test]
    fn decision_modules_do_not_import_shadow() {
        let manifest_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
        let files = [
            "src/agent.rs",
            "src/agio.rs",
            "src/bundle.rs",
            "src/capital.rs",
            "src/factor.rs",
            "src/market.rs",
            "src/timemarket.rs",
        ];
        let mut matches = Vec::new();

        for file in files {
            let source = fs::read_to_string(manifest_dir.join(file)).expect("source readable");
            for (line_index, line) in source.lines().enumerate() {
                if line.contains("shadow") {
                    matches.push(format!("{file}:{}:{line}", line_index + 1));
                }
            }
        }

        assert!(
            matches.is_empty(),
            "decision modules must not reference shadow: {matches:?}"
        );
    }
}
