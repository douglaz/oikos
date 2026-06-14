# OIKOS

Working title. A colony sim / civ builder that runs from a stone-age founding
band to an advanced financialized civilization, built around an
emergent-economy engine forked from the [praxsim](../praxsim) laboratory.

The design constitution is [`docs/game-spec.md`](docs/game-spec.md)
(revision 2, reviewed). The short version of the pitch: the first colony
builder where the economy is real — prices discovered by actual trades
between colonists, money that *emerges* from barter (a different money good
per map), capital structure that lengthens because colonists actually saved,
and late-game financial crises that follow from the player's own monetary
policy by causal necessity.

## Workspace

```
econ/    the economy engine — fork of praxsim-core (pure std, deterministic)
life/    needs → wants: colonist value scales generated from need state (G1)
world/   the spatial substrate — grid, terrain, nodes, stockpiles, movement (G2a)
sim/     the two-rate orchestrator (G2b) + region (G2c) + content & production chain (G3a) + role emergence (G3b)
viewer/  the oikos binary — read-only debug viewer + price/colonist inspectors (G2d)
docs/    the game spec and design documents
```

Future crates per the spec's §4.1: a standalone `content/` crate (a TOML loader
over the `ContentSet` seam G3a establishes as a `sim` module), `ui/` (Bevy
client), `tools/` (headless runners, balance CI). They arrive with their
milestones — empty scaffolding is not kept ahead of need.

## Provenance and the lab relationship

`econ/` was forked at praxsim commit `0729227` (post-M21: the full M0–M21
lab ladder — ordinal value scales, CDA markets, Mengerian money emergence,
banking/fiat/ABCT, the eight-surface tender thread including tax
receivability, and the emergence-robustness instrument with the adopted
M20 envelope). The fork carries the lab's complete test suite as the
engine's **conformance suite**, including the four byte-exact series
goldens (M0/M1/M2/M3) and the M18/M20 emergence goldens — these are the
proof that the fork preserves behavior, and they gate every change to
`econ/` until the engines deliberately diverge (game-spec §10.1).

The praxsim repo stays alive as the clean-room: new economic *mechanisms*
are proven there against its invariant suite, then ported here
(game-spec §13).

## Status: G0b (engine migrations) — complete

Per game-spec §11. G0a forked the lab verbatim; G0b is the first deliberate
divergence — three migrations, each behind a compatibility surface that keeps
the lab goldens byte-identical. Every deliberate divergence is recorded in
[`docs/engine-divergence.md`](docs/engine-divergence.md).

G0a (fork):

- [x] fork `praxsim-core` → `econ`; conformance suite green; lab goldens
      replay byte-identical through the fork
- [x] `aggregate_input_goods` O(N²) scan → order-preserving index map
      (identical output by construction; golden-guarded)
- [ ] per-tick provisioning recompute (Concern-5) — DEFERRED: a real
      caching refactor, not a drop-in; belongs with the G2 perf budget
- [ ] tape retention design — DEFERRED to the inspector/WorldView work
      (G2): an in-memory ring buffer changes test-visible tape contents,
      so it is not a behavior-preserving G0a change; the design decision
      is recorded here rather than smuggled in

G0b (migrations behind compatibility):

- [x] dynamic `GoodRegistry` — goods become data; `lab_default()` interns the
      exact lab set in the exact id order; the `GoodId` constants and
      `good_name` stay as lab-compat surface
- [x] generational `AgentId` — `u32 → u64` packing `(generation, index)`;
      generation-0 ids are byte-identical in ordering and formatting
- [x] `AgentArena` — stable-identity storage replacing `Vec<Agent>` +
      id-resolution; id-ordered, deterministic, no `HashMap`; slot reuse and
      generation bumping unit-tested (no engine path frees yet)
- [x] `Command` result/error semantics — additive `apply_command` returning
      `Applied | Rejected(reason)`, sharing the event path's logic; the
      scenario event path keeps its silent-tolerance semantics

The conformance suite stays green natively and all goldens are byte-identical;
see `econ/tests/g0b_engine_migrations.rs` for the migration acceptance tests.

## Status: G1 (needs → wants, the `life` crate) — complete

Per game-spec §11. G1 adds the `life` crate and the single most important
transformation the game makes to the lab engine: **a colonist's ordinal value
scale is generated from need state each tick, not authored once.** The heart is
one pure, deterministic function:

