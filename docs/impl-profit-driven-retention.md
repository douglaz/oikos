# impl-35 — S22c: Profit-Driven Cultivation Retention (does a monetary stay-decision turn fluid participation into occupation?)

Status: SPEC-READY — Codex spec-review NEEDS-REVISION → five decisions settled (§8) and the punch-list
folded in: the return signal is **per-agent cultivation-sale proceeds** (provenance+trade attribution),
NOT gold-delta (vacuous by phase order — bread sells a tick after it's produced); the rolling-return
accumulators are **digested ON-only** (they steer behaviour — not runtime-only) (§3.2/§6); the outside
option is a **rate** with a colony-reference fallback (§8.2); the headline is **skill-OFF (S22a base)**
compared to the **matched-skill** no-retention baseline (§2/§3.4); the non-vacuity test requires a real
**counterfactual exit flip** + cross-agent variation (§7); `RETURN_WINDOW=48`, `RETENTION_MARGIN_BPS=0` +
a material floor (§7/§8). Round 2 (NEEDS-REVISION → substantive parts sound, 2 consistency fixes
applied: §5 baselines matched to skill setting, §10 wording). **Confirmed-implementable attribution
(Codex):** extend `produced_lots` — on a post-promotion bread spot sale, draw the seller's lots and
credit cultivation-sale proceeds only for lots where `lot.producer == seller` (ignore resold bread),
crediting at sale-time to the original same-agent producer — which resolves the phase-order vacuity.

## 0. One-paragraph summary

S22b found that accumulated productivity (cultivation skill) **bites** but does **not** produce occupation:
the cultivation *exit* is hunger-only, so agents leave as soon as hunger eases, no matter how skilled —
churn stays at the S22a fluid baseline and no persistent cohort forms. S22b named the next condition:
occupation needs a mechanism that changes the **decision to stay**. S22c is the smallest authentic such
mechanism (Codex-scoped): a default-off **profit-driven retention** rule that, **only after money
exists**, lets a cultivating agent **remain** cultivating past the normal hunger-exit when its **recent
realized cultivation return ≥ its outside option**. Entry stays hunger-gated (S22a/b unchanged) — *hunger
discovers the role; money makes it occupationally persistent.* The hard anti-circularity guard: the
profit calculation is inert until `current_money_good() == Some(SALT)` (no pre-money monetary calc). The
central question: **does a realized monetary stay-decision turn S22a's fluid participation into a stable
role split** — a persistent cultivator cohort + persistent buyers — while preserving money, mortality,
and provenance? Measured on the **same S22b stickiness spine** (churn drop vs matched-seed baseline +
persistent membership cohort) so it is directly comparable. Classify-not-tune, with a mandatory
**non-vacuity / SignalVacuous** check (the signal must actually exist and discriminate, the S22b
lever-bite lesson) and the same conservation/digest discipline (all goldens byte-identical when off).

## 1. Why this milestone, why this lever

S22b isolated the failure precisely: the cultivation `cultivate_now` decision exits whenever a
currently-cultivating agent's hunger drops below `cultivate_hunger_out` (settlement.rs ~9758), regardless
of how productive or skilled it is. So *productivity-while-in* can never become *occupation* — the exit
is hunger-only. S22c attacks exactly that branch: make the **exit** profit-modulated. This is the
minimal authentic "decision to stay" — an agent that is *realizing monetary gains* from selling its
`SelfProduced` surplus keeps cultivating; one that is not, leaves on the normal hunger exit. Deferred
(Codex): heritable skill (demographic persistence, not a stay decision), durable role-specific capital
(authentic but larger; still needs a stay/ownership decision), and a global role chooser (too broad).

## 2. The central question and pre-named outcomes

**Central question.** Does a realized, post-money profit-stay rule on the cultivation *exit* turn S22a's
fluid participation into a **stable** role split (persistent cultivator cohort + persistent
non-cultivating buyers), while SALT still promotes on clean `SelfProduced` bread, mortality is survived,
and conservation holds?

**SUCCESS** (all, across `SEEDS`, vs the **matched-seed no-retention baseline with the SAME skill
setting** — S22a for the skill-off headline, S22b for the skill-on variant; same stickiness spine as
S22b):
1. **Churn falls materially** — per-ever-cultivating-agent churn ≤ `CHURN_DROP` (0.5) × the matched-seed
   same-skill no-retention baseline churn.
2. **A persistent membership cohort forms** — ≥ `PERSIST_COHORT` (4) distinct agent **ids** each
   cultivate ≥ `PERSIST_FRACTION` (0.5) of the final window, ≥2 non-lineage.
3. **A persistent non-cultivating buyer cohort remains** (material bought food, living).
4. **Money survives** — SALT promotes and remains money; food materially bought after promotion;
   WOOD↔SALT clears.
5. **Provenance clean** — pre-promotion + sold bread is `SelfProduced`, `seeded_minted == 0`.
6. **No commune collapse, no monopolization cull, no extinction; conservation every tick.**

**Finding modes (pre-named; each first-class):**
- **SIGNAL VACUOUS** — the recent-return / outside-option signal doesn't actually exist or discriminate
  (e.g. agents accrue no measurable per-window cultivation return for the rule to act on), so the rule
  never fires. Detected by the **mandatory non-vacuity test** (§7); a distinct outcome from "no stay"
  (the S22b LEVER-INERT lesson — don't call an inert signal "no stickiness").
- **NO STAY DESPITE PROFIT** — the signal exists and fires but churn stays S22b-like (the stay-decision
  doesn't translate into a persistent cohort).
- **COMMUNE COLLAPSE** — retention makes too many agents stay cultivating; bought food collapses.
- **MONOPOLIZATION CULL** — retained cultivators dominate grain and starve the demand side.
- **MONEY FAILURE FROM LOCK-IN** — sticky roles form but break the exchange lanes / SALT promotion.
- **OSCILLATION** — agents chase stale returns; churn rises / never settles.

**Ordered classifier (mutually exclusive, top-down — the S21i/S22a/b discipline):**
1. **SIGNAL VACUOUS** (precondition, from the non-vacuity test) → headline verdict; per-seed classification reported but moot.
2. **BROKEN-INVARIANT / EXTINCT** — any guard fails.
3. **MONOPOLIZATION CULL** — top-cultivator grain share ≥ `MONO_SHARE` (0.75) AND non-lineage/buyer collapse (dominance AND damage).
4. **COMMUNE COLLAPSE** — rolling cultivator share ≥ `COMMUNE_SHARE` AND post-promotion bought < `MATERIAL_BOUGHT_FLOOR`.
5. **MONEY FAILURE FROM LOCK-IN** — `SelfProduced` bread produced + sold but `current_money_good() != SALT` at horizon.
6. **OSCILLATION** — churn ≥ `CHURN_LIMIT` AND no settled final-window cultivator-share band.
7. **NO STAY DESPITE PROFIT** — money + mortality survive but churn > `CHURN_DROP ×` the matched-seed same-skill no-retention baseline AND no persistent membership cohort.
8. **SUCCESS** — none of the above AND §SUCCESS 1–6 hold.

Verdict test computes this order, prints verdict + deciding metrics, **does not assert SUCCESS**. All thresholds pre-stated in §7. Churn always vs the **matched-seed same-skill no-retention** run (S22a for the skill-off headline, S22b for the skill-on variant).

## 3. What gets built

### 3.1 The gate (additive, default-off, ON-only digest)

- New `ChainConfig` flag `profit_driven_retention: bool` (default false) + a
  `profit_driven_retention_active(&self)` helper (active when the flag is on AND
  `endogenous_cultivation_entry_active()` — composes on S22a; orthogonal to S22b skill, works with it on
  or off). Canonicalized **ON-only** with the next free digest tag (9 unless master advanced). Off ⇒
  byte-identical to the current stream. The retention changes the per-agent `cultivating` trajectory,
  which IS digested under the cultivation gate, so the new scenario's digest is new (expected); existing
  goldens are untouched because the flag is off for them.

### 3.2 The per-agent rolling-return tracker (behavioral state — digested ON-only, NOT runtime-only)

Research confirms **no clean per-agent recent-return signal exists** (all are lifetime-cumulative or
class-binned), so S22c adds one. **Because `profit_stay_active` reads it to change the next `cultivating`
flag, this tracker is FUTURE-BEHAVIOUR STATE, not a diagnostic** — so it is serialized into
`canonical_bytes` **ON-only under the gate** (the `cultivation_skill` discipline), NOT runtime-only
(Codex P1: runtime-only is only for things that don't steer behaviour). Off ⇒ not emitted ⇒ goldens
byte-identical; on ⇒ digested deterministically. (Only the *diagnostic* read-outs in §3.4 stay
runtime-only.)

The signal is **per-agent cultivation-sale proceeds**, attributed to the producing cultivator — NOT a
gold-delta (which is vacuous by phase order, §8.1):

- Extend the bread-provenance / trade pass with **per-agent seller-origin attribution**: when an agent
  sells bread for SALT post-money and the outgoing bread is `SelfProduced` **and was produced by that
  same agent** (its own cultivation surplus), credit the proceeds to
  `recent_cultivation_proceeds[id]` over a rolling `RETURN_WINDOW` (48 ticks).
- `recent_outside_proceeds[id]` — the agent's realized proceeds from **non-cultivation** sales (e.g.
  WOOD) over the same window — the realized outside option.
- Both are kept as per-tick rates (proceeds / observed ticks) so a continuous cultivator (no outside
  ticks) falls back to the **colony reference** outside rate (median/mean non-cultivating-seller rate
  over the window), per §8.2.
- Ring-buffer / windowed sums; deterministic; no RNG. Serialized ON-only (the windowed accumulators that
  feed the decision are part of the digested state when the gate is on).

### 3.3 The retention rule (the one behavioral injection)

At the cultivation exit branch (settlement.rs ~9758, the `was_cultivating && (hunger >= cult_out || …)`
disjunction), inject a profit-stay term:

```rust
let cultivate_now = cultivation && eligible
    && (pressure >= cult_patience
        || (was_cultivating
            && (hunger >= cult_out
                || has_cultivation_input_in_flight
                || self.profit_stay_active(id))));   // S22c
```

`profit_stay_active(id)` returns true iff **all** of:
- `current_money_good() == Some(SALT)` (the anti-circularity gate — no pre-money monetary calc; the
  signal itself is post-money realized sale proceeds, §8.3), AND
- the agent's `recent_cultivation_proceeds[id]` clears the **material floor** (it is actually realizing
  cultivation sales — not one dust sale; not vacuous), AND
- `recent_cultivation_proceeds_RATE[id] ≥ outside_RATE[id] + RETENTION_MARGIN_BPS` where `outside_RATE`
  is the agent's realized non-cultivation rate or, if it has no recent outside ticks, the colony
  reference outside rate (§8.2). I.e. cultivating earns it at least as much, per tick, as its realized
  (or the typical) alternative.

Entry is **unchanged** (still hunger/pressure-gated); only the **exit** is profit-modulated. Skill (S22b)
may raise the cultivation proceeds (more surplus to sell) but the stay is mediated by **realized gain**,
not by "skilled" — they compose, not conflate.

### 3.4 Diagnostics (runtime-only) + scenario

Reuse the S22a/b diagnostics + add (runtime-only read-outs): count of agents currently
retained-by-profit (stayed past a hunger exit they'd otherwise have taken — the counterfactual flip
count), the recent-return distributions, and the persistent-membership cohort (S22b metric). **Headline
scenario `frontier_profit_retention` derives from `frontier_endogenous_cultivation` (S22a, skill OFF)** +
`profit_driven_retention = true`, to isolate the stay decision as the sole new lever; a **skill-ON
variant** derives from `frontier_occupational_stickiness` (S22b) + the flag, reported as composition.
Each is compared to its matched-skill no-retention baseline (S22a / S22b respectively).

## 4. Anti-circularity & anti-tuning discipline

- **Anti-circularity (the main trap, Codex):** `profit_stay_active` is hard-gated on
  `current_money_good() == Some(SALT)`; before money it always returns false and the exit is pure
  hunger. The return signal is **realized post-money bread-sale proceeds read from the trade tape**
  (credited only on spot sales after promotion), never a gold-delta (which promotion/conversion/transfers
  could pollute, Codex P1) and never an imputed pre-money valuation.
- **Anti-tuning:** `RETURN_WINDOW`, `RETENTION_MARGIN`, and any signal threshold are **predeclared
  modest constants**, swept in the robustness mini-sweep, never hand-tuned to manufacture a cohort. The
  verdict test never asserts SUCCESS. The shuffle/zero-returns control (§5) proves the *signal* (not a
  tuned preference) drives any stickiness.

## 5. Controls (classify, never tune)

- **Flag-off baselines (matched to skill setting)** — the headline skill-off treatment compares to
  `frontier_endogenous_cultivation` (S22a); the skill-on variant compares to
  `frontier_occupational_stickiness` (S22b). Each baseline reproduces its source milestone's metrics
  (the comparison baseline for the churn-drop bar).
- **Profit-stay ON** = `frontier_profit_retention` (headline, skill-off S22a base) — the treatment.
- **Signal-unavailable control** — profit-stay on but money never promotes (or the return tracker forced
  empty): must NOT create stickiness (proves the rule needs the realized signal).
- **Shuffle/zero-returns control** — feed the rule shuffled or zeroed recent-return values: stickiness
  must disappear (proves the *signal*, not the rule's mere presence, drives it).
- **Skill-on + profit-stay-off** = S22b — remains NoStickiness (isolates that profit-stay, not skill, is
  the new lever).
- **High-retention sensitivity** — a permissive margin / long window: classified SENSITIVITY (likely
  commune / monopolization), excluded from the core verdict, shows the boundary.

## 6. Determinism & goldens (the safety argument)

One additive default-off gate (tag 9, ON-only) + the per-agent rolling-return accumulators (digested
ON-only under the gate, since they steer the next `cultivating` flag — NOT runtime-only, Codex P1; only
the §3.4 read-out diagnostics are runtime-only) + the one exit-branch injection (only fires under the
gate). Off ⇒ no tag 9, the accumulators are never created/consulted, the exit branch is exactly today's
⇒ **all five pinned goldens byte-identical** (asserted in the new suite). On ⇒ the digest changes
deterministically (a new scenario, expected). Conservation: profit-stay only keeps an agent `cultivating`
longer (more grain hauled, bounded by the conserved node; no minting, no recipe change) — a hard per-tick
`conserves()` guard on every cell.

## 7. Acceptance criteria (independent verification)

- **MANDATORY non-vacuity test (a real exit FLIP, not mere reachability — Codex P2):** under the
  treatment, ≥1 post-money agent with `hunger < cultivate_hunger_out`, no input in flight, and clean
  cultivation-sale return ≥ outside option is **RETAINED where the matched flag-off run would have
  EXITED** (a demonstrated counterfactual stay), AND the cultivation-proceeds signal **varies across
  agents** (not a single agent firing once). If not → headline **SIGNAL VACUOUS**, not "no stay" (the
  S22b lever-bite lesson).
- New suite `sim/tests/profit_driven_retention.rs`: the §2 ordered classifier across `SEEDS`,
  diagnostics printed, verdict not asserted SUCCESS.
- **Pre-stated thresholds (a priori):** `CHURN_DROP=0.5` (vs the matched-seed same-skill no-retention baseline),
  `PERSIST_FRACTION=0.5`, `PERSIST_COHORT=4` (≥2 non-lineage), `MONO_SHARE=0.75`, `COMMUNE_SHARE`,
  `CHURN_LIMIT`, `MATERIAL_BOUGHT_FLOOR`, `RETURN_WINDOW = 48` econ ticks, `RETENTION_MARGIN_BPS = 0`
  (+ a small material proceeds floor so one dust sale can't lock an agent in).
- Flag-off control reproduces S22b (behavior/metrics-identical); shuffle/zero-returns kills stickiness;
  signal-unavailable doesn't create it.
- Guards every run + cell: conservation, `bread_minted_max == 0`, provenance clean-or-disqualified, `!extinct`.
- Robustness mini-sweep over `RETURN_WINDOW`/`RETENTION_MARGIN` + grain flow, classified, no tuning.
- Workspace: all tests pass; **all goldens byte-identical**; fmt + clippy -D warnings clean.

## 8. Resolved decisions (Codex spec-review)

1. **Return signal → per-agent cultivation-sale proceeds (NOT gold-delta).** Credit only realized
   **post-money bread-sale proceeds** where the seller's outgoing bread is `SelfProduced` **and produced
   by that same agent** — via per-agent seller-origin attribution on the bread-provenance / trade pass.
   Gold-delta bucketed by "was cultivating this tick" is **vacuous by phase order** (Codex P1): cultivated
   bread is produced *after* the market, so the surplus sells on a *later* tick — if the agent exits
   before selling, the sale lands in the outside bucket; gold-delta also catches non-cultivation sales
   and other gold movements. The signal must be cultivation-output-specific.
2. **Outside option → a rolling RATE with a colony-reference fallback.** Compare per-tick *rates*, not
   raw rolling sums (Codex P2): a continuous cultivator has no recent outside ticks, so a raw outside sum
   is undefined/zero and would make retention trivially easy. Use the agent's realized non-cultivation
   sale-proceeds *rate* when observed; else the colony's median/mean non-cultivating-seller rate over the
   same window; fall back to zero only when no reference exists at all, and then report the outside option
   as *weak* (a classified caveat, not a silent pass).
3. **Window + margin → `RETURN_WINDOW = 48` econ ticks, `RETENTION_MARGIN_BPS = 0`** for the shipped
   first slice, plus a small **material floor** (one dust sale must not lock an agent in). Sweep
   window/margin as sensitivity.
4. **Headline scenario → skill-OFF (S22a base) + profit-stay**, to isolate the stay decision; a skill-ON
   (S22b base) variant is reported as composition. **Churn is compared to the matched-seed
   no-retention baseline with the SAME skill setting** — S22a for the headline, S22b for the skill-on
   variant (Codex P2: don't compare a skill-off treatment to the S22b baseline).
5. **Non-vacuity → a real counterfactual exit FLIP, not mere reachability** (Codex P2): at least one
   post-money agent with `hunger < cultivate_hunger_out`, no input in flight, and clean
   cultivation-sale return ≥ outside option is **retained where the flag-off path would have exited**,
   AND the signal varies across agents (not "fires once").

## 9. Scope boundary (deferred to S22d+)

Heritable skill; durable role-specific capital; a global role chooser; profit-driven *entry* (this slice
only modulates *exit/retention*, entry stays hunger-gated); endogenizing the clearing institution.

## 10. Risks

- **Circularity (the main trap).** Pre-money monetary calc → reviewer attack. Mitigated by the hard
  `current_money_good() == Some(SALT)` gate; realized post-money cultivation-sale proceeds signal (drawn
  from same-agent-producer `produced_lots` at sale time — never a gold-delta).
- **Signal vacuity (the S22b lesson).** A signal that never fires or never discriminates makes the result
  meaningless. Mitigated by the mandatory non-vacuity test + the SIGNAL-VACUOUS verdict.
- **Tuning trap.** Predeclared constants, the shuffle/zero control, the sweep, no SUCCESS assertion.
- **Conservation.** Retention only extends cultivation (bounded node draw); hard `conserves()` guard.

## 11. Pipeline

Codex spec-review (settle §8) → SPEC-READY → setsid rb-lite `claude,codex` → independent verification
(workspace + all goldens byte-identical + the new suite + the verdict run) → Codex review-of-results →
merge + report-update + memory + pin.
