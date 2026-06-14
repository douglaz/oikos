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

> **Superseded by G4a.** The freeze-in-place tombstone described in this section
> was the G1 stopgap. G4a retired it: `Society::tombstone` is replaced by
> `Society::remove_agent`, which settles the estate, frees the arena slot, and
> reconciles the external caches. The G1/`Camp` tests were migrated to the
> real-removal semantics. The contract below is kept for the historical record;
> see the **G4a** section at the end of this document for what is now done.

When a colonist's hunger holds at its critical ceiling for the death window,
`Society::tombstone(AgentId)` marked it dead (G1, now `remove_agent`):

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

---

## G2b — two-rate loop + delivery escrow (the `sim` crate, `docs/impl-g2b.md`)

G2b adds the `sim` crate — the two-rate orchestrator (game-spec §4.1, §4.3) — and
wires `world` delivery under the econ tick for **one** settlement. `sim` owns a
`world::World`, per-colonist `life` need state, and an `econ::Society`, and runs
the fast loop (`FAST_TICKS_PER_ECON_TICK` `world` ticks of movement / harvest /
haul) under one economic tick (transfer → needs/tombstone → scale regeneration →
`Society::step` → consumption read-back → task reassignment). It reuses `life`'s
`regenerate_scale` / `NeedState` / `CultureParams` / tombstone and `world` /
`econ` as-is; the only engine edits are the two **additive, conserving**
accessors that realize the world↔econ seam, recorded below. The conformance
suite (the four series goldens M0/M1/M2/M3, the M18/M20 emergence goldens, and
the M5/M6 anchors) and the entire G1 (`life`) and G2a (`world`) suites stay
green and byte-identical — the proof is the unchanged workspace `cargo test`.

### 0. `sim` supersedes `Camp` as the driver (Camp kept)

`life::Camp` was the lean pre-`sim` G1 stand-in. `sim::Settlement` absorbs its
role — generate colonists, update needs, tombstone deaths, regenerate scales,
step the econ market, read consumption back — and adds the spatial fast loop and
the transfer seam. `Camp` is **not deleted**: it stays as the G1 non-spatial
reference harness with its 11 tests intact, keeping the G1 proof and bounding the
diff. Going forward `sim` is the driver.

### 1. The world→econ transfer seam (the load-bearing design)

A good has **one owner at a time** — `world` (node / carry / stockpile) **or**
`econ` (agent stock) — never both. The lifecycle is

```
node stock --harvest--> hauler carry --deposit--> exchange stockpile
  --[econ-tick transfer]--> econ agent stock --trade/consume--> (econ)
```

While in `node` / `carry` / `stockpile` a unit is a `world` good (G2a's
conservation owns it). Carry-while-moving **is** the §4.3 delivery escrow:
committed to delivery, conserved, **retained** (never destroyed) if the hauler is
blocked or dies. There is **no separate escrow ledger** — escrow is just carry.
At each econ tick `sim` performs the transfer: delivered exchange-stockpile units
are *credited to the depositing colonist's econ stock* and then *withdrawn from
the world*, atomically and conservingly. A unit that cannot be credited stays
world-owned in the exchange stockpile, never destroyed and never double-counted in
econ — two cases: a **live** depositor whose stock is momentarily at the `u32`
ceiling is transient (the attribution is retried each econ tick and transfers once
consumption opens headroom), while a **removed** (dead) depositor is rejected
permanently at the transfer — its freed id no longer resolves — so its pending units
stay world-owned in the exchange (still conserved, never crossing the seam) until G4a's
estate settlement drains such stranded escrow to the commons on death (see the G4a
section). The transfer is **net-zero**
(`world` −n, `econ` +n); harvest
and deposit are net-zero relocations; node regen (source) and consumption (sink)
are the only non-zero deltas, each independently accounted. This is the
whole-system ledger invariant `sim` checks every econ tick (the G2b conservation
DoD, the test-2 tripwire).

### 2. Two additive accessors realize the seam (goldens byte-identical)

The seam needs one new capability on each side of the world↔econ boundary. Both
are purely additive — they touch no existing path, no scale/quote/money/market
state — and are called by **no** engine path, so the goldens cannot move (the
unchanged suite is the proof). Each is unit-tested in its own crate.

- **`world`: `World::stockpile_withdraw(StockpileId, GoodId, u32) -> u32`** (and
  its `Stockpile::withdraw` mirror of `deposit`). `world`'s public API had no
  sink — through it the world total is monotonically non-decreasing — so
  realizing the spec's "**removed from the world stockpile**" required exactly
  this one accessor: the mirror of a deposit, relocating units *out of the world*
  to the caller. It is **out-of-tick** — `World::tick` never calls it, so the G2a
  per-tick `TickReport` and every G2a conservation/movement test are untouched
  and byte-identical. After a withdraw, `World::total_goods` drops by exactly the
  amount removed (the world's only way to lose a unit). This is the `world` side
  of the seam, the analog of the `econ` accessor below; G2a's "no econ-tick
  coupling" boundary is unchanged (the *coupling* lives in `sim`, the accessor is
  inert plumbing).
- **`econ`: `Society::credit_stock(AgentId, GoodId, u32) -> bool`** — credit a
  good to a live agent's stock (returns `false` for an unknown, stale, or
  tombstoned id, then credits nothing). The `econ` side of the seam: `sim`
  credits a depositing colonist exactly the units it will withdraw from the
  world once the credit succeeds. Additive and off-by-default like the G1 hooks;
  rejecting tombstoned agents preserves the frozen-holdings death contract.

### 3. The settlement economy (mechanism, not balance)

The minimal division of labor the spec calls for: **gatherers** harvest FOOD
from a node and haul it to the exchange (the transfer credits it to their econ
stock; they sell it); **consumers** sit at the exchange and buy/eat FOOD. The
distance→price experiment moves the gatherers' node and compares two otherwise
identical runs (`SettlementConfig::price_probe`); fewer units reach the market
per econ tick (test 6, monotone), so the realized FOOD price is strictly higher
(test 5, **sign only** — no magnitude is pinned; price tuning is G2+/later).
Notes on the modeling choices, so they are explicit and not mistaken for engine
changes:

- **FOOD** is the spatial good (source = its node's regen, sink = consumption).
  **WOOD** is a closed provisioning good (it never enters the world; source none,
  sink consumption — so test 2 holds for it with regen 0): the colonists' warmth
  staple and the medium that recirculates gold so the market stays liquid.
- **Money is closed in every settlement config**: no `sim` path mints or burns
  gold, so the fast loop never moves money and `Society::step` only redistributes
  a conserved total (the §4.3 rule; test 4). The distance *probe* uses a larger
  initial consumer gold balance, not loop-time income, so scarce supply can lift
  bids without any money mutation outside `Society::step`.
- **Sustenance is a smoke test** (test 7): a supply-rich, closed-gold `viable()`
  config runs several econ-years without collapse (both vocations populated,
  hunger bounded), deterministic. It is not a balance claim.

### Excluded from G2b (deferred)

- No multi-settlement and no `Society`-monolith extraction (**G2c**).
- No binary, viewer, or inspectors (**G2d**).
- No **wage-labor escrow** — the same §4.3 escrow pattern, but it needs spatial
  hiring/projects that arrive later; G2b's escrow is the haul form only (noted,
  not built).
