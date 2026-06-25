# impl-29 — S21f: Endogenous Pre-Money Household Production-for-Barter

Status: DRAFT (pre-Codex-spec-review)
Branch: `feat/household-production-barter`
Base: master @ `6856297` (S21e landed)

## 0. What this milestone is — the authentic supply mechanism

S21e proved (bounded diagnostic) that a finite *seeded* pre-promotion bread supply is sufficient
for SALT to monetize under market-financed survival, then for production to take over. The seed was
a one-time scaffold. **S21f makes that pre-promotion supply ENDOGENOUS:** households *cultivate*
bread by their own labor, eat what they need, and barter the surplus — *before money exists* — so
SALT emerges from barter over genuinely produced surplus. This is the Mengerian/regression-theorem
bootstrap: direct production-for-use + barter of surplus precedes money.

The decisive new bar vs S21e: **the pre-promotion bread sold for SALT must be `SelfProduced`
(cultivated), not `SeededMinted` (seeded).** The acquisition ledger already distinguishes these
channels (`FoodChannel::SelfProduced` is credited by own-use cultivation,
`settlement.rs:9040-9043`), so the endogenous-supply claim is directly provable.

## 1. The key design question for Codex spec-review: NO new engine seam (compose existing mechanisms)?

A prior direction review feared S21f needs a *gated pre-market production seam* because own-use
cultivation runs **post-market** (`run_own_use_cultivation`, `settlement.rs:6935`, after
`society.step()` at `:6834`) — so cultivated bread is not barter-visible the *same* tick. Two facts
from the S21f research argue a new seam is **NOT** required:

1. **Cross-tick selling already works.** Cultivation post-market in tick `t` leaves the surplus in
   stock; tick `t+1`'s barter generator sees `held − stock_reserved_for_near_wants_barter(bread)`
   (`agent.rs:317`) as offerable and posts a sell lane. The surplus persists across ticks (perishable
   decay only trims it above the 20-floor). So the supply reaches the market one tick later — fine
   for a pre-promotion accumulation that must cross the strong-bar window anyway.
2. **Selling cultivated surplus is a SOLVED mechanism.** The `cultivation_sells_surplus` flag (S16)
   already routes lineage-cultivated surplus into the market, and `frontier_multigood` (S18) is the
   proven 3-role template (SALT-anchor consumers ⇄ bread cultivators (lineages) ⇄ WOOD gatherers).
   S16/S18 cleared real cultivated-bread trades — they just failed to *monetize* SALT under the
   *legacy single-layer* metric. S21f composes that same supply mechanism with the now-landed
   two-layer (S21b) + two-lane (S20) + marketability (S21a) machinery.

**Hypothesis: S21f is primarily a SCENARIO composition (no new engine code) — turn on the existing
cultivation-sells-surplus supply atop the S21e money machinery, with cross-tick selling.** The spec
asks Codex to confirm this, or to identify the minimal pre-market seam if cross-tick proves
insufficient. (Fallback, only if needed: a gated pre-market cultivation phase inserted between the
input-bid overrides `:6808` and the market `:6822`, reusing the same `report.produced`/
`consumed_as_input` booking.)

## 2. The trigger (cold-start) — the open colony supplies its own hunger pressure

