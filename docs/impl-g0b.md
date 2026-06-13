# Implementation Spec G0b: Engine Migrations Behind Compatibility

## Purpose

G0a forked the lab engine verbatim; G0b is the first deliberate divergence.
Three migrations from `game-spec.md` §11, each priced honestly and each
gated by the same proof: **the conformance suite stays green and the lab
goldens stay byte-identical** — natively where possible, through a thin
compat layer where not.

1. **Dynamic `GoodRegistry`** — goods become data, not constants, so
   `content/` (G3) can define them.
2. **Generational `AgentId`** — identity survives a changing cast, so
   demography (G4) can kill and birth colonists without dangling
   references.
3. **`Command` result/error semantics** — player input cannot silently
   no-op (game-spec §7), unlike authored scenario events which may.

The game-spec's original G0 demanded byte-identical goldens AND the
migrations that break them — the codex review called out the
contradiction, and the resolution is this milestone's design rule:
**every migration must either preserve the lab's observable surface
natively, or provide an explicit lab-compat construction that does.**
Nothing about the lab's economics changes; this is plumbing.

## Verified Base Facts (2026-06-12, oikos @ `05928f4`, 628 tests green)

1. **Goods are hard-coded constants.** `good.rs`: `GoodId(pub u16)` with
   static constants (GOLD, FOOD, WOOD, NET, SALT, CLOTH, ORE),
   `good_name() -> &'static str`, `Stock::new(n)` allocating `n + 1`
   slots sized by max good id; `worldgen::GOOD_POOL` is a static slice;
   price-belief vectors are sized from the same constants. The codex
   spec review flagged exactly this: `GoodRegistry` is new architecture.
2. **`AgentId` is a bare `u32` tuple struct** (`agent.rs:8`,
   `AgentId(pub u32)`), embedded in ledgers, tapes, debts, banks,
   metrics, barter, factor, and market records — and constructed as a
   LITERAL (`AgentId(212)`) in thousands of test and scenario sites.
   Agent ids in lab casts are sparse (1–10, 100–124, 200–215, 300–315,
   400) and are NOT storage indices: `Society` keeps `agents: Vec<Agent>`
   in cast order plus `agent_order` (indices sorted by id) and resolves
   ids by search. Nothing ever frees an agent today.
3. **Tapes and CSV output print ids and good names as plain text** —
   any change to `Display`/formatting of either type moves every golden.
