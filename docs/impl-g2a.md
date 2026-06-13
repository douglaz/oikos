# Implementation Spec G2a: the `world` crate (spatial substrate)

## Purpose

G2 in the game-spec roadmap (§11) bundles four large pieces into one
milestone: the `world` crate (map, movement, stockpiles), the two-rate loop
with the §4.3 delivery-escrow contract, the Society-monolith extraction for
multiple settlements, and the debug viewer with inspectors. That is far too
much for one reviewed change — G1 (a pure function + a driver) already took
eight review rounds. So G2 is decomposed, and this spec is the first slice.

**Proposed G2 decomposition (supersedes the single-G2 lump in game-spec §11):**

- **G2a (this milestone): the `world` crate** — the spatial substrate as a
  standalone, econ-*independent* component: tile grid, terrain, resource
  nodes, agent positions, deterministic movement/pathfinding, and
  stockpiles. Tested in isolation; touches no econ behavior, so the econ
  conformance goldens are safe by construction (econ does not depend on
  `world`).
- **G2b: two-rate loop + escrow** — wire `world` delivery under the econ
  tick via the §4.3 delivery-escrow contract for ONE settlement. DoD:
  distance measurably affects realized prices; delivery escrow conserves
  exactly.
- **G2c: settlement-scoped service extraction** — pull market/labor/barter
  books out of the `Society` monolith so multiple settlements exist.
- **G2d: debug viewer + inspectors** — the first binary; the price→trades
  and colonist→scale-and-why inspectors the game-spec mandates for G2.

G2a is deliberately the lowest-risk slice: a new crate with real spatial
mechanics and property tests, in the proven G1 shape (new crate, clear
mechanism, isolated). It is the foundation G2b/G2c/G2d build on, and it is
needed next regardless of how the rest is sliced — so building it first, in
dependency order, with its own tests, is incremental engineering, not empty
scaffolding.

This is the spatial substrate only. It is NOT the economy integration (G2b —
no prices, money, value scales, or econ-tick coupling here), NOT
multi-settlement (G2c), NOT the viewer (G2d), and NOT a change to any econ or
life behavior.

## Design decisions (settled)

1. **`world` depends on `econ` only for shared primitives** — `GoodId`,
   `AgentId`, `Rng` — so G2b can bridge world↔econ with no type
   translation. It uses NO econ economic logic and changes NO econ behavior.
   (Mirrors how `life` depends on `econ`.)
2. **Pure spatial, no economics.** `world` knows positions, terrain,
   movement, harvest yields, and storage. It does not know prices, money,
   wants, or trades. Goods are tracked only as integer quantities of
   `GoodId` at locations / carried by agents.
