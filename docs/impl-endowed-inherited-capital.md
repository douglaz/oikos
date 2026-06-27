# impl-37 — S22e: Endowed + Inherited Cultivation Capital (can capital given UP FRONT and passed by inheritance finally stabilize an occupation?)

Status (spec): DRAFT — pending Codex spec-review. Base: master `fb9502c` (S22d landed + the article). Composes
on S22d (`durable_cultivation_tool`) → S22c (`profit_driven_retention`) → S22a (`endogenous_cultivation_entry`).

## 0. One-paragraph summary

The role-topology arc is a clean **four-step negative**: hunger discovers the role (S22a, fluid), accumulated
skill doesn't change the hunger-gated exit (S22b), a realized profit stay-incentive retains only marginally
(S22c), and even sunk **earned** owned capital concentrates in a dominant few rather than a class (S22d) —
because the lock-in asset can only be *earned by already sustaining* the fluid role (the **chicken-and-egg**).
S22e tests the one mechanism that *side-steps* the chicken-and-egg: give the lock-in **up front**. A default-off
gate (a) **endows a minority** of colonists with a durable cultivation tool (a plow) at generation — a
conservation-safe initial endowment, no earning required — and (b) **passes the tool by inheritance** to a
lineage heir on the owner's death (instead of dissolving it to the commons), so the durable capital can
*persist in a lineage across deaths*. Everything else reuses S22d unchanged: the owner-exclusive haul boost,
and stickiness arising ONLY through the unmodified S22c profit-stay (no exit edit, no fiat "stay" flag). The
question is whether a **persistent owner-cultivator cohort** (by agent id or by lineage) finally forms while
money + mortality + provenance + conservation survive. **Crucial honesty (the main trap):** if it succeeds
this is **institutional/endowment sufficiency**, NOT "endogenous occupation" — the honest claim would be
*"durable inherited capital can stabilize an occupation that earned capital could not,"* and a dedicated
finding mode (`EndowmentOnlyScaffold`) + controls guard against having merely **re-pinned** the S21 producer
class by another name. Classify-not-tune, the same stickiness spine as S22b/c/d (churn drop vs a matched
baseline + a persistent **membership** cohort), with hard anti-fiat / anti-mint / conservation guards.

## 1. Why this milestone, why this lever

S22d named the boundary precisely: earned capital fails to produce a class because acquiring the lock-in
requires already sustaining the (un-sticky) role, so only a rare 1–2 agents capitalize and dominate. The
canonical economic escape is that real occupational capital is usually **not** earned from scratch each
generation — it is **endowed and inherited** (the family farm, the inherited forge). S22e supplies exactly
that: the lock-in exists *before* the role must be sustained (endowment), and it *survives the owner*
(inheritance), so a lineage can hold a durable comparative advantage in cultivation across generations. This
is a **different question** from S22a–d's "does a class self-form from a fluid base" — it is "can an
institution/endowment *stabilize* a class that does not self-form." Both are first-class: a SUCCESS turns the
division-of-labor section from four negatives into "negatives + the condition that resolves them"; a negative
(`NoStickinessDespiteEndowment`) would be a striking result that even given capital up front, the
hunger/profit exit logic still rotates everyone.

The primitives already exist from S22d and are the precedent to reuse, not reinvent:
- **Endow-at-generation**: the S22d non-vacuity test already grants a plow at generation via
  `with_cultivation_tool` (a conservation-safe initial endowment booked as `endowment`). S22e generalizes this
  from a single test-designated agent to a **configured minority** at scenario generation.
- **Death handling**: the dead colonist's estate currently settles **to the commons**. S22e adds a gated
  **inheritance route** that, before the commons settlement, transfers any held cultivation tool(s) to a
  chosen heir (same lineage preferred). Conservation-safe (a transfer, never a mint).
- **The owner-exclusive boost + the profit-stay** are S22d/S22c, used **unchanged**.

## 2. The central question and pre-named outcomes

**Central question.** If a minority *starts* with durable cultivation tools and tools pass by **inheritance**,
does a **persistent owner-cultivator cohort** finally form (churn drop vs the matched S22d/S22c baseline + a
persistent membership cohort who ARE the tool-owners, by id or by lineage) — while money promotes on
`SelfProduced` bread, mortality runs, provenance stays clean, and conservation holds — AND is the result a
genuine *stabilized occupation* rather than a relabeled pin (`EndowmentOnlyScaffold`) or an owner
monopoly that kills the buyers (`InheritedMonopoly`)?

