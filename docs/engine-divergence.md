# Engine divergence log

`econ/` was forked from the **praxsim** laboratory at commit `0729227`
(post-M21). The lab stays alive as the clean-room; `econ/` is free to diverge
where the colony game needs architecture the lab does not. This log records
every **deliberate** post-fork divergence — what changed, why the lab's
observable surface is still preserved, and what lab-compat scaffolding is kept
rather than removed.

The standing rule (game-spec §10.1): the conformance suite — the four series
goldens (M0/M1/M2/M3), the M18/M20 emergence goldens, and the M5/M6 anchors —
gates every change and must stay **byte-identical** until a divergence is
deliberately taken and recorded here. "Behavior-preserving" means exactly: the
goldens do not move.

## Fork point

- Lab: praxsim @ `0729227` (the full M0–M21 ladder).
- Fork: `econ/` (G0a fork commit `fddea58`), conformance suite green,
  goldens replay byte-identical.

---

## G0b — engine migrations behind compatibility (`docs/impl-g0b.md`)

Three migrations from game-spec §11, each priced honestly and each gated by the
same proof: the conformance suite stays green and the lab goldens stay
byte-identical — natively where possible, through a thin compat layer where
not. None of the lab's economics change; this is plumbing.

### 1. `AgentId` widened `u32 → u64` with packed generation

- **Lab shape:** `AgentId(pub u32)` — a bare numeric id, never reused, never
  freed.
- **Fork shape:** `AgentId(pub u64)` packing `(generation << 32) | index`. The
  low 32 bits are the numeric id exactly as before; the high 32 bits are a
  generation that demography (G4) will bump when a slot is reused.
- **Why preserved:** every lab id has generation 0, so the packed value equals
  the index. The derived `Ord` therefore matches the old `u32` ordering, and
  `Display`/`{}`/`.0`-based formatting prints the same digits — every tape and
  CSV path is byte-identical. A regenerated id (generation ≥ 1) sorts *after*
  its generation-0 ancestor and formats as `index#gen`, a surface no existing
  golden references.
- **Width-dependent sites fixed to be width-agnostic** (behavior-preserving by
  the golden gate): construction sites that passed a typed `u32`
  (`AgentId(id)`) now pass `AgentId(u64::from(id))`; a handful of test golden
  hashes that fed `AgentId.0` into a `u32` sink now use `.index()`. Same bytes
  out.
- **Lab-compat surface kept:** the public `.0` field stays (`pub u64`);
  accessors `index()`, `generation()`, `with_generation(index, generation)`
  carry the new semantics. `with_generation(n, 0) == AgentId(u64::from(n))`.

### 2. Dynamic `GoodRegistry` (goods become data)

- **Lab shape:** goods are hard-coded `GoodId` constants
  (`GOLD..ORE = GoodId(0)..GoodId(6)`) with a `good_name(GoodId) -> &'static str`
  match and a static `worldgen::GOOD_POOL`.
- **Fork shape:** `registry::GoodRegistry` interns goods by name into `GoodId`
  values (index `== GoodId.0`), so a future `content/` layer (G3) can define
  them. `Society` holds a `registry: GoodRegistry`, constructed `lab_default()`
  everywhere the lab constructs a society.
- **Why preserved:** `GoodRegistry::lab_default()` interns the exact lab set in
  the exact id order (`gold, food, wood, net, salt, cloth, ore`), so `GoodId`
  values, `Stock`/belief slot counts, and names are bit-for-bit the lab's. The
  acceptance suite asserts the lab-default registry equals the legacy constants
  one by one (names, ids, count) and that `len()` drives `Stock` sizing equal to
  the constant-derived size.
- **Lab-compat surface kept (NOT removed in G0b):** the `GOLD..ORE` constants
  and `good_name` remain exported. `good_name` is now a thin shim over the
  lab-default name table (`good::LAB_GOOD_NAMES`) that the registry also
  interns, so the registry, the constants, and `good_name` agree by
  construction. Registry-aware callers use `GoodRegistry::name` /
  `Society::good_name`; the free `good_name` stays for the display paths the
  goldens cover. `worldgen::GOOD_POOL` keeps its lab shape (no `content/` yet).
