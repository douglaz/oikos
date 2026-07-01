# impl-42 — S24b: Abandonable commitment-norm adoption (the clean institution-selection test)

Status (result): LANDED as a FINDING — `NormDiesBack` 5/5 on `SEEDS = {3,7,11,19,23}` (`CleanInstitutionSpread`
0/5, `DriftNotSelection` 0/5), so the strict clean-positive bar is NOT met — **and the way it fails is the
result.** With abandonable adoption the norm dies back **completely** (final adopters = 0 every seed; the only
flips are abandonment — `adopt=0, abandon=7–10` per run). **The mechanism (Codex-confirmed): on the GENERIC
welfare score the committed CULTIVATORS are not better off than the well-fed fluid BUYERS** (buyers ~37–48
alive, post_bought ~15k–32k — buyers eat plenty by buying; cultivators bear the production burden), so
bidirectional welfare-imitation flows *away* from the productive role — nobody ever imitates into being a
committed cultivator, and the seed adopters abandon at expiry. S24a's institution only spread because *sticky*
adoption **ratcheted** it; remove the ratchet and generic-welfare selection dissolves it. **The honest insight:
generic individual-welfare imitation does not preserve a division-of-labor institution when the producers who
sustain it are not individually better off than the buyers they feed — a "tragedy of imitation".** Together
S24a (sticky → over-spread/drift) + S24b (abandonable → die-back) show generic-imitation institution selection
is a **knife-edge**: neither extreme yields a clean bounded equilibrium. **Codex review-of-results:
PASS-WITH-CAVEATS** — `NormDiesBack` is the honest label, the interpretation is sound, and it is NOT an
artifact (the random null DOES adopt and `sticky_reference` forms cores, so the false→true path is live, not
dead-by-construction; the three candidate artifact-bugs — abandonment applied before a fair adoption
opportunity / warm-up before adopters show benefit / a score window that excludes the cultivator-benefit period
— were checked absent: adopters stay observable through their binding term, the warm-up gates on score-history,
the window captures recent welfare; the only timing nuance is a benign one-tick abandon *lag* that gives
adoption *more* chance). The clean positive is a genuinely NEW mechanism for **S24c** (role-crediting /
group-payoff imitation, OR explicit adopt/abandon hysteresis), NOT a re-run. Verified: workspace all goldens
byte-identical / fmt + clippy clean / suite passes. Base: master `4bf6277` (S24a landed). **Second slice of the
S24 INSTITUTION-SELECTION arc.** Composes directly on S24a (`commitment_norm_spread`), changing exactly one
thing: adoption is no longer **sticky** — the norm can be **abandoned**. The clean-positive test S24a named:
S24a found a working institution *can* spread by generic local imitation but, under *sticky* (never-abandoned)
adoption, usually over-spreads to a re-pin (3/5) or cannot be separated from the random-imitation null (1/5
drift); only 1/5 was a clean bounded selection-driven success. Codex's diagnosis + recommendation: sticky
adoption is the confound — make adoption abandonable so the drift null is clean and adoption settles below
saturation.

## 0. One-paragraph summary

