# impl-63 — C3R.b: Capital Inheritance for Mortal Chain-Producers (does the mill passing to an heir lift the C3R.a collapse?)

Status (spec): **v2 — SPEC-READY** (revised after the v1 result-review REJECT; Codex v2-review NEEDS-REVISION →
4 P1s folded into the authoritative `## −0` section: sweep `{0,1,2,3}` × cap `{1,2,3}`; producer-house-scoped +
demand-side + bread-per-staffed-tick telemetry; split verdict `StructurePersistsUnderInheritance`/`FlowCapped`
with the `ControlDidNotCollapse` disqualifier removed; food_provision as a test-level axis, cushion RETAINED,
byte-identity verified against the OLD bases directly). The v1 build
landed mechanically-clean but the result-review rejected it as a scientific gate: the producer-household
**hearth floods bread demand** and floors the chain to bread=0 *before* inheritance can be evaluated (a
`producer_house` `food_provision=3` mints so much free food that the market dies). Diagnostic sweep proved
it — at `food_provision=1` the chain **runs** (`InheritanceCell` → `ChainPersistsUnderInheritance`,
both-stage-staffed 1489 ticks; `Control` bread 1869). And it surfaced the real finding — an **inversion**:
inheritance buys continuous *staffing* but demand-capped *output* (bread 9), because inheritance sustains
the producer households → they reproduce heavily (357 newborns vs the control's 16) → the newborns' hearth
floods demand → output stays capped; re-building (control) lets the households die back → less flood → real
output (1869). So v2 (a) makes the hearth subsidy a **swept** parameter (classify-not-tune, the v1 sin was
pinning `food_provision=3`), (b) adds telemetry to characterize the **structure-vs-flow inversion**
(hearth-food minted, per-producer output, producer tenure, population trajectory), and (c) revises the
verdict ladder to be subsidy-dependent. **v1 status (superseded): SPEC-READY** (Codex xhigh, 1 round, no
P0/P1 on the mechanism; the confound was a scientific-inference gap the *result*-review caught, not a code
defect). The second slice of C3R (the keystone: a mortal production chain).

## −1. The v1→v2 pivot (the confound and what it revealed)

**v1 was confounded, not wrong.** The estate→heir→re-adopt mechanism works perfectly (v1: 294/294 tools
inherit, 0 heirless, ~291 heir-adoptions) — but on the v1 base the chain was already dead (bread=0) from the
household hearth flooding demand, so "inheritance is insufficient" was unattributable. The diagnostic (drop
`food_provision` 3→1) recovered the chain, proving the hearth subsidy — not inheritance — was the
response-variable floor. This is the program's recurring demand-side lesson (a free-food subsidy large
enough to flood demand kills the market — S23d/S23e) reappearing inside the keystone.

**The real finding (v2's target): a structure-vs-flow inversion.** At a viable subsidy, inheritance
*sustains the chain's structure* (both milling and baking stages continuously staffed — the keystone's first
positive) yet caps its *flow* (output), because the mechanism is self-loading on the demand side: inheritance
keeps the producer households populated → they keep reproducing → the newborns are hearth-fed Consumers who
flood bread demand → output is suppressed. Re-building (the inheritance-denied control) lets the producer
households die back, so it produces *more* bread but staffs the stages only intermittently. Having capital
continuously (inheritance preserves tool + role) is not the same as using it productively (Böhm-Bawerk):
inherited tenure sustains occupancy but the demographic subsidy it rides caps output. v2 characterizes this
across the subsidy sweep.

## −0. v2 acceptance shape (AUTHORITATIVE — supersedes §2/§4/§7 below wherever they conflict; folds Codex v2-review P1s)

