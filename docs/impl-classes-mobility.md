# impl-56 — C9: Classes, Social Mobility, and Long-Run Demography (does a durable division of labor finally form at scale?)

Status (spec): **DRAFT — pending Codex spec-review.** Layer C9 of the civilization-core roadmap
(`docs/spec-civ-core-roadmap.md`). A **measurement study**, not a new mechanism — **no new decision
flag**, so **no new digest tag**. Composes as an observation over **C1–C4** running together + the merged
**S14/S17** demography. Mirrors the `docs/impl-robustness-appendix.md` study structure.

Falsifiable bar (headline): over a long horizon with C1–C4 on, do **durable economic classes with real
mobility** form — a non-trivial, non-frozen income-source transition matrix, a persistent
owner/entrepreneur stratum **and** upward moves from wage labor — i.e. does the S22 occupation arc finally
resolve **at scale**, embedded in firms and households with property?

## 0. Dependency & premise (read first)

C9 is the **arc-closing answer to S22**. The role-topology arc (S22a–f) proved that *no in-the-moment
incentive or capital lever* produces a durable occupation from a fluid base — only an exit-overriding
voluntary contract stabilized a *core*, not a colony-wide class. C9 asks whether, once the economy is
*living* (C1 wages), *organized* (C2 firms), *inherited* (C3 households), and *rent-bearing* (C4), a
durable **class structure** (persistent income-source strata) with **real mobility** finally emerges — not
as a new lever, but as a *measured outcome* of the composed stack. It is therefore **provisional on C1–C4
landing** (if C1 is `WageInertDemandStillDead`, there are no income sources to stratify and C9 reports
`NoStableStructure`).

## 1. Praxeology — class is catallactic, not Marxian

Class here is a **catallactic** category (Mises): a household's *current dominant source of income* —
**wages** (labor), **profit** (entrepreneurship), **rent/interest** (property/capital) — **fluid by
construction**. A saving laborer buys capital and becomes an owner; a misappraising owner consumes his
capital and returns to wage labor; heirs dissipate or grow estates. There is **no fixed class** and no
class *acts*; mobility is the market's ongoing re-sorting of individuals. **Anti-smuggling:** classes are
**measured** from realized income sources, never **assigned**; mobility is driven by saving / profit-loss
/ inheritance, never a scripted transition; and — critically — **no decision path reads a class label**
(econ purism; class is a read-only observable, like Gini). C9 mints nothing and steers nothing.

## 2. What already exists (C9 is measurement over the stack)

- **Per-agent wealth/income observation:** `metrics.rs` `AgentWealthRecord` (`primary_role`,
  `spendable_money`, `stock_value`, `real_wealth`, `realized_delta`; econ/src/metrics.rs:151–164),
  `build_metric_observation` (:633), `gini_bps`/Lorenz shares (:501–576), `idle_labor_bps` (:622) — all
  **read-only** (metrics unimportable from decision modules).
- **Role/vocation:** `Role { Household, Producer, Trader, Capitalist, Worker, Consumer }` (agent.rs:68);
  `primary_role = roles[0]`, captured per tick.
- **Income sources to classify — an explicit UPSTREAM CONTRACT (spec-review P1).** `AgentWealthRecord`
  today carries `primary_role`, `spendable_money`, `stock_value`, `real_wealth`, `realized_delta`
  (record.rs:151) but **no per-source income buckets**. C9's classifier is valid **only if** C1/C2/C4 each
  expose a per-agent/per-household **income-source record** — wage proceeds (the C1 wage-proceeds bucket),
  firm profit (C2 P/L), rent/interest (C4 tenancy + S10). C9 therefore **requires** those upstream
  milestones to record income-by-source; it does **not** assume it exists in the current checkout, and if a
  source isn't recorded upstream, C9 cannot classify it (a disclosed dependency, added to §0).
- **Demography + heritable selection:** `HouseholdSpec.time_preference_base_bps` + `mutation_delta_bps`
  (demography.rs:29–102) — the heritable trait under selection that drives lineage class trajectories; the
  Malthusian band (S14/S17) bounds population.
- **Study-harness precedent:** `docs/impl-robustness-appendix.md` — `WIDE_SEEDS` (12), per-axis 1-D
  sweeps across `CROSS_SEEDS`, interaction maps, a capstone verdict that **classifies, never asserts
  ROBUST**, with the broken-invariant guards (`conserved`, `!extinct`, provenance-clean).

## 3. Mechanism — a gated study harness (no engine mutation)

- **Classify** each household per study window by **dominant income source** (max share of wage / profit /
  rent+interest / endowment over the window), from the existing per-agent income records — a Laborer /
  Entrepreneur / Rentier / Dependent label that is *read-only*.
- **Measure mobility:** the (from-class → to-class) **transition matrix** across windows; upward-mobility
  rate; stratum persistence (matrix diagonal); **lineage intergenerational correlation** (parent class vs.
  heir class — whether saving/inheritance preserve class across generations, C3).
