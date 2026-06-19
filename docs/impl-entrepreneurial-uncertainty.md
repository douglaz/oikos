# Implementation Spec: entrepreneurial uncertainty + profit/loss selection (S11)

> Codex's post-S10 verdict: with money emergence and capital choice now individual,
> "the next least-authentic mechanism is entrepreneurial appraisal from last realized
> prices." Today every entrepreneurial decision (adopt a recipe, build a tool, bid for
> inputs) uses `realized_price` — the **last** trade's price — as a *certain* point
> estimate, identical for everyone, and a wrong call costs the agent nothing
> differential. Misesian entrepreneurship is action under **uncertainty**: actors
> *forecast* future prices, differ in their forecasts, can be **wrong**, and bear the
> **profit or loss**. This milestone makes forecasts heterogeneous and fallible, and
> makes the loss *select* — through capital, not mortality.

## The two halves, and what stays out

1. **Forecasts under uncertainty.** Replace the shared last-realized-price point estimate
   in entrepreneurial decisions with a **per-agent forecast** that (a) differs across
   agents (a heritable `forecast_bias`), (b) adapts from experience (the existing
   `PriceBelief`), and (c) can be **wrong** (the market still clears at the real price,
   not the forecast).
2. **Profit/loss selection — through capital, NOT starvation.** A wrong forecast must
   have a *differential* consequence: an over-optimist that builds/adopts on an
   inflated price forecast sinks real WOOD/inputs into capital that underperforms, so it
   ends with **less** capital to invest, while an accurate/conservative forecaster
   accumulates and expands. Selection operates on **capital accumulation** (sunk WOOD,
   poor gold return), so it does NOT need starvation death — that is the *separate next*
   milestone (kept distinct on purpose). `hunger_critical` stays disabled here.

NOT starvation/mortality selection (next milestone), NOT firms/credit, NOT live RNG
(forecasts are deterministic — heritable bias + adaptive belief), NOT a change to market
clearing (the real price is forecast-independent; goldens byte-identical; additive +
gated, default off).

## Verified Base Facts (oikos @ `8435362`)

1. **Decisions use last-realized price as a certain point estimate.** `realized_price`
   returns the last trade's price, no smoothing (`econ/src/society.rs:3816`). It feeds —
   unmodified — the role-choice adopt appraisal (`settlement.rs:6689-6696` →
   `recipe_adoption_pays_for_money`), the capital build (`:7034-7038` →
   `capital_build_surplus` / `appraise_capital_tool_bundle_for_money`), and the input bid
   (`:6420` → `imputed_input_reservation`). Every one is a deterministic point estimate;
   none carries uncertainty.
