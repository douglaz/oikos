# impl-73 — C3R.h: Stale Input Price vs Final Demand (why the oven margin computes negative, and which fix clears the five-seed gate)

Status (spec): **v2 — DRAFT** (Codex xhigh spec-review folded, 1 round: DIAGNOSIS-OVER-READ + 6×P1
+ 2×P2 → the authoritative `## −0` section supersedes §§0–8 where they conflict). Successor to
impl-72 (C3R.g); it must clear the immortal five-seed viability gate before impl-71 (C3R.f,
lifespan). **v1 (superseded)** diagnosed a live "price inversion / final demand missing" and
recommended restoring bread demand (L1). Codex's review — verified against the code — showed the
primary mechanism is a **stale input-price appraisal**, not a live inversion: role-choice reads
`realized_price(flour)` with no age gate (`phases.rs:2270`), and that accessor is the *last trade's
price, persisted forever* (`econ/src/society.rs:4779`); with bread at 1 a baker's flour bid is
capped near `3·1 − 1 = 2` (the Mengerian ceiling), so **no flour clears at 12** once bread is
cheap — the `P_flour = 12` carried from tick ~300 to 1600 is a phantom frozen from the early boom.

## −0. v2 revision (AUTHORITATIVE — folds the Codex spec-review; supersedes §§0–8 on conflict)

**Corrected diagnosis (P0).** The baker's `MarginNonpositive` rejection is a **stale-input-price
role-choice failure**, not a live economic price inversion. `3·P_bread − P_flour − cost = 3·1 − 12
− 1 = −10` is the *appraisal's* arithmetic, but `P_flour = 12` is a stale last-trade
(`realized_price` has no recency, `society.rs:4779`; the appraisal reads it raw, `phases.rs:2270`),
and no flour can contemporaneously clear at 12 when bread is 1 (baker bid ceiling ≈ 2). So the
baker is rejected on a **phantom input cost**. Weak final demand is retained as a *plausible
secondary* contributor, not the primary cause. `P_bread = 1` is likewise unproven as a demand floor
until late bread *trades* (not the realized price) are shown. The `+34` mill margin is also
stale-based and does NOT establish flour overproduction/surplus capture — the 12 is the last
executed trade's resting-order limit (`econ/src/market.rs:441`), not yield arithmetic.

**Phase-1 measured (2026-07-19, trade-level, failing immortal seed 7).** Flour and bread
*trades* per 100-tick window resolve the staleness question directly:
- **Flour is stale — CONFIRMED.** Flour trades run 71 → 83 → 7 → **0**, and stay 0 from tick ~300
  to 1600, while `realized_price(flour)` holds frozen at 12. The −10 appraisal reads a phantom
  input cost with no live market. **L2 is real and primary.**
- **Bread is live, not stale, and not a demand vacuum.** Bread trades run 240–385 *per window*
  the whole run at price 1 — a real active market floored at 1 (hearths + `producer_subsistence`
  mint bread → flooded supply → floor), NOT the absence of demand my v1 claimed.
- **Live flour ask measured — STALE-PRICE-SUFFICES strongly indicated.** `order_stats_by_vocation`
  at ticks 800 and 1200 shows **7 millers asking flour at price 1** the whole run (no baker bid —
  there are no bakers), while the realized price sits stale at 12. So the flour supply side is
  *alive and cheap*; the baker is blocked *solely* by appraising the stale 12. Against the live ask
  the margin is `3·1 − 1 − 1 = +1 > 0` — baking is profitable. This also refutes the chicken-and-egg
  worry: millers overproduce flour (3× yield) and ask continuously regardless of baker demand, so
  there is always a live flour ask to appraise against; no bootstrap deadlock. The thin +1 margin
  and second-order effects (does the ask hold once a baker buys? does bread stay at 1?) are what the
  2×2 build confirms — not asserted from a static snapshot.

