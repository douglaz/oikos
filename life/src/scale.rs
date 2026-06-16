//! The milestone: `regenerate_scale` — a colonist's ordinal value scale,
//! GENERATED from need state each tick rather than authored once.
//!
//! This is the single most important transformation the game makes to the lab
//! engine (game-spec §4.3, §5.4). praxsim treats `Agent.scale` as a fixture; in
//! the colony the scale is regenerated every tick from hunger/warmth/rest. The
//! function is **pure and deterministic** — no RNG, no clock, no global state —
//! and produces a `Vec<Want>` in strict descending urgency with each marginal
//! unit listed separately, so diminishing marginal utility is baked in by
//! position with no cardinal magnitude anywhere.
//!
//! Construction (all integer, ordinal): every want is an *item* with an integer
//! `urgency` on one shared axis and a deterministic tiebreak. Items are sorted
//! by descending urgency (ties broken by a fixed channel/unit key), and the
//! resulting position *is* the ordinal rank.
//!
//! - a consumable need with deficit `d` emits up to `MAX_PRESENT_UNITS`
//!   present (`Now`) units, the k-th at urgency `(d-k)·UNIT` — more depletion
//!   means more units AND a higher top unit (satiation monotonicity), and
//!   successive units rank strictly lower (positional DMU);
//! - the colonist also emits future-provisioning (`Later`) units for the
//!   *savings* good — money is the generalized future good, the precautionary
//!   claim on goods not yet needed. Their count is the saving target (see
//!   `save_units`) and their urgency falls as `time_preference_bps` rises.
//!   This is how culture orders horizons, structurally, with no discount rate;
//!   it is also what makes a colonist sell its surplus into the real econ market
//!   (the lab's M1 agents save the money good for exactly this reason), so trade
//!   is emergent rather than scripted;
//! - rest emits at least one Leisure unit (always present, so labor supply stays
//!   emergent), ranked from the rest deficit plus a `leisure_weight_bps` bonus.

use econ::agent::{Want, WantKind};
use econ::good::{GoodId, Horizon, FOOD, GOLD, WOOD};

use crate::culture::CultureParams;
use crate::need::NeedState;

/// Spacing between adjacent depletion levels on the urgency axis.
const URGENCY_UNIT: i64 = 1_000;
/// Cap on present marginal units emitted per consumable need (bounds scale size;
/// higher deficits raise urgency rather than add unbounded units).
const MAX_PRESENT_UNITS: u16 = 4;
/// Bounds on the money-saving target (the number of single-unit `Later` savings
/// wants). Patience raises it: a present-biased colonist wants to hold little
/// money (it saves only [`MIN_SAVE_UNITS`]), a patient one wants to hold much
/// ([`MAX_SAVE_UNITS`]). This is the structural form of time preference acting on
/// savings — no discount rate. The behavioral consequence in the camp: a
/// present-biased colonist fills its small target and trades only to replace what
/// it spends (gold circulates in balance, the camp runs indefinitely), whereas a
/// patient colonist's target stays unmet so it keeps offering surplus, which lets
/// buyer competition discover a food price that climbs under scarcity. Unit lots
/// keep the book liquid — a seller's reservation to part with surplus is one
/// money unit — so a gold-poor seller never prices itself out.
const MIN_SAVE_UNITS: u16 = 4;
const MAX_SAVE_UNITS: u16 = 60;
/// Horizon assigned to future-provisioning wants.
const FUTURE_HORIZON: u8 = 4;
/// Nominal level (in `URGENCY_UNIT`s) of the top future unit at zero present
/// bias; `time_preference_bps` subtracts from here.
const FUTURE_BASE_LEVEL: i64 = 3;
/// Tiny within-level offsets so present consumption edges out same-level future
/// provisioning and the channels order deterministically.
const HUNGER_OFFSET: i64 = 6;
const WARMTH_OFFSET: i64 = 5;
/// Within-level offset for the subsistence-food fallback. Below [`HUNGER_OFFSET`]
/// so the preferred staple outranks the subsistence food at each depletion level
/// (a colonist reaches for bread before raw grain), while still ranking with the
/// present-consumption band so a hungry colonist eats to survive rather than
/// hoard. Only emitted when [`KnownGoods::subsistence`] is `Some`.
const SUBSISTENCE_OFFSET: i64 = 4;

