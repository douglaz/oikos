# impl-69 — DH.b: the reproductive-burden robustness audit

Status: SPEC-READY v5 (Codex xhigh, 5 rounds: R1 ×12, R2 ×9, R3 ×8, R4 ×5, R5 =
SPEC-READY with 1 NIT, folded into this text)

Changelog v5 (R4): trade records gain good+quantity and validation moves to the
purchase-credit seam with identity-only downstream joins (R4-1); nonmonotone pairs
restricted to q∈{1,2,3,4,8} with no-q=0 tests (R4-2); succession evaluated once per
streak with executions inside it, exempted from the per-window heading (R4-3);
Vocation::Miller/RecipeId::Mill and Vocation::Baker/RecipeId::Bake named exactly
(R4-4); "a future DH.c rerun is authorized only if" (R4-5).

Changelog v4 (R3): exhaustive ordered class-failure payloads on rungs 3–5 (R3-1); the
FoodChannel table enumerated in full (R3-2); completeness with multiplicity one (R3-3);
live seam mismatches hard-fail and §6 rewritten to the pure/live distinction (R3-4);
nonmonotone entries carry the saving arm (R3-5); adoption/execution bits pinned to
class-correct role/recipe with wrong-role/wrong-recipe negatives (R3-6); grand-oracle
non-rerun scoped to scientific follow-up vs regression execution (R3-7); stale wording
fixed (R3-8).

Changelog v3 (R2): nested common-streak sets C⊇S⊇F⊇E with highest-to-lowest
classification and earliest-streak-within-highest-set (R2-1); funding failure as a
deterministic nonempty reason bitset aggregated over the proof streak (R2-2); synthesis
rows 6/8 made disjoint (R2-3); row 4 restricted to StressOnly with the pairwise
nonmonotonicity flag orthogonal and always printed (R2-4); the independent BirthOccurred
event stream and non-tautological completeness equality (R2-5); live-suite hard failure
on instrumentation corruption, conservative Unverifiable kept only for the pure
classifier (R2-6); FoodLot identity/taint lifecycle semantics pinned and every new hook
gated through closure_active() so the landed force-disable control governs it (R2-7);
the six-bit causal-succession diagnostic with class↔tool↔role↔recipe correspondence and
real-seam positive/negative tests (R2-8); "two saving arms, Off or On" (R2-9).
Milestone: DH.b — the reproduction frontier's first slice (per the accepted DH.a S-row
selection and the 2026-07-13 scoping consultation, both in the record)
Digest tag: NONE (R1-11: q is already serialized in the demography bytes, and an active
motive already emits tag 31 — a new tag would add no identity; identity is instead
PROVEN by tests, §5.6)
Base: `frontier_closed_circulation()` (DH.a, master ea9650f) — the exact closed base;
each cell differs from it by exactly {`child_food_endowment=q`, the two-field saving
arm} and nothing else.

Changelog v2 (R1): arm/seed aggregation and the DH.c gate defined (R1-1); classifier
made total with NoBirth, the succession bitset, per-class flow payload, and the q=0
CostlessReplacement rules (R1-2); the synthesis rebuilt as a precedence-ordered
exhaustive table (R1-3); W=36, M=5, RUN_TICKS=1600, the start formula, and common-streak
selection pinned (R1-4); founders pinned to a generation-time registry snapshot (R1-5);
the birth-funding telemetry contract rebuilt (purchase identity + endowment taint +
per-birth records + completeness assertions; rung 6 renamed) (R1-6); succession events
causally joined (R1-7); the role-change guard corrected to invariant-preservation
(R1-8); hard-guard failures now fail the suite (R1-9); the saving arm pinned as the
two-field state (R1-10); tag 35 removed in favor of identity tests + DH.a inertness
re-verification (R1-11); §8 restores the recorded deselections (R1-12).

## 1. Motivation and the preregistered question

