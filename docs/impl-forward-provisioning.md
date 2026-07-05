# impl-59 — P1.5: The Forward-Provisioning Persistence Probe (can anticipated need make renewal chosen before hunger returns?)

Status (spec): **SPEC-READY** (Codex xhigh spec-review round 1: 4 P1 [engine-exact cap-before-draws
recurrence + roster pool; the real threshold-survival ordinal leisure guard replacing a false
outranks-by-construction claim; the final-fate map surviving the early-return paths with
matched-elsewhere override; nulls as classifier predicates, not guards] + 3 P2 + 1 P3 folded in;
round 2: all findings RESOLVED, no new P0/P1, polish folded). P1.5 of the replan
(`docs/review-and-replan-2026-07.md`), the follow-on the C1R review-of-results named. Base: branch
**`feat/share-tenancy-impl-rb` @ `fac230b`** (C1R landed + complexity-reduced). Flag
**`share_forward_provisioning`** (bool, composes on `share_tenancy` — NOT a fourth `ShareTenancyMode`:
the mode axis is acceptance semantics and is digested; the forward gate is an orthogonal
candidacy-horizon knob, per the wage/share `bool + mode` precedent). Digest **tag 24** as its own
ON-only block (code-verified free; the C3 paper reservation has no code footprint) so every
share-ON/forward-OFF golden — including the landed C1R goldens — stays byte-identical.

Falsifiable bar (headline): with the worker's contract decision extended from the **instantaneous**
outside-option forecast to a **term-horizon** forecast of its own deterministic need, do voluntary share
contracts finally **renew** and **stand in the final window** — the base's first persistent economic
relationship — or does renewal still decline (with the new fate telemetry saying exactly why)?

## 0. One-paragraph summary

