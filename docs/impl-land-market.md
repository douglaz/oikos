# impl-40 — S23b: Post-Money Alienable Land Market (does illiquid priced land + budget hysteresis stabilize an occupation?)

Status (spec): DRAFT — pending Codex spec-review. Base: master `be2febb` (S23a landed). Second slice of the
**S23 private-property arc**. Composes on S23a (`private_land_tenure`) → S22a (`endogenous_cultivation_entry`)
on the population-scaled land base; the other S22 exit-cost levers (skill, profit-stay, capital, commitment)
are **OFF** in the headline so the land market is the only new exit-cost mechanism. Codex-scoped ("spec S23b —
post-money alienable land market with budget-constraint hysteresis").

## 0. One-paragraph summary

S22 + S23a established: occupation needs an **exit-cost institution**, and its **design matters** — S22f's
*binding voluntary fixed-term contract* stabilized a core, but S23a's *involuntary use-it-or-lose-it land
forfeiture* **thrashes** (claim→lost-on-idle→reclaimed, churn ~10× commons, no cohort) even with adequate
land. S23a tested a *harsh* tenure rule (lose land by idleness); it did **not** test the strongest, most
authentic property mechanism: **alienable, scarce, illiquid productive land priced *after money exists*.**
S23b adds exactly that — and a genuinely new stabilizing hypothesis, **budget-constraint hysteresis** (not a
contract, not forfeiture): pre-money, homesteading/open plot use is unchanged (so SALT still bootstraps from
`SelfProduced` bread + barter); **post-promotion only**, plots become **alienable assets bought and sold for
SALT** at an **endogenous price** (capitalized from each plot's *realized* grain yield / rent + local sale
history — never a hardcoded "expensive enough to stick" constant, which would be a fiat pin). Holding a plot
costs a disclosed **carrying cost** (a small SALT maintenance/tax); leaving cultivation means **selling**
(recovering SALT) or **letting title lapse** (forced sale) — *not* idle-forfeiture. Re-entry means **buying a
plot at the market price**, so a lapsed farmer who sold its land and spent the SALT on food **may not be able
to re-buy comparable land** — a *mechanical, budgetary* exit cost, no contract and no foresight required. The
hypothesis: this canonical illiquid-asset hysteresis stabilizes a **bounded owner-cultivator core** with a
surviving non-owner buyer cohort — the property arc's potential first SUCCESS. **The central trap (Codex):
the price must be ENDOGENOUS** (from realized rent / sale history), or the result is a tuned constant; a
battery of controls (land-market-off, pre-money-forbidden, free-rebuy/zero-price, non-excludable title,
no-carrying-cost, abundant-land, price-cap sensitivity) must show the stickiness comes from
*illiquid-priced-scarce-alienable land*, not a pin or a tuned price. Classify-not-tune; money + mortality +
provenance + conservation survive; goldens byte-identical off.

## 1. Why this milestone, why this lever — and the grounding

S23a proved private land is not *automatically* stabilizing and that a forfeiture rule actively thrashes. The
remaining, canonical property mechanism is the **illiquid alienable asset**: in the real world farming is
sticky partly because land is owned, scarce, *priced*, and costly to re-acquire once sold — the classic
budget-constrained, place-specific, illiquid wealth. S23b is the cleanest test of whether **that** (rather
than a contract or forfeiture) stabilizes occupation, via budget hysteresis.

**Grounding (S23a engine reused; only the market is new):**
- S23a already gives excludable owned grain plots (sim-side plot registry keyed by `NodeId`,
  claim/harvest-gate/inheritance, the population-scaled heterogeneous layout, the deterministic targeting).
  S23b **replaces the idle-forfeiture exit cost with a market exit cost** (sell / carrying-cost / forced
  sale) and adds buying/selling for SALT.
- The money system already promotes SALT pre-market from `SelfProduced` barter (S21f), so a land market that
  activates **only post-promotion** does not touch the bootstrap.
- Pairwise SALT clearing already exists (S20 two-lane book) — the land market reuses the same *pairwise,
  bilateral* discipline (sellers ask, buyers bid, clear pairwise), not a central auctioneer.

**Design decisions (Codex):** post-money money-land-market (NOT non-forfeiting title — a never-lose title with
no exercised exit cost risks a static pin); price **endogenous** from realized rent + sale history; the exit
cost is *budgetary* (sell + can't-rebuy), disclosed carrying cost, post-promotion activation only.

## 2. The central question and pre-named outcomes

**Central question.** Once money exists, when grain plots are **alienable** (bought/sold for SALT at an
**endogenous** capitalized-rent price), holding costs a disclosed **carrying cost**, leaving means **selling**
(not idle-forfeit), and re-entry means **re-buying at market price** — does **budget-constraint hysteresis**
(a lapsed farmer who sold + spent its SALT cannot re-buy) finally produce a **persistent bounded-minority
owner-cultivator cohort** with a **surviving non-owner buyer cohort**, while SALT promotes *before* the market
activates and money/mortality/provenance/conservation hold — AND is the stickiness genuinely from
illiquid-priced-scarce-alienable land (controls), not a fiat pin or a tuned price?

**Primary success = `LandMarketStickySuccess`** (all, across `SEEDS = {3,7,11,19,23}`, vs the matched-seed
**S23a/property-off baseline**):
1. **SALT promotes BEFORE land-market activation** (the market is post-promotion-gated; the bootstrap is
   untouched) and land trades occur **only post-promotion**.
2. **The market is non-vacuous + endogenous** — ≥ `MIN_LAND_TRADES` land trades clear post-promotion at
   prices that **track realized plot rent** (good plots trade dearer than marginal; price is not a constant),
   and ≥1 lapsed seller is later **priced out** of re-buying comparable land (the budget-hysteresis trace).
3. **Churn falls materially** — per-ever-cultivating churn ≤ `CHURN_DROP` (0.5) × the matched baseline.
4. **A persistent owner-cultivator cohort forms** — ≥ `PERSIST_COHORT` (4) distinct ids cultivate ≥
   `PERSIST_FRACTION` (0.5) of the final window **and are the plot-owners**.
5. **Bounded MINORITY ownership, open market** — owner share ≤ `OWNER_SHARE_MAX` (0.6); the market is liquid
   enough that non-owners *can* buy (trades occur) but illiquid/expensive enough that re-entry binds. Neither
   universal ownership (`HardBarrier`) nor a frictionless flip market (`LiquidChurn`).
6. **A surviving non-owner buyer cohort materially buys food** — post-promotion bought ≥ `MATERIAL_BOUGHT_FLOOR`,
   living.
7. **Money survives** — SALT remains money; food materially bought after promotion.
8. **Provenance clean** — sold/pre-promotion bread is `SelfProduced`; `seeded_minted == 0`. SALT paid for
   land is a transfer between agents (no mint); conservation + the plot-registry + SALT accounting hold.
9. **NOT downgraded by the controls (§4)** — free-rebuy/zero-price, non-excludable-title, no-carrying-cost,
   land-market-off, pre-money-forbidden each fail to reproduce the stickiness; the price-cap sensitivity shows
   the result is not a tuned price constant.

**Finding modes (pre-named; first-class; verdict prints, does NOT assert SUCCESS):**
- `LandMarketInert` (precondition fail) — titles trade rarely / prices don't bind behaviour (no real market).
- `MoneyFailureFromLandMarket` — the land-market machinery disrupts the money bootstrap; SALT fails/demonetizes.
- `ConservationBroken` / `extinct` — any conservation / registry / SALT-accounting break, or colony death.
- `LiquidChurn` — land trades frequently but no persistent cohort forms (the market is *too* liquid; re-entry
  doesn't bind — budget hysteresis absent).
- `LandMonopolyCull` — owners concentrate land (owner grain/land share ≥ `MONO_SHARE` = 0.75) AND the buyer
  cohort collapses.
- `HardBarrier` — re-entry becomes impossible for too many agents (no affordable land; near-universal owner
  share or a closed market).
- `NoStickinessDespiteLandMarket` — the market bites (trades, prices track rent, some hysteresis) but churn
  stays > the bar AND no persistent owner cohort.
- `LandMarketStickySuccess` — all nine success clauses, not downgraded.

**Ordered classifier (top-down, first-match-wins):** `LandMarketInert` → `MoneyFailureFromLandMarket` →
`ConservationBroken`/`extinct` → `HardBarrier` → `LandMonopolyCull` → `LiquidChurn` → **then the explicit
final gate:** `if ALL NINE success clauses pass { LandMarketStickySuccess } else
{ NoStickinessDespiteLandMarket }`. Predeclare every threshold as a `const`; do NOT fit.

## 3. Engine design (additive, default-off, conservation-safe)

1. **NEW default-off flag** `ChainConfig::land_market: bool` + fields: `land_carrying_cost` (per-period SALT
   maintenance/tax on a held plot), `land_price_cap_factor` (the capitalization factor turning realized rent
   into a price — a discount-rate analogue, NOT a price level), and the control toggles (§4). Helper
   `land_market_active(&self)` = flag on AND `private_land_tenure_active()`. Canonicalize ON-only with the
   **next free flag-digest tag (14** unless master advanced) + these fields + the per-agent/per-plot market
   state that steers behaviour. Off ⇒ byte-identical.

2. **Post-promotion activation gate (Codex — bootstrap-safe).** The land market is INERT until
   `current_money_good() == Some(SALT)`. Pre-money: S23a homesteading/open plot use is **unchanged** (plots
   claimed by labor, `SelfProduced` bread barters, SALT promotes). Land buying/selling/carrying-cost all begin
   only post-promotion. (`pre_money_land_market_forbidden` control asserts no land trade before promotion.)

3. **Endogenous price (THE anti-fiat crux, Codex).** A plot's price is **capitalized realized rent**:
   `price(plot) = land_price_cap_factor × recent_realized_yield(plot)` where `recent_realized_yield` is the
   plot's own grain harvested over a rolling window (its rent proxy), blended with its **local sale history**
   (the last cleared price for nearby plots). Good (high-regen) plots therefore price dearer than marginal
   ones, and the price LEVEL is set by realized productivity, **not** a constant. A **seller's ask** = the
   capitalized rent of its plot; a **buyer's bid** = its own capitalized expected rent (what the plot would
   yield it) bounded by its SALT on hand. Trades clear **pairwise** (bid ≥ ask) at the midpoint (or ask),
   SALT transferred buyer→seller, title transferred seller→buyer — a conserved transfer, no mint. (The
   `price_cap_sensitivity` control sweeps `land_price_cap_factor` to show the result is not a tuned constant;
   a fixed-nominal-price variant, if needed for a first slice, is classified DIAGNOSTIC, not headline.)

4. **Carrying cost + the exit cost = SELL or LAPSE (replaces idle-forfeiture).** Under the market, S23a's
   idle-forfeiture is OFF; instead a held plot incurs `land_carrying_cost` SALT per period (booked as a
   transfer to the commons/exchange, conserved). An owner that **leaves cultivation** may **sell** its plot
   (list an ask; recover SALT) or keep paying the carrying cost; an owner that **cannot pay** the carrying
   cost has its plot **force-sold/foreclosed** (listed at a discounted ask). Leaving therefore converts the
   plot back to SALT (minus market loss), not a free re-grab.

5. **Budget hysteresis (the stabilizer).** Re-entry requires **buying** a plot at the market price. A lapsed
   farmer that sold its plot and spent the SALT on food has too little SALT to re-buy comparable land → it is
   priced out (must save up, or take only cheap marginal land if affordable). This is the mechanical exit cost
   — no contract, no foresight. Record a **budget-hysteresis trace**: lapsed sellers' re-buy attempts that
   fail for lack of SALT vs stayers who keep their land.

6. **Everything else is S23a/S22a unchanged** — the hunger-gated cultivate entry/exit, the plot registry,
   excludable owner-only harvest, the heterogeneous population-scaled layout, deterministic targeting,
   inheritance. NO fiat "owners must cultivate", NO hardcoded price level, NO `Vocation` mutation. Per-agent
   SALT + per-plot price/owner state that steers behaviour is serialized ON-only under tag 14.

7. **Diagnostics (runtime-only):** land trades (count, prices, buyer/seller ids); price-vs-plot-rent
   correlation (endogeneity check); carrying-cost paid / foreclosures; owner share + owner ∩ persistent
   cohort; budget-hysteresis trace (priced-out re-buyers vs stayers); non-owner buyer cohort + post-promo
   bought; churn vs matched baseline; pre-vs-post-promotion trade timing.

## 4. The new suite `sim/tests/land_market.rs`

- **MANDATORY NON-VACUITY + ENDOGENEITY + POST-MONEY TEST** (else `LandMarketInert` / `MoneyFailureFromLandMarket`):
  ≥ `MIN_LAND_TRADES` post-promotion land trades clear; **no land trade occurs pre-promotion**; prices
  **track realized plot rent** (good plots dearer than marginal — assert a positive price↔rent relationship,
  not a constant); and ≥1 lapsed seller is later priced out of re-buying (budget-hysteresis trace).
- **The ordered classifier (§2)**, printed `--nocapture`; verdict prints + deciding metrics; does NOT assert SUCCESS.
- **Scenario:** `frontier_land_market` (HEADLINE) = the S23a population-scaled base (S22a on; skill/profit-stay/
  capital/commitment OFF; idle-forfeiture OFF) + `land_market = true` + a disclosed carrying cost + the
  endogenous-price formation. Matched baseline = the same with `land_market = false` (= S23a/property-off).
- **Controls (each a test; one variable each):**
  - **land_market_off** (`land_market = false`) = the S23a/property baseline (the matched churn baseline).
  - **pre_money_land_market_forbidden** — assert zero land trades before SALT promotes (bootstrap/anti-circularity).
  - **free_rebuy / zero_price** (`land_price_cap_factor = 0` / re-buy at no cost): no budget hysteresis ⇒ must
    NOT produce stickiness (proves it's the *illiquid price*, not owning).
  - **non_excludable_title** (ownership recorded, harvest gate off): title alone doesn't stabilize.
  - **no_carrying_cost** (`land_carrying_cost = 0`, no forced sale): separates "owning priced title" from the
    illiquid budget constraint.
  - **abundant_good_land** (good plots ≥ population): weak prices ⇒ weak stickiness.
  - **price_cap_sensitivity** — sweep `land_price_cap_factor`; the verdict must be **outcome-driving** with it
    AND the result must NOT hinge on one tuned value (if it only "works" at a single cap, that's a tuned-price
    diagnostic, not a finding).
- **HARD GUARDS every run + cell:** conservation every tick (SALT paid for land + carrying cost are transfers,
  never mints); `bread_minted_max == 0`; provenance clean-or-disqualified; `!extinct`; the plot-registry
  invariant (≤1 owner; claim/buy/sell/inherit/foreclose preserve the finite plot set; no dead-owner plots);
  the SALT-accounting invariant (land payments + carrying costs conserve total SALT); post-promotion-only
  trade guard.
- **goldens_unchanged** test pinning the five tripwire digests (copy from `private_land_tenure.rs`).
- **Robustness mini-sweep** over `land_carrying_cost` + `land_price_cap_factor` + total-land + grain flow,
  classified, no tuning; price-cap + carrying-cost axes MUST be outcome-driving.

## 5. Verification (independent gate)

Redirect cargo to files; never pipe to head/grep (EPIPE → spurious exit 101).
- `cargo test -p sim --test land_market` passes (non-vacuity/endogeneity/post-money + the controls).
- `cargo test --workspace` passes; **all existing goldens byte-identical** (`goldens_unchanged` +
  private_land_tenure / voluntary_cultivation_commitment / endowed_inherited_capital / durable_cultivation_capital
  / profit_driven_retention / occupational_stickiness / endogenous_cultivation_entry / robustness_appendix /
  household_barter / mortality / open_colony_mortality / demand_survival_bridge).
- `cargo fmt --check` clean; `cargo clippy --workspace --all-targets -- -D warnings` clean; conservation holds.

## 6. Honesty and scope (state these in the result; do not let a SUCCESS overclaim)

- **A SUCCESS = "illiquid priced alienable land + budget hysteresis stabilizes an occupation"** — the
  canonical property mechanism — joining S22f (contract) as a *second* working exit-cost institution, with
  S23a (forfeiture) as the failure that shows design matters. If it instead `LiquidChurn`s or monopolizes,
  that is the honest finding.
- **The price MUST be endogenous** — the whole result is void if the price is a tuned constant; the
  `price_cap_sensitivity` control + the price↔rent endogeneity assertion are load-bearing.
- **Post-promotion-only activation** keeps the money bootstrap clean; assert no pre-money land trade.
- **Bounded to this WOOD-poor, mortality-on, population-scaled regime** + this capitalized-rent pricing; like
  S21h/i expect possible band-qualification — report the carrying-cost / cap-factor windows where it holds.
- Follow repo conventions; NEVER add Claude/AI/assistant references in code, comments, or committed text.
