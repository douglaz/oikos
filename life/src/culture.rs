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
}
