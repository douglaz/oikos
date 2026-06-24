//! Physical marketability signals used by indirect barter acceptance.

use std::collections::BTreeMap;

use crate::good::GoodId;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct GoodMarketability {
    /// Per-holding-tick physical decay in basis points.
    pub decay_bps: u16,
    /// Quantity-equivalent carry burden over the configured holding horizon.
    pub carry_cost: u32,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct MarketabilityConfig {
    /// Fixed minimum holding period before a prospective medium can plausibly clear.
    pub hold_horizon: u32,
    pub goods: BTreeMap<GoodId, GoodMarketability>,
}

impl MarketabilityConfig {
    pub fn good(&self, good: GoodId) -> GoodMarketability {
        self.goods.get(&good).copied().unwrap_or_default()
    }

    pub fn with_good(mut self, good: GoodId, marketability: GoodMarketability) -> Self {
        self.goods.insert(good, marketability);
        self
    }

    pub fn is_empty_durable_default(&self) -> bool {
        self.hold_horizon == 0 && self.goods.is_empty()
    }
}
