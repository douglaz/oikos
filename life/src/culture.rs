//! `CultureParams` — the per-colonist value-shaping constants.
//!
//! Culture is **structural**, not cardinal: these knobs shift *where* a want
//! lands in the ordinal scale relative to others, never a utility magnitude.
//! There is no discount rate and no scalar utility anywhere (the lab's purism,
//! preserved — game-spec §5.4). In G1 they are per-colonist constants set at
//! generation; G4 makes them heritable.

/// Per-colonist cultural value-shaping. Integer basis points only.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CultureParams {
    /// Present-bias: higher pushes `Later`-horizon (future-provisioning) wants
    /// further *down* the scale relative to present ones. It moves rank, not a
    /// discounted magnitude.
    pub time_preference_bps: u16,
    /// Leisure weight: higher makes the rest-derived Leisure want outrank goods
    /// *sooner* (at a lower rest deficit). This is the cultural dial on how
    /// readily a colonist trades work for rest — labor supply stays emergent.
    pub leisure_weight_bps: u16,
}

impl CultureParams {
    /// A neutral, present-leaning culture: a mild leisure weight and a moderate
    /// present bias. A reasonable lab default; not a balance target.
    pub const fn lab_default() -> Self {
        Self {
            time_preference_bps: 5_000,
            leisure_weight_bps: 3_000,
        }
    }

    /// Construct from raw basis points, clamping each to `0..=10_000` so the
    /// ordinal placement math stays in a well-defined range.
    pub const fn new(time_preference_bps: u16, leisure_weight_bps: u16) -> Self {
        Self {
            time_preference_bps: clamp_bps(time_preference_bps),
            leisure_weight_bps: clamp_bps(leisure_weight_bps),
        }
    }

    /// A child's culture (G4b): the parent's, with each field nudged by a small
    /// bounded delta **derived deterministically** from the field and a stable
    /// `birth_seq` — never a live `Rng` draw. This is the selection substrate: a
    /// patient parent (low `time_preference_bps`) begets a child patient within
    /// `max_delta_bps`, so a lineage's time preference drifts but persists across
    /// generations, and the market's selection pressure (patient lineages saving
    /// more) shifts the population distribution. The mutation is the heritable
    /// **ordinal** bias `regenerate_scale` already consumes — there is no scalar
    /// fitness anywhere.
    ///
    /// `birth_seq` is the colony's monotonic birth counter (a unique, stable
    /// sequence number per birth), so the same `(parent, birth_seq, max_delta)` →
    /// the same child every run (the determinism tripwire). The delta lands in
    /// `-max_delta_bps..=max_delta_bps`; [`Self::new`] re-clamps each field to
    /// `0..=10_000`, so an extreme parent cannot drift out of range.
    pub fn inherit(self, birth_seq: u64, max_delta_bps: u16) -> Self {
        Self::new(
            mutate_field(
                self.time_preference_bps,
                birth_seq,
                TIME_PREFERENCE_SALT,
                max_delta_bps,
            ),
            mutate_field(
                self.leisure_weight_bps,
                birth_seq,
                LEISURE_WEIGHT_SALT,
                max_delta_bps,
            ),
        )
    }
}

/// Per-field salts so the two culture fields draw independent (but still fully
/// deterministic) deltas from the same birth sequence — otherwise both fields
/// would always move together.
const TIME_PREFERENCE_SALT: u64 = 0x1234_5678_9abc_def0;
const LEISURE_WEIGHT_SALT: u64 = 0x0fed_cba9_8765_4321;

/// Nudge `field` by a bounded delta derived deterministically from
/// `(field, birth_seq, salt)`. The delta is uniform over
/// `-max_delta..=max_delta`; `max_delta == 0` returns the field unchanged (a
/// no-mutation lineage). Pure integer, no RNG — same inputs → same output.
fn mutate_field(field: u16, birth_seq: u64, salt: u64, max_delta: u16) -> u16 {
    if max_delta == 0 {
        return field;
    }
    let span = u64::from(max_delta) * 2 + 1;
    let draw = deterministic_mix64(birth_seq ^ salt ^ u64::from(field)) % span;
    // `draw` in 0..span maps to a delta in -max_delta..=max_delta.
    let delta = draw as i64 - i64::from(max_delta);
    let mutated = i64::from(field) + delta;
    // `new` clamps to 0..=10_000; clamp here too so the cast is well-defined.
    mutated.clamp(0, i64::from(u16::MAX)) as u16
}

