# impl-73 — C3R.h: Final-Stage Demand (can the oven stage earn a positive margin — and does that let the chain sustain on all five seeds?)

Status (spec): **v1 — DRAFT** (fix milestone; pending Codex spec-review). Origin: the impl-72
(C3R.g) diagnostic localized the baker collapse to a non-positive role-choice margin, and a
price-path measurement (2026-07-19) pinned *why*. Successor to impl-72; it is the milestone that
must clear the immortal five-seed viability gate before impl-71 (C3R.f, lifespan) is meaningful.
**Unlike impl-72 this changes economics, not just telemetry** — so it lives behind a new
scenario/flag and every non-target golden stays byte-identical.

## 0. One-paragraph summary

impl-72 showed the Baker role is rejected by `MarginNonpositive` on ~93% of appraisals on the
failing seeds. A price-path probe on a failing immortal seed shows why, and it stabilizes by
tick ~300 and holds for the rest of the run: `P_grain = 1`, `P_flour = 12`, `P_bread = 1`,
`operating_cost = 1`. The chain is **price-inverted** — milling earns `3·12 − 1 − 1 = +34` while
baking earns `3·1 − 12 − 1 = −10`. Milling captures the whole chain surplus (nearly-free foraged
grain → flour at 3× yield); the final good, bread, is **floored at 1 because the population is fed
by forage + the hearth subsidy and never needs to buy bread**, so the last stage cannot pay,
mortality-independent. This milestone restores a viable final-stage margin and tests whether that
lets a functioning chain (`FlowRuns`) appear on all five seeds — not just seed 3.

## 1. Base facts (measured 2026-07-19)

Failing immortal seed (`FlagOffHeritable`, food=0, cap=2), realized prices, steady state from
tick ~300 to 1600:

| | grain | flour | bread | operating_cost |
|---|---|---|---|---|
| price | 1 | 12 | 1 | 1 |

- **mill margin** `3·P_flour − P_grain − cost = 3·12 − 1 − 1 = +34` (milling very profitable).
- **bake margin** `3·P_bread − P_flour − cost = 3·1 − 12 − 1 = −10` (baking loses every cycle).
- Early transient: `P_bread` 62 → 3 → 1 and `P_flour` 1 → 3 → 12 over the first ~300 ticks, then
  frozen. The role-choice margin is `3·P_bread − P_flour − operating_cost` (yield 3 per input,
  `content.rs:80,90`; the appraisal at `phases.rs:2298`).

## 2. The central question and pre-named outcomes

**Q: Can the oven stage earn a non-negative role-choice margin, and does restoring it produce a
functioning chain (`FlowRuns`) on all five seeds?** Pre-named outcomes:

