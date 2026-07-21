# impl-71 — C3R.f: Producer-Lineage Lifespan Sensitivity (does a longer producer-house life lift C3R.b's cushion-bound limp toward healthy flow?)

Status (spec): **REDIRECTED — do NOT build the lifespan sweep** (2026-07-21). A Codex + Fable dual
review (Fable *ran* the sweep ×4/×8) proved **lifespan is not the causal lever**: the mortal chain
collapses via a **flour-market re-ignition deadlock**, not life-length, and flow stays 0 across a
16× lifespan range. See **§−2 (AUTHORITATIVE)** below. The successor milestone is the re-ignition
deadlock; this lifespan axis is parked as at most a later mortality-*frequency* sensitivity. The
immortal five-seed viability gate impl-73 cut 2 cleared is on the *immortal* base only — the mortal
base still collapses (§−1).

**Prior status: BLOCKED** (grill-with-docs 2026-07-18; refined by Codex review same day) —
pending impl-72 (C3R.g), the baker-role profit diagnostic. A pre-check found this base's chain
functions on only 1/5 seeds *even with immortal producers*, and the binding constraint is the
Baker *role* being rejected by the role-choice recipe-profit test
(`recipe_adoption_pays_for_money` / `recipe_is_profitable`, `phases.rs:2298-2318`) — NOT a payback
bar, and the margin is yield-aware (`3·P_bread − P_flour − cost`; bread price ~1 alone is
inconclusive). Mortality-independent, upstream of any lifespan sweep. **Unblock condition (Codex):
an immortal five-seed viability gate must pass first** — a functioning chain on all five seeds,
not one — before this milestone is meaningful. **impl-73 (C3R.h) is that milestone**: the impl-72
diagnostic + a price-path measurement + a Codex review localized the wall to a **stale
input-price appraisal** (the baker is rejected on a phantom flour price frozen from an early boom;
`realized_price` has no recency gate) — with weak final demand a secondary contributor. impl-73
fixes that and must clear the five-seed gate first. See `docs/impl-final-stage-demand.md` §−0. Prior status (now blocked): **v2 —
SPEC-READY** (Codex xhigh spec-review, 1 round: 1 P0 + 9 P1 folded into the authoritative `## −0`
section below). Successor to impl-62 (C3R.a: mortal chain-producers, no
succession) and impl-63 (C3R.b: capital inheritance). Renamed C3R.c → **C3R.f** (C3R.c is impl-64
earned provisioning; C3R.d/e are also taken). Design origin:
`docs/design-mortal-producer-succession.md` (office-hours 2026-07-17) + the plan-eng-review that
verified the C3R.b subsystem. **v1 (superseded):** framed the sweep as a "subsidy-free ratio" —
Codex's review showed `food_provision=0` is NOT subsidy-free (the `producer_subsistence` cushion
and legacy-household provisioning remain), the "ratio" has no operational denominator, and the
intervention is producer-house-LINEAGE lifespan (not producer lifespan), with a real demographic
coupling. v2 reframes to an absolute lifespan-sensitivity experiment at the C3R.b minimal-cushion
floor, with the coupling measured, not assumed away.

## −2. Dual-review outcome (AUTHORITATIVE, 2026-07-21): lifespan is NOT the lever — REDIRECT

A Codex + Fable dual review of the §−1 re-scope both returned **NEEDS-REVISION**, and they
converge (Fable *ran* the sweep; Codex reasoned it from the code): **raising producer lifespan does
not fix the mortal collapse.** Fable's read-only probe swept lifespan ×4/×8 on the exact §−1 base
and **final-window bread stayed 0 in 15/15 mortal runs across a 16× range** — re-adoption *attempts*
rise with life, but flow never re-forms.

**The real wall — a flour-market re-ignition deadlock (the sibling of cut 1's stale-price wall, on
the supply side).** After the founder bakers die, the chain enters an **absorbing de-staffed state**:
no baker → no flour demand → millers under working-capital discipline (`project_input_bids`) stop
producing flour → no living agent holds flour → L2's own no-holder-decline (`fresh_input_ask`,
`mod.rs:10103`; `phases.rs:2316`) rejects every heir's bake appraisal as **`InputPriceAbsent`
(83–93% of bake rejections, `margin_nonpositive = 0`)**. The baker stage dies at the **first**
founder die-off, not "across generations". Longer life only delays entry into this state. Both
reviews also verified the code **rules out** a mandatory handover gap and "heir starts poor"
(estate + tools transfer *before* same-tick role choice/production, `mod.rs:7183`,
`demography.rs:366`) — so the cause is the market re-ignition, not the succession mechanics.