S24a established that gating S22f's commitment behind an adopted-norm bit and spreading it by **generic local
imitation of observed success** does propagate the institution and recreate a committed core — but *sticky*
adoption (a bit, once set, never cleared) is a confound: outcome-blind **random** copying ratchets the bit
monotonically toward universal, so the random null *also* reaches the core (drift indistinguishable from
selection on several seeds), and the genuine signal over-spreads to a re-pin. S24b removes the confound by
making imitation **bidirectional / abandonable**: each imitation step copies the norm bit of whichever
observed neighbour is doing better on the *same generic survival score* — **adopter or non-adopter** — so an
agent that adopted but is *not* better off (or observes a better-off non-adopter) **drops** the norm.
Bidirectional copying removes S24a's monotonic ratchet, so the *hypothesis* (tested by the `random_imitation`
null, NOT assumed — finite stochastic copying can still fixate) is that an outcome-blind null behaves like a
random walk rather than a ratchet and fails to reach the core, while the genuine signal **settles at the
adoption level the real outcome-advantage supports** rather than over-spreading. The hypothesis (the arc's
potential first *clean* institution-selection positive): with abandonable adoption, **bounded
selection-driven spread that recreates the S22f core becomes the MAJORITY of seeds, the random null is clean
(no core), and adoption stays below the re-pin ceiling** — while money/mortality/provenance/conservation hold.
The anti-smuggling discipline is unchanged and *strengthened* by symmetry: the score is generic (no
institution/profit fields; score-purity invariant), the copy-driver is reported, and because imitation now
copies non-adopters too, a "prefer commitment" bias cannot hide in the rule. Classify-not-tune; goldens
byte-identical off.

## 1. Why this milestone, why this change — and the grounding

S24a's mixed result has a single named cause (Codex review-of-results): **sticky adoption**. Two symptoms
both trace to it: (1) the random-imitation null reaches the core on most seeds because outcome-blind copying
of a population that contains adopters can only ever *add* adopters (a ratchet), so adoption climbs to
saturation regardless of outcomes; (2) the genuine signal over-spreads past the bounded ceiling for the same
reason. Abandonable adoption removes the ratchet: if copying is bidirectional on a generic outcome, the
expected drift of an outcome-blind null is zero (a random walk), and a genuine outcome-advantage produces a
*bounded equilibrium* adoption level (adoption rises while adopters are better off and falls when they are
not). This is the cleanest test of institution selection: the institution persists at the level its real
performance advantage supports, and disappears under a blind null.

**Grounding (S24a reused unchanged except the adoption update rule):**
- S24a already gives: the `adopts_commitment_norm` gate on S22f commitment entry, the deterministic minority
  seed, the deterministic neighbourhood (`IMITATION_RADIUS` Manhattan + market co-location, dedup, cap
  `IMITATION_MAX_MODELS`, ties by id), the normalized generic score (`2·alive + hunger_relief + food_score`,
  SALT excluded, score-purity invariant), the copy-driver + adopter-advantage diagnostics, and the controls.
- S22f's commitment mechanism (entry signal / term / renewal / exit-override) is untouched.
- The only change is **how the norm bit updates** each imitation step (§3).

**Design decision (to be validated by Codex):** abandonment via **bidirectional generic imitation** — copy the
better-off observed neighbour's bit regardless of whether they adopt — rather than a self-referential
"abandon if my own commitment return is poor" rule (which would re-introduce an institution/profit signal into
the update and risk circularity). The symmetric generic rule keeps the anti-smuggling guarantee and directly
removes the ratchet. *(Open design question flagged for spec-review: is bidirectional generic imitation the
right abandonment mechanism, or is a generic self-outcome abandonment — e.g. drop if my own generic survival
score fell over the window — cleaner / less prone to a churn equilibrium?)*

## 2. The central question and pre-named outcomes

**Central question.** When S24a's commitment-norm spread is made **abandonable** — each step copies the norm
bit of the better-off observed neighbour on the *generic* survival score, adopter or non-adopter, so the norm
can be dropped — does institution selection become **clean**: a **bounded** adoption level that recreates the
S22f committed core in the **majority** of seeds, while the outcome-blind `random_imitation` null no longer
reaches the core (a true random walk) and money/mortality/provenance/conservation hold — without any
institution/profit term in the update rule?

**Primary success = `CleanInstitutionSpread`** (majority of `SEEDS = {3,7,11,19,23}`, vs the matched-seed
`no_imitation` and `random_imitation` nulls and the S24a-sticky reference):
1. **Bounded adoption equilibrium** — final adopter share ∈ `[ADOPTER_SHARE_MIN (0.15), ADOPTER_SHARE_MAX
   (0.6)]` (it settles between the seed and saturation; neither dies back to seed-only nor saturates).