- No deletion of `life::Camp` or any change to `econ`/`world`/`life` *behavior* —
  the two seam accessors are additive and the goldens stay byte-identical.
- No balance tuning or asserted magnitudes — conservation is exact and
  distance→price is **sign only**.
- No RNG in either loop (consumed only at `Settlement::generate`); no `HashMap`
  in logic; no new external dependencies (pure std over the three path deps).
- No pre-optimization against imagined scale — the G2a-deferred per-tick BFS and
  stockpile-sum costs did not bite under the two-rate load (8–16 agents, a
  corridor grid), so they stay deferred, not re-litigated here.

---

## G2d — debug viewer + inspectors (the `oikos` binary, `docs/impl-g2d.md`)

G2d adds the `viewer` crate — the workspace's **first binary** (`oikos`) — a
headless, deterministic, text-only debug viewer with the two inspectors the G2
roadmap mandates (price→trades, colonist→scale/why). It is recorded here not
because it changes any engine behavior — it does not — but because it is the
first runnable artifact and because it touches `sim`'s public surface (additively),
so the standing rule's proof is worth stating explicitly: the six conformance
goldens (the four series M0/M1/M2/M3, the M18/M20 emergence goldens, and the
M5/M6 anchors) and the entire G1 (`life`) / G2a (`world`) / G2b (`sim`) suites
stay green and byte-identical — the unchanged workspace `cargo test` is the
proof.

### 1. The viewer is **read-only** — it renders, it never mutates

Every renderer (the `run` dashboard and both inspectors) draws **only** from
`sim`'s existing read-only accessors over a `Settlement` that was advanced by a
*seeded* run (`Settlement::generate` + `econ_tick`/`run`):

- the dashboard reads `living_count` / `realized_price` / `tracked_goods` and the
  per-tick `EconTickReport` (`transferred_of` / `consumed_of` / `conserves` and
  the whole-system before/after totals for the loud `VIOLATED:<good>` cell), plus
  `need_of` / `is_alive` / `population` for the needs summary;
- the **price→trades** inspector reads `society().trades` (the trade tape) filtered
  to the good/tick, with `society().good_name` labels, and `realized_price(good)`.
  Because `realized_price` is the engine's *most recent* clearing price (carried
  forward across quiet ticks), the inspector shows it plainly when a trade cleared
  at the inspected tick, but on a tick that cleared none it labels the price as
  carried over and names the source tick (`carried from tick S; no <good> trade
  cleared at tick T`) — so the price is never read as "the price behind these
  (zero) trades", and the price→trades pairing stays honest on quiet ticks (e.g.
  the `far` distance scenario, whose long haul leaves many ticks uncleared);
- the **colonist→scale/why** inspector reads `society().agents.get(id)` for the
  ranked value `scale` and `gold`, and `vocation_of` / `is_alive` / `need_of` /
  `carry_of` for the rest of the "why" (a tombstoned colonist surfaces as dead
  with the emptied scale `sim` already maintains).

The viewer draws **no** RNG and runs no new economic mechanic. Determinism is
inherited: same `(scenario, ticks, seed)` → byte-identical output (the acceptance
suite's tripwire). No `sim`/`econ`/`world`/`life` *behavior* is touched.

### 2. The only engine-surface change: additive read-only re-exports on `sim`

The renderers read through the accessors above, whose return and element types
are `econ`/`life` types — the `Society` behind `society()` and its trade tape's
`Trade`, the `Agent` behind `society().agents` and its value scale's `Want` /
`WantKind` / `Horizon`, the realized-price `Gold`, the `NeedState` behind
`need_of`, and the good ids (`GoodId`, `FOOD`, `WOOD`) plus `AgentId`. So `sim`
**re-exports** that read surface (`pub use` of `econ::{agent, good, market,
society}` items and `life::NeedState`). This is the
entire diff to a non-`viewer` crate, and it is purely additive: it adds public
surface but defines no new behavior and touches no existing path, so it cannot
move a golden (the unchanged suite is the proof). Keeping the re-exports on `sim`
lets the viewer depend on **`sim` alone** — a thin binary over one crate —
instead of reaching into `econ`/`life` directly.

**No new `sim` accessor was required.** The spec budgeted for an additive
read-only `sim` accessor if a renderer needed one; in the event, the G2b accessor
surface (`society()`, `realized_price`, `tracked_goods`, `need_of`, `vocation_of`,
`is_alive`, `carry_of`, `colonist_id`, `population`, `living_count`, the
`EconTickReport`) already exposed everything, so none was added — the smallest
possible change.

### 3. The binary, and the `check`-artifact lesson

`viewer` is both a library (so `viewer/tests/` can drive the renderers and assert
on their `String` output — renderers return strings, never write stdout) and the
`oikos` binary (`src/main.rs`, which only parses args, calls `viewer::run`, and
prints). Errors are **loud**: an unknown scenario / flag / missing required
argument yields a message plus the usage block and a non-zero exit, never a
silent default or a panic. The build artifact is the `target/` tree (already
gitignored, along with the stray root `check` binary); no new artifact is
committed.

### Excluded from G2d (deferred)

- No Bevy / TUI / color / graphics and no interactivity or input-driven commands
  (**G9**); text-only, std formatting only.
- No multi-settlement (**G2c**) — the viewer renders one `Settlement`.
- No new lib behavior, no new economic mechanics, no balance tuning — the viewer
  only READS; distance→price is surfaced **sign only**.
- No `HashMap` in logic; no new external dependencies (pure std over the `sim`
  path dep; `econ` is a *test-only* dev-dependency for the read-only
  non-perturbation check).

---

## G2c — multiple settlements + caravans (the `Region`, `docs/impl-g2c.md`)

G2c completes the revised G2 (G2a space, G2b space-meets-economy, G2d viewer,
G2c here): **multiple settlements exist and trade.** The game-spec frames this as
"extract settlement-scoped services from the `Society` monolith." We achieve the
*end* — several independent settlement economies that trade — **by composition,
not internal surgery**, and that choice is the headline divergence recorded here.
The only engine/`sim` edits are **additive**; the six conformance goldens (the
four series M0/M1/M2/M3, the M18/M20 emergence goldens, the M5/M6 anchors) and the
whole G1/G2a/G2b/G2d suites stay green and byte-identical — the unchanged
workspace `cargo test` is the proof.

### 0. Multi-settlement by **composition**, not `Society`-monolith extraction

The roadmap's G2c is "pull market/labor/barter books out of the `Society`
monolith so multiple settlements exist." We deliberately **do not** carve up
`Society`. Instead a new `sim::Region` holds a `Vec<Settlement>`, each
`Settlement` **unchanged** from G2b — its own `World`, `Society`, colonists,
exchange, and per-econ-tick conservation receipt. Several settlement economies
therefore exist and trade with **zero** `Society` internal change.

Why composition and not extraction:

- **Golden safety by construction.** Splitting the books out of `Society` would
  touch the exact clearing/settlement code every conformance golden exercises —
  the highest-risk possible edit against the standing rule (§10.1). Composing N
  whole `Society`s touches none of it: the goldens cannot move because the engine
  is not edited. The acceptance test `econ_settlement_unchanged` plus the
  workspace gate confirm it.
- **G2b `Settlement` tests stay valid.** A plain settlement (no resident traders)
  is byte-identical to G2b — proven by `no_resident_traders_is_byte_identical_to_a_plain_settlement`
  and the unchanged G2b suite.
- **The end is reached.** The DoD is "several independent settlement economies
  that trade," and composition delivers exactly that. A genuine `Society`
  service-extraction (sharing books/markets across settlements in one process)
  buys nothing G2c needs and is left for if/when a later milestone actually
  requires cross-settlement shared infrastructure.

### 1. The caravan is a **permanent resident trader pair** (the G4 deferral)

Runtime agent-roster mutation — `AgentArena::free` + the Society position-cache
reconciliation — is the **G4-deferred** work (recorded in the G0b and G1 entries
above). So a caravan must **never** add or remove an agent from a `Society` at
runtime. Instead a caravan is a **pair of permanent resident trader agents** —
one per linked settlement, created at *generation* — and the `Region` shuttles
their **wealth** between them as route escrow, never the agents. Consequences:

- Each settlement's agent count is **constant** for the whole run (guarded by
  `trader_pairs_are_permanent_no_roster_mutation`); the caravan moves value, not
  agents.
