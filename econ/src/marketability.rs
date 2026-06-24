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

#[derive(Clone, Copy, Debug)]
pub struct MarketabilityAcceptance<'a> {
    pub durability_aware_acceptance: bool,
    pub config: &'a MarketabilityConfig,
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

    /// Binary holding rule: can `qty` of `good` plausibly carry through the
    /// holding horizon as a medium?
    ///
    /// `decay_bps` and `carry_cost` are the physical penalty terms that place a
    /// good on the durable↔perishable axis, but the acceptance decision itself is
    /// intentionally binary (the S21a lever returns `bool`): the good is acceptable
    /// only if what survives its per-tick decay over `hold_horizon`, net of the
    /// carry burden, is no less than what was received. A durable, zero-carry
    /// medium passes; a perishable one (decay erodes it) or a durable-but-carried
    /// one (the burden eats into it) fails.
    pub fn can_cover_holding_period(&self, good: GoodId, qty: u32) -> bool {
        let attrs = self.good(good);
        let surviving = surviving_quantity(qty, attrs.decay_bps, self.hold_horizon);
        surviving.saturating_sub(attrs.carry_cost) >= qty
    }
}

fn surviving_quantity(qty: u32, decay_bps: u16, hold_horizon: u32) -> u32 {
    // A non-decaying medium (or a zero-length horizon) carries through unchanged,
    // so skip the per-tick loop — a long horizon on a durable good stays O(1).
    if qty == 0 {
        return 0;
    }
    if decay_bps == 0 || hold_horizon == 0 {
        return qty;
    }

    const BPS_SCALE: u128 = 10_000;
    let retain_bps = u128::from(10_000u16.saturating_sub(decay_bps.min(10_000)));
    let mut quantity = u128::from(qty);
    for _ in 0..hold_horizon {
        quantity = quantity.saturating_mul(retain_bps) / BPS_SCALE;
        // A decaying medium can only shrink, so once it hits zero the rest of the
        // horizon cannot change it — bounding the loop for any genuinely perishable
        // good regardless of how long the horizon is.
        if quantity == 0 {
            break;
        }
    }
    u32::try_from(quantity).unwrap_or(u32::MAX)
}