**SUCCESS** (all, across `SEEDS`, vs the matched-seed **S22d/S22c baseline**):
1. **Churn falls materially** — per-ever-cultivating churn ≤ `CHURN_DROP` (0.5) × the matched-seed baseline.
2. **A persistent MEMBERSHIP cohort forms** — ≥ `PERSIST_COHORT` (4) distinct agent **ids** (or distinct
   lineages, reported both ways) each cultivate ≥ `PERSIST_FRACTION` (0.5) of the final window, ≥2
   non-lineage-of-the-S21-pinned-class, **and those are the tool-OWNERS** (the sticky cohort is the endowed
   one, not a coincidental rotation).
3. **A surviving non-owner buyer cohort remains and MATERIALLY buys** (post-promotion bought food ≥
   `MATERIAL_BOUGHT_FLOOR`, living) — the market is not just owners feeding themselves.
4. **Money survives** — SALT promotes and remains money; food materially bought after promotion.
5. **Provenance clean** — sold/pre-promotion bread is `SelfProduced`; `seeded_minted == 0`.
6. **Capital is endowed/inherited, not fiat or minted** — endowed tools are booked `endowment`; inherited
   tools are a conserved transfer; conservation holds every tick; `bread_minted_max == 0`; the tool-stock
   accounting invariant holds **including the inheritance transfer** (built + endowed − destroyed = live-agent
   stock + commons + in-flight; inheritance moves stock between agents, never changes the total).
7. **Endowment + inheritance are the cause, and it is a stabilized occupation, not a re-pin** (§5): the
   **no-inheritance** control (endow but tools dissolve to commons on death) does NOT sustain the cohort
   across deaths; the **no-endowment** control (inheritance on, no initial seed) reproduces S22d; ownership is
   a **minority** (owner-share ≤ `OWNER_SHARE_MAX` = 0.6); and the **productivity-only** control (the same
   boost to all, no owned/endowed/inheritable asset) is not sticky.

**Finding modes (pre-named; each first-class; the verdict test prints the classification and does NOT assert
SUCCESS):**
- `EndowmentLeverInert` (precondition / non-vacuity failure) — endowment didn't take: no endowed owner ever
  out-produces a non-owner, or no tool survives to an heir when an owner dies, or no owner enters the
  retention signal. (Distinguishes "the mechanism didn't bite" from "no stickiness.")
- `ConservationBroken` / `extinct` — any conservation failure, minted/seeded-bread leak, boost-without-tool-
  stock, or tool-stock/inheritance accounting mismatch; or the colony dies.
- `InheritedMonopoly` — owners dominate grain (top-owner or owner-class grain share ≥ `MONO_SHARE` = 0.75)
  AND the non-owner buyer cohort collapses (post-promo bought < `MATERIAL_BOUGHT_FLOOR` / buyers die). Capital
  concentrated into a dynasty that starves the market — a different failure than S22d's diffuse domination.
- `CommuneCollapse` — cultivator share ≥ `COMMUNE_SHARE` AND post-promo bought < floor (everyone self-
  provisions; no money market).
- `MoneyFailureFromLockIn` — SALT fails to promote / demonetizes because the topology kills medium use.
- `EndowmentOnlyScaffold` (THE honest-trap mode) — the cohort "persists" only because the endowed owners
  function as **pinned producers**: money survives but the non-owner side is non-viable without them in a way
  that means we merely re-installed the S21 producer pin via endowment. Detected when SUCCESS-like metrics
  hold BUT the **no-inheritance** control *also* clears them within-generation (i.e. the persistence is just
  the static initial seed acting as a pin for one generation, not inheritance stabilizing a lineage), OR the
  buyer cohort survives only by the configured emergency floor with ~zero market purchases.
- `NoStickinessDespiteEndowment` — the lever bites (owners out-produce, tools inherit) but churn > 0.5×
  baseline AND no persistent owner cohort: even capital given up front and inherited does not retain — the
  hunger/profit exit still rotates owners out.
