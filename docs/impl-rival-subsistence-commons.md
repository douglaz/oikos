# impl-47 — S23e: Finite rival subsistence commons (does scarcity of the outside option force the market S23d could not?)

Status (spec): **SPEC-READY** (Codex spec-review round 1 folded in — P0 `φ`/`D0` baseline-anchored predeclaration
+ P0 `ScarcityStarves` absolute-floor redefine, P1 `K` pinned + after-market S21h routing + owner-seller
attribution + `NoSurplusUnderMortality` sharpening, P2 denominators + diagnostic-only lifespan probe + tag
registry, P3 pool distinctness; see §9). Base: the **S23d branch** `feat/mortal-landowner-impl-rb`
@ `d965d35` (the mortal-landowner demography base — `DemographyBaseUnviable`/subsidy-bound — which itself
composes the S23c secure-tenure heir engine). This is the **lever S23d's finding points to**, scoped by Codex's
strategic review as "option A, mechanism a2: a **finite rival outside-option scarcity**, NOT a tuned floor."
S23e's job is to make the S23d base **viable as a market** by an independently-justified mechanism, so that the
deferred S23c generational tenure study finally has ground to run on — while refusing the tuning trap (do NOT
turn the survival floor down until a market appears; introduce a scarcity mechanism justified by ecology/mass
balance, with controls that fail where they should).

## 0. One-paragraph summary

S23d proved the mortal-landowner base is **subsistence-bound**: with an **unlimited** own-labor emergency
survival floor (`emergency_self_provision`), every agent feeds itself, so owners' cultivated grain is never
bought — no producer/buyer money economy forms (`floor_share = 1.000`, `buyer_bought = 0`), and the base is
`DemographyBaseUnviable` even though the demographic mechanism works (owners mortal, inheritance fires
endogenously). The single identified cause is that **self-provision is adequate**, so there is no gain from
trade. S23e removes exactly that adequacy in the **principled** way: it replaces the unlimited own-labor floor
with a **finite, regenerating, non-excludable RIVAL commons** — a shared subsistence stock with an ecological
**carrying capacity** `K` and regen `r`, which agents deplete by drawing on it. Private land now matters for an
economically real reason: **subsistence access is rivalrous** — when too many lean on the commons it runs down,
so some agents must depend on owner-produced surplus. The scarcity is set by **mass balance** (a single
predeclared fraction `φ` = the share of subsistence-seekers the commons can sustain at steady state), **swept**
across {abundant, marginal, scarce}, never searched for a passing verdict. The headline (scarce) may only be
called a success (`ScarcityForcesMarket`) if buyers **materially buy owner-produced bread** and survival
**improves over the owner-surplus-disabled control** — proving a real market formed, not a relabeled floor. Two
controls keep it honest: `abundant_outside_option` must *reproduce* the S23d subsidy-bound null, and
`scarce + owner-surplus-disabled` must *stay non-viable* (scarcity without supply is just starvation). And S23e
carries the caveat S23d could not retire: adult lifespans are short, so the run must be **instrumented** for
surplus-under-mortality (owner tenure-before-death, surplus-sold-before-death, inventory at death, inherited
capital) — if a scarcity-controlled base *still* cannot produce owner surplus before death, that is the honest
`NoSurplusUnderMortality` finding (the "option C" dead-end signal), not a base to keep tuning. Gated behind
`rival_subsistence_commons` (digest tag 21, ON-only; goldens byte-identical when off).

## 1. Why this milestone, why this lever — and why *scarcity*, not a floor tweak

S23d isolated the base's failure to a single mechanism: the **own-labor emergency survival floor is
unlimited**, so self-provision is always adequate and the market has nothing to clear. To make land tenure
testable, self-provision must become **insufficient** so that exchange becomes necessary. There are two ways to
do that, and the discipline forces the choice:

- **a1 — tighten the floor** (raise the hunger threshold / cut the ration until buyers must buy). This is a
  **raw knob dialed toward the answer**: no independent justification, and afterward "found a threshold" is
  indistinguishable from "fit the number until the market appeared." Forbidden by classify-not-tune.
