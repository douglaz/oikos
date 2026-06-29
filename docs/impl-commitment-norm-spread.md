# impl-41 — S24a: Endogenous spread of the commitment institution by local imitation of observed success

Status (spec): SPEC-READY (Codex round 1: 4 P1 + 2 P2; round 2: 2 P1, all folded in §9; Codex pre-approved SPEC-READY after the round-2 fixes; core confirmed non-circular + success window reachable). Base: master `082da6f` (S23b landed). **First slice of the
S24 INSTITUTION-SELECTION arc** — the bridge from "institutions work when the experimenter supplies them" to
"a working institution can *propagate* under local social selection." Composes directly on S22f
(`voluntary_cultivation_commitment`, the one lever that stabilized a two-tier occupational core). **The
headline base is exactly S22f's:** `voluntary_cultivation_commitment = true` AND `profit_driven_retention =
true` (S22f's commitment entry signal is built from the S22c profit-retention data — `profit_driven_retention`
is part of the base, NOT off), on the `frontier_profit_retention_expanded()` roster; skill / capital /
endowment / land-tenure / land-market are **OFF**. Codex-scoped ("spec S24a — endogenous-commitment-norm-spread
via local imitation of observed success").

## 0. One-paragraph summary

Every institution in OIKOS so far that the experimenter *supplied* — the exchange rule, the SALT anchor, and
crucially S22f's exit-overriding commitment contract — was switched on globally by a flag; agents never *chose*
it. S24a asks the next question: **can the one known working occupation institution stop being
experimenter-universal and instead SPREAD through the population from a small seed, by generic local
imitation of observed success?** The institution is exactly S22f's voluntary fixed-term cultivation commitment,
unchanged in mechanism — but **availability is now gated** behind a per-agent `adopts_commitment_norm` bit:
only adopters may enter a commitment. A small **deterministic minority seed** starts with the norm; everyone
else starts without it. The norm spreads by **local imitation of observed success** — periodically a
non-adopter compares itself to nearby/observable agents on **generic observable outcomes** (alive / low hunger
/ recently consumed food / SALT-or-stock position — NOT "is a committer", NOT "commitment is profitable", NOT
"would this institution stabilize occupation"); if an observed neighbour outperforms it by a pinned margin over
a rolling window, it **copies that neighbour's norm bit**. The copied object is a *practice bit*, and the
selection pressure is *generic cultural imitation of success* — the rule never encodes that commitment is good.
**The central anti-smuggling trap (Codex):** outcome imitation must not become a disguised fitness oracle — the
score stays generic, and the test *reports which observable drove copying*; a `random_imitation` control (copy
a random model, same cadence) must NOT reproduce the result (else it is drift, not selection). SUCCESS =
adoption grows materially beyond the seed AND non-seed adopters enter real commitments AND the S22f stable
committed core + two-tier market re-forms — **without the experimenter granting commitment to everyone.**
Classify-not-tune; money + mortality + provenance + conservation hold; goldens byte-identical off.

## 1. Why this milestone, why this lever — and the grounding

S5–S23b established a graded ledger of *which conditions* money emergence needs and *which* a stable occupation
does not get for free — culminating in S22f, where a **supplied** exit-overriding contract finally stabilized a
core. But S22f (like every institution here) was experimenter-imposed: the flag turned commitment on for all
eligible agents. The honest next frontier (Codex) is **endogenous institution selection** — whether the
institutional layer the experimenter has supplied can *itself* emerge. S24a is the cleanest, most decisive
first slice: not agent *choice among* institutions (commons vs private vs contract — that mixes the
collapse-prone property regimes back in and blurs the causal read), but the **endogenous spread of the single
known-working institution** from a seed. If a working institution can propagate under generic local imitation,
the institutional layer is no longer purely exogenous.

**Grounding (S22f reused unchanged; only the availability gate + spread are new):**
- S22f already gives the full commitment mechanism: post-money an eligible agent whose own realized
  cultivation-return signal clears its outside option may commit for `commitment_term`; while committed the
  hunger/profit exit can't turn cultivation off; re-decide at expiry. S24a does **not** touch that mechanism —
  it only gates *who is allowed to enter it* (`adopts_commitment_norm`) and adds the norm's seed + spread.
- Spatial neighbourhoods, Manhattan distance, and per-agent hunger/consumption/SALT state already exist
  (the generic observables read existing fields; no new physics).
- Deterministic per-agent heterogeneity from a hashed seed already exists (used to pick the seed minority).

**Design decisions (Codex):** the copied object is a generic *practice bit*, spread by *imitation of observed
success on generic outcomes*, NOT a "prefer the better institution" rule (which would smuggle the conclusion);
start with *spread of the one working institution*, not *choice among* institutions (a later slice).

## 2. The central question and pre-named outcomes

**Central question.** When the S22f commitment institution is available only to agents carrying an
`adopts_commitment_norm` bit, a small deterministic minority seeds the norm, and the norm spreads by **generic
local imitation of observed success** (copy a better-off observed neighbour's norm bit — on alive/hunger/
food/SALT outcomes, never on institution identity or its profitability) — does the institution **spread
materially beyond the seed** and thereby **recreate the S22f stable committed cultivator core + a surviving
two-tier market**, while money/mortality/provenance/conservation hold, *without* the experimenter granting
commitment to everyone — AND is the spread genuinely *selection* (it dies under `no_imitation`, and
`random_imitation` does not reproduce it), not drift or a disguised fitness oracle?

**Primary success = `InstitutionSpreadSuccess`** (all, across `SEEDS = {3,7,11,19,23}`, vs the matched-seed
`global_commitment_on` (S22f) and `no_imitation` baselines):
1. **Adoption grows materially beyond the seed** — final adopters ≥ `SPREAD_FACTOR` (3) × the seed count AND
   ≥ `MIN_NONSEED_ADOPTERS` (4) distinct non-seed agents adopt (the norm propagates, not just persists).
2. **Non-seed adopters enter REAL commitments** — ≥ `MIN_NONSEED_COMMITS` (4) commitments are entered by
   non-seed adopters, and ≥1 renews from a fresh post-expiry signal (the spread reaches the institution, not
   just the bit).
3. **The S22f stable committed core re-forms** — a persistent committed cultivator core by the same S22f
   stickiness metrics (≥ `PERSIST_COHORT` distinct committed ids cultivating ≥ `PERSIST_FRACTION` of the final
   window, all renewing).
4. **A surviving fluid non-committed buyer tier remains** — non-adopters/non-committed survive and materially
   buy food (post-money bought ≥ `MATERIAL_BOUGHT_FLOOR`).
5. **Bounded adoption, NOT a universal re-pin (Codex P1.3 — require BOTH).** adopter share ≤
   `ADOPTER_SHARE_MAX` (0.6) **AND** a live non-adopter/non-committed buyer tier **materially buys** (post-money
   bought ≥ `MATERIAL_BOUGHT_FLOOR`). If adopter share **exceeds** the cap → `UniversalCommitmentRePin`
   **even if a few non-adopters remain** (a tiny residual tier does not rescue it).
6. **Money + mortality + provenance survive** — SALT promotes; `seeded_minted == 0`; bread `SelfProduced`;
   conservation holds every tick.
7. **The spread is SELECTION, not drift or oracle** — `no_imitation` (seed only) does NOT reproduce the core;
   `random_imitation` (same adoption cadence, model chosen at random not by outcome) does NOT reliably
   reproduce it; the milestone reports **which observable drove copying** (a generic outcome, not a
   commitment-specific score).

**Finding modes (pre-named; first-class; verdict prints, does NOT assert SUCCESS):**
- `SeedDies` (precondition fail) — the seeded adopters fail/die before the norm can propagate.
- `MoneyFailure` — the adoption machinery disrupts the SALT bootstrap.
- `ConservationBroken` / `extinct` / `Cull` — any conservation break, colony death, or a monopoly/extinction cull.
- `SeedOnlyNoSpread` — seeded adopters may commit, but the norm does not propagate (final adopters ≈ seed).
- `DriftNotSelection` — the norm spreads, but `random_imitation` reproduces it just as well (spread is not
  outcome-driven; it is drift), OR the adoption is outcome-blind.
- `SpreadWithoutOccupation` — the norm spreads materially but no stable committed core forms.
- `UniversalCommitmentRePin` — adoption goes (near-)universal and collapses the fluid buyer tier (a re-pin of
  S22f-for-all, i.e. effectively the supplied institution, not bounded emergence).
- `WealthProxySelection` (anti-oracle, Codex P1.2) — the headline (generic, no-SALT) score does NOT produce
  the spread, but adding SALT/stock to the score DOES — i.e. the result rides a commitment-profit proxy, not a
  generic survival observable. Not a clean `InstitutionSpreadSuccess`.
- `InstitutionSpreadSuccess` — all seven success clauses, not downgraded, on the generic no-SALT score.

**Ordered classifier (top-down, first-match-wins):** `SeedDies` → `MoneyFailure` → `ConservationBroken`/
`extinct`/`Cull` → `SeedOnlyNoSpread` → `UniversalCommitmentRePin` → `DriftNotSelection` →
`SpreadWithoutOccupation` → **then the explicit final gate:** `if ALL SEVEN success clauses pass on the generic
no-SALT headline score { InstitutionSpreadSuccess } else { SpreadWithoutOccupation }`. (`WealthProxySelection`
is decided by the `salt_in_score` sensitivity: it labels a headline that FAILS but whose SALT-variant SUCCEEDS
— reported alongside, never upgraded to success.) Predeclare every threshold as a `const`; do NOT fit.

## 3. Engine design (additive, default-off, conservation-safe)

1. **NEW default-off flag** `ChainConfig::commitment_norm_spread: bool` + pinned fields/consts:
   `commitment_seed_share` (deterministic minority seed fraction, default small e.g. 0.15),
   `IMITATION_PERIOD` (econ ticks between imitation checks, default 24), `IMITATION_WINDOW` (rolling window for
   the observed-outcome comparison, default 48), `IMITATION_MARGIN_BPS` (how much better a model must be to be
   copied, default e.g. 1500), `ADOPTER_SHARE_MAX` (0.6), plus the control toggles (§4). Helper
   `commitment_norm_spread_active(&self)` = flag on AND `voluntary_cultivation_commitment` on (S24a spreads the
   S22f institution; if S22f is off the flag is inert). Canonicalize ON-only with the **next free flag-digest
   tag (15** unless master advanced) + these fields + the per-agent `adopts_commitment_norm` bit + imitation
   bookkeeping that steers behaviour. Off ⇒ byte-identical.

2. **Availability gate (the only change to S22f's mechanism).** When `commitment_norm_spread_active()`, an
   agent may enter an S22f commitment **only if** `adopts_commitment_norm == true`. The S22f entry-signal /
   term / renewal / exit-override logic is otherwise **unchanged**. (When the flag is off, S22f behaves exactly
   as before — every eligible agent may commit — so goldens are byte-identical.)

3. **Deterministic minority seed.** At generation, `adopts_commitment_norm` is set true for a deterministic
   minority chosen by `hash(seed, agent_or_household_id) < commitment_seed_share`; everyone else starts false.
   Seed adopters are tracked (a `seed_adopter` set) so the test can separate seed from non-seed adoption. The
   `no_seed` control sets the seed share to 0.

4. **Spread = local imitation of observed success (THE anti-smuggling crux, Codex; score + neighbourhood now
   PINNED).** Every `IMITATION_PERIOD`, each **non-adopter** agent forms a **deterministic observation set**:
   all agents within `IMITATION_RADIUS` (default = the existing spatial observation radius; pin the constant)
   by Manhattan distance, PLUS agents co-located at the market this window; deduplicated; capped at
   `IMITATION_MAX_MODELS` (default 8) nearest by `(manhattan, agent_id)`; ties broken by `agent_id`. It
   computes a **generic observed-success score** for itself and each observed agent over `IMITATION_WINDOW`.
   **PINNED HEADLINE SCORE (conservative + NORMALIZED, Codex round-2 P1):** each component is normalized to
   `[0,1]` (expressed in integer bps `[0,10000]` for determinism), so the fixed weights are meaningful:
   - `alive_score = 1` if the agent is alive at the end of the window (else `0`);
   - `hunger_relief = (need_max − mean_hunger_over_window) / need_max` (clamped to `[0,1]`);
   - `food_score = min(1, food_consumed_over_window / FOOD_WINDOW_TARGET)` (a declared const cap, NOT raw
     units — raw units over a 48-tick window would dominate and void the weights);
   - `score_bps = 2·alive_score + hunger_relief + food_score` on the fixed integer bps scale (max 4·10000),
     weights `w_alive=2, w_hunger=1, w_food=1` (declared consts). **SALT/stock is NOT in the headline score** (it is a commitment-profit proxy —
   a leak risk); it is reported only as a **sensitivity** (a `salt_in_score` variant): if SUCCESS appears ONLY
   with SALT in the score, classify `WealthProxySelection`, not `InstitutionSpreadSuccess`. **The score MUST
   NOT reference** institution identity (committer/adopter), commitment state/profitability, or any "would this
   institution help" term — enforced by the score-purity guard (§4). If the best observed neighbour's score
   exceeds the agent's own by ≥ `IMITATION_MARGIN_BPS` over the window, the agent **copies that neighbour's
   `adopts_commitment_norm` bit** (adopts iff the model is an adopter). The milestone records the **copy-driver**
   (which generic component was largest in the copied model's advantage) to prove the driver is generic.
   Adoption is **sticky** (kept once adopted) for this slice; abandonment is a later slice.

5. **Anti-drift + anti-oracle structure (null fixed, Codex P1.4).** `random_imitation` (control): **same
   `IMITATION_PERIOD`, same observation-set rule, same `commitment_seed_share`, same opportunity checks** —
   but the copied model is chosen **uniformly at random** among the observed set (outcome-blind), and **the
   adoption RATE is left endogenous** (do NOT force it to match the headline rate — matching would leak
   headline-outcome information into the null). It must NOT reproduce the committed **core** (the classifier
   checks the CORE + two-tier outcome under random, not merely whether the *bit* spreads — sticky random
   copying can ratchet the bit toward universal by accumulation, so bit-spread alone is not the test).
   `DriftNotSelection` fires if random imitation produces spread **and** a core comparable to the headline.
   The generic-observable score + the copy-driver diagnostic + the adopter-advantage diagnostic (§7) +
   `random_imitation` + `unprofitable_seed` together establish the spread is *outcome-driven selection*, not a
   hidden "prefer commitment" rule.

6. **Adopter-advantage diagnostic (anti-oracle, Codex P2.5).** Record, per copy event, whether the copied
   model had a **positive generic-score advantage** over the copier in the **pre-copy window** — i.e. spread
   tracks an *actual realized advantage*, and that advantage is generic (not institution-labelled). The
   headline must show copied adopters held a positive pre-copy advantage; `unprofitable_seed` must **remove
   both the advantage AND the spread** (the cleanest selection-not-oracle evidence: no advantage ⇒ no spread).

7. **Everything else is S22f/base unchanged** — the commitment mechanism, the hunger-gated cultivate
   entry/exit for non-committed agents, money promotion, mortality, conservation. NO fiat "adopt commitment",
   NO "commitment is good" term anywhere, NO `Vocation` mutation. Per-agent `adopts_commitment_norm` +
   imitation bookkeeping serialized ON-only under tag 15.

8. **Diagnostics (runtime-only):** seed count + final adopter count (+ non-seed adopters); adoption-over-time
   curve; non-seed commitments entered + renewed; the committed core (S22f metrics) + adopter∩core; the
   **copy-driver** breakdown (which generic observable drove each copy) + the **adopter-advantage** trace (§7);
   fluid non-adopter buyer cohort + post-money bought; adopter share (bounded vs universal); spread vs the
   `no_imitation`/`random_imitation`/`unprofitable_seed` controls; money promotion + mortality + provenance +
   conservation.

## 4. The new suite `sim/tests/commitment_norm_spread.rs`

- **MANDATORY NON-VACUITY + ANTI-ORACLE TEST** (else `SeedDies` / `SeedOnlyNoSpread` / `DriftNotSelection`):
  the seed is a real minority (0 < seed < all); the norm **spreads** (final adopters > seed, ≥
  `MIN_NONSEED_ADOPTERS`); non-seed agents enter **real commitments**; and the **copy-driver diagnostic shows a
  GENERIC observable** drove copying (not an institution-specific score) — assert the score function never
  reads adopter/committer identity.
- **The ordered classifier (§2)**, printed `--nocapture`; verdict prints + deciding metrics; does NOT assert SUCCESS.
- **Scenario:** `frontier_commitment_norm_spread` (HEADLINE) = the S22f base (`voluntary_cultivation_commitment
  = true`, `profit_driven_retention = true`, `frontier_profit_retention_expanded()` roster; skill/capital/
  endowment/land OFF) + `commitment_norm_spread = true` + a minority seed + the imitation spread on the
  **generic no-SALT score**. Matched references: `global_commitment_on` (= S22f, commitment for all, the
  SUPPLIED positive control) and `no_imitation` (seed only, no spread = the floor baseline).
- **Controls (each a test; one variable each):**
  - **global_commitment_on** — S22f with commitment universal; the supplied-institution positive control
    (classified as supplied, NOT emergence — sanity that the core CAN form).
  - **no_imitation** — seeded adopters only, spread disabled: tests that imitation is load-bearing (no core
    from the seed alone).
  - **random_imitation** — same `IMITATION_PERIOD`, same observation-set rule, same `commitment_seed_share`,
    same opportunity checks, but the model is chosen UNIFORMLY AT RANDOM (outcome-blind) and the **adoption
    rate is left ENDOGENOUS** (NOT forced to match the headline — matching would leak headline outcome into the
    null, Codex P1.4): must NOT reproduce the committed CORE (the bit may still ratchet under sticky random
    copying; the test checks the CORE, not bit-spread) ⇒ else `DriftNotSelection`.
  - **no_seed** — seed share 0: the norm must NOT appear (no spontaneous innovation in this slice).
  - **unprofitable_seed** (PINNED, Codex round-2 P1) — the institution is granted to the seed but made
    **non-advantageous** by setting `commitment_term = 1` (a non-binding term: an adopter can commit but the
    commitment confers no stickiness benefit — exactly S22f's `nonbinding_term` / `TermTooShortFinding` shape).
    Money still promotes; the norm bit still exists and could spread. What it proves: with **no realized
    commitment advantage**, the adopter-advantage diagnostic (§3.6) shows **no pre-copy generic advantage AND
    no spread** → the spread really tracks an *advantage*, not the mere presence of the norm (selection, not
    oracle).
  - **salt_in_score** (SENSITIVITY, Codex P1.2) — adds SALT/stock to the score; reported, not the headline. If
    SUCCESS appears here but NOT on the generic no-SALT headline → `WealthProxySelection` (the result rode a
    commitment-profit proxy), never upgraded to `InstitutionSpreadSuccess`.
- **HARD GUARDS every run + cell:** conservation every tick; `bread_minted_max == 0`; provenance
  clean-or-disqualified; `!extinct`; money promotes (SALT); the adoption invariant (adopter set only grows by
  imitation/seed, never by fiat; seed deterministic; non-adopters cannot commit); **the score-function purity
  guard** (the imitation score never reads adopter/committer/commitment fields — the anti-oracle invariant).
- **goldens_unchanged** test pinning the five tripwire digests (copy from a CURRENT suite, e.g.
  `voluntary_cultivation_commitment.rs` — NOT a stale one).
- **Robustness mini-sweep** over `commitment_seed_share` + `IMITATION_MARGIN_BPS` + `IMITATION_PERIOD`,
  classified, no tuning; the seed-share + margin axes MUST be outcome-driving (too-large seed → trivially
  universal; too-large margin → no spread).

## 5. Verification (independent gate)

Redirect cargo to files; never pipe to head/grep (EPIPE → spurious exit 101).
- `cargo test -p sim --test commitment_norm_spread` passes (non-vacuity/anti-oracle + the controls).
- `cargo test --workspace` passes; **all existing goldens byte-identical** (`goldens_unchanged` +
  land_market / private_land_tenure / voluntary_cultivation_commitment / endowed_inherited_capital /
  durable_cultivation_capital / profit_driven_retention / occupational_stickiness /
  endogenous_cultivation_entry / robustness_appendix / household_barter / mortality / open_colony_mortality /
  demand_survival_bridge).
- `cargo fmt --check` clean; `cargo clippy --workspace --all-targets -- -D warnings` clean; conservation holds.

## 6. Honesty and scope (state these in the result; do not let a SUCCESS overclaim)

- **A SUCCESS = "a working occupation institution can PROPAGATE from a seed under generic local imitation of
  observed success"** — the institutional layer is no longer purely exogenous; the bridge from supplied to
  selected. If it instead `SeedOnlyNoSpread`s, `DriftNotSelection`s, `SpreadWithoutOccupation`s, or
  `UniversalCommitmentRePin`s, that is the honest finding.
- **The anti-smuggling guard is load-bearing** — the imitation score MUST be generic (the score-purity guard +
  the copy-driver diagnostic + the `random_imitation` control); if copying is driven by an
  institution-specific/fitness score, the result is void.
- **This spreads the ONE known working institution (S22f), not arbitrary institutions** — it does not yet test
  *choice among* institutions (a later S24 slice), nor innovation from zero (no_seed must stay inert here).
- **Bounded to this WOOD-poor, mortality-on base** + this imitation rule; like prior milestones expect possible
  band-qualification — report the seed-share / margin windows where it holds.
- Follow repo conventions; NEVER add Claude/AI/assistant references in code, comments, or committed text.

## 9. Codex spec-review resolutions (round 1)

- **P1.1 base composition** — §1/§4: the headline base is S22f's actual base — `voluntary_cultivation_commitment
  = true` AND `profit_driven_retention = true` (the commitment entry signal is built from S22c profit-retention
  data), `frontier_profit_retention_expanded()` roster; profit-stay is NOT off.
- **P1.2 score formula pinned + WealthProxySelection** — §3.4: `score = 2·alive + 1·hunger_relief +
  1·recent_food_consumed` (fixed consts); **SALT/stock NOT in the headline score** (commitment-profit proxy),
  only a `salt_in_score` sensitivity; if success appears only with SALT → `WealthProxySelection` (new finding
  mode), never upgraded.
- **P1.3 boundedness requires BOTH** — §2.5: adopter share ≤ `ADOPTER_SHARE_MAX` AND a live non-adopter buyer
  tier materially buys; share over cap → `UniversalCommitmentRePin` even with a residual tier.
- **P1.4 random_imitation null fixed** — §3.5/§4: same period/observation-set/seed-share/opportunity-checks,
  model chosen uniformly at random, **adoption rate ENDOGENOUS** (not forced to match headline); classify
  `DriftNotSelection` only if random reproduces the CORE (not merely bit-spread).
- **P2.5 adopter-advantage diagnostic** — §3.6: per-copy pre-window generic-score advantage recorded; headline
  must show copied adopters held a positive pre-copy advantage; `unprofitable_seed` removes advantage AND
  spread.
- **P2.6 neighbourhood rule pinned** — §3.4: observation set = within `IMITATION_RADIUS` (Manhattan) + market
  co-located, deduped, capped at `IMITATION_MAX_MODELS`=8 nearest by `(manhattan, agent_id)`, ties by `agent_id`.

### Round 2 (2 P1 → SPEC-READY)

- **P1 score units normalized** — §3.4: each component normalized to [0,1] (integer bps) so weights are
  meaningful — `alive_score` 0/1, `hunger_relief = (need_max − mean_hunger)/need_max`, `food_score =
  min(1, food_consumed / FOOD_WINDOW_TARGET)`; `score_bps = 2·alive + hunger_relief + food_score` (raw food
  units would dominate the 48-tick window and void the weights).
- **P1 unprofitable_seed pinned** — §4: the institution is granted but non-advantageous via `commitment_term =
  1` (non-binding, S22f's TermTooShortFinding shape); money still promotes; proves spread tracks a real
  advantage (no advantage ⇒ no spread), not the mere presence of the norm.
- Codex confirmed: the no-SALT score is non-circular (may yield `SeedOnlyNoSpread` if committers/buyers look
  alike — a real finding, not an empty spec); bounded spread is reachable (universal ⇒ `UniversalCommitmentRePin`,
  correct); copying better-off committers is not circular as long as the score reads no institution/profit
  fields and the controls are strong (they are).
