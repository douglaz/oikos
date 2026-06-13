# Implementation Spec G1: Needs → Wants (the `life` crate)

## Purpose

G0a/G0b forked and re-plumbed the engine; G1 is the first genuinely new game
code. It adds the `life` crate, whose job is the single most important
transformation the game makes to the lab engine: **a colonist's ordinal value
scale is GENERATED from need state each tick, not authored once.** praxsim
treats `Agent.scale` as a fixture; in the game, hunger/warmth/rest drive it.

The heart of the milestone is one pure function:

```
regenerate_scale(needs, culture, known_goods) -> Vec<Want>
```

Everything else exists to prove that function produces sensible *emergent*
behavior when its output is fed to the real, unchanged econ market: a camp
that feeds, fuels, and rests itself through trade and labor, where labor
supply rises and falls with rest because Leisure is a want like any other.

Per game-spec §11, G1 is deliberately **mechanism-only and pre-spatial**: the
definition of done asserts scale-generation *properties* and a non-collapse
smoke test, NOT balance numbers — balance tuning before G2's spatial layer
exists would tune a model G2 throws away.

This is needs→wants. It is not space (G2), demography/birth/aging/households
(G4 — G1 has death-by-starvation only, and via tombstone, not arena free),
content/tech (G3), the `sim` orchestrator crate (G2), or any change to the
econ engine's economic behavior (the goldens stay byte-identical).

## Verified Base Facts (2026-06-13, oikos @ `1e5f65c`, 654 tests green)

These were verified against the forked engine and OVERRIDE anything below
that conflicts.

1. **The scale type the function must produce.** `agent.rs:52-63`:
   `WantKind::{Good(GoodId), Leisure}`; `Want { kind, horizon: Horizon,
   qty: u32, satisfied: bool }`; `Horizon::{Now, Next, Later(u8)}`
   (`good.rs:65`). Lower index in `Agent.scale: Vec<Want>` = more urgent
   (the rank-walk reservation pricing the engine already runs consumes it
   exactly this way). `Leisure` is already a first-class `WantKind`, so
   labor supply is already emergent — G1 only has to RANK it from rest.
2. **The injection point.** Each market tick (`step_m1`, society.rs:563+)
   clears and recomputes satisfaction over `Agent.scale` before clearing
   markets. Overwriting `scale` at the TOP of a tick — exactly what
   game-spec §4.3 calls "scale regeneration, step one of the economic
   tick" — is the natural seam and changes no engine behavior.
3. **Hunger plumbing already exists.** `Agent.hunger_deficit` (a lifetime
   diagnostic, never affects planning) and `Consumption { food_consumed,
   hunger_deficit }` (agent.rs) already track FOOD consumption. G1's
   hunger need reuses this; warmth (fuel/WOOD) and rest (Leisure) are new
   need accounting in `life`, not new econ mechanics.
4. **`Society` is drivable from outside.** `Society::from_scenario`
   (society.rs:297) builds it; `step()` (society.rs:563) is public;
   `agents: AgentArena` is a public field with index access. The camp
   harness can build a Society, and between steps read each agent's
   consumption and overwrite each agent's `scale` — no engine edits beyond
   additive read accessors if any are missing.
5. **`AgentArena::free` exists but its cache reconciliation is a documented
   G4 prerequisite** (`engine-divergence.md`). G1 therefore does NOT free
   dead colonists from the arena; it tombstones them (see Known Seam).
6. **Determinism discipline** (lab rule, inherited): one seed → one `Rng`,
   consumed at world-generation only, never in the tick loop. The camp's
   colonist generation uses `Rng`; the tick loop draws nothing.

## Known Seam (declared, not hidden)

**Death by starvation is a tombstone, not an arena free.** When a colonist's
hunger crosses the death threshold it is marked `Dead`: removed from
activation order, its scale emptied so it posts no orders, its holdings
frozen in place. The arena slot is NOT freed and the holdings are NOT settled
to anyone — full estate settlement and the `AgentArena::free` +
Society-cache-reconciliation work are G4 (game-spec §5.6, and the G0b
divergence log parks the free path there). Consequence for G1: a dead
colonist's gold/stock remain in conservation totals (frozen), and the
"population" the smoke test tracks is *living* colonists. This is the honest
minimal death model; it does not pretend to do demography.

The other declared simplification: G1's need set is the load-bearing trio
that maps onto existing lab goods — **hunger↔FOOD, warmth↔fuel (WOOD),
rest↔Leisure**. Shelter/social/security needs named in game-spec §5.2 are
out of scope until they have goods/buildings to satisfy them (G2/G3).

## Milestone Boundary

