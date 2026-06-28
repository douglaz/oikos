# impl-37 — S22e: Endowed + Inherited Cultivation Capital (can capital given UP FRONT and passed down a lineage finally stabilize an occupation?)

Status (spec): SPEC-READY (round-1 8 findings + round-2 2 findings folded in, §8). Base: master `fb9502c`
(S22d landed + the article). Composes on S22d (`durable_cultivation_tool`) → S22c (`profit_driven_retention`)
→ S22a (`endogenous_cultivation_entry`).

Status (landed): IMPLEMENTED — verdict **`NoStickinessDespiteEndowment`** across all five seeds. The
default-off `endowed_cultivation_capital` gate (endowment at generation by a deterministic hash of
`(seed, household_id)` + the plow estate-routing switch, digest tag 11, ON-only) lands additive and
conservation-safe; every existing golden is byte-identical (`goldens_unchanged` across all named suites).
On the expanded `ROSTER_HOUSEHOLDS = 8` base the gate is **non-vacuous** (an endowed owner out-produces a
no-tool cultivator ~3×; 640–680 real plow→living-heir inheritance transfers per run, and those heirs
cultivate; ≥1 owner enters the retention signal) and the precondition holds (the gate-off expanded base
still promotes SALT, sustains mortality, keeps provenance clean, and shows no owner-lineage cohort with
high churn). Yet even capital given **up front and inherited** does not retain: churn stays ~1× the matched
baseline (not the ≤0.5× bar), cultivation share settles ~4%, and **no persistent owner-cultivator LINEAGE
cohort forms** (cohort `0/8`), while money + mortality + provenance + conservation all survive and the
§3.5 tool-stock invariant holds every cell. The controls isolate the read cleanly: the no-inheritance and
productivity-only controls do **not** clear the bars, the too-many-tools control classifies
`UniversalOwnership` (owner-lineage share → 1.00), no-endowment is `EndowmentLeverInert`, and the
`endowed_tool_count` axis is outcome-driving (owner-lineage share 0.12 → 1.00). The honest headline: in this
WOOD-poor mortality regime, durable + endowed + inheritable capital was **not** sufficient to stabilize a
cultivator lineage — the four-step negative extends to a five-step one. (Engine: `sim/src/settlement.rs`;
suite: `sim/tests/endowed_inherited_capital.rs`.)

## 0. One-paragraph summary

The role-topology arc is a clean **four-step negative**: hunger discovers the role (S22a, fluid), accumulated
skill doesn't change the hunger-gated exit (S22b), a realized profit stay-incentive retains only marginally
(S22c), and even sunk **earned** owned capital concentrates in a dominant few rather than a class (S22d),
because the lock-in asset can only be *earned by already sustaining* the fluid role (the **chicken-and-egg**).
S22e tests the one mechanism that *side-steps* the chicken-and-egg: give the lock-in **up front** and let it
**pass down a lineage**. A default-off gate (a) **endows a minority of lineage households** with a durable
cultivation tool (a plow) at generation — a conservation-safe initial endowment, no earning required — and
(b) **gates whether plows are inherited**: the engine *already* routes a dead colonist's stock (including
tools) to the household heir via `settle_estate_to_heirs` before commons fallback, so the genuinely new lever
is a **switch** — inheritance ON keeps plows on that heir path; inheritance OFF **forces plows to the commons**
(the falsifying control). Everything else reuses S22d unchanged: the owner-exclusive haul boost, and
stickiness arising ONLY through the unmodified S22c profit-stay (no exit edit, no fiat "stay" flag). The
question is whether a **persistent owner-cultivator LINEAGE cohort** finally forms — and, decisively, whether
it **survives the founder's death** (an inherited-tool heir is in the sticky cohort) — while money + mortality
+ provenance + conservation survive. **Crucial honesty (the main trap):** a SUCCESS here is **institutional /
endowment / dynastic sufficiency**, NOT "endogenous occupation," and NOT a *non-lineage* occupational class
(S22a–d already showed that does not self-form). The honest claim would be *"durable, endowed, inheritable
capital can stabilize a cultivator **lineage** that earned capital could not."* A dedicated `EndowmentOnlyScaffold`
finding mode + the no-inheritance / no-endowment / productivity-only / too-many-tools controls guard against
merely **re-pinning** the S21 producer class via a static seed. Classify-not-tune, the same stickiness spine as
S22b/c/d (churn drop vs a matched baseline + a persistent **membership** cohort), with hard anti-fiat /
anti-mint / conservation guards.

