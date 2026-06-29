# impl-41 — S24a: Endogenous spread of the commitment institution by local imitation of observed success

Status (spec): DRAFT — pending Codex spec-review. Base: master `082da6f` (S23b landed). **First slice of the
S24 INSTITUTION-SELECTION arc** — the bridge from "institutions work when the experimenter supplies them" to
"a working institution can *propagate* under local social selection." Composes directly on S22f
(`voluntary_cultivation_commitment`, the one lever that stabilized a two-tier occupational core); all other
S22/S23 levers (skill, profit-stay, capital, endowment, land-tenure, land-market) are **OFF** in the headline.
Codex-scoped ("spec S24a — endogenous-commitment-norm-spread via local imitation of observed success").

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
5. **Bounded adoption, NOT a universal re-pin** — adopter share ≤ `ADOPTER_SHARE_MAX` (0.6) OR a live
   non-adopter buyer tier persists (the institution spreads but does not pin the whole colony =
   `UniversalCommitmentRePin`).
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
- `InstitutionSpreadSuccess` — all seven success clauses, not downgraded.

**Ordered classifier (top-down, first-match-wins):** `SeedDies` → `MoneyFailure` → `ConservationBroken`/
`extinct`/`Cull` → `SeedOnlyNoSpread` → `UniversalCommitmentRePin` → `DriftNotSelection` →
`SpreadWithoutOccupation` → **then the explicit final gate:** `if ALL SEVEN success clauses pass
{ InstitutionSpreadSuccess } else { SpreadWithoutOccupation }`. Predeclare every threshold as a `const`; do
NOT fit.

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

4. **Spread = local imitation of observed success (THE anti-smuggling crux, Codex).** Every `IMITATION_PERIOD`,
   each **non-adopter** agent forms an **observation set** of nearby/observable agents (spatial neighbours
   within an existing observation radius / co-located at market — reuse existing spatial adjacency; pin the
   neighbourhood rule). It computes a **generic observed-success score** for itself and each observed agent
   over `IMITATION_WINDOW` — a composite of **generic survival/welfare observables ONLY**:
   `alive` + low recent hunger + recently consumed food + SALT-or-stock position. **The score MUST NOT
   reference** institution identity (whether the model is a committer/adopter), commitment profitability, or
   any "would this institution help" term — it is the same score every agent could compute about any neighbour
   regardless of institutions. If the best observed neighbour's score exceeds the agent's own by ≥
   `IMITATION_MARGIN_BPS` over the window, the agent **copies that neighbour's `adopts_commitment_norm` bit**
   (adopts iff the model is an adopter). The milestone **records which observable component was largest in the
   copied model's advantage** (the "copy driver" diagnostic) to prove the driver is generic. Adoption is
   sticky (once adopted, kept) for this slice; a later slice may add abandonment.

5. **Anti-drift + anti-oracle structure.** `random_imitation` (control): same period/cadence/rate of adoption
   events, but the copied model is chosen **at random** among observed neighbours (outcome-blind) — this must
   NOT reliably reproduce the core (else the spread is drift). The generic-observable score + the copy-driver
   diagnostic + `random_imitation` together establish the spread is *outcome-driven selection*, not a hidden
   "prefer commitment" rule.

6. **Everything else is S22f/base unchanged** — the commitment mechanism, the hunger-gated cultivate
   entry/exit for non-committed agents, money promotion, mortality, conservation. NO fiat "adopt commitment",
   NO "commitment is good" term anywhere, NO `Vocation` mutation. Per-agent `adopts_commitment_norm` +
   imitation bookkeeping serialized ON-only under tag 15.

7. **Diagnostics (runtime-only):** seed count + final adopter count (+ non-seed adopters); adoption-over-time
   curve; non-seed commitments entered + renewed; the committed core (S22f metrics) + adopter∩core; the
   **copy-driver** breakdown (which generic observable drove each copy); fluid non-adopter buyer cohort +
   post-money bought; adopter share (bounded vs universal); spread vs the `no_imitation`/`random_imitation`
   controls; money promotion + mortality + provenance + conservation.

## 4. The new suite `sim/tests/commitment_norm_spread.rs`

- **MANDATORY NON-VACUITY + ANTI-ORACLE TEST** (else `SeedDies` / `SeedOnlyNoSpread` / `DriftNotSelection`):
  the seed is a real minority (0 < seed < all); the norm **spreads** (final adopters > seed, ≥
  `MIN_NONSEED_ADOPTERS`); non-seed agents enter **real commitments**; and the **copy-driver diagnostic shows a
  GENERIC observable** drove copying (not an institution-specific score) — assert the score function never
  reads adopter/committer identity.
- **The ordered classifier (§2)**, printed `--nocapture`; verdict prints + deciding metrics; does NOT assert SUCCESS.
- **Scenario:** `frontier_commitment_norm_spread` (HEADLINE) = the S22f base (voluntary commitment on; skill/
  profit/capital/endowment/land OFF) + `commitment_norm_spread = true` + a minority seed + the imitation
  spread. Matched references: `global_commitment_on` (= S22f, commitment for all, the SUPPLIED positive
  control) and `no_imitation` (seed only, no spread = the floor baseline).
- **Controls (each a test; one variable each):**
  - **global_commitment_on** — S22f with commitment universal; the supplied-institution positive control
    (classified as supplied, NOT emergence — sanity that the core CAN form).
  - **no_imitation** — seeded adopters only, spread disabled: tests that imitation is load-bearing (no core
    from the seed alone).
  - **random_imitation** — same adoption cadence/rate, model chosen at RANDOM (outcome-blind): must NOT
    reliably reproduce the core (separates selection from drift) ⇒ else `DriftNotSelection`.
  - **no_seed** — seed share 0: the norm must NOT appear (no spontaneous innovation in this slice).
  - **unprofitable_seed** — seed placed where commitment does NOT visibly outperform (e.g. a regime/topology
    with no realized-return advantage to committing): the norm must NOT spread (the driver really is observed
    success).
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
