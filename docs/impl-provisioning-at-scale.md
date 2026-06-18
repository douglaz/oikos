# Implementation Spec: provisioning at scale via productive re-entry (S6)

> The deferred milestone was named "S6 — scaling / churn: replacement producers
> so output tracks a growing population." Grounding it in the current code
> (read-only investigation, oikos @ `cde6223`) shows that framing is imprecise on
> two counts, so this spec **reframes** it the way the Codex spec-review reframed
> the endogenous milestone. The honest target is: **no colonist is permanently
> stranded at high hunger — a hungry, unprovisioned colonist (whether idle or
> mis-allocated to non-edible work) takes up the lowest available means of feeding
> itself (gathering edible food) driven by its own hunger, so provisioning keeps
> pace as the colony lives and grows.**

## Why the original framing is imprecise (two grounded findings)

**Finding A — population is bounded, not "growing" unboundedly.** Births are one
child per household per `birth_interval`, capped at `max_household_size` living
members per household (`sim/src/demography.rs:75-82`). `frontier` seeds **2
households × cap 5 = 10** lineage members max (`sim/src/settlement.rs:2065-2096`),
plus a **fixed** non-lineage roster of **18** (4 consumers + 8 gatherers + 6 latent
producers, `:2125-2126`, `:2035-2036`). The config hard cap is therefore **28**
(18 + 10); the viewer plateaus near 26 by birth/old-age timing, not by construction.
Either way it is bounded — it does not outgrow output unboundedly. The "output
can't keep up with growth" story is not what the run shows.

**Finding B — the hungry p95 tail is a *stranded fixed subset*, not a scaling
gap.** The Codex review measured tail hunger mean ~3 but p95 ~12. That p95 is a
fixed set of **non-lineage, non-producer, cash-poor** colonists — chiefly the 4
non-lineage `consumers` (`household: None`, `node: None`, produce nothing) and the
WOOD-node gatherers. There are exactly three food paths and none covers them:
1. the **household hearth** `deliver_demography_provisions` feeds only
   `household.is_some()` colonists (lineage members + newborns)
   (`sim/src/settlement.rs:4950-4970`);
2. the **producer-subsistence hearth** `run_producer_subsistence` feeds only
   Miller/Baker/latent producers — everyone else hits `continue`
   (`:5404-5408`);
3. the **edible-grain fallback** (`subsistence_on_grain` → `known.subsistence`,
   `:3036`; eaten at `:4586-4592`) feeds only a colonist that **holds grain**.

A non-lineage consumer has no hearth, produces nothing, holds no grain, and once
its starting gold circulates away (spoilage prevents hoarding,
`perishable_decay_bps = 1500`, `:2463`, `run_spoilage` `:5739-5788`) it has **no
acquisition path at all**. And because `frontier` sets `hunger_critical =
need_max + 1` (`sim/src/settlement.rs:2147-2151`), a starving colonist **never
dies** (only old age culls, and only lineage members have a lifespan) — so it
persists forever at high hunger, forming the stationary p95 tail.

