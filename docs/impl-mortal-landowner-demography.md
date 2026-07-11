# impl-46 — S23d: Mortal-landowner demography base (make generational land tenure measurable)

**Status (impl): LANDED (rb-lite) — HONEST NULL `DemographyBaseUnviable` (all 5 seeds); the inheritance
vacuity is SOLVED but the base is subsidy-bound. NOT merged to master; S23c stays deferred.** The demographic
fix works: owners are mortal lineage reproducers (`immortal_roster_owned_plot_ticks == 0`,
`non_lineage_owner_plot_ticks == 0`), owners die of old age (`owner_old_age = 21–28`), and **inheritance fires
endogenously** in normal play (`inherit_eligible = 21–28 ≥ 20`, real heir transfers, no forced mortality) —
the exact S23c blocker (impl-45, `DisqualifiedNoInheritance`) is removed. Money promotes on `SelfProduced`
bread (`seeded_minted == 0`); born-in-sim agents grow up and own (`born_owners = 19–26`). **But the economy is
subsidy-dependent:** final-window consumption is `12768/12768 = 1.000` from the S21h own-labor emergency floor
and non-owner buyers buy nothing through money (`buyer_bought == 0`) → the ordered classifier returns
`DemographyBaseUnviable` (`final_floor_share > SUBSIDY_CAP` and material-bought-fail, independently). Codex
review-of-results: **ACCEPT-AS-HONEST-NULL** (no P0; verified this is a true structural null, not an artifact —
the floor runs *after* the market and subtracts market consumption before minting emergency units, so it does
not preempt purchases; everyone genuinely self-provisions, so no one buys). One P1 folded in: the
`min_lineage_after_cold_start` cutoff conflated the generation-count const with a tick offset — fixed to a named
`COLD_START_TICKS` (= one mean adult lifespan); verdict unchanged (`min_lineage` stays 0, so the lineage
collapse is real, not a cold-start transient). Full workspace green, `fmt`/`clippy` clean, goldens
byte-identical off. **Finding:** making landowners mortal-and-reproducing removes the inheritance vacuity, but
*this* composed base — with an own-labor survival floor — is subsistence-bound: cultivator-owners' grain is
never traded, so the producer/buyer money economy does not persist. The honest next step is a **lever** (tighter
survival floor / scarcity margin that forces exchange), NOT tuning S23d's floor to pass. Preserved unmerged on
`feat/mortal-landowner-impl-rb`; S23c re-run stays deferred pending a viable base.