G1 includes:

- a new `life` workspace crate (depends on `econ`, pure std, deterministic);
- `NeedState` (hunger, warmth, rest — integer levels) + per-tick dynamics
  (depletion + replenishment from consumption/rest);
- `CultureParams` (time-preference bias, leisure weight — integer/bps;
  defined here, *inherited* in G4);
- `regenerate_scale(&NeedState, &CultureParams, &KnownGoods) -> Vec<Want>`,
  pure and deterministic;
- a minimal `Camp` driver in `life`: build a Society of generated colonists,
  and each tick — update needs from last tick's consumption/labor, regenerate
  every living colonist's scale, step the econ market, apply death-by-
  starvation tombstones;
- additive-only `econ` accessors if the camp needs them (read consumption /
  read-write scale / iterate living agents) — no economic-behavior change,
  goldens byte-identical;
- the divergence-log entry recording the tombstone seam and the deferred
  estate/free work;
- acceptance tests in `life/tests/g1_needs_to_wants.rs`;
- README + `engine-divergence.md` updates.

G1 excludes:

- no spatial structure, movement, stockpiles, or two-rate loop (G2);
- no `sim` orchestrator crate (the `Camp` driver is the minimal G1 stand-in,
  to be absorbed/replaced by `sim` at G2 — keep it lean, not a framework);
- no birth/aging/households/migration/estate settlement; no arena free (G4);
- no `content/` goods/recipes/tech; the good set is the lab's (G3);
- no balance tuning or asserted economic magnitudes — properties and
  non-collapse only;
- no change to any econ economic rule, scenario, tape, or golden;
- no `HashMap` in logic, no new external dependencies, integer math in the
  sim path.

## Domain Semantics

### NeedState

```rust
pub struct NeedState { pub hunger: u16, pub warmth: u16, pub rest: u16 }
// 0 = fully satisfied; higher = more depleted; a per-need MAX is the
// critical ceiling. All integer, deterministic.
```

Per-tick dynamics (pure, integer):
- each need DEPLETES by a fixed step each tick (hunger and warmth always;
  rest depletes when the colonist worked last tick, replenishes when it took
  leisure);
- consumption REPLENISHES: FOOD consumed → hunger down; fuel (WOOD) consumed
  → warmth down; leisure taken → rest down;
- hunger at/over its critical ceiling for the death window → the colonist
  dies (tombstone).

### CultureParams

```rust
pub struct CultureParams {
    pub time_preference_bps: u16,  // higher = more present-biased
    pub leisure_weight_bps: u16,   // higher = rest outranks goods sooner
}
```

