# Implementation Spec G3a: production chains (content recipes, seeded)

## Purpose

G2 gave colonists needs, space, a spatial economy, and trade — but goods are
only *gathered* (raw from nodes) and *consumed*. G3 adds **production**:
multi-stage transformation of goods via recipes, with tools as productivity
capital. The signature target is the **grain → flour → bread chain** — flour
is the output of one recipe and the input of the next.

Per the game-spec v2 (which flagged G3 as an *emergence* milestone with a
two-step gate), G3 is split:

- **G3a (this milestone): production chains operate, seeded.** A
  content-defined recipe chain runs end-to-end with **seeded** producer roles
  (hand-placed millers/bakers): grain is gathered, milled to flour, baked to
  bread, and the bread is consumed. Conservation holds across the
  transformations. The mechanism works.
- **G3b (next): the chain ARISES from price spreads** with no scripted role
  assignment — entrepreneurs choose to mill/bake because the spread pays.
  Deferred to keep G3a tractable.

It is NOT the emergence of who-produces-what (G3b), NOT the TOML content
loader (content is a code-level `ContentSet` in G3a; the file loader is
deferred and noted), NOT demography (G4), and NOT a change to `econ` market
behavior (the six goldens stay byte-identical).

## Verified Base Facts (2026-06-14, oikos @ `1f2d148`, 796 tests green)

1. **`econ::Recipe` already models the whole mechanism** (project.rs:13):
   `{ id, name, labor, input_good: Option<(GoodId, u32)>, required_tool:
   Option<GoodId>, output_good, output_qty, enabled }`. A single-input
   recipe chains naturally (grain→flour→bread, each one input). `required_tool`
   already models **tools as a capital gate** (a recipe needs a tool good
   held). `execute_direct_recipe_for_agent_*` (sim.rs:702+) applies a recipe:
   consume input + labor, produce output. G3a REUSES this; it adds no recipe
   execution logic to econ.
2. **`GoodRegistry::intern`** (registry.rs:51, from G0b) is the content seam:
   G3a's chain goods (grain, flour, bread, plus tool goods mill/oven) are
   interned, not hard-coded constants.
3. **`world::ResourceNode`** is the only source of raw goods (node.rs); grain
   is gathered from a node, as food/wood are in G2b.
4. **`sim::Settlement` (G2b)** runs the spatial two-rate loop with the
   transfer seam and whole-system conservation; G3a extends its conservation
   to account recipe transformations and adds producer vocations.
5. **`ProjectLine`** (capital.rs:22, multi-input projects) exists but G3a uses
   the simpler single-input `Recipe` path for the production chain; buildings-
   as-multi-input-Projects can use `ProjectLine` in a later slice.
6. **Determinism** inherited: integer, `Rng` at generation only, nothing in
   the loops, id-ordered, `BTreeMap`/`Vec`, no `HashMap`.

## Conservation under transformation (the load-bearing change)

G2b conservation was: per good, Δ = +harvest/regen − consumption (transfer
and movement net-zero). G3a generalizes it: a recipe is an **accounted
conversion** — it consumes `input_qty` of the input good and creates
`output_qty` of the output good, both recorded. The whole-system invariant
becomes, per good X:

```
Δ(total X across world + econ) =
    + harvested/regen X        (raw source, world)
    + recipe output of X       (production, accounted)
    − recipe input of X        (production, accounted)
    − consumed X               (sink, econ)
```

A recipe is not conservation of one good; it is a conserved *transformation*
(input mass → output mass per the recipe's ratio, with the ratio itself the
accounted conversion). The G3a conservation report adds `produced_of(good)`
and `consumed_as_input_of(good)` alongside G2b's harvested/regen/consumed, so
every unit's appearance and disappearance is attributable. Tools are durable
(not consumed by `required_tool`); a recipe needing a tool checks the producer
holds it but does not destroy it.

## Milestone Boundary

G3a includes:

- a code-level `ContentSet` in a new `content` module/crate: interns chain
  goods (grain, flour, bread, mill, oven) and defines the chain recipes
  (gather is a world node; grain+labor+mill→flour; flour+labor+oven→bread);
- producer vocations in `sim` (miller, baker) that apply recipes during the
  econ tick using the existing `execute_direct_recipe` path: consume input
  from stock + labor, produce output to stock, gated by `required_tool`;
- seeded role assignment (hand-placed gatherers/millers/bakers/consumers and
  the tools they hold) — NO emergence of roles (G3b);
- conservation generalized to account recipe transformations (the report
  gains produced/consumed-as-input per good); tools durable;
- a `chain` scenario in the `oikos` viewer (read-only) showing the three
  goods' prices/volumes and conservation OK;
- acceptance tests in `sim/tests/g3a_production.rs`;
- README + `engine-divergence.md` (production via reused `Recipe`; content as
  code-level `ContentSet`; TOML loader and role-emergence deferred).

G3a excludes:

- no emergence of who-produces-what (G3b — roles are seeded);
- no TOML content file loader (content is a code `ContentSet`; loader later);
- no multi-input buildings-as-Projects (uses single-input `Recipe`; later);
- no tool production/wear/depreciation (tools are durable, pre-placed; tool
  economics later);
- no demography (G4); no change to `econ` market behavior (goldens byte-
  identical); only additive `econ`/`sim` accessors;
- no balance tuning or asserted magnitudes beyond the chain operating and
  conserving; no `HashMap` in logic; nothing drawn in the loops.

## Domain Semantics

### Content

```rust
pub struct ContentSet { /* interned goods + recipes for the chain */ }
impl ContentSet {
    pub fn grain_flour_bread() -> Self;   // the G3a chain, code-level
    pub fn goods(&self) -> ...;           // grain, flour, bread (+ mill, oven tools)
    pub fn recipes(&self) -> &[Recipe];   // mill: grain+labor(+mill)->flour
                                          // bake: flour+labor(+oven)->bread
}
```

Goods are interned via `GoodRegistry`; the `ContentSet` is built once at
generation. (Forward-compatible with a TOML loader; the loader is G3-later.)

### Producer vocations and the econ tick

`sim` gains miller/baker vocations. In the econ tick, after gatherers deliver
grain to the exchange and the transfer credits it, a miller (holding a mill
tool) with grain in econ stock applies the mill recipe (consume grain + labor
→ produce flour); a baker (holding an oven) with flour applies the bake recipe
(→ bread); consumers buy and eat bread. Each recipe application goes through
the existing `execute_direct_recipe` path; the sim records produced/consumed-
as-input for conservation. Recipes draw no RNG.

### The chain (DoD)

Seeded roster: grain gatherers, millers (with mills), bakers (with ovens),
bread consumers. Over a run: grain flows node→gather→mill→flour→bake→bread
→consumed. The market prices all three goods from realized trades. The DoD is
that the chain OPERATES end-to-end and CONSERVES (every transformation
accounted) — not that anyone *chose* their role (G3b).

## Implementation Tasks

1. `content` (module or crate): `ContentSet::grain_flour_bread`, interning
   goods and building the two chain recipes (single input, with
   `required_tool`).
2. `sim`: miller/baker vocations; recipe application in the econ tick via
   `execute_direct_recipe`; tool-gated; producer task/role seeding in the
   settlement config.
3. Conservation: generalize the report to account recipe transformations
   (produced_of / consumed_as_input_of per good); tools durable.
4. Any additive `econ`/`sim` accessor needed (apply a recipe to an agent;
   read produced/consumed) — additive only; goldens unchanged.
5. Viewer `chain` scenario (read-only).
6. Tests (below).
7. README + `engine-divergence.md` updates.

## Acceptance Tests

`sim/tests/g3a_production.rs` (+ unit tests):

1. `chain_run_is_deterministic` — same `(seed, config)` → byte-identical run.
2. `grain_flour_bread_chain_operates_end_to_end` — over a seeded run, flour
   is produced from grain and bread from flour, and bread is consumed; all
   three goods see nonzero production/consumption (the chain flows).
3. `production_conserves_with_transformations` — per good, the whole-system
   delta equals harvested/regen + recipe-output − recipe-input − consumed,
   exactly, every econ tick; no unit unaccounted across a transformation.
4. `tools_gate_production_and_are_durable` — a would-be miller WITHOUT a mill
   produces no flour; a miller WITH a mill produces flour and still holds the
   mill afterward (tools are not consumed).
5. `recipe_input_is_consumed_exactly` — applying the mill recipe consumes
   exactly `input_qty` grain per `output_qty` flour produced; no grain leaks
   or is created.
6. `chain_sustains_without_collapse` — a viable seeded chain runs N econ-years
   without collapse (producers and consumers alive, needs bounded); smoke
   only, deterministic.
7. `econ_unchanged` — full workspace suite passes; six econ goldens byte-
   identical; all G1/G2* tests green; `cargo clippy --workspace --all-targets
   -- -D warnings`; `cargo fmt --check`.

Manual check:

```bash
cargo test -p sim
cargo run -p viewer -- run chain --ticks 30
```

## Handoff Notes

- REUSE `econ::Recipe` + `execute_direct_recipe`; add no recipe-execution
  logic to econ. G3a is wiring + conservation generalization + content.
- Conservation now spans transformations: a recipe is a conserved conversion
  (accounted input consumed, accounted output produced). Test 3 is the
  tripwire; tools are durable (not a consumed input).
- Roles are SEEDED in G3a — do not implement role emergence (that is G3b).
  The first test asserting an entrepreneur *chose* to mill is out of scope.
- Content is a code-level `ContentSet` (forward-compatible with a TOML
  loader); do not build the file loader here.
- `econ` market behavior is unchanged: goldens byte-identical; any econ edit
  is an additive accessor. Test 7 confirms.
- Determinism: integer, `Rng` at generation only, nothing in the loops,
  id-ordered, `BTreeMap`/`Vec`, no `HashMap`. Test 1 is the tripwire.
- `git add` new files; gitignore stray build artifacts.