Status (spec): **SPEC-READY** (Codex spec-review round 1 folded in — P0 owner-identity strengthening, P1
generation-turnover + classifier-order + substrate wording, P2 inherit-count framing + `no_lineage_land_claims`
control + partial-failure reporting; see §9). Base: master `855be95` (S24c landed; property arc S23a/b
plus institution-selection arc S24a/b/c published). This is a **base-building milestone, not a lever test**:
it does not ask a new economic question, it builds the demographic substrate that a *later* re-run of S23c
(secure land tenure, impl-45) needs in order to be non-vacuous. Scoped by Codex ("rank B > C > A; commit to B;
spec `mortal-lineage-landowner-demography-base` as option B, standalone-then-S23c"). Sequencing is
**standalone first**: land and tenure policy are *not* introduced here — this milestone only proves that a
colony whose land-owning cultivators are themselves **mortal and reproducing** can survive across generations
with money and a two-tier economy. S23c (impl-45) is then re-run on this frozen base.

## 0. One-paragraph summary

S23c (impl-45) built and verified a secure-tenure inheritance engine (universal heirs, non-forfeiting title,
partible fractional shares) — and then found it **vacuous on the current base**: `DisqualifiedNoInheritance`,
0 inheritance events in normal play. The reason is structural, not a bug. OIKOS today runs **two disjoint
populations**: (a) an **immortal standalone cultivator roster** (`lifespan = None`, `household = None`) that
persistently cultivates and — under S23a — claims and owns land but **never dies or reproduces**; and (b)
**mortal reproducing lineage households** (hearth-fed consumers) that age, die, and bear children but **never
persistently cultivate or claim land**. So the set of land-owners and the set of reproducers **do not
intersect**: owners are immortal, and the mortals own nothing to bequeath. No inheritance of land can ever
fire. This milestone removes that split by making the **mortal reproducing lineage households themselves the
persistent cultivator-owners** — composing the S21f production-for-barter seam (households cultivate
`SelfProduced` bread and barter pre-money) + the S21h demand-side survival bridge (which lets the demand cohort
survive the S21g cold-start cull) + the S23a homesteading-claim machinery (cultivators own the plot they work)
with the mortality/reproduction the lineage households **already have**. When the land-owning cultivators are
mortal and reproducing, their **old-age deaths fire the inheritance engine** and land passes to heirs — which
is exactly what S23c needs. **The success bar here is tenure-independent**: no tenure *policy* is compared;
success is only that this colony is demographically and economically viable across ≥5 generations with money
and a surviving two-tier economy. Everything is behind a default-off flag `mortal_landowner_demography`
(digest tag 19, ON-only); goldens are byte-identical when off. **Anti-smuggling: base parameters are frozen
on the viability bar below *before* S23c measures any tenure outcome — this milestone is tuned for demographic
survival, never for concentration, inheritance counts, or a nice landed class.**

## 1. Why this milestone, why now — and the exact structural gap

The role-topology arc (S22) and the property arc (S23a/b) both ran a scarce productive factor against a fixed
cast of cultivators. Every generational question about land — does a plot passed to heirs concentrate or
fragment, does partible inheritance drive Malthusian morcellement, does secure title let a landed lineage
persist — presumes that **the people who own land are the people who die and have children.** In OIKOS they are
not. The S23c disqualification made the gap concrete and measured it:

**Grounding (verified during S23c, impl-45):**
- Land owners in the S23a/S23c engine are drawn from the **immortal standalone cultivator roster**
  (`lifespan = None`): they cultivate every tick, so under homesteading they out-compete transient cultivators
  for persistent plot ownership. They never age out → `old_age_deaths` among owners ≈ 0.
- The **mortal lineage founders** are hearth-fed consumers. S22a (`endogenous_cultivation_entry`) lets them
  cultivate *fluidly under hunger*, but they do not hold a plot persistently, so they rarely become the
  registered owner, and when they die they own nothing.
- S23c instrumentation on the shipped base: births ≈ 0 among owners, `old_age_deaths` among owners = 0,
  **0 inheritance events across 306 death traces.** The heir engine is correct (verified by forced-mortality
  probes) but **never invoked** in normal play → `DisqualifiedNoInheritance`.

**The fix (Codex option B, ranked B > C > A):** make the mortal reproducing lineage households the *persistent
cultivator-owners*, rather than mutating the immortal roster into a demographic actor it was never built to be
(A), or unifying the whole population model in one rewrite (C). B is the smallest credible change and it
**reuses machinery that already exists and is verified**:
- **S21f** (`household_production_barter`) — households cultivate `SelfProduced` bread and barter pre-money;
  this is the persistent-household-cultivation seam. *Supply question closed.*
- **S21h** (`demand_side_survival_bridge`) — the produced own-labor emergency survival floor that carried the
  demand cohort through the S21g cold-start cull so money and mortality coexist.
- **S23a claim machinery** — homesteading claim / owner-only harvest / plot registry (used with idle-forfeiture
  **off**, i.e. the secure variant, so this base does not re-introduce the S23a thrash).
- **Lineage mortality + reproduction** — already present on the household model (hearths, lineages, births,
  old-age death, `settle_estate_to_heirs`).

Composed, the persistent cultivator-owners become mortal and reproducing, so their old-age deaths **fire the
inheritance engine endogenously.** This milestone's whole job is to prove that composition is *viable* — that
it does not collapse at cold start, that money still promotes, and that a two-tier economy survives across
generations — **before** S23c uses it.

## 2. The central question and pre-named outcomes (tenure-independent)

**Central question.** With the mortal reproducing lineage households configured as the **persistent
cultivator-owners** (they cultivate `SelfProduced` bread, homestead and own the grain plot they work, age, die
of old age, reproduce, and bequeath their plot via the verified heir engine), and with **no tenure policy under
comparison**, does the colony reach a **demographically and economically viable generational regime** —
surviving the S21g cold-start cull via the S21h bridge, promoting and keeping money, sustaining a two-tier
producer/buyer economy across **≥5 generations**, and generating **≥20 inheritance-eligible owner deaths with
living heirs** — while conservation, provenance, and the land/registry invariants all hold?

**Primary success = `DemographyBaseViable`** (all clauses; predeclare every threshold as a `const`, do NOT
fit). Measured on the `mortal_landowner_demography = true` headline vs the `= false` control:

1. **Owners are lineage-household reproductive actors — not the immortal roster, and not mortal shells**
   (Codex P0). Every registered land-owner across the run is a mortal lineage household that is a *reproductive
   actor*: it has `lifespan = Some`, `household.is_some()`, `lineage_id.is_some()`, is `reproduction_eligible`,
   and appears in the lineage birth/kinship graph. Enforced by three counters that must all hold:
   `immortal_roster_owned_plot_ticks == 0` **and** `non_lineage_owner_plot_ticks == 0` **and**
   `owner_old_age_deaths > 0`. `immortal_roster_owned_plot_ticks == 0` alone is **necessary, not sufficient** —
   a mortal non-reproducing shell or a transient hunger-cultivator could satisfy `lifespan = Some` while still
   leaving the owner and reproducer sets disjoint. *(Kills the disjoint-population residue at its root.)*
2. **≥5 generations elapse with real cohort turnover** (Codex P1). Sim length spans ≥ `GENERATIONS_MIN` (5) ×
   mean adult lifespan, **and** living lineage cultivator-owners are present throughout (no generation gap
   where the owner class empties), **and** — so "five generations" is *turnover*, not merely elapsed founder-
   lifetime — at least `BORN_IN_SIM_OWNER_COHORTS` (≥1, target ≥3) distinct **born-in-sim** agents reach
   adulthood and themselves own+cultivate a plot (`born_in_sim_owner_count ≥ BORN_IN_SIM_OWNER_COHORTS`). Heirs
   merely *existing* at a death does not satisfy this — a child born in-sim must grow up and become a
   cultivator-owner.
3. **Endogenous replacement.** Reproduction sustains the lineage without immortal top-up: cumulative
   `lineage_births ≥ BIRTHS_MIN`, and the living-lineage count never falls below `LINEAGE_FLOOR` after cold
   start (the population turns over, it does not merely coast on the founders).
4. **≥20 inheritance-eligible owner deaths with living heirs — a non-vacuity / sample-floor gate only**
   (Codex P2). `inherit_eligible_owner_deaths ≥ 20` — an owner who died of old age holding a plot with ≥1
   living heir at death (the event the heir engine consumes). **Only the eligibility *count* is gated here** —
   never concentration, share distribution, heir-count shape, landed-class persistence, or fragmentation, all
   of which are S23c's to measure on the frozen base. *(This is the clause S23c needs; it must be reached
   without any tenure policy on.)*