- **a2 — a finite rival commons** (this milestone). Replace the *unlimited* floor with a **scarce, regenerating**
  outside option — a commons with carrying capacity that agents deplete. Exchange becomes necessary from the
  **mechanism** (rivalry in subsistence access), not from a tuned threshold; the scarcity band is set by
  **mass balance / ecological carrying capacity**, an axis justified on its own terms and swept, not searched.

The economic content is Ricardian/Malthusian and squarely Austrian: an **excludable** private factor (owned
land) acquires value precisely when the **non-excludable** outside option (the commons) becomes **rival and
scarce**. When the commons is abundant, private land is worthless-at-the-margin and the economy is a subsistence
commune (= the S23d null); when the commons is scarce, holding productive land — and buying its surplus — is the
only way to eat, and a producer/buyer market is *forced*. Whether that market actually forms under **mortal,
short-lived** owners is the open question S23d could not answer.

**Grounding (verified — the substrate exists):** OIKOS already has a finite regenerating forage substrate and
carrying-capacity machinery (S14 `forage_commons`, `forage_node`, `KnownGoods::subsistence`), and the S21h
emergency floor is a distinct `emergency_self_provision` path that mints unlimited own-labor subsistence
(`emergency_bread_provisioned()`). S23e's new surface is to route emergency subsistence through a **finite
rival commons pool** instead of the unlimited own-labor mint — reusing the existing conserved
regen/carrying-capacity concepts, gated and digested ON-only so all goldens are byte-identical when off.

## 2. The central question and pre-named outcomes

**Central question.** On the S23d mortal-landowner base, when the unlimited own-labor emergency floor is
replaced by a **finite, regenerating, non-excludable rival subsistence commons** whose carrying capacity is
predeclared by mass balance (`φ` = the fraction of subsistence-seekers it can sustain at steady state), does a
**producer/buyer money market form** in the scarce regime — non-owner buyers **materially buy owner-produced
bread**, the economy is **no longer subsistence-bound**, and survival **improves over the owner-surplus-disabled
control** — while every S23d demographic clause still holds (owners are mortal lineage reproducers, inheritance
fires endogenously, money promotes, conservation/registry hold) — AND do **mortal, short-lived owners actually
accumulate and sell a marketable surplus before death** (the instrumented test that this is a real market, not
scarcity-driven starvation or a relabeled floor)?

**The mass-balance scarcity predeclaration (the anti-smuggling crux, Codex P0).** The scarcity axis is defined
from the **flag-off S23d baseline throughput**, NOT from live (endogenous) per-agent need or population counts —
because the real emergency floor computes a *dynamic residual* need after market consumption / hunger depletion /
threshold / already-eaten food (`settlement.rs:14388`), and the subsistence-seeker set changes with deaths,
ownership, and role, so a `c·N` product would be gameable. Predeclare, measured on the **S23d flag-off** seed
spine `{3,7,11,19,23}`:
- `D0` = the mean **final-window `emergency_bread_provisioned` delta per econ tick** on the S23d flag-off base
  (the baseline subsistence throughput the unlimited floor supplies), computed per seed then averaged; pin the
  tick unit and integer rounding.
- `N0` = the distinct eligible emergency consumers with positive emergency need in that same baseline window
  (reported).
- `c_eff = D0 / N0` — **reported only**, a descriptive per-consumer draw; it is NOT used to set `r`.

The rival commons regenerates `r`/tick where **`r = φ · D0`** — `φ` directly scales the *measured baseline
throughput*, so `φ = 0.25` means the commons regenerates a quarter of the subsistence the unlimited floor was
providing. **`φ` is the single predeclared axis, SWEPT — never searched:** `φ ∈ { abundant = 1.25, marginal =
0.5, scarce = 0.25 }` (pinned `const`s). Headline = **scarce** (`φ = 0.25`). The buffer is pinned too:
**`K = K_TICKS · r` with `K_TICKS = 3`, `initial_stock = K`** (a three-tick larder), and **scarcity/verdict are
measured only in the post-warmup final window** so the initial larder cannot drive the outcome. Nothing here is
chosen to pass a verdict — `D0` is measured off the frozen S23d base and `φ` is swept.

