# impl-73 — C3R.h: Stale Input Price vs Final Demand (why the oven margin computes negative, and which fix clears the five-seed gate)

Status (spec): **v2 — DRAFT** (Codex xhigh spec-review folded, 1 round: DIAGNOSIS-OVER-READ + 6×P1
+ 2×P2 → the authoritative `## −0` section supersedes §§0–8 where they conflict). Successor to
impl-72 (C3R.g); it must clear the immortal five-seed viability gate before impl-71 (C3R.f,
lifespan). **v1 (superseded)** diagnosed a live "price inversion / final demand missing" and
recommended restoring bread demand (L1). Codex's review — verified against the code — showed the
primary mechanism is a **stale input-price appraisal**, not a live inversion: role-choice reads
`realized_price(flour)` with no age gate (`phases.rs:2270`), and that accessor is the *last trade's
price, persisted forever* (`econ/src/society.rs:4779`); with bread at 1 a baker's flour bid is
capped near `3·1 − 1 = 2` (the Mengerian ceiling), so **no flour clears at 12** once bread is
cheap — the `P_flour = 12` carried from tick ~300 to 1600 is a phantom frozen from the early boom.

## −0. v2 revision (AUTHORITATIVE — folds the Codex spec-review; supersedes §§0–8 on conflict)

**Corrected diagnosis (P0).** The baker's `MarginNonpositive` rejection is a **stale-input-price
role-choice failure**, not a live economic price inversion. `3·P_bread − P_flour − cost = 3·1 − 12
− 1 = −10` is the *appraisal's* arithmetic, but `P_flour = 12` is a stale last-trade
(`realized_price` has no recency, `society.rs:4779`; the appraisal reads it raw, `phases.rs:2270`),
and no flour can contemporaneously clear at 12 when bread is 1 (baker bid ceiling ≈ 2). So the
baker is rejected on a **phantom input cost**. Weak final demand is retained as a *plausible
secondary* contributor, not the primary cause. `P_bread = 1` is likewise unproven as a demand floor
until late bread *trades* (not the realized price) are shown. The `+34` mill margin is also
stale-based and does NOT establish flour overproduction/surplus capture — the 12 is the last
executed trade's resting-order limit (`econ/src/market.rs:441`), not yield arithmetic.

**Phase-1 measured (2026-07-19, trade-level, failing immortal seed 7).** Flour and bread
*trades* per 100-tick window resolve the staleness question directly:
- **Flour is stale — CONFIRMED.** Flour trades run 71 → 83 → 7 → **0**, and stay 0 from tick ~300
  to 1600, while `realized_price(flour)` holds frozen at 12. The −10 appraisal reads a phantom
  input cost with no live market. **L2 is real and primary.**
- **Bread is live, not stale, and not a demand vacuum.** Bread trades run 240–385 *per window*
  the whole run at price 1 — a real active market floored at 1 (hearths + `producer_subsistence`
  mint bread → flooded supply → floor), NOT the absence of demand my v1 claimed.
- **Flour reservation ask ≈ 1 (reconstructed, not a live executable quote).**
  `order_stats_by_vocation` at ticks 800/1200 shows ~7 millers with a *reservation* ask of 1
  (`reservation_ask_for_money`, `mod.rs:13506` — a one-unit reconstruction, NOT a live
  resting/matchable order; the real market shades the reservation, checks availability, can fail
  posting, and trades at the resting limit). So cheap flour supply is *probable* but **not measured
  as executable**. **Corrected conclusion (Codex review v3, 2026-07-20 — the session's fifth price
  over-read, again a proxy read as ground truth): staleness is CONFIRMED, but L2-*sufficiency* is
  UNVERIFIED and likely insufficient alone.** Two reasons: (1) `3·1 − 1 − 1 = +1` is a **knife-edge**
  (at flour 2 it is exactly 0 and strict profitability fails; execution order decides which side's
  limit fills, `market.rs:485`); (2) a positive *appraised* margin need not *realize* — bread is
  minted-flooded (hearths + `producer_subsistence`), and the demand-responsive restock guard stops a
  baker buying flour while its own bread sits **unsold** (`mod.rs:8480`), so the baker's bread may
  never clear against minted bread. **L2 is a real contributor; `L1+L2` is the defensible combined
  arm; whether L1 is necessary is exactly what the strengthened 2×2 resolves — measured on
  baker-ORIGIN bread sales and realized round-trip margin, not global bread trades.**

