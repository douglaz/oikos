# Implementation Spec: per-agent intertemporal capital choice / originary interest (S10)

> Codex's evaluation after S9: money emergence is settled; the **least authentic major
> mechanism left is capital formation**. S7's build decision is a *settlement-level*
> heuristic — one global stage choice (mill vs oven by capacity bottleneck) + a scalar
> `margin × capital_payback_cycles > cost` test + first-eligible-fed-builder assignment.
> That is "build if the math beats cost", not Böhm-Bawerk/Mises capital: an *individual*
> actor choosing to sacrifice present goods/labor for a future, more-roundabout return,
> on its own value scale, with its own time preference. This milestone moves the capital
> decision from the planner into the colonist.

## The decisive design fact: this engine has NO cardinal discount rate (by design)

Time preference in oikos is **purely ordinal**: `CultureParams::time_preference_bps`
(per-agent, per-lineage, heritable — `life/src/culture.rs`) is read in exactly one place,
`push_future_ladder` (`life/src/scale.rs:235-263`), where it repositions a colonist's
**savings (`Later`) wants** on its value scale (more patient → more savings units, ranked
higher). There is deliberately **no discount-rate multiplier, no cardinal present-good
premium** anywhere ("no discount rate and no scalar utility anywhere", `culture.rs:5`;
`scale.rs:24-25, 47`). Future proceeds are never discounted by a magnitude; the appraisal
chain (`recipe_adoption_pays_for_money`, `appraise_project_bundle_for_money`, `agio.rs`)
compares raw quantities against an **ordinal** "does this provision a future-money want
without breaking a higher-ranked want" test; horizon is only a due-by-tick gate.

**Therefore S10 expresses originary interest ORDINALLY, not as a discount knob** — which
is the *more* Misesian framing (originary interest is the universal preference of present
over future goods, not a given rate). The per-agent build decision reads each colonist's
**own value scale** (already shaped by its `time_preference_bps`): a patient colonist
(savings wants ranked high) is willing to forgo present goods/labor now for the tool's
future proceeds; a present-biased one is not. The time-preference→capital-formation
response then *falls out* of the ordinal appraisal — no cardinal agio is invented.

## Purpose & the honest bar

Replace S7's settlement-level capital heuristic with a **per-agent ordinal intertemporal
appraisal**: each eligible colonist decides, on its own value scale, whether to commit
present WOOD + labor (a multi-tick sacrifice) to build a durable tool whose future
proceeds provision its own future (savings) wants — **no global stage choice, no
first-eligible-builder assignment, no fixed `capital_payback_cycles` scalar**. Capital
formation must then track each colonist's **time preference**: patient colonists invest
in roundabout production, present-biased ones do not.

NOT a cardinal discount rate (the engine has none by design — express it ordinally), NOT
firms/credit, NOT a change to the build's mechanical substrate (own-WOOD funding,
multi-tick gestation, own-stock delivery already per-builder), NOT a change to existing
emergence/market behavior (S5–S9 scenarios + all goldens byte-identical — additive +
gated; the new mode default off).

## Verified Base Facts (oikos @ `29d2d28`)

1. **S7's build is an explicitly-flagged planner follow-on.** `run_capital_formation`
   (`sim/src/settlement.rs:6799`): Part 1 advances in-flight builds (per-builder, fine);
   **Part 2 is the planner** — a single global stage choice by capacity bottleneck
   (`:6933-6974`), one-build-in-flight pacing (`:6986`), and a first-eligible-fed-builder
   loop (`:6989-7064`). Its own comment: "a settlement-level heuristic, NOT a per-colonist
   ordinal-scale appraisal ... A fully individual ordinal appraisal is a follow-on"
   (`:6887-6893`).
2. **The scalar appraisal + fixed knob to replace.** `capital_build_surplus` (`:9267-9297`):
   `margin_per_run × payback_cycles − (wood_build_cost + labor_opp_cost + first_input) > 0`,
   uniform for all colonists; `capital_payback_cycles` is a fixed `ChainConfig` knob (`:902`,
   8/16/32). "Discounted" in its doc means net-of-cost, **not** time-discounted.