5. **Cultivators claim, own, and cultivate before any tenure evaluation.** `owner_claims ≥ CLAIMS_MIN`, plots
   are harvested owner-only, and the owner cohort produces `SelfProduced` bread (not seeded/minted supply).
6. **Money promotes and stays money post-cold-start.** SALT promotes on `SelfProduced` bread
   (`seeded_minted == 0`), remains the money good to run-end, and food is materially bought after promotion
   (`post_promo_bought ≥ MATERIAL_BOUGHT_FLOOR`).
7. **A two-tier economy survives ≥5 generations, not subsidy-dependent.** Both a producer/owner-cultivator tier
   and a non-owner buyer tier persist across the ≥5-generation span (buyers survive and materially buy), and
   the economy is **not living off the emergency floor**: emergency-floor-sourced consumption ≤ `SUBSIDY_CAP`
   (0.5) of total consumption in the final window (the S21h floor is a cold-start *bridge*, not the diet). The
   final-window floor share is additionally **reported** (a stricter observability metric, not a gate) so a
   marginally-passing base is visible rather than silently near the cap.
8. **Conservation, provenance, and the land/registry invariants hold every tick.** Grain/bread/money conserve
   (ownership and inheritance are metadata transfers, nothing created/destroyed); each plot has ≤1 owner; every
   claim/abandon/inherit transition preserves the finite plot set; no negative balances; no orphaned claims; no
   duplicated ownership; sold/pre-promotion bread is `SelfProduced`.