**The rival draw (conservation-safe, after-market, S21h eligibility — Codex P1).** The draw runs at the existing
S21h emergency step (**after market clearing**), over the **same S21h-eligible residual-need set** (no broadened
eligibility, no pre-market allocation; owners draw only where S21h already would). Each eligible agent draws its
residual need, **total draw ≤ current stock** (rivalry); on shortfall, allocation is deterministic (pinned
order, e.g. by hunger then agent-id) and the unfed **stay hungry** — they must have bought owner bread earlier
in the tick, or go without. The commons is a tracked good with **regen as a named source** and **draw as a named
sink** — the per-tick conservation identity holds exactly (nothing minted beyond the declared regen).

**Primary success = `ScarcityForcesMarket`** (all; predeclare every threshold as a `const`, do NOT fit; measured
on the scarce headline vs the S23d base and vs the controls):
1. **Scarcity bites** — the commons is measurably depleted below `K` for a sustained window and ≥1 agent is
   left unfed by it (the outside option is genuinely rival and scarce, not decorative).
2. **A real producer/buyer market forms** — non-owner buyers materially buy (`final_buyer_bought ≥
   MATERIAL_BOUGHT_FLOOR`), and the economy is **not subsistence-bound**: `final_commons_share = (commons-drawn
   hunger-relief) / (total hunger-relief consumption)` in the final window, paired by seed, is `≤ MARKET_CAP`
   (the outside option is no longer the whole diet).