- A trader is an `econ::Society` agent the settlement does **not** manage: it has
  no `Vocation`, no `NeedState`, is never tombstoned, and the settlement's
  per-econ-tick phases (needs, scale regeneration, task assignment) skip it. Its
  scale starts **empty** (an empty scale posts no orders, so it is inert), and the
  `Region` sets it to a buy or sell ladder when it activates the trader.
- The trader takes the **lowest** id in its settlement (so it leads the
  id-ordered market) and is given a *parked* world agent at the exchange — never
  tasked — purely to keep world and econ `AgentId`s coincident for the colonists,
  which now begin at id `num_traders`. For a plain settlement `num_traders == 0`,
  so colonists keep ids `0,1,2,…` exactly as in G2b and nothing moves.

### 2. Three additive `econ` accessors realize the caravan seam (goldens safe)

G2b added `credit_stock` (the world→econ deposit). G2c adds its peers — the
**withdraw/transfer** half of the wealth-shuttle:

- **`Society::debit_stock(AgentId, GoodId, u32) -> bool`** — the mirror of
  `credit_stock`; removes stock from a live agent, returning `false` for an
  unknown/stale/tombstoned id **or** when the agent holds less than asked (the
  atomic `Stock::remove` is the never-negative guarantee).
- **`Society::credit_gold(AgentId, Gold) -> bool`** / **`debit_gold(...)`** — the
  gold analogs, operating on the agent's `gold` balance (the legacy closed-money
  model the `sim` settlement uses — a `Designated`-GOLD M1 society with no
  `money_system`, where `total_gold` sums agent gold). Both first gate on the
  actual money regime (`uses_closed_gold_money`): they reject not only
  ledger-backed (M3) societies but also **emergent-money** regimes such as
  `MengerSaltMoney`, where `money_system` is `None` yet the circulating medium is a
  *good* held in stock — there `Agent.gold` is not money, so touching it would mint
  a phantom balance `total_gold` would wrongly count.

Each is **purely additive**: it touches no scale, quote, market, or
`money_system` state, and is **called by no engine path**, so the goldens cannot
move (the unchanged suite is the proof). Each rejects tombstoned ids (the same
frozen-holdings death contract `credit_stock` honours) and is unit-tested
(`additive_accessors_*`). They are **half-moves**: a `debit_stock` on a departing
trader is paired with a `+escrow` credit in the `Region`, so the move is
**net-zero** across the `[Σ societies ∪ escrow]` ledger — value is moved, never
minted or burned. The `Region` is the ledger that accounts the pairing.

`Settlement` gains two additive seams the `Region` drives the pair through:
`society_mut()` (documented as the caravan seam — it must touch only the
resident-trader ids; the settlement owns every colonist) and
`resident_trader_ids()`. Caravan moves run **between** econ ticks (outside
`Settlement::econ_tick`), so each settlement's own per-tick conservation receipt
is untouched.

### 3. The region-wide conservation invariant, and convergence (sign only)

`Region::econ_tick` advances each settlement's econ tick (the unchanged G2b
loop), runs the caravan step (decide / escrow-move / (de)activate traders), then
rolls up a region-wide report. The invariant it checks every tick:

```
for each good X:  Σ_settlements whole_system_total(X)  +  route_escrow_X
for gold:         Σ_settlements total_gold             +  route_escrow_gold
  change only by  (+regen per settlement, accounted) (−consumed per settlement, accounted)
  every caravan transfer is net-zero; escrow in transit is conserved and
  RETAINED if a leg never completes (the G2b escrow ethos, now inter-settlement).
```

This is checked by `region_conserves_every_econ_tick` and
`caravan_escrow_in_transit_is_conserved` (a 10 000-tick transit strands goods
mid-leg; the escrow is counted in the roll-up, at no settlement, and never
destroyed).

**Convergence is proven SIGN ONLY against a no-caravan control.** With the
caravan, the realized FOOD-price gap between the cheap settlement (A, a near node)
and the dear one (B, a far node that starts FOOD-scarce) narrows over the run; the
no-caravan control twin (`caravans_enabled = false`, the pair present but idle —
so agent counts still match) keeps the gap. The realized price in this buyers-lead
book is the consumers' marginal willingness to pay; the caravan lowers B's by
relieving its scarcity (the inverse of the G2b distance→price mechanism) and the
control leaves it. Tests `caravan_narrows_the_price_gap` and
`no_caravan_control_keeps_the_gap` are the falsification pair; no magnitude is
pinned. Determinism is inherited — integer state, the econ `Rng` consumed only at
generation, nothing drawn in the region loop or caravan step, settlement/id-ordered
iteration, `BTreeMap`/`Vec` — `region_run_is_deterministic` is the tripwire.

### Excluded from G2c (deferred)

- No `Society` internal change (we compose N societies; goldens byte-identical)
  and no `Settlement` behaviour change (G2b tests untouched; the resident-trader
  field and `society_mut` seam are additive and opt-in).
- No runtime agent roster mutation — the **G4** deferral is respected; trader
  pairs are permanent.
- No caravan **loss/risk**, no **roads-as-projects**, no **>2 settlements**, and
  no **multi-good / dynamic multi-hop** routing (all later); routes are abstract
  transit-tick counts, not intra-settlement grid movement.
- No balance tuning or asserted price magnitudes — convergence is **sign only**
  (gap-narrows-vs-control) and conservation is exact.
- No `HashMap` in logic; no new external dependencies (pure std over the existing
  path deps); nothing drawn in the loops.

---

## G3a — production chains (content recipes, seeded) (`docs/impl-g3a.md`)

G3a adds **production**: a content-defined recipe chain — grain → flour → bread —
transforms goods through seeded producer roles. It is content + `sim` wiring + a
conservation generalization, with **no new recipe logic in `econ`**. Per the
game-spec's two-step G3 gate, this is the seeded half; role *emergence* is **G3b**.

### 1. The recipe mechanism is reused, not rebuilt

