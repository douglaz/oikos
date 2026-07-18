# impl-72 — C3R.g (diagnostic): The Baker-Stage Margin (why does role-choice sustain milling but not baking, even immortally?)

Status (spec): **v1 — DRAFT** (diagnostic milestone). Origin: the grill-with-docs re-diagnosis of
2026-07-18 (`docs/design-mortal-producer-succession.md` §8) that found the C3R succession line is
blocked on a chain whose final (baking) stage does not economically sustain. Blocks impl-71
(C3R.f) and every downstream succession milestone. **Diagnostic, not a mechanism change** — it
adds runtime-only telemetry to localize a margin, changes no engine behavior, and touches no
golden.

## 0. One-paragraph summary

On the mortal-producer heritable base, the mill→oven→bread chain fails to run: producers occupy
roles (via inheritance + subsidy) but produce ~9 loaves in 1600 ticks. A demand-viability
pre-check (immortal producers, `FlagOffHeritable`) reaches a functioning chain (13,068 loaves,
`FlowRuns`) on **only 1 of 5 seeds**; the other four collapse the baker stage (`bakers=0`, ~400
loaves) with a bread market *active* (≈390 trades at price ~1). So the wall is not mortality, not
flour starvation, and not absent demand — it is the **baking role being appraised unprofitable**
under `run_role_choice`: the baker's build/adopt appraisal is
`capital_build_surplus(&bake_recipe, bread_price, flour_price, …)` (`phases.rs:2783`), and with
bread clearing at ~1 the final-stage margin `bread_price − flour_price − operating_cost` sits
below the payback bar while milling's `flour_price − grain_price` clears it. This milestone traces
that margin per tick to confirm the term and quantify why baking never pays.

## 1. Base facts (verified 2026-07-18)

- Role-choice appraises milling and baking via a spread × payback vs build-cost rule
  (`phases.rs:2711, 2740-2795`); comment at `2574`: "when bread demand is met the per-run margin
  falls below the payback bar and no tool is [built]".
- Immortal control (`FlagOffHeritable`) per seed: `[13068;10,9]` (seed 3) vs `[459;7,0]`,
  `[351;7,0]`, `[489;7,0]`, `[414;8,0]` (seeds 7/11/19/23) — miller role sustained, baker not.
- On the failing seeds bread trades (`late_bread_trades ≈ 390`, `late_bread_price = 1`,
  `bread_bought ≈ 4300`); on the working seed bread does NOT trade (`late_bread_trades = 0`) yet
  13,068 loaves are produced — a glut with no market. Contradictory aggregates ⇒ a per-tick trace
  is required.

## 2. The central question and pre-named outcomes

**Q: Why does `run_role_choice` sustain the miller role but not the baker role?** Trace, per tick,
per colonist appraising the baker role: the baker margin `bread_price − flour_price −
operating_cost`, the payback bar it must clear, and the miller margin for contrast. Pre-named
outcomes:

- **MARGIN-STARVED** — `bread_price − flour_price` is structurally too thin (final good underpriced
  relative to its flour input) so the baker margin never clears the bar, while milling does. The
  chain's *economic balance* is the wall (leading hypothesis: bread clears at ~1).
- **INPUT-STARVED** — flour is not actually reaching would-be bakers (miller output not sold on to
  the baker stage), so the baker never has an input to appraise. (The active bread market on
  failing seeds argues against this, but the trace settles it.)
- **PRICE-ABSENT** — bread has no observed clearing price at the appraisal instant, so the baker
  role has no appraisal basis (a cold-start/observability artifact, not an economic one).
- **SEED-BISTABLE** — the margin is near the bar and initial conditions tip it (seed 3 clears,
  others don't), i.e. a fragile bistable bootstrap rather than a clean structural verdict.

## 3. Mechanism (telemetry only — no behavior change)

Add runtime-only counters/trace (OUT of `canonical_bytes`, so goldens are byte-identical): per
tick, for each colonist that appraises the baker recipe, record `(bread_price, flour_price,
operating_cost, computed_margin, payback_bar, cleared: bool)` and the same for the miller recipe.
Aggregate to per-run summaries (mean baker margin, fraction of appraisals clearing, first tick the
baker role is adopted and last tick it is held). Emit on the heritable base (mortal) AND the
immortal control, across `SEEDS = [3,7,11,19,23]`, so seed 3 (works) vs the rest (fail) separates
the outcome.

## 4. Deliverable

Localize the wall to one of the §2 outcomes with the margin arithmetic that proves it. **No fix
here** — the fix (re-tune the chain's final-stage economics, change the appraisal, or re-base the
succession work onto a chain whose oven stage sustains) is scoped after the diagnosis. Unblocks
impl-71 and the C3R line, or redirects them.

## 5. Falsifiable-bar summary

**Pass:** a per-tick trace on the heritable base + immortal control, across the five seeds, that
pins exactly one §2 outcome with the baker-vs-miller margin numbers behind it. **Fail:** a claim
of the mechanism from aggregates alone (the aggregates already contradict each other across
seeds), or a telemetry addition that perturbs any golden.
