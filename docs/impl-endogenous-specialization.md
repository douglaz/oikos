# Implementation Spec: the endogenous specialization economy (sliced)

> Revised after a Codex spec review (2026-06-17). The first draft was one
> oversized milestone with a contradiction (it called Experiment 10 the
> "subsistence base," but Experiment 10 is fed by the curated `subsistence_advance`
> the DoD turns off). This version slices the work, fixes the base, specifies the
> highest-risk piece (the order-book hook), and sharpens the falsifiable tests.

## Purpose & the honest bar

Make the grain→flour→bread division of labor **emerge atop a household/subsistence
base and sustain on real market trade**, with **no chain-specific *global*
placement of food or inputs** by a planner. Local/household allocation (family
provision, custom, inheritance) is *not* scaffolding and is allowed; a global
"richest holder hands grain to every producer" phase (`run_input_advance`) is.

This replaces the over-strict "atomized market-only survival" bar. The twelve
experiments (`docs/experiment-money-circulation.md`) showed the chain only
sustains under curated placement (Experiment 12 scaffold); strip it and the chain
dies ~tick 150 (Experiments 12 ablations, 13). The target: it lives without the
chain-specific global placement.

NOT firms/wage-labor, NOT banks/fiat/credit, NOT value-scale surgery, NOT a change
to econ market-CLEARING behavior (six conformance goldens byte-identical; any econ
edit additive and gated, proven by a disabled-hook regression test).

## Verified Base Facts (2026-06-17, oikos @ `8d0a233`)

1. **There is no real subsistence base yet for the chain colony.** Experiment 10's
   fed state (`frontier_in_kind`) comes from the curated `subsistence_advance`
   phase, which the endogenous DoD turns OFF. A real **household/raw-food**
   subsistence path is needed (the `subsistence_on_grain` edible-grain fallback
   exists — `KnownGoods::subsistence`, `ChainConfig::subsistence_on_grain` — and
   household provision exists via demography; compose one of these so colonists
   self-feed locally with no global food placement).
2. **The blocker (Experiments 9, 13): input buying uses the GENERIC spot bid,
   blind to the recipe.** Adoption uses the project-bundle appraisal
   (`recipe_adoption_pays_for_money`, `sim/src/settlement.rs:7572` →
   `appraise_project_bundle_for_money`, `econ/src/bundle.rs:50`); the input bid
   uses `ensure_bid` → `Agent::reservation_bid_for_money` (`econ/src/society.rs:2408`,
   `econ/src/agent.rs:356`), which has no recipe/output context. The producer's
   input want is ranked below its present-good wants (`scale_input_insert_position`,
   `sim/src/settlement.rs:7746`).
3. **`Society::step()` owns the order book** (`econ/src/society.rs:608`): it cancels
   changed live quotes, loops every (agent, good), calls `ensure_bid`/`ensure_ask`
   → `ensure_order`, which is the SOLE order-insertion path and also maintains
   `live_quotes`, `reservations`, TTL, M3 settlement, and `trades`. **Poking
   `Society::books` (`econ/src/society.rs:245`) directly is wrong** — it bypasses
   that machinery. (Codex.)
4. **The Experiment-13 sim phase is a direct bilateral trade, not order-book
   endogenous** (`run_project_input_bids`, `sim/src/settlement.rs:5459`, runs
   *after* `step()`). It conserves and buys some inputs but does not sustain.
5. **`recurring_motive`** (`run_role_choice`, `sim/src/settlement.rs:5702`) keeps a
   producer adopted while profitable — sound in intent, but currently scalar on a
   possibly-stale `realized_price` and treats a missing input price as zero cost;
   fold its profitability test into the bundle/live-demand appraisal.
6. **The revolving loan repayment sweeps producers cash-light**
   (`run_capital_repayment`, `sim/src/settlement.rs:5550`), which fights repeated
   market bidding. Working capital must persist.
7. Conservation (`EconTickReport::conserves()`), determinism, goldens — as before.

## The slices (build in order; each independently testable)

The pieces are interdependent, so **do not build them as one DoD** — partial
builds would all read as failure. Slice it (Codex):

- **S1 — econ order-book bid override (highest risk).** A gated per-`(agent, good)`
  spot-bid override inside `Society`: the sim sets `(reservation, limit)` overrides
  before `step()`; `ensure_bid` checks the override first and uses it instead of
  `reservation_bid_for_money`; `live_quote_changed`/cancellation must respect the
  override; `ensure_order` stays the only insertion path; overrides cleared after
  the step. Additive + gated. **Unit test:** an override bid enters the real book,
  reserves gold, fills against a willing ask, and records a `Trade`. **Regression:**
  with the override unused, every econ golden is byte-identical (the tripwire).