Cultivation is hunger-gated: `cultivate_now` requires `hunger >= cultivate_hunger_in (6)` sustained
`cultivate_patience (2)` ticks (`settlement.rs:8903-8917`). It does NOT self-trigger on a fed colony.
But S21f's colony has the **food mints retired** (S21d/e), so its agents *are* hunger-stressed once
the cold-start buffers thin — hunger climbs past 6 (mortality off → they stay hungry, don't die), so
the existing hunger gate **fires**. So the existing trigger should suffice; the spec must *verify*
cultivation actually runs (a non-vacuity assertion), not assume it.

## 3. The scenario — `frontier_household_barter` (a `SettlementConfig`)

Derive from `frontier_open_survival` (`settlement.rs:3802`) — NOT `frontier_seeded_surplus` — and add
the endogenous cultivation supply:
- **Keep** (the S21d/e money machinery): `retire_food_mints = true`, `acquisition_ledger = true`,
  `multi_offer_medium` (S20), `durability_aware_acceptance` + marketability table (S21a),
  `two_layer_saleability` + `min_direct_use_acceptors` (S21b), the S9 strong-bar gates,
  `min_indirect_target_goods = 2`, mortality OFF.
- **No seed:** `seeded_surplus_bread = 0` (the whole point — endogenous, not seeded).
- **Turn on cultivation supply:** `content.with_cultivate()`, `own_use_cultivation = true`,
  `cultivation_sells_surplus = true` (the S16 lineage-only buy/sell split — keeps the SALT-rich
  non-lineage consumers as pure demand, avoiding buy-side collapse), the S15 cultivation knobs
  (`cultivate_consume`, `cultivate_hunger_in/out`, `cultivate_patience`).
- **WOOD-poor cultivators:** keep `wood_buffer = 12` + zeroed household WOOD so lineage cultivators
  have the unsatisfied WOOD target that makes them post `bread → SALT IndirectFor{WOOD}` (the S21e
  mechanism). Mirror `frontier_multigood`'s role separation: SALT consumers ⇄ bread cultivators
  (lineages) ⇄ WOOD gatherers; neutralize the WOOD<bread lowest-good-id offer-ordering artifact as
  `frontier_money_from_cultivation` does (`settlement.rs:3568-3576`).
- **Grain flow (the disclosed sweep axis):** the grain node regen/stock/cap sets the cultivated-bread
  flow. Disclose the chosen values; the flow is a real depleting commons (S15: 120/4/300), NOT a
  recurring mint — but it IS the tuning-risk axis (a too-generous regen is "a seed by another name").
  The sweep (below) must show promotion across a *window* of grain-flow rates.

## 4. Falsifiable bar + controls

Classify (seed 7, 1600 ticks):
- **Non-vacuity (gate):** cultivation actually runs (cultivators produce bread, `SelfProduced`
  credited) AND ≥1 cleared pre-promotion `bread → SALT IndirectFor{WOOD}` lane whose bread is
  `SelfProduced`. If cultivation never fires or no SALT lane clears → bad probe (fix), not a finding.
- **Endogenous supply:** pre-promotion bread sold for SALT is `SelfProduced`, with `SeededMinted`
  contribution **zero** (no seed, no mint) — the core S21f claim.
- **SALT promotes** as the medium leader, indirect breadth includes the non-food WOOD target,
  SALT-mediated share dominant (reuse S21e's `HEADLINE_MIN_SALT_SHARE_BPS`, not a dust bar).
- **Self-sustaining:** production continues (cultivated + later specialized chain) through the run;
  food consumed is `SelfProduced`/`Bought`, never `SeededMinted`/`Foraged`; conservation every tick.
- **Post-promotion** (optional, the stronger result): specialized chain roles adopt and acquire
  inputs by market trade (the S21e Phase-B clearance, now atop endogenous supply).

Controls (classify, never tune):
- **cultivation off** (`own_use_cultivation=false`) → reproduces the S21d zero-trade collapse (no
  endogenous supply).
- **seeded-surplus (S21e)** → the positive control (seeded supply works) — S21f must match it with
  *cultivated* supply.
- **no WOOD-poor target** → no/weaker monetization (cultivators don't post the medium lane).
- **two-layer off / multi-offer off** → no promotion (the S20/S21b machinery is load-bearing).
- **buy/sell split off** (`cultivation_sells_surplus=false`) → consumers self-cultivate → buy-side
  collapse → no monetization (proves the role separation matters).
- **grain-flow sweep** → promotion holds across a *window* of grain regen rates (not one tuned
  point); report the lower boundary (flow too thin to monetize) and confirm no rate is a de-facto
  permanent mint.

## 5. Slices

- **S21f.0** — the `frontier_household_barter` scenario (compose the flags; no engine change if the
  cross-tick hypothesis holds) + the non-vacuity instrument (cultivation runs + `SelfProduced` SALT
  lane). [If Codex/cross-tick requires it: a gated pre-market cultivation seam.]
- **S21f.1** — the classification suite + the run: assert the bar (endogenous `SelfProduced` supply
  monetizes SALT) OR classify; the control matrix incl. the grain-flow sweep; determinism; cross-seed
  robustness; a live run.

## 6. Determinism / golden contract

- All additions gated/scenario-scoped; if the cross-tick composition needs no new flag, the new
  scenario is purely additive (new `SettlementConfig` builder) ⇒ all 19 golden suites byte-identical.
  Any new gated flag defaults off + canonicalized ON-only with a digest regression. Acquisition/
  cultivation traces stay runtime-only.
- `cargo fmt --check` + `clippy --workspace --all-targets -- -D warnings` clean; conservation every
  tick; deterministic.

## 7. Honest scope

S21f is the **authentic supply mechanism** the S21d→e arc pointed to: endogenous pre-money
production (cultivated `SelfProduced` bread), not a seeded scaffold. It is still: mortality OFF; the
grain flow is a disclosed configured commons (the sweep proves a window, not a tuned point); the
direct-use SALT anchor + thresholds remain configured. It does NOT yet add the positive check
(mortality-on is the next milestone) or claim full demographic realism. If SALT monetizes on
cultivated supply, the open colony bootstraps money from genuine pre-money production-for-barter —
the capstone of the supply question; if it fails, the gate is localized (e.g. cultivation flow
insufficient, or cross-tick latency too slow for the strong-bar window) as a finding.

## 8. Pipeline

Codex spec-review (confirm the no-new-seam composition or pin the seam) → SPEC-READY → rb-lite
`codex,claude` → independent verification (workspace + all 19 goldens byte-identical + the new suite
+ a live run) → Codex review-of-results → merge + report/memory + pin.