So the defect to fix is **a permanently-stranded, non-producing, hearth-less
underclass**, not output-vs-growth. The praxeologically faithful remedy is the
same instinct the deferred note had ("a hungry colonist takes up an unserved
trade") — but the trade it can actually take **without capital** is *gathering*,
not the tooled chain (see the carve-out).

## The carve-out: tooled-trade scaling needs producible capital (defer to S7)

The deferred note assumed a hungry colonist could become a *replacement Miller/
Baker*. The code forbids this today, and fixing it is a **separate, larger**
milestone:

- A colonist is eligible for a chain vocation only if `Colonist.latent =
  Some(Mill|Bake)` (`sim/src/settlement.rs:2709`), assigned **write-once at
  generation** (`:3365`) — there is **no** `.latent =` assignment anywhere in
  `sim/src`, and every newborn is hard-coded `latent: None` (`:5116`). The pool of
  potential chain producers is frozen at 6 and can only shrink.
- `run_role_choice` re-appraises **only** latent colonists (`let Some(latent) =
  ... else { continue }`, `:5876`); a non-latent colonist can never adopt a chain
  role.
- Production is **tool-gated**: Mill needs `required_tool: Some(mill)`, Bake needs
  `Some(oven)` (`sim/src/content.rs:123,133`); the executor returns `None` if the
  tool is not held (`econ/src/sim.rs:781-797`). **No recipe or project ever
  produces a mill or oven** (grep for `output_good: mill/oven` → none; only
  generation seeding endows them, `:7509/7514/7526/7531`), and a top-ranked "tool
  anchor" scale want stops a producer from ever selling its tool (`:8001-8004`).

So a *new* chain producer is impossible without either (a) a higher-order
capital-good recipe that **produces** mills/ovens, or (b) tool inheritance/transfer
from an exiting producer (which the tool anchor currently prevents). Both are real
econ-model changes (recipes, tiers, the scale anchor) and deserve their own
milestone. **S6 does NOT touch tools.** It scales the *untooled subsistence base*
beneath the chain — which is exactly what feeds the stranded tail.

## Purpose & the honest bar

Make **provisioning universal on the untooled subsistence base**: a hungry,
unprovisioned colonist **appraises edible gathering as the lowest-capital means to
relieve its felt hunger and adopts that occupation** — an individual action on its
own value scale, not a planner assignment. Over a long horizon this must leave **no
colonist permanently stranded at high hunger**: the hunger tail (p95 **and** max,
**and** the count of chronically-hungry colonists) drops below the endogenous
baseline and stays bounded and non-drifting as the colony lives and grows — **without
regressing the S5 bread chain or starving the WOOD supply** (see Missing
Interactions).

NOT tool-making / higher-order capital goods (S7), NOT replacement *chain*
producers, NOT firms/wage labor, NOT a change to econ market-clearing (the six
conformance goldens stay byte-identical; any econ edit additive and gated), NOT
raising `hunger_critical` to hide the tail, NOT a planner hearth handout to the
tail (they must **earn** subsistence by producing — the hearth-extension shortcut
is rejected precisely because it is the curated-placement pattern this project has
been removing).

## Verified Base Facts (oikos @ `cde6223`)

1. **The stranded set exists and never dies** (Finding B above): non-lineage,
   non-producer, cash-poor colonists, fed by none of the three food paths, kept
   alive by `hunger_critical = need_max + 1` (`:2147-2151`).
2. **Colony gathering is the world fast-loop, and it is untooled.** Colonists do
   not gather via the econ `GatherFood`/`CutWood` recipes; they gather in the world
   loop — `assign_idle_gatherer_tasks` (`sim/src/settlement.rs:4548`) issues
   `Task::GoHarvest(node, carry_cap)` (`:4562`), and the world deposit credits the
   harvested good back into econ stock. This node-harvest path has **no tool gate**,
   so a colonist needs no capital to gather. A grain-node gatherer accumulates
   grain, **eats it** via the edible-grain fallback, and can **sell the surplus**
   (grain has real buyers in the endogenous run — the millers). This both feeds the
   colonist and supplies the chain.
3. **Vocation is already flipped mid-run for latent colonists** by
   `run_role_choice` (`:5848`/`:5868`), so changing a colonist's vocation in a gated
   phase is an established pattern — but it has only ever flipped *latent* colonists,
   and only between `Unassigned`↔`Miller/Baker`. Re-entry must (a) flip a
   *non-latent* `Consumer` (`node: None`) to a grain `Gatherer`, and (b) re-point a
   hungry `Gatherer` mis-allocated to a non-edible node (WOOD) onto the edible grain
   node — both new.
4. **Only the original spatial roster has a world agent.** Consumers/gatherers/
   latent producers are generated as spatial colonists with world agents (`:3280-
   3375`, `household: None`); a non-lineage consumer has a world agent and
   `node: None`, so flipping it to `Gatherer` with a node makes
   `assign_idle_gatherer_tasks` work. But **demography founders and newborns are
   non-spatial** (no world agent, hearth-backed; newborns created at `:5088-5122`
   with no world insertion). Re-entry is therefore **scoped to the spatial
   non-lineage roster** — which is exactly the stranded set; founders/newborns are
   hearth-fed and not stranded. Covering them would require building world-agent
   creation, which is out of scope. This scoping is the **highest implementation
   risk** — see Handoff.
5. **Conservation, determinism, goldens** as in the endogenous milestone
   (`EconTickReport::conserves()` every tick; `canonical_bytes` digest; econ
   byte-identical regression). A new chain/runtime knob that steers future ticks
   must be added to `canonical_bytes` with a regression test (the digest tripwire —
   see the four-flag fix in `cde6223`).
6. **Rich read-only accessors exist** for per-colonist tests: `need_of(i)`,
   `is_alive(i)`, `vocation_of(i)`, `household_of(i)`, `population()`,
   `living_count(vocation)`, `whole_system_total(good)`, `max_living_hunger()`
   (`sim/src/settlement.rs:6704-6961`). The acceptance tests use these directly.

## The slices (build in order; each independently testable)

- **S6.1 — the productive re-entry phase (the core, highest risk).** A gated,
  additive `econ_tick` phase (default OFF; runs before the market) that, for each
  live **spatial non-lineage** colonist (Base Fact 4 — has a world agent;
  `household: None`) which is **hungry above a threshold H_in and not currently
  feeding itself**, has it adopt edible gathering. Two stranded subsets are covered:
  (a) an idle `Consumer` (`node: None`) — or non-latent `Unassigned` — becomes a
  grain `Gatherer`; (b) a `Gatherer` mis-allocated to a non-edible node (WOOD) is
  re-pointed to the **edible grain node** (a hungry actor gathers food before wood —
  hunger outranks wood-for-trade on its scale). The decision is **per-colonist, on
  that colonist's own value scale** (it appraises gathering as the lowest-capital
  way to relieve felt hunger), never a global quota. Skip latent/seeded chain
  producers and anyone on a hearth. Gate it so lab/conformance scenarios never run
  it. **Unit test:** a stranded, hungry, cash-poor non-lineage consumer becomes a
  `Gatherer` on the grain node, **actually accumulates grain (via `carry_of` / a per-
  colonist stock accessor) and eats it, and its hunger falls** over the following
  ticks (assert food acquired, not just the label). **Regression:** with the phase
  off, every existing scenario (incl. `endogenous`) is byte-identical and the econ
  goldens are unchanged.
