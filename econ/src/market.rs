//! Continuous double auction order books and reserve accounting.

use std::cmp::Reverse;
use std::collections::{BTreeMap, BTreeSet};

use crate::agent::{Agent, AgentId};
use crate::arena::AgentLookup;
use crate::good::{Gold, GoodId, Stock};
use crate::ledger::{MoneyComposition, MoneySystem};
use crate::money::AcceptedMedia;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum OrderSide {
    Bid,
    Ask,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Order {
    pub agent: AgentId,
    pub side: OrderSide,
    pub good: GoodId,
    pub limit: Gold,
    pub qty: u32,
    pub seq: u64,
    pub expires_tick: u64,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Trade {
    pub tick: u64,
    pub good: GoodId,
    pub buyer: AgentId,
    pub seller: AgentId,
    pub price: Gold,
    pub qty: u32,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ExecutedTrade {
    pub trade: Trade,
    pub payment: Option<MoneyComposition>,
}

/// One exact candidate crossing considered by the CDA. This is observation-only
/// metadata: ordinary order APIs do not request or allocate it, and matching
/// decisions never read it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct MatchAttempt {
    pub incoming_seq: u64,
    pub resting_seq: u64,
    pub incoming_side: OrderSide,
    pub good: GoodId,
    pub buyer: AgentId,
    pub seller: AgentId,
    pub qty: u32,
    pub bid_limit: Gold,
    pub ask_limit: Gold,
    pub status: MatchStatus,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum MatchStatus {
    Succeeded,
    Rejected,
}

pub struct OrderBook {
    pub good: GoodId,
    pub bids: BTreeMap<(Reverse<Gold>, u64), Order>,
    pub asks: BTreeMap<(Gold, u64), Order>,
    live_seqs: BTreeSet<u64>,
    pub tape: Vec<Trade>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Reservations {
    pub gold: Vec<Gold>,
    pub goods: Vec<Stock>,
    agent_ids: Vec<AgentId>,
    max_good_id: u16,
}

impl Reservations {
    pub fn new(agents: &[Agent], max_good_id: u16) -> Self {
        let len = agents.len();
        let mut agent_ids = agents.iter().map(|agent| agent.id).collect::<Vec<_>>();
        agent_ids.sort();
        Self {
            gold: vec![Gold::ZERO; len],
            goods: vec![Stock::new(max_good_id); len],
            agent_ids,
            max_good_id,
        }
    }

    pub fn free_gold(&self, agent: &Agent) -> Gold {
        agent.gold.saturating_sub(self.reserved_gold(agent.id))
    }

    pub fn free_stock(&self, agent: &Agent, good: GoodId) -> u32 {
        agent
            .stock
            .get(good)
            .saturating_sub(self.reserved_stock(agent.id, good))
    }

    pub fn reserved_gold(&self, agent: AgentId) -> Gold {
        self.index_of(agent)
            .and_then(|index| self.gold.get(index).copied())
            .unwrap_or(Gold::ZERO)
    }

    pub fn reserved_stock(&self, agent: AgentId, good: GoodId) -> u32 {
        self.index_of(agent)
            .and_then(|index| self.goods.get(index))
            .map(|stock| stock.get(good))
            .unwrap_or(0)
    }

    pub fn reserve_order<A: AgentLookup + ?Sized>(&mut self, agents: &A, order: &Order) -> bool {
        match order.side {
            OrderSide::Bid => {
                let Some(amount) = order.limit.mul_qty(order.qty) else {
                    return false;
                };
                let Some(agent) = agents.get_agent(order.agent) else {
                    return false;
                };
                if self.free_gold(agent) < amount {
                    return false;
                }
                let index = self.ensure_agent(order.agent);
                self.gold[index] = self.gold[index]
                    .checked_add(amount)
                    .expect("reserved gold cannot exceed agent gold");
                true
            }
            OrderSide::Ask => {
                let Some(agent) = agents.get_agent(order.agent) else {
                    return false;
                };
                if self.free_stock(agent, order.good) < order.qty {
                    return false;
                }
                let index = self.ensure_agent(order.agent);
                self.goods[index].add(order.good, order.qty);
                true
            }
        }
    }

    pub fn release_order(&mut self, order: &Order) {
        match order.side {
            OrderSide::Bid => {
                if let Some(amount) = order.limit.mul_qty(order.qty) {
                    self.release_gold(order.agent, amount);
                }
            }
            OrderSide::Ask => self.release_stock(order.agent, order.good, order.qty),
        }
    }

    fn release_filled(&mut self, order: &Order, qty: u32) {
        match order.side {
            OrderSide::Bid => {
                if let Some(amount) = order.limit.mul_qty(qty) {
                    self.release_gold(order.agent, amount);
                }
            }
            OrderSide::Ask => self.release_stock(order.agent, order.good, qty),
        }
    }

    fn release_gold(&mut self, agent: AgentId, amount: Gold) {
        let Some(index) = self.index_of(agent) else {
            return;
        };
        if let Some(reserved) = self.gold.get_mut(index) {
            *reserved = reserved.saturating_sub(amount);
        }
    }

    fn release_stock(&mut self, agent: AgentId, good: GoodId, qty: u32) {
        let Some(index) = self.index_of(agent) else {
            return;
        };
        if let Some(stock) = self.goods.get_mut(index) {
            stock.remove(good, qty);
        }
    }

    fn ensure_agent(&mut self, agent: AgentId) -> usize {
        match self.agent_ids.binary_search(&agent) {
            Ok(index) => index,
            Err(index) => {
                self.agent_ids.insert(index, agent);
                self.gold.insert(index, Gold::ZERO);
                self.goods.insert(index, Stock::new(self.max_good_id));
                index
            }
        }
    }

    /// Materialize a zero-reservation slot for `agent` (G4b birth) — the
    /// insert-side mirror of [`Reservations::forget_agent`]. The constructor
    /// pre-creates an (empty) slot for every agent in the initial cast, so a
    /// runtime-born agent must too, keeping the table's "every live agent has a
    /// reservation slot" invariant. Reserving nothing (`Gold::ZERO`, empty stock)
    /// is exactly a newborn's state, and lazy [`Reservations::reserve_order`] would
    /// create the same slot on the agent's first bid/ask — this just does it eagerly
    /// at birth so the cache is reconciled the instant the agent joins. Called only
    /// on a birth; no lab/no-birth path invokes it, so the goldens are byte-identical.
    pub fn ensure_agent_slot(&mut self, agent: AgentId) {
        self.ensure_agent(agent);
    }

    fn index_of(&self, agent: AgentId) -> Option<usize> {
        self.agent_ids.binary_search(&agent).ok()
    }

    /// Drop every reserved-amount slot for `agent` (G4a real death). A removed
    /// agent's resting orders are cancelled first, so its reserved gold/stock are
    /// already released to zero — but this id-keyed table still carries the empty
    /// slot in `agent_ids`. [`crate::society::Society::remove_agent`] calls this
    /// after freeing the arena slot so no reservation cache dangles a reference to
    /// a freed agent. Called only on death; no lab/no-free path invokes it, so the
    /// conformance goldens are byte-identical.
    pub fn forget_agent(&mut self, agent: AgentId) {
        if let Ok(index) = self.agent_ids.binary_search(&agent) {
            self.agent_ids.remove(index);
            self.gold.remove(index);
            self.goods.remove(index);
        }
    }
}

impl OrderBook {
    pub fn new(good: GoodId) -> Self {
        Self {
            good,
            bids: BTreeMap::new(),
            asks: BTreeMap::new(),
            live_seqs: BTreeSet::new(),
            tape: Vec::new(),
        }
    }

    pub fn add_order(
        &mut self,
        order: Order,
        tick: u64,
        agents: &mut [Agent],
        reservations: &mut Reservations,
    ) -> Vec<Trade> {
        self.add_order_inner(order, tick, agents, reservations, None, None)
            .into_iter()
            .map(|execution| execution.trade)
            .collect()
    }

    pub(crate) fn add_order_observed(
        &mut self,
        order: Order,
        tick: u64,
        agents: &mut [Agent],
        reservations: &mut Reservations,
    ) -> (Vec<Trade>, Vec<MatchAttempt>) {
        let mut attempts = Vec::new();
        let executions = self
            .add_order_inner(order, tick, agents, reservations, None, Some(&mut attempts))
            .into_iter()
            .map(|execution| execution.trade)
            .collect();
        (executions, attempts)
    }

    pub fn add_order_m3(
        &mut self,
        order: Order,
        tick: u64,
        agents: &mut [Agent],
        reservations: &mut Reservations,
        money_system: &mut MoneySystem,
        accepted_media: AcceptedMedia,
    ) -> Vec<ExecutedTrade> {
        self.add_order_inner(
            order,
            tick,
            agents,
            reservations,
            Some((money_system, accepted_media)),
            None,
        )
    }

    fn add_order_inner(
        &mut self,
        mut order: Order,
        tick: u64,
        agents: &mut [Agent],
        reservations: &mut Reservations,
        money_system: Option<(&mut MoneySystem, AcceptedMedia)>,
        attempts: Option<&mut Vec<MatchAttempt>>,
    ) -> Vec<ExecutedTrade> {
        if order.good != self.good || order.qty == 0 || order.expires_tick <= tick {
            reservations.release_order(&order);
            return Vec::new();
        }
        let money_settlement = money_system.map(|(system, accepted_media)| MoneySettlement {
            system,
            accepted_media,
        });
        let mut execution = MarketExecution {
            agents,
            reservations,
            money_settlement,
        };

        let executions = match order.side {
            OrderSide::Bid => self.match_bid(&mut order, tick, &mut execution, attempts),
            OrderSide::Ask => self.match_ask(&mut order, tick, &mut execution, attempts),
        };

        if order.qty > 0 {
            self.insert(order);
        }
        self.tape
            .extend(executions.iter().map(|execution| execution.trade.clone()));
        executions
    }

    pub fn purge_expired(&mut self, tick: u64, reservations: &mut Reservations) -> u32 {
        let mut expired = 0;
        let bid_keys = self
            .bids
            .iter()
            .filter(|(_, order)| order.expires_tick <= tick)
            .map(|(key, _)| *key)
            .collect::<Vec<_>>();
        for key in bid_keys {
            if let Some(order) = self.bids.remove(&key) {
                self.live_seqs.remove(&order.seq);
                reservations.release_order(&order);
                expired += 1;
            }
        }

        let ask_keys = self
            .asks
            .iter()
            .filter(|(_, order)| order.expires_tick <= tick)
            .map(|(key, _)| *key)
            .collect::<Vec<_>>();
        for key in ask_keys {
            if let Some(order) = self.asks.remove(&key) {
                self.live_seqs.remove(&order.seq);
                reservations.release_order(&order);
                expired += 1;
            }
        }

        expired
    }

    pub fn cancel(
        &mut self,
        agent: AgentId,
        side: OrderSide,
        good: GoodId,
        seq: u64,
        reservations: &mut Reservations,
    ) -> Option<Order> {
        if good != self.good {
            return None;
        }
        let order = match side {
            OrderSide::Bid => {
                let key = self
                    .bids
                    .iter()
                    .find(|(_, order)| order.agent == agent && order.seq == seq)
                    .map(|(key, _)| *key)?;
                self.bids.remove(&key)
            }
            OrderSide::Ask => {
                let key = self
                    .asks
                    .iter()
                    .find(|(_, order)| order.agent == agent && order.seq == seq)
                    .map(|(key, _)| *key)?;
                self.asks.remove(&key)
            }
        }?;
        self.live_seqs.remove(&order.seq);
        reservations.release_order(&order);
        Some(order)
    }

    pub fn contains_seq(&self, seq: u64) -> bool {
        self.live_seqs.contains(&seq)
    }

    pub fn live_order_counts(&self) -> (u32, u32) {
        (
            u32::try_from(self.bids.len()).unwrap_or(u32::MAX),
            u32::try_from(self.asks.len()).unwrap_or(u32::MAX),
        )
    }

    fn match_bid(
        &mut self,
        order: &mut Order,
        tick: u64,
        execution: &mut MarketExecution<'_>,
        mut attempts: Option<&mut Vec<MatchAttempt>>,
    ) -> Vec<ExecutedTrade> {
        let mut trades = Vec::new();
        let mut skipped_resting = BTreeSet::new();
        while order.qty > 0 {
            let mut selected = None;
            for (key, resting) in &self.asks {
                if skipped_resting.contains(&resting.seq) {
                    continue;
                }
                if order.limit < resting.limit {
                    break;
                }
                if order.agent == resting.agent {
                    continue;
                }
                selected = Some((*key, resting.clone()));
                break;
            }
            let Some((key, resting)) = selected else {
                break;
            };
            self.asks.remove(&key);
            self.live_seqs.remove(&resting.seq);
            let qty = order.qty.min(resting.qty);
            let price = resting.limit;
            let trade = Trade {
                tick,
                good: order.good,
                buyer: order.agent,
                seller: resting.agent,
                price,
                qty,
            };
            let payment = apply_trade(&trade, execution, order, &resting);
            if let Some(rows) = attempts.as_deref_mut() {
                rows.push(MatchAttempt {
                    incoming_seq: order.seq,
                    resting_seq: resting.seq,
                    incoming_side: OrderSide::Bid,
                    good: order.good,
                    buyer: order.agent,
                    seller: resting.agent,
                    qty,
                    bid_limit: order.limit,
                    ask_limit: resting.limit,
                    status: if payment.is_some() {
                        MatchStatus::Succeeded
                    } else {
                        MatchStatus::Rejected
                    },
                });
            }
            if let Some(payment) = payment {
                order.qty -= qty;
                let mut remainder = resting;
                remainder.qty -= qty;
                if remainder.qty > 0 {
                    self.insert(remainder);
                }
                trades.push(ExecutedTrade { trade, payment });
            } else {
                skipped_resting.insert(resting.seq);
                self.insert(resting);
            }
        }
        trades
    }

    fn match_ask(
        &mut self,
        order: &mut Order,
        tick: u64,
        execution: &mut MarketExecution<'_>,
        mut attempts: Option<&mut Vec<MatchAttempt>>,
    ) -> Vec<ExecutedTrade> {
        let mut trades = Vec::new();
        let mut skipped_resting = BTreeSet::new();
        while order.qty > 0 {
            let mut selected = None;
            for (key, resting) in &self.bids {
                if skipped_resting.contains(&resting.seq) {
                    continue;
                }
                if resting.limit < order.limit {
                    break;
                }
                if order.agent == resting.agent {
                    continue;
                }
                selected = Some((*key, resting.clone()));
                break;
            }
            let Some((key, resting)) = selected else {
                break;
            };
            self.bids.remove(&key);
            self.live_seqs.remove(&resting.seq);
            let qty = order.qty.min(resting.qty);
            let price = resting.limit;
            let trade = Trade {
                tick,
                good: order.good,
                buyer: resting.agent,
                seller: order.agent,
                price,
                qty,
            };
            let payment = apply_trade(&trade, execution, &resting, order);
            if let Some(rows) = attempts.as_deref_mut() {
                rows.push(MatchAttempt {
                    incoming_seq: order.seq,
                    resting_seq: resting.seq,
                    incoming_side: OrderSide::Ask,
                    good: order.good,
                    buyer: resting.agent,
                    seller: order.agent,
                    qty,
                    bid_limit: resting.limit,
                    ask_limit: order.limit,
                    status: if payment.is_some() {
                        MatchStatus::Succeeded
                    } else {
                        MatchStatus::Rejected
                    },
                });
            }
            if let Some(payment) = payment {
                order.qty -= qty;
                let mut remainder = resting;
                remainder.qty -= qty;
                if remainder.qty > 0 {
                    self.insert(remainder);
                }
                trades.push(ExecutedTrade { trade, payment });
            } else {
                skipped_resting.insert(resting.seq);
                self.insert(resting);
            }
        }
        trades
    }

    fn insert(&mut self, order: Order) {
        self.live_seqs.insert(order.seq);
        match order.side {
            OrderSide::Bid => {
                self.bids.insert((Reverse(order.limit), order.seq), order);
            }
            OrderSide::Ask => {
                self.asks.insert((order.limit, order.seq), order);
            }
        }
    }
}

struct MarketExecution<'a> {
    agents: &'a mut [Agent],
    reservations: &'a mut Reservations,
    money_settlement: Option<MoneySettlement<'a>>,
}

struct MoneySettlement<'a> {
    system: &'a mut MoneySystem,
    accepted_media: AcceptedMedia,
}

fn apply_trade(
    trade: &Trade,
    execution: &mut MarketExecution<'_>,
    bid_order: &Order,
    ask_order: &Order,
) -> Option<Option<MoneyComposition>> {
    if trade.buyer == trade.seller {
        return None;
    }
    let payment = trade.price.mul_qty(trade.qty)?;
    let buyer_pos = execution
        .agents
        .iter()
        .position(|agent| agent.id == trade.buyer)?;
    let seller_pos = execution
        .agents
        .iter()
        .position(|agent| agent.id == trade.seller)?;
    if buyer_pos == seller_pos {
        return None;
    }

    if execution.agents[buyer_pos].gold < payment
        || !execution.agents[seller_pos]
            .stock
            .can_remove(trade.good, trade.qty)
    {
        return None;
    }
    execution.agents[buyer_pos]
        .stock
        .get(trade.good)
        .checked_add(trade.qty)
        .map(|_| ())?;

    let payment_composition = if let Some(settlement) = execution.money_settlement.as_mut() {
        if settlement
            .system
            .accepted_spendable_total(trade.buyer, settlement.accepted_media)
            < payment
        {
            return None;
        }
        let composition = settlement
            .system
            .transfer_spendable_with_media(
                trade.buyer,
                trade.seller,
                payment,
                settlement.accepted_media,
            )
            .ok()?;
        settlement
            .system
            .reconcile_agent_cache_at(execution.agents, buyer_pos);
        settlement
            .system
            .reconcile_agent_cache_at(execution.agents, seller_pos);
        Some(composition)
    } else {
        let new_buyer_gold = execution.agents[buyer_pos]
            .gold
            .checked_sub(payment)
            .expect("payment was checked");
        let new_seller_gold = execution.agents[seller_pos].gold.checked_add(payment)?;
        execution.agents[buyer_pos].gold = new_buyer_gold;
        execution.agents[seller_pos].gold = new_seller_gold;
        None
    };

    execution.reservations.release_filled(bid_order, trade.qty);
    execution.reservations.release_filled(ask_order, trade.qty);
    let (buyer, seller) = two_agents_mut(execution.agents, buyer_pos, seller_pos);
    seller.stock.remove(trade.good, trade.qty);
    buyer.stock.add(trade.good, trade.qty);
    Some(payment_composition)
}

fn two_agents_mut(agents: &mut [Agent], a: usize, b: usize) -> (&mut Agent, &mut Agent) {
    if a < b {
        let (left, right) = agents.split_at_mut(b);
        (&mut left[a], &mut right[0])
    } else {
        let (left, right) = agents.split_at_mut(a);
        (&mut right[0], &mut left[b])
    }
}

#[cfg(test)]
mod tests {
    use super::{MatchStatus, Order, OrderBook, OrderSide, Reservations};
    use crate::agent::{Agent, AgentId, Role, Want, WantKind};
    use crate::good::{Gold, Horizon, Stock, FOOD};

    fn agent(id: u32, gold: Gold, food: u32) -> Agent {
        let mut stock = Stock::new(3);
        stock.add(FOOD, food);
        Agent {
            id: AgentId(u64::from(id)),
            scale: vec![Want {
                kind: WantKind::Good(FOOD),
                horizon: Horizon::Next,
                qty: 1,
                satisfied: false,
            }],
            stock,
            gold,
            labor_capacity: 0,
            hunger_deficit: 0,
            roles: vec![Role::Trader],
            expect: Vec::new(),
        }
    }

    fn order(agent: u32, side: OrderSide, limit: Gold, qty: u32, seq: u64) -> Order {
        Order {
            agent: AgentId(u64::from(agent)),
            side,
            good: FOOD,
            limit,
            qty,
            seq,
            expires_tick: 3,
        }
    }

    #[test]
    fn forget_agent_drops_only_its_reservation_slot() {
        // G4a real-death reconciliation: forgetting a removed agent drops exactly
        // its id-keyed slot from the three parallel reservation tables, leaving
        // every other agent's reserved amounts intact (the slot indices shift, but
        // the id-keyed lookups must still resolve).
        let agents = vec![
            agent(1, Gold(10), 5),
            agent(2, Gold(10), 5),
            agent(3, Gold(10), 5),
        ];
        let mut reservations = Reservations::new(&agents, 3);
        // Agent 1 and 3 reserve FOOD stock (asks); agent 2 reserves gold (a bid).
        assert!(reservations.reserve_order(&agents, &order(1, OrderSide::Ask, Gold(1), 2, 1)));
        assert!(reservations.reserve_order(&agents, &order(2, OrderSide::Bid, Gold(3), 1, 2)));
        assert!(reservations.reserve_order(&agents, &order(3, OrderSide::Ask, Gold(1), 4, 3)));
        assert_eq!(reservations.reserved_gold(AgentId(2)), Gold(3));
        assert_eq!(reservations.reserved_stock(AgentId(1), FOOD), 2);
        assert_eq!(reservations.reserved_stock(AgentId(3), FOOD), 4);

        reservations.forget_agent(AgentId(2));

        // The removed agent has no reservation; its neighbours are untouched.
        assert_eq!(reservations.reserved_gold(AgentId(2)), Gold::ZERO);
        assert_eq!(reservations.reserved_stock(AgentId(2), FOOD), 0);
        assert_eq!(reservations.reserved_stock(AgentId(1), FOOD), 2);
        assert_eq!(reservations.reserved_stock(AgentId(3), FOOD), 4);

        // Forgetting an unknown/already-removed id is a no-op.
        reservations.forget_agent(AgentId(2));
        reservations.forget_agent(AgentId(99));
        assert_eq!(reservations.reserved_stock(AgentId(1), FOOD), 2);
        assert_eq!(reservations.reserved_stock(AgentId(3), FOOD), 4);
    }

    #[test]
    fn cda_matches_at_resting_limit() {
        let mut agents = vec![agent(1, Gold(10), 0), agent(2, Gold(0), 2)];
        let mut reservations = Reservations::new(&agents, 3);
        let mut book = OrderBook::new(FOOD);
        let ask = order(2, OrderSide::Ask, Gold(5), 1, 1);
        assert!(reservations.reserve_order(&agents, &ask));
        assert!(book
            .add_order(ask, 0, &mut agents, &mut reservations)
            .is_empty());

        let bid = order(1, OrderSide::Bid, Gold(7), 1, 2);
        assert!(reservations.reserve_order(&agents, &bid));
        let trades = book.add_order(bid, 0, &mut agents, &mut reservations);

        assert_eq!(trades[0].price, Gold(5));

        let mut agents = vec![agent(1, Gold(10), 0), agent(2, Gold(0), 2)];
        let mut reservations = Reservations::new(&agents, 3);
        let mut book = OrderBook::new(FOOD);
        let bid = order(1, OrderSide::Bid, Gold(7), 1, 1);
        assert!(reservations.reserve_order(&agents, &bid));
        assert!(book
            .add_order(bid, 0, &mut agents, &mut reservations)
            .is_empty());

        let ask = order(2, OrderSide::Ask, Gold(5), 1, 2);
        assert!(reservations.reserve_order(&agents, &ask));
        let trades = book.add_order(ask, 0, &mut agents, &mut reservations);

        assert_eq!(trades[0].price, Gold(7));
    }

    #[test]
    fn cda_supports_partial_fill() {
        let mut agents = vec![agent(1, Gold(20), 0), agent(2, Gold(0), 3)];
        let mut reservations = Reservations::new(&agents, 3);
        let mut book = OrderBook::new(FOOD);
        let ask = order(2, OrderSide::Ask, Gold(3), 3, 1);
        assert!(reservations.reserve_order(&agents, &ask));
        assert!(book
            .add_order(ask, 0, &mut agents, &mut reservations)
            .is_empty());

        let bid = order(1, OrderSide::Bid, Gold(3), 2, 2);
        assert!(reservations.reserve_order(&agents, &bid));
        let trades = book.add_order(bid, 0, &mut agents, &mut reservations);

        assert_eq!(trades[0].qty, 2);
        assert_eq!(book.asks.values().next().unwrap().qty, 1);
    }

    #[test]
    fn cda_rejects_self_trade() {
        let mut agents = vec![agent(1, Gold(10), 2)];
        let mut reservations = Reservations::new(&agents, 3);
        let mut book = OrderBook::new(FOOD);
        let ask = order(1, OrderSide::Ask, Gold(3), 1, 1);
        assert!(reservations.reserve_order(&agents, &ask));
        book.add_order(ask, 0, &mut agents, &mut reservations);
        let bid = order(1, OrderSide::Bid, Gold(5), 1, 2);
        assert!(reservations.reserve_order(&agents, &bid));

        let trades = book.add_order(bid, 0, &mut agents, &mut reservations);

        assert!(trades.is_empty());
        assert_eq!(book.asks.len(), 1);
        assert_eq!(book.bids.len(), 1);
    }

    #[test]
    fn cda_self_order_does_not_block_other_counterparty() {
        let mut agents = vec![agent(1, Gold(10), 2), agent(2, Gold(0), 1)];
        let mut reservations = Reservations::new(&agents, 3);
        let mut book = OrderBook::new(FOOD);
        let own_ask = order(1, OrderSide::Ask, Gold(3), 1, 1);
        let other_ask = order(2, OrderSide::Ask, Gold(4), 1, 2);
        assert!(reservations.reserve_order(&agents, &own_ask));
        assert!(reservations.reserve_order(&agents, &other_ask));
        book.add_order(own_ask, 0, &mut agents, &mut reservations);
        book.add_order(other_ask, 0, &mut agents, &mut reservations);

        let bid = order(1, OrderSide::Bid, Gold(5), 1, 3);
        assert!(reservations.reserve_order(&agents, &bid));
        let trades = book.add_order(bid, 0, &mut agents, &mut reservations);

        assert_eq!(trades.len(), 1);
        assert_eq!(trades[0].seller, AgentId(2));
        assert_eq!(trades[0].price, Gold(4));
        assert_eq!(book.asks.len(), 1);
        assert_eq!(book.asks.values().next().unwrap().agent, AgentId(1));
        assert!(book.bids.is_empty());
    }

    #[test]
    fn failed_resting_ask_does_not_block_valid_ask() {
        let mut agents = vec![
            agent(1, Gold(3), 0),
            agent(2, Gold(u64::MAX), 1),
            agent(3, Gold(0), 1),
        ];
        let mut reservations = Reservations::new(&agents, 3);
        let mut book = OrderBook::new(FOOD);
        let broken_ask = order(2, OrderSide::Ask, Gold(1), 1, 1);
        let valid_ask = order(3, OrderSide::Ask, Gold(2), 1, 2);
        assert!(reservations.reserve_order(&agents, &broken_ask));
        assert!(reservations.reserve_order(&agents, &valid_ask));
        book.add_order(broken_ask, 0, &mut agents, &mut reservations);
        book.add_order(valid_ask, 0, &mut agents, &mut reservations);

        let bid = order(1, OrderSide::Bid, Gold(2), 1, 3);
        assert!(reservations.reserve_order(&agents, &bid));
        let (trades, attempts) = book.add_order_observed(bid, 0, &mut agents, &mut reservations);

        assert_eq!(trades.len(), 1);
        assert_eq!(trades[0].seller, AgentId(3));
        assert_eq!(trades[0].price, Gold(2));
        assert_eq!(agents[0].stock.get(FOOD), 1);
        assert_eq!(agents[2].stock.get(FOOD), 0);
        assert_eq!(book.asks.len(), 1);
        assert_eq!(book.asks.values().next().unwrap().agent, AgentId(2));
        assert_eq!(attempts.len(), 2);
        assert_eq!(attempts[0].incoming_seq, 3);
        assert_eq!(attempts[0].resting_seq, 1);
        assert_eq!(attempts[0].incoming_side, OrderSide::Bid);
        assert_eq!(attempts[0].bid_limit, Gold(2));
        assert_eq!(attempts[0].ask_limit, Gold(1));
        assert_eq!(attempts[0].status, MatchStatus::Rejected);
        assert_eq!(attempts[1].resting_seq, 2);
        assert_eq!(attempts[1].ask_limit, Gold(2));
        assert_eq!(attempts[1].status, MatchStatus::Succeeded);
    }

    #[test]
    fn failed_resting_bid_does_not_block_valid_bid() {
        let mut agents = vec![
            agent(1, Gold(5), u32::MAX),
            agent(2, Gold(5), 0),
            agent(3, Gold(0), 1),
        ];
        let mut reservations = Reservations::new(&agents, 3);
        let mut book = OrderBook::new(FOOD);
        let broken_bid = order(1, OrderSide::Bid, Gold(3), 1, 1);
        let valid_bid = order(2, OrderSide::Bid, Gold(2), 1, 2);
        assert!(reservations.reserve_order(&agents, &broken_bid));
        assert!(reservations.reserve_order(&agents, &valid_bid));
        book.add_order(broken_bid, 0, &mut agents, &mut reservations);
        book.add_order(valid_bid, 0, &mut agents, &mut reservations);

        let ask = order(3, OrderSide::Ask, Gold(1), 1, 3);
        assert!(reservations.reserve_order(&agents, &ask));
        let trades = book.add_order(ask, 0, &mut agents, &mut reservations);

        assert_eq!(trades.len(), 1);
        assert_eq!(trades[0].buyer, AgentId(2));
        assert_eq!(trades[0].price, Gold(2));
        assert_eq!(agents[1].stock.get(FOOD), 1);
        assert_eq!(agents[2].stock.get(FOOD), 0);
        assert_eq!(book.bids.len(), 1);
        assert_eq!(book.bids.values().next().unwrap().agent, AgentId(1));
    }

    #[test]
    fn trade_rejects_buyer_stock_overflow_without_destroying_goods() {
        let mut agents = vec![agent(1, Gold(1), u32::MAX), agent(2, Gold(0), 1)];
        let mut reservations = Reservations::new(&agents, 3);
        let mut book = OrderBook::new(FOOD);
        let ask = order(2, OrderSide::Ask, Gold(1), 1, 1);
        assert!(reservations.reserve_order(&agents, &ask));
        book.add_order(ask, 0, &mut agents, &mut reservations);

        let bid = order(1, OrderSide::Bid, Gold(1), 1, 2);
        assert!(reservations.reserve_order(&agents, &bid));
        let trades = book.add_order(bid, 0, &mut agents, &mut reservations);

        assert!(trades.is_empty());
        assert_eq!(agents[0].stock.get(FOOD), u32::MAX);
        assert_eq!(agents[1].stock.get(FOOD), 1);
        assert_eq!(reservations.reserved_gold(AgentId(1)), Gold(1));
        assert_eq!(reservations.reserved_stock(AgentId(2), FOOD), 1);
    }

    #[test]
    fn sparse_agent_ids_do_not_allocate_by_raw_id() {
        let agents = vec![agent(1_000_000, Gold(5), 0)];
        let reservations = Reservations::new(&agents, 3);

        assert_eq!(reservations.gold.len(), 1);
        assert_eq!(reservations.goods.len(), 1);
        assert_eq!(reservations.reserved_gold(AgentId(1_000_000)), Gold::ZERO);
    }

    #[test]
    fn expired_orders_release_reserves() {
        let mut agents = vec![agent(1, Gold(5), 0)];
        let mut reservations = Reservations::new(&agents, 3);
        let mut book = OrderBook::new(FOOD);
        let bid = order(1, OrderSide::Bid, Gold(5), 1, 1);
        assert!(reservations.reserve_order(&agents, &bid));
        book.add_order(bid, 0, &mut agents, &mut reservations);

        assert_eq!(reservations.reserved_gold(AgentId(1)), Gold(5));
        assert_eq!(book.purge_expired(3, &mut reservations), 1);
        assert_eq!(reservations.reserved_gold(AgentId(1)), Gold::ZERO);
    }

    #[test]
    fn agent_cannot_double_spend_reserved_gold() {
        let agents = vec![agent(1, Gold(5), 0)];
        let mut reservations = Reservations::new(&agents, 3);
        let first = order(1, OrderSide::Bid, Gold(4), 1, 1);
        let second = order(1, OrderSide::Bid, Gold(4), 1, 2);

        assert!(reservations.reserve_order(&agents, &first));
        assert!(!reservations.reserve_order(&agents, &second));
    }

    #[test]
    fn agent_cannot_sell_reserved_good() {
        let agents = vec![agent(1, Gold(0), 1)];
        let mut reservations = Reservations::new(&agents, 3);
        let first = order(1, OrderSide::Ask, Gold(1), 1, 1);
        let second = order(1, OrderSide::Ask, Gold(1), 1, 2);

        assert!(reservations.reserve_order(&agents, &first));
        assert!(!reservations.reserve_order(&agents, &second));
    }
}
