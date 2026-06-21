//! G4b demography — the configuration and deterministic helpers for births,
//! aging, households, and culture inheritance.
//!
//! Demography is an **opt-in overlay** on a settlement, exactly as the G3a
//! production chain is: [`crate::SettlementConfig::demography`] is `None` for
//! every pre-G4b config (so they stay byte-identical), and `Some` turns on the
//! whole mechanism. The settlement then carries no spatial colonists of its own —
//! the households' members are **non-spatial** householders (they trade in the
//! econ market but never haul), fed by a renewable per-member **provision** (a
//! conserved source, the household hearth) so the only deaths are of **old age**,
//! not starvation.
//!
//! Everything here is pure and **deterministic** — same `(seed, config)` →
//! byte-identical run. The loop draws no `Rng`: a colonist's old-age lifespan and
//! starting age, and a child's mutation and seed, all derive from a stable
//! per-colonist seed via [`life::deterministic_mix64`] (a SplitMix64 finalizer),
//! never a live draw. The seeds themselves are hashed from the world seed and a
//! monotonic birth sequence, so generation consumes the `Rng` only for culture (as
//! G1/G2/G3 already do) and demography adds no `Rng` draw at all.
//!
//! See `docs/impl-g4b.md` and the G4b section of `docs/engine-divergence.md`.

use life::deterministic_mix64;

/// One seeded **household** — a lineage with a shared time-preference bias and a
/// renewable provision. The two-lineage `lineages` config seeds a patient one and
/// a present-biased one and measures the patient lineage out-accumulating the
/// other (sign only — the selection result).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct HouseholdSpec {
    /// Founder members generated for this household at world generation.
    pub founders: u16,
    /// The household's time-preference base (bps) — the heritable **ordinal**
    /// bias `regenerate_scale` consumes (G1). Low = patient (a high saving target,
    /// so it keeps offering surplus and accumulates gold); high = present-biased
    /// (a small target, so it spends gold down). A child inherits this with a
    /// bounded, deterministic mutation.
    pub time_preference_base_bps: u16,
    /// Renewable FOOD provisioned to each living member each econ tick — the
    /// household hearth. Sized so members stay fed (hunger never reaches the
    /// critical ceiling), so deaths are **old-age**, not starvation. A conserved
    /// source (accounted in the econ-tick report), not a mint.
    pub food_provision: u32,
    /// Renewable WOOD provisioned to each living member each econ tick. The trade
    /// asymmetry: a wood-surplus household sells its surplus for gold, a wood-poor
    /// one buys wood for (non-lethal) warmth — so gold flows from the present-biased
    /// spenders to the patient savers. A conserved source.
    pub wood_provision: u32,
    /// Starting gold per founder (same across lineages in the curated config, so the
    /// accumulation gap is behavioral, not endowed).
    pub starting_gold: u64,
    /// Starting FOOD buffer per founder.
    pub starting_food: u32,
    /// Starting WOOD buffer per founder.
    pub starting_wood: u32,
}

/// The G4b demography overlay: the households plus the aging, mortality, and birth
/// cadence. `None` on [`crate::SettlementConfig::demography`] for every pre-G4b
/// config (byte-identical); `Some` activates the mechanism.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DemographyConfig {
    /// The seeded households (lineages).
    pub households: Vec<HouseholdSpec>,
    /// Econ ticks per "year" — the aging cadence. Old age and lifespans are
    /// expressed in years and converted to ticks with this.
    pub ticks_per_year: u64,
    /// Age (years) at which old-age mortality begins. A colonist's deterministic
    /// lifespan is `old_age_onset_years + hash(seed) % (lifespan_span_years + 1)`.
    pub old_age_onset_years: u64,
    /// The lifespan spread (years) past the onset — the only source of lifespan
    /// variation (deterministic per colonist).
    pub lifespan_span_years: u64,
    /// Minimum econ ticks between births in one household — the birth-rate bound
    /// (one birth per household per interval, regardless of member count).
    pub birth_interval: u64,
    /// Need-security gate: every living member's hunger must be at or below this for
    /// the household to birth (the "sustained food margin").
    pub birth_hunger_ceiling: u16,
    /// A household will not birth past this many living members — the blowup bound
    /// (population is capped at `households × max_household_size`).
    pub max_household_size: u16,
    /// The newborn's FOOD endowment — a conserved **transfer** the chosen parent
    /// must hold (a starting staple buffer); the birth is gated on the parent
    /// holding it, so a household that cannot feed a child does not have one.
    pub child_food_endowment: u32,
    /// The newborn's gold endowment — a conserved transfer, **best-effort**: clamped
    /// to the parent's balance, so poverty delays a lineage's *wealth*, never its
    /// *births* (a present-biased, gold-poor lineage still reproduces).
    pub child_gold_endowment: u64,
    /// Bound on the per-field culture mutation at birth (bps). `0` is a no-mutation
    /// (pure-clone) lineage.
    pub mutation_delta_bps: u16,
    /// S13 **spatial households**: when set, every lineage member (founders at
    /// generation + newborns at birth) is given a **world agent** at its exact econ
    /// `AgentId`, so the reproducing population is spatial and can be assigned
    /// forage/gather/haul tasks — the structural unification that unblocks the
    /// scarcity arc. `false` for every pre-S13 config (founders/newborns stay
    /// econ-only, so the run is byte-identical). Purely structural: it grants the
    /// *capability*, not forage scarcity, cultivation, or mortality.
    pub spatial_households: bool,
}

