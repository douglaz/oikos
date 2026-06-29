# impl-40 — S23b: Post-Money Alienable Land Market (does illiquid priced land + budget hysteresis stabilize an occupation?)

Status (spec): SPEC-READY (Codex spec-review round 1: 5 P1 + 3 P2; round 2: 3 P1 + 1 P2, all folded in §7; Codex pre-approved SPEC-READY after the round-2 fixes). Base: master `be2febb` (S23a landed). Second slice of the
**S23 private-property arc**. Composes on S23a (`private_land_tenure`) → S22a (`endogenous_cultivation_entry`)
on the population-scaled land base; the other S22 exit-cost levers (skill, profit-stay, capital, commitment)
are **OFF** in the headline so the land market is the only new exit-cost mechanism. Codex-scoped ("spec S23b —
post-money alienable land market with budget-constraint hysteresis").

Status (implementation): LANDED. With agent-local buyer eligibility (a buyer must itself be
cultivating-or-attempting — not merely admitted because some plot is listed), the sustained-zero
`HardBarrier` gate (max affordable-listed over the final window, per §2's "through the final window"),
12-econ-tick idle listings, and a priced-out trace that excludes the contested listed plot as its own
stayer, the verdict classifier prints `{3: LandMarketInert, 7: LandMarketInert, 11: LandMarketInert,
19: LandMarketInert, 23: LandMarketInert}`: no headline `LandMarketStickySuccess`. The aggregate
non-vacuity, endogeneity (good plots trade dearer than marginal by ≥ `PRICE_RENT_GAP_BPS`),
post-money-only (trades and carrying charges), conservation, SALT-accounting, registry, and control guards
all pass; old-age deaths settle land tenure under the active market so the registry invariant holds every
tick. Property remains a non-success in this regime — the canonical illiquid-priced-alienable-land lever
does not stabilize an occupation here.

**Codex review-of-results: PASS-WITH-CAVEATS (keep `LandMarketInert`, honestly caveated).** The rb-lite panel
did NOT substantively review this run — a reviewer-config bug (the local, gitignored reviewers file carried a
stale S22d checklist) left reviewer 2 reviewing the wrong target while reviewer 1 (generic `codex review`) was
clean; the multi-round churn chased the misconfigured reviewer, so the run was stopped at its round-4
stabilized (codex-reviewer-clean) state and gated on independent verification + Codex review-of-results. Two
honesty caveats from that review: **(1) per-seed non-vacuity FAILS** — every headline seed clears only 5–7
land trades (< `MIN_LAND_TRADES`=8), so `LandMarketInert` is the *correct, non-post-hoc* verdict (lowering the
bar after seeing 5–7 would be tuning); the "non-vacuity passes" above is **aggregate endogeneity/thin-market
evidence** (prices track rent, foreclosures fire, priced-out traces exist), NOT per-seed market activation —
the market is *physically real but too thin to be the load-bearing institution*. **(2) Scope/confound:** the
`land_market_off` baseline on this population-scaled S23a base is *already* owner-dominant + buyer-thin
(buyers≈1, owners≈95% grain), so S23b tests whether a land market can **rescue an already owner-collapsed
private-tenure regime** (it cannot) — narrower than "land markets fail generally"; it does NOT test a market
in a *functioning two-tier* colony. `buyers=0` / `owner_grain_bps≈98%` are the disclosed regime, not a
conservation/degeneracy bug. The honest finding stands beside S22f: **private-land institutions so far either
THRASH under forfeiture (S23a) or go too THIN over owner-dominance (S23b); only the voluntary fixed-term
contract (S22f) preserved a two-tier market.** A clean test of a land market over a non-collapsed two-tier
base is possible future work, not a re-run of S23b.

Round-3 review hardening (verdict unchanged): (1) post-promotion the homestead path is now closed in
`validate_harvest_tasks` itself — a would-be homesteader heading to an unowned plot is rerouted/idled
*before* `world.tick`, so a non-owner can no longer extract unowned grain for free instead of buying
listed title (the post-claim guard remains as the load-bearing suppression under the `harvest_gate`-off
`non_excludable_title` control, where validation is skipped); (2) the `§3.6` budget-hysteresis trace is
decoupled from the bid — a buyer that can afford only a cheaper marginal listing while being budget-outbid
on a comparable-or-better one now records that priced-out event (best-land-first, still gated on a live
comparable stayer), capturing the "can only re-buy worse land" case the metric exists to detect.

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
4. **A persistent, MARKET-stabilized owner-cultivator cohort forms** — ≥ `PERSIST_COHORT` (4) distinct ids
   cultivate ≥ `PERSIST_FRACTION` (0.5) of the final window **and are the plot-owners**, **and each has
   market-relevant title history** (bought its plot, or retained it through priced-out re-entry pressure, or
   paid carrying costs through the final window — NOT merely an original homesteader/heir sitting on title;
   §3.7). The title-share breakdown `{original-claim, inherited, bought, foreclosed-out}` is reported.
5. **Bounded MINORITY ownership, open market** — owner share ≤ `OWNER_SHARE_MAX` (0.6); the market is liquid
   enough that non-owners *can* buy (trades occur) but illiquid/expensive enough that re-entry binds. Neither
   universal ownership (`HardBarrier`) nor a frictionless flip market (`LiquidChurn`).
6. **A surviving non-owner buyer cohort materially buys food** — post-promotion bought ≥ `MATERIAL_BOUGHT_FLOOR`,
   living.
7. **Money survives** — SALT remains money; food materially bought after promotion.
8. **Provenance clean** — sold/pre-promotion bread is `SelfProduced`; `seeded_minted == 0`. SALT paid for
   land is a transfer between agents (no mint); conservation + the plot-registry + SALT accounting hold.
9. **NOT downgraded by the controls (§4)** — free-rebuy/zero-price, non-excludable-title, abundant-good-land,
   land-market-off, pre-money-forbidden each fail to reproduce the stickiness; the price-cap sensitivity holds
   at ≥1 adjacent cap value (not a tuned constant → not `TunedPriceDiagnostic`). (no-carrying-cost is a
   reported sensitivity, not a gate.)

**Finding modes (pre-named; first-class; verdict prints, does NOT assert SUCCESS):**
- `LandMarketInert` (precondition fail) — titles trade rarely / prices don't bind behaviour (no real market).
- `MoneyFailureFromLandMarket` — the land-market machinery disrupts the money bootstrap; SALT fails/demonetizes.
- `ConservationBroken` / `extinct` — any conservation / registry / SALT-accounting break, or colony death.
- `LiquidChurn` — `land_trades ≥ LIQUID_CHURN_TRADES` (high turnover) AND churn > `CHURN_DROP × baseline`
  AND no persistent cohort (the market is *too* liquid; re-entry doesn't bind — budget hysteresis absent).
- `LandMonopolyCull` — owner land/grain share ≥ `MONO_SHARE` (0.75) AND the buyer cohort collapses
  (`final_buyer_cohort < MIN_BUYER_COHORT` / post-promo bought < `MATERIAL_BOUGHT_FLOOR`).
- `HardBarrier` — re-entry impossible for non-owners: `affordable_listed_plots_for_nonowners == 0` through the
  final window (no plot any eligible non-owner can afford), OR owner share → universal — and the buyer cohort
  cannot access land at all.
- `NoStickinessDespiteLandMarket` — the market bites (trades clear, prices track rent, some priced-out
  hysteresis) but churn stays > the bar AND no persistent market-stabilized cohort.
- `TunedPriceDiagnostic` — SUCCESS-like at exactly one `land_price_cap_factor` but not at any adjacent swept
  value (the result is a tuned price constant, not a robust finding). Never a headline success.
- `LandMarketStickySuccess` — all nine success clauses, not downgraded, and robust across ≥1 adjacent cap value.

**Pinned thresholds (predeclared consts; do NOT fit):** `MIN_LAND_TRADES = 8` (post-promotion, for
non-vacuity), `LIQUID_CHURN_TRADES = 200`, `MONO_SHARE = 0.75` (bps 7500), `OWNER_SHARE_MAX = 0.6`,
`MIN_BUYER_COHORT = 2`, `MATERIAL_BOUGHT_FLOOR` (shared), price↔rent endogeneity = a **strictly positive**
rank relationship (good-plot mean price > marginal-plot mean price by ≥ `PRICE_RENT_GAP_BPS = 2000`), and
`priced_out` per §3.6.

**Ordered classifier (top-down, first-match-wins):** `LandMarketInert` → `MoneyFailureFromLandMarket` →
`ConservationBroken`/`extinct` → `HardBarrier` → `LandMonopolyCull` → `LiquidChurn` → `TunedPriceDiagnostic`
→ **then the explicit final gate:** `if ALL NINE success clauses pass { LandMarketStickySuccess } else
{ NoStickinessDespiteLandMarket }`. Predeclare every threshold as a `const`; do NOT fit.

## 3. Engine design (additive, default-off, conservation-safe)

1. **NEW default-off flag** `ChainConfig::land_market: bool` + pinned fields/consts:
   `land_carrying_cost: u64` (SALT per `LAND_CARRYING_PERIOD` econ ticks on a held plot), `LAND_CARRYING_PERIOD`
   (default 12), `land_price_cap_factor: u64` (rent→price capitalization, a discount-rate analogue), the rent
   window `LAND_RENT_WINDOW` (default `ROLLING_WINDOW`=100), `LAND_MIN_RENT_HISTORY` (default 8 ticks of
   realized yield before realized rent replaces the quality prior), `LAND_SALE_HISTORY_WEIGHT` (default 0.5),
   `LAND_SALE_HISTORY_K` (nearest-K plots for the local blend, default 3), `LAND_LIST_IDLE` (idle econ ticks
   before an owner auto-lists, default `LAND_CARRYING_PERIOD`), `LAND_FORECLOSE_DISCOUNT_BPS` (default 2000),
   `LAND_PRICE_MIN`=1, plus the control toggles (§4). Helper `land_market_active(&self)` = flag on AND
   `private_land_tenure_active()`. Canonicalize ON-only with the **next free flag-digest tag (14** unless master
   advanced) + these fields + the per-plot market state (price, listing, last-sale) + per-agent SALT that steers
   behaviour. Off ⇒ byte-identical.

2. **Idle-forfeiture OFF from tick 0 (Codex P1.3 — resolves the contradiction).** Under `land_market`, S23a's
   `forfeit_on_idle` is **disabled from tick 0** (NOT only post-money), so the pre-money phase is *claim +
   owner-exclusive harvest, no forfeiture* — homesteading/`SelfProduced` barter still bootstraps SALT, but the
   pre-money phase does NOT inherit S23a's thrash. The land *market* (buy/sell/carrying-cost/foreclosure)
   activates **only post-promotion** (`current_money_good() == Some(SALT)`); a `pre_money_forbidden` control
   asserts zero land trades/charges before promotion.

3. **Endogenous price (THE anti-fiat crux, Codex P1.1 — fully pinned).**
   `rent(plot)` = the plot's **rolling realized grain yield** (grain actually harvested from it) over
   `LAND_RENT_WINDOW`, per period; until the plot has ≥ `LAND_MIN_RENT_HISTORY` realized-yield ticks, use a
   **quality prior** `prior(plot) = f(regen, cap, distance)` (e.g. `regen × cap_weight / (1 + distance)`),
   then switch to realized rent. `base_price(plot) = land_price_cap_factor × rent(plot)`. The **listed price**
   blends in **local sale history**: `price = round((1 − w)·base_price + w·mean_last_sale(nearest
   LAND_SALE_HISTORY_K plots))` with `w = LAND_SALE_HISTORY_WEIGHT` (0 if no local sales yet), clamped to
   `[LAND_PRICE_MIN, max(LAND_PRICE_MIN, base_price × 4)]` (Codex round-2 P2: the upper bound never falls below
   the floor even if realized rent → 0; the quality prior also floors the rent basis so a productive plot never
   prices at zero). Good (high-regen) plots therefore price strictly dearer than marginal
   ones and the price **level scales with realized productivity** — `land_price_cap_factor` is a slope, not a
   level. **Anti-tuning bar:** a SUCCESS must hold at the shipped `land_price_cap_factor` AND ≥1 adjacent
   swept value; if it holds at only one cap value → `TunedPriceDiagnostic`, not a headline success.

4. **Deterministic listing / bidding / matching (Codex P1.2 + P2.3 — the institution).**
   - **List (seller) + lifecycle (Codex round-2 P1.2):** an owner lists its plot (ask = its `price`) iff
     post-money AND (idle: it has not worked the plot for `LAND_LIST_IDLE` ticks **or** arrears: it could not
     pay this period's carrying cost). At most one live ask per plot; one owner per plot. **Lifecycle:** an
     *idle* listing is **recomputed each market sweep and cancels** once the owner works the plot/pays normally
     again; an *arrears/foreclosure* listing **persists until it sells or the arrears are paid**; a listed
     owner **may keep harvesting** its plot, but **a cleared sale always transfers title** (selling wins over
     holding).
   - **Bid (buyer):** an eligible buyer is a **non-owner, S22a-eligible, alive, post-money** agent that is
     cultivating-or-attempting and holds SALT; it bids on the **nearest** listed plot it can afford. Its
     **reservation = the plot's fundamental value `land_price_cap_factor × rent(plot)`** — the SAME
     rent/quality-prior basis as the ask's `base_price`, **ignoring the sale-history premium** (so a buyer pays
     up to fundamental value but not a bubbly sale-history markup); `bid = min(SALT, reservation)`. It clears
     iff `bid ≥ ask` (the listed price); affordable fundamental-value plots therefore clear, while a listed
     price inflated by sale history above fundamental does not (no systematic `LandMarketInert`, no instant
     flips). One live bid per agent; an agent that already owns a plot does not bid (one-plot-per-agent). The
     ask-vs-bid gap is reported.
   - **Match:** a single **deterministic sorted global sweep** (by `(plot price, plot node_id)`, then bidders
     by `(−bid, agent_id)`), **pairwise** (one buyer ↔ one seller ↔ one plot), clearing when `bid ≥ ask` at
     the **ask** price. SALT transfers buyer→seller; title transfers seller→buyer. NO multilateral/ring
     clearing. Reservations recomputed each sweep (no extra digest surface).

5. **Carrying cost + foreclosure as CONSERVED transfers (Codex P1.4 + P1.5; round-2 P1.3).** Each
   `LAND_CARRYING_PERIOD`, every plot owner pays `land_carrying_cost` SALT into an explicit non-agent
   settlement account **`land_fee_pool_salt`** — a conserved **sink**, digested ON-only, included in the
   SALT-accounting invariant, and **NOT spendable or redistributed in S23b** (any redistribution is deferred to
   a later milestone so it cannot change behaviour here). Payment ordering: charged before the market sweep. An
   owner that **cannot pay** auto-lists at a **foreclosure discount** (`price × (1 − LAND_FORECLOSE_DISCOUNT_BPS)`)
   but **keeps title and may keep harvesting** until a buyer clears; **if no buyer clears, the plot stays with
   the owner, listed, re-priced down each period to `LAND_PRICE_MIN`** — it is **never** silently converted to
   unowned (that would be a forfeiture variant; avoided so the carrying cost is a *budgetary* pressure, not a
   relabeled forfeiture). The carrying cost is **part of the institution** (it is what converts "stop
   cultivating" into an actual sale, creating the budget event), so SUCCESS is scoped as *priced alienability
   **plus** carrying-cost pressure*; `no_carrying_cost` is therefore a **SENSITIVITY** (reported), not a
   must-fail control (Codex P1.5).

6. **Budget hysteresis (the stabilizer).** Re-entry requires **buying** at the market price; a lapsed farmer
   that sold its plot and spent the SALT on food is **priced out** — recorded precisely (P2.1 def): *an agent
   attempted to bid on a comparable-or-better plot, its bid < the ask solely because its SALT on hand was too
   low, while a stayer retained comparable land in the same window*. The trace counts priced-out re-buyers vs
   stayers. This is the mechanical exit cost — no contract, no foresight.

7. **Cohort must be MARKET-stabilized, not inherited/static (Codex P2.2).** Because S23a already has
   claim+inheritance, the persistent owner-cultivator cohort (§2.4) must include **market-relevant title
   history** — each cohort id must have, in the final window, either **bought** its plot, or **retained it
   through priced-out re-entry pressure**, or **paid carrying costs through the final window** — not merely be
   an original homesteader/heir sitting on title. Report the title-share breakdown:
   `{original-claim, inherited, bought, foreclosed-out}`.

8. **Everything else is S23a/S22a unchanged** — hunger-gated cultivate entry/exit, plot registry, owner-only
   harvest, heterogeneous population-scaled layout, deterministic targeting, inheritance. NO fiat "owners must
   cultivate", NO hardcoded price level, NO `Vocation` mutation. Per-agent SALT + per-plot market state
   serialized ON-only under tag 14.

9. **Diagnostics (runtime-only):** land trades (count, prices, buyer/seller ids, tick); **price↔plot-rent
   correlation** (endogeneity check) + good-vs-marginal mean price; carrying paid / foreclosure listings /
   no-clear re-prices; `land_fee_pool` balance (conservation); owner share + owner∩persistent-cohort + the
   title-share breakdown (§7); budget-hysteresis trace (priced-out re-buyers vs stayers); non-owner buyer
   cohort + post-promo bought; churn vs matched baseline; pre-vs-post-promotion trade timing.

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
  - **abundant_good_land** (good plots ≥ population): weak prices ⇒ weak stickiness.
  - **price_cap_sensitivity** (REQUIRED falsifier, Codex P1.1) — sweep `land_price_cap_factor`; the verdict
    must be **outcome-driving** with it AND a SUCCESS must hold at ≥1 adjacent value, else `TunedPriceDiagnostic`.
  - **no_carrying_cost** (`land_carrying_cost = 0`) — a **SENSITIVITY, not a must-fail** (Codex P1.5): the
    hypothesis is *priced alienability **plus** carrying-cost pressure* (the carrying cost is what converts
    "stop cultivating" into an actual sale). Reported to show how much the carrying-cost pressure contributes;
    it does NOT gate the verdict (price hysteresis from sale + re-buy could be real without it).
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

## 7. Codex spec-review resolutions (round 1)

- **P1.1 price under-specified / anti-fiat** — §3.3: pinned `rent(plot)` = rolling realized grain yield over
  `LAND_RENT_WINDOW` (quality prior from regen/cap/distance until `LAND_MIN_RENT_HISTORY`), `base_price =
  cap_factor × rent`, blended with local sale history (`LAND_SALE_HISTORY_WEIGHT`/`_K`), clamped/rounded;
  `cap_factor` is a slope not a level; SUCCESS must hold at ≥1 adjacent swept cap value else
  `TunedPriceDiagnostic` (§2 mode + the `price_cap_sensitivity` falsifier).
- **P1.2 listing/bidding/matching missing** — §3.4: deterministic list rule (idle `LAND_LIST_IDLE` or
  can't-pay), buyer eligibility (non-owner/S22a-eligible/post-money/cultivating-or-attempting/has-SALT, nearest
  affordable listed plot), one-plot-per-agent/one-owner-per-plot, single sorted pairwise sweep clearing at ask.
- **P1.3 idle-forfeiture vs pre-money-unchanged contradiction** — §3.2: `forfeit_on_idle` OFF **from tick 0**
  (pre-money = claim + owner-exclusive harvest, NO forfeiture, so no inherited thrash); the *market* activates
  post-promotion only; `pre_money_forbidden` control asserts it.
- **P1.4 foreclosure not a conserved transfer** — §3.5: carrying cost → explicit conserved `land_fee_pool`
  (in the SALT invariant); can't-pay → auto-list at a foreclosure discount but **title stays with owner +
  may keep harvesting**; no-buyer → stays listed, re-priced to `LAND_PRICE_MIN`, **never silently unowned**
  (so it is not a relabeled forfeiture).
- **P1.5 no-carrying-cost overstated** — §3.5/§4: the hypothesis is *priced alienability + carrying-cost
  pressure*; `no_carrying_cost` is now a reported **SENSITIVITY**, not a must-fail control.
- **P2.1 classifier thresholds not pinned** — §2: pinned `MIN_LAND_TRADES`/`LIQUID_CHURN_TRADES`/`MONO_SHARE`/
  `OWNER_SHARE_MAX`/price↔rent gap/`priced_out` def; sharpened `LiquidChurn`/`HardBarrier`.
- **P2.2 cohort could be inherited/static** — §2.4 + §3.7: the persistent cohort must have market-relevant
  title history (bought / retained-through-priced-out / paid-carrying-through-final-window); title-share
  breakdown `{original-claim, inherited, bought, foreclosed-out}` reported.
- **P2.3 pairwise vs central matcher** — §3.4: a single deterministic sorted global sweep, strictly pairwise,
  no multilateral/ring clearing.

### Round 2 (3 P1 + 1 P2 → SPEC-READY)

- **P1.1 buyer bid formula** — §3.4: buyer reservation = the plot's FUNDAMENTAL `cap_factor × rent(plot)`
  (same rent/quality-prior basis as the ask's base_price, IGNORING the sale-history premium); `bid =
  min(SALT, reservation)`, clears iff `bid ≥ ask`. Affordable fundamental-value plots clear; bubbly
  above-fundamental asks don't — no systematic LandMarketInert, no instant flips. Ask-vs-bid gap reported.
- **P1.2 listing lifecycle** — §3.4: idle listings recompute each sweep + cancel when the owner works/pays
  again; arrears/foreclosure listings persist until sold or arrears paid; listed owners may keep harvesting,
  but a cleared sale always transfers title.
- **P1.3 land_fee_pool destination** — §3.5: `land_fee_pool_salt` is an explicit non-agent conserved SINK,
  digested ON-only, NOT spendable/redistributed in S23b.
- **P2 price clamp edge** — §3.3: upper bound `max(LAND_PRICE_MIN, base_price × 4)` so it never falls below
  the floor; quality prior floors the rent basis.
- Codex confirmed: post-money gate clean; budget hysteresis is a valid empirical hypothesis (LiquidChurn is a
  legitimate possible finding, not a spec failure); title-retained foreclosure avoids relabeled forfeiture.
