# impl-53 — C6: Technology and the Knowledge Ladder (does roundabout production climb only when capital and patience allow?)

Status (spec): **DRAFT — pending Codex spec-review.** Layer C6 of the civilization-core roadmap
(`docs/spec-civ-core-roadmap.md`). Generalizes the merged **G6b** research substrate; composes on **S7**
producible capital + **S10** originary interest; benefits from a living economy (C1) to fund research
from real surplus. Flag `tech_ladder`, digest **tag 27**, ON-only.

Falsifiable bar (headline): does a society with **lower time preference** invest surplus in research,
climb a ladder of knowledge-gated recipes, and reach higher-productivity methods — while a present-biased
one does not — i.e. is tech adoption **capital- and time-preference-gated**, not free?

## 0. Dependency & premise (read first)

C6 is the faithful "tech tree." Its praxeological spine (Mises): **technological knowledge unlocks
*possibility*, but capital is the binding constraint** — knowing a longer, more productive process is
useless without the saved capital to embody it and sustain workers through its longer waiting time — and
**time preference** (S10) decides whether a society actually takes the longer road. So C6 is not "unlock
tech → get more output"; it is "unlock *possibility*; capital accumulation (S7) + patience (S10) decide
realization."

C6 depends least on C1's specific verdict of the Wave-1 milestones — the G6b tech mechanism already works
on master — **but for research to be an *economic* choice** (surplus diverted from present consumption to
scholars), it needs a functioning economy, so it is still cleaner on a C1-living base. Provisional in the
weak sense: the *mechanism* is testable now, the *economic significance* sharpens once buyers can earn.

## 1. Praxeology — possibility vs. realization

- **Knowledge is produced, not gifted.** A research sector (scholars) produces knowledge from labor + a
  real input (grain) at the expense of present output — an **intertemporal choice** (S10). Never a timer.
- **A tech unlock enables a recipe/building *definition* only.** It sets **no** price, wage, or quantity;
  it flips a recipe from `enabled:false` to `enabled:true`. The market decides whether the new method is
  *worth* adopting (relative prices), and capital (S7) decides whether it *can* be run (the tool must be
  built).
- **Anti-smuggling:** knowledge stays a **non-rival, non-conserved accumulator** (§2.1) — it is *not* a
  tradeable good and must never enter the goods-conservation ledger; and a low-time-preference society
  must genuinely *forgo present consumption* to fund research, not receive it free.

## 2. What already exists (G6b is C6's kernel)

The G6b substrate is remarkably complete and C6 **generalizes** it rather than inventing it:

- **The research recipe:** `Research` (content.rs:277 — `labor + GRAIN_PER_RESEARCH grain + LIBRARY tool
  → KNOWLEDGE`), the `Scholar` vocation that runs it (`run_production`, settlement.rs:12536).
- **Tier-gated recipes:** `Confect` (content.rs:286) ships `enabled:false`; the executor honors
  `Recipe.enabled` (`execute_direct_recipe_for_agent_checked` returns `None` for a disabled recipe,
  settlement.rs:12565); `maybe_unlock_tier_two` (settlement.rs:12614) flips it `true` **one-way** when
  `self.knowledge ≥ chain.tier2_threshold`.
- **Capital as the constraint:** every recipe (Mill/Bake/Research/Confect) has a `required_tool`
  (content.rs:194/205/281/291); the tool must be **built** (S7 `BuildMill`/`BuildOven` capital formation,
  settlement.rs:14607) — a knowledge-unlocked recipe still needs its tool produced from WOOD+labor.
- **Time-preference funding:** per-agent `time_preference_bps` (settlement.rs:21237) sets the
  multi-horizon savings-ladder depth in `appraise_capital_tool_bundle_for_money` (settlement.rs:22451);
  patient agents finance longer projects.
- **Era detection:** `era.rs` measures institutional eras; the tech tier (`1`/`2`, settlement.rs:17556)
  is a separate content axis that may diverge from eras (game-spec §5.8).

### 2.1 The load-bearing invariant: knowledge is NOT conserved (preserve exactly)

