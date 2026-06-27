# impl-36 — S22d: Durable Role-Specific Cultivation Capital (does sunk, owned, asset-specific capital finally produce occupation?)

Status: SPEC-READY — Codex spec-review NEEDS-REVISION → five decisions settled (§8) and the 6-item
punch-list folded in: a SEPARATE gated `run_cultivation_capital_formation` (not a reuse of
money-gated `run_capital_formation`); a NEW cultivation-tool `GoodId` + `BuildCultivationTool` template
(not mill/oven identity); the build trigger is a NEW realized-cultivation tenure counter, not
`cultivate_pressure`; the boost is **owner-EXCLUSIVE** + a hard **owner-share isolation guard** (minority
ownership + surviving non-owner buyer cohort, else PRODUCTIVITY-ONLY); the non-durable control consumes
the tool after one cultivation opportunity; a precise tool-stock accounting invariant. Round 2
(NEEDS-REVISION → "isolation substantively sound", pre-approved): pinned `OWNER_SHARE_MAX = 0.6` and the
`cultivation_tenure` credit rule to realized cultivation output only (reset otherwise).

## 0. One-paragraph summary

The role-topology arc is a clean 3-step negative: hunger discovers the role (S22a, fluid), accumulated
**skill** doesn't change the hunger-gated exit (S22b, no stickiness), and a realized **profit stay-incentive**
retains only marginally (S22c, no-stay-despite-profit). Each lever *bites* yet none yields a durable
cultivator class. The consistent boundary: occupation seems to need **durable lock-in** — making *leaving*
costly — not a stronger in-the-moment incentive. S22d is the sufficiency test: a default-off, buildable,
**durable, agent-owned, role-specific cultivation tool** (capital). A cultivating agent invests real
inputs (WOOD + labor, a **sunk cost**) to build a durable tool it then **owns**; the tool raises that
agent's cultivation return **only while it cultivates** (asset-specificity — a plow earns nothing while
you buy bread), so the owner has a *persistent comparative advantage in cultivation* that is forfeited if
it leaves. The hypothesis: composed with the S22c profit-stay exit, this durable owned advantage makes
*owners* stay (their cultivation return durably clears their outside option) → a sticky cultivator cohort
finally forms — **a potential first SUCCESS in the arc**. The whole milestone is built to prove any
stickiness comes from **durability/ownership, not raw productivity** (the S22b trap): the headline is
flanked by a **productivity-only** control (same return boost, no durable owned asset) and a
**non-durable/rented-tool** control (same per-use boost, no persistence) — if either also produces
stickiness, S22d did *not* isolate capital. Classify-not-tune, same stickiness spine (churn drop vs
matched baseline + persistent **membership** cohort), with hard anti-fiat / anti-mint guards.

## 1. Why this milestone, why this lever

S22a-c isolated the boundary precisely and ruled out the *incentive* family (hunger, skill, profit-stay)
as insufficient on their own. The remaining hypothesis is **asset specificity / sunk cost**: durable
role-specific capital is the canonical economic source of occupational persistence (a blacksmith stays a
blacksmith partly because the forge is worthless doing anything else). The existing Miller/Baker tool +
capital-formation machinery is the precedent: a tool is **built from conserved WOOD + labor**, becomes a
**durable GoodId owned in the agent's stock** (serialized in `canonical_bytes`, inherited on death, never
traded, never consumed by recipes), and **gates a higher-output recipe**. S22d reuses that template for a
cultivation tool. The lock-in is *not* "if tool then cultivate forever" (fiat) — it is that the owner's
**realized** cultivation return is durably higher than its outside option, so the **existing S22c
profit-stay** retains it for a *real economic* reason; the sunk WOOD cost is the commitment that creates
the owner population.

## 2. The central question and pre-named outcomes

**Central question.** Does a durable, owned, role-specific cultivation tool (sunk WOOD+labor cost; raises
cultivation return only while cultivating) — composed with the S22c profit-stay exit — finally turn the
fluid regime into a **stable role split** (a persistent membership cohort of tool-owning cultivators +
persistent non-cultivating buyers), while money/mortality/provenance/conservation survive — AND does the
stickiness come from **durability/ownership**, not raw productivity (proven by the controls)?

**SUCCESS** (all, across `SEEDS`, vs the matched-seed **S22c baseline** — profit-stay on, no tool):
1. **Churn falls materially** — per-ever-cultivating churn ≤ `CHURN_DROP` (0.5) × the matched-seed S22c
   baseline churn.