```
regenerate_scale(&NeedState, &CultureParams, &KnownGoods) -> Vec<Want>
```

It emits wants in strict descending urgency with each marginal unit listed
separately (diminishing marginal utility is positional, no cardinal magnitude),
keeps Leisure always present (so labor supply stays emergent), is satiation-
monotone, and is never empty. The need set is the load-bearing trio that maps
onto existing lab goods — hunger↔FOOD, warmth↔fuel (WOOD), rest↔Leisure.

A lean `Camp` driver (the pre-`sim` stand-in, to be absorbed by `sim` at G2)
feeds that output to the **real, unchanged** econ market: a camp that feeds,
fuels, and rests itself through trade and labor. Death by starvation was a
**tombstone** in G1 — the colonist marked dead, its scale emptied, dropped from
activation with its holdings frozen in place. **G4a retired the tombstone for
real removal**: a starved colonist's estate now settles to a commons, its arena
slot is freed, and the `Society` caches reconcile (see the G4a section below); the
G1/`Camp` tests were migrated to that semantics.

G1 is deliberately mechanism-only and pre-spatial: the acceptance suite asserts
scale-generation *properties* and non-collapse, never balance numbers. `life`
adds no econ economic-behavior change — the `econ` edits are additive public
hooks/accessors for reading consumption, invalidating stale quotes after a scale
rewrite, and removing starvation deaths, proven harmless by the unchanged
conformance suite. See `life/tests/g1_needs_to_wants.rs` for the eleven
acceptance tests and `docs/engine-divergence.md` for the death seam (real
removal as of G4a).

G1:

- [x] `life` workspace crate (depends on `econ`, pure std, deterministic)
- [x] `NeedState` (hunger/warmth/rest) + integer per-tick dynamics
- [x] `CultureParams` (time-preference / leisure-weight, integer bps)
- [x] `regenerate_scale` — the pure, deterministic milestone function
- [x] `Camp` driver: generate colonists, update needs, remove starvation deaths
      (real removal as of G4a), regenerate scales, step the econ market, read
      consumption/labor back
- [x] additive-only `econ` hooks/accessors (read price/labor/consumption,
      invalidate stale quotes after scale rewrites, death seam);
      goldens byte-identical
- [x] acceptance suite + divergence-log and README updates

## Status: G2a (the `world` crate — spatial substrate) — complete

Per game-spec §11. G2 in the roadmap bundles four large pieces — the `world`
crate, the two-rate loop with the §4.3 delivery-escrow contract, the
Society-monolith extraction for multiple settlements, and the debug viewer — into
one milestone. That is too much for one reviewed change (G1, a pure function plus
a driver, took eight rounds), so **G2 is decomposed** (this supersedes the
single-G2 lump in game-spec §11):

- **G2a (this milestone): the `world` crate** — the spatial substrate as a
  standalone, econ-*independent* component.
- **G2b: two-rate loop + escrow** — wire `world` delivery under the econ tick via
  the §4.3 delivery-escrow contract for one settlement (distance affects realized
  prices; escrow conserves exactly).
- **G2c: settlement-scoped service extraction** — pull market/labor/barter books
  out of the `Society` monolith so multiple settlements exist.
- **G2d: debug viewer + inspectors** — the first binary; the price→trades and
  colonist→scale-and-why inspectors the game-spec mandates for G2.

G2a is the lowest-risk slice and the foundation the rest build on. The `world`
crate is a **pure spatial substrate**: a tile grid with passable/impassable
terrain, resource nodes (a good, a stock, an optional regen), stockpiles
(capacity-bounded storage), and agents with positions and carried inventory that
move along **deterministic BFS shortest paths** around obstacles and harvest /
deposit on arrival.

It knows positions, terrain, movement, harvest yields, and storage — and **no
economics**: no prices, money, wants, or trades (those are G2b). It depends on
`econ` only for the shared primitives `GoodId` / `AgentId` / `Rng`; it calls no
econ economic logic and changes no econ behavior, and `econ` does not depend on
`world`, so the conformance goldens and the G1 tests are safe by construction.

Two invariants are the contract. **Determinism:** integer state, the `Rng`
consumed at world *generation* only (`tick()` draws nothing), `AgentId`-ordered
iteration, `BTreeMap`/`Vec` only — same seed + same command sequence yields a
byte-identical world. **Conservation:** node regen is the *only* source of goods
(clamped to `cap`, fully accounted in a per-tick report); movement, harvest, and
deposit relocate units without ever creating or destroying one, and deposits that
overflow a stockpile's capacity stay carried, never destroyed.