## 1. Why this milestone, why this lever — and the corrected primitives

S22d named the boundary: earned capital fails to produce a class because acquiring the lock-in requires
already sustaining the (un-sticky) role. The canonical economic escape is that real occupational capital is
usually **not** earned from scratch each generation — it is **endowed and inherited** (the family farm). S22e
supplies exactly that and asks whether an *institution* can **stabilize** a class that does not **self-form**.
This is a different question from S22a–d and is first-class either way.

**Corrected grounding (Codex P1).** The estate machinery already does most of the inheriting:
- `settle_death` → `settle_estate_to_heirs` (settlement.rs ~9005/9134/9165) credits **all** of a dead
  colonist's stock goods — including the S22d `CULTIVATION_TOOL` (plow) — to the **household heir** via
  `heir_for`, with the commons as fallback. So *tools already inherit within a household by default.*
- `heir_for` (settlement.rs ~9279) returns a heir only for colonists with a `household` (the lineage); it
  returns `None` for non-lineage Consumers/Gatherers (`household == None`). **Inheritance can only operate for
  lineage households**, which forces the S22e design decision below.

So the new engine surface is small and precise: **endow a minority of households up front**, and add a
**plow-specific estate-routing switch** so we can turn inheritance OFF (force plows to commons) as the
falsifying control. The owner-exclusive boost and the profit-stay are reused unchanged.