**Finding modes (pre-named, first-class; the verdict test prints the classification and does NOT assert
SUCCESS):**
- `DemographyBaseUnviable` — the honest cold-start / sustainability failure Codex named: colony extinction,
  cold-start cull that does not recover, barter/money never bootstraps, or **subsidy-dependence** (emergency
  floor > `SUBSIDY_CAP` of consumption in the final window). The composition cannot reach positive
  food/money circulation before old-age deaths + reproduction pressure hit.
- `NoGenerationalTurnover` — colony survives with money but owners do not die of old age / no inheritance-
  eligible deaths accrue (`inherit_eligible_owner_deaths < 20`): the S23c vacuity **reproduced** — the base did
  not actually make owners mortal reproducers in practice.
- `ImmortalOwnerResidue` — some land is owned by the immortal roster for a sustained window
  (`immortal_roster_owned_plot_ticks > 0`): the disjoint-population split persists; owners are not all mortal
  reproducers.
- `NonLineageOwnerResidue` (Codex P0) — some land is owned for a sustained window by a *mortal* agent that is
  not a reproductive lineage household (`non_lineage_owner_plot_ticks > 0`): a mortal shell or transient
  hunger-cultivator holds title, so the owner set is still disjoint from the reproducing set even though no
  immortal owns. Distinct from `ImmortalOwnerResidue` — the owner is mortal but not a reproducer.
- `MoneyFailure` — SALT never promotes, or demonetizes, under the mortal-owner composition.
- `ConservationBroken` / `extinct` — any conservation, provenance, or registry-invariant break, or colony
  death.
- `DemographyBaseViable` — all eight clauses pass; the base is frozen for the S23c re-run.

**Ordered classifier (top-down, first-match-wins — S21i non-gameability discipline).** Structural owner residue
is caught **before** generic viability (Codex P1), so a run that survives *because* an immortal or non-lineage
actor owns land is exposed as residue, not laundered as merely unviable:
`ConservationBroken`/`extinct` → `ImmortalOwnerResidue` / `NonLineageOwnerResidue` → `MoneyFailure` →
`DemographyBaseUnviable` (extinction / no-recovery / subsidy-dependence) → `NoGenerationalTurnover` → **then the
explicit final gate:** `if ALL EIGHT clauses (§2.1–§2.8) pass { DemographyBaseViable } else { report the first
failed clause }`. Predeclare every threshold as a `const`; do not fit.

## 3. Engine design (additive, default-off, conservation-safe)

**One flag gates the whole composition:** `mortal_landowner_demography: bool` (default `false`). When `false`,
the current base is untouched and all goldens are byte-identical. When `true`, the base is assembled from the
already-verified seams — this milestone is mostly **composition + wiring + measurement**, not new economics.