- **Study structure (mirror the robustness appendix):** run the composed C1–C4 stack on `WIDE_SEEDS` at
  long horizon; sweep the CORE axes that drive class formation (time-preference mutation rate, land
  parcellation/scarcity, inheritance rules, demography birth-ceiling) across `CROSS_SEEDS`; interaction
  maps over (time-preference × capital-scarcity) and (inheritance-rules × land-supply); a **capstone
  verdict** printed as a transition-matrix heatmap.
- **No new state, no new flag.** C9 is a test/measurement harness + read-only metrics; the outcome is a
  **report**, not a merged mechanism, so **nothing is digested** and goldens are trivially unchanged.

## 4. Praxeology / anti-smuggling guards

1. **Measured, not assigned.** Class labels are computed from realized income; a test asserts no decision
   path imports/reads the class label (same discipline as `metrics` purism).
2. **Mobility is market re-sorting.** Transitions arise from saving/profit-loss/inheritance, not a scripted
   rate; a `frozen_prices` or `no_inheritance` control should change the mobility matrix, proving it is the
   economics driving it.
3. **Nothing minted/steered.** C9 adds no source/sink and no gated behavior; conservation and all prior
   goldens are untouched.
4. **Honest capstone.** Like the robustness appendix, **do not assert** "classes form"; classify the
   transition matrix and assert the guards hold.

## 5. Conservation & determinism

No engine change → conservation and determinism are inherited unchanged; **no digest tag** (measurement
only); all prior goldens byte-identical by construction. The study is deterministic (fixed seeds, no live
RNG); the metrics are pure read-backs.

## 6. Slices

- **Slice A — the classifier + transition matrix.** Read-only per-household income-source classification
  and the windowed transition matrix, over the composed C1–C4 stack. *DoD: classes are computed from
  realized income; a mobility matrix is produced; no decision reads a class label.*
- **Slice B — the study harness.** `WIDE_SEEDS` spine + CORE-axis sweeps + interaction maps + capstone
  verdict, mirroring the robustness appendix. *DoD: the study runs across seeds/axes and prints a
  classified capstone.*

## 7. Acceptance suite (`sim/tests/classes_mobility.rs`)

- **Predeclared thresholds (swept):** persistent-stratum share (owner/entrepreneur), upward-mobility rate
  (wage→owner), lineage class correlation, matrix off-diagonal mass.
- **Ordered verdict enum:** base-precondition (C1–C4 living / conservation / guards) → outcome:
  `ClassesFormWithMobility` (a persistent owner/entrepreneur stratum **and** real mobility — non-trivial
  off-diagonal, genuine wage→owner ascents) / `FrozenStrata` (durable classes but no mobility — a caste,
  which would itself be a finding) / `NoStableStructure` (no durable strata — the fluid S22 result persists
  even at scale; expected if C1 failed).
- **Mandatory non-vacuity:** classes are populated from realized income (not vacuous labels); a real
  mobility event (a wage-earner who saved, bought capital, and is measured as an owner next window) that a
  matched `no_capital_purchase` control lacks.
- **Controls (must be pre-existing upstream flags or pure scenario/config choices — spec-review P2, to keep
  C9 measurement-only):** `no_inheritance` (an existing C3/S23c toggle — breaks lineage persistence, the
  mobility matrix must change); `frozen_prices` / `no_capital_purchase` (existing upstream mechanisms or
  scenario endowment choices — remove profit/loss re-sorting); the demography config held fixed so the
  Malthusian band is identical and mobility *within* it is the measured finding. **C9 introduces no new
  gated behavior of its own** — a control that would require new steering state is out of scope for this
  measurement milestone.
- **`goldens_unchanged()`:** trivially — C9 adds no gated state; all prior goldens byte-identical.

Build/verify: `cargo test -p sim --test classes_mobility -- --nocapture`, `cargo test --lib`, fmt, clippy
`-D warnings`, workspace green.

## 8. Risks & open questions

1. **Upstream dependency (top).** `NoStableStructure` is the expected verdict if C1–C4 didn't produce a
   living stratified economy — C9 measures, it cannot manufacture.
2. **Classification ambiguity.** A household with mixed income needs a principled dominant-source rule
   (share threshold), swept, not a single cutoff.
3. **Predeclared, out-of-sample.** Per the S22f review lesson, the two-tier/mobility metric must be a
   **predeclared** measurement with its own predictions, **not** a retroactive relabel of an earlier
   milestone's verdict (no success-bar repair).
4. **Horizon.** Intergenerational mobility needs many generations; the horizon must be long enough for
   lineages to turn over (S23d lifespans), disclosed.

## 9. Falsifiable-bar summary

Measuring — read-only, over the composed C1–C4 stack — each household's dominant income source and the
windowed class-transition matrix should reveal whether a **durable division of labor finally forms at
scale**: a persistent owner/entrepreneur stratum with **real mobility** (genuine wage→owner ascents,
non-frozen off-diagonal, lineage turnover), the arc-closing answer S22 could not reach with any single
lever (`ClassesFormWithMobility`). The honest alternatives are `FrozenStrata` (a caste with no mobility)
or `NoStableStructure` (the fluid result persists even at scale) — each a first-class, predeclared finding,
with the capstone *classified, never asserted*.