/// The goods a colonist knows satisfy which need. In G1 this is the fixed lab
/// mapping (hunger ↔ FOOD, warmth ↔ fuel/WOOD); rest is satisfied by Leisure,
/// which is a `WantKind`, not a good. `savings` is the store-of-value good used
/// for generalized future provisioning (the money good, GOLD) — money as the
/// future good, not a need of its own. Shelter/social/security are out of scope
/// until they have goods/buildings to satisfy them (G2/G3).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct KnownGoods {
    pub hunger: GoodId,
    pub warmth: GoodId,
    pub savings: GoodId,
    /// An optional **subsistence** food that also satisfies hunger, ranked just
    /// below the preferred `hunger` staple. `None` (the lab default and every
    /// existing scenario) means there is no fallback — hunger is satisfied only
    /// by `hunger`, exactly as before, so scales stay byte-identical. `Some(g)`
    /// adds a directly-edible floor: a colonist prefers the `hunger` staple but
    /// will acquire and eat `g` to survive when the staple is unavailable (e.g.
    /// raw grain when the bread chain has stalled). The colony game uses this to
    /// make a roundabout food chain *optional specialization on top of* a
    /// subsistence base, instead of the sole food source.
    pub subsistence: Option<GoodId>,
}

impl KnownGoods {
    /// The lab mapping: hunger → FOOD, warmth → WOOD, savings → GOLD, no
    /// subsistence fallback.
    pub const fn lab_default() -> Self {
        Self {
            hunger: FOOD,
            warmth: WOOD,
            savings: GOLD,
            subsistence: None,
        }
    }
}

impl Default for KnownGoods {
    fn default() -> Self {
        Self::lab_default()
    }
}

/// Deterministic tiebreak channels, ordered so that at equal urgency present
/// consumption precedes future provisioning and the order is stable.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum Channel {
    HungerNow = 0,
    /// The optional subsistence-food fallback (see [`KnownGoods::subsistence`]).
    /// Ordered immediately after `HungerNow` so that at equal urgency the
    /// preferred staple outranks the subsistence food. Inserting it here keeps
    /// the relative order of the other channels unchanged, so scenarios with no
    /// subsistence good (every existing one) emit no items on this channel and
    /// stay byte-identical.
    SubsistenceNow = 1,
    WarmthNow = 2,
    Leisure = 3,
    Savings = 4,
}

struct Item {
    urgency: i64,
    channel: Channel,
    unit: u16,
    want: Want,
}

/// Generate a colonist's value scale from its need state and culture.
///
/// The output is in strict descending urgency (index 0 = most urgent), each
/// marginal unit listed separately, Leisure always present, satiation-monotone,
/// and never empty. Pure: identical inputs → identical output.
pub fn regenerate_scale(
    needs: &NeedState,
    culture: &CultureParams,
    known: &KnownGoods,
) -> Vec<Want> {
    let mut items: Vec<Item> = Vec::new();

    push_present_ladder(
        &mut items,
        Channel::HungerNow,
        WantKind::Good(known.hunger),
        needs.hunger,
        HUNGER_OFFSET,
    );
    // Optional subsistence-food fallback: a parallel hunger ladder for the
    // directly-edible food, ranked just below the preferred staple at each level.
    // A no-op (byte-identical) when `subsistence` is `None` or equals the staple.
    if let Some(subsistence) = known.subsistence {
        if subsistence != known.hunger {
            push_present_ladder(
                &mut items,
                Channel::SubsistenceNow,
                WantKind::Good(subsistence),
                needs.hunger,
                SUBSISTENCE_OFFSET,
            );
        }
    }
    push_present_ladder(
        &mut items,
        Channel::WarmthNow,
        WantKind::Good(known.warmth),
        needs.warmth,
        WARMTH_OFFSET,
    );
    push_leisure_ladder(&mut items, needs.rest, culture);
    push_future_ladder(&mut items, WantKind::Good(known.savings), culture);

    // Strict descending urgency; the (channel, unit) key is unique per item, so
    // the order is a deterministic total order.
    items.sort_by(|a, b| {
        b.urgency
            .cmp(&a.urgency)
            .then(a.channel.cmp(&b.channel))
            .then(a.unit.cmp(&b.unit))
    });

    items.into_iter().map(|item| item.want).collect()
}

/// Present consumption units: the k-th unit at urgency `(deficit-k)·UNIT`, for
/// `k` up to `min(deficit, MAX_PRESENT_UNITS)`. None when the need is satisfied.
fn push_present_ladder(
    items: &mut Vec<Item>,
    channel: Channel,
    kind: WantKind,
    deficit: u16,
    offset: i64,
) {
    let count = deficit.min(MAX_PRESENT_UNITS);
    for unit in 0..count {
        let level = i64::from(deficit) - i64::from(unit);
        items.push(Item {
            urgency: level * URGENCY_UNIT + offset,
            channel,
            unit,
            want: want(kind, Horizon::Now),
        });
    }
}

