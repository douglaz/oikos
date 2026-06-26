# impl-33 — S22a: Endogenous Cultivation Entry (does the food-producing class form from pressure, not lineage identity?)

Status: LANDED — verdict **SUCCESS, reframed as FLUID cultivation participation** (Codex
review-of-results PASS-WITH-CAVEATS, no P1 code defect; the framing caveats below are folded in). The
single engine change (the default-off `endogenous_cultivation_entry` gate + the §3.2 eligibility
override, canonicalized ON-only with digest tag 7) plus runtime-only diagnostics is implemented; the new
suite `sim/tests/endogenous_cultivation_entry.rs` classifies the headline
(`frontier_endogenous_cultivation`, mortality on) as **SUCCESS across all SEEDS** via the §2 ordered
classifier: **all 18 non-lineage roles enter cultivation** at some point (≫ `MATERIAL_ENTRY_FLOOR=4`),
14–17 of them sell `SelfProduced` bread for SALT (≥2) at production-time provenance, SALT promotes on a
clean (`seeded_minted==0`) supply, food is materially bought after promotion, a live non-cultivating
buyer cohort (7–10) persists, and the WOOD↔SALT lane clears — money + mortality survive the relaxed
producer identity.

**HONEST FRAMING (Codex P2 — not a stable occupational class).** The cultivation is **FLUID/ROTATING
participation**, NOT a fixed producer class: at any instant only ~5% are cultivating (rolling share
settled), but the *membership rotates rapidly* — churn ≈ 23–24 enter/exit transitions per
ever-cultivating non-lineage colonist over 1600 ticks, and *every* non-lineage role dips in. The honest
reading is "everyone occasionally self-provisions under acute hunger, then returns to buying," not "a
food-producing class self-forms." So S22a shows **cultivation is an endogenous survival behavior
available to all under pressure**, dissolving the lineage *privilege* — it does not show a stable
division of labor (that, and any occupational stickiness, is S22b+).