- **S2 — project-aware producer bid (fixed population).** The sim computes each
  active producer's imputed input reservation via the **bundle appraisal** (reuse
  `recipe_adoption_pays_for_money`'s logic, NOT scalar `recipe_is_profitable`),
  sets it as the S1 override, and suppresses the generic low-ranked input want to
  avoid a duplicate bid. **Test:** an active producer acquires its input through a
  real order-book `Trade` and produces. Decide and document the imputation's
  output-price source (last realized vs live bid vs TTL-limited).
- **S3 — working-capital persistence.** Producers retain earnings up to a
  working-capital reserve; no per-tick planner top-up; any loan is real saved money
  repaid only from surplus above the reserve (or: just retained earnings, since
  `recurring_motive` guards satiation). **Test:** a producer bids and produces over
  many ticks with no planner loan/sweep.
- **S4 — cold-start bootstrap.** Seeded buffers (`latent_flour_seed`,
  `bread_buffer`) produce the first realized flour/bread prices so latent
  millers→bakers adopt deterministically in pipeline order. **Test:** adoption
  emerges from seeded prices/buffers, no curated advance.
- **S5 — the endogenous scenario (the real DoD).** A real household/subsistence
  base (Base Fact 1), S1–S4 composed, **all chain-specific global placement OFF**
  (`subsistence_advance`, `input_advance` off). **Test:** the clean endogenous
  metric below.
- **S6 — scaling / churn (separate milestone).** Replacement producers (a hungry
  colonist takes up an unserved trade — needs tool/latent-recipe acquisition) so
  output tracks population and hunger stays bounded as it grows. Deferred:
  underspecified (tools, training, inheritance) and would obscure whether S1–S5
  work; prove fixed-population first.

## Acceptance Tests (the S5 DoD, sharpened)

`sim/tests/endogenous_economy.rs`:

1. `endogenous_run_is_deterministic` — same `(seed, config)` → byte-identical.
2. `inputs_acquired_by_market_trade` — THE clean metric. With every chain-specific
   global-placement phase OFF, after tick 300 there exist `Society::trades` records
   with `trade.good ∈ {grain, flour}`, `trade.buyer` an active Miller/Baker,
   `trade.seller ≠ the buyer`, and the acquired input is **later consumed as a
   recipe input** — and NO transfer/placement counter moved those units. (Codex:
   "gold falls/rises" alone is insufficient — the failed sim phase moved gold too;
   require the actual `Trade` record + downstream consumption.)
3. `specialization_sustains` — `bread.made > 0` through tick 800 and 1600.
4. `hunger_and_provisioning_are_stationary` — over tail windows: population trend,
   bread produced **per capita**, and hunger mean/p95 slope are bounded and
   non-drifting; no hidden drawdown of initial food buffers. (Distinguishes a
   stable economy from managed decline — a fixed ceiling alone can miss it.)
5. `endogenous_conserves` — whole-system conservation every tick across all new
   flows (override bids, trades, working capital).
6. `econ_unchanged` — full suite; six goldens byte-identical (incl. the S1
   disabled-hook regression); clippy `-D warnings`; fmt `--check`.

Manual: `cargo run -p viewer -- run endogenous --ticks 1600`.

## Handoff Notes

- **Highest-risk part: the S1 override lifecycle in `Society::step()`.** It MUST
  integrate with `live_quote_changed`, `reservations`, and `ensure_order` — else it
  is canceled, bypasses the real market, or perturbs econ behavior. The
  disabled-hook golden regression is the tripwire; gate it so lab scenarios never
  set an override.
- The bid price is the **bundle-appraised imputed reservation**, not the scalar
  heuristic and not a `Society::books` poke. Suppress the generic input want.
- Working capital = retained earnings / real saved money with a reserve, **not** a
  per-tick planner loan — otherwise "no curated advances" is false.
- The base is a **household/raw-food** subsistence path that works with
  `subsistence_advance = false` (Base Fact 1). If a test only passes with a
  global-placement phase on, it is still scaffolded — fix the market/base path.
- Build S1→S5 as separate commits with their own tests; S6 is a later milestone.
- `git add` new files; gitignore stray build artifacts.