`econ::Recipe` already models the whole mechanism — `{ labor, input_good:
Option<(GoodId, u32)>, required_tool: Option<GoodId>, output_good, output_qty }`.
A single-input recipe **chains naturally** (grain→flour→bread, each one input), and
`required_tool` already models a tool as a **capital gate** (a held good the
recipe needs). The recipe data lives in `sim`'s `ContentSet`, while application
reuses econ's existing `execute_direct_recipe_for_agent` path through the
additive `Society::execute_direct_recipe_for_agent_checked` seam. That wrapper
rejects unknown/tombstoned agents, preflights output headroom so `Stock::add`
cannot saturate after inputs are removed, delegates the mutation to the existing
direct-recipe executor, records labor, and returns the accounted conversion
(labor, input consumed, output produced) for `sim`'s conservation report. No market
clearing behavior changes. `econ`'s only G3a edits are additive and inert unless a
driver calls them: two `RecipeId` variants (`Mill`, `Bake`) for content recipe ids,
`Society::intern_good`, the checked direct-recipe accessor, and a `PartialEq`/`Eq`
derive on `Recipe` so the content `ContentSet` can compare equal (the determinism
check). **Tools are durable**: `required_tool` is checked by the direct recipe
executor but never removed.

### 2. Content is a code-level `ContentSet` (the loader is deferred)

`sim/src/content.rs` defines `ContentSet::grain_flour_bread()`: it interns the five
chain goods (grain, flour, bread, mill, oven) through `econ::GoodRegistry` over the
lab catalog, so they take ids **after** the seven lab goods (`grain = 7 … oven =
11`) and never collide with `GOLD`/`FOOD`/`WOOD`/`NET`, which the spatial economy
still uses. It then builds the two recipes as data (mill: grain + mill → flour;
bake: flour + oven → bread). This is the G0b `GoodRegistry` "goods as data" seam
(recorded there: *"a future `content/` layer (G3) can define them"*) realized at
the code level. A **TOML content-file loader is deferred** (game-spec G3-later);
the `ContentSet` API is the forward-compatible shape that loader will populate, and
`content/` graduates to a standalone crate then. `Society::intern_good` is an
additive naming accessor the `Settlement` calls to register the content names so
the viewer resolves them (it touches no market state; a society whose driver never
calls it keeps the lab catalog and every golden).

### 3. Seeded producer vocations + a production phase in the settlement tick

`sim` gains two vocations, `Miller` and `Baker`, **opt-in** behind a new
`SettlementConfig.chain: Option<ChainConfig>` field — `None` for every G2b/G2c
config, so each one (and the six econ goldens, and the G2d viewer goldens) is
byte-identical by construction; every chain code path is skipped. On the chain
path **bread is the staple** (`hunger ↔ bread`), grain is the gathered raw good (a
world node yields grain exactly as FOOD nodes do in G2b), and producers are
hand-placed holding their durable tool. The econ tick gains a **production phase**
*after* the market (so a producer has the input it just bought on hand): each
living producer applies its recipe up to a throughput cap via
`Society::execute_direct_recipe_for_agent_checked`, recording the transformation.
The scale phase appends two production wants to a producer's regenerated need
scale — a top-ranked tool **anchor** (so the durable tool is held, never sold) and
**input** wants (so it buys what it transforms, below food/warmth but above
savings). Both are deterministic; no RNG is drawn.

### 4. Conservation generalized across transformations

G2b's whole-system invariant was, per good, `Δ = +regen − consumed` (the transfer
net-zero). G3a generalizes it: a recipe is a **conserved conversion** — it consumes
an accounted input and produces an accounted output. The `EconTickReport` gains
`produced` / `consumed_as_input` maps (and `produced_of` / `consumed_as_input_of`),
and the invariant becomes, per good X:

```text
after(X) == before(X) + regen(X) + produced(X) − consumed_as_input(X) − consumed(X)
```

For a plain settlement both new maps are empty, so it reduces **exactly** to the
G2b form (every G2b/G2c conservation test stays green). A recipe is *not*
conservation of one good; it is a conserved transformation, with the recipe's ratio
the accounted conversion — the input and output goods each keep their own ledger,
so a yield ratio other than 1:1 is not a mass leak. **Tools are durable**: they
appear in neither production term, so a recipe needing a tool never moves the
tool's ledger (test 3 pins this). The whole-system snapshot is taken *after* the
production phase, so production is on the balanced side of the receipt.

The same generalization reaches the **region** (G2c) ledger: `RegionTickReport`
gains the matching `produced` / `consumed_as_input` maps and rolls them up from
each settlement's report, so its invariant is the identical generalized form
(Σ settlements + route escrow). A chain settlement composed into a `Region` (one
or both sides) therefore conserves region-wide instead of tripping the old
`+regen − consumed` assertion on its first transform; for a plain region the maps
stay empty and it reduces to the G2c form (`region_conserves_with_a_composed_chain_settlement`
pins the chain case).

### A note on chain throughput and the seeded roster

The CDA market clears **one unit per seller per good per econ tick**, so each
stage's flour/bread throughput is bounded by its *seller* count. A market-routed
staple chain therefore cannot fully feed an arbitrary roster from trade alone; the
seeded config is producer-heavy and gives producers input buffers and consumers a
staple buffer, so the chain operates and **runs collapse-free over the smoke
horizon** while all three goods still realize a price from real trades. This is a
seeded-content tuning choice (mechanism, not balance); G3b's price-spread emergence
is what makes the producer mix arise rather than be hand-set.

### Excluded from G3a (deferred)

- **No role emergence (G3b).** Producers are seeded (hand-placed millers/bakers);
  that an entrepreneur *chooses* to mill because the spread pays is the next slice.
- **No TOML content loader.** Content is a code `ContentSet`; the file loader and a
  standalone `content/` crate come later.
- **No multi-input buildings-as-`Project`s.** G3a uses single-input `Recipe`s;
  `capital.rs`'s `ProjectLine` (multi-input) waits for a later slice.
- **No tool production / wear / depreciation.** Tools are durable and pre-placed;
  tool economics come later.
- **No demography (G4)** and **no change to `econ` market behaviour** — every
  `econ` edit is additive (the `Mill`/`Bake` ids, the `Recipe` `PartialEq`/`Eq`
  derive, `Society::intern_good`, and the checked direct-recipe accessor);
  `ContentSet` and the opt-in `chain` field live in `sim`; the six goldens are
  byte-identical and `econ_unchanged` confirms it.
- No balance tuning or asserted magnitudes beyond the chain operating and
  conserving; no `HashMap` in logic; nothing drawn in the loops. Determinism is
  inherited (`chain_run_is_deterministic` is the tripwire).

## G3b — production roles emerge from price spreads (`docs/impl-g3b.md`)

G3a operated the grain→flour→bread chain with **seeded** producer roles. G3b removes
the seeding: a colonist **chooses** to mill or bake because the realized price spread
pays. This is the emergence half of G3, and — like the lab's money-emergence work —
it proves the **mechanism on a curated config with a falsification control**, not a
multi-seed robustness gate (that study is deferred, below).

### 1. Occupation is **ordinal entrepreneurship**, reusing M2.5 — no `econ` edit

