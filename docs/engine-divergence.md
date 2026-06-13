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
