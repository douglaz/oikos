# impl-38 — S22f: Voluntary Fixed-Term Cultivation Commitment (does an institution that changes the EXIT finally stabilize an occupation?)

Status (spec): DRAFT — pending Codex spec-review. Base: master `4969748` (S22e landed + the article at
five-step). Composes on S22c (`profit_driven_retention`, the realized-return signal) → S22a
(`endogenous_cultivation_entry`); runs on the **expanded S22e roster** but **without** endowed capital in the
headline. Scoped by Codex ("Build S22f … voluntary fixed-term cultivation commitment").

## 0. One-paragraph summary

The role-topology arc is a clean **five-step negative**: hunger (S22a), accumulated skill (S22b), a realized
profit stay-incentive (S22c), sunk **earned** capital (S22d), and even **endowed + inherited** capital
(S22e) each *bite* but none yields a division of labor. S22e was decisive: it removed the chicken-and-egg of
*acquiring* capital (gave it up front + inherited it) and *still* got a flat 0/8 cohort, because the binding
constraint is the hunger/profit **exit**, which rotates owners out **regardless of who owns the means**. So
the arc names its next condition precisely: an institution that changes the **exit** itself, not another
capital or incentive lever. S22f tests exactly that with the most authentic, least-fiat version — a
**voluntary fixed-term cultivation commitment**: post-money, an agent whose *own* realized cultivation-return
signal (the unchanged S22c signal) clears its outside option may **choose** to enter a cultivator commitment;
once in, it is **bound for a fixed term** during which the normal hunger/profit exit cannot turn cultivation
off; at term expiry it **re-decides from fresh realized returns** (a renewal only if the signal still
clears). Uptake is endogenous; the institution is configured. **The central trap (foregrounded):** an
exit-overriding institution is one step from merely **re-pinning** the S21 producer class that S22a
deliberately relaxed. The honest design makes commitment **voluntary** (entry gated by the agent's own
post-money realized return, inert pre-money, no quota/assignment), keeps non-committed agents **fully fluid**,
forces **term expiry to re-open choice** (no "once a cultivator, always a cultivator"), and proves it is not
a pin via a `fiat_pin` control (forced commitment ⇒ classified `RePinScaffold`, never headline success) plus
a **renewals-from-fresh-signals** requirement so persistence across multiple terms must come from
*re-choosing*, not one long binding. If it succeeds it is the arc's **first positive** — honestly framed as
*institutional sufficiency with endogenous uptake* ("a voluntary binding vocation institution stabilizes the
occupation that hunger, skill, profit incentives, earned capital, and inherited capital could not"), NOT
"occupation emerges without institutions." Classify-not-tune, same stickiness spine, hard anti-fiat /
anti-circularity / conservation guards.

## 1. Why this milestone, why this lever

S22a–e isolated the boundary to the **exit**: every lever that left the hunger/profit exit intact failed,
including capital given up front. The remaining hypothesis is institutional — a commitment device that makes
*leaving costly for a chosen term*, the canonical economic source of occupational persistence that is **not**
a productivity or capital advantage (an apprenticeship indenture, a guild oath, a fixed-term contract). The
authentic, non-circular version is **voluntary**: the agent opts in under its own realized-return signal
(reusing S22c, so entry is praxeologically grounded and inert before money), and the institution binds the
exit for a fixed term. This is the first lever in the arc that touches the **exit rule** rather than the
agent's productivity, incentive, or assets — exactly the condition the five-step negative named.

Design constraints (Codex): do **not** mutate `Vocation`; add a per-agent commitment state that steers the
`cultivating` decision and keep the existing role-choice machinery isolated. The entry signal is the
unchanged S22c realized cultivation-return vs outside option; the only new behavior is (a) a voluntary opt-in
that sets a term, (b) the exit being blocked while the term runs, and (c) re-decision at expiry.

## 2. The central question and pre-named outcomes

**Central question.** Post-money, when an agent may **voluntarily** commit to cultivation for a fixed term
(entry gated by its own realized cultivation-return clearing its outside option) and the commitment **binds
the exit** for that term (re-deciding at expiry), does a **persistent committed cultivator cohort** finally
form (churn ≤ 0.5× the matched S22e/S22c baseline + ≥ `PERSIST_COHORT` committed agent ids cultivating ≥
`PERSIST_FRACTION` of the final window) — while a non-committed buyer cohort survives and materially buys,
SALT promotes on `SelfProduced` bread, mortality runs, provenance is clean, conservation holds — AND is it a
genuinely **voluntary** institution (not a fiat re-pin), with persistence across terms driven by **renewals
from fresh signals**?

**Primary success = `RoleStickySuccess` (by agent id** — the institution is individual voluntary commitment,
not inheritance; lineage persistence reported secondary):
1. **Commitment non-vacuous** — ≥ `MIN_COMMITS` (4) agents **voluntarily** commit *after* money exists
   (entry gated by their own realized return; not forced).
2. **Churn falls materially** — per-ever-cultivating churn ≤ `CHURN_DROP` (0.5) × the matched-seed baseline.
3. **A persistent committed cohort forms** — ≥ `PERSIST_COHORT` (4) distinct committed agent ids are
   cultivating/committed ≥ `PERSIST_FRACTION` (0.5) of the final window.
4. **Persistence is re-chosen, not one long pin** — where a cohort id's persistence spans more than one term,
   it shows ≥1 **renewal from a fresh post-expiry signal** (the anti-disguised-pin clause); the shipped term
   is ≤ the final window.
5. **A surviving non-committed buyer cohort materially buys** — post-promotion bought food ≥
   `MATERIAL_BOUGHT_FLOOR`, living.
6. **Money survives** — SALT promotes and remains money; food materially bought after promotion.
7. **Provenance clean** — sold/pre-promotion bread is `SelfProduced`; `seeded_minted == 0`.
8. **Mortality + conservation + tool invariants hold** (the latter only if the S22e-composed variant runs).
9. **NOT downgraded by the controls (§4)** — the `fiat_pin` control is `RePinScaffold`; the
   `unprofitable_offer` control gets zero uptake; commitment is genuinely voluntary.

**Finding modes (pre-named; first-class; verdict test prints the classification, does NOT assert SUCCESS):**
- `CommitmentUnchosen` (precondition / non-vacuity failure) — the institution is offered but **no** agent
  voluntarily enters (e.g. no signal ever clears). Distinguishes "nobody chose it" from "no stickiness."
- `SignalVacuous` — the S22c realized-return signal doesn't discriminate (no real entry decision to make).
- `ConservationBroken` / `extinct` — any invariant break or colony death.
- `RePinScaffold` (the honest-trap mode) — success-like metrics hold ONLY under the `fiat_pin` control
  (forced commitment), OR uptake is not actually voluntary (entry not gated by the agent's own cleared
  signal), OR persistence is one un-renewed mega-term. Not headline success.
- `MonopolizationCull` — committed cultivators dominate grain/bread (share ≥ `MONO_SHARE` = 0.75) AND the
  buyer cohort collapses.
- `CommuneCollapse` — cultivator/commit share ≥ `COMMUNE_SHARE` AND post-promo bought < floor.
- `MoneyFailureFromCommitment` — commitment disrupts medium exchange; SALT fails/demonetizes.
- `TermTooShortFinding` — short terms bite (real commitments) but cannot form persistence (the term < what
  the cohort bar needs).
- `NoStickinessDespiteCommitment` — commitments happen and bind, but churn/cohort bars still fail.
- `RoleStickySuccess` — all nine success clauses, not downgraded.

**Ordered classifier (top-down, first-match-wins):** `CommitmentUnchosen` → `SignalVacuous` →
`ConservationBroken`/`extinct` → `MonopolizationCull` → `CommuneCollapse` → `MoneyFailureFromCommitment` →
`RePinScaffold` → `TermTooShortFinding` → **then the explicit final gate:** `if ALL NINE success clauses pass
{ RoleStickySuccess } else { NoStickinessDespiteCommitment }`. Predeclare every threshold as a `const`; do
NOT fit.

## 3. Engine design (additive, default-off, conservation-safe)

1. **NEW default-off flag** `ChainConfig::voluntary_cultivation_commitment: bool` + fields:
   `commitment_term: u16` (the binding length; shipped from an existing scale — `RETURN_WINDOW` or
   `2 × RETURN_WINDOW`; swept `{12, 24, 48, 96}`; MUST be ≤ the final measurement window), and a
   `commitment_entry_floor` reusing the S22c material-floor logic. Helper
   `voluntary_cultivation_commitment_active(&self)` = flag on AND `profit_driven_retention_active()` (the
   entry signal is the S22c one; inert pre-money via the same `current_money_good()==Some(SALT)` gate).
   Canonicalize ON-only with the **next free flag-digest tag (12** unless master advanced) + `commitment_term`
   + the entry params. Off ⇒ byte-identical.

2. **Per-agent commitment state (steers `cultivating`, NOT `Vocation`):** add `commitment_remaining: u16`
   (ticks left in the current term; 0 = uncommitted), `commitment_renewals: u16` (count of re-commits from a
   fresh post-expiry signal). These STEER behavior so they MUST be serialized into `canonical_bytes` ON-only
   under the gate (the S22c/S22e discipline) — NOT runtime-only. Pure steering state; no goods, so no direct
   conservation term (cultivation effects flow through the existing grain/bread accounting, which remains the
   guard).

3. **Entry (voluntary opt-in):** at the cultivate decision, post-money, for an **uncommitted** agent: if its
   S22c realized cultivation-return signal clears the entry floor vs its outside option (the **same** rule
   S22c uses for retention — reuse, don't reinvent), it MAY opt in → set `commitment_remaining =
   commitment_term`. No quota/top-N: if nobody's signal clears, nobody commits. Inert pre-money. (Whether
   opt-in is automatic-on-clear or a probabilistic/deterministic chooser — keep it deterministic on the
   cleared signal; document + digest.)

4. **Binding (the exit override — the one new exit behavior):** while `commitment_remaining > 0`, the agent
   **cultivates** — the normal hunger/profit exit branch (settlement.rs ~9755 disjunction) cannot turn
   cultivation off until the term expires. Decrement `commitment_remaining` once per econ tick. This is the
   ONLY exit edit in the arc, and it is gated entirely behind the voluntary, signal-cleared, post-money entry
   — it is not a fiat "cultivators must cultivate" rule.

5. **Expiry + renewal (re-open choice):** when `commitment_remaining` reaches 0, the agent returns to the
   normal S22a/S22c fluid logic and **re-decides from fresh realized returns**; if the signal still clears it
   may re-commit (increment `commitment_renewals`). No permanent commitment; expiry always re-opens the
   choice.

6. **Base + composition:** headline scenario `frontier_voluntary_commitment` = the **expanded S22e roster**
   (`ROSTER_HOUSEHOLDS = 8`, reachability) with `endowed_cultivation_capital = FALSE` (so the result is the
   commitment institution, NOT capital + a contract) + `profit_driven_retention = true` +
   `voluntary_cultivation_commitment = true`. A **secondary** S22e-composed variant (endowed capital ON) is
   reported separately; the headline must NOT require endowed tools.

7. **Diagnostics (runtime-only):** uptake tick + the signal value at uptake per committer; committed-cohort
   ids; renewals per id; committed vs non-committed grain/bread share; non-committed buyer cohort + post-promo
   bought; churn vs matched-seed baseline; the fiat/nonbinding control read-outs.

## 4. The new suite `sim/tests/voluntary_cultivation_commitment.rs`

- **MANDATORY NON-VACUITY / VOLUNTARINESS TEST** (else `CommitmentUnchosen`): ≥ `MIN_COMMITS` agents
  **voluntarily** commit *after* money exists, each entry traceable to that agent's own cleared S22c signal
  (record the signal at uptake); AND at least one commitment **binds** a tick that the matched flag-off run
  would have exited (a real exit-override). The signal must discriminate across agents (else `SignalVacuous`).
- **The ordered classifier (§2)**, printed `--nocapture`; verdict test prints verdict + deciding metrics
  (primary `RoleStickySuccess` by id + secondary lineage persistence), does NOT assert SUCCESS.
- **Controls (each a test):**
  - **commitment_off** = the expanded base + S22c only (reproduces S22e/S22c no-stickiness).
  - **unprofitable_offer** (`commitment_entry_floor` impossibly high): the institution is offered but gets
    **zero uptake** → `CommitmentUnchosen` (proves uptake is voluntary/signal-gated, not automatic).
  - **nonbinding_term** (`commitment_term = 1`): a one-tick "commitment" should reproduce S22c **marginal**
    retention (proves the binding *term*, not the act of committing, is what matters).
  - **fiat_pin** (forcibly commit a matched number of agents from tick 0, bypassing the voluntary signal):
    must classify `RePinScaffold` and NEVER count as headline success (the key anti-repin falsifier — shows
    a forced pin is a different thing from voluntary commitment).
  - **capital_composition** (S22e endowed capital ON): reported as the secondary variant; the headline must
    succeed/fail without it.
- **HARD GUARDS every run + cell:** conservation every tick; `bread_minted_max == 0`; provenance
  clean-or-disqualified; `!extinct`; (S22e-composed variant only) the tool-stock invariant; the term ≤
  final-window assertion.
- **goldens_unchanged** test pinning the five tripwire digests (copy from `endowed_inherited_capital.rs`).
- **Robustness mini-sweep** over `commitment_term ∈ {12, 24, 48, 96}` + grain flow, classified, no tuning.
  The `commitment_term` axis MUST be outcome-driving: show the verdict/persistence move with it (too short →
  `TermTooShortFinding`; adequate → potential success; over-long → flag the disguised-pin risk via the
  renewals check).
- A `RePinScaffold` separation test: the headline (voluntary) and the `fiat_pin` control must be
  **distinguishable** — even if both show low churn, only the voluntary one has signal-gated uptake +
  renewals; assert the headline is NOT merely the fiat outcome.

## 5. Verification (independent gate)

Redirect cargo to files; never pipe to head/grep (EPIPE → spurious exit 101).
- `cargo test -p sim --test voluntary_cultivation_commitment` passes (non-vacuity/voluntariness + the exit-
  override + the controls incl. fiat_pin separation).
- `cargo test --workspace` passes; **all existing goldens byte-identical** (`goldens_unchanged` +
  endowed_inherited_capital / durable_cultivation_capital / profit_driven_retention / occupational_stickiness
  / endogenous_cultivation_entry / robustness_appendix / household_barter / mortality / open_colony_mortality
  / demand_survival_bridge).
- `cargo fmt --check` clean; `cargo clippy --workspace --all-targets -- -D warnings` clean; conservation holds.

## 6. Honesty and scope (state these in the result; do not let a SUCCESS overclaim)

- **A SUCCESS is institutional sufficiency with ENDOGENOUS UPTAKE, not endogenous self-formation.** Honest
  headline: *"a voluntary binding vocation institution stabilizes the occupation that hunger, skill, profit
  incentives, earned capital, and inherited capital could not."* The institution is **configured**; the
  *uptake* (who commits, when) is endogenous and signal-gated. NOT "occupation emerges without institutions."
- **The `fiat_pin` control + the voluntary-uptake gating + the renewals clause are load-bearing for that
  claim** — they are what distinguish a genuine voluntary institution from re-pinning the S21 producer class.
  The classifier downgrades to `RePinScaffold` if they don't separate.
- **The binding exit-override is the first and only exit edit in the arc** — disclose it as exactly that, and
  show (nonbinding_term control) that the *term* is what matters, and (unprofitable_offer control) that
  uptake is voluntary.
- **Bounded to this WOOD-poor, mortality-on, expanded-roster regime** and this commitment design; like
  S21h/i, expect possible band-qualification — report the `commitment_term` and grain windows where it holds.
- Follow repo conventions; NEVER add Claude/AI/assistant references in code, comments, or committed text.
