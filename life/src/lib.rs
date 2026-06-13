//! `life` — needs → wants (game milestone G1).
//!
//! The first genuinely new game crate on top of the forked `econ` engine. Its
//! job is the single most important transformation the colony makes to the lab:
//! **a colonist's ordinal value scale is GENERATED from need state each tick,
//! not authored once** (game-spec §4.3, §5.4). praxsim treats `Agent.scale` as
//! a fixture; here hunger/warmth/rest drive it through the pure function
//! [`regenerate_scale`].
//!
//! Everything else exists to prove that function produces sensible *emergent*
//! behavior when its output is fed to the real, unchanged econ market: the
//! [`Camp`] driver builds a `Society` of generated colonists that feeds, fuels,
//! and rests itself through trade and labor, where labor supply rises and falls
//! with rest because Leisure is a want like any other.
//!
//! G1 is deliberately **mechanism-only and pre-spatial** (game-spec §11): it
//! asserts scale-generation *properties* and non-collapse, never balance
//! numbers. It is not space (G2), demography/birth/aging/households (G4 — G1 has
//! death-by-starvation only, via tombstone, not arena free), content/tech (G3),
//! the `sim` orchestrator (G2), or any change to the econ engine's economic
//! behavior (the goldens stay byte-identical).
//!
//! Pure std, integer math in the sim path, no `HashMap` in logic, deterministic
//! generation (one seed → one `Rng`, consumed at world-generation only; the
//! tick loop draws nothing).

pub mod camp;
pub mod culture;
pub mod need;
pub mod scale;

pub use camp::{Camp, CampEnv, Vocation, TICKS_PER_YEAR};
pub use culture::CultureParams;
pub use need::{NeedDynamics, NeedIntake, NeedState};
pub use scale::{regenerate_scale, KnownGoods};