**What ON changes (all sim-side; `world` crate and goldens untouched when off):**
1. **Persistent cultivator-owner identity = the mortal lineage households.** Enable the S21f
   production-for-barter household cultivation and the S22a hunger-entry seam for lineage households, and route
   the S23a homesteading claim so that the mortal household that persistently works a plot is its registered
   owner. The immortal standalone roster is **not** the persistent owner in this regime — either it is not
   spawned as a land-cultivator under the flag, or its claims are disallowed (implementer's choice; the invariant
   is `immortal_roster_owned_plot_ticks == 0`, §2.1).
2. **A fixed non-comparative ownership substrate (no tenure regime is compared here)** (Codex P1). Plots use
   the S23a claim/owner-only-harvest/registry machinery with **idle-forfeiture off** and **no market**. This is
   a tenure *substrate*, not a tenure *comparison*: a single fixed minimal non-forfeiting ownership rule exists
   **solely so that bequeathable plots exist**. This base does not compare forfeiture (S23a) vs market (S23b)
   vs secure (S23c) — that comparison is entirely S23c's job on the frozen base.
3. **Inheritance uses the verified S23c heir engine.** On an owner's death, `settle_estate_to_heirs` routes the
   plot via the impl-45 universal-heir order (live child → nearest live kin → household successor → colony
   next-of-kin → revert unowned). This is the engine S23c already built and probe-verified; here it fires
   **endogenously** because owners are mortal.
4. **Cold-start survival via the S21h bridge.** The produced own-labor emergency survival floor is on so the
   demand cohort survives the S21g cull while cultivation/barter/money bootstrap. Honest failure mode: if the
   composition cannot reach positive food/money circulation before old-age deaths + reproduction pressure hit,
   the colony collapses or goes subsidy-dependent → `DemographyBaseUnviable` (see §7).

**Digest / canonicalization discipline.** `mortal_landowner_demography` is canonicalized **ON-only** under a
new injective **digest tag 19** (13 = S23a, 14 = S23b, 15/16/17 = S24a/b/c, 18 = S23c secure-tenure,
19 = this). When the flag is off, no field is emitted and `canonical_bytes` / digest are byte-identical to the
current base. New measurement counters (`immortal_roster_owned_plot_ticks`, `inherit_eligible_owner_deaths`,
`lineage_births`, etc.) are test-visible telemetry, not canonicalized behaviour.

**Conservation and invariants.** Grain/bread/money conservation is asserted every tick as today. Ownership and
inheritance are **metadata transfers** — no good is created or destroyed by a claim, abandon, death, or heir
routing. Plot-registry invariant: each plot has ≤1 owner; the finite plot set is preserved across every
transition; no orphaned claims; no plot silently unowned (on heir-failure it reverts to unowned, logged).

## 4. The new suite `sim/tests/mortal_landowner_demography.rs`

Deterministic, seed-spined `{3, 7, 11, 19, 23}` headline (S22f-style robustness seeds only if needed). Tests:
- `goldens_unchanged` — every golden byte-identical with the flag off (the canonicalization guard).
- `canonical_bytes_split_only_when_demography_active` — canonical bytes/digest diverge from base **only** when
  the flag is on (tag 19 injectivity).
- `demography_base_viable_headline` — on the headline cell, classify `DemographyBaseViable` (or print the first
  failed clause); assert the classification, not a hand-set outcome.
- `owners_are_lineage_reproducers` — `immortal_roster_owned_plot_ticks == 0` **and**
  `non_lineage_owner_plot_ticks == 0` **and** `owner_old_age_deaths > 0`, and every owner has
  `lifespan = Some`, `household.is_some()`, `lineage_id.is_some()`, `reproduction_eligible` (§2.1, the Codex-P0
  strengthening — `lifespan = Some` alone is not asserted as sufficient).