The single load-bearing divergence from G3a is *how a vocation is acquired*. A pool
of colonists hold latent production capital (a `mill` or an `oven`) in a new
`sim::Vocation::Unassigned`, and a new **role-choice phase** in the settlement tick
(after needs and fresh scale regeneration; changed roles trigger a second pure
scale refresh so this tick's market sees active/latent production wants) re-appraises
each of them. The appraisal is **ordinal and reused**:

- `sim::recipe_adoption_pays` frames *running the recipe once* as a project bundle —
  sell the output at its realized price for a near-term receivable, costing the
  realized input price plus a per-operation operating cost — and delegates to
  `econ::bundle::appraise_project_bundle_for_money`, the **M2.5 entrepreneurial
  appraisal the lab planner already uses**. It returns `Some` iff that revenue−cost
  spread newly provisions a **future-gold (savings) want on the colonist's own value
  scale** without breaking a higher-ranked want.
- So there is **no scalar profit-maximizer** and **no argmax across colonists**: the
  decision is decided on the agent's own scale, and each unassigned colonist decides
  for itself in `AgentId` order — the §pillar-1 "colonists act" rule applied to
  occupation. A gold-sated colonist (every savings want already provisioned) declines
  even a fat spread, the ordinal tell a scalar maximizer would fail
  (`role_choice_is_ordinal_not_scalar`).
- **`econ` is reused with no edit.** `appraise_project_bundle_for_money` and the
  direct-recipe executor already existed (M2.5 / G3a); G3b only calls them from
  `sim`. The praxeology source-gate still holds — the decision reads only per-good
  **realized prices** (not an aggregate) and the agent's own scale.

A role is **sticky while the spread holds and reverts when it collapses**, because
the same appraisal runs every tick: drop the output price (or raise the input cost)
below the operating floor and the next re-appraisal returns `false`, so the colonist
reverts to `Unassigned` and stops producing (`role_reverts_when_spread_collapses`).

### 2. Latent vs active: a latent producer prices nothing on its own

A latent (`Unassigned`) producer anchors **only its tool** (a top-ranked want, so it
never sells its capital) and posts **no input bid**; only an *active* producer —
adopted G3b or seeded G3a — bids `throughput` units of its input each tick. This
latent/active split is load-bearing for the control: without it, latent producers
would bid for the intermediate good among themselves and price it with no downstream
demand, so roles would form even with the spread removed — defeating the
falsification. With it, an intermediate good prices **only** once a downstream
producer (pulled in by end demand) bids for it.

### 3. The chain prices itself bottom-up; the control removes the demand

Bread is the staple in the `emergent-chain` config, so consumer demand prices bread;
a latent baker adopts on the bread−flour spread and starts buying flour, which prices
flour, which lets a latent miller adopt on the flour−grain spread, which prices grain.
No producer is hand-placed — the producer mix *arises* from the spread
(`roles_emerge_from_the_spread`).

The one bootstrap stock is explicit: latent millers start with a small flour holding
that they do not reserve as an input, so the first baker can buy flour and create the
middle-good price signal. That is not role seeding — the holder remains
`Unassigned` until its own ordinal appraisal sees the flour−grain spread.

The **no-spread control** (`emergent-chain-control`) is the same world with the
spread removed: the grain node and latent pool stay fixed, but hunger maps to seeded
FOOD buffers instead of bread (`bread_is_staple = false`), so **bread is never
demanded**, never prices, and the *same* role-choice appraisal — over the *same*
latent pool — forms **no** roles and produces no flour or bread
(`no_spread_no_roles`). Demand is the causal difference between the twins; it is the
spread that makes the roles.

Conservation is unchanged from G3a (a recipe is still a conserved conversion); role
adoption/reversion mutates only vocations, never the physical ledger, so
`emergent_chain_conserves` holds exactly under emergent roles. The six econ goldens
stay byte-identical and every G1/G2*/G3a test is green (`econ_unchanged`). The
canonical digest grows only where future behaviour can: the `Unassigned` vocation tag
(`4`) appears only for a colonist that is actually unassigned, so every pre-chain
(G2b/G2c) digest is byte-identical; the latent-recipe bytes and the operating cost —
the role-choice-only knobs — extend the digest only for a settlement that *has* a
latent pool, so a seeded G3a chain (which has none, and runs role-choice as a no-op)
is unmoved by them, and two seeded chains differing only in operating cost still
digest equal (`seeded_chain_digest_ignores_unused_operating_cost`). The staple-mapping
triple is the one exception: because the staple steers the needs/scale phase for *any*
chain, role-choice or not, it is appended for every chain settlement, which does shift
a seeded G3a chain's absolute bytes — but no G3a test pins absolute `sim` bytes (only
same-seed determinism and conservation are compared), so all G3a tests stay green and
the tripwire is exact. Determinism is inherited: nothing is drawn in the role-choice
or production phases (`emergent_chain_run_is_deterministic` is the tripwire).

### Excluded from G3b (deferred)

- **No multi-seed robustness study.** G3b proves the mechanism + falsification
  control on a curated config; the "≥X% of N random worlds" gate (analogous to
  M18/M19 for money emergence) is deferred to a possible **G3-study** milestone. The
  acceptance suite deliberately asserts no price magnitudes and chases no robustness
  percentage — role formation is a presence/sign claim.
- **No scalar profit-maximization** (ordinal appraisal only) and **no new
  `econ` recipe/market/appraisal logic** (reused) — G3b changes only *how* a vocation
  is acquired, not the recipe execution or conservation (G3a, unchanged).
- **No demography (G4)**; **no change to `econ` market behaviour** — the role-choice
  reuses `econ`'s existing `appraise_project_bundle_for_money` with no `econ` edit,
  and the `Unassigned` vocation, the emergent configs, and the opt-in chain field
  live in `sim`.
- No `HashMap` in logic; nothing drawn in the loops; no asserted price magnitudes.

---

## G4a — real death: arena free, estate, cache reconciliation (`docs/impl-g4a.md`)

Every milestone since G0b deferred one piece: actually **removing** an agent from a
running `Society`. G0b built `AgentArena::free` but parked its Society-cache
reconciliation; G1 tombstoned the dead (froze them in place); G2c's caravans used a
permanent trader pair to avoid roster changes. G4a lands that deferred core — the
engine-integration half of demography — isolated from the demographic *mechanics*
(births, aging, households, inheritance), which are G4b.

### 1. `Society::tombstone` → `Society::remove_agent` (the tombstone seam retired)

`Society::tombstone(AgentId) -> bool` (freeze-in-place) is **replaced** by
`Society::remove_agent(AgentId) -> Option<Estate>` (real removal). It runs the
spec's order of operations, which is load-bearing:

1. **SETTLE** the estate — extract the agent's gold and econ stock into the returned
   `Estate { gold, stock }` (a conserved hand-off the caller routes to a commons in
   G4a, or to heirs in G4b), emptying its scale and zeroing labor capacity so the
   teardown posts nothing.
2. **CANCEL** its market presence — cancel every resting spot quote, barter offer,
   labor order, and loan order, releasing each reservation (the same per-book
   cancellation the G1 tombstone used, now **before** the free; freeing first would
   strand a reservation against a slot the arena no longer resolves).
3. **FREE** the arena slot — `AgentArena::free` bumps the slot generation, so the dead
   id resolves to `None` and the slot is reusable.
4. **RECONCILE** the external position/id caches — see §2.

A dead id is still recorded (in the renamed `dead_agents` list) so any capital project
or open debt it owns stays **frozen** — heirs/capital inheritance are G4b, matching
G1's freeze for that holdings class. The G1 architectural note (N per-phase guards)
still applies and is satisfied by the same list plus the arena resolving a freed id to
`None`.

### 2. Reconciling **every** external cache (the load-bearing work)

