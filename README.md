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
empty M3 `MoneySystem` balance. A non-empty M3 ledger balance was refused before
removal at G4a (routing that estate was deferred — now **resolved in G8a**, which drains
the specie into the `Estate`). The `sim` `Settlement` and the `life`
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
- [x] `World::withdraw_agent_carry` + `World::remove_agent` — drain a dead colonist's
      world escrow to the commons, then remove the spatial agent from future world ticks
- [x] `sim`/`life` estate-to-commons (a conserved sink in `Settlement` and `Camp`),
      folded into `total_gold` / `whole_system_total`
- [x] G1 tombstone tests migrated to real-removal semantics (slot freed and reusable)
- [x] acceptance suite (`sim/tests/g4a_death.rs`: the eight acceptance tests) + econ
      arena/reconcile unit tests; README + divergence-log updates

Deferred (noted in `docs/engine-divergence.md`): births/aging/households/inheritance
and estate-to-heirs (G4b), non-empty M3 ledger estate routing (specie resolved in G8a;
fiat/claims deferred to G8b/c), and the population-stability study (G4b/later). See
`sim/tests/g4a_death.rs` and
`docs/engine-divergence.md` (the G4a entry).

## Status: G4b (births, aging, households, culture inheritance) — complete

G4a gave the engine real death (runtime removal + estate + cache reconciliation).
**G4b completes demography**: colonists **age**, **die of old age** (via G4a's removal
path), are **born** into **households** when the household can support them, and
children **inherit** their parents' `CultureParams` with bounded mutation — so time
preference drifts under selection across generations. This is the first milestone where
the population is not a fixed cast.

The insert-side mirror of G4a lands first: `Society::add_agent(Agent) -> AgentId` inserts
into the arena (a fresh or reused slot with a fresh generation) and **reconciles every
external cache** — it appends the new agent's position to `agent_order` and materializes
its spot-reservation slot — so the newborn participates from the next econ tick. It is
the exact mirror of `remove_agent`: a missed cache would be a birth that can't trade.
Like removal, no lab scenario adds an agent at runtime, so the path is game-only and the
**six econ goldens stay byte-identical by construction**.

The `sim` `Settlement` gains an opt-in `demography` overlay (`None` for every pre-G4b
config, so they are byte-identical; `Some` activates the mechanism), seeded as
**households** of non-spatial householders:

- **aging + old-age death** — each colonist tracks an age (econ ticks) and a
  deterministic lifespan derived from a stable per-colonist seed (`onset + hash(seed) %
  span`); when `age ≥ lifespan` it dies through `remove_agent`. No `Rng` in the loop.
- **births** — a household that clears a need-security margin (its members fed) under a
  size cap and past a birth interval bears one child: a new colonist with an
  inherited+**mutated** culture (a hash of the parent's culture and the colony's
  monotonic birth sequence — no `Rng`), endowed by a **conserved transfer** debited from
  a parent's unreserved balances (a FOOD buffer plus a best-effort gold gift), added via
  `add_agent`.
