//! Fiat issuer policy and accounting.

use crate::good::Gold;
use crate::ledger::IssuerId;
use crate::money::Regime;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Issuer {
    pub id: IssuerId,
    pub fiat_issued: Gold,
    pub fiat_retired: Gold,
    pub fiat_credit_outstanding: Gold,
    pub policy: IssuerPolicy,
    /// M21 tax-receivability counters. These are pure observability: they
    /// never feed `fiat_credit_outstanding` or `credit_retired`. `taxes_levied`
    /// counts liabilities raised by `LevyTax`; the receipt and default counters
    /// move only in `settle_tax_debt_m3`.
    pub taxes_levied: Gold,
    pub tax_receipts_fiat: Gold,
    pub tax_receipts_specie: Gold,
    pub taxes_defaulted: Gold,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct IssuerPolicy {
    pub fiscal_enabled: bool,
    pub credit_enabled: bool,
    pub max_fiscal_issue_per_tick: Gold,
    pub max_credit_issue_per_tick: Gold,
    pub loan_present: Gold,
    pub loan_horizon: u8,
    pub loan_future_due: Gold,
}

impl Default for IssuerPolicy {
    fn default() -> Self {
        Self {
            fiscal_enabled: false,
            credit_enabled: false,
            max_fiscal_issue_per_tick: Gold::ZERO,
            max_credit_issue_per_tick: Gold::ZERO,
            loan_present: Gold::ZERO,
            loan_horizon: 0,
            loan_future_due: Gold::ZERO,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum IssuerError {
    FiscalDisabled,
    CreditDisabled,
    SoundGoldDisallowsFiat,
    CapacityExceeded,
    Overflow,
}

impl Issuer {
    pub fn fiscal_issue(&mut self, regime: Regime, amount: Gold) -> Result<(), IssuerError> {
        if regime == Regime::SoundGold {
            return Err(IssuerError::SoundGoldDisallowsFiat);
        }
        if !self.policy.fiscal_enabled {
            return Err(IssuerError::FiscalDisabled);
        }
        if amount > self.policy.max_fiscal_issue_per_tick {
            return Err(IssuerError::CapacityExceeded);
        }
        self.fiat_issued = self
            .fiat_issued
            .checked_add(amount)
            .ok_or(IssuerError::Overflow)?;
        Ok(())
    }

    pub fn credit_lend_capacity(&self, regime: Regime) -> Gold {
        if regime == Regime::SoundGold
            || !self.policy.credit_enabled
            || self.policy.loan_present == Gold::ZERO
            || self.policy.loan_horizon == 0
            || self.policy.loan_future_due == Gold::ZERO
        {
            return Gold::ZERO;
        }
        self.policy.max_credit_issue_per_tick
    }

    pub fn record_credit_loan(&mut self, regime: Regime, amount: Gold) -> Result<(), IssuerError> {
        if regime == Regime::SoundGold {
            return Err(IssuerError::SoundGoldDisallowsFiat);
        }
        if !self.policy.credit_enabled {
            return Err(IssuerError::CreditDisabled);
        }
        if amount > self.policy.max_credit_issue_per_tick {
            return Err(IssuerError::CapacityExceeded);
        }
        self.fiat_issued = self
            .fiat_issued
            .checked_add(amount)
            .ok_or(IssuerError::Overflow)?;
        self.fiat_credit_outstanding = self
            .fiat_credit_outstanding
            .checked_add(amount)
            .ok_or(IssuerError::Overflow)?;
        Ok(())
    }

    pub fn retire_credit_principal(&mut self, amount: Gold) -> Result<Gold, IssuerError> {
        if amount == Gold::ZERO {
            return Ok(Gold::ZERO);
        }
        if self.fiat_credit_outstanding < amount {
            return Err(IssuerError::CapacityExceeded);
        }
        self.fiat_credit_outstanding = self
            .fiat_credit_outstanding
            .checked_sub(amount)
            .ok_or(IssuerError::CapacityExceeded)?;
        self.fiat_retired = self
            .fiat_retired
            .checked_add(amount)
            .ok_or(IssuerError::Overflow)?;
        Ok(amount)
    }

    pub fn record_fiat_retirement(&mut self, amount: Gold) -> Result<(), IssuerError> {
        if amount == Gold::ZERO {
            return Ok(());
        }
        self.fiat_retired = self
            .fiat_retired
            .checked_add(amount)
            .ok_or(IssuerError::Overflow)?;
        Ok(())
    }

    /// Records a tax liability raised against this issuer at levy time. No money
    /// moves; this is the levy-side observability counter only.
    pub fn record_tax_levied(&mut self, amount: Gold) {
        self.taxes_levied = self.taxes_levied.saturating_add(amount);
    }

    /// Records the fiat and specie a settled (or partially paid) tax returned to
    /// the issuer. Never touches `fiat_credit_outstanding` or any credit metric.
    pub fn record_tax_receipt(&mut self, fiat: Gold, specie: Gold) {
        self.tax_receipts_fiat = self.tax_receipts_fiat.saturating_add(fiat);
        self.tax_receipts_specie = self.tax_receipts_specie.saturating_add(specie);
    }

    /// Records the unpaid remainder of a defaulted tax. No money moves and no
    /// enforcement follows — a defaulted tax is a measured policy weakness.
    pub fn record_tax_default(&mut self, unpaid: Gold) {
        self.taxes_defaulted = self.taxes_defaulted.saturating_add(unpaid);
    }
}

#[cfg(test)]
mod tests {
    use super::{Issuer, IssuerPolicy};
    use crate::agent::{Agent, AgentId, Role};
    use crate::cantillon::{CantillonRoute, CantillonRouter};
    use crate::expect::PriceBelief;
    use crate::good::Gold;
    use crate::good::Stock;
    use crate::ledger::{IssuerId, MoneySystem};
    use crate::money::Regime;

    fn issuer() -> Issuer {
        Issuer {
            id: IssuerId(1),
            fiat_issued: Gold::ZERO,
            fiat_retired: Gold::ZERO,
            fiat_credit_outstanding: Gold::ZERO,
            policy: IssuerPolicy {
                fiscal_enabled: true,
                credit_enabled: true,
                max_fiscal_issue_per_tick: Gold(10),
                max_credit_issue_per_tick: Gold(5),
                loan_present: Gold(1),
                loan_horizon: 4,
                loan_future_due: Gold(1),
            },
            taxes_levied: Gold::ZERO,
            tax_receipts_fiat: Gold::ZERO,
            tax_receipts_specie: Gold::ZERO,
            taxes_defaulted: Gold::ZERO,
        }
    }

    fn agent(id: u32) -> Agent {
        Agent {
            id: AgentId(u64::from(id)),
            scale: Vec::new(),
            stock: Stock::new(3),
            gold: Gold::ZERO,
            labor_capacity: 0,
            hunger_deficit: 0,
            roles: vec![Role::Household],
            expect: vec![PriceBelief::new(Gold(1), Gold(1)); 4],
        }
    }

    #[test]
    fn fiat_fiscal_issue_credits_named_agents() {
        let mut issuer = issuer();
        let mut agents = vec![agent(2), agent(1), agent(3)];
        let credits = CantillonRouter::route(
            &CantillonRoute::Agents(vec![AgentId(2), AgentId(1)]),
            &agents,
            Gold(5),
        );
        let mut money = MoneySystem::from_agents(&agents);

        issuer.fiscal_issue(Regime::Fiat, Gold(5)).unwrap();
        for (agent, amount) in credits {
            money.credit_fiat(agent, amount).unwrap();
        }
        money.reconcile_agent_cache(&mut agents);

        assert_eq!(issuer.fiat_issued, Gold(5));
        assert_eq!(issuer.fiat_credit_outstanding, Gold::ZERO);
        assert_eq!(money.public_fiat(AgentId(1)), Gold(3));
        assert_eq!(money.public_fiat(AgentId(2)), Gold(2));
        assert_eq!(money.public_fiat(AgentId(3)), Gold::ZERO);
        assert_eq!(agents[0].gold, Gold(2));
        assert_eq!(agents[1].gold, Gold(3));
        assert_eq!(agents[2].gold, Gold::ZERO);
    }

    #[test]
    fn fiat_credit_issue_creates_debt() {
        let mut issuer = issuer();

        issuer.record_credit_loan(Regime::Fiat, Gold(2)).unwrap();

        assert_eq!(issuer.fiat_issued, Gold(2));
        assert_eq!(issuer.fiat_credit_outstanding, Gold(2));
    }

    #[test]
    fn fiat_credit_repayment_retires_created_principal() {
        let mut issuer = issuer();
        issuer.record_credit_loan(Regime::Fiat, Gold(3)).unwrap();

        let retired = issuer.retire_credit_principal(Gold(2)).unwrap();

        assert_eq!(retired, Gold(2));
        assert_eq!(issuer.fiat_credit_outstanding, Gold(1));
        assert_eq!(issuer.fiat_retired, Gold(2));
    }

    #[test]
    fn tax_counters_accumulate_without_touching_credit() {
        let mut issuer = issuer();
        issuer.record_tax_levied(Gold(5));
        issuer.record_tax_receipt(Gold(3), Gold(1));
        issuer.record_tax_default(Gold(1));

        assert_eq!(issuer.taxes_levied, Gold(5));
        assert_eq!(issuer.tax_receipts_fiat, Gold(3));
        assert_eq!(issuer.tax_receipts_specie, Gold(1));
        assert_eq!(issuer.taxes_defaulted, Gold(1));
        // Tax accounting never moves the credit-contraction metrics.
        assert_eq!(issuer.fiat_credit_outstanding, Gold::ZERO);
        assert_eq!(issuer.fiat_retired, Gold::ZERO);
    }

    #[test]
    fn fiat_disabled_under_sound_gold() {
        let mut issuer = issuer();

        assert!(issuer.fiscal_issue(Regime::SoundGold, Gold(1)).is_err());
        assert!(issuer
            .record_credit_loan(Regime::SoundGold, Gold(1))
            .is_err());
        assert_eq!(issuer.fiat_issued, Gold::ZERO);
    }
}