Heritable in G4; in G1 they are per-colonist constants set at generation.
Time preference is STRUCTURAL — it shifts whether a future-horizon want
ranks above or below a present one in `regenerate_scale`; there is no
cardinal-utility discount and no scalar rate (the lab's purism, preserved).

### regenerate_scale (the milestone)

Contract:
- input: a colonist's `NeedState`, its `CultureParams`, and `KnownGoods`
  (which goods it knows satisfy which need — a fixed lab mapping in G1);
- output: a `Vec<Want>` in **strict descending urgency** (index 0 = most
  urgent), with **each marginal unit listed separately** so diminishing
  marginal utility is baked in by position (no cardinal magnitude anywhere);
- **Leisure is ALWAYS present** in the scale, ranked from `rest`
  (and `leisure_weight_bps`) — this is what keeps labor supply emergent;
- a more-depleted need ranks its want NO LOWER than when less depleted
  (**satiation monotonicity** — the central property);
- `time_preference_bps` places `Later`-horizon wants relative to `Now` ones;
- pure and deterministic: identical inputs → identical output; no RNG, no
  clock, no global state.

The function NEVER emits an empty scale (a colonist always wants something —
at minimum, leisure).

### The Camp driver (minimal, pre-`sim`)

```rust
pub struct Camp { /* Society + per-living-colonist NeedState/CultureParams */ }
impl Camp {
    pub fn generate(seed: u64, population: u16, env: &CampEnv) -> Self;
    pub fn step(&mut self);          // one economic tick (below)
    pub fn living_count(&self) -> usize;
    pub fn realized_food_price(&self) -> Option<...>; // for harvest-shock test
}
```

`Camp::step` order (mirrors game-spec §4.3's economic tick, minus the
not-yet-existing fast loop):
1. update each living colonist's `NeedState` from last tick's consumption +
   whether it worked or rested;
2. apply death-by-starvation tombstones (mark dead, empty scale, drop from
   activation);
3. `regenerate_scale` for every living colonist, overwriting `Agent.scale`;
4. `Society::step()` (the unchanged econ market/labor clearing);
5. read realized consumption/labor back for the next tick's need update.

`CampEnv` carries the per-tick resource endowment flow (FOOD, WOOD) and
labor capacity that make a viable camp — the knobs a harvest shock varies.
All generation randomness is `Rng`-seeded; `step` draws nothing.

## Implementation Tasks

1. Add `life` to the workspace (`Cargo.toml` member; `life/Cargo.toml` with
   `econ` path dep; pure std).
2. `life/src/need.rs`: `NeedState`, per-tick dynamics, death threshold.
3. `life/src/culture.rs`: `CultureParams`.
4. `life/src/scale.rs`: `regenerate_scale` + `KnownGoods` lab mapping.
5. `life/src/camp.rs`: `Camp`, `CampEnv`, `generate`, `step`, accessors;
   tombstone death.
6. Any additive `econ` accessors the camp needs (read consumption, iterate
   living agents) — additive only; run the full econ conformance suite to
   prove goldens unchanged.
7. Tests (below).
8. `engine-divergence.md`: record the tombstone-death seam + deferred
   estate/free (G4); README: add the `life` crate + G1 status.

## Acceptance Tests

`life/tests/g1_needs_to_wants.rs` (+ unit tests in each module):

1. `scale_is_satiation_monotone` — property test across many seeded
   (NeedState, CultureParams): increasing any one need's depletion never
   lowers that need's want rank.
2. `scale_always_contains_leisure` — every generated scale has a Leisure
   want, at every need configuration.
3. `scale_is_never_empty` — no input produces an empty scale.
4. `scale_generation_is_deterministic` — identical inputs → identical
   output (run twice, byte-equal).
5. `diminishing_marginal_utility_is_positional` — a need with capacity for
   multiple units lists them at strictly descending ranks (later units
   rank below earlier ones; no cardinal number used).
6. `time_preference_orders_horizons` — raising `time_preference_bps` moves
   `Later` wants down relative to `Now` (monotone), holding needs fixed.
7. `rested_colonist_works_exhausted_colonist_rests` — at low rest-depletion
   Leisure ranks below goods (colonist will work to provision); at high
   rest-depletion Leisure outranks goods (colonist rests). The emergent
   labor-supply proof, at the scale level.
8. `camp_of_50_does_not_collapse` — `Camp::generate(seed, 50, viable_env)`
   run 5 "years" (define a year = 60 ticks → 300 ticks): no panic; living
   count stays > 0 throughout; needs stay bounded (no runaway past
   ceilings given the viable endowment). Smoke test only — asserts
   survival and boundedness, NOT specific prices or counts. Deterministic
   across two runs.
9. `starvation_kills_via_tombstone` — a colonist cut off from FOOD crosses
   the death threshold, is marked dead, posts no further orders, and is
   excluded from living count; the arena slot is NOT freed and total
   conservation still balances (frozen holdings included).
10. `harvest_shock_raises_food_price` — run a viable camp to a stable
    stretch, then cut FOOD endowment flow; realized FOOD price after the
    cut is higher than before (SIGN only, no magnitude). The market
    responding to scarcity that the needs created.
11. `econ_goldens_unchanged` — the full econ conformance suite (all five
    goldens + M5/M6 anchors) passes from `life`'s workspace; `life` added
    no econ behavior change. `cargo clippy --workspace --all-targets -- -D
    warnings`; `cargo fmt --check`.

Manual check:

```bash
cargo test -p life
cargo test            # whole workspace incl. econ conformance
```

## Handoff Notes

- `regenerate_scale` is the milestone; it is PURE — no RNG, no clock, no
  global state. If a test needs variation, vary the inputs, not the
  function.
- The goldens are econ's; `life` must not change econ's economic behavior.
  Any econ edit is an additive accessor, proven harmless by the unchanged
  conformance suite.
- Death is a TOMBSTONE in G1 — do NOT call `AgentArena::free`, do NOT
  settle estates, do NOT touch the Society caches the divergence log parks
  at G4. Frozen-holdings-in-conservation is the declared, correct G1
  behavior.
- The `Camp` driver is the minimal G1 stand-in for the future `sim` crate —
  keep it lean (a struct with `step`), not a framework; G2's `sim` absorbs
  it.
- Mechanism-only: assert properties and non-collapse. The first test that
  pins a specific price or count is out of scope (G2+ tuning).
- Leisure must always be on the scale or labor supply stops being emergent —
  test 2 guards it.
- No `HashMap` in logic, integer math in the sim path, deterministic
  generation, nothing drawn in the loop.
- `git add` new files so the diff-scoped reviewer sees them.