G2a:

- [x] `world` workspace crate (depends on `econ` for primitives, pure std
      otherwise, deterministic)
- [x] `Grid` + `Pos` + terrain (passable / impassable), placement validation
- [x] `ResourceNode` (good, stock, optional regen, cap) with conserving harvest
- [x] `Stockpile` (capacity-bounded, integer contents) with overflow-safe deposit
- [x] agent spatial state (position + carried inventory) and `Task`
      (go-to-node-and-harvest, go-to-stockpile-and-deposit, go-to-tile)
- [x] deterministic BFS pathfinding around impassable terrain with a fixed
      tie-break; unreachable targets set `Blocked` (no panic)
- [x] `World::tick()` — movement, arrivals, regen, and a per-tick conservation
      report; `World::generate(seed, &WorldGen)`; query accessors
- [x] acceptance suite (`world/tests/g2a_world.rs`, eleven tests) + per-module
      unit tests; divergence-log and README updates

See `world/tests/g2a_world.rs` and `docs/engine-divergence.md` (the G2a entry and
the recorded G2 decomposition).

## Status: G2b (two-rate loop + delivery escrow, the `sim` crate) — complete

Per game-spec §11 (the G2 decomposition above). G2b makes space **economically
meaningful** by wiring `world` delivery under the economy. The new `sim` crate is
the two-rate orchestrator (§4.1, §4.3): a `Settlement` owns a `world::World`,
per-colonist `life` need state, and an `econ::Society`, and runs the fast loop
(`FAST_TICKS_PER_ECON_TICK` `world` ticks of movement / harvest / haul) under one
economic tick (transfer → needs/tombstone → scale regeneration → market clearing
→ consumption read-back → task reassignment). `sim` **supersedes** `life::Camp` as
the driver (Camp stays as the G1 non-spatial reference harness).

The load-bearing design is the **world→econ transfer seam**: a good has one owner
at a time — `world` (node / carry / stockpile) **or** `econ` (agent stock). The
econ-tick transfer is the only crossing and it is net-zero (`world` −n, `econ`
+n): delivered exchange-stockpile units are credited to the depositing
colonist's econ stock and withdrawn from the world. If stock headroom is not
available yet, they remain world-owned in the exchange and retry later.
Carry-while-moving **is** the §4.3 delivery escrow — there is no separate escrow
ledger — so goods that don't arrive (a blocked or dead hauler) are **retained**
in carry, never destroyed.

The milestone proves two things. **Conservation is exact:** every physical good
is accounted across its full node → carry → stockpile → econ → consumed lifecycle,
with node regen the only source and consumption the only sink, checked every econ
tick. **Distance affects realized prices:** a node farther from the exchange
delivers fewer units per econ tick, so the good realizes a strictly higher price
(sign only — no magnitude is pinned). `sim` reuses `world` / `life` / `econ`
as-is; the only engine edits are two additive, conserving accessors that realize
the seam (`world::World::stockpile_withdraw`, `econ::Society::credit_stock`),
proven harmless by the unchanged eight conformance goldens. Determinism is
inherited and mandatory: integer state, the `Rng` consumed only at generation,
nothing drawn in either loop, `AgentId`-ordered iteration, `BTreeMap`/`Vec` only.

G2b:

- [x] `sim` workspace crate (`world` + `life` + `econ` path deps, pure std,
      deterministic)
- [x] `Settlement` / `SettlementConfig` orchestrator: a `World`, per-colonist
      `NeedState`/`CultureParams`, and a `Society`; one exchange stockpile and
      resource nodes at chosen positions
- [x] the two-rate loop + haul-escrow: in-transit (carried) goods accounted as
      escrow; arrival transfers to econ; non-arrival (blocked/dead) retains
- [x] the world→econ transfer seam — additive `world` withdraw + `econ`
      stock-credit accessors; goldens byte-identical
- [x] a whole-system conservation check + per-econ-tick report; realized-price
      and digest accessors
- [x] acceptance suite (`sim/tests/g2b_two_rate.rs`: the eight acceptance tests
      + three unit tests) + per-module unit tests; divergence-log and README updates

See `sim/tests/g2b_two_rate.rs` and `docs/engine-divergence.md` (the G2b entry:
the `sim` crate, the transfer seam, escrow-as-carry, and `sim` superseding
`Camp`).