3. **Survival lifts over the owner-surplus-disabled control** — the final-window **survival fraction** (mean
   living count over the final window ÷ the seed's cohort) on the headline exceeds the `scarce_no_owner_surplus`
   control by ≥ `SURVIVAL_LIFT` — the *market*, not scarcity alone, is what keeps people alive. **Not** measured
   against the S23d unlimited-floor headcount (see `ScarcityStarves`, §finding-modes).
4. **Owners actually supply surplus under mortality, and buyers buy it** (Codex P1) — instrumented: mean owner
   **surplus-sold-before-death** ≥ `OWNER_SURPLUS_FLOOR`; **and** the material bought in clause 2 is
   **owner-produced with seller attribution** — bread bought by non-owners **from a current-or-ever landowner
   seller**, carrying `SelfProduced` provenance. (`seeded_minted == 0` rules out *seeded* supply but does NOT
   prove the seller was an owner; the seller-is-owner attribution does.) *(Separates a real land-dependent
   market from a fluke; see §4.)*
5. **Every S23d demographic clause still holds** — owners are lineage reproductive actors
   (`immortal_roster_owned_plot_ticks == 0` AND `non_lineage_owner_plot_ticks == 0` AND
   `owner_old_age_deaths > 0`), inheritance fires endogenously (`inherit_eligible_owner_deaths ≥ 20`, real heir
   transfers), born-in-sim agents own, money promotes and persists.
6. **Conservation / provenance / registry invariants hold every tick** — commons regen/draw conserved (no
   subsistence minted beyond declared regen), grain/bread/money conserve, ≤1 owner/plot, no orphaned claims.

**Finding modes (pre-named, first-class; the verdict test PRINTS the classification, does NOT assert SUCCESS):**
- `AbundanceReproducesNull` — the `abundant_outside_option` control (`φ ≈ 1.25`) reproduces S23d's
  subsidy-bound `DemographyBaseUnviable` (`commons_share → 1.0`, `buyer_bought ≈ 0`). **Required** — proves the
  scarcity is the operative change, not some other edit.
- `ScarcityStarves` (Codex P0 — an ABSOLUTE floor, not the S23d headcount) — scarcity kills without a market
  replacing the outside option: **`extinct`, OR final-window survival fraction below a predeclared absolute
  floor `SURVIVE_MIN`, OR no survival lift over `scarce_no_owner_surplus` while owner supply is immaterial.**
  Deliberately **NOT** "below the S23d unlimited-floor baseline" — a genuine scarce market may sustain a
  *smaller* population than the artificial unlimited-floor commune while still being viable and market-mediated,
  so the S23d headcount must never be an early failure gate. The `scarce_no_owner_surplus` control **must** land
  here (or non-viable) — proving you cannot fake a verdict by starving the outside option.
- `NoSurplusUnderMortality` — the sharp dead-end signal (Codex P1), requiring **ALL** of: commons **depleted**
  (scarcity bit), **unmet buyer demand** (buyers wanted to buy), owners **alive and owning long enough to
  attempt production** (`owner_tenure_before_death` ≥ `OWNER_ATTEMPT_MIN` — they had the chance), and **low
  lifetime `owner_surplus_sold_before_death`**. Report the sub-split: owners **produced surplus but died holding
  inventory** vs **never produced surplus** (both are the wall, but distinct diagnoses). The **"option C"
  dead-end**: short lifespans × weak intergenerational capital, which another lever will not fix.
- `SubsistenceBoundDespiteScarcity` — scarce, owners **did sell some** surplus (so NOT the mortality wall), but
  the market still does not clear to viability (buyers don't materially buy / survival doesn't lift): a partial
  negative, distinct from both `NoSurplusUnderMortality` (owners couldn't supply in time) and `ScarcityStarves`
  (mass death).
- `ScarcityForcesMarket` — all six success clauses; the base becomes viable as a market and S23c can re-run.
- Plus the base guards (checked first): `ConservationBroken` / `extinct` / `ImmortalOwnerResidue` /
  `NonLineageOwnerResidue` / `MoneyFailure`.

**Ordered classifier (top-down, first-match-wins — S21i non-gameability discipline):**
`ConservationBroken`/`extinct`(non-scarcity cause) → `ImmortalOwnerResidue`/`NonLineageOwnerResidue` →
`MoneyFailure` → `ScarcityStarves` (extinct / final-window survival fraction below absolute `SURVIVE_MIN` / no
lift over `scarce_no_owner_surplus` with immaterial owner supply — **never** the S23d headcount) →
`AbundanceReproducesNull` (abundant control only) → `NoSurplusUnderMortality` (commons depleted AND unmet buyer
demand AND `owner_tenure_before_death ≥ OWNER_ATTEMPT_MIN` AND `owner_surplus_sold_before_death` below floor) →
`SubsistenceBoundDespiteScarcity` (scarce, owners sold *some* surplus, market still doesn't clear) → **then the
explicit final gate:** `if ALL SIX success clauses pass { ScarcityForcesMarket } else { report the first failed
clause }`. Predeclare every threshold as a `const`; do not fit.

## 3. Engine design (additive, default-off, conservation-safe)

**One flag gates the mechanism:** `rival_subsistence_commons: bool` (default `false`). When `false`, the base is
untouched and all goldens are byte-identical. When `true` (composed with `mortal_landowner_demography = true` for
the headline), the **unlimited own-labor emergency floor is replaced by a finite rival commons**:
1. **A finite regenerating non-excludable commons pool.** A tracked subsistence stock with carrying cap `K` and
   regen `r`/tick (derived from the mass-balance `φ`/`D0` of §2; `initial_stock = K`). Non-excludable: any
   **S21h-eligible** hungry agent may draw; rival: the sum of draws in a tick cannot exceed the current stock.
   Draw allocation on shortfall is deterministic (pinned order). Regen is a named conservation **source**, draw
   a named **sink**. This **subsistence-commons pool is a distinct accounting object** from the S23c estate's
   revert-to-unowned "commons" (an ownership *state*, not a stock) — give it a distinct name in code (e.g.
   `subsistence_commons_stock`) and a distinct ledger channel, to avoid terminology/accounting confusion.
2. **The emergency subsistence path routes through the commons — after the market, same eligibility** (Codex
   P1). The S21h emergency step already runs **after market clearing** and immediately consumes the produced
   bread, so it never becomes offerable supply (`settlement.rs:14323`, `:14333`). The flag changes only its
   *source*: the **same S21h-eligible residual-need set** (after the market clears and hunger/threshold/
   already-eaten are netted out — no broadened eligibility, no pre-market allocation, owners draw only where
   S21h already would) draws from the finite commons pool instead of the unlimited own-labor mint. When the pool
   is depleted, the residual **stays hungry** (must have bought owner bread earlier in the tick, or goes
   without). No other S23d behavior changes.
3. **Private land and its market are unchanged from S23d** — the secure ownership substrate (forfeiture off),
   the S23a claim/harvest-gate/registry, the S23c heir engine. S23e adds *scarcity of the outside option*, not
   a new tenure rule. (Buyers buy owner bread through the existing money market that S21f/S16 provide.)

**Digest / canonicalization.** `rival_subsistence_commons` canonicalized **ON-only** under a new injective
**digest tag 21**. Verify the registry against the **landed S23d branch code** (`sim/src/settlement.rs`
`canonical_bytes`), which uses: 13 = S23a `private_land_tenure`, 14 = S23b `land_market`, 15 = S24a
`commitment_norm_spread`, 16 = S24b `abandonable_norm`, 17 = S24c `group_payoff_imitation`, 18 = S23c
`secure_land_tenure`, 19 = `fixed_commitment_norm`, 20 = S23d `mortal_landowner_demography` — so **21** is the
first free tag for `rival_subsistence_commons` (confirm no other push occupies 21 before building). Emit the
commons state (stock, `K`, `r`, `φ`) only when the flag is on; with it off, `canonical_bytes`/digest are
byte-identical to the S23d/base stream. The new mass-balance constants and instrumentation counters are test-visible telemetry, not
canonicalized behavior beyond the commons-state block.

**Conservation.** The per-tick identity gains the commons as a source(regen)/sink(draw) pair; assert it holds
every tick (no subsistence created beyond declared regen; the commons stock never goes negative; total draw ≤
stock). Grain/bread/money/land invariants unchanged from S23d.

## 4. Mortality/surplus instrumentation (Codex's requirement — separates success from the mortality wall)

New test-visible counters (telemetry, not canonicalized) so `ScarcityForcesMarket` cannot be confused with
`NoSurplusUnderMortality`:
- **`owner_age_at_first_claim`** (distribution) — how old owners are when they first hold land.
- **`owner_tenure_before_death`** (ticks owned before old-age death) — how long owners actually hold plots.
- **`owner_surplus_produced_minus_consumed`** per owner lifetime — did they produce more bread than they ate?
- **`owner_surplus_sold_before_death`** — of that surplus, how much was actually **sold** into the market before
  death (the success clause §2.4).
- **`owner_inventory_at_death`** and **`inherited_stock_to_heirs`** — unsold surplus / capital passed on (does
  intergenerational transfer compound, or is each short life a fresh start from zero?).
- **`buyer_purchases_by_owner_age_cohort`** — are purchases coming from mature owners (a working market) or
  never happening (the wall)?
- **`owner_seller_attributed_bought`** (success clause §2.4) — non-owner bread purchases whose **seller is a
  current-or-ever landowner** and whose provenance is `SelfProduced` (not merely `seeded_minted == 0`); this is
  the metric that proves the market is *land-dependent*, not incidental.

These make the difference between "scarcity forced a market" and "scarcity + short lifespans = owners die before
they can supply" **measurable**, not a matter of interpretation.

## 5. The new suite `sim/tests/rival_subsistence_commons.rs` (+ controls)

Deterministic, seed spine `{3, 7, 11, 19, 23}`. Tests:
- `goldens_unchanged` — every golden byte-identical with the flag off.
- `s23d_baseline_reproduced_for_D0` (Codex P3) — the **flag-off S23d base reproduces** on the seed spine and is
  the sole source of `D0`/`N0`; the derivation is asserted deterministic (same `D0` every run) so `r = φ·D0` is
  pinned, not drifting.
- `canonical_bytes_split_only_when_commons_active` — canonical bytes/digest diverge from the S23d/base stream
  **only** when `rival_subsistence_commons` is on (tag 21 injectivity).
- `commons_conserves_and_is_rival` — commons regen/draw conserved every tick, stock never negative, total draw ≤
  stock (rivalry holds).
- `scarce_headline_classifies` — on the scarce headline (`φ ≈ 0.25`), print + classify (`ScarcityForcesMarket`
  or the first failed clause); assert the classification, not a hand-set outcome.
- `owners_supply_surplus_or_not` — print the §4 instrumentation; assert nothing beyond guards (this is the
  measurement that distinguishes success from `NoSurplusUnderMortality`).
- `demographic_clauses_still_hold` — the S23d P0 owner-identity + inheritance-fires + money-promotes clauses
  still pass under the commons.
- **Controls (each isolates the lever / keeps it honest):**
  - `abundant_outside_option` (`φ ≈ 1.25`) → `AbundanceReproducesNull` (reproduces S23d subsidy-bound — proves
    scarcity is the operative change).
  - `scarce_no_owner_surplus` (`φ ≈ 0.25` + owner harvest capped at self-subsistence, no sellable surplus) →
    `ScarcityStarves` / non-viable (proves scarcity alone cannot fake a verdict — starving the outside option
    without supply is death, not a market).
  - `marginal_outside_option` (`φ ≈ 0.5`) → reported as the midpoint of the `φ` sweep (the axis is swept, not a
    single searched point).
- **Diagnostic-only probe (NOT a verdict axis — Codex P2).** `scarce_inherited_stock_diagnostic` (or a
  `scarce_long_lived_owner_diagnostic`): a single non-verdict variant that relaxes the suspected mortality
  bottleneck (seed a modest inherited subsistence/working stock to heirs, or lengthen adult lifespan) and
  **reports** whether owner surplus-sold-before-death then clears the floor. This exists ONLY to sharpen a
  `NoSurplusUnderMortality` diagnosis (is the wall really mortality/inheritance, or something else?) — it is
  **excluded from the classifier** and MUST NOT be used to rescue `ScarcityForcesMarket` on the headline (that
  would be tuning). Print its metrics; assert nothing on the verdict.

## 6. Verification (independent gate)

The orchestrator's verification + Codex review-of-results is the gate (never the rb-lite panel's "clean"):
- Full workspace green; new suite passes; `cargo fmt --check` + `cargo clippy --workspace --all-targets -- -D
  warnings` clean.
- **All goldens byte-identical** with the flag off (ON-only tag-21 discipline).
- The `φ` sweep {abundant, marginal, scarce} classifies as above; the two honesty controls
  (`abundant_outside_option`, `scarce_no_owner_surplus`) land where predeclared (the lever is shown operative +
  non-fakeable), and the §4 instrumentation is reported.
- Conservation / provenance / registry / commons-rivalry invariants asserted, not assumed.

## 7. Honesty, anti-smuggling, and the dead-end signal (state in the result)

- **Anti-smuggling (the point of a2).** The scarcity band `φ` is predeclared by mass balance and **swept**
  {abundant, marginal, scarce}; it is **never** searched for `ScarcityForcesMarket`. The `abundant_outside_option`
  control must reproduce the S23d null and the `scarce_no_owner_surplus` control must stay non-viable — if either
  fails to behave, the lever is not doing what it claims and the result is disqualified, not massaged.
- **A success is a market, not a relabel.** `ScarcityForcesMarket` requires buyers to **materially buy
  owner-produced bread** and survival to **lift over the owner-surplus-disabled control** — a scarce commons that
  merely reclassifies the same subsistence as "bought" without a real producer/buyer flow is **not** success.
- **The dead-end is a first-class outcome.** If the scarce base still cannot produce owner surplus before death
  (`NoSurplusUnderMortality`), that is the honest signal that the barrier is **structural** (short lifespans ×
  weak intergenerational capital), needing a deeper population/market-model change (Codex "option C"), and that
  another lever would be throwing good effort after bad — report it as the finding, do **not** tune `φ` or the
  floor to escape it.
- **Scope.** S23e tests whether *this* rival-commons mechanism, on *this* mortal-landowner base, forces a market;
  a genuinely dispersed multi-commons geography, or endogenous commons ownership, is future work.

## 8. Sequencing

1. **This milestone (S23e) lands on the S23d base.** If `ScarcityForcesMarket`, the base is viable as a market
   and its parameters are **frozen**.
2. **Then re-run S23c (impl-45) on the frozen viable base** with `mortal_landowner_demography = true` +
   `rival_subsistence_commons = true` + `secure_land_tenure = true` — the generational secure-tenure study
   (impartible vs partible, Malthusian land-per-capita) finally runs against endogenous owner deaths, real
   heirs, **and** a functioning land-dependent market. If instead S23e returns `NoSurplusUnderMortality` /
   `SubsistenceBoundDespiteScarcity`, S23c stays deferred and the honest next question is the deeper
   population-model change, not another lever.

## 9. Codex spec-review resolutions (round 1 — folded in; verdict was NOT-SPEC-READY, now addressed)

- **P0 — `φ = (r/c)/N` underdefined (endogenous `c`,`N`).** §2 now anchors the axis to the **flag-off S23d
  baseline**: `D0` = measured final-window `emergency_bread_provisioned`/tick, `N0`/`c_eff` reported only,
  **`r = φ·D0`**; `φ` swept {1.25, 0.5, 0.25}. Added the `s23d_baseline_reproduced_for_D0` guard test.
- **P0 — `ScarcityStarves` preempts a real scarce market.** Redefined to an **absolute** floor: `extinct` OR
  survival-fraction below `SURVIVE_MIN` OR no lift over `scarce_no_owner_surplus` with immaterial supply —
  **never** "below the S23d unlimited-floor headcount" (a viable scarce market may sustain a smaller
  population). Classifier order updated.
- **P1 — `K` gameable.** Pinned `K = K_TICKS·r`, `K_TICKS = 3`, `initial_stock = K`; verdict measured in the
  post-warmup final window.
- **P1 — routing must preserve S21h timing/eligibility.** §3.2 now: market clears first, then the **same
  S21h-eligible residual-need set** draws from the pool; no broadened eligibility, no pre-market allocation,
  owners draw only where S21h already would; unmet residual stays hungry.
- **P1 — owner-produced needs seller attribution.** Clause §2.4 + §4 `owner_seller_attributed_bought` now
  require a **current-or-ever-landowner seller** with `SelfProduced` provenance, not just `seeded_minted == 0`.
- **P1 — `NoSurplusUnderMortality` sharpened.** Requires commons depleted AND unmet buyer demand AND
  `owner_tenure_before_death ≥ OWNER_ATTEMPT_MIN` AND low surplus-sold, with the inventory-at-death vs
  never-produced sub-split; cleanly separated from `SubsistenceBoundDespiteScarcity` and `ScarcityStarves`.
- **P2 — denominators.** `final_commons_share` = commons-drawn / total hunger-relief (final window, per seed);
  survival lift = final-window survival fraction. **P2 — lifespan/capital probe** added as a *diagnostic-only*
  `scarce_inherited_stock_diagnostic`, excluded from the verdict (may not rescue success). **P2 — tag registry**
  made explicit and to-be-verified against the landed S23d branch (21 first free).
- **P3 — pool distinctness.** The `subsistence_commons_stock` is a distinct accounting object/ledger channel
  from the S23c estate revert-to-unowned "commons".