**Corrected demand topology (P1).** Bread already *is* the preferred hunger staple
(`generation.rs:83`); the fallback is **edible raw grain** (`subsistence_on_grain = true`,
`scenarios.rs:286`), not a distinct FORAGE good, and hearths + `producer_subsistence` **mint bread
itself** (`demography.rs:1080`, `phases.rs:944`). The diagnostic `food = 0` zeros only the six
appended producer-house hearths; two legacy hearths stay at 3 and `producer_subsistence = 4`
remain. So "the population never needs bread" is **false**; the real L1 is *retiring the bread mints
and the raw-grain substitution* so market bread demand can form — not re-architecting what the
colony eats.

**Levers (reprioritized):**
- **L2 — stale-input-price fix (primary; mechanism now pinned by measurement).** The appraisal
  must value the input at the **live executable ask**, not the stale realized price. The Phase-1
  order-book measurement settles the mechanism: millers post a live flour ask at 1 the whole run,
  so the fix is to prefer that ask (age-gate the realized input price and fall back to the current
  best executable ask). **Do NOT pass a stale price as `None`** (that zeros the input cost,
  `mod.rs:14596/15418`, manufacturing a false positive) — a live ask exists, so use it. Capital
  formation's recency gate is NOT a precedent (it gates the output/demand signal, `phases.rs:2789`,
  not the input price). Needs a default-off `ChainConfig` behavior flag with conditional digest
  bytes, coverage-guard classification, and off-identity / on-divergence tests.
- **L1 — retire bread mints + raw-grain substitution (secondary).** Compose *existing,
  already-digested* fields (`producer_subsistence` `digest.rs:61`, raw-grain subsistence
  `digest.rs:748`, food-mint retirement `digest.rs:229`, household provisions `digest.rs:1922`) so
  market bread demand can form. Promote to primary only if the 2×2 (below) shows it independently
  necessary. A new `HouseholdSpec` field, if introduced, needs its own coverage guard (the
  `DemographyConfig` guard does not destructure `HouseholdSpec`).
- **L3 — yield/cost rebalance (fallback), unchanged.**

**Phase 1 (measure before choosing) — trade-level, not realized-price.** Record: flour and bread
**last-trade ages**, live bid/ask limits, failed crossings, buyer class + acquisition channel,
flour stocks/fills, and hunger/starvation. A bread trade at 1 does NOT prove a demand floor
(abundant minted supply / low reservations / stale price all reproduce it). This must discriminate
L1 vs L2 by running the actual **2×2 intervention: base / L2-only / L1-only / L1+L2**.

**Outcomes (exhaustive, non-overlapping — replaces §2):**
- **STALE-PRICE-SUFFICES** — L2 alone clears the gate.
- **DEMAND-SUFFICES** — L1 alone clears the gate.
- **EITHER-SUFFICES** — both single arms clear it independently.
- **BOTH-NEEDED** — both single arms fail; L1+L2 clears it.
- **DEEPER-WALL** — L1+L2 fails on ≥1 seed (flour supply/route, capacity/utilization, or
  seed-fragile bootstrap; Phase 1's observables must cover these). Mixed-seed results classified
  explicitly.

**Acceptance (corrected — replaces §5).** Profitability is **strict** `revenue > input + cost`
(`mod.rs:14587`; a *zero* margin is `MarginNonpositive`, `phases.rs:2321`), so require a
**strictly positive** steady-state bake margin computed on **contemporaneous executed** prices —
plus final-window flour fills, bake executions, bread output/trades, and a **starvation /
bounded-hunger control**. `FlowRuns` on all five immortal seeds is the isolation gate; mortal
`FlowRuns` belongs to impl-71, but add a **mortal non-regression smoke**.

## 0. One-paragraph summary (superseded by §−0 where it conflicts)

impl-72 showed the Baker role is rejected by `MarginNonpositive` on ~93% of appraisals on the
failing seeds. A price-path probe on a failing immortal seed stabilizes by tick ~300 and holds:
`P_grain = 1`, `P_flour = 12`, `P_bread = 1`, `operating_cost = 1`, so the *appraisal* computes
mill `+34` / bake `−10`. **Per §−0 the `P_flour = 12` is a stale last-trade, not a live clearing
price** — so the primary fix is the stale-input-price appraisal (L2), with weak final demand (L1)
a secondary contributor, decided by the Phase-1 2×2. The milestone must let a functioning chain
(`FlowRuns`) appear on all five immortal seeds — not just seed 3.

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