## Status: G2d (debug viewer + inspectors, the `oikos` binary) — complete

Per game-spec §11 (the G2 decomposition above) and pillar 3 / §8 (legibility).
G2d delivers the workspace's **first runnable artifact** — the `oikos` binary, a
headless, deterministic, text-only debug viewer — and the two inspectors the G2
roadmap mandates: **price → the trades behind it** and **colonist → its value
scale and why**. This is what turns "passing tests" into "something you can
watch."

The new `viewer` crate is a thin binary over `sim`: it renders settlement state
entirely from `sim`'s existing read-only accessors (and `sim`'s read-only
re-exports of the `econ`/`life` types), and **changes no `econ`/`world`/`life`/
`sim` behavior** — the six econ conformance goldens and the G1/G2a/G2b suites
stay green and byte-identical. Commands:

```bash
cargo run -p viewer -- run viable --ticks 20
cargo run -p viewer -- inspect price price-probe --good food --at-tick 14
cargo run -p viewer -- inspect colonist viable --id 1 --at-tick 10
cargo run -p viewer -- scenarios
cargo run -p viewer -- help
```

- **`oikos run <scenario>`** prints a per-econ-tick dashboard: living colonists
  by vocation, realized price per tracked good (or `—` if none cleared),
  conservation `OK`/`VIOLATED:<good>`, a needs summary (max/mean living hunger),
  and transferred/consumed totals.
- **`oikos inspect price <scenario> --good NAME`** prints the realized price for
  a good at a tick and **exactly** the `society().trades` behind it (buyer,
  seller, price, qty) — the answer to "why is the price N?".
