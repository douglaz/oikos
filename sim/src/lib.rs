//! `sim` — the two-rate orchestrator (game milestone G2b).
//!
//! G2a built the spatial substrate (`world`) in isolation; G2b makes space
//! **economically meaningful** by wiring it under the economy. `sim` owns a
//! [`world::World`], per-colonist [`life`] need state, and an [`econ::Society`],
//! and runs the **two-rate loop** the game-spec (§4.1, §4.3) calls for: a fast
//! loop of `FAST_TICKS_PER_ECON_TICK` `world` ticks (movement, harvest, haul)
//! under one economic tick (transfer → needs/tombstone → scale regeneration →
//! market clearing → consumption read-back → task reassignment).
//!
//! The milestone proves two things:
//!
//! 1. **Whole-system conservation is exact.** Every physical good is accounted
//!    across its full spatial+economic lifecycle — node stock → hauler carry →
//!    exchange stockpile → econ stock → consumed — with node regen the only
//!    source and consumption the only sink. Goods in transit (a hauler's carry)
//!    are the §4.3 **delivery escrow**: conserved, retained (never destroyed)
//!    when a hauler is blocked or dies.
//! 2. **Distance measurably affects realized prices.** A good gathered from a
//!    node FAR from the exchange realizes a higher price than the same good
//!    gathered NEAR, because travel eats the fast-tick budget so fewer units
//!    reach the market per economic tick (sign only — the lab's
//!    direction-not-magnitude discipline).
//!
//! **The world→econ transfer seam** is the load-bearing design: a good has ONE
//! owner at a time — `world` (node / carry / stockpile) **or** `econ` (agent
//! stock). The econ-tick transfer is the only crossing, and it is net-zero
//! (`world` −n, `econ` +n): delivered exchange-stockpile units are *credited to
//! the depositing colonist's econ stock* and *withdrawn from the world*. If a
//! depositor has no stock headroom yet, the unit stays world-owned in the
//! exchange stockpile and is retried later. See `docs/engine-divergence.md`.
//!
//! `sim` reuses `life`'s `regenerate_scale` / `NeedState` / `CultureParams` /
//! tombstone mechanism and `world` / `econ` as-is, adding only the two additive,
//! conserving accessors that realize the seam ([`world::World::stockpile_withdraw`]
//! and [`econ::society::Society::credit_stock`]). It **supersedes** `life::Camp`
//! as the driver (Camp stays as the G1 non-spatial reference harness).
//!
//! Pure std, integer state, deterministic: the `Rng` is consumed only at
//! [`Settlement::generate`]; neither the fast loop nor the econ tick draws any
//! randomness; iteration is `AgentId`-ordered; storage is `BTreeMap`/`Vec`,
//! never `HashMap`. Same seed + same [`SettlementConfig`] → byte-identical run.

pub mod settlement;

pub use settlement::{
    EconTickReport, NodeSpec, Settlement, SettlementConfig, Vocation, ECON_TICKS_PER_YEAR,
    FAST_TICKS_PER_ECON_TICK,
};

/// Read-only re-exports of the `econ`/`life` types that make up the settlement's
/// read surface — the return and element types of the accessors above, plus the
/// good ids a downstream reader names. The G2d `oikos` debug viewer renders
/// settlement state through these: the [`Society`] behind [`Settlement::society`]
/// and its trade tape's [`Trade`], the [`Agent`] behind `society().agents` and
/// its value scale's [`Want`] / [`WantKind`] / [`Horizon`], the realized-price
/// [`Gold`], the [`NeedState`] behind [`Settlement::need_of`], and [`GoodId`] /
/// [`FOOD`] / [`WOOD`]. These are pure re-exports: they add public surface but
/// change no behavior, so the econ conformance goldens and the G1/G2a/G2b suites
/// stay byte-identical (the unchanged workspace `cargo test` is the proof).
/// Keeping them on `sim` lets the viewer depend on `sim` alone — a thin binary
/// over one crate — instead of reaching into `econ`/`life` directly.
pub use econ::agent::{Agent, AgentId, Want, WantKind};
pub use econ::good::{Gold, GoodId, Horizon, FOOD, WOOD};
pub use econ::market::Trade;
pub use econ::society::Society;
pub use life::NeedState;
