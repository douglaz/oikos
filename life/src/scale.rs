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
/// S10 (per-agent capital, gated): the deepest future-savings horizon **level** the
/// multi-horizon ladder spans. Level `L` (1-indexed) emits savings wants at
/// `Later(FUTURE_HORIZON * L)` — so `MAX_LADDER_DEPTH = 4` reaches `Later(16)`. Depth 1
/// is the base [`FUTURE_HORIZON`]-only ladder every off-path scale keeps. Only the deep
/// (`regenerate_scale_for_capital`) variant ever emits beyond level 1.
const MAX_LADDER_DEPTH: u8 = 4;
/// S10: savings units emitted at each DEEPER (`level >= 2`) horizon of the multi-horizon
/// ladder — a lean mini-ladder (just the deep wants a built tool's late receipts can
/// fill), kept small so the deep scale stays bounded. The base `Later(4)` level keeps
/// its full [`save_units`] count, so the deep ladder's level-1 block is byte-identical to
/// the single-horizon ladder.
const DEEP_HORIZON_UNITS: u16 = 4;
/// S10: a tiny per-level urgency nudge so a deeper horizon's `u`-th unit ranks just below
/// the same unit of a shallower horizon — a deterministic tiebreak that keeps each
/// horizon's top unit near the base urgency (so a patient colonist's deep savings want
/// can still outrank its leisure), never a cardinal magnitude.
const HORIZON_LEVEL_OFFSET: i64 = 1;
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
    regenerate_scale_inner(needs, culture, known, false)
}

/// S10 (per-agent capital, gated): a colonist's value scale with the savings ladder
/// extended to **multiple future horizons** (`Later(4), Later(8), …`) up to a depth set
/// by the colonist's own `time_preference_bps` ([`savings_ladder_depth`]). Identical to
/// [`regenerate_scale`] except for the deeper savings wants: the `Later(4)` block is
/// byte-identical, and a present-biased colonist (depth 1) gets no deeper wants at all,
/// so the two only diverge when patience actually warrants a deeper horizon. The deep
/// wants are what a durable tool's gestation-delayed receipt stream can fill — so a
/// patient colonist's late-due savings can be provisioned by a build it appraises while a
/// present-biased one's shallow near-savings cannot (the originary-interest response).
/// Only the `per_agent_capital` path calls this, so every existing scale is unchanged.
pub fn regenerate_scale_for_capital(
    needs: &NeedState,
    culture: &CultureParams,
    known: &KnownGoods,
) -> Vec<Want> {
    regenerate_scale_inner(needs, culture, known, true)
}