- **`oikos inspect colonist <scenario> --id N`** prints the colonist's ranked
  value scale (each want's kind/horizon/satisfied), needs, vocation, alive/dead,
  carried escrow, and gold — the answer to "why did this colonist do that?". A
  dead colonist (G4a real removal) shows as dead with its estate settled to the
  commons (its arena slot freed).

Three contracts hold it together. **Determinism:** the run is seeded and the
viewer draws no RNG, so the same `(scenario, ticks, seed)` prints byte-identical
output (the acceptance suite's tripwire). **Loud errors:** an unknown scenario,
unknown flag, or missing required argument prints a message plus the usage block
— never a silent default or a panic. **Text-only and dependency-free:** no TUI,
color, or graphics crates (that is G9), std formatting only, no `HashMap` in
logic. Renderers return a `String` (never write stdout directly) so the output
is unit-testable; `main` just prints it.

G2d:

- [x] `viewer` workspace crate producing the `oikos` binary (`sim` path dep,
      pure std; also a library so the renderers are unit-testable)
- [x] hand-rolled arg dispatch mirroring the lab `prax` CLI; `run` / `inspect
      price` / `inspect colonist` / `scenarios` / `help` subcommands
- [x] the price→trades and colonist→scale/why inspectors, rendered from
      read-only `sim` accessors; additive read-only re-exports on `sim` (the
      `econ`/`life` types the viewer names), goldens byte-identical
- [x] a scenario registry (`viable`, `price-probe`, `near`/`far` for the
      distance contrast, `starved-hauler`) with `near-node`/`far-node` aliases
- [x] acceptance suite (`viewer/tests/g2d_viewer.rs`: the seven acceptance tests
      + extras) and per-module unit tests; README + divergence-log updates

See `viewer/tests/g2d_viewer.rs` and `docs/engine-divergence.md` (the G2d entry:
the first binary, the read-only viewer, and the additive `sim` re-exports).

## Status: G2c (multiple settlements + caravans, the `Region`) — complete

The final G2 slice: **multiple settlements exist and trade**, completing the
revised G2 (space → space-meets-economy → viewer → here). The game-spec frames
this as "extract settlement-scoped services from the `Society` monolith." We
reach the *end* — several independent settlement economies that trade — **by
composition, not internal surgery**: a `Region` holds N self-contained
`Settlement`s (each **unchanged** from G2b — its own `World` + `Society`), linked
by an abstract inter-settlement **route**, with a **caravan** carrying a good from
where it is cheap to where it is dear. No `Society` and no `Settlement` internal
behaviour changes, so the six econ goldens and the whole G1/G2a/G2b/G2d suites
stay byte-identical *by construction*.

The caravan is the load-bearing design. Runtime agent-roster mutation (the
`AgentArena` free/cache reconciliation) was G4-deferred at G2c (it lands in G4a),
so a caravan is a **pair of permanent resident trader agents** — one per linked
settlement, created at generation — and the `Region` shuttles their **wealth**
between them as route escrow, never the agents. So each settlement's agent count
is constant for the whole run. A trader takes the lowest id in its settlement (it
leads the id-ordered market) and is otherwise inert (an empty value scale posts no
orders) until the `Region` activates it for a buy or a sell.

It proves two things, the DoD:

1. **Region-wide conservation is exact.** For every good and for all gold, the
   total over all settlements **plus** the in-transit route escrow changes only by
   accounted node regen (the source) and consumption (the sink). Every caravan
   transfer is **net-zero** (the additive `econ` accessors `debit_stock` /
   `credit_gold` / `debit_gold` MOVE value, never mint or burn it), and escrow in
   transit is conserved and **retained** if a leg never completes — never
   destroyed. The `Region` roll-up is the ledger, checked every econ tick.
2. **Trade converges prices.** With a caravan active between two settlements that
   price a good differently, the realized-price gap narrows over time **versus a
   no-caravan control** (the falsification twin keeps the gap). Sign only — no
   price magnitude is pinned.

```bash
cargo test -p sim                                     # incl. sim/tests/g2c_region.rs
cargo run -p viewer -- run region --ticks 30          # per-settlement prices + gap
cargo run -p viewer -- run region-control --ticks 30  # the no-caravan twin
```

G2c:

- [x] a `Region` type in `sim` (holds `Vec<Settlement>` unchanged + a route + a
      caravan + a region-wide conservation roll-up + per-settlement realized-price
      and gap accessors); `RegionConfig` with a `caravans_enabled` control toggle
- [x] abstract inter-settlement **routes** (a transit-tick count) and the
      **caravan** operator (permanent trader pairs; `Region`-shuttled wealth as
      route escrow; a deterministic buy-low/sell-high on the realized differential)
- [x] additive, conserving `econ` accessors `debit_stock` / `credit_gold` /
      `debit_gold` (reject unknown/tombstoned ids, never go negative, move value),
      goldens byte-identical
- [x] a read-only `region` / `region-control` scenario in the `oikos` viewer
      (per-settlement prices + the convergence gap over time)
- [x] acceptance suite (`sim/tests/g2c_region.rs`: the eight acceptance tests +
      extras) and per-module unit tests; README + divergence-log updates

This completes the revised G2. See `sim/tests/g2c_region.rs` and
`docs/engine-divergence.md` (the G2c entry: multi-settlement by composition, the
caravan-as-trader-pair model, and why no `Society` internal extraction).

## Status: G3a (production chains — content recipes, seeded) — complete

G2 gave colonists needs, space, a spatial economy, and trade, but goods were only
*gathered* and *consumed*. G3a adds **production**: multi-stage transformation of
goods via recipes, with tools as productivity capital. The signature target is the
**grain → flour → bread chain** — flour is the *output* of one recipe and the
*input* of the next. Per the game-spec's two-step G3 gate, this is the **seeded**
half: the chain operates end-to-end with hand-placed producer roles. That the
chain *arises* from price spreads (entrepreneurs choosing to mill/bake because the
spread pays) is **G3b**, deliberately deferred.

The mechanism is **reused, not rebuilt**. `econ::Recipe` already models a recipe —
`{ labor, input_good, required_tool, output_good, output_qty }` — and a single-
input recipe chains naturally (grain→flour→bread, each one input). G3a is content
+ sim wiring + a conservation generalization, **not** new recipe logic in `econ`:

- **content as a code-level `ContentSet`** (`sim/src/content.rs`): the chain goods
  (grain, flour, bread, plus the mill/oven tools) are **interned** via
  `econ::GoodRegistry` (ids after the lab catalog, `grain = 7 … oven = 11`), and
  the two chain recipes are built as data. A TOML content-file loader is deferred
  (game-spec G3-later); the `ContentSet` API is the shape that loader will fill.
- **seeded producer vocations** in `sim` (`Miller`, `Baker`): hand-placed, holding
  their durable tool. In the econ tick's new **production phase** — after the
  market, so a producer has its bought input on hand — each applies its recipe
  through `Society::execute_direct_recipe_for_agent_checked`, an additive wrapper
  around econ's existing `execute_direct_recipe_for_agent` path. It consumes input
  + produces output, **gated by the held tool**, preflights output headroom, and
  returns the accounted conversion for the conservation report. Roles are seeded,
  not emergent (G3b).
- **conservation generalized across transformations.** A recipe is a *conserved
  conversion* — it consumes an accounted input and produces an accounted output.
  The whole-system invariant becomes, per good X:

  ```text
  Δ(total X) = +regen +recipe_output −recipe_input −consumed
  ```

  The report gains `produced_of` / `consumed_as_input_of` alongside G2b's
  regen/consumed. **Tools are durable**: `required_tool` is checked but never
  consumed, so it never moves the ledger.

It proves, the DoD:

1. **The chain operates end-to-end.** Over a seeded run grain flows
   node→gather→mill→flour→bake→bread→consumed; every stage sees nonzero activity
   and the market prices all three goods from realized trades.
2. **Conservation holds across the transformations, exactly, every econ tick** —
   no unit is unaccounted across a recipe; tools never wear (the tripwire).

`econ` market behaviour is **unchanged**: the six econ goldens stay byte-identical
and every G1/G2a/G2b/G2c/G2d test is green — every `econ` edit is an additive
accessor (the `Society::intern_good` naming seam, the checked direct-recipe
execution seam, and two `RecipeId` variants), `ContentSet` lives in `sim`, and the
`chain` config field is opt-in. Determinism is inherited: integer state, the `Rng`
consumed only at generation, nothing drawn in the loops or the production phase,
`BTreeMap`/`Vec` only.

```bash
cargo test -p sim                          # incl. sim/tests/g3a_production.rs
cargo run -p viewer -- run chain --ticks 30  # the three goods' prices + conservation
```

G3a:

- [x] a code-level `ContentSet` (`sim/src/content.rs`): interns the chain goods +
      tools and builds the grain→flour→bread recipes (single-input, tool-gated)
- [x] seeded `Miller` / `Baker` vocations + a production phase that applies the
      recipes through econ's checked direct-recipe accessor (reusing
      `econ::Recipe` and `execute_direct_recipe_for_agent`, durable tools, exact
      input); tool-gated; producer roster in the config
- [x] additive `econ` edits only — `RecipeId::Mill`/`Bake`, `Society::intern_good`,
      `Society::execute_direct_recipe_for_agent_checked`, a `PartialEq`/`Eq`
      derive on `Recipe`; market behavior and goldens unchanged
- [x] conservation generalized (produced / consumed-as-input per good; tools
      durable) in the `EconTickReport`
- [x] a read-only `chain` scenario in the `oikos` viewer (the three goods'
      prices/volumes + conservation OK)
- [x] acceptance suite (`sim/tests/g3a_production.rs`: the seven acceptance tests)
      and per-module unit tests; README + divergence-log updates

Deferred to later G3 slices: role **emergence** (G3b — who produces what arises
from the spread), the **TOML content loader** (content stays a code `ContentSet`),
multi-input buildings-as-`Project`s (G3a uses single-input `Recipe`s), and tool
production/wear (tools are durable, pre-placed). See `sim/tests/g3a_production.rs`
and `docs/engine-divergence.md` (the G3a entry: production via the reused
`Recipe`, content as a code-level `ContentSet`, conservation under transformation).

## Status: G3b (production roles emerge from price spreads) — complete

G3a proved the grain→flour→bread chain *operates* with **seeded** producer roles.
G3b removes the seeding: a colonist **chooses** to mill or bake because the realized
price spread pays — entrepreneurship the praxeology-honest way. This is the
emergence half of G3. It proves the **mechanism on a curated config with a
falsification control** (mirroring how the lab proved money emergence): the chain
forms from prices when a profitable spread exists, and does **not** form when the
spread is removed. The multi-seed robustness *study* (the "≥X% of N random worlds"
gate, analogous to M18/M19 for money) is **deferred** to a possible G3-study
milestone; G3b is the mechanism + control, not the robustness number.

The choice is **ordinal and reused, not rebuilt**. A pool of colonists hold latent
production capital — a `mill` or an `oven` — and start in a new `Unassigned`
vocation. Each econ tick, after needs advance and scales regenerate, every such colonist
**re-appraises** the recipe it could run:

- **`recipe_adoption_pays`** (in `sim`) frames running the recipe once as a project
  bundle — sell its output at the realized output price for a future receivable,
  costing the realized input price plus a per-operation operating cost — and hands
  it to `econ`'s M2.5 **`appraise_project_bundle_for_money`** (the same machinery the
  lab planner uses). That returns `Some` iff the revenue−cost spread newly provisions
  a **future-gold (savings) want on the colonist's own value scale** without breaking
  a higher want. There is **no scalar profit number** and **no argmax across
  colonists**: each decides for itself, in id order — the §pillar-1 "colonists act"
  rule applied to occupation. `econ` is reused unchanged; G3b adds no `econ` edit.
