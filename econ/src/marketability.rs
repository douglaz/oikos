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

    pub fn can_cover_holding_period(&self, good: GoodId, qty: u32) -> bool {
        let attrs = self.good(good);
        let surviving = surviving_quantity(qty, attrs.decay_bps, self.hold_horizon);
        surviving.saturating_sub(attrs.carry_cost) >= qty
    }
}

fn surviving_quantity(qty: u32, decay_bps: u16, hold_horizon: u32) -> u32 {
    const BPS_SCALE: u128 = 10_000;

    let retain_bps = u128::from(10_000u16.saturating_sub(decay_bps.min(10_000)));
    let mut quantity = u128::from(qty);
    for _ in 0..hold_horizon {
        quantity = quantity.saturating_mul(retain_bps) / BPS_SCALE;
        if quantity == 0 {
            break;
        }
    }
    u32::try_from(quantity).unwrap_or(u32::MAX)
}