fn regenerate_scale_inner(
    needs: &NeedState,
    culture: &CultureParams,
    known: &KnownGoods,
    deep_savings: bool,
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
    if deep_savings {
        push_multi_horizon_future_ladder(&mut items, WantKind::Good(known.savings), culture);
    } else {
        push_future_ladder(&mut items, WantKind::Good(known.savings), culture);
    }

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

/// S10: the per-agent-capital **multi-horizon** savings ladder. Each horizon level
/// `L` (1-indexed) emits savings wants at `Later(FUTURE_HORIZON * L)` — level 1 the full
/// [`save_units`] count at the base `Later(4)` (byte-identical to [`push_future_ladder`]),
/// deeper levels a lean [`DEEP_HORIZON_UNITS`] mini-ladder each. Every level restarts its
/// urgency near the base (minus a tiny [`HORIZON_LEVEL_OFFSET`] per level), so a patient
/// colonist's deep savings wants sit near the top of the savings block — able to outrank
/// its leisure — while a present-biased colonist (depth 1) gets only the shallow base
/// ladder. The ladder DEPTH tracks `time_preference_bps` ([`savings_ladder_depth`]): that
/// is what makes a built tool's late-due receipts (which a [`FUTURE_HORIZON`]-deep
/// gestation pushes past `Later(4)`) provision a patient colonist's deep want while
/// missing a present-biased one's shallow one — the structural originary-interest bite.
fn push_multi_horizon_future_ladder(
    items: &mut Vec<Item>,
    kind: WantKind,
    culture: &CultureParams,
) {
    let penalty = (i64::from(culture.time_preference_bps) * URGENCY_UNIT) / 2_500;
    let base_units = save_units(culture.time_preference_bps);
    let depth = savings_ladder_depth(culture.time_preference_bps);
    let mut unit_key: u16 = 0;
    for level in 0..depth {
        let horizon = FUTURE_HORIZON.saturating_mul(level + 1);
        let units = if level == 0 {
            base_units
        } else {
            DEEP_HORIZON_UNITS.min(base_units)
        };
        for unit in 0..units {
            let urgency = FUTURE_BASE_LEVEL * URGENCY_UNIT
                - penalty
                - i64::from(unit) * URGENCY_UNIT
                - i64::from(level) * HORIZON_LEVEL_OFFSET;
            items.push(Item {
                urgency,
                channel: Channel::Savings,
                unit: unit_key,
                want: want(kind, Horizon::Later(horizon)),
            });
            unit_key = unit_key.saturating_add(1);
        }
    }
}

/// S10: the per-agent-capital savings-ladder DEPTH (count of future horizons) as a
/// function of time preference: patience (low bps) raises it from `1` (a present-biased
/// colonist — only the base `Later(4)`) toward [`MAX_LADDER_DEPTH`] (a patient colonist —
/// `Later(4), Later(8), …`). Mirrors [`save_units`]'s patience math exactly (the same
/// `10_000 − tp` patience scaled into an integer range), so the deep ladder a colonist
/// carries — and therefore whether a built tool's late receipts can fill one of its
/// savings wants — is a deterministic function of its own scale, never a cardinal
/// discount. Pure and integer.
pub fn savings_ladder_depth(time_preference_bps: u16) -> u8 {
    let patience = 10_000u32.saturating_sub(u32::from(time_preference_bps.min(10_000)));
    let extra = patience * (u32::from(MAX_LADDER_DEPTH) - 1) / 10_000;
    1 + u8::try_from(extra).unwrap_or(MAX_LADDER_DEPTH - 1)
}

/// The deepest `Later(n)` horizon the gated multi-horizon savings ladder can emit.
pub fn max_savings_ladder_horizon() -> u64 {
    u64::from(FUTURE_HORIZON) * u64::from(MAX_LADDER_DEPTH)
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
    use super::{
        max_savings_ladder_horizon, regenerate_scale, regenerate_scale_for_capital,
        savings_ladder_depth, KnownGoods, FUTURE_HORIZON, MAX_LADDER_DEPTH,
    };
    use crate::culture::CultureParams;
    use crate::need::NeedState;
    use econ::agent::WantKind;
    use econ::good::{Horizon, FOOD, GOLD, WOOD};

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

    #[test]
    fn savings_ladder_depth_tracks_time_preference() {
        // S10: patience (low bps) deepens the ladder; the most present-biased colonist
        // gets only the base horizon, the most patient the full depth — deterministic,
        // monotone non-increasing in time preference.
        assert_eq!(
            savings_ladder_depth(10_000),
            1,
            "max present bias -> base only"
        );
        assert_eq!(
            savings_ladder_depth(0),
            MAX_LADDER_DEPTH,
            "max patience -> full"
        );
        assert_eq!(
            max_savings_ladder_horizon(),
            u64::from(FUTURE_HORIZON) * u64::from(MAX_LADDER_DEPTH),
            "the exported maximum horizon must match the ladder constants"
        );
        // Present-biased (8000): only the base Later(4) horizon (depth 1).
        assert_eq!(savings_ladder_depth(8_000), 1);
        // Patient (2000): a deeper ladder that reaches at least Later(8) (depth >= 2).
        assert!(savings_ladder_depth(2_000) >= 2);
        // Monotone: more present bias never deepens the ladder.
        for tp in 0..=10_000u16 {
            if tp > 0 {
                assert!(savings_ladder_depth(tp) <= savings_ladder_depth(tp - 1));
            }
            assert!((1..=MAX_LADDER_DEPTH).contains(&savings_ladder_depth(tp)));
        }
    }

    #[test]
    fn deep_savings_ladder_preserves_the_base_and_adds_deeper_horizons() {
        // S10: the deep variant's Later(4) block is byte-identical to the single-horizon
        // ladder (the off-path scale is untouched), and a patient colonist additionally
        // carries deeper-horizon savings wants the base ladder never emits.
        let needs = NeedState::rested();
        let known = KnownGoods::lab_default();

        // Present-biased (depth 1): the deep variant equals the base variant exactly.
        let biased = CultureParams::new(8_000, 3_000);
        assert_eq!(
            regenerate_scale(&needs, &biased, &known),
            regenerate_scale_for_capital(&needs, &biased, &known),
            "a depth-1 colonist's deep scale must equal its base scale"
        );

        // Patient (deep): the base Later(4) savings wants are all still present, and at
        // least one deeper-horizon (Later(>4)) savings want is added.
        let patient = CultureParams::new(400, 3_000);
        let base = regenerate_scale(&needs, &patient, &known);
        let deep = regenerate_scale_for_capital(&needs, &patient, &known);
        let base_later4 = base
            .iter()
            .filter(|w| w.kind == WantKind::Good(GOLD) && w.horizon == Horizon::Later(4))
            .count();
        let deep_later4 = deep
            .iter()
            .filter(|w| w.kind == WantKind::Good(GOLD) && w.horizon == Horizon::Later(4))
            .count();
        assert_eq!(
            deep_later4, base_later4,
            "the base Later(4) ladder is preserved"
        );
        assert!(
            deep.iter().any(|w| w.kind == WantKind::Good(GOLD)
                && matches!(w.horizon, Horizon::Later(h) if h > 4)),
            "a patient colonist's deep ladder must carry a deeper-than-Later(4) savings want"
        );
        // The deeper savings want sits near the TOP of the savings block (above leisure)
        // — so a built tool's late receipts that provision it can still outrank the
        // leisure the build sacrifices.
        let first_deep = deep
            .iter()
            .position(|w| {
                w.kind == WantKind::Good(GOLD) && matches!(w.horizon, Horizon::Later(h) if h > 4)
            })
            .expect("a deep savings want");
        let first_leisure = deep
            .iter()
            .position(|w| w.kind == WantKind::Leisure)
            .expect("leisure is always present");
        assert!(
            first_deep < first_leisure,
            "a patient rested colonist's first deep savings want must outrank its leisure"
        );
    }
}
