# Implementation Spec: spatial households — unify the colonist model (S13, structural prerequisite)

> The owner chose to "do it right": before the scarcity-driven-cultivation arc, fix the
> structural obstacle underneath it. Today the colony has **two disjoint populations**: a
> *spatial* non-lineage roster (world agents — they forage/gather/haul) that **never
> reproduces**, and *non-spatial* lineage members (econ-only — fed by the hearth mint) that
> **do reproduce**. So the population that *grows* (lineages) cannot *forage*, and a
> forage-carrying-capacity story is impossible: pressure can't build on the food the growing
> population can't gather. This milestone **unifies the model** — make lineage members
> (founders + newborns) **spatial** (give them world agents) so the reproducing population
> can forage like anyone else. It is purely structural: it unblocks the scarcity arc
> (S14+), it does NOT itself add forage scarcity, cultivation, or mortality.

## What the research established (the cost is low)

- **"Spatial" = has a world agent** (`AgentState { pos, carry, task, … }`, `world/src/world.rs:94`)
  keyed by the *same* `AgentId` as the colonist's econ agent. Every colonist is already both a
  world and an econ agent **except** lineage founders/newborns, who are econ-only by a
  documented modeling choice ("a NON-SPATIAL householder … never hauls", `settlement.rs:4188`).
  It is **not** a performance or correctness constraint.
- **Consumption is econ-side and identical for all** (`update_needs_and_remove_dead` reads the
  econ consumption log, `settlement.rs:5574`). Making a lineage member spatial does **not**
  change how it eats — it only grants it world tasks (forage/gather/haul).