**Confounds/proxies both reviews flagged (heed these in any successor):**
- The "immortal control" is **not** `life = ∞` of the mortal system — with `mortal_chain_producers`
  off, role-choice/capital admit `lifespan == None` adopters (`phases.rs:2220`) and producer-house
  tagging changes (`generation.rs:560`), so "immortal sustains / mortal collapses" partly measures
  **adopter-pool restriction**, not mortality. A no-death control must keep all mortal plumbing on
  and give producer houses a lifespan beyond the horizon.
- "Succession fires hard" over-reads proxy counters: `producer_tool_inheritances` counts tool
  *units* not distinct handovers; `heir_tool_adoptions` is aggregate. Neither proves a
  death→inherit→adopt→retain→**bake** join. Instrument the real join (`burden.rs:83`).
- Current-vocation `baker_class_gold` (cut 2's metric) is churn-unstable for a mortal class (reads
  0 during staffing gaps; loses estate gold on not-yet-promoted heirs). Use fixed producer-house
  *lineage* liquidity over a window.

**REDIRECT.** Do **not** build this lifespan sweep as the milestone — it would return a correct but
**unattributed MORTALITY-BREAKS-IT** with a wrong causal frame (an eighth "obvious lever isn't the
wall"). The next milestone is the **flour-market re-ignition deadlock** (make the flour market
re-ignite after a producer die-off — e.g. millers keep a flour buffer / keep supplying without a
current baker buyer, or a would-be baker may bootstrap-appraise against a miller *ask* even with no
flour currently held). Lifespan is PARKED as at most a later mortality-*frequency* sensitivity axis
— revisit only after the re-ignition seam is fixed and the mortal chain functions.

**If ever built, fix these first (both reviews):** the lifespan mechanism has **three** construction
sites (seeded producers `generation.rs:542`, ordinary founders `generation.rs:685`, births
`phases.rs:532`) — scale the sampled lifespan AND founder age with checked integer arithmetic
(`lifespan_scale_bps: Option<u32>`), clamp `founder_age < lifespan`; add a `HouseholdSpec`
digest-coverage guard (the current guard hides `households: _`, `digest.rs:2553`) with emit-only-when-
`Some` sparse encoding; one common horizon (delete §5 T2); a structure/flow split + exhaustive
per-seed outcome tree (mixed-seed, non-monotonic, sustains-but-insolvent, right-censored).

## −1. Post-cut-2 re-scope (AUTHORITATIVE, 2026-07-21 — supersedes §−0 and §§0–7 where they conflict)

**The v2 premise is now false.** v2 (and §2 below) assumed the mortal-producer chain *dies* at
`food_provision = 0` and asked whether raising lifespan rescues it (RATIO-BAND / STRUCTURE-ONLY /
RATIO-NULL). But the diagnosis journey (impl-72 + cut 1/2 of impl-73) found the wall was **not
lifespan** — it was a **stale input-price appraisal**, fixed by `ChainConfig::stale_input_price_fix`
(cut 1). With that fix the *immortal* base **functions and stays solvent** — all five seeds sustain
(impl-73 cut 2, `EITHER_SUFFICES`; master `b6404ba`). So the *stale-price* confound is gone.

**But the mortal base still collapses even with L2 — measured, 2026-07-21.** On
`mortal_chain_producers + mortal_producer_inheritance + mortal_producer_tool_inheritance` with L2
on at default lifespan, all five seeds end with **0 living bakers and 0 final-window bread** —
*despite* succession firing hard (173–218 producer deaths, 165–202 heir-adoptions, hundreds–thousands
of tool inheritances/run). So L2 + the existing succession machinery are **not sufficient** for the
mortal chain at default life; the baker stage dies out across the generations.

**Re-scoped central question:** the immortal limit (life = ∞) functions; the mortal default
(life ≈ 27-tick tenure) collapses even with L2 and heavy succession. **Does raising
per-producer-house lifespan bridge that gap** — is there a finite life at which the mortal chain
sustains across deaths? The office-hours life/payback intuition, now with the stale-price confound
removed and both endpoints pinned (immortal sustains, mortal-default collapses).

