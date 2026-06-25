//! The acting man: ordinal value scale, wants, satisfaction, and consumption.

use std::fmt;

use crate::expect::PriceBelief;
use crate::good::{Gold, GoodId, Horizon, Stock, FOOD, GOLD};
use crate::marketability::MarketabilityAcceptance;

/// Stable colonist identity.
///
/// G0b widens the lab's bare `u32` to a `u64` that packs a generation into the
/// high 32 bits and the numeric index into the low 32 bits. A bare
/// `AgentId(212)` literal still compiles and still means *index 212,
/// generation 0*, because the literal infers `u64` and its high bits are zero.
/// For generation-0 ids the packed value equals the index, so `Ord` matches the
/// old `u32` ordering and every tape/CSV digit is unchanged. A regenerated id
/// (generation ≥ 1) sorts after its generation-0 ancestor and formats
/// distinguishably — a surface no existing golden can reference.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct AgentId(pub u64);

impl AgentId {
    /// The numeric index — the low 32 bits. For lab casts this is the authored id.
    pub fn index(self) -> u32 {
        (self.0 & 0xFFFF_FFFF) as u32
    }

    /// The generation — the high 32 bits. Zero for every lab id.
    pub fn generation(self) -> u32 {
        (self.0 >> 32) as u32
    }

    /// Pack an `(index, generation)` pair. `with_generation(n, 0) == AgentId(u64::from(n))`.
    pub fn with_generation(index: u32, generation: u32) -> Self {
        AgentId((u64::from(generation) << 32) | u64::from(index))
    }
}