- `SUCCESS` — all seven SUCCESS conditions, and NOT downgraded to `EndowmentOnlyScaffold` by the controls.

**Ordered classifier (top-down, first-match-wins — the S21i non-gameability discipline):**
`EndowmentLeverInert` → `ConservationBroken`/`extinct` → `InheritedMonopoly` → `CommuneCollapse` →
`MoneyFailureFromLockIn` → `EndowmentOnlyScaffold` (SUCCESS-like but the no-inheritance control also clears,
or owner-share > `OWNER_SHARE_MAX`, or buyers don't materially buy) → `NoStickinessDespiteEndowment` →
`SUCCESS`. Predeclare every threshold as a `const`; do NOT fit.

## 3. Engine design (additive, default-off, conservation-safe)

1. **NEW default-off flag** `ChainConfig::endowed_cultivation_capital: bool` + config fields:
   `endowed_tool_count: u16` (how many colonists are seeded with a plow at generation — a MINORITY: small
   enough that owner-share ≤ ~0.3 at the shipped value, swept), and `cultivation_tool_inheritance: bool`
   (whether a dead owner's tool routes to an heir vs the commons). Helper
   `endowed_cultivation_capital_active(&self)` = flag on AND `durable_cultivation_tool_active()` (which already
   requires S22a+S22c). Canonicalize ON-only with the **next free flag-digest tag (11** unless master
   advanced) + `endowed_tool_count` + `cultivation_tool_inheritance`. Off ⇒ byte-identical.

2. **Endowment at generation.** When active, deterministically select `endowed_tool_count` colonists (a
   stable, seed-derived selection — e.g. lowest ids among eligible cultivator-capable colonists; document the
   rule, digest it) and grant each one cultivation-tool (the existing `CULTIVATION_TOOL` good) into stock at
   generation, booked as `endowment` (conservation-safe; the same path `with_cultivation_tool` uses). Reuse the
   S22d good + serialization; do NOT invent a second tool good.

3. **Inheritance routing on death.** When active AND `cultivation_tool_inheritance`, in the death/estate path,
   BEFORE the existing commons settlement, transfer any cultivation-tool(s) the dead colonist holds to a
   chosen **heir**: prefer a living same-lineage colonist (deterministic selection — e.g. nearest living
   descendant / lowest-id lineage member; document + digest the rule); if none, fall to the existing commons
   path (so the tool is never destroyed by inheritance). A pure stock **transfer** — conservation-safe, no
   mint, the tool-stock total is unchanged. The non-inheritance control leaves the existing commons settlement
   untouched.

4. **Everything else is S22d/S22c, unchanged.** The owner-exclusive haul boost (owner holds the plow → higher
   grain-haul ceiling while cultivating; non-owner = the S22c no-tool return), the `cultivation_tenure`/build
   path (still available — endowed owners may also build more, but the headline tests endowment+inheritance),
   and the **unmodified S22c `profit_stay_active`** as the ONLY retention mechanism. NO exit-branch edit, NO
   fiat "endowed agents always cultivate" flag — an endowed owner stays only because its durable advantage
   makes its realized cultivation return clear its outside option through the existing profit-stay.

5. **Diagnostics (runtime-only):** endowed-owner ids; tools inherited (count of inheritance transfers) +
   heir ids; owners by lineage; owner-share among ever-cultivators; owner ∩ persistent-cohort overlap (by id
   AND by lineage); per-owner realized cultivation proceeds; non-owner buyer cohort + post-promo bought;
   churn vs matched-seed S22d/S22c baseline. Owner status reads the cultivation-tool GoodId stock (NOT
   `acquired_tool`).

## 4. The new suite `sim/tests/endowed_inherited_capital.rs`

- **MANDATORY NON-VACUITY TEST** (else `EndowmentLeverInert`): (a) an endowed owner harvests STRICTLY MORE
  grain AND produces STRICTLY MORE bread than a non-owner under matched conditions (reuse the S22d
  owner-vs-plain harness); (b) at least one inheritance transfer actually occurs (an owner dies with a tool,
  a living heir receives it, tool-stock total unchanged); (c) ≥1 owner enters the retention signal.
- **The ordered classifier (§2)**, printed `--nocapture`; verdict test prints verdict + deciding metrics,
  does NOT assert SUCCESS.
- **Scenarios:** `frontier_endowed_capital` (HEADLINE) = `frontier_cultivation_capital` (S22d) +
  `endowed_cultivation_capital=true` + `cultivation_tool_inheritance=true` + a minority `endowed_tool_count`.
  A capital-earned-off variant is unnecessary (S22d is the off-control).
- **Controls (each a test):**
  - **flag-off** = `frontier_cultivation_capital` (reproduces S22d `NoStickinessDespiteCapital`).
  - **no-inheritance** (endow on, `cultivation_tool_inheritance=false`): tools dissolve to commons on death.
    If the cohort does NOT persist across deaths here but DOES in the headline, inheritance is load-bearing;
    if the headline only matches this within one generation, the headline is `EndowmentOnlyScaffold`.
  - **no-endowment** (inheritance on, `endowed_tool_count=0`): reproduces S22d (tools must be earned) — few/no
    tools, `EndowmentLeverInert`/`NoStickiness`.
  - **productivity-only** (same haul boost to ALL cultivators, no owned/endowed/inheritable asset): must NOT
    produce stickiness — proves it is endowment/ownership, not raw productivity.
  - **too-many-tools** (`endowed_tool_count` ≈ universal, owner-share > `OWNER_SHARE_MAX`): must classify as
    productivity/topology (downgraded), NOT `SUCCESS` — universal ownership is not an occupation.
  - **free/large-endowment sensitivity** classified `SENSITIVITY`, excluded from the core verdict.
- **HARD GUARDS every run + cell:** conservation every tick; `bread_minted_max == 0`; provenance clean-or-
  disqualified; `!extinct`; the tool-stock accounting invariant **including inheritance** (endowed + built −
  destroyed = live stock + commons + in-flight; an inheritance transfer changes per-agent stock but not the
  total — assert non-negativity before the equality, the S22d P3 lesson).
- **goldens_unchanged** test pinning the five tripwire digests (copy from `durable_cultivation_capital.rs`).
- **Robustness mini-sweep** over `endowed_tool_count` (minority→universal) and the haul ceiling + grain flow,
  classified, no tuning — the `endowed_tool_count` axis must be *outcome-driving* (the S21i vacuous-axis
  lesson): show owner-share and the verdict actually move with it.
- A `plow_never_trades`-style guard (the S22d P2): endowed/inherited plows still never clear in trade.

## 5. Verification (independent gate)

Redirect cargo to files; never pipe to head/grep (EPIPE → spurious exit 101).
- `cargo test -p sim --test endowed_inherited_capital` passes (incl. non-vacuity + the inheritance-transfer
  test + the isolating controls).
- `cargo test --workspace` passes; **all existing goldens byte-identical** (`goldens_unchanged` +
  durable_cultivation_capital / profit_driven_retention / occupational_stickiness / endogenous_cultivation_entry
  / robustness_appendix / household_barter / mortality / open_colony_mortality / demand_survival_bridge).
- `cargo fmt --check` clean; `cargo clippy --workspace --all-targets -- -D warnings` clean; conservation holds.

## 6. Honesty and scope (state these in the result, do not let a SUCCESS overclaim)

- **If SUCCESS: the claim is institutional/endowment sufficiency, NOT endogenous occupation.** Honest
  headline: *"durable inherited capital can stabilize an occupational cohort that earned capital (S22d) could
  not."* It does NOT show occupation self-forms from a fluid base (S22a–d already showed it does not).
- **The no-inheritance and productivity-only controls are load-bearing for that claim** — without them a
  SUCCESS is indistinguishable from re-pinning the S21 producer class via a static initial seed. The
  classifier must downgrade to `EndowmentOnlyScaffold` if they don't separate.
- **Bounded to this WOOD-poor colony / this endowment rule / mortality-on regime.** Like S21h/i, expect the
  result may be band-qualified; report the `endowed_tool_count` and grain/haul windows where it holds.
- **Endowment + inheritance are configured institutions**, not emergent — disclose them exactly as the S20
  clearing institution and the S21h survival floor are disclosed. The contribution is the *conditional*: which
  configured institution suffices to stabilize the occupation the incentive levers could not.
- Follow repo conventions; NEVER add Claude/AI/assistant references in code, comments, or committed text.