DH.a established that the closed regime's producer lineages are extinct before the first
classified window: reproduction, not demand, binds first (report §33; article §9). The
keystone names one exogenous number at the center of that wall: the four-loaf
birth-provisioning burden — `DemographyConfig::child_food_endowment` (demography.rs:86,
default 4 at :144) — which simultaneously gates birth, debits the parent, and endows the
newborn. C3R.d proved the saving motive works *toward* that target and loses the
allocation contest ~500:1; its sufficiency control proved a household HOLDING four
loaves births at every otherwise-eligible opportunity.

**The question:** is producer-lineage extinction on the exact closed base robust to the
configured reproductive-provisioning burden, or is the "reproduction wall" substantially
an artifact of requiring one parent to hold and transfer four loaves at once?

Praxeological status: `child_food_endowment` is imposed natural data — a physiological
conversion factor, like lifespans and hunger. Sweeping environmental data and observing
action imposes no institution. The canonical value 4 REMAINS canonical regardless of
outcome: a passing lower value is a sensitivity boundary — not emergence, not
calibration, not permission to change the default, not an institutional solution.

## 2. The grid (60 cells; classify, don't tune)

On `frontier_closed_circulation()`, seeds = the landed [3, 7, 11, 19, 23]:

- **Burden** `q ∈ {0, 1, 2, 3, 4, 8}`.
- **Saving arm (two-field, R1-10):** Off = `(birth_stock_saving=false, mode=Off)`;
  On = `(birth_stock_saving=true, mode=Motive)`. `SufficiencyControl` is asserted
  unreachable in every cell. The saving TARGET auto-derives from
  `child_food_endowment` (landed: mod.rs:16130 reads it; :16178 passes exactly q to
  `medium_scale_extension`) — the existing target test is extended across all six q
  values including q=0's no-op.
- Nothing else varies. Frozen explicitly: birth interval, hunger ceiling, household
  cap, lifespan machinery (`ticks_per_year=6`, `old_age_onset_years=3`,
  `lifespan_span_years=3` ⇒ producer lifespans {18,24,30,36}), prices/order-book
  rules, starting holdings, demand composition, every DH.a config field. No ignition,
  support, wage labor, merger, new actor, or altered mortality.

An **arm** is a `(q, saving)` pair (12 arms × 5 seeds). Cell roles:
- `q=0` — costless-birth reachability control; scored on the ordinary ladder but
  **capped** (§3): it can never carry an economic rank and never enters the economic or
  motive synthesis.
- `q=1..3` — the sub-canonical lattice. `q=4` — the canonical reference. `q=8` —
  stress control (cannot authorize a future DH.c grand-oracle rerun; a q=8-only pass
  is a non-monotone finding, not a success).

## 3. The succession-survival oracle

The DH.a/C3R.e oracle is NOT executed or interpreted as a new scientific follow-up in
this slice (its existing golden and inertness suites still re-run purely as
regressions, §5.6/§7.1 — R3-7). **The DH.c gate (R1-1, R4-5):** a FUTURE DH.c
grand-oracle rerun is AUTHORIZED only if a `q=4` arm passes on **5/5 valid seeds** at rung 6+/7 (§3, Per-cell classifier), with
the passing arm(s) named. A q passes when at least one of its two saving arms, Off or On, passes 5/5.
Partial seed results are reported, never promoted. If only a relaxed q passes, DH.b
stops and reports sensitivity.

**Suite mechanics (R1-4, R1-9):** `sim/tests/reproductive_burden.rs`, serial, seeds
outermost, arms in fixed (q ascending, Off-then-On) order; `RUN_TICKS = 1600` (the
landed oracle constant); **W = 36** (the actual maximum producer lifespan on this base:
`(3+3)×6`), **M = 5** consecutive windows = 180 scored ticks. Tick `t` = its
post-`econ_tick` sample; events in `[t, t+1)`. Per seed:
`start = 36 × ceil((last_founder_death_tick + 1) / 36)`; only complete windows before
tick 1600 are enumerated. Every founder's death tick is printed.

**Founders (R1-5):** `original_producer_founders` = the generation-time snapshot of the
six initial Miller/Baker `AgentId`s from the closure registry (NOT derived from
`HouseholdSpec::founders`, which is 0 for these households — mod.rs:4111; the seeded
producers are assigned separately, mod.rs:10276). A **nonfounder** is an agent absent
from that set AND backed by a recorded birth event. Registry consistency = a per-tick
closure-registry invariant over every living actor and fixed household class (not the
private-land registry check).