- `born_in_sim_owner_reaches_ownership` — `born_in_sim_owner_count ≥ BORN_IN_SIM_OWNER_COHORTS`: at least one
  agent *born during the sim* grows to adulthood and becomes a cultivator-owner (§2.2 cohort-turnover, distinct
  from heirs merely existing at a death).
- `inheritance_fires_endogenously` — `inherit_eligible_owner_deaths ≥ 20` and heir routing actually transfers
  plots in normal play (the clause S23c needs; contrast the S23c-base probe which needed forced mortality).
- `money_promotes_and_persists` — SALT promotes on `SelfProduced` bread, `seeded_minted == 0`, stays money,
  material bought after.
- `two_tier_not_subsidy_dependent` — buyer tier survives + buys; emergency-floor consumption ≤ `SUBSIDY_CAP`.
- `conservation_and_registry_invariants_hold` — grain/bread/money conserve every tick; ≤1 owner/plot; no
  orphaned claims; no duplicated ownership.
- **Controls (each isolates a load-bearing seam):**
  - `demography_off` → the current base → `NoGenerationalTurnover` / `ImmortalOwnerResidue` reproduced (proves
    the flag is what makes owners mortal reproducers).
  - `no_reproduction` → births suppressed → lineage dies out → `DemographyBaseUnviable` (reproduction is
    load-bearing for the generational span).
  - `no_emergency_floor` → the S21h bridge off → the S21g cold-start cull reappears → `DemographyBaseUnviable`
    at cold start (the bridge is load-bearing for survival).
  - `no_lineage_land_claims` (a.k.a. `legacy_roster_claims_only`, Codex P2) → lineage households may still
    cultivate, but the homesteading-claim reroute to them is disabled so only the legacy roster can own →
    reproduces `NoGenerationalTurnover` / `ImmortalOwnerResidue`. This isolates the actual S23d seam — mortal
    households becoming *persistent land claimants* — which the other three controls (reproduction, survival
    floor) do not touch.

## 5. Verification (independent gate)

The orchestrator's verification + Codex review-of-results is the gate (never the rb-lite panel's "clean"). All
must hold on the landed diff:
- Full workspace green; new suite passes; `cargo fmt --check` + `cargo clippy` clean.
- **All goldens byte-identical** with the flag off (the ON-only tag-19 discipline).
- The headline classifies `DemographyBaseViable` on the predeclared bar, and the three controls classify as
  above (the seams are shown load-bearing, not decorative).
- Conservation / provenance / registry invariants asserted, not assumed.

## 6. Honesty and scope (state these in the result; do not let a viable base overclaim)

- **This proves a base is viable, not a lever finding.** `DemographyBaseViable` says only "a mortal-reproducing-
  landowner colony survives across generations with money and a two-tier economy." It makes **no** claim about
  land tenure, concentration, inheritance regimes, or occupational stickiness — those are S23c's questions, on
  this frozen base.
- **Anti-smuggling (Codex rule 5).** Freeze all base parameters on §2's viability bar **before** S23c measures
  any tenure outcome. Tune only for demographic/economic viability (survival, money, two-tier persistence),
  **never** for concentration, inheritance counts, morcellement, or a landed-class result. A base tuned to make
  a nice landed lineage would smuggle S23c's answer into its premise.
- **If it fails, it fails honestly.** `DemographyBaseUnviable` (cold-start collapse / subsidy-dependence) or
  `NoGenerationalTurnover` are first-class outcomes — if the mortal-owner composition cannot reach a viable
  generational regime, that is itself the finding (generational land tenure is not measurable in OIKOS without
  a deeper population-model change, i.e. Codex option C), and S23c stays deferred rather than run on an
  unviable base.

## 7. Cold-start fragility — the predeclared failure mode (S21g / S21h)

