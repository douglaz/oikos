//! `ResourceNode` — a location that holds a stock of one good and (optionally)
//! regenerates it.
//!
//! A node is the *only* source of goods in the world: its `regen_per_tick`
//! (clamped to `cap`) is the single place a unit is created, and that creation is
//! fully accounted in the per-tick conservation report (see `world.rs`).
//! Harvesting moves units out of the node into an agent's carry — it never
//! creates or destroys a unit, it relocates it.

use econ::good::GoodId;

use crate::grid::Pos;

/// Stable node identity within one `World` (index into the node list).
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct NodeId(pub u32);

/// A resource deposit at a fixed tile: a stock of one `GoodId` that depletes when
/// harvested and refills by `regen_per_tick` up to `cap`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ResourceNode {
    pub pos: Pos,
    pub good: GoodId,
    pub stock: u32,
    pub regen_per_tick: u32,
    pub cap: u32,
}

impl ResourceNode {
    /// Build a node, clamping the initial `stock` to `cap` so the regen
    /// invariant (`stock <= cap`) holds from the start and never underflows.
    pub fn new(pos: Pos, good: GoodId, stock: u32, regen_per_tick: u32, cap: u32) -> Self {
        Self {
            pos,
            good,
            stock: stock.min(cap),
            regen_per_tick,
            cap,
        }
    }

    /// Move up to `min(want, stock, carry_room)` units out of the node, returning
    /// the amount actually removed. The node's stock drops by exactly that; the
    /// caller adds exactly that to its carry — so the world total is unchanged by
    /// a harvest (relocation, not creation).
    pub fn harvest(&mut self, want: u32, carry_room: u32) -> u32 {
        let moved = want.min(self.stock).min(carry_room);
        self.stock -= moved;
        moved
    }

    /// Regenerate at tick end: `stock = min(cap, stock + regen_per_tick)`.
    /// Returns the number of units *created* (the increase) — the only positive
    /// contribution to the world good total, recorded in the per-tick report.
    pub fn regen(&mut self) -> u32 {
        let new = self.stock.saturating_add(self.regen_per_tick).min(self.cap);
        let created = new.saturating_sub(self.stock);
        self.stock = new;
        created
    }

    pub(crate) fn write_canonical(&self, out: &mut Vec<u8>) {
        out.extend_from_slice(&self.pos.x.to_le_bytes());
        out.extend_from_slice(&self.pos.y.to_le_bytes());
        out.extend_from_slice(&self.good.0.to_le_bytes());
        out.extend_from_slice(&self.stock.to_le_bytes());
        out.extend_from_slice(&self.regen_per_tick.to_le_bytes());
        out.extend_from_slice(&self.cap.to_le_bytes());
    }
}

#[cfg(test)]
mod tests {
    use super::{NodeId, ResourceNode};
    use crate::grid::Pos;
    use econ::good::FOOD;

    fn node(stock: u32, regen: u32, cap: u32) -> ResourceNode {
        ResourceNode::new(Pos::new(0, 0), FOOD, stock, regen, cap)
    }

    #[test]
    fn new_clamps_initial_stock_to_cap() {
        assert_eq!(node(100, 0, 10).stock, 10);
        assert_eq!(node(5, 0, 10).stock, 5);
    }

    #[test]
    fn harvest_is_limited_by_want_stock_and_room() {
        // Want is the binding constraint.
        let mut n = node(10, 0, 10);
        assert_eq!(n.harvest(3, 10), 3);
        assert_eq!(n.stock, 7);

        // Stock is the binding constraint.
        let mut n = node(2, 0, 10);
        assert_eq!(n.harvest(5, 10), 2);
        assert_eq!(n.stock, 0);

        // Carry room is the binding constraint.
        let mut n = node(10, 0, 10);
        assert_eq!(n.harvest(8, 4), 4);
        assert_eq!(n.stock, 6);
    }

    #[test]
    fn harvest_from_empty_node_moves_nothing() {
        let mut n = node(0, 0, 10);
        assert_eq!(n.harvest(5, 5), 0);
        assert_eq!(n.stock, 0);
    }

    #[test]
    fn regen_clamps_to_cap_and_reports_the_increase() {
        let mut n = node(0, 3, 10);
        assert_eq!(n.regen(), 3);
        assert_eq!(n.stock, 3);

        // Near the cap, only the room-up-to-cap is created.
        let mut n = node(8, 5, 10);
        assert_eq!(n.regen(), 2);
        assert_eq!(n.stock, 10);

        // At the cap, nothing is created.
        let mut n = node(10, 5, 10);
        assert_eq!(n.regen(), 0);
        assert_eq!(n.stock, 10);
    }

    #[test]
    fn zero_regen_node_never_creates() {
        let mut n = node(4, 0, 10);
        assert_eq!(n.regen(), 0);
        assert_eq!(n.stock, 4);
    }

    #[test]
    fn node_id_orders_numerically() {
        assert!(NodeId(1) < NodeId(2));
    }
}
