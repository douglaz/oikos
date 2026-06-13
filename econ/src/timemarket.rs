//! Present-gold for future-gold loan books and debt settlement.

use std::cmp::Reverse;
use std::collections::{BTreeMap, BTreeSet};

use crate::agent::{Agent, AgentId};
use crate::arena::AgentLookup;
use crate::bank::Bank;
use crate::good::Gold;
use crate::issuer::Issuer;
use crate::ledger::{BankId, IssuerId, MoneyComposition, MoneySystem};
use crate::money::{
    AcceptedMedia, BankRepaymentTender, IssuerRepaymentTender, PublicDebtTender, Regime,
    TaxReceivability,
};
use crate::project::Tick;
use crate::purpose::{CreditLender, CreditSource, DebtPurpose, LoanPurpose, M2ProjectId};
use crate::record::{
    BankRepaymentAuditRecord, DebtPaymentAuditRecord, DebtPaymentState, IssuerRepaymentAuditRecord,
    TaxAuditRecord,
};

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct DebtId(pub u64);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DebtState {
    Open,
    Settled,
    Defaulted,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DebtContract {
    pub id: DebtId,
    pub lender: CreditLender,
    pub borrower: AgentId,
    pub opened_tick: Tick,
    pub due_tick: Tick,
    pub principal: Gold,
    pub due: Gold,
    pub paid: Gold,
    pub state: DebtState,
    pub purpose: DebtPurpose,
    pub funding: CreditSource,
}

impl DebtContract {
    pub fn remaining_due(&self) -> Gold {
        self.due.saturating_sub(self.paid)
    }

    pub fn is_open(&self) -> bool {
        self.state == DebtState::Open
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LoanSide {
    Lend,
    Borrow,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LoanOrder {
    pub agent: AgentId,
    pub lender: CreditLender,
    pub side: LoanSide,
    pub present: Gold,
    pub future_limit: Gold,
    pub horizon: u8,
    pub seq: u64,
    pub expires_tick: u64,
    pub purpose: LoanPurpose,
    pub funding: CreditSource,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LoanTrade {
    pub tick: u64,
    pub lender: CreditLender,
    pub borrower: AgentId,
    pub present: Gold,
    pub future_due: Gold,
    pub horizon: u8,
    pub debt: DebtId,
    pub purpose: LoanPurpose,
    pub project: Option<M2ProjectId>,
    pub funding: CreditSource,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct DebtSettlementSummary {
    pub settled: u32,
    pub defaulted: u32,
    pub paid: Gold,
    pub unpaid: Gold,
    pub credit_retired: Gold,
}

pub struct DebtSettlementM3Context<'a> {
    pub agents: &'a mut [Agent],
    pub debts: &'a mut [DebtContract],
    pub tick: Tick,
    pub money_system: &'a mut MoneySystem,
    pub banks: &'a mut [Bank],
    pub issuers: &'a mut [Issuer],
    pub public_debt_tender: PublicDebtTender,
    pub bank_repayment_tender: BankRepaymentTender,
    pub issuer_repayment_tender: IssuerRepaymentTender,
    pub tax_receivability: TaxReceivability,
    pub debt_payment_audit: &'a mut Vec<DebtPaymentAuditRecord>,
    pub bank_repayment_audit: &'a mut Vec<BankRepaymentAuditRecord>,
    pub issuer_repayment_audit: &'a mut Vec<IssuerRepaymentAuditRecord>,
    pub tax_audit: &'a mut Vec<TaxAuditRecord>,
}

struct BankDebtSettlementContext<'a> {
    tick: Tick,
    money_system: &'a mut MoneySystem,
    banks: &'a mut [Bank],
    bank_id: BankId,
    tender: BankRepaymentTender,
    audit: &'a mut Vec<BankRepaymentAuditRecord>,
    summary: &'a mut DebtSettlementSummary,
}

struct IssuerDebtSettlementContext<'a> {
    tick: Tick,
    money_system: &'a mut MoneySystem,
    issuers: &'a mut [Issuer],
    issuer_id: IssuerId,
    tender: IssuerRepaymentTender,
    audit: &'a mut Vec<IssuerRepaymentAuditRecord>,
    summary: &'a mut DebtSettlementSummary,
}

struct IssuerDebtSettlementAuditContext<'a> {
    tick: Tick,
    issuer_id: IssuerId,
    owed: Gold,
    tender: IssuerRepaymentTender,
    audit: &'a mut Vec<IssuerRepaymentAuditRecord>,
}

struct TaxDebtSettlementContext<'a> {
    tick: Tick,
    money_system: &'a mut MoneySystem,
    issuers: &'a mut [Issuer],
    issuer_id: IssuerId,
    receivability: TaxReceivability,
    audit: &'a mut Vec<TaxAuditRecord>,
    summary: &'a mut DebtSettlementSummary,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct LoanReservations {
    present_gold: Vec<(AgentId, Gold)>,
    future_due: Vec<(AgentId, u8, Gold)>,
    bank_fiduciary_open: BTreeMap<BankId, Gold>,
    bank_fiduciary_this_tick: BTreeMap<BankId, Gold>,
    issuer_fiat_credit_open: BTreeMap<IssuerId, Gold>,
    issuer_fiat_credit_this_tick: BTreeMap<IssuerId, Gold>,
    policy_lend_order_open: BTreeMap<u64, Gold>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct PolicyCreditCommitPlan {
    amount: Gold,
    uses_open_order: bool,
}

impl LoanReservations {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn reserved_gold(&self, agent: AgentId) -> Gold {
        self.present_gold
            .iter()
            .find(|(entry, _)| *entry == agent)
            .map(|(_, gold)| *gold)
            .unwrap_or(Gold::ZERO)
    }

    pub fn reserved_future_due(&self, agent: AgentId, horizon: u8) -> Gold {
        self.future_due
            .iter()
            .filter(|(entry, entry_horizon, _)| *entry == agent && *entry_horizon == horizon)
            .fold(Gold::ZERO, |total, (_, _, due)| total.saturating_add(*due))
    }

    pub fn reserved_future_due_entries(&self, agent: AgentId) -> Vec<(u8, Gold)> {
        self.future_due
            .iter()
            .filter(|(entry, _, _)| *entry == agent)
            .map(|(_, horizon, due)| (*horizon, *due))
            .collect()
    }

    pub fn future_due_entries(&self) -> &[(AgentId, u8, Gold)] {
        &self.future_due
    }

    pub fn reserve_order<A: AgentLookup + ?Sized>(
        &mut self,
        agents: &A,
        order: &LoanOrder,
    ) -> bool {
        match order.side {
            LoanSide::Lend => {
                if !matches!(order.funding, CreditSource::Commodity) {
                    return true;
                }
                let CreditLender::Agent(lender) = order.lender else {
                    return false;
                };
                let Some(agent) = agents.get_agent(order.agent) else {
                    return false;
                };
                if agent.id != lender {
                    return false;
                }
                if agent.gold.saturating_sub(self.reserved_gold(lender)) < order.present {
                    return false;
                }
                self.add_present(lender, order.present);
                true
            }
            LoanSide::Borrow => {
                self.add_future(order.agent, order.horizon, order.future_limit);
                true
            }
        }
    }

    pub fn reserve_order_m3<A: AgentLookup + ?Sized>(
        &mut self,
        agents: &A,
        order: &LoanOrder,
        banks: &[Bank],
        issuers: &[Issuer],
        regime: Regime,
    ) -> bool {
        if matches!(order.side, LoanSide::Lend)
            && matches!(order.funding, CreditSource::BankFiduciary(_))
        {
            let CreditLender::Bank(bank_id) = order.lender else {
                return false;
            };
            if order.funding != CreditSource::BankFiduciary(bank_id) {
                return false;
            }
            let Some(bank) = banks.iter().find(|bank| bank.id == bank_id) else {
                return false;
            };
            return self.reserve_bank_fiduciary_lend(bank, order, regime);
        }
        if matches!(order.side, LoanSide::Lend)
            && matches!(order.funding, CreditSource::FiatCredit(_))
        {
            let CreditLender::Issuer(issuer_id) = order.lender else {
                return false;
            };
            if order.funding != CreditSource::FiatCredit(issuer_id) {
                return false;
            }
            let Some(issuer) = issuers.iter().find(|issuer| issuer.id == issuer_id) else {
                return false;
            };
            return self.reserve_issuer_fiat_credit_order(issuer, order, regime);
        }
        self.reserve_order(agents, order)
    }

    pub fn reset_tick_lender_capacity(&mut self) {
        self.bank_fiduciary_this_tick.clear();
        self.issuer_fiat_credit_this_tick.clear();
    }

    pub fn bank_fiduciary_issued_this_tick(&self, bank: BankId) -> Gold {
        self.bank_fiduciary_this_tick
            .get(&bank)
            .copied()
            .unwrap_or(Gold::ZERO)
    }

    pub fn bank_fiduciary_open(&self, bank: BankId) -> Gold {
        self.bank_fiduciary_open
            .get(&bank)
            .copied()
            .unwrap_or(Gold::ZERO)
    }

    pub fn bank_fiduciary_capacity(&self, bank: &Bank, regime: Regime) -> Gold {
        bank.fiduciary_lend_capacity_after_tick_issuance(
            regime,
            self.bank_fiduciary_issued_this_tick(bank.id),
        )
        .saturating_sub(self.bank_fiduciary_open(bank.id))
    }

    pub fn issuer_fiat_credit_issued_this_tick(&self, issuer: IssuerId) -> Gold {
        self.issuer_fiat_credit_this_tick
            .get(&issuer)
            .copied()
            .unwrap_or(Gold::ZERO)
    }

    pub fn issuer_fiat_credit_open(&self, issuer: IssuerId) -> Gold {
        self.issuer_fiat_credit_open
            .get(&issuer)
            .copied()
            .unwrap_or(Gold::ZERO)
    }

    pub fn issuer_fiat_credit_capacity(&self, issuer: &Issuer, regime: Regime) -> Gold {
        issuer
            .credit_lend_capacity(regime)
            .saturating_sub(self.issuer_fiat_credit_issued_this_tick(issuer.id))
            .saturating_sub(self.issuer_fiat_credit_open(issuer.id))
    }

    pub fn release_order(&mut self, order: &LoanOrder) {
        match order.side {
            LoanSide::Lend => match order.funding {
                CreditSource::Commodity => {
                    if let CreditLender::Agent(lender) = order.lender {
                        self.release_present(lender, order.present);
                    }
                }
                CreditSource::BankFiduciary(bank) => {
                    self.release_bank_fiduciary_order(bank, order);
                }
                CreditSource::FiatCredit(issuer) => {
                    self.release_issuer_fiat_credit_order(issuer, order);
                }
                // Taxes are never loan orders, so there is no reservation to release.
                CreditSource::FiatFiscal(_) | CreditSource::Tax(_) => {}
            },
            LoanSide::Borrow => self.release_future(order.agent, order.horizon, order.future_limit),
        }
    }

    fn release_filled(&mut self, order: &LoanOrder) {
        if matches!(
            order.funding,
            CreditSource::BankFiduciary(_) | CreditSource::FiatCredit(_)
        ) && matches!(order.side, LoanSide::Lend)
        {
            return;
        }
        self.release_order(order);
    }

    fn reserve_bank_fiduciary_lend(
        &mut self,
        bank: &Bank,
        order: &LoanOrder,
        regime: Regime,
    ) -> bool {
        if self.policy_lend_order_open.contains_key(&order.seq) {
            return true;
        }
        if self.bank_fiduciary_capacity(bank, regime) < order.present {
            return false;
        }
        self.add_bank_fiduciary_open(bank.id, order.present);
        self.add_policy_lend_order_open(order.seq, order.present);
        true
    }

    fn reserve_issuer_fiat_credit_order(
        &mut self,
        issuer: &Issuer,
        order: &LoanOrder,
        regime: Regime,
    ) -> bool {
        if self.policy_lend_order_open.contains_key(&order.seq) {
            return true;
        }
        if self.issuer_fiat_credit_capacity(issuer, regime) < order.present {
            return false;
        }
        self.add_issuer_fiat_credit_open(issuer.id, order.present);
        self.add_policy_lend_order_open(order.seq, order.present);
        true
    }

    fn release_bank_fiduciary_order(&mut self, bank: BankId, order: &LoanOrder) {
        let released = self.release_policy_lend_order_open(order.seq);
        if released > Gold::ZERO {
            self.release_bank_fiduciary_open(bank, released);
        }
    }

    fn release_issuer_fiat_credit_order(&mut self, issuer: IssuerId, order: &LoanOrder) {
        let released = self.release_policy_lend_order_open(order.seq);
        if released > Gold::ZERO {
            self.release_issuer_fiat_credit_open(issuer, released);
        }
    }

    fn commit_bank_fiduciary(
        &mut self,
        bank: BankId,
        order_seq: u64,
        plan: PolicyCreditCommitPlan,
    ) {
        if plan.amount == Gold::ZERO {
            return;
        }
        if plan.uses_open_order {
            let released = self.release_policy_lend_order_amount(order_seq, plan.amount);
            debug_assert_eq!(released, plan.amount);
            self.release_bank_fiduciary_open(bank, released);
        }
        let issued = self
            .bank_fiduciary_this_tick
            .entry(bank)
            .or_insert(Gold::ZERO);
        *issued = issued
            .checked_add(plan.amount)
            .expect("bank fiduciary preflight checked per-tick counter capacity");
    }

    fn bank_fiduciary_commit_plan(
        &self,
        bank: &Bank,
        regime: Regime,
        order_seq: u64,
        amount: Gold,
    ) -> Option<PolicyCreditCommitPlan> {
        if amount == Gold::ZERO {
            return Some(PolicyCreditCommitPlan {
                amount,
                uses_open_order: false,
            });
        }
        let order_open = self.policy_lend_order_open(order_seq);
        let uses_open_order = order_open > Gold::ZERO;
        let open = self.bank_fiduciary_open(bank.id);
        let usable_open = if uses_open_order {
            if order_open < amount {
                return None;
            }
            amount
        } else {
            Gold::ZERO
        };
        let capacity = bank
            .fiduciary_lend_capacity_after_tick_issuance(
                regime,
                self.bank_fiduciary_issued_this_tick(bank.id),
            )
            .saturating_sub(open.saturating_sub(usable_open));
        if capacity < amount
            || self
                .bank_fiduciary_issued_this_tick(bank.id)
                .checked_add(amount)
                .is_none()
        {
            return None;
        }
        Some(PolicyCreditCommitPlan {
            amount,
            uses_open_order,
        })
    }

    fn commit_issuer_fiat_credit(
        &mut self,
        issuer: IssuerId,
        order_seq: u64,
        plan: PolicyCreditCommitPlan,
    ) {
        if plan.amount == Gold::ZERO {
            return;
        }
        if plan.uses_open_order {
            let released = self.release_policy_lend_order_amount(order_seq, plan.amount);
            debug_assert_eq!(released, plan.amount);
            self.release_issuer_fiat_credit_open(issuer, released);
        }
        let issued = self
            .issuer_fiat_credit_this_tick
            .entry(issuer)
            .or_insert(Gold::ZERO);
        *issued = issued
            .checked_add(plan.amount)
            .expect("issuer fiat-credit preflight checked per-tick counter capacity");
    }

    fn issuer_fiat_credit_commit_plan(
        &self,
        issuer: &Issuer,
        regime: Regime,
        order_seq: u64,
        amount: Gold,
    ) -> Option<PolicyCreditCommitPlan> {
        if amount == Gold::ZERO {
            return Some(PolicyCreditCommitPlan {
                amount,
                uses_open_order: false,
            });
        }
        let order_open = self.policy_lend_order_open(order_seq);
        let uses_open_order = order_open > Gold::ZERO;
        let open = self.issuer_fiat_credit_open(issuer.id);
        let usable_open = if uses_open_order {
            if order_open < amount {
                return None;
            }
            amount
        } else {
            Gold::ZERO
        };
        let issued_this_tick = self.issuer_fiat_credit_issued_this_tick(issuer.id);
        let capacity = issuer
            .credit_lend_capacity(regime)
            .saturating_sub(issued_this_tick)
            .saturating_sub(open.saturating_sub(usable_open));
        if capacity < amount || issued_this_tick.checked_add(amount).is_none() {
            return None;
        }
        Some(PolicyCreditCommitPlan {
            amount,
            uses_open_order,
        })
    }

    fn add_bank_fiduciary_open(&mut self, bank: BankId, amount: Gold) {
        add_gold_map_entry(&mut self.bank_fiduciary_open, bank, amount);
    }

    fn release_bank_fiduciary_open(&mut self, bank: BankId, amount: Gold) {
        release_gold_map_entry(&mut self.bank_fiduciary_open, bank, amount);
    }

    fn add_issuer_fiat_credit_open(&mut self, issuer: IssuerId, amount: Gold) {
        add_gold_map_entry(&mut self.issuer_fiat_credit_open, issuer, amount);
    }

    fn release_issuer_fiat_credit_open(&mut self, issuer: IssuerId, amount: Gold) {
        release_gold_map_entry(&mut self.issuer_fiat_credit_open, issuer, amount);
    }

    fn policy_lend_order_open(&self, order_seq: u64) -> Gold {
        self.policy_lend_order_open
            .get(&order_seq)
            .copied()
            .unwrap_or(Gold::ZERO)
    }

    fn add_policy_lend_order_open(&mut self, order_seq: u64, amount: Gold) {
        add_gold_map_entry(&mut self.policy_lend_order_open, order_seq, amount);
    }

    fn release_policy_lend_order_open(&mut self, order_seq: u64) -> Gold {
        self.policy_lend_order_open
            .remove(&order_seq)
            .unwrap_or(Gold::ZERO)
    }

    fn release_policy_lend_order_amount(&mut self, order_seq: u64, amount: Gold) -> Gold {
        if amount == Gold::ZERO {
            return Gold::ZERO;
        }
        let (released, remaining) = {
            let Some(open) = self.policy_lend_order_open.get_mut(&order_seq) else {
                return Gold::ZERO;
            };
            let released = (*open).min(amount);
            (released, (*open).saturating_sub(released))
        };
        if remaining == Gold::ZERO {
            self.policy_lend_order_open.remove(&order_seq);
        } else {
            self.policy_lend_order_open.insert(order_seq, remaining);
        }
        released
    }

    fn add_present(&mut self, agent: AgentId, amount: Gold) {
        if amount == Gold::ZERO {
            return;
        }
        if let Some((_, reserved)) = self
            .present_gold
            .iter_mut()
            .find(|(entry, _)| *entry == agent)
        {
            *reserved = reserved.saturating_add(amount);
        } else {
            self.present_gold.push((agent, amount));
            self.present_gold.sort_by_key(|(entry, _)| *entry);
        }
    }

    fn add_future(&mut self, agent: AgentId, horizon: u8, due: Gold) {
        if due == Gold::ZERO {
            return;
        }
        self.future_due.push((agent, horizon, due));
        self.future_due
            .sort_by_key(|(entry, entry_horizon, _)| (*entry, *entry_horizon));
    }

    fn release_present(&mut self, agent: AgentId, amount: Gold) {
        if let Some((_, reserved)) = self
            .present_gold
            .iter_mut()
            .find(|(entry, _)| *entry == agent)
        {
            *reserved = reserved.saturating_sub(amount);
        }
        self.present_gold.retain(|(_, gold)| *gold > Gold::ZERO);
    }

    fn release_future(&mut self, agent: AgentId, horizon: u8, due: Gold) {
        if let Some(index) = self
            .future_due
            .iter()
            .position(|entry| entry.0 == agent && entry.1 == horizon && entry.2 == due)
        {
            self.future_due.remove(index);
        }
    }
}

fn add_gold_map_entry<K: Copy + Ord>(entries: &mut BTreeMap<K, Gold>, key: K, amount: Gold) {
    if amount == Gold::ZERO {
        return;
    }
    entries
        .entry(key)
        .and_modify(|existing| *existing = existing.saturating_add(amount))
        .or_insert(amount);
}

fn release_gold_map_entry<K: Copy + Ord>(entries: &mut BTreeMap<K, Gold>, key: K, amount: Gold) {
    if let Some(existing) = entries.get_mut(&key) {
        *existing = existing.saturating_sub(amount);
    }
    if entries.get(&key).copied() == Some(Gold::ZERO) {
        entries.remove(&key);
    }
}

pub struct LoanOrderBook {
    pub lends: BTreeMap<(Gold, u64), LoanOrder>,
    pub borrows: BTreeMap<(Reverse<Gold>, u64), LoanOrder>,
    live_seqs: BTreeSet<u64>,
    pub tape: Vec<LoanTrade>,
}

pub struct LoanM3Context<'a> {
    pub agents: &'a mut [Agent],
    pub reservations: &'a mut LoanReservations,
    pub debts: &'a mut Vec<DebtContract>,
    pub next_debt_id: &'a mut u64,
    pub money_system: &'a mut MoneySystem,
    pub banks: &'a mut [Bank],
    pub issuers: &'a mut [Issuer],
    pub regime: Regime,
}

impl LoanOrderBook {
    pub fn new() -> Self {
        Self {
            lends: BTreeMap::new(),
            borrows: BTreeMap::new(),
            live_seqs: BTreeSet::new(),
            tape: Vec::new(),
        }
    }

    pub fn add_order(
        &mut self,
        order: LoanOrder,
        tick: u64,
        agents: &mut [Agent],
        reservations: &mut LoanReservations,
        debts: &mut Vec<DebtContract>,
        next_debt_id: &mut u64,
    ) -> Vec<LoanTrade> {
        let mut settlement = LoanSettlement {
            agents,
            reservations,
            debts,
            next_debt_id,
            money_system: None,
            banks: None,
            issuers: None,
            regime: Regime::SoundGold,
        };
        self.add_order_inner(order, tick, &mut settlement)
    }

    pub fn add_order_m3(
        &mut self,
        order: LoanOrder,
        tick: u64,
        context: LoanM3Context<'_>,
    ) -> Vec<LoanTrade> {
        let mut settlement = LoanSettlement {
            agents: context.agents,
            reservations: context.reservations,
            debts: context.debts,
            next_debt_id: context.next_debt_id,
            money_system: Some(context.money_system),
            banks: Some(context.banks),
            issuers: Some(context.issuers),
            regime: context.regime,
        };
        self.add_order_inner(order, tick, &mut settlement)
    }

    fn add_order_inner(
        &mut self,
        mut order: LoanOrder,
        tick: u64,
        settlement: &mut LoanSettlement<'_>,
    ) -> Vec<LoanTrade> {
        if order.present == Gold::ZERO
            || order.future_limit == Gold::ZERO
            || order.horizon == 0
            || order.expires_tick <= tick
        {
            settlement.reservations.release_order(&order);
            return Vec::new();
        }

        let trades = match order.side {
            LoanSide::Borrow => self.match_borrow(&mut order, tick, settlement),
            LoanSide::Lend => self.match_lend(&mut order, tick, settlement),
        };

        if !trades
            .iter()
            .any(|trade| trade.lender == order.lender || trade.borrower == order.agent)
        {
            if !settlement.ensure_resting_order_reserved(&order) {
                settlement.reservations.release_order(&order);
                return trades;
            }
            self.insert(order);
        }
        self.tape.extend(trades.iter().cloned());
        trades
    }

    pub fn purge_expired(&mut self, tick: u64, reservations: &mut LoanReservations) -> u32 {
        let mut expired = 0;
        let lend_keys = self
            .lends
            .iter()
            .filter(|(_, order)| order.expires_tick <= tick)
            .map(|(key, _)| *key)
            .collect::<Vec<_>>();
        for key in lend_keys {
            if let Some(order) = self.lends.remove(&key) {
                self.live_seqs.remove(&order.seq);
                reservations.release_order(&order);
                expired += 1;
            }
        }

        let borrow_keys = self
            .borrows
            .iter()
            .filter(|(_, order)| order.expires_tick <= tick)
            .map(|(key, _)| *key)
            .collect::<Vec<_>>();
        for key in borrow_keys {
            if let Some(order) = self.borrows.remove(&key) {
                self.live_seqs.remove(&order.seq);
                reservations.release_order(&order);
                expired += 1;
            }
        }
        expired
    }

    pub fn cancel_lender(
        &mut self,
        lender: CreditLender,
        reservations: &mut LoanReservations,
    ) -> u32 {
        let keys = self
            .lends
            .iter()
            .filter(|(_, order)| order.lender == lender)
            .map(|(key, _)| *key)
            .collect::<Vec<_>>();
        let mut canceled = 0;
        for key in keys {
            if let Some(order) = self.lends.remove(&key) {
                self.live_seqs.remove(&order.seq);
                reservations.release_order(&order);
                canceled += 1;
            }
        }
        canceled
    }

    /// Cancel every resting order an agent posted on either side (lend or
    /// borrow), releasing each order's reservation. This is the time-market
    /// analogue of cancelling an agent's resting spot quotes: it removes the
    /// agent's own orders and un-earmarks the gold it had reserved against them
    /// — the gold stays with the agent, nothing settles to a counterparty. The
    /// G1 death tombstone calls it so a dead agent's resting credit orders can
    /// never match a later counterparty (G1 itself runs M1, where the loan book
    /// is empty; this keeps the public death hook's "posts no orders, holdings
    /// frozen" contract complete for any society kind). Returns the count
    /// cancelled.
    pub fn cancel_agent(&mut self, agent: AgentId, reservations: &mut LoanReservations) -> u32 {
        let mut canceled = 0;
        let lend_keys = self
            .lends
            .iter()
            .filter(|(_, order)| order.agent == agent)
            .map(|(key, _)| *key)
            .collect::<Vec<_>>();
        for key in lend_keys {
            if let Some(order) = self.lends.remove(&key) {
                self.live_seqs.remove(&order.seq);
                reservations.release_order(&order);
                canceled += 1;
            }
        }
        let borrow_keys = self
            .borrows
            .iter()
            .filter(|(_, order)| order.agent == agent)
            .map(|(key, _)| *key)
            .collect::<Vec<_>>();
        for key in borrow_keys {
            if let Some(order) = self.borrows.remove(&key) {
                self.live_seqs.remove(&order.seq);
                reservations.release_order(&order);
                canceled += 1;
            }
        }
        canceled
    }

    pub fn has_live(&self, agent: AgentId, side: LoanSide, present: Gold, horizon: u8) -> bool {
        let matches_order = |order: &LoanOrder| {
            is_agent_order_for(order, agent)
                && order.side == side
                && order.present == present
                && order.horizon == horizon
        };
        match side {
            LoanSide::Lend => self.lends.values().any(matches_order),
            LoanSide::Borrow => self.borrows.values().any(matches_order),
        }
    }

    pub fn has_live_with_purpose(
        &self,
        agent: AgentId,
        side: LoanSide,
        present: Gold,
        horizon: u8,
        purpose: &LoanPurpose,
    ) -> bool {
        let matches_order = |order: &LoanOrder| {
            is_agent_order_for(order, agent)
                && order.side == side
                && order.present == present
                && order.horizon == horizon
                && order.purpose == *purpose
        };
        match side {
            LoanSide::Lend => self.lends.values().any(matches_order),
            LoanSide::Borrow => self.borrows.values().any(matches_order),
        }
    }

    pub fn live_lender_order_count(
        &self,
        lender: CreditLender,
        funding: CreditSource,
        present: Gold,
        future_limit: Gold,
        horizon: u8,
    ) -> usize {
        self.lends
            .values()
            .filter(|order| {
                order.lender == lender
                    && order.funding == funding
                    && order.present == present
                    && order.future_limit == future_limit
                    && order.horizon == horizon
            })
            .count()
    }

    pub fn crossable_borrow_count(&self, present: Gold, future_limit: Gold, horizon: u8) -> usize {
        self.borrows
            .values()
            .filter(|order| {
                order.present == present
                    && order.horizon == horizon
                    && order.future_limit >= future_limit
            })
            .count()
    }

    pub fn live_order_counts(&self) -> (u32, u32) {
        (
            u32::try_from(self.lends.len()).unwrap_or(u32::MAX),
            u32::try_from(self.borrows.len()).unwrap_or(u32::MAX),
        )
    }

    fn match_borrow(
        &mut self,
        order: &mut LoanOrder,
        tick: u64,
        settlement: &mut LoanSettlement<'_>,
    ) -> Vec<LoanTrade> {
        let mut trades = Vec::new();
        let mut skipped_resting = BTreeSet::new();
        loop {
            let mut selected = None;
            for (key, resting) in &self.lends {
                if skipped_resting.contains(&resting.seq) {
                    continue;
                }
                if resting.future_limit > order.future_limit {
                    break;
                }
                if loan_orders_can_match(resting, order) {
                    selected = Some((*key, resting.clone()));
                    break;
                }
            }
            let Some((key, resting)) = selected else {
                break;
            };
            self.lends.remove(&key);
            self.live_seqs.remove(&resting.seq);
            let Some(trade) =
                apply_loan_trade(tick, &resting, order, resting.future_limit, settlement)
            else {
                skipped_resting.insert(resting.seq);
                self.insert(resting);
                continue;
            };
            trades.push(trade);
            break;
        }
        trades
    }

    fn match_lend(
        &mut self,
        order: &mut LoanOrder,
        tick: u64,
        settlement: &mut LoanSettlement<'_>,
    ) -> Vec<LoanTrade> {
        let mut trades = Vec::new();
        let mut skipped_resting = BTreeSet::new();
        loop {
            let mut selected = None;
            for (key, resting) in &self.borrows {
                if skipped_resting.contains(&resting.seq) {
                    continue;
                }
                if resting.future_limit < order.future_limit {
                    break;
                }
                if loan_orders_can_match(order, resting) {
                    selected = Some((*key, resting.clone()));
                    break;
                }
            }
            let Some((key, resting)) = selected else {
                break;
            };
            self.borrows.remove(&key);
            self.live_seqs.remove(&resting.seq);
            let Some(trade) = apply_loan_trade(
                tick,
                order,
                &resting,
                future_due_when_lend_crosses_borrow(order, &resting),
                settlement,
            ) else {
                skipped_resting.insert(resting.seq);
                self.insert(resting);
                continue;
            };
            trades.push(trade);
            break;
        }
        trades
    }

    fn insert(&mut self, order: LoanOrder) {
        self.live_seqs.insert(order.seq);
        match order.side {
            LoanSide::Lend => {
                self.lends.insert((order.future_limit, order.seq), order);
            }
            LoanSide::Borrow => {
                self.borrows
                    .insert((Reverse(order.future_limit), order.seq), order);
            }
        }
    }
}

impl Default for LoanOrderBook {
    fn default() -> Self {
        Self::new()
    }
}

struct LoanSettlement<'a> {
    agents: &'a mut [Agent],
    reservations: &'a mut LoanReservations,
    debts: &'a mut Vec<DebtContract>,
    next_debt_id: &'a mut u64,
    money_system: Option<&'a mut MoneySystem>,
    banks: Option<&'a mut [Bank]>,
    issuers: Option<&'a mut [Issuer]>,
    regime: Regime,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct BankCreditApplication {
    bank: BankId,
    order_seq: u64,
    borrower: AgentId,
    amount: Gold,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct IssuerCreditApplication {
    issuer: IssuerId,
    order_seq: u64,
    borrower: AgentId,
    amount: Gold,
}

impl LoanSettlement<'_> {
    fn ensure_resting_order_reserved(&mut self, order: &LoanOrder) -> bool {
        if !matches!(order.side, LoanSide::Lend)
            || !matches!(
                order.funding,
                CreditSource::BankFiduciary(_) | CreditSource::FiatCredit(_)
            )
        {
            return true;
        }
        let Some(banks) = self.banks.as_deref() else {
            return false;
        };
        let Some(issuers) = self.issuers.as_deref() else {
            return false;
        };
        self.reservations
            .reserve_order_m3(self.agents, order, banks, issuers, self.regime)
    }
}

fn apply_loan_trade(
    tick: u64,
    lend: &LoanOrder,
    borrow: &LoanOrder,
    future_due: Gold,
    settlement: &mut LoanSettlement<'_>,
) -> Option<LoanTrade> {
    if lend.side != LoanSide::Lend
        || borrow.side != LoanSide::Borrow
        || !loan_orders_can_match(lend, borrow)
    {
        return None;
    }
    if let CreditLender::Agent(lender) = lend.lender {
        if lender == borrow.agent {
            return None;
        }
    }
    if lend.funding != expected_funding_for_lender(lend.lender) {
        return None;
    }
    let id = DebtId(*settlement.next_debt_id);
    let next_debt_id = settlement.next_debt_id.checked_add(1)?;

    match lend.funding {
        CreditSource::Commodity => apply_commodity_loan(lend, borrow, settlement)?,
        CreditSource::BankFiduciary(bank_id) => {
            let CreditLender::Bank(lender_bank) = lend.lender else {
                return None;
            };
            if lender_bank != bank_id {
                return None;
            }
            let borrower_pos = settlement
                .agents
                .iter()
                .position(|agent| agent.id == borrow.agent)?;
            let money_system = settlement.money_system.as_deref_mut()?;
            let banks = settlement.banks.as_deref_mut()?;
            apply_bank_fiduciary_loan(
                money_system,
                banks,
                settlement.reservations,
                settlement.regime,
                BankCreditApplication {
                    bank: bank_id,
                    order_seq: lend.seq,
                    borrower: borrow.agent,
                    amount: lend.present,
                },
            )?;
            money_system.reconcile_agent_cache_at(settlement.agents, borrower_pos);
            settlement.reservations.release_filled(lend);
            settlement.reservations.release_filled(borrow);
        }
        CreditSource::FiatCredit(issuer_id) => {
            let CreditLender::Issuer(lender_issuer) = lend.lender else {
                return None;
            };
            if lender_issuer != issuer_id {
                return None;
            }
            let borrower_pos = settlement
                .agents
                .iter()
                .position(|agent| agent.id == borrow.agent)?;
            let money_system = settlement.money_system.as_deref_mut()?;
            let issuers = settlement.issuers.as_deref_mut()?;
            apply_issuer_fiat_credit_loan(
                money_system,
                issuers,
                settlement.reservations,
                settlement.regime,
                IssuerCreditApplication {
                    issuer: issuer_id,
                    order_seq: lend.seq,
                    borrower: borrow.agent,
                    amount: lend.present,
                },
            )?;
            money_system.reconcile_agent_cache_at(settlement.agents, borrower_pos);
            settlement.reservations.release_filled(lend);
            settlement.reservations.release_filled(borrow);
        }
        // Taxes are not lent into existence; `LevyTax` seeds them directly.
        CreditSource::FiatFiscal(_) | CreditSource::Tax(_) => return None,
    }

    *settlement.next_debt_id = next_debt_id;
    let purpose = match borrow.purpose {
        LoanPurpose::Consumption => DebtPurpose::Consumption,
        LoanPurpose::ProjectFunding(plan) => DebtPurpose::ProjectFunding {
            plan,
            project: None,
        },
    };
    let debt = DebtContract {
        id,
        lender: lend.lender,
        borrower: borrow.agent,
        opened_tick: Tick(tick),
        due_tick: Tick(tick.saturating_add(u64::from(lend.horizon))),
        principal: lend.present,
        due: future_due,
        paid: Gold::ZERO,
        state: DebtState::Open,
        purpose,
        funding: lend.funding,
    };
    settlement.debts.push(debt);

    Some(LoanTrade {
        tick,
        lender: lend.lender,
        borrower: borrow.agent,
        present: lend.present,
        future_due,
        horizon: lend.horizon,
        debt: id,
        purpose: borrow.purpose.clone(),
        project: None,
        funding: lend.funding,
    })
}

fn expected_funding_for_lender(lender: CreditLender) -> CreditSource {
    match lender {
        CreditLender::Agent(_) => CreditSource::Commodity,
        CreditLender::Bank(bank) => CreditSource::BankFiduciary(bank),
        CreditLender::Issuer(issuer) => CreditSource::FiatCredit(issuer),
    }
}

fn is_agent_order_for(order: &LoanOrder, agent: AgentId) -> bool {
    order.agent == agent
        && order.funding == CreditSource::Commodity
        && matches!(order.lender, CreditLender::Agent(lender) if lender == agent)
}

fn loan_orders_can_match(lend: &LoanOrder, borrow: &LoanOrder) -> bool {
    lend.present == borrow.present
        && lend.horizon == borrow.horizon
        && !matches!(lend.lender, CreditLender::Agent(lender) if lender == borrow.agent)
}

fn future_due_when_lend_crosses_borrow(
    incoming_lend: &LoanOrder,
    resting_borrow: &LoanOrder,
) -> Gold {
    if matches!(
        incoming_lend.funding,
        CreditSource::BankFiduciary(_) | CreditSource::FiatCredit(_)
    ) {
        incoming_lend.future_limit
    } else {
        resting_borrow.future_limit
    }
}

fn apply_commodity_loan(
    lend: &LoanOrder,
    borrow: &LoanOrder,
    settlement: &mut LoanSettlement<'_>,
) -> Option<()> {
    let CreditLender::Agent(lender) = lend.lender else {
        return None;
    };
    let agent_index = sorted_agent_index(settlement.agents);
    let lender_pos = agent_position(&agent_index, lender)?;
    let borrower_pos = agent_position(&agent_index, borrow.agent)?;
    if lender_pos == borrower_pos {
        return None;
    }
    if let Some(money_system) = settlement.money_system.as_deref_mut() {
        money_system
            .transfer_spendable(lender, borrow.agent, lend.present)
            .ok()?;
        money_system.reconcile_agent_cache_at(settlement.agents, lender_pos);
        money_system.reconcile_agent_cache_at(settlement.agents, borrower_pos);
    } else {
        if settlement.agents[lender_pos].gold < lend.present {
            return None;
        }
        let borrower_gold = settlement.agents[borrower_pos]
            .gold
            .checked_add(lend.present)?;
        let lender_gold = settlement.agents[lender_pos]
            .gold
            .checked_sub(lend.present)?;
        settlement.agents[lender_pos].gold = lender_gold;
        settlement.agents[borrower_pos].gold = borrower_gold;
    }
    settlement.reservations.release_filled(lend);
    settlement.reservations.release_filled(borrow);
    Some(())
}

fn apply_bank_fiduciary_loan(
    money_system: &mut MoneySystem,
    banks: &mut [Bank],
    reservations: &mut LoanReservations,
    regime: Regime,
    application: BankCreditApplication,
) -> Option<()> {
    let bank_id = application.bank;
    let amount = application.amount;
    let bank_pos = banks.iter().position(|bank| bank.id == bank_id)?;
    let commit_plan = reservations.bank_fiduciary_commit_plan(
        &banks[bank_pos],
        regime,
        application.order_seq,
        amount,
    )?;
    let mut staged_money = money_system.clone();
    let mut staged_bank = banks[bank_pos].clone();

    staged_money
        .issue_demand_claim(bank_id, application.borrower, amount, Gold::ZERO)
        .ok()?;
    staged_bank.record_fiduciary_loan(regime, amount).ok()?;
    reservations.commit_bank_fiduciary(bank_id, application.order_seq, commit_plan);

    *money_system = staged_money;
    banks[bank_pos] = staged_bank;
    Some(())
}

fn apply_issuer_fiat_credit_loan(
    money_system: &mut MoneySystem,
    issuers: &mut [Issuer],
    reservations: &mut LoanReservations,
    regime: Regime,
    application: IssuerCreditApplication,
) -> Option<()> {
    let issuer_id = application.issuer;
    let amount = application.amount;
    let issuer_pos = issuers.iter().position(|issuer| issuer.id == issuer_id)?;
    let commit_plan = reservations.issuer_fiat_credit_commit_plan(
        &issuers[issuer_pos],
        regime,
        application.order_seq,
        amount,
    )?;
    let mut staged_money = money_system.clone();
    let mut staged_issuer = issuers[issuer_pos].clone();

    staged_money
        .credit_fiat(application.borrower, amount)
        .ok()?;
    staged_issuer.record_credit_loan(regime, amount).ok()?;
    reservations.commit_issuer_fiat_credit(issuer_id, application.order_seq, commit_plan);

    *money_system = staged_money;
    issuers[issuer_pos] = staged_issuer;
    Some(())
}

pub fn settle_due_debts(
    agents: &mut [Agent],
    debts: &mut [DebtContract],
    tick: Tick,
) -> DebtSettlementSummary {
    settle_due_debts_excluding_agents(agents, debts, tick, &[])
}

pub fn settle_due_debts_excluding_agents(
    agents: &mut [Agent],
    debts: &mut [DebtContract],
    tick: Tick,
    frozen_agents: &[AgentId],
) -> DebtSettlementSummary {
    let mut summary = DebtSettlementSummary::default();
    let agent_index = sorted_agent_index(agents);
    for debt in debts.iter_mut() {
        if debt.state != DebtState::Open || debt.due_tick > tick {
            continue;
        }
        if debt_touches_frozen_agent(debt, frozen_agents) {
            continue;
        }
        let CreditLender::Agent(lender) = debt.lender else {
            continue;
        };
        let Some(lender_pos) = agent_position(&agent_index, lender) else {
            continue;
        };
        let Some(borrower_pos) = agent_position(&agent_index, debt.borrower) else {
            continue;
        };
        if lender_pos == borrower_pos {
            continue;
        }
        let owed = debt.remaining_due();
        if owed == Gold::ZERO {
            debt.state = DebtState::Settled;
            summary.settled = summary.settled.saturating_add(1);
            continue;
        }

        let lender_headroom = Gold(u64::MAX.saturating_sub(agents[lender_pos].gold.0));
        let paid_headroom = Gold(u64::MAX.saturating_sub(debt.paid.0));
        let payment = agents[borrower_pos]
            .gold
            .min(owed)
            .min(lender_headroom)
            .min(paid_headroom);
        if payment > Gold::ZERO {
            let Some(borrower_gold) = agents[borrower_pos].gold.checked_sub(payment) else {
                continue;
            };
            let Some(lender_gold) = agents[lender_pos].gold.checked_add(payment) else {
                continue;
            };
            let Some(paid) = debt.paid.checked_add(payment) else {
                continue;
            };
            agents[borrower_pos].gold = borrower_gold;
            agents[lender_pos].gold = lender_gold;
            debt.paid = paid;
            summary.paid = summary.paid.saturating_add(payment);
        }

        if debt.paid >= debt.due {
            debt.state = DebtState::Settled;
            summary.settled = summary.settled.saturating_add(1);
        } else {
            debt.state = DebtState::Defaulted;
            summary.defaulted = summary.defaulted.saturating_add(1);
            summary.unpaid = summary.unpaid.saturating_add(debt.remaining_due());
        }
    }
    summary
}

pub fn settle_due_debts_m3(context: DebtSettlementM3Context<'_>) -> DebtSettlementSummary {
    settle_due_debts_m3_excluding_agents(context, &[])
}

pub fn settle_due_debts_m3_excluding_agents(
    context: DebtSettlementM3Context<'_>,
    frozen_agents: &[AgentId],
) -> DebtSettlementSummary {
    let DebtSettlementM3Context {
        agents,
        debts,
        tick,
        money_system,
        banks,
        issuers,
        public_debt_tender,
        bank_repayment_tender,
        issuer_repayment_tender,
        tax_receivability,
        debt_payment_audit,
        bank_repayment_audit,
        issuer_repayment_audit,
        tax_audit,
    } = context;
    let mut summary = DebtSettlementSummary::default();
    for debt in debts.iter_mut() {
        if debt.state != DebtState::Open || debt.due_tick > tick {
            continue;
        }
        if debt_touches_frozen_agent(debt, frozen_agents) {
            continue;
        }
        let owed = debt.remaining_due();
        if owed == Gold::ZERO {
            debt.state = DebtState::Settled;
            summary.settled = summary.settled.saturating_add(1);
            continue;
        }

        match debt.funding {
            CreditSource::Commodity => settle_commodity_debt_m3(
                debt,
                owed,
                tick,
                money_system,
                public_debt_tender,
                debt_payment_audit,
                &mut summary,
            ),
            CreditSource::BankFiduciary(bank_id) => settle_bank_debt_m3(
                debt,
                owed,
                BankDebtSettlementContext {
                    tick,
                    money_system,
                    banks,
                    bank_id,
                    tender: bank_repayment_tender,
                    audit: bank_repayment_audit,
                    summary: &mut summary,
                },
            ),
            CreditSource::FiatCredit(issuer_id) => settle_fiat_debt_m3(
                debt,
                owed,
                IssuerDebtSettlementContext {
                    tick,
                    money_system,
                    issuers,
                    issuer_id,
                    tender: issuer_repayment_tender,
                    audit: issuer_repayment_audit,
                    summary: &mut summary,
                },
            ),
            CreditSource::Tax(issuer_id) => settle_tax_debt_m3(
                debt,
                owed,
                TaxDebtSettlementContext {
                    tick,
                    money_system,
                    issuers,
                    issuer_id,
                    receivability: tax_receivability,
                    audit: tax_audit,
                    summary: &mut summary,
                },
            ),
            CreditSource::FiatFiscal(_) => {
                debt.state = DebtState::Defaulted;
                summary.defaulted = summary.defaulted.saturating_add(1);
                summary.unpaid = summary.unpaid.saturating_add(owed);
            }
        }
    }
    money_system.reconcile_agent_cache(agents);
    summary
}

fn settle_commodity_debt_m3(
    debt: &mut DebtContract,
    owed: Gold,
    tick: Tick,
    money_system: &mut MoneySystem,
    public_debt_tender: PublicDebtTender,
    debt_payment_audit: &mut Vec<DebtPaymentAuditRecord>,
    summary: &mut DebtSettlementSummary,
) {
    let CreditLender::Agent(lender) = debt.lender else {
        debt.state = DebtState::Defaulted;
        summary.defaulted = summary.defaulted.saturating_add(1);
        summary.unpaid = summary.unpaid.saturating_add(owed);
        return;
    };
    let accepted = public_debt_tender.accepted_media();
    let payment = money_system
        .accepted_spendable_total(debt.borrower, accepted)
        .min(owed);
    let mut composition = MoneyComposition::default();
    if payment > Gold::ZERO {
        if let Ok(transferred) =
            money_system.transfer_spendable_with_media(debt.borrower, lender, payment, accepted)
        {
            composition = transferred;
            debt.paid = debt.paid.saturating_add(payment);
            summary.paid = summary.paid.saturating_add(payment);
        }
    }
    finish_settlement(debt, summary);
    debt_payment_audit.push(DebtPaymentAuditRecord {
        tick: tick.0,
        debt: debt.id.0,
        from: debt.borrower,
        to: lender,
        owed,
        paid: composition.total(),
        remaining: debt.remaining_due(),
        public_fiat: composition.fiat,
        demand_claims: composition
            .claims
            .iter()
            .fold(Gold::ZERO, |total, (_, claim)| total.saturating_add(*claim)),
        public_specie: composition.specie,
        tender: public_debt_tender,
        state: debt_payment_state(debt.state),
    });
}

fn debt_payment_state(state: DebtState) -> DebtPaymentState {
    match state {
        DebtState::Settled => DebtPaymentState::Settled,
        DebtState::Defaulted => DebtPaymentState::Defaulted,
        DebtState::Open => unreachable!("debt audit state should be finalized before rendering"),
    }
}

fn settle_bank_debt_m3(
    debt: &mut DebtContract,
    owed: Gold,
    context: BankDebtSettlementContext<'_>,
) {
    let BankDebtSettlementContext {
        tick,
        money_system,
        banks,
        bank_id,
        tender,
        audit,
        summary,
    } = context;
    if debt.lender != CreditLender::Bank(bank_id) {
        debt.state = DebtState::Defaulted;
        summary.defaulted = summary.defaulted.saturating_add(1);
        summary.unpaid = summary.unpaid.saturating_add(owed);
        audit.push(bank_repayment_audit_record(
            tick,
            debt,
            bank_id,
            owed,
            &MoneyComposition::default(),
            Gold::ZERO,
            tender,
        ));
        return;
    }
    let accepted = tender.accepted_media();
    let claim_limits = bank_payment_claim_limits(banks, bank_id);
    let payment = owed.min(money_system.spendable_total_with_media_and_claim_limits(
        debt.borrower,
        accepted,
        &claim_limits,
    ));
    let mut committed_composition = MoneyComposition::default();
    let mut committed_credit_retired = Gold::ZERO;
    if payment > Gold::ZERO {
        let principal_left = debt.principal.saturating_sub(debt.paid.min(debt.principal));
        let principal_retired = principal_left.min(payment);
        if let Ok(composition) = money_system.repayment_composition_with_media_and_claim_limits(
            debt.borrower,
            payment,
            accepted,
            &claim_limits,
        ) {
            if validate_bank_debt_payment(
                money_system,
                banks,
                bank_id,
                &composition,
                principal_retired,
            )
            .is_some()
            {
                let mut staged_money = money_system.clone();
                let mut staged_banks = banks.to_vec();
                let Ok(debited_composition) = staged_money
                    .debit_for_repayment_with_media_and_claim_limits(
                        debt.borrower,
                        payment,
                        accepted,
                        &claim_limits,
                    )
                else {
                    finish_settlement(debt, summary);
                    audit.push(bank_repayment_audit_record(
                        tick,
                        debt,
                        bank_id,
                        owed,
                        &committed_composition,
                        committed_credit_retired,
                        tender,
                    ));
                    return;
                };
                debug_assert_eq!(
                    debited_composition, composition,
                    "bank repayment debit composition changed between planning and commit"
                );
                if debited_composition != composition {
                    finish_settlement(debt, summary);
                    audit.push(bank_repayment_audit_record(
                        tick,
                        debt,
                        bank_id,
                        owed,
                        &committed_composition,
                        committed_credit_retired,
                        tender,
                    ));
                    return;
                }
                let Some(credit_retired) = apply_bank_debt_payment(
                    &mut staged_money,
                    &mut staged_banks,
                    bank_id,
                    &debited_composition,
                    principal_retired,
                ) else {
                    finish_settlement(debt, summary);
                    audit.push(bank_repayment_audit_record(
                        tick,
                        debt,
                        bank_id,
                        owed,
                        &committed_composition,
                        committed_credit_retired,
                        tender,
                    ));
                    return;
                };
                *money_system = staged_money;
                banks.clone_from_slice(&staged_banks);
                let paid = debited_composition.total();
                debt.paid = debt.paid.checked_add(paid).unwrap_or(debt.due);
                summary.paid = summary.paid.saturating_add(paid);
                summary.credit_retired = summary.credit_retired.saturating_add(credit_retired);
                committed_composition = debited_composition;
                committed_credit_retired = credit_retired;
            }
        }
    }
    finish_settlement(debt, summary);
    audit.push(bank_repayment_audit_record(
        tick,
        debt,
        bank_id,
        owed,
        &committed_composition,
        committed_credit_retired,
        tender,
    ));
}

fn bank_repayment_audit_record(
    tick: Tick,
    debt: &DebtContract,
    bank_id: BankId,
    owed: Gold,
    composition: &MoneyComposition,
    credit_retired: Gold,
    tender: BankRepaymentTender,
) -> BankRepaymentAuditRecord {
    BankRepaymentAuditRecord {
        tick: tick.0,
        debt: debt.id.0,
        borrower: debt.borrower,
        bank: bank_id,
        owed,
        paid: composition.total(),
        remaining: debt.remaining_due(),
        public_fiat: composition.fiat,
        demand_claims: composition
            .claims
            .iter()
            .fold(Gold::ZERO, |total, (_, claim)| total.saturating_add(*claim)),
        public_specie: composition.specie,
        credit_retired,
        tender,
        state: debt_payment_state(debt.state),
    }
}

fn settle_fiat_debt_m3(
    debt: &mut DebtContract,
    owed: Gold,
    context: IssuerDebtSettlementContext<'_>,
) {
    let IssuerDebtSettlementContext {
        tick,
        money_system,
        issuers,
        issuer_id,
        tender,
        audit,
        summary,
    } = context;
    let mut committed_composition = MoneyComposition::default();
    let mut committed_credit_retired = Gold::ZERO;

    if debt.lender != CreditLender::Issuer(issuer_id) {
        debt.state = DebtState::Defaulted;
        summary.defaulted = summary.defaulted.saturating_add(1);
        summary.unpaid = summary.unpaid.saturating_add(owed);
        audit.push(issuer_repayment_audit_record(
            tick,
            debt,
            issuer_id,
            owed,
            &MoneyComposition::default(),
            Gold::ZERO,
            tender,
        ));
        return;
    }

    // Fiat-credit principal retires ONLY when fiat leaves the borrower and returns to
    // the issuer. Draw FIAT alone, never specie: specie cannot retire created fiat, so a
    // borrower holding only specie partially or fully DEFAULTS and the created fiat stays
    // in circulation as the inflation overhang. This keeps `credit_retired` meaning
    // "money-stock contraction," matching the concerns2 Option-A invariant.
    let payment = if tender.accepts_fiat() {
        owed.min(money_system.public_fiat(debt.borrower))
    } else {
        Gold::ZERO
    };
    if payment > Gold::ZERO {
        if let Some((staged_money, staged_issuers, composition, credit_retired)) =
            stage_issuer_fiat_repayment(debt, payment, money_system, issuers, issuer_id)
        {
            *money_system = staged_money;
            issuers.clone_from_slice(&staged_issuers);
            let paid = composition.total();
            debug_assert_eq!(
                paid, payment,
                "issuer fiat-credit repayment committed amount differs from intended fiat draw"
            );
            debt.paid = debt.paid.checked_add(paid).unwrap_or(debt.due);
            summary.paid = summary.paid.saturating_add(paid);
            summary.credit_retired = summary.credit_retired.saturating_add(credit_retired);
            committed_composition = composition;
            committed_credit_retired = credit_retired;
        }
    }
    let mut audit_context = IssuerDebtSettlementAuditContext {
        tick,
        issuer_id,
        owed,
        tender,
        audit,
    };
    finish_issuer_debt_settlement(
        debt,
        summary,
        &mut audit_context,
        &committed_composition,
        committed_credit_retired,
    );
}

fn stage_issuer_fiat_repayment(
    debt: &DebtContract,
    payment: Gold,
    money_system: &MoneySystem,
    issuers: &[Issuer],
    issuer_id: IssuerId,
) -> Option<(MoneySystem, Vec<Issuer>, MoneyComposition, Gold)> {
    let principal_left = debt.principal.saturating_sub(debt.paid.min(debt.principal));
    let principal_retired = principal_left.min(payment);
    let mut staged_money = money_system.clone();
    let mut staged_issuers = issuers.to_vec();
    let issuer_pos = staged_issuers
        .iter()
        .position(|issuer| issuer.id == issuer_id)?;
    let composition = staged_money
        .debit_fiat_for_repayment(debt.borrower, payment)
        .ok()?;
    let fiat_retired = staged_money.absorb_issuer_payment(&composition).ok()?;
    let credit_retired = staged_issuers[issuer_pos]
        .retire_credit_principal(principal_retired)
        .ok()?;
    staged_issuers[issuer_pos]
        .record_fiat_retirement(fiat_retired.saturating_sub(credit_retired))
        .ok()?;
    Some((staged_money, staged_issuers, composition, credit_retired))
}

fn finish_issuer_debt_settlement(
    debt: &mut DebtContract,
    summary: &mut DebtSettlementSummary,
    audit_context: &mut IssuerDebtSettlementAuditContext<'_>,
    committed_composition: &MoneyComposition,
    committed_credit_retired: Gold,
) {
    finish_settlement(debt, summary);
    audit_context.audit.push(issuer_repayment_audit_record(
        audit_context.tick,
        debt,
        audit_context.issuer_id,
        audit_context.owed,
        committed_composition,
        committed_credit_retired,
        audit_context.tender,
    ));
}

fn issuer_repayment_audit_record(
    tick: Tick,
    debt: &DebtContract,
    issuer_id: IssuerId,
    owed: Gold,
    composition: &MoneyComposition,
    credit_retired: Gold,
    tender: IssuerRepaymentTender,
) -> IssuerRepaymentAuditRecord {
    debug_assert!(composition.claims.is_empty());
    debug_assert_eq!(composition.specie, Gold::ZERO);
    IssuerRepaymentAuditRecord {
        tick: tick.0,
        debt: debt.id.0,
        borrower: debt.borrower,
        issuer: issuer_id,
        owed,
        paid: composition.total(),
        remaining: debt.remaining_due(),
        public_fiat: composition.fiat,
        public_specie: composition.specie,
        credit_retired,
        tender,
        state: debt_payment_state(debt.state),
    }
}

// A levied tax (M21) is a `DebtContract` with `principal = ZERO` whose lender
// is the issuer. Discharge is gated by `TaxReceivability`, never by the credit
// tenders. Fiat receipts return to the issuer via `absorb_issuer_payment` +
// `record_fiat_retirement`, and specie receipts vault — so `credit_retired`
// and `fiat_credit_outstanding` are untouched (concerns2 Option-A). A fully
// unpaid default moves no money; a partial payment first commits accepted
// media, then defaults only the unpaid remainder. Defaults trigger no
// enforcement. Bank claims are never accepted, so no claim ever appears in a
// tax composition (the receivability media table forbids it).
fn settle_tax_debt_m3(debt: &mut DebtContract, owed: Gold, context: TaxDebtSettlementContext<'_>) {
    let TaxDebtSettlementContext {
        tick,
        money_system,
        issuers,
        issuer_id,
        receivability,
        audit,
        summary,
    } = context;
    let mut committed = MoneyComposition::default();

    if debt.lender != CreditLender::Issuer(issuer_id) {
        debt.state = DebtState::Defaulted;
        summary.defaulted = summary.defaulted.saturating_add(1);
        summary.unpaid = summary.unpaid.saturating_add(owed);
        if let Some(issuer) = issuers.iter_mut().find(|issuer| issuer.id == issuer_id) {
            issuer.record_tax_default(owed);
        }
        audit.push(tax_audit_record(
            tick,
            debt,
            issuer_id,
            owed,
            &committed,
            receivability,
        ));
        return;
    }

    let accepted = receivability.accepted_media();
    let payment = owed.min(money_system.accepted_spendable_total(debt.borrower, accepted));
    if payment > Gold::ZERO {
        if let Some((staged_money, staged_issuers, composition)) =
            stage_tax_payment(debt, payment, accepted, money_system, issuers, issuer_id)
        {
            *money_system = staged_money;
            issuers.clone_from_slice(&staged_issuers);
            let paid = composition.total();
            debug_assert_eq!(
                paid, payment,
                "tax payment staging must draw the accepted spendable amount"
            );
            debt.paid = debt.paid.checked_add(paid).unwrap_or(debt.due);
            summary.paid = summary.paid.saturating_add(paid);
            committed = composition;
        }
    }

    finish_settlement(debt, summary);
    if debt.state == DebtState::Defaulted {
        if let Some(issuer) = issuers.iter_mut().find(|issuer| issuer.id == issuer_id) {
            issuer.record_tax_default(debt.remaining_due());
        }
    }
    audit.push(tax_audit_record(
        tick,
        debt,
        issuer_id,
        owed,
        &committed,
        receivability,
    ));
}

fn stage_tax_payment(
    debt: &DebtContract,
    payment: Gold,
    accepted: AcceptedMedia,
    money_system: &MoneySystem,
    issuers: &[Issuer],
    issuer_id: IssuerId,
) -> Option<(MoneySystem, Vec<Issuer>, MoneyComposition)> {
    let mut staged_money = money_system.clone();
    let mut staged_issuers = issuers.to_vec();
    let issuer_pos = staged_issuers
        .iter()
        .position(|issuer| issuer.id == issuer_id)?;
    // Claim limits are empty: `TaxReceivability` never accepts bank claims, so
    // the media-gated debit draws only fiat and/or specie in that order.
    let composition = staged_money
        .debit_for_repayment_with_media_and_claim_limits(debt.borrower, payment, accepted, &[])
        .ok()?;
    debug_assert!(
        composition.claims.is_empty(),
        "tax receipts never include bank claims"
    );
    let fiat_retired = staged_money.absorb_issuer_payment(&composition).ok()?;
    staged_issuers[issuer_pos]
        .record_fiat_retirement(fiat_retired)
        .ok()?;
    staged_issuers[issuer_pos].record_tax_receipt(composition.fiat, composition.specie);
    Some((staged_money, staged_issuers, composition))
}

fn tax_audit_record(
    tick: Tick,
    debt: &DebtContract,
    issuer_id: IssuerId,
    owed: Gold,
    composition: &MoneyComposition,
    receivability: TaxReceivability,
) -> TaxAuditRecord {
    debug_assert!(composition.claims.is_empty());
    TaxAuditRecord {
        tick: tick.0,
        debt: debt.id.0,
        agent: debt.borrower,
        issuer: issuer_id,
        owed,
        paid: composition.total(),
        remaining: debt.remaining_due(),
        paid_fiat: composition.fiat,
        paid_specie: composition.specie,
        receivability,
        state: debt_payment_state(debt.state),
    }
}

fn validate_bank_debt_payment(
    money_system: &MoneySystem,
    banks: &[Bank],
    lender_bank_id: BankId,
    composition: &MoneyComposition,
    principal_retired: Gold,
) -> Option<()> {
    money_system
        .validate_bank_payment_absorption(composition)
        .ok()?;
    for (claim_bank_id, claim) in &composition.claims {
        let claim_bank = banks.iter().find(|bank| bank.id == *claim_bank_id)?;
        if claim_bank.demand_deposits < *claim {
            return None;
        }
        if *claim_bank_id != lender_bank_id && claim_bank.reserves < *claim {
            return None;
        }
    }
    let lender_bank = banks.iter().find(|bank| bank.id == lender_bank_id)?;
    lender_bank
        .reserves
        .checked_add(lender_reserve_credit_from_payment(
            lender_bank_id,
            composition,
        )?)?;
    if lender_bank.loans_outstanding < principal_retired {
        return None;
    }
    Some(())
}

fn apply_bank_debt_payment(
    money_system: &mut MoneySystem,
    banks: &mut [Bank],
    lender_bank_id: BankId,
    composition: &MoneyComposition,
    principal_retired: Gold,
) -> Option<Gold> {
    money_system.absorb_bank_payment(composition).ok()?;
    let lender_pos = banks.iter().position(|bank| bank.id == lender_bank_id)?;
    let mut lender_reserve_credit = composition.specie;

    for (claim_bank_id, claim) in &composition.claims {
        let claim_pos = banks.iter().position(|bank| bank.id == *claim_bank_id)?;
        if *claim_bank_id == lender_bank_id {
            banks[claim_pos].retire_demand_deposit(*claim).ok()?;
        } else {
            banks[claim_pos].retire_redeemed_claim(*claim).ok()?;
            banks[claim_pos].debit_reserves(*claim).ok()?;
            lender_reserve_credit = lender_reserve_credit.checked_add(*claim)?;
        }
    }
    banks[lender_pos]
        .credit_reserves(lender_reserve_credit)
        .ok()?;
    let lender_bank = banks.iter_mut().find(|bank| bank.id == lender_bank_id)?;
    let credit_retired = lender_bank
        .retire_fiduciary_principal(principal_retired)
        .ok()?;
    Some(credit_retired)
}

fn bank_payment_claim_limits(banks: &[Bank], lender_bank_id: BankId) -> Vec<(BankId, Gold)> {
    let mut limits = banks
        .iter()
        .filter_map(|bank| {
            let limit = if bank.id == lender_bank_id {
                bank.demand_deposits
            } else {
                bank.demand_deposits.min(bank.reserves)
            };
            (limit > Gold::ZERO).then_some((bank.id, limit))
        })
        .collect::<Vec<_>>();
    limits.sort_by_key(|(bank, _)| *bank);
    limits
}

fn lender_reserve_credit_from_payment(
    lender_bank_id: BankId,
    composition: &MoneyComposition,
) -> Option<Gold> {
    composition
        .claims
        .iter()
        .filter(|(claim_bank_id, _)| *claim_bank_id != lender_bank_id)
        .map(|(_, claim)| *claim)
        .try_fold(composition.specie, Gold::checked_add)
}

fn finish_settlement(debt: &mut DebtContract, summary: &mut DebtSettlementSummary) {
    if debt.paid >= debt.due {
        debt.state = DebtState::Settled;
        summary.settled = summary.settled.saturating_add(1);
    } else {
        debt.state = DebtState::Defaulted;
        summary.defaulted = summary.defaulted.saturating_add(1);
        summary.unpaid = summary.unpaid.saturating_add(debt.remaining_due());
    }
}

fn debt_touches_frozen_agent(debt: &DebtContract, frozen_agents: &[AgentId]) -> bool {
    frozen_agents.contains(&debt.borrower)
        || debt
            .lender
            .agent()
            .is_some_and(|lender| frozen_agents.contains(&lender))
}

fn sorted_agent_index(agents: &[Agent]) -> Vec<(AgentId, usize)> {
    let mut index = agents
        .iter()
        .enumerate()
        .map(|(position, agent)| (agent.id, position))
        .collect::<Vec<_>>();
    index.sort_by_key(|(agent, _)| *agent);
    index
}

fn agent_position(index: &[(AgentId, usize)], agent: AgentId) -> Option<usize> {
    index
        .binary_search_by_key(&agent, |(entry, _)| *entry)
        .ok()
        .map(|position| index[position].1)
}

#[cfg(test)]
mod tests {
    use super::{
        apply_bank_fiduciary_loan, apply_issuer_fiat_credit_loan, settle_due_debts,
        BankCreditApplication, DebtContract, DebtId, DebtSettlementM3Context,
        DebtSettlementSummary, DebtState, IssuerCreditApplication, LoanM3Context, LoanOrder,
        LoanOrderBook, LoanReservations, LoanSide,
    };
    use crate::agent::{Agent, AgentId, Role, Want, WantKind};
    use crate::bank::{Bank, BankPolicy};
    use crate::good::{Gold, Horizon, Stock, GOLD};
    use crate::issuer::{Issuer, IssuerPolicy};
    use crate::ledger::{BankId, IssuerId, MoneySystem};
    use crate::money::{
        BankRepaymentTender, IssuerRepaymentTender, PublicDebtTender, Regime, ReserveRatioBps,
        TaxReceivability,
    };
    use crate::project::Tick;
    use crate::purpose::{CreditLender, CreditSource, DebtPurpose, LoanPurpose};
    use crate::report::render_loan_tape;

    fn settle_due_debts_m3(
        agents: &mut [Agent],
        debts: &mut [DebtContract],
        tick: Tick,
        money_system: &mut MoneySystem,
        banks: &mut [Bank],
        issuers: &mut [Issuer],
    ) -> DebtSettlementSummary {
        let mut debt_payment_audit = Vec::new();
        let mut bank_repayment_audit = Vec::new();
        let mut issuer_repayment_audit = Vec::new();
        let mut tax_audit = Vec::new();
        super::settle_due_debts_m3(DebtSettlementM3Context {
            agents,
            debts,
            tick,
            money_system,
            banks,
            issuers,
            public_debt_tender: PublicDebtTender::ParAll,
            bank_repayment_tender: BankRepaymentTender::ParAll,
            issuer_repayment_tender: IssuerRepaymentTender::FiatOnly,
            tax_receivability: TaxReceivability::SpecieOnly,
            debt_payment_audit: &mut debt_payment_audit,
            bank_repayment_audit: &mut bank_repayment_audit,
            issuer_repayment_audit: &mut issuer_repayment_audit,
            tax_audit: &mut tax_audit,
        })
    }

    fn agent(id: u32, gold: Gold) -> Agent {
        Agent {
            id: AgentId(u64::from(id)),
            scale: vec![Want {
                kind: WantKind::Good(GOLD),
                horizon: Horizon::Later(4),
                qty: 1,
                satisfied: false,
            }],
            stock: Stock::new(3),
            gold,
            labor_capacity: 0,
            hunger_deficit: 0,
            roles: vec![Role::Trader],
            expect: Vec::new(),
        }
    }

    fn order(agent: u32, side: LoanSide, future_limit: Gold, seq: u64) -> LoanOrder {
        LoanOrder {
            agent: AgentId(u64::from(agent)),
            lender: CreditLender::Agent(AgentId(u64::from(agent))),
            side,
            present: Gold(1),
            future_limit,
            horizon: 4,
            seq,
            expires_tick: 3,
            purpose: LoanPurpose::Consumption,
            funding: CreditSource::Commodity,
        }
    }

    fn bank() -> Bank {
        Bank {
            id: BankId(1),
            name: "test bank",
            reserves: Gold::ZERO,
            demand_deposits: Gold::ZERO,
            time_deposits: Gold::ZERO,
            loans_outstanding: Gold::ZERO,
            fiduciary_issued: Gold::ZERO,
            reserve_ratio_bps: ReserveRatioBps(0),
            convertible: true,
            policy: BankPolicy {
                max_new_fiduciary_per_tick: Gold(5),
                loan_present: Gold(1),
                loan_horizon: 4,
                loan_future_due: Gold(1),
                enabled: true,
            },
        }
    }

    fn bank_lend(seq: u64) -> LoanOrder {
        LoanOrder {
            agent: AgentId(u64::from(u32::MAX - 1)),
            lender: CreditLender::Bank(BankId(1)),
            side: LoanSide::Lend,
            present: Gold(1),
            future_limit: Gold(1),
            horizon: 4,
            seq,
            expires_tick: 3,
            purpose: LoanPurpose::Consumption,
            funding: CreditSource::BankFiduciary(BankId(1)),
        }
    }

    fn issuer() -> Issuer {
        Issuer {
            id: IssuerId(1),
            fiat_issued: Gold::ZERO,
            fiat_retired: Gold::ZERO,
            fiat_credit_outstanding: Gold::ZERO,
            policy: IssuerPolicy {
                fiscal_enabled: false,
                credit_enabled: true,
                max_fiscal_issue_per_tick: Gold::ZERO,
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

    fn issuer_lend(seq: u64) -> LoanOrder {
        LoanOrder {
            agent: AgentId(u64::from(u32::MAX - 1_001)),
            lender: CreditLender::Issuer(IssuerId(1)),
            side: LoanSide::Lend,
            present: Gold(1),
            future_limit: Gold(1),
            horizon: 4,
            seq,
            expires_tick: 3,
            purpose: LoanPurpose::Consumption,
            funding: CreditSource::FiatCredit(IssuerId(1)),
        }
    }

    macro_rules! loan_m3_context {
        ($agents:expr, $reservations:expr, $debts:expr, $next_debt_id:expr, $money:expr, $banks:expr) => {
            LoanM3Context {
                agents: $agents,
                reservations: $reservations,
                debts: $debts,
                next_debt_id: $next_debt_id,
                money_system: $money,
                banks: $banks,
                issuers: &mut [],
                regime: Regime::FractionalConvertible,
            }
        };
    }

    #[test]
    fn loan_book_matches_at_resting_future_due() {
        let mut agents = vec![agent(1, Gold(5)), agent(2, Gold(4))];
        let mut reservations = LoanReservations::new();
        let mut book = LoanOrderBook::new();
        let mut debts = Vec::new();
        let mut next_debt_id = 1;

        let lend = order(1, LoanSide::Lend, Gold(2), 1);
        assert!(reservations.reserve_order(&agents, &lend));
        assert!(book
            .add_order(
                lend,
                0,
                &mut agents,
                &mut reservations,
                &mut debts,
                &mut next_debt_id,
            )
            .is_empty());
        let borrow = order(2, LoanSide::Borrow, Gold(3), 2);
        assert!(reservations.reserve_order(&agents, &borrow));
        let trades = book.add_order(
            borrow,
            0,
            &mut agents,
            &mut reservations,
            &mut debts,
            &mut next_debt_id,
        );

        assert_eq!(trades.len(), 1);
        assert_eq!(trades[0].future_due, Gold(2));
        assert_eq!(agents[0].gold, Gold(4));
        assert_eq!(agents[1].gold, Gold(5));
        assert_eq!(debts[0].due, Gold(2));
    }

    #[test]
    fn commodity_lender_crosses_at_resting_borrow_future_due() {
        let mut agents = vec![agent(1, Gold(5)), agent(2, Gold(4))];
        let mut reservations = LoanReservations::new();
        let mut book = LoanOrderBook::new();
        let mut debts = Vec::new();
        let mut next_debt_id = 1;

        let borrow = order(2, LoanSide::Borrow, Gold(3), 1);
        assert!(reservations.reserve_order(&agents, &borrow));
        assert!(book
            .add_order(
                borrow,
                0,
                &mut agents,
                &mut reservations,
                &mut debts,
                &mut next_debt_id,
            )
            .is_empty());
        let lend = order(1, LoanSide::Lend, Gold(2), 2);
        assert!(reservations.reserve_order(&agents, &lend));
        let trades = book.add_order(
            lend,
            0,
            &mut agents,
            &mut reservations,
            &mut debts,
            &mut next_debt_id,
        );

        assert_eq!(trades.len(), 1);
        assert_eq!(trades[0].future_due, Gold(3));
        assert_eq!(debts[0].due, Gold(3));
    }

    #[test]
    fn loan_book_rejects_self_loans() {
        let mut agents = vec![agent(1, Gold(5))];
        let mut reservations = LoanReservations::new();
        let mut book = LoanOrderBook::new();
        let mut debts = Vec::new();
        let mut next_debt_id = 1;
        let lend = order(1, LoanSide::Lend, Gold(2), 1);
        assert!(reservations.reserve_order(&agents, &lend));
        book.add_order(
            lend,
            0,
            &mut agents,
            &mut reservations,
            &mut debts,
            &mut next_debt_id,
        );
        let borrow = order(1, LoanSide::Borrow, Gold(3), 2);
        assert!(reservations.reserve_order(&agents, &borrow));

        let trades = book.add_order(
            borrow,
            0,
            &mut agents,
            &mut reservations,
            &mut debts,
            &mut next_debt_id,
        );

        assert!(trades.is_empty());
        assert_eq!(book.live_order_counts(), (1, 1));
        assert!(debts.is_empty());
    }

    #[test]
    fn loan_book_skips_failed_resting_lend() {
        let mut agents = vec![agent(1, Gold(1)), agent(2, Gold(4)), agent(3, Gold(5))];
        let mut reservations = LoanReservations::new();
        let mut book = LoanOrderBook::new();
        let mut debts = Vec::new();
        let mut next_debt_id = 1;
        let stale_lend = order(1, LoanSide::Lend, Gold(2), 1);
        assert!(reservations.reserve_order(&agents, &stale_lend));
        assert!(book
            .add_order(
                stale_lend,
                0,
                &mut agents,
                &mut reservations,
                &mut debts,
                &mut next_debt_id,
            )
            .is_empty());
        agents[0].gold = Gold::ZERO;

        let valid_lend = order(3, LoanSide::Lend, Gold(2), 2);
        assert!(reservations.reserve_order(&agents, &valid_lend));
        assert!(book
            .add_order(
                valid_lend,
                0,
                &mut agents,
                &mut reservations,
                &mut debts,
                &mut next_debt_id,
            )
            .is_empty());
        let borrow = order(2, LoanSide::Borrow, Gold(3), 3);
        assert!(reservations.reserve_order(&agents, &borrow));
        let trades = book.add_order(
            borrow,
            0,
            &mut agents,
            &mut reservations,
            &mut debts,
            &mut next_debt_id,
        );

        assert_eq!(trades.len(), 1);
        assert_eq!(trades[0].lender, CreditLender::Agent(AgentId(3)));
        assert_eq!(agents[2].gold, Gold(4));
        assert_eq!(agents[1].gold, Gold(5));
    }

    #[test]
    fn exhausted_debt_id_counter_rejects_loan_without_collision() {
        let mut agents = vec![agent(1, Gold(5)), agent(2, Gold(4))];
        let mut reservations = LoanReservations::new();
        let mut book = LoanOrderBook::new();
        let mut debts = Vec::new();
        let mut next_debt_id = u64::MAX;
        let lend = order(1, LoanSide::Lend, Gold(2), 1);
        assert!(reservations.reserve_order(&agents, &lend));
        assert!(book
            .add_order(
                lend,
                0,
                &mut agents,
                &mut reservations,
                &mut debts,
                &mut next_debt_id,
            )
            .is_empty());
        let borrow = order(2, LoanSide::Borrow, Gold(3), 2);
        assert!(reservations.reserve_order(&agents, &borrow));
        let trades = book.add_order(
            borrow,
            0,
            &mut agents,
            &mut reservations,
            &mut debts,
            &mut next_debt_id,
        );

        assert!(trades.is_empty());
        assert!(debts.is_empty());
        assert_eq!(next_debt_id, u64::MAX);
        assert_eq!(agents[0].gold, Gold(5));
        assert_eq!(agents[1].gold, Gold(4));
        assert_eq!(book.live_order_counts(), (1, 1));
    }

    #[test]
    fn loan_expiry_releases_present_gold_reserve() {
        let agents = vec![agent(1, Gold(5))];
        let mut reservations = LoanReservations::new();
        let mut book = LoanOrderBook::new();
        let lend = order(1, LoanSide::Lend, Gold(2), 1);
        assert!(reservations.reserve_order(&agents, &lend));
        assert_eq!(reservations.reserved_gold(AgentId(1)), Gold(1));
        book.add_order(
            lend,
            0,
            &mut agents.clone(),
            &mut reservations,
            &mut Vec::new(),
            &mut 1,
        );

        assert_eq!(book.purge_expired(3, &mut reservations), 1);
        assert_eq!(reservations.reserved_gold(AgentId(1)), Gold::ZERO);
    }

    #[test]
    fn cancel_agent_clears_resting_orders_and_releases_only_its_reserves() {
        // Two agents each rest one non-crossing order: agent 1 lends (demanding
        // a high future limit) and agent 2 borrows (offering a low one), so they
        // never match and both sit in the book. This mirrors the death
        // tombstone's need to clear a dead agent's resting credit orders.
        let agents = vec![agent(1, Gold(5)), agent(2, Gold(5))];
        let mut reservations = LoanReservations::new();
        let mut book = LoanOrderBook::new();

        let lend = order(1, LoanSide::Lend, Gold(5), 1);
        assert!(reservations.reserve_order(&agents, &lend));
        book.add_order(
            lend,
            0,
            &mut agents.clone(),
            &mut reservations,
            &mut Vec::new(),
            &mut 1,
        );
        let borrow = order(2, LoanSide::Borrow, Gold(2), 2);
        assert!(reservations.reserve_order(&agents, &borrow));
        book.add_order(
            borrow,
            0,
            &mut agents.clone(),
            &mut reservations,
            &mut Vec::new(),
            &mut 1,
        );
        assert_eq!(book.live_order_counts(), (1, 1));
        assert_eq!(reservations.reserved_gold(AgentId(1)), Gold(1));

        // Tombstoning agent 1 drops its lend order and releases its reserve only.
        assert_eq!(book.cancel_agent(AgentId(1), &mut reservations), 1);
        assert_eq!(book.live_order_counts(), (0, 1));
        assert_eq!(reservations.reserved_gold(AgentId(1)), Gold::ZERO);
        // Agent 2's resting borrow order is untouched.
        assert!(book.has_live(AgentId(2), LoanSide::Borrow, Gold(1), 4));
        // Cancelling an agent with no resting orders is a no-op.
        assert_eq!(book.cancel_agent(AgentId(1), &mut reservations), 0);
    }

    #[test]
    fn debt_settlement_transfers_existing_gold() {
        let mut agents = vec![agent(1, Gold(0)), agent(2, Gold(5))];
        let mut debts = vec![DebtContract {
            id: DebtId(1),
            lender: CreditLender::Agent(AgentId(1)),
            borrower: AgentId(2),
            opened_tick: Tick(0),
            due_tick: Tick(1),
            principal: Gold(1),
            due: Gold(3),
            paid: Gold::ZERO,
            state: DebtState::Open,
            purpose: DebtPurpose::Consumption,
            funding: CreditSource::Commodity,
        }];

        let summary = settle_due_debts(&mut agents, &mut debts, Tick(1));

        assert_eq!(summary.settled, 1);
        assert_eq!(agents[0].gold, Gold(3));
        assert_eq!(agents[1].gold, Gold(2));
        assert_eq!(debts[0].state, DebtState::Settled);
    }

    #[test]
    fn debt_settlement_ignores_debts_before_due_tick() {
        let mut agents = vec![agent(1, Gold(0)), agent(2, Gold(5))];
        let mut debts = vec![DebtContract {
            id: DebtId(1),
            lender: CreditLender::Agent(AgentId(1)),
            borrower: AgentId(2),
            opened_tick: Tick(0),
            due_tick: Tick(2),
            principal: Gold(1),
            due: Gold(3),
            paid: Gold::ZERO,
            state: DebtState::Open,
            purpose: DebtPurpose::Consumption,
            funding: CreditSource::Commodity,
        }];

        let summary = settle_due_debts(&mut agents, &mut debts, Tick(1));

        assert_eq!(summary, DebtSettlementSummary::default());
        assert_eq!(agents[0].gold, Gold::ZERO);
        assert_eq!(agents[1].gold, Gold(5));
        assert_eq!(debts[0].paid, Gold::ZERO);
        assert_eq!(debts[0].state, DebtState::Open);
    }

    #[test]
    fn debt_default_conserves_gold() {
        let mut agents = vec![agent(1, Gold(1)), agent(2, Gold(2))];
        let initial = agents
            .iter()
            .fold(Gold::ZERO, |total, agent| total.saturating_add(agent.gold));
        let mut debts = vec![DebtContract {
            id: DebtId(1),
            lender: CreditLender::Agent(AgentId(1)),
            borrower: AgentId(2),
            opened_tick: Tick(0),
            due_tick: Tick(1),
            principal: Gold(1),
            due: Gold(5),
            paid: Gold::ZERO,
            state: DebtState::Open,
            purpose: DebtPurpose::Consumption,
            funding: CreditSource::Commodity,
        }];

        let summary = settle_due_debts(&mut agents, &mut debts, Tick(1));
        let final_gold = agents
            .iter()
            .fold(Gold::ZERO, |total, agent| total.saturating_add(agent.gold));

        assert_eq!(summary.defaulted, 1);
        assert_eq!(final_gold, initial);
        assert_eq!(agents[0].gold, Gold(3));
        assert_eq!(agents[1].gold, Gold::ZERO);
        assert_eq!(debts[0].state, DebtState::Defaulted);
    }

    #[test]
    fn debt_settlement_receiver_overflow_defaults_without_partial_transfer() {
        let mut agents = vec![agent(1, Gold(u64::MAX)), agent(2, Gold(1))];
        let mut debts = vec![DebtContract {
            id: DebtId(1),
            lender: CreditLender::Agent(AgentId(1)),
            borrower: AgentId(2),
            opened_tick: Tick(0),
            due_tick: Tick(1),
            principal: Gold(1),
            due: Gold(1),
            paid: Gold::ZERO,
            state: DebtState::Open,
            purpose: DebtPurpose::Consumption,
            funding: CreditSource::Commodity,
        }];

        let summary = settle_due_debts(&mut agents, &mut debts, Tick(1));

        assert_eq!(summary.defaulted, 1);
        assert_eq!(summary.unpaid, Gold(1));
        assert_eq!(agents[0].gold, Gold(u64::MAX));
        assert_eq!(agents[1].gold, Gold(1));
        assert_eq!(debts[0].paid, Gold::ZERO);
        assert_eq!(debts[0].state, DebtState::Defaulted);
    }

    #[test]
    fn agent_commodity_loan_still_transfers_existing_money() {
        let mut agents = vec![agent(1, Gold(5)), agent(2, Gold::ZERO)];
        let mut money = MoneySystem::from_agents(&agents);
        let mut reservations = LoanReservations::new();
        let mut book = LoanOrderBook::new();
        let mut debts = Vec::new();
        let mut next_debt_id = 1;
        let mut banks = Vec::new();

        let lend = order(1, LoanSide::Lend, Gold(2), 1);
        assert!(reservations.reserve_order(&agents, &lend));
        assert!(book
            .add_order_m3(
                lend,
                0,
                loan_m3_context!(
                    &mut agents,
                    &mut reservations,
                    &mut debts,
                    &mut next_debt_id,
                    &mut money,
                    &mut banks
                ),
            )
            .is_empty());
        let borrow = order(2, LoanSide::Borrow, Gold(3), 2);
        assert!(reservations.reserve_order(&agents, &borrow));
        let trades = book.add_order_m3(
            borrow,
            0,
            loan_m3_context!(
                &mut agents,
                &mut reservations,
                &mut debts,
                &mut next_debt_id,
                &mut money,
                &mut banks
            ),
        );

        assert_eq!(trades.len(), 1);
        assert_eq!(trades[0].funding, CreditSource::Commodity);
        assert_eq!(agents[0].gold, Gold(4));
        assert_eq!(agents[1].gold, Gold(1));
        assert_eq!(money.snapshot().public_specie, Gold(5));
        assert_eq!(money.snapshot().demand_claims, Gold::ZERO);
    }

    #[test]
    fn bank_lend_order_does_not_debit_agent_gold() {
        let mut agents = vec![agent(1, Gold(5)), agent(2, Gold::ZERO)];
        let mut money = MoneySystem::from_agents(&agents);
        let mut reservations = LoanReservations::new();
        let mut book = LoanOrderBook::new();
        let mut debts = Vec::new();
        let mut next_debt_id = 1;
        let mut banks = vec![bank()];

        let lend = bank_lend(1);
        assert!(reservations.reserve_order(&agents, &lend));
        assert!(book
            .add_order_m3(
                lend,
                0,
                loan_m3_context!(
                    &mut agents,
                    &mut reservations,
                    &mut debts,
                    &mut next_debt_id,
                    &mut money,
                    &mut banks
                ),
            )
            .is_empty());
        let borrow = order(2, LoanSide::Borrow, Gold(2), 2);
        assert!(reservations.reserve_order(&agents, &borrow));
        let trades = book.add_order_m3(
            borrow,
            0,
            loan_m3_context!(
                &mut agents,
                &mut reservations,
                &mut debts,
                &mut next_debt_id,
                &mut money,
                &mut banks
            ),
        );

        assert_eq!(trades.len(), 1);
        assert_eq!(agents[0].gold, Gold(5));
        assert_eq!(agents[1].gold, Gold(1));
        assert_eq!(money.snapshot().public_specie, Gold(5));
        assert_eq!(money.snapshot().demand_claims, Gold(1));
        assert_eq!(money.snapshot().fiduciary, Gold(1));
    }

    #[test]
    fn open_bank_lend_order_reserves_policy_capacity() {
        let agents = vec![agent(2, Gold::ZERO)];
        let mut reservations = LoanReservations::new();
        let mut banks = vec![bank()];
        banks[0].policy.max_new_fiduciary_per_tick = Gold(1);

        let first = bank_lend(1);
        let second = bank_lend(2);

        assert!(reservations.reserve_order_m3(
            &agents,
            &first,
            &banks,
            &[],
            Regime::FractionalConvertible
        ));
        assert_eq!(reservations.bank_fiduciary_open(BankId(1)), Gold(1));
        assert!(!reservations.reserve_order_m3(
            &agents,
            &second,
            &banks,
            &[],
            Regime::FractionalConvertible
        ));

        reservations.release_order(&first);
        assert_eq!(reservations.bank_fiduciary_open(BankId(1)), Gold::ZERO);
        assert!(reservations.reserve_order_m3(
            &agents,
            &second,
            &banks,
            &[],
            Regime::FractionalConvertible
        ));
    }

    #[test]
    fn open_issuer_lend_order_reserves_policy_capacity() {
        let agents = vec![agent(2, Gold::ZERO)];
        let mut reservations = LoanReservations::new();
        let banks = Vec::new();
        let mut issuers = vec![issuer()];
        issuers[0].policy.max_credit_issue_per_tick = Gold(1);

        let first = issuer_lend(1);
        let second = issuer_lend(2);

        assert!(reservations.reserve_order_m3(&agents, &first, &banks, &issuers, Regime::Fiat));
        assert_eq!(reservations.issuer_fiat_credit_open(IssuerId(1)), Gold(1));
        assert!(!reservations.reserve_order_m3(&agents, &second, &banks, &issuers, Regime::Fiat));

        reservations.release_order(&first);
        assert_eq!(
            reservations.issuer_fiat_credit_open(IssuerId(1)),
            Gold::ZERO
        );
        assert!(reservations.reserve_order_m3(&agents, &second, &banks, &issuers, Regime::Fiat));
    }

    #[test]
    fn bank_lend_order_matches_sparse_agent_id_collision() {
        let mut agents = vec![agent(u32::MAX - 1, Gold::ZERO)];
        let mut money = MoneySystem::from_agents(&agents);
        let mut reservations = LoanReservations::new();
        let mut book = LoanOrderBook::new();
        let mut debts = Vec::new();
        let mut next_debt_id = 1;
        let mut banks = vec![bank()];

        let lend = bank_lend(1);
        assert_eq!(lend.agent, agents[0].id);
        assert!(reservations.reserve_order_m3(
            &agents,
            &lend,
            &banks,
            &[],
            Regime::FractionalConvertible
        ));
        assert!(book
            .add_order_m3(
                lend,
                0,
                loan_m3_context!(
                    &mut agents,
                    &mut reservations,
                    &mut debts,
                    &mut next_debt_id,
                    &mut money,
                    &mut banks
                ),
            )
            .is_empty());

        let borrow = order(u32::MAX - 1, LoanSide::Borrow, Gold(1), 2);
        assert!(reservations.reserve_order(&agents, &borrow));
        let trades = book.add_order_m3(
            borrow,
            0,
            loan_m3_context!(
                &mut agents,
                &mut reservations,
                &mut debts,
                &mut next_debt_id,
                &mut money,
                &mut banks
            ),
        );

        assert_eq!(trades.len(), 1);
        assert_eq!(trades[0].borrower, AgentId(u64::from(u32::MAX - 1)));
        assert_eq!(trades[0].lender, CreditLender::Bank(BankId(1)));
        assert_eq!(agents[0].gold, Gold(1));
        assert_eq!(money.snapshot().demand_claims, Gold(1));
    }

    #[test]
    fn bank_lend_order_must_clear_with_borrow_order() {
        let mut agents = vec![agent(1, Gold::ZERO)];
        let mut money = MoneySystem::from_agents(&agents);
        let mut reservations = LoanReservations::new();
        let mut book = LoanOrderBook::new();
        let mut debts = Vec::new();
        let mut next_debt_id = 1;
        let mut banks = vec![bank()];

        let lend = bank_lend(1);
        assert!(reservations.reserve_order(&agents, &lend));
        let trades = book.add_order_m3(
            lend,
            0,
            loan_m3_context!(
                &mut agents,
                &mut reservations,
                &mut debts,
                &mut next_debt_id,
                &mut money,
                &mut banks
            ),
        );

        assert!(trades.is_empty());
        assert!(debts.is_empty());
        assert_eq!(money.snapshot().demand_claims, Gold::ZERO);
        assert_eq!(book.live_order_counts(), (1, 0));
    }

    #[test]
    fn bank_lend_order_crosses_at_policy_future_due() {
        let mut agents = vec![agent(2, Gold::ZERO)];
        let mut money = MoneySystem::from_agents(&agents);
        let mut reservations = LoanReservations::new();
        let mut book = LoanOrderBook::new();
        let mut debts = Vec::new();
        let mut next_debt_id = 1;
        let mut banks = vec![bank()];

        let borrow = order(2, LoanSide::Borrow, Gold(4), 1);
        assert!(reservations.reserve_order(&agents, &borrow));
        assert!(book
            .add_order_m3(
                borrow,
                0,
                loan_m3_context!(
                    &mut agents,
                    &mut reservations,
                    &mut debts,
                    &mut next_debt_id,
                    &mut money,
                    &mut banks
                ),
            )
            .is_empty());

        let lend = bank_lend(2);
        assert!(reservations.reserve_order_m3(
            &agents,
            &lend,
            &banks,
            &[],
            Regime::FractionalConvertible
        ));
        let trades = book.add_order_m3(
            lend,
            0,
            loan_m3_context!(
                &mut agents,
                &mut reservations,
                &mut debts,
                &mut next_debt_id,
                &mut money,
                &mut banks
            ),
        );

        assert_eq!(trades.len(), 1);
        assert_eq!(trades[0].future_due, Gold(1));
        assert_eq!(debts[0].due, Gold(1));
    }

    #[test]
    fn issuer_lend_order_must_clear_with_borrow_order() {
        let mut agents = vec![agent(2, Gold::ZERO)];
        let mut money = MoneySystem::from_agents(&agents);
        let mut reservations = LoanReservations::new();
        let mut book = LoanOrderBook::new();
        let mut debts = Vec::new();
        let mut next_debt_id = 1;
        let mut banks = Vec::new();
        let mut issuers = vec![issuer()];

        let lend = issuer_lend(1);
        assert!(reservations.reserve_order(&agents, &lend));
        let resting = book.add_order_m3(
            lend,
            0,
            LoanM3Context {
                agents: &mut agents,
                reservations: &mut reservations,
                debts: &mut debts,
                next_debt_id: &mut next_debt_id,
                money_system: &mut money,
                banks: &mut banks,
                issuers: &mut issuers,
                regime: Regime::Fiat,
            },
        );

        assert!(resting.is_empty());
        assert!(debts.is_empty());
        assert_eq!(money.snapshot().public_fiat, Gold::ZERO);
        assert_eq!(book.live_order_counts(), (1, 0));

        let borrow = order(2, LoanSide::Borrow, Gold(1), 2);
        assert!(reservations.reserve_order(&agents, &borrow));
        let trades = book.add_order_m3(
            borrow,
            0,
            LoanM3Context {
                agents: &mut agents,
                reservations: &mut reservations,
                debts: &mut debts,
                next_debt_id: &mut next_debt_id,
                money_system: &mut money,
                banks: &mut banks,
                issuers: &mut issuers,
                regime: Regime::Fiat,
            },
        );

        assert_eq!(trades.len(), 1);
        assert_eq!(trades[0].lender, CreditLender::Issuer(IssuerId(1)));
        assert_eq!(trades[0].funding, CreditSource::FiatCredit(IssuerId(1)));
        assert_eq!(debts[0].lender, CreditLender::Issuer(IssuerId(1)));
        assert_eq!(debts[0].funding, CreditSource::FiatCredit(IssuerId(1)));
        assert_eq!(money.snapshot().public_fiat, Gold(1));
        assert_eq!(agents[0].gold, Gold(1));
        assert_eq!(issuers[0].fiat_credit_outstanding, Gold(1));
    }

    #[test]
    fn issuer_lend_order_crosses_at_policy_future_due() {
        let mut agents = vec![agent(2, Gold::ZERO)];
        let mut money = MoneySystem::from_agents(&agents);
        let mut reservations = LoanReservations::new();
        let mut book = LoanOrderBook::new();
        let mut debts = Vec::new();
        let mut next_debt_id = 1;
        let mut banks = Vec::new();
        let mut issuers = vec![issuer()];

        let borrow = order(2, LoanSide::Borrow, Gold(4), 1);
        assert!(reservations.reserve_order(&agents, &borrow));
        assert!(book
            .add_order_m3(
                borrow,
                0,
                LoanM3Context {
                    agents: &mut agents,
                    reservations: &mut reservations,
                    debts: &mut debts,
                    next_debt_id: &mut next_debt_id,
                    money_system: &mut money,
                    banks: &mut banks,
                    issuers: &mut issuers,
                    regime: Regime::Fiat,
                },
            )
            .is_empty());

        let lend = issuer_lend(2);
        assert!(reservations.reserve_order_m3(&agents, &lend, &banks, &issuers, Regime::Fiat));
        let trades = book.add_order_m3(
            lend,
            0,
            LoanM3Context {
                agents: &mut agents,
                reservations: &mut reservations,
                debts: &mut debts,
                next_debt_id: &mut next_debt_id,
                money_system: &mut money,
                banks: &mut banks,
                issuers: &mut issuers,
                regime: Regime::Fiat,
            },
        );

        assert_eq!(trades.len(), 1);
        assert_eq!(trades[0].future_due, Gold(1));
        assert_eq!(debts[0].due, Gold(1));
    }

    #[test]
    fn bank_fiduciary_capacity_is_consumed_per_tick() {
        let mut agents = vec![agent(2, Gold::ZERO), agent(3, Gold::ZERO)];
        let mut money = MoneySystem::from_agents(&agents);
        let mut reservations = LoanReservations::new();
        let mut book = LoanOrderBook::new();
        let mut debts = Vec::new();
        let mut next_debt_id = 1;
        let mut banks = vec![bank()];
        banks[0].policy.max_new_fiduciary_per_tick = Gold(1);

        let lend = bank_lend(1);
        assert!(reservations.reserve_order_m3(
            &agents,
            &lend,
            &banks,
            &[],
            Regime::FractionalConvertible
        ));
        book.add_order_m3(
            lend,
            0,
            loan_m3_context!(
                &mut agents,
                &mut reservations,
                &mut debts,
                &mut next_debt_id,
                &mut money,
                &mut banks
            ),
        );
        let borrow = order(2, LoanSide::Borrow, Gold(1), 2);
        assert!(reservations.reserve_order(&agents, &borrow));
        let first = book.add_order_m3(
            borrow,
            0,
            loan_m3_context!(
                &mut agents,
                &mut reservations,
                &mut debts,
                &mut next_debt_id,
                &mut money,
                &mut banks
            ),
        );

        assert_eq!(first.len(), 1);
        assert_eq!(
            reservations.bank_fiduciary_issued_this_tick(BankId(1)),
            Gold(1)
        );

        let resting_lend = bank_lend(3);
        assert!(!reservations.reserve_order_m3(
            &agents,
            &resting_lend,
            &banks,
            &[],
            Regime::FractionalConvertible
        ));
        book.add_order_m3(
            resting_lend,
            0,
            loan_m3_context!(
                &mut agents,
                &mut reservations,
                &mut debts,
                &mut next_debt_id,
                &mut money,
                &mut banks
            ),
        );
        let blocked_borrow = order(3, LoanSide::Borrow, Gold(1), 4);
        assert!(reservations.reserve_order(&agents, &blocked_borrow));
        let blocked = book.add_order_m3(
            blocked_borrow,
            0,
            loan_m3_context!(
                &mut agents,
                &mut reservations,
                &mut debts,
                &mut next_debt_id,
                &mut money,
                &mut banks
            ),
        );

        assert!(blocked.is_empty());
        assert_eq!(debts.len(), 1);

        reservations.reset_tick_lender_capacity();
        let next_tick_lend = bank_lend(5);
        assert!(reservations.reserve_order_m3(
            &agents,
            &next_tick_lend,
            &banks,
            &[],
            Regime::FractionalConvertible
        ));
        let next_tick = book.add_order_m3(
            next_tick_lend,
            1,
            loan_m3_context!(
                &mut agents,
                &mut reservations,
                &mut debts,
                &mut next_debt_id,
                &mut money,
                &mut banks
            ),
        );

        assert_eq!(next_tick.len(), 1);
        assert_eq!(debts.len(), 2);
        assert_eq!(banks[0].loans_outstanding, Gold(2));
    }

    #[test]
    fn reserve_bound_fractional_bank_can_use_remaining_same_tick_capacity() {
        let mut agents = vec![agent(2, Gold::ZERO), agent(3, Gold::ZERO)];
        let mut money = MoneySystem::from_agents(&agents);
        let mut reservations = LoanReservations::new();
        let mut book = LoanOrderBook::new();
        let mut debts = Vec::new();
        let mut next_debt_id = 1;
        let mut banks = vec![bank()];
        banks[0].reserves = Gold(1);
        banks[0].reserve_ratio_bps = ReserveRatioBps(5_000);
        banks[0].policy.max_new_fiduciary_per_tick = Gold(2);

        for (borrower, lend_seq, borrow_seq) in [(2, 1, 2), (3, 3, 4)] {
            let lend = bank_lend(lend_seq);
            assert!(reservations.reserve_order_m3(
                &agents,
                &lend,
                &banks,
                &[],
                Regime::FractionalConvertible
            ));
            assert!(book
                .add_order_m3(
                    lend,
                    0,
                    loan_m3_context!(
                        &mut agents,
                        &mut reservations,
                        &mut debts,
                        &mut next_debt_id,
                        &mut money,
                        &mut banks
                    ),
                )
                .is_empty());

            let borrow = order(borrower, LoanSide::Borrow, Gold(2), borrow_seq);
            assert!(reservations.reserve_order(&agents, &borrow));
            let trades = book.add_order_m3(
                borrow,
                0,
                loan_m3_context!(
                    &mut agents,
                    &mut reservations,
                    &mut debts,
                    &mut next_debt_id,
                    &mut money,
                    &mut banks
                ),
            );

            assert_eq!(trades.len(), 1);
        }

        assert_eq!(debts.len(), 2);
        assert_eq!(banks[0].demand_deposits, Gold(2));
        assert_eq!(banks[0].fiduciary_issued, Gold(2));
        assert_eq!(
            reservations.bank_fiduciary_issued_this_tick(BankId(1)),
            Gold(2)
        );
        assert_eq!(
            reservations.bank_fiduciary_capacity(&banks[0], Regime::FractionalConvertible),
            Gold::ZERO
        );
    }

    #[test]
    fn issuer_fiat_credit_capacity_is_consumed_per_tick() {
        let mut agents = vec![agent(2, Gold::ZERO), agent(3, Gold::ZERO)];
        let mut money = MoneySystem::from_agents(&agents);
        let mut reservations = LoanReservations::new();
        let mut book = LoanOrderBook::new();
        let mut debts = Vec::new();
        let mut next_debt_id = 1;
        let mut banks = Vec::new();
        let mut issuers = vec![issuer()];
        issuers[0].policy.max_credit_issue_per_tick = Gold(1);

        let lend = issuer_lend(1);
        assert!(reservations.reserve_order_m3(&agents, &lend, &banks, &issuers, Regime::Fiat));
        book.add_order_m3(
            lend,
            0,
            LoanM3Context {
                agents: &mut agents,
                reservations: &mut reservations,
                debts: &mut debts,
                next_debt_id: &mut next_debt_id,
                money_system: &mut money,
                banks: &mut banks,
                issuers: &mut issuers,
                regime: Regime::Fiat,
            },
        );
        let borrow = order(2, LoanSide::Borrow, Gold(1), 2);
        assert!(reservations.reserve_order(&agents, &borrow));
        let first = book.add_order_m3(
            borrow,
            0,
            LoanM3Context {
                agents: &mut agents,
                reservations: &mut reservations,
                debts: &mut debts,
                next_debt_id: &mut next_debt_id,
                money_system: &mut money,
                banks: &mut banks,
                issuers: &mut issuers,
                regime: Regime::Fiat,
            },
        );

        assert_eq!(first.len(), 1);
        assert_eq!(
            reservations.issuer_fiat_credit_issued_this_tick(IssuerId(1)),
            Gold(1)
        );

        let blocked_lend = issuer_lend(3);
        assert!(!reservations.reserve_order_m3(
            &agents,
            &blocked_lend,
            &banks,
            &issuers,
            Regime::Fiat
        ));
        let blocked_borrow = order(3, LoanSide::Borrow, Gold(1), 4);
        assert!(reservations.reserve_order(&agents, &blocked_borrow));
        let blocked = book.add_order_m3(
            blocked_borrow,
            0,
            LoanM3Context {
                agents: &mut agents,
                reservations: &mut reservations,
                debts: &mut debts,
                next_debt_id: &mut next_debt_id,
                money_system: &mut money,
                banks: &mut banks,
                issuers: &mut issuers,
                regime: Regime::Fiat,
            },
        );

        assert!(blocked.is_empty());
        assert_eq!(debts.len(), 1);

        reservations.reset_tick_lender_capacity();
        let next_tick_lend = issuer_lend(5);
        assert!(reservations.reserve_order_m3(
            &agents,
            &next_tick_lend,
            &banks,
            &issuers,
            Regime::Fiat
        ));
        let next_tick = book.add_order_m3(
            next_tick_lend,
            1,
            LoanM3Context {
                agents: &mut agents,
                reservations: &mut reservations,
                debts: &mut debts,
                next_debt_id: &mut next_debt_id,
                money_system: &mut money,
                banks: &mut banks,
                issuers: &mut issuers,
                regime: Regime::Fiat,
            },
        );

        assert_eq!(next_tick.len(), 1);
        assert_eq!(debts.len(), 2);
        assert_eq!(issuers[0].fiat_credit_outstanding, Gold(2));
    }

    #[test]
    fn bank_loan_trade_creates_debt_to_bank() {
        let mut agents = vec![agent(2, Gold::ZERO)];
        let mut money = MoneySystem::from_agents(&agents);
        let mut reservations = LoanReservations::new();
        let mut book = LoanOrderBook::new();
        let mut debts = Vec::new();
        let mut next_debt_id = 1;
        let mut banks = vec![bank()];

        let lend = bank_lend(1);
        assert!(reservations.reserve_order(&agents, &lend));
        book.add_order_m3(
            lend,
            0,
            loan_m3_context!(
                &mut agents,
                &mut reservations,
                &mut debts,
                &mut next_debt_id,
                &mut money,
                &mut banks
            ),
        );
        let borrow = order(2, LoanSide::Borrow, Gold(2), 2);
        assert!(reservations.reserve_order(&agents, &borrow));
        let trades = book.add_order_m3(
            borrow,
            0,
            loan_m3_context!(
                &mut agents,
                &mut reservations,
                &mut debts,
                &mut next_debt_id,
                &mut money,
                &mut banks
            ),
        );

        assert_eq!(trades.len(), 1);
        assert_eq!(trades[0].lender, CreditLender::Bank(BankId(1)));
        assert_eq!(trades[0].funding, CreditSource::BankFiduciary(BankId(1)));
        assert_eq!(debts[0].lender, CreditLender::Bank(BankId(1)));
        assert_eq!(debts[0].funding, CreditSource::BankFiduciary(BankId(1)));
    }

    #[test]
    fn fiduciary_retires_on_repayment() {
        let mut agents = vec![agent(2, Gold::ZERO)];
        let mut money = MoneySystem::from_agents(&agents);
        money
            .issue_demand_claim(BankId(1), AgentId(2), Gold(1), Gold::ZERO)
            .unwrap();
        money.reconcile_agent_cache(&mut agents);
        let mut banks = vec![bank()];
        banks[0]
            .record_fiduciary_loan(Regime::FractionalConvertible, Gold(1))
            .unwrap();
        let mut debts = vec![DebtContract {
            id: DebtId(1),
            lender: CreditLender::Bank(BankId(1)),
            borrower: AgentId(2),
            opened_tick: Tick(0),
            due_tick: Tick(1),
            principal: Gold(1),
            due: Gold(1),
            paid: Gold::ZERO,
            state: DebtState::Open,
            purpose: DebtPurpose::Consumption,
            funding: CreditSource::BankFiduciary(BankId(1)),
        }];

        let summary = settle_due_debts_m3(
            &mut agents,
            &mut debts,
            Tick(1),
            &mut money,
            &mut banks,
            &mut [],
        );

        assert_eq!(summary.settled, 1);
        assert_eq!(summary.credit_retired, Gold(1));
        assert_eq!(money.snapshot().demand_claims, Gold::ZERO);
        assert_eq!(money.snapshot().fiduciary, Gold::ZERO);
        assert_eq!(banks[0].loans_outstanding, Gold::ZERO);
        assert_eq!(debts[0].state, DebtState::Settled);
    }

    #[test]
    fn fiat_credit_repayment_retires_created_principal_in_ledger() {
        let mut agents = vec![agent(2, Gold::ZERO)];
        let mut money = MoneySystem::from_agents(&agents);
        money.credit_fiat(AgentId(2), Gold(1)).unwrap();
        money.reconcile_agent_cache(&mut agents);
        let mut banks = Vec::new();
        let mut issuers = vec![issuer()];
        issuers[0]
            .record_credit_loan(Regime::Fiat, Gold(1))
            .unwrap();
        let mut debts = vec![DebtContract {
            id: DebtId(1),
            lender: CreditLender::Issuer(IssuerId(1)),
            borrower: AgentId(2),
            opened_tick: Tick(0),
            due_tick: Tick(1),
            principal: Gold(1),
            due: Gold(1),
            paid: Gold::ZERO,
            state: DebtState::Open,
            purpose: DebtPurpose::Consumption,
            funding: CreditSource::FiatCredit(IssuerId(1)),
        }];

        let summary = settle_due_debts_m3(
            &mut agents,
            &mut debts,
            Tick(1),
            &mut money,
            &mut banks,
            &mut issuers,
        );

        assert_eq!(summary.settled, 1);
        assert_eq!(summary.credit_retired, Gold(1));
        assert_eq!(money.snapshot().public_fiat, Gold::ZERO);
        assert_eq!(money.base.issuer_fiat_unissued, Gold(1));
        assert_eq!(money.base.fiat_base, Gold(1));
        assert_eq!(issuers[0].fiat_credit_outstanding, Gold::ZERO);
        assert_eq!(issuers[0].fiat_retired, Gold(1));
        assert_eq!(debts[0].state, DebtState::Settled);
        assert!(money.invariants_hold(&agents));
    }

    #[test]
    fn fiat_credit_repayment_with_only_specie_defaults() {
        // Borrower holds specie but not the created fiat (the fiat it borrowed was spent
        // to agent 3). Specie must NOT retire fiat-credit principal: the debt defaults,
        // `credit_retired` stays 0, and the created fiat remains in circulation.
        let mut agents = vec![agent(2, Gold(1)), agent(3, Gold::ZERO)];
        let mut money = MoneySystem::from_agents(&agents);
        money.credit_fiat(AgentId(3), Gold(1)).unwrap();
        money.reconcile_agent_cache(&mut agents);
        let mut banks = Vec::new();
        let mut issuers = vec![issuer()];
        issuers[0]
            .record_credit_loan(Regime::Fiat, Gold(1))
            .unwrap();
        let mut debts = vec![DebtContract {
            id: DebtId(1),
            lender: CreditLender::Issuer(IssuerId(1)),
            borrower: AgentId(2),
            opened_tick: Tick(0),
            due_tick: Tick(1),
            principal: Gold(1),
            due: Gold(1),
            paid: Gold::ZERO,
            state: DebtState::Open,
            purpose: DebtPurpose::Consumption,
            funding: CreditSource::FiatCredit(IssuerId(1)),
        }];

        let summary = settle_due_debts_m3(
            &mut agents,
            &mut debts,
            Tick(1),
            &mut money,
            &mut banks,
            &mut issuers,
        );

        assert_eq!(summary.settled, 0);
        assert_eq!(summary.defaulted, 1);
        assert_eq!(summary.credit_retired, Gold::ZERO);
        assert_eq!(money.snapshot().public_specie, Gold(1)); // borrower's specie untouched
        assert_eq!(money.snapshot().public_fiat, Gold(1)); // created fiat still circulating
        assert_eq!(money.base.issuer_gold_vault, Gold::ZERO); // no specie vaulted
        assert_eq!(issuers[0].fiat_credit_outstanding, Gold(1)); // principal unretired
        assert_eq!(debts[0].state, DebtState::Defaulted);
        assert!(money.invariants_hold(&agents));
    }

    #[test]
    fn fiat_credit_partial_fiat_repayment_retires_only_returned_fiat() {
        // Borrower borrowed Gold(2) fiat, spent Gold(1) to agent 3, holds Gold(1). It can
        // only return the Gold(1) fiat it still holds: that retires Gold(1) of principal,
        // the rest partially defaults, and the unreturned fiat stays in circulation.
        let mut agents = vec![agent(2, Gold::ZERO), agent(3, Gold::ZERO)];
        let mut money = MoneySystem::from_agents(&agents);
        money.credit_fiat(AgentId(2), Gold(1)).unwrap();
        money.credit_fiat(AgentId(3), Gold(1)).unwrap();
        money.reconcile_agent_cache(&mut agents);
        let mut banks = Vec::new();
        let mut issuers = vec![issuer()];
        issuers[0]
            .record_credit_loan(Regime::Fiat, Gold(2))
            .unwrap();
        let mut debts = vec![DebtContract {
            id: DebtId(1),
            lender: CreditLender::Issuer(IssuerId(1)),
            borrower: AgentId(2),
            opened_tick: Tick(0),
            due_tick: Tick(1),
            principal: Gold(2),
            due: Gold(2),
            paid: Gold::ZERO,
            state: DebtState::Open,
            purpose: DebtPurpose::Consumption,
            funding: CreditSource::FiatCredit(IssuerId(1)),
        }];

        let summary = settle_due_debts_m3(
            &mut agents,
            &mut debts,
            Tick(1),
            &mut money,
            &mut banks,
            &mut issuers,
        );

        // credit_retired == fiat_retired: exactly the Gold(1) of fiat the borrower returned.
        assert_eq!(summary.credit_retired, Gold(1));
        assert_eq!(summary.defaulted, 1); // remaining Gold(1) of principal defaults
        assert_eq!(money.snapshot().public_fiat, Gold(1)); // agent 3 still holds the overhang
        assert_eq!(money.base.issuer_gold_vault, Gold::ZERO); // no specie involved
        assert_eq!(issuers[0].fiat_credit_outstanding, Gold(1)); // unreturned principal
        assert_eq!(debts[0].state, DebtState::Defaulted);
        assert!(money.invariants_hold(&agents));
    }

    #[test]
    fn fiat_credit_missing_issuer_defaults_without_panic() {
        let mut agents = vec![agent(2, Gold(1))];
        let mut money = MoneySystem::from_agents(&agents);
        let mut banks = Vec::new();
        let mut issuers = Vec::new();
        let mut debts = vec![DebtContract {
            id: DebtId(1),
            lender: CreditLender::Issuer(IssuerId(9)),
            borrower: AgentId(2),
            opened_tick: Tick(0),
            due_tick: Tick(1),
            principal: Gold(1),
            due: Gold(1),
            paid: Gold::ZERO,
            state: DebtState::Open,
            purpose: DebtPurpose::Consumption,
            funding: CreditSource::FiatCredit(IssuerId(9)),
        }];

        let summary = settle_due_debts_m3(
            &mut agents,
            &mut debts,
            Tick(1),
            &mut money,
            &mut banks,
            &mut issuers,
        );

        assert_eq!(summary.defaulted, 1);
        assert_eq!(summary.paid, Gold::ZERO);
        assert_eq!(money.snapshot().public_specie, Gold(1));
        assert_eq!(debts[0].state, DebtState::Defaulted);
        assert!(money.invariants_hold(&agents));
    }

    #[test]
    fn issuer_lend_order_reservation_checks_policy_capacity() {
        let agents = vec![agent(2, Gold::ZERO)];
        let mut reservations = LoanReservations::new();
        let banks = Vec::new();
        let mut issuers = vec![issuer()];
        issuers[0].policy.max_credit_issue_per_tick = Gold::ZERO;

        let lend = issuer_lend(1);

        assert!(!reservations.reserve_order_m3(&agents, &lend, &banks, &issuers, Regime::Fiat));
    }

    #[test]
    fn bank_claim_interest_payment_retires_full_claim_payment() {
        let mut agents = vec![agent(2, Gold::ZERO)];
        let mut money = MoneySystem::from_agents(&agents);
        money
            .issue_demand_claim(BankId(1), AgentId(2), Gold(2), Gold::ZERO)
            .unwrap();
        money.reconcile_agent_cache(&mut agents);
        let mut banks = vec![bank()];
        banks[0].demand_deposits = Gold(2);
        banks[0].loans_outstanding = Gold(1);
        banks[0].fiduciary_issued = Gold(1);
        let mut debts = vec![DebtContract {
            id: DebtId(1),
            lender: CreditLender::Bank(BankId(1)),
            borrower: AgentId(2),
            opened_tick: Tick(0),
            due_tick: Tick(1),
            principal: Gold(1),
            due: Gold(2),
            paid: Gold::ZERO,
            state: DebtState::Open,
            purpose: DebtPurpose::Consumption,
            funding: CreditSource::BankFiduciary(BankId(1)),
        }];

        let summary = settle_due_debts_m3(
            &mut agents,
            &mut debts,
            Tick(1),
            &mut money,
            &mut banks,
            &mut [],
        );

        assert_eq!(summary.settled, 1);
        assert_eq!(summary.paid, Gold(2));
        assert_eq!(summary.credit_retired, Gold(1));
        assert_eq!(money.snapshot().demand_claims, Gold::ZERO);
        assert_eq!(banks[0].demand_deposits, Gold::ZERO);
        assert_eq!(banks[0].loans_outstanding, Gold::ZERO);
        assert_eq!(banks[0].fiduciary_issued, Gold::ZERO);
        assert!(money.invariants_hold(&agents));
    }

    #[test]
    fn bank_debt_can_be_paid_with_another_bank_claim() {
        let mut agents = vec![agent(2, Gold(1)), agent(3, Gold::ZERO)];
        let mut money = MoneySystem::from_agents(&agents);
        money
            .issue_demand_claim(BankId(1), AgentId(2), Gold(1), Gold(1))
            .unwrap();
        money
            .issue_demand_claim(BankId(2), AgentId(3), Gold(1), Gold::ZERO)
            .unwrap();
        money.reconcile_agent_cache(&mut agents);
        let mut banks = vec![
            Bank {
                id: BankId(1),
                demand_deposits: Gold(1),
                reserves: Gold(1),
                ..bank()
            },
            Bank {
                id: BankId(2),
                demand_deposits: Gold(1),
                loans_outstanding: Gold(1),
                fiduciary_issued: Gold(1),
                ..bank()
            },
        ];
        let mut debts = vec![DebtContract {
            id: DebtId(1),
            lender: CreditLender::Bank(BankId(2)),
            borrower: AgentId(2),
            opened_tick: Tick(0),
            due_tick: Tick(1),
            principal: Gold(1),
            due: Gold(1),
            paid: Gold::ZERO,
            state: DebtState::Open,
            purpose: DebtPurpose::Consumption,
            funding: CreditSource::BankFiduciary(BankId(2)),
        }];

        let summary = settle_due_debts_m3(
            &mut agents,
            &mut debts,
            Tick(1),
            &mut money,
            &mut banks,
            &mut [],
        );

        assert_eq!(summary.settled, 1);
        assert_eq!(summary.credit_retired, Gold(1));
        assert_eq!(money.snapshot().demand_claims, Gold(1));
        assert_eq!(banks[0].demand_deposits, Gold::ZERO);
        assert_eq!(banks[0].reserves, Gold::ZERO);
        assert_eq!(banks[1].demand_deposits, Gold(1));
        assert_eq!(banks[1].reserves, Gold(1));
        assert_eq!(banks[1].loans_outstanding, Gold::ZERO);
        assert_eq!(banks[1].fiduciary_issued, Gold::ZERO);
        assert_eq!(money.snapshot().bank_reserves, Gold(1));
        assert!(money.invariants_hold_with_banks(&agents, &banks));
    }

    #[test]
    fn unbacked_foreign_bank_claim_does_not_mint_lender_reserves() {
        let mut agents = vec![agent(2, Gold::ZERO), agent(3, Gold::ZERO)];
        let mut money = MoneySystem::from_agents(&agents);
        money
            .issue_demand_claim(BankId(1), AgentId(2), Gold(1), Gold::ZERO)
            .unwrap();
        money
            .issue_demand_claim(BankId(2), AgentId(3), Gold(1), Gold::ZERO)
            .unwrap();
        money.reconcile_agent_cache(&mut agents);
        let mut banks = vec![
            Bank {
                id: BankId(1),
                demand_deposits: Gold(1),
                fiduciary_issued: Gold(1),
                ..bank()
            },
            Bank {
                id: BankId(2),
                demand_deposits: Gold(1),
                loans_outstanding: Gold(1),
                fiduciary_issued: Gold(1),
                ..bank()
            },
        ];
        let mut debts = vec![DebtContract {
            id: DebtId(1),
            lender: CreditLender::Bank(BankId(2)),
            borrower: AgentId(2),
            opened_tick: Tick(0),
            due_tick: Tick(1),
            principal: Gold(1),
            due: Gold(1),
            paid: Gold::ZERO,
            state: DebtState::Open,
            purpose: DebtPurpose::Consumption,
            funding: CreditSource::BankFiduciary(BankId(2)),
        }];

        let summary = settle_due_debts_m3(
            &mut agents,
            &mut debts,
            Tick(1),
            &mut money,
            &mut banks,
            &mut [],
        );

        assert_eq!(summary.defaulted, 1);
        assert_eq!(summary.credit_retired, Gold::ZERO);
        assert_eq!(money.snapshot().demand_claims, Gold(2));
        assert_eq!(money.snapshot().bank_reserves, Gold::ZERO);
        assert_eq!(banks[0].reserves, Gold::ZERO);
        assert_eq!(banks[1].reserves, Gold::ZERO);
        assert_eq!(banks[0].demand_deposits, Gold(1));
        assert_eq!(banks[0].fiduciary_issued, Gold(1));
        assert_eq!(banks[1].loans_outstanding, Gold(1));
        assert_eq!(banks[1].fiduciary_issued, Gold(1));
        assert!(money.invariants_hold_with_banks(&agents, &banks));
    }

    #[test]
    fn foreign_claim_clearing_reduces_issuing_bank_fiduciary() {
        let mut agents = vec![agent(2, Gold::ZERO), agent(3, Gold::ZERO)];
        let mut money = MoneySystem::from_agents_with_bank_reserves(&agents, Gold(1));
        money
            .issue_demand_claim(BankId(1), AgentId(2), Gold(1), Gold::ZERO)
            .unwrap();
        money
            .issue_demand_claim(BankId(2), AgentId(3), Gold(1), Gold::ZERO)
            .unwrap();
        money.reconcile_agent_cache(&mut agents);
        let mut banks = vec![
            Bank {
                id: BankId(1),
                demand_deposits: Gold(1),
                reserves: Gold(1),
                fiduciary_issued: Gold(1),
                ..bank()
            },
            Bank {
                id: BankId(2),
                demand_deposits: Gold(1),
                loans_outstanding: Gold(1),
                fiduciary_issued: Gold(1),
                ..bank()
            },
        ];
        let mut debts = vec![DebtContract {
            id: DebtId(1),
            lender: CreditLender::Bank(BankId(2)),
            borrower: AgentId(2),
            opened_tick: Tick(0),
            due_tick: Tick(1),
            principal: Gold(1),
            due: Gold(1),
            paid: Gold::ZERO,
            state: DebtState::Open,
            purpose: DebtPurpose::Consumption,
            funding: CreditSource::BankFiduciary(BankId(2)),
        }];

        let summary = settle_due_debts_m3(
            &mut agents,
            &mut debts,
            Tick(1),
            &mut money,
            &mut banks,
            &mut [],
        );

        assert_eq!(summary.settled, 1);
        assert_eq!(banks[0].demand_deposits, Gold::ZERO);
        assert_eq!(banks[0].reserves, Gold::ZERO);
        assert_eq!(banks[0].fiduciary_issued, Gold::ZERO);
        assert_eq!(banks[1].reserves, Gold(1));
        assert_eq!(banks[1].fiduciary_issued, Gold::ZERO);
        assert_eq!(money.snapshot().demand_claims, Gold(1));
        assert_eq!(money.snapshot().bank_reserves, Gold(1));
        assert!(money.invariants_hold_with_banks(&agents, &banks));
    }

    #[test]
    fn bank_debt_can_be_paid_with_specie_without_destroying_claims() {
        let mut agents = vec![agent(2, Gold(1)), agent(3, Gold::ZERO)];
        let mut money = MoneySystem::from_agents(&agents);
        money
            .issue_demand_claim(BankId(1), AgentId(3), Gold(1), Gold::ZERO)
            .unwrap();
        money.reconcile_agent_cache(&mut agents);
        let mut banks = vec![bank()];
        banks[0].demand_deposits = Gold(1);
        banks[0].loans_outstanding = Gold(1);
        banks[0].fiduciary_issued = Gold(1);
        let mut debts = vec![DebtContract {
            id: DebtId(1),
            lender: CreditLender::Bank(BankId(1)),
            borrower: AgentId(2),
            opened_tick: Tick(0),
            due_tick: Tick(1),
            principal: Gold(1),
            due: Gold(1),
            paid: Gold::ZERO,
            state: DebtState::Open,
            purpose: DebtPurpose::Consumption,
            funding: CreditSource::BankFiduciary(BankId(1)),
        }];

        let summary = settle_due_debts_m3(
            &mut agents,
            &mut debts,
            Tick(1),
            &mut money,
            &mut banks,
            &mut [],
        );

        assert_eq!(summary.settled, 1);
        assert_eq!(money.snapshot().public_specie, Gold::ZERO);
        assert_eq!(money.snapshot().demand_claims, Gold(1));
        assert_eq!(money.snapshot().bank_reserves, Gold(1));
        assert_eq!(money.snapshot().fiduciary, Gold::ZERO);
        assert_eq!(banks[0].reserves, Gold(1));
        assert_eq!(banks[0].demand_deposits, Gold(1));
        assert_eq!(banks[0].loans_outstanding, Gold::ZERO);
        assert_eq!(banks[0].fiduciary_issued, Gold::ZERO);
        assert!(money.invariants_hold(&agents));
    }

    #[test]
    fn bank_repayment_skips_unclearing_foreign_claim_when_specie_can_pay() {
        let mut agents = vec![agent(2, Gold(1)), agent(3, Gold::ZERO)];
        let mut money = MoneySystem::from_agents(&agents);
        money
            .issue_demand_claim(BankId(1), AgentId(2), Gold(1), Gold::ZERO)
            .unwrap();
        money
            .issue_demand_claim(BankId(2), AgentId(3), Gold(1), Gold::ZERO)
            .unwrap();
        money.reconcile_agent_cache(&mut agents);
        let mut banks = vec![
            Bank {
                id: BankId(1),
                demand_deposits: Gold(1),
                fiduciary_issued: Gold(1),
                ..bank()
            },
            Bank {
                id: BankId(2),
                demand_deposits: Gold(1),
                loans_outstanding: Gold(1),
                fiduciary_issued: Gold(1),
                ..bank()
            },
        ];
        let mut debts = vec![DebtContract {
            id: DebtId(1),
            lender: CreditLender::Bank(BankId(2)),
            borrower: AgentId(2),
            opened_tick: Tick(0),
            due_tick: Tick(1),
            principal: Gold(1),
            due: Gold(1),
            paid: Gold::ZERO,
            state: DebtState::Open,
            purpose: DebtPurpose::Consumption,
            funding: CreditSource::BankFiduciary(BankId(2)),
        }];

        let summary = settle_due_debts_m3(
            &mut agents,
            &mut debts,
            Tick(1),
            &mut money,
            &mut banks,
            &mut [],
        );

        assert_eq!(summary.settled, 1);
        assert_eq!(summary.defaulted, 0);
        assert_eq!(summary.paid, Gold(1));
        assert_eq!(summary.credit_retired, Gold(1));
        assert_eq!(money.snapshot().demand_claims, Gold(2));
        assert_eq!(money.snapshot().bank_reserves, Gold(1));
        assert_eq!(banks[0].demand_deposits, Gold(1));
        assert_eq!(banks[0].fiduciary_issued, Gold(1));
        assert_eq!(banks[1].reserves, Gold(1));
        assert_eq!(banks[1].loans_outstanding, Gold::ZERO);
        assert_eq!(banks[1].fiduciary_issued, Gold::ZERO);
        assert!(money.invariants_hold_with_banks(&agents, &banks));
    }

    #[test]
    fn backed_suspended_foreign_bank_claim_still_clears_before_specie() {
        let mut agents = vec![agent(2, Gold(1)), agent(3, Gold::ZERO)];
        let mut money = MoneySystem::from_agents_with_bank_reserves(&agents, Gold(1));
        money
            .issue_demand_claim(BankId(1), AgentId(2), Gold(1), Gold::ZERO)
            .unwrap();
        money
            .issue_demand_claim(BankId(2), AgentId(3), Gold(1), Gold::ZERO)
            .unwrap();
        money.reconcile_agent_cache(&mut agents);
        let mut banks = vec![
            Bank {
                id: BankId(1),
                demand_deposits: Gold(1),
                reserves: Gold(1),
                convertible: false,
                ..bank()
            },
            Bank {
                id: BankId(2),
                demand_deposits: Gold(1),
                loans_outstanding: Gold(1),
                fiduciary_issued: Gold(1),
                ..bank()
            },
        ];
        let mut debts = vec![DebtContract {
            id: DebtId(1),
            lender: CreditLender::Bank(BankId(2)),
            borrower: AgentId(2),
            opened_tick: Tick(0),
            due_tick: Tick(1),
            principal: Gold(1),
            due: Gold(1),
            paid: Gold::ZERO,
            state: DebtState::Open,
            purpose: DebtPurpose::Consumption,
            funding: CreditSource::BankFiduciary(BankId(2)),
        }];

        let summary = settle_due_debts_m3(
            &mut agents,
            &mut debts,
            Tick(1),
            &mut money,
            &mut banks,
            &mut [],
        );

        assert_eq!(summary.settled, 1);
        assert_eq!(summary.defaulted, 0);
        assert_eq!(summary.paid, Gold(1));
        assert_eq!(summary.credit_retired, Gold(1));
        assert_eq!(money.snapshot().public_specie, Gold(1));
        assert_eq!(money.snapshot().demand_claims, Gold(1));
        assert_eq!(money.snapshot().bank_reserves, Gold(1));
        assert_eq!(banks[0].demand_deposits, Gold::ZERO);
        assert_eq!(banks[0].reserves, Gold::ZERO);
        assert_eq!(banks[1].reserves, Gold(1));
        assert_eq!(banks[1].loans_outstanding, Gold::ZERO);
        assert_eq!(banks[1].fiduciary_issued, Gold::ZERO);
        assert!(money.invariants_hold_with_banks(&agents, &banks));
    }

    #[test]
    fn bank_principal_repayment_succeeds_after_fiduciary_already_redeemed() {
        let mut agents = vec![agent(2, Gold(1)), agent(3, Gold(1))];
        let mut money = MoneySystem::from_agents(&agents);
        money
            .issue_demand_claim(BankId(2), AgentId(3), Gold(1), Gold(1))
            .unwrap();
        money.reconcile_agent_cache(&mut agents);
        let mut banks = vec![Bank {
            id: BankId(2),
            reserves: Gold(1),
            demand_deposits: Gold(1),
            loans_outstanding: Gold(1),
            fiduciary_issued: Gold::ZERO,
            ..bank()
        }];
        let mut debts = vec![DebtContract {
            id: DebtId(1),
            lender: CreditLender::Bank(BankId(2)),
            borrower: AgentId(2),
            opened_tick: Tick(0),
            due_tick: Tick(1),
            principal: Gold(1),
            due: Gold(1),
            paid: Gold::ZERO,
            state: DebtState::Open,
            purpose: DebtPurpose::Consumption,
            funding: CreditSource::BankFiduciary(BankId(2)),
        }];

        let summary = settle_due_debts_m3(
            &mut agents,
            &mut debts,
            Tick(1),
            &mut money,
            &mut banks,
            &mut [],
        );

        assert_eq!(summary.settled, 1);
        assert_eq!(summary.defaulted, 0);
        assert_eq!(summary.paid, Gold(1));
        assert_eq!(summary.credit_retired, Gold::ZERO);
        assert_eq!(banks[0].loans_outstanding, Gold::ZERO);
        assert_eq!(banks[0].fiduciary_issued, Gold::ZERO);
        assert_eq!(banks[0].reserves, Gold(2));
        assert_eq!(money.snapshot().demand_claims, Gold(1));
        assert_eq!(money.snapshot().fiduciary, Gold::ZERO);
        assert!(money.invariants_hold_with_banks(&agents, &banks));
    }

    #[test]
    fn failed_bank_claim_issue_leaves_bank_unchanged() {
        let mut agents = vec![agent(2, Gold::ZERO)];
        let mut money = MoneySystem::from_agents(&agents);
        money.claims.demand_claims = Gold(u64::MAX);
        let mut reservations = LoanReservations::new();
        let mut book = LoanOrderBook::new();
        let mut debts = Vec::new();
        let mut next_debt_id = 1;
        let mut banks = vec![bank()];

        let lend = bank_lend(1);
        assert!(reservations.reserve_order(&agents, &lend));
        book.add_order_m3(
            lend,
            0,
            loan_m3_context!(
                &mut agents,
                &mut reservations,
                &mut debts,
                &mut next_debt_id,
                &mut money,
                &mut banks
            ),
        );
        let borrow = order(2, LoanSide::Borrow, Gold(2), 2);
        assert!(reservations.reserve_order(&agents, &borrow));
        let trades = book.add_order_m3(
            borrow,
            0,
            loan_m3_context!(
                &mut agents,
                &mut reservations,
                &mut debts,
                &mut next_debt_id,
                &mut money,
                &mut banks
            ),
        );

        assert!(trades.is_empty());
        assert!(debts.is_empty());
        assert_eq!(banks[0].demand_deposits, Gold::ZERO);
        assert_eq!(banks[0].loans_outstanding, Gold::ZERO);
        assert_eq!(banks[0].fiduciary_issued, Gold::ZERO);
    }

    #[test]
    fn policy_reserved_bank_fill_updates_tick_counter() {
        let mut agents = vec![agent(2, Gold::ZERO)];
        let mut money = MoneySystem::from_agents(&agents);
        let mut reservations = LoanReservations::new();
        let mut book = LoanOrderBook::new();
        let mut debts = Vec::new();
        let mut next_debt_id = 1;
        let mut banks = vec![bank()];
        banks[0].policy.max_new_fiduciary_per_tick = Gold(1);

        let lend = bank_lend(1);
        assert!(reservations.reserve_order_m3(
            &agents,
            &lend,
            &banks,
            &[],
            Regime::FractionalConvertible
        ));
        assert!(book
            .add_order_m3(
                lend,
                0,
                loan_m3_context!(
                    &mut agents,
                    &mut reservations,
                    &mut debts,
                    &mut next_debt_id,
                    &mut money,
                    &mut banks
                ),
            )
            .is_empty());
        assert_eq!(reservations.bank_fiduciary_open(BankId(1)), Gold(1));

        let borrow = order(2, LoanSide::Borrow, Gold(1), 2);
        assert!(reservations.reserve_order(&agents, &borrow));
        let trades = book.add_order_m3(
            borrow,
            0,
            loan_m3_context!(
                &mut agents,
                &mut reservations,
                &mut debts,
                &mut next_debt_id,
                &mut money,
                &mut banks
            ),
        );

        assert_eq!(trades.len(), 1);
        assert_eq!(reservations.bank_fiduciary_open(BankId(1)), Gold::ZERO);
        assert_eq!(
            reservations.bank_fiduciary_issued_this_tick(BankId(1)),
            Gold(1)
        );
        assert!(!reservations.reserve_order_m3(
            &agents,
            &bank_lend(3),
            &banks,
            &[],
            Regime::FractionalConvertible
        ));
    }

    #[test]
    fn policy_reserved_issuer_fill_updates_tick_counter() {
        let mut agents = vec![agent(2, Gold::ZERO)];
        let mut money = MoneySystem::from_agents(&agents);
        let mut reservations = LoanReservations::new();
        let mut book = LoanOrderBook::new();
        let mut debts = Vec::new();
        let mut next_debt_id = 1;
        let mut banks = Vec::new();
        let mut issuers = vec![issuer()];
        issuers[0].policy.max_credit_issue_per_tick = Gold(1);

        let lend = issuer_lend(1);
        assert!(reservations.reserve_order_m3(&agents, &lend, &banks, &issuers, Regime::Fiat));
        assert!(book
            .add_order_m3(
                lend,
                0,
                LoanM3Context {
                    agents: &mut agents,
                    reservations: &mut reservations,
                    debts: &mut debts,
                    next_debt_id: &mut next_debt_id,
                    money_system: &mut money,
                    banks: &mut banks,
                    issuers: &mut issuers,
                    regime: Regime::Fiat,
                },
            )
            .is_empty());
        assert_eq!(reservations.issuer_fiat_credit_open(IssuerId(1)), Gold(1));

        let borrow = order(2, LoanSide::Borrow, Gold(1), 2);
        assert!(reservations.reserve_order(&agents, &borrow));
        let trades = book.add_order_m3(
            borrow,
            0,
            LoanM3Context {
                agents: &mut agents,
                reservations: &mut reservations,
                debts: &mut debts,
                next_debt_id: &mut next_debt_id,
                money_system: &mut money,
                banks: &mut banks,
                issuers: &mut issuers,
                regime: Regime::Fiat,
            },
        );

        assert_eq!(trades.len(), 1);
        assert_eq!(
            reservations.issuer_fiat_credit_open(IssuerId(1)),
            Gold::ZERO
        );
        assert_eq!(
            reservations.issuer_fiat_credit_issued_this_tick(IssuerId(1)),
            Gold(1)
        );
        assert!(!reservations.reserve_order_m3(
            &agents,
            &issuer_lend(3),
            &banks,
            &issuers,
            Regime::Fiat
        ));
    }

    #[test]
    fn bank_policy_reserved_order_can_commit_multiple_partial_fills() {
        let agents = vec![agent(2, Gold::ZERO), agent(3, Gold::ZERO)];
        let mut money = MoneySystem::from_agents(&agents);
        let mut reservations = LoanReservations::new();
        let mut banks = vec![bank()];
        banks[0].policy.max_new_fiduciary_per_tick = Gold(2);
        let mut lend = bank_lend(1);
        lend.present = Gold(2);

        assert!(reservations.reserve_order_m3(
            &agents,
            &lend,
            &banks,
            &[],
            Regime::FractionalConvertible
        ));
        assert_eq!(reservations.bank_fiduciary_open(BankId(1)), Gold(2));

        assert!(apply_bank_fiduciary_loan(
            &mut money,
            &mut banks,
            &mut reservations,
            Regime::FractionalConvertible,
            BankCreditApplication {
                bank: BankId(1),
                order_seq: lend.seq,
                borrower: AgentId(2),
                amount: Gold(1),
            },
        )
        .is_some());

        assert_eq!(reservations.bank_fiduciary_open(BankId(1)), Gold(1));
        assert_eq!(reservations.policy_lend_order_open(lend.seq), Gold(1));
        assert!(reservations.policy_lend_order_open.contains_key(&lend.seq));
        assert_eq!(
            reservations.bank_fiduciary_issued_this_tick(BankId(1)),
            Gold(1)
        );

        assert!(apply_bank_fiduciary_loan(
            &mut money,
            &mut banks,
            &mut reservations,
            Regime::FractionalConvertible,
            BankCreditApplication {
                bank: BankId(1),
                order_seq: lend.seq,
                borrower: AgentId(3),
                amount: Gold(1),
            },
        )
        .is_some());

        assert_eq!(reservations.bank_fiduciary_open(BankId(1)), Gold::ZERO);
        assert_eq!(reservations.policy_lend_order_open(lend.seq), Gold::ZERO);
        assert!(!reservations.policy_lend_order_open.contains_key(&lend.seq));
        assert_eq!(
            reservations.bank_fiduciary_issued_this_tick(BankId(1)),
            Gold(2)
        );
        assert_eq!(money.spendable_total(AgentId(2)), Gold(1));
        assert_eq!(money.spendable_total(AgentId(3)), Gold(1));
        assert_eq!(banks[0].fiduciary_issued, Gold(2));
    }

    #[test]
    fn issuer_policy_reserved_order_can_commit_multiple_partial_fills() {
        let agents = vec![agent(2, Gold::ZERO), agent(3, Gold::ZERO)];
        let mut money = MoneySystem::from_agents(&agents);
        let mut reservations = LoanReservations::new();
        let banks = Vec::new();
        let mut issuers = vec![issuer()];
        issuers[0].policy.max_credit_issue_per_tick = Gold(2);
        let mut lend = issuer_lend(1);
        lend.present = Gold(2);

        assert!(reservations.reserve_order_m3(&agents, &lend, &banks, &issuers, Regime::Fiat));
        assert_eq!(reservations.issuer_fiat_credit_open(IssuerId(1)), Gold(2));

        assert!(apply_issuer_fiat_credit_loan(
            &mut money,
            &mut issuers,
            &mut reservations,
            Regime::Fiat,
            IssuerCreditApplication {
                issuer: IssuerId(1),
                order_seq: lend.seq,
                borrower: AgentId(2),
                amount: Gold(1),
            },
        )
        .is_some());

        assert_eq!(reservations.issuer_fiat_credit_open(IssuerId(1)), Gold(1));
        assert_eq!(reservations.policy_lend_order_open(lend.seq), Gold(1));
        assert!(reservations.policy_lend_order_open.contains_key(&lend.seq));
        assert_eq!(
            reservations.issuer_fiat_credit_issued_this_tick(IssuerId(1)),
            Gold(1)
        );

        assert!(apply_issuer_fiat_credit_loan(
            &mut money,
            &mut issuers,
            &mut reservations,
            Regime::Fiat,
            IssuerCreditApplication {
                issuer: IssuerId(1),
                order_seq: lend.seq,
                borrower: AgentId(3),
                amount: Gold(1),
            },
        )
        .is_some());

        assert_eq!(
            reservations.issuer_fiat_credit_open(IssuerId(1)),
            Gold::ZERO
        );
        assert_eq!(reservations.policy_lend_order_open(lend.seq), Gold::ZERO);
        assert!(!reservations.policy_lend_order_open.contains_key(&lend.seq));
        assert_eq!(
            reservations.issuer_fiat_credit_issued_this_tick(IssuerId(1)),
            Gold(2)
        );
        assert_eq!(money.spendable_total(AgentId(2)), Gold(1));
        assert_eq!(money.spendable_total(AgentId(3)), Gold(1));
        assert_eq!(issuers[0].fiat_credit_outstanding, Gold(2));
    }

    #[test]
    fn bank_fiduciary_counter_overflow_does_not_mutate_ledger() {
        let mut agents = vec![agent(2, Gold::ZERO)];
        let mut money = MoneySystem::from_agents(&agents);
        let mut reservations = LoanReservations::new();
        let mut book = LoanOrderBook::new();
        let mut debts = Vec::new();
        let mut next_debt_id = 1;
        let mut banks = vec![bank()];
        banks[0].policy.max_new_fiduciary_per_tick = Gold(u64::MAX);

        let lend = bank_lend(1);
        assert!(reservations.reserve_order_m3(
            &agents,
            &lend,
            &banks,
            &[],
            Regime::FractionalConvertible
        ));
        assert!(book
            .add_order_m3(
                lend,
                0,
                loan_m3_context!(
                    &mut agents,
                    &mut reservations,
                    &mut debts,
                    &mut next_debt_id,
                    &mut money,
                    &mut banks
                ),
            )
            .is_empty());
        reservations
            .bank_fiduciary_this_tick
            .insert(BankId(1), Gold(u64::MAX));

        let money_before = money.clone();
        let bank_before = banks[0].clone();
        let open_before = reservations.bank_fiduciary_open(BankId(1));
        let counter_before = reservations.bank_fiduciary_issued_this_tick(BankId(1));
        let spendable_before = money.spendable_total(AgentId(2));
        let debt_count_before = debts.len();
        let trade_count_before = book.tape.len();

        let borrow = order(2, LoanSide::Borrow, Gold(1), 2);
        assert!(reservations.reserve_order(&agents, &borrow));
        let trades = book.add_order_m3(
            borrow,
            0,
            loan_m3_context!(
                &mut agents,
                &mut reservations,
                &mut debts,
                &mut next_debt_id,
                &mut money,
                &mut banks
            ),
        );

        assert!(trades.is_empty());
        assert_eq!(money, money_before);
        assert_eq!(banks[0], bank_before);
        assert_eq!(reservations.bank_fiduciary_open(BankId(1)), open_before);
        assert_eq!(
            reservations.bank_fiduciary_issued_this_tick(BankId(1)),
            counter_before
        );
        assert_eq!(money.spendable_total(AgentId(2)), spendable_before);
        assert_eq!(debts.len(), debt_count_before);
        assert_eq!(book.tape.len(), trade_count_before);
    }

    #[test]
    fn issuer_credit_counter_overflow_does_not_mutate_ledger() {
        let mut agents = vec![agent(2, Gold::ZERO)];
        let mut money = MoneySystem::from_agents(&agents);
        let mut reservations = LoanReservations::new();
        let mut book = LoanOrderBook::new();
        let mut debts = Vec::new();
        let mut next_debt_id = 1;
        let mut banks = Vec::new();
        let mut issuers = vec![issuer()];
        issuers[0].policy.max_credit_issue_per_tick = Gold(u64::MAX);

        let lend = issuer_lend(1);
        assert!(reservations.reserve_order_m3(&agents, &lend, &banks, &issuers, Regime::Fiat));
        assert!(book
            .add_order_m3(
                lend,
                0,
                LoanM3Context {
                    agents: &mut agents,
                    reservations: &mut reservations,
                    debts: &mut debts,
                    next_debt_id: &mut next_debt_id,
                    money_system: &mut money,
                    banks: &mut banks,
                    issuers: &mut issuers,
                    regime: Regime::Fiat,
                },
            )
            .is_empty());
        reservations
            .issuer_fiat_credit_this_tick
            .insert(IssuerId(1), Gold(u64::MAX));

        let money_before = money.clone();
        let issuer_before = issuers[0].clone();
        let open_before = reservations.issuer_fiat_credit_open(IssuerId(1));
        let counter_before = reservations.issuer_fiat_credit_issued_this_tick(IssuerId(1));
        let spendable_before = money.spendable_total(AgentId(2));
        let debt_count_before = debts.len();
        let trade_count_before = book.tape.len();

        let borrow = order(2, LoanSide::Borrow, Gold(1), 2);
        assert!(reservations.reserve_order(&agents, &borrow));
        let trades = book.add_order_m3(
            borrow,
            0,
            LoanM3Context {
                agents: &mut agents,
                reservations: &mut reservations,
                debts: &mut debts,
                next_debt_id: &mut next_debt_id,
                money_system: &mut money,
                banks: &mut banks,
                issuers: &mut issuers,
                regime: Regime::Fiat,
            },
        );

        assert!(trades.is_empty());
        assert_eq!(money, money_before);
        assert_eq!(issuers[0], issuer_before);
        assert_eq!(
            reservations.issuer_fiat_credit_open(IssuerId(1)),
            open_before
        );
        assert_eq!(
            reservations.issuer_fiat_credit_issued_this_tick(IssuerId(1)),
            counter_before
        );
        assert_eq!(money.spendable_total(AgentId(2)), spendable_before);
        assert_eq!(debts.len(), debt_count_before);
        assert_eq!(book.tape.len(), trade_count_before);
    }

    #[test]
    fn loan_tape_distinguishes_purpose_from_funding_source() {
        let mut agents = vec![agent(2, Gold::ZERO)];
        let mut money = MoneySystem::from_agents(&agents);
        let mut reservations = LoanReservations::new();
        let mut book = LoanOrderBook::new();
        let mut debts = Vec::new();
        let mut next_debt_id = 1;
        let mut banks = vec![bank()];

        let lend = bank_lend(1);
        assert!(reservations.reserve_order(&agents, &lend));
        book.add_order_m3(
            lend,
            0,
            loan_m3_context!(
                &mut agents,
                &mut reservations,
                &mut debts,
                &mut next_debt_id,
                &mut money,
                &mut banks
            ),
        );
        let mut borrow = order(2, LoanSide::Borrow, Gold(2), 2);
        borrow.purpose = LoanPurpose::ProjectFunding(crate::purpose::ProjectPlanId(7));
        assert!(reservations.reserve_order(&agents, &borrow));
        let trades = book.add_order_m3(
            borrow,
            0,
            loan_m3_context!(
                &mut agents,
                &mut reservations,
                &mut debts,
                &mut next_debt_id,
                &mut money,
                &mut banks
            ),
        );
        let tape = render_loan_tape(&trades);

        assert!(tape.starts_with(
            "tick,lender,borrower,present,future_due,horizon,debt,purpose,project,funding,lender_party"
        ));
        assert!(tape.contains("ProjectFunding(plan=7)"));
        assert!(tape.contains("BankFiduciary(1)"));
        assert!(tape.contains("Bank(1)"));
    }
}