**Hard guards (per cell):** conservation; the registry invariant; every founder dead
before `start`; zero immortal producers (no lifespan-less agent maps to a producer
ClosureClass, per tick); zero intervention/support-origin stock; funding-attribution
completeness (§3.4). Guard failure prints `PreconditionInvalid { guard }` AND **fails
the integration test after printing** (R1-9) — the suite additionally asserts exactly
60 cells ran and all were guard-valid. Only scientific outcomes are print-only.

**Scored per producer class (Miller, Baker; DH.a ClosureClass mapping) — criteria 1
and 3 are per-window; criterion 2 is evaluated ONCE PER STREAK (R4-3): each class
needs one class-correct chain whose successor execution occurs WITHIN the streak;
the inheritance and adoption events may predate the streak but must strictly precede
that execution:**
1. **Nonfounder continuity** — ≥1 living nonfounder member at every sample.
2. **Causal succession (R1-7, R2-8)** — from the real seams' events
   `ToolInherited { tick, class, deceased, heir, tool }` (mod.rs:14839 region) and
   `InheritedToolRoleAdopted { tick, class, heir, tool, role }` (mod.rs:18447 region):
   the SAME born-in-simulation `(class, heir, tool)` pair, inheritance strictly before
   adoption, continued possession at adoption, and ≥1 recipe execution BY THAT
   successor within the streak — with **stage correspondence enforced**: Miller ↔ mill
   ↔ `Vocation::Miller` + `RecipeId::Mill`; Baker ↔ oven ↔ `Vocation::Baker` +
   `RecipeId::Bake` (a Miller-class heir inheriting/adopting/executing the wrong
   stage satisfies nothing). The
   `SuccessionBits` diagnostic records exactly which of {inheritance_event,
   adoption_event, tuple_join, strict_ordering, possession_at_adoption,
   successor_execution} failed. REAL-SEAM tests required (not classifier fixtures):
   a positive inheritance→adoption→execution chain, and negatives for wrong
   heir/tool/class, wrong ROLE and wrong RECIPE (all preceding tuple fields correct —
   R3-6), reversed ordering, lost possession, and absent execution.
3. **Staffed flow** — the class's stage runs (recipe executions > 0) in the window.
4. **Birth funding** — every qualifying birth classified per §3.4.

**Streak rule (R1-4, R2-1):** define nested sets of COMMON M=5-window streak starts —
`C` (Miller AND Baker each satisfy nonfounder continuity in every window of the same
streak), `S` (⊆C: both classes' causal-succession criterion also holds over the
streak), `F` (⊆S: staffed flow also holds for both classes in every window), `E` (⊆F:
every qualifying birth in the streak is EconomicallyFunded). Classification is
highest-to-lowest: E≠∅→rung 7, else F≠∅→rung 6, else S≠∅→rung 5, else C≠∅→rung 4,
else→rung 3 (births exist but no common continuity streak — including the
disjoint-streak case where each class has its own M-streak but never a common one).
The proof streak is the EARLIEST start within the highest achieved set. No stitching
of classes, criteria, or periods. Unit tests required: streaks at grid start/end,
off-by-one boundaries, and disjoint Miller/Baker streaks at EVERY criterion boundary.

### 3.4 Birth-funding telemetry (R1-6 — the contract, built on DH.a machinery)

Activated by the existing `closed_circulation` marker (no third experimental config
field; observation-only; DH.a's per-tick inertness comparison and committed golden must
still pass unchanged, §5.6):

- Per settled trade: `{trade_id, buyer, good, quantity, earned_paid, endowed_paid,
  positive_consideration}` recorded from the DH.a gold split at event time (R4-1).
  Buyer/good/quantity validation happens AT THE PURCHASE-CREDIT SEAM: all fresh
  `Bought` fragments for a trade must be credited to its buyer, carry its good, and
  aggregate exactly to its quantity — a mismatch there hard-fails. Downstream
  split/inheritance/birth records join by purchase identity ONLY (no comparison of a
  later fragment's current owner or quantity against the original trade — legitimate
  provenance would falsely reject). Real-seam positives for split/inheritance joins
  and negatives for wrong buyer, wrong good, and wrong aggregate quantity required.