- **Adopt / revert from the spread.** A positive spread (and an unprovisioned savings
  want) makes the colonist adopt `Miller`/`Baker`; re-running the appraisal every tick
  makes the role *sticky while the spread holds* and *revert when it collapses*.
- **The chain prices itself bottom-up.** Bread is the staple, so consumer demand
  prices bread; that lets a latent baker adopt on the bread−flour spread and start
  buying flour, which prices flour, which lets a latent miller adopt on the
  flour−grain spread, which prices grain. No role is hand-placed — the producer mix
  *arises*.
- **The bootstrap is mechanical, not a seeded role.** Latent millers start with a
  small flour stock they do not reserve, so the first baker has flour to buy and the
  middle good can realize a price. They still start `Unassigned`; the stock is only
  the price-discovery bridge that lets the ordinal spread appraisal fire.

It proves, the DoD:

1. **Roles emerge from the spread.** In `emergent-chain` (no seeded roles), over a
   run at least one colonist adopts milling and at least one adopts baking, and bread
   is produced and consumed — the chain forms from prices alone.
2. **No spread, no roles (the falsification control).** In `emergent-chain-control`
   the spread is removed (the grain node and latent pool stay fixed, but bread is
   not the staple, so bread demand is absent and bread/flour never realize a price).
   The **same** role-choice appraisal, run over the **same** latent pool every tick,
   adopts no production vocation and produces no flour or bread. Paired with (1)
   this shows the spread is what creates the roles.

