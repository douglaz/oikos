//! Goods, horizons, and the `Stock` container.

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct GoodId(pub u16);

pub const GOLD: GoodId = GoodId(0);
pub const FOOD: GoodId = GoodId(1);
pub const WOOD: GoodId = GoodId(2);
pub const NET: GoodId = GoodId(3);
pub const SALT: GoodId = GoodId(4);
pub const CLOTH: GoodId = GoodId(5);
pub const ORE: GoodId = GoodId(6);

/// The lab-default good table: names in `GoodId` order, index `== GoodId.0`.
///
/// This is the single source of truth that the dynamic `GoodRegistry`
/// (`registry.rs`) interns for `lab_default()` and that `good_name` shims over.
/// The constants above are exactly these positions. Keeping it here lets
/// `good.rs` stay self-contained while the registry derives from it (G0b
/// divergence: goods become data, the constants/`good_name` stay as lab-compat
/// surface). Do not reorder — the four series goldens depend on these ids.
pub(crate) const LAB_GOOD_NAMES: [&str; 7] =
    ["gold", "food", "wood", "net", "salt", "cloth", "ore"];

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord)]
pub struct Gold(pub u64);

impl Gold {
    pub const ZERO: Gold = Gold(0);

    pub fn checked_add(self, other: Gold) -> Option<Gold> {
        self.0.checked_add(other.0).map(Gold)
    }

    pub fn checked_sub(self, other: Gold) -> Option<Gold> {
        self.0.checked_sub(other.0).map(Gold)
    }

    pub fn saturating_add(self, other: Gold) -> Gold {
        Gold(self.0.saturating_add(other.0))
    }

    pub fn saturating_sub(self, other: Gold) -> Gold {
        Gold(self.0.saturating_sub(other.0))
    }

    pub fn mul_qty(self, qty: u32) -> Option<Gold> {
        self.0.checked_mul(u64::from(qty)).map(Gold)
    }
}

/// Lab-compat name lookup: a thin shim over the lab-default table.
///
/// Retained as the display surface every series golden covers; registry-aware
/// callers prefer [`crate::registry::GoodRegistry::name`]. Not removed in G0b
/// (recorded in `docs/engine-divergence.md`).
pub fn good_name(good: GoodId) -> &'static str {
    LAB_GOOD_NAMES
        .get(usize::from(good.0))
        .copied()
        .unwrap_or("unknown")
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Horizon {
    Now,
    Next,
    Later(u8),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Stock {
    quantities: Vec<u32>,
}

impl Stock {
    pub fn new(max_good_id: u16) -> Self {
        Self {
            quantities: vec![0; usize::from(max_good_id) + 1],
        }
    }

    pub fn get(&self, good: GoodId) -> u32 {
        self.quantities
            .get(usize::from(good.0))
            .copied()
            .unwrap_or(0)
    }

    pub fn add(&mut self, good: GoodId, qty: u32) {
        let index = usize::from(good.0);
        if index >= self.quantities.len() {
            self.quantities.resize(index + 1, 0);
        }
        self.quantities[index] = self.quantities[index].saturating_add(qty);
    }

    pub fn remove(&mut self, good: GoodId, qty: u32) -> bool {
        if qty == 0 {
            return true;
        }

        let index = usize::from(good.0);
        let Some(quantity) = self.quantities.get_mut(index) else {
            return false;
        };
        if *quantity < qty {
            return false;
        }
        *quantity -= qty;
        true
    }

    pub fn can_remove(&self, good: GoodId, qty: u32) -> bool {
        self.get(good) >= qty
    }

    pub fn positive_goods(&self) -> impl Iterator<Item = GoodId> + '_ {
        self.quantities
            .iter()
            .enumerate()
            .filter(|(_, qty)| **qty > 0)
            .filter_map(|(index, _)| u16::try_from(index).ok().map(GoodId))
    }
}

#[cfg(test)]
mod tests {
    use super::{good_name, Gold, Stock, CLOTH, FOOD, GOLD, NET, ORE, SALT, WOOD};

    #[test]
    fn gold_checked_arithmetic_is_safe() {
        assert_eq!(Gold(2).checked_add(Gold(3)), Some(Gold(5)));
        assert_eq!(Gold(u64::MAX).checked_add(Gold(1)), None);
        assert_eq!(Gold(5).checked_sub(Gold(3)), Some(Gold(2)));
        assert_eq!(Gold(3).checked_sub(Gold(5)), None);
        assert_eq!(Gold(4).mul_qty(3), Some(Gold(12)));
        assert_eq!(Gold(u64::MAX).mul_qty(2), None);
    }

    #[test]
    fn stock_remove_is_atomic() {
        let mut stock = Stock::new(3);
        stock.add(FOOD, 1);

        assert!(!stock.remove(FOOD, 2));
        assert_eq!(stock.get(FOOD), 1);

        assert!(stock.remove(FOOD, 1));
        assert_eq!(stock.get(FOOD), 0);
    }

    #[test]
    fn removing_zero_from_unknown_good_is_noop() {
        let mut stock = Stock::new(3);

        assert!(stock.remove(super::GoodId(99), 0));
        assert_eq!(stock.get(FOOD), 0);
    }

    #[test]
    fn v2_good_names_are_stable() {
        assert_eq!(good_name(GOLD), "gold");
        assert_eq!(good_name(FOOD), "food");
        assert_eq!(good_name(WOOD), "wood");
        assert_eq!(good_name(NET), "net");
        assert_eq!(good_name(SALT), "salt");
        assert_eq!(good_name(CLOTH), "cloth");
        assert_eq!(good_name(ORE), "ore");
    }

    #[test]
    fn stock_reports_positive_goods_in_id_order() {
        let mut stock = Stock::new(6);
        stock.add(CLOTH, 2);
        stock.add(FOOD, 1);

        assert_eq!(
            stock.positive_goods().collect::<Vec<_>>(),
            vec![FOOD, CLOTH]
        );
    }
}
