# impl-72 — C3R.g (diagnostic): The Baker-Role Rejection (why does role-choice sustain milling but not baking, even immortally?)

Status (spec): **v2 — DRAFT** (diagnostic milestone; corrected after Codex review 2026-07-18).
Origin: the grill-with-docs re-diagnosis of 2026-07-18
(`docs/design-mortal-producer-succession.md` §8), refined by an adversarial Codex review that
found v1 named the wrong decision seam. Blocks impl-71 (C3R.f) and every downstream succession
milestone. **Diagnostic, not a mechanism change** — runtime-only telemetry, no engine behavior
change, no golden touched. **v1 (superseded)** attributed the baker collapse to
`capital_build_surplus` and a `bread_price − flour_price` payback margin; both were wrong (see §1).

## 0. One-paragraph summary

On the mortal-producer heritable base the mill→oven→bread chain fails to run, and the failure is
not mortality: even with immortal producers (`FlagOffHeritable`) the chain reaches a functioning
state (`FlowRuns`) on only 1 of 5 seeds; the other four end with the **Baker role rejected** while
Miller persists. Producers here are latent agents that already hold their tools and adopt via
`run_role_choice`; the money and mortality gates are ruled out, so the rejection is the
role-choice **recipe-profit test** — `recipe_adoption_pays_for_money`, plus (under
`recurring_motive`) `recipe_is_profitable` — returning `false` for baking (`phases.rs:2298-2318`).
This milestone traces that test per adoption attempt to localize *why* baking is rejected, with
mutually-exclusive per-attempt reasons and a staged escalation into flow/build telemetry only if
the role-profit test is not the answer.

## 1. Base facts (verified 2026-07-18; corrections from the Codex review)

- **The seam is role adoption, not tool building.** `run_role_choice` scores a recipe with
  `recipe_adoption_pays_for_money` (ordinal savings test) OR, under `recurring_motive`,
  `recipe_is_profitable` (`phases.rs:2298-2318`). There is **no payback bar** here.
  `capital_build_surplus` (`mod.rs:14555`) is a *separate* settlement-level tool-BUILD heuristic
  (`margin·cycles > P_wood·wood_qty + operating_cost·build_labor + input_cost`, gated on demand,
  bottleneck, and a `held_tools ≤ active_producers + slack` utilization guard, `phases.rs:2754`);
  it governs capacity expansion, not why an oven-holder abandons the Baker role.
- **The margin is yield-aware.** `FLOUR_PER_MILL = BREAD_PER_BAKE = 3` (`content.rs:80,90`), so
  the baker margin is `3·P_bread − P_flour − operating_cost`, not `P_bread − P_flour`. Bread
  clearing at ~1 is inconclusive without the flour price.
- **`FlagOffHeritable` rules out money/mortality gates.** It only disables mortality + inheritance
  (`mortal_producer_inheritance.rs:139`); the base seeds zero fixed producers and 3 latent Mill +
  3 latent Bake candidates that start `Unassigned` holding their tools (`generation.rs:480`,
  `mod.rs:3535, 13968`). So a terminal Baker=0 with a persisted bread price implies the terminal
  recipe-profit test rejected — a role-decision fact, not a structural payback fact.
- **The v1 "contradictory aggregates" were an artifact.** `bread_producer = 13068` is cumulative
  over 1,600 ticks; `late_bread_trades` covers only the final 160 (`mortal_producer_inheritance.rs:203`).
  Realized prices persist (stale) and role-choice reads that stale price, while capital formation
  needs recent trades — so seed 3 may be stale-price/early-output persistence, and the failing
  seeds' ~4,300 bread trades are mixed provenance (hearths + `producer_subsistence` mint staple
  bread pre-market, and agents consume owned bread before posting asks: `demography.rs:1079`,
  `phases.rs:944`, `econ/src/society.rs:923`). Aggregates alone cannot name the mechanism.

## 2. The central question and per-attempt reasons