- **DEMAND-FIXES-IT** — giving bread real recurring demand (final good actually eaten) raises
  `P_bread` above the flour break-even (~`(P_flour + cost)/3 ≈ 4.3`), the bake margin turns
  positive, and the chain reaches `FlowRuns` on all five seeds. The final-demand precondition was
  the wall (the C-series' recurring lesson).
- **STALE-PRICE-FIXES-IT** — `P_flour = 12` is a *stale* early-boom artifact (bakers bid flour up
  when bread was 62, then stopped trading flour, and the realized price persists). The baker is
  rejected on a phantom input cost; age-gating the appraised input price (as capital formation
  already requires recent trades, `phases.rs:2767`) recovers the margin without changing demand.
- **BOTH-NEEDED** — demand and the stale-price appraisal each contribute; neither alone clears the
  five-seed gate.
- **DEEPER-WALL** — neither clears it: the chain does not sustain on ≥1 seed even with a positive
  bake margin, implicating a further constraint (flour supply, capacity/utilization, seed-fragile
  bootstrap). A real negative that re-scopes the succession line.

## 3. Phase 1 — resolve the two open measurements (do first, cheap)

Neither is inferred; measure both before choosing the fix:

1. **Is `P_flour = 12` a real clearing price or stale?** Trace flour *trades* (not just the
   realized price) over the run: does flour still clear after the ~tick-300 freeze, or is 12 a
   frozen last-trade the appraisal reads with no live market? Decides whether STALE-PRICE is in play.
2. **Is `P_bread = 1` demand-floored?** Confirm bread trades occur at 1 (real floor demand) and
   that the population's food is coming from forage/hearth, not bread — i.e. reducing the
   bread-substituting food raises bread demand rather than starving the colony.

## 4. Phase 2 — the fix (candidate levers; grounded in §1)

- **L1 — Final demand (recommended).** Make the population actually depend on bread instead of
  forage + hearth subsidy, so `P_bread` clears above the flour break-even and the bake margin is
  positive. This is the genetic precondition the last stage was missing. **Tension to respect:**
  C3R.b showed a *large* food subsidy floods demand and kills the chain; the lever here is the
  opposite end (less substitution so bread IS demanded) — it must raise bread demand without
  starving the colony (Phase-1 measurement #2 bounds this).
- **L2 — Stale-price appraisal fix (companion, only if Phase-1 #1 confirms staleness).** Age-gate
  the appraised input (flour) price in `recipe_adoption_pays_for_money` / `recipe_is_profitable`
  so a frozen early-boom price cannot reject a baker on a phantom cost. Mechanism fix, not economic.
- **L3 — Yield/cost rebalance (fallback).** Change the chain's yields, grain cost, or operating
  cost so milling does not capture the entire surplus. Riskier: it retunes the whole chain balance
  and would move the chain scenarios' goldens broadly; prefer L1/L2 first.

Recommended path: **Phase 1 → L1 (+ L2 if flour is stale)**; hold L3 as the fallback if
DEEPER-WALL.

## 5. Acceptance — the immortal five-seed viability gate

- **Gate (unblocks impl-71):** a functioning chain — `StructurePersistsUnderInheritance` +
  `FlowRuns` — on **all five** immortal `FlagOffHeritable` seeds `[3,7,11,19,23]`, not just seed 3,
  with the bake margin measured non-negative in steady state. Pin it with an asserting test.
- **Then** the mortal cells can be re-evaluated (they were a distinct *accepts-but-flow-fails*
  mode; a positive-margin chain may or may not survive mortality — that is impl-71's question,
  now on a substrate that actually functions).

## 6. Conservation & determinism

**This changes behavior**, so it is NOT byte-identical on the target scenario. Confine it: put the
fix behind a **new scenario/flag** (e.g. `frontier_mortal_producers_bread_demand`) or a gated
`ChainConfig`/`DemographyConfig` field defaulting to today's behavior, so **every existing golden
and digest is byte-identical** and only the new scenario's goldens are new. Any new
behavior-steering field is DIGESTED and classified in the digest-coverage guard (`digest.rs`);
conservation and the money identity are asserted per tick as today.

## 7. Risks

- **Fixing the margin need not fix sustain** (DEEPER-WALL). The five-seed gate is the honest bar;
  a positive margin that still dies on some seed is a finding, not a failure.
- **Demand vs starvation** (L1). Raising bread demand by cutting food substitution risks starving
  the colony; Phase-1 #2 bounds the safe range, and the conservation asserts catch mistakes.
- **Golden blast radius** (L3). A yield/cost retune would move many chain goldens; kept as fallback
  and, if used, scoped to a new scenario.

## 8. Falsifiable-bar summary

**Pass:** Phase 1 resolves both measurements with trade-level (not just realized-price) evidence;
the chosen lever yields a measured non-negative steady-state bake margin AND a `FlowRuns` chain on
all five immortal seeds, pinned by an asserting test, with all non-target goldens byte-identical.
**Fail:** claiming the fix from the realized-price snapshot without the flour-trade check (the
stale-price confound), moving unrelated goldens, or asserting sustain from a single seed.