C6 must preserve G6b's exact treatment: KNOWLEDGE is **drained out of goods conservation** and reported
on its own non-conserved line. On a research tick the scholar's knowledge output is *immediately debited*
from its econ stock into a per-settlement counter (`self.knowledge`), so it never enters circulation, the
digest goods-ledger, or the conservation identity (settlement.rs:12577–12587; `knowledge_produced`
:5493/5535; the identity explicitly excludes it :5574–5590; `goods()` excludes it, content.rs:619). The
*conserved input* (grain) is accounted in `consumed_as_input`; only the non-rival knowledge accumulator is
excluded. **C6 must not describe knowledge as a conserved good, and must not let it enter the ledger.**

## 3. Mechanism — generalize the single tier into a ladder

- **A ladder, not one tier.** Replace G6b's single `tier2_threshold`/`tier2_recipe_id` with an ordered
  **ladder** of `(knowledge_threshold, unlocked_recipe/tool_def)` rungs; each rung unlocks one-way when
  cumulative `self.knowledge` crosses it (reuse `maybe_unlock_tier_two`'s one-way flip per rung). Higher
  rungs are more productive (higher `output_qty` per labor) but need a more advanced `required_tool` that
  must be built (S7) — so climbing needs **both** knowledge (possibility) and capital (realization). **Each
  rung needs authored content (spec-review P2):** new recipe/tool/output good IDs in `content.rs` (not just a
  generalized loop over the one Confect example), and each rung's threshold + unlocked-recipe id + unlock-tick
  is digested under tag 27 (extending the G6b :20622 serialization). The headline authors ≥2–3 principled
  rungs, swept.
- **Research as an endogenous, ordinal funding choice (new; spec-review P1).** G6b today has *configured*
  scholars and tiers — **not** a funding market — and the S7/S10 appraisal gates *capital-tool* builds, not
  research (settlement.rs:1509/22451). C6 must define the funder and the ordinal object explicitly: a
  **funder** (a C2 firm, or a patient saving household) funds a **research project** — a `FundResearch`
  project modeled on the S7 capital-tool project but whose payoff is the *expected future margin of the
  recipe the next rung would unlock* — only if, on the funder's **own** ordinal savings ladder (S10, the
  `appraise_capital_tool_bundle_for_money` pattern), the present cost (grain + labor + the scholar's C1
  wage) is outranked by that future receivable provisioning one of its own future-money wants. A
  present-biased funder declines. **No planner, no aggregate productivity number** — one funder's ordinal
  intertemporal appraisal, exactly like a capital build; this funding market is new code, not the existing
  configured scholars. Knowledge diffusion across settlements rides trade contact (C8), when present.
- **No new economics for adoption.** Whether a newly-unlocked recipe is actually run is the existing
  recipe-profitability/role-choice decision at realized prices; C6 only changes *what is possible*.

## 4. Praxeology / anti-smuggling guards

1. **Knowledge non-conserved (the invariant).** Preserve G6b's drain-out treatment exactly; a test
   asserts knowledge never enters `goods()`/the conservation identity, and that research grain input *is*
   in `consumed_as_input`.
2. **Possibility ≠ output.** A tech unlock only flips `Recipe.enabled`; it sets no price/wage/quantity. A
   `free_tools` control (unlock without requiring the built tool) must be separated — it would break the
   capital-constraint and is the anti-smuggling twin.
3. **Capital + patience gate.** The headline compares a patient vs. a present-biased colony (per-agent
   `time_preference_bps`); the patient one climbs, the present-biased one does not.
4. **Produced, not gifted.** Research consumes real grain + labor (opportunity cost); no timer unlock.
5. **One-way unlocks.** Rungs flip once (determinism; no flapping), as G6b already does.

## 5. Conservation & determinism

- **Conservation.** Unchanged from G6b: knowledge outside the ledger, grain input conserved. Building the
  higher-tier tools reuses S7's conserved capital formation (WOOD→consumed_as_input, tool→produced).