- Lots gain (runtime-only) a **purchase identity** and an orthogonal
  **ultimate-construction-endowment taint**. Lifecycle semantics (R2-7): splitting,
  inheritance, and birth transfer preserve BOTH identity and taint; resale OVERWRITES
  identity with the new trade ID while preserving taint; coalescing requires equal
  identity AND taint; taint is set at construction and never cleared; coalescing
  retains ALL existing equality requirements (channel, intervention flag) PLUS
  identity and taint. The full landed `FoodChannel` enumeration (R3-2):
  `Bought` — purchase identity required; resale overwrites identity; taint preserved;
  payment rules apply. `SeededMinted` — on this base, construction stock: no purchase
  identity, construction-endowment taint SET. `SelfProduced` — no purchase identity,
  taint CLEAR; qualifies for EconomicallyFunded. `Foraged`, `Commons` — hard-asserted
  UNREACHABLE on this exact base. Real lifecycle unit tests required.
- **Activation (R2-7):** every new hook gates through `closure_active()` — the same
  predicate the landed force-disable control flips — so DH.a's inertness comparison
  genuinely disables the new telemetry on its off side. The inertness test re-runs
  with the telemetry compiled in.
- **Two independent per-birth streams (R2-5):** (a) `BirthOccurred { tick, class,
  parent, child }` emitted at successful newborn insertion into a fixed Miller/Baker
  closure class (a *qualifying birth*; `birth_id` = the child `AgentId`); (b) the
  funding record `{tick, class, parent, child, q, drawn lots (identities, taints)}`
  emitted after the exact lot transfer. The two streams have separate emission sites —
  the completeness equality is not tautological.
- Completeness assertions (hard guards, multiplicity-aware — R3-3): exactly ONE
  BirthOccurred event and exactly ONE funding record per qualifying child ID; equal
  stream cardinalities; equal ID sets; each q>0 birth's drawn quantities sum exactly
  to q.
- Classification per birth: **EconomicallyFunded** requires zero ultimate-Endowed
  physical quantity AND, for every Bought unit, a joined trade with
  `endowed_paid == 0` and positive consideration; SelfProduced units qualify. Failures
  set FundingBits (§3, Per-cell classifier, rung 6). **Corruption vs result (R2-6):** the PURE
  classifier stays conservative (malformed synthetic input → Unverifiable), but the
  LIVE integration suite hard-fails on instrumentation corruption — duplicate trade
  IDs, a Bought lot without purchase identity, an identity without exactly one
  matching trade record (spot-trade IDs are globally derivable on this base; every
  Bought lot must join exactly once), or any purchase-credit-seam buyer/good/quantity
  mismatch per the R4-1 rule above (downstream fragments join by identity only).

### Per-cell classifier (pure, total, precedence-ordered; R1-2)

1. `PreconditionInvalid { guard }` (also fails the suite)
2. `NoBirth { q }` — no qualifying birth ever (total for all q, including q=0: free
   provisioning does not bypass cadence, hunger, extinction, or cap gates)
3. `BirthsButLineageExtinct { detail: MissingPrivateStreaks { classes: nonempty
   Miller-before-Baker-ordered set } | DisjointPrivateStreaks }` — births occur; the
   common-continuity set C is empty (R3-1: covers one class lacking a private streak,
   BOTH lacking one, and the both-have-private-but-never-common case)
4. `LineagePersistsSuccessionAbsent { failures: nonempty map<ClosureClass,
   SuccessionBits> }` — C≠∅, S=∅; per-class bits aggregated on the earliest C-streak,
   Miller-before-Baker ordering. `SuccessionBits` (R2-8, R3-6) = {inheritance_event,
   adoption_event (requires the class-correct Vocation), tuple_join, strict_ordering,
   possession_at_adoption, successor_execution (requires the class-correct RecipeId)}