**Base (all arms):** `frontier_mortal_producers_heritable` with `stale_input_price_fix = true`,
`mortal_chain_producers = true`, `mortal_producer_inheritance = true`,
`mortal_producer_tool_inheritance = true`, `food_provision = 0`, `producer_house_cap = 2`,
`SEEDS = [3,7,11,19,23]`. (Immortal is the cut-2 control that already sustains.)

**Swept axis:** per-producer-house lifespan × `{0.5, 1, 2, 4, 8}` (the §3.1 per-`HouseholdSpec`
lifespan override — still real code to build, with its digest/`HouseholdSpec`-guard obligations).

**Metrics = cut 2's** (not the superseded `StructurePersists`/`FlowRuns`): per (ratio, seed) — the
baker stage staffs to the final window, production sustains (`window_bread_produced ≥ floor`), and
the baker class stays **solvent** (gold floors > 0 over a long horizon), all **across real producer
deaths + successions** (assert deaths and heir-adoptions actually fire, so mortality is exercised).

**Pre-named outcomes:**
- **MORTALITY-TOLERANT** — the chain sustains (functions + solvent) at *every* ratio including the
  shortest life: L2 + the existing succession machinery cross the generation on their own; lifespan
  is not load-bearing. **Now UNLIKELY** — the measured mortal-default (short life) *collapses* even
  with L2 and heavy succession, so this would require the collapse to be seed/window noise, not a
  real short-life failure.
- **LIFESPAN-BAND** — a *minimum* lifespan is needed to sustain across deaths (short life →
  collapse, long life → sustain): the office-hours ratio hypothesis confirmed on a functioning
  substrate. Report the band.
- **MORTALITY-BREAKS-IT** — the *mortal* chain fails to sustain even with L2 (unlike the immortal
  control), at all tested ratios: death/succession is a wall L2 does not cross. A real negative.

`food = 0` stays (the cushion is C3R.b territory); the per-house lifespan mechanism, the
`subsistence_on_grain`/cap-fixed falsification guards, and the conservation/digest obligations
below all still apply. §§2, 3.2, 5 are re-read against these metrics and outcomes.

## −0. v2 revision (AUTHORITATIVE — folds the Codex spec-review; supersedes §§0–7 where they conflict)

**Reframed estimand (P0 + P1-denominator + P1-lineage).** This is NOT a "subsidy-free ratio"
experiment. It is: *at the C3R.b minimal-cushion floor* (`food_provision = 0` on the six
producer-house hearths, `producer_subsistence` RETAINED — exactly C3R.b's cleanest bracket, not a
zeroed base that starves producers), *does raising producer-house-lineage **absolute** lifespan
move the response toward `FlowRuns`?* The cushion is a **measured covariate**
(`producer_house_hearth_food_minted`, and a new `producer_subsistence`-minted tally), not a claim
of its absence. The swept quantity is **absolute integer lifespan**, published as an exact table;
the life/payback *ratio* is an interpretation applied **only post-hoc**, after realized payback is
measured (§7 assignment), and its absence does not block the experiment.

**Resolutions, P0 + P1 (each maps to a Codex finding):**

1. **[P0] Not subsidy-free → reframed + demand-viability control.** Drop every "first subsidy-free
   sustain" claim. Add a **positive demand-viability control**: an immortal / no-mortality cell on
   the same base that establishes `FlowRuns` is achievable on this substrate at all — so a null
   reads as "lifespan not binding," not "the substrate expired." Report cushion magnitude per cell.
2. **[P1] No denominator → absolute-lifespan experiment.** Sweep absolute
   producer-house lifespan over a pinned integer set; publish the lifespan table. Realized-payback
   measurement stays a prerequisite for the *ratio* narrative only, not for the run.
3. **[P1] Lineage, not producer, lifespan → declared estimand + coupled telemetry.** The estimand
   is producer-house-**lineage** lifespan. Add producer-house-scoped telemetry for the coupled
   variables the longer life perturbs: producer-house population, Consumer-role person-ticks in
   producer houses, births and deaths-by-cause, gold, food/cushion minted, inheritances, adoptions.
   The coupling is characterized, not assumed inert.