**Q: For each tick a colonist could adopt/retain the Baker role, why does the role-choice
recipe-profit test return `false`?** Emit one **mutually-exclusive** reason per attempt:

- **ROLE-PRICE-ABSENT** — no observed bread output price at the appraisal instant (no appraisal
  basis; input price valued at zero when absent, so this is specifically the *output* price).
- **ROLE-MARGIN-NONPOSITIVE** — a bread price exists but `3·P_bread − P_flour − operating_cost ≤ 0`
  (the yield-aware margin does not pay).
- **ROLE-ORDINAL-DECLINE** — margin is positive but `recipe_adoption_pays_for_money`'s ordinal
  savings test still prefers not to adopt (and `recurring_motive`, if on, does not rescue it).
- **ROLE-ACCEPTS-BUT-FLOW-FAILS** — the role-profit test *accepts* (the colonist adopts/holds
  Baker) yet no sustained bread flow results — i.e. the wall is downstream of role choice.

Seed dependence (only seed 3 functions) is reported as a **separate** axis, not a fifth reason —
it can coexist with any of the above.

## 3. Mechanism (telemetry only; staged; no behavior change)

**Stage 1 — the role-choice trace (always).** Per tick, per colonist appraising the mill or bake
recipe, record: stage (mill/bake), candidate/tool origin (seeded-latent / inherited / built),
output price and its **age** (ticks since the trade it came from), input price and age, recipe
input/output quantities, revenue (`3·P_out`), input cost, operating cost, the
`recipe_adoption_pays_for_money` result, the `recipe_is_profitable` result, the final `pays`, the
prior and next vocation, and switch-readiness. Aggregate per run: the per-attempt reason
histogram, first/last tick the Baker role is held, mean baker vs miller margin.

**Stage 2 — only if the dominant reason is ROLE-ACCEPTS-BUT-FLOW-FAILS:** add flour holdings /
bids / fills for Baker-role holders, bake executions attempted vs completed, and bread
produced / sold / consumed / inventory / provenance — to find where the accepted role fails to
produce.

**Stage 3 — only if Stage 2 implicates tool scarcity / capacity:** add the separate
`capital_build_surplus` funnel with exact LHS/RHS (`margin·cycles` vs
`P_wood·wood_qty + operating_cost·build_labor + input_cost`) and the utilization-guard state
(`held_ovens ≤ active_bakers + slack`).

### 3.1 Pinned run (reproducibility)

Match impl-71's authoritative estimand: `food_provision = 0`, `producer_house_cap = 2`,
`RUN_TICKS = 1600`, final window `160`, `SEEDS = [3, 7, 11, 19, 23]`, on the heritable
`InheritanceCell` **and** the `FlagOffHeritable` immortal control (not the default control — the
matched food=0/cap=2 one).

## 4. Conservation & determinism

All new counters are **runtime-only, non-steering, and excluded from `canonical_bytes`** (like the
existing producer telemetry). Any new `Settlement` field is classified explicitly non-digested and
triggers the digest-coverage guard (`digest.rs:2027`); add a canonical-byte exclusion test
(mutating the counter leaves `canonical_bytes` unchanged). Existing goldens stay byte-identical.

## 5. Deliverable & unblock condition

Localize the Baker rejection to one dominant §2 reason with the margin/price-age numbers behind
it. **No fix here** — impl-72 *selects* the next fix (re-price the final good, change the appraisal,
retune the chain economics, or re-base). **It does not unblock impl-71:** per Codex, succession
work resumes only after an **immortal five-seed viability gate** passes — a functioning chain
(`FlowRuns`) on all five seeds, not one.

## 6. Falsifiable-bar summary

**Pass:** a per-attempt role-choice trace on the heritable base + matched immortal control across
the five seeds that assigns each Baker-rejection to exactly one §2 reason, with the yield-aware
margin and price-age numbers. **Fail:** naming the mechanism from aggregates (they cannot
discriminate), tracing the wrong seam (the build heuristic instead of the role-profit test), or a
telemetry addition that perturbs any golden.