**Corrected demand topology (P1).** Bread already *is* the preferred hunger staple
(`generation.rs:83`); the fallback is **edible raw grain** (`subsistence_on_grain = true`,
`scenarios.rs:286`), not a distinct FORAGE good, and hearths + `producer_subsistence` **mint bread
itself** (`demography.rs:1080`, `phases.rs:944`). The diagnostic `food = 0` zeros only the six
appended producer-house hearths; two legacy hearths stay at 3 and `producer_subsistence = 4`
remain. So "the population never needs bread" is **false**; the real L1 is *retiring the bread mints
and the raw-grain substitution* so market bread demand can form — not re-architecting what the
colony eats.

**Levers (reprioritized):**
- **L2 — stale-input-price fix (a real contributor; mechanism pinned precisely).** Value the input
  at a **candidate-specific actual quote: the lowest non-self, unexpired resting flour ask this
  candidate can afford** — not the stale realized price. If no such ask exists, **decline
  explicitly** (never pass `None` into the appraisal — that zeros the input cost, `mod.rs:14596/15418`,
  manufacturing a false positive). Flag-off must retain the raw realized-price path **byte-for-byte**.
  **Determinism/digest (Codex v3):** `canonical_bytes` serializes beliefs and realized prices
  (`digest.rs:1007,1371`) but NOT live order books or last-trade timestamps; so either **derive the
  appraisal input from already-serialized state** or **serialize the order-book/age state L2 reads,
  ON-only** — the coverage-guard + off-identity/on-divergence tests do not close that gap by
  themselves. Capital formation's recency gate is NOT a precedent (it gates the output/demand
  signal, `phases.rs:2789`, not the input price).
- **L1 — retire bread mints + raw-grain substitution (secondary).** Compose *existing,
  already-digested* fields (`producer_subsistence` `digest.rs:61`, raw-grain subsistence
  `digest.rs:748`, food-mint retirement `digest.rs:229`, household provisions `digest.rs:1922`) so
  market bread demand can form. Promote to primary only if the 2×2 (below) shows it independently
  necessary. A new `HouseholdSpec` field, if introduced, needs its own coverage guard (the
  `DemographyConfig` guard does not destructure `HouseholdSpec`).
- **L3 — yield/cost rebalance (fallback), unchanged.**

**Phase 1 (measure before choosing) — trade-level, not realized-price.** Record: flour and bread
**last-trade ages**, live bid/ask limits, failed crossings, buyer class + acquisition channel,
flour stocks/fills, and hunger/starvation. A bread trade at 1 does NOT prove a demand floor
(abundant minted supply / low reservations / stale price all reproduce it). This must discriminate
L1 vs L2 by running the actual **2×2 intervention: base / L2-only / L1-only / L1+L2**.