- **Most of the cost is free:** foraging/gathering routes through the existing
  harvest→deposit→transfer path (conserved, node regen is the source); `live_colonist_slots`
  already appends births / removes deaths (`:6135`, `:7999`); death already removes world
  agents (`collect_estate` → `world.remove_agent`, `:5744`); canonical-bytes gating is the
  established pattern. The **one new mechanic** is **mid-run world-agent creation at birth**
  (today world agents are fixed at generation; `world.add_agent` exists and works, it just
  isn't called in `run_births`).
- **The id-coincidence crux (Codex P0) — lockstep-by-ordering does NOT work; mirror the
  exact id instead.** `World::add_agent` emits a fresh *monotonic* generation-0 id
  (`next_agent_index`, `world.rs:390`), but `Society::add_agent` uses an `AgentArena` that
  **reuses freed numeric slots with a bumped GENERATION** after a death (e.g. `3#1`,
  `econ/src/arena.rs:189`, `society.rs:4383`). So after *any* old-age death, a newborn's econ
  id is a reused `slot#gen` while `world.add_agent` returns a fresh `N` — they diverge.
  Giving founders world agents at generation only closes the *initial* gap; it breaks on the
  first birth-after-death. **Fix:** add `World::add_agent_with_id(id, pos, carry_cap,
  move_speed)` that inserts at the **exact econ `AgentId`** (generation included), after
  checking no live duplicate, and use it for every colonist world agent (founders at
  generation, newborns at birth). World then *mirrors* the econ id space exactly — coincidence
  holds by construction across births AND deaths, regardless of arena reuse. (This is the
  load-bearing design point; a separate econ↔world id map is the fallback, but mirroring the
  id is cleaner since world calls already take `AgentId`.)

## Purpose & the honest bar

On a gated path (`DemographyConfig::spatial_households`, default false): make lineage members
(founders + newborns) **spatial** — they get world agents, positions, and the ability to be
assigned world tasks (forage/gather/haul) — so the **reproducing population can forage**.
Behavior is otherwise unchanged (same econ feeding, same demography); the *point* is the new
*capability*, which the scarcity arc (S14+) will exploit. Success = lineage members are
spatial and can actually forage/gather, world↔econ id coincidence holds for all (incl.
newborns born mid-run), conservation + determinism hold, and **with the flag off every
existing scenario + golden is byte-identical**.

NOT forage scarcity / carrying capacity (S14), NOT pre-money cultivation (S15), NOT money
co-emergence (S16), NOT mortality (later). This is *only* the structural unification.

## Verified Base Facts (oikos @ `68ddff7`)

1. **Spatial = world agent, same id as econ.** `world.add_agent(pos, carry_cap, move_speed)`
   (`world/src/world.rs:390`) returns the next sequential `AgentId`; settlement asserts it
   coincides with the econ id (`settlement.rs:4068-4075`). World agent state: pos, carry,
   carry_cap, move_speed, task (`world.rs:94`). Death removes it (`world.remove_agent`,
   `settlement.rs:5744`).
2. **Founders/newborns are econ-only by choice.** Founders: built with an econ agent, no
   `world.add_agent` (`settlement.rs:4185-4194`); newborns: `society.add_agent(child)` with no
   world add (`:6099-6133`); both `household: Some(_)`, `node: None`. The reason is "hearth-fed,
   never hauls" — a modeling choice (`:4188`), not a constraint.
3. **Only lineages reproduce; the spatial roster is fixed.** `run_births` filters
   `household == Some(h)` (`settlement.rs:6035`); the non-lineage roster is assigned once at
   generation and never grows. So today the growing population is non-spatial and the foraging
   population is fixed — the obstacle this milestone removes.
4. **Consumption + conservation are spatial-agnostic.** Needs advance from the econ
   consumption log (`:5574`); harvest/deposit/transfer are relocations, node regen the only
   source (`:33-52`). A foraging lineage member routes through the same conserved path; no new
   conservation surface.
5. **Econ ids are arena slots with generations; world ids are monotonic (Codex P0).** The
   econ `AgentArena` reuses a freed numeric slot with a **bumped generation** on the next
   insert (`econ/src/arena.rs:189`, `society.rs:4383`, tested `society.rs:7291`), while
   `World::add_agent` only ever appends a fresh monotonic generation-0 id (`world.rs:390`).
   So coincidence-by-insertion-order survives generation but **not the first birth-after-death**
   (the newborn's econ id is a reused `slot#gen`, the world id is fresh `N`). The fix is to
   **mirror the exact econ id into the world** (`add_agent_with_id`), not to align by order.
   `live_colonist_slots` is colonist-insertion order, appended on birth, removed on death
   (`:6135`, `:7999`) — spatial/non-spatial agnostic.
6. **Gating + determinism are established patterns.** Optional config overlays default-off
   (`demography: Option<…>`, `:1382`); per-colonist + world state serialized in `canonical_bytes`
   (`:9859`, world `:9386`/`world.rs:804`), with gated blocks the norm. A `spatial_households`
   flag gates the founder/newborn world-agent block so flag-off configs are byte-identical.

## The slices (build in order; each independently testable)

- **S13.0 — `World::add_agent_with_id` (the id-mirror primitive).** Add
  `World::add_agent_with_id(id: AgentId, pos, carry_cap, move_speed)` that inserts a world
  agent at the **exact** given id (generation included). (Keep `add_agent` for existing call
  sites.) Two invariants (Codex P2): (a) reject the insert if **any** live world agent shares
  the numeric slot `id.index()` — even with a different generation — mirroring the econ arena's
  "one live generation per numeric slot" guarantee; (b) if the id is generation-0 and at/above
  `next_agent_index`, advance `next_agent_index` past it so a later legacy `add_agent` cannot
  collide (a reused `slot#gen` below the watermark leaves `next_agent_index` monotonic — fine).
  **Test:** an arena-style `slot#gen` id round-trips; a same-slot live insert (any generation)
  is rejected; a gen-0 insert bumps `next_agent_index`.
- **S13.1 — founders spatial at generation (mirror the econ id).** Behind `spatial_households`
  (default false): in `generate()`, give each lineage founder a world agent via
  `add_agent_with_id(founder_econ_id, …)` at the exchange, so `world_id == econ_id` by
  construction. Founders stay Idle (no task) and fed exactly as today. **Test:** with the flag
  on, every founder has a world agent at its exact econ id; consumption/feeding unchanged;
  conservation holds; **flag off → byte-identical**.
- **S13.2 — newborns spatial at birth (mid-run, mirror the econ id).** In `run_births`, after
  `society.add_agent(child)`, call `add_agent_with_id(child_econ_id, …)` (flag-on) — mirroring
  the arena's reused `slot#gen` id so the newborn's `world_id == econ_id` even after a death
  recycled the slot. Death already removes the world agent (`collect_estate`). **Test:** a
  newborn born AFTER a death (so its econ id is a reused `slot#gen`) gets a world agent at that
  exact id; coincidence holds across births and deaths over a long run; flag off → byte-identical.
- **S13.3 — lineage forage/gather eligibility (a scoped behavior change) + the scenario/DoD.**
  This slice DOES change behavior (the structural S13.0–S13.2 do not): relax the
  `household.is_none()` eligibility gate in the own-labor/forage assignment
  (`settlement.rs:6541`) — and, where relevant, the re-entry skip (`:7277`) — so spatial
  lineage members can be *assigned* forage/gather/haul tasks (gated by `spatial_households`).
  Add `frontier_spatial_households` (a demographic scenario, flag on). With no forage scarcity
  yet (S14), the *motivation* is absent — so the test demonstrates the **capability** (a
  lineage member assigned a forage task actually moves/forages/deposits), not live foraging.
  **Test:** the clean metric below — conserved, deterministic, capability proven.

## Acceptance Tests (the S13.3 DoD) — `sim/tests/spatial_households.rs`

1. `spatial_households_run_is_deterministic` — byte-identical `(seed, config)`.
2. `lineage_members_are_spatial` — with the flag on, every founder AND every mid-run newborn
   has a world agent whose `world_id == econ_id` (coincidence holds for all, incl. births);
   **every living colonist has a spatial world agent** (the right invariant — the world may
   also hold resident traders, so do NOT equate total world-agent count with colonist count).
3. `lineage_members_can_forage` — a hungry spatial lineage member is assigned a forage/gather
   task, moves, harvests/forages, and deposits (its world carry rises then deposits to econ) —
   the reproducing population can now gather, the structural goal.
4. `feeding_and_demography_unchanged_in_substance` — with the flag on but no scarcity/forage
   pressure, lineage feeding (provision/market) and the demography (births/deaths/plateau)
   behave as before — this milestone adds *capability*, not a behavior change.
5. `spatial_households_conserves` — whole-system conservation every tick across founder +
   newborn world agents foraging/gathering (relocation + node regen; no new source).
6. `id_coincidence_holds_across_births_and_deaths` — over a long run with births and deaths,
   `world_id == econ_id` for every living colonist every tick (the lockstep invariant).
7. `goldens_unchanged` — with `spatial_households` off, S5–S12 scenarios + the six econ +
   g5a/g5b/coemergence + demographic (`lineages`) goldens are byte-identical; S5–S12 suites
   green; the new world-agent/flag state has `canonical_bytes_include_*` regressions; clippy
   `-D warnings`; fmt `--check`.

Manual: `cargo run -p viewer -- run spatial-households --ticks 1600`.

## Missing Interactions (track explicitly)

- **Mirroring the econ id is the crux (Codex P0).** Use `add_agent_with_id` to insert each
  colonist's world agent at its exact econ id (founders + newborns), so coincidence holds by
  construction across the arena's generational slot reuse — NOT by insertion order (which
  breaks on the first birth-after-death). Prove it with a coincidence test where a newborn is
  born after a death (reused `slot#gen`). A `world_id ↔ econ_id` map is the fallback only if
  exact-id insertion proves infeasible.
- **Mid-run world-agent creation is the one new mechanic.** `world.add_agent` works mid-run;
  ensure births call it (flag-on) and that placement (the exchange tile) is always valid;
  deaths already remove (`collect_estate`). Verify no world-agent leak (added on birth,
  removed on death) over a long run.
- **Don't change feeding here.** Spatial lineage members still eat via the econ path; this
  milestone does NOT retire the hearth mint or add forage scarcity (those are S14+). A spatial
  lineage member that isn't assigned a task just sits Idle, fed as before.
- **Eligibility scope.** S13.3 lets lineage members be *assigned* world tasks, but with no
  forage scarcity yet they have little reason to forage (the hearth feeds them). The test
  forces/demonstrates the capability; the *motivation* arrives with S14's forage carrying
  capacity. Keep the eligibility additive and gated.
- **Determinism.** New world agents (founders at gen, newborns at birth) + their positions
  enter `canonical_bytes` only on the flag-on path; flag-off stays byte-identical.

## Handoff Notes

- **This is the structural prerequisite, scoped narrow.** Make lineage members spatial
  (founders at gen + newborns at birth, ids mirrored via `add_agent_with_id`) so the
  reproducing population *can* forage. Do NOT add forage scarcity, cultivation,
  money-from-produced-bread, or mortality — those are the deferred S14→S16 arc this unblocks.
- **Mirror the exact econ id** (`add_agent_with_id`) is the chosen id-coincidence fix; prove
  it with a coincidence test where a newborn is born after a death (arena slot reuse).
- **S13.0–S13.2 are structural (no behavior change); S13.3 is the one scoped behavior change**
  (relax the `household.is_none()` forage-eligibility gate for spatial lineage members).
- **Reuse the existing world/task/deposit machinery** — the new calls are `add_agent_with_id`
  (the id-mirror primitive) at generation + in `run_births`; eligibility extends the existing
  forage/gather assignment.
- **Gate everything** (`spatial_households` default false) so S5–S12 + all goldens stay
  byte-identical; the demographic `lineages` golden is the key tripwire (it has founders).
- Build S13.1→S13.3 as separate commits with their own tests; `git add` new files.
- **Next (the unblocked arc):** S14 forage carrying capacity + endogenous population plateau
  (now the growing population can forage); S15 pre-money own-use cultivation; S16 money from
  produced bread; then mortality (the positive check).
