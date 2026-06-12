//! Labor factor orders and reserve accounting.

use std::cmp::Reverse;
use std::collections::{BTreeMap, BTreeSet};

use crate::agent::{Agent, AgentId, WantKind};
use crate::agio::{temporal_provisioning_bitmap_for_money, TemporalEndowment};
use crate::capital::{
    advance_project, find_line, M2Project, M2ProjectId, M2ProjectState, ProjectLine,
};
use crate::good::{Gold, GoodId, Horizon, GOLD};
use crate::ledger::MoneySystem;
use crate::money::{AcceptedMedia, LaborWageTender};
use crate::project::Tick;
use crate::purpose::CreditLender;
use crate::record::WagePaymentAuditRecord;
use crate::timemarket::{DebtContract, DebtState};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FactorKind {
    Labor,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FactorSide {
    Hire,
    Work,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LaborOrder {
    pub agent: AgentId,
    pub side: FactorSide,
    pub wage_limit: Gold,
    pub qty: u32,
    pub seq: u64,
    pub expires_tick: u64,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LaborTrade {
    pub tick: u64,
    pub employer: AgentId,
    pub worker: AgentId,
    pub wage: Gold,
    pub qty: u32,
    pub project: M2ProjectId,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct LaborEntry {
    order: LaborOrder,
    project: Option<M2ProjectId>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct LaborReservations {
    gold: Vec<(AgentId, Gold)>,
    labor: Vec<(AgentId, u32)>,
}

impl LaborReservations {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn reserved_gold(&self, agent: AgentId) -> Gold {
        self.gold
            .iter()
            .find(|(entry, _)| *entry == agent)
            .map(|(_, gold)| *gold)
            .unwrap_or(Gold::ZERO)
    }

    pub fn reserved_labor(&self, agent: AgentId) -> u32 {
        self.labor
            .iter()
            .find(|(entry, _)| *entry == agent)
            .map(|(_, qty)| *qty)
            .unwrap_or(0)
    }

    pub fn reserve_order(&mut self, agents: &[Agent], order: &LaborOrder) -> bool {
        match order.side {
            FactorSide::Hire => {
                let Some(agent) = agents.iter().find(|agent| agent.id == order.agent) else {
                    return false;
                };
                let Some(amount) = order.wage_limit.mul_qty(order.qty) else {
                    return false;
                };
                if agent.gold.saturating_sub(self.reserved_gold(order.agent)) < amount {
                    return false;
                }
                self.add_gold(order.agent, amount);
                true
            }
            FactorSide::Work => {
                let Some(agent) = agents.iter().find(|agent| agent.id == order.agent) else {
                    return false;
                };
                if agent
                    .labor_capacity
                    .saturating_sub(self.reserved_labor(order.agent))
                    < order.qty
                {
                    return false;
                }
                self.add_labor(order.agent, order.qty);
                true
            }
        }
    }

    pub fn release_order(&mut self, order: &LaborOrder) {
        match order.side {
            FactorSide::Hire => {
                if let Some(amount) = order.wage_limit.mul_qty(order.qty) {
                    self.release_gold(order.agent, amount);
                }
            }
            FactorSide::Work => self.release_labor(order.agent, order.qty),
        }
    }

    fn release_filled(&mut self, order: &LaborOrder, qty: u32) {
        match order.side {
            FactorSide::Hire => {
                if let Some(amount) = order.wage_limit.mul_qty(qty) {
                    self.release_gold(order.agent, amount);
                }
            }
            FactorSide::Work => self.release_labor(order.agent, qty),
        }
    }

    fn add_gold(&mut self, agent: AgentId, amount: Gold) {
        if let Some((_, gold)) = self.gold.iter_mut().find(|(entry, _)| *entry == agent) {
            *gold = gold.saturating_add(amount);
        } else {
            self.gold.push((agent, amount));
            self.gold.sort_by_key(|(entry, _)| *entry);
        }
    }

    fn add_labor(&mut self, agent: AgentId, qty: u32) {
        if let Some((_, labor)) = self.labor.iter_mut().find(|(entry, _)| *entry == agent) {
            *labor = labor.saturating_add(qty);
        } else {
            self.labor.push((agent, qty));
            self.labor.sort_by_key(|(entry, _)| *entry);
        }
    }

    fn release_gold(&mut self, agent: AgentId, amount: Gold) {
        if let Some((_, gold)) = self.gold.iter_mut().find(|(entry, _)| *entry == agent) {
            *gold = gold.saturating_sub(amount);
        }
        self.gold.retain(|(_, gold)| *gold > Gold::ZERO);
    }

    fn release_labor(&mut self, agent: AgentId, qty: u32) {
        if let Some((_, labor)) = self.labor.iter_mut().find(|(entry, _)| *entry == agent) {
            *labor = labor.saturating_sub(qty);
        }
        self.labor.retain(|(_, labor)| *labor > 0);
    }
}

pub struct LaborBook {
    work: BTreeMap<(Gold, u64), LaborEntry>,
    hire: BTreeMap<(Reverse<Gold>, u64), LaborEntry>,
    live_seqs: BTreeSet<u64>,
    pub tape: Vec<LaborTrade>,
}

pub struct LaborMarketView<'a> {
    pub agents: &'a mut [Agent],
    pub reservations: &'a mut LaborReservations,
    pub projects: &'a mut [M2Project],
    pub lines: &'a [ProjectLine],
    pub money_system: Option<&'a mut MoneySystem>,
    pub wage_media: AcceptedMedia,
    pub wage_audit: Option<&'a mut Vec<WagePaymentAuditRecord>>,
    pub wage_tender: LaborWageTender,
}

impl LaborBook {
    pub fn new() -> Self {
        Self {
            work: BTreeMap::new(),
            hire: BTreeMap::new(),
            live_seqs: BTreeSet::new(),
            tape: Vec::new(),
        }
    }

    pub fn add_order(
        &mut self,
        order: LaborOrder,
        project: Option<M2ProjectId>,
        tick: u64,
        market: &mut LaborMarketView<'_>,
    ) -> Vec<LaborTrade> {
        if order.qty == 0 || order.wage_limit == Gold::ZERO || order.expires_tick <= tick {
            market.reservations.release_order(&order);
            return Vec::new();
        }
        let mut entry = LaborEntry { order, project };
        let trades = match entry.order.side {
            FactorSide::Hire => self.match_hire(&mut entry, tick, market),
            FactorSide::Work => self.match_work(&mut entry, tick, market),
        };
        if entry.order.qty > 0 {
            if !labor_entry_still_valid(&entry, market) {
                market.reservations.release_order(&entry.order);
                return trades;
            }
            self.insert(entry);
        }
        self.tape.extend(trades.iter().cloned());
        trades
    }

    pub fn purge_expired(&mut self, tick: u64, reservations: &mut LaborReservations) -> u32 {
        let mut expired = 0;
        let work_keys = self
            .work
            .iter()
            .filter(|(_, entry)| entry.order.expires_tick <= tick)
            .map(|(key, _)| *key)
            .collect::<Vec<_>>();
        for key in work_keys {
            if let Some(entry) = self.work.remove(&key) {
                self.live_seqs.remove(&entry.order.seq);
                reservations.release_order(&entry.order);
                expired += 1;
            }
        }
        let hire_keys = self
            .hire
            .iter()
            .filter(|(_, entry)| entry.order.expires_tick <= tick)
            .map(|(key, _)| *key)
            .collect::<Vec<_>>();
        for key in hire_keys {
            if let Some(entry) = self.hire.remove(&key) {
                self.live_seqs.remove(&entry.order.seq);
                reservations.release_order(&entry.order);
                expired += 1;
            }
        }
        expired
    }

    pub fn purge_invalid_hires(
        &mut self,
        reservations: &mut LaborReservations,
        projects: &[M2Project],
        lines: &[ProjectLine],
    ) -> u32 {
        let mut purged = 0;
        let hire_keys = self
            .hire
            .iter()
            .filter(|(_, entry)| !hire_entry_still_valid(entry, projects, lines))
            .map(|(key, _)| *key)
            .collect::<Vec<_>>();
        for key in hire_keys {
            if let Some(entry) = self.hire.remove(&key) {
                self.live_seqs.remove(&entry.order.seq);
                reservations.release_order(&entry.order);
                purged += 1;
            }
        }
        purged
    }

    pub fn has_live(&self, agent: AgentId, side: FactorSide) -> bool {
        match side {
            FactorSide::Hire => self.hire.values().any(|entry| entry.order.agent == agent),
            FactorSide::Work => self.work.values().any(|entry| entry.order.agent == agent),
        }
    }

    pub fn live_order(&self, agent: AgentId, side: FactorSide) -> Option<LaborOrder> {
        match side {
            FactorSide::Hire => self
                .hire
                .values()
                .find(|entry| entry.order.agent == agent)
                .map(|entry| entry.order.clone()),
            FactorSide::Work => self
                .work
                .values()
                .find(|entry| entry.order.agent == agent)
                .map(|entry| entry.order.clone()),
        }
    }

    pub fn cancel(
        &mut self,
        agent: AgentId,
        side: FactorSide,
        reservations: &mut LaborReservations,
    ) -> bool {
        match side {
            FactorSide::Hire => {
                let key = self
                    .hire
                    .iter()
                    .find(|(_, entry)| entry.order.agent == agent)
                    .map(|(key, _)| *key);
                let Some(key) = key else {
                    return false;
                };
                if let Some(entry) = self.hire.remove(&key) {
                    self.live_seqs.remove(&entry.order.seq);
                    reservations.release_order(&entry.order);
                    return true;
                }
                false
            }
            FactorSide::Work => {
                let key = self
                    .work
                    .iter()
                    .find(|(_, entry)| entry.order.agent == agent)
                    .map(|(key, _)| *key);
                let Some(key) = key else {
                    return false;
                };
                if let Some(entry) = self.work.remove(&key) {
                    self.live_seqs.remove(&entry.order.seq);
                    reservations.release_order(&entry.order);
                    return true;
                }
                false
            }
        }
    }

    fn match_hire(
        &mut self,
        entry: &mut LaborEntry,
        tick: u64,
        market: &mut LaborMarketView<'_>,
    ) -> Vec<LaborTrade> {
        let mut trades = Vec::new();
        let mut skipped = BTreeSet::new();
        while entry.order.qty > 0 {
            let mut selected = None;
            for (key, resting) in &self.work {
                if skipped.contains(&resting.order.seq) {
                    continue;
                }
                if resting.order.wage_limit > entry.order.wage_limit {
                    break;
                }
                if resting.order.agent == entry.order.agent {
                    continue;
                }
                selected = Some((*key, resting.clone()));
                break;
            }
            let Some((key, resting)) = selected else {
                break;
            };
            self.work.remove(&key);
            self.live_seqs.remove(&resting.order.seq);
            let qty = entry.order.qty.min(resting.order.qty);
            let wage = resting.order.wage_limit;
            if let Some(trade) = apply_labor_trade(
                tick,
                &entry.order,
                entry.project,
                &resting.order,
                wage,
                qty,
                market,
            ) {
                entry.order.qty -= trade.qty;
                let mut remainder = resting;
                remainder.order.qty -= trade.qty;
                if remainder.order.qty > 0 {
                    self.insert(remainder);
                }
                trades.push(trade);
            } else {
                skipped.insert(resting.order.seq);
                self.insert(resting);
            }
        }
        trades
    }

    fn match_work(
        &mut self,
        entry: &mut LaborEntry,
        tick: u64,
        market: &mut LaborMarketView<'_>,
    ) -> Vec<LaborTrade> {
        let mut trades = Vec::new();
        let mut skipped = BTreeSet::new();
        while entry.order.qty > 0 {
            let mut selected = None;
            for (key, resting) in &self.hire {
                if skipped.contains(&resting.order.seq) {
                    continue;
                }
                if resting.order.wage_limit < entry.order.wage_limit {
                    break;
                }
                if resting.order.agent == entry.order.agent {
                    continue;
                }
                selected = Some((*key, resting.clone()));
                break;
            }
            let Some((key, resting)) = selected else {
                break;
            };
            self.hire.remove(&key);
            self.live_seqs.remove(&resting.order.seq);
            let qty = entry.order.qty.min(resting.order.qty);
            let wage = resting.order.wage_limit;
            if let Some(trade) = apply_labor_trade(
                tick,
                &resting.order,
                resting.project,
                &entry.order,
                wage,
                qty,
                market,
            ) {
                entry.order.qty -= trade.qty;
                let mut remainder = resting;
                remainder.order.qty -= trade.qty;
                if remainder.order.qty > 0 {
                    self.insert(remainder);
                }
                trades.push(trade);
            } else {
                if labor_entry_still_valid(&resting, market) {
                    skipped.insert(resting.order.seq);
                    self.insert(resting);
                } else {
                    market.reservations.release_order(&resting.order);
                }
            }
        }
        trades
    }

    fn insert(&mut self, entry: LaborEntry) {
        self.live_seqs.insert(entry.order.seq);
        match entry.order.side {
            FactorSide::Work => {
                self.work
                    .insert((entry.order.wage_limit, entry.order.seq), entry);
            }
            FactorSide::Hire => {
                self.hire
                    .insert((Reverse(entry.order.wage_limit), entry.order.seq), entry);
            }
        }
    }
}

impl Default for LaborBook {
    fn default() -> Self {
        Self::new()
    }
}

impl Agent {
    pub fn reservation_labor_ask(&self, qty: u32) -> Option<Gold> {
        self.reservation_labor_ask_for_money(qty, GOLD)
    }

    pub fn reservation_labor_ask_for_money(&self, qty: u32, money_good: GoodId) -> Option<Gold> {
        self.reservation_labor_ask_with_debts_for_money(qty, &[], Tick(0), money_good)
    }

    pub fn reservation_labor_ask_with_debts(
        &self,
        qty: u32,
        existing_debts: &[DebtContract],
        tick: Tick,
    ) -> Option<Gold> {
        self.reservation_labor_ask_with_debts_for_money(qty, existing_debts, tick, GOLD)
    }

    pub fn reservation_labor_ask_with_debts_for_money(
        &self,
        qty: u32,
        existing_debts: &[DebtContract],
        tick: Tick,
        money_good: GoodId,
    ) -> Option<Gold> {
        let receivables = existing_debts
            .iter()
            .filter(|debt| {
                debt.state == DebtState::Open && debt.lender == CreditLender::Agent(self.id)
            })
            .cloned()
            .collect::<Vec<_>>();
        let payables = existing_debts
            .iter()
            .filter(|debt| debt.state == DebtState::Open && debt.borrower == self.id)
            .cloned()
            .collect::<Vec<_>>();
        self.reservation_labor_ask_from_claims_for_money(
            qty,
            &receivables,
            &payables,
            tick,
            money_good,
        )
    }

    pub(crate) fn reservation_labor_ask_from_claims_for_money(
        &self,
        qty: u32,
        receivables: &[DebtContract],
        payables: &[DebtContract],
        tick: Tick,
        money_good: GoodId,
    ) -> Option<Gold> {
        if qty == 0 || self.labor_capacity < qty {
            return None;
        }
        let before_endowment = TemporalEndowment {
            stock: &self.stock,
            gold: self.gold,
            receivables,
            payables,
            tick,
        };
        let before =
            temporal_provisioning_bitmap_for_money(&self.scale, &before_endowment, money_good);
        let leisure_rank = self.scale.iter().position(|want| {
            want.kind == WantKind::Leisure && matches!(want.horizon, Horizon::Now)
        });
        for wage in labor_wage_candidates(
            self,
            &before,
            receivables,
            payables,
            tick,
            leisure_rank,
            money_good,
        ) {
            let after_gold = self.gold.checked_add(Gold(wage))?;
            let after_endowment = TemporalEndowment {
                stock: &self.stock,
                gold: after_gold,
                receivables,
                payables,
                tick,
            };
            let after =
                temporal_provisioning_bitmap_for_money(&self.scale, &after_endowment, money_good);
            let target = self.scale.iter().enumerate().find_map(|(index, want)| {
                if want.kind == WantKind::Good(money_good)
                    && !before.get(index).copied().unwrap_or(false)
                    && after.get(index).copied().unwrap_or(false)
                    && leisure_rank.map(|rank| index < rank).unwrap_or(true)
                {
                    Some(index)
                } else {
                    None
                }
            });
            let Some(target) = target else {
                continue;
            };
            if preserved_above_target(&before, &after, target) {
                return Some(Gold(wage));
            }
        }
        None
    }
}

fn apply_labor_trade(
    tick: u64,
    hire: &LaborOrder,
    project: Option<M2ProjectId>,
    work: &LaborOrder,
    wage: Gold,
    qty: u32,
    market: &mut LaborMarketView<'_>,
) -> Option<LaborTrade> {
    if hire.side != FactorSide::Hire
        || work.side != FactorSide::Work
        || hire.agent == work.agent
        || qty == 0
    {
        return None;
    }
    let project_id = project?;
    let employer_pos = market
        .agents
        .iter()
        .position(|agent| agent.id == hire.agent)?;
    let worker_pos = market
        .agents
        .iter()
        .position(|agent| agent.id == work.agent)?;
    if employer_pos == worker_pos {
        return None;
    }
    let project_pos = market
        .projects
        .iter()
        .position(|project| project.id == project_id && project.owner == hire.agent)?;
    let line = find_line(market.lines, market.projects[project_pos].line)?;
    if market.projects[project_pos].state != M2ProjectState::Forming
        || market.projects[project_pos].labor_advanced >= line.required_labor
    {
        return None;
    }
    let remaining_labor = line
        .required_labor
        .saturating_sub(market.projects[project_pos].labor_advanced);
    let fill_qty = qty.min(remaining_labor);
    if fill_qty == 0 {
        return None;
    }

    let payment = wage.mul_qty(fill_qty)?;
    let mut advanced_project = market.projects[project_pos].clone();
    if !advance_project(&mut advanced_project, line, payment, fill_qty) {
        return None;
    }

    if let Some(money_system) = market.money_system.as_deref_mut() {
        if money_system.accepted_spendable_total(hire.agent, market.wage_media) < payment {
            return None;
        }
        let mut staged_money = money_system.clone();
        let payment_composition = staged_money
            .transfer_spendable_with_media(hire.agent, work.agent, payment, market.wage_media)
            .ok()?;
        *money_system = staged_money;
        money_system.reconcile_agent_cache_at(market.agents, employer_pos);
        money_system.reconcile_agent_cache_at(market.agents, worker_pos);
        if let Some(wage_audit) = market.wage_audit.as_deref_mut() {
            let demand_claims = payment_composition
                .claims
                .iter()
                .fold(Gold::ZERO, |total, (_, claim)| total.saturating_add(*claim));
            wage_audit.push(WagePaymentAuditRecord {
                tick,
                project: project_id,
                employer: hire.agent,
                worker: work.agent,
                wage,
                qty: fill_qty,
                amount: payment,
                public_fiat: payment_composition.fiat,
                demand_claims,
                public_specie: payment_composition.specie,
                tender: market.wage_tender,
            });
        }
    } else {
        let employer_gold = market.agents[employer_pos].gold.checked_sub(payment)?;
        let worker_gold = market.agents[worker_pos].gold.checked_add(payment)?;
        market.agents[employer_pos].gold = employer_gold;
        market.agents[worker_pos].gold = worker_gold;
    }
    market.projects[project_pos] = advanced_project;
    market.reservations.release_filled(hire, fill_qty);
    market.reservations.release_filled(work, fill_qty);

    Some(LaborTrade {
        tick,
        employer: hire.agent,
        worker: work.agent,
        wage,
        qty: fill_qty,
        project: project_id,
    })
}

fn labor_entry_still_valid(entry: &LaborEntry, market: &LaborMarketView<'_>) -> bool {
    if entry.order.side != FactorSide::Hire {
        return true;
    }
    hire_entry_still_valid(entry, market.projects, market.lines)
}

fn hire_entry_still_valid(
    entry: &LaborEntry,
    projects: &[M2Project],
    lines: &[ProjectLine],
) -> bool {
    let Some(project_id) = entry.project else {
        return false;
    };
    let Some(project) = projects
        .iter()
        .find(|project| project.id == project_id && project.owner == entry.order.agent)
    else {
        return false;
    };
    let Some(line) = find_line(lines, project.line) else {
        return false;
    };
    project.state == M2ProjectState::Forming && project.labor_advanced < line.required_labor
}

fn labor_wage_candidates(
    agent: &Agent,
    before: &[bool],
    receivables: &[DebtContract],
    payables: &[DebtContract],
    tick: Tick,
    leisure_rank: Option<usize>,
    money_good: GoodId,
) -> Vec<u64> {
    let mut candidates = Vec::new();
    for (index, want) in agent.scale.iter().enumerate() {
        if want.kind != WantKind::Good(money_good)
            || before.get(index).copied().unwrap_or(false)
            || !leisure_rank.map(|rank| index < rank).unwrap_or(true)
        {
            continue;
        }
        let wage =
            wage_needed_for_money_rank(agent, receivables, payables, tick, index, money_good);
        if wage > 0 {
            candidates.push(wage);
        }
    }
    candidates.sort_unstable();
    candidates.dedup();
    candidates
}

fn wage_needed_for_money_rank(
    agent: &Agent,
    receivables: &[DebtContract],
    payables: &[DebtContract],
    tick: Tick,
    target: usize,
    money_good: GoodId,
) -> u64 {
    let Some(want) = agent.scale.get(target) else {
        return 0;
    };
    let required = money_required_through_rank(&agent.scale, target, money_good);
    let temporal_adjustment = match want.horizon {
        Horizon::Now | Horizon::Next => 0,
        Horizon::Later(horizon) => {
            let due_by = Tick(tick.0.saturating_add(u64::from(horizon)));
            debt_capacity_due_by(payables, due_by)
                .saturating_sub(debt_capacity_due_by(receivables, due_by))
        }
    };
    required
        .saturating_add(temporal_adjustment)
        .saturating_sub(agent.gold.0)
}

fn money_required_through_rank(
    scale: &[crate::agent::Want],
    target: usize,
    money_good: GoodId,
) -> u64 {
    scale
        .iter()
        .take(target.saturating_add(1))
        .filter(|want| want.kind == WantKind::Good(money_good))
        .map(|want| u64::from(want.qty))
        .fold(0u64, u64::saturating_add)
}

fn debt_capacity_due_by(debts: &[DebtContract], due_by: Tick) -> u64 {
    debts
        .iter()
        .filter(|debt| debt.state == DebtState::Open && debt.due_tick <= due_by)
        .map(|debt| debt.remaining_due().0)
        .fold(0u64, u64::saturating_add)
}

fn preserved_above_target(before: &[bool], after: &[bool], target: usize) -> bool {
    before
        .iter()
        .zip(after)
        .take(target)
        .all(|(was, now)| !*was || *now)
}

#[cfg(test)]
mod tests {
    use super::{FactorSide, LaborBook, LaborMarketView, LaborOrder, LaborReservations};
    use crate::agent::{Agent, AgentId, Role, Want, WantKind};
    use crate::capital::{build_fish_trap_line, start_project, M2ProjectId, M2ProjectState};
    use crate::good::{Gold, Horizon, Stock, GOLD};
    use crate::money::LaborWageTender;
    use crate::project::Tick;
    use crate::purpose::{CreditLender, CreditSource, DebtPurpose};
    use crate::timemarket::{DebtContract, DebtId, DebtState};

    fn want(kind: WantKind, horizon: Horizon) -> Want {
        Want {
            kind,
            horizon,
            qty: 1,
            satisfied: false,
        }
    }

    fn agent(id: u32, gold: Gold, labor_capacity: u32, scale: Vec<Want>) -> Agent {
        Agent {
            id: AgentId(id),
            scale,
            stock: Stock::new(3),
            gold,
            labor_capacity,
            hunger_deficit: 0,
            roles: vec![Role::Trader],
            expect: Vec::new(),
        }
    }

    #[test]
    fn labor_ask_respects_leisure_rank() {
        let worker = agent(
            1,
            Gold::ZERO,
            1,
            vec![
                want(WantKind::Leisure, Horizon::Now),
                want(WantKind::Good(GOLD), Horizon::Now),
            ],
        );
        assert_eq!(worker.reservation_labor_ask(1), None);

        let worker = agent(
            1,
            Gold::ZERO,
            1,
            vec![
                want(WantKind::Good(GOLD), Horizon::Now),
                want(WantKind::Leisure, Horizon::Now),
            ],
        );
        assert_eq!(worker.reservation_labor_ask(1), Some(Gold(1)));
    }

    #[test]
    fn labor_ask_accounts_for_future_payables() {
        let worker = agent(
            1,
            Gold::ZERO,
            1,
            vec![
                want(WantKind::Good(GOLD), Horizon::Later(4)),
                want(WantKind::Leisure, Horizon::Now),
            ],
        );
        let debts = vec![DebtContract {
            id: DebtId(1),
            lender: CreditLender::Agent(AgentId(2)),
            borrower: AgentId(1),
            opened_tick: Tick(0),
            due_tick: Tick(4),
            principal: Gold(1),
            due: Gold(1),
            paid: Gold::ZERO,
            state: DebtState::Open,
            purpose: DebtPurpose::Consumption,
            funding: CreditSource::Commodity,
        }];

        assert_eq!(
            worker.reservation_labor_ask_with_debts(1, &debts, Tick(0)),
            Some(Gold(2))
        );
    }

    #[test]
    fn labor_ask_does_not_scan_large_holdings_linearly() {
        let worker = agent(
            1,
            Gold(u64::MAX),
            1,
            vec![
                Want {
                    kind: WantKind::Good(GOLD),
                    horizon: Horizon::Now,
                    qty: 1,
                    satisfied: false,
                },
                want(WantKind::Leisure, Horizon::Now),
            ],
        );

        assert_eq!(worker.reservation_labor_ask(1), None);
    }

    #[test]
    fn labor_book_matches_at_resting_wage() {
        let line = build_fish_trap_line();
        let project = start_project(
            AgentId(1),
            &line,
            M2ProjectId(1),
            Tick(0),
            Gold(9),
            Gold::ZERO,
        );
        let mut projects = vec![project];
        let mut agents = vec![
            agent(
                1,
                Gold(5),
                0,
                vec![want(WantKind::Good(GOLD), Horizon::Later(4))],
            ),
            agent(
                2,
                Gold::ZERO,
                1,
                vec![
                    want(WantKind::Good(GOLD), Horizon::Now),
                    want(WantKind::Leisure, Horizon::Now),
                ],
            ),
        ];
        let mut reservations = LaborReservations::new();
        let mut book = LaborBook::new();
        let lines = vec![line.clone()];
        let hire = LaborOrder {
            agent: AgentId(1),
            side: FactorSide::Hire,
            wage_limit: Gold(3),
            qty: 1,
            seq: 1,
            expires_tick: 3,
        };
        assert!(reservations.reserve_order(&agents, &hire));
        let mut market = LaborMarketView {
            agents: &mut agents,
            reservations: &mut reservations,
            projects: &mut projects,
            lines: &lines,
            money_system: None,
            wage_media: LaborWageTender::ParAll.accepted_media(),
            wage_audit: None,
            wage_tender: LaborWageTender::ParAll,
        };
        assert!(book
            .add_order(hire, Some(M2ProjectId(1)), 0, &mut market)
            .is_empty());
        let work = LaborOrder {
            agent: AgentId(2),
            side: FactorSide::Work,
            wage_limit: Gold(1),
            qty: 1,
            seq: 2,
            expires_tick: 3,
        };
        assert!(market.reservations.reserve_order(market.agents, &work));
        let trades = book.add_order(work, None, 0, &mut market);

        assert_eq!(trades.len(), 1);
        assert_eq!(trades[0].wage, Gold(3));
        assert_eq!(market.projects[0].labor_advanced, 1);
    }

    #[test]
    fn labor_book_rejects_stale_hire_project() {
        let line = build_fish_trap_line();
        let mut project = start_project(
            AgentId(1),
            &line,
            M2ProjectId(1),
            Tick(0),
            Gold(9),
            Gold::ZERO,
        );
        project.state = M2ProjectState::Abandoned;
        let mut projects = vec![project];
        let mut agents = vec![
            agent(
                1,
                Gold(5),
                0,
                vec![want(WantKind::Good(GOLD), Horizon::Later(4))],
            ),
            agent(
                2,
                Gold::ZERO,
                1,
                vec![
                    want(WantKind::Good(GOLD), Horizon::Now),
                    want(WantKind::Leisure, Horizon::Now),
                ],
            ),
        ];
        let mut reservations = LaborReservations::new();
        let mut book = LaborBook::new();
        let lines = vec![line.clone()];
        let hire = LaborOrder {
            agent: AgentId(1),
            side: FactorSide::Hire,
            wage_limit: Gold(3),
            qty: 1,
            seq: 1,
            expires_tick: 3,
        };
        assert!(reservations.reserve_order(&agents, &hire));
        let mut market = LaborMarketView {
            agents: &mut agents,
            reservations: &mut reservations,
            projects: &mut projects,
            lines: &lines,
            money_system: None,
            wage_media: LaborWageTender::ParAll.accepted_media(),
            wage_audit: None,
            wage_tender: LaborWageTender::ParAll,
        };
        assert!(book
            .add_order(hire, Some(M2ProjectId(1)), 0, &mut market)
            .is_empty());
        let work = LaborOrder {
            agent: AgentId(2),
            side: FactorSide::Work,
            wage_limit: Gold(1),
            qty: 1,
            seq: 2,
            expires_tick: 3,
        };
        assert!(market.reservations.reserve_order(market.agents, &work));
        let trades = book.add_order(work, None, 0, &mut market);

        assert!(trades.is_empty());
        assert_eq!(market.agents[0].gold, Gold(5));
        assert_eq!(market.agents[1].gold, Gold::ZERO);
        assert_eq!(market.projects[0].labor_advanced, 0);
    }

    #[test]
    fn labor_trade_advances_project_by_filled_qty() {
        let line = build_fish_trap_line();
        let project = start_project(
            AgentId(1),
            &line,
            M2ProjectId(1),
            Tick(0),
            Gold(9),
            Gold::ZERO,
        );
        let mut projects = vec![project];
        let mut agents = vec![
            agent(
                1,
                Gold(10),
                0,
                vec![want(WantKind::Good(GOLD), Horizon::Later(4))],
            ),
            agent(
                2,
                Gold::ZERO,
                2,
                vec![
                    want(WantKind::Good(GOLD), Horizon::Now),
                    want(WantKind::Leisure, Horizon::Now),
                ],
            ),
        ];
        let mut reservations = LaborReservations::new();
        let mut book = LaborBook::new();
        let lines = vec![line.clone()];
        let hire = LaborOrder {
            agent: AgentId(1),
            side: FactorSide::Hire,
            wage_limit: Gold(3),
            qty: 2,
            seq: 1,
            expires_tick: 3,
        };
        assert!(reservations.reserve_order(&agents, &hire));
        let mut market = LaborMarketView {
            agents: &mut agents,
            reservations: &mut reservations,
            projects: &mut projects,
            lines: &lines,
            money_system: None,
            wage_media: LaborWageTender::ParAll.accepted_media(),
            wage_audit: None,
            wage_tender: LaborWageTender::ParAll,
        };
        assert!(book
            .add_order(hire, Some(M2ProjectId(1)), 0, &mut market)
            .is_empty());
        let work = LaborOrder {
            agent: AgentId(2),
            side: FactorSide::Work,
            wage_limit: Gold(1),
            qty: 2,
            seq: 2,
            expires_tick: 3,
        };
        assert!(market.reservations.reserve_order(market.agents, &work));
        let trades = book.add_order(work, None, 0, &mut market);

        assert_eq!(trades.len(), 1);
        assert_eq!(trades[0].qty, 2);
        assert_eq!(market.projects[0].labor_advanced, 2);
        assert_eq!(market.projects[0].advanced_gold, Gold(6));
        assert_eq!(market.agents[0].gold, Gold(4));
        assert_eq!(market.agents[1].gold, Gold(6));
    }
}