4. **Event application returns nothing.** `Society::apply_event_kind`
   mutates and silently tolerates missing targets (unknown debt id in
   `SetDebtDueTick`, no-issuer `LevyTax` — both unit-tested as silent
   no-ops). This is correct for authored scenarios and wrong for player
   commands (game-spec §7's contract: `Applied | Rejected(reason)`).
5. **The four series goldens (M0/M1/M2/M3), the M18/M20 emergence
   goldens, and the M5/M6 anchors** all pass in the fork (G0a DoD) and
   are the regression tripwire for everything below.

## Design rule: native compatibility first

The migrations are designed so the goldens hold NATIVELY, not through a
translation layer:

- **`AgentId` widens, it does not restructure.** `AgentId(pub u32)` →
  `AgentId(pub u64)` with packed semantics: low 32 bits = the numeric id
  exactly as today, high 32 bits = generation. Every lab literal
  `AgentId(212)` still compiles and means index 212, generation 0; `Ord`,
  `Eq`, `Copy` survive; and `Display`-style formatting of a generation-0
  id prints the same digits as today, so every tape and golden is
  byte-identical with ZERO call-site changes. Accessors
  (`fn index(self) -> u32`, `fn generation(self) -> u32`,
  `fn with_generation(index: u32, generation: u32) -> AgentId`) carry the
  new semantics; formatting of nonzero generations (`"212#1"` or
  similar) is a new, never-before-printed surface so it cannot move a
  golden. If any site turns out to depend on the exact integer WIDTH
  (serialization, hashing into goldens), it is fixed to be
  width-agnostic — behavior-preserving by the golden gate.
- **The registry's lab construction IS the constants.**
  `GoodRegistry::lab_default()` interns the exact lab set in the exact
  id order so `GoodId` values, `Stock` sizing, belief-slot sizing, and
  names are bit-for-bit today's. The static constants remain exported
  (now defined in terms of the lab-default ordering) so no test changes.
- **Commands are a new entry point, not a change to the old one.** The
  scenario event path keeps its silent-tolerance semantics untouched;
  `apply_command` is additive.

## Milestone Boundary

G0b includes:

- `econ/src/registry.rs`: `GoodRegistry` (intern by name → `GoodId`,
  name lookup, `len()`, `lab_default()`); `Society`/worldgen/report
  surfaces that currently consult constants or `good_name()` consult the
  registry instance instead (held by `Society`, constructed lab-default
  everywhere the lab constructs a society);
- `AgentId` u64 widening with packed generation + accessors;
- `econ/src/arena.rs`: `AgentArena` — stable-identity agent storage with
  slot reuse and generation bumping on free, iteration in id order;
  `Society` storage swapped to it with the invariant that a never-freeing
  population behaves exactly like today's `Vec` (the lab case);
- `Command` plumbing in `econ`: `apply_command(&mut self, EventKind) ->
  CommandResult` where
  `CommandResult = Applied | Rejected(CommandRejection)` and every
  currently-silent no-op case returns a named rejection reason; the
  scenario path is untouched;
- conformance: the full suite green NATIVELY; all goldens byte-identical;
- new unit tests for the three migrations (below);
- README status update; a short `docs/engine-divergence.md` log opened to
  record every deliberate post-fork divergence from the lab (this
  milestone writes its first three entries).

G0b excludes:

- no content files, no good ATTRIBUTES beyond name (G3 brings
  durability/perish/etc. with `content/`);
- no actual agent death/birth (G4) — the arena exists and is tested at
  the unit level, but no engine path frees an agent yet;
- no game `Command` enum (DesignateZone etc. are sim-crate work, G2+) —
  only the result-semantics plumbing over the existing `EventKind`;
- no tape retention, no provisioning cache (deferred per README);
- no change to any economic rule, scenario, or output format;
- no lab back-port (the lab keeps its own shapes; the divergence log is
  the bridge documentation).

## Implementation notes per migration

### GoodRegistry

```rust
pub struct GoodRegistry { names: Vec<String> /* index = GoodId.0 */ }
impl GoodRegistry {
    pub fn lab_default() -> Self;               // exact lab set & order
    pub fn intern(&mut self, name: &str) -> GoodId;  // existing name = same id
    pub fn name(&self, good: GoodId) -> &str;
    pub fn len(&self) -> usize;                 // Stock/belief sizing source
}
```

- `Society` gains a `registry: GoodRegistry` field; every construction
  site uses `lab_default()` (one constructor change, not N call sites —
  verify with the conformance suite).
- `good_name(GoodId) -> &'static str` survives as a thin shim over the
  lab-default table (it is called from display paths the goldens cover);
  registry-aware callers migrate to `registry.name()`. Mark the shim
  `#[doc(hidden)]`-style discouraged, do not remove it in G0b.
- `Stock::new` / belief-slot sizing take the slot count from the
  registry where a registry is in scope; the lab-default count equals
  today's constant-derived count (assert this in a unit test).

### AgentArena

```rust
pub struct AgentArena { /* id-ordered storage; free list; generations */ }
impl AgentArena {
    pub fn insert(&mut self, agent: Agent) -> AgentId;   // fresh or reused slot
    pub fn insert_with_id(&mut self, agent: Agent);      // lab casts: authored ids, gen 0
    pub fn free(&mut self, id: AgentId) -> Option<Agent>; // bumps slot generation
    pub fn get(&self, id: AgentId) -> Option<&Agent>;     // None on stale generation
    pub fn iter(&self) -> impl Iterator<Item = &Agent>;   // id order, live only
}
```

- The lab path uses `insert_with_id` (authored sparse ids, generation 0,
  never freed) — iteration order and lookup results must equal today's
  `Vec` + `agent_order` exactly; the goldens are the proof.
- Internal structure is the implementer's choice (`BTreeMap<u32, Slot>`
  is the obvious deterministic candidate); no `HashMap` (engine rule).
- `Society`'s `agents: Vec<Agent>` and `agent_order` migrate to the
  arena. This is the wide mechanical part: every `agents[index]` /
  `iter().position(...)` site moves to arena lookups. Keep each change
  mechanical; the conformance suite is the checklist.

### Command results

