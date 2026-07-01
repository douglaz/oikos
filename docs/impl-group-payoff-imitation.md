# impl-43 — S24c: Group-payoff imitation (does group-level selection preserve the institution?)

Status (spec): DRAFT — pending Codex spec-review. Base: master `e654443` (S24b landed). **Third slice of the
S24 INSTITUTION-SELECTION arc** — the clean-positive test via a genuinely new mechanism (Codex-scoped:
"group-payoff imitation, not hysteresis"). Composes on S24b (`abandonable_norm`), changing exactly one thing:
imitation is scored on **local GROUP welfare**, not individual welfare.

## 0. One-paragraph summary

S24a + S24b bracket a knife-edge: sticky commitment-norm spread over-spreads / can't be told from drift, and
*abandonable* spread (S24b) **dissolves** the institution — because on a generic *individual*-welfare score the
committed CULTIVATORS are not better off than the well-fed fluid BUYERS, so imitation-of-success flows *away*
from the productive role (the "tragedy of imitation"). S24c tests the natural next theory: **institution
selection may need to see GROUP-level outcomes, not individual welfare.** Keep S24b's abandonable adoption, but
replace the individual model score with a **local group score**: at each imitation step an agent compares
nearby candidate GROUPS (all live agents within `GROUP_RADIUS` of a centre) scored *only* on **generic welfare
aggregates** (alive share, mean hunger-relief, mean recent-food — no SALT in the headline, and **never** any
adopter/committer/vocation/profit field), and copies the **modal `adopts_commitment_norm` bit of the best-off
group**. The idea: a neighbourhood that *contains* committed producers and therefore eats well collectively can
propagate the norm, even though the individual producer is not the best-off agent — so the practice spreads
because the group carrying it has better generic outcomes. The central anti-smuggling guard (Codex): "groups
with committers do better" is **not** smuggling *iff* the score never reads commitment identity AND the
controls show the advantage **vanishes** when commitment is made unprofitable or the group choice is randomized
— the agent sees *welfare* and copies a *visible norm*, it never "chooses the group with the most committers".
The hypothesis (the arc's potential first *clean* positive): group-payoff imitation gives **bounded,
selection-driven spread that recreates the S22f core in the majority of seeds, with the blind null failing**,
where individual-welfare imitation killed it. If it fails cleanly, the knife-edge (S24a+S24b) is the arc's
terminal result. Classify-not-tune; money + mortality + provenance + conservation hold; goldens byte-identical
off.

## 1. Why this milestone, why this change — and the grounding

S24b's `NormDiesBack` has a precise cause: individual-welfare imitation rewards the *comfortable buyer*, not
the *committed producer* who makes the buyers' food possible, so the institution loses under free abandonment.
The natural repair is **not** to change the stickiness (hysteresis — deferred; it risks being "S24a but
softer") nor to credit the role directly (role-crediting — deferred; it almost inevitably reads
institution/commitment identity and breaks score-purity), but to change **what unit success is measured over**:
if agents imitate the practice of the better-off *neighbourhood* rather than the better-off *individual*, a
group that internalizes the producers' contribution (they feed the group) can carry the norm. This is the
sharpest, least-circular first test of whether institution selection needs group-level payoff.

**Grounding (S24b reused unchanged except the scoring unit):**
- S24b already gives: the `adopts_commitment_norm` gate on S22f commitment entry, the abandonable bidirectional
  update, the staged-at-expiry abandonment (`next_norm_bit`, no mid-term break), the deterministic seed, the
  generic per-agent welfare score (`2·alive + hunger_relief + food_score`, SALT excluded, score-purity), the
  copy-driver / adopter-advantage diagnostics, the flip accounting, and the control scaffolding.
- The only change is: the imitation **score is computed over a local GROUP**, and the copied bit is the group's
  **modal** norm bit (§3).

**Design decision (Codex):** group-payoff imitation over hysteresis/role-crediting; group = local spatial
cluster; group score = generic welfare aggregate only; copy the modal norm of the best group; keep S24b
abandonability.

## 2. The central question and pre-named outcomes

**Central question.** When S24b's abandonable adoption is driven by **local group-welfare** imitation — an
agent copies the modal `adopts_commitment_norm` bit of the best-off nearby GROUP (scored only on generic alive
/ hunger / food aggregates, never on institution identity) — does institution selection become **clean**: a
**bounded** adoption level that recreates the S22f committed core in the **majority** of seeds, with the
matched `random_group_imitation` null failing to reach the core and money/mortality/provenance/conservation
holding — where individual-welfare imitation (S24b) killed it, and without any institution/profit term in the
score?

**Primary success = `CleanInstitutionSpread`** (≥3/5 of `SEEDS = {3,7,11,19,23}` AND zero
`GroupDriftNotSelection`, vs the matched-seed `individual_score_control` / `random_group_imitation` /
`no_imitation` nulls and the `sticky_reference`):
1. **Bounded adoption equilibrium** — final adopter share ∈ `[ADOPTER_SHARE_MIN (0.15), ADOPTER_SHARE_MAX
   (0.6)]`.
2. **Persistent committed core** — by the S24b/S22f metrics (≥ `PERSIST_COHORT` ids cultivating ≥
   `PERSIST_FRACTION` of the final window, all renewing).
3. **Surviving fluid non-adopter buyer tier materially buys** (post-money bought ≥ `MATERIAL_BOUGHT_FLOOR`,
   `final_buyer_cohort ≥ MIN_BUYER_COHORT`).
4. **PER-SEED clean group selection with a MARGIN** — the headline beats its matched `random_group_imitation`
   seed by `CORE_MARGIN (= PERSIST_COHORT)` AND that random group null does not itself satisfy the bounded
   two-tier success; AND `no_imitation` forms no core; AND ≥1 copy is driven by a **positive pre-copy GROUP
   welfare advantage** (the best group really out-welfared the agent's own before the copy); score-purity
   holds; SALT contributes zero copies.
5. **Non-seed participation** — ≥ `MIN_NONSEED_ADOPTERS` non-seed agents adopt and ≥ `MIN_NONSEED_COMMITS`
   non-seed commitments (+ ≥1 renewal); the core is not merely the seed cluster (`SeedClusterOnly` fails).
6. **Money + mortality + provenance + conservation hold** (SALT promotes; `seeded_minted == 0`; bread
   `SelfProduced`; conservation every tick).
7. **The signal is group-payoff SELECTION** — `individual_score_control` (S24b score) reproduces `NormDiesBack`
   (so the win is from group scoring), and `random_group_imitation` does not reach the core (so it is not
   drift), and `unprofitable_commitment` does not spread (so the group advantage is real).

**Finding modes (pre-named; first-class; verdict prints, does NOT assert SUCCESS):**
- `SeedDies` / `MoneyFailure` / `ConservationBroken` / `extinct` / `Cull` — precondition / integrity failures.
- `GroupSignalVacuous` — no observed group-welfare advantage precedes copies (the group signal doesn't bind).
- `NormDiesBack` — group signal *still* selects away from commitment (final share → 0, no core): the
  group-payoff theory FAILS; the tragedy of imitation holds even at group level.
- `UniversalCommitmentRePin` — group imitation over-spreads past `ADOPTER_SHARE_MAX` and the buyer tier
  collapses.
- `GroupDriftNotSelection` — the matched `random_group_imitation` null reaches a comparable core (spread is
  drift, not group-payoff selection).
- `SeedClusterOnly` — the seeded adopter cluster forms/holds a core but the norm does not spread beyond the
  seed (no material non-seed adoption/commitment).
- `SpreadWithoutOccupation` — bounded norm spread but no stable committed core.
- `CleanInstitutionSpread` — all seven success clauses (the arc's first clean positive).

**Ordered classifier (top-down, first-match-wins):** `SeedDies` → `MoneyFailure` →
`ConservationBroken`/`extinct`/`Cull` → `GroupSignalVacuous` → `NormDiesBack` → `UniversalCommitmentRePin` →
`GroupDriftNotSelection` → `SeedClusterOnly` → `SpreadWithoutOccupation` → **then the final gate:** `if all
seven success clauses pass { CleanInstitutionSpread } else { SpreadWithoutOccupation }`. Milestone SUCCESS =
**≥3/5 `CleanInstitutionSpread` AND zero `GroupDriftNotSelection`** (strict, per S24b). Predeclare every
threshold as a `const`; do NOT fit.

## 3. Engine design (additive, default-off, conservation-safe)

1. **NEW default-off flag** `ChainConfig::group_payoff_imitation: bool` + consts: `GROUP_RADIUS` (Manhattan
   radius for a group, default e.g. `IMITATION_RADIUS`), `GROUP_MIN_SIZE` (min live members for a group to be
   scored, default 3), reuse of all S24a/S24b consts (`IMITATION_PERIOD/WINDOW/MARGIN_BPS/MAX_MODELS`, the
   score weights, `ADOPTER_SHARE_MIN/MAX`, `CORE_MARGIN`, `MIN_ABANDONMENTS`, `commitment_seed_share`) + the
   control toggles (§4). Helper `group_payoff_imitation_active(&self)` = flag on AND `abandonable_norm_active()`
   (S24c modifies S24b's update; if S24b is off the flag is inert). Canonicalize ON-only with the **next free
   flag-digest tag (17** unless master advanced) + the flag + group bookkeeping that steers behaviour. Off ⇒
   byte-identical (S24b individual-score behaviour preserved).

2. **The one change — group scoring (§0 design).** When `group_payoff_imitation_active()`, the per-agent
   imitation step (still bidirectional/abandonable, same `IMITATION_PERIOD`, same abandonment-timing state
   `next_norm_bit`) is scored on GROUPS instead of individuals:
   - **Candidate groups (deterministic):** the agent's OWN group (`group(self)`) plus one group centred on each
     agent in the S24b observation set (within `IMITATION_RADIUS`, capped `IMITATION_MAX_MODELS`). A
     `group(center)` = all **live** agents within `GROUP_RADIUS` (Manhattan) of `center`, deterministic;
     groups with `< GROUP_MIN_SIZE` live members are skipped.
   - **Group score (generic, PINNED):** over `IMITATION_WINDOW`, `group_score = w_alive·alive_share(group) +
     w_hunger·mean_hunger_relief(group) + w_food·mean_food_score(group)`, each component normalized to `[0,1]`
     (integer bps) exactly as the S24b per-agent components (`alive_share` = fraction alive; `hunger_relief` /
     `food_score` = the S24b per-member terms, averaged), weights `w_alive=2, w_hunger=1, w_food=1`. **SALT is
     NOT in the headline group score** (a `salt_in_score` sensitivity only). The group score **MUST NOT** read
     any member's adopter/committer/commitment/vocation/profit field (the score-purity invariant, now at group
     level).
   - **Copy:** if the best-scoring observed group beats the agent's OWN group's score by ≥
     `IMITATION_MARGIN_BPS`, the agent copies the **modal `adopts_commitment_norm` bit** of that best group
     (majority vote over the best group's live members; ties → keep current bit). The norm bit is read ONLY to
     compute the modal value to copy — never to score. Apply the copied bit through S24b's
     `stage_or_apply_commitment_norm_bit` (immediate if unbound, staged to expiry if in a binding term).
   - Record the **group-advantage** diagnostic (best-group score − own-group score at copy time) and the
     copy-driver (which generic aggregate dominated), to prove the driver is generic group welfare.

3. **Everything else is S24b/S24a/S22f unchanged** — the abandonability, the staged-at-expiry timing, the
   commitment-entry gate, the deterministic seed, the commitment mechanism, money/mortality/conservation. NO
   fiat, NO "commitment is good"/"most committers" term, NO `Vocation` mutation, NO reading norm/commitment in
   the score. Flag + group bookkeeping serialized ON-only under tag 17.

4. **Diagnostics (runtime-only):** adoption-over-time (non-monotonic); final adopter share + equilibrium;
   abandonment + adopt↔abandon flip counts + flip-rate + final-window share-variance; committed core (S24b
   metrics) + adopter∩core; **group-advantage** trace (positive pre-copy best-vs-own group welfare gap) +
   copy-driver; per-seed matched `random_group_imitation` final share + whether it reached the core; the
   `individual_score_control` verdict (expect NormDiesBack); fluid buyer cohort + post-money bought;
   money/mortality/provenance/conservation.

## 4. The new suite `sim/tests/group_payoff_imitation.rs`

- **MANDATORY NON-VACUITY + GROUP-SIGNAL + ANTI-DRIFT TEST**: the seed is a real minority; the norm spreads
  beyond seed (non-seed adopters ≥ `MIN_NONSEED_ADOPTERS`, non-seed commitments ≥ `MIN_NONSEED_COMMITS`); ≥1
  copy has a **positive pre-copy group-welfare advantage** (the group signal binds — else `GroupSignalVacuous`);
  the copy-driver shows a GENERIC group aggregate; the score-purity guard asserts the group score never reads
  any adopter/committer/commitment field; and the matched `random_group_imitation` null does not reach the core
  by `CORE_MARGIN`.
- **The ordered classifier (§2)**, printed `--nocapture`; per-seed verdicts + tally; does NOT assert SUCCESS.
- **Scenario:** `frontier_group_payoff_imitation` (HEADLINE) = the S24b base + `group_payoff_imitation = true`.
  Matched references: `individual_score_control` (S24b individual score — expect `NormDiesBack`),
  `random_group_imitation` (clean null), `no_imitation` (seed only), `sticky_reference` (S24a contrast).
- **Controls (each a test; one variable each):**
  - **individual_score_control** (`group_payoff_imitation = false`, S24b) — must reproduce `NormDiesBack` (so
    the win, if any, is from group scoring).
  - **random_group_imitation** — same group observation/cadence, but the copied group is chosen UNIFORMLY at
    random (ignoring group score), rate endogenous/reported: must NOT reach the core by `CORE_MARGIN` ⇒ else
    `GroupDriftNotSelection`.
  - **no_imitation** — seed only: no spread, no core.
  - **no_seed** — share 0: the norm never appears.
  - **unprofitable_commitment** — commitment made non-advantageous (`commitment_term = 1`, S24b's shape): the
    group-welfare advantage and the spread must both vanish (proves the group advantage is real, not the
    norm's mere presence).
  - **salt_in_score** (SENSITIVITY) — SALT in the group score; success only here → `WealthProxySelection`.
  - **seed_cluster_only_check** — success REQUIRES non-seed adopters + non-seed committed-core participation
    (else `SeedClusterOnly`).
- **HARD GUARDS every run + cell:** conservation every tick; `bread_minted_max == 0`; provenance
  clean-or-disqualified; `!extinct`; SALT promotes; the adoption invariant (bit changes ONLY by seed + group
  imitation never fiat; non-adopters cannot commit; abandonment doesn't break a binding term); the **group
  score-purity guard** (the group score never reads adopter/committer/commitment/vocation/profit fields).
- **goldens_unchanged** test pinning the five tripwire digests (copy from a CURRENT suite, e.g.
  `abandonable_norm.rs`).
- **Robustness mini-sweep** over `commitment_seed_share` + `IMITATION_MARGIN_BPS` + `GROUP_RADIUS`, classified,
  no tuning; the margin + radius axes MUST be outcome-driving.

## 5. Verification (independent gate)

Redirect cargo to files; never pipe to head/grep (EPIPE → spurious exit 101).
- `cargo test -p sim --test group_payoff_imitation` passes (non-vacuity/group-signal/anti-drift + the controls).
- `cargo test --workspace` passes; **all existing goldens byte-identical** (`goldens_unchanged` +
  abandonable_norm / commitment_norm_spread / land_market / private_land_tenure / voluntary_cultivation_commitment /
  endowed_inherited_capital / durable_cultivation_capital / profit_driven_retention / occupational_stickiness /
  endogenous_cultivation_entry / robustness_appendix / household_barter / mortality / open_colony_mortality /
  demand_survival_bridge).
- `cargo fmt --check` clean; `cargo clippy --workspace --all-targets -- -D warnings` clean; conservation holds.

## 6. Honesty and scope (state these in the result; do not let a SUCCESS overclaim)

- **A SUCCESS = "institution selection needs GROUP-level payoff"** — group-welfare imitation preserves the
  division-of-labor institution where individual-welfare imitation (S24b) killed it: the arc's first clean
  positive, and a substantive result about *what selection sees*. If it instead `NormDiesBack`s (group signal
  still selects away), `GroupDriftNotSelection`s, `UniversalCommitmentRePin`s, or `SeedClusterOnly`s, that is
  the honest finding — and if it fails cleanly the S24 arc's terminal result is the knife-edge + the tragedy of
  imitation (S24a+S24b), with group-payoff shown insufficient.
- **The anti-smuggling guard is load-bearing** — the group score MUST be generic (score-purity at group level
  + the group-advantage diagnostic + `random_group_imitation` + `unprofitable_commitment` + the
  `individual_score_control` reproducing NormDiesBack); if copying is driven by "the group with the most
  committers" the result is void.
- **Bounded to this base + this imitation rule**; expect possible band-qualification (margin/radius window).
- Follow repo conventions; NEVER add Claude/AI/assistant references in code, comments, or committed text.
