# Implementation Spec G4a: real death — arena free, estate, cache reconciliation

## Purpose

Every milestone since G0b has deferred ONE piece: actually removing an agent
from a running `Society`. G1 tombstoned the dead (froze them in place); G2c's
caravans used permanent trader pairs to avoid roster changes; the G0b
divergence log explicitly parked "`AgentArena::free` + Society-cache
reconciliation" at G4. G4a is where that lands — the engine-integration core
of demography, isolated from the demographic *mechanics* (births, aging,
households, inheritance) which follow in G4b.

G4a replaces the G1 tombstone with **real removal**: when a colonist dies, its
**estate settles**, its **arena slot is freed**, and every Society cache that
referenced it **reconciles** correctly — with whole-system conservation
preserved and the engine continuing correctly afterward.

It is NOT births/aging/households/culture-inheritance (G4b — G4a settles
estates to the settlement **commons**, not heirs), NOT a change to `econ`
*market* behavior (the six goldens stay byte-identical — the lab never frees
an agent, so the free path is game-only by construction), and NOT a
population-stability study (G4b/later).

## Verified Base Facts (2026-06-14, oikos @ `f091674`, 830 tests green)

1. **`AgentArena` reconciles its OWN maps on free** (arena.rs, G0b): `free`
   bumps the slot generation and updates `index_of`/`live_order` (swap_remove
   internally); stale ids resolve `None`. What it does NOT touch is state
   OUTSIDE the arena.
2. **Society's external, position-based caches are the reconciliation work**
   (society.rs): `agent_order: Vec<usize>` (physical positions!),
   `reservations`, `loan_reservations`, `labor_reservations`, `labor_book`.
   The G0b divergence log flagged exactly this: `free`'s swap_remove
   relocates agents, invalidating these external indices. G4a reconciles
   them on death.
3. **The lab never frees an agent** — every golden scenario runs a fixed
   roster with no births/deaths. So the free + reconcile path is game-only;
   if the NO-free hot path stays structurally unchanged, the six econ goldens
   are byte-identical by construction (they never invoke the new path).
4. **G1 already has the death trigger + estate freezing** — `Society::tombstone`
   and the sim's starvation-death detection (settlement.rs). G4a swaps the
   tombstone's freeze for real removal: settle estate → free → reconcile.
5. **Conservation is established** (G2b/G3a): the sim's whole-system roll-up.
   Estate settlement must be a conserved transfer (dead agent's gold + stock
   → commons), not a vanish.
6. **Determinism + no-HashMap** inherited; `BTreeMap`/`Vec`; `Rng` at
   generation only; reconciliation is deterministic (id-ordered rebuild).

## The reconciliation contract (the load-bearing work)

When colonist `id` dies at an econ tick:

```
1. SETTLE estate: move id's econ gold + stock to the settlement commons
   (a conserved transfer — a commons pool the sim owns; G4b routes to heirs
   instead). World-carried (escrow) goods of id likewise settle to commons.
2. CANCEL id's market presence: release every reservation id holds
   (reservations / loan_reservations / labor_reservations) and remove its
   resting orders from every book (spot order books, labor_book, loan book) —
   reusing the G1 tombstone's per-book cancellation, now BEFORE the free.
3. FREE the arena slot: AgentArena::free(id) (bumps generation; id now
   resolves None).
4. RECONCILE the external caches: rebuild agent_order from the arena's live
   agents in the SAME priority order; rebuild/remap any position-indexed
   reservation state so no entry points at a relocated or freed slot.
5. ASSERT: no cache references a freed/relocated position; whole-system
   conservation holds (commons gained exactly what the estate held); the
   run continues.
```

Goldens-safe-by-construction rule: the no-death code path (every lab
scenario) must be byte-identical to today. The reconciliation runs ONLY when
a death occurs; the cache STRUCTURES and their no-death behavior are
unchanged. (If a structure must change to support reconciliation, the change
must be provably inert when no free has happened — the goldens are the gate.)

## Milestone Boundary

G4a includes:

- a `Society` removal operation: settle-estate hook + `AgentArena::free` +
  full external-cache reconciliation (agent_order, reservations, loan/labor
  reservations, labor_book), conserving;
- the sim swapping the G1 tombstone for real removal at starvation death;
  estate → settlement **commons** (a conserved sim-owned pool);
- whole-system conservation extended to include the commons; the freed
  agent leaves no dangling cache reference;
- migration of the G1 tombstone tests to real-removal semantics (a dead
  colonist's slot is freed and reusable, not frozen-in-place);
- `engine-divergence.md`: the G0b-deferred free/reconciliation now done;
  estate-to-commons (heirs are G4b);
- acceptance tests in `sim/tests/g4a_death.rs` (+ econ arena/reconcile unit
  tests);
- README updates.

G4a excludes:

- no births, aging, households, or culture inheritance (G4b);
- no estate-to-heirs (settles to commons; G4b routes to households);
- no change to econ MARKET behavior — six goldens byte-identical; the free
  path is game-only; any econ edit is additive or inert-when-no-free;
- no population-stability study (G4b/later);
- no `HashMap` in logic; deterministic reconciliation; nothing drawn in the
  loops.

## Acceptance Tests

`sim/tests/g4a_death.rs` (+ econ unit tests):

1. `death_frees_the_arena_slot` — a starving colonist dies; its `AgentId`
   resolves `None` after; its arena slot is freed (and reusable — a
   subsequent insert may reuse it with a bumped generation).
2. `estate_settles_to_commons_conserving` — the dead colonist's gold + stock
   (econ) and any world-carried escrow move to the commons exactly; whole-
   system conservation holds across the death (nothing created/destroyed).
3. `caches_reconcile_no_dangling_reference` — after a death, `agent_order`
   and all reservation/book state contain no reference to the freed or a
   relocated position; the next econ tick runs correctly (no panic, no
   stale order matching).
4. `dead_colonist_places_no_orders_and_is_not_activated` — the freed
   colonist never bids/asks/works again and is absent from activation.
5. `reconciliation_is_deterministic` — a run with deaths is byte-identical
   across two invocations (the rebuild order is fixed).
6. `survivors_unaffected_by_a_death` — a death does not corrupt survivors'
   holdings, reservations, or market behavior (a survivor mid-trade before a
   death completes it correctly after).
7. `no_death_path_is_byte_identical` — a run with NO deaths produces the same
   records/digest as the equivalent pre-G4a (tombstone-era) run would for
   the no-death case; the reconciliation never fires without a death.
8. `econ_unchanged` — full workspace suite; six econ goldens byte-identical;
   all G1/G2*/G3* tests green (tombstone tests migrated to real-removal);
   `cargo clippy --workspace --all-targets -- -D warnings`; `cargo fmt
   --check`.

Manual check:

```bash
cargo test -p sim
cargo test -p econ          # arena free/reconcile unit tests
cargo run -p viewer -- run starved-hauler --ticks 20   # a death occurs, run continues
```

## Handoff Notes

- The goldens are safe BY CONSTRUCTION because the lab never frees an agent —
  but only if the NO-death path stays structurally byte-identical. Test 7 +
  the goldens are the tripwire; if a golden moves, the free-path machinery
  leaked into the no-free path. Fix that, never re-pin a golden.
- Reconcile EVERY external cache that holds a position or an agent id:
  agent_order, reservations, loan_reservations, labor_reservations,
  labor_book. A missed one = a dangling reference / stale order. Test 3 is
  the tripwire.
- Estate settlement is a CONSERVED transfer to the commons (gold + econ stock
  + world escrow). Nothing vanishes. Test 2 is the tripwire. Heirs are G4b.
- Order of operations: settle estate → cancel orders/release reservations →
  free → reconcile. Freeing before cancelling would strand reservations.
- Reconciliation must be deterministic (id-ordered rebuild), nothing drawn.
  Test 5 is the tripwire.
- This unblocks G4b (births/aging/households/inheritance) and retires the
  G1 tombstone seam recorded in `engine-divergence.md`.
- `git add` new files; gitignore stray build artifacts.
