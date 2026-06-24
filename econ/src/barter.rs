//! Goods-for-goods barter offers and reciprocal clearing.

use std::collections::BTreeMap;

use crate::agent::{Agent, AgentId};
use crate::good::{GoodId, Stock};
use crate::marketability::{MarketabilityAcceptance, MarketabilityConfig};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BarterReason {
    DirectWant,
    /// Instrumental receipt for a final target want.
    ///
    /// The book rechecks the current provisional saleability leader when this
    /// variant is posted and when matches are cleared.
    IndirectFor {
        target: GoodId,
    },
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BarterOffer {
    pub agent: AgentId,
    pub give_good: GoodId,
    pub receive_good: GoodId,
    pub qty: u32,
    pub reason: BarterReason,
    pub seq: u64,
    pub expires_tick: u64,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BarterTrade {
    pub tick: u64,
    pub a: AgentId,
    pub b: AgentId,
    pub a_gives: GoodId,
    pub b_gives: GoodId,
    pub qty: u32,
    pub a_reason: BarterReason,
    pub b_reason: BarterReason,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct BarterReservation {
    agent: AgentId,
    good: GoodId,
    qty: u32,
    seq: u64,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct BarterBook {
    offers: Vec<BarterOffer>,
    reservations: Vec<BarterReservation>,
}

impl BarterBook {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn live_offers(&self) -> &[BarterOffer] {
        &self.offers
    }

    pub fn reserved_qty(&self, agent: AgentId, good: GoodId) -> u32 {
        self.reservations
            .iter()
            .filter(|reservation| reservation.agent == agent && reservation.good == good)
            .map(|reservation| reservation.qty)
            .fold(0u32, u32::saturating_add)
    }

    pub fn reserved_stock_for(&self, agent: AgentId) -> Vec<(GoodId, u32)> {
        let mut reserved: Vec<(GoodId, u32)> = Vec::new();
        for reservation in self
            .reservations
            .iter()
            .filter(|reservation| reservation.agent == agent)
        {
            if let Some((_, qty)) = reserved
                .iter_mut()
                .find(|(good, _)| *good == reservation.good)
            {
                *qty = (*qty).saturating_add(reservation.qty);
            } else {
                reserved.push((reservation.good, reservation.qty));
            }
        }
        reserved
    }

    pub fn post_offer(&mut self, agents: &[Agent], offer: BarterOffer, tick: u64) -> bool {
        self.post_offer_with_provisional_leader(agents, offer, tick, None)
    }

    pub fn post_offer_with_provisional_leader(
        &mut self,
        agents: &[Agent],
        offer: BarterOffer,
        tick: u64,
        provisional_leader: Option<GoodId>,
    ) -> bool {
        if offer.qty == 0
            || offer.expires_tick <= tick
            || offer.give_good == offer.receive_good
            || self.offers.iter().any(|live| live.seq == offer.seq)
            || !offer_has_valid_saleability_context(&offer, provisional_leader)
        {
            return false;
        }
        let Some(agent) = agents.iter().find(|agent| agent.id == offer.agent) else {
            return false;
        };
        let available_stock = self.unreserved_stock(agent);
        if !available_stock.can_remove(offer.give_good, offer.qty) {
            return false;
        }
        if !agent_accepts_offer_with_stock(agent, &available_stock, &offer, offer.qty) {
            return false;
        }

        self.reservations.push(BarterReservation {
            agent: offer.agent,
            good: offer.give_good,
            qty: offer.qty,
            seq: offer.seq,
        });
        self.offers.push(offer);
        self.offers.sort_by_key(|offer| offer.seq);
        true
    }

    pub fn cancel_offer(&mut self, seq: u64) -> bool {
        let Some(index) = self.offers.iter().position(|offer| offer.seq == seq) else {
            return false;
        };
        self.remove_offer_at(index);
        true
    }

    pub fn expire_offers(&mut self, tick: u64) -> u32 {
        let expired: Vec<u64> = self
            .offers
            .iter()
            .filter(|offer| offer.expires_tick <= tick)
            .map(|offer| offer.seq)
            .collect();
        let count = u32::try_from(expired.len()).unwrap_or(u32::MAX);
        for seq in expired {
            self.cancel_offer(seq);
        }
        count
    }

    pub fn cancel_invalid(&mut self, agents: &[Agent], provisional_leader: Option<GoodId>) {
        self.cancel_invalid_offers(agents, provisional_leader);
    }

    /// Drop every live offer and reservation for a removed agent (G4a real death).
    /// `Society::remove_agent` calls this before/alongside arena reconciliation so
    /// a dead agent has no visible barter order even before the next clearing pass.
    pub fn forget_agent(&mut self, agent: AgentId) {
        self.offers.retain(|offer| offer.agent != agent);
        self.reservations
            .retain(|reservation| reservation.agent != agent);
    }

    pub fn clear_matches(&mut self, agents: &mut [Agent], tick: u64) -> Vec<BarterTrade> {
        self.clear_matches_with_provisional_leader(agents, tick, None)
    }

    pub fn clear_matches_with_provisional_leader(
        &mut self,
        agents: &mut [Agent],
        tick: u64,
        provisional_leader: Option<GoodId>,
    ) -> Vec<BarterTrade> {
        self.expire_offers(tick);
        self.cancel_invalid_offers(agents, provisional_leader);
        let mut trades = Vec::new();
        let mut skipped_pairs = Vec::new();
        let mut needs_final_revalidation = false;

        while let Some((a_index, b_index)) = self.next_match_indices(&skipped_pairs) {
            let a_offer = self.offers[a_index].clone();
            let b_offer = self.offers[b_index].clone();
            let qty = a_offer.qty.min(b_offer.qty);
            if qty == 0 {
                self.cancel_zero_qty_offer(a_offer.seq);
                self.cancel_zero_qty_offer(b_offer.seq);
                continue;
            }

            let a_live = offer_still_valid(agents, &a_offer, provisional_leader);
            let b_live = offer_still_valid(agents, &b_offer, provisional_leader);
            if !a_live || !b_live {
                if !a_live {
                    self.cancel_offer(a_offer.seq);
                }
                if !b_live {
                    self.cancel_offer(b_offer.seq);
                }
                skipped_pairs.clear();
                continue;
            }

            if !can_apply_swap(agents, &a_offer, &b_offer, qty, provisional_leader) {
                skipped_pairs.push(pair_key(a_offer.seq, b_offer.seq));
                continue;
            }

            let applied = apply_swap(agents, &a_offer, &b_offer, qty, provisional_leader);
            debug_assert!(applied);
            if !applied {
                skipped_pairs.push(pair_key(a_offer.seq, b_offer.seq));
                continue;
            }
            trades.push(BarterTrade {
                tick,
                a: a_offer.agent,
                b: b_offer.agent,
                a_gives: a_offer.give_good,
                b_gives: b_offer.give_good,
                qty,
                a_reason: a_offer.reason,
                b_reason: b_offer.reason,
            });
            self.fill_offer(a_offer.seq, qty);
            self.fill_offer(b_offer.seq, qty);
            needs_final_revalidation = true;
            skipped_pairs.clear();
        }

        if needs_final_revalidation {
            self.cancel_invalid_offers(agents, provisional_leader);
        }

        trades
    }

    fn unreserved_stock(&self, agent: &Agent) -> Stock {
        stock_with_reservations_removed(
            agent,
            self.reservations
                .iter()
                .copied()
                .filter(|reservation| reservation.agent == agent.id),
        )
    }

    fn next_match_indices(&self, skipped_pairs: &[(u64, u64)]) -> Option<(usize, usize)> {
        let mut by_pair: BTreeMap<(GoodId, GoodId), Vec<usize>> = BTreeMap::new();
        for (a_index, a) in self.offers.iter().enumerate() {
            by_pair
                .entry((a.give_good, a.receive_good))
                .or_default()
                .push(a_index);
        }

        for (a_index, a) in self.offers.iter().enumerate() {
            let Some(candidates) = by_pair.get(&(a.receive_good, a.give_good)) else {
                continue;
            };
            for b_index in candidates {
                let b_index = *b_index;
                let b = &self.offers[b_index];
                if a_index == b_index
                    || a.agent == b.agent
                    || skipped_pairs.contains(&pair_key(a.seq, b.seq))
                {
                    continue;
                }
                return Some((a_index, b_index));
            }
        }
        None
    }

    fn remove_offer_at(&mut self, index: usize) -> BarterOffer {
        let offer = self.offers.remove(index);
        self.reservations
            .retain(|reservation| reservation.seq != offer.seq);
        offer
    }

    fn cancel_zero_qty_offer(&mut self, seq: u64) {
        if self
            .offers
            .iter()
            .any(|offer| offer.seq == seq && offer.qty == 0)
        {
            self.cancel_offer(seq);
        }
    }

    fn cancel_invalid_offers(&mut self, agents: &[Agent], provisional_leader: Option<GoodId>) {
        let agents_by_id = agents
            .iter()
            .map(|agent| (agent.id, agent))
            .collect::<BTreeMap<_, _>>();
        let mut kept_offers = Vec::new();
        let mut kept_reservations = Vec::new();

        for offer in &self.offers {
            let Some(agent) = agents_by_id.get(&offer.agent).copied() else {
                continue;
            };
            let available_stock =
                stock_with_reservations_removed(agent, kept_reservations.iter().copied());
            if offer_still_valid_for_stock(agent, &available_stock, offer, provisional_leader) {
                kept_reservations.push(BarterReservation {
                    agent: offer.agent,
                    good: offer.give_good,
                    qty: offer.qty,
                    seq: offer.seq,
                });
                kept_offers.push(offer.clone());
            }
        }

        self.offers = kept_offers;
        self.reservations = kept_reservations;
    }

    fn fill_offer(&mut self, seq: u64, qty: u32) {
        let Some(index) = self.offers.iter().position(|offer| offer.seq == seq) else {
            return;
        };
        self.release_reservation(seq, qty);
        let remaining = self.offers[index].qty.saturating_sub(qty);
        if remaining == 0 {
            self.offers.remove(index);
            self.reservations
                .retain(|reservation| reservation.seq != seq);
        } else {
            self.offers[index].qty = remaining;
        }
    }

    fn release_reservation(&mut self, seq: u64, qty: u32) {
        let Some(index) = self
            .reservations
            .iter()
            .position(|reservation| reservation.seq == seq)
        else {
            return;
        };
        let remaining = self.reservations[index].qty.saturating_sub(qty);
        if remaining == 0 {
            self.reservations.remove(index);
        } else {
            self.reservations[index].qty = remaining;
        }
    }
}

fn offer_has_valid_saleability_context(
    offer: &BarterOffer,
    provisional_leader: Option<GoodId>,
) -> bool {
    match offer.reason {
        BarterReason::DirectWant => true,
        BarterReason::IndirectFor { target } => {
            provisional_leader == Some(offer.receive_good) && target != offer.receive_good
        }
    }
}

fn pair_key(a: u64, b: u64) -> (u64, u64) {
    if a <= b {
        (a, b)
    } else {
        (b, a)
    }
}

fn stock_with_reservations_removed<I>(agent: &Agent, reservations: I) -> Stock
where
    I: IntoIterator<Item = BarterReservation>,
{
    let mut available = agent.stock.clone();
    for reservation in reservations {
        if reservation.agent != agent.id {
            continue;
        }
        if !available.remove(reservation.good, reservation.qty) {
            let held = available.get(reservation.good);
            available.remove(reservation.good, held);
        }
    }
    available
}

fn offer_still_valid(
    agents: &[Agent],
    offer: &BarterOffer,
    provisional_leader: Option<GoodId>,
) -> bool {
    if offer.qty == 0 || offer.give_good == offer.receive_good {
        return false;
    }
    if !offer_has_valid_saleability_context(offer, provisional_leader) {
        return false;
    }
    let Some(agent) = agents.iter().find(|agent| agent.id == offer.agent) else {
        return false;
    };
    offer_still_valid_for_stock(agent, &agent.stock, offer, provisional_leader)
}

fn offer_still_valid_for_stock(
    agent: &Agent,
    stock: &Stock,
    offer: &BarterOffer,
    provisional_leader: Option<GoodId>,
) -> bool {
    if offer.qty == 0 || offer.give_good == offer.receive_good {
        return false;
    }
    if !offer_has_valid_saleability_context(offer, provisional_leader) {
        return false;
    }
    stock.can_remove(offer.give_good, offer.qty)
        && stock
            .get(offer.receive_good)
            .checked_add(offer.qty)
            .is_some()
        && agent_accepts_offer_with_stock(agent, stock, offer, offer.qty)
}

fn can_apply_swap(
    agents: &[Agent],
    a: &BarterOffer,
    b: &BarterOffer,
    qty: u32,
    provisional_leader: Option<GoodId>,
) -> bool {
    if a.agent == b.agent || qty == 0 {
        return false;
    }
    if !offer_has_valid_saleability_context(a, provisional_leader)
        || !offer_has_valid_saleability_context(b, provisional_leader)
    {
        return false;
    }
    let Some(a_pos) = agents.iter().position(|agent| agent.id == a.agent) else {
        return false;
    };
    let Some(b_pos) = agents.iter().position(|agent| agent.id == b.agent) else {
        return false;
    };
    if a_pos == b_pos {
        return false;
    }
    agents[a_pos].stock.can_remove(a.give_good, qty)
        && agents[b_pos].stock.can_remove(b.give_good, qty)
        && agents[a_pos]
            .stock
            .get(b.give_good)
            .checked_add(qty)
            .is_some()
        && agents[b_pos]
            .stock
            .get(a.give_good)
            .checked_add(qty)
            .is_some()
        && agent_accepts_offer_with_stock(&agents[a_pos], &agents[a_pos].stock, a, qty)
        && agent_accepts_offer_with_stock(&agents[b_pos], &agents[b_pos].stock, b, qty)
}

fn apply_swap(
    agents: &mut [Agent],
    a: &BarterOffer,
    b: &BarterOffer,
    qty: u32,
    provisional_leader: Option<GoodId>,
) -> bool {
    if !can_apply_swap(agents, a, b, qty, provisional_leader) {
        return false;
    }
    let Some(a_pos) = agents.iter().position(|agent| agent.id == a.agent) else {
        return false;
    };
    let Some(b_pos) = agents.iter().position(|agent| agent.id == b.agent) else {
        return false;
    };

    let (a_agent, b_agent) = two_agents_mut(agents, a_pos, b_pos);
    if !a_agent.stock.can_remove(a.give_good, qty) || !b_agent.stock.can_remove(b.give_good, qty) {
        return false;
    }
    let a_removed = a_agent.stock.remove(a.give_good, qty);
    let b_removed = b_agent.stock.remove(b.give_good, qty);
    if !a_removed || !b_removed {
        if a_removed {
            a_agent.stock.add(a.give_good, qty);
        }
        if b_removed {
            b_agent.stock.add(b.give_good, qty);
        }
        return false;
    }
    a_agent.stock.add(b.give_good, qty);
    b_agent.stock.add(a.give_good, qty);
    true
}

fn agent_accepts_offer_with_stock(
    agent: &Agent,
    stock: &Stock,
    offer: &BarterOffer,
    qty: u32,
) -> bool {
    match offer.reason {
        BarterReason::DirectWant => agent.would_accept_barter_swap_with_stock(
            stock,
            offer.give_good,
            offer.receive_good,
            qty,
        ),
        BarterReason::IndirectFor { target } => {
            // The durability holding rule is enforced once, at offer-posting time
            // (`society.rs`), and that is sufficient here: the gate is a pure
            // function of the offer's fixed fields (`receive_good`, which an
            // `IndirectFor` offer pins to the durable saleability leader, and the
            // fixed `qty`), so re-applying it at match / re-validation cannot
            // change the verdict. A posted indirect offer already passed it, and a
            // stale offer whose `receive_good` no longer equals the leader is
            // dropped earlier by the saleability-context check. So matching is
            // marketability-blind by design — the config is left at its inert
            // default rather than threaded through every book method.
            let config = MarketabilityConfig::default();
            agent.would_accept_indirect_barter_swap_with_stock(
                stock,
                offer.give_good,
                offer.receive_good,
                target,
                qty,
                MarketabilityAcceptance {
                    durability_aware_acceptance: false,
                    config: &config,
                },
            )
        }
    }
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
    use super::{BarterBook, BarterOffer, BarterReason};
    use crate::agent::{Agent, AgentId, Role, Want, WantKind};
    use crate::good::{Gold, GoodId, Horizon, Stock, CLOTH, FOOD, SALT, WOOD};

    #[test]
    fn barter_trade_swaps_only_stock_goods() {
        let mut agents = reciprocal_agents();
        let initial_gold = (agents[0].gold, agents[1].gold);
        let mut book = BarterBook::new();

        assert!(book.post_offer(&agents, offer(1, FOOD, WOOD, 1), 0));
        assert!(book.post_offer(&agents, offer(2, WOOD, FOOD, 2), 0));
        let trades = book.clear_matches(&mut agents, 1);

        assert_eq!(trades.len(), 1);
        assert_eq!(agents[0].stock.get(FOOD), 0);
        assert_eq!(agents[0].stock.get(WOOD), 1);
        assert_eq!(agents[1].stock.get(FOOD), 1);
        assert_eq!(agents[1].stock.get(WOOD), 0);
        assert_eq!((agents[0].gold, agents[1].gold), initial_gold);
    }

    #[test]
    fn barter_trade_preserves_goods_conservation() {
        let mut agents = reciprocal_agents();
        let before_food = total_stock(&agents, FOOD);
        let before_wood = total_stock(&agents, WOOD);
        let mut book = BarterBook::new();

        assert!(book.post_offer(&agents, offer(1, FOOD, WOOD, 1), 0));
        assert!(book.post_offer(&agents, offer(2, WOOD, FOOD, 2), 0));
        book.clear_matches(&mut agents, 1);

        assert_eq!(total_stock(&agents, FOOD), before_food);
        assert_eq!(total_stock(&agents, WOOD), before_wood);
    }

    #[test]
    fn barter_rejects_self_trade() {
        let mut agents = vec![agent(
            1,
            &[(FOOD, 1), (WOOD, 1)],
            &[(WOOD, Horizon::Now, 2), (FOOD, Horizon::Now, 2)],
        )];
        let mut book = BarterBook {
            offers: vec![offer(1, FOOD, WOOD, 1), offer(1, WOOD, FOOD, 2)],
            reservations: Vec::new(),
        };

        let trades = book.clear_matches(&mut agents, 1);

        assert!(trades.is_empty());
        assert_eq!(agents[0].stock.get(FOOD), 1);
        assert_eq!(agents[0].stock.get(WOOD), 1);
    }

    #[test]
    fn barter_reservations_prevent_double_spend() {
        let agents = vec![agent(
            1,
            &[(FOOD, 1)],
            &[(WOOD, Horizon::Now, 1), (CLOTH, Horizon::Next, 1)],
        )];
        let mut book = BarterBook::new();

        assert!(book.post_offer(&agents, offer(1, FOOD, WOOD, 1), 0));
        assert!(!book.post_offer(&agents, offer(1, FOOD, CLOTH, 2), 0));
        assert_eq!(book.reserved_qty(AgentId(1), FOOD), 1);
    }

    #[test]
    fn forget_agent_cancels_live_offers_and_reservations() {
        let agents = vec![
            agent(1, &[(FOOD, 1)], &[(WOOD, Horizon::Now, 1)]),
            agent(2, &[(WOOD, 1)], &[(FOOD, Horizon::Now, 1)]),
        ];
        let mut book = BarterBook::new();

        assert!(book.post_offer(&agents, offer(1, FOOD, WOOD, 1), 0));
        assert!(book.post_offer(&agents, offer(2, WOOD, FOOD, 2), 0));
        assert_eq!(book.reserved_qty(AgentId(1), FOOD), 1);
        assert_eq!(book.reserved_qty(AgentId(2), WOOD), 1);

        book.forget_agent(AgentId(1));

        assert!(book
            .live_offers()
            .iter()
            .all(|offer| offer.agent != AgentId(1)));
        assert_eq!(book.reserved_qty(AgentId(1), FOOD), 0);
        assert_eq!(book.reserved_qty(AgentId(2), WOOD), 1);
        assert_eq!(book.live_offers().len(), 1);
    }

    #[test]
    fn barter_offer_acceptance_uses_unreserved_holdings() {
        let agents = vec![agent(
            1,
            &[(FOOD, 2)],
            &[
                (FOOD, Horizon::Next, 1),
                (WOOD, Horizon::Next, 1),
                (CLOTH, Horizon::Next, 1),
            ],
        )];
        let mut book = BarterBook::new();

        assert!(book.post_offer(&agents, offer(1, FOOD, WOOD, 1), 0));
        assert!(!book.post_offer(&agents, offer(1, FOOD, CLOTH, 2), 0));
        assert_eq!(book.reserved_qty(AgentId(1), FOOD), 1);
    }

    #[test]
    fn barter_expiration_releases_reserved_stock() {
        let agents = vec![agent(1, &[(FOOD, 1)], &[(WOOD, Horizon::Now, 1)])];
        let mut book = BarterBook::new();

        assert!(book.post_offer(&agents, expiring_offer(1, FOOD, WOOD, 1, 2), 0));
        assert_eq!(book.reserved_qty(AgentId(1), FOOD), 1);
        assert_eq!(book.expire_offers(2), 1);
        assert_eq!(book.reserved_qty(AgentId(1), FOOD), 0);
        assert!(book.post_offer(&agents, offer(1, FOOD, WOOD, 2), 2));
    }

    #[test]
    fn barter_offer_requires_ordinal_improvement() {
        let agents = vec![agent(1, &[(FOOD, 1)], &[(SALT, Horizon::Later(1), 1)])];
        let mut book = BarterBook::new();

        assert!(!book.post_offer(&agents, offer(1, FOOD, WOOD, 1), 0));
        assert_eq!(book.reserved_qty(AgentId(1), FOOD), 0);
    }

    #[test]
    fn barter_offer_preserves_higher_ranked_wants() {
        let agents = vec![agent(
            1,
            &[(FOOD, 1)],
            &[(FOOD, Horizon::Next, 1), (WOOD, Horizon::Next, 1)],
        )];
        let mut book = BarterBook::new();

        assert!(!book.post_offer(&agents, offer(1, FOOD, WOOD, 1), 0));
        assert_eq!(book.reserved_qty(AgentId(1), FOOD), 0);
    }

    #[test]
    fn barter_offer_preserves_higher_ranked_partial_wants() {
        let agents = vec![agent(
            1,
            &[(FOOD, 1)],
            &[(FOOD, Horizon::Next, 2), (WOOD, Horizon::Next, 1)],
        )];
        let mut book = BarterBook::new();

        assert!(!book.post_offer(&agents, offer(1, FOOD, WOOD, 1), 0));
        assert_eq!(book.reserved_qty(AgentId(1), FOOD), 0);
    }

    #[test]
    fn barter_invalid_partial_fill_keeps_still_valid_offers() {
        let mut agents = vec![
            agent(1, &[(FOOD, 2)], &[(WOOD, Horizon::Now, 2)]),
            agent(2, &[(WOOD, 1)], &[(FOOD, Horizon::Now, 1)]),
        ];
        let mut first = offer(1, FOOD, WOOD, 1);
        first.qty = 2;
        let mut book = BarterBook::new();

        assert!(book.post_offer(&agents, first, 0));
        assert!(book.post_offer(&agents, offer(2, WOOD, FOOD, 2), 0));
        let trades = book.clear_matches(&mut agents, 1);

        assert!(trades.is_empty());
        assert_eq!(book.live_offers().len(), 2);
        assert_eq!(book.reserved_qty(AgentId(1), FOOD), 2);
        assert_eq!(book.reserved_qty(AgentId(2), WOOD), 1);
        assert_eq!(agents[0].stock.get(FOOD), 2);
        assert_eq!(agents[1].stock.get(WOOD), 1);
    }

    #[test]
    fn barter_partial_fill_cancels_no_longer_valid_residual() {
        let mut agents = vec![
            agent(1, &[(FOOD, 2)], &[(WOOD, Horizon::Now, 1)]),
            agent(2, &[(WOOD, 1)], &[(FOOD, Horizon::Now, 1)]),
        ];
        let mut first = offer(1, FOOD, WOOD, 1);
        first.qty = 2;
        let mut book = BarterBook::new();

        assert!(book.post_offer(&agents, first, 0));
        assert!(book.post_offer(&agents, offer(2, WOOD, FOOD, 2), 0));
        let trades = book.clear_matches(&mut agents, 1);

        assert_eq!(trades.len(), 1);
        assert!(book.live_offers().is_empty());
        assert_eq!(book.reserved_qty(AgentId(1), FOOD), 0);
        assert_eq!(book.reserved_qty(AgentId(2), WOOD), 0);
        assert_eq!(agents[0].stock.get(FOOD), 1);
        assert_eq!(agents[0].stock.get(WOOD), 1);
        assert_eq!(agents[1].stock.get(FOOD), 1);
        assert_eq!(agents[1].stock.get(WOOD), 0);
    }

    #[test]
    fn barter_fill_revalidates_other_live_offers_for_affected_agents() {
        let mut agents = vec![
            agent(1, &[(FOOD, 2)], &[(WOOD, Horizon::Now, 1)]),
            agent(2, &[(WOOD, 1)], &[(FOOD, Horizon::Now, 1)]),
        ];
        let mut book = BarterBook::new();

        assert!(book.post_offer(&agents, offer(1, FOOD, WOOD, 1), 0));
        assert!(book.post_offer(&agents, offer(1, FOOD, WOOD, 2), 0));
        assert_eq!(book.reserved_qty(AgentId(1), FOOD), 2);
        assert!(book.post_offer(&agents, offer(2, WOOD, FOOD, 3), 0));

        let trades = book.clear_matches(&mut agents, 1);

        assert_eq!(trades.len(), 1);
        assert!(book.live_offers().is_empty());
        assert_eq!(book.reserved_qty(AgentId(1), FOOD), 0);
        assert_eq!(agents[0].stock.get(FOOD), 1);
        assert_eq!(agents[0].stock.get(WOOD), 1);
    }

    #[test]
    fn barter_clear_purges_unmatched_invalid_offer_reservation() {
        let mut agents = vec![agent(1, &[(FOOD, 1)], &[(WOOD, Horizon::Now, 1)])];
        let mut book = BarterBook::new();

        assert!(book.post_offer(&agents, offer(1, FOOD, WOOD, 1), 0));
        assert_eq!(book.reserved_qty(AgentId(1), FOOD), 1);
        assert!(agents[0].stock.remove(FOOD, 1));

        let trades = book.clear_matches(&mut agents, 1);

        assert!(trades.is_empty());
        assert!(book.live_offers().is_empty());
        assert_eq!(book.reserved_qty(AgentId(1), FOOD), 0);

        agents[0].stock.add(FOOD, 1);
        assert!(book.post_offer(&agents, offer(1, FOOD, WOOD, 2), 1));
        assert_eq!(book.reserved_qty(AgentId(1), FOOD), 1);
    }

    #[test]
    fn barter_clear_cancels_cumulatively_unbacked_reservations() {
        let mut agents = vec![agent(
            1,
            &[(FOOD, 2)],
            &[(WOOD, Horizon::Now, 1), (CLOTH, Horizon::Next, 1)],
        )];
        let mut book = BarterBook::new();

        assert!(book.post_offer(&agents, offer(1, FOOD, WOOD, 1), 0));
        assert!(book.post_offer(&agents, offer(1, FOOD, CLOTH, 2), 0));
        assert_eq!(book.reserved_qty(AgentId(1), FOOD), 2);
        assert!(agents[0].stock.remove(FOOD, 1));

        let trades = book.clear_matches(&mut agents, 1);

        assert!(trades.is_empty());
        assert_eq!(book.live_offers().len(), 1);
        assert_eq!(book.live_offers()[0].seq, 1);
        assert_eq!(book.reserved_qty(AgentId(1), FOOD), 1);
        assert_eq!(agents[0].stock.get(FOOD), 1);
    }

    #[test]
    fn indirect_offer_requires_provisional_saleability() {
        let agents = vec![agent(1, &[(FOOD, 1)], &[(CLOTH, Horizon::Now, 1)])];

        let mut book = BarterBook::new();
        assert!(!book.post_offer(&agents, indirect_offer(1, FOOD, SALT, CLOTH, 1), 0));
        assert_eq!(book.reserved_qty(AgentId(1), FOOD), 0);

        let mut book = BarterBook::new();
        assert!(!book.post_offer_with_provisional_leader(
            &agents,
            indirect_offer(1, FOOD, SALT, CLOTH, 1),
            0,
            Some(WOOD),
        ));
        assert_eq!(book.reserved_qty(AgentId(1), FOOD), 0);

        let mut book = BarterBook::new();
        assert!(book.post_offer_with_provisional_leader(
            &agents,
            indirect_offer(1, FOOD, SALT, CLOTH, 1),
            0,
            Some(SALT),
        ));
        assert_eq!(book.reserved_qty(AgentId(1), FOOD), 1);
    }

    #[test]
    fn indirect_offer_can_receive_saleable_good_for_final_want() {
        let mut agents = vec![
            agent(1, &[(FOOD, 1)], &[(CLOTH, Horizon::Now, 1)]),
            agent(2, &[(SALT, 1)], &[(FOOD, Horizon::Now, 1)]),
        ];
        let mut book = BarterBook::new();

        assert!(book.post_offer_with_provisional_leader(
            &agents,
            indirect_offer(1, FOOD, SALT, CLOTH, 1),
            0,
            Some(SALT),
        ));
        assert!(book.post_offer(&agents, offer(2, SALT, FOOD, 2), 0));
        let trades = book.clear_matches_with_provisional_leader(&mut agents, 1, Some(SALT));

        assert_eq!(trades.len(), 1);
        assert_eq!(
            trades[0].a_reason,
            BarterReason::IndirectFor { target: CLOTH }
        );
        assert_eq!(agents[0].stock.get(FOOD), 0);
        assert_eq!(agents[0].stock.get(SALT), 1);
        assert_eq!(agents[0].stock.get(CLOTH), 0);
    }

    #[test]
    fn indirect_offer_rechecks_provisional_saleability_at_clear() {
        let mut agents = vec![
            agent(1, &[(FOOD, 1)], &[(CLOTH, Horizon::Now, 1)]),
            agent(2, &[(SALT, 1)], &[(FOOD, Horizon::Now, 1)]),
        ];
        let mut book = BarterBook::new();

        assert!(book.post_offer_with_provisional_leader(
            &agents,
            indirect_offer(1, FOOD, SALT, CLOTH, 1),
            0,
            Some(SALT),
        ));
        assert!(book.post_offer(&agents, offer(2, SALT, FOOD, 2), 0));
        let trades = book.clear_matches_with_provisional_leader(&mut agents, 1, Some(WOOD));

        assert!(trades.is_empty());
        assert_eq!(book.reserved_qty(AgentId(1), FOOD), 0);
        assert_eq!(agents[0].stock.get(FOOD), 1);
        assert_eq!(agents[0].stock.get(SALT), 0);
    }

    #[test]
    fn indirect_offer_preserves_higher_ranked_wants() {
        let agents = vec![agent(
            1,
            &[(FOOD, 1)],
            &[(FOOD, Horizon::Next, 1), (CLOTH, Horizon::Now, 1)],
        )];
        let mut book = BarterBook::new();

        assert!(!book.post_offer_with_provisional_leader(
            &agents,
            indirect_offer(1, FOOD, SALT, CLOTH, 1),
            0,
            Some(SALT),
        ));
        assert_eq!(book.reserved_qty(AgentId(1), FOOD), 0);
    }

    #[test]
    fn indirect_offer_preserves_higher_ranked_partial_wants() {
        let agents = vec![agent(
            1,
            &[(FOOD, 1)],
            &[(FOOD, Horizon::Next, 2), (CLOTH, Horizon::Now, 1)],
        )];
        let mut book = BarterBook::new();

        assert!(!book.post_offer_with_provisional_leader(
            &agents,
            indirect_offer(1, FOOD, SALT, CLOTH, 1),
            0,
            Some(SALT),
        ));
        assert_eq!(book.reserved_qty(AgentId(1), FOOD), 0);
    }

    #[test]
    fn indirect_offer_requires_leader_to_be_tradeable_for_final_target() {
        let agents = vec![agent(
            1,
            &[(FOOD, 1)],
            &[(SALT, Horizon::Next, 1), (CLOTH, Horizon::Now, 1)],
        )];
        let mut book = BarterBook::new();

        assert!(!book.post_offer_with_provisional_leader(
            &agents,
            indirect_offer(1, FOOD, SALT, CLOTH, 1),
            0,
            Some(SALT),
        ));
        assert_eq!(book.reserved_qty(AgentId(1), FOOD), 0);
    }

    #[test]
    fn indirect_offer_cannot_give_final_target_good() {
        let agents = vec![agent(1, &[(CLOTH, 1)], &[(CLOTH, Horizon::Next, 2)])];
        let mut book = BarterBook::new();

        assert!(!book.post_offer_with_provisional_leader(
            &agents,
            indirect_offer(1, CLOTH, SALT, CLOTH, 1),
            0,
            Some(SALT),
        ));
        assert_eq!(book.reserved_qty(AgentId(1), CLOTH), 0);
    }

    fn reciprocal_agents() -> Vec<Agent> {
        vec![
            agent(1, &[(FOOD, 1)], &[(WOOD, Horizon::Now, 1)]),
            agent(2, &[(WOOD, 1)], &[(FOOD, Horizon::Now, 1)]),
        ]
    }

    fn agent(id: u32, stock_goods: &[(GoodId, u32)], wants: &[(GoodId, Horizon, u32)]) -> Agent {
        let mut stock = Stock::new(6);
        for (good, qty) in stock_goods {
            stock.add(*good, *qty);
        }
        Agent {
            id: AgentId(u64::from(id)),
            scale: wants
                .iter()
                .map(|(good, horizon, qty)| Want {
                    kind: WantKind::Good(*good),
                    horizon: *horizon,
                    qty: *qty,
                    satisfied: false,
                })
                .collect(),
            stock,
            gold: Gold(7),
            labor_capacity: 0,
            hunger_deficit: 0,
            roles: vec![Role::Trader],
            expect: Vec::new(),
        }
    }

    fn offer(agent: u32, give_good: GoodId, receive_good: GoodId, seq: u64) -> BarterOffer {
        expiring_offer(agent, give_good, receive_good, seq, 10)
    }

    fn indirect_offer(
        agent: u32,
        give_good: GoodId,
        receive_good: GoodId,
        target: GoodId,
        seq: u64,
    ) -> BarterOffer {
        BarterOffer {
            agent: AgentId(u64::from(agent)),
            give_good,
            receive_good,
            qty: 1,
            reason: BarterReason::IndirectFor { target },
            seq,
            expires_tick: 10,
        }
    }

    fn expiring_offer(
        agent: u32,
        give_good: GoodId,
        receive_good: GoodId,
        seq: u64,
        expires_tick: u64,
    ) -> BarterOffer {
        BarterOffer {
            agent: AgentId(u64::from(agent)),
            give_good,
            receive_good,
            qty: 1,
            reason: BarterReason::DirectWant,
            seq,
            expires_tick,
        }
    }

    fn total_stock(agents: &[Agent], good: GoodId) -> u32 {
        agents
            .iter()
            .map(|agent| agent.stock.get(good))
            .fold(0u32, u32::saturating_add)
    }
}