Controls: the pinned-topology baseline still succeeds (12/18); money-machinery-off fails to promote
(Oscillation/MoneyFailure); low/no grain-flow does not fake success (everyone cultivates, nothing
trades — CommuneCollapse); the mortality-off sanity variant succeeds. **Three control findings (Codex,
reported not forced):** (1) the no-hysteresis control's literal `cultivate_hunger_out =
cultivate_hunger_in` is rejected by the engine invariant `out < in`, so it is pinned to the narrowest
realizable band (`out = in-1`, `patience = 1`) — disclosed, not a retune; (2) **the no-hysteresis
control does NOT create a distinct failure regime** — the headline already churns far above
`CHURN_LIMIT`, so removing most hysteresis leaves aggregate stability intact while per-agent churn stays
high in both; the hysteresis is **not load-bearing** for aggregate stability here; (3) the
no-emergency-floor control does NOT reproduce the S21g cull under endogenous entry — relaxing
eligibility makes cultivation itself a survival path, so the emergency floor is no longer the sole
demand-side bridge. All five tripwire goldens are byte-identical; off the flag the chain is
byte-identical to the S21h stream; `cargo fmt --check` and
`cargo clippy --workspace --all-targets -- -D warnings` clean; conservation holds every tick.

Prior: SPEC-READY — two Codex spec-review rounds folded in. Round 1 (NEEDS-REVISION): four decisions
settled (§8) + 6-item punch-list (eligibility override pinned §3.2; ordered classifier + numeric
thresholds §2/§7; held/bought gate removed §4; no-hysteresis control pinned §5; material-buyer +
production-time-provenance diagnostics §3.3). Round 2 (NEEDS-REVISION → pre-approved): the override
preserves the `Consumer|Gatherer|Unassigned` vocation filter (§3.2), and `CHURN_LIMIT=8` is predeclared
a priori, not fitted to the control (§7).

## 0. One-paragraph summary

Through S21, the open colony's **food-producing class is pinned**: a pre-identified cultivator
*lineage* (the spatial households) cultivates `SelfProduced` bread and barters/sells the surplus, while
non-lineage Consumers (buyers) and Gatherers (woodcutters) never cultivate. That lineage privilege is
the biggest remaining scaffold against "division of labor arises from *choice*, not placement." S22a is
the smallest slice that attacks it: a **default-off `endogenous_cultivation_entry` gate** that relaxes
cultivation eligibility from "spatial household lineage member" to **"any spatial colonist under
sustained hunger pressure,"** reusing the existing S15/S21f cultivation **pressure/patience hysteresis**
(no profit optimizer — the opportunity cost is *structural*: a tick spent cultivating cannot gather
WOOD or do the current task). The central question: **can the food-producing class self-form from agent
pressure rather than lineage identity, while the open colony still supports money and mortality?** This
is **cultivation participation**, not full vocation topology (the honest scope; profit-driven drift,
Miller/Baker entry, and a global role optimizer are explicitly deferred to S22b+). The milestone
classifies the outcome against pre-named success and five finding modes; it must **not tune** thresholds
to manufacture a self-organized split — it reuses the existing thresholds, sweeps them, and accepts
commune/failure as a first-class finding.

## 1. Why this milestone, why this slice

Codex's strategic evaluation ranked endogenizing the role topology as the top authenticity milestone,
and its scoping review chose **cultivation entry** as the right first slice: it is the smallest change
that attacks the biggest scaffold (the pinned cultivator lineage as the food-supply class), it reuses an
existing hysteresis rather than inventing a cardinal chooser, and it is additively gated so all goldens
stay byte-identical. Consumer/Gatherer *profit* drift is tempting but leaves the food producer pinned
and needs an invented "return to buying" comparison; a full highest-net-return role optimizer is too big
and too cardinal too early. So S22a asks specifically: **who enters household bread production** — and
whether the open-colony money+mortality result survives once that is no longer an assigned identity.

## 2. The central question and pre-named outcomes

**Central question.** Can the food-producing class form from agent **hunger pressure** rather than
**lineage identity**, while the open colony still promotes SALT (on `SelfProduced` bread) and survives
the positive check?

**SUCCESS** (all of, classified by the diagnostics in §3.3, across `SEEDS`):
1. **Non-lineage entry is real** — a material number of non-lineage agents actually enter cultivation
   under the new gate (not negligible; the threshold for "material" is pre-stated in §7, not tuned).
2. **Provenance clean** — pre-promotion bread sold for SALT is `SelfProduced` (`seeded_minted == 0`),
   exactly as S21f/S21h require.
3. **SALT promotes** under the unchanged S20/S21a/b/c machinery.
4. **Material buying after promotion** — food is still bought on the market (`bought` channel above the
   `MATERIAL_BOUGHT_FLOOR`), i.e. a real demand side persists.
5. **A non-cultivating demand side remains alive and buying** — not everyone becomes a cultivator.
6. **WOOD exchange remains material** — the bread→SALT `IndirectFor{WOOD}` lane still clears.
7. **Not a single-commune state** — rolling-window diagnostics show *both* a cultivator cohort and a
   non-cultivating buyer/woodcutter cohort coexisting (a genuine split, not a uniform self-provisioning
   commune).

**Finding modes (pre-named; any one is a first-class, publishable result, not a failure to fix by
tuning):**
- **COMMUNE COLLAPSE** — most survivors cultivate/self-provision, `bought` food collapses, and money
  fails or becomes irrelevant. (Division of labor dissolves once entry is free.)
- **PINNED-LINEAGE NECESSITY** — only the original lineage cultivates materially; non-lineage entry is
  negligible. (The assigned producer identity was load-bearing — itself a real result.)
- **DEMAND-SIDE COLLAPSE** — the non-cultivators die or disappear before money forms (an S21g-like cull
  re-emerges because the survival bridge / entry interacts badly with mortality).
- **OSCILLATION** — agents churn in and out of cultivation; output and trade never stabilize. (The
  hysteresis is insufficient; a no-hysteresis control should *exhibit* this on purpose.)
- **MONEY FAILURE DESPITE PRODUCTION** — bread is produced and sold, but SALT fails to promote once
  identity pinning is removed (the medium-leadership conditions need the pinned structure).

**Ordered classifier (mutually exclusive — checked top-down; the FIRST that matches is the verdict, the
S21i non-gameability discipline).** The categories above overlap (a run can have material entry *and*
collapsed buying *and* no promotion), so the verdict is assigned by priority, not by an unordered OR:

1. **BROKEN-INVARIANT / EXTINCT** — any guard fails (conservation, `bread_minted_max>0`, provenance
   disqualified, whole-colony die-off). A hard test failure, never a regime.
2. **DEMAND-SIDE COLLAPSE** — living non-lineage roles fall to ~0 before promotion (the S21g cull
   re-emerges).
3. **PINNED-LINEAGE NECESSITY** — distinct non-lineage cultivation entrants `< MATERIAL_ENTRY_FLOOR`
   (entry is negligible; only the old lineage cultivates materially).
4. **COMMUNE COLLAPSE** — rolling cultivator share ≥ `COMMUNE_SHARE` (most survivors cultivate) AND
   post-promotion `bought` food below `MATERIAL_BOUGHT_FLOOR` (the market dissolves).
5. **OSCILLATION** — role churn (enter/exit transitions per capita) ≥ `CHURN_LIMIT` AND no stable
   rolling split (cultivator share never settles within a band over the final window).
6. **MONEY FAILURE DESPITE PRODUCTION** — `SelfProduced` bread is produced and sold but
   `current_money_good() != SALT` at horizon.
7. **SUCCESS** — none of the above AND all seven §SUCCESS criteria hold (material entry, provenance
   clean, SALT promoted, material buying, a living non-cultivating buying cohort, material WOOD
   exchange, a stable two-cohort split).

The verdict test computes this order and prints the verdict + the deciding metrics; it **does not assert
SUCCESS**. Numeric thresholds (`MATERIAL_ENTRY_FLOOR`, `COMMUNE_SHARE`, `CHURN_LIMIT`,
`MATERIAL_BOUGHT_FLOOR`) are pre-stated in §7, not fitted post hoc. The report §-update states the
verdict plainly (e.g. "cultivation participation endogenizes cleanly" vs "the pinned lineage was
load-bearing").

## 3. What gets built

### 3.1 The gate (additive, default-off, canonicalized ON-only)

- A new `ChainConfig` flag `endogenous_cultivation_entry: bool` (default `false`), with an
  `endogenous_cultivation_entry_active(&self)` helper (mirroring `emergency_self_provision_active`).
- Canonicalized **ON-only** with the **next free distinct digest tag** (7 unless master has since
  advanced the tag scheme — the implementer reads the current max; tags 2–6 are used by S21d/e/f/h), so a
  flag-off chain is byte-identical to the current stream and every existing golden is unaffected.
- **Off ⇒ behavior is exactly today's** (lineage-only cultivation). The flag is the *only* behavioral
  change, verified by a `canonical_bytes`-split / revert-to-identical test (the S21h pattern).

### 3.2 The eligibility relaxation (the one engine change)

Today, household-barter cultivation entry is privileged to the spatial **lineage** by the buy/sell-split
eligibility branch in the own-labor subsistence phase (`run_own_labor_subsistence` / the cultivation
pressure gate, settlement.rs ~9275–9335). The current predicate is:

```rust
let eligible = if cultivation_sells_surplus_active {   // buy/sell-split (S16/S21f) on
    colonist.household.is_some() && spatial_active      // → lineage only
} else {
    colonist.household.is_none() || spatial_active
};
```

Because S21f/S21h compose with `cultivation_sells_surplus_active()` ON, non-lineage agents are excluded
on the first branch — so the relaxation must be pinned **exactly** as a new top branch of the
`spatial_member` computation, while **preserving the existing vocation filter** (Codex P1: the override
relaxes only the *household/spatial* membership, NOT the `Consumer|Gatherer|Unassigned` restriction — an
active Miller/Baker/etc. must still be excluded from cultivation):

```rust
let spatial_member = if endogenous_cultivation_entry_active {
    spatial_active                                      // ANY spatial colonist, regardless of household
} else if cultivation_sells_surplus_active {
    colonist.household.is_some() && spatial_active
} else {
    colonist.household.is_none() || spatial_active
};
let eligible = spatial_member
    && matches!(colonist.vocation, Vocation::Consumer | Vocation::Gatherer | Vocation::Unassigned);