- **S6.2 — re-entry is sticky and reversible (no thrash).** Re-entry must not churn
  every tick. Decide and document the hysteresis: re-enter at hunger ≥ H_in; a fed
  re-entrant may return to WOOD gathering / idle only below H_out < H_in (so a
  colonist that has relieved its hunger can resume wood work, keeping the WOOD
  supply alive — see Missing Interactions). **Test:** a re-entrant holds its role
  across many ticks without per-tick thrash, and the tail vocation mix is stable.
- **S6.3 — the scaling scenario + DoD.** Compose S6.1/S6.2 onto
  `frontier_endogenous` as `frontier_endogenous_scaling` / a new `scaling`
  scenario. To exercise *growth* (not just the fixed stranded set), also seed a
  larger colony and/or raise `max_household_size` so population climbs further,
  and show provisioning still holds. **Test:** the clean metric below.

## Acceptance Tests (the S6.3 DoD)

`sim/tests/provisioning_at_scale.rs`:

1. `re_entry_run_is_deterministic` — same `(seed, config)` → byte-identical
   (`canonical_bytes` and `digest`).
2. `no_colonist_is_permanently_stranded` — THE clean metric, and it must not count
   a still-hungry "active" colonist as success. Over the tail (e.g. last 400
   ticks), **every** living high-hunger colonist (hunger ≥ H_in for K consecutive
   sampled ticks) must be **actually obtaining food** — on the **edible** (grain)
   node *and* its grain carry/stock or grain-eaten is rising — or on a hearth. A
   WOOD gatherer at hunger 12 is **still stranded**, not a pass. Measured via
   `vocation_of`/`household_of`/`need_of` **plus** a `node_of` accessor and
   `carry_of`/per-colonist grain stock (add `node_of` and, if needed, a
   per-colonist `stock_of`).
3. `hunger_tail_is_reduced_not_moved` — the tail is **fixed, not relocated**:
   versus the `endogenous` baseline, tail **p95**, **max** (`max_living_hunger`),
   **and the count of chronically-hungry colonists** all drop, and stay
   non-drifting first-vs-last window. (p95 alone can be gamed by moving the tail to
   one fewer colonist.)
4. `provisioning_tracks_population` — with a larger/growing colony (bigger seed
   and/or raised `max_household_size`), per-capita food *eaten* (grain + bread) and
   the hunger tail stay bounded as population rises across tail windows.