2. **A persistent MEMBERSHIP cohort forms** — ≥ `PERSIST_COHORT` (4) distinct agent **ids** each
   cultivate ≥ `PERSIST_FRACTION` (0.5) of the final window, ≥2 non-lineage, **and those ids are the
   tool-OWNERS** (the sticky cohort is the capitalized one, not a coincidental rotation).
3. **A persistent non-cultivating buyer cohort remains** (material bought food, living).
4. **Money survives** — SALT promotes and remains money; food materially bought after promotion.
5. **Provenance clean** — sold/pre-promotion bread is `SelfProduced`, `seeded_minted == 0`.
6. **Capital is real, not fiat or minted** — the tool is built from conserved WOOD+labor (a measured
   sunk cost), durable, owned; conservation holds every tick; `bread_minted_max == 0`; the tool is never
   eaten/double-counted.
7. **Durability is the cause** — the **productivity-only** and **non-durable** controls (§5) do NOT
   produce stickiness (else the result is productivity, not capital).

**Finding modes (pre-named; each first-class):**
- **CAPITAL LEVER INERT** — the tool builds but capitalized agents don't produce materially more
  conserved bread (the boost doesn't bite). The mandatory non-vacuity test catches it (S22b/c lesson).
- **NO STICKINESS DESPITE CAPITAL** — the tool bites but churn/cohort stay baseline (durable capital
  *still* doesn't change persistence — a deep result: the model needs an explicit role-choice
  institution, not just sunk assets).
- **PRODUCTIVITY ONLY** — stickiness appears in the headline **but also in the productivity-only
  control** → the effect is raw output, not durability; S22d did not isolate capital (reported honestly,
  not as a capital success).
- **COMMUNE COLLAPSE** — too many agents capitalize + lock into cultivation; bought food collapses.
- **MONOPOLIZATION CULL** — a small capitalized cohort dominates grain/bread and starves buyers/trade.
- **MONEY FAILURE FROM LOCK-IN** — role persistence forms but SALT no longer promotes.

**Ordered classifier (mutually exclusive, top-down — the S21i/S22a-c discipline):**
1. **CAPITAL LEVER INERT** (precondition, from non-vacuity) → headline verdict; per-seed reported but moot.
2. **BROKEN-INVARIANT / EXTINCT** (incl. any tool/output accounting leak — ConservationBroken).
3. **MONOPOLIZATION CULL** — top-cultivator grain share ≥ `MONO_SHARE` (0.75) AND buyer/non-lineage collapse.
4. **COMMUNE COLLAPSE** — rolling cultivator share ≥ `COMMUNE_SHARE` AND post-promotion bought < `MATERIAL_BOUGHT_FLOOR`.
5. **MONEY FAILURE FROM LOCK-IN** — bread produced+sold but `current_money_good() != SALT` at horizon.
6. **PRODUCTIVITY ONLY** — the headline would otherwise be SUCCESS, but the productivity-only control *also* clears the churn-drop + cohort bars (durability not isolated).
7. **NO STICKINESS DESPITE CAPITAL** — money+mortality survive, tool bites, but churn > `CHURN_DROP ×` baseline AND no persistent owner-cohort.
8. **SUCCESS** — none of the above AND §SUCCESS 1–7 hold (the owner-cohort is sticky AND the controls don't reproduce it).

Verdict test computes this order, prints verdict + deciding metrics, **does not assert SUCCESS**. Thresholds pre-stated in §7. Churn vs the matched-seed S22c baseline.

## 3. What gets built

### 3.1 The gate (additive, default-off, ON-only digest)

- New `ChainConfig` fields: `durable_cultivation_tool: bool` (default false); `tool_build_wood: u32`
  (sunk WOOD cost); `tool_build_labor: u32` (build labor); `tool_build_patience: u16` (cultivation-tenure
  ticks before an agent invests); the tool haul-ceiling boost parameter (§3.3). Plus a new dedicated
  cultivation-tool **`GoodId`** and a **`BuildCultivationTool`** project template with its own canonical
  bytes (§3.2 — not the mill/oven identity). Helper `durable_cultivation_tool_active(&self)` (active when
  the flag is on AND `endogenous_cultivation_entry_active()` AND `profit_driven_retention_active()` — S22d
  composes on S22a + S22c; the tool works *through* the profit-stay exit). Canonicalized **ON-only** with
  the next free digest tag (10) + the build params; the new tool GoodId is only ever built under the gate,
  so off-runs never hold it. Off ⇒ byte-identical.

### 3.2 Building + owning the tool (a SEPARATE gated cultivation-capital phase)

- **NOT a reuse of `run_capital_formation`** (Codex P1): that phase is gated on `chain.producible_capital`
  AND `current_money_good().is_some()` and hardcodes the mill/oven goods/recipes (settlement.rs ~11340).
  S22d adds a **separate gated `run_cultivation_capital_formation`** phase (or a factored shared
  project-lifecycle helper), so cultivation-capital can build **pre-money** and is independent of the
  producer chain.
- **A new cultivation-tool identity (Codex P1):** add a dedicated `GoodId` (e.g. `CULTIVATION_TOOL` /
  "plow") and a dedicated project template / `ProjectTemplateId::BuildCultivationTool` with its own
  canonical template bytes — **do not reuse `BuildMill`/`BuildOven` identity** (that would pollute
  producer-chain semantics + owner diagnostics). The `Cultivate` recipe stays `required_tool: None`
  (the tool boosts *haul*, it does not gate the recipe — §3.3).
- **Build trigger = a NEW ON-only per-agent build-eligibility counter** (Codex P2), `cultivation_tenure`,
  credited **only on a tick of realized cultivation output** (`SelfProduced` bread > 0 / grain actually
  converted), and **reset when the agent is not cultivating or produces no output** — distinct from
  `cultivate_pressure` (a hunger-*entry* counter). An agent
  whose `cultivation_tenure ≥ build_patience` AND that can afford `tool_build_wood` invests: consume
  `tool_build_wood` (booked `consumed_as_input`) + `tool_build_labor` over the build, output one durable
  cultivation-tool GoodId into its stock (conserved `produced`). Owned (in stock, serialized), durable
  (never consumed by a recipe; headline has no decay), inherited on death (existing estate path), never
  traded — the Miller/Baker tool semantics, but a distinct good built by a distinct phase.
- **The sunk cost is real and measured:** `tool_build_wood` is permanently consumed; an owner that never
  recoups it via cultivation has destroyed value — the commitment that creates the owner population and
  the opportunity cost of leaving. The WOOD cost must be high enough that **not everyone builds** (a
  *minority* owner population — §3.3); the sweep checks this.

### 3.3 The capitalized cultivation return (conservation-safe, asset-specific)

- A tool-OWNING agent, **while cultivating**, gets a higher cultivation return via the **S22b haul
  lever** (`cultivation_haul` with a higher ceiling) — a faster draw on the conserved grain node,
  bounded by `node.stock`; **the 1:1 grain→bread recipe is unchanged, no new `produced` term, the tool
  is never minted into bread** (conservation by construction; hard guard). The boost is
  **owner-EXCLUSIVE** (only agents holding the cultivation-tool good get it; non-owners cultivate at the
  S22c no-tool return) and applies **only while cultivating** (asset specificity): a tool-owner that
  stops cultivating earns the tool nothing. So an owner's cultivation return durably exceeds its outside
  option *and* exceeds a non-owner's — a **persistent, exclusive comparative advantage**. The **existing
  S22c profit-stay** then retains owners for a real reason. **No fiat stay flag is added** — S22d does
  not touch the exit branch; it raises only the owner's *realized return*, which S22c already reads.
- **Why this isolates durability/ownership from raw productivity (the S22b trap, Codex P1):** the
  advantage is (i) **durable** (doesn't decay), (ii) **owned/exclusive** (concentrated in the minority
  who paid the sunk WOOD cost — NOT shared by every cultivator), so it is a *persistent per-agent
  comparative edge*. The two falsifying controls (§5) give the SAME boost magnitude but break exactly one
  property each (productivity-only = boost to all, no ownership; non-durable = boost but no persistence)
  — if either reproduces the stickiness, it was productivity, not capital.
- **Hard isolation guard (Codex P1#3):** if ownership becomes **universal** among ever-cultivators (the
  sunk cost too low / everyone builds), the tool is a cosmetic global productivity boost, not capital —
  the suite computes **owner-share-among-ever-cultivators** and SUCCESS REQUIRES a *minority* owner
  cohort that is the sticky cohort **plus** a surviving material **non-owner buyer** cohort. Universal
  ownership ⇒ classified PRODUCTIVITY ONLY / not-isolated, never SUCCESS.

### 3.4 Diagnostics (runtime-only) + scenarios

Runtime-only read-outs: tool-owner count + ids, the owner∩persistent-cohort overlap (SUCCESS criterion
2), per-owner sunk cost vs realized cultivation proceeds (did the asset pay off?), churn vs matched S22c
baseline, the S22a-c diagnostics. Scenario `frontier_cultivation_capital` = `frontier_profit_retention`
(S22c, skill-off) + `durable_cultivation_tool = true`. (Open question §8: also a skill-on variant?)

## 4. Anti-fiat / anti-mint / anti-tuning discipline

- **Anti-fiat (the worst trap, Codex):** S22d adds **no** stay flag and does **not** edit the exit
  branch. Stickiness, if any, arises only because the owner's *realized* cultivation return (via the
  conserved haul boost) durably clears its outside option through the **unmodified S22c profit-stay**. An
  owner whose tool doesn't pay off is not retained.
- **Anti-mint:** the tool is built from conserved WOOD+labor and is a durable good never consumed by a
  recipe; the cultivation boost is the haul lever (bounded conserved-node draw), never a recipe-ratio or
  `produced` change. Hard per-tick `conserves()` + `bread_minted_max == 0` + a "tool count conserved"
  guard on every cell.
- **Anti-tuning:** `tool_build_wood`, `tool_build_labor`, the tool haul ceiling, and the build trigger
  are predeclared modest constants, swept as sensitivity, never tuned to manufacture a cohort. The
  verdict test never asserts SUCCESS; the productivity-only + non-durable controls are the falsifiers.

## 5. Controls (classify, never tune) — the isolating core

- **Flag-off control** = `frontier_profit_retention` (S22c) — reproduces the S22c baseline.
- **Capital ON** = `frontier_cultivation_capital` — the treatment.
- **Productivity-only control** (the key isolator) — every cultivating agent gets the SAME cultivation
  return boost the tool confers, but with **no buildable, owned, durable asset** (a colony-wide transient
  bump, no sunk cost, no ownership). Expected: NO stickiness (the boost is shared/non-persistent → no
  per-agent durable advantage). **If it DOES produce stickiness, the verdict is PRODUCTIVITY ONLY** — the
  result is output, not capital.
- **Non-durable / rented-tool control (exact mechanic, Codex P2)** — the same owner-only boost applies,
  but the tool is **consumed/removed after one successful cultivation opportunity** (no persistent stock;
  the agent must re-build/re-pay each time to get the boost again). Same per-use productivity, **no
  durable ownership** → isolates *durability* specifically. Expected: NO stickiness (no persistent
  per-agent advantage accrues).
- **Zero-build-input control** — `tool_build_wood` so high (or WOOD starved) that no tool is ever built:
  must reproduce S22c (no fake success).
- **Free-tool sensitivity** (NOT headline) — `tool_build_wood = 0` (no sunk cost): classified
  SENSITIVITY to show the sunk-cost's role; excluded from the core verdict.

## 6. Determinism & goldens

One additive default-off gate (tag 10, ON-only) + the build path + the owner haul boost, all gated. Off ⇒
no tag 10, no tool built, the cultivation path is exactly S22c ⇒ **all pinned goldens byte-identical**
(asserted in the new suite). Tool ownership lives in the agent's stock (already serialized) — built only
under the gate, so off-runs never have it. Conservation preserved by construction (§4). On ⇒ new-scenario
digest (expected).

## 7. Acceptance criteria (independent verification)

- **MANDATORY non-vacuity test:** a tool-owning cultivator must harvest strictly more grain AND produce
  strictly more bread than a no-tool cultivator under matched conditions over the same horizon, AND ≥1
  tool is actually built from a real WOOD sunk cost. Else → **CAPITAL LEVER INERT** (not "no stickiness").
- New suite `sim/tests/durable_cultivation_capital.rs`: the §2 ordered classifier across `SEEDS`,
  diagnostics printed, verdict not asserted SUCCESS.
- **Pre-stated thresholds (a priori):** `CHURN_DROP=0.5` (vs matched-seed S22c baseline),
  `PERSIST_FRACTION=0.5`, `PERSIST_COHORT=4` (≥2 non-lineage, **owners**), `MONO_SHARE=0.75`,
  `COMMUNE_SHARE`, `MATERIAL_BOUGHT_FLOOR`, `OWNER_SHARE_MAX = 0.6` (the maximum
  owner-share-among-ever-cultivators for ownership to count as a *minority* edge),
  `tool_build_wood`, `tool_build_labor`, the tool haul ceiling, `build_patience`.
- The **owner∩persistent-cohort + isolation** check (Codex P1#3): SUCCESS requires (a) the sticky ids ARE
  tool-owners (not a coincidental rotation), (b) owner-share-among-ever-cultivators ≤ `OWNER_SHARE_MAX`
  (a *minority* — universal ownership ⇒ PRODUCTIVITY ONLY, not SUCCESS), and (c) a material **non-owner
  buyer** cohort survives.
- Controls (§5) each a test and behave as classified; **the productivity-only + non-durable controls
  must NOT clear the stickiness bars** for a SUCCESS verdict (else PRODUCTIVITY ONLY).
- Guards every run + cell: conservation, `bread_minted_max == 0`, provenance clean-or-disqualified,
  `!extinct`, and the **tool-stock accounting invariant**: *cumulative cultivation-tools produced −
  cumulative destroyed/decayed = live-agent cultivation-tool stock + estate/commons stock + any
  completed-but-undeposited in-flight* (owner diagnostics read the actual cultivation-tool GoodId stock,
  NOT the generic `acquired_tool` bool).
- Robustness mini-sweep over `tool_build_wood` / haul-ceiling + grain flow, classified, no tuning.
- Workspace: all tests pass; **all goldens byte-identical**; fmt + clippy -D warnings clean.

## 8. Resolved decisions (Codex spec-review)

1. **Boost → owner-only haul-capacity boost, NOT a higher-yield recipe** (a higher yield would change
   the physical conversion ratio and blur conservation). The haul lever is acceptable **only if strictly
   owner-exclusive, persistent, and falsified by the productivity-only / non-durable controls** (§3.3).
2. **Build trigger → sustained ACTUAL cultivation + own WOOD** (a NEW ON-only per-agent build-eligibility
   counter credited on *realized cultivation output / consecutive cultivating ticks* — §3.2), NOT
   `cultivate_pressure` (that is a hunger-entry counter that resets once eating works; it would let
   agents pre-build on entry pressure alone). Pre-money builds are fine (the tool is physical).
3. **Headline composes S22c profit-stay ON; report a profit-stay-OFF variant** (durable advantage flows
   through the already-tested stay decision; the off-variant tests whether capital alone does anything
   against the hunger-only exit — expected no).
4. **SUCCESS requires the persistent cohort to BE the tool-owners** AND a material non-owner buyer
   cohort to remain AND ownership NOT to be universal among ever-cultivators (§2/§3.3/§7).
5. **Non-vacuity:** ≥1 real tool built from *consumed* WOOD; the owner-only boost demonstrably raises
   grain hauled AND bread produced under matched opportunity; ≥1 owner enters the retention signal. Else
   → CAPITAL-LEVER-INERT. **ConservationBroken:** any per-tick conservation failure, minted/seeded bread
   leak, a tool boost applied without the tool in stock, or a tool-stock accounting mismatch (§7 formula).

## 9. Scope boundary (deferred to S22e+)

Heritable skill/capital across generations; a global role chooser; profit-driven *entry*; Miller/Baker
specialized-producer entry; endogenizing the clearing institution; tool resale markets. S22d tests
whether **durable owned role-specific cultivation capital** produces occupation — the sufficiency test
for the boundary S22a-c named.

## 10. Risks
- **Fiat lock-in** → manufactures occupation. Mitigated: no stay flag, no exit edit; stickiness only via
  realized return through the unmodified S22c profit-stay.
- **Productivity not durability** → the headline "succeeds" but so does productivity-only. Mitigated: the
  two isolating controls are first-class falsifiers; PRODUCTIVITY ONLY is a pre-named verdict.
- **Hidden mint** → tool or boost creates bread. Mitigated: conserved build inputs, durable non-consumed
  tool, haul-only boost, hard guards.
- **Overclaim** → "division of labor" when owners still rotate. Mitigated: owner∩persistent-membership
  by stable ids.

## 11. Pipeline

Codex spec-review (settle §8) → SPEC-READY → setsid rb-lite `claude,codex` → independent verification
(workspace + all goldens byte-identical + the new suite + the verdict run) → Codex review-of-results →
merge + report-update + memory + pin.