```

Otherwise S22a either lands vacuous (an unrelaxed gate → no non-lineage entry) or admits specialized
producers into cultivation (a dropped vocation filter).

Under that override a non-lineage spatial colonist that sustains hunger ≥ `cultivate_hunger_in` for
`cultivate_patience` ticks enters cultivation by the **same existing pressure/patience hysteresis** and
exits by the same `cultivate_hunger_out`/pressure-decay rule. The **behavioral contract**:

- The relaxation is **only** the eligibility set — the pressure counter, patience, consume draw, and
  exit hysteresis are the **existing** S15/S21f fields, unchanged (no new thresholds invented).
- It does **not** route through `run_role_choice` and does **not** relax the money gate for specialized
  Miller/Baker production (those stay exactly as today).
- The opportunity cost is **structural and already enforced** — cultivating consumes the colonist's
  one-task-per-tick, so a cultivating tick cannot also gather WOOD or buy; no cardinal return is
  computed.
- Vocation **labels need not change** (a non-lineage Consumer that cultivates stays `Vocation::Consumer`
  with `cultivating = true`); S22a endogenizes *participation*, not the vocation enum. (If the
  implementer finds a vocation change is unavoidable for some phase to fire, it must be inside the gate
  and is disclosed — but the preferred design is steering-flag only, to avoid vocation churn in the
  digest and to keep the change minimal.)
- **Conservation + no role-choice recursion:** the relaxation must not create phantom
  production/consumption and must not re-trigger `run_role_choice` mid-tick; conservation holds every
  tick (a hard test guard).

### 3.3 Diagnostics (runtime-only, never digested)

Add per-run, runtime-only accessors (the S21h diagnostic pattern — outside `canonical_bytes`):
- **cultivation entrants by class** — count of distinct lineage vs **non-lineage** colonists that ever
  entered cultivation.
- **rolling cultivator share** — the fraction of living colonists cultivating, sampled over a rolling
  window (to detect commune collapse vs a stable split vs oscillation).
- **rolling non-cultivating material-buyer count** — living non-lineage colonists that are NOT
  cultivating AND have bought food materially, over the same rolling window (Codex P2: distinguishes a
  genuine division-of-labor split from a commune where non-cultivators are alive but not buying — the
  latter is not a success).
- **role churn** — count of cultivation enter/exit transitions per colonist (to detect oscillation).
- **bread sold by entrant class (production-time provenance)** — `SelfProduced` bread sold for SALT,
  split lineage vs non-lineage by **the producer's class recorded at PRODUCTION time** (the entrant who
  cultivated the grain), NOT by the seller's `cultivating` state at trade time (Codex P2: cultivated
  bread may sell a later tick when `cultivating` is already false, so trade-time classification would
  misattribute it).
- **bought food after promotion** — the existing `bought` channel post-promotion (the demand-side
  persistence check).

These feed the §2 classification and the report.

### 3.4 The scenario

`frontier_endogenous_cultivation` deriving from `frontier_emergency_provision` (S21h): identical except
`endogenous_cultivation_entry = true`. Keeps S20 + S21a/b/c money machinery, the emergency floor, the
grain commons, the WOOD-poor topology, and mortality on. (This is the right base because S21i showed the
pinned-lineage version coexists with mortality; S22a asks whether *relaxing the producer identity* keeps
that.)

## 4. The decision rule (hunger-pressure hysteresis, reused — not profit, not tuned)

- **Entry:** a spatial colonist whose hunger stays ≥ `cultivate_hunger_in` for `cultivate_patience`
  consecutive ticks (the existing pressure streak) enters cultivation — consuming its one-task-per-tick
  (structural opportunity cost). **The gate is hunger/pressure only** — there is NO new held/bought-food
  protection predicate (Codex P1): bought food suppresses entry *indirectly* on the next tick by
  lowering hunger through the consumption readback (the existing seam), so no new stock/acquisition gate
  is introduced.
- **Exit:** hunger falls below `cultivate_hunger_out` / the pressure decays (the existing exit
  hysteresis); equivalently, recent market food acquisition lowers hunger below the band next tick.
- **Anti-oscillation:** the patience streak + the in/out hysteresis band are what prevent churn; a
  dedicated **no-hysteresis control** (§5) is expected to *exhibit* oscillation, proving the hysteresis
  is load-bearing.
- **Anti-tuning discipline:** S22a **reuses the existing S15/S21f/S21h threshold values** as shipped;
  it does **not** introduce new thresholds and does **not** hand-tune them until "just enough" agents
  cultivate. The robustness mini-sweep (§5) sweeps the *existing* thresholds and grain flow and reports
  the regime map; commune/pinned/failure outcomes in the sweep are findings, not bugs.

## 5. Controls (classify, never tune)

- **Pinned-topology control** = the S21h `frontier_emergency_provision` (gate off) — must still succeed
  (the S21h result), establishing the baseline.
- **Endogenous-entry ON** = `frontier_endogenous_cultivation` — the treatment; classified vs the pinned
  baseline.
- **No-hysteresis control** — pinned via the **existing fields**: `cultivate_patience = 1` and
  `cultivate_hunger_out = cultivate_hunger_in − 1` (the engine validates `out < in`, so the literal
  `out = in` is unrealizable — disclosed). *Pre-named expectation was oscillation;* **landed result
  (control finding):** it does NOT create a distinct failure regime — the headline already churns far
  above `CHURN_LIMIT`, so removing most hysteresis leaves aggregate stability intact while per-agent
  churn stays high in both. The hysteresis is **not load-bearing** for aggregate stability here.
- **No-emergency-floor control** — endogenous entry with the S21h emergency floor off. *Pre-named
  expectation was an S21g-like cull;* **landed result (control finding):** it does NOT reproduce the
  cull — relaxing eligibility makes cultivation itself a survival path, so the emergency floor is no
  longer the sole demand-side bridge.
- **Money-machinery-off controls** — endogenous entry with two-layer saleability / two-lane clearing
  off — money should fail/degrade, proving the money machinery is still load-bearing (not that entry
  alone makes money).
- **Low / no grain-flow control** — endogenous entry with the grain node starved — cultivation entry
  without real food input must **not** fake success (provenance + buying must collapse).

## 6. Determinism & goldens (the safety argument)

The change is one additive, default-off gate (the next free ON-only digest tag) plus runtime-only
diagnostics. Off,
every chain is byte-identical to the current stream, so **all five pinned goldens are byte-identical**
(`lineages()`@300/@800, `frontier()`@300, `frontier_spatial_households()`@300, `viable()`@60) — asserted
in the new suite (`goldens_unchanged`). The new scenario's own digest is new (it gates a real behavior
change). Vocation is serialized in `canonical_bytes`, so the design keeps cultivation entry a
**steering-flag** change (no vocation churn) to keep the digest clean; any unavoidable vocation change is
inside the gate and disclosed. Conservation holds every tick (hard test guard); the relaxation must not
re-trigger `run_role_choice` recursion.

## 7. Acceptance criteria (independent verification)

- New suite `sim/tests/endogenous_cultivation_entry.rs`: classifies the treatment scenario via the §2
  **ordered classifier** across `SEEDS`, with the diagnostics printed; the verdict is whatever the data
  shows (the test does **not** assert SUCCESS — it asserts the classification is computed and the guards
  hold).
- **Pre-stated numeric thresholds (a priori, not fitted post hoc):**
  - `MATERIAL_ENTRY_FLOOR = 4` distinct non-lineage cultivation entrants (below ⇒ PINNED-LINEAGE
    NECESSITY); and at least **2** non-lineage entrants must sell `SelfProduced` bread / contribute
    post-promotion food for SUCCESS.
  - `MATERIAL_BOUGHT_FLOOR` = the existing constant (reused) — post-promotion `bought` below it (with
    high cultivator share) ⇒ COMMUNE COLLAPSE.
  - `COMMUNE_SHARE = 0.75` rolling cultivator share (≥ ⇒ commune side of the commune test).
  - `CHURN_LIMIT = 8` enter/exit cultivation transitions per ever-cultivating non-lineage colonist over
    the 1600-tick run, AND no settled cultivator-share band in the final window ⇒ OSCILLATION
    (**predeclared a priori, NOT fitted to the control** — Codex P1: tuning the classifier against the
    control after seeing data is forbidden). The no-hysteresis control is *expected* to cross it; if it
    does not, that is reported as a control finding, not a reason to adjust the constant.
  - DEMAND-SIDE COLLAPSE = living non-lineage ≈ 0 before the promotion tick (reuse the S21g/S21h
    survivor metric).
- All six controls (§5) are tests; each is classified and its result reported (whether or not it matches
  the pre-named expectation). Landed: pinned succeeds; money-off fails; grain-starved doesn't fake
  success (CommuneCollapse); mortality-off succeeds; **two control findings** — no-hysteresis creates no
  distinct failure regime (the hysteresis is not load-bearing for aggregate stability), and
  no-emergency-floor does not reproduce the S21g cull (cultivation entry is itself a survival path).
- Conservation holds every tick on every run; `bread_minted_max == 0`; provenance clean-or-disqualified;
  no extinction masquerading as a regime (hard `!extinct` guard, the S21i lesson).
- Robustness mini-sweep over the existing pressure thresholds + grain flow, classified, no tuning to
  pass; the regime map is reported.
- Workspace: all tests pass; **all existing goldens byte-identical**; `cargo fmt --check` and
  `cargo clippy --workspace --all-targets -- -D warnings` clean.

## 8. Resolved decisions (Codex spec-review)

1. **Steering-flag, no `Vocation` mutation.** Cultivation entry is a pure `cultivating`-flag change.
   Grain task assignment and deposit attribution already key off `colonist.cultivating`, and own-use
   cultivation converts any cultivating agent's grain. The **only** change required is overriding the
   S16 buy/sell-split eligibility branch that restricts cultivation to `household.is_some() &&
   spatial_active` (pinned exactly in §3.2). No vocation churn → digest stays clean.
2. **`MATERIAL_ENTRY_FLOOR = 4` distinct non-lineage entrants**, and at least **2** non-lineage entrants
   must sell `SelfProduced` bread or contribute to post-promotion food supply. Four is non-token without
   requiring the whole 18-role demand side to become cultivators.
3. **Headline on `frontier_emergency_provision` (mortality on)** + a **mortality-off sanity variant**
   (diagnostic only — proves the entry seam works when the positive check is removed).
4. **Reuse the existing hysteresis; no cooldown in S22a.** A heterogeneous pool that oscillates is the
   OSCILLATION finding; a cooldown would be S22b, not a rescue knob.

## 9. Scope boundary (explicitly deferred to S22b+)

- Profit-driven Consumer/Gatherer drift (opportunity cost as a cardinal return comparison).
- Miller/Baker (specialized producer) entry before money / relaxing the money gate.
- A global "pick the best role among all roles" optimizer.
- Endogenizing the two-lane clearing institution and the SALT direct-use anchor.
S22a endogenizes **cultivation participation**, not the full vocation topology — the report must say so.

## 10. Risks (and how the spec pre-empts them)

- **Overclaim trap.** Naming this "endogenize vocation topology" would overstate it. The spec, commit,
  and report all say **cultivation participation** and keep the deferred list (§9) visible.
- **Tuning trap (the worst).** Adjusting thresholds until "just enough" agents cultivate makes the
  result worthless. Mitigation: reuse the shipped S15/S21h thresholds, sweep them, and accept
  commune/pinned/failure as real findings; the verdict test never asserts SUCCESS.
- **Engine-correctness trap.** A mid-tick vocation/eligibility change that re-triggers `run_role_choice`
  or creates phantom production breaks conservation. Mitigation: steering-flag design, hard conservation
  + `!extinct` + provenance guards on every cell, and a no-recursion check.

## 11. Pipeline

Codex spec-review (settle §8) → SPEC-READY → rb-lite `claude,codex` → independent verification
(workspace + all goldens byte-identical + the new suite + the classification run) → Codex
review-of-results → merge + report-update + memory + pin.