4. **[P1] Two-site implementation seam.** The override is consumed through a **household-aware
   lifespan helper** applied at BOTH assignment sites — seeded generation (`generation.rs:542`) AND
   birth (`phases.rs:532`) — or the treatment is lost after the first cohort. It scales the FULL
   `old_age_onset + span` distribution (not onset-only), with `founder_start_age` scaled/clamped so
   it can never meet or exceed a shortened lifespan, in checked integer arithmetic.
5. **[P1] Untreated producers in the response.** Ordinary Consumer/Gatherer lineages can build
   capital and adopt producer roles (`phases.rs:2217, 2813`); the override covers only the six
   tagged houses. Scope the response (staffing/output) to producer-house producers, and report the
   count of untreated adopter/latent producers so `FlowRuns` is attributable to treated lineages.
6. **[P1] Exhaustive, non-overlapping verdicts + both negatives pinned.** Preconditions
   (`BaseUnviable` / `ReservoirOpen` / `ConservationBroken` / `RegistryBroken`) are distinct from
   outcomes. Separately pin **RATIO-NULL** (`StructureDoesNotPersist` at every lifespan) and
   **STRUCTURE-ONLY** (`StructurePersistsUnderInheritance` but never `FlowRuns` at any lifespan) —
   neither is "no FlowRuns" alone. Name the mixed-seed, control-sustains, top-edge-only, and
   non-common-winning-ratio cases explicitly.
7. **[P1] Run-length confound.** ONE common horizon sized for the longest-life cell (not per-cell
   scaling, which confounds lifespan with calendar exposure). Normalize flow to **per-staffed-tick**
   (`bread_per_staffed_tick`, already emitted), not the absolute 100-loaf threshold. Gate every
   verdict on a **minimum count of completed succession events** (mill + oven deaths → inheritances
   → adoptions) per cell — elapsed lifespans are not handovers. The final window must be ≥ one
   longest-cell lifespan.
8. **[P1] Digest.** `HouseholdSpec` is NOT in the digest-coverage guard (confirmed: the guard
   destructures `DemographyConfig` at `digest.rs:2509`, not `HouseholdSpec`). Add a
   `digest_coverage_household_spec` guard. Encode the lifespan override **all-`None`-silent** (emit
   bytes only when `Some`, keyed by household index) so default-inherit is byte-identical, integer
   only. Tests: identity (all-`None` → unchanged), divergence (a `Some` splits), injectivity
   (distinct scales → distinct bytes).
9. **[P1] Control attribution.** The tool-off `Control` estimates the marginal effect of tool
   inheritance only; it does not remove cushion/demographic-demand/untreated-producer effects. The
   demand-viability positive control (res. 1) is what licenses the "lifespan is not binding"
   reading of a null.
10. **[P2] Scope + naming.** Pin `producer_house_cap = 2`; all conclusions are cap-conditional.
    Milestone renamed C3R.c → **C3R.f** (title above).

## 0. One-paragraph summary

C3R.b landed the full succession mechanism — producers are mortal, the mill/oven passes to a
household heir, and the heir assumes the producer role (`phases.rs:2360-2377`, firing 200–750×
per run). C3R.b's own result was an **inversion**: inheritance sustains chain *structure* (both
stages staffed) only while a producer-household hearth subsidy props the lineage, and that same
subsidy floods bread demand and *caps flow* (output). A fresh sweep (2026-07-17) confirms the
sharp edge: **at `food_provision = 0` the chain dies every seed; a healthy flow (`FlowRuns`)
occurred once in 80 runs, subsidized.** The one axis every C3R sweep has held fixed is producer
**lifespan** (`producer_mean_tenure ≈ 27` ticks in every cell). C3R.c sweeps it. The question:
does raising the life/payback ratio let a producer accumulate enough within one life that the
chain sustains at `food_provision = 0` — the first subsidy-free, healthy-flow mortal-producer
chain in the project — or does succession-at-longer-life still die without the demographic
subsidy, proving lifespan is not the binding constraint?

## 1. Base facts (verified 2026-07-17; cites at `1e10a8c`)

The succession machinery is **landed and fires**, contra the replan's "succession does not exist":

- Producer households: `MORTAL_PRODUCER_HOUSEHOLDS = 6` (`mod.rs:141`), tagged at
  `generation.rs:560-567`.
- Gates: `mortal_producer_inheritance_active` / `mortal_producer_tool_inheritance_active`
  (`mod.rs:9008-9022`); estate routes mill/oven to the heir (`demography.rs:303-336, 421`).