`econ` market behaviour is **unchanged**: the six econ goldens stay byte-identical
and every G1/G2*/G3a test is green — the role-choice reuses `econ`'s existing
`appraise_project_bundle_for_money` (no `econ` edit), the `Unassigned` vocation and
the emergent configs live in `sim`, and the chain field stays opt-in. The praxeology
source-gate (no decision module reads an aggregate; the choice is ordinal) still
holds — the decision reads only per-good realized prices and the colonist's own
scale. Determinism is inherited: integer state, the `Rng` consumed only at
generation, **nothing drawn in the role-choice or production phases**, id-ordered,
`BTreeMap`/`Vec` only.

```bash
cargo test -p sim                                   # incl. sim/tests/g3b_emergence.rs
cargo run -p viewer -- run emergent-chain --ticks 40          # roles adopt + the three prices
cargo run -p viewer -- run emergent-chain-control --ticks 40  # no spread → no roles
```

The emergence window is the **first ~20 ticks**: roles adopt and the chain prices
itself bottom-up there. Long-run colony *viability* (keeping every mouth fed over a
full horizon) is **G4 demography work**, not G3b — a 40-tick `emergent-chain` run
shows the roles form and operate, then the curated buffers drain and some colonists
starve, which is why the milestone asserts role formation and conservation, never
survival. Use a shorter `--ticks` to watch just the emergence if the late-run
drain distracts.

G3b:

- [x] ordinal role-choice in `sim`: unassigned colonists appraise and adopt
      miller/baker vocations from realized spreads (reusing `econ`'s
      `appraise_project_bundle_for_money` — no scalar optimizer, no `econ` edit), with
      a per-tick re-appraisal so a role reverts when its spread collapses
- [x] an `emergent-chain` config (no seeded producer roles) and a `flat-prices`/
      no-bread-demand control config (the falsification twin)
- [x] a read-only `emergent-chain` viewer scenario showing roles adopted + the three
      prices (and `emergent-chain-control`)
- [x] acceptance suite (`sim/tests/g3b_emergence.rs`: the seven acceptance tests) +
      per-module unit tests; README + divergence-log updates

Deferred (noted in `docs/engine-divergence.md`): the **multi-seed robustness study**
(the "≥X% of N worlds" gate) — G3b is the mechanism + control, not the robustness
number. See `sim/tests/g3b_emergence.rs` and `docs/engine-divergence.md` (the G3b
entry: ordinal entrepreneurship for occupation; robustness study deferred).