- **Renderer registry-awareness is deferred to G3 (recorded, not done here).**
  `report.rs` renderers (`render_v2_table`, `render_v2_csv`, `render_tape`) and
  the per-good capture in records like `MarketRecord` still resolve names
  through the lab-compat `good_name` shim and print the lab's fixed named
  columns. This is *correct-by-construction* in G0b: no code path can yet
  construct a non-lab good (no `content/`, `GOOD_POOL` is the lab set), so every
  `GoodId` a renderer sees is in the lab-default table and resolves to its real
  name — never `"unknown"`. Migrating the renderers to registry-aware naming
  (and surfacing dynamic per-good columns) lands with the first dynamic content
  (G3) or the G2 inspectors, whichever comes first. Doing it now would add an
  unexercised code path with no golden to guard it.

### 3. `Command` result/error semantics (additive)

- **Lab shape:** `Society::apply_event_kind` mutates and returns nothing,
  silently tolerating a missing target (an unknown debt id in `SetDebtDueTick`,
  a no-issuer `LevyTax`, a redemption with no money system). This silence is
  **load-bearing** for authored scenarios.
- **Fork shape:** an additive command surface — `apply_command(EventKind) ->
  CommandResult` where `CommandResult = Applied | Rejected(CommandRejection)`
  and every currently-silent no-op returns a named `RejectReason`
  (`UnknownDebt`, `NoIssuer`, `UnknownAgent`, `UnknownBank`, `UnknownIssuer`,
  `UnknownRecipe`, `NotApplicableToKernel`, `Ineligible`).
- **Why preserved:** the command and event paths share one implementation
  (`apply_event_kind`, now returning the result and taking an `ApplyMode`). The
  event path (`apply_event`) runs it in `ApplyMode::Event`, discards the result,
  and so its mutations and its silent tolerance are byte-for-byte the lab's —
  the existing event tests are untouched. The command path (`apply_command`)
  runs it in `ApplyMode::Command` and returns the result.
- **Command-only preconditions (the asymmetry).** Most rejections are
  *mutation-preserving*: the lab also performed no mutation when the target was
  missing, so an event-path `Rejected` (discarded) is byte-identical to the
  lab's silent no-op. A few cases are NOT symmetric — the lab *mutated
  regardless* of whether a referenced agent or bank existed, and that silent
  tolerance is load-bearing for authored scenarios:
  - `LevyTax` seeds the tax debt unconditionally after the mutation-preserving
    one-issuer check — even for a zero amount (an open zero-due liability) or a
    missing borrower (an unowned one);
  - `SeedCommodityDebt` seeds the debt even when neither party is a live agent;
  - `RedeemDemandClaims` on a missing bank records `BankMissing` audit rows for
    an explicit `Agents` route; a zero per-agent cap requests nothing and
    mutates no balances (it may still record `NoClaim` rows for explicit
    zero-claim holders), so a command rejects it rather than report `Applied`;
  - `StopBankCredit` / `StopIssuerCredit` cancel resting lender quotes
    unconditionally.
  These existence/amount preconditions are gated on `ApplyMode::Command` only:
  the event path keeps the lab's unconditional mutation; a command rejects
  loudly (`UnknownAgent` / `UnknownBank` / `UnknownIssuer` / `Ineligible`) and
  side-effect-free, *before* any audit row or quote cancel. The acceptance
  suite asserts both sides (e.g. `event_path_seeds_debt_against_missing_agent`
  versus the command-path rejections, and the in-crate
  `targeted_redemption_route_records_explicit_failures` for the audit path).
- Nothing in `econ` calls `apply_command` yet besides tests; it is plumbing for
  the sim crate's future command queue (game-spec §7).

### Storage: `AgentArena` replaces `Vec<Agent>` + `agent_order`

