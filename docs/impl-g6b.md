# Implementation Spec G6b: research & tech tiers

## Purpose

G6a names the era the society has *earned*; G6b lets it *advance its
capabilities*: a **scholar** vocation produces **Knowledge** from labor, and
crossing a Knowledge threshold **unlocks a higher tech tier**, enabling a
recipe that was gated until then. Progression is research-driven (knowledge
accumulated by actual scholar labor), not a timer — the tech analogue of the
"earned, not timed" pillar.

Scope (sliced, like prior milestones): G6b proves the **mechanism** for ONE
tier unlock — tier-1 (the grain→flour→bread chain) plus a gated tier-2 recipe
that becomes available only after research crosses the threshold, with a
**control** (no scholars → no unlock). Multi-tier trees, knowledge diffusion
via trade (game-spec §5.7), and building-defs (vs recipe-defs) are deferred.

It is NOT a multi-tier tech tree (one unlock), NOT knowledge diffusion
(deferred), NOT building defs (recipe defs only), and NOT a change to econ's
recipe-execution behavior (the six goldens stay byte-identical).

## Verified Base Facts (2026-06-15, oikos @ `b14b9b2`, 928 tests green)

1. **Recipe gating exists**: `Recipe.enabled: bool` (project.rs:26) and the
   `DisableRecipe(RecipeId)` event (scenario.rs:360). A tier-gated recipe
   starts `enabled: false` and is enabled on unlock — reusing this flag, no
   new gating machinery in econ.
2. **`ContentSet` (G3a) holds the recipes** (content.rs:55): G6b extends it
   with a tier-2 recipe (gated) and tier metadata.
3. **Production/vocations exist** (G3a/G3b): the scholar vocation produces
   Knowledge the way millers/bakers run recipes (reuse the project/recipe
   execution path).
4. **Knowledge is an accumulator, not a tradeable good**: research consumes
   scholar labor (and optionally conserved good inputs) and accumulates a
   per-settlement Knowledge counter that is never traded or consumed — so it
   is OUTSIDE the goods-conservation ledger (documented). Any good INPUTS to
   research are conserved-consumed (accounted like consumption).
5. **Determinism + goldens**: research/unlock are deterministic and game-only;
   the lab uses no scholars/tiers, so the six econ goldens are byte-identical
   by construction.

## Milestone Boundary

G6b includes:

- a Knowledge accumulator per settlement (produced by scholar labor; optional
  conserved good inputs); NOT a tradeable good, NOT in the goods conservation
  ledger (its own accounted line);
- a scholar vocation that produces Knowledge (reusing the production path);
  scholars can be seeded or emerge (seeded is fine for G6b — emergence of the
  scholar role is out of scope, like G3a seeded production before G3b);
- tech tiers on recipes: a tier-2 recipe gated `enabled: false` until the
  Knowledge threshold unlocks it (flips `enabled: true` for that settlement);
- the unlock: crossing the threshold enables the tier-2 recipe, which then
  gets used (a higher-order good produced that was impossible before);
- a `research` config that unlocks tier 2 and a `no-scholars` control that
  never does (the falsification twin);
- conservation: good inputs to research accounted; Knowledge is a documented
  non-conserved accumulator;
- viewer surfacing: Knowledge level, current tier, unlock tick;
- acceptance tests in `sim/tests/g6b_research.rs`;
- README + `engine-divergence.md` (research/tiers; multi-tier trees,
  diffusion, building-defs deferred).

G6b excludes:

- no multi-tier tech tree (ONE unlock: tier 1 → tier 2);
- no knowledge diffusion via trade (deferred, §5.7);
- no building-defs (recipe-defs only);
- no emergence of the scholar role (seeded; like G3a before G3b);
- no change to econ recipe-execution behavior — six goldens byte-identical;
  any econ edit additive;
- no `HashMap` in logic; nothing drawn; no asserted magnitudes beyond
  unlock-happens / control-doesn't and conserved inputs.

## Domain Semantics

### Research and the Knowledge accumulator

A scholar produces Knowledge each econ tick from labor (and optional good
inputs, which are conserved-consumed and accounted). Knowledge accumulates in
a per-settlement counter — monotonic, never traded, never consumed, so it is
NOT part of the goods conservation invariant (it has its own reported line:
`knowledge_produced`). Good inputs to research ARE conserved (accounted as
research consumption alongside ordinary consumption).

### Tech tiers and the unlock

Recipes carry a tier. A settlement starts with tier-1 recipes enabled and
tier-2 recipes `enabled: false`. When the Knowledge counter crosses the tier-2
threshold, the settlement enables its tier-2 recipes (flips `enabled`). The
unlock is per-settlement, deterministic, and one-way (a tier, once unlocked,
stays). After unlock, producers can run the tier-2 recipe (a higher-order good
appears that was impossible pre-unlock).

### Control

The `no-scholars` control has no scholar vocation, so Knowledge never
accumulates and the tier-2 recipe never unlocks — proving the unlock is driven
by research, not by time or anything else.

## Acceptance Tests

`sim/tests/g6b_research.rs` (+ unit tests):

1. `research_run_is_deterministic` — same `(seed, config)` → byte-identical
   run including Knowledge accumulation and the unlock tick.
2. `research_unlocks_tier_two` — in `research`, scholar labor accumulates
   Knowledge, it crosses the threshold, the tier-2 recipe flips to enabled at
   a definite tick, and afterward the tier-2 good is produced (impossible
   before).
3. `no_scholars_control_never_unlocks` — the falsification twin: with no
   scholars, Knowledge stays at zero, the tier-2 recipe stays disabled, and
   the tier-2 good is never produced. Paired with test 2, shows research
   drives the unlock.
4. `research_inputs_conserve` — any good inputs to research are
   conserved-consumed (accounted); whole-system goods conservation holds;
   Knowledge is reported on its own non-conserved line (not in the goods
   ledger).
5. `tier_gate_blocks_pre_unlock` — before the unlock, the tier-2 recipe is
   `enabled: false` and cannot be run (no tier-2 good produced), even if a
   would-be producer holds its inputs.
6. `unlock_is_one_way` — once unlocked, the tier stays unlocked (no flapping).
7. `econ_unchanged` — full workspace suite; six econ goldens byte-identical;
   all prior tests green; `cargo clippy --workspace --all-targets -- -D
   warnings`; `cargo fmt --check`.

Manual check:

```bash
cargo test -p sim
cargo run -p viewer -- run research --ticks 60      # knowledge accrues, tier 2 unlocks
cargo run -p viewer -- run research-control --ticks 60
```

## Handoff Notes

- Reuse `Recipe.enabled` for tier gating (start tier-2 `false`, flip on
  unlock) and the production path for scholar Knowledge output — no new
  econ recipe machinery.
- Knowledge is an ACCUMULATOR, not a tradeable good: monotonic, never
  traded/consumed, OUTSIDE the goods conservation ledger (its own reported
  line). Good INPUTS to research are conserved-consumed. Test 4 is the
  tripwire.
- The control (test 3) is the proof: no scholars → no unlock. If the tier
  unlocks without research, the gate is reading time/something else — fix
  that, don't weaken the test.
- Scope is ONE tier unlock with SEEDED scholars; multi-tier trees, knowledge
  diffusion, building-defs, and scholar-role emergence are deferred (note in
  `engine-divergence.md`).
- econ goldens byte-identical (scholars/tiers are game-only); determinism.
- `git add` new files; gitignore stray build artifacts.