- Heir assumes role: `phases.rs:2360-2377` (`Consumer → Baker/Miller` on an inherited, held
  tool), instrumented `heir_tool_adoptions`.
- Subsystem test: `sim/tests/mortal_producer_inheritance.rs` (C3R.b v2), 5 seeds × 1600 ticks,
  `Control` / `FlagOffHeritable` twins; **asserts invariants only, never sustain**
  ("v2_sweep_prints_split_classification_without_verdict_assertions").

**Empirical baseline (the 2026-07-17 sweep, food × cap, lifespan fixed):**

| regime | cells | outcome |
|---|---|---|
| `food_provision = 0` | 10/10 | `StructureDoesNotPersist` (dies every seed; both-stage staffed ≈ 696/1600) |
| `food_provision = 1` | ~6 | `StructurePersistsUnderInheritance` / `FlowCapped` (staffed ≈ 1527/1600, output capped) |
| `food_provision ≥ 2` | 20 | `SubsidyFloodsChainDies` (free hearth food destroys bread demand) |
| `FlowRuns` (healthy), any cell | **1 of 80** | subsidized |

`producer_mean_tenure ≈ 27` in **every** cell — lifespan is fixed colony-wide and has never
been an axis. That ≈ 27 is the confirmed numerator; the payback is the missing denominator
(see §7 / the design doc assignment).

## 2. The central question and pre-named outcomes

**Q: Does raising the producer life/payback ratio produce a subsidy-free sustain?**

Sweep a producer-house lifespan scale spanning ratio ≈ 0.5×–8× the measured payback, **at
`food_provision = 0`** (the clean, currently-dying regime). Pre-named outcomes (name them before
the run so a null cannot be reframed as a win):

- **RATIO-BAND** — some ratio makes `food = 0` reach `StructurePersistsUnderInheritance` **and**
  `FlowRuns`. The first subsidy-free healthy mortal-producer chain; report the band and its
  edges. The strong positive.
- **STRUCTURE-ONLY** — longer life yields `StructurePersistsUnderInheritance` at `food = 0` but
  never `FlowRuns` (staffing without healthy output). Succession spans the generation but the
  chain still under-produces for a non-subsidy reason — a real finding that relocates the
  binding constraint off lifespan.
- **RATIO-NULL** — `food = 0` stays `StructureDoesNotPersist` at every tested ratio. Lifespan is
  **not** the binding constraint; C3R.b's subsidy-dependence is not a timescale artifact. A
  genuine negative — pin it (see §6, T1) so a later change can't silently flip it.

The honest prior: the machinery fires 200–750×/run and still dies at `food = 0` today, so
RATIO-NULL or STRUCTURE-ONLY is at least as likely as RATIO-BAND. State this in the result.

## 3. Mechanism (additive, default-off, conservation-safe)

### 3.1 The one generation change — per-producer-house lifespan

Lifespan is currently colony-wide: `DemographyConfig::lifespan_ticks(seed)` (`demography.rs:163`)
derives from top-level `old_age_onset_years` / `lifespan_span_years`; `HouseholdSpec`
(`demography.rs:30`) carries per-household `founders` / `time_preference_base_bps` /
`food_provision` / wood but **no lifespan**. Scaling the colony knob would lengthen *every*
lineage's life — including the consumer/buyer households — shifting the whole Malthusian balance
and confounding "producers live longer" with "the consumer economy changed". **The only clean
lever is a per-producer-household lifespan override.**

Add an optional per-`HouseholdSpec` lifespan scale (or an `old_age_onset_years` override),
`None` by default = inherit the colony lifespan. The producer-house tagging already isolates the
6 producer households (`generation.rs:560`), so the override applies to exactly them. A
test-level axis mutates the appended producer `HouseholdSpec`s (mirroring how C3R.b's
`mutate_producer_house_food` sets `food_provision`), holding consumer lineages at the colony
default.

### 3.2 The swept axis

`producer_lifespan_scale ∈ {0.5, 1, 2, 3, 4, 6, 8}×` the measured payback, at
`food_provision = 0`, `producer_house_cap` held at the C3R.b viable slice. Classify-not-tune:
the set is pinned and reported, not searched.

### 3.3 What is deliberately NOT wired (falsification integrity)