Supporting the generational `AgentId`, `Society`'s `agents: Vec<Agent>` and the
id-resolution index migrate to `arena::AgentArena` — id-ordered, deterministic
(`BTreeMap`, no `HashMap`), with slot reuse and generation bumping on `free`.
The lab path uses `insert_with_id` (authored sparse ids, generation 0, never
freed); iteration order, positional access, and lookups equal the old
`Vec` + `agent_order` exactly (the goldens are the proof, and a property test
checks it directly). `free`/`insert` are unit-tested but **no engine path frees
an agent in G0b** — that arrives with demography (G4). `free` is
*order-preserving* (it slides later agents down a slot rather than swapping the
tail in), so `as_slice`'s documented cast-order invariant survives a free; a
unit test (`free_preserves_slice_order_for_a_middle_agent`) checks it. When a
slot's generation cannot advance on `free` (it is already at `u32::MAX`), the
slot is **retired** rather than reused: reusing it would have to reissue the
just-freed id and defeat stale-id detection, so it is dropped from the free list
permanently and the freed id stays resolvable to `None`
(`free_at_max_generation_retires_slot_without_reissuing_id`). Note that
`iter()` yields **id order** while `as_slice()` yields **insertion order**; the
two coincide for the lab but an order-sensitive G4 caller must choose
deliberately. The scenario-policy
`agent_order` (a priority override used by two M14 scenarios) is retained as a
distinct concept; it is not the same as id order.

**G4 follow-up (not a G0b change).** `Society` still keeps a few *physical*
position caches that point into the arena's dense slice (e.g. the order-book
`agent_index` rebuilt in `ensure_bid`/`ensure_ask`, and the scenario `agent_order`).
Today nothing frees an agent mid-run, so those caches are always rebuilt against
a stable layout and the lab is unaffected. When G4 wires `free`/`insert` into an
engine path, a removal shifts later slots, so those caches must move to stable
`AgentId`s (or be rebuilt after any arena mutation) before demography lands. The
order-preserving `free` keeps the arena self-consistent in the meantime; it does
not retroactively fix external position caches — that is deliberately G4 work.

`free`'s cost is also a documented G4 prerequisite. The order-preserving removal
is **O(N)**: it slides every later live agent down one slot and walks `index_of`
and `live_order` to re-point the shifted positions. G0b's contract for `free` is
CORRECTNESS only (stale ids resolve `None`, no revival, iteration excludes freed
agents), proven at the unit level; no engine path frees, so the cost is never
paid on a hot loop today. When G4 turns on per-tick demography this becomes a
real bottleneck, to be addressed there (e.g. tombstone-in-place with a compaction
pass, or a slab that does not shift) alongside the position-cache reconciliation
above — not re-litigated as a G0b regression.

### Excluded from G0b (deferred)

- No good attributes beyond name (G3 brings durability/perish with `content/`).
- No actual agent death/birth (G4); the arena exists and is unit-tested only.
- No game `Command` enum (DesignateZone etc. are sim-crate work, G2+) — only
  the result-semantics plumbing over the existing `EventKind`.
- No lab back-port: the lab keeps its own shapes; this log is the bridge.

---

## G1 — needs → wants (the `life` crate, `docs/impl-g1.md`)

G1 adds the `life` crate (the first genuinely new game code) and the
needs→wants transformation: a colonist's value scale is **generated from need
state each tick** rather than authored once. `life` depends on `econ` and must
not change its economic behavior — the conformance suite stays byte-identical,
proven green by `cargo test` across the workspace. The only `econ` changes are
**additive** and are recorded here.

### 1. Additive public hooks on `Society` (no behavior change)

The `Camp` driver builds a `Society` and, between steps, reads realized
quantities back to advance needs, invalidates stale quotes after overwriting a
scale, and tombstones starvation deaths. The public surface is additive;
goldens never touch it:

- `realized_price(GoodId) -> Option<Gold>` — the last trade price for a good
  (reads the existing `realized_prices` map).
- `enable_consumption_log()` / `consumption_log_last_tick() ->
  &[(AgentId, GoodId, u32)]` — an **opt-in, off-by-default** per-tick log of
  consumed-good quantities, captured in the consume phase of `step_m1` only when
  enabled. The capture happens before direct-labor provisioning, so it is a
  conservative readback (a direct-labor recipe that satisfies a current want is
  not credited here). The capture is gated behind a flag the goldens never set,
  so the M1 series golden is byte-identical; the camp scans the batch slice each
  tick to read FOOD/WOOD consumption back for need replenishment.
- `labor_used_last_tick() -> &[(AgentId, u32)]` — batch read access to the
  existing per-tick labor tally, so the G1 camp can update rest without a
  per-agent scan.