- **Digest (tag 27, ON-only).** Generalize G6b's serialization (settlement.rs:20622 currently pushes
  `tier2_threshold`, `knowledge`, `tier2_unlocked_at`) to the **full ladder**: `if self.tech_ladder_active()
  { out.push(27); ... per-rung thresholds + unlocked-tick(s) + cumulative knowledge }`. Every rung's
  threshold and unlock-tick **steer** future unlocks → digested (the determinism contract, G6b test 1).
  Off-path (`tech_ladder` false, i.e. the plain single-tier G6b or no research): byte-identical.
- **Determinism.** Unlocks are deterministic functions of the monotone knowledge counter. Integer-only.

## 6. Slices

- **Slice A — the ladder.** Generalize `tier2_*` to an ordered rung list; per-rung one-way unlock; the
  tag-27 digest of the full ladder. *DoD: multiple rungs unlock in order as knowledge accumulates;
  goldens byte-identical off (single-tier G6b unchanged).*
- **Slice B — capital + patience gating.** Higher rungs need built tools (S7); research funded by patient
  agents' surplus (S10). *DoD: a patient colony climbs and builds the tools; a present-biased one does not;
  the `free_tools` control separates.*
- **Slice C — acceptance suite + controls** (§7).

## 7. Acceptance suite (`sim/tests/tech_ladder.rs`)

`SEEDS=[3,7,11,19,23]`, long horizon.

- **Predeclared thresholds (swept):** ladder rung count reached, knowledge-accumulation rate,
  productivity lift per rung, patient-vs-present-biased build counts.
- **Ordered verdict enum:** base-precondition (knowledge conservation invariant / determinism) → outcome:
  `RoundaboutClimbs` (a patient colony funds research, climbs ≥K rungs, builds the higher-tier tools, and
  reaches higher output-per-labor; a present-biased colony climbs fewer/none) / `TechWithoutCapitalInert`
  (rungs unlock but the higher methods are never realized because the capital isn't built / not worth it).
- **Mandatory non-vacuity:** knowledge accumulates and ≥K rungs actually unlock; a real counterfactual —
  a patient colony builds a higher-tier tool that the matched present-biased colony does not.
- **Controls:** `present_biased` vs `patient` (time_preference sweep, outcome-driving); `free_tools`
  (unlock without the built-tool requirement → scaffold, breaks the capital constraint); `no_research`
  (scholars off → ladder never climbs); `tech_ladder_off` reproduces single-tier G6b.
- **`goldens_unchanged()`:** with `tech_ladder` off, byte-identical (single-tier G6b digest preserved);
  the knowledge-non-conservation regression (research grain in `consumed_as_input`, knowledge excluded).

Build/verify: `cargo test -p sim --test tech_ladder -- --nocapture`, `cargo test --lib`, fmt, clippy
`-D warnings`, workspace green; the G6b suite green.

## 8. Risks & open questions

1. **Knowledge-conservation regression (top).** The load-bearing invariant; any drift that lets knowledge
   into the ledger is a hard failure — a dedicated regression guards it.
2. **Economic significance depends on C1.** Without a living economy, research is funded from a thin
   surplus and the ladder may barely climb — a scoped finding about whether patience alone (without a rich
   market) suffices.
3. **Content authorship.** A real ladder needs authored higher-tier recipes/tools (content), not just the
   G6b Confect example; C6 should define ≥2–3 rungs of principled content, swept, not one.
4. **Tech vs. era divergence.** Tech tier (content) and institutional era (measured) may diverge — that's
   intended (game-spec §5.8), reported, not forced to co-move.

## 9. Falsifiable-bar summary

Generalizing G6b's single knowledge-gated tier into an ordered ladder — knowledge produced by scholars at
the expense of present consumption (S10 intertemporal choice), each rung unlocking a more productive
recipe whose tool must still be built (S7 capital) — should make roundabout production **climb only when
capital and patience allow**: a patient colony funds research, ascends rungs, builds the higher-tier tools,
and reaches higher output-per-labor, while a present-biased one does not (`RoundaboutClimbs`), with
`free_tools` separating as scaffold and knowledge staying strictly non-conserved. The honest alternative
is `TechWithoutCapitalInert` — rungs unlock but the methods aren't realized — a first-class finding that
capital, not knowledge, is the binding constraint (the Misesian point, confirmed or refined).