3. **Determinism is mandatory and inherited.** Integer state; `Rng`
   (econ's xorshift64*) consumed at world-GENERATION only; the `tick`
   advances movement/harvest/regen with NO RNG draws. Same seed + same
   command sequence → byte-identical world. `BTreeMap`/`Vec` only, no
   `HashMap` in logic.
4. **Conservation is the spatial analogue of the econ ledger invariant.**
   Goods are neither created nor destroyed by movement or hauling: a
   harvest moves units from a node's stock to an agent's carry; a deposit
   moves units from carry to a stockpile; the sum (all node stocks + all
   carried + all stockpile contents) changes only by explicit harvest-yield
   and (optional) node regeneration, and every such change is accounted.

## Verified Base Facts (2026-06-13, oikos @ `4f66410`, 683 tests green)

1. Workspace members are `econ`, `life` (`Cargo.toml`); G2a adds `world`.
2. Reusable primitives: `AgentId(pub u64)` (agent.rs:19, post-G0b u64
   packing), `GoodId(pub u16)` (good.rs:4), `Rng::new(seed)` /
   `next_u64()` (rng.rs:9,21). All `Copy`/`Ord`-friendly and exportable.
3. `econ` does not and must not depend on `world`; the econ goldens
   (M0–M3 series + M18/M20 emergence + M5/M6 anchors) are unaffected by a
   new leaf crate and must still pass from the workspace.

## Milestone Boundary

G2a includes:

- a new `world` workspace crate (depends on `econ` for primitives; pure
  std otherwise; deterministic);
- `Grid` + `Pos` + terrain (passable / impassable);
- `ResourceNode` (location, yielded `GoodId`, stock, optional deterministic
  regen-per-tick);
- `Stockpile` (location, capacity, integer good contents);
- agent spatial state (`AgentId` → `Pos` + carried inventory) and
  `MoveTarget`/task assignment (go-to-node-and-harvest,
  go-to-stockpile-and-deposit, go-to-tile);
- deterministic movement + pathfinding around impassable terrain
  (BFS/flow-field — deterministic tie-breaking);
- `World::tick()` advancing movement, applying arrivals (harvest into
  carry, deposit into stockpile), and node regen; query accessors
  (positions, grid distance / travel estimate, stockpile contents,
  conservation total);
- world generation from a seed (`World::generate(seed, &WorldGen)`);
- acceptance tests in `world/tests/g2a_world.rs` + unit tests;
- README + `engine-divergence.md` (note: new leaf crate, the G2
  decomposition recorded).

G2a excludes:

- no econ-tick coupling, prices, money, wants, or trades (G2b);
- no escrow ledger (G2b — `world` only reports delivered/undelivered
  quantities; the escrow accounting lives in the integration);
- no multiple settlements / monolith extraction (G2c);
- no binary, viewer, or inspectors (G2d);
- no `life`/`Camp` changes (G2b rewires the driver);
- no RNG in `tick`; no `HashMap` in logic; no new external dependencies.

## Domain Semantics

### Grid and terrain

```rust
pub struct Pos { pub x: u16, pub y: u16 }
pub enum Terrain { Passable, Impassable }
pub struct Grid { width: u16, height: u16, terrain: Vec<Terrain> /* row-major */ }
```

Integer grid; `Impassable` blocks movement and cannot host a node/stockpile
that an agent must reach (placement on impassable is rejected at
construction — tested).

### Resource nodes, stockpiles, agents

```rust
pub struct ResourceNode { pub pos: Pos, pub good: GoodId, pub stock: u32, pub regen_per_tick: u32, pub cap: u32 }
pub struct Stockpile   { pub pos: Pos, pub cap: u32, /* good -> qty, BTreeMap */ }
// per agent: Pos, carried inventory (good -> qty, BTreeMap), current task
pub enum Task { Idle, GoHarvest(NodeId, u32 /*want*/), GoDeposit(StockpileId), GoTo(Pos) }
```

- harvest: on arrival at the node, move `min(want, node.stock, carry_room)`
  units node→carry; node stock decreases by exactly that; conservation
  holds;
- deposit: on arrival at the stockpile, move `min(carried, stockpile_room)`
  units carry→stockpile; conservation holds; overflow stays carried (never
  destroyed);
- regen: at tick end, `node.stock = min(cap, stock + regen_per_tick)` —
  the ONE place goods are created, fully accounted in the conservation
  query (a per-tick `harvested`/`regenerated`/`net` report).

### Movement and pathfinding

- each tick, an agent with a move target advances up to `move_speed` grid
  steps along a shortest path (4-connectivity) avoiding `Impassable`;
- pathfinding is deterministic BFS with fixed tie-breaking (e.g.
  lowest-index neighbor first); an unreachable target leaves the agent
  put and sets a `Blocked` status (no panic, deterministic);
- arrival triggers the task's spatial action (harvest/deposit) on the same
  or next tick per a stated rule (pin it; the test asserts the exact
  step count for a known layout).

### World tick order

```
World::tick():
  1. advance each agent (id order) toward its target by move_speed steps
  2. apply arrivals: harvest / deposit (id order, deterministic)
  3. regen nodes (id order)
  4. update the per-tick spatial report (harvested, regenerated, conserved)
```

No RNG in `tick`. Agent iteration is always `AgentId` order.

## Implementation Tasks

1. Add `world` to the workspace; `world/Cargo.toml` (econ path dep; pure
   std).
2. `world/src/grid.rs`: `Pos`, `Terrain`, `Grid` (+ bounds/passability
   helpers, placement validation).
3. `world/src/node.rs`, `stockpile.rs`: `ResourceNode`, `Stockpile`,
   integer harvest/deposit/regen with conservation.
4. `world/src/move.rs` (or `path.rs`): deterministic BFS pathfinding +
   step advance + `Blocked`.
5. `world/src/world.rs`: `World`, `WorldGen`, `generate`, `assign_task`,
   `tick`, accessors, conservation report.
6. Tests (below).
7. README (the `world` crate + the G2 decomposition) and
   `engine-divergence.md` (G2a entry: new leaf crate, no econ coupling,
   the decomposition recorded as superseding the single-G2 lump).

## Acceptance Tests

`world/tests/g2a_world.rs` (+ unit tests per module):

1. `world_generation_is_deterministic` — same `(seed, WorldGen)` →
   byte-identical world (hash a canonical serialization); two runs equal.
2. `tick_is_deterministic` — same world + same task assignments → identical
   state after N ticks across two runs.
3. `movement_reaches_reachable_targets` — an agent assigned `GoTo`/harvest
   reaches the target in exactly the expected step count for a known
   open-grid layout (pin the count).
4. `pathfinding_avoids_impassable_and_is_deterministic` — with an obstacle
   wall and a gap, the agent routes through the gap; the path is the
   fixed-tie-break shortest path; reproducible.
5. `unreachable_target_blocks_without_panic` — a fully walled-off target
   leaves the agent put with `Blocked`, deterministically, no panic.
6. `harvest_conserves_goods` — harvesting moves units node→carry exactly;
   node stock down by the amount, carry up by the amount; the conservation
   total is unchanged by harvest alone (regen aside).
7. `deposit_respects_capacity_and_conserves` — depositing into a stockpile
   clamps to capacity; overflow stays carried; nothing destroyed; totals
   balance.
8. `regen_is_the_only_source_and_is_accounted` — node regen is the sole
   creator of goods, clamped to `cap`, and the per-tick report's
   `regenerated` exactly equals the increase; with `regen_per_tick = 0`
   the world's good total is invariant across ticks.
9. `placement_on_impassable_is_rejected` — constructing a node/stockpile on
   an impassable tile (or a target an agent must stand on) is rejected, not
   silently accepted.
10. `distance_estimate_is_monotonic` — the travel/distance accessor grows
    monotonically with grid separation on an open grid (the property G2b
    will lean on for "distance affects price").
11. `econ_and_life_unchanged` — the full workspace suite passes, all six
    econ goldens byte-identical, all G1 tests green; `cargo clippy
    --workspace --all-targets -- -D warnings`; `cargo fmt --check`.

Manual check:

```bash
cargo test -p world
cargo test            # whole workspace incl. econ conformance + life
```

## Handoff Notes

- `world` is a pure spatial substrate: NO prices, money, wants, or trades.
  If a test reaches for an economic concept, it belongs in G2b, not here.
- Determinism is the contract: integer state, `Rng` at generation only,
  nothing drawn in `tick`, `AgentId`-ordered iteration, `BTreeMap`/`Vec`
  only. Tests 1–2 are the tripwire.
- Conservation is the spatial ledger invariant: regen is the only source,
  and it is accounted in the per-tick report. Movement and hauling never
  create or destroy a unit.
- `world` depends on `econ` only for `GoodId`/`AgentId`/`Rng`; it must not
  call econ economic logic and cannot change any econ behavior (econ does
  not depend on `world`, so the goldens are safe — test 11 confirms).
- Keep it lean and real, not stubs: pathfinding, conservation, and capacity
  are genuine mechanics with the properties above — that is what makes this
  a milestone and not scaffolding.
- `git add` new files so the diff-scoped reviewer sees them; gitignore any
  stray build artifacts (the G1 `check`-binary lesson).