- `cancel_changed_live_quotes_for_agent(AgentId) -> bool` — an explicit
  invalidation hook for callers that rewrite an agent's scale between econ
  ticks. It cancels only quotes whose reservation no longer matches the current
  scale/holdings/tender state, releasing stale reservations before the next
  consume phase.
- `cancel_changed_live_quotes_for_agents(&[AgentId])` — the same invalidation
  hook in batch form for drivers that rewrite many scales at once.
- `tombstone(AgentId) -> bool` — the G1 starvation-death hook recorded below.

The field additions (`consumption_log`, `consumption_log_enabled`) are inert
unless the flag is set. The conformance suite is the proof.

### 2. Death by starvation is a **tombstone**, not an arena free (the seam)

When a colonist's hunger holds at its critical ceiling for the death window,
`Society::tombstone(AgentId)` marks it dead:

- its value scale is **emptied** (so the order/quote machinery posts nothing
  for it) and its labor capacity zeroed;
- its resting orders across every book — spot quotes, labor-book orders, and
  time-market (loan) orders — are cancelled and their reservations released, so
  no stale order can match a counterparty after death (releasing a reservation
  only un-earmarks the dead agent's own gold; nothing settles to anyone);
- due-debt settlement skips open contracts whose borrower or agent-lender is
  tombstoned, so existing debts cannot move a dead agent's frozen holdings;
- any capital project it still owns is **frozen in place** — project maturity
  neither mints the output nor credits the dead owner, and abandonment never
  returns salvage or records capital loss for a tombstoned owner's project. G1
  itself runs M1, which has no projects; this keeps the public hook's "holdings
  frozen" guarantee complete for any society kind;
- it is **dropped from the activation order** so no later tick processes it.

The arena slot is **NOT freed** and the holdings (gold + stock) are **NOT
settled** to anyone — they stay frozen in place. Consequences, all deliberate
for G1:

- a dead colonist's gold/stock remain in the conservation totals
  (`total_gold` / `total_stock`) — *frozen-holdings-in-conservation is the
  correct G1 behavior*; the smoke test tracks **living** colonists separately;
- `AgentArena::free` is **not** called (its cache reconciliation is the G0b-
  documented G4 prerequisite above);
- no estate settlement, no birth/aging/households, and none of the `Society`
  position caches that a real `free` would have to reconcile are touched.

This is the honest minimal death model. **Full estate settlement, the
`AgentArena::free` + Society-cache-reconciliation path, and demography are G4**
(game-spec §5.6); G1 does not pretend to do them. The tombstone is the declared
seam parked here until then.

> **Architectural note for G4.** G1 enforces tombstone inertness with a
> guard in each agent-iterating phase (order books, debt settlement, project
> maturity, project abandonment) rather than a single aliveness gate — the
> coverage is complete and tested, but it is N guards, one per phase. When
> G4 adds real `free`/estate settlement it should consolidate these into one
> `is_tombstoned`/activation check that every phase consults, so a future
> agent-iterating phase cannot silently reintroduce a leak. This was a
> deliberate G1 stopgap, recorded so the cleanup is explicit and not
> rediscovered.

### Excluded from G1 (deferred)

- No spatial structure, movement, stockpiles, or two-rate loop (G2); the `Camp`
  driver is the lean pre-`sim` stand-in, to be absorbed by `sim` at G2.
- No birth/aging/households/migration/estate settlement; no arena free (G4).
- No new goods/recipes/tech (G3); the good set is the lab's.
- The G1 need set is the load-bearing trio that maps onto existing lab goods —
  hunger↔FOOD, warmth↔fuel (WOOD), rest↔Leisure. Shelter/social/security
  (game-spec §5.2) wait until they have goods/buildings to satisfy them (G2/G3).
- No balance tuning or asserted economic magnitudes — the acceptance suite
  asserts scale-generation *properties* and non-collapse only.

---

## G2a — the `world` crate (spatial substrate, `docs/impl-g2a.md`)

G2a adds the `world` crate (the spatial substrate) and, with it, a planning
decision: **G2 is decomposed.** This is recorded here because it changes the
roadmap shape, not because it changes any engine behavior — the standing rule
above is satisfied *vacuously*, since `econ` is not edited at all.

### 0. The G2 decomposition (supersedes the single-G2 lump in game-spec §11)

The roadmap's G2 bundles four large pieces — the `world` crate, the two-rate loop
with the §4.3 delivery-escrow contract, the `Society`-monolith extraction for
multiple settlements, and the debug viewer with inspectors — into one milestone.
That is far too much for one reviewed change (G1, a pure function plus a driver,
took eight review rounds). G2 is therefore sliced in dependency order:

- **G2a (this entry): the `world` crate** — the spatial substrate, standalone and
  econ-independent.
- **G2b: two-rate loop + escrow** — wire `world` delivery under the econ tick via
  the §4.3 delivery-escrow contract for one settlement (DoD: distance measurably
  affects realized prices; delivery escrow conserves exactly).
- **G2c: settlement-scoped service extraction** — pull market/labor/barter books
  out of the `Society` monolith so multiple settlements exist.
- **G2d: debug viewer + inspectors** — the first binary; the price→trades and
  colonist→scale-and-why inspectors the game-spec mandates for G2.

G2a is the lowest-risk slice and the foundation G2b/G2c/G2d build on; it is needed
next regardless of how the rest is sliced, so it is built first.

### 1. New **leaf** crate — no econ coupling, goldens safe by construction

`world` is a new workspace member that depends on `econ` **only** for the shared
primitives `GoodId` / `AgentId` / `Rng` (re-exported from `world` so G2b can
bridge world↔econ with no type translation — mirroring how `life` depends on
`econ`). Crucially:

- `world` calls **no** econ economic logic and **no** mutation path on any econ
  type beyond constructing/reading the three primitives; it changes no econ
  behavior.
- `econ` does **not** depend on `world` — the dependency edge is one-way. `world`
  is a leaf: nothing in the engine, in `life`, or in the conformance suite can
  observe it.

So the econ conformance goldens (the four series goldens M0/M1/M2/M3, the M18/M20
emergence goldens, and the M5/M6 anchors) and the entire G1 `life` suite are
**byte-identical and green by construction** — adding a leaf crate cannot move
them. There is no compatibility shim to keep here because nothing in the engine
was touched; the acceptance test `econ_and_life_unchanged` re-runs econ scenarios
from `world`'s own workspace as a usability/non-perturbation check, and the
workspace-wide `cargo test` / `cargo clippy --workspace --all-targets -- -D
warnings` / `cargo fmt --check` are the gate.

### 2. The spatial substrate, and what it deliberately is *not*

`world` provides a tile `Grid` with passable/impassable terrain, `ResourceNode`s
(a good, a stock, an optional per-tick regen clamped to a cap), `Stockpile`s
(capacity-bounded integer storage), and agent spatial state (position + carried
inventory + a `Task`). `World::tick()` advances movement along **deterministic
BFS shortest paths** around obstacles, applies arrivals (harvest into carry /
deposit into stockpile), regenerates nodes, and emits a per-tick conservation
report. `World::generate(seed, &WorldGen)` builds a world from a seed.

Two invariants are enforced and tested:

- **Determinism.** Integer state; the `Rng` is consumed at *generation* only and
  `tick()` draws nothing; agents iterate in `AgentId` order; storage is
  `BTreeMap`/`Vec`, never `HashMap`. Same seed + same command sequence →
  byte-identical world (`canonical_bytes` / `digest`).
- **Conservation.** Node regen is the *only* source of goods (clamped to `cap`,
  fully accounted in the per-tick report); movement, harvest, and deposit
  relocate units without creating or destroying one; a deposit that overflows a
  stockpile's capacity leaves the remainder carried, never destroyed. The world
  total changes per tick by exactly the report's `regenerated`.

It is a **pure spatial** layer: it does **not** know prices, money, wants, or
trades. Goods are tracked only as integer quantities of `GoodId` at locations or
carried by agents.

### Excluded from G2a (deferred)

- No econ-tick coupling, prices, money, wants, or trades; no escrow ledger — the
  two-rate loop and the §4.3 delivery-escrow contract are **G2b** (`world` only
  reports delivered/undelivered quantities; the escrow accounting lives in the
  integration).
- No multiple settlements / `Society` monolith extraction (**G2c**).
- No binary, viewer, or inspectors (**G2d**).
- No `life`/`Camp` changes — G2b rewires the driver onto `world`.
- No RNG in `tick`; no `HashMap` in logic; no new external dependencies (pure std
  besides the `econ` primitive dep).