**Outcomes (exhaustive, non-overlapping — replaces §2):**
- **STALE-PRICE-SUFFICES** — L2 alone clears the gate.
- **DEMAND-SUFFICES** — L1 alone clears the gate.
- **EITHER-SUFFICES** — both single arms clear it independently.
- **BOTH-NEEDED** — both single arms fail; L1+L2 clears it.
- **DEEPER-WALL** — L1+L2 fails on ≥1 seed (flour supply/route, capacity/utilization, or
  seed-fragile bootstrap; Phase 1's observables must cover these). Mixed-seed results classified
  explicitly.

**Acceptance (corrected — replaces §5; strengthened by Codex v3).** Profitability is **strict**
`revenue > input + cost` (`mod.rs:14587`; a *zero* margin is `MarginNonpositive`, `phases.rs:2321`),
so require a **strictly positive realized round-trip margin, baker-attributed**: flour PURCHASES,
bake executions, and **sales of baker-ORIGIN bread**, inventory change accounted, on
**contemporaneous executed** prices (snapshot quotes and the knife-edge `+1` do not count — execution
order decides the fill, `market.rs:485`). The existing `FlowRuns` (staffing + *cumulative*
production) is **insufficient**: minted bread can supply every observed bread sale while the baker's
own output never clears (`demography.rs:1087`, `phases.rs:944`; the restock guard `mod.rs:8480` then
stalls its flour buying). A passing arm MUST show final-window **baker-origin bread actually sold**.
Run the 2×2 (base / L2 / L1 / L1+L2) **per seed** across all five immortal seeds — report per-seed,
do NOT pool — with a **starvation / bounded-hunger control**; `L1+L2` is the defensible combined
arm. mortal `FlowRuns` belongs to impl-71, but add a **mortal non-regression smoke**.

## Cuts and status

**Cut 1 — the L2 lever — LANDED** (master `0cb6f8d`, PR #2). A default-off
`ChainConfig::stale_input_price_fix`: `run_role_choice` values the recipe input at the minimum
non-self holder `reservation_ask_for_money` (pure over serialized `scale`/`stock`/`gold` — no
order-book/timestamp reads, no new digest state), declining explicitly when no holder exists
(never `None→0`); off is byte-identical. **Result:** on the immortal five-seed base, L2 collapses
the stale-price rejection (`margin_nonpositive` ~8,400 → 0) and makes the baker stage **sustain on
all five seeds** (0 → 9 bakers; bread ~400 → ~12,300, ~30×). ~~**STALE-PRICE-SUFFICES is the
leading result**~~ — measured on cumulative *production* (a strong proxy: the restock guard stalls
unsold output), not yet baker-origin *sales*. **FALSIFIED by cut 2 (below): the production proxy
did not hold. The staffing and output result stands; the "suffices" reading does not.**

**Cut 2 — the rigorous close — LANDED. Result: `BakerProducesButDoesNotSell` /
`DEEPER-WALL`, 5/5 seeds.** The telemetry (a non-digested per-run Baker-class accumulator: flour
gold spent, bread gold earned, bread units sold, bread units produced) plus the 2×2
(`sim/tests/baker_roundtrip_2x2.rs`) measured *sales*, not production, and refuted cut 1's leading
reading.

| arm | living bakers | bread produced | bread sold | final-window sold | realized round trip |
| --- | --- | --- | --- | --- | --- |
| base | 0 (9 on seed 3) | 351-489 | 42-45 | **0** | +693 … +814 |
| **L2** | **9** | **~12,000-12,400** | **46-59** | **0** | **−3,183 … −3,868** |
| L1 | 3 | 4,065 | 28-32 | **0** | +948 … +1,781 |
| L1+L2 | **0** | 27 | 15 | **0** | +78 |

Readings, in order of how much they change the picture:

- **The wall is CLEARING, not production.** L2 makes the stage staff and bake ~30× more bread, and
  the extra loaves do not sell: 46-59 sold out of ~12,000 baked, and **zero** sold across the final
  160-tick window on every arm and every seed. No arm reaches the pre-declared `SUBSTANTIAL = 300`
  baker-origin sales floor. `DEEPER-WALL` (L1+L2 fails) on all five seeds, so neither lever alone
  nor both together closes the loop.
- **L2 runs the stage at a LOSS.** It buys ~4,100 gold of flour against ~900 of bread revenue — a
  realized cash round trip near **−3,200**. The arms that bake far *less* (base, L1) are cash
  *positive*. So L2 does not merely fail to convert output to money; it converts money to unsold
  inventory. That is a stronger statement than "sales are low" and it is the thing impl-74 has to
  explain.
- **L1+L2 is a negative interaction on staffing.** L2 alone sustains 9 bakers, L1 alone 3, both
  together **0** (27 loaves, the seed stock). Retiring the food floor removes the very subsistence
  that let the stale-price fix keep producers alive long enough to adopt.
- **The demand side is pinned at the hunger ceiling.** `window_max_hunger` is 11-12 against
  `need_max = 12` on every arm, while ~12,000 loaves go unsold. Hunger is not the missing demand —
  purchasing power is. (No starvation: this base inherits `hunger_critical = need_max + 1`, so the
  ceiling is unreachable and a hunger *bound* would be a vacuous assertion; the mortal smoke is
  where a reachable ceiling is exercised, at `starvation_deaths_total == 0`.)
- **Attribution caveat, bounded.** Bread sales are attributed by seller vocation on the spot tape.
  On `base`/`L2` a Baker can also be selling *minted* loaves (`phases.rs:971`,
  `demography.rs:1101` both mint `known.hunger`, which is bread here, unless `retire_food_mints`).
  The bias is strictly **upward on sales**, so it cannot manufacture this null — and the L1 arms
  retire both mints, are contamination-free, and fail too. A per-loaf provenance ledger becomes
  mandatory only if a future cut produces a *pass*.
- **impl-71 (C3R.f, lifespan) does NOT unblock.** That gate was conditioned on confirmation; the
  confirmation did not arrive.

**Cut 2 — original scope (v2 folds a Codex+Fable dual review).**
Both reviews returned NEEDS-REVISION on the v1 scope; the corrections (all verified against the
code) — the milestone is a small **default-off, non-steering telemetry trace** (impl-72-sized) +
config arms + the 2×2 test:

- **[P0] The v1 sales accessor is DEAD here — replaced.** `bread_for_salt_volume_by_provenance()`
  is `(0,0)` on every arm: it populates only under `bread_provenance_active()` =
  `cultivation_sells_surplus_active()` (`mod.rs:10639` → `gates.rs:169`), which the frontier chain
  ancestry never sets, and it attributes only bread→`barter_medium` trades while
  `frontier_endogenous` sets **`barter = None`** (designated GOLD, `scenarios.rs:283`) — bread
  sells for **gold**. **Instead:** add a **default-off, non-steering, non-digested per-run
  Baker-class accumulator** (like impl-72's `role_choice_diag`): gold spent buying flour, gold
  earned selling bread, bread units produced, bread units sold — attributed to agents in the Baker
  vocation at the trade/production instant (hook the spot-trade settlement + bake paths). Fable's
  tape-only (`Society.trades` seller==Baker, `mod.rs:12158`) is a lighter fallback but is
  **origin-contaminated** (a baker can resell minted/endowment bread), so the trace tracks origin
  (produced-vs-sold) rather than trusting seller-vocation as a proxy — the whole point of cut 2.
- **[P1] L1 = `retire_food_mints=true` AND `subsistence_on_grain=false`** (both existing
  `ChainConfig` fields). `retire_food_mints` alone gates both recurring bread mints (demographic
  hearth `demography.rs:1098`, producer staple leg `phases.rs:957` — verified complete for *mints*)
  but leaves the **raw-grain substitution** (`subsistence_on_grain`, `scenarios.rs:290`) that §−0's
  L1 also names, so hunger diverts to grain and bread demand still won't form. The L1 bundle
  changes producer survival too, so rename the label **FOOD-FLOOR-RETIREMENT-SUFFICES** (not
  "DEMAND-SUFFICES" — it is not pure demand isolation).
- **[P1] Realized margin = executed cash flows** — per Baker class: bread-sale gold − flour-buy
  gold, inventory-accounted (FIFO completed cycles), with `operating_cost` labeled **imputed**
  (it is an appraisal threshold, never a real gold debit — `mod.rs:1019`, no payment site). Pin
  which inequality gates: realized-cash-positive is the acceptance; note it can differ from the
  appraised (cost-subtracted) margin at the knife-edge.
- **[P1/P2] Outcome = a per-seed EXCLUSIVE truth table**, suite-labelled only when all five seeds
  agree; evaluate the combined arm first: `DEEPER-WALL` iff L1+L2 fails; else
  `STALE-PRICE-SUFFICES` / `FOOD-FLOOR-RETIREMENT-SUFFICES` / `EITHER` / `BOTH-NEEDED`; plus
  `BASE-SUFFICES`, `NEGATIVE-INTERACTION` (a single arm passes but L1+L2 fails), and `MIXED-SEED`.
- **[P2] Pin the executables:** final window = last 160 ticks; "substantial" baker-origin sales =
  a pre-declared minimum (pick a number, e.g. ≥ a few ×100 bread sold/seed) so acceptance is
  falsifiable; the hunger control samples `max_living_hunger()` **each tick** and keeps the window
  max; the survivor floor is scoped to the **mortal lineage** side — the immortal base sets
  `hunger_critical = need_max + 1` (`mod.rs:3665`) so producer starvation is vacuous; use
  `population()/is_alive()/household_of()` for non-lineage survivors, not `living_count`.
- **[P2] Assert (not just print) cut-1's result** — all five L2 arms sustain the baker stage and
  raise output — and pin the **mortal smoke** (scenario/seed/arm/horizon + `starvation_deaths_total`
  + living-floor).
- **Determinism/goldens (both reviews: sound):** the new trace is non-steering + non-digested
  (impl-72 pattern), the two treatment flags are default-off ON-only-digested, and the 2×2 arms are
  additive in-test configs → every existing golden byte-identical.
- **On confirmation, impl-71 (C3R.f, lifespan) unblocks** — the immortal five-seed viability gate is
  met.

## 0. One-paragraph summary (superseded by §−0 where it conflicts)

impl-72 showed the Baker role is rejected by `MarginNonpositive` on ~93% of appraisals on the
failing seeds. A price-path probe on a failing immortal seed stabilizes by tick ~300 and holds:
`P_grain = 1`, `P_flour = 12`, `P_bread = 1`, `operating_cost = 1`, so the *appraisal* computes
mill `+34` / bake `−10`. **Per §−0 the `P_flour = 12` is a stale last-trade, not a live clearing
price** — so the primary fix is the stale-input-price appraisal (L2), with weak final demand (L1)
a secondary contributor, decided by the Phase-1 2×2. The milestone must let a functioning chain
(`FlowRuns`) appear on all five immortal seeds — not just seed 3.

## 1. Base facts (measured 2026-07-19)

Failing immortal seed (`FlagOffHeritable`, food=0, cap=2), realized prices, steady state from
tick ~300 to 1600:

| | grain | flour | bread | operating_cost |
|---|---|---|---|---|
| price | 1 | 12 | 1 | 1 |

- **mill margin** `3·P_flour − P_grain − cost = 3·12 − 1 − 1 = +34` (milling very profitable).
- **bake margin** `3·P_bread − P_flour − cost = 3·1 − 12 − 1 = −10` (baking loses every cycle).
- Early transient: `P_bread` 62 → 3 → 1 and `P_flour` 1 → 3 → 12 over the first ~300 ticks, then
  frozen. The role-choice margin is `3·P_bread − P_flour − operating_cost` (yield 3 per input,
  `content.rs:80,90`; the appraisal at `phases.rs:2298`).

## 2. The central question and pre-named outcomes

**Q: Can the oven stage earn a non-negative role-choice margin, and does restoring it produce a
functioning chain (`FlowRuns`) on all five seeds?** Pre-named outcomes:

- **DEMAND-FIXES-IT** — giving bread real recurring demand (final good actually eaten) raises
  `P_bread` above the flour break-even (~`(P_flour + cost)/3 ≈ 4.3`), the bake margin turns
  positive, and the chain reaches `FlowRuns` on all five seeds. The final-demand precondition was
  the wall (the C-series' recurring lesson).
- **STALE-PRICE-FIXES-IT** — `P_flour = 12` is a *stale* early-boom artifact (bakers bid flour up
  when bread was 62, then stopped trading flour, and the realized price persists). The baker is
  rejected on a phantom input cost; age-gating the appraised input price (as capital formation
  already requires recent trades, `phases.rs:2767`) recovers the margin without changing demand.
- **BOTH-NEEDED** — demand and the stale-price appraisal each contribute; neither alone clears the
  five-seed gate.
- **DEEPER-WALL** — neither clears it: the chain does not sustain on ≥1 seed even with a positive
  bake margin, implicating a further constraint (flour supply, capacity/utilization, seed-fragile
  bootstrap). A real negative that re-scopes the succession line.

## 3. Phase 1 — resolve the two open measurements (do first, cheap)

Neither is inferred; measure both before choosing the fix:

1. **Is `P_flour = 12` a real clearing price or stale?** Trace flour *trades* (not just the
   realized price) over the run: does flour still clear after the ~tick-300 freeze, or is 12 a
   frozen last-trade the appraisal reads with no live market? Decides whether STALE-PRICE is in play.
2. **Is `P_bread = 1` demand-floored?** Confirm bread trades occur at 1 (real floor demand) and
   that the population's food is coming from forage/hearth, not bread — i.e. reducing the
   bread-substituting food raises bread demand rather than starving the colony.

## 4. Phase 2 — the fix (candidate levers; grounded in §1)

- **L1 — Final demand (recommended).** Make the population actually depend on bread instead of
  forage + hearth subsidy, so `P_bread` clears above the flour break-even and the bake margin is
  positive. This is the genetic precondition the last stage was missing. **Tension to respect:**
  C3R.b showed a *large* food subsidy floods demand and kills the chain; the lever here is the
  opposite end (less substitution so bread IS demanded) — it must raise bread demand without
  starving the colony (Phase-1 measurement #2 bounds this).
- **L2 — Stale-price appraisal fix (companion, only if Phase-1 #1 confirms staleness).** Age-gate
  the appraised input (flour) price in `recipe_adoption_pays_for_money` / `recipe_is_profitable`
  so a frozen early-boom price cannot reject a baker on a phantom cost. Mechanism fix, not economic.
- **L3 — Yield/cost rebalance (fallback).** Change the chain's yields, grain cost, or operating
  cost so milling does not capture the entire surplus. Riskier: it retunes the whole chain balance
  and would move the chain scenarios' goldens broadly; prefer L1/L2 first.

Recommended path: **Phase 1 → L1 (+ L2 if flour is stale)**; hold L3 as the fallback if
DEEPER-WALL.

## 5. Acceptance — the immortal five-seed viability gate

- **Gate (unblocks impl-71):** a functioning chain — `StructurePersistsUnderInheritance` +
  `FlowRuns` — on **all five** immortal `FlagOffHeritable` seeds `[3,7,11,19,23]`, not just seed 3,
  with the bake margin measured non-negative in steady state. Pin it with an asserting test.
- **Then** the mortal cells can be re-evaluated (they were a distinct *accepts-but-flow-fails*
  mode; a positive-margin chain may or may not survive mortality — that is impl-71's question,
  now on a substrate that actually functions).

## 6. Conservation & determinism

**This changes behavior**, so it is NOT byte-identical on the target scenario. Confine it: put the
fix behind a **new scenario/flag** (e.g. `frontier_mortal_producers_bread_demand`) or a gated
`ChainConfig`/`DemographyConfig` field defaulting to today's behavior, so **every existing golden
and digest is byte-identical** and only the new scenario's goldens are new. Any new
behavior-steering field is DIGESTED and classified in the digest-coverage guard (`digest.rs`);
conservation and the money identity are asserted per tick as today.

## 7. Risks

- **Fixing the margin need not fix sustain** (DEEPER-WALL). The five-seed gate is the honest bar;
  a positive margin that still dies on some seed is a finding, not a failure.
- **Demand vs starvation** (L1). Raising bread demand by cutting food substitution risks starving
  the colony; Phase-1 #2 bounds the safe range, and the conservation asserts catch mistakes.
- **Golden blast radius** (L3). A yield/cost retune would move many chain goldens; kept as fallback
  and, if used, scoped to a new scenario.

## 8. Falsifiable-bar summary

**Pass:** Phase 1 resolves both measurements with trade-level (not just realized-price) evidence;
the chosen lever yields a measured non-negative steady-state bake margin AND a `FlowRuns` chain on
all five immortal seeds, pinned by an asserting test, with all non-target goldens byte-identical.
**Fail:** claiming the fix from the realized-price snapshot without the flour-trade check (the
stale-price confound), moving unrelated goldens, or asserting sustain from a single seed.