`AgentArena::free` reconciles its **own** maps; what it does not touch is state
*outside* the arena. G4a reconciles every `Society` cache that holds a position or an
agent id, on death only:

- **`agent_order: Vec<usize>`** (physical positions) — the free is order-preserving
  (every later live agent slides down one slot), so dropping the freed position and
  decrementing every entry past it rebuilds the activation order at the relocated
  positions, in unchanged priority order. Deterministic (`reconcile_agent_order_after_free`).
- **`reservations` / `labor_reservations` / `loan_reservations`** — a new
  `forget_agent` on each drops the dead id's entry (the orders were already cancelled,
  so the entries are zero; the spot table also held an empty id-keyed slot). The
  ledger-money (M3) `MoneySystem` likewise drops the freed agent's (empty) balance via
  `MoneySystem::forget_agent`, so the money invariant's "every balance has a live
  agent" check holds. A non-empty M3 balance is refused before any removal mutation;
  estate routing for such balances is G4b.
- **`barter_book`** — live barter offers and reservations for the dead id are
  explicitly forgotten in the removal path, not left for the next clearing pass.
- **`project_funding_plans`** — plans owned by the dead id are frozen for G4b:
  reserved gold is released, unstarted plans expire, and debt/project links remain
  recorded without requiring the owner to resolve as a live arena agent.
- **`labor_book` / loan book / spot books** — orders are cancelled in step 2, so they
  carry no order for the dead agent.

A missed cache would be a dangling reference / stale order; the reconciliation is
deterministic (id-ordered, integer, draws nothing), so a run with deaths is
byte-identical across invocations.

### 3. Estate → the settlement **commons** (a conserved sink; heirs are G4b)

The `sim` `Settlement` (and the `life` `Camp`) own a **commons** — `commons_gold` plus
a per-good `commons_stock`. On a starvation death the driver routes `remove_agent`'s
returned `Estate` (gold + econ stock) into the commons, and **drains the dead
colonist's world-carried delivery escrow** out of the world into the commons via the
new `World::withdraw_agent_carry` (where G1 left it frozen in place). It likewise
drains any **stranded exchange-deposit escrow** — units the colonist delivered to the
exchange stockpile whose econ credit was still pending at death — out of the exchange
into the commons, dropping the pending attribution so no entry keyed by the freed id
lingers (a conserved world → commons transfer that preserves the pending↔exchange
invariant; empty in the starvation-only model, where the transfer credits a still-live
depositor before it can die, so this is a defensive settle). The commons joins
`total_gold` and `whole_system_total`, so whole-system conservation holds **across** a
death: nothing is created or destroyed, only relocated society/world → commons. G4b
will route the same estate to heirs/households instead of pooling it.

### 4. Goldens safe **by construction** (the no-free path is byte-identical)