5. `bread_chain_does_not_regress` — S5 is preserved: with re-entry ON, `bread.made`
   is still > 0 through tick 1600, Miller/Baker adoption does not collapse
   (`living_count` of each stays > 0 in the tail), and real grain/flour input
   `Society::trades` by active producers still occur after tick 300 (the S5 clean
   metric). **Specifically guard the subsistence↔specialization tension
   (Experiment 4):** mass raw-grain eating must not gut bread demand and de-adopt
   the chain, and a grain glut from many new gatherers must not crash grain's price
   so far that millers de-adopt — assert bread production and miller adoption hold.
6. `wood_supply_does_not_collapse` — WOOD does not become the new tail: whole-system
   WOOD made/held stays bounded and non-drifting across the tail (the S6.2
   hysteresis lets fed colonists return to wood).
7. `re_entry_conserves` — whole-system conservation every tick across the new phase
   (the flip mints nothing; gathering is the existing conserved node-regen source).
8. `endogenous_unchanged` — with the re-entry phase OFF, the `endogenous` scenario
   and its 6-test suite are byte-identical; the six econ conformance goldens are
   byte-identical; clippy `-D warnings`; fmt `--check`.

Manual: `cargo run -p viewer -- run scaling --ticks 1600` (and compare the
`endogenous` p95/max tail before/after).

## Missing Interactions (track explicitly — Codex)

- **Subsistence vs. specialization (the recurring tension, Experiment 4).** Raw
  grain is directly edible (`subsistence_on_grain`); if re-entry sends many
  colonists to eat grain, **bread demand can fall and weaken the chain** — the same
  trade-off that crowded out the chain in Experiment 4. Re-entry must feed the
  stranded tail *without* collapsing bread. Test 5 guards this.
- **Grain glut → miller de-adoption.** More grain gatherers raise grain supply; if
  grain's realized price crashes, the recurring-motive/bundle appraisal could
  de-adopt millers. Watch grain price and miller `living_count`.
- **WOOD starvation.** Pulling WOOD gatherers to grain (S6.1b) reduces WOOD; the
  S6.2 hysteresis (fed colonists resume wood) must keep WOOD bounded. Test 6.

## Handoff Notes

- **Highest risk: a flipped colonist must become a *functioning* gatherer, not a
  relabel.** Scope re-entry to the **spatial non-lineage roster** (Base Fact 4):
  those colonists already have a world agent, so flipping vocation + assigning a
  node makes `assign_idle_gatherer_tasks` (`:4548`) issue `GoHarvest` and the world
  deposit credit grain into econ stock. Do **not** apply re-entry to demography
  founders/newborns (non-spatial, no world agent, `:5088-5122`) — they are
  hearth-fed and not stranded; covering them would mean building world-agent
  creation (out of scope). The S6.1 unit test must assert *grain actually
  accumulated and hunger actually fell* — not just that the vocation label changed.
- **Test accessors:** only `carry_of(index, good)` exists today (`:6740`); there is
  no `node_of` or per-colonist econ-stock reader. Add a read-only `node_of(index)`
  (and a per-colonist grain `stock_of` if `carry_of` is insufficient — carry is
  world-carry, not deposited econ stock) so the tests can prove edible-node
  assignment and real food acquisition. Keep additions read-only.
- **Keep it praxeological:** re-entry is the *individual colonist* acting on its
  own hunger given an available opportunity, not a central allocator filling a
  quota. No global "assign N gatherers"; each stranded hungry colonist decides for
  itself. Do not reorder anyone's value scale.
- **No planner handout.** Do not feed the tail via an extended hearth — that is the
  curated-placement anti-pattern. They earn subsistence by gathering.
- **Digest discipline:** any new steering knob → `canonical_bytes` + a
  `canonical_bytes_include_*` regression test (cf. `cde6223`).
- **Gate everything** so conformance/lab scenarios are inert (the disabled-phase
  byte-identical regression is the tripwire).
- Build S6.1→S6.3 as separate commits with their own tests. `git add` new files.
- **S7 (separate, later):** producible capital goods — a higher-order recipe that
  makes mills/ovens (or tool inheritance from exiting producers, relaxing the tool
  anchor on death/retire) so the **tooled** grain→flour→bread chain can also scale,
  not just the gathering base. That is the genuine "replacement chain producer"
  path the original note imagined; it is out of scope here.