5. `FunctionalSuccessionFlowAbsent { classes: nonempty ordered set }` — S≠∅, F=∅;
   every class whose flow failed on the earliest S-streak
6. `FunctionalSuccessionEconomicFundingUnproven { reasons: FundingBits }` — F≠∅, E=∅.
   `FundingBits` (R2-2) = a deterministic NONEMPTY bitset aggregated over the earliest
   F-streak's qualifying births: {PhysicalEndowmentTaint, EndowedPayment, MixedPayment,
   ZeroConsideration, Unverifiable} — any `endowed_paid>0` sets EndowedPayment; both
   payment buckets >0 additionally sets MixedPayment; every combination table-tested
   (renamed from "EndowmentDependent": the bitset distinguishes proven endowment
   dependence from a failed join — the old name overclaimed)
7. `FunctionalSuccessionEconomicallyFunded` — E≠∅
q=0 cap (R1-2): a q=0 cell that would reach rung 6/7 returns `CostlessReplacement`
instead (criteria 1–3 on a common streak, no economic rank, excluded from the economic
and motive syntheses; the classifier asserts q=0 never emits rungs 6/7).

### Cross-grid synthesis (pure, precedence-ordered, exhaustive; R1-3)

Evaluated top-down; the first matching row is the verdict; the payloads (exact passing-q
sets, per-arm per-seed rung tables, non-monotonicity flags, costless results, motive
lists) always print in full:

1. `InvalidGrid` — any cell `PreconditionInvalid` (the suite has already failed).
2. `CanonicalBurdenSurvives { arms }` — a q=4 arm at rung 6+/7 on 5/5 seeds.
   (Authorizes a future DH.c grand-oracle rerun; does not execute it in DH.b.)
3. `SubcanonicalSurvives { highest_q, arms }` — some 0<q<4 arm passes 5/5; q=4 does not.
4. `StressOnly { arms }` — ONLY q=8 arms pass 5/5 (R2-4).
5. `SeedHeterogeneousSurvival { table }` — some q>0 cell reaches rung 6/7 but no arm
   passes 5/5.
6. `ContinuityWithoutEconomicSuccession { best_rung_by_arm }` — at least one q>0 cell
   at rung 4 or 5, and NO q>0 cell above rung 5 (R2-3).
7. `CostlessOnlyReplacement` — only q=0 reaches CostlessReplacement; every q>0 cell is
   at rungs 2–3.
8. `RobustExtinction` — every valid q>0 cell at rungs 2–3 AND q=0 did not reach
   CostlessReplacement. (Rows 6/7/8 are disjoint by construction — R2-3; discriminating
   rung-3-only and rung-4 table tests required.)
Orthogonal payloads (always printed, never verdicts):
- `motive_effect = { on_better: [(q, seed)…], off_better: [(q, seed)…] }` — exact
  matched-pair lists (same q and seed, On rung vs Off rung), q=0 excluded; both lists
  may be non-empty; inversions print, never smoothed.
- `nonmonotone = [(saving, q_low, q_high)…]` (R2-4, R3-5, R4-2) — every tested pair
  with q_low < q_high, **both drawn from {1,2,3,4,8}** (q=0 is capped and excluded —
  comparing against it would report false inversions), where the q_high arm of that
  saving arm passes 5/5 and the matching q_low arm does not (pairwise, so
  contiguous-but-not-downward-closed sets like {2,3} are caught); printed under every
  synthesis row including rows 2/3; tests assert no emitted tuple contains q=0.

## 4. Honest nulls (all publishable)

RobustExtinction (the wall hardens); CostlessOnlyReplacement (mechanically possible,
economically unfunded at any burden); SubcanonicalSurvives (the canonical conclusion is
burden-sensitive; the constant stays); NoBirth/BirthsButLineageExtinct dominant (the
gate was not the whole continuity problem); LineagePersistsSuccessionAbsent (demography
repaired, functional inheritance did not); FundingUnproven dominant (survival on the
finite construction bridge or unverifiable); empty motive separation (no evidence the
saving behavior causes anything).

## 5. Mechanism inventory

