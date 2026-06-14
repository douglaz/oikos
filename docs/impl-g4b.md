# Implementation Spec G4b: births, aging, households, culture inheritance

## Purpose

G4a gave the engine real death (runtime removal, estate, cache
reconciliation). G4b completes demography: colonists **age**, **die of old
age** (via G4a's removal path), are **born** into **households** when the
household can support them, and children **inherit** their parents'
`CultureParams` with mutation — so time-preference drifts under selection
across generations. This is the first milestone where the population is not a
fixed cast.

Scope (as with G3b's emergence and G4a's deferral discipline): G4b proves the
**mechanisms** on curated configs — population sustains (births ≈ deaths, no
extinction or blowup), inheritance mutates, estates route to heirs, and a
**patient lineage out-accumulates an impatient one** (sign only) on a curated
two-lineage config. The statistical robustness gates (the game-spec's
"100-seed stability band" and a multi-seed selection study) are **deferred**
to a G4 study milestone, analogous to M18/M19 for money emergence.

It is NOT a multi-seed stability/selection study (deferred), NOT migration
between settlements (later), NOT a change to `econ` market behavior (the six
goldens stay byte-identical — births/deaths are game-only), and NOT scalar
optimization (reproduction is a threshold rule, patience is the heritable
ordinal bias from G1).

## Verified Base Facts (2026-06-14, oikos @ `dddf01e`, 849 tests green)

1. **The removal side is done (G4a)**: `Society::remove_agent` (estate +
   free + full cache reconciliation, returns an `Estate`, refuses funded M3
   gracefully). Old-age death reuses this exactly.
2. **The insert primitive exists (G0b)**: `AgentArena::insert` (fresh or
   reused slot, fresh generation) and `insert_with_id` (arena.rs:159,191).
   What is MISSING is the Society-level **insert-side reconciliation** — a
   `Society::add_agent` that inserts AND extends the external caches
   (`agent_order`, `reservations`, loan/labor reservations) so the new agent
   participates correctly. G4b adds it as the mirror of G4a's `remove_agent`.
3. **`CultureParams` is the heritable trait** (life/src/culture.rs):
   `{ time_preference_bps, leisure_weight_bps }`, both clamped bps; G1 made
   time preference the structural, ordinal bias `regenerate_scale` consumes.
   Inheritance = child copies parent params with bounded mutation.
4. **The lab never adds or removes agents at runtime** — so, exactly as in
   G4a, the birth/death path is game-only and the six econ goldens are
   byte-identical by construction provided the no-birth/no-death path stays
   structurally unchanged.
5. **Determinism without RNG-in-loop** (load-bearing): births happen mid-run,
   but the loop draws no RNG. Birth mutation and any birth/death tie-breaking
   must be derived deterministically (e.g. a hash of parent params + a
   monotonic birth sequence number / lineage seed), NOT a live `Rng` draw.
6. **Conservation (G2b/G3a/G4a)**: a birth creates a colonist with a defined
   (possibly zero) endowment drawn from the household/commons — a conserved
   transfer, not a mint; a death routes the estate to the household (G4a
   routed to commons; G4b routes to heirs, commons as fallback).

## Milestone Boundary

G4b includes:

- `Society::add_agent` — runtime insert + insert-side cache reconciliation
  (the mirror of G4a's `remove_agent`); goldens byte-identical by
  construction (lab never adds at runtime);
- aging in `life`/`sim`: colonists track age (a year = a fixed econ-tick
  count); old-age mortality rises with age and kills via G4a's removal;
- households: colonists belong to a household; estate routes to the household
  (heirs), falling back to the commons if the household is extinct;
- births: when a household clears a need-security threshold (food margin +
  the household's holdings can endow a child), a birth occurs — `add_agent`
  with a fresh colonist (`NeedState`, inherited+mutated `CultureParams`,
  endowment a conserved transfer from the household);
- culture inheritance: child copies parent `CultureParams` with bounded,
  DETERMINISTIC mutation (hash of parent params + birth sequence; no RNG in
  loop);
- a `lineages` curated config (a patient household and an impatient one) and
  the measurement that the patient lineage out-accumulates the impatient one;
- viewer surfacing population/births/deaths and per-lineage wealth;
- acceptance tests in `sim/tests/g4b_demography.rs` (+ econ add_agent unit
  tests);
- README + `engine-divergence.md` (insert-side reconciliation done; demography
  mechanisms; the stability/selection studies deferred).

G4b excludes:

- no multi-seed stability or selection STUDY (deferred to a G4 study
  milestone, like M18/M19 for money);
- no inter-settlement migration (later);
- no scalar reproduction optimization (a threshold rule + the ordinal
  heritable bias);
- no change to econ MARKET behavior — six goldens byte-identical; birth/death
  paths game-only; any econ edit additive or inert-when-no-birth/death;
- no `HashMap` in logic; nothing drawn in the loops (deterministic mutation);
  no asserted magnitudes beyond the sign claims below.

## Domain Semantics

### add_agent (the insert-side mirror)

```
Society::add_agent(agent) -> AgentId:
  insert into the arena (fresh/reused slot, fresh generation);
  extend agent_order in the SAME priority order;
  extend reservations / loan_reservations / labor_reservations for the new id;
  the new agent participates from the next econ tick.
```

Goldens-safe rule (as G4a): the no-add path is structurally byte-identical;
reconciliation runs only on a birth.

### Aging and old-age death

Each colonist has an integer age (econ ticks since birth; seeded colonists
get a starting age). A year = a fixed tick count. Old-age mortality is a
deterministic threshold/curve in age (no RNG in loop — derive any per-tick
mortality decision deterministically from age + a stable per-colonist seed).
Death routes through G4a's `remove_agent`; the estate goes to the household.

### Births and households

A `Household` groups colonists. When a household's need-security clears the
threshold (e.g. sustained food margin and enough holdings to endow a child),
a birth fires: a new colonist is created (`NeedState` at a newborn baseline,
`CultureParams` inherited + mutated), endowed by a conserved transfer from
the household, and added via `add_agent` into the same household. Birth rate
is bounded (one per household per interval) and deterministic.

### Culture inheritance (the selection substrate)

Child `CultureParams` = parent's, each field mutated by a small bounded delta
derived deterministically from (parent field, birth sequence number) — no RNG
draw. Over generations, lineages drift; selection (patient lineages saving
more, surviving downturns) shifts the population distribution. G4b's curated
`lineages` config seeds one patient and one impatient household and measures
the patient one accumulating more capital/gold (sign).

## Acceptance Tests

`sim/tests/g4b_demography.rs` (+ econ unit tests):

1. `demography_run_is_deterministic` — same `(seed, config)` → byte-identical
   run with births and deaths (deterministic mutation; nothing drawn).
2. `add_agent_reconciles_caches` — a birth's new colonist appears in
   `agent_order` and participates (bids/asks/works) the next tick; no cache
   omits it; conservation holds (its endowment is a transfer, not a mint).
3. `population_sustains_without_collapse` — a viable config runs many
   econ-years with births ≈ deaths: population stays in a band (no extinction,
   no unbounded blowup), deterministic. Smoke/sign, not a tuned number.
4. `old_age_death_routes_through_removal` — an aged colonist dies via
   `remove_agent`; its slot frees and its estate settles (to the household);
   conservation holds across the death.
5. `child_inherits_mutated_culture` — a child's `CultureParams` equal the
   parent's within the bounded mutation delta, and the mutation is
   deterministic (same birth → same child params).
6. `estate_routes_to_household_then_commons` — a death's estate goes to the
   household heirs; if the household is extinct it falls back to the commons;
   conserved either way.
7. `patient_lineage_outaccumulates_impatient` — on the curated two-lineage
   config, the patient household's lineage ends with more accumulated
   capital/gold than the impatient one (SIGN only). The selection result.
8. `econ_unchanged` — full workspace suite; six econ goldens byte-identical;
   all G1/G2*/G3*/G4a tests green; `cargo clippy --workspace --all-targets
   -- -D warnings`; `cargo fmt --check`.

Manual check:

```bash
cargo test -p sim
cargo test -p econ          # add_agent reconciliation unit tests
cargo run -p viewer -- run lineages --ticks 200   # population + per-lineage wealth
```

## Handoff Notes

- `add_agent` is the exact mirror of G4a's `remove_agent`: insert + reconcile
  EVERY external cache (agent_order, reservations, loan/labor reservations).
  A missed one = a birth that can't trade. Goldens byte-identical by
  construction (lab never adds); test 8 + the goldens are the tripwire.
- Determinism without RNG-in-loop is mandatory: mutation and any
  birth/death decision derive deterministically from age/params + a stable
  sequence/seed. NO live `Rng` draw in the loop. Test 1 is the tripwire.
- Conservation: a birth's endowment is a TRANSFER from the household/commons
  (not a mint); a death's estate is a TRANSFER to the household (G4a's
  commons is the fallback). Nothing created or destroyed. Tests 2, 4, 6.
- Reproduction is a THRESHOLD rule + the heritable ordinal patience bias — no
  scalar fitness optimizer. Patience does its work through `regenerate_scale`
  (G1), not a fitness function.
- The stability BAND and the selection result are sign/smoke claims on
  curated configs; the multi-seed STUDIES are deferred (note in
  `engine-divergence.md`). Do not chase a tuned population number or a
  statistical selection gate here.
- `git add` new files; gitignore stray build artifacts.