The known risk is a re-trigger of the **S21g cold-start cull**: mortal cultivator-owners that enter as
hearth-fed consumers before cultivation, barter, and money demand stabilize can be culled at ~tick 7 (as in
S21g) before the economy forms. The design composes the **S21h demand-side survival bridge** precisely to
carry them through. The honest, predeclared failure mode: **if the mortal cultivator households cannot reach
positive food/money circulation before old-age deaths and reproduction pressure arrive, the economy collapses
or becomes subsidy-dependent** — classified `DemographyBaseUnviable`, not worked around. The `no_emergency_floor`
control demonstrates the bridge is load-bearing; the `SUBSIDY_CAP` clause (§2.7) demonstrates the bridge is a
*bridge*, not the diet.

**Partial-composition failures are reported as the first failed clause** (Codex P2), not swept into a single
`DemographyBaseUnviable`, so a base that half-works is diagnosable: `OwnerClassGap` (owners disappear
temporarily but the colony survives — fails §2.2's "present throughout"); `NoBuyerTier` (owners + money survive
but the non-owner buyer cohort vanishes — fails §2.7); `NoLineageReplacement` (births occur but no born-in-sim
agent reaches adulthood + ownership — fails §2.2's cohort-turnover clause). These are reported failed-clause
labels, not new named verdicts.

## 8. Sequencing — standalone base first, then S23c re-run (Codex rule 4)

1. **This milestone (S23d) lands standalone.** Do **not** combine it with tenure policy. Verify the mortal-
   reproducing-landowner colony is `DemographyBaseViable` across ≥5 generations with money and a surviving
   two-tier economy. Freeze its parameters.
2. **Then re-run S23c (impl-45) on this frozen base** with `mortal_landowner_demography = true` +
   `secure_land_tenure = true`, so the secure-tenure inheritance study (impartible vs partible, Malthusian
   land-per-capita) runs against endogenous owner deaths and real heirs — the study that was
   `DisqualifiedNoInheritance` on the old base. S23c's own success bar (impl-45 §2) is unchanged; only its base
   changes.

## 9. Codex spec-review resolutions (round 1 — folded in; verdict: SPEC-READY after these edits)

- **P0 — owner identity too weak.** `lifespan = Some` did not prove an owner is a *reproducing lineage
  household* (a mortal non-reproducing shell could satisfy it and preserve the exact vacuity). §2.1 now requires
  `household.is_some()` + `lineage_id.is_some()` + `reproduction_eligible` + presence in the birth/kinship
  graph, enforced by `immortal_roster_owned_plot_ticks == 0` **and** `non_lineage_owner_plot_ticks == 0`
  **and** `owner_old_age_deaths > 0`. Added finding mode `NonLineageOwnerResidue`; strengthened the suite test.
- **P1 — generation bar underspecified.** §2.2 now requires ≥`BORN_IN_SIM_OWNER_COHORTS` born-in-sim agents to
  reach adulthood and become cultivator-owners (turnover, not elapsed founder-lifetime); added the
  `born_in_sim_owner_reaches_ownership` test.
- **P1 — classifier order.** `ImmortalOwnerResidue` / `NonLineageOwnerResidue` now precede `MoneyFailure` /
  `DemographyBaseUnviable`, so survival *via* structural residue is exposed, not laundered as merely unviable.
- **P1 — "tenure policy OFF" wording.** §3.2 reworded to "a fixed non-comparative ownership substrate … solely
  so bequeathable plots exist" (a substrate, not a comparison).
- **P2 — inherit-eligible count.** §2.4 states it is a non-vacuity / sample-floor gate only — never
  concentration, share, heir shape, landed-class, or fragmentation (all S23c's to measure).
- **P2 — missing control.** Added `no_lineage_land_claims` (legacy-roster-claims-only) to isolate the
  ownership-reroute seam; reproduces `NoGenerationalTurnover` / `ImmortalOwnerResidue`.
- **P2 — partial-composition failures.** §7 now reports `OwnerClassGap` / `NoBuyerTier` / `NoLineageReplacement`
  as first-failed-clause labels; §2.7 additionally reports the final-window floor share as observability.