2. **The S22f committed core forms** — persistent committed cultivator core by the S24a/S22f metrics
   (≥ `PERSIST_COHORT` ids cultivating ≥ `PERSIST_FRACTION` of the final window, all renewing).
3. **A surviving fluid non-adopter buyer tier materially buys** (post-money bought ≥ `MATERIAL_BOUGHT_FLOOR`,
   `final_buyer_cohort ≥ MIN_BUYER_COHORT`).
4. **PER-SEED clean selection with a MARGIN (Codex P1.2)** — the headline must **beat its matched
   `random_imitation` seed by a predeclared margin**, not merely "random forms no core" (finite blind
   bidirectional copying can transiently concentrate): require `headline.core ≥ random.core + CORE_MARGIN`
   (`CORE_MARGIN = PERSIST_COHORT`, i.e. the headline core at least a full cohort above the matched random) AND
   the matched random does **NOT** itself satisfy the full bounded two-tier success (clauses 1–3); AND
   `no_imitation` forms no core; AND spread is driven by ≥1 generic copy with positive pre-copy advantage;
   score-purity holds; SALT contributes zero copies.
5. **Abandonment actually happens** — ≥ `MIN_ABANDONMENTS` norm-drop events occur (the bit is genuinely
   non-sticky, not sticky-by-another-name), and the random null's adoption does **not** ratchet to saturation
   (its final share ≤ the headline's, ideally near the seed level).
6. **Money + mortality + provenance + conservation hold** (SALT promotes; `seeded_minted == 0`; bread
   `SelfProduced`; conservation every tick).
7. **Majority of seeds** classify `CleanInstitutionSpread` (the arc's first clean positive), and **none**
   classify `DriftNotSelection`.

**Finding modes (pre-named; first-class; verdict prints, does NOT assert SUCCESS):**
- `SeedDies` / `MoneyFailure` / `ConservationBroken` / `extinct` / `Cull` — precondition / integrity failures.
- `NormDiesBack` — abandonment overshoots: final adopter share < `ADOPTER_SHARE_MIN` (0.15) AND no core
  (over-correction; the opposite of S24a's over-spread).
- `ChurnEquilibrium` — the bit flips too readily: the per-agent adopt↔abandon **flip rate** over the final
  window ≥ `CHURN_FLIP_RATE` (flips per adopter-period ≥ 0.5) OR final-window adopter-share variance ≥
  `CHURN_SHARE_VAR` — AND no persistent core.
- `DriftNotSelection` — the headline does NOT beat its matched `random_imitation` seed by `CORE_MARGIN`
  (`= PERSIST_COHORT`), i.e. the abandonable blind null reaches a comparable core on this seed (drift not
  removed) — the S24b hypothesis FAILS for that seed.
- `UniversalCommitmentRePin` — adoption still saturates past `ADOPTER_SHARE_MAX` and the buyer tier collapses
  (abandonment didn't bound it).
- `SpreadWithoutOccupation` — bounded spread but no stable core.
- `CleanInstitutionSpread` — all seven success clauses (the clean positive).

**Pinned thresholds (predeclared consts; do NOT fit):** `ADOPTER_SHARE_MIN = 0.15`, `ADOPTER_SHARE_MAX = 0.6`,
`MIN_ABANDONMENTS = 8`, `CORE_MARGIN = PERSIST_COHORT (4)`, `CHURN_FLIP_RATE = 0.5` (flips per adopter per
final-window period), `CHURN_SHARE_VAR = 0.01` (final-window adopter-share variance bound — i.e. share
std-dev ≳ 0.1 signals an unsettled/oscillating adoption level), reusing the S24a
`PERSIST_COHORT`/`PERSIST_FRACTION`/`MIN_BUYER_COHORT`/`MATERIAL_BOUGHT_FLOOR`/`IMITATION_*` consts.

**Ordered classifier (top-down, first-match-wins):** `SeedDies` → `MoneyFailure` →
`ConservationBroken`/`extinct`/`Cull` → `NormDiesBack` → `UniversalCommitmentRePin` → `DriftNotSelection` →
`ChurnEquilibrium` → `SpreadWithoutOccupation` → **then the final gate:** `if all seven success clauses pass
{ CleanInstitutionSpread } else { SpreadWithoutOccupation }`. Headline-level result = the per-seed tally; the
milestone SUCCESS requires a MAJORITY `CleanInstitutionSpread` and ZERO `DriftNotSelection`. Predeclare every
threshold as a `const`; do NOT fit.

## 3. Engine design (additive, default-off, conservation-safe)

1. **NEW default-off flag** `ChainConfig::abandonable_norm: bool` + consts: `ADOPTER_SHARE_MIN` (0.15),
   `MIN_ABANDONMENTS` (e.g. 8), reuse of all S24a consts (`IMITATION_PERIOD/RADIUS/MAX_MODELS/WINDOW`,
   `IMITATION_MARGIN_BPS`, the score weights, `commitment_seed_share`) + the control toggles (§4). Helper
   `abandonable_norm_active(&self)` = flag on AND `commitment_norm_spread_active()` (S24b modifies S24a's
   update; if S24a is off the flag is inert). Canonicalize ON-only with the **next free flag-digest tag (16**
   unless master advanced) + the field + abandonment bookkeeping that steers behaviour. Off ⇒ byte-identical
   (S24a sticky behaviour preserved when `abandonable_norm` is off).

2. **The one change — bidirectional / abandonable update (§1 design).** When `abandonable_norm_active()`, the
   per-`IMITATION_PERIOD` update applies to **every** agent (not only non-adopters): an agent forms the same
   deterministic generic-scored observation set as S24a, finds the best-scoring observed neighbour, and if that
   neighbour beats it by ≥ `IMITATION_MARGIN_BPS`, it **copies that neighbour's `adopts_commitment_norm` bit**
   — which may set it true (adopt) **or false (abandon)**.
   **Pinned abandonment-timing state (Codex P1.1).** Add a digested ON-only per-agent `next_norm_bit:
   Option<bool>` (the pending copy result). Apply rule:
   - if the agent is **not** in a binding S22f term, the copied bit is applied **immediately** to
     `adopts_commitment_norm` (and `next_norm_bit` cleared);
   - if the agent **is** in a binding term, the copy is **staged** in `next_norm_bit` — the active
     `adopts_commitment_norm` stays **true** for the remainder of the term (no mid-term break; S22f
     exit-override semantics intact), and the staged bit is applied **at term expiry, BEFORE the renewal
     decision** (so an abandon means it does not renew and cannot re-commit until it re-adopts).
   - An **abandonment event is counted only when the applied bit actually changes** `true → false` (likewise
     an adoption only on `false → true`); re-copying the same value is not a flip.
   No institution/profit term enters the update (the score is the S24a generic score; score-purity invariant
   unchanged).

3. **Why this removes the ratchet (the testable mechanism).** Under an outcome-blind null (random model
   choice), bidirectional copying has zero expected drift (a random walk), so `random_imitation` should NOT
   climb to saturation or reach the core. Under the genuine generic score, adoption rises while adopters are
   really better off and falls otherwise → a **bounded equilibrium** at the level the real advantage supports.
   Both are *predictions the controls test*, not assumptions.

4. **Everything else is S24a/S22f unchanged** — the gate on commitment entry, the deterministic seed +
   neighbourhood, the generic score + score-purity, the copy-driver + adopter-advantage diagnostics, the
   commitment mechanism, money/mortality/conservation. NO fiat, NO "commitment is good" term, NO `Vocation`
   mutation. Field + abandonment bookkeeping serialized ON-only under tag 16.

5. **Diagnostics (runtime-only):** adoption-over-time curve (now non-monotonic); final adopter share +
   equilibrium check; abandonment count + adopt↔abandon flip counts; committed core (S24a metrics) +
   adopter∩core; copy-driver + adopter-advantage; the random-null final share + whether it reached the core
   (per-seed); fluid buyer cohort + post-money bought; money/mortality/provenance/conservation.

## 4. The new suite `sim/tests/abandonable_norm.rs`

- **MANDATORY NON-VACUITY + ABANDONMENT + ANTI-DRIFT TEST**: the seed is a real minority; the norm spreads
  beyond seed; **≥ `MIN_ABANDONMENTS` abandonment events occur** (the bit is genuinely non-sticky); the
  copy-driver shows a GENERIC observable; and **the matched `random_imitation` (abandonable) null does NOT
  ratchet to saturation** (final share materially below the headline / near seed) — the core mechanism claim.
- **The ordered classifier (§2)**, printed `--nocapture`; per-seed verdicts + the tally; does NOT assert SUCCESS.
- **Scenario:** `frontier_abandonable_norm` (HEADLINE) = the S24a base + `abandonable_norm = true`. Matched
  references: `no_imitation` (seed only), `random_imitation` (abandonable + outcome-blind — the clean null),
  and the **S24a-sticky** reference (`abandonable_norm = false`, the prior mixed result) for contrast.
- **Controls (each a test; one variable each):**
  - **sticky_reference** (`abandonable_norm = false`) = S24a (the mixed 3 RePin / 1 Drift / 1 Success), to
    show abandonment changes the outcome.
  - **no_imitation** — seed only: no spread, no core (imitation load-bearing).
  - **random_imitation** (PINNED mechanics, Codex P1.3) — same `IMITATION_PERIOD`, same observation set, same
    opportunity cadence, same abandonable bidirectional update, but the model is sampled **uniformly at random
    among the observed set ignoring score AND institution** (outcome-blind), then its bit is copied; the
    induced adoption rate is left ENDOGENOUS and **reported**. The clean-null claim (a *tested* hypothesis, not
    a guarantee): the headline must beat its **matched** random seed by `CORE_MARGIN` and random must not
    satisfy the bounded two-tier success; a seed where it does not is `DriftNotSelection`.
  - **no_seed** — share 0: norm never appears.
  - **tiny_margin** (`IMITATION_MARGIN_BPS` → small) — predict `ChurnEquilibrium` (the bit flips too readily,
    no persistent core): a sensitivity that the margin governs stability.
  - **salt_in_score** (SENSITIVITY) — SALT in the score; if success only here → `WealthProxySelection`.
- **HARD GUARDS every run + cell:** conservation every tick; `bread_minted_max == 0`; provenance
  clean-or-disqualified; `!extinct`; SALT promotes; adoption invariant (the bit changes ONLY by seed +
  generic imitation, never fiat; non-adopters cannot commit; abandonment doesn't break a binding term
  mid-commitment); the **score-purity guard** (the update score never reads adopter/committer/commitment
  fields).
- **goldens_unchanged** test pinning the five tripwire digests (copy from a CURRENT suite, e.g.
  `commitment_norm_spread.rs`).
- **Robustness mini-sweep** over `commitment_seed_share` + `IMITATION_MARGIN_BPS` + `IMITATION_PERIOD`,
  classified, no tuning; the margin axis MUST be outcome-driving (tiny → churn, huge → no spread).

## 5. Verification (independent gate)

Redirect cargo to files; never pipe to head/grep (EPIPE → spurious exit 101).
- `cargo test -p sim --test abandonable_norm` passes (non-vacuity/abandonment/anti-drift + the controls).
- `cargo test --workspace` passes; **all existing goldens byte-identical** (`goldens_unchanged` +
  commitment_norm_spread / land_market / private_land_tenure / voluntary_cultivation_commitment /
  endowed_inherited_capital / durable_cultivation_capital / profit_driven_retention / occupational_stickiness /
  endogenous_cultivation_entry / robustness_appendix / household_barter / mortality / open_colony_mortality /
  demand_survival_bridge).
- `cargo fmt --check` clean; `cargo clippy --workspace --all-targets -- -D warnings` clean; conservation holds.

## 6. Honesty and scope (state these in the result; do not let a SUCCESS overclaim)

- **A SUCCESS = "with abandonable adoption, institution selection is CLEAN"** — a working institution settles
  at a bounded adoption level that recreates the core in the majority of seeds, the blind null no longer
  reaches the core (drift removed), no institution/profit term in the update — the arc's first clean
  institution-selection positive. If instead it `NormDiesBack`s, `ChurnEquilibrium`s, or still
  `DriftNotSelection`s/`UniversalCommitmentRePin`s, that is the honest finding (and it tells us abandonment is
  not the cure, or the margin/window govern it).
- **The anti-drift claim is the load-bearing one** — the random-null-no-ratchet + per-seed no-core + the
  abandonment-count are what distinguish this from S24a; report them prominently.
- **Bounded to this base + this imitation rule**; expect possible band-qualification (the margin window).
- Follow repo conventions; NEVER add Claude/AI/assistant references in code, comments, or committed text.

## 7. Codex spec-review resolutions (round 1)

- **P1.1 abandonment timing pinned** — §3.2: digested ON-only `next_norm_bit: Option<bool>`; not-in-term →
  apply immediately; in a binding term → stage, keep norm true to expiry, apply BEFORE the renewal decision;
  count a flip only when the applied bit actually changes.
- **P1.2 per-seed random MARGIN** — §2.4/§2-modes: headline must beat its matched random seed by `CORE_MARGIN =
  PERSIST_COHORT` AND random must not satisfy the bounded two-tier success (finite blind copying can transiently
  concentrate; "random forms no core" alone is too weak) → else `DriftNotSelection`.
- **P1.3 random_imitation mechanics pinned** — §4: same period/observation/cadence/abandonable update, model
  sampled UNIFORMLY ignoring score AND institution, bit copied, induced rate reported.
- **P1.4 exact predicate constants** — §2: `NormDiesBack` (final share < `ADOPTER_SHARE_MIN`=0.15 + no core),
  `ChurnEquilibrium` (flip-rate ≥ `CHURN_FLIP_RATE`=0.5 or final-window share-variance ≥ `CHURN_SHARE_VAR`, +
  no core), `MIN_ABANDONMENTS`=8, `CORE_MARGIN`=PERSIST_COHORT.
- **P2.1 softened "random walk"** — §1/§3: stated as a *tested null/hypothesis* (finite stochastic copying can
  fixate), not a mechanical guarantee.
- **P2.2 bounded bar kept** — Codex: `[0.15,0.6]` + live buyer tier is sound; no explicit hysteresis added (if
  the bit flips too much that is the `ChurnEquilibrium` finding).
- **P2.3 process note removed** — §6: the rb-lite/reviewer-reliability note belongs in the run record, not the spec.

### Round 2 (1 pin → SPEC-READY)

- **CHURN_SHARE_VAR pinned = 0.01** (§2): final-window adopter-share variance bound for `ChurnEquilibrium`
  (share std-dev ≳ 0.1 → unsettled/oscillating).
- Codex confirmed: (reachability) a persistent core IS compatible with abandonable adoption — the S22f binding
  term is the hysteresis (mid-term abandonment staged, persistence from fresh renewals at expiry; die-back/churn
  if committers stop clearing the generic advantage is a real finding, not empty-by-construction); (success bar)
  KEEP strict — milestone success = **≥3/5 `CleanInstitutionSpread` AND zero `DriftNotSelection`** (2/5 would be
  a qualified improvement/finding, not the clean positive).