3. **The mechanical substrate is already per-builder and conserved — reuse it.**
   `start_project` consumes the builder's own WOOD up front (booked `consumed_as_input`),
   `advance_project` adds the builder's own labor over `tool_build_labor` ticks (a real
   multi-tick gestation/waiting period), `complete_project_if_ready` delivers the tool to
   the builder's own stock (`econ/src/project.rs:213-345`, used at `settlement.rs:6832-6885,
   7031-7060`). Only the *decision* (stage + go/no-go) is centralized.
4. **Time preference is per-agent, ordinal, and already plumbed.** `colonist.culture.time_preference_bps`
   (`settlement.rs:3123`, set per-colonist `:3869`, heritable `culture.rs:56-95`) shapes the
   savings ladder (`scale.rs:235-263`). But **the build/appraisal path reads `agent.scale`,
   not `culture` directly** — so a per-agent build decision reading the colonist's own scale
   automatically inherits its time preference, no new field needed.
5. **An ordinal per-agent project appraisal exists, but it is ONE-PERIOD.**
   `recipe_adoption_pays_for_money` (`:9399-9478`) + `appraise_project_bundle_for_money`
   (`econ/src/bundle.rs:50-95`) test, on the agent's own scale, whether a project newly
   provisions a future-money savings want without breaking a higher want
   (`soonest_savings_horizon` `:9496`). But it is **one receivable, one payable, one
   `project_period`**, and savings wants are emitted at a **fixed `Later(4)`**
   (`scale.rs:56, 235`) — so a tool that gestates several ticks then pays a *stream* would
   miss the only savings horizon. This precedent must be **generalized**, not merely reused
   (see "The capital-tool appraisal" below).
6. **Ordinal ≠ no money calculation: a DERIVED present-value bound is allowed.**
   `AgioSchedule::present_value` (`econ/src/agio.rs:76-93`) derives "the most present gold
   the agent would exchange for a future-gold receipt" **from the agent's own scale** (its
   lending quotes, themselves a function of its savings ladder / time preference). An agio
   (present-vs-future spread) thus *emerges* per agent (`AgioQuote`, `agio.rs:9-14`) without
   any config discount. The capital-tool appraisal may use this derived, scale-grounded
   present value to collapse a future proceeds stream — the forbidden move is only a
   *uniform `capital_payback_cycles` / config discount imposed from outside the agent*.
7. **Gating keeps goldens byte-identical.** A new `ChainConfig` flag `per_agent_capital`
   (default false) follows the established pattern (`recurring_motive` `:816`,
   `producible_capital` `:894`; defaults in both ctors `:1007-1022, :1080-1087`;
   `run_capital_formation` already early-returns on its gate `:6804-6809`). The three
   capital scenarios (`frontier_capital`/`frontier_coemergent`/`frontier_coemergent_strong`)
   stay on the heuristic (byte-identical) unless explicitly switched. Determinism: iterate
   `live_colonist_slots` in slot order; any new per-colonist build-intent state →
   `canonical_bytes` (`:8462-8488` is the S7 capital digest block) + a
   `canonical_bytes_include_*` regression.
8. Conservation/digest/accessors as before: `tools_built()`, `acquired_tool_of()`,
   `producer_cash()`, `need_of()`, `whole_system_total(tool)`, `stock_of(i, WOOD)`,
   `living_count()` (`:8030-8053`, `:8303-8329`).

## The capital-tool appraisal (concrete — the milestone's core; Codex-required)

Specify `appraise_capital_tool_bundle_for_money(agent, tool_recipe, build_wood, build_labor,
prices, tick) -> bool` — a per-agent, scale-grounded decision (generalizing
`appraise_project_bundle_for_money`, NOT the scalar `capital_build_surplus`):

- **Present side (the sacrifice, ordinally — operational):** (a) the **WOOD** committed now —
  removed from the agent's own stock (its present use is forgone); (b) the **labor sacrifice**,
  modeled concretely as **forgone Leisure**: the `build_labor` build ticks displace the agent's
  `WantKind::Leisure` want (`agent.rs:52`), which sits at a definite scale rank set by the
  agent's `culture.leisure_weight_bps`. So the present cost is exactly: the WOOD's present use
  + the Leisure want at the agent's leisure rank. **Not** the S7 scalar
  `operating_cost × tool_build_labor` (`:9289`). (Finer modeling — forgone *gather output* in
  those ticks — is a deferred refinement; Leisure displacement is the operational present cost
  for S10.)
- **Future side (the stream, as dated receivables):** the tool yields its recipe's net margin
  over a finite horizon of `H` future runs (a multi-tick stream beginning after the
  `build_labor` gestation), expressed as a **generalized temporal bundle of dated receivables**
  (not one receipt). Where the agent has a lending quote, the agent's OWN
  `AgioSchedule::present_value` (`agio.rs:76`, scale-derived) gives a present-value **bound** —
  used as supporting evidence, **NOT the sole gate** (it is a lending-quote lower bound and is
  liquidity-constrained; a cash-poor, WOOD-rich builder may have no quote yet still value the
  future receipts — Codex). Never a config discount.
- **Acceptance (ordinal — the gate):** the build is taken iff the altered temporal endowment
  (−WOOD now, −Leisure for the build duration, +dated tool receipts later) **newly provisions
  one of the agent's own future-money savings wants while preserving every higher-ranked want**
  — the `bundle_accepts_due` / `preserved_above_target` test (`bundle.rs:97-162`) generalized
  to the dated stream, with the displaced Leisure rank and the committed WOOD's present use
  among the wants the future provision must outrank.
- **Horizon (PINNED — multi-horizon savings wants, with the principled reason):** the existing
  savings ladder emits only `Later(4)` (`scale.rs:56,235`), and `future_capacity_due_by`
  (`agio.rs:463`) counts a receipt only if it is due **by** the want's horizon — so a tool whose
  stream pays after `tick+4` provisions **nothing**, and *even a patient agent would reject*.
  That structural incompatibility IS the principled reason (Codex's "unless S10 fails for a
  principled horizon reason") to **extend the savings ladder to multiple horizons** in the
  per-agent-capital path: emit savings wants at `Later(4), Later(8), …` up to a **patience
  depth set by the agent's own `time_preference_bps`** (reuse the existing `save_units`/patience
  logic — `scale.rs:259`; patient → deeper ladder, present-biased → shallow). Then the tool's
  dated receipts provision the **deeper** wants via the **unchanged** `future_capacity_due_by`
  due-by logic — **no new due-date semantics**. Time preference bites structurally: a patient
  agent has deep savings wants the tool's late receipts can fill (→ builds); a present-biased
  agent has only shallow near wants the late receipts miss (→ declines). **Gate it** to the
  per-agent-capital path (default off) so existing scenarios keep the `Later(4)`-only ladder and
  stay byte-identical. `present_value` (`agio.rs:76`) remains a supporting bound where a lending
  quote exists, not the gate.

Originary interest is then **emergent**: a patient agent's scale ranks the future provision
above the present sacrifice (it builds); a present-biased agent's does not (it doesn't). No
rate is imposed.

## The slices (build in order; each independently testable)

- **S10.1 — the per-agent ordinal build appraisal (gated, the core).** Behind a default-off
  `per_agent_capital` flag, replace Part 2's planner with a per-colonist decision: iterate
  `live_colonist_slots` (deterministic order); for each eligible colonist (the existing
  fed/idle/holds-WOOD/holds-no-tool checks), have **it** run
  `appraise_capital_tool_bundle_for_money` (above) on its **own scale** for each tool it
  could build — committing the present WOOD + labor (modeled as forgone present
  action/leisure, not a scalar) against the dated future tool-proceeds stream. **No global
  stage choice; no first-eligible assignment** — each colonist that its own appraisal accepts
  starts its own build (the per-builder substrate, Base Fact 3); the slot order is only
  iteration order, not selection. Emit a per-tick **decision diagnostic** (candidate, tool,
  accept/reject, the target savings want/rank, rejection reason) so a test can prove earlier
  eligible colonists *declined on their own scales* while a later one accepted (Codex). **Unit
  test:** with the flag on, an individual colonist builds via its own appraisal AND the
  diagnostic shows ≥1 earlier-eligible colonist that declined (not merely "first eligible");
  with the flag off, behavior is byte-identical to S7.
- **S10.2 — the originary-interest response (the falsifiable core).** The decision reads the
  colonist's scale, which `time_preference_bps` shapes — so **patient colonists invest more,
  present-biased ones less**, with no cardinal discount. **Test two ways (Codex):** (a) a
  **controlled microtest** — identical agents/state differing ONLY in `time_preference_bps`:
  the patient one accepts the build, the present-biased one rejects it (strict, clean,
  deterministic, isolates the mechanism); (b) a **live-run aggregate** — a present-biased
  colony forms **materially less / non-more** capital than a patient one (NOT strict
  monotonicity across the whole noisy scenario — full-run formation is lumpy from prices,
  stocks, hunger, discrete thresholds).
- **S10.3 — the flagship scenario + DoD.** Add `frontier_coemergent_strong_originary` (derive
  from `frontier_coemergent_strong` — the most authentic base) with `per_agent_capital = on`.
  **Test:** the clean metric below — capital is built by individuals on their own scales,
  output responds to time preference, the chain still sustains, conserved, deterministic.

## Acceptance Tests (the S10.3 DoD) — `sim/tests/originary_interest.rs`

1. `originary_run_is_deterministic` — byte-identical `(seed, config)`.
2. `capital_decision_is_per_agent_not_planned` — with `per_agent_capital` on, there is **no
   settlement-level stage choice or builder assignment**: via the decision diagnostic, prove
   ≥1 earlier-eligible colonist **declined on its own scale** while a later one accepted (so
   the builder is chosen by its own appraisal, not slot-order-first), and tools are funded
   from the builder's own WOOD/labor.
3. `higher_time_preference_forms_less_capital` — THE clean metric, tested two ways (Codex):
   (a) a **microtest** — two otherwise-identical agents differing only in `time_preference_bps`:
   the patient builds, the present-biased declines (strict, deterministic); (b) a **live
   aggregate** — a present-biased colony forms materially less / non-more capital than a
   patient one over the run (NOT strict per-step monotonicity — full-run formation is lumpy).
4. `capital_still_responds_to_demand` — a colonist builds only when the tool's future
   proceeds justify the present sacrifice on its scale; with demand met / margin gone, no
   one builds (no overinvestment) — now via individual appraisal, not a planner stop.
5. `chain_sustains_under_per_agent_capital` — the flagship scenario still emerges money,
   sustains bread to t1600, builds ≥1 tool by individual choice, conserves every tick.
6. `originary_conserves` — whole-system conservation every tick across the per-agent builds.
7. `goldens_unchanged` — with `per_agent_capital` off, S5–S9 scenarios + the six econ +
   g5a/g5b/coemergence goldens are byte-identical; `producible_capital`/`money_coemergence`/
   `strong_bar_emergence` suites green; new digest knob has a `canonical_bytes_include_*`
   regression; clippy `-D warnings`; fmt `--check`.

Manual: `cargo run -p viewer -- run originary --ticks 1600` (compare tool counts to a
present-biased variant).

## Missing Interactions (track explicitly)

- **Valuing a durable, multi-period tool — specified in "The capital-tool appraisal".** The
  return is a *stream* of recipe proceeds (Böhm-Bawerk roundaboutness), represented as a
  **generalized dated-receivable temporal bundle** provisioned against a **gated multi-horizon
  savings ladder** (depth set by `time_preference_bps`) via the **unchanged**
  `future_capacity_due_by` due-by logic (`agio.rs:463`) — so no new due-date semantics, and
  `present_value` is a supporting bound, not the gate. This is the milestone's intellectual
  core — if the ladder depth doesn't track time preference, time preference won't bite (test 3a
  is the tripwire).
- **No cardinal config discount — but a DERIVED present-value bound is allowed.** Do NOT add a
  discount-rate knob or keep a uniform `capital_payback_cycles`. A scale-grounded present
  value (`agio.rs:76`, from the agent's own lending quotes / savings ladder) is acceptable
  and is how time preference enters.
- **Digest hygiene (Codex):** serialize `per_agent_capital` whenever the capital phase can
  run. In the per-agent mode `capital_payback_cycles` is **inert** — do NOT digest it under
  `per_agent_capital` (it would create false digest splits for a behavior-inert knob); keep
  digesting it only in the legacy-heuristic mode. Any new per-colonist build-intent / decision
  state that steers future ticks goes in `canonical_bytes` with a `canonical_bytes_include_*`
  regression.
- **Don't regress demand-responsiveness / no overinvestment.** The per-agent decision must
  still stop building when the return no longer justifies the sacrifice (the present-good
  premium on the scale is the brake) — verify it does not over- or under-build vs S7.
- **Determinism with per-agent decisions.** Multiple colonists may decide to build the same
  tick; keep slot-ordered iteration and serialize any new per-colonist build-intent state.
- **Heterogeneity is the point, not noise.** Time preference is per-lineage/heritable — the
  spread of who-builds-what should reflect it; assert it (S10.2), don't average it away.

## Handoff Notes

- **Reuse the substrate, relocate the decision.** Keep `start_project`/`advance_project`/
  `complete_project_if_ready` (per-builder, conserved); remove only Part 2's planner
  (`settlement.rs:6933-6974` stage choice, `:6986` single-in-flight gate, the
  `:6989-7064` first-eligible assignment) behind the `per_agent_capital` branch.
- **Express originary interest ordinally** via the colonist's own scale (Base Fact 4) —
  NOT a discount knob; this is the engine's deliberate design (`culture.rs:5`).
- **Gate + flagship:** default off so S5–S9 + goldens are byte-identical; demonstrate in a
  new `frontier_coemergent_strong_originary` scenario. The S7 heuristic stays as the legacy/
  comparison path (like the curated-advance scenarios kept for comparison).
- **Falsifiable bar is the time-preference response** (S10.2/test 3) — if capital formation
  does NOT vary with time preference, the decision isn't really reading the ordinal scale,
  and the milestone has failed its purpose (land that honestly).
- Build S10.1→S10.3 as separate commits with their own tests; `git add` new files.
- **Follow-ons:** entrepreneurial uncertainty + profit/loss (forecast prices, exit);
  re-enabled starvation selection; S(provisioning)-at-scale under emergence; making the
  SALT direct-use itself emergent.