## Status: G4a (real death — arena free, estate, cache reconciliation) — complete

Every milestone since G0b deferred one piece: actually **removing** a colonist from a
running `Society`. G0b built `AgentArena::free` but parked its Society-cache
reconciliation; G1 tombstoned the dead (froze them in place); G2c's caravans dodged
roster changes with a permanent trader pair. **G4a lands that deferred core** — the
engine-integration half of demography — isolated from the demographic *mechanics*
(births, aging, households, inheritance), which are G4b.

When a colonist starves it is removed for real. `Society::tombstone` is replaced by
`Society::remove_agent`, which runs the load-bearing order of operations: **settle**
the estate (gold + econ stock) into a returned `Estate`, **cancel** its resting orders
and release their reservations, **free** the arena slot (`AgentArena::free`, bumping
the slot generation so the id resolves to `None` and the slot is reusable), then
**reconcile** every external cache that held a position or an id — `agent_order`
(rebuilt at the relocated positions), `reservations` / `loan_reservations` /
`labor_reservations` (the dead id forgotten), the labor/loan/spot books (orders
cancelled), `barter_book` (offers/reservations forgotten), dead-owned
`project_funding_plans` (reserved gold released and unstarted plans expired), and an
empty M3 `MoneySystem` balance. A non-empty M3 ledger balance is refused before
removal because routing that estate is G4b. The `sim` `Settlement` and the `life`
`Camp` route the returned estate, plus the dead colonist's world-carried delivery
escrow and any stranded exchange-deposit escrow (both drained out of the world), into
a **commons** — a conserved, sim-owned sink that joins `total_gold` and
`whole_system_total`, so whole-system conservation holds **across** the death: nothing
is created or destroyed, only relocated. Estate-to-heirs is G4b; G4a settles to the
commons.

The goldens are safe **by construction**: the lab never frees an agent, so the
free + reconcile path is game-only, an empty commons is omitted from the canonical
digest (it joins only once a death settles an estate into it), and the no-death hot
path is byte-identical. The six econ goldens and the existing G1/G2*/G3* digest suites
stay byte-identical.

G4a:

- [x] `Society::remove_agent(AgentId) -> Option<Estate>` (settle → cancel → free →
      reconcile), replacing the G1 `tombstone`; `Estate { gold, stock }`
- [x] external-cache reconciliation: `agent_order` rebuild + `forget_agent` on the
      spot/labor/loan/barter reservations, dead-owned project-funding plans frozen,
      and empty M3 `MoneySystem` entries dropped
- [x] `World::withdraw_agent_carry` — drain a dead colonist's world escrow to the commons
- [x] `sim`/`life` estate-to-commons (a conserved sink in `Settlement` and `Camp`),
      folded into `total_gold` / `whole_system_total`
- [x] G1 tombstone tests migrated to real-removal semantics (slot freed and reusable)
- [x] acceptance suite (`sim/tests/g4a_death.rs`: the eight acceptance tests) + econ
      arena/reconcile unit tests; README + divergence-log updates

Deferred (noted in `docs/engine-divergence.md`): births/aging/households/inheritance
and estate-to-heirs (G4b), non-empty M3 ledger estate routing (G4b), and the
population-stability study (G4b/later). See `sim/tests/g4a_death.rs` and
`docs/engine-divergence.md` (the G4a entry).

## Build and test

```bash
cargo test          # full conformance suite incl. goldens
cargo clippy --workspace --all-targets -- -D warnings
cargo fmt --check
```

## Run

The `oikos` binary (G2d) is the workspace's first runnable artifact:

```bash
cargo run -p viewer -- help          # usage
cargo run -p viewer -- scenarios     # list the scenarios
cargo run -p viewer -- run viable --ticks 20
cargo run -p viewer -- run chain --ticks 30           # G3a: grain→flour→bread chain
cargo run -p viewer -- run emergent-chain --ticks 40          # G3b: roles emerge from the spread
cargo run -p viewer -- run emergent-chain-control --ticks 40  # G3b: no spread → no roles
cargo run -p viewer -- run region --ticks 30          # G2c: two settlements + a caravan
cargo run -p viewer -- run region-control --ticks 30  # the no-caravan twin
cargo run -p viewer -- run starved-hauler --ticks 20  # G4a: a colonist dies, the run continues
```