**Two swept axes (classify-not-tune — both pinned sets, reported, NOT searched; the v1 sin was pinning
`food_provision=3`):**
- `producer_house_food_provision ∈ {0, 1, 2, 3}` — the per-member hearth subsidy. `0` = the no-producer-hearth
  bracket (producers fed only by the retained cushion — see below); if `0` starves producers or prevents heirs
  it classifies `BaseUnviable`/no-hearth, NOT a failed tuning point. Included because `food_provision=1` still
  left inheritance bread ≈ 9 (partial flood), so the bracket must reach the un-subsidised end.
- `producer_house_cap ∈ {1, 2, 3}` — the reproduction/population axis, to separate population-flood from
  per-capita subsidy. **`cap=1` is a no-heir / no-reproduction control** (a producer with no room for a
  child-heir → no births → no population flood; the mill sinks to commons on death, so the chain can only
  re-build); `cap=2` is the minimal inheritance-capable case; `cap=3` widens population at fixed subsidy.
- **Source of truth = a TEST-LEVEL axis** that mutates the appended producer `HouseholdSpec`s'
  `food_provision` and the producer-house cap per cell — NOT new `ChainConfig` fields. Demography config bytes
  already serialize `food_provision` (mod.rs:27166) and the cap rides tag 28, so no digest-layout change; each
  (food_provision, cap) pair is simply a distinct scenario. **Cushion note:** because `food_provision=0` must
  not starve producers, v2 KEEPS `producer_subsistence` (diagnostic ruled it out as the confound — restoring it
  did not recover the chain at high subsidy) instead of zeroing it; the double-provision it was zeroed to avoid
  is itself a *subsidy* now measured by the sweep (at `food_provision=0` only the cushion feeds; at 3 both do).

**Per-cell triad** (C3R.a baseline / inheritance-denied control / inheritance cell) at each (subsidy, cap)
point; `SEEDS=[3,7,11,19,23]`.

**Verdict ladder (v2 — split structure from flow; the `ControlDidNotCollapse` DISQUALIFIER of §2/§4/§7 is
REMOVED):**
```
Preconditions: BaseUnviable (deaths==0, or food_provision=0 starves) / ReservoirOpen (immortal_producer_count>0)
               / ConservationBroken / RegistryBroken
Subsidy regime:
  SubsidyFloodsChainDies — the hearth demand-flood floors bread≈0 across cells regardless of inheritance
                           (the v1 food_provision=3 regime). A demand-side finding, not an inheritance one.
Viable-subsidy reading (STRUCTURE and FLOW reported SEPARATELY, not collapsed to one pass/fail):
  StructurePersistsUnderInheritance — both milling+baking stages stay jointly staffed to the final window in
                           the inheritance cell (the keystone's first structural positive). Does NOT require
                           non-trivial output.
  FlowCapped / FlowRuns — an ORTHOGONAL axis on producer_bread_output (and bread-per-staffed-tick): FlowCapped
                           when structure persists but output is demand-suppressed (the inversion); FlowRuns
                           when output is non-trivial.
```
The control is a MATCHED COMPARISON, not a disqualifier: a control with high *output* but weak *structure*
(re-building: bread high, staffing intermittent) is the informative contrast to the cell's structure-high /
flow-capped; the control only undercuts a structural claim if it *also* structurally persists without
inheritance. The headline is the **structure-vs-flow inversion reported across the (subsidy, cap) grid** —
explicitly meaning "inheritance keeps the stages staffed but the subsidised households it sustains suppress
market demand, capping output" — not a single verdict.

**Telemetry (v2 — runtime-only, OUT of canonical_bytes; folds the P1 telemetry gap):**
- Subsidy magnitude, producer-house-scoped: `producer_house_hearth_food_minted` (NOT the global endowment) +
  `non_producer_hearth_food_minted` (the lineage baseline) — the actual injected free food.
- Demand side: bread late-window realized price / trade count, `bread_consumed`, `bread_bought` — to show
  demand is suppressed by the flood.
- Flow denominator: `producer_bread_output`, `both_stage_staffed_ticks`, `producer_role_ticks`, and
  **bread-per-staffed-tick** (output normalized by staffing — the direct inversion metric).