impl DemographyConfig {
    /// The curated **two-lineage** config: a patient household and a present-biased
    /// one, identical but for their time preference and their wood provision. The
    /// patient lineage gets a wood surplus (it sells, accumulating gold); the
    /// present-biased one gets none (it buys wood for warmth, spending gold down).
    /// Both are food-secure, so deaths are old age and both lineages reproduce —
    /// and the patient lineage ends richer (sign only; the selection demonstration).
    pub fn lineages() -> Self {
        Self {
            households: vec![
                // The patient lineage: a low time preference (a high saving target)
                // and a wood surplus to sell. It accumulates gold.
                HouseholdSpec {
                    founders: 3,
                    time_preference_base_bps: 500,
                    food_provision: 2,
                    wood_provision: 3,
                    starting_gold: 16,
                    starting_food: 6,
                    starting_wood: 4,
                },
                // The present-biased lineage: a high time preference (a small saving
                // target) and no wood provision. It buys wood for warmth, spending
                // its gold down — the gold flows to the patient lineage.
                HouseholdSpec {
                    founders: 3,
                    time_preference_base_bps: 9_400,
                    food_provision: 2,
                    wood_provision: 0,
                    starting_gold: 16,
                    starting_food: 6,
                    starting_wood: 4,
                },
            ],
            ticks_per_year: 12,
            old_age_onset_years: 6,
            lifespan_span_years: 4,
            birth_interval: 8,
            birth_hunger_ceiling: 8,
            max_household_size: 8,
            child_food_endowment: 4,
            child_gold_endowment: 4,
            mutation_delta_bps: 200,
            // Non-spatial by default: the `lineages` golden keeps econ-only founders.
            spatial_households: false,
        }
    }

    /// Total founder count across all households.
    pub fn founder_count(&self) -> usize {
        self.households
            .iter()
            .map(|h| usize::from(h.founders))
            .sum()
    }

    /// A colonist's deterministic old-age lifespan in **econ ticks**, derived from
    /// its stable `seed` — `onset + hash(seed) % (span + 1)` years, in ticks. No
    /// RNG: same seed → same lifespan (the determinism tripwire).
    pub fn lifespan_ticks(&self, seed: u64) -> u64 {
        let span = self.lifespan_span_years + 1;
        let extra_years = deterministic_mix64(seed ^ LIFESPAN_SALT) % span;
        (self.old_age_onset_years + extra_years) * self.ticks_per_year
    }

    /// A founder's deterministic starting age in **econ ticks**, spread across
    /// `0..old_age_onset_years` from its seed so the founders age into old age at
    /// staggered times (no synchronized die-off). No RNG.
    pub fn founder_start_age_ticks(&self, seed: u64) -> u64 {
        let span = self.old_age_onset_years.max(1);
        let years = deterministic_mix64(seed ^ FOUNDER_AGE_SALT) % span;
        years * self.ticks_per_year
    }
}

/// A founder's stable seed, hashed from the world seed and its global founder
/// index — pure, so generation draws no extra `Rng` for demography.
pub fn founder_seed(world_seed: u64, founder_index: usize) -> u64 {
    deterministic_mix64(
        world_seed ^ FOUNDER_SEED_SALT ^ (founder_index as u64).wrapping_mul(0x9e37_79b9),
    )
}

/// A child's stable seed, hashed from its parent's seed and the colony's monotonic
/// birth sequence — pure and unique per birth, so its lifespan and its own
/// children's mutations are deterministic with no loop-time `Rng`.
pub fn child_seed(parent_seed: u64, birth_seq: u64) -> u64 {
    deterministic_mix64(parent_seed.wrapping_add(CHILD_SEED_SALT) ^ birth_seq.rotate_left(17))
}

// Per-purpose salts so the same seed yields independent (but deterministic) draws
// for lifespan, starting age, and the seed derivations.
const LIFESPAN_SALT: u64 = 0xa1b2_c3d4_e5f6_0718;
const FOUNDER_AGE_SALT: u64 = 0x0f1e_2d3c_4b5a_6978;
const FOUNDER_SEED_SALT: u64 = 0xdead_beef_cafe_f00d;
const CHILD_SEED_SALT: u64 = 0x1357_9bdf_2468_ace0;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lifespan_is_deterministic_and_in_range() {
        let cfg = DemographyConfig::lineages();
        let onset = cfg.old_age_onset_years * cfg.ticks_per_year;
        let max = (cfg.old_age_onset_years + cfg.lifespan_span_years) * cfg.ticks_per_year;
        for seed in 0..256u64 {
            let life = cfg.lifespan_ticks(seed);
            assert!(life >= onset && life <= max, "lifespan out of range");
            assert_eq!(
                life,
                cfg.lifespan_ticks(seed),
                "lifespan must be deterministic"
            );
        }
    }

    #[test]
    fn founder_start_age_is_below_onset() {
        let cfg = DemographyConfig::lineages();
        let onset = cfg.old_age_onset_years * cfg.ticks_per_year;
        for seed in 0..256u64 {
            assert!(cfg.founder_start_age_ticks(seed) < onset);
        }
    }

    #[test]
    fn child_and_founder_seeds_are_deterministic_and_varied() {
        let mut seeds = std::collections::BTreeSet::new();
        for i in 0..64 {
            let s = founder_seed(0xC0FFEE, i);
            assert_eq!(s, founder_seed(0xC0FFEE, i));
            seeds.insert(s);
        }
        assert!(seeds.len() > 32, "founder seeds must vary by index");

        let mut child_seeds = std::collections::BTreeSet::new();
        for seq in 0..64u64 {
            let s = child_seed(42, seq);
            assert_eq!(s, child_seed(42, seq));
            child_seeds.insert(s);
        }
        assert!(
            child_seeds.len() > 32,
            "child seeds must vary by birth sequence"
        );
    }
}