C1R proved the no-advance share contract clears voluntarily but never renews: the binding margin is the
worker's own outside-option gate, which evaluates **this tick only** — a share-fed worker is below the
hunger threshold at expiry, `forecast_commons_sufficiency` reports covered, and the worker exits the
pool until hunger regrows (episodic labor by construction). But this engine's need law is **closed-form
deterministic** (`hunger(t+T) = min(h + 2T, 12)`, `need.rs:79–138`; consumption seams equally pure), so
"will my held bread plus the commons cover my need over the **next term**?" is a **pure function of
already-digested state** — the exact construction the existing forecast uses, projected T ticks forward.
P1.5 makes that the worker's question. Nothing is smuggled: no new want kind, no contract preference
(swept: none exists), no realized-return history (the S22c/S22f windows are explicitly off-limits —
they'd smuggle experience into what must be a forecast). Praxeologically this is provisioning for the
future — the same root as saving — entering the labor relation: a worker renews *because its need will
return*, before eligibility lapses. Slice A ships the telemetry the C1R review-of-results asked for
(per-cause renewal-fate + commons-substitution counters), which alone makes C1R's no-renewal airtight;
Slice B adds the forward gate; the suite pre-names `StandingTenancyForms`-family outcomes and the honest
nulls. Classify-not-tune throughout.

## 1. Base facts (verified on the branch)

1. **The need law is pure and closed-form.** `hunger_deplete = 2`/tick, `hunger_per_food = 3`,
   `need_max = 12` (`need.rs:79–91`); `advance` is integer/deterministic (`need.rs:110–138`);
   `food_needed_to_reach_hunger` (settlement.rs:25759) is the pure per-tick demand helper. Held-bread
   depletion is deterministic too: the emergency seam **eats held bread first**, commons covers only the
   residue (settlement.rs:15235–15248).
2. **The current gate is instantaneous.** `share_worker_outside_option_fails` →
   `!forecast_commons_sufficiency` (:16010), which snapshots this-tick hunger, held bread, and one
   commons step (`stock + regen`, :16677) over the current claimant pool. The acceptance evaluator
   (:16040–16090) fires only on a **Now**-horizon `Good(BREAD)` want.
3. **The scale cannot express future bread wants.** Bread wants are hardcoded `Now` (scale.rs:257);
   `Later` wants are money-only (scale.rs:294/335); the provisioning bitmap **skips** non-money goods at
   future horizons (`agio.rs:466–468`). So the forward evaluation must be a **new pure evaluator beside
   the bitmap** — a disclosed engine limitation (a future-goods want layer is named follow-on work),
   not a smuggle: the evaluator derives entirely from the need law + digested stocks.
4. **Renewal-decline causes are decidable at pinned sites** (the Slice-A counter map):
   (a) *fed-out* vs (b) *base-ineligible* split at the pool test (:16239) by calling
   `share_worker_base_eligible`; (c) *owner/node no longer a candidate* at :16242; (d) *incumbent
   bread-declined* at :16248; (e) *matched a different plot* is **not currently decidable** — the
   general pass doesn't know which workers carried renewal hints, so Slice A threads hint-membership
   (a fell-through set) into it.
5. **Commons draws have no per-agent surface.** Only the acquisition ledger's aggregate channels exist;
   per-agent attribution happens once, inside `fulfill_subsistence_commons_requests`
   (:15341–15343, `FoodChannel::Commons`). Slice A adds a per-agent Commons-credited accessor
   (mirroring the existing `bought_credited_by_agent`) so the suite can compare worker-cohort commons
   draws across cells — the substitution measurement the RoR asked for.
6. **Tag 24 is free in code**; runtime-only counters stay out of the digest (the tag-22/23 exclusion
   discipline, :24240–24244).

## 2. The central question and pre-named outcomes

**Central question.** When the worker's contract decision — eligibility *and* acceptance, for new
contracts and renewals symmetrically — evaluates its **anticipated unmet need over the coming term**
(a pure forecast from the deterministic need law, held-bread depletion, and the projected commons
recurrence), do voluntary share contracts **renew from fresh state** and **persist into the final
window** across `SEEDS=[3,7,11,19,23]`, with all C1R guards intact — and does persistence then move the
lift needle, or does substitution still absorb it?

**Ordered verdict enum** (first-match; the forward cell is voluntary-only, so no scaffold arm):

```
Preconditions (disqualifying):
  BaseUnviable            — the C1R base fails to reproduce (forward-off Voluntary cell must land
                            ShareClearsButNoLift with the C1R traces)
  ConservationBroken      — goods / commons / money / provenance conservation failed a tick
  RegistryBroken          — plot-registry / S23d owner-identity invariants violated
Outcome ladder:
  ForwardGateInert        — the forward gate never extends eligibility beyond the instantaneous gate
                            (forward_only_eligibility == 0: the term forecast always says covered —
                            e.g. the commons projection absorbs everyone at this φ)
  RenewalStillDeclined    — forward-eligibility fires but voluntary renewals stay ≈ 0; the fate
                            counters name the new binding margin (expect owner-side at-cap supply, (c))
  StandingTenancyNoLift   — renewals ≥ MIN_RENEWALS and open contracts persist in the final window,
                            but no survival/consumption lift over NoContract (substitution still
                            absorbs the income — persistence proven, welfare unchanged)
  StandingTenancyLifts    — persistence AND a material lift (the full positive)
```

Every rung below the preconditions is an honest, publishable finding; none is tuned toward.
`StandingTenancyNoLift` and `StandingTenancyLifts` both establish **the base's first standing economic
relationship** — the difference is whether it also raises welfare, which the substitution telemetry
(Slice A) will explain either way.

## 3. Mechanism

### 3.1 Slice A — the telemetry (pure, no digest changes; C1R's RoR ask)

- **Per-cause renewal-fate counters** — implemented as **one final-fate map, not site-local
  increments** (spec-review P1: `run_share_tenancy_phase` early-returns when the worker pool or owner
  candidate set is empty, :15826–15837 — *before* the classification sites, which would silently drop
  exactly the fed-out and owner-supply cases; and a preliminary (c)/(d) can be superseded when the
  worker matches elsewhere in the general pass). Shape: at phase start, every renewal hint enters a
  fate map `hint → PendingFate`; fates are assigned wherever decidable — `renewal_fed_out` vs
  `renewal_base_ineligible` split via `share_worker_base_eligible` at the pool test,
  `renewal_owner_not_candidate` (:16242), `renewal_bread_declined` (:16248), with
  `renewal_matched_elsewhere` **overriding** a preliminary (c)/(d) if the worker matches a different
  plot in the general pass — and the map is finalized **exactly once per hint not same-plot-renewed**,
  including on the early-return paths. Plain runtime fields beside the existing share counters, zeroed
  at both init sites, surfaced through `ShareTenancyStats`, **out of `canonical_bytes`** per the
  tag-22/23 discipline. Internal-consistency guard: fates sum to hints minus same-plot renewals.
- **Per-agent commons attribution:** a `commons_credited_by_agent` accessor on the acquisition ledger
  (mirror of `bought_credited_by_agent`, :7769), populated at the existing single credit site (:15341).
  **The comparison cohort is predeclared** (spec-review P2, survivorship/selection guard): the cohort is
  the **Voluntary cell's ever-worker id set**, applied as a *counterfactual cohort* to the matched
  same-seed `NoContract` cell (those ids exist there — same roster, same seed — they simply never
  contract); ids dead-by-window-end in either cell are reported alongside, not silently dropped. The
  suite derives: cohort commons draw per cell, the Voluntary-vs-NoContract cohort delta, and
  `substitution_share` = (cohort commons-draw reduction) / (cohort share income) — the direct test of
  the RoR's crowding-out reading.
- **Payoff before Slice B exists:** re-running the C1R Voluntary cell with these counters must show the
  no-renewal decomposition (expected: cause (a) fed-out dominating) — C1R's finding made airtight, as
  the RoR requested.

### 3.2 Slice B — the forward gate (the probe)

A new pure evaluator, `forecast_term_need_unmet(worker, bread, T) -> u64`, computed **only** from
already-digested state (the anti-smuggling spine):

- Project the worker's hunger forward T econ ticks by the closed-form law, applying per-tick demand
  `food_needed_to_reach_hunger(h_t, deplete, per_food, threshold − 1)`;
- deplete **held bread first** (the emergency-seam order, :15235–15237), then draw the projected
  commons — the same rationing math as today's forecast, iterated **cap-before-draws exactly as the
  engine does** (spec-review P1; :15291/:16677): `available_k = min(stock_k + regen, cap);
  stock_{k+1} = available_k − draws_k`;
- with the claimant pool held constant, where "the pool" is the **roster of currently
  emergency-eligible non-lineage `Consumer|Gatherer` colonists** (spec-review P1: the *roster*, not the
  current positive-request set — a low-hunger renewal worker must not vanish from its own forecast;
  each roster member's projected demand follows the same closed-form law);
- return the cumulative bread need left uncovered over the term. **Retained grain is deliberately
  outside the forecast** (spec-review P3): the outside option is held *bread* + commons; unconverted
  grain is real property but not yet food, and modeling its conversion would entangle the forecast
  with the labor decision it feeds.

With `share_forward_provisioning` on, the predicate replaces the instantaneous question at **both**
worker seams, **symmetrically for new contracts and renewals** (a renewal-only ratchet would smuggle
stickiness):

- **Eligibility** (:16010): the outside option fails iff `forecast_term_need_unmet(worker, bread,
  share_term) > 0` — a worker whose need *will* outrun its bread and the commons within the term is
  eligible **now**, before hunger returns.
- **Acceptance** (:16074–16090): the existing Now-arm is kept; a **forward arm** is added — accept iff
  `expected_share > 0`, `forecast_term_need_unmet > 0`, **and a real ordinal leisure guard passes**
  (spec-review P1 correction: "survival need outranks Now-leisure by construction" is FALSE in general —
  scale generation can rank high-rest Leisure above threshold bread). The guard is pure and
  scale-derived: compute the rank at which a **synthetic threshold-hunger Now `Good(bread)` want**
  would sit in the worker's *generated* scale (the same ladder construction `push_present_ladder`
  uses for a worker whose hunger stands at the threshold), and accept only if that rank is **above**
  (index below) the worker's current first-unsatisfied Now-Leisure rank. Disclosed precisely: this is a
  **threshold-survival ordinal guard** — the anticipated need is ranked as the *present* want it will
  become, since the scale cannot carry future bread wants (§1.3) — not a future-bread rank-walk.
  The forecast is of the **no-contract** world (held bread + commons only) — the expected share is
  never counted as cover in the same forecast (no self-justifying contracts).

Everything the evaluator reads is digested (need state, stocks, commons state, `share_term`), so the
gate itself adds **no new digested state**; tag 24 carries exactly one byte (the sub-flag), emitted
ON-only in its own block — share-ON/forward-OFF byte-identity preserved for the landed C1R goldens.

### 3.3 What does NOT change

The owner's cap-waste gate, the split, the escrow-free settlement, death routing, the anti-title
guards, mutual exclusion with wage labor — all untouched. The probe moves one thing: the horizon of the
worker's own question.

## 4. Anti-smuggling guards

1. **Pure forecast, no experience.** The evaluator is a function of the deterministic need law +
   digested stocks + the commons recurrence. The S22c/S22f realized-return machinery is explicitly
   off-limits. No new want kind, no contract preference (swept: none exists in the engine).
2. **Symmetric, not sticky.** The forward horizon applies to new contracts and renewals identically;
   persistence must come from re-choosing under the same gate (the S22f discipline).
3. **No self-justification.** The forecast excludes the prospective contract's own income.
4. **Disclosed approximations:** claimant-pool-held-constant (inherited from the existing forecast);
   the forward arm sits beside the bitmap because the scale cannot express future goods wants (the
   future-want layer is named follow-on work); `N̂` remains the disclosed flow bound.
5. **Not tuned.** The horizon is `share_term` (already pinned + swept: {6, 12, 24}); no new free
   parameter is introduced. `ForwardGateInert` and `RenewalStillDeclined` are first-class.

## 5. Conservation & determinism

No new sources/sinks/transfers — the probe changes decisions, not flows; all C1R conservation guards
(goods, commons rivalry, money invariant, provenance finalizers, `share_stock_drawdown == 0`) carry
unchanged. The forecast is integer-pure and deterministic. **Digest:** tag 24 = ON-only
`{ push(24); push(u8::from(share_forward_provisioning)) }`, **emitted iff
`share_forward_provisioning && share_tenancy_active()`** (spec-review P2 — the composed active-predicate
idiom, :18530–18535: the sub-flag alone, without the share substrate, emits nothing). The fate map is
rebuilt per phase (runtime-only, never crosses a tick); the fate/substitution counters and the
per-agent commons map are runtime-only diagnostics — all excluded per the established discipline.
Off-path: byte-identical to the C1R branch goldens (and, with all civ tags off, to master).

## 6. Slices

- **A — telemetry.** Fate counters (incl. the fell-through set for cause (e)), the per-agent commons
  accessor, `ShareTenancyStats`/suite `line()` extensions; re-run the C1R Voluntary cell and print the
  no-renewal decomposition. *DoD: counters populated, digest untouched (byte-identity regression), the
  C1R verdict lines unchanged apart from the appended fields.*
- **B — the forward gate.** `forecast_term_need_unmet` + the two seam extensions + the tag-24 block +
  config/runtime plumbing (the `share_tenancy` template sites). *DoD: forward-off byte-identical;
  forward-on deterministic; `forward_only_eligibility` counter live (a worker eligible under the term
  forecast but not the instantaneous one).*
- **C — acceptance suite.** A `ForwardProvisioning` scenario cell (suite-level enum — free) beside the
  C1R cells; the §2 verdict ladder; sweeps (φ, share_bps, `share_term` as the horizon); lift re-test
  with the substitution metrics. *DoD: suite green; forward-off cells reproduce C1R byte-for-byte;
  verdicts printed, never asserted.*

## 7. Acceptance suite (`sim/tests/share_tenancy.rs`, extended)

- **Predeclared thresholds (swept):** `MIN_RENEWALS`, `MIN_FINAL_OPEN_CONTRACTS`, the existing
  lift/`θ` bars for the lift re-test. (`forward_only_eligibility` has **no threshold const** — the
  classifier's zero/nonzero routing below is authoritative; it is reported, never barred.)
- **Cells:** `ForwardProvisioning` (headline, φ=marginal) beside the unchanged C1R cells
  (`Voluntary` forward-off must reproduce `ShareClearsButNoLift` — the regression control), `NoContract`
  (for the substitution deltas), the φ/share/term sweeps on the forward cell.
- **Classifier predicates, NOT hard guards** (spec-review P1: asserting them would assert away the
  honest nulls): `forward_only_eligibility == 0` **routes to `ForwardGateInert`**;
  `forward_only_eligibility > 0 && renewals < MIN_RENEWALS` **routes to `RenewalStillDeclined`**; the
  forward-traceable-renewal counterfactual (a renewal by a worker the instantaneous gate would have
  excluded) is a **reported trace** that qualifies the `StandingTenancy*` rungs, never an assertion.
- **Hard guards (only invariants):** all C1R guards + the fate map's internal consistency (fates sum
  to hints minus same-plot renewals).
- **Matched-volume diagnostic** (spec-review P2, the supply confound): report per-window new-contract
  volume and at-cap candidate counts for the forward cell vs the forward-off Voluntary cell, so
  "the gate works but its own extra volume exhausts at-cap supply" is distinguishable from "renewal
  still not chosen" — read together with the `renewal_owner_not_candidate` fate share.
- **`goldens_unchanged` + `canonical_bytes` split test** for tag 24 (forward-on splits; forward-off
  byte-identical to the branch goldens).

Build/verify: `cargo test -p sim --test share_tenancy -- --nocapture`, full workspace, fmt, clippy
`-D warnings`; the wage_labor + S23c/d/e suites stay green.

## 8. Risks & open questions

1. **`ForwardGateInert` is live.** At marginal φ the commons projection may cover everyone over a
   12-tick horizon once population settles — then the forecast never extends eligibility and the
   finding is that the commons, not myopia, bounds the institution at this scarcity (readable via
   `forward_only_eligibility = 0` + the substitution metrics; the φ/term sweeps probe it).
2. **The owner side may become the new binding margin.** Standing renewals need standing at-cap supply;
   cause-(c) counters will show whether owner candidacy, not worker myopia, now blocks persistence —
   itself a finding (the below-cap ordinal comparison becomes the next lever).
3. **Projection fidelity.** The claimant-pool-constant approximation degrades over longer horizons;
   it is the same approximation the instantaneous gate already makes, disclosed, and the term sweep
   exposes its sensitivity.
4. **Lift may still be zero.** Substitution can absorb persistent income exactly as it absorbed
   episodic income — `StandingTenancyNoLift` is pre-named and distinct, and the substitution telemetry
   quantifies it.

## 9. Falsifiable-bar summary

Extending the share worker's one question from "does the commons cover me *this tick*?" to "will my
bread and the commons cover my **deterministic need over the coming term**?" — a pure, integer,
digested-state forecast with no new want, no experience, no self-justification — should show whether
**anticipated need is sufficient to make renewal chosen before hunger returns**: voluntary renewals and
final-window standing contracts (`StandingTenancyForms`-family), with the lift question answered
separately by the substitution telemetry; or the honest alternatives — `ForwardGateInert` (the commons
projection covers the term: scarcity, not myopia, bounds the institution) and `RenewalStillDeclined`
(the margin moves to the owner's at-cap supply) — each named before the run, each first-class, and each
now *readable* thanks to the per-cause fate counters C1R's review asked for.
