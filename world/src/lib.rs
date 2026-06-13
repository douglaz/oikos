//! `world` — the pure spatial substrate (game milestone G2a).
//!
//! The first slice of G2 (game-spec §11, decomposed in `docs/impl-g2a.md`): a
//! standalone, econ-*independent* spatial layer the rest of G2 builds on. It
//! provides a tile grid with terrain, resource nodes, stockpiles, agents with
//! positions and carried inventory, deterministic movement and pathfinding
//! around obstacles, and a conserving `tick`.
//!
//! **It is space, not economy.** `world` knows positions, terrain, movement,
//! harvest yields, and storage — it does **not** know prices, money, wants, or
//! trades. Goods appear only as integer quantities of `GoodId` at locations or
//! carried by agents. The economic coupling — the two-rate loop, the §4.3
//! delivery-escrow contract, distance-affects-price — is **G2b** and lives in
//! the integration layer, not here. If a concept is economic, it belongs in G2b.
//!
//! `world` depends on `econ` **only** for the shared primitives [`AgentId`],
//! [`GoodId`], and [`Rng`] (re-exported below), so G2b can bridge world↔econ
//! with no type translation. It calls no econ economic logic and changes no econ
//! behavior — `econ` does not depend on `world`, so the econ conformance goldens
//! and the G1 `life` tests are safe by construction.
//!
//! Two invariants are the contract (see [`world::World`]):
//!
//! - **Determinism.** Integer state; the `Rng` is consumed at world *generation*
//!   only and `tick()` draws nothing; agents iterate in `AgentId` order; storage
//!   is `BTreeMap`/`Vec`, never `HashMap`. Same seed + same command sequence →
//!   byte-identical world.
//! - **Conservation.** Node regen is the *only* source of goods; movement,
//!   harvest, and deposit relocate units without creating or destroying one, and
//!   every per-tick change is accounted in [`world::TickReport`].

pub mod grid;
pub mod node;
pub mod path;
pub mod stockpile;
pub mod world;

// The shared econ primitives `world` is built on, re-exported so downstream
// (G2b) can name them through `world` without depending on `econ`'s layout.
pub use econ::agent::AgentId;
pub use econ::good::GoodId;
pub use econ::rng::Rng;

pub use grid::{Grid, Pos, Terrain};
pub use node::{NodeId, ResourceNode};
pub use path::{shortest_path, travel_cost};
pub use stockpile::{Stockpile, StockpileId};
pub use world::{AgentStatus, PlacementError, Task, TickReport, World, WorldGen};