Already landed (verify, do not rebuild): the configurable burden; deterministic
mortality/cadence/hunger/cap; the six class-fixed households; conserved birth transfer;
estate/heir routing; tool inheritance + re-adoption seams (mod.rs:14839, :18447); C3R.d
saving wants deriving the target from the burden (mod.rs:16130, :16178); DH.a's
provenance (gold split, physical origin, raw tape, registry, reducers).

Build only:
1. The 60-cell suite (serial, fixed order, guard-failure = test failure).
2. The classifier + synthesis (pure; table-tested over every variant, precedence,
   streak boundaries, the q=0 cap, and the funding classification matrix).
3. Founder snapshot + nonfounder identification (registry-based, R1-5).
4. The two succession events at the real seams (runtime-only, marker-gated).
5. The §3.4 funding telemetry (marker-gated, observation-only).
6. **Identity tests replacing a digest tag (R1-11):** (a) all 12 arm configurations
   are pairwise distinguished by EXISTING canonical state (q rides the demography
   bytes at mod.rs:30462; the motive emits tag 31); (b) the `(q=4, Off)` cell's config
   is byte-identical to the landed closed base; (c) DH.a's per-tick inertness
   comparison and the committed ignition-withdrawal golden re-run UNCHANGED with the
   new telemetry compiled in. Vocation-transition invariance (R1-8): every transition
   preserves age, lifespan, seed, parent, household, and fixed ClosureClass — asserted;
   no adoption creates or refreshes a lifespan.

## 6. False-green / tuning kills (each an acceptance criterion)

- No post-hoc q selection; the full grid prints and synthesizes; no cell dropped/re-run.
- q=4 canonical in every constructor; the sweep lives only in the test.
- The q=0 cap assertion; q=8 barred from the DH.c gate.
- Birth count is never success; rungs require the common M-streak.
- A living member is not succession: the causal `(class, heir, tool)` join with
  ordering, possession, and successor execution (R1-7).
- No immortal agent maps to a producer class (per-tick hard guard).
- Vocation transitions preserve identity/lifespan (R1-8).
- Endowed taint routes below rung 7; malformed joins produce `Unverifiable` only in
  pure classifier fixtures — every equivalent LIVE condition hard-fails the suite
  (R3-4).
- The saving target equals q (asserted per cell, all six q values).
- Monotonicity is a diagnostic expectation; the pairwise `nonmonotone` payload always
  prints (the `StressOnly` verdict covers only the q=8-only-pass shape; general
  inversions live in the payload, never smoothed).
- Verdicts computed through the real classifiers; printed enums only.

## 7. Acceptance criteria

1. Flags-off goldens byte-identical; conservation every tick; ALL landed suites
   reproduce — including the DH.a oracle golden and its inertness check with the new
   telemetry compiled in (§5.6c).
2. The 60-cell grid runs serial, all guards valid (asserted), per-cell verdicts +
   per-window diagnostics + the synthesis printed; wall clock reported (estimate
   ~1.6s/run × 60 ≈ 100s — confirm and disclose).
3. The full test battery of §5.2/§5.6 and §6 green.
4. Config-identity per cell: `frontier_closed_circulation()` + exactly
   {child_food_endowment=q, (birth_stock_saving, mode) ∈ {(false, Off), (true,
   Motive)}} — nothing else; `SufficiencyControl` unreachable.
5. fmt/clippy/test gates green.
6. The writeup reports the full grid, the synthesis, the motive-effect lists, and the
   DH.c gate decision under the §3 rule.

## 8. Out of scope, with the recorded deselections (R1-12)

Any succession institution is out of scope here, and the record's deselections stand:
the **pooled-heir guild** is the retained contingent follow-up (only meaningful once
any heirs exist to pool); **apprenticeship from the gatherer pool is deselected**
because the only candidates are deathless — promotion would reopen the
immortal-producer reservoir the keystone closed; **household merger is deselected**
because it pools people and creates none; **succession-by-hiring is deselected**
because it bundles wage formation, producer employers, firm ownership, worker
succession, and mortality into one slice. This audit's classification selects among
the retained options; it does not re-open the deselected ones. Also out of scope: any
change to lifespans, cadence, hunger, prices, or the closed base; the DH.c rerun.