**Design decision (Codex P1 #2 — eligibility).** Endowment is restricted to existing **lineage households**
(the only agents inheritance can operate on). The honest consequence, stated up front and in §6: the
persistent class S22e can produce is an **endowed lineage / dynastic** cultivator class, **not** the
non-lineage occupational class S22a–d sought. We do NOT invent a synthetic "capital lineage" for non-lineage
roles (more machinery, weaker claim). We accept the narrower, honest claim and let the controls prove it is
*inheritance of an endowed minority*, not a re-pin of the whole S21 producer lineage.

## 2. The central question and pre-named outcomes

**Central question.** If a **minority of lineage households** *start* with durable cultivation tools and plows
**pass to the household heir** on death, does a **persistent owner-cultivator LINEAGE cohort** form (churn
drop vs the matched S22d/S22c baseline + a persistent membership cohort of owner lineages) that **survives the
founding owner's death** (an inherited-tool heir is in the final-window sticky cohort) — while a non-owner
buyer cohort survives and materially buys, SALT promotes on `SelfProduced` bread, mortality runs, provenance
is clean, and conservation holds — AND is it a genuinely *inheritance-stabilized* class rather than a static
re-pin (`EndowmentOnlyScaffold`) or an owner dynasty that starves the market (`InheritedMonopoly`)?

**Primary success = `LineageStickySuccess`** (all, across `SEEDS`, vs the matched-seed **S22d/S22c baseline**):
1. **Churn falls materially** — per-ever-cultivating churn ≤ `CHURN_DROP` (0.5) × the matched-seed baseline.
2. **A persistent owner-LINEAGE cohort forms** — ≥ `PERSIST_COHORT` (4) distinct **owner lineages**, each with
   a living member cultivating ≥ `PERSIST_FRACTION` (0.5) of the final window, **and those lineages are the
   endowed/inheriting ones** (the sticky cohort is the capitalized one, not a coincidental rotation). **This
   bar is only reachable on an EXPANDED lineage roster** (Codex P1): the base `frontier()` has only 2
   households, so the headline scenario and all matched controls run on a roster of ≥ `ROSTER_HOUSEHOLDS` (8,
   default; the shipped value gives owner-share head-room) lineage households, with the coherence constraint
   `PERSIST_COHORT ≤ endowed_tool_count ≤` a minority of `ROSTER_HOUSEHOLDS` (so the cohort can form yet
   ownership stays a minority). All counts predeclared as `const`, swept.
3. **Inheritance is load-bearing (the decisive clause, Codex P1 #4)** — the final-window sticky cohort
   **includes ≥1 inherited-tool holder**: an heir who received a plow via a real post-founder-death transfer
   and is in the cohort. I.e. the cohort persists *past* the founders, not just because founders lived the
   whole run.
4. **A surviving non-owner buyer cohort materially buys** — post-promotion bought food ≥ `MATERIAL_BOUGHT_FLOOR`,
   living; the market is not just owners feeding themselves.
5. **Money survives** — SALT promotes and remains money; food materially bought after promotion.
6. **Provenance clean** — sold/pre-promotion bread is `SelfProduced`; `seeded_minted == 0`.
7. **Capital is endowed/inherited, not fiat or minted** — endowed tools counted by `endowed_tools_total` and
   included in the initial conservation baseline; inheritance is a conserved estate **transfer** (never a
   mint); conservation holds every tick; `bread_minted_max == 0`; the tool-stock invariant holds (§3.5).
8. **NOT downgraded by the controls (§4)** — no-inheritance does not also clear over the post-death window;
   owner-share ≤ `OWNER_SHARE_MAX` (0.6); productivity-only is not sticky.

`IdStickySuccess` (persistent owner **ids**) is computed and **reported as a secondary metric**, never the
primary gate (Codex P1 #3: per-agent persistence can hold without inheritance mattering, and vice versa).

**Finding modes (pre-named; first-class; the verdict test prints the classification, does NOT assert SUCCESS):**
- `EndowmentLeverInert` (precondition / non-vacuity failure) — endowment didn't bite: no endowed owner ever
  out-produces a non-owner, OR no plow ever transfers to a living heir on an owner's death, OR no owner enters
  the retention signal.
- `ConservationBroken` / `extinct` — any conservation failure, minted/seeded-bread leak, boost-without-tool-
  stock, tool-stock/estate accounting mismatch, or colony death.
- `InheritedMonopoly` — owner-class grain share ≥ `MONO_SHARE` (0.75) AND the non-owner buyer cohort collapses
  (post-promo bought < `MATERIAL_BOUGHT_FLOOR` / buyers die): a dynasty that starves the market.
- `CommuneCollapse` — cultivator share ≥ `COMMUNE_SHARE` AND post-promo bought < floor.
- `MoneyFailureFromLockIn` — SALT fails to promote / demonetizes because the topology kills medium use.
- `EndowmentOnlyScaffold` (the honest-trap mode) — SUCCESS-like metrics hold BUT (a) the **no-inheritance**
  control *also* clears them over the post-death window (so the persistence is the static initial seed acting
  as a one-generation pin, not inheritance stabilizing a lineage), OR (b) the buyer cohort survives only via
  the configured emergency floor with ~zero market purchases (re-pinned producers, dead market).
- `ProductivityOnly` — the **productivity-only** control (same boost to all, no owned/endowed asset) also
  clears the stickiness bars (downgrade: it was productivity, not capital).
- `UniversalOwnership` — owner-share > `OWNER_SHARE_MAX` (ownership is not a minority; e.g. the too-many-tools
  control): topology, not an occupation.
- `NoStickinessDespiteEndowment` — the lever bites (owners out-produce, plows inherit) but churn > 0.5×
  baseline AND no persistent owner-lineage cohort: even capital given up front and inherited does not retain.
- `LineageStickySuccess` — all eight primary conditions, not downgraded above.

**Ordered classifier (top-down, first-match-wins — the S21i non-gameability discipline):**
`EndowmentLeverInert` → `ConservationBroken`/`extinct` → `InheritedMonopoly` → `CommuneCollapse` →
`MoneyFailureFromLockIn` → `UniversalOwnership` → `ProductivityOnly` → `EndowmentOnlyScaffold` → **then the
explicit final success gate (Codex P2):** `if ALL EIGHT success clauses (§2.1–§2.8) pass { LineageStickySuccess }
else { NoStickinessDespiteEndowment }`. So `LineageStickySuccess` is emitted ONLY when every success clause
holds; any partial miss not caught by an earlier mode falls to `NoStickinessDespiteEndowment`. Predeclare every
threshold as a `const`; do NOT fit.

## 3. Engine design (additive, default-off, conservation-safe)

1. **NEW default-off flag** `ChainConfig::endowed_cultivation_capital: bool` + config fields:
   `endowed_tool_count: u16` (how many lineage households are seeded with a plow at generation — a MINORITY:
   owner-share ≤ ~0.3 at the shipped value, swept), and `cultivation_tool_inheritance: bool` (default true
   when the gate is on; the no-inheritance control sets it false). Helper
   `endowed_cultivation_capital_active(&self)` = flag on AND `durable_cultivation_tool_active()` (which already
   requires S22a+S22c). Canonicalize ON-only with the **next free flag-digest tag (11** unless master advanced)
   + `endowed_tool_count` + `cultivation_tool_inheritance` + the endowment-selection params. Off ⇒
   byte-identical.

2. **Endowment at generation (Codex P3 — selection; P2 — accounting).** When active, select `endowed_tool_count`
   **lineage households** by a **deterministic hash of `(seed, household_id)`** over the eligible household set,
   sorted by hash (NOT lowest-ids, which can select a special roster segment). Grant each selected household's
   founding member one `CULTIVATION_TOOL` (the existing S22d good) into stock at generation. Accounting: there
   is no tick report at generation, so record the grant via an `endowed_tools_total` counter and include it in
   the **initial whole-system stock / conservation baseline** (do NOT claim `EconTickReport::endowment`). Digest
   the selected household ids (or the selection params) ON-only; report the selected vocation/household mix.

3. **Plow estate-routing switch (Codex P1 #1).** Tools already inherit to the household heir via
   `settle_estate_to_heirs`. Add a plow-specific routing decision in the estate-map step **after**
   `collect_estate` (not a pre-transfer before estate collection): if `cultivation_tool_inheritance == true`,
   plows follow the existing heir routing (lineage heir; commons fallback if no heir); if `false`, plows are
   **forced to the commons** even when the rest of the estate goes to the heir. A pure stock transfer in both
   cases — conservation-safe, never a mint, tool-stock total unchanged. Off (gate inactive) ⇒ the existing
   behavior is completely untouched (goldens byte-identical). **Impl note (Codex):** implement as a single
   **stock-map partition before placement** — partition the collected estate map into {plows} vs {everything
   else}, route each partition to its destination (heir or commons) exactly once, then place; do NOT
   pre-transfer plows before `collect_estate` (that risks a double-count).

4. **Everything else is S22d/S22c, unchanged.** The owner-exclusive haul boost (owner holds a plow → higher
   grain-haul ceiling while cultivating; non-owner = the S22c no-tool return), the `cultivation_tenure`/build
   path (still available; endowed owners may also build, but the headline isolates endowment+inheritance), and
   the **unmodified S22c `profit_stay_active`** as the ONLY retention mechanism. NO exit-branch edit, NO fiat
   "endowed/owner agents always cultivate" flag — an owner stays only because its durable advantage makes its
   realized cultivation return clear its outside option through the existing profit-stay.

5. **Tool-stock accounting invariant (Codex P2 #5).** `endowed_tools_total + tools_built − tools_destroyed ==
   whole_system_total(plow)` (live-agent stock + commons), asserting **non-negativity first** (the S22d P3
   lesson). NO "in-flight" term — a plow exists only after project completion; an in-flight build has consumed
   WOOD but produced no plow. Inheritance moves a plow between holders and never changes the total.

6. **Diagnostics (runtime-only):** endowed-owner ids + selected households; plow inheritance transfers (count +
   heir ids + whether the heir joins the sticky cohort); owners by lineage; owner-share among ever-cultivators;
   owner ∩ persistent-cohort overlap (by lineage AND by id); per-owner realized cultivation proceeds; non-owner
   buyer cohort + post-promo bought; churn vs matched-seed baseline; the post-founder-death window markers.
   Owner status reads the plow GoodId stock (NOT `acquired_tool`).

## 4. The new suite `sim/tests/endowed_inherited_capital.rs`

- **MANDATORY NON-VACUITY TEST** (else `EndowmentLeverInert`): (a) an endowed owner harvests STRICTLY MORE
  grain AND produces STRICTLY MORE bread than a non-owner under matched conditions (reuse the S22d
  owner-vs-plain harness); (b) **at least one real inheritance transfer occurs** — an endowed owner dies
  holding a plow, a living household heir receives it (tool-stock total unchanged), AND that heir subsequently
  cultivates; (c) ≥1 owner enters the retention signal.
- **The ordered classifier (§2)**, printed `--nocapture`; verdict test prints verdict + deciding metrics
  (primary `LineageStickySuccess` + secondary id-stickiness), does NOT assert SUCCESS.
- **Scenarios (EXPANDED BASE, Codex P1):** `frontier_endowed_capital` (HEADLINE) = `frontier_cultivation_capital`
  (S22d) **expanded to `ROSTER_HOUSEHOLDS` (≥8) lineage households** + `endowed_cultivation_capital=true` +
  `cultivation_tool_inheritance=true` + a minority `endowed_tool_count`. The roster is expanded
  **proportionally** — preserve the WOOD-poor cultivator/woodcutter/consumer ratios, the S21h survival floor,
  and the grain commons — so the base still sustains money + mortality and is NOT a broken colony. **ALL matched
  baselines and controls run on the SAME expanded base** (the "matched S22d/S22c baseline" for churn is this
  scenario with `endowed_cultivation_capital=false`, NOT the original 2-household `frontier_cultivation_capital`).
- **PRECONDITION CHECK (a test):** on the expanded base with the gate OFF, the colony must reproduce S22d-style
  `NoStickinessDespiteCapital` (money promotes, mortality coexists, no owner cohort, churn high). If the
  expanded base itself can't sustain money+mortality or already shows stickiness, that is a **base problem** to
  report (not an S22e success) — the headline is only interpretable against a working, still-fluid base.
- **Controls (each a test):**
  - **flag-off** = `frontier_cultivation_capital` (reproduces S22d `NoStickinessDespiteCapital`).
  - **no-inheritance** (endow on, `cultivation_tool_inheritance=false`, plows forced to commons): evaluated
    over the **same post-founder-death window** as the headline. If the headline cohort persists past the
    founders but this one does NOT, inheritance is load-bearing; if the headline only matches this within one
    generation, the headline is `EndowmentOnlyScaffold`.
  - **no-endowment** (inheritance on, `endowed_tool_count=0`): reproduces S22d (tools must be earned) →
    `EndowmentLeverInert`/`NoStickiness`.
  - **productivity-only** (same haul boost to ALL cultivators, no owned/endowed/inheritable asset): classify;
    if it clears the bars the headline downgrades to `ProductivityOnly` (Codex P2 #7 — a verdict, not a hard
    assert).
  - **too-many-tools** (`endowed_tool_count` ≈ universal, owner-share > `OWNER_SHARE_MAX`): must classify
    `UniversalOwnership`, NOT `LineageStickySuccess`.
  - **free/large-endowment sensitivity** classified `SENSITIVITY`, excluded from the core verdict.
- **HARD GUARDS every run + cell:** conservation every tick; `bread_minted_max == 0`; provenance clean-or-
  disqualified; `!extinct`; the tool-stock invariant of §3.5 (non-negativity first; no in-flight term;
  inheritance preserves the total).
- **goldens_unchanged** test pinning the five tripwire digests (copy from `durable_cultivation_capital.rs`).
- **Robustness mini-sweep** over `endowed_tool_count` (minority→universal) and the haul ceiling + grain flow,
  classified, no tuning. The `endowed_tool_count` axis MUST be *outcome-driving* (the S21i vacuous-axis
  lesson): show owner-share and the verdict actually move with it (minority → potential success;
  universal → `UniversalOwnership`).
- A `plow_never_trades`-style guard (S22d P2): endowed/inherited plows still never clear in trade.

## 5. Verification (independent gate)

Redirect cargo to files; never pipe to head/grep (EPIPE → spurious exit 101).
- `cargo test -p sim --test endowed_inherited_capital` passes (incl. non-vacuity with the real inheritance
  transfer + the isolating controls).
- `cargo test --workspace` passes; **all existing goldens byte-identical** (`goldens_unchanged` +
  durable_cultivation_capital / profit_driven_retention / occupational_stickiness / endogenous_cultivation_entry
  / robustness_appendix / household_barter / mortality / open_colony_mortality / demand_survival_bridge).
- `cargo fmt --check` clean; `cargo clippy --workspace --all-targets -- -D warnings` clean; conservation holds.

## 6. Honesty and scope (state these in the result; do not let a SUCCESS overclaim)

- **A SUCCESS is institutional / endowment / dynastic sufficiency, NOT endogenous occupation, and NOT a
  non-lineage occupational class.** Honest headline: *"durable, endowed, inheritable capital can stabilize a
  cultivator **lineage** that earned capital (S22d) could not."* S22a–d already showed occupation does not
  self-form from a fluid base; S22e (if it succeeds) shows a *configured institution* can stabilize one.
- **The decisive evidence is post-founder-death persistence + the no-inheritance control.** Without the
  inherited-tool-holder-in-the-cohort clause and the commons-forced control, a SUCCESS is indistinguishable
  from re-pinning the S21 producer lineage via a static seed; the classifier downgrades to
  `EndowmentOnlyScaffold` if they don't separate.
- **Endowment restricted to lineage households is a disclosed limitation** (inheritance can't operate on
  non-lineage roles); the claim is correspondingly about a lineage/dynastic class, not the broad occupation.
- **Bounded to this WOOD-poor colony / this endowment count / mortality-on regime.** Like S21h/i, expect the
  result may be band-qualified; report the `endowed_tool_count`, haul, and grain windows where it holds.
- **Endowment + the inheritance switch are configured institutions**, not emergent — disclose them exactly as
  the S20 clearing institution and the S21h survival floor are disclosed. The contribution is the
  *conditional*: which configured institution suffices to stabilize the occupation the incentive levers could
  not.
- Follow repo conventions; NEVER add Claude/AI/assistant references in code, comments, or committed text.

## 7. (reserved)

## 8. Codex spec-review resolutions (round 1 → this revision)

- **P1 inheritance premise** — corrected: `settle_estate_to_heirs` already inherits tools to the household
  heir; the lever is a plow-specific estate-routing **switch** (false ⇒ force commons) applied **after**
  `collect_estate` (§1, §3.3).
- **P1 eligibility** — resolved: endow **lineage households only** (inheritance needs `household`); claim
  downgraded to an endowed **lineage/dynastic** class, not a non-lineage occupation (§1 design decision, §6).
- **P1 id-or-lineage gameable** — resolved: primary metric is `LineageStickySuccess`; id-stickiness is a
  reported secondary (§2).
- **P1 no-inheritance proves inheritance only post-transfer** — resolved: success requires an inherited-tool
  heir in the **final-window** sticky cohort (post-founder-death), and the no-inheritance control is evaluated
  over that same window (§2.3, §4).
- **P2 tool-stock invariant in-flight** — resolved: `endowed + built − destroyed == whole_system_total`, no
  in-flight term, non-negativity first (§3.5).
- **P2 generation endowment accounting** — resolved: `endowed_tools_total` counter + initial conservation
  baseline, not `EconTickReport::endowment` (§3.2).
- **P2 productivity-only / too-many-tools** — resolved: first-class `ProductivityOnly` / `UniversalOwnership`
  verdict downgrades, tests classify rather than hard-assert (§2 modes, §4 controls).
- **P3 selection by hash** — resolved: deterministic hash of `(seed, household_id)`, sorted, digested ON-only;
  report the selected mix (§3.2).

### Round 2 (→ SPEC-READY)
- **P1 success floor impossible on the 2-household base** — resolved: the headline + all matched controls run
  on an EXPANDED roster of ≥ `ROSTER_HOUSEHOLDS` (8) lineage households (proportionally expanded to preserve
  money+mortality), with the coherence constraint `PERSIST_COHORT ≤ endowed_tool_count ≤` minority of the
  roster, and a precondition test that the gate-off expanded base reproduces S22d `NoStickiness` (§2.2, §4).
- **P2 explicit final success gate** — resolved: the classifier ends with `if all eight success clauses pass
  { LineageStickySuccess } else { NoStickinessDespiteEndowment }` (§2 ordered classifier).
- Codex confirmed sound: corrected inheritance premise, lineage restriction, the post-founder-death /
  no-inheritance triad (non-circular), and the after-`collect_estate` plow routing (conservation-safe as a
  single stock-map partition before placement — folded into §3.3).
