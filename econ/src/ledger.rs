//! Monetary ledgers for M3 scenarios.

use crate::agent::{Agent, AgentId};
use crate::bank::Bank;
use crate::good::Gold;
use crate::money::{AcceptedMedia, BankRepaymentTender, PublicSpotTender};

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct BankId(pub u32);

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct IssuerId(pub u32);

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AgentMoneyBalance {
    pub agent: AgentId,
    pub public_specie: Gold,
    pub public_fiat: Gold,
    pub demand_claims: Vec<(BankId, Gold)>,
}

impl Default for AgentMoneyBalance {
    fn default() -> Self {
        Self {
            agent: AgentId(0),
            public_specie: Gold::ZERO,
            public_fiat: Gold::ZERO,
            demand_claims: Vec::new(),
        }
    }
}

impl AgentMoneyBalance {
    pub fn demand_claims_total(&self) -> Gold {
        sum_claims(&self.demand_claims)
    }

    pub fn spendable_total(&self) -> Gold {
        self.public_specie
            .saturating_add(self.public_fiat)
            .saturating_add(self.demand_claims_total())
    }

    pub fn accepted_spendable_total(&self, media: AcceptedMedia) -> Gold {
        let fiat = if media.fiat {
            self.public_fiat
        } else {
            Gold::ZERO
        };
        let claims = if media.bank_claims {
            self.demand_claims_total()
        } else {
            Gold::ZERO
        };
        let specie = if media.specie {
            self.public_specie
        } else {
            Gold::ZERO
        };
        fiat.saturating_add(claims).saturating_add(specie)
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct BaseLedger {
    pub commodity_base: Gold,
    pub fiat_base: Gold,
    pub issuer_gold_vault: Gold,
    pub issuer_fiat_unissued: Gold,
    pub bank_reserves: Gold,
    pub bank_fiat_reserves: Gold,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ClaimsLedger {
    pub demand_claims: Gold,
    pub fiduciary: Gold,
    pub time_deposits: Gold,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct MoneyStock {
    pub public_specie: Gold,
    pub public_fiat: Gold,
    pub demand_claims: Gold,
    pub bank_reserves: Gold,
    pub fiduciary: Gold,
    pub time_deposits: Gold,
}

impl MoneyStock {
    pub fn tms(&self) -> Gold {
        self.public_specie
            .saturating_add(self.public_fiat)
            .saturating_add(self.demand_claims)
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct MoneyComposition {
    pub specie: Gold,
    pub fiat: Gold,
    pub claims: Vec<(BankId, Gold)>,
}

impl MoneyComposition {
    pub fn total(&self) -> Gold {
        self.specie
            .saturating_add(self.fiat)
            .saturating_add(sum_claims(&self.claims))
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MoneyError {
    AgentNotFound(AgentId),
    InsufficientFunds {
        agent: AgentId,
        requested: Gold,
        available: Gold,
    },
    InitialClaimsExceedAgentMoney {
        bank: BankId,
        requested: Gold,
        available: Gold,
    },
    Overflow,
    ReserveBackingExceedsClaim {
        amount: Gold,
        backed_by_reserves: Gold,
    },
    ReconciliationFailed,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct MoneySystem {
    pub base: BaseLedger,
    pub claims: ClaimsLedger,
    pub balances: Vec<AgentMoneyBalance>,
}

impl MoneySystem {
    pub fn from_agents(agents: &[Agent]) -> Self {
        Self::from_agents_with_bank_reserves(agents, Gold::ZERO)
    }

    pub fn from_agents_with_bank_reserves(agents: &[Agent], bank_reserves: Gold) -> Self {
        let mut balances = agents
            .iter()
            .map(|agent| AgentMoneyBalance {
                agent: agent.id,
                public_specie: agent.gold,
                public_fiat: Gold::ZERO,
                demand_claims: Vec::new(),
            })
            .collect::<Vec<_>>();
        balances.sort_by_key(|balance| balance.agent);
        let commodity_base = sum_agent_gold(agents).saturating_add(bank_reserves);
        Self {
            base: BaseLedger {
                commodity_base,
                bank_reserves,
                ..BaseLedger::default()
            },
            claims: ClaimsLedger::default(),
            balances,
        }
    }

    pub fn from_agents_with_banks(agents: &[Agent], banks: &[Bank]) -> Result<Self, MoneyError> {
        let bank_reserves = banks
            .iter()
            .map(|bank| bank.reserves)
            .try_fold(Gold::ZERO, Gold::checked_add)
            .ok_or(MoneyError::Overflow)?;
        let mut balances = agents
            .iter()
            .map(|agent| AgentMoneyBalance {
                agent: agent.id,
                public_specie: agent.gold,
                public_fiat: Gold::ZERO,
                demand_claims: Vec::new(),
            })
            .collect::<Vec<_>>();
        balances.sort_by_key(|balance| balance.agent);
        let mut system = Self {
            base: BaseLedger {
                bank_reserves,
                ..BaseLedger::default()
            },
            claims: ClaimsLedger::default(),
            balances,
        };

        let mut deposits = banks
            .iter()
            .filter(|bank| bank.demand_deposits > Gold::ZERO)
            .map(|bank| (bank.id, bank.demand_deposits))
            .collect::<Vec<_>>();
        deposits.sort_by_key(|(bank, _)| *bank);

        for (bank, requested) in deposits {
            let mut remaining = requested;
            let mut allocated = Gold::ZERO;
            for balance in &mut system.balances {
                if remaining == Gold::ZERO {
                    break;
                }
                let amount = balance.public_specie.min(remaining);
                if amount == Gold::ZERO {
                    continue;
                }
                balance.public_specie = balance.public_specie.checked_sub(amount).ok_or(
                    MoneyError::InsufficientFunds {
                        agent: balance.agent,
                        requested: amount,
                        available: balance.public_specie,
                    },
                )?;
                add_claim(&mut balance.demand_claims, bank, amount)?;
                allocated = allocated.checked_add(amount).ok_or(MoneyError::Overflow)?;
                remaining = remaining.saturating_sub(amount);
            }
            if remaining > Gold::ZERO {
                return Err(MoneyError::InitialClaimsExceedAgentMoney {
                    bank,
                    requested,
                    available: allocated,
                });
            }
            system.claims.demand_claims = system
                .claims
                .demand_claims
                .checked_add(requested)
                .ok_or(MoneyError::Overflow)?;
        }
        let public_specie = system
            .balances
            .iter()
            .map(|balance| balance.public_specie)
            .try_fold(Gold::ZERO, Gold::checked_add)
            .ok_or(MoneyError::Overflow)?;
        system.base.commodity_base = public_specie
            .checked_add(bank_reserves)
            .ok_or(MoneyError::Overflow)?;
        system.refresh_fiduciary();
        Ok(system)
    }

    pub fn spendable_total(&self, agent: AgentId) -> Gold {
        self.balance(agent)
            .map(AgentMoneyBalance::spendable_total)
            .unwrap_or(Gold::ZERO)
    }

    fn balance(&self, agent: AgentId) -> Option<&AgentMoneyBalance> {
        self.balances
            .binary_search_by_key(&agent, |balance| balance.agent)
            .ok()
            .map(|index| &self.balances[index])
    }

    /// Drop the ledger balance entry for a removed agent (G4a real death), so the
    /// money invariant's "every balance has a live agent" check holds after the
    /// arena frees the slot. Returns the removed balance, or `None` if the agent
    /// had no entry.
    ///
    /// Callers must empty the row before forgetting it. G8a's
    /// `Society::remove_agent` does that for funded public specie by draining the
    /// specie into the returned estate; fiat and demand claims are still refused
    /// before this method is reached (G8b/G8c). A nonzero drop would silently break
    /// money conservation, so this asserts instead of gracefully refusing.
    pub fn forget_agent(&mut self, agent: AgentId) -> Option<AgentMoneyBalance> {
        let index = self
            .balances
            .binary_search_by_key(&agent, |balance| balance.agent)
            .ok()?;
        assert_eq!(
            self.balances[index].spendable_total(),
            Gold::ZERO,
            "forget_agent requires the caller to drain or refuse funded M3 balances first"
        );
        let balance = self.balances.remove(index);
        debug_assert_eq!(
            balance.spendable_total(),
            Gold::ZERO,
            "forget_agent requires the caller to drain or refuse funded M3 balances first"
        );
        Some(balance)
    }

    pub fn balance_snapshot(&self, agent: AgentId) -> Option<AgentMoneyBalance> {
        self.balance(agent).cloned()
    }

    /// Ensure `agent` has a ledger row, initialized to an empty balance when it did
    /// not already exist. Runtime insertion (`Society::add_agent`) uses this as the
    /// ledger-side mirror of `forget_agent`: the new live id is represented in the
    /// money system immediately, without minting any specie, fiat, or claims.
    pub fn ensure_agent_balance(&mut self, agent: AgentId) {
        self.ensure_balance(agent);
    }

    pub fn public_fiat(&self, agent: AgentId) -> Gold {
        self.balance(agent)
            .map(|balance| balance.public_fiat)
            .unwrap_or(Gold::ZERO)
    }

    pub fn demand_claim_on(&self, agent: AgentId, bank: BankId) -> Gold {
        self.balance(agent)
            .map(|balance| claim_amount(&balance.demand_claims, bank))
            .unwrap_or(Gold::ZERO)
    }

    pub fn demand_claim_holders(&self, bank: BankId) -> Vec<(AgentId, Gold)> {
        self.balances
            .iter()
            .filter_map(|balance| {
                let claim = claim_amount(&balance.demand_claims, bank);
                (claim > Gold::ZERO).then_some((balance.agent, claim))
            })
            .collect()
    }

    pub fn validate_specie_credit(&self, agent: AgentId, amount: Gold) -> Result<(), MoneyError> {
        if amount == Gold::ZERO {
            return Ok(());
        }
        let public_specie = self
            .balance(agent)
            .map(|balance| balance.public_specie)
            .unwrap_or(Gold::ZERO);
        public_specie
            .checked_add(amount)
            .ok_or(MoneyError::Overflow)?;
        self.base
            .commodity_base
            .checked_add(amount)
            .ok_or(MoneyError::Overflow)?;
        Ok(())
    }

    pub fn credit_specie(&mut self, agent: AgentId, amount: Gold) -> Result<(), MoneyError> {
        if amount == Gold::ZERO {
            self.ensure_balance(agent);
            return Ok(());
        }
        self.validate_specie_credit(agent, amount)?;
        let mut balance = self.balance(agent).cloned().unwrap_or(AgentMoneyBalance {
            agent,
            ..AgentMoneyBalance::default()
        });
        balance.public_specie = balance
            .public_specie
            .checked_add(amount)
            .ok_or(MoneyError::Overflow)?;
        let commodity_base = self
            .base
            .commodity_base
            .checked_add(amount)
            .ok_or(MoneyError::Overflow)?;
        self.set_balance(balance);
        self.base.commodity_base = commodity_base;
        Ok(())
    }

    pub fn validate_specie_debit(&self, agent: AgentId, amount: Gold) -> Result<(), MoneyError> {
        if amount == Gold::ZERO {
            return Ok(());
        }
        let public_specie = self
            .balance(agent)
            .map(|balance| balance.public_specie)
            .unwrap_or(Gold::ZERO);
        if public_specie < amount {
            return Err(MoneyError::InsufficientFunds {
                agent,
                requested: amount,
                available: public_specie,
            });
        }
        self.base
            .commodity_base
            .checked_sub(amount)
            .ok_or(MoneyError::ReconciliationFailed)?;
        Ok(())
    }

    pub fn debit_specie(&mut self, agent: AgentId, amount: Gold) -> Result<(), MoneyError> {
        if amount == Gold::ZERO {
            self.ensure_balance(agent);
            return Ok(());
        }
        self.validate_specie_debit(agent, amount)?;
        let mut balance = self
            .balance(agent)
            .cloned()
            .ok_or(MoneyError::AgentNotFound(agent))?;
        balance.public_specie =
            balance
                .public_specie
                .checked_sub(amount)
                .ok_or(MoneyError::InsufficientFunds {
                    agent,
                    requested: amount,
                    available: balance.public_specie,
                })?;
        self.base.commodity_base = self
            .base
            .commodity_base
            .checked_sub(amount)
            .ok_or(MoneyError::ReconciliationFailed)?;
        self.set_balance(balance);
        Ok(())
    }

    pub fn credit_fiat(&mut self, agent: AgentId, amount: Gold) -> Result<(), MoneyError> {
        if amount == Gold::ZERO {
            self.ensure_balance(agent);
            return Ok(());
        }
        let mut balance = self.balance(agent).cloned().unwrap_or(AgentMoneyBalance {
            agent,
            ..AgentMoneyBalance::default()
        });
        balance.public_fiat = balance
            .public_fiat
            .checked_add(amount)
            .ok_or(MoneyError::Overflow)?;
        let fiat_base = self
            .base
            .fiat_base
            .checked_add(amount)
            .ok_or(MoneyError::Overflow)?;
        self.set_balance(balance);
        self.base.fiat_base = fiat_base;
        Ok(())
    }

    pub fn issue_demand_claim(
        &mut self,
        bank: BankId,
        agent: AgentId,
        amount: Gold,
        backed_by_reserves: Gold,
    ) -> Result<(), MoneyError> {
        self.validate_demand_claim_issue(bank, agent, amount, backed_by_reserves)?;
        if amount == Gold::ZERO {
            self.ensure_balance(agent);
            return Ok(());
        }
        let mut balance = self.balance(agent).cloned().unwrap_or(AgentMoneyBalance {
            agent,
            ..AgentMoneyBalance::default()
        });
        let available_specie = balance.public_specie;
        balance.public_specie = balance
            .public_specie
            .checked_sub(backed_by_reserves)
            .ok_or(MoneyError::InsufficientFunds {
                agent,
                requested: backed_by_reserves,
                available: available_specie,
            })?;
        add_claim(&mut balance.demand_claims, bank, amount)?;
        let ledger_demand_claims = self
            .claims
            .demand_claims
            .checked_add(amount)
            .ok_or(MoneyError::Overflow)?;
        let bank_reserves = self
            .base
            .bank_reserves
            .checked_add(backed_by_reserves)
            .ok_or(MoneyError::Overflow)?;

        self.set_balance(balance);
        self.claims.demand_claims = ledger_demand_claims;
        self.base.bank_reserves = bank_reserves;
        self.refresh_fiduciary();
        Ok(())
    }

    pub fn redeem_demand_claim_for_specie(
        &mut self,
        agent: AgentId,
        bank: BankId,
        amount: Gold,
    ) -> Result<(), MoneyError> {
        if amount == Gold::ZERO {
            return Ok(());
        }

        let mut balance = self.balance(agent).cloned().unwrap_or(AgentMoneyBalance {
            agent,
            ..AgentMoneyBalance::default()
        });
        remove_claim(&mut balance.demand_claims, agent, bank, amount)?;
        if self.base.bank_reserves < amount {
            return Err(MoneyError::InsufficientFunds {
                agent,
                requested: amount,
                available: self.base.bank_reserves,
            });
        }
        if self.claims.demand_claims < amount {
            return Err(MoneyError::ReconciliationFailed);
        }

        balance.public_specie = balance
            .public_specie
            .checked_add(amount)
            .ok_or(MoneyError::Overflow)?;
        let demand_claims = self
            .claims
            .demand_claims
            .checked_sub(amount)
            .ok_or(MoneyError::ReconciliationFailed)?;
        let bank_reserves = self
            .base
            .bank_reserves
            .checked_sub(amount)
            .ok_or(MoneyError::ReconciliationFailed)?;

        self.set_balance(balance);
        self.claims.demand_claims = demand_claims;
        self.base.bank_reserves = bank_reserves;
        self.refresh_fiduciary();
        Ok(())
    }

    pub fn validate_demand_claim_issue(
        &self,
        bank: BankId,
        agent: AgentId,
        amount: Gold,
        backed_by_reserves: Gold,
    ) -> Result<(), MoneyError> {
        if backed_by_reserves > amount {
            return Err(MoneyError::ReserveBackingExceedsClaim {
                amount,
                backed_by_reserves,
            });
        }
        if amount == Gold::ZERO {
            return Ok(());
        }
        let mut balance = self.balance(agent).cloned().unwrap_or(AgentMoneyBalance {
            agent,
            ..AgentMoneyBalance::default()
        });
        if balance.public_specie < backed_by_reserves {
            return Err(MoneyError::InsufficientFunds {
                agent,
                requested: backed_by_reserves,
                available: balance.public_specie,
            });
        }
        let available_specie = balance.public_specie;
        balance.public_specie = balance
            .public_specie
            .checked_sub(backed_by_reserves)
            .ok_or(MoneyError::InsufficientFunds {
                agent,
                requested: backed_by_reserves,
                available: available_specie,
            })?;
        add_claim(&mut balance.demand_claims, bank, amount)?;
        self.claims
            .demand_claims
            .checked_add(amount)
            .ok_or(MoneyError::Overflow)?;
        self.base
            .bank_reserves
            .checked_add(backed_by_reserves)
            .ok_or(MoneyError::Overflow)?;
        Ok(())
    }

    pub fn transfer_spendable(
        &mut self,
        from: AgentId,
        to: AgentId,
        amount: Gold,
    ) -> Result<MoneyComposition, MoneyError> {
        self.transfer_spendable_with_media(
            from,
            to,
            amount,
            PublicSpotTender::ParAll.accepted_media(),
        )
    }

    pub fn accepted_spendable_total(&self, agent: AgentId, media: AcceptedMedia) -> Gold {
        self.balance(agent)
            .map(|balance| balance.accepted_spendable_total(media))
            .unwrap_or(Gold::ZERO)
    }

    pub fn payment_composition_with_media(
        &self,
        from: AgentId,
        amount: Gold,
        media: AcceptedMedia,
    ) -> Result<MoneyComposition, MoneyError> {
        self.debit_plan_with_media(from, amount, media)
    }

    pub fn transfer_spendable_with_media(
        &mut self,
        from: AgentId,
        to: AgentId,
        amount: Gold,
        media: AcceptedMedia,
    ) -> Result<MoneyComposition, MoneyError> {
        let composition = self.payment_composition_with_media(from, amount, media)?;
        if from == to {
            return Ok(composition);
        }
        let mut debited_from = self
            .balance(from)
            .cloned()
            .ok_or(MoneyError::AgentNotFound(from))?;
        debit_balance(&mut debited_from, from, &composition)?;
        let credited_to = self.credited_balance(to, &composition)?;

        self.set_balance(debited_from);
        self.set_balance(credited_to);
        Ok(composition)
    }

    pub fn debit_for_repayment(
        &mut self,
        from: AgentId,
        amount: Gold,
    ) -> Result<MoneyComposition, MoneyError> {
        let composition = self.repayment_composition(from, amount)?;
        self.apply_debit(from, &composition)?;
        Ok(composition)
    }

    pub fn debit_fiat_for_repayment(
        &mut self,
        from: AgentId,
        amount: Gold,
    ) -> Result<MoneyComposition, MoneyError> {
        let available = self.public_fiat(from);
        if available < amount {
            return Err(MoneyError::InsufficientFunds {
                agent: from,
                requested: amount,
                available,
            });
        }
        let composition = MoneyComposition {
            fiat: amount,
            ..MoneyComposition::default()
        };
        self.apply_debit(from, &composition)?;
        Ok(composition)
    }

    /// Legacy bank-repayment compatibility shim. This is equivalent to using
    /// `BankRepaymentTender::ParAll.accepted_media()` with the media-gated helper.
    pub fn debit_for_repayment_with_claim_limits(
        &mut self,
        from: AgentId,
        amount: Gold,
        claim_limits: &[(BankId, Gold)],
    ) -> Result<MoneyComposition, MoneyError> {
        let composition = self.repayment_composition_with_media_and_claim_limits(
            from,
            amount,
            BankRepaymentTender::ParAll.accepted_media(),
            claim_limits,
        )?;
        self.apply_debit(from, &composition)?;
        Ok(composition)
    }

    /// Debits a bank-loan repayment under an explicit media policy while still applying
    /// per-bank claim clearing limits. The debit order is intentionally the legacy
    /// bank-repayment order: public fiat, limited demand claims, then public specie.
    pub fn debit_for_repayment_with_media_and_claim_limits(
        &mut self,
        from: AgentId,
        amount: Gold,
        accepted: AcceptedMedia,
        claim_limits: &[(BankId, Gold)],
    ) -> Result<MoneyComposition, MoneyError> {
        let composition = self.repayment_composition_with_media_and_claim_limits(
            from,
            amount,
            accepted,
            claim_limits,
        )?;
        self.apply_debit(from, &composition)?;
        Ok(composition)
    }

    pub fn repayment_composition(
        &self,
        from: AgentId,
        amount: Gold,
    ) -> Result<MoneyComposition, MoneyError> {
        let available = self.spendable_total(from);
        if available < amount {
            return Err(MoneyError::InsufficientFunds {
                agent: from,
                requested: amount,
                available,
            });
        }
        self.debit_plan(from, amount)
    }

    /// Legacy bank-repayment compatibility shim. This is equivalent to using
    /// `BankRepaymentTender::ParAll.accepted_media()` with the media-gated helper.
    pub fn spendable_total_with_claim_limits(
        &self,
        agent: AgentId,
        claim_limits: &[(BankId, Gold)],
    ) -> Gold {
        self.spendable_total_with_media_and_claim_limits(
            agent,
            BankRepaymentTender::ParAll.accepted_media(),
            claim_limits,
        )
    }

    /// Returns accepted spendable funds for bank repayment under explicit media gates.
    /// Demand claims are counted only up to the supplied clearing limits.
    pub fn spendable_total_with_media_and_claim_limits(
        &self,
        agent: AgentId,
        accepted: AcceptedMedia,
        claim_limits: &[(BankId, Gold)],
    ) -> Gold {
        self.balance(agent)
            .map(|balance| {
                let fiat = if accepted.fiat {
                    balance.public_fiat
                } else {
                    Gold::ZERO
                };
                let claims = if accepted.bank_claims {
                    sum_limited_claims(&balance.demand_claims, claim_limits)
                } else {
                    Gold::ZERO
                };
                let specie = if accepted.specie {
                    balance.public_specie
                } else {
                    Gold::ZERO
                };
                fiat.saturating_add(claims).saturating_add(specie)
            })
            .unwrap_or(Gold::ZERO)
    }

    /// Legacy bank-repayment compatibility shim. This is equivalent to using
    /// `BankRepaymentTender::ParAll.accepted_media()` with the media-gated helper.
    pub fn repayment_composition_with_claim_limits(
        &self,
        from: AgentId,
        amount: Gold,
        claim_limits: &[(BankId, Gold)],
    ) -> Result<MoneyComposition, MoneyError> {
        self.repayment_composition_with_media_and_claim_limits(
            from,
            amount,
            BankRepaymentTender::ParAll.accepted_media(),
            claim_limits,
        )
    }

    /// Plans a bank-loan repayment under an explicit media policy while preserving the
    /// legacy fiat -> limited-claims -> specie debit order.
    pub fn repayment_composition_with_media_and_claim_limits(
        &self,
        from: AgentId,
        amount: Gold,
        accepted: AcceptedMedia,
        claim_limits: &[(BankId, Gold)],
    ) -> Result<MoneyComposition, MoneyError> {
        let available =
            self.spendable_total_with_media_and_claim_limits(from, accepted, claim_limits);
        if available < amount {
            return Err(MoneyError::InsufficientFunds {
                agent: from,
                requested: amount,
                available,
            });
        }
        self.debit_plan_with_media_and_claim_limits(from, amount, accepted, claim_limits)
    }

    pub fn retire_claims_from_payment(
        &mut self,
        composition: &MoneyComposition,
    ) -> Result<Gold, MoneyError> {
        let amount = checked_sum_claims(&composition.claims).ok_or(MoneyError::Overflow)?;
        if amount == Gold::ZERO {
            return Ok(Gold::ZERO);
        }
        if self.claims.demand_claims < amount {
            return Err(MoneyError::ReconciliationFailed);
        }
        self.claims.demand_claims = self
            .claims
            .demand_claims
            .checked_sub(amount)
            .ok_or(MoneyError::Overflow)?;
        self.refresh_fiduciary();
        Ok(amount)
    }

    pub fn absorb_issuer_payment(
        &mut self,
        composition: &MoneyComposition,
    ) -> Result<Gold, MoneyError> {
        if !composition.claims.is_empty() {
            return Err(MoneyError::ReconciliationFailed);
        }
        self.base.issuer_gold_vault = self
            .base
            .issuer_gold_vault
            .checked_add(composition.specie)
            .ok_or(MoneyError::Overflow)?;
        self.retire_fiat_from_payment(composition)
    }

    fn retire_fiat_from_payment(
        &mut self,
        composition: &MoneyComposition,
    ) -> Result<Gold, MoneyError> {
        if !composition.claims.is_empty() {
            return Err(MoneyError::ReconciliationFailed);
        }
        if composition.fiat == Gold::ZERO {
            return Ok(Gold::ZERO);
        }
        self.base.issuer_fiat_unissued = self
            .base
            .issuer_fiat_unissued
            .checked_add(composition.fiat)
            .ok_or(MoneyError::Overflow)?;
        Ok(composition.fiat)
    }

    pub fn absorb_bank_payment(
        &mut self,
        composition: &MoneyComposition,
    ) -> Result<Gold, MoneyError> {
        self.validate_bank_payment_absorption(composition)?;
        let claims_retired = self.retire_claims_from_payment(composition)?;
        self.base.bank_reserves = self
            .base
            .bank_reserves
            .checked_add(composition.specie)
            .ok_or(MoneyError::Overflow)?;
        self.base.bank_fiat_reserves = self
            .base
            .bank_fiat_reserves
            .checked_add(composition.fiat)
            .ok_or(MoneyError::Overflow)?;
        self.refresh_fiduciary();
        Ok(claims_retired)
    }

    pub fn validate_bank_payment_absorption(
        &self,
        composition: &MoneyComposition,
    ) -> Result<(), MoneyError> {
        let claims = checked_sum_claims(&composition.claims).ok_or(MoneyError::Overflow)?;
        if self.claims.demand_claims < claims {
            return Err(MoneyError::ReconciliationFailed);
        }
        self.base
            .bank_reserves
            .checked_add(composition.specie)
            .ok_or(MoneyError::Overflow)?;
        self.base
            .bank_fiat_reserves
            .checked_add(composition.fiat)
            .ok_or(MoneyError::Overflow)?;
        Ok(())
    }

    fn apply_debit(
        &mut self,
        from: AgentId,
        composition: &MoneyComposition,
    ) -> Result<(), MoneyError> {
        let balance = self.balance_mut(from)?;
        debit_balance(balance, from, composition)
    }

    pub fn snapshot(&self) -> MoneyStock {
        MoneyStock {
            public_specie: self.derived_public_specie(),
            public_fiat: self.derived_public_fiat(),
            demand_claims: self.claims.demand_claims,
            bank_reserves: self
                .base
                .bank_reserves
                .saturating_add(self.base.bank_fiat_reserves),
            fiduciary: self.claims.fiduciary,
            time_deposits: self.claims.time_deposits,
        }
    }

    pub fn reconcile_agent_cache(&self, agents: &mut [Agent]) {
        for agent in agents {
            agent.gold = self.spendable_total(agent.id);
        }
    }

    pub fn reconcile_agent_cache_at(&self, agents: &mut [Agent], index: usize) -> bool {
        let Some(agent) = agents.get_mut(index) else {
            return false;
        };
        agent.gold = self.spendable_total(agent.id);
        true
    }

    pub fn mirror_public_specie_from_agents(&mut self, agents: &[Agent]) -> Result<(), MoneyError> {
        if !self.is_public_specie_only() {
            return Err(MoneyError::ReconciliationFailed);
        }
        let public_specie = sum_agent_gold(agents);
        if public_specie
            .saturating_add(self.base.bank_reserves)
            .saturating_add(self.base.issuer_gold_vault)
            != self.base.commodity_base
        {
            return Err(MoneyError::ReconciliationFailed);
        }
        self.balances = agents
            .iter()
            .map(|agent| AgentMoneyBalance {
                agent: agent.id,
                public_specie: agent.gold,
                public_fiat: Gold::ZERO,
                demand_claims: Vec::new(),
            })
            .collect();
        self.balances.sort_by_key(|balance| balance.agent);
        Ok(())
    }

    pub fn invariants_hold(&self, agents: &[Agent]) -> bool {
        self.invariants_hold_with_bank_reserves(agents, None)
    }

    pub fn invariants_hold_with_banks(
        &self,
        agents: &[Agent],
        banks: &[crate::bank::Bank],
    ) -> bool {
        let Some(bank_reserves) = banks
            .iter()
            .map(|bank| bank.reserves)
            .try_fold(Gold::ZERO, Gold::checked_add)
        else {
            return false;
        };
        self.invariants_hold_with_bank_reserves(agents, Some(bank_reserves))
    }

    fn invariants_hold_with_bank_reserves(
        &self,
        agents: &[Agent],
        bank_reserves: Option<Gold>,
    ) -> bool {
        let Some(stock) = self.checked_snapshot() else {
            return false;
        };
        if bank_reserves.is_some_and(|bank_reserves| bank_reserves != self.base.bank_reserves) {
            return false;
        }
        let public_specie_reconciles = checked_sum3(
            stock.public_specie,
            self.base.bank_reserves,
            self.base.issuer_gold_vault,
        )
        .is_some_and(|total| total == self.base.commodity_base);
        let public_fiat_reconciles = checked_sum3(
            stock.public_fiat,
            self.base.bank_fiat_reserves,
            self.base.issuer_fiat_unissued,
        )
        .is_some_and(|total| total == self.base.fiat_base);
        let claims_reconcile = stock.demand_claims == self.claims.demand_claims;
        let Some(fiduciary) = self.checked_derived_fiduciary() else {
            return false;
        };
        let fiduciary_reconciles = self.claims.fiduciary == fiduciary
            && self.claims.fiduciary <= self.claims.demand_claims;
        let tms_reconciles =
            checked_sum3(stock.public_specie, stock.public_fiat, stock.demand_claims)
                .is_some_and(|total| total == stock.tms());
        let agent_caches_reconcile = agents.iter().all(|agent| {
            let spendable = self
                .balance(agent.id)
                .map(checked_spendable_total)
                .unwrap_or(Some(Gold::ZERO));
            spendable.is_some_and(|spendable| agent.gold == spendable)
        });
        let mut agent_ids = agents.iter().map(|agent| agent.id).collect::<Vec<_>>();
        agent_ids.sort();
        agent_ids.dedup();
        let balances_have_agents = self
            .balances
            .iter()
            .all(|balance| agent_ids.binary_search(&balance.agent).is_ok());

        public_specie_reconciles
            && public_fiat_reconciles
            && claims_reconcile
            && fiduciary_reconciles
            && tms_reconciles
            && agent_caches_reconcile
            && balances_have_agents
    }

    fn checked_snapshot(&self) -> Option<MoneyStock> {
        let public_specie = self
            .balances
            .iter()
            .map(|balance| balance.public_specie)
            .try_fold(Gold::ZERO, Gold::checked_add)?;
        let public_fiat = self
            .balances
            .iter()
            .map(|balance| balance.public_fiat)
            .try_fold(Gold::ZERO, Gold::checked_add)?;
        let demand_claims = self
            .balances
            .iter()
            .try_fold(Gold::ZERO, |total, balance| {
                total.checked_add(checked_sum_claims(&balance.demand_claims)?)
            })?;
        Some(MoneyStock {
            public_specie,
            public_fiat,
            demand_claims,
            bank_reserves: self
                .base
                .bank_reserves
                .checked_add(self.base.bank_fiat_reserves)?,
            fiduciary: self.claims.fiduciary,
            time_deposits: self.claims.time_deposits,
        })
    }

    fn derived_public_specie(&self) -> Gold {
        self.base
            .commodity_base
            .saturating_sub(self.base.bank_reserves)
            .saturating_sub(self.base.issuer_gold_vault)
    }

    fn derived_public_fiat(&self) -> Gold {
        self.base
            .fiat_base
            .saturating_sub(self.base.bank_fiat_reserves)
            .saturating_sub(self.base.issuer_fiat_unissued)
    }

    fn credited_balance(
        &self,
        to: AgentId,
        composition: &MoneyComposition,
    ) -> Result<AgentMoneyBalance, MoneyError> {
        let mut balance = self.balance(to).cloned().unwrap_or(AgentMoneyBalance {
            agent: to,
            ..AgentMoneyBalance::default()
        });
        balance.public_fiat = balance
            .public_fiat
            .checked_add(composition.fiat)
            .ok_or(MoneyError::Overflow)?;
        for (bank, claim) in &composition.claims {
            add_claim(&mut balance.demand_claims, *bank, *claim)?;
        }
        balance.public_specie = balance
            .public_specie
            .checked_add(composition.specie)
            .ok_or(MoneyError::Overflow)?;
        Ok(balance)
    }

    fn debit_plan(&self, agent: AgentId, amount: Gold) -> Result<MoneyComposition, MoneyError> {
        self.debit_plan_with_media(agent, amount, PublicSpotTender::ParAll.accepted_media())
    }

    fn debit_plan_with_media(
        &self,
        agent: AgentId,
        amount: Gold,
        media: AcceptedMedia,
    ) -> Result<MoneyComposition, MoneyError> {
        let balance = self
            .balance(agent)
            .ok_or(MoneyError::AgentNotFound(agent))?;
        let mut remaining = amount;
        let mut composition = MoneyComposition::default();

        if media.fiat {
            let fiat = balance.public_fiat.min(remaining);
            composition.fiat = fiat;
            remaining = remaining.saturating_sub(fiat);
        }

        if media.bank_claims {
            for (bank, held) in &balance.demand_claims {
                if remaining == Gold::ZERO {
                    break;
                }
                let claim = (*held).min(remaining);
                if claim > Gold::ZERO {
                    composition.claims.push((*bank, claim));
                    remaining = remaining.saturating_sub(claim);
                }
            }
        }

        if media.specie {
            let specie = balance.public_specie.min(remaining);
            composition.specie = specie;
            remaining = remaining.saturating_sub(specie);
        }
        if remaining == Gold::ZERO {
            Ok(composition)
        } else {
            Err(MoneyError::InsufficientFunds {
                agent,
                requested: amount,
                available: balance.accepted_spendable_total(media),
            })
        }
    }

    fn debit_plan_with_media_and_claim_limits(
        &self,
        agent: AgentId,
        amount: Gold,
        media: AcceptedMedia,
        claim_limits: &[(BankId, Gold)],
    ) -> Result<MoneyComposition, MoneyError> {
        let balance = self
            .balance(agent)
            .ok_or(MoneyError::AgentNotFound(agent))?;
        let mut remaining = amount;
        let mut composition = MoneyComposition::default();

        if media.fiat {
            let fiat = balance.public_fiat.min(remaining);
            composition.fiat = fiat;
            remaining = remaining.saturating_sub(fiat);
        }

        if media.bank_claims {
            for (bank, held) in &balance.demand_claims {
                if remaining == Gold::ZERO {
                    break;
                }
                let claim = (*held).min(claim_limit(claim_limits, *bank)).min(remaining);
                if claim > Gold::ZERO {
                    composition.claims.push((*bank, claim));
                    remaining = remaining.saturating_sub(claim);
                }
            }
        }

        if media.specie {
            let specie = balance.public_specie.min(remaining);
            composition.specie = specie;
            remaining = remaining.saturating_sub(specie);
        }
        if remaining == Gold::ZERO {
            Ok(composition)
        } else {
            Err(MoneyError::InsufficientFunds {
                agent,
                requested: amount,
                available: self.spendable_total_with_media_and_claim_limits(
                    agent,
                    media,
                    claim_limits,
                ),
            })
        }
    }

    fn balance_mut(&mut self, agent: AgentId) -> Result<&mut AgentMoneyBalance, MoneyError> {
        let index = self.ensure_balance(agent);
        self.balances
            .get_mut(index)
            .ok_or(MoneyError::AgentNotFound(agent))
    }

    fn ensure_balance(&mut self, agent: AgentId) -> usize {
        match self
            .balances
            .binary_search_by_key(&agent, |balance| balance.agent)
        {
            Ok(index) => index,
            Err(index) => {
                self.balances.insert(
                    index,
                    AgentMoneyBalance {
                        agent,
                        ..AgentMoneyBalance::default()
                    },
                );
                index
            }
        }
    }

    fn set_balance(&mut self, balance: AgentMoneyBalance) {
        match self
            .balances
            .binary_search_by_key(&balance.agent, |entry| entry.agent)
        {
            Ok(index) => self.balances[index] = balance,
            Err(index) => self.balances.insert(index, balance),
        }
    }

    fn refresh_fiduciary(&mut self) {
        self.claims.fiduciary = self.derived_fiduciary();
    }

    fn is_public_specie_only(&self) -> bool {
        self.base.fiat_base == Gold::ZERO
            && self.base.bank_reserves == Gold::ZERO
            && self.base.issuer_gold_vault == Gold::ZERO
            && self.base.issuer_fiat_unissued == Gold::ZERO
            && self.base.bank_fiat_reserves == Gold::ZERO
            && self.claims.demand_claims == Gold::ZERO
            && self.claims.fiduciary == Gold::ZERO
            && self.claims.time_deposits == Gold::ZERO
            && self.balances.iter().all(|balance| {
                balance.public_fiat == Gold::ZERO && balance.demand_claims.is_empty()
            })
    }

    fn derived_fiduciary(&self) -> Gold {
        let backing = self
            .base
            .bank_reserves
            .saturating_add(self.base.bank_fiat_reserves);
        self.claims.demand_claims.saturating_sub(backing)
    }

    fn checked_derived_fiduciary(&self) -> Option<Gold> {
        let backing = self
            .base
            .bank_reserves
            .checked_add(self.base.bank_fiat_reserves)?;
        Some(self.claims.demand_claims.saturating_sub(backing))
    }
}

fn add_claim(
    claims: &mut Vec<(BankId, Gold)>,
    bank: BankId,
    amount: Gold,
) -> Result<(), MoneyError> {
    if amount == Gold::ZERO {
        return Ok(());
    }
    match claims.binary_search_by_key(&bank, |(entry, _)| *entry) {
        Ok(index) => {
            claims[index].1 = claims[index]
                .1
                .checked_add(amount)
                .ok_or(MoneyError::Overflow)?;
        }
        Err(index) => claims.insert(index, (bank, amount)),
    }
    Ok(())
}

fn remove_claim(
    claims: &mut Vec<(BankId, Gold)>,
    agent: AgentId,
    bank: BankId,
    amount: Gold,
) -> Result<(), MoneyError> {
    if amount == Gold::ZERO {
        return Ok(());
    }
    let Ok(index) = claims.binary_search_by_key(&bank, |(entry, _)| *entry) else {
        return Err(MoneyError::InsufficientFunds {
            agent,
            requested: amount,
            available: Gold::ZERO,
        });
    };
    if claims[index].1 < amount {
        return Err(MoneyError::InsufficientFunds {
            agent,
            requested: amount,
            available: claims[index].1,
        });
    }
    claims[index].1 = claims[index].1.saturating_sub(amount);
    if claims[index].1 == Gold::ZERO {
        claims.remove(index);
    }
    Ok(())
}

fn debit_balance(
    balance: &mut AgentMoneyBalance,
    agent: AgentId,
    composition: &MoneyComposition,
) -> Result<(), MoneyError> {
    let available_fiat = balance.public_fiat;
    balance.public_fiat =
        balance
            .public_fiat
            .checked_sub(composition.fiat)
            .ok_or(MoneyError::InsufficientFunds {
                agent,
                requested: composition.fiat,
                available: available_fiat,
            })?;
    for (bank, claim) in &composition.claims {
        remove_claim(&mut balance.demand_claims, agent, *bank, *claim)?;
    }
    let available_specie = balance.public_specie;
    balance.public_specie = balance
        .public_specie
        .checked_sub(composition.specie)
        .ok_or(MoneyError::InsufficientFunds {
            agent,
            requested: composition.specie,
            available: available_specie,
        })?;
    Ok(())
}

fn sum_claims(claims: &[(BankId, Gold)]) -> Gold {
    claims.iter().fold(Gold::ZERO, |total, (_, amount)| {
        total.saturating_add(*amount)
    })
}

fn checked_sum_claims(claims: &[(BankId, Gold)]) -> Option<Gold> {
    claims
        .iter()
        .map(|(_, amount)| *amount)
        .try_fold(Gold::ZERO, Gold::checked_add)
}

fn sum_limited_claims(claims: &[(BankId, Gold)], claim_limits: &[(BankId, Gold)]) -> Gold {
    claims.iter().fold(Gold::ZERO, |total, (bank, amount)| {
        total.saturating_add((*amount).min(claim_limit(claim_limits, *bank)))
    })
}

fn claim_limit(claim_limits: &[(BankId, Gold)], bank: BankId) -> Gold {
    claim_limits
        .iter()
        .find(|(entry, _)| *entry == bank)
        .map(|(_, limit)| *limit)
        .unwrap_or(Gold::ZERO)
}

fn claim_amount(claims: &[(BankId, Gold)], bank: BankId) -> Gold {
    claims
        .binary_search_by_key(&bank, |(entry, _)| *entry)
        .ok()
        .map(|index| claims[index].1)
        .unwrap_or(Gold::ZERO)
}

fn checked_sum3(a: Gold, b: Gold, c: Gold) -> Option<Gold> {
    a.checked_add(b)?.checked_add(c)
}

fn checked_spendable_total(balance: &AgentMoneyBalance) -> Option<Gold> {
    balance
        .public_specie
        .checked_add(balance.public_fiat)?
        .checked_add(checked_sum_claims(&balance.demand_claims)?)
}

fn sum_agent_gold(agents: &[Agent]) -> Gold {
    agents
        .iter()
        .fold(Gold::ZERO, |total, agent| total.saturating_add(agent.gold))
}

#[cfg(test)]
mod tests {
    use super::{BankId, MoneyComposition, MoneyError, MoneyStock, MoneySystem};
    use crate::agent::{Agent, AgentId, Role};
    use crate::bank::{Bank, BankPolicy};
    use crate::good::{Gold, Stock};
    use crate::money::{BankRepaymentTender, PublicSpotTender, ReserveRatioBps};

    fn agent(id: u32, gold: Gold) -> Agent {
        Agent {
            id: AgentId(u64::from(id)),
            scale: Vec::new(),
            stock: Stock::new(3),
            gold,
            labor_capacity: 0,
            hunger_deficit: 0,
            roles: vec![Role::Trader],
            expect: Vec::new(),
        }
    }

    fn bank(id: u32, reserves: Gold, demand_deposits: Gold) -> Bank {
        Bank {
            id: BankId(id),
            name: "test bank",
            reserves,
            demand_deposits,
            time_deposits: Gold::ZERO,
            loans_outstanding: Gold::ZERO,
            fiduciary_issued: demand_deposits.saturating_sub(reserves),
            reserve_ratio_bps: ReserveRatioBps::FULL,
            convertible: true,
            policy: BankPolicy::default(),
        }
    }

    #[test]
    fn money_transfer_preserves_composition() {
        let mut system = MoneySystem::from_agents(&[agent(1, Gold(5)), agent(2, Gold::ZERO)]);
        system.credit_fiat(AgentId(1), Gold(2)).unwrap();
        system
            .issue_demand_claim(BankId(1), AgentId(1), Gold(3), Gold::ZERO)
            .unwrap();

        let composition = system
            .transfer_spendable(AgentId(1), AgentId(2), Gold(9))
            .unwrap();

        assert_eq!(composition.fiat, Gold(2));
        assert_eq!(composition.claims, vec![(BankId(1), Gold(3))]);
        assert_eq!(composition.specie, Gold(4));
        let receiver = system
            .balances
            .iter()
            .find(|balance| balance.agent == AgentId(2))
            .unwrap();
        assert_eq!(receiver.public_fiat, Gold(2));
        assert_eq!(receiver.demand_claims, vec![(BankId(1), Gold(3))]);
        assert_eq!(receiver.public_specie, Gold(4));
    }

    #[test]
    fn specie_only_payment_ignores_fiat_and_claims() {
        let mut system = MoneySystem::from_agents(&[agent(1, Gold(1)), agent(2, Gold::ZERO)]);
        system.credit_fiat(AgentId(1), Gold(2)).unwrap();
        system
            .issue_demand_claim(BankId(1), AgentId(1), Gold(3), Gold::ZERO)
            .unwrap();

        let composition = system
            .transfer_spendable_with_media(
                AgentId(1),
                AgentId(2),
                Gold(1),
                PublicSpotTender::SpecieOnly.accepted_media(),
            )
            .unwrap();

        assert_eq!(composition.fiat, Gold::ZERO);
        assert!(composition.claims.is_empty());
        assert_eq!(composition.specie, Gold(1));
        let sender = system.balance_snapshot(AgentId(1)).unwrap();
        assert_eq!(sender.public_fiat, Gold(2));
        assert_eq!(sender.demand_claims, vec![(BankId(1), Gold(3))]);
        assert_eq!(sender.public_specie, Gold::ZERO);
    }

    #[test]
    fn specie_only_payment_rejects_fiat_only_balance_without_mutation() {
        let mut system = MoneySystem::from_agents(&[agent(1, Gold::ZERO), agent(2, Gold::ZERO)]);
        system.credit_fiat(AgentId(1), Gold(2)).unwrap();
        let before = system.clone();

        assert_eq!(
            system.transfer_spendable_with_media(
                AgentId(1),
                AgentId(2),
                Gold(1),
                PublicSpotTender::SpecieOnly.accepted_media(),
            ),
            Err(MoneyError::InsufficientFunds {
                agent: AgentId(1),
                requested: Gold(1),
                available: Gold::ZERO,
            })
        );
        assert_eq!(system, before);
    }

    #[test]
    fn fiat_and_specie_payment_spends_fiat_before_specie() {
        let mut system = MoneySystem::from_agents(&[agent(1, Gold(2)), agent(2, Gold::ZERO)]);
        system.credit_fiat(AgentId(1), Gold(2)).unwrap();

        let composition = system
            .transfer_spendable_with_media(
                AgentId(1),
                AgentId(2),
                Gold(1),
                PublicSpotTender::FiatAndSpecie.accepted_media(),
            )
            .unwrap();

        assert_eq!(composition.fiat, Gold(1));
        assert!(composition.claims.is_empty());
        assert_eq!(composition.specie, Gold::ZERO);
        assert_eq!(
            system.balance_snapshot(AgentId(1)).unwrap().public_specie,
            Gold(2)
        );
    }

    #[test]
    fn fiat_and_specie_payment_ignores_bank_claims() {
        let mut system = MoneySystem::from_agents(&[agent(1, Gold(1)), agent(2, Gold::ZERO)]);
        system
            .issue_demand_claim(BankId(1), AgentId(1), Gold(3), Gold::ZERO)
            .unwrap();

        let composition = system
            .transfer_spendable_with_media(
                AgentId(1),
                AgentId(2),
                Gold(1),
                PublicSpotTender::FiatAndSpecie.accepted_media(),
            )
            .unwrap();

        assert_eq!(composition.fiat, Gold::ZERO);
        assert!(composition.claims.is_empty());
        assert_eq!(composition.specie, Gold(1));
        assert_eq!(
            system.balance_snapshot(AgentId(1)).unwrap().demand_claims,
            vec![(BankId(1), Gold(3))]
        );
    }

    #[test]
    fn par_all_payment_preserves_existing_order() {
        let mut system = MoneySystem::from_agents(&[agent(1, Gold(5)), agent(2, Gold::ZERO)]);
        system.credit_fiat(AgentId(1), Gold(2)).unwrap();
        system
            .issue_demand_claim(BankId(1), AgentId(1), Gold(3), Gold::ZERO)
            .unwrap();

        let composition = system
            .transfer_spendable_with_media(
                AgentId(1),
                AgentId(2),
                Gold(9),
                PublicSpotTender::ParAll.accepted_media(),
            )
            .unwrap();

        assert_eq!(composition.fiat, Gold(2));
        assert_eq!(composition.claims, vec![(BankId(1), Gold(3))]);
        assert_eq!(composition.specie, Gold(4));
    }

    #[test]
    fn par_all_claim_limit_shims_match_media_gated_helpers() {
        let mut system = MoneySystem::from_agents(&[agent(1, Gold(5))]);
        system.credit_fiat(AgentId(1), Gold(2)).unwrap();
        system
            .issue_demand_claim(BankId(1), AgentId(1), Gold(3), Gold::ZERO)
            .unwrap();
        system
            .issue_demand_claim(BankId(2), AgentId(1), Gold(4), Gold::ZERO)
            .unwrap();
        let claim_limits = [(BankId(1), Gold(2)), (BankId(2), Gold(1))];
        let par_all = BankRepaymentTender::ParAll.accepted_media();

        assert_eq!(
            system.spendable_total_with_claim_limits(AgentId(1), &claim_limits),
            system.spendable_total_with_media_and_claim_limits(AgentId(1), par_all, &claim_limits)
        );
        assert_eq!(
            system.repayment_composition_with_claim_limits(AgentId(1), Gold(8), &claim_limits),
            system.repayment_composition_with_media_and_claim_limits(
                AgentId(1),
                Gold(8),
                par_all,
                &claim_limits
            )
        );

        let mut legacy = system.clone();
        let mut media_gated = system.clone();
        let legacy_composition = legacy
            .debit_for_repayment_with_claim_limits(AgentId(1), Gold(8), &claim_limits)
            .unwrap();
        let media_composition = media_gated
            .debit_for_repayment_with_media_and_claim_limits(
                AgentId(1),
                Gold(8),
                par_all,
                &claim_limits,
            )
            .unwrap();

        assert_eq!(legacy_composition, media_composition);
        assert_eq!(legacy, media_gated);
    }

    #[test]
    fn claim_limit_helpers_accept_unsorted_limits() {
        let mut system = MoneySystem::from_agents(&[agent(1, Gold(5))]);
        system.credit_fiat(AgentId(1), Gold(2)).unwrap();
        system
            .issue_demand_claim(BankId(1), AgentId(1), Gold(3), Gold::ZERO)
            .unwrap();
        system
            .issue_demand_claim(BankId(2), AgentId(1), Gold(4), Gold::ZERO)
            .unwrap();
        let claim_limits = [(BankId(2), Gold(1)), (BankId(1), Gold(2))];
        let par_all = BankRepaymentTender::ParAll.accepted_media();
        let expected = MoneyComposition {
            fiat: Gold(2),
            claims: vec![(BankId(1), Gold(2)), (BankId(2), Gold(1))],
            specie: Gold(3),
        };

        assert_eq!(
            system.spendable_total_with_claim_limits(AgentId(1), &claim_limits),
            Gold(10)
        );
        assert_eq!(
            system.repayment_composition_with_claim_limits(AgentId(1), Gold(8), &claim_limits),
            Ok(expected.clone())
        );
        assert_eq!(
            system.repayment_composition_with_media_and_claim_limits(
                AgentId(1),
                Gold(8),
                par_all,
                &claim_limits
            ),
            Ok(expected.clone())
        );

        let debited = system
            .debit_for_repayment_with_claim_limits(AgentId(1), Gold(8), &claim_limits)
            .unwrap();
        assert_eq!(debited, expected);
    }

    #[test]
    fn demand_claim_transfer_preserves_bank_claim() {
        let mut system = MoneySystem::from_agents(&[agent(1, Gold::ZERO), agent(2, Gold::ZERO)]);
        system
            .issue_demand_claim(BankId(9), AgentId(1), Gold(3), Gold::ZERO)
            .unwrap();

        let composition = system
            .transfer_spendable(AgentId(1), AgentId(2), Gold(2))
            .unwrap();

        assert_eq!(composition.claims, vec![(BankId(9), Gold(2))]);
        let sender = system
            .balances
            .iter()
            .find(|balance| balance.agent == AgentId(1))
            .unwrap();
        let receiver = system
            .balances
            .iter()
            .find(|balance| balance.agent == AgentId(2))
            .unwrap();
        assert_eq!(sender.demand_claims, vec![(BankId(9), Gold(1))]);
        assert_eq!(receiver.demand_claims, vec![(BankId(9), Gold(2))]);
        assert_eq!(system.claims.demand_claims, Gold(3));
    }

    #[test]
    fn reserve_backed_demand_claim_moves_specie_to_reserves() {
        let mut agents = vec![agent(1, Gold(5))];
        let mut system = MoneySystem::from_agents(&agents);

        system
            .issue_demand_claim(BankId(1), AgentId(1), Gold(5), Gold(5))
            .unwrap();
        system.reconcile_agent_cache(&mut agents);

        let balance = system
            .balances
            .iter()
            .find(|balance| balance.agent == AgentId(1))
            .unwrap();
        assert_eq!(balance.public_specie, Gold::ZERO);
        assert_eq!(balance.demand_claims, vec![(BankId(1), Gold(5))]);
        assert_eq!(system.base.bank_reserves, Gold(5));
        assert_eq!(system.claims.demand_claims, Gold(5));
        assert_eq!(system.claims.fiduciary, Gold::ZERO);
        assert_eq!(agents[0].gold, Gold(5));
        assert!(system.invariants_hold(&agents));
    }

    #[test]
    fn redeem_claim_for_specie_preserves_commodity_base() {
        let mut system = MoneySystem::from_agents_with_bank_reserves(&[agent(1, Gold(1))], Gold(1));
        let commodity_base = system.base.commodity_base;
        system
            .issue_demand_claim(BankId(1), AgentId(1), Gold(1), Gold::ZERO)
            .unwrap();

        system
            .redeem_demand_claim_for_specie(AgentId(1), BankId(1), Gold(1))
            .unwrap();

        let balance = system.balance_snapshot(AgentId(1)).unwrap();
        assert_eq!(balance.public_specie, Gold(2));
        assert_eq!(balance.demand_claims_total(), Gold::ZERO);
        assert_eq!(system.claims.demand_claims, Gold::ZERO);
        assert_eq!(system.base.bank_reserves, Gold::ZERO);
        assert_eq!(system.base.commodity_base, commodity_base);
    }

    #[test]
    fn redeem_claim_for_specie_rejects_missing_claim_without_mutation() {
        let mut system = MoneySystem::from_agents_with_bank_reserves(&[agent(1, Gold(1))], Gold(1));
        let before = system.clone();

        assert_eq!(
            system.redeem_demand_claim_for_specie(AgentId(1), BankId(1), Gold(1)),
            Err(MoneyError::InsufficientFunds {
                agent: AgentId(1),
                requested: Gold(1),
                available: Gold::ZERO,
            })
        );
        assert_eq!(system, before);
    }

    #[test]
    fn redeem_claim_for_specie_rejects_insufficient_aggregate_reserves() {
        let mut system = MoneySystem::from_agents(&[agent(1, Gold::ZERO)]);
        system
            .issue_demand_claim(BankId(1), AgentId(1), Gold(1), Gold::ZERO)
            .unwrap();
        let before = system.clone();

        assert_eq!(
            system.redeem_demand_claim_for_specie(AgentId(1), BankId(1), Gold(1)),
            Err(MoneyError::InsufficientFunds {
                agent: AgentId(1),
                requested: Gold(1),
                available: Gold::ZERO,
            })
        );
        assert_eq!(system, before);
    }

    #[test]
    fn demand_claim_holders_are_agent_id_ordered() {
        let mut system = MoneySystem::from_agents(&[
            agent(3, Gold::ZERO),
            agent(1, Gold::ZERO),
            agent(2, Gold::ZERO),
        ]);
        system
            .issue_demand_claim(BankId(1), AgentId(3), Gold(1), Gold::ZERO)
            .unwrap();
        system
            .issue_demand_claim(BankId(1), AgentId(1), Gold(2), Gold::ZERO)
            .unwrap();
        system
            .issue_demand_claim(BankId(2), AgentId(2), Gold(9), Gold::ZERO)
            .unwrap();

        assert_eq!(
            system.demand_claim_holders(BankId(1)),
            vec![(AgentId(1), Gold(2)), (AgentId(3), Gold(1))]
        );
    }

    #[test]
    fn failed_transfer_does_not_debit_sender() {
        let mut system = MoneySystem::from_agents(&[agent(1, Gold::ZERO), agent(2, Gold::ZERO)]);
        system.credit_fiat(AgentId(1), Gold(1)).unwrap();
        system
            .balances
            .iter_mut()
            .find(|balance| balance.agent == AgentId(2))
            .unwrap()
            .public_fiat = Gold(u64::MAX);
        let balances_before = system.balances.clone();

        assert_eq!(
            system.transfer_spendable(AgentId(1), AgentId(2), Gold(1)),
            Err(MoneyError::Overflow)
        );
        assert_eq!(system.balances, balances_before);
    }

    #[test]
    fn specie_conservation_reconciles() {
        let agents = [agent(1, Gold(7)), agent(2, Gold(4))];
        let mut system = MoneySystem::from_agents(&agents);

        system
            .transfer_spendable(AgentId(1), AgentId(2), Gold(3))
            .unwrap();
        let mut reconciled_agents = agents.to_vec();
        system.reconcile_agent_cache(&mut reconciled_agents);

        assert!(system.invariants_hold(&reconciled_agents));
        assert_eq!(system.snapshot().public_specie, Gold(11));
        assert_eq!(system.base.commodity_base, Gold(11));
    }

    #[test]
    fn tms_excludes_bank_reserves() {
        let stock = MoneyStock {
            public_specie: Gold(10),
            public_fiat: Gold(4),
            demand_claims: Gold(6),
            bank_reserves: Gold(99),
            fiduciary: Gold::ZERO,
            time_deposits: Gold::ZERO,
        };

        assert_eq!(stock.tms(), Gold(20));
    }

    #[test]
    fn time_deposits_excluded_from_tms() {
        let stock = MoneyStock {
            public_specie: Gold(3),
            public_fiat: Gold(2),
            demand_claims: Gold(1),
            bank_reserves: Gold::ZERO,
            fiduciary: Gold::ZERO,
            time_deposits: Gold(100),
        };

        assert_eq!(stock.tms(), Gold(6));
    }

    #[test]
    fn agent_gold_cache_matches_spendable_total() {
        let mut agents = vec![agent(1, Gold::ZERO)];
        let mut system = MoneySystem::from_agents(&agents);
        system.credit_specie(AgentId(1), Gold(5)).unwrap();
        system.credit_fiat(AgentId(1), Gold(2)).unwrap();
        system
            .issue_demand_claim(BankId(7), AgentId(1), Gold(3), Gold::ZERO)
            .unwrap();

        system.reconcile_agent_cache(&mut agents);

        assert_eq!(agents[0].gold, Gold(10));
        assert_eq!(agents[0].gold, system.spendable_total(AgentId(1)));
    }

    #[test]
    fn public_specie_mirror_refuses_to_clear_non_specie_state() {
        let agents = [agent(1, Gold(5))];
        let mut system = MoneySystem::from_agents(&agents);
        system.credit_fiat(AgentId(1), Gold(2)).unwrap();
        system
            .issue_demand_claim(BankId(3), AgentId(1), Gold(4), Gold::ZERO)
            .unwrap();

        assert_eq!(
            system.mirror_public_specie_from_agents(&agents),
            Err(MoneyError::ReconciliationFailed)
        );

        let stock = system.snapshot();
        assert_eq!(stock.public_fiat, Gold(2));
        assert_eq!(stock.demand_claims, Gold(4));
        assert_eq!(system.base.fiat_base, Gold(2));
        assert_eq!(system.claims.demand_claims, Gold(4));
    }

    #[test]
    fn public_specie_mirror_refuses_bank_reserve_state() {
        let agents = [agent(1, Gold(5))];
        let mut system = MoneySystem::from_agents_with_bank_reserves(&agents, Gold(1));

        assert_eq!(
            system.mirror_public_specie_from_agents(&agents),
            Err(MoneyError::ReconciliationFailed)
        );
        assert_eq!(system.base.bank_reserves, Gold(1));
        assert_eq!(system.base.commodity_base, Gold(6));
    }

    #[test]
    fn initial_bank_deposits_are_agent_claims() {
        let agents = [agent(1, Gold(5)), agent(2, Gold(2))];
        let banks = [bank(1, Gold(2), Gold(3))];

        let system = MoneySystem::from_agents_with_banks(&agents, &banks).unwrap();

        let holder = system
            .balances
            .iter()
            .find(|balance| balance.agent == AgentId(1))
            .unwrap();
        assert_eq!(holder.public_specie, Gold(2));
        assert_eq!(holder.demand_claims, vec![(BankId(1), Gold(3))]);
        assert_eq!(system.claims.demand_claims, Gold(3));
        assert_eq!(system.claims.fiduciary, Gold(1));
        assert_eq!(system.base.commodity_base, Gold(6));
        assert_eq!(system.spendable_total(AgentId(1)), Gold(5));
        assert!(system.invariants_hold_with_banks(&agents, &banks));
    }

    #[test]
    fn initial_bank_ledger_reports_public_specie_overflow() {
        let agents = [agent(1, Gold(u64::MAX)), agent(2, Gold(1))];
        let banks = [bank(1, Gold::ZERO, Gold::ZERO)];

        assert_eq!(
            MoneySystem::from_agents_with_banks(&agents, &banks),
            Err(MoneyError::Overflow)
        );
    }

    #[test]
    fn phantom_agent_balance_fails_reconciliation() {
        let mut agents = vec![agent(1, Gold::ZERO)];
        let mut system = MoneySystem::from_agents(&agents);

        system.credit_fiat(AgentId(99), Gold(1)).unwrap();
        system.reconcile_agent_cache(&mut agents);

        assert!(!system.invariants_hold(&agents));
    }

    #[test]
    fn issuer_payment_absorbs_specie_into_vault() {
        let mut agents = vec![agent(1, Gold(2))];
        let mut system = MoneySystem::from_agents(&agents);
        let composition = MoneyComposition {
            specie: Gold(1),
            ..MoneyComposition::default()
        };

        system.apply_debit(AgentId(1), &composition).unwrap();
        let fiat_retired = system.absorb_issuer_payment(&composition).unwrap();
        system.reconcile_agent_cache(&mut agents);

        assert_eq!(fiat_retired, Gold::ZERO);
        assert_eq!(system.base.issuer_gold_vault, Gold(1));
        assert_eq!(system.snapshot().public_specie, Gold(1));
        assert!(system.invariants_hold(&agents));
    }
}