- **estate → heirs** — a death's estate routes to a living household member (the heir);
  if the lineage is extinct it falls back to the **commons** (G4a's sink). Conserved
  either way — nothing is created or destroyed, only relocated within the whole system.
- Demography state is digest-honest: when the overlay is present, canonical bytes include
  both future-steering config knobs (provisions, birth cadence, endowments, mutation/lifespan
  parameters) and runtime counters. The no-overlay path omits all of it and remains
  byte-identical to pre-G4b runs.
- Long-run cleanup is live-roster based: dead colonists remain inspectable by generation
  index, but hot phases iterate a compact live-slot roster and id lookup is by stable
  generational `AgentId`; spatial dead agents are removed from `World` after estate drain.
- **culture inheritance** is the selection substrate: `CultureParams::inherit` nudges
  each field by a bounded, deterministic delta, and the heritable ordinal patience bias
  does its work through `regenerate_scale` (G1) — there is no scalar fitness function.

The curated `lineages` config seeds a **patient** household and a **present-biased** one,
identical but for time preference and a wood provision. Both are food-secure (so deaths
are old age, not starvation) and both reproduce; the patient lineage sells its wood
surplus and **out-accumulates** the present-biased one, which spends its gold down buying
warmth (sign only — the multi-seed selection study is deferred). The `oikos run lineages`
dashboard surfaces population, cumulative births/old-age deaths, and per-lineage wealth
tick over tick. Scope is the **mechanism + curated demonstrations**: population sustains
in a band (births ≈ deaths, no extinction or blowup), inheritance mutates
deterministically, estates route to heirs, and a patient lineage out-saves an impatient
one — not a tuned population number or a statistical selection gate.

G4b:

- [x] `Society::add_agent(Agent) -> AgentId` (insert + agent_order/reservation
      reconciliation), the insert-side mirror of `remove_agent`; goldens byte-identical
- [x] `CultureParams::inherit` — bounded, deterministic culture mutation (hash of parent
      params + birth sequence, no `Rng`)
- [x] `sim` demography overlay: aging + old-age death (via `remove_agent`), households,
      births (via `add_agent`, debiting unreserved parent balances), estate-to-heirs
      (commons fallback), a headroom-clamped renewable provision as a conserved source; the
      `lineages` curated config
- [x] viewer: the `lineages` scenario + population/births/deaths/per-lineage-wealth
      surfacing
- [x] acceptance suite (`sim/tests/g4b_demography.rs`: the eight acceptance tests plus
      digest-regression coverage) + econ `add_agent` and `life` `inherit` unit tests;
      README + divergence-log updates

Deferred (noted in `docs/engine-divergence.md`): the **multi-seed stability/selection
studies** (the game-spec's 100-seed stability band and a multi-seed selection study,
analogous to M18/M19 for money emergence), inter-settlement migration, and non-empty M3
ledger estate routing (**resolved in G8a** — M3 demography now drains/credits specie
through the ledger). See `sim/tests/g4b_demography.rs` and `docs/engine-divergence.md`
(the G4b entry).

## Status: G5a (money emerges from spatial barter) — complete

Every settlement before G5a ran on econ's **designated GOLD** market — money was
assumed. **G5a makes money emerge.** A curated **barter camp** starts with no
designated money: gatherers haul FOOD and WOOD from two nodes, colonists barter
goods-for-goods at the exchange, and a money good is **promoted** by the
Mengerian saleability rule the lab proved (M5/M6) and studied (M18/M19/M20) — but now
driven by **spatial** trade. It is the spatial counterpart of the lab's
money-emergence result, sliced down to the **mechanism + a falsification control**.

G5a is **spatial wiring + a curated config + a control** — it adds **no** emergence
rule to econ. The barter camp runs `MarketMoneyConfig::Emergent` (econ's V2 path):
`step_v2` clears the `BarterBook`, the realized spatial barter feeds econ's reused
`SaleabilityTracker`, and when `MengerianEmergence::winner` fires the winning good's
stock converts to money units (the lab's **conserved promotion**) — after which the
settlement runs the existing G2b money-priced market unchanged. No money moves in the
fast loop; barter swaps relocate goods and the promotion converts good→money exactly,
so whole-system conservation holds across the barter → promotion → money phase
transition.

The `sim` `Settlement` gains an opt-in `barter` overlay (`None` for every pre-G5a
config, so they stay byte-identical; `Some` runs the emergent path):

- **barter phase** — colonists demand a durable **SALT** medium via a `Horizon::Next`
  "hold the medium" want layered on the value scale (the same slot a chain uses for
  producer inputs, not a need-model change). Its universal demand — traded against both
  the FOOD and the WOOD that specialist gatherers sell — makes SALT the good accepted
  against the most counterparts, the most saleable, so it is the good that emerges.
- **promotion** — on the tick the reused `winner` rule fires, SALT's econ stock is
  converted to gold 1-for-1 (the lab's conserved promotion), recorded in the tick
  report's `promoted` ledger so the whole-system identity balances across the phase
  transition. From the next tick trade is money-priced (the G2b market).
- **the control** — `barter-camp-control` is the same camp with the SALT medium's
  **supply removed**. The same emergence machinery runs over the same FOOD/WOOD barter,
  but the only swaps that clear are perfectly reciprocal FOOD-for-WOOD, so no good ever
  leads by the promotion margin and **nothing monetizes** — the settlement stays in
  barter. This is the milestone's proof: the saleable medium, not luck, is what
  monetizes. (If both monetized, the wiring would be reading something other than
  realized spatial barter.)
- digest-honest: when the overlay is present, canonical bytes include the savings good,
  the current money good, the promotion tick, and the **full Mengerian emergence runtime**
  (the saleability tracker's accumulated per-candidate acceptances and distinct
  acceptor/counterpart sets, plus the promotion-timing latch) — all of which steer the
  future promotion decision, so the determinism tripwire spans the phase transition. The
  no-overlay path omits all of it and stays byte-identical to pre-G5a runs.

The only econ edits are **additive**: read-only accessors on `Society`/`MengerianEmergence`
(emergence state, promotion tick, saleability leader, the adopted config), a
consumption-log capture in `step_v2` that is **inert unless the log is enabled**, and an
opt-in V2 step boundary that lets the spatial sim reject world-regenerated node goods as
unsupported money goods. The normal econ `step()` path uses no rejection list, so the
**six econ goldens stay byte-identical**.

G5a:

- [x] a barter-start `Settlement` mode (`MarketMoneyConfig::Emergent` driving the V2
      barter/saleability/promotion machinery inside the spatial two-rate loop)
- [x] the spatial→saleability wiring (realized spatial barter feeds the reused
      `SaleabilityTracker`; the Mengerian `winner` rule promotes) + the conserved
      promotion transition to the G2b money market
- [x] the curated `barter-camp` config (monetizes) and the `barter-camp-control`
      falsification twin (does not), plus the `barter` overlay on `SettlementConfig`
- [x] viewer: the `barter-camp`/`barter-camp-control` scenarios + barter/money phase,
      saleability leader, and promotion-tick surfacing
- [x] acceptance suite (`sim/tests/g5a_emergence.rs`: the seven acceptance tests plus
      unit tests) + README + divergence-log updates

Deferred (noted in `docs/engine-divergence.md`): the **multi-seed spatial robustness
STUDY** (emergence rate under encounter/transport frictions, analogous to M18/M19 for the
lab's non-spatial money emergence). G5a is the mechanism slice — a plain gatherer/consumer
barter camp; composition with the full stack is **G5b** (next). See
`sim/tests/g5a_emergence.rs` and `docs/engine-divergence.md` (the G5a entry).

## Status: G5b (emergence composed with the full stack — the `frontier`) — complete

G5a/G3b/G4b each proved one emergent phenomenon in isolation. **G5b composes all three
in ONE settlement.** `SettlementConfig::frontier()` is a barter camp where money
**emerges** (G5a), then producers take up milling/baking from the resulting **money price
spreads** (G3b), while **births and deaths** run demographic selection (G4b) — all
conserving and deterministic. It proves the simulation composes: the whole economic
foundation (G1 needs → G2 space/trade → G3 production → G4 demography → G5a money) runs as
one coherent society, not as separate demos.

G5b is **composition, not new mechanism**: G5a money emergence, G3b role-choice, and G4b
demography are reused unchanged. The work is ordering them coherently in one econ tick, a
combined config, and fixing the interaction bugs the combination surfaces:

- **the combined econ tick** — FAST gather/haul → TRANSFER world→econ → EXCHANGE
  (pre-promotion spatial barter + saleability + promotion check; post-promotion the money
  market) → PRODUCTION (recipes; latent colonists appraise and adopt roles) → DEMOGRAPHY
  (needs, old-age + starvation deaths, births into households, estates to heirs) → MEASURE
  (whole-system conservation over every pool + flow).
- **the economic ordering is load-bearing** — appraising a flour−grain spread needs
  realized *money* prices, which exist only after promotion. So **production roles emerge
  only AFTER money does** (role-choice is gated on the post-promotion money phase): a
  division of labor presupposes a medium of exchange. No role is adopted during the barter
  phase. The role-choice appraisal is threaded with the settlement's *current* money good
  (the emergent **SALT**, not assumed GOLD), so the appraisal and the market agree on what
  the future savings want is.
- **conservation with ALL flows at once** — a single econ tick can run a barter swap (a
  relocation, net 0), the promotion conversion (good→money, exact), a recipe transformation,
  a birth endowment + a death estate (transfers), harvest/regen, and consumption — and the
  whole-system identity still balances, including the awkward coincidence of a birth on the
  promotion tick.
- **the veto list now bites** — the promotion-rejection list covers every **renewable**
  source: the spatial nodes, the chain's recipe outputs, AND (newly, because demography is
  active) the household hearth's provisioned goods. So a demography-provisioned staple
  (bread) cannot monetize; money emerges on the durable, non-renewable **SALT** medium —
  or not at all.
- **interaction fixes the combination surfaced** — the generation guard that made barter
  mutually exclusive with production/demography is lifted (with new guards that every
  composed gold source is zero before promotion and the medium is non-renewable); the
  emergent-medium endowment now lands on the chain path too; the demography hearth
  provisions the settlement's hunger staple (FOOD on a `lineages` colony, bread on the
  frontier) so members are fed the good they eat. Every change keeps the no-overlay paths
  structurally unchanged, so the six econ goldens and all G1/G2*/G3*/G4*/G5a tests stay
  byte-identical. The only econ touch is **additive accessors** (none here — G5b reuses
  G5a/G3b/G4b's).

G5b:

- [x] the combined `frontier` config (barter-start + production roles + demography) and the
      coherent econ-tick phase ordering so all three coexist
- [x] role-choice gated on the money phase (roles follow money) + the appraisal threaded
      with the current (emergent) money good
- [x] the promotion-rejection list extended to recipe outputs and the demography hearth, so
      a renewable/provisioned good cannot monetize
- [x] whole-system conservation with all flows active simultaneously (barter, promotion,
      recipes, births, deaths), including a birth on the promotion tick
- [x] viewer `frontier` scenario surfacing the phase, money good, producer roles, and
      population together
- [x] acceptance suite (`sim/tests/g5b_frontier.rs`: the seven acceptance tests plus unit
      tests) + README + divergence-log updates

Deferred (noted in `docs/engine-divergence.md`): the multi-seed robustness **study** and
**multi-settlement** composition (the Region with all overlays). G5b is a single combined
settlement. See `sim/tests/g5b_frontier.rs` and `docs/engine-divergence.md` (the G5b entry).

## Status: G6a (era detection — eras are earned, not timed) — complete

The frontier (G5b) already passes through institutional phases — forage, barter, a money
good emerges, producers specialize, a roundabout chain runs — but nothing *named* the era.
**G6a adds the era detector**: a read-only classification of the settlement's institutional
era from **measured** quantities, with hysteresis, surfaced in the viewer. This is game-spec
pillar 2 — *"eras are earned, not timed"* — and the lab's *"phase is measured, never set"*
doctrine: the era is a derived statistic, never a state the engine sets or a timer advances.

`sim::EraDetector` classifies an **ordered** ladder from existing accessors:

```text
Forager     — no sustained exchange (negligible barter volume)
Barter      — sustained reciprocal exchange (cumulative barter trade volume)
Money       — a money good has been promoted (current_money_good is Some)
Specialist  — a sustained division of labor (producer-role share ≥ a floor over a window)
Capital     — sustained roundabout production (both chain stages staffed: a produced
              intermediate is itself consumed as a recipe input) over a window
```

It is **measurement-only**, the discipline the milestone is about:

- **Era is MEASURED, never set.** The detector reads only `sim`'s read-only accessors
  (vocations, the money good, barter volume, population), mutates nothing (`observe` borrows
  `&Settlement`), draws no RNG, and holds no `HashMap` — so the era timeline is a pure
  function of the run. Running a settlement with vs without a detector observing it is
  **byte-identical**, and the six econ goldens and every prior G1–G5 test stay green by
  construction (era detection writes no econ/sim state).
- **No decision reads the era** (purism). Like econ's `metrics` module, the era is a layer no
  decision path may import — a **source-gate** test enforces it, so running with vs without
  querying the era cannot change a run.
- **Hysteresis is the anti-flap rule.** An era is *entered* only when its trigger holds for a
  sustained window of ticks, and is not abandoned on a single-tick dip: the reached era only
  regresses when the current rung's trigger fails for a sustained window. Eras are ordered;
  the detector tracks the reached era and the first tick each rung was earned (never cleared
  by a later regression). Barter and Money are monotonic *milestones* (a camp that has
  bartered, a money good that has been promoted, do not un-happen); Specialist reads the live
  producer-role share, and Specialist/Capital are ongoing *structure* the window protects from
  flapping.
- **No new econ measurement.** It reuses the existing signals — nothing new is measured in
  `econ`.

G6a:

- [x] `sim::EraDetector` (read-only) — the measured era ladder with hysteresis; reports the
      current era + each rung's first-tick, with a pure `apply_triggers` hysteresis core
- [x] viewer surfacing — an era **banner** (the timeline of earned rungs) and a per-tick `era`
      column in the frontier dashboard
- [x] acceptance suite (`sim/tests/g6a_eras.rs`: the six acceptance tests plus unit tests) +
      README + divergence-log updates

The **Credit** and **Modern** eras (chartered banks, state money) were **deferred to G8**: they
need finance machinery that did not exist in the game at G6a, and G6a does not invent
placeholder finance to reach them. G8a lays the M3 ledger foundation (specie money) and G8b adds
banks/credit, but neither adds a rung; **G8c-1 unlocks both** — the detector now reaches **Credit**
when institutionally-created credit circulates and **Modern** when state fiat is the marginal medium
(measured, with hysteresis; see the G8c-1 status section). Era detection is also **not**
research/tech-tier unlocking (G6b). See `sim/tests/g6a_eras.rs` and
`docs/engine-divergence.md` (the G6a entry).

## Status: G6b (research & tech tiers — capabilities are earned, not timed) — complete

G6a *names* the era a society has earned; **G6b lets it advance its capabilities**: a
**scholar** vocation produces **Knowledge** from labor, and crossing a Knowledge threshold
**unlocks a higher tech tier** — a recipe that was gated until then. Progression is
research-driven (Knowledge accumulated by actual scholar labor), not a timer — the tech
analogue of the *"earned, not timed"* pillar. G6b proves the **mechanism** for ONE tier
unlock (tier 1 → tier 2) with **seeded** scholars, plus a **control** (no scholars → no
unlock).

The `research` scenario is the seeded grain→flour→bread chain (G3a) plus scholars and a
confectioner:

```text
scholar    holds a library; runs grain + labor → Knowledge (the research recipe)
Knowledge  a per-settlement ACCUMULATOR — monotonic, never traded or consumed
tier 2     the confect recipe (flour + labor + atelier → pastry) starts enabled:false
unlock     Knowledge ≥ threshold → the confect recipe flips enabled:true (one-way)
pastry     the higher-order good produced only AFTER the unlock (impossible before)
```

It reuses the existing machinery — no new recipe gating in `econ`:

- **Tier gating reuses `Recipe.enabled`.** A tier-2 recipe starts `enabled: false`; crossing
  the threshold flips it `true` via one additive `econ` accessor (`Society::set_recipe_enabled`)
  called by no engine path. The direct-recipe executor already refuses a disabled recipe, so a
  confectioner produces **nothing while gated even while holding its flour input** — the tier
  gate, proven by the `tier_gate_blocks_pre_unlock` test.
- **Knowledge is an ACCUMULATOR, not a tradeable good.** Scholar Knowledge output runs through
  the existing production path, but `sim` immediately drains it into a per-settlement counter:
  it is monotonic, never traded or consumed, and lives **OUTSIDE the goods-conservation
  ledger** (reported on its own non-conserved line, `knowledge_produced`). The good **inputs**
  to research (grain) ARE conserved-consumed and accounted exactly like consumption — so
  whole-system goods conservation still holds every tick (`research_inputs_conserve`, the
  tripwire).
- **The no-scholars control is the proof.** With the scholars removed, Knowledge never
  accumulates, so the tier-2 recipe stays disabled and pastry is never produced — even though
  the confectioner is present and holds its inputs the whole time. If the tier unlocked there,
  the gate would be reading time (or anything other than research).
- **The unlock is per-settlement, deterministic, and one-way.** Integer state, the `Rng` drawn
  only at generation, nothing drawn in the loops, no `HashMap` — so the same `(seed, config)`
  is byte-identical down to the unlock tick. Once unlocked, the tier never re-disables (no
  flapping).
- **econ recipe-execution behaviour is unchanged.** Scholars, Knowledge, and tiers are
  game-only (`sim`); the lab uses none of them, so the six econ conformance goldens are
  byte-identical by construction and every prior G1–G6a test stays green.

G6b:

- [x] a `Knowledge` accumulator + `Scholar`/`Confectioner` vocations + per-recipe tier
      metadata + the per-settlement unlock in `sim`
- [x] `ContentSet::research_tiers` — the chain extended with the research and gated tier-2
      recipes (and the Knowledge / pastry / library / atelier goods)
- [x] `research` config (unlocks tier 2) + `research-control` (no scholars → never unlocks)
- [x] viewer surfacing — a research **banner** (Knowledge / tier / unlock tick) and per-tick
      `know` / `k.tick` / `tier` columns
- [x] acceptance suite (`sim/tests/g6b_research.rs`: the seven acceptance tests plus unit
      tests) + README + divergence-log updates

**Multi-tier tech trees, knowledge diffusion via trade (game-spec §5.7), building-defs (vs
recipe-defs), and emergence of the scholar role** are **deferred** — G6b proves one unlock with
seeded scholars. See `sim/tests/g6b_research.rs` and `docs/engine-divergence.md` (the G6b
entry).

## Status: G7 (roads — infrastructure cuts trip cost) — complete

G2c proved a **caravan** converges two settlements' prices; **G7 adds a road** — the one
genuinely-new trade mechanic the game-spec reserved for this slot. A road is a **public-works
project built from community labor** that, once complete, **cuts the route's transit cost**, so
caravans cycle faster and the realized-price gap converges faster — infrastructure investment with
a measurable return, and the first **public works** in the game. Per the §5.9 funding ladder, state
taxation does not exist yet (G8), so a G7 road is **community-funded by labor**, not a state
treasury. Scope is ONE road on the ONE G2c route, with a **no-road control** proving the road is
what speeds convergence.

The `roads` scenario is the two-settlement caravan region on a longer route, plus a road the
community builds from labor:

```text
road       a public-works Project on the route, built from contributed labor
labor      every living colonist contributes each tick (community labor, gated on a living pop)
materials  conserved community stock (WOOD) drawn from a region road fund as the road is built
complete   labor ≥ cost → the route transit drops (20 → 8 here), one-way (never flaps)
effect     fewer transit ticks → faster caravan cycles → the price gap converges faster
```

It reuses the existing machinery — no new project/labor system in `econ`:

- **The road is COMMUNITY LABOR, reusing the G3 project-labor path.** Colonists contribute labor
  to the road `Project` each tick (the reused `econ::project` `start`/`advance`/`complete`
  lifecycle), gated on a living population — it is community labor, not a timer, and **not** a
  state-treasury expenditure (taxation-funded works are G8). The only `econ` edit is an additive
  `ProjectTemplateId::BuildRoad` variant + a `build_road_template` constructor kept **out of**
  `builtin_project_templates`, so the lab planner never sees it and the goldens are byte-identical.
- **The build is a conserved expenditure that creates no good.** A road changes an abstract route's
  transit cost, not the physical ledger: the template's `output_qty` is `0`. Its optional conserved
  materials are drawn from a region road fund and accounted as `consumed_as_input` **every tick
  across the build**, so whole-system conservation holds throughout; the labor itself is abstract
  (as in G3/G6b) and reported on its own non-conserved line. Building creates no good and destroys
  none beyond the labor/inputs spent (`road_build_conserves`, the tripwire).
- **The effect is a one-way route `transit_ticks` cut.** On completion the route's `transit_ticks`
  (the G2c field, reused) drops to a defined amount below the unbuilt route; the caravan /
  convergence machinery is otherwise unchanged. Once built the reduction **stays** — the road step
  returns early forever, so it never flaps (`road_is_one_way`).
- **The no-road control is the proof.** `roads-control` is the same region and caravan on the same
  route with no road, so the road — not the caravan, which G2c already had — is the only difference.
  With the road the gap is tighter at a fixed horizon than the control's, which keeps a wider gap.
  If both converged identically the road would not be cutting transit. Sign only — no magnitude is
  pinned.
- **econ behaviour is unchanged.** The `Region` and the road are game-only (`sim`); the lab uses
  neither, so the six econ conformance goldens are byte-identical by construction and every prior
  G1–G6b test stays green.

G7:

- [x] a road public-works `Project` on a `Region` route (community labor + a conserved materials
      fund) and the transit cut on completion, in `sim::region` (`RoadPlan`, `Region` road state)
- [x] `roads` config (the road builds, convergence accelerates) + `roads-control` (no road →
      slower convergence)
- [x] viewer surfacing — the region dashboard's `transit` and `road` (build-progress) columns
      alongside the convergence gap
- [x] acceptance suite (`sim/tests/g7_roads.rs`: the seven acceptance tests plus unit tests) +
      README + divergence-log updates

**State-funded public works / taxation (G8), road networks, grid-pathable roads, and >2
settlements / multi-route topology** are **deferred** — G7 is one community-labor road on the one
abstract route. See `sim/tests/g7_roads.rs` and `docs/engine-divergence.md` (the G7 entry).

## Status: G8a (the M3-ledger money settlement — finance foundation) — complete

Every settlement through G7 ran on **closed-GOLD M1** money (`Agent.gold`, no ledger). **G8a** is
the finance foundation: it runs the spatial settlement on econ's **M3 `MoneySystem`** instead —
money is M3 **specie** (NO banks, NO fiat, NO demand claims — those are G8b/G8c) — and routes every
sim money flow (spot trades, the world→econ settlement, wage/birth/estate transfers) through that
ledger rather than mutating `Agent.gold`. It also resolves the runtime-M3-removal piece **G4a/b
deferred**: a funded M3 colonist's death now **drains** its ledger specie into the estate
(conserved) instead of refusing removal. econ's M3 market/ledger **behavior is reused unchanged** —
G8a routes the SIM's flows through the ledger and resolves removal; it does not change how M3 clears
markets, so the six conformance goldens stay byte-identical by construction.

The `m3-settlement` scenario is the `viable` economy run on M3 specie. Because specie with no
banks/fiat behaves economically like the M1 gold did, it produces the **same spatial pricing,
provisioning, and sustenance** as the M1 settlement — M3 here is M1, only ledger-accounted, which is
the proof the wiring is correct.

- **The settlement runs on M3 specie.** A `SettlementConfig::m3 = true` flag (`false`, so inert, for
  every pre-G8a config) builds the society as the pure-specie M3 scenario
  (`EmergedGoldSoundControl`: `MarketM3` kind, `SoundGold`, no banks, no issuers, no project lines).
  The only active M3 machinery is the **ledger-settled spot market**; the loan/labor passes are
  inert for a gatherer/consumer roster.
- **Conservation spans the M3 ledger (specie) + goods** every econ tick, and the M3 ledger's **own**
  conservation holds (`money_ledgers_reconcile`) — across spot trades, births, and deaths. Every sim
  money flow is a ledger move, never an `Agent.gold` mutation.
- **M3 estate routing resolved.** `Society::remove_agent` (M3) drains the dead colonist's specie into
  the `Estate` (`commodity_base` falls by exactly that, the row is forgotten, the invariant holds);
  the sim routes it to the commons or, via the new additive `Society::credit_estate_gold`, to an
  heir. `can_remove_agent` no longer refuses a funded **specie** balance (a fiat/claims balance is
  still refused — G8b/c). Deaths and births conserve M3 balances.
- **Pure specie.** The M3 composition is specie only — fiat, demand claims, bank reserves, fiduciary,
  and time deposits are all zero. The viewer's `m3-settlement` dashboard surfaces it as a
  `money: M3 ledger — specie S · fiat 0 · claims 0 · reserves 0` banner.
- **Goldens byte-identical by construction.** The M3-removal drain and the shared consumed-provision
  capture are game-only (the lab never frees an agent) and/or gated on flags the lab
  never sets, so the six econ goldens and every G1–G7 test stay green.

G8a:

- [x] the M3-money settlement mode (`SettlementConfig::m3` / `m3_settlement`, the `EmergedGoldSoundControl`
      specie society) and the routing of the sim's money flows through the M3 ledger
- [x] the resolved M3 estate routing — `remove_agent` drains specie into the `Estate`, `credit_estate_gold`
      re-credits an heir, `can_remove_agent` allows funded specie (econ unit tests migrated)
- [x] the additive, gated consumed-provision capture in `run_m3_tick` (so the spatial sim reads its
      consumed sink back on M3; the M3 goldens stay byte-identical)
- [x] viewer surfacing — the `m3-settlement` scenario + the M3 money-composition banner
- [x] acceptance suite (`sim/tests/g8a_m3_money.rs`: the seven acceptance tests) + econ M3-removal unit
      tests + README + divergence-log updates

**Banks, deposits, fiduciary, and credit (G8b); fiat, the regime ladder, tender policies, and taxation
(G8c); and the Credit/Modern era rungs** are **deferred** — G8a is M3 **specie** money only. See
`sim/tests/g8a_m3_money.rs` and `docs/engine-divergence.md` (the G8a entry).

## Status: G8b (banks & credit) — complete

G8a put the settlement on M3 ledger **specie**. **G8b** adds the **bank**: a chartered institution
that takes **deposits** and lends **fiduciary credit** — demand claims beyond its reserves — gated by
its reserve ratio. This is the credit layer the lab proved drives the Austrian boom/bust, now in the
spatial game on emerged/ledger money. The reuse is **total**: deposits and fiduciary lending route
through econ's existing M3 ledger / `Bank` balance-sheet paths **unchanged** — the bank is chartered
in the *settlement* (config-chartered; the player-`Command` charter is G8c/UI), not in a new econ
scenario, so the spot market is byte-identical to G8a and the six conformance goldens stay
byte-identical by construction.

The `bank` scenario is the `m3-settlement` economy with one chartered fractional-reserve bank; the
`bank-full-reserve` scenario is its **falsification twin** — identical but for a 100% reserve ratio.

- **Deposits become claims backed by reserves.** Each econ tick, consumers deposit M3 specie into the
  bank: `MoneySystem::issue_demand_claim` moves the specie into the bank's reserves and credits the
  depositor an equal demand claim, and `Bank::credit_reserves` mirrors it on the balance sheet. The
  depositor's spendable total is unchanged (specie became a claim), so **claims circulate as money**
  in the specie's place — the colony keeps trading and stays fed on a claim-dominated money supply.
- **Fiduciary lending creates credit beyond reserves.** The bank lends up to
  `Bank::fiduciary_lend_capacity(regime)`, with a sim-side reserve headroom buffer for depositor
  death withdrawals, as claims issued with zero reserve backing (so the ledger tracks
  `fiduciary = demand_claims − reserves`) to the gatherers, who spend them into the economy. The
  chartered bank runs the `FractionalConvertible` regime (set once via econ's existing
  `apply_command(SetRegime)` — its fixed operating regime, **not** the G8c regime *ladder*).
- **The 100%-reserve control lends ZERO fiduciary.** A `ReserveRatioBps::FULL` bank's lend capacity
  is zero, so the same phase lends nothing while its deposits still circulate as claims (every claim
  fully backed: `demand_claims == reserves`). Paired with the fractional bank — same deposits, same
  regime, only the reserve ratio differs — this isolates credit creation to the fractional reserve
  (the lab's `hundred_pct_reserve_lends_no_fiduciary`, in the sim).
- **Conservation spans the M3 ledger with credit + goods** every econ tick: `fiduciary <=
  demand_claims`, reserves back claims, **specie is conserved** (`commodity_base` never moves —
  fiduciary is credit, not minted specie), goods are conserved, and the ledger reconciles
  (`money_ledgers_reconcile`). The broad money (TMS = specie + claims) exceeds the specie base by
  exactly the fiduciary — credit expansion without new specie.
- **Viewer surfacing.** The `bank` / `bank-full-reserve` dashboards surface the M3 composition as
  `money: M3 ledger — specie S · fiat 0 · claims C · reserves R · fiduciary F` plus a bank
  balance-sheet banner `bank: NAME — reserves R · deposits D · fiduciary issued F · reserve ratio P%`.
- **Goldens byte-identical by construction.** The bank phase is skipped entirely without a charter
  (a `SettlementConfig::bank` of `None`, the default for every pre-G8b config), so the six econ
  goldens and every G1–G8a test stay green.
- **Claims estates are deferred — depositor deaths settle by deposit withdrawal.** The dead-agent
  estate carries **specie only** (claim/fiat estates land with the G8c finance work). The colony is
  viable only over a **bounded horizon** — its depositing consumers eventually starve once their
  finite WOOD income runs out, true with or without a bank — so a depositor can reach the
  starvation-death window still holding the claims its deposits created. G8b settles that with **no
  econ change**: `Settlement::liquidate_bank_deposit_on_death` *withdraws the deposit* (the dying
  colonist's claims are redeemed for specie through econ's existing `redeem_demand_claim_for_specie`
  path — the bank paying specie from reserves, the mirror of the deposit), after which the colonist
  holds only specie and settles as the ordinary G8a specie estate. The direct lending buffer keeps
  the bank inside its configured reserve ratio across that withdrawal. Banked demography,
  non-curated banked layouts, and custom bank charters are still rejected (heir/old-age claim
  settlement and broader finance are G8c).

G8b:

- [x] the bank charter overlay (`SettlementConfig::bank` / `bank` / `bank_full_reserve`, one econ
      `Bank` chartered in `society.banks`, the `FractionalConvertible` operating regime)
- [x] the deposit + fiduciary-lend bank phase (`Settlement::run_bank_phase`, routed through
      `issue_demand_claim` / `credit_reserves` / `record_fiduciary_loan` — no bank logic added to econ)
- [x] viewer surfacing — the `bank` / `bank-full-reserve` scenarios, the M3 fiduciary composition, and
      the bank balance-sheet banner
- [x] acceptance suite (`sim/tests/g8b_banks.rs`: the seven acceptance tests + a depositor-death
      settlement regression + unit tests) + README + divergence-log updates

**Fiat, the regime ladder, tender policies, and taxation (G8c); the full ABCT boom/bust
demonstration (it needs the regime ladder to enable-then-stop credit, G8c); the player-`Command` bank
charter (G8c/UI); demand-claim estate routing; and the Credit/Modern era rungs** are **deferred** —
G8b proves the lending **mechanism** + the reserve control. See `sim/tests/g8b_banks.rs` and
`docs/engine-divergence.md` (the G8b entry).

## Status: G8c-1 (fiat, the regime ladder, and the credit cycle) — complete

This is the **climax** of the economic engine: the **Austrian business cycle**, in the colony game,
on econ's **unchanged** ABCT/regime/shadow machinery. G8a put the sim on M3 ledger money; G8b added
banks and fiduciary credit. **G8c-1** adds **fiat** and the **regime ladder** (`SoundGold →
FractionalConvertible → SuspendedConvertibility → Fiat`) and demonstrates the cycle the lab proved
(`emerged-gold-fiat-credit-expansion`): cheap credit drives the market rate **below** the
credit-disabled **shadow** natural rate (a measured **gap**), capitalists over-invest in roundabout
production (the **boom**), credit **stops**, the rate reasserts, the malinvested projects are
**abandoned**, and **capital is consumed** (the **bust**) — against a **sound-money control** that
shows no gap and no cycle.

The reuse is **total**: the regime ladder (`SetRegime`), fiat issuance
(`SetIssuerPolicy`/`StopIssuerCredit`), the boom/bust/abandonment/capital-consumption M3 records, and
the credit-disabled `run_credit_disabled_shadow` counterfactual are all econ's, **unchanged**. A
`credit-cycle` settlement is a **finance** settlement (`SettlementConfig::credit_cycle`): it has no
spatial colony, its `Society` is built from econ's credit-ladder scenario, and each econ tick simply
steps that society so the cycle runs **endogenously**; the sim only routes the regime/issuance in and
reads the measured signals back. So the spot market is untouched and the six conformance goldens stay
byte-identical by construction.

- **The regime ladder descends.** `SetRegime` walks `SoundGold → FractionalConvertible →
  SuspendedConvertibility → Fiat` over the first ticks (the per-tick `regime` column shows the
  descent); under `Fiat` the issuer extends **fiat-credit** into the economy. The credit-cycle's
  current regime is `Fiat`; the control stays `SoundGold`.
- **The shadow gap is the authoritative signal.** The settlement replays a credit-disabled **shadow**
  (`Settlement::shadow_gap_bps`) to get the natural rate; `gap = shadow_natural_rate − market_rate`.
  Cheap credit pushes the market rate below the natural rate, so the gap opens **positive** during the
  boom (`max_shadow_gap_bps() > 0`). MEASURED, never set.
- **Boom → stop → bust → capital consumed.** Seeing cheap credit, capitalists start the long
  roundabout project, so the measured structure lengthens **above** the shadow baseline
  (`structure_rose_above_shadow()`) — the **boom**. When credit **stops** (`StopIssuerCredit`), the
  rate reasserts, the malinvested projects no longer pencil out and are **abandoned**
  (`bust_abandoned_projects() > 0`), consuming non-salvaged embodied capital
  (`capital_consumed() > 0`) — reusing the M2/M3 abandonment + capital-consumption machinery. The
  bust is a cluster of individually-rational abandonments, not a global trigger.
- **The sound-money control has no cycle.** `SoundGold`, no fiat, no credit — the same agents and the
  same roundabout project line, only credit differs. The gap stays ≈ 0, no boom forms, nothing is
  abandoned, and no capital is consumed. Paired with the cycle, it isolates the boom/bust to **credit
  expansion**, not the production/spatial dynamics — *if the control busts, the cycle is not coming
  from credit*.
- **Fiat conserves.** Fiat is **credit, not minted specie**: the specie base
  (`public_specie + bank_reserves`) is unchanged across the cycle, the fiat base = **issued − retired**
  equals the outstanding circulating fiat, broad money is exactly `specie + fiat`, and the M3 ledger
  reconciles every tick — a default changes the money stock by rule (retirement/booking), never by a
  leak.
- **The Credit and Modern era rungs unlock.** The G6a detector gains two rungs above Capital: it
  reaches **Credit** when institutionally-created credit circulates and **Modern** when state fiat is
  the marginal medium — both MEASURED, with the same hysteresis. The finance path climbs the full
  ladder (`forager → barter → money → specialist → capital → credit → modern`); the sound-money
  control tops out at Money. The emergent-chain frontier's measured timeline is **byte-identical**
  (the new rungs are unreachable without chartered credit or fiat, so it still tops out at Capital).
- **Viewer surfacing.** The `credit-cycle` / `sound-money` dashboards surface the regime ladder (a
  per-tick `regime` column + a `gap.bps` column), the era banner climbing to `modern`, and a cycle
  banner `cycle: KIND — regime R · shadow gap(max) G bps · boom B · bust X · capital consumed C ·
  fiat base F`.

G8c-1:

- [x] the regime ladder + fiat issuance as sim policy (`SettlementConfig::credit_cycle` /
      `sound_money`, the finance `Settlement` built from econ's credit-ladder scenario, the
      `SetRegime`/`SetIssuerPolicy`/`StopIssuerCredit` timeline — no ABCT/regime/shadow logic added to
      econ)
- [x] the shadow-gap wiring (`Settlement::shadow_gap_bps` / `max_shadow_gap_bps` /
      `structure_rose_above_shadow`, the credit-disabled `run_credit_disabled_shadow` replay) and the
      boom/bust/capital-consumed/fiat-base accessors read from the M3 records
- [x] the Credit/Modern era rungs (`sim/src/era.rs`: Credit/Modern measured from path-independent
      M3-record signals — `credit_ever_circulated` / `fiat_ever_circulated` — so a chartered-bank
      (G8b) colony also sets the Credit trigger; the lower-rung emergent path is unchanged and the
      bank-free frontier's measured timeline stays byte-identical)
- [x] viewer surfacing — the `credit-cycle` / `sound-money` scenarios, the regime + gap columns, the
      cycle banner, and the era climb to `modern`
- [x] acceptance suite (`sim/tests/g8c1_cycle.rs`: the seven acceptance tests + unit tests) + viewer
      dashboard tests + README + divergence-log updates

**Tender policies (M11–M17) and tax receivability as player levers (G8c-2), and the multi-seed
robustness study of the cycle (deferred)** are **not** here — G8c-1 is fiat + the regime ladder + the
cycle + the control. See `sim/tests/g8c1_cycle.rs` and `docs/engine-divergence.md` (the G8c-1 entry).

## Status: G8c-2 (tender policies — the acceptance levers) — complete

G8c-1 gave the game the credit cycle. **G8c-2** adds the **tender policies** the lab built across
M11–M17 — explicit rules for *which media must be accepted* on each settlement surface (spot exchange,
public debt, bank-loan repayment, issuer repayment, and **labor wages**) — as sim policy levers. The reuse is **total**: econ's
`PublicSpotTender` / `LaborWageTender` / `PublicDebtTender` (and the bank/issuer-repayment tenders),
their `accepted_media()`, and the `SetXTender` events are all **unchanged**; G8c-2 only **routes** each
settlement surface through its tender policy (config-set; the player-`Command` route is G9). It adds
**no** tender logic to econ, so the six conformance goldens stay byte-identical by construction.

- **The headline: wage tender gates the credit cycle.** This is the lab's **M17** result, now in the
  spatial cycle. In the G8c-1 credit cycle the fiat-credit borrowers (would-be employers) hold fiat,
  and the boom transmits **through wages**:
  - **`wage-tender-cycle`** (`SettlementConfig::wage_tender_cycle`, fiat wages legal tender): the
    fiat-credit employers can pay fiat wages → the fiat credit reaches workers → demand follows → the
    boom→stop→bust **transmits**. The cycle **fires** (`cycle_fired() == true`: gap > 0, boom, bust,
    capital consumed).
  - **`wage-refusal-cycle`** (`SettlementConfig::wage_refusal_cycle`, specie-only wages): the **same**
    fiat-credit issuance (`credit_ever_circulated() == true`) is **inert** — the employers cannot pay
    fiat wages, the credit never enters the real economy, and **no boom and no bust** form. The control
    is the proof the **wage surface is the transmission valve**: the *only* difference is the wage
    tender. *If the cycle fired under specie-only wages, the wage gate would not be routing
    settlement.*
- **Tender gates composition, never totals.** A refused medium **cannot** settle its surface even if
  *held*, and the active medium does — but no money is created or destroyed. The fiat-displacement
  **benches** make this exact: under `spot-tender-refusal` (`SpecieOnly`) the printed fiat is still
  held (`public_fiat` unchanged) yet **none** of it settles the spot market (`spot_fiat_settled() ==
  0`, specie settles instead); under `spot-tender-legal` (`FiatAndSpecie`) the held fiat settles
  (`spot_fiat_settled() > 0`). The **broad money is identical** across the twin — only which medium
  settled flipped. The debt benches mirror it on debt discharge (M12).
- **The other surfaces wire as the same lever.** `PublicSpotTender` / `PublicDebtTender` and the
  bank/issuer-repayment tenders each enforce their refusal-vs-acceptance on their surface — the lab's
  M11-M16 results, reachable as sim config levers (`SettlementConfig::spot_tender_bench`,
  `debt_tender_bench`, `bank_repayment_tender_bench`, `issuer_repayment_tender_bench`) routed through
  the *same* `SetXTender` mechanism as the wage×cycle headline.
- **Conservation holds under every policy.** Tender changes the **medium**, not the stock: the
  displacement benches hold the specie base (16) and the broad money (24) fixed whichever medium
  settles, repayment benches route through econ's normal credit-retirement accounting, and the cycle
  conserves the specie base with the fiat base an exact `issued − retired` identity — the M3 ledger
  reconciles every tick under every tender policy.
- **A default tender is byte-identical to G8c-1.** `TenderPolicy::default()` equals econ's per-surface
  defaults (`ParAll` for spot/wage/debt/bank-repayment, `FiatOnly` for issuer-repayment), so it emits
  **no** `SetXTender` event — the plain `credit-cycle` / `sound-money` settlements (and every spatial
  settlement) are unchanged. The `wage-tender-cycle` makes the legal-tender choice **explicit** (and
  the plain cycle's `ParAll` wages already accept fiat, so both transmit).
- **Viewer surfacing.** The finance dashboards add a `tender:` banner — `tender: spot S · wage W · debt
  D · bank-repayment B · issuer-repayment I [· SURFACE settled fiat F / claims C / specie P] · broad
  money M` — and the cycle banner gains `· wages W · fired|transmitting|pending|inert|no-credit`, so
  the transmission valve, active tender policies, and settlement composition proof are visible.

G8c-2:

- [x] the tender policies as sim config levers (`sim::TenderPolicy` + the `SetXTender` routing in
      `cycle_scenario` / `tender_bench_scenario` — no tender logic added to econ; a default policy
      emits no events, keeping the G8c-1 finance bytes byte-identical)
- [x] the headline — `wage_tender_cycle` (fiat wages → the cycle fires) and `wage_refusal_cycle`
      (specie-only wages → the same credit is inert), with the `cycle_fired()` outcome accessor and the
      `wage_fiat_settled` / `wage_specie_settled` composition reads
- [x] the spot/debt/repayment benches (`spot_tender_bench`, `debt_tender_bench`,
      `bank_repayment_tender_bench`, `issuer_repayment_tender_bench`, the M11-M16 surfaces) with the
      `spot_*_settled`, `debt_*_settled`, `bank_repayment_*`, and `issuer_repayment_*` composition
      reads
- [x] viewer surfacing — the `wage-tender-cycle` / `wage-refusal-cycle` and `spot-tender-*` /
      `debt-tender-*` / repayment-tender scenarios, the `tender:` banner, and the cycle outcome
- [x] acceptance suite (`sim/tests/g8c2_tender.rs`: the seven acceptance tests + unit tests) + viewer
      dashboard tests + README + divergence-log updates

**Tax receivability (the state's counter-lever, M21 — G8c-3), the player-`Command` tender route (G9),
and the multi-seed robustness study (deferred)** are **not** here — G8c-2 is the tender surfaces + the
wage×cycle headline. See `sim/tests/g8c2_tender.rs` and `docs/engine-divergence.md` (the G8c-2 entry).

## Status: G8c-3 (tax receivability — the state's counter-lever) — complete

G8c-2 gave the player the *private* acceptance levers: when the labor market refuses fiat wages, fiat
credit is **inert**. **G8c-3** adds the *state's* counter-lever — **tax receivability** (the lab's
**M21**, chartalist) — as a sim policy on that same settlement. The reuse is **total**: econ's
`apply_levy_tax`, `settle_due_debts_m3` (gated by `TaxReceivability`, **never** the credit tenders),
the `SetTaxReceivability` / `LevyTax` events, and the issuer tax accounts (`taxes_levied`,
`tax_receipts_fiat` / `tax_receipts_specie`, `taxes_defaulted`) are all **unchanged**; G8c-3 only
**routes** the levy/receivability in (config-set; the player-`Command` route is G9). It adds **no** tax
logic to econ, so the six conformance goldens stay byte-identical by construction. **G8c-3 is the last
economic milestone before the G9 graphical-UI hand-off.**

- **The headline: a fiat-receivable tax compels what the market refused.** This is the chartalist
  answer to private refusal. In a settlement whose **wages are specie-only** (the G8c-2
  `wage-refusal-cycle`: fiat credit inert, no private fiat demand), a **fiat-receivable** tax routes
  fiat through the **fiscal** channel even where the **labor** channel refused it:
  - **`tax-in-fiat`** (`SettlementConfig::tax_in_fiat`, `FiatOnly` tax): the fiat-credit capitalist
    holding idle fiat must remit it to the state, so the tax settles in fiat (`tax_receipts_fiat > 0`)
    while **no** fiat wage ever settles (`wage_fiat_settled() == 0`). Fiat circulates **via tax** where
    the labor market refused it.
  - **`tax-in-specie`** (`SettlementConfig::tax_in_specie`, `SpecieOnly` tax): the **control** — the
    specie-holding trader remits specie (`tax_receipts_specie > 0`) and **no** fiat is compelled
    (`tax_receipts_fiat == 0`). The twin levies the **same** set; the *only* difference is the
    receivability, so the compelled fiat demand is isolated to that gate (not the levy or the spatial
    dynamics). *If the control showed fiat receipts, the receivability gate would not be routing
    settlement.*
- **The receivability gate decides the tax surface.** A medium **not** in the active `TaxReceivability`
  cannot discharge the tax **even if held**; the receivable medium does. Under the fiat-receivable tax
  the specie-holder defaults **though it holds specie**; under the specie-receivable tax the fiat-holder
  defaults **though it holds fiat** — the M21 media gate, in the sim.
- **Tax is fiscal, not credit.** A levy is a **zero-principal** `DebtContract` owed to the single state
  issuer (funded as `Tax`, not credit); the tax levy/receipt **never** moves `credit_retired` or
  `fiat_credit_outstanding` — through the levy's due tick `credit_retired` stays zero while the tax
  settles, so the receipt is honest money contraction (`fiat_retired`) / the specie vault, never credit
  retirement.
- **Conservation is exact.** A levy is either **received** (into the issuer, in the receivable medium)
  or **defaulted** (unmet **by rule** — the holder lacks the receivable medium), never created or
  destroyed: `levied == receipts_fiat + receipts_specie + defaulted`. The M3 ledger reconciles every
  tick and the fiat base stays the exact `issued − retired` identity.
- **A no-tax settlement is byte-identical.** A settlement that levies no tax (every plain cycle, bench,
  and spatial settlement) omits the canonical tax block entirely and surfaces no tax banner — unchanged.
- **Viewer surfacing.** A tax settlement adds a `tax:` banner — `tax: receivability R · levied L ·
  receipts fiat F / specie P · defaulted D` — so the active receivability (the chartalist gate), the
  levy, the fiat-vs-specie split, and the by-rule defaults are visible.

G8c-3:

- [x] the state levy + receivability as a sim config overlay (`sim::TaxPolicy` + the
      `SetTaxReceivability` / `LevyTax` routing in `cycle_scenario` — no tax logic added to econ; a
      no-tax settlement omits the canonical block, keeping the finance bytes byte-identical)
- [x] the headline — `tax_in_fiat` (a fiat-receivable tax compels fiat through the fiscal channel) and
      `tax_in_specie` (the specie-receivable control compels none), with the `tax_receivability`,
      `taxes_levied`, `tax_receipts_fiat` / `tax_receipts_specie`, and `taxes_defaulted` accessors
- [x] tax is fiscal not credit (zero-principal liability to the single issuer; receipts never touch the
      credit aggregates) + exact conservation (`levied == received + defaulted`, a default unmet-by-rule)
- [x] viewer surfacing — the `tax-in-fiat` / `tax-in-specie` scenarios and the `tax:` banner
- [x] acceptance suite (`sim/tests/g8c3_tax.rs`: the seven acceptance tests + unit tests) + viewer
      dashboard test + README + divergence-log updates

**The player-`Command` tax/tender route and the Bevy graphical UI (G9), and the multi-seed robustness
study (deferred)** are **not** here — G8c-3 is the tax-receivability counter-lever + the chartalist
headline. **The next milestone is G9 (the Bevy graphical UI), which cannot be driven by the headless
test loop and is the explicit hand-off point to the user.** See `sim/tests/g8c3_tax.rs` and
`docs/engine-divergence.md` (the G8c-3 entry).

## Status: endogenous specialization (the grain→flour→bread chain self-organizes) — complete

The twelve money-circulation experiments showed the production chain only sustained under **curated
placement** — a planner handing food and inputs to every producer; strip it and the chain died
~tick 150. This milestone makes the **division of labor self-organize**: grain→flour→bread
specialization emerges atop a **household/subsistence base** and **sustains on real market trade**,
with **no chain-specific global placement**. Sliced per `docs/impl-endogenous-specialization.md`:

- [x] **S1 — econ order-book bid override** (`econ/src/society.rs`): a gated per-`(agent, good)`
      spot-bid override. The sim sets `(reservation, limit)` before `step()`; `ensure_bid` consults it
      first and `live_quote_changed` mirrors the branch so the resting override bid survives the next
      tick's reconciliation; `ensure_order` stays the sole insertion path (the bid reserves gold, fills
      against a willing ask, records a real `Trade`); overrides are cleared after the step. **Additive
      and gated** — with no override set, the six conformance goldens (m5/m6/m7/m8/m9) are
      byte-identical (the disabled-hook regression is the tripwire).
- [x] **S2 — project-aware producer bid** (`sim/src/settlement.rs`): the input bid price is the imputed
      reservation from the **project-bundle appraisal** (`imputed_input_reservation` reuses
      `recipe_adoption_pays_for_money` / `appraise_project_bundle_for_money`) — the highest input price
      at which running the recipe-as-project still provisions the producer's savings want, off the
      output's last realized price. When that savings want is already satiated, a **recurring**
      owner-operator falls back to restocking at the Mengerian break-even `ceiling` (revenue −
      operating cost per input unit) rather than dropping out — a scalar, but a deliberate
      recurring-consumption motive, not the recipe-blind generic bid (which is suppressed so the
      override is the sole input bid).
- [x] **S3 — working-capital persistence**: real **retained earnings**, no per-tick planner loan
      (`capital_advance` off). A local **producer-subsistence hearth** (`producer_subsistence`) feeds
      each producer its staple + WOOD so its money frees entirely for inputs, and a **demand-responsive
      restock gate** keeps it from over-producing into a saturated market and draining its purse.
- [x] **S4 — cold-start bootstrap**: the seeded `latent_flour_seed` / `bread_buffer` yield the first
      realized flour/bread prices so the latent pool adopts in deterministic pipeline order (bread
      demand pulls a baker in, the baker's flour purchase pulls a miller in), with no curated advance.
- [x] **S5 — the endogenous DoD** (`SettlementConfig::frontier_endogenous`, the `endogenous` scenario):
      a designated-GOLD colony on the household demography + edible-grain subsistence base, composing
      S1–S4 with `subsistence_advance` / `input_advance` **off**. The chain sustains: bread is still
      produced through tick 1600, producers retain working capital, and population, per-capita bread,
      and hunger are stationary — all conserving and deterministic.
- [x] acceptance suite (`sim/tests/endogenous_economy.rs`: the six named tests, incl.
      `inputs_acquired_by_market_trade` requiring an actual `Society::trade` by an active producer plus
      downstream recipe consumption with no placement counter) + the S1/S2/S3/S4 slice tests + the
      viewer `endogenous` scenario.
- [x] **S6 — provisioning at scale via productive re-entry** (`SettlementConfig::frontier_endogenous_scaling`,
      the `scaling` scenario): the endogenous economy plus a default-off, gated re-entry phase that lets
      hungry spatial non-lineage consumers and WOOD gatherers adopt edible-grain gathering, then return
      home through hysteresis once fed. The larger roster and household cap exercise the growing-colony
      case while preserving S5: bread production, Miller/Baker adoption, real input trades, WOOD supply,
      conservation, and deterministic canonical bytes all hold.
- [x] acceptance suite (`sim/tests/provisioning_at_scale.rs`: the eight named tests) + S6.1/S6.2 slice
      tests (`sim/tests/productive_reentry.rs`) + the viewer `scaling` scenario.
- [x] **S7 — producible capital goods** (`SettlementConfig::frontier_capital`, the `capital` scenario):
      two default-off, gated steps that let the **tooled** chain grow. S7.1 relaxes role-choice so a
      colonist that *holds* the required tool is admitted to the adoption appraisal (and anchors the
      tool so it is never sold before it adopts); S7.2 adds a per-agent BuildMill/BuildOven project. A
      gated settlement phase appraises a **demand-anchored real-resource investment** — when the
      chain's bottleneck final good is clearing and a durable tool's multi-period proceeds
      (`margin_per_run × capital_payback_cycles`) exceed its WOOD + labor build cost, it funds **one**
      build from the **selected fed builder's own WOOD + labor** (a conserved project booked
      WOOD→`consumed_as_input` at start, tool→`produced` at completion); that builder then adopts and
      produces. It is not a planner handout (no tool placement; the builder pays from its own
      endowment), but it is a settlement-level heuristic, **not yet a per-colonist ordinal-scale
      appraisal** (a noted follow-on). On a larger colony `capital` ends with more tools, more
      producers, and higher, non-declining bread than the same colony with the gates off, with **no
      runaway over-build in the tested 1600-tick run** (the heuristic's brakes — bottleneck choice,
      one build in flight, idle-tool slack — bound it; a general stop theorem is not proven) and
      conservation every tick.
- [x] acceptance suite (`sim/tests/producible_capital.rs`: the eight named tests) + S7.1/S7.2 mechanism
      and digest tests (in `sim/src/settlement.rs`) + the viewer `capital` scenario.

The curated-placement scenarios (`in-kind-advance`, `input-advance`, `economy`) and their flags are
**kept for comparison**; the S5/S6/S7 DoDs pass with them off. The `endogenous` scenario keeps productive
re-entry off for comparison, `scaling` turns it on to address the stranded high-hunger tail, and
`capital` adds producible capital so the tooled chain grows with demand in the tested run. (The
S5/S6/S7 result is "self-organizing" only in the scoped sense of this stack — a household/subsistence
base + productive re-entry + project-aware input bids + the demand-anchored build heuristic — not a
market-alone economy.)

**Honest scope of the claim.** What is proven: the chain *acquires its inputs by real market trade*
and *keeps producing through tick 1600* with no global food/input placement and no per-tick capital
loan, on a designated-GOLD colony. What it rests on (disclosed, not "market alone"): a *local
producer-subsistence hearth* that mints each producer's staple + WOOD (a household garden, never the
chain inputs grain/flour), seeded *cold-start buffers* for the first prices, and *designated gold* —
so this does **not** re-prove Mengerian money emergence. The `endogenous` colony is well-fed in the
mean but leaves a stranded high-hunger tail; the `scaling` scenario gives that tail a **subsistence
path** (productive re-entry) without collapsing the bread chain or WOOD supply. The tail is
**materially reduced, not eliminated** — severe hunger can still occur (observed tail max ~11) — and
re-entry is a hunger-threshold survival rule (direct self-provisioning), not a market/value-scale-
derived choice. The tests prove the *reduction and boundedness*; the live run shows the *mean*
improvement (tail mean ~1.3 at pop 40 vs ~4.1 at pop 26). "Provisioning at scale" is scoped to
**untooled subsistence** through tick 1600 — the **tooled** grain→flour→bread chain is scaled
separately by **S7 producible capital** (the `capital` scenario), where colonists *build* new
mills/ovens from saved WOOD + labor under unmet bread demand, so the chain's capacity is no longer
hard-capped at the seeded tool count. The shipped S7 throttle intentionally keeps one capital build
in flight at a time, so this proves demand-responsive capital formation through the tested frontier
scale, not an unbounded colony-wide construction rate.

## Status: S8 (money co-emergence with the specialized economy) — complete, with a principled finding

S5/S6/S7 run on **designated GOLD** — the colony is *handed* a money good and only then calculates,
bids, and builds. This milestone removes that scaffold: money, the grain→flour→bread division of
labor, and capital all **co-emerge in one run** from a no-money barter start. Sliced per
`docs/impl-money-coemergence.md`:

- [x] **S8.0 — the emergence probe** (`Settlement::emergence_acceptances` / `producer_cash` /
      `bread_for_salt_volume` / `peak_pre_promotion_hunger` / `critical_ticks_before_promotion`,
      surfaced in the viewer's emergence-probe panel): read-only diagnostics that separate a
      *principled* failure from a *tuning* one — the promotion tick, per-candidate barter saleability,
      each producer's working capital, the bread-for-SALT leg, and the pre-promotion hunger trough.
      Pure read-back, absent from `canonical_bytes`, deterministic (`sim/tests/emergence_probe.rs`).
- [x] **S8.1–S8.3 — the co-emergent base** (`SettlementConfig::frontier_coemergent`, the `coemergent`
      scenario): built from `frontier()` (the barter-start emergent base — `barter = Some(..)`, the
      SALT medium, **every gold endowment zero**), composing the S5 sustain stack and the S7 capital
      phase on the **emerged** unit. SALT promotes endogenously (~tick 20) from real barter
      acceptances **under a configured universal SALT demand** (the load-bearing asymmetry — see the
      finding below) — no designated money, zero gold at generation; the chain waits on money (no
      producer/no chain output before promotion); inputs clear by real `Society::trade` across the
      cutover; bread sustains through tick 1600 at a real rate (tail ~450/100-tick window); hunger is
      bounded but the colony is **semi-hungry** (mean ~7.6, p95 ~12 — healthy provisioning is S11); ≥1
      mill/oven is **built** (`produced`) after promotion by a formerly-non-latent colonist;
      everything conserves every tick (incl. the promotion sink) and is byte-deterministic.
- [x] acceptance suite (`sim/tests/money_coemergence.rs`: the eleven named tests) + the viewer
      `coemergent` scenario (the era column climbs `barter → money → specialist → capital`). All edits
      additive/gated: the G5a/G5b goldens, the six econ conformance goldens, and the S5/S6/S7 suites
      stay byte-identical/green; `frontier_coemergent` reuses only existing serialized knobs (no new
      `canonical_bytes` surface) and does not alter the `frontier`/`frontier_endogenous` builders.

**Honest scope of the claim (Codex-reviewed: PASS on the narrow bar, NOT the strong one).** What is
proven: S8 **removes designated gold** and demonstrates a sustained specialized chain plus producible
capital running on **SALT promoted from a no-money barter start**, with no seeded gold and no curated
input/food/capital placement — the barter→money→specialization→capital composition the prior 13
experiments never reached (bare `frontier` froze ~tick 150). What it does **NOT** yet prove: *fully
authentic Mengerian money emergence from indirect exchange*. The money side still **depends on a
configured universal SALT want** — SALT promotes on want-breadth, not a thick volume of indirect
exchange (the no-saleability control sets `medium_want_qty = 0` and nothing monetizes). What it
further rests on (disclosed): exogenous, producer-only and household subsistence hearths (not
"market alone") — tuned **lean** (`producer_subsistence` 2 vs S5's 4; demographic `food/wood_provision`
1 vs 3) so a fed agent recirculates rather than hoards (a money-sink avoidance, not a handout) — plus
the same cold-start buffers as S5, and a modest, semi-hungry colony (healthy provisioning-at-scale
under emergence is deferred to S11, where the raw-grain floor would crowd out the bread-for-SALT trade
that monetizes SALT).

**The principled finding (Tension B), reported as a passing diagnostic — not papered over.** The
spec hypothesized the cutover working capital would be *barter-earned* SALT. It is not: the universal
medium want makes SALT promote on want-breadth, not trade volume — it crosses the threshold with
almost no SALT changing hands (consumers retain ~318 of 320 units), so latent producers earn ~zero
barter SALT and hold zero converted gold at promotion (robust across tuning). The chain survives the
cutover **anyway** — producers fund working capital from **post-promotion money-market earnings**
(earned, not endowed; the test proves zero gold at promotion then real input trades after, though it
does not trace the exact earning good). `tension_b_working_capital_is_earned_post_
promotion_not_in_barter` asserts this observed mechanism via the S8.0 probe rather than forcing a
pass with designated gold or a curated advance. Because the chain does not freeze, the sustain and
capital tests still pass — so the only deviation from the specified DoD is *which faithful source*
capitalizes the producers across the cutover.

## Status: S9 (strong-bar money emergence — the regression theorem) — complete (Codex: PASS on the strong bar)

S8 left one circularity: SALT promoted because every colonist was *configured* to want it as a medium
(`medium_want_qty`) — agents wanting SALT as money before it is money. S9 removes that and tests
whether money emerges from **real saleability**. Sliced per `docs/impl-strong-bar-emergence.md`:

- [x] **S9.1 — SALT's heterogeneous real direct use** (`BarterConfig::salt_direct_use_qty` /
      `salt_direct_use_period`): a modest, **fixed** `Good(SALT)/Now` consumption want given to only
      ~1-in-8 colonists at a time (heterogeneous — a *universal* want would suppress indirect
      acceptance, since an agent that directly wants the leader posts no `IndirectFor` offer), consumed
      into the `consumed` bucket, active pre-promotion only. **It is an *exogenously modeled* commodity
      use — not circular money demand, and not itself emergent.**
- [x] **S9.2 — indirect-exchange breadth gate** (`econ/src/menger.rs`): `observe_trade` now records,
      per candidate, the count + **distinct acceptors** + distinct targets of acceptances tagged
      `BarterReason::IndirectFor` (a side took SALT for an end other than SALT itself); `base_eligible`
      withholds promotion until `min_indirect_acceptances` (12), `min_indirect_acceptor_agents` (6),
      and `min_indirect_target_goods` (1) all clear. (Target breadth is the **weak** dimension — set
      to 1; the 6-distinct-acceptor floor carries the anti-gaming breadth. No broad-target-diversity
      claim is made.)
- [x] **S9.3 — the `strong-emergence` scenario** (`frontier_coemergent_strong`, derived from
      `frontier_coemergent`): `medium_want_qty = 0`, heterogeneous SALT direct use on, the breadth gate
      on. Live (seed 1, pop 20): SALT promotes at **tick 443** (vs S8's tick 20 — it must *earn* it via
      real saleability), era `forager → barter → money → capital`, SALT's direct use is consumed
      pre-promotion (`salt.eaten`≈1/tick, `salt.made`=0 endowed), bread sustains (5250 total / 1797 tail),
      ≥1 tool built on emerged money, conserved every tick.
- [x] acceptance suite (`sim/tests/strong_bar_emergence.rs`: ten named tests) — incl. the **decisive
      control** `no_indirect_acceptance_control_does_not_monetize` (SALT still becomes provisional
      leader and trades *directly*, but with indirect acceptance off it does **not** monetize — proving
      promotion depends on real indirect exchange, not configured SALT demand) and
      `no_direct_use_control_does_not_monetize`. All additive/gated: the g5a/g5b/coemergence emergence
      goldens + the six econ conformance goldens stay byte-identical (new fields default inert).

**Honest scope (Codex-reviewed: PASS on the strong bar).** What is earned: **money emerges from real
saleability inside this model** — SALT becomes money because actors discover it is the most marketable
good (real indirect exchange), *not* because config says they want it as money. This is genuine
Mengerian indirect-exchange emergence, not a renamed medium want (the no-indirect control proves it).
What it does **not** earn: "fully authentic praxeology" in the global sense. The remaining modeled
artifacts (disclosed): the SALT direct-use is an *exogenous* preference schedule (the use, not the
money, is given); weak one-target indirect breadth; lean hearth provisioning; cold-start buffers; and
the scalar S7 capital heuristic. Money emergence is now non-circular; several *other* mechanisms remain
parameter-supported.

## Status: S10 (per-agent intertemporal capital choice — originary interest) — complete

S9 left the **capital** decision as the least authentic major mechanism: S7's build was a
*settlement-level* heuristic — one global stage choice by capacity bottleneck, a scalar `margin ×
capital_payback_cycles` test, and a first-eligible-fed-builder assignment. That is "build if the math
beats cost", not Böhm-Bawerk/Mises capital: an *individual* actor choosing to sacrifice present
goods/labor for a future, more-roundabout return, on its own value scale, with its own time preference.
S10 moves the decision from the planner into the colonist. Sliced per `docs/impl-originary-interest.md`:

- [x] **S10.1 — the per-agent ordinal build appraisal** (`appraise_capital_tool_bundle_for_money`,
      `sim/src/settlement.rs`), behind a default-off `ChainConfig::per_agent_capital`. The PRESENT side
      is the WOOD removed from the builder's own stock + the labor sacrifice as forgone Leisure (the
      build displaces the agent's `Leisure` want at its scale rank — not S7's scalar `operating_cost ×
      tool_build_labor`); the FUTURE side is the tool's recipe net margin as a **generalized dated
      receivable stream**; ACCEPTANCE (the gate) is that the altered temporal endowment newly provisions
      one of the agent's own future-money savings wants while preserving every higher-ranked want (the
      `bundle_accepts_due`/`preserved_above_target` test generalized) AND outranks the displaced leisure.
      HORIZON: a **multi-horizon savings ladder** (`Later(4), Later(8), …`, depth set by the agent's own
      `time_preference_bps` — `life::savings_ladder_depth`), gated to the per-agent path, so a tool's
      gestation-delayed receipts provision the *deeper* wants via the **unchanged** `future_capacity_due_by`
      due-by logic. The S7 Part-2 planner (stage choice + single-in-flight gate + first-eligible
      assignment) is replaced behind the gate; the per-builder substrate
      (`start_project`/`advance_project`/`complete_project_if_ready`) is reused unchanged. A per-tick
      decision diagnostic (`Settlement::last_capital_decisions`) records each candidate's accept/reject,
      target savings want rank, and decline reason.
- [x] **S10.2 — the originary-interest response (the falsifiable core)**, tested two ways:
      (a) a **microtest** — two otherwise-identical colonists differing ONLY in `time_preference_bps`:
      the patient one (depth ≥ 2) ACCEPTS the build, the present-biased one (depth 1) REJECTS it
      (`NoFutureProvision` — the gestation-delayed receipts reach no savings want), strict and
      deterministic; (b) a **live aggregate** — a present-biased colony forms materially-less / non-more
      capital than a patient one over the run.
- [x] **S10.3 — the `originary` scenario** (`frontier_coemergent_strong_originary`, derived from
      `frontier_coemergent_strong` with `per_agent_capital` on). Live (seed 1, pop ~18): money still
      EMERGES (SALT promotes at **tick 479**, identically to the strong-bar base — the savings ladder
      does not gate promotion), then **18 tools are built by individuals on their own scales** (the
      builders become Millers/Bakers), bread sustains to t1600, conserved every tick.
- [x] acceptance suite (`sim/tests/originary_interest.rs`: seven named tests). All additive/gated: with
      `per_agent_capital` off the S5–S9 scenarios + the six econ + the g5a/g5b/coemergence emergence
      goldens are byte-identical (`capital_payback_cycles` is digested only in the legacy path; the gate
      is serialized in the per-agent path — `canonical_bytes_include_per_agent_capital`).

**Does time preference actually drive capital formation? Yes.** Holding the SALT-emergence machinery
fixed, a uniformly patient colony builds **22 tools** while a uniformly present-biased one builds **0**
— and money emerges (tick 479) in *both*, so the difference is the capital response to time preference,
not a money-emergence artifact. The decision genuinely reads the ordinal scale: originary interest is
**emergent** from each colonist's own savings ladder, with no cardinal discount rate anywhere (the
engine has none by design). The honest scope (Codex-reviewed: PASS, no P0/P1): the **microtest** is
the clean per-build proof (two identical agents, only `time_preference_bps` differs → patient accepts,
present-biased declines because the receipts reach no savings want); the **live 22-vs-0** aggregate
also reflects the deeper ladder shaping broader savings/market behavior, not the build appraisal alone.
`capital_payback_cycles` is now inert in the per-agent mode; the build/no-build **acceptance** is
ordinal, but the WHICH-tool ordering still ranks the two candidates by realized margin (a per-agent
profit preference, not a planner stage choice); the labor sacrifice is modeled as forgone Leisure
(first-Leisure-rank displacement, not a full multi-tick opportunity-cost of forgone gathering); demand
response is bounded (no runaway/WOOD-drain), not proven optimal; the disclosed S9 artifacts are unchanged.

## Status: S11 (entrepreneurial uncertainty + profit/loss selection) — complete

S10 made the capital *decision* individual, but every entrepreneurial appraisal still read the **last
realized price** as a *certain* point estimate, identical for everyone — a wrong call cost the actor
nothing differential. Misesian entrepreneurship is action under **uncertainty**: actors *forecast*
future prices, differ in their forecasts, can be **wrong**, and bear the **profit or loss**. S11 makes
forecasts heterogeneous and fallible, and makes the loss *select* — through **capital, not mortality**.
Sliced per `docs/impl-entrepreneurial-uncertainty.md`, behind a default-off
`ChainConfig::entrepreneurial_forecasts`:

- [x] **S11.1 — heterogeneous fallible forecasts feed decisions.** A heritable
      `CultureParams::forecast_bias_bps` (u16 bps, neutral `10_000` = ×1.0, clamped exactly
      `5_000..=20_000`), drawn at generation by a deterministic SplitMix that consumes no extra `Rng`
      (so the generation sequence — and every flag-off golden — is byte-identical) and inherited via
      `deterministic_mix64` with its own salt. `PriceBelief` gains an explicit `observed` flag (set on
      `observe`/`nudge`) so the **grounded** forecast `forecast_price_for(agent, good)` uses the agent's
      own `belief.expected` only once it has actually seen the good, else the public `realized_price`,
      else skips the decision — distinguishing never-observed from a tick-0 observation (not
      `last_seen == 0`). `forecast = base × forecast_bias`, routed into the role-choice adopt, the
      per-agent capital build, and the project input-bid OUTPUT price; input/build costs stay observed.
      The market still clears at the **real** price.
- [x] **S11.2 — profit/loss realization + capital selection (the falsifiable core).** A net-worth
      balance sheet `agent_capital(i) = gold + WOOD × realized_wood_price + tools × V`, where `V` is the
      tool's realized liquidation price if tools ever trade **else ZERO** (tools don't trade — an idle
      tool adds nothing, so a sunk-WOOD loss cannot hide in it), plus realized-proceeds / forecast /
      belief accessors. **The negative-NPV microtest (test 3) — the tripwire — shows a clean signed
      selection effect:** on a controlled chain where building does NOT pay at the real price, the
      accurate forecaster **declines and preserves** capital while the over-optimist **builds, realizes
      the real (lower) proceeds, and ends STRICTLY LOWER on `agent_capital`**.
- [x] **S11.3 — shock → discoordination → recovery (a real chain shock).** A settlement-level
      `set_bake_stage_enabled(false)` over `[A, B)` (the `maybe_unlock_tier_two` dual-flip path) — the
      test first asserts the shock actually collapses bread output in `[A, B)` (not a no-op), then that
      the production dip recovers to pre-shock bounds in the tail with no planner correction, conserving
      every tick.
- [x] **S11.4 — the `entrepreneurial` scenario** (`frontier_coemergent_strong_entrepreneurial`, derived
      from the S10 originary base with `entrepreneurial_forecasts` on) + the acceptance suite
      (`sim/tests/entrepreneurial_uncertainty.rs`: eight named tests). All additive/gated: with the flag
      off the S5–S10 scenarios + the six econ + the g5a/g5b/coemergence emergence goldens are
      byte-identical (the flag, the per-colonist `forecast_bias_bps`, and the per-belief `observed` flag
      are serialized only under the flag — `canonical_bytes_include_forecast_bias` /
      `canonical_bytes_include_entrepreneurial_flag_and_belief_observed`).

**Does profit/loss selection actually bite? Yes.** On the controlled negative-NPV chain (uniform bias,
seed 1), an all-accurate colony declines/preserves (it tools up minimally and keeps its WOOD/gold) while
an all-optimist colony overbuilds and ends with **materially lower** total `agent_capital` — the
over-optimist sinks real WOOD into capital that under-earns at the real price. Selection operates on
**capital accumulation**, so no starvation is needed (`hunger_critical` stays disabled; every death in
the flagship run is old age). The single clean lever is the **output-revenue** forecast; input-cost and
build-cost forecasting, richer expectation (variance/confidence), and re-enabled mortality selection are
noted follow-ons.

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
cargo run -p viewer -- run lineages --ticks 200        # G4b: two households age, reproduce, inherit
cargo run -p viewer -- run barter-camp --ticks 40             # G5a: money emerges (barter → promotion → money-priced)
cargo run -p viewer -- run barter-camp-control --ticks 40     # G5a: no saleability differential → stays in barter
cargo run -p viewer -- run frontier --ticks 80                # G5b: money emerges, then roles adopt, with demography
cargo run -p viewer -- run endogenous --ticks 1600           # endogenous specialization: the chain self-organizes on a subsistence base and sustains, no curated placement
cargo run -p viewer -- run scaling --ticks 1600              # S6: productive re-entry provisions the stranded tail while preserving the endogenous chain
cargo run -p viewer -- run capital --ticks 1600              # S7: colonists build mills/ovens under unmet demand — more tools + higher bread than scaling
cargo run -p viewer -- run coemergent --ticks 1600          # S8: money + chain + capital CO-EMERGE from a no-money barter start (era goes barter → money, then bread sustains)
cargo run -p viewer -- run strong-emergence --ticks 1600    # S9: strong-bar emergence — money emerges from real indirect-exchange breadth (no configured medium want)
cargo run -p viewer -- run originary --ticks 1600           # S10: originary interest — capital forms by a PER-AGENT intertemporal choice; patient colonists build, present-biased ones do not
cargo run -p viewer -- run entrepreneurial --ticks 1600     # S11: entrepreneurial uncertainty — decisions weigh a PER-AGENT fallible forecast; a wrong forecast is borne as profit/loss through capital
#                                                              # G6a: the frontier/barter-camp dashboards show an era
#                                                              #      banner + per-tick era column (forager → … → capital)
cargo run -p viewer -- run research --ticks 60                # G6b: Knowledge accrues, tier 2 unlocks, pastry is produced
cargo run -p viewer -- run research-control --ticks 60        # G6b: no scholars → no Knowledge → tier 2 never unlocks
cargo run -p viewer -- run roads --ticks 60                   # G7: a road is built from labor, transit drops, the gap converges faster
cargo run -p viewer -- run roads-control --ticks 60           # G7: no road → transit stays high → the gap converges slower
cargo run -p viewer -- run m3-settlement --ticks 40           # G8a: the viable economy on M3 ledger money (specie composition banner)
cargo run -p viewer -- run bank --ticks 40                    # G8b: a fractional-reserve bank — deposits, claims, fiduciary credit
cargo run -p viewer -- run bank-full-reserve --ticks 40       # G8b: the 100%-reserve control — deposits circulate, zero fiduciary
cargo run -p viewer -- run credit-cycle --ticks 80           # G8c-1: the Austrian cycle — regime descends to Fiat, gap opens, boom, stop, bust, capital consumed
cargo run -p viewer -- run sound-money --ticks 80            # G8c-1: the sound-money control — SoundGold, no fiat, no gap, no cycle
cargo run -p viewer -- run wage-tender-cycle --ticks 80      # G8c-2: fiat wages legal tender → the credit transmits to the boom/bust (the cycle fires)
cargo run -p viewer -- run wage-refusal-cycle --ticks 80     # G8c-2: specie-only wages → the same fiat credit is inert (no boom, no bust)
cargo run -p viewer -- run spot-tender-legal --ticks 12      # G8c-2: spot tender (M11) — fiat is legal tender, the held fiat settles goods trades
cargo run -p viewer -- run spot-tender-refusal --ticks 12    # G8c-2: spot tender control — fiat refused, specie settles the same trades (broad money unchanged)
cargo run -p viewer -- run debt-tender-legal --ticks 12      # G8c-2: debt tender (M12) — fiat is legal tender, the debt is discharged in fiat
cargo run -p viewer -- run debt-tender-refusal --ticks 12    # G8c-2: debt tender control — fiat refused, the debt is discharged in specie (broad money unchanged)
cargo run -p viewer -- run bank-repayment-tender-legal --ticks 5      # G8c-2: bank repayment (M15) — bank claim accepted, credit retired
cargo run -p viewer -- run bank-repayment-tender-refusal --ticks 5    # G8c-2: bank repayment control — held claim refused
cargo run -p viewer -- run issuer-repayment-tender-legal --ticks 14   # G8c-2: issuer repayment (M16) — fiat accepted, credit retired
cargo run -p viewer -- run issuer-repayment-tender-refusal --ticks 14 # G8c-2: issuer repayment control — held fiat refused
cargo run -p viewer -- run tax-in-fiat --ticks 80           # G8c-3: a fiat-receivable tax compels fiat through the fiscal channel where wages refused it
cargo run -p viewer -- run tax-in-specie --ticks 80         # G8c-3: the specie-receivable control — tax settles in specie, no compelled fiat demand
```