- Population/reproduction: cumulative `producer_house_births` / `producer_house_deaths` /
  `producer_house_person_ticks`, `producer_mean_tenure` (ticks holding the role before death/handoff) — to
  tie low output to sustained population (the 357-vs-16 newborn gap).
- Rejection accounting: recipe-pay / build / adoption rejection counts — to show WHY output is low (demand
  gate), not a wiring failure.
- Carried forward: `producer_tool_inheritances`, `heirless_producer_deaths`, `heir_tool_adoptions`,
  `immortal_producer_count` (guard=0), `mortal_producer_old_age_deaths`, era.

**Byte-identity (v2 — folds the P1 evidence fix):** verify the OLD bases DIRECTLY — `frontier`,
`frontier_capital`, `frontier_mortal_producers` bytes unchanged (do NOT use the flag-off heritable base as
byte-identity evidence, since it carries the appended producer households). Tag-28 canonical-split test stands.

**Everything below (§1–§9) stands as the v1 mechanism spec** (households, estate seam, S7 re-adopt, the
matched control, tag 28) — v2 only adds the two-axis sweep + the inversion telemetry + the split verdict on
top, and RETAINS `producer_subsistence` (does not zero it). Build base: branch **`feat/mortal-producers-impl-rb` @ `d8e0ddc`** (C3R.a — mortal
chain-producers, the landed `ChainCollapsesOnProducerDeath` null). Flag **`mortal_producer_inheritance`**
(bool on `ChainConfig`), gated `mortal_producer_inheritance_active() = flag && demography.is_some() &&
chain.is_some() && mortal_chain_producers` (composes ON TOP of C3R.a's mortality). Digest **tag 28**
(code-verified free — the chain-runtime tag stream tops out at 27) — payload = the two flag bytes
(`mortal_producer_inheritance`, `mortal_producer_tool_inheritance`) + the producer-house cap. New base
`frontier_mortal_producers_heritable()` deriving from `frontier_capital()`. C3R.a's
`frontier_mortal_producers()` is **left untouched → byte-identical**.

Falsifiable bar (headline): C3R.a proved the chain **collapses** when producers die — because the vacated
role is refilled only by frantic re-adoption and, decisively, each dead producer's **mill sinks to the
commons** (heirless, `household: None`), where it is permanently unusable, so the next mortal must rebuild a
16-cycle-payback tool from scratch (2–3 builds vs ~150 deaths — the payback horizon). C3R.b closes exactly
that capital-destruction channel: give the mortal producers **reproducing households** so a dead producer's
mill routes to a live **heir** (the existing estate seam, unchanged), who — now holding the mill — adopts
the vacated role via the **existing** mortal-gated S7 tool-holder path (unchanged). The question: **does
carrying the capital across the generational gap lift the collapse** — a standing chain forming on mortal
producers — or does it **still collapse** because the heir-timing and recipe-must-pay gaps bind?

## 0. One-paragraph summary

The C3R research established (probes a39f0e2d + a8c95834, file:line below) that capital inheritance needs
**almost no new code**: the estate seam already sweeps a dead colonist's entire stock — the mill/oven
included, with no tool special-casing (the S22e *plow* switch is a live precedent that ordinary tools
*default* to the heir) — to `heir_for(id)`; the mill/oven `GoodId` the heir then holds is exactly the good
the S7 tool-holder adopt-gate checks; and a born-Consumer heir (mortal, `latent: None`) passes C3R.a's
mortal-only formation gate and re-adopts the role *iff the recipe pays*. The one thing missing in C3R.a is
that producers have **no household**, so `heir_for` (which requires a household) returns none and the mill
goes to the commons. C3R.b supplies bounded, reproducing **producer households** so a live heir exists at
death — and then lets the existing estate + S7 machinery fire. It changes two things versus C3R.a
(producers gain households *and* the mill inherits), so it carries a **matched control** — households-on but
tool-inheritance *denied* (mill force-routed to commons, mirroring the S22e plow switch) — to attribute any
lift to *inheritance* rather than to the extra reproducing population. Three gaps keep the test honest and
falsifiable: the heir must be **alive at the moment of death** (reproduction must out-pace producer
lifespan, else the mill still hits commons), the recipe must **pay** for the heir to adopt (inheritance
guarantees capital *continuity*, not immediate re-adoption), and in-flight builds are **not** inherited.
Any of these can make C3R.b *still* collapse — a first-class outcome that would then motivate C3R.c
(explicit role succession / a mortality-aware appraisal).

## 1. Base facts (verified across both C3R.b probes; cites `sim/src/settlement/mod.rs` @ d8e0ddc)

1. **The estate seam already routes the mill to the heir — zero new code.** `settle_death` (13081)
   dispatches heirs vs commons on `demography.is_some()`; `collect_estate` (13107) drains the WHOLE estate
   incl. every good in `self.goods` (the loop 13119–13124 — the mill/oven is an ordinary good);
   `settle_estate_to_heirs` (13231) credits all stock to `heir_for(id)` (loop 13285–13324). The **only**
   tool special-cased is the plow (S22e switch 13260–13267, force-to-commons when
   `endowed_cultivation_capital_active() && !cultivation_tool_inheritance_active()`) — the mill/oven hit no
   such branch, so they follow the heir loop unconditionally. `private_land_heir_eligible` (13445, excludes
   Miller/Baker) is **LAND-specific** (used by `transfer_private_land_on_death` before the estate seam) and
   does NOT gate stock/tool inheritance.
2. **`heir_for` needs a household + a live member.** `heir_for` (13427) returns the first live same-household
   member in slot order (NOT youngest/kin), or `None` if no household or no live member → falls back to
   commons (13326). C3R.a producers are `household: None` (9052) → heirless → mill → commons. **This is the
   single reason C3R.a's capital is destroyed on death.**
3. **The heir-holds-mill → S7 re-adopt path composes end-to-end.** `run_role_choice` (16513): the mortal
   gate (16548, `mortal_only && lifespan.is_none() → continue`) passes a mortal heir; tool-holder admission
   (16561–16575, keyed on `latent`, not vocation) admits a `latent: None` colonist holding `mill > 0` →
   `candidates[0] = Mill`; the switch gate (16805) is `true` for a Consumer. A born-Consumer heir (mortal,
   `latent: None`, holding the inherited mill) is *admitted to the appraisal* purely by holding the tool —
   but **adoption still fires only through the money phase (16521) iff `recipe_adoption_pays_for_money`
   (16622) clears** (or `recurring_motive && recipe_is_profitable` 16636). So inheritance guarantees the
   heir becomes a *candidate* holding the capital; whether it *adopts* is the existing recipe-pays gate (a
   first-class C3R.b result, §2, not a given). **No new role-transfer code.**
4. **The mill-good identity lines up.** `content.mill()`/`content.oven()` are distinct interned `GoodId`s
   held in stock (a seeded latent miller holds `mill:1` at 24261); the S7 path checks that same
   `content.mill()` (16532/16566). The inherited `mill:1` makes the heir a tool-holder instantly.
5. **Commons tools are dead; in-flight builds are a separate, un-inherited channel.** A mill that sinks to
   commons is write/decay/serialize-only — no path credits it back to an agent (`commons_stock` writes
   13206/decay 16444). An in-flight build whose builder dies is dropped and its WOOD forfeited
   (16881–16894) — NOT reachable by estate inheritance (the tool doesn't exist yet). C3R.b closes exactly
   ONE channel: **completed mill/oven tools in the dead producer's stock**.
6. **Households: `max_household_size` is GLOBAL** (`demography.rs:82`; birth gate 14254) — a producer house
   cannot be capped below the lineage cap without a new per-house knob. But a `founders: 0` `HouseholdSpec`
   with producers **tagged into it in the main colonist loop** is a valid, reproducing household
   (membership is by `colonist.household == Some(h)`, 14249; births iterate `demo.households`, 14235).
7. **The hearth is additive; the cushion is a top-up; ordering makes dropping the cushion safe.**
   `deliver_demography_provisions` (14174–14180) credits a FIXED `food_provision`/`wood_provision` per
   member per tick; `run_producer_subsistence` (14822–14897) tops producers to `producer_subsistence` only.
   Tick order (`econ_tick` 10440): old-age death (10539) → hearth provision (10608) → producer cushion
   (10616) → market → consume. The hearth runs **before** consume, so a household-fed producer with the
   cushion dropped does not starve. `food_provision=3` is proven food-secure (lineage members survive to
   old age on it).
8. **Heirs pass C3R.a's mortal-only formation gate.** Newborn heirs are mortal `Consumer` (`lifespan:
   Some`, 14390) → pass `run_role_choice` (16548), capital formation (16871), `is_tool_acquisition_eligible`
   (21838). No conflict — the gate was built for a reproducing mortal pool.
9. **Digest.** Tag 27 = flag byte only (23331). The whole `households` Vec is digested
   (`push_demography_config_bytes` 26921–26950) AND the per-colonist `household` field is digested
   (23988–23994) — so *any* producer-household change diverges from C3R.a's `frontier_mortal_producers`.
   Tag 28 is free; a gated block after 23334 preserves every existing golden. The producer-house **cap**
   rides tag 28's payload, NOT a new `HouseholdSpec` field (which would perturb `lineages`/`frontier`
   configs' serialization).

## 2. The central question and pre-named outcomes

**Central question.** On `frontier_mortal_producers_heritable()` (C3R.a's mortal base + bounded reproducing
producer households + `producer_subsistence=0`, so a dead producer's mill routes to a live heir who
re-adopts via the existing S7 path), across `SEEDS=[3,7,11,19,23]`: does carrying the capital across the
generational gap **lift** the C3R.a collapse — a standing chain forming on mortal producers — or does it
**still collapse** because the heir-timing / recipe-must-pay gaps bind? And is any lift attributable to the
**inheritance** (vs the extra reproducing population) — i.e. does the matched **tool-inheritance-denied**
control still collapse?

**Ordered verdict enum** (first-match):

```
Preconditions (disqualifying):
  BaseUnviable        — the heritable base can't even reproduce the C3R.a mortal chain before inheritance
                        bites (mortal_producer_old_age_deaths == 0), or the C3R.a-equivalent flag-off
                        control fails to climb to the Capital stage
  ReservoirOpen       — immortal_producer_count > 0 (C3R.a's mortal-only gate regressed under the new base)
  ControlDidNotCollapse — the matched tool-inheritance-DENIED control (households on, mill → commons) does
                        NOT still collapse: then a lift is a POPULATION artifact, not inheritance, and the
                        headline result is disqualified as unattributable
  ConservationBroken / RegistryBroken
Outcome ladder (inheritance cell):
  InheritanceInert    — mills essentially never inherit (producer_tool_inheritances ≈ 0): producers die faster than
                        their households bear a surviving heir (the heir-timing gap dominates) → same fate
                        as C3R.a. Honest: reproduction cannot out-pace producer lifespan on this base.
  ChainInheritsButStillCollapses — mills DO inherit to heirs (producer_tool_inheritances > 0) but the chain still
                        collapses: the heir holds the mill yet the role does not sustain (recipe doesn't pay
                        at the death cadence, or single-heir throughput can't hold both stages). The null
                        that motivates C3R.c (explicit role succession / mortality-aware appraisal).
  ChainPersistsUnderInheritance — both milling and baking stages stay jointly staffed across producer
                        deaths via inherited-tool re-adoption (Capital-stage staffing held to the final
                        window), AND the control still collapses → capital continuity is the lift. The
                        keystone's first POSITIVE: a production chain that survives its producers.
```

Every rung is first-class. `InheritanceInert` and `ChainInheritsButStillCollapses` are honest nulls that
sharpen C3R.c; `ChainPersistsUnderInheritance` (with the control collapsing) would be the first standing
mortal chain.

## 3. Mechanism

### 3.1 The heritable base (the only generation change)
`frontier_mortal_producers_heritable()` = `frontier_capital()` + `mortal_chain_producers = true` +
`mortal_producer_inheritance = true`, plus:
1. **Bounded, reproducing producer households.** Append a small fixed set of dedicated producer
   `HouseholdSpec`s with `founders: 0` (`food_provision = 3`, `wood_provision = 3`), and under the flag tag
   the 6 seeded latent producers into them in the main colonist loop (flip 9052 `None → Some(prod_house)`).
   **Primary shape: 6 producer houses, one producer each, producer-house cap = 2** (each producer + a
   dedicated one-slot child-heir buffer → strict 1:1 mill-to-own-heir inheritance). This shape is chosen
   over the denser 2-houses-×-3-producers alternative deliberately: because `heir_for` scans the *first
   live same-household member in slot order* (13437), a multi-producer house would let a dead producer's
   tool stack onto **another incumbent producer** in the same house before ever reaching a child — muddying
   the attribution. One producer per house forces the tool to that producer's own child-heir. The pool is
   **bounded, not tick-proportional**: cap 2 holds the producer-side population at ≤ 12 (it grows from the 6
   founders to ≤ 12 as heirs are born, then stays there — a bounded ceiling, not a scaffold that grows with
   the run). The producer-house cap rides tag-28 state, applied in the birth gate (14254) for producer
   households only, leaving the lineage `max_household_size` untouched.
2. **`producer_subsistence = 0`** in the new base — every producer is now a hearth-fed household member, so
   the top-up cushion is redundant and would double-provision (§1.7). The hearth (10608) feeds before
   consume, so no starvation.

Everything else — the estate seam, `heir_for`, `run_role_choice`, the mortal-only formation gate, the S7
tool-holder path — is **unchanged**; C3R.b only makes the producers into lineage members with heirs and
lets those existing mechanisms fire.

### 3.2 The matched control (attribution — the crux)
C3R.b changes two things vs C3R.a: producers gain reproducing households (+population, +pool) AND the mill
inherits. To attribute a lift to **inheritance**, a companion switch
`mortal_producer_tool_inheritance: bool` (default `true` under the base flag; **mirrors the S22e plow
switch** at 13260–13267): when **false**, a dead producer's mill/oven is force-routed to the commons even
though the producer has a household (the heir still exists, just gets no tool). The acceptance suite runs
the triad:
- **C3R.a baseline** (`frontier_mortal_producers`, no households): the landed collapse.
- **Control** (`frontier_mortal_producers_heritable` + `tool_inheritance = false`): households on, mill →
  commons. Isolates the population/pool effect.
- **Inheritance cell** (`frontier_mortal_producers_heritable`, `tool_inheritance = true`): the mechanism.

A lift is attributed to inheritance **iff** baseline collapses AND control collapses AND cell persists.

### 3.3 What is deliberately NOT wired (falsification integrity)
No explicit role-transfer (the heir adopts only via the *existing* S7 tool-holder path, iff the recipe
pays — C3R.c is explicit role succession). No mortality term in the capital appraisal (still C3R.c). No
in-flight-build inheritance (§1.5 — out of scope; the lost-WOOD channel is disclosed, not fixed). No
**new** C3R.b nudge to make the recipe pay for heirs (the recipe-must-pay gap is a *result*, not something
to engineer around — the base's existing `recurring_motive`/`project_input_bids` from `frontier_capital`
are unchanged, not a C3R.b addition). Bundling any of these would hide whether inheritance *alone* lifts
the collapse.

## 4. Anti-smuggling guards
1. **Attribution by matched control** — a persistence result is disqualified (`ControlDidNotCollapse`)
   unless the tool-inheritance-denied control still collapses; so a lift cannot be the extra population.
2. **Bounded, stationary pool** — the producer-house cap makes the producer population non-growing, so a
   persistence-over-1600-ticks result is the inheritance seam, not a reproducing-scaffold. `mortal_builder_
   adopter_pool` (C3R.a 5–6) is reported; a cell whose pool balloons is flagged.
3. **The gaps are results, not engineered away** — `InheritanceInert` (heir-timing) and
   `ChainInheritsButStillCollapses` (recipe-pays / throughput) are first-class; the spec does not tune
   reproduction or prices to force a lift. Heirless-producer-death rate is reported.
4. **Add households + let existing seams fire — add no succession** — the estate and S7 paths are
   unchanged; C3R.b's only new behavior is the heritable base + the tool-inheritance switch.
5. **Not tuned** — lifespans are the demography's own draw; the producer-house cap is pinned (swept across
   the 6×1-cap-2 / 2×3-cap-4 shapes if load-bearing), not searched; the outcome is reported.

## 5. Conservation & determinism
No new goods flows: inheritance routes through the existing estate seam (heir credit — conserves); the
tool-inheritance-denied control routes through the existing commons seam (mirrors the plow switch —
conserves); dropping `producer_subsistence` removes a mint, not a sink. Integer, deterministic. **Digest:**
tag 28 = ON-only `{ push(28); push(u8::from(mortal_producer_inheritance)); push(u8::from(mortal_producer_
tool_inheritance)); push(producer_house_cap) }`; the producer households + per-colonist `household` values
ride the existing demography/colonist digest on the NEW base only (a new scenario), so C3R.a's
`frontier_mortal_producers` and all older goldens stay byte-identical (flag off → producers keep
`household: None`). Telemetry (all runtime-only, OUT of `canonical_bytes`): `producer_tool_inheritances`
(producer **mill AND oven** tools routed to a live heir — count both goods), `heirless_producer_deaths`
(tool → commons for want of a live heir — the heir-timing gap), `heir_tool_adoptions` (a colonist that
INHERITED a producer tool then adopted Miller/Baker while holding it — a NEW, NARROW counter distinct from
C3R.a's existing broad `role_readoptions` at mod.rs:16668, which counts *any* mortal adoption after any
death; the narrow counter is what attributes the lift to inheritance), `final` Capital-stage staffing /
`living_millers`/`living_bakers`, `mortal_producer_old_age_deaths`, `immortal_producer_count` (guard = 0),
`mortal_builder_adopter_pool`, `era`.

## 6. Slices
- **A — the heritable base + flags.** `frontier_mortal_producers_heritable()`; `mortal_producer_inheritance`
  + `mortal_producer_tool_inheritance` flags (7-site template each); bounded producer households (founders=0,
  producers tagged in, producer-house cap in tag-28 state); `producer_subsistence=0`; the tool-inheritance
  switch (force mill→commons when denied, mirroring 13260–13267); tag 28. *DoD: flag-off byte-identical
  (goldens_unchanged incl. `frontier_mortal_producers`); tag-28 split test; flag-on producers reproduce
  (heirs born), `immortal_producer_count == 0`, `mortal_producer_old_age_deaths > 0`.*
- **B — inheritance + re-adopt observation.** `producer_tool_inheritances`, `heirless_producer_deaths`,
  `heir_tool_adoptions`, Capital-stage staffing over the final window, the C3R.a telemetry carried forward.
  *DoD: the persist-vs-collapse signal and the inheritance-vs-population attribution are measurable +
  deterministic; a mill inherited to a heir who re-adopts is distinguishable from a fresh build.*
- **C — the suite.** `sim/tests/mortal_producer_inheritance.rs`: the triad (baseline / control / cell) + the
  §2 ladder; `SEEDS=[3,7,11,19,23]`. *DoD: suite green; the control still collapses; verdicts printed,
  never asserted.*

## 7. Acceptance suite (`sim/tests/mortal_producer_inheritance.rs`, new)
- **Predeclared thresholds (swept where load-bearing):** the final-window Capital-stage-staffing floor that
  separates persist from collapse; the `producer_tool_inheritances > 0` bar for InheritanceInert. Producer-house
  shape/cap swept (6×1-cap-2 vs 2×3-cap-4) if load-bearing; no tuned lifespan/price.
- **Cells:** `InheritanceCell` (heritable base, tool-inheritance on); `Control` (heritable base, tool-
  inheritance DENIED — must still collapse); the C3R.a `MortalProducers` baseline (must collapse); the
  flag-off heritable base as the byte-identity/BaseUnviable control (must reach Capital stage).
- **Classifier, NOT asserted:** the §2 ladder, keyed on `mortal_producer_old_age_deaths`,
  `immortal_producer_count`, the control's fate, `producer_tool_inheritances`, and final Capital-stage staffing.
- **Hard guards (invariants only):** conservation, money invariant, colonist/household/estate registry,
  `immortal_producer_count == 0`, the flag-off control climbing to Capital (base viability),
  `mortal_producer_old_age_deaths > 0` (non-vacuity), the tool-inheritance-denied control still collapsing
  (attribution).
- **`goldens_unchanged` + the tag-28 canonical-split test** (`frontier` / `frontier_capital` /
  `frontier_mortal_producers` byte-identical off).

Build/verify: `cargo test -p sim --test mortal_producer_inheritance -- --nocapture`, full workspace, fmt,
clippy `-D warnings`; the mortal_producers + producible_capital + g5b_frontier + g4b_demography + share/wage/
succession suites stay green; every prior digest unchanged.

## 8. Risks & open questions
1. **The heir-timing gap may dominate** (`InheritanceInert`) — if producers die faster than their households
   bear a surviving heir (`birth_interval=4`, hunger ceiling, food-endowment gates 14239–14303), the mill
   still hits commons. Honest if so; reported via `heirless_producer_deaths`, and the producer-house shape
   (dedicated 1:1 heir buffer) is chosen to give inheritance a *fair* test, not to force it.
2. **Single-heir throughput may not hold both stages** (`ChainInheritsButStillCollapses`) — one heir per
   producer re-adopting may still not keep milling AND baking jointly staffed at the death cadence; that is
   the honest motivation for C3R.c.
3. **The recipe-must-pay gap** — inheritance preserves the tool, but the heir adopts only if the recipe
   pays (16622); a collapse despite inheritance may be a pricing/demand issue, disclosed via
   `heir_tool_adoptions` vs `producer_tool_inheritances`.
4. **Producer-house cap vs `max_household_size` global** — the cap must apply to producer houses only
   (tag-28 state at the birth gate 14254) without touching the lineage selection households; the review
   should confirm the split is clean and digest-injective.
5. **Producer as food-endowing parent** — a producer in a household may itself be the birth food-endower
   (14284); producer houses need enough food headroom to both feed the producer and rear the heir
   (`food_provision=3` proven secure, but confirm with the cushion dropped).

## 9. Falsifiable-bar summary
C3R.a destroyed the chain's capital on every producer death (mill → commons) and left the role to frantic
re-adoption. C3R.b closes the capital-destruction channel with the cheapest faithful mechanism — bounded
reproducing producer households so the mill routes to a live heir who re-adopts via the *existing* engine —
and asks whether carrying capital across the generational gap is enough to make a production chain survive
its producers. If both stages stay staffed and the tool-inheritance-denied control still collapses
(`ChainPersistsUnderInheritance`), it is the keystone's first standing mortal chain and capital continuity
is the located lift. If mills never inherit in time (`InheritanceInert`) or inherit but the chain still
collapses (`ChainInheritsButStillCollapses`), the null is honest and names C3R.c — explicit role succession
and a mortality-aware appraisal — as the next slice. Each outcome is named before the run, each first-class.