2. **`PriceBelief` does adaptive per-agent expectation; econ already uses it for project
   valuation, but the COLONY chain appraisals do not.** `econ/src/expect.rs:5-46`:
   `{ expected, step }`, `observe()` moves `expected` toward realized each tick. It shades
   posted bids/asks (`society.rs:2509,2552`) AND econ project valuation already reads it
   for expected revenue/input (`society.rs:6439`) — but the **sim chain appraisals**
   (role-choice / capital / input-bid) still use raw `realized_price`, so the seam for S11
   is valid there. It is per-agent and serialized (`settlement.rs:9130-9135`).
   **Caveat (Codex P1):** `PriceBelief` starts at a neutral default and only updates for
   trade participants / live-quote watchers (`expect.rs:21`, `society.rs:2906`) — a
   would-be builder may never have observed the output good, so routing raw
   `belief.expected` risks cold-start arbitrariness. The forecast must be *grounded* (see
   the mechanism's `forecast_price_for`).
3. **There is real resource loss but NO selection.** A bad role de-adopts **costlessly**
   (vocation reverts, gold/stock kept; spent input is sunk but no agent-level marker —
   `settlement.rs:6718-6743`). A bad build forfeits committed WOOD (`salvage_bps:0`,
   `project.rs:227`), and an idle completed tool is a sunk loss — but the bad appraiser
   keeps its gold, never starves (`hunger_critical = need_max+1`, `:2078`), and reallocates
   freely. **So a bad appraiser fares no worse financially than a good one** — the missing
   ingredient is *differential* outcome, not loss per se.
4. **No profit/loss accounting.** `Agent` carries only `gold` + `stock`
   (`econ/src/agent.rs:90-100`); no earnings ledger, realized-vs-expected, or role ROI.
   A per-agent realized-proceeds / expected-error accessor must be added for the test.
5. **Heterogeneity is deterministic and heritable — the established pattern.**
   `CultureParams { time_preference_bps, leisure_weight_bps }` (`life/src/culture.rs:9-20`)
   is per-agent, drawn at generation (`draw_culture`, `settlement.rs:9268`), inherited via
   `deterministic_mix64(birth_seq ^ salt ^ field)` (`culture.rs:56-105`), and serialized
   (`settlement.rs:9164-9165`). A `forecast_bias_bps` field rides this exactly — no live
   RNG.
6. **Shock harness — econ `EventKind` exists but may not reach the SIM chain (Codex P1).**
   `EventKind` (`econ/src/scenario.rs:359-423`: `DisableRecipe`/`SeedStock`/`SetRegime`)
   fires at tick boundaries but is applied inside `Society` (`society.rs:1190`); the
   grain→flour→bread chain is **sim-side** production, so an econ recipe event may not
   perturb it. S11.3 therefore uses a **settlement-level** shock that demonstrably hits the
   chain — the existing `set_recipe_enabled` path (`settlement.rs:5409`, as
   `maybe_unlock_tier_two` uses) to time-box a chain-stage disable, or a conserved chain
   stock drain — and the test must assert the shock actually changed chain output.
7. **Gating + accessors as established.** Default-off `ChainConfig` flag pattern
   (`recurring_motive`/`producible_capital`/`per_agent_capital`, `:737-1005`, gated digest
   `:8701-8869`). Rich read-only probes for the falsifiable bar: `gold_by_vocation()`
   (`:8498`), `producer_cash()` (`:8624`), `tools_built()`, `living_count()`,
   `whole_system_total()`, `order_stats_by_vocation()`, `max_living_hunger()`.

## The forecast & selection mechanism (concrete — the core)

- **Forecast (grounded — `forecast_price_for(agent, good)`, Codex P1).** The expected
  **output** price is `forecast = base · forecast_bias`, where `base` is **grounded**, not
  the cold-start neutral default: use the agent's `belief.expected(good)` **only if the
  agent has actually observed the good** (its belief has been updated — i.e. `last_seen` is
  set / it has participated or watched a live quote); otherwise fall back to the public
  `realized_price(good)`; if neither exists (no trade has ever cleared), there is no
  forecast and the decision is skipped (as today with a missing price). So the forecast is
  always anchored to either the agent's own observation or the public realized price, then
  tilted by its bias — never an arbitrary neutral default. `forecast_bias` is a per-agent
  heritable bps multiplier. This replaces raw `realized_price(output)` in the three
  appraisals (Base Fact 1). Input/build costs stay at observed prices for S11 (one clean
  lever — output-revenue optimism; input-cost forecast is a noted extension). The forecast
  is **deterministic** (belief + bias are digested state).
- **`forecast_bias_bps` (exact, Codex P3).** A `CultureParams` field, u16 bps, **neutral
  default `10_000`** (= ×1.0, so a default agent forecasts the grounded base unchanged —
  which keeps `entrepreneurial_forecasts`-off byte-identical and a neutral-bias colony
  close to today's behavior). Drawn at generation with bounded jitter and inherited via
  `deterministic_mix64(birth_seq ^ FORECAST_BIAS_SALT ^ field)` like the other culture
  fields; **clamped to exactly `5_000..=20_000`** (×0.5–×2.0) so optimism is bounded, not
  delusional. Serialized in `canonical_bytes` (the per-colonist culture
  block) with a `canonical_bytes_include_*` regression.
- **Fallibility.** The market still clears at the **real** price (forecast-independent).
  So an optimist that adopted/built on an inflated output forecast earns the real (lower)
  revenue: its committed WOOD/inputs are sunk and its gold return is poor.
- **Selection through capital.** Differential capital accumulation IS the selection: the
  persistent over-optimist depletes WOOD/gold on capital that underperforms → forms
  **less** future capital (it can't afford the WOOD, and its lower realized proceeds fail
  the next ordinal appraisal); the accurate/conservative forecaster accumulates → builds
  more. No agent is killed; `belief.observe()` lets each agent **learn** (its `expected`
  tracks realized), so a biased agent still systematically over/under-shoots but is not
  permanently delusional. **This needs no new "kill" machinery** — it falls out of
  forecast → real WOOD cost → real (forecast-independent) proceeds → the existing ordinal
  capital/role appraisals.
- **Profit/loss visibility.** Add a per-agent realized-proceeds (and/or cumulative
  expected-vs-realized error) accessor so the test can show optimists end poorer.

## The slices (build in order; each independently testable)

- **S11.1 — heterogeneous fallible forecasts feed decisions (gated).** Add
  `forecast_bias_bps` to `CultureParams` (drawn + inherited like the others) and a
  default-off `entrepreneurial_forecasts` flag. In the per-flag path, route each agent's
  `forecast_price_for(agent, output)` (the grounded rule above) into the role-choice /
  capital / input-bid appraisals instead of raw `realized_price`. Distinguish "never
  observed" from a tick-0 observation explicitly (an `Option`/`observed` flag, not a
  `last_seen == 0` sentinel) so the belief-vs-realized fallback is correct at tick 0.
  **Unit test:** with the flag on, two agents
  with different `forecast_bias` appraise the **same** market state differently (one
  adopts/builds, the other doesn't); the realized clearing price is unchanged
  (forecast-independent); flag off → byte-identical.
- **S11.2 — profit/loss realization + capital selection (the falsifiable core).** Wire the
  realized outcome so a wrong forecast costs the forecaster differentially; add a per-agent
  realized-proceeds (and cumulative expected-vs-realized error) accessor AND a **net-worth
  balance-sheet accessor** `agent_capital(i) = gold + WOOD·realized_wood_price + tools·V`,
  where **V = the tool's realized liquidation price if tools ever trade, else ZERO**
  (conservative — tools don't trade today, the anchor prevents selling; an idle/unproductive
  tool adds nothing to net worth, and a *productive* tool's worth already shows up as the
  gold it earned). So an optimist cannot hide a sunk-WOOD loss inside idle tools (Codex P2). **Microtest — a
  controlled NEGATIVE-NPV opportunity:** construct a market state where building is
  *unprofitable at the real price* but *appears profitable at an inflated forecast*. The
  accurate forecaster **declines and preserves** its resources; the over-optimist **builds,
  realizes lower-than-forecast proceeds, and ends strictly lower on `agent_capital`** than
  the accurate one. (Deterministic, signed, isolates the mechanism — this is the
  falsifiability tripwire.)
- **S11.3 — shock → discoordination → recovery (a REAL chain shock, Codex P1).** The econ
  `EventKind::DisableRecipe` lives inside `Society` and may not touch the **sim-side** chain
  production — so S11.3 uses a **settlement-level** shock that demonstrably perturbs the
  grain→flour→bread chain: a gated, time-boxed disable of a chain stage via the existing
  `set_recipe_enabled` path (the same mechanism `maybe_unlock_tier_two` uses,
  `settlement.rs:5409`) — e.g. disable Bake for ticks `[A,B)` then re-enable — OR a
  conserved stock drain of a chain good over a window. **The test must first assert the
  shock actually changed chain output** (bread dips during `[A,B)`) so it is not a no-op,
  then show the colony **temporarily discoordinates** (hunger spike / production dip /
  order-book gap; some agents mis-forecast across the shock) and **recovers to pre-shock
  bounds** in the tail with no planner correction (beliefs re-learn, decisions
  re-coordinate).
- **S11.4 — the flagship scenario + DoD.** Add `frontier_coemergent_strong_entrepreneurial`
  (derive from the S10 originary base) with `entrepreneurial_forecasts` on. **Test:** the
  clean metric below.

## Acceptance Tests (the S11.4 DoD) — `sim/tests/entrepreneurial_uncertainty.rs`

1. `entrepreneurial_run_is_deterministic` — byte-identical `(seed, config)`.
2. `forecasts_are_heterogeneous_and_feed_decisions` — agents with different
   `forecast_bias` reach different adopt/build decisions on the same market state; the
   realized clearing price is forecast-independent.
3. `optimist_overbuilds_and_ends_poorer` — THE clean selection microtest, a controlled
   **negative-NPV** opportunity (unprofitable at the real price, profitable at an inflated
   forecast): the accurate forecaster **declines and preserves** resources; the over-optimist
   **builds, realizes lower-than-forecast proceeds, and ends strictly lower on the
   `agent_capital` balance sheet** (gold + WOOD-at-realized + tools-at-realized-value, so it
   can't hide a gold loss in idle tools). Deterministic, signed.
4. `forecasts_can_be_wrong` — there exist decisions where the agent's forecast materially
   differs from the realized price (an expected-vs-realized gap via the new accessor), AND
   beliefs adapt toward realized over time (`observe()` is live) — forecasting under
   uncertainty, not clairvoyance.
5. `shock_causes_discoordination_then_recovery` — a **settlement-level** chain shock (gated
   time-boxed `set_recipe_enabled(stage,false)` over `[A,B)`, or a conserved chain-stock
   drain): FIRST assert the shock actually changed chain output (bread dips in `[A,B)` — not
   a no-op), THEN a measurable temporary disruption (hunger spike / production dip /
   order-book gap) that **recovers to pre-shock bounds** in the tail, with no planner
   correction.
6. `selection_is_not_mortality` — confirm no starvation deaths occur (selection is
   capital, not death; `hunger_critical` still disabled) — keeps this milestone distinct
   from the starvation milestone.
7. `entrepreneurial_conserves` — whole-system conservation every tick (forecasts move no
   goods; only the decision changes; the real trade conserves as always).
8. `goldens_unchanged` — with `entrepreneurial_forecasts` off, S5–S10 scenarios + the six
   econ + g5a/g5b/coemergence goldens are byte-identical; S5–S10 suites green; the new
   `forecast_bias`/flag state has `canonical_bytes_include_*` regressions; clippy
   `-D warnings`; fmt `--check`.

Manual: `cargo run -p viewer -- run entrepreneurial --ticks 1600`.

## Missing Interactions (the central risks — track explicitly)

- **The noise risk (Codex's warning: "easy to make noisy and unfalsifiable").** Forecasts
  must be *deterministic* (heritable bias + adaptive belief, both digested) and the
  selection must be a *clean, signed* effect (over-optimist ends poorer — test 3 is the
  tripwire). If "bad forecasts" only add variance with no directional selection, the
  milestone has failed its purpose — land that honestly.
- **Forecast must actually be able to be WRONG.** If `belief.observe()` converges so fast
  that forecast≈realized always, there is no uncertainty and no selection. Keep the bias a
  standing multiplier so a biased agent systematically mis-forecasts even as its belief
  tracks the level (test 4).
- **Selection vs. churn.** A bad forecaster should lose *capital*, not thrash roles every
  tick. Ensure the de-adopt/re-adopt and build/no-build don't oscillate on forecast noise
  (reuse S10's demand-gating / hysteresis where relevant).
- **Don't break S10's time-preference response or S9 emergence.** The forecast feeds the
  same appraisals S10/S9 use; verify the originary-interest and emergence tests still hold
  with forecasts on (the flagship derives from the S10 base).
- **Digest.** `forecast_bias_bps` (culture) + the flag + any new realized-proceeds state
  that steers future ticks → `canonical_bytes` + regressions (`PriceBelief` is already
  digested).

## Handoff Notes

- **Reuse `PriceBelief` — don't reinvent expectation.** It is already per-agent, adaptive,
  and digested; route it via `forecast_price_for` into the three appraisals. The only new
  per-agent field is `forecast_bias_bps` on `CultureParams`.
- **Selection is capital, not death.** No starvation here (that's the next milestone);
  the loss is sunk WOOD + poor realized proceeds → less future capital. Prove it with the
  optimist-vs-accurate microtest (test 3), the clean falsifiable signal.
- **Single clean lever:** bias the **output** (revenue) forecast for S11; input-cost and
  build-cost forecasts are noted extensions. One lever keeps the selection signal legible.
- **S11.3 shock is SETTLEMENT-LEVEL, not econ `EventKind`** (Base Fact 6): time-box a
  chain-stage disable via `set_recipe_enabled` (`settlement.rs:5409`) or a conserved
  chain-stock drain — econ `EventKind` may not reach the sim chain. The test must first
  assert the shock changed chain output (not a no-op).
- **Gate everything** (default off) so S5–S10 + goldens stay byte-identical; the
  multi-horizon ladder and per-agent capital from S10 must remain intact in the flagship.
- Build S11.1→S11.4 as separate commits with their own tests; `git add` new files.
- **Follow-ons:** re-enabled starvation / mortality selection (the remaining stopping-point
  piece); fully-endogenous provisioning-at-scale under emergence (S12); input-cost
  forecasting; richer expectation (variance/confidence, not just a point + bias).