/// Leisure units from the rest deficit and the cultural leisure weight. Always
/// emits at least one, so labor supply stays emergent (Leisure is on the scale
/// even when fully rested, just ranked low).
fn push_leisure_ladder(items: &mut Vec<Item>, rest: u16, culture: &CultureParams) {
    let bonus = (i64::from(culture.leisure_weight_bps) * URGENCY_UNIT) / 10_000;
    let count = rest.clamp(1, MAX_PRESENT_UNITS);
    for unit in 0..count {
        let level = i64::from(rest) - i64::from(unit);
        items.push(Item {
            urgency: level * URGENCY_UNIT + bonus,
            channel: Channel::Leisure,
            unit,
            want: want(WantKind::Leisure, Horizon::Now),
        });
    }
}

/// Future-provisioning (savings) units. Their urgency falls as
/// `time_preference_bps` rises, pushing the future wants below present ones — the
/// structural horizon ordering, with no discount magnitude. This standing money
/// demand is what makes a colonist sell its surplus into the econ market.
fn push_future_ladder(items: &mut Vec<Item>, kind: WantKind, culture: &CultureParams) {
    // 0..=10_000 bps maps to a 0..=4·UNIT penalty, so a maximally present-biased
    // colonist sinks every future unit below all present consumption.
    let penalty = (i64::from(culture.time_preference_bps) * URGENCY_UNIT) / 2_500;
    let units = save_units(culture.time_preference_bps);
    for unit in 0..units {
        let urgency = FUTURE_BASE_LEVEL * URGENCY_UNIT - penalty - i64::from(unit) * URGENCY_UNIT;
        items.push(Item {
            urgency,
            channel: Channel::Savings,
            unit,
            want: want(kind, Horizon::Later(FUTURE_HORIZON)),
        });
    }
}

/// The saving target (count of unit savings wants) as a function of time
/// preference: patience (low bps) raises it from [`MIN_SAVE_UNITS`] toward
/// [`MAX_SAVE_UNITS`]. A present-biased colonist saves little — it fills its
/// small target and then trades only to replace what it spends, so a closed camp
/// circulates gold in balance and runs indefinitely. A patient colonist's target
/// stays unmet, so it keeps offering its surplus; with the food buyers leading
/// the book that sustained supply lets buyer competition discover a food price
/// that climbs when a harvest shock makes the hungry bid harder. Pure and integer.
fn save_units(time_preference_bps: u16) -> u16 {
    let patience = 10_000u32.saturating_sub(u32::from(time_preference_bps.min(10_000)));
    let span = MAX_SAVE_UNITS - MIN_SAVE_UNITS;
    MIN_SAVE_UNITS + u16::try_from(patience * u32::from(span) / 10_000).unwrap_or(span)
}

fn want(kind: WantKind, horizon: Horizon) -> Want {
    Want {
        kind,
        horizon,
        qty: 1,
        satisfied: false,
    }
}

#[cfg(test)]
mod tests {
    use super::{regenerate_scale, KnownGoods};
    use crate::culture::CultureParams;
    use crate::need::NeedState;
    use econ::agent::WantKind;
    use econ::good::{FOOD, WOOD};

    fn first_rank(scale: &[econ::agent::Want], kind: WantKind) -> Option<usize> {
        scale.iter().position(|want| want.kind == kind)
    }

    #[test]
    fn never_empty_and_always_has_leisure_when_fully_satisfied() {
        let scale = regenerate_scale(
            &NeedState::rested(),
            &CultureParams::lab_default(),
            &KnownGoods::lab_default(),
        );
        assert!(!scale.is_empty());
        assert!(scale.iter().any(|want| want.kind == WantKind::Leisure));
    }

    #[test]
    fn hunger_outranks_when_hungrier_than_rested() {
        let scale = regenerate_scale(
            &NeedState::new(6, 0, 0),
            &CultureParams::lab_default(),
            &KnownGoods::lab_default(),
        );
        let food = first_rank(&scale, WantKind::Good(FOOD)).expect("food want present");
        let leisure = first_rank(&scale, WantKind::Leisure).expect("leisure present");
        assert!(food < leisure);
    }

    #[test]
    fn warmth_want_uses_known_good() {
        let scale = regenerate_scale(
            &NeedState::new(0, 4, 0),
            &CultureParams::lab_default(),
            &KnownGoods::lab_default(),
        );
        assert!(scale.iter().any(|want| want.kind == WantKind::Good(WOOD)));
    }
}