impl fmt::Display for AgentId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        // Generation-0 ids print exactly their index, byte-identical to the
        // pre-widening `u32` formatting every lab golden depends on. A
        // regenerated id gains a `#<gen>` suffix — a never-before-printed
        // surface, so it cannot move a golden.
        if self.generation() == 0 {
            write!(f, "{}", self.index())
        } else {
            write!(f, "{}#{}", self.index(), self.generation())
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WantKind {
    Good(GoodId),
    Leisure,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Want {
    pub kind: WantKind,
    pub horizon: Horizon,
    pub qty: u32,
    pub satisfied: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Role {
    Household,
    Producer,
    Trader,
    Capitalist,
    Worker,
    Consumer,
}

impl fmt::Display for Role {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Role::Household => f.write_str("Household"),
            Role::Producer => f.write_str("Producer"),
            Role::Trader => f.write_str("Trader"),
            Role::Capitalist => f.write_str("Capitalist"),
            Role::Worker => f.write_str("Worker"),
            Role::Consumer => f.write_str("Consumer"),
        }
    }
}

#[derive(Clone, Debug)]
pub struct Agent {
    pub id: AgentId,
    pub scale: Vec<Want>,
    pub stock: Stock,
    pub gold: Gold,
    pub labor_capacity: u32,
    /// Lifetime diagnostic count of missed food units; it never affects planning.
    pub hunger_deficit: u32,
    pub roles: Vec<Role>,
    pub expect: Vec<PriceBelief>,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Consumption {
    pub food_consumed: u32,
    pub hunger_deficit: u32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Side {
    Bid,
    Ask,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Reservation {
    pub side: Side,
    pub good: GoodId,
    pub limit: Gold,
    pub qty: u32,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct TickProvisions {
    pub(crate) provided: Vec<bool>,
    pub(crate) allocated: Vec<u32>,
    pub(crate) reserved: Vec<(GoodId, u32)>,
}

impl TickProvisions {
    pub(crate) fn new(wants: usize) -> Self {
        Self {
            provided: vec![false; wants],
            allocated: vec![0; wants],
            reserved: Vec::new(),
        }
    }

    pub(crate) fn mark(&mut self, index: usize) {
        if let Some(provided) = self.provided.get_mut(index) {
            *provided = true;
        }
    }

    pub(crate) fn allocated(&self, index: usize) -> u32 {
        self.allocated.get(index).copied().unwrap_or(0)
    }

    pub(crate) fn remaining_for(&self, index: usize, qty: u32) -> u32 {
        qty.saturating_sub(self.allocated(index))
    }

    pub(crate) fn allocate(&mut self, index: usize, qty: u32) {
        if qty == 0 {
            return;
        }
        if let Some(allocated) = self.allocated.get_mut(index) {
            *allocated = allocated.saturating_add(qty);
        }
    }

    pub(crate) fn is_fully_allocated(&self, index: usize, qty: u32) -> bool {
        self.remaining_for(index, qty) == 0
    }

    pub(crate) fn reserve(&mut self, good: GoodId, qty: u32) {
        reserve(&mut self.reserved, good, qty);
    }
}

impl Agent {
    pub fn clear_satisfaction(&mut self) {
        for want in &mut self.scale {
            want.satisfied = false;
        }
    }

    /// Clear tick-local satisfaction and mark on-hand `Next` goods provisioned.
    ///
    /// Higher-ranked `Now` wants reserve stock for the consumption pass but are
    /// not marked satisfied here. `Later` goods require a capital-using recipe
    /// in the simulator.
    pub fn recompute_satisfaction(&mut self) {
        self.recompute_satisfaction_for_money(GOLD);
    }

    pub fn recompute_satisfaction_without_money(&mut self) {
        self.recompute_satisfaction_with_optional_money(None);
    }

    pub fn recompute_satisfaction_for_money(&mut self, money_good: GoodId) {
        self.recompute_satisfaction_with_optional_money(Some(money_good));
    }

    fn recompute_satisfaction_with_optional_money(&mut self, money_good: Option<GoodId>) {
        self.clear_satisfaction();

        let mut reserved = Vec::new();
        let mut reserved_money = Gold::ZERO;
        let mut blocked_goods: Vec<GoodId> = Vec::new();
        let mut money_blocked = false;
        for want in &mut self.scale {
            if let WantKind::Good(good) = want.kind {
                if Some(good) == money_good {
                    if money_blocked {
                        continue;
                    }
                    if self.gold.saturating_sub(reserved_money).0 >= u64::from(want.qty) {
                        reserved_money = reserved_money.saturating_add(Gold(u64::from(want.qty)));
                        want.satisfied = true;
                    } else {
                        money_blocked = true;
                    }
                    continue;
                }

                if blocked_goods.contains(&good) {
                    continue;
                }
                match want.horizon {
                    Horizon::Now => {
                        if available_after_reserved(&self.stock, &reserved, good) >= want.qty {
                            reserve(&mut reserved, good, want.qty);
                        } else {
                            blocked_goods.push(good);
                        }
                    }
                    Horizon::Next => {
                        if available_after_reserved(&self.stock, &reserved, good) >= want.qty {
                            reserve(&mut reserved, good, want.qty);
                            want.satisfied = true;
                        } else {
                            blocked_goods.push(good);
                        }
                    }
                    Horizon::Later(_) => {}
                }
            }
        }
    }

    pub fn first_unsatisfied_rank(&self, kind: WantKind) -> Option<usize> {
        self.scale
            .iter()
            .position(|want| !want.satisfied && want.kind == kind)
    }

    pub fn first_unsatisfied_good_rank(&self, good: GoodId) -> Option<usize> {
        let provisions = provisioning(&self.scale, &self.stock, self.gold);
        self.scale.iter().enumerate().position(|(index, want)| {
            want.kind == WantKind::Good(good)
                && !provisions.provided.get(index).copied().unwrap_or(false)
        })
    }

    pub(crate) fn near_unsatisfied_goods_without_money(&self) -> Vec<GoodId> {
        let provisions = barter_provisioning(&self.scale, &self.stock);
        let max_good_id = self
            .scale
            .iter()
            .filter_map(|want| match want.kind {
                WantKind::Good(good) => Some(usize::from(good.0)),
                WantKind::Leisure => None,
            })
            .max()
            .unwrap_or(0);
        let mut first_unprovided_goods = vec![false; max_good_id.saturating_add(1)];
        let mut near_goods = Vec::new();
        for (index, want) in self.scale.iter().enumerate() {
            let WantKind::Good(good) = want.kind else {
                continue;
            };
            if want.satisfied
                || !matches!(want.horizon, Horizon::Now | Horizon::Next)
                || provisions.provided.get(index).copied().unwrap_or(false)
            {
                continue;
            }
            let seen = &mut first_unprovided_goods[usize::from(good.0)];
            if !*seen {
                *seen = true;
                near_goods.push(good);
            }
        }
        near_goods
    }

    pub(crate) fn stock_reserved_for_near_wants_for_money(
        &self,
        good: GoodId,
        money_good: GoodId,
    ) -> u32 {
        let provisions = provisioning_for_money(&self.scale, &self.stock, self.gold, money_good);
        self.scale
            .iter()
            .enumerate()
            .filter(|(_, want)| {
                want.kind == WantKind::Good(good)
                    && match want.horizon {
                        Horizon::Now => !want.satisfied,
                        Horizon::Next => true,
                        Horizon::Later(_) => false,
                    }
            })
            .map(|(index, _)| provisions.allocated.get(index).copied().unwrap_or(0))
            .fold(0u32, u32::saturating_add)
    }

    /// Stock of `good` reserved to this agent's Now/Next wants under **barter**
    /// provisioning (no money) — the protected near allocation the barter
    /// preservation rule (`barter_swap_acceptable` / `preserved_near_allocations_above_target`)
    /// guards. Holdings above this are *offerable*: giving a unit keeps every
    /// equal-or-higher-priority near want at least as provisioned, so the offer
    /// generator can post it. Unlike the indirect-swap predicate, this is
    /// **target-independent** — it answers "is this good removable surplus", not
    /// "does the agent currently want some other good for it".
    pub fn stock_reserved_for_near_wants_barter(&self, good: GoodId) -> u32 {
        let provisions = barter_provisioning(&self.scale, &self.stock);
        self.scale
            .iter()
            .enumerate()
            .filter(|(_, want)| {
                want.kind == WantKind::Good(good)
                    && matches!(want.horizon, Horizon::Now | Horizon::Next)
            })
            .map(|(index, _)| provisions.allocated.get(index).copied().unwrap_or(0))
            .fold(0u32, u32::saturating_add)
    }

    pub fn first_unsatisfied_leisure_rank(&self) -> Option<usize> {
        self.first_unsatisfied_rank(WantKind::Leisure)
    }

    pub fn consume_now_wants(&mut self) -> Consumption {
        self.consume_now_wants_with_provisions().0
    }

    pub fn would_accept_buy(&self, good: GoodId, qty: u32, price: Gold) -> bool {
        self.would_accept_buy_for_money(good, qty, price, GOLD)
    }

    pub fn would_accept_buy_for_money(
        &self,
        good: GoodId,
        qty: u32,
        price: Gold,
        money_good: GoodId,
    ) -> bool {
        price > Gold::ZERO
            && self
                .reservation_bid_for_money(good, qty, money_good)
                .map(|reservation| price <= reservation)
                .unwrap_or(false)
    }

    pub fn would_accept_sell(&self, good: GoodId, qty: u32, price: Gold) -> bool {
        self.would_accept_sell_for_money(good, qty, price, GOLD)
    }

    pub fn would_accept_sell_for_money(
        &self,
        good: GoodId,
        qty: u32,
        price: Gold,
        money_good: GoodId,
    ) -> bool {
        price > Gold::ZERO
            && self
                .reservation_ask_for_money(good, qty, money_good)
                .map(|reservation| price >= reservation)
                .unwrap_or(false)
    }

    pub fn reservation_bid(&self, good: GoodId, qty: u32) -> Option<Gold> {
        self.reservation_bid_for_money(good, qty, GOLD)
    }

    pub fn reservation_bid_for_money(
        &self,
        good: GoodId,
        qty: u32,
        money_good: GoodId,
    ) -> Option<Gold> {
        if good == money_good || qty == 0 || self.gold == Gold::ZERO {
            return None;
        }

        let before = provisioning_for_money(&self.scale, &self.stock, self.gold, money_good);
        let target = self.scale.iter().enumerate().position(|(index, want)| {
            want.kind == WantKind::Good(good)
                && matches!(want.horizon, Horizon::Now | Horizon::Next)
                && !before.provided.get(index).copied().unwrap_or(false)
        })?;

        let mut after_stock = self.stock.clone();
        after_stock.add(good, qty);

        let protected_money = allocated_money_before_rank(&self.scale, &before, target, money_good);
        let price = self.gold.checked_sub(protected_money)?;
        if price == Gold::ZERO {
            return None;
        }
        let after_gold = self.gold.checked_sub(price)?;
        let after = provisioning_for_money(&self.scale, &after_stock, after_gold, money_good);

        if after.provided.get(target).copied().unwrap_or(false)
            && preserved_above_target(&before.provided, &after.provided, target)
        {
            Some(price)
        } else {
            None
        }
    }

    pub fn reservation_ask(&self, good: GoodId, qty: u32) -> Option<Gold> {
        self.reservation_ask_for_money(good, qty, GOLD)
    }

    pub fn reservation_ask_for_money(
        &self,
        good: GoodId,
        qty: u32,
        money_good: GoodId,
    ) -> Option<Gold> {
        if good == money_good || qty == 0 || !self.stock.can_remove(good, qty) {
            return None;
        }

        let before = provisioning_for_money(&self.scale, &self.stock, self.gold, money_good);
        let mut removed_stock = self.stock.clone();
        if !removed_stock.remove(good, qty) {
            return None;
        }
        let after_without_price =
            provisioning_for_money(&self.scale, &removed_stock, self.gold, money_good);
        let lost_rank = self
            .scale
            .iter()
            .enumerate()
            .find(|(index, want)| {
                want.kind == WantKind::Good(good)
                    && before.allocated.get(*index).copied().unwrap_or(0)
                        > after_without_price
                            .allocated
                            .get(*index)
                            .copied()
                            .unwrap_or(0)
            })
            .map(|(index, _)| index)
            .unwrap_or(self.scale.len());

        if !preserved_above_target(&before.provided, &after_without_price.provided, lost_rank) {
            return None;
        }

        let price = first_money_gain_price_at_or_above(
            &self.scale,
            &before.provided,
            self.gold,
            lost_rank,
            money_good,
        )?;
        let after_gold = self.gold.checked_add(price)?;
        let after = provisioning_for_money(&self.scale, &removed_stock, after_gold, money_good);
        if preserved_above_target(&before.provided, &after.provided, lost_rank)
            && money_want_gained_at_or_above(
                &self.scale,
                &before.provided,
                &after.provided,
                lost_rank,
                money_good,
            )
        {
            Some(price)
        } else {
            None
        }
    }

    pub fn would_accept_barter_swap(
        &self,
        give_good: GoodId,
        receive_good: GoodId,
        qty: u32,
    ) -> bool {
        self.would_accept_barter_swap_with_stock(&self.stock, give_good, receive_good, qty)
    }

    pub(crate) fn would_accept_barter_swap_with_stock(
        &self,
        stock: &Stock,
        give_good: GoodId,
        receive_good: GoodId,
        qty: u32,
    ) -> bool {
        barter_swap_acceptable(&self.scale, stock, give_good, receive_good, qty)
    }

    pub fn would_accept_indirect_barter_swap_with_stock(
        &self,
        stock: &Stock,
        give_good: GoodId,
        receive_good: GoodId,
        target_good: GoodId,
        qty: u32,
        marketability: MarketabilityAcceptance<'_>,
    ) -> bool {
        if give_good == receive_good
            || give_good == target_good
            || receive_good == target_good
            || qty == 0
            || !stock.can_remove(give_good, qty)
            || stock.get(receive_good).checked_add(qty).is_none()
        {
            return false;
        }
        if marketability.durability_aware_acceptance
            && !marketability
                .config
                .can_cover_holding_period(receive_good, qty)
        {
            return false;
        }

        let before = barter_provisioning(&self.scale, stock);
        let Some(target) = self.scale.iter().enumerate().position(|(index, want)| {
            want.kind == WantKind::Good(target_good)
                && matches!(want.horizon, Horizon::Now | Horizon::Next)
                && !before.provided.get(index).copied().unwrap_or(false)
        }) else {
            return false;
        };

        let mut after_stock = stock.clone();
        if !after_stock.remove(give_good, qty) {
            return false;
        }
        after_stock.add(receive_good, qty);
        let after = barter_provisioning(&self.scale, &after_stock);
        if !barter_swap_acceptable(&self.scale, &after_stock, receive_good, target_good, qty) {
            return false;
        }

        preserved_near_allocations_above_target(&self.scale, &before, &after, target)
    }

    pub(crate) fn money_reserved_for_near_wants_for_money(&self, money_good: GoodId) -> Gold {
        let provisions = provisioning_for_money(&self.scale, &self.stock, self.gold, money_good);
        let reserved = self
            .scale
            .iter()
            .enumerate()
            .filter(|(_, want)| {
                want.kind == WantKind::Good(money_good)
                    && matches!(want.horizon, Horizon::Now | Horizon::Next)
            })
            .map(|(index, _)| u64::from(provisions.allocated.get(index).copied().unwrap_or(0)))
            .fold(0u64, u64::saturating_add);
        Gold(reserved)
    }
}

fn barter_swap_acceptable(
    scale: &[Want],
    stock: &Stock,
    give_good: GoodId,
    receive_good: GoodId,
    qty: u32,
) -> bool {
    if give_good == receive_good
        || qty == 0
        || !stock.can_remove(give_good, qty)
        || stock.get(receive_good).checked_add(qty).is_none()
    {
        return false;
    }

    let before = barter_provisioning(scale, stock);
    let mut after_stock = stock.clone();
    if !after_stock.remove(give_good, qty) {
        return false;
    }
    after_stock.add(receive_good, qty);
    let after = barter_provisioning(scale, &after_stock);

    let Some(target) = scale.iter().enumerate().position(|(index, want)| {
        want.kind == WantKind::Good(receive_good)
            && matches!(want.horizon, Horizon::Now | Horizon::Next)
            && !before.provided.get(index).copied().unwrap_or(false)
            && after.provided.get(index).copied().unwrap_or(false)
    }) else {
        return false;
    };

    preserved_near_allocations_above_target(scale, &before, &after, target)
}

impl Agent {
    pub(crate) fn consume_now_wants_with_provisions(&mut self) -> (Consumption, TickProvisions) {
        self.consume_now_wants_with_provisions_for_money(GOLD)
    }

    pub(crate) fn consume_now_wants_with_provisions_without_money(
        &mut self,
    ) -> (Consumption, TickProvisions) {
        let (consumption, allocations) = self.consume_now_wants_with_allocations(None);
        let provisions = self.provisions_after_consumption(&allocations);
        self.recompute_satisfaction_with_provisions_without_money(&provisions);

        (consumption, provisions)
    }

    pub(crate) fn consume_now_wants_with_provisions_for_money(
        &mut self,
        money_good: GoodId,
    ) -> (Consumption, TickProvisions) {
        let (consumption, allocations) = self.consume_now_wants_with_allocations(Some(money_good));
        let provisions = self.provisions_after_consumption(&allocations);
        self.recompute_satisfaction_with_provisions_for_money(&provisions, money_good);

        (consumption, provisions)
    }

    pub(crate) fn recompute_satisfaction_with_provisions(&mut self, provisions: &TickProvisions) {
        self.recompute_satisfaction_with_provisions_for_money(provisions, GOLD);
    }

    pub(crate) fn recompute_satisfaction_with_provisions_without_money(
        &mut self,
        provisions: &TickProvisions,
    ) {
        self.recompute_satisfaction_with_provisions_with_optional_money(provisions, None);
    }

    pub(crate) fn recompute_satisfaction_with_provisions_for_money(
        &mut self,
        provisions: &TickProvisions,
        money_good: GoodId,
    ) {
        self.recompute_satisfaction_with_provisions_with_optional_money(
            provisions,
            Some(money_good),
        );
    }

    fn recompute_satisfaction_with_provisions_with_optional_money(
        &mut self,
        provisions: &TickProvisions,
        money_good: Option<GoodId>,
    ) {
        self.clear_satisfaction();

        let mut reserved = Vec::new();
        let mut reserved_money = Gold::ZERO;
        let mut money_blocked = false;
        for (good, qty) in &provisions.reserved {
            reserve(&mut reserved, *good, *qty);
        }

        let stock = &self.stock;
        let mut blocked_goods: Vec<GoodId> = Vec::new();
        for (index, want) in self.scale.iter_mut().enumerate() {
            if provisions.provided.get(index).copied().unwrap_or(false) {
                if let WantKind::Good(good) = want.kind {
                    if Some(good) == money_good {
                        if self.gold.saturating_sub(reserved_money).0 >= u64::from(want.qty) {
                            reserved_money =
                                reserved_money.saturating_add(Gold(u64::from(want.qty)));
                        } else {
                            money_blocked = true;
                        }
                    }
                }
                continue;
            }

            if let WantKind::Good(good) = want.kind {
                if Some(good) == money_good {
                    if money_blocked {
                        continue;
                    }
                    if self.gold.saturating_sub(reserved_money).0 >= u64::from(want.qty) {
                        reserved_money = reserved_money.saturating_add(Gold(u64::from(want.qty)));
                        want.satisfied = true;
                    } else {
                        money_blocked = true;
                    }
                    continue;
                }

                if blocked_goods.contains(&good) {
                    continue;
                }
                let needed = provisions.remaining_for(index, want.qty);
                match want.horizon {
                    Horizon::Now => {
                        if needed > 0 {
                            blocked_goods.push(good);
                        }
                    }
                    Horizon::Next => {
                        if needed == 0 || available_after_reserved(stock, &reserved, good) >= needed
                        {
                            reserve(&mut reserved, good, needed);
                            want.satisfied = true;
                        } else {
                            blocked_goods.push(good);
                        }
                    }
                    Horizon::Later(_) => {}
                }
            }
        }

        for (want, provided) in self.scale.iter_mut().zip(&provisions.provided) {
            if *provided {
                want.satisfied = true;
            }
        }
    }

    fn consume_now_wants_with_allocations(
        &mut self,
        money_good: Option<GoodId>,
    ) -> (Consumption, Vec<u32>) {
        let mut consumption = Consumption::default();
        let mut allocations = vec![0; self.scale.len()];
        let mut reserved = Vec::new();

        for (index, want) in self.scale.iter_mut().enumerate() {
            if want.satisfied {
                if let WantKind::Good(good) = want.kind {
                    if Some(good) != money_good && matches!(want.horizon, Horizon::Next) {
                        reserve(&mut reserved, good, want.qty);
                    }
                }
                continue;
            }
            if let WantKind::Good(good) = want.kind {
                if Some(good) == money_good {
                    continue;
                }
                if matches!(want.horizon, Horizon::Next) {
                    let provisioned =
                        available_after_reserved(&self.stock, &reserved, good).min(want.qty);
                    reserve(&mut reserved, good, provisioned);
                    continue;
                }
            }
            if let WantKind::Good(good) = want.kind {
                if !matches!(want.horizon, Horizon::Now) {
                    continue;
                }
                if want.qty == 0 {
                    want.satisfied = true;
                    continue;
                }

                let consumed = available_after_reserved(&self.stock, &reserved, good).min(want.qty);
                if consumed > 0 {
                    self.stock.remove(good, consumed);
                    if good == FOOD {
                        consumption.food_consumed += consumed;
                    }
                    allocations[index] = consumed;
                }
                if consumed == want.qty {
                    want.satisfied = true;
                } else if good == FOOD {
                    let deficit = want.qty - consumed;
                    consumption.hunger_deficit += deficit;
                    self.hunger_deficit += deficit;
                }
            }
        }

        (consumption, allocations)
    }

    fn provisions_after_consumption(&self, consumed_by_want: &[u32]) -> TickProvisions {
        let mut provisions = TickProvisions::new(self.scale.len());
        for (index, want) in self.scale.iter().enumerate() {
            let consumed = consumed_by_want.get(index).copied().unwrap_or(0);
            if consumed > 0 {
                provisions.allocate(index, consumed);
            }
            if want.satisfied && matches!(want.horizon, Horizon::Now) {
                provisions.mark(index);
            }
        }
        provisions
    }
}

pub fn want_provisioned_by_holding(scale: &[Want], idx: usize, stock: &Stock, gold: Gold) -> bool {
    provisioning(scale, stock, gold)
        .provided
        .get(idx)
        .copied()
        .unwrap_or(false)
}

struct Provisioning {
    provided: Vec<bool>,
    allocated: Vec<u32>,
}

fn provisioning(scale: &[Want], stock: &Stock, gold: Gold) -> Provisioning {
    provisioning_for_money(scale, stock, gold, GOLD)
}

fn provisioning_for_money(
    scale: &[Want],
    stock: &Stock,
    gold: Gold,
    money_good: GoodId,
) -> Provisioning {
    provisioning_with_optional_money(scale, stock, Some((money_good, gold)))
}

fn barter_provisioning(scale: &[Want], stock: &Stock) -> Provisioning {
    provisioning_with_optional_money(scale, stock, None)
}

fn provisioning_with_optional_money(
    scale: &[Want],
    stock: &Stock,
    money: Option<(GoodId, Gold)>,
) -> Provisioning {
    let mut provided = vec![false; scale.len()];
    let mut allocated = vec![0; scale.len()];
    let mut reserved = Vec::new();
    let mut blocked_goods: Vec<GoodId> = Vec::new();
    let mut reserved_money = Gold::ZERO;
    let mut money_blocked = false;

    for (index, want) in scale.iter().enumerate() {
        let WantKind::Good(good) = want.kind else {
            continue;
        };

        if let Some((money_good, gold)) = money {
            if good == money_good {
                if money_blocked {
                    continue;
                }
                let available = gold
                    .saturating_sub(reserved_money)
                    .0
                    .min(u64::from(want.qty));
                let available_qty = u32::try_from(available).unwrap_or(u32::MAX);
                if available > 0 {
                    reserved_money = reserved_money.saturating_add(Gold(available));
                    allocated[index] = available_qty;
                }
                if available == u64::from(want.qty) {
                    provided[index] = true;
                } else {
                    money_blocked = true;
                }
                continue;
            }
        }

        if blocked_goods.contains(&good) {
            continue;
        }
        if matches!(want.horizon, Horizon::Later(_)) {
            continue;
        }
        let available = available_after_reserved(stock, &reserved, good).min(want.qty);
        if available > 0 {
            reserve(&mut reserved, good, available);
            allocated[index] = available;
        }
        if available == want.qty {
            provided[index] = true;
        } else {
            blocked_goods.push(good);
        }
    }

    Provisioning {
        provided,
        allocated,
    }
}

fn preserved_above_target(before: &[bool], after: &[bool], target: usize) -> bool {
    before
        .iter()
        .zip(after)
        .take(target)
        .all(|(was, now)| !*was || *now)
}

fn preserved_near_allocations_above_target(
    scale: &[Want],
    before: &Provisioning,
    after: &Provisioning,
    target: usize,
) -> bool {
    scale.iter().enumerate().take(target).all(|(index, want)| {
        !matches!(want.horizon, Horizon::Now | Horizon::Next)
            || before.allocated.get(index).copied().unwrap_or(0)
                <= after.allocated.get(index).copied().unwrap_or(0)
    })
}

fn allocated_money_before_rank(
    scale: &[Want],
    before: &Provisioning,
    target: usize,
    money_good: GoodId,
) -> Gold {
    let grains = scale
        .iter()
        .enumerate()
        .take(target)
        .filter(|(_, want)| want.kind == WantKind::Good(money_good))
        .map(|(index, _)| u64::from(before.allocated.get(index).copied().unwrap_or(0)))
        .fold(0u64, u64::saturating_add);
    Gold(grains)
}

fn first_money_gain_price_at_or_above(
    scale: &[Want],
    before: &[bool],
    gold: Gold,
    lost_rank: usize,
    money_good: GoodId,
) -> Option<Gold> {
    let upper = if lost_rank >= scale.len() {
        scale.len()
    } else {
        lost_rank + 1
    };
    let mut required = 0u64;

    for (index, want) in scale.iter().enumerate().take(upper) {
        if want.kind != WantKind::Good(money_good) {
            continue;
        }
        required = required.saturating_add(u64::from(want.qty));
        if before.get(index).copied().unwrap_or(false) || required <= gold.0 {
            continue;
        }
        return Some(Gold(required - gold.0));
    }

    None
}

fn money_want_gained_at_or_above(
    scale: &[Want],
    before: &[bool],
    after: &[bool],
    lost_rank: usize,
    money_good: GoodId,
) -> bool {
    let upper = if lost_rank >= scale.len() {
        scale.len()
    } else {
        lost_rank + 1
    };
    scale
        .iter()
        .zip(before)
        .zip(after)
        .take(upper)
        .any(|((want, was), now)| want.kind == WantKind::Good(money_good) && !*was && *now)
}

fn available_after_reserved(stock: &Stock, reservations: &[(GoodId, u32)], good: GoodId) -> u32 {
    let reserved = reservations
        .iter()
        .filter(|(reserved_good, _)| *reserved_good == good)
        .map(|(_, qty)| *qty)
        .sum::<u32>();
    stock.get(good).saturating_sub(reserved)
}

fn reserve(reservations: &mut Vec<(GoodId, u32)>, good: GoodId, qty: u32) {
    if qty == 0 {
        return;
    }
    if let Some((_, reserved)) = reservations
        .iter_mut()
        .find(|(reserved_good, _)| *reserved_good == good)
    {
        *reserved = reserved.saturating_add(qty);
    } else {
        reservations.push((good, qty));
    }
}

#[cfg(test)]
mod tests {
    use super::{Agent, AgentId, Want, WantKind};
    use crate::good::{Gold, Horizon, Stock, CLOTH, FOOD, GOLD, NET, SALT, WOOD};

    #[test]
    fn consume_now_wants_respects_scale() {
        let mut stock = Stock::new(3);
        stock.add(FOOD, 1);
        let mut agent = Agent {
            id: AgentId(1),
            scale: vec![
                Want {
                    kind: WantKind::Good(FOOD),
                    horizon: Horizon::Now,
                    qty: 1,
                    satisfied: false,
                },
                Want {
                    kind: WantKind::Good(NET),
                    horizon: Horizon::Now,
                    qty: 1,
                    satisfied: false,
                },
                Want {
                    kind: WantKind::Good(FOOD),
                    horizon: Horizon::Now,
                    qty: 1,
                    satisfied: false,
                },
            ],
            stock,
            labor_capacity: 0,
            hunger_deficit: 0,
            gold: crate::good::Gold::ZERO,
            roles: vec![crate::agent::Role::Household],
            expect: Vec::new(),
        };

        let consumption = agent.consume_now_wants();

        assert_eq!(consumption.food_consumed, 1);
        assert_eq!(consumption.hunger_deficit, 1);
        assert!(agent.scale[0].satisfied);
        assert!(!agent.scale[1].satisfied);
        assert!(!agent.scale[2].satisfied);
        assert_eq!(agent.hunger_deficit, 1);
    }

    #[test]
    fn stock_does_not_satisfy_later_wants() {
        let mut stock = Stock::new(3);
        stock.add(FOOD, 10);
        let mut agent = Agent {
            id: AgentId(1),
            scale: vec![Want {
                kind: WantKind::Good(FOOD),
                horizon: Horizon::Later(4),
                qty: 1,
                satisfied: false,
            }],
            stock,
            labor_capacity: 0,
            hunger_deficit: 0,
            gold: crate::good::Gold::ZERO,
            roles: vec![crate::agent::Role::Household],
            expect: Vec::new(),
        };

        agent.recompute_satisfaction();

        assert!(!agent.scale[0].satisfied);
    }

    #[test]
    fn consume_now_wants_preserves_higher_ranked_next_food() {
        let mut stock = Stock::new(3);
        stock.add(FOOD, 1);
        let mut agent = Agent {
            id: AgentId(1),
            scale: vec![
                Want {
                    kind: WantKind::Good(FOOD),
                    horizon: Horizon::Next,
                    qty: 1,
                    satisfied: false,
                },
                Want {
                    kind: WantKind::Good(FOOD),
                    horizon: Horizon::Now,
                    qty: 1,
                    satisfied: false,
                },
            ],
            stock,
            labor_capacity: 0,
            hunger_deficit: 0,
            gold: crate::good::Gold::ZERO,
            roles: vec![crate::agent::Role::Household],
            expect: Vec::new(),
        };

        agent.recompute_satisfaction();
        let consumption = agent.consume_now_wants();

        assert_eq!(consumption.food_consumed, 0);
        assert_eq!(consumption.hunger_deficit, 1);
        assert_eq!(agent.stock.get(FOOD), 1);
        assert!(agent.scale[0].satisfied);
        assert!(!agent.scale[1].satisfied);
    }

    #[test]
    fn consume_now_wants_reserves_partial_higher_ranked_next_food() {
        let mut stock = Stock::new(3);
        stock.add(FOOD, 1);
        let mut agent = Agent {
            id: AgentId(1),
            scale: vec![
                Want {
                    kind: WantKind::Good(FOOD),
                    horizon: Horizon::Next,
                    qty: 2,
                    satisfied: false,
                },
                Want {
                    kind: WantKind::Good(FOOD),
                    horizon: Horizon::Now,
                    qty: 1,
                    satisfied: false,
                },
            ],
            stock,
            labor_capacity: 0,
            hunger_deficit: 0,
            gold: crate::good::Gold::ZERO,
            roles: vec![crate::agent::Role::Household],
            expect: Vec::new(),
        };

        agent.recompute_satisfaction();
        let consumption = agent.consume_now_wants();

        assert_eq!(consumption.food_consumed, 0);
        assert_eq!(consumption.hunger_deficit, 1);
        assert_eq!(agent.stock.get(FOOD), 1);
        assert!(!agent.scale[0].satisfied);
        assert!(!agent.scale[1].satisfied);
    }

    #[test]
    fn consume_now_wants_refreshes_lower_ranked_next_food() {
        let mut stock = Stock::new(3);
        stock.add(FOOD, 1);
        let mut agent = Agent {
            id: AgentId(1),
            scale: vec![
                Want {
                    kind: WantKind::Good(FOOD),
                    horizon: Horizon::Now,
                    qty: 1,
                    satisfied: false,
                },
                Want {
                    kind: WantKind::Good(FOOD),
                    horizon: Horizon::Next,
                    qty: 1,
                    satisfied: false,
                },
            ],
            stock,
            labor_capacity: 0,
            hunger_deficit: 0,
            gold: crate::good::Gold::ZERO,
            roles: vec![crate::agent::Role::Household],
            expect: Vec::new(),
        };

        agent.recompute_satisfaction();
        let consumption = agent.consume_now_wants();

        assert_eq!(consumption.food_consumed, 1);
        assert_eq!(consumption.hunger_deficit, 0);
        assert_eq!(agent.stock.get(FOOD), 0);
        assert!(agent.scale[0].satisfied);
        assert!(!agent.scale[1].satisfied);
    }

    #[test]
    fn consume_now_wants_does_not_double_reserve_consumed_now_food() {
        let mut stock = Stock::new(3);
        stock.add(FOOD, 2);
        let mut agent = Agent {
            id: AgentId(1),
            scale: vec![
                Want {
                    kind: WantKind::Good(FOOD),
                    horizon: Horizon::Now,
                    qty: 1,
                    satisfied: false,
                },
                Want {
                    kind: WantKind::Good(FOOD),
                    horizon: Horizon::Next,
                    qty: 1,
                    satisfied: false,
                },
            ],
            stock,
            labor_capacity: 0,
            hunger_deficit: 0,
            gold: crate::good::Gold::ZERO,
            roles: vec![crate::agent::Role::Household],
            expect: Vec::new(),
        };

        agent.recompute_satisfaction();
        let consumption = agent.consume_now_wants();

        assert_eq!(consumption.food_consumed, 1);
        assert_eq!(consumption.hunger_deficit, 0);
        assert_eq!(agent.stock.get(FOOD), 1);
        assert!(agent.scale[0].satisfied);
        assert!(agent.scale[1].satisfied);
    }

    #[test]
    fn provided_now_gold_does_not_satisfy_later_gold_again() {
        let mut agent = Agent {
            id: AgentId(1),
            scale: vec![
                Want {
                    kind: WantKind::Good(GOLD),
                    horizon: Horizon::Now,
                    qty: 1,
                    satisfied: false,
                },
                Want {
                    kind: WantKind::Good(GOLD),
                    horizon: Horizon::Later(1),
                    qty: 1,
                    satisfied: false,
                },
            ],
            stock: Stock::new(3),
            labor_capacity: 0,
            hunger_deficit: 0,
            gold: crate::good::Gold(1),
            roles: vec![crate::agent::Role::Household],
            expect: Vec::new(),
        };

        agent.recompute_satisfaction();
        agent.consume_now_wants();

        assert!(agent.scale[0].satisfied);
        assert!(!agent.scale[1].satisfied);
    }

    #[test]
    fn consume_now_wants_consumes_available_partial_qty() {
        let mut stock = Stock::new(3);
        stock.add(FOOD, 1);
        let mut agent = Agent {
            id: AgentId(1),
            scale: vec![Want {
                kind: WantKind::Good(FOOD),
                horizon: Horizon::Now,
                qty: 2,
                satisfied: false,
            }],
            stock,
            labor_capacity: 0,
            hunger_deficit: 0,
            gold: crate::good::Gold::ZERO,
            roles: vec![crate::agent::Role::Household],
            expect: Vec::new(),
        };

        let consumption = agent.consume_now_wants();

        assert_eq!(consumption.food_consumed, 1);
        assert_eq!(consumption.hunger_deficit, 1);
        assert_eq!(agent.stock.get(FOOD), 0);
        assert!(!agent.scale[0].satisfied);
        assert_eq!(agent.hunger_deficit, 1);
    }

    #[test]
    fn consume_now_wants_consumes_non_food_stock() {
        let mut stock = Stock::new(3);
        stock.add(NET, 1);
        let mut agent = Agent {
            id: AgentId(1),
            scale: vec![Want {
                kind: WantKind::Good(NET),
                horizon: Horizon::Now,
                qty: 1,
                satisfied: false,
            }],
            stock,
            labor_capacity: 0,
            hunger_deficit: 0,
            gold: crate::good::Gold::ZERO,
            roles: vec![crate::agent::Role::Household],
            expect: Vec::new(),
        };

        let consumption = agent.consume_now_wants();

        assert_eq!(consumption.food_consumed, 0);
        assert_eq!(consumption.hunger_deficit, 0);
        assert_eq!(agent.stock.get(NET), 0);
        assert!(agent.scale[0].satisfied);
    }

    #[test]
    fn consume_now_wants_preserves_satisfied_leisure() {
        let mut agent = Agent {
            id: AgentId(1),
            scale: vec![Want {
                kind: WantKind::Leisure,
                horizon: Horizon::Now,
                qty: 1,
                satisfied: true,
            }],
            stock: Stock::new(3),
            labor_capacity: 0,
            hunger_deficit: 0,
            gold: crate::good::Gold::ZERO,
            roles: vec![crate::agent::Role::Household],
            expect: Vec::new(),
        };

        let consumption = agent.consume_now_wants();

        assert_eq!(consumption, Default::default());
        assert!(agent.scale[0].satisfied);
    }

    #[test]
    fn reservation_bid_respects_scale() {
        let agent = market_agent(
            Gold(10),
            Stock::new(3),
            vec![
                good_want(GOLD, Horizon::Later(1)),
                good_want(GOLD, Horizon::Later(1)),
                good_want(GOLD, Horizon::Later(1)),
                good_want(FOOD, Horizon::Next),
            ],
        );

        assert_eq!(agent.reservation_bid(FOOD, 1), Some(Gold(7)));
        assert!(agent.would_accept_buy(FOOD, 1, Gold(7)));
        assert!(!agent.would_accept_buy(FOOD, 1, Gold(8)));
    }

    #[test]
    fn reservation_ask_respects_scale() {
        let mut stock = Stock::new(3);
        stock.add(FOOD, 1);
        let agent = market_agent(
            Gold::ZERO,
            stock,
            vec![
                good_want(FOOD, Horizon::Next),
                good_want(GOLD, Horizon::Later(1)),
            ],
        );

        assert_eq!(agent.reservation_ask(FOOD, 1), None);

        let mut stock = Stock::new(3);
        stock.add(FOOD, 1);
        let agent = market_agent(
            Gold::ZERO,
            stock,
            vec![
                good_want(GOLD, Horizon::Later(1)),
                good_want(FOOD, Horizon::Next),
            ],
        );

        assert_eq!(agent.reservation_ask(FOOD, 1), Some(Gold(1)));
        assert!(agent.would_accept_sell(FOOD, 1, Gold(1)));
        assert!(!agent.would_accept_sell(FOOD, 1, Gold::ZERO));
    }

    #[test]
    fn would_accept_buy_uses_designated_money_good() {
        let agent = market_agent(Gold(3), Stock::new(6), vec![good_want(FOOD, Horizon::Next)]);

        assert!(agent.would_accept_buy_for_money(FOOD, 1, Gold(3), SALT));
        assert!(!agent.would_accept_buy_for_money(FOOD, 1, Gold(4), SALT));
        assert!(!agent.would_accept_buy_for_money(SALT, 1, Gold(1), SALT));
    }

    #[test]
    fn would_accept_sell_uses_designated_money_good() {
        let mut stock = Stock::new(6);
        stock.add(FOOD, 1);
        stock.add(SALT, 1);
        let agent = market_agent(Gold::ZERO, stock, vec![good_want(SALT, Horizon::Later(1))]);

        assert!(agent.would_accept_sell_for_money(FOOD, 1, Gold(1), SALT));
        assert!(!agent.would_accept_sell_for_money(FOOD, 1, Gold::ZERO, SALT));
        assert!(!agent.would_accept_sell_for_money(SALT, 1, Gold(1), SALT));
    }

    #[test]
    fn reservation_bid_never_exceeds_free_gold() {
        let agent = market_agent(
            Gold(2),
            Stock::new(3),
            vec![
                good_want(GOLD, Horizon::Later(1)),
                good_want(FOOD, Horizon::Next),
            ],
        );

        assert_eq!(agent.reservation_bid(FOOD, 1), Some(Gold(1)));
    }

    #[test]
    fn reservation_bid_preserves_partial_higher_ranked_gold() {
        let agent = market_agent(
            Gold(1),
            Stock::new(3),
            vec![
                Want {
                    kind: WantKind::Good(GOLD),
                    horizon: Horizon::Later(1),
                    qty: 2,
                    satisfied: false,
                },
                good_want(FOOD, Horizon::Next),
            ],
        );

        assert_eq!(agent.reservation_bid(FOOD, 1), None);
    }

    #[test]
    fn reservation_ask_preserves_partial_higher_ranked_stock() {
        let mut stock = Stock::new(3);
        stock.add(FOOD, 1);
        let agent = market_agent(
            Gold::ZERO,
            stock,
            vec![
                Want {
                    kind: WantKind::Good(FOOD),
                    horizon: Horizon::Next,
                    qty: 2,
                    satisfied: false,
                },
                good_want(GOLD, Horizon::Later(1)),
            ],
        );

        assert_eq!(agent.reservation_ask(FOOD, 1), None);
    }

    #[test]
    fn later_consumer_wants_are_not_bid_targets() {
        let agent = market_agent(
            Gold(5),
            Stock::new(3),
            vec![good_want(WOOD, Horizon::Later(1))],
        );

        assert_eq!(agent.reservation_bid(WOOD, 1), None);
    }

    #[test]
    fn dynamic_money_good_wants_are_satisfied_from_money_balance() {
        let mut agent = market_agent(
            Gold(2),
            Stock::new(6),
            vec![Want {
                kind: WantKind::Good(SALT),
                horizon: Horizon::Now,
                qty: 2,
                satisfied: false,
            }],
        );

        agent.recompute_satisfaction_for_money(SALT);

        assert!(agent.scale[0].satisfied);
        assert_eq!(agent.stock.get(SALT), 0);
    }

    #[test]
    fn legacy_gold_wrappers_still_match_existing_behavior() {
        let mut stock = Stock::new(3);
        stock.add(FOOD, 2);
        stock.add(WOOD, 1);
        let agent = market_agent(
            Gold(3),
            stock,
            vec![
                good_want(GOLD, Horizon::Now),
                good_want(FOOD, Horizon::Now),
                good_want(WOOD, Horizon::Next),
                good_want(GOLD, Horizon::Later(1)),
            ],
        );
        let mut wrapper_agent = agent.clone();
        let mut dynamic_agent = agent.clone();

        wrapper_agent.recompute_satisfaction();
        dynamic_agent.recompute_satisfaction_for_money(GOLD);
        assert_eq!(wrapper_agent.scale, dynamic_agent.scale);

        let wrapper_consumption = wrapper_agent.consume_now_wants_with_provisions();
        let dynamic_consumption = dynamic_agent.consume_now_wants_with_provisions_for_money(GOLD);
        assert_eq!(wrapper_consumption, dynamic_consumption);
        assert_eq!(wrapper_agent.stock, dynamic_agent.stock);
        assert_eq!(wrapper_agent.scale, dynamic_agent.scale);

        assert_eq!(
            agent.reservation_bid(FOOD, 1),
            agent.reservation_bid_for_money(FOOD, 1, GOLD)
        );
        assert_eq!(
            agent.reservation_ask(FOOD, 1),
            agent.reservation_ask_for_money(FOOD, 1, GOLD)
        );
    }

    #[test]
    fn near_unsatisfied_goods_skip_later_before_deduping() {
        let agent = market_agent(
            Gold::ZERO,
            Stock::new(6),
            vec![
                good_want(SALT, Horizon::Later(1)),
                good_want(SALT, Horizon::Now),
                good_want(CLOTH, Horizon::Next),
                good_want(SALT, Horizon::Next),
            ],
        );

        assert_eq!(
            agent.near_unsatisfied_goods_without_money(),
            vec![SALT, CLOTH]
        );
    }

    #[test]
    fn near_unsatisfied_goods_ignore_already_satisfied_current_wants() {
        let agent = market_agent(
            Gold::ZERO,
            Stock::new(6),
            vec![
                Want {
                    kind: WantKind::Good(SALT),
                    horizon: Horizon::Now,
                    qty: 1,
                    satisfied: true,
                },
                good_want(CLOTH, Horizon::Next),
            ],
        );

        assert_eq!(agent.near_unsatisfied_goods_without_money(), vec![CLOTH]);
    }

    #[test]
    fn barter_swap_accepts_ordinal_improvement() {
        let mut stock = Stock::new(3);
        stock.add(FOOD, 1);
        let agent = market_agent(Gold::ZERO, stock, vec![good_want(WOOD, Horizon::Now)]);

        assert!(agent.would_accept_barter_swap(FOOD, WOOD, 1));
    }

    #[test]
    fn barter_swap_rejects_breaking_higher_ranked_provided_want() {
        let mut stock = Stock::new(3);
        stock.add(FOOD, 1);
        let agent = market_agent(
            Gold::ZERO,
            stock,
            vec![
                good_want(FOOD, Horizon::Next),
                good_want(WOOD, Horizon::Next),
            ],
        );

        assert!(!agent.would_accept_barter_swap(FOOD, WOOD, 1));
    }

    #[test]
    fn barter_swap_rejects_breaking_higher_ranked_partial_want() {
        let mut stock = Stock::new(3);
        stock.add(FOOD, 1);
        let agent = market_agent(
            Gold::ZERO,
            stock,
            vec![
                Want {
                    kind: WantKind::Good(FOOD),
                    horizon: Horizon::Next,
                    qty: 2,
                    satisfied: false,
                },
                good_want(WOOD, Horizon::Next),
            ],
        );

        assert!(!agent.would_accept_barter_swap(FOOD, WOOD, 1));
    }

    #[test]
    fn barter_reserved_stock_counts_satisfied_now_wants() {
        let mut stock = Stock::new(3);
        stock.add(FOOD, 1);
        let agent = market_agent(
            Gold::ZERO,
            stock,
            vec![
                Want {
                    kind: WantKind::Good(FOOD),
                    horizon: Horizon::Now,
                    qty: 1,
                    satisfied: true,
                },
                good_want(WOOD, Horizon::Next),
            ],
        );

        assert_eq!(agent.stock_reserved_for_near_wants_barter(FOOD), 1);
        assert!(!agent.would_accept_barter_swap(FOOD, WOOD, 1));
    }

    #[test]
    fn barter_swap_rejects_same_good_and_missing_stock() {
        let mut stock = Stock::new(6);
        stock.add(FOOD, 1);
        let agent = market_agent(
            Gold::ZERO,
            stock,
            vec![
                good_want(WOOD, Horizon::Now),
                good_want(CLOTH, Horizon::Next),
            ],
        );

        assert!(!agent.would_accept_barter_swap(FOOD, FOOD, 1));
        assert!(!agent.would_accept_barter_swap(SALT, CLOTH, 1));
    }

    #[test]
    fn barter_swap_treats_gold_as_ordinary_stock() {
        let mut stock = Stock::new(3);
        stock.add(FOOD, 1);
        let agent = market_agent(
            Gold::ZERO,
            stock,
            vec![Want {
                kind: WantKind::Good(GOLD),
                horizon: Horizon::Now,
                qty: 1,
                satisfied: false,
            }],
        );

        assert!(agent.would_accept_barter_swap(FOOD, GOLD, 1));
    }

    #[test]
    fn barter_swap_ignores_money_balance_for_stock_gold_wants() {
        let mut stock = Stock::new(3);
        stock.add(FOOD, 1);
        let agent = market_agent(
            Gold(1),
            stock,
            vec![
                good_want(GOLD, Horizon::Next),
                good_want(FOOD, Horizon::Next),
                good_want(GOLD, Horizon::Next),
            ],
        );

        assert!(agent.would_accept_barter_swap(FOOD, GOLD, 1));
    }

    fn market_agent(gold: Gold, stock: Stock, scale: Vec<Want>) -> Agent {
        Agent {
            id: AgentId(1),
            scale,
            stock,
            gold,
            labor_capacity: 0,
            hunger_deficit: 0,
            roles: vec![crate::agent::Role::Trader],
            expect: Vec::new(),
        }
    }

    fn good_want(good: crate::good::GoodId, horizon: Horizon) -> Want {
        Want {
            kind: WantKind::Good(good),
            horizon,
            qty: 1,
            satisfied: false,
        }
    }
}