- **No skill transfer.** Bakers/millers have no skill state (`cultivation_skill` is the only
  skill field on `Colonist`); adding one is a new mechanic that would confound the lifespan
  answer. Out of scope; revisit only if a band exists and STRUCTURE-ONLY implicates skill.
- **No subsidy.** `food_provision = 0` throughout — the whole point is subsidy-free. Higher-food
  cells are C3R.b's territory, already characterized.
- **No cap sweep here.** Hold `producer_house_cap` fixed (§3.2) — lifespan and cap interact
  (longer life + fixed birth interval grows the producer-house population against the cap), so
  co-sweeping would re-confound. Cap stays a C3R.b axis.

## 4. Conservation & determinism

- The new per-house lifespan field is **behavior-steering** (it changes who dies when → future
  ticks), so it is classified **DIGESTED** in the digest-coverage guard (`settlement/digest.rs`)
  and gets a `canonical_bytes_include_producer_lifespan_*` test. The compile-time guard (impl of
  oik-e9l) forces the classification — a field added without it will not compile.
- Default-`None` (inherit colony lifespan) ⇒ every existing golden and digest byte-identical;
  verify against the C3R.b bases directly (mirror impl-63's
  `old_bases_are_byte_identical_and_tag_28_splits_heritable_config`).
- Conservation and money identity asserted per tick as in C3R.b's `trace_run` (both already
  span the full tick after oik-1ui).

## 5. Acceptance suite (`sim/tests/mortal_producer_inheritance.rs`, extended)

Reuse `trace_run` / `classify` / the `Trace` verdicts wholesale — they already emit
`StructurePersistsUnderInheritance` and `FlowRuns`. Add a `lifespan_scale` field to `Case`, thread
it through `heritable_cell`, and add the lifespan axis at `food = 0`.

**T1 — the asserting test the subsystem lacks (the core deliverable).** C3R.b's sweep pins
nothing. C3R.c must assert its outcome, both directions:
- If **RATIO-BAND**: assert `FlowRuns` at `food = 0` at the winning ratio across all seeds (the
  first pinned subsidy-free sustain), with the matched `Control` (tool-inheritance off) **not**
  persisting at the same ratio — the attribution.
- If **RATIO-NULL/STRUCTURE-ONLY**: assert `food = 0` fails to reach `FlowRuns` at **every**
  tested ratio, so an accidental future pass is caught. A negative must be pinned or it is not a
  result.

**T2 — RUN_TICKS must scale with lifespan (correctness trap).** `RUN_TICKS = 1600` at life ≈ 27
is 50+ generations; at ratio 8× (life ≈ 216) it is ~7 generations — too few deaths to
distinguish "sustains across handovers" from "the first cohort hasn't died yet". Scale
`RUN_TICKS` per cell to a fixed generation count (e.g. ≥ 30 producer lifespans), or the long-life
cells read as false sustains.

**T3 — keep the reservoir invariant.** `immortal_producer_max == 0 → ReservoirOpen` guards
against a lifespan scale that overflows into effectively-immortal; cap the sweep below that on
purpose.

`SEEDS = [3, 7, 11, 19, 23]` (match C3R.b). Print the full per-cell classification line (reuse
`print_trace`) so the band (or its absence) is legible in CI output.

## 6. Risks & open questions

- **May be negative.** RATIO-NULL is a real possible outcome; go in expecting it and pin it (T1).
- **Payback denominator.** The ratio is only meaningful once the oven/mill payback is measured on
  the base (design-doc §7 assignment). `producer_mean_tenure` and `producer_bread_output` in the
  existing `Trace` already give most of it; instrument the "starts-saving → oven-paid-off →
  surplus" interval before centering the sweep.
- **Lifespan×cap coupling** (§3.3) — held fixed here; a follow-up (C3R.d?) could co-sweep if a
  band exists.
- **Determinism of the scale field** — must be integer-derived (no float lifespan) to keep the
  `Colonist` state integer and the digest stable; express the scale as an integer
  `old_age_onset_years` override, not a float multiplier applied at runtime.

## 7. Falsifiable-bar summary

**Pass (a real result, either sign):** the suite pins one of the three pre-named outcomes at
`food = 0` across all seeds, with the matched control attributing any positive to
inheritance-at-ratio and not to the base. **Fail:** an unasserted "prints classification"
observatory (C3R.b's gap repeated), a positive claimed without the control contrast, or a
long-life cell judged sustained on too few generations (T2).