```rust
pub enum CommandResult { Applied, Rejected(CommandRejection) }
pub struct CommandRejection { pub reason: RejectReason /* enum */, pub detail: String }
```

- `apply_command` wraps the same mutation logic the event path uses —
  factored so the checks are shared, not duplicated: the event path
  discards the result (preserving silent semantics), the command path
  returns it.
- Every silent no-op today gets a named reason: `UnknownDebt`,
  `NoIssuer`, `UnknownAgent`, `NotApplicableToKernel` (M0), etc.
- This is plumbing for the sim crate's future command queue; nothing in
  `econ` calls it yet besides tests.

## Acceptance Tests

`econ/tests/g0b_engine_migrations.rs` (plus unit tests in the new
modules):

1. `goldens_hold_natively` — implicitly: the existing golden tests pass
   unmodified; this test additionally asserts the lab-default registry
   matches the legacy constants (names, ids, counts) one by one.
2. `agent_id_packing_is_compatible` — `AgentId(212)` has index 212,
   generation 0; ordering of generation-0 ids equals u32 ordering;
   formatting of generation-0 ids is digit-identical to the old type;
   nonzero generations order AFTER their generation-0 ancestor and
   format distinguishably.
3. `arena_matches_vec_semantics_when_nothing_dies` — property test:
   random authored-id casts inserted via `insert_with_id`; iteration
   order, lookups, and count equal the legacy Vec+order construction.
4. `arena_reuse_bumps_generation` — insert/free/insert reuses the slot
   with a bumped generation; the stale id resolves to `None`; the new id
   resolves to the new agent; iteration never yields freed agents.
5. `registry_intern_is_stable` — interning existing names returns the
   same id; new names extend; `len` drives `Stock` sizing equal to the
   legacy constant-derived size for `lab_default`.
6. `commands_reject_loudly_where_events_are_silent` — for each known
   silent no-op (unknown debt, no-issuer levy, M0-inapplicable event):
   the EVENT path still silently tolerates (existing tests untouched),
   the COMMAND path returns `Rejected` with the right reason; applied
   commands mutate identically to the event path.
7. Full conformance suite green; all goldens byte-identical; `cargo
   clippy --workspace --all-targets -- -D warnings`; `cargo fmt --check`.

## Handoff Notes

- The golden gate is the definition of "behavior-preserving" — if a
  golden moves, the migration leaked; fix the migration, never re-pin a
  golden in this milestone.
- `AgentId` widening must not change any printed digit for generation-0
  ids — check every `format!`/CSV path the compiler does not.
- No `HashMap`, pure std, integer math in sim logic — the lab's engine
  rules apply to `econ` unchanged.
- The event path's silent semantics are LOAD-BEARING for authored
  scenarios; share the logic, do not change the behavior.
- Do not remove the legacy constants or `good_name` in G0b; mark them as
  lab-compat surface in the divergence log.
- Record each divergence (id width, registry, command plumbing) in
  `docs/engine-divergence.md` with the lab commit the fork left from.
- `git add` new files so the diff-scoped reviewer sees them.

## Scope clarifications (spec-owner, added during review 2026-06-13)

These dispose recurring review threads; they are AUTHORITATIVE for this
milestone's definition of done:

1. **Renderer/report registry-awareness is G3 scope.** Until `content/`
   exists, no code path can construct a non-lab good, so `report.rs`
   consulting the lab-compat `good_name` shim is correct-by-construction
   today. Migrating renderers to registry-aware naming lands with the
   first dynamic content (G3) or the G2 inspectors, whichever comes
   first — recorded in `engine-divergence.md`, not done here.
2. **`AgentArena::free` performance and Society-cache reconciliation are
   G4 scope.** G0b's contract for `free` is CORRECTNESS only (stale ids
   resolve `None`, no revival, iteration excludes freed agents), proven
   at unit level; no engine path frees. The O(N) cost and the cache
   reconciliation design are documented G4 prerequisites in
   `engine-divergence.md`. Re-raising them does not block G0b.
3. **Generation saturation**: `free` refusing (or saturating safely)
   when a slot's generation cannot advance is in scope as a one-line
   correctness guard — accepted.
4. Remaining review effort should target: golden integrity, event-path
   behavior preservation, command-rejection atomicity, and arena/registry
   CORRECTNESS — not future-milestone performance or rendering work.
