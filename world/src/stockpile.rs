//! `Stockpile` — a location that stores goods up to a total capacity.
//!
//! A stockpile holds integer quantities of goods in a `BTreeMap<GoodId, u32>`
//! (good-ordered, deterministic — no `HashMap`). `cap` is the total number of
//! units it can hold across *all* goods. A deposit clamps to the remaining room;
//! anything that does not fit stays with the depositor (carried, never
//! destroyed). Storing a good neither creates nor destroys a unit — it relocates
//! it from carry to the stockpile.

use std::collections::BTreeMap;

use econ::good::GoodId;

use crate::grid::Pos;

/// Stable stockpile identity within one `World` (index into the stockpile list).
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct StockpileId(pub u32);

/// A storage location with a total-unit capacity shared across all goods.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Stockpile {
    pub pos: Pos,
    pub cap: u32,
    contents: BTreeMap<GoodId, u32>,
}

impl Stockpile {
    /// An empty stockpile at `pos` with total capacity `cap`.
    pub fn new(pos: Pos, cap: u32) -> Self {
        Self {
            pos,
            cap,
            contents: BTreeMap::new(),
        }
    }

    /// Units of `good` currently stored.
    pub fn get(&self, good: GoodId) -> u32 {
        self.contents.get(&good).copied().unwrap_or(0)
    }

    /// Total units stored across all goods.
    pub fn total(&self) -> u32 {
        self.contents.values().sum()
    }

    /// Free capacity remaining (`cap - total`).
    pub fn room(&self) -> u32 {
        self.cap.saturating_sub(self.total())
    }

    /// Store up to `min(qty, room)` units of `good`, returning the amount
    /// accepted. Overflow (`qty - accepted`) is the caller's to keep carried; it
    /// is never destroyed here.
    pub fn deposit(&mut self, good: GoodId, qty: u32) -> u32 {
        let accepted = qty.min(self.room());
        if accepted > 0 {
            *self.contents.entry(good).or_insert(0) += accepted;
        }
        accepted
    }

    /// Remove up to `min(qty, get(good))` units of `good`, returning the amount
    /// removed. The mirror of [`Stockpile::deposit`]: a unit withdrawn is
    /// *relocated* to the caller, never destroyed here. This is the world side of
    /// the G2b world→econ transfer seam — the exchange stockpile is drained into
    /// econ stock once per econ tick (see `docs/engine-divergence.md`). It is an
    /// out-of-tick accessor; `World::tick` never calls it, so the G2a per-tick
    /// conservation receipt and every G2a test are untouched.
    pub fn withdraw(&mut self, good: GoodId, qty: u32) -> u32 {
        let Some(held) = self.contents.get_mut(&good) else {
            return 0;
        };
        let removed = qty.min(*held);
        *held -= removed;
        if *held == 0 {
            self.contents.remove(&good);
        }
        removed
    }

    /// Iterate stored `(good, qty)` pairs in `GoodId` order (deterministic).
    pub fn contents(&self) -> impl Iterator<Item = (GoodId, u32)> + '_ {
        self.contents.iter().map(|(&good, &qty)| (good, qty))
    }

    pub(crate) fn write_canonical(&self, out: &mut Vec<u8>) {
        out.extend_from_slice(&self.pos.x.to_le_bytes());
        out.extend_from_slice(&self.pos.y.to_le_bytes());
        out.extend_from_slice(&self.cap.to_le_bytes());
        out.extend_from_slice(&(self.contents.len() as u32).to_le_bytes());
        for (good, qty) in self.contents() {
            out.extend_from_slice(&good.0.to_le_bytes());
            out.extend_from_slice(&qty.to_le_bytes());
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{Stockpile, StockpileId};
    use crate::grid::Pos;
    use econ::good::{FOOD, WOOD};

    fn pile(cap: u32) -> Stockpile {
        Stockpile::new(Pos::new(0, 0), cap)
    }

    #[test]
    fn deposit_accumulates_and_reports_total() {
        let mut sp = pile(10);
        assert_eq!(sp.deposit(FOOD, 3), 3);
        assert_eq!(sp.deposit(FOOD, 2), 2);
        assert_eq!(sp.get(FOOD), 5);
        assert_eq!(sp.deposit(WOOD, 1), 1);
        assert_eq!(sp.total(), 6);
        assert_eq!(sp.room(), 4);
    }

    #[test]
    fn deposit_clamps_to_capacity_and_reports_overflow() {
        let mut sp = pile(5);
        // Only 5 fit; the deposit reports 5 accepted, the caller keeps the rest.
        assert_eq!(sp.deposit(FOOD, 8), 5);
        assert_eq!(sp.get(FOOD), 5);
        assert_eq!(sp.room(), 0);
        // A full stockpile accepts nothing more.
        assert_eq!(sp.deposit(WOOD, 3), 0);
        assert_eq!(sp.get(WOOD), 0);
        assert_eq!(sp.total(), 5);
    }

    #[test]
    fn capacity_is_shared_across_goods() {
        let mut sp = pile(4);
        assert_eq!(sp.deposit(FOOD, 3), 3);
        // Only one unit of room is left for any other good.
        assert_eq!(sp.deposit(WOOD, 3), 1);
        assert_eq!(sp.total(), 4);
    }

    #[test]
    fn contents_iterate_in_good_order() {
        let mut sp = pile(10);
        sp.deposit(WOOD, 2);
        sp.deposit(FOOD, 1);
        let goods: Vec<_> = sp.contents().map(|(g, _)| g).collect();
        assert_eq!(goods, vec![FOOD, WOOD]);
    }

    #[test]
    fn withdraw_relocates_up_to_held_and_conserves() {
        let mut sp = pile(10);
        sp.deposit(FOOD, 6);
        sp.deposit(WOOD, 2);

        // Withdraw fewer than held: exactly that many leave, the rest stay.
        assert_eq!(sp.withdraw(FOOD, 4), 4);
        assert_eq!(sp.get(FOOD), 2);
        assert_eq!(sp.total(), 4);

        // Withdraw more than held: clamps to what is there, empties the entry.
        assert_eq!(sp.withdraw(FOOD, 5), 2);
        assert_eq!(sp.get(FOOD), 0);
        assert_eq!(sp.total(), 2);

        // An absent good withdraws nothing, never underflows.
        assert_eq!(sp.withdraw(FOOD, 1), 0);
        assert_eq!(sp.get(WOOD), 2);
    }

    #[test]
    fn deposit_then_full_withdraw_round_trips_to_empty() {
        let mut sp = pile(5);
        assert_eq!(sp.deposit(FOOD, 5), 5);
        assert_eq!(sp.withdraw(FOOD, 5), 5);
        assert_eq!(sp.total(), 0);
        assert_eq!(sp.room(), 5, "room is restored after a full withdraw");
    }

    #[test]
    fn stockpile_id_orders_numerically() {
        assert!(StockpileId(0) < StockpileId(1));
    }
}