The lab never frees an agent — every golden scenario runs a fixed roster — so the
free + reconcile path is **game-only**. The no-death code path is structurally
unchanged: the commons starts empty and an empty commons is omitted from the canonical
digest (it joins the digest only once a death settles an estate into it, so two
distinct post-death states cannot collide), the new `forget_agent`/`remove_agent` are
invoked only on death, and the `dead_agents`
list stays empty in the lab (every freeze guard's binary-search is a no-op). The six
econ conformance goldens and the existing G1/G2*/G3* digest tests are byte-identical;
the G4a acceptance suite's `no_death_path_is_byte_identical` pins a no-death digest as
the forward tripwire.

### Excluded from G4a (deferred)

- **No births, aging, households, or culture inheritance (G4b)** — death only.
- **No estate-to-heirs** — the estate settles to the commons; G4b routes it to
  households.
- **No non-empty M3 ledger estate routing** — G4a frees only empty-ledger agents (the
  closed-GOLD M1 drivers keep no `MoneySystem`; the lab never frees). A non-empty M3
  estate is G4b.
- **No population-stability study** (G4b/later).
- No `HashMap` in logic; deterministic reconciliation; nothing drawn in the loops.

---

## G4b — births, aging, households, culture inheritance (`docs/impl-g4b.md`)

G4a gave the engine real death (runtime removal + estate + cache reconciliation). G4b
completes demography: colonists **age**, **die of old age** (reusing G4a's removal),
are **born** into **households** when the household can support them, and children
**inherit** their parents' `CultureParams` with bounded mutation — so time preference
drifts under selection across generations. This is the first milestone where the
population is not a fixed cast.

### 1. `Society::add_agent` — the insert-side mirror of `remove_agent`

The G0b divergence log parked one half of demography's engine integration: a
`Society`-level **insert** that reconciles the external caches. G4a did the removal half;
G4b adds the insert half. `Society::add_agent(Agent) -> AgentId`:

1. **INSERT** into the arena (`AgentArena::insert`) — appends the agent at the end of the
   dense live slice and assigns its id (a reused numeric index carries the bumped
   generation the free recorded, so a stale ancestor id stays `None`). No existing agent
   relocates, so no other cache's positions shift.
2. **RECONCILE `agent_order`** — append the newborn's (last) position, so the per-tick
   activation loop iterates it (`reconcile_agent_order_after_insert`). Without this the
   newborn is never activated and posts no orders.
3. **EXTEND the reservation caches** — materialize the new id's empty spot-reservation
   slot (`Reservations::ensure_agent_slot`, the mirror of removal's `forget_agent`). The
   id-keyed labor/loan reservation tables hold only nonzero entries by invariant, and a
   newborn reserves nothing, so they need no eager slot — the lazy `reserve_order` adds
   one on the first order.

It is the exact mirror of `remove_agent`: a missed cache would be a birth that can't
trade. Determinism is preserved — the caller supplies a fully formed `Agent`, and any
culture mutation or birth decision is made deterministically upstream, never by an `Rng`
in this path. `add_agent` moves no gold or goods of its own; it installs the agent the
caller already endowed (a conserved transfer), so it mints nothing.

**Goldens safe by construction.** No lab scenario adds an agent at runtime, so the
insert + reconcile path is game-only; the no-add hot path is structurally unchanged. The
six econ conformance goldens and every G1/G2*/G3*/G4a digest test stay byte-identical
(econ `add_agent` unit tests pin the reconciliation directly; the round-trip
`add_agent` → `remove_agent` restores `agent_order` and the live count exactly).

### 2. Deterministic mutation without an `Rng` in the loop

Births happen **mid-run**, but the loop draws no `Rng` — the load-bearing determinism
rule. Everything that could be random is a pure function of a stable seed:

- **culture inheritance** — `CultureParams::inherit(birth_seq, max_delta)` nudges each
  field by a bounded delta derived from a SplitMix64 hash of `(field, birth_seq, salt)`;
  same `(parent, birth_seq, max_delta)` → the same child.
- **old-age lifespan + founder starting age** — `onset + hash(seed) % span` years and a
  staggered start age from the same seed, so colonists age into old age at different
  ticks (no synchronized die-off) with no per-tick draw.
- **per-colonist seeds** — founders hash `(world_seed, founder_index)`, children hash
  `(parent_seed, birth_seq)`; generation consumes the `Rng` only for culture (as G1–G3
  already do), so demography adds **no** `Rng` draw at all.

The colony's monotonic `birth_seq` counter gives each birth a unique, stable sequence
number; reused arena slots get fresh children, so a sequence number is never reissued.
A run with births and deaths is byte-identical across invocations (tick-by-tick digest
lockstep is the tripwire).

### 3. Households, the conserved provision, and estate-to-heirs

The `sim` `Settlement` gains an opt-in `demography: Option<DemographyConfig>` overlay
(`None` for every pre-G4b config, so the canonical layout and every golden are
byte-identical; the demography fields and runtime are omitted from the digest entirely
when absent). When present, the canonical bytes include both future-steering demography
config (household provisions/endowments, birth cadence, lifespan/mutation knobs, caps) and
runtime counters, so two states that would diverge on the next birth/provision/death tick
do not digest equal. The overlay seeds **households** of **non-spatial** householders —
they have an econ agent but **no world agent**, so the fast loop, the deposit transfer,
and the world↔econ id coincidence the gatherers rely on are untouched, and a dead
householder's world-escrow drain is a no-op.

- **Provision (a conserved source).** Each living member is fed a renewable FOOD/WOOD
  provision (the household hearth) delivered straight into econ stock, recorded in a new
  `EconTickReport.endowment` term. The credited amount is clamped to the recipient's stock
  headroom and only the actually credited units are reported, so saturated public configs
  cannot claim source units that never entered stock. Conservation generalizes to
  `after = before + regen + endowment + produced − consumed_as_input − consumed`;
  `endowment` is empty without a demography overlay, so the identity reduces to the
  G2b/G3a form and every existing conservation test is unchanged. `Region::econ_tick`
  rolls the per-settlement `endowment` into the region-wide receipt the same way it
  already rolls `regen`/`produced`, so a `Region` composed from demography settlements
  conserves too (empty, and so inert, for a plain region). The provision keeps members
  fed, so deaths are **old age**, not starvation.
- **Births/deaths move goods *within* the whole system.** A birth's endowment is a
  transfer debited from a parent's **unreserved** balances (FOOD required free after spot/
  barter reservations; gold best-effort, clamped after spot/loan/labor/project
  reservations — so a gold-poor lineage still reproduces without overcommitting live
  quotes). A death's estate routes to a living household **heir** (the first survivor in
  colonist-insertion order), or falls back to the **commons** if the lineage is extinct
  (G4a's sink). Both are conserved transfers within `[society ∪ commons]`, so they need no
  conservation term and whole-system totals hold across every birth and death (the closed
  gold balance is invariant).
- **Long-run cleanup.** Dead colonists remain in the generation-indexed public roster for
  inspection, but hot tick phases iterate a compact live-slot roster and resolve
  consumption/labor logs through a stable `AgentId -> slot` map. Spatial deaths drain carry
  and then remove the world agent, so `World::tick` scales with live spatial agents, not
  historical deaths. No `HashMap` is used.
- **Phase order.** The econ tick gains, behind the overlay: aging + old-age death (after
  the needs/starvation phase, reusing `remove_agent`), the provision (after scale
  regeneration, before the market — mirroring `life::Camp`'s harvest), and births (after
  the market, so a newborn first trades the *next* tick, and before the conservation
  snapshot, so its transferred-in holdings balance the parent's debit).

### 4. The `lineages` curated demonstration

`SettlementConfig::lineages` seeds two households — a **patient** one (low time
preference → a high saving target) and a **present-biased** one (high time preference →
a small target) — identical but for time preference and a wood provision: the patient
household gets a wood surplus it sells, the present-biased one buys wood for (non-lethal)
warmth. Gold flows from the spenders to the savers, so the patient lineage
**out-accumulates** the other (`patient_lineage_outaccumulates_impatient`, sign only).
Both are food-secure, so the population **sustains in a band** — births ≈ old-age deaths,
capped at `households × max_household_size`, neither extinct nor blowing up
(`population_sustains_without_collapse`). Patience does its selection work through
`regenerate_scale` (G1), **not** a scalar fitness function; reproduction is a threshold
rule. The `oikos run lineages` dashboard surfaces population, cumulative births/old-age
deaths, and per-lineage wealth tick over tick.

### Excluded from G4b (deferred)

- **No multi-seed stability or selection STUDY.** G4b proves the mechanism + curated
  demonstrations on `lineages`; the game-spec's **100-seed stability band** and a
  **multi-seed selection study** (analogous to M18/M19 for money emergence) are deferred
  to a possible **G4-study** milestone. The acceptance suite asserts no tuned population
  number and no statistical selection gate — the stability band and the selection result
  are sign/smoke claims on a curated config.
- **No inter-settlement migration** (later) — a household does not move between
  settlements.
- **No scalar reproduction optimization** — a threshold rule plus the heritable ordinal
  patience bias, never a fitness function.
- **No change to econ MARKET behaviour** — `add_agent` is additive and game-only, the six
  goldens are byte-identical, and births/deaths are `sim`/`life`-only. **No non-empty M3
  ledger estate routing** (the closed-GOLD M1 drivers keep no `MoneySystem`).
- No `HashMap` in logic; nothing drawn in the loops (deterministic mutation); no asserted
  magnitudes beyond the sign claims.

## G5a — money emerges from spatial barter (`docs/impl-g5a.md`)

Every settlement before G5a ran on econ's **designated GOLD** market (`step_v2`'s
designated branch, `MarketMoneyConfig::Designated(GOLD)`) — money was assumed. G5a makes
money **emerge**: a curated **barter camp** starts with no designated money, colonists
barter goods-for-goods at the exchange, and a money good is **promoted** by the Mengerian
saleability rule the lab proved (M5/M6) and studied (M18/M19/M20) — driven by **spatial**
trade. It is the spatial counterpart of the lab's money-emergence result, sliced to the
**mechanism + a falsification control**. G5a is **spatial wiring + a curated config + a
control**; it adds **no emergence rule to econ**.

### 1. Reuse, not reimplement — the emergent path is the lab's V2 machinery

The barter camp drives econ's existing V2 emergence machinery unchanged: `BarterBook`
(barter.rs), `SaleabilityTracker` + `MengerianEmergence::winner` (menger.rs), the
`MengerianConfig` envelope, and `MarketMoneyConfig::Emergent`. The `sim` `Settlement`
selects the regime by config: a `barter` overlay maps to `ScenarioName::MengerSaltMoney`
+ `MarketMoneyConfig::Emergent` (the V2 path), absence maps to the pre-G5a
designated-GOLD M1 path. `step_v2` clears the barter book, feeds the tracker from realized
spatial barter, and fires `winner` to promote — exactly as the lab's non-spatial Society
does. The promotion **decision** never leaves econ; the sim only wires spatial barter into
the tracker and reads the result. Acceptance test `emergence_reuses_the_lab_rule` replays
`tracker().winner(config)` on the pre-promotion tally and reproduces the promoted good,
proving the choice is the lab rule's, not a sim re-implementation.

### 2. The only econ edits are additive (goldens byte-identical)

Three econ surfaces are added, all safe for the six conformance goldens (M0/M1/M2/M3, the
M18/M20 emergence goldens, the M5/M6 anchors):

- **Read-only accessors.** `MengerianEmergence::config()`, `stable_winner()`,
  `stable_winner_ticks()`; `SaleabilityTracker::candidate_saleability()`;
  `Society::emergence()`, `money_promoted_at_tick()`, `saleability_provisional_leader()`.
  They add no rule, only a read surface the spatial wiring, the digest, and the viewer
  consume.
- **A consumption-log capture in `step_v2`.** The V2 direct passes (both the money phase
  and the barter phase) now record the per-tick consumption log, and `step_v2` clears it at
  tick start — but **only when `consumption_log_enabled`**, which is opt-in and which the
  lab goldens never set. The `enable_consumption_log` debug-assert was widened from "M1
  only" to "M1 and V2"; M2/M3 stay inert. With the log disabled the path is byte-for-byte
  the old `step_v2`, so M18/M19/M20 and every V2 record stay identical. The sim enables the
  log (as the G1 `Camp` does for M1) to read the eaten sink for its conservation receipt.
- **An opt-in V2 promotion rejection boundary.** `Society::step()` still runs the lab's V2
  path with no rejection list. The spatial sim calls the additive boundary with its
  gathered node goods so econ's `winner` can identify the candidate, but a world-regenerated
  good is recorded as `V2PromotionFailureReason::UnsupportedMoneyGood` and the emergence
  latch stays in barter. This is not a new emergence rule: it is the spatial substrate
  declining a promotion it cannot conserve.

`econ_unchanged` (and the dedicated M0–M21 golden files) pin byte-identity; a plain G2b
settlement is digest-identical with or without an explicitly-`None` barter overlay.

### 3. The spatial→saleability wiring and the medium demand

Pre-promotion, the camp must generate a **saleability differential** from spatial barter
or nothing emerges. The wiring:

- **Two gathered goods, specialist sellers.** Gatherers split round-robin over a FOOD node
  and a WOOD node (four each); tight survival buffers force each specialist to **trade** for
  the good it does not haul — the gains-from-trade that thicken the barter book.
- **A durable medium demanded on every scale.** Colonists demand a non-gathered **SALT**
  medium via a `Horizon::Next` "hold the medium" want inserted just below the present
  survival block (`medium_scale_extension`) — the **same value-scale slot** the G3a/G3b
  chain uses for producer inputs, **not** a need-model change. SALT's universal demand,
  traded against both FOOD and WOOD, makes it the good accepted against the most
  counterparts — the most saleable — so it is the good that emerges. The savings good
  (`known.savings`) is SALT too, so post-promotion the money market provisions those
  store-of-value wants in the emerged money exactly like a designated-money camp.
- **The medium is never a gathered node good.** A money good the world re-mints each tick
  would break the conserved promotion, so `generate` asserts no node harvests the medium.
  The tracker may still observe gathered FOOD/WOOD as candidates for the control proof; if
  a custom envelope or seed makes one of those gathered goods the winner, the sim rejects
  that unsupported promotion through econ's V2 failure path and remains in barter. The
  scale extension runs only while still in barter (`current_money_good().is_none()`); once a
  money good emerges the scale is pure need-driven and the G2b money market clears.

### 4. The conserved promotion across the phase transition

A barter swap is a conserved **relocation** (goods change hands, nothing minted). The
**promotion** is the only phase event that crosses the good↔money seam: econ converts the
winning good's stock to gold 1-for-1 (the lab's conserved promotion). The sim detects the
`None → Some(good)` transition around `society.step()`, computes the minted gold as the
society's gold delta, and records it in a new `EconTickReport.promoted` term. Whole-system
conservation generalizes to
`after = before + regen + endowment + produced − consumed_as_input − consumed − promoted`;
`promoted` is empty on every non-promotion tick and every non-emergent settlement, so the
identity reduces to the G4b form and every existing conservation test is unchanged. The
good→money side is a sink for the physical good, matched 1-for-1 by the gold the promotion
mints (the gold checkpoints account the minted gold). `barter_and_promotion_conserve` is
the tripwire: per-good ledgers balance every tick, total gold is constant except at the
single promotion tick where it rises by exactly the converted stock, and the promoted
good's stock is then zero.

### 5. Determinism across barter → promotion → money, and the digest

The whole run is integer; the `Rng` is consumed only at generation (cultures), nothing is
drawn in the loops; state is `BTreeMap`/`Vec`, no `HashMap`. When the barter overlay is
present the canonical bytes include the savings good, the current money good (option), the
promotion tick (option), and the **full Mengerian emergence runtime**: the saleability
tracker's accumulated per-candidate acceptances and **distinct** acceptor-agent and
counterpart-good sets, plus the promotion-timing latch (the stable winner and its
consecutive-tick count). Every one of those steers the **future** promotion decision — two
barter states agreeing on holdings and the current leader but differing in a stability
counter or an acceptor set promote on different future ticks — so they belong in the
"byte-identical iff future behaviour identical" identity (the provisional leader the
earlier draft captured is merely a derived projection of this state, and the member lists,
not just their counts, are serialized because a later acceptance only advances the
eligibility counts when its acceptor/counterpart is new). The no-overlay path omits all of
it and stays byte-identical to pre-G5a runs (`barter_camp_run_is_deterministic` and
`canonical_bytes_include_emergence_runtime` are the tripwires; the no-overlay identity is in
`econ_unchanged`).

### 6. The `barter-camp` mechanism + the `barter-camp-control` falsification twin

`SettlementConfig::barter_camp` monetizes: SALT leads the saleability tally and is
promoted from realized spatial barter, after which trade is SALT-money-priced. The control
`barter_camp_control` is the **same** camp — same nodes, roster, cultures, and reused M20
envelope — with the SALT medium's **supply removed** (no colonist endowed with SALT, and
so no "hold the medium" demand it could support). The same emergence machinery runs over
the same FOOD/WOOD barter every tick, but the only swaps that clear are perfectly
reciprocal FOOD-for-WOOD (each counts one FOOD and one WOOD acceptance), so no good ever
leads by the promotion margin and **nothing monetizes** — the settlement stays in barter
the whole horizon. The pair isolates the cause: identical machinery and raw-input supply,
the saleable medium's presence the only difference. The control is non-vacuous — its
tracker observes real FOOD and WOOD barter (nonzero acceptance shares) — it simply never
produces a winner. If both monetized, the wiring would be reading something other than
realized spatial barter.