/// A SplitMix64 finalizer — a pure, deterministic avalanche hash. Used by G4b
/// demography and culture inheritance to derive bounded decisions from stable
/// seeds. It draws nothing and replaces no `Rng`; no statistical claim rides on it.
pub fn deterministic_mix64(mut x: u64) -> u64 {
    x = x.wrapping_add(0x9e37_79b9_7f4a_7c15);
    x = (x ^ (x >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
    x = (x ^ (x >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
    x ^ (x >> 31)
}

impl Default for CultureParams {
    fn default() -> Self {
        Self::lab_default()
    }
}

const fn clamp_bps(bps: u16) -> u16 {
    if bps > 10_000 {
        10_000
    } else {
        bps
    }
}

#[cfg(test)]
mod tests {
    use super::CultureParams;

    #[test]
    fn new_clamps_basis_points_to_range() {
        let params = CultureParams::new(60_000, 12_345);
        assert_eq!(params.time_preference_bps, 10_000);
        assert_eq!(params.leisure_weight_bps, 10_000);
    }

    #[test]
    fn lab_default_is_in_range() {
        let params = CultureParams::lab_default();
        assert!(params.time_preference_bps <= 10_000);
        assert!(params.leisure_weight_bps <= 10_000);
        assert_eq!(params, CultureParams::default());
    }

    #[test]
    fn inherit_is_deterministic_and_bounded() {
        let parent = CultureParams::new(5_000, 3_000);
        let max_delta = 200;

        // Same (parent, birth_seq, max_delta) → byte-identical child, every time.
        let a = parent.inherit(42, max_delta);
        let b = parent.inherit(42, max_delta);
        assert_eq!(a, b, "inheritance must be deterministic");

        // Each field stays within max_delta of the parent (and in range).
        for seq in 0..256u64 {
            let child = parent.inherit(seq, max_delta);
            let tp_delta =
                i32::from(child.time_preference_bps) - i32::from(parent.time_preference_bps);
            let lw_delta =
                i32::from(child.leisure_weight_bps) - i32::from(parent.leisure_weight_bps);
            assert!(
                tp_delta.abs() <= i32::from(max_delta),
                "tp delta out of bounds"
            );
            assert!(
                lw_delta.abs() <= i32::from(max_delta),
                "lw delta out of bounds"
            );
            assert!(child.time_preference_bps <= 10_000);
            assert!(child.leisure_weight_bps <= 10_000);
        }
    }

    #[test]
    fn inherit_with_zero_delta_copies_the_parent() {
        let parent = CultureParams::new(1_234, 5_678);
        assert_eq!(parent.inherit(7, 0), parent);
        assert_eq!(parent.inherit(999, 0), parent);
    }

    #[test]
    fn inherit_can_move_both_directions_and_varies_with_sequence() {
        // Over a span of births the mutation must explore both signs (not a
        // one-directional drift) and depend on the sequence number.
        let parent = CultureParams::new(5_000, 5_000);
        let mut saw_up = false;
        let mut saw_down = false;
        let mut distinct = std::collections::BTreeSet::new();
        for seq in 0..512u64 {
            let child = parent.inherit(seq, 300);
            if child.time_preference_bps > parent.time_preference_bps {
                saw_up = true;
            }
            if child.time_preference_bps < parent.time_preference_bps {
                saw_down = true;
            }
            distinct.insert(child.time_preference_bps);
        }
        assert!(saw_up && saw_down, "mutation must move both up and down");
        assert!(
            distinct.len() > 16,
            "mutation must vary with the birth sequence"
        );
    }

    #[test]
    fn inherit_clamps_an_extreme_parent_into_range() {
        // A maximally present-biased parent cannot drift above the bps ceiling.
        let parent = CultureParams::new(10_000, 10_000);
        for seq in 0..64u64 {
            let child = parent.inherit(seq, 500);
            assert!(child.time_preference_bps <= 10_000);
            assert!(child.leisure_weight_bps <= 10_000);
        }
        // A patient parent at the floor cannot drift below zero.
        let parent = CultureParams::new(0, 0);
        for seq in 0..64u64 {
            let child = parent.inherit(seq, 500);
            // u16 cannot be negative; the floor is structurally 0.
            assert!(child.time_preference_bps <= 500);
        }
    }
}
