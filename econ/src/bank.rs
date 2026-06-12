//! Bank balance sheet and fiduciary credit policy.

use crate::good::Gold;
use crate::ledger::BankId;
use crate::money::{Regime, ReserveRatioBps};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Bank {
    pub id: BankId,
    pub name: &'static str,
    pub reserves: Gold,
    pub demand_deposits: Gold,
    pub time_deposits: Gold,
    pub loans_outstanding: Gold,
    pub fiduciary_issued: Gold,
    pub reserve_ratio_bps: ReserveRatioBps,
    pub convertible: bool,
    pub policy: BankPolicy,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct BankPolicy {
    pub max_new_fiduciary_per_tick: Gold,
    pub loan_present: Gold,
    pub loan_horizon: u8,
    pub loan_future_due: Gold,
    pub enabled: bool,
}

impl Default for BankPolicy {
    fn default() -> Self {
        Self {
            max_new_fiduciary_per_tick: Gold::ZERO,
            loan_present: Gold::ZERO,
            loan_horizon: 0,
            loan_future_due: Gold::ZERO,
            enabled: false,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BankError {
    PolicyDisabled,
    RegimeDoesNotPermitFiduciary,
    FullReserve,
    CapacityExceeded,
    Overflow,
}

impl Bank {
    pub fn fiduciary_lend_capacity(&self, regime: Regime) -> Gold {
        self.fiduciary_lend_capacity_after_tick_issuance(regime, Gold::ZERO)
    }

    pub fn fiduciary_lend_capacity_after_tick_issuance(
        &self,
        regime: Regime,
        issued_this_tick: Gold,
    ) -> Gold {
        if !self.policy.enabled
            || self.policy.loan_present == Gold::ZERO
            || self.policy.loan_horizon == 0
            || self.policy.loan_future_due == Gold::ZERO
            || !regime_permits_fiduciary(regime)
            || self.reserve_ratio_bps == ReserveRatioBps::FULL
        {
            return Gold::ZERO;
        }

        let policy_capacity = self
            .policy
            .max_new_fiduciary_per_tick
            .saturating_sub(issued_this_tick);
        if !self.convertible || regime == Regime::SuspendedConvertibility {
            return policy_capacity;
        }
        policy_capacity.min(self.convertible_deposit_capacity())
    }

    pub fn can_issue_fiduciary(&self, regime: Regime, amount: Gold) -> bool {
        amount > Gold::ZERO && self.fiduciary_lend_capacity(regime) >= amount
    }

    pub fn validate_fiduciary_loan(&self, regime: Regime, amount: Gold) -> Result<(), BankError> {
        self.fiduciary_loan_totals(regime, amount).map(|_| ())
    }

    pub fn record_fiduciary_loan(&mut self, regime: Regime, amount: Gold) -> Result<(), BankError> {
        let (demand_deposits, loans_outstanding, fiduciary_issued) =
            self.fiduciary_loan_totals(regime, amount)?;
        self.demand_deposits = demand_deposits;
        self.loans_outstanding = loans_outstanding;
        self.fiduciary_issued = fiduciary_issued;
        Ok(())
    }

    pub fn retire_demand_deposit(&mut self, amount: Gold) -> Result<(), BankError> {
        self.demand_deposits = self
            .demand_deposits
            .checked_sub(amount)
            .ok_or(BankError::CapacityExceeded)?;
        Ok(())
    }

    pub fn retire_redeemed_claim(&mut self, amount: Gold) -> Result<Gold, BankError> {
        self.retire_demand_deposit(amount)?;
        let fiduciary_retired = self.fiduciary_issued.min(amount);
        self.fiduciary_issued = self
            .fiduciary_issued
            .checked_sub(fiduciary_retired)
            .ok_or(BankError::CapacityExceeded)?;
        Ok(fiduciary_retired)
    }

    pub fn credit_reserves(&mut self, amount: Gold) -> Result<(), BankError> {
        self.reserves = self
            .reserves
            .checked_add(amount)
            .ok_or(BankError::Overflow)?;
        Ok(())
    }

    pub fn debit_reserves(&mut self, amount: Gold) -> Result<(), BankError> {
        self.reserves = self
            .reserves
            .checked_sub(amount)
            .ok_or(BankError::CapacityExceeded)?;
        Ok(())
    }

    pub fn retire_fiduciary_principal(&mut self, amount: Gold) -> Result<Gold, BankError> {
        let loans_outstanding = self
            .loans_outstanding
            .checked_sub(amount)
            .ok_or(BankError::CapacityExceeded)?;
        let retired_fiduciary = self.fiduciary_issued.min(amount);
        let fiduciary_issued = self
            .fiduciary_issued
            .checked_sub(retired_fiduciary)
            .ok_or(BankError::CapacityExceeded)?;
        self.loans_outstanding = loans_outstanding;
        self.fiduciary_issued = fiduciary_issued;
        Ok(retired_fiduciary)
    }

    fn fiduciary_loan_totals(
        &self,
        regime: Regime,
        amount: Gold,
    ) -> Result<(Gold, Gold, Gold), BankError> {
        if !self.policy.enabled {
            return Err(BankError::PolicyDisabled);
        }
        if !regime_permits_fiduciary(regime) {
            return Err(BankError::RegimeDoesNotPermitFiduciary);
        }
        if self.reserve_ratio_bps == ReserveRatioBps::FULL {
            return Err(BankError::FullReserve);
        }
        if self.fiduciary_lend_capacity(regime) < amount {
            return Err(BankError::CapacityExceeded);
        }
        let demand_deposits = self
            .demand_deposits
            .checked_add(amount)
            .ok_or(BankError::Overflow)?;
        let loans_outstanding = self
            .loans_outstanding
            .checked_add(amount)
            .ok_or(BankError::Overflow)?;
        let fiduciary_issued = self
            .fiduciary_issued
            .checked_add(amount)
            .ok_or(BankError::Overflow)?;
        Ok((demand_deposits, loans_outstanding, fiduciary_issued))
    }

    fn convertible_deposit_capacity(&self) -> Gold {
        let ratio = u128::from(self.reserve_ratio_bps.0);
        if ratio == 0 {
            return Gold(u64::MAX);
        }
        let max_deposits = u128::from(self.reserves.0).saturating_mul(10_000) / ratio;
        let current = u128::from(self.demand_deposits.0);
        if max_deposits <= current {
            Gold::ZERO
        } else {
            let capacity = max_deposits - current;
            Gold(u64::try_from(capacity).unwrap_or(u64::MAX))
        }
    }
}

pub fn regime_permits_fiduciary(regime: Regime) -> bool {
    matches!(
        regime,
        Regime::FractionalConvertible | Regime::SuspendedConvertibility | Regime::Fiat
    )
}

#[cfg(test)]
mod tests {
    use super::{Bank, BankPolicy};
    use crate::good::Gold;
    use crate::ledger::BankId;
    use crate::money::{Regime, ReserveRatioBps};

    fn bank(ratio: ReserveRatioBps, convertible: bool, enabled: bool) -> Bank {
        Bank {
            id: BankId(1),
            name: "test bank",
            reserves: Gold(10),
            demand_deposits: Gold::ZERO,
            time_deposits: Gold::ZERO,
            loans_outstanding: Gold::ZERO,
            fiduciary_issued: Gold::ZERO,
            reserve_ratio_bps: ratio,
            convertible,
            policy: BankPolicy {
                max_new_fiduciary_per_tick: Gold(20),
                loan_present: Gold(1),
                loan_horizon: 7,
                loan_future_due: Gold(1),
                enabled,
            },
        }
    }

    #[test]
    fn full_reserve_bank_cannot_issue_fiduciary() {
        let bank = bank(ReserveRatioBps::FULL, true, true);

        assert_eq!(
            bank.fiduciary_lend_capacity(Regime::FractionalConvertible),
            Gold::ZERO
        );
        assert!(!bank.can_issue_fiduciary(Regime::FractionalConvertible, Gold(1)));
    }

    #[test]
    fn fractional_bank_issues_claim_within_capacity() {
        let mut bank = bank(ReserveRatioBps(2_000), true, true);

        bank.record_fiduciary_loan(Regime::FractionalConvertible, Gold(3))
            .unwrap();

        assert_eq!(bank.demand_deposits, Gold(3));
        assert_eq!(bank.loans_outstanding, Gold(3));
        assert_eq!(bank.fiduciary_issued, Gold(3));
    }

    #[test]
    fn convertible_bank_capacity_respects_reserve_ratio() {
        let mut bank = bank(ReserveRatioBps(2_000), true, true);
        bank.demand_deposits = Gold(45);

        assert_eq!(
            bank.fiduciary_lend_capacity(Regime::FractionalConvertible),
            Gold(5)
        );
    }

    #[test]
    fn suspended_bank_can_use_policy_capacity() {
        let mut bank = bank(ReserveRatioBps(2_000), false, true);
        bank.demand_deposits = Gold(100);

        assert_eq!(
            bank.fiduciary_lend_capacity(Regime::SuspendedConvertibility),
            Gold(20)
        );
    }

    #[test]
    fn fiduciary_retires_on_repayment() {
        let mut bank = bank(ReserveRatioBps(2_000), true, true);
        bank.record_fiduciary_loan(Regime::FractionalConvertible, Gold(4))
            .unwrap();

        bank.retire_demand_deposit(Gold(3)).unwrap();
        bank.retire_fiduciary_principal(Gold(3)).unwrap();

        assert_eq!(bank.demand_deposits, Gold(1));
        assert_eq!(bank.loans_outstanding, Gold(1));
        assert_eq!(bank.fiduciary_issued, Gold(1));
    }
}