### Excluded from G5a (deferred)

- **No emergence composed with production or demography (G5b).** The G5a config is a plain
  gatherer/consumer barter camp; `generate` asserts a barter overlay is mutually exclusive
  with a production `chain` and a `demography` overlay. Composing emergence with the full
  stack (production, demography, multi-settlement) is **G5b**.
- **No multi-seed spatial robustness STUDY.** G5a proves the mechanism + falsification
  control on a curated config; the spatial robustness gate (emergence rate under
  encounter/transport frictions, analogous to **M18/M19** for the lab's non-spatial money
  emergence) is a deferred **G5-study**. The acceptance suite asserts only
  promotion-happens / control-doesn't (sign) and exact conservation — no emergence rate,
  no tuned tick, no asserted magnitude beyond the conservation identity.
- **No multi-settlement emergence** (later) — emergence runs in a single `Settlement`, not
  across a `Region`.
- **No change to econ's emergence RULE or `MengerianConfig` defaults** — the envelope is
  the adopted M20 default reused unchanged (only `candidate_goods` names the camp's
  tradeable set), and the decision routes through `MengerianEmergence::winner`.
- **No change to econ MARKET/emergence behaviour** — the six goldens are byte-identical,
  every econ edit is additive (read accessors + the opt-in, default-off consumption log +
  the opt-in spatial promotion rejection boundary).
- No `HashMap` in logic; nothing drawn in the loops; no money moves in the fast loop.
