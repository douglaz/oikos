# impl-64 — C3R.c: Earned Provisioning — the circular flow inside the keystone (can a mortal chain feed its dependents from externally-earned income instead of a mint?)

Status (spec): **SPEC-READY** (Codex xhigh, 3 rounds — R1 no P0, 4 P1s folded: pinned tick phase,
`genuine_external_revenue` class split with `AccountingLoopOnly` before `EarnedIncomeInsufficient`, the
{3,1,0} surround sweep, exact verdict conjuncts. R2 confirmed those and left 3 mechanical P1s. R3 narrowed
to the last two, both folded with the reviewer's prescriptions: **the funded-query rule finalized as
fund-one-loaf-at-last-realized-price on an UNPROVIDED Now bread want, with the member's own SHADED bid left
un-overridden and belief-lag measured via `funded_but_unfilled`** — the deliberately modest, un-smuggled
claim; and the `SavingsFundBridgesGap` three-exact-conjunct rule made consistent everywhere). The third
slice of C3R (the keystone: a mortal production chain), and the direct attack on the program's recurring
demand-side wall. Build base: branch
**`feat/mortal-producer-inheritance-v2-impl-rb` @ `395d11b`** (C3R.b v2 landed: inheritance preserves
structure, flow capped by the hearth subsidy). Flag **`earned_provisioning`** (bool on `ChainConfig`), gated
`earned_provisioning_active() = flag && mortal_producer_inheritance_active()` (composes on the C3R.b stack).
Digest **tag 29** (next free — the chain-runtime tag stream tops out at 28) — flag byte only. New base
`frontier_mortal_producers_earned()` deriving from `frontier_mortal_producers_heritable()` with **both
producer-side mints retired** (`food_provision=0` on the producer households, `producer_subsistence=0`).
All prior bases byte-identical off.

Falsifiable bar (headline): C3R.b measured the bind — capital inheritance sustains the chain's *structure*
only by keeping producer households populated, and the hearth mint that provisions those households floods
bread demand and caps the chain's *flow* (bread ≈ 9 at the viable subsidy; `SubsidyFloodsChainDies` above
it). Four milestones (S23d, S23e, C1, C3R.b) have now died on the same joint, sharpened by the plan review
to: **no durable earned circular-flow fund that survives mortality without mints.** C3R.c supplies the
candidate mechanism: retire the producer-side mints and provision the producer households **from the
producer's own externally-earned bread-sale revenue** — the producer transfers conserved gold to its hungry
household members, who buy bread on the existing market with it. Reproduction then *constitutes* demand
instead of destroying it, and gold *recirculates* instead of pooling. The question: does the mortal chain
then run with **both structure and flow** — or does the cold-start gap bite (a producer must sell before its
household starves — the Böhm-Bawerk/Strigl prior-saving question C1 named), or the earned income prove
insufficient (the price never recovers off the floor), or the "circular flow" collapse into an **accounting
loop** (producers buying from producers, recirculating the same finite gold with no external base)? Every
one of those is pre-named and first-class.

## 0. One-paragraph summary

The research (probes a261f6aa + abf8e782) established that the mechanism is wire-able **entirely from
existing parts** and that the honest dangers are economic, not missing primitives. GOLD is a closed balance
(no mint/burn anywhere); bread sells on the spot tape for GOLD with buyer+seller identity on every `Trade`;
a conserved `Society::transfer_gold` primitive exists (used live at births); a producer-household child is a
`Vocation::Consumer` whose hunger already generates market bread bids — so "member buys bread with
transferred gold" needs no new market path. The C1 wage-attribution pass is the exact template for the P0
**`external_earned_revenue` ledger**: a post-market scan of the spot-trade suffix crediting a seller only
when the buyer's household differs from the seller's — with a **buyer-class split** (immortal consumer /
gatherer / lineage / *other* producer-house / same-house) because the plan review's P0 (self-dealing) has a
class-level twin: a "recovery" dominated by producer-households buying from each other is an internal loop
over the same finite gold, not external demand. Both producer-side mints are retired in the headline (they
are config flips; after them a producer-house member has **no minted food and no commons** — every inflow is
conserved or earned). The demand side is honestly bounded and instrumented rather than assumed: the 44
immortal consumers hold a one-time 2,640-GOLD endowment with no recurring income (the source itself calls
the economy "viable only over a bounded horizon"), late-window bread trades fall to zero even on the healthy
immortal chain, and the *lineage* hearth (a surround mint of ~60k bread/run) may keep the bread price
floored regardless of the producer-side retirement — so price recovery is a measured hypothesis with a
disclosed surround-mint sweep, not an assumption. First-order arithmetic pre-states the wall: at the floored
price a producer's net revenue covers ~10–25% of a 2-member household's need (short 4–9×), so **flow runs
only if retiring the flood lets the price form off the floor** — exactly the joint price+volume+spread
recovery the classifier keys on.

## 1. Base facts (verified by the two probes; cites `sim/src/settlement/mod.rs` @ 395d11b unless noted)

1. **GOLD is closed.** No settlement path mints or burns it (mod.rs:55); never a node, never harvested
   (8620). All purchasing power is one-time endowment that *circulates*: consumers `starting_gold=60` × 44
   = 2,640 (3876, applied 4614), gatherers 60 × 24 (3875), lineage households 60 + `child_gold_endowment=16`
   (3898–3901), producers `producer_gold=16` one-time (3882, 24619).
2. **The bread market is the spot/GOLD tape.** `barter=None` (3874). `econ/src/market.rs`: `Trade { good,
   qty, price, buyer, seller }` (30–34); clearing debits buyer gold / credits seller gold in place (562–568),
   affordability-gated (524). Buyer identity is on every trade; households are decidable at the sale site
   (`colonist_household` 13592, `is_producer_household` 17670; the barter-side `BreadSellerProvenance`
   already records seller/buyer household — the pattern exists at 6043/19564 but reads only barter trades,
   so the spot-side ledger is a **new pass**).
3. **The ledger template exists.** C1's `run_wage_labor_market_attribution` (wage_labor.rs:659) scans the
   spot-trade suffix post-market, computes `payment = price×qty`, credits per-seller gold-provenance FIFO
   buckets with an anti-subsidy spend-earned-first discipline (625, 673–686, 704). Inert on this base (gated
   SALT), so a **template to copy, not a live path**.
4. **A conserved living transfer primitive exists.** `Society::transfer_gold(from, to, Gold)` — used at
   births (14536, 14575–14579: parent → child, clamped to parent free gold). NOT a mint (contrast
   `deliver_demography_provision_unit`, which credits fresh stock — the mint being retired).
5. **A household child can already buy.** Newborns are `Vocation::Consumer` (14589) with a `NeedState`
   (14545); hunger regenerates a `Want{Good(bread), Now}` on the agent's scale each tick (22696–22699), and
   M1 clearing matches any agent holding gold. The `household.is_some()` filter appears only in a read-only
   telemetry counter, not in want generation or clearing. **No new market-participation path needed.**
6. **The mints, and what remains after retiring them.** Producer-house hearth `food_provision` (mint,
   14358–14390; retire → 0, early-return 14378) and `producer_subsistence` (mint, producers only,
   15037–15108; retire → 0, early-return 15042). Both mint **bread itself** (staple==bread, 3438, 14362,
   15045) — the same good the chain sells, hence the direct flood. After retirement a producer-house member
   has NO minted FOOD and NO commons (emergency self-provision OFF and lineage-excluded; own-labor
   subsistence OFF; no forage commons on this base — all verified defaults). Remaining inflows, all
   conserved/earned: birth food endowment (parent-stock transfer, 14526–14573), birth gold gift (≤16,
   conserved, 14536), the one-time `producer_gold=16`, and market bread bought with transferred/earned gold.
   **Disclosed non-food residual:** the producer-house `wood_provision=3` WOOD mint (4013, 14366) remains —
   warmth does not kill on this base (1730) but it is a recurring economic endowment; the headline scope is
   the FOOD circular flow, and the WOOD mint is disclosed (retiring it would change warmth economics beyond
   this milestone's variable).
7. **Households are not actors; no pool exists.** `docs/impl-unified-households.md` (§, DRAFT): members act
   individually on their own scales; a shared budget pool is an optional, separately-gated, UNIMPLEMENTED
   sub-experiment. **C3R.c must use individual holdings + explicit conserved `transfer_gold`, never a pool.**
8. **The external demand is real but depleting.** Consumers are NOT hearth-fed (household:None — no
   `food_provision`) and NOT food-scaffolded, so their purchases are genuine; but their gold is a one-time
   stock with no recurring income (they sell ~nothing on this base), thinned further by free edible-grain
   self-feeding (`productive_reentry` + `subsistence_on_grain`: a hungry consumer eats free grain instead of
   buying bread). The source comment admits the bounded horizon (13168–13171). Even on the healthy immortal
   chain (`FlagOffHeritable`), late-window bread trades are **zero** (bread_bought=311 lifetime). The
   gatherers are the one class with *recurring earned income* (they sell WOOD to consumers) — whether they
   buy bread is a load-bearing empirical question the ledger's buyer-class split answers.
9. **The flood moves the PRICE margin.** The mint satiates members (zero Now bread wants) and adds surplus
   supply → the realized bread price collapses to its integer floor of 1 (prices are integer `Gold`; the
   observed late-window price is `Some(1)`; note the earlier-cited `.max(1)` at 16115 is input-advance
   pricing, not the spot floor — the floor here is the integer price semantics) → the baker spread
   inverts (`3·P_bread > P_flour + 1` fails once P_flour ≥ 3; recipe gate 25413–25428) → ~4,800 recipe-pay
   rejects → output 0. `recurring_motive` keeps producers *willing*; the arithmetic stops them. So **demand
   recovery = realized bread price off `Some(1)` + positive trade volume + a positive producer spread.**
10. **The surround mint.** The 2 *lineage* households' hearth (`food_provision=3`) mints ~60k bread/run in
    every C3R.b cell (`non_producer_hearth_food_minted`), and the bread price sat at the floor even in cells
    with the producer mint at zero — so the price floor may be **lineage-mint-bound**, not producer-mint-
    bound. C3R.c discloses this and sweeps it (§3.4) rather than assuming producer-side retirement suffices.
11. **The arithmetic of the wall.** Need: `hunger_deplete=2`/tick, `hunger_per_food=3` per bread
    (life/src/need.rs:82,84) → ⅔ bread/tick/member → a 2-member household ≈ 2,133 bread/1600 ticks. At the
    floored price with observed volume (~550 loaves), net revenue ≈ 245 GOLD — **4–9× short**. Flow can only
    run if the price forms off the floor.

## 2. The central question and pre-named outcomes

**Central question.** On the C3R.b stack with both producer-side mints retired and producer households
provisioned by conserved gold transfers out of the producer's **externally-earned** bread revenue (the P0
ledger), across `SEEDS=[3,7,11,19,23]`: does the mortal chain run with **structure AND flow** — price off
the floor, positive spread, external revenue covering provisioning, households sustained — or which of the
pre-named walls bites first?

**Ordered verdict enum** (first-match; structure/flow read as in C3R.b):

```
Preconditions (disqualifying):
  BaseUnviable        — the mint-on reference cells fail to reproduce C3R.b's landed grid
  ReservoirOpen       — immortal_producer_count > 0
  ConservationBroken / RegistryBroken — incl. the new ledger invariants (external_earned_revenue ≤ gold
                        received from cross-household sales; provisioning transfers conserved)
Outcome ladder (headline cell):
  ColdStartGapBites   — producer households starve/fail before the first external sales can fund
                        provisioning (the 16-gold + cold-start-buffer runway is exhausted first; discriminator:
                        genuine_external_revenue ≈ 0 at failure time — sales never happened). The
                        Böhm-Bawerk/Strigl prior-saving gap, landed as a measured finding: the required
                        advance exceeds the available fund. Reported with the runway telemetry.
  AccountingLoopOnly  — provisioning nominally runs but the revenue is DOMINATED by intra-producer-class
                        purchases (producer households buying each other's bread with the same recirculating
                        gold) while GENUINE external classes contribute ~nothing. Evaluated BEFORE
                        EarnedIncomeInsufficient (a producer-class loop must not be buried in an
                        insufficiency verdict). The ledger's key derived quantity:
                        **genuine_external_revenue = from_immortal_consumers + from_gatherers + from_lineage**
                        (cross-household producer-class revenue is tracked but EXCLUDED from "genuine") —
                        this also catches the jointly-circular A↔B pattern (A's child buys from B while B's
                        child buys from A: cross-household, but producer-class, hence not genuine).
  EarnedIncomeInsufficient — genuine external sales happen but the revenue cannot cover household need: the
                        bread price stays floored (disclosed: producer-mint-retired but surround-mint still
                        flooding — §3.4 attributes it) or volume stays thin; provisioning transfers dry up;
                        households shrink/starve (discriminator vs ColdStartGapBites: genuine_external_
                        revenue > 0 flowed before failure). The §1.11 arithmetic realized.
  SavingsFundBridgesGap — pinned discriminator, EXACT integer conjuncts (all three): (i) structure+flow
                        predicates (the same exact predicates the ladder already defines) hold over the
                        early/mid windows but NOT the final window, (ii) endowment_funded_provisioning > 0
                        (the initial fund did the bridging), (iii) late-window genuine-external bread trade
                        count == 0 (exact). The external classes' gold trajectory (consumers+gatherers,
                        start/mid/final totals) is REPORTED as the interpreting evidence, not a deterministic
                        conjunct (no "depleted toward 0" threshold). An honest bounded-horizon result naming
                        durable external income as the next wall.
  StructureAndFlowRun — structure persists (C3R.b's bar: both stages jointly staffed to the final window)
                        AND flow runs (price off Some(1), positive spread, non-trivial bread output and
                        bread-per-staffed-tick) AND provisioning is funded by GENUINE external earned
                        revenue (the §3.1 derived quantity, not total cross-household) with households
                        sustained through the final window. The program's first self-sustaining mortal
                        economy — the circular flow closed.
```

Every rung is first-class. `ColdStartGapBites` is the doctrine's own prediction (as C1's null was) and
would be a *confirming* finding, not a failure; `AccountingLoopOnly` is the P0 hazard made a verdict;
`SavingsFundBridgesGap` is the honest bounded-horizon middle.

## 3. Mechanism

### 3.1 The P0 ledger — `external_earned_revenue` with a buyer-class split (Slice A; no behavior change)
A new post-market pass modeled directly on `run_wage_labor_market_attribution` (wage_labor.rs:659): capture
`spot_trades_start = society.trades.len()` before the market, scan the suffix after it, keep
`trade.good == bread`, and for each sale credit the seller's ledger **only when
`colonist_household(buyer) != colonist_household(seller)`** — same-household sales are counted separately as
`intra_household_sales` and NEVER credit the ledger. Each credit is **classed by buyer**: `from_immortal_
consumers`, `from_gatherers`, `from_lineage`, `from_other_producer_households`. Gold-provenance FIFO buckets
(the C1 template) split every producer's gold into `endowed` (the one-time producer_gold + birth gifts) vs
`earned` (ledger credits), and **provisioning spends earned-first** with `endowment_funded_provisioning`
reported separately — so the cold-start's reliance on the initial fund is measured, not hidden. Ledger
invariants (hard guards): credits ≤ actual gold received; Σ(class splits) = total external credits;
conservation of every transfer.

### 3.2 The provisioning loop (Slice C — the headline behavior)
When `earned_provisioning_active()`, a **pinned tick phase** runs the transfer pass: **after
`regenerate_scales()` (mod.rs:10678) and after the existing pre-market mints/provisions, before
`society.step()`** (the market, mod.rs:10739/10835) — so the member's want ladder is current when read and
the transferred gold is spendable in the *same* tick's market. Deterministic order (AgentId-sorted
producers, then members). A producer-household **producer** transfers conserved gold to a same-household
member iff:
- the member currently has an **UNPROVIDED `Now` bread want** — hungry on its own regenerated scale AND
  holding no bread stock that would satisfy the want before the market (the same stock/money provisioning
  predicate the engine already applies; a member already holding a loaf posts no bid and gets no transfer).
  **The funded-query rule (explicit, because `reservation_bid_for_money` returns `None` at zero gold —
  econ/src/agent.rs:384/398 — so a cashless dependent cannot express a shortfall through it):**
  `gap = last_realized_bread_price.max(1) − member_free_gold`, clamped ≥ 0 — fund one loaf at the last
  observed realized price (`Society::realized_price`, a last-trade cache — econ/src/society.rs:4312 /
  mod.rs:20227; `.max(1)` = the integer price minimum). **The claim is deliberately modest and un-smuggled:
  the transfer funds an actual SHADED bid by the member's own machinery** (`ensure_bid` posts
  `shade_bid(reservation).min(gold)`, capped at `expected + step` — society.rs:2969 / expect.rs:36), so if
  the member's price belief lags a recovered market price the bid may go unfilled — that is real agent
  price-discovery behavior, NOT overridden: no valuation-capped bid override is added. The lag is MEASURED
  instead: `funded_but_unfilled` telemetry (members that received a transfer, posted a bid, and did not
  clear) is first-class, so a belief-lag-throttled flow shows up as a finding, never a silent artifact. No
  new threshold parameter: the trigger is the member's own (unprovided) want, the amount an existing market
  observable;
- the producer has free gold (earned-first FIFO), transfer = the gap, clamped to free gold.
The member's purchase then clears on the existing spot market; the bread arrives with `Bought` provenance
(`transfer_as_bought`, 19721–19734 — the food-side honesty check). Nothing else changes: no pool, no
household actor, no new market path, no bid subsidy. A producer with no earnings and no endowment simply
cannot provision — that is the cold-start gap being *allowed to bite*.

### 3.3 What is retired (the headline base)
`frontier_mortal_producers_earned()` = `frontier_mortal_producers_heritable()` + `earned_provisioning=true`
+ producer-house `food_provision=0` + `producer_subsistence=0`. After this, §1.6 holds: every
producer-household food inflow is conserved or earned. The lineage households and the immortal surround are
UNCHANGED in the headline (the isolation: one class moves from minted to earned provisioning).

### 3.4 The surround-mint sweep (attributing the price floor)
Because the bread price may be floored by the *lineage* hearth (~60k/run, §1.10) regardless of the
producer-side retirement — and lineage minted bread is **real sell-side supply on the tape**, not only
demand removal (asks post from any agent with removable stock and a reservation ask, econ/src/society.rs:2996
/ econ/src/agent.rs:419) — the suite sweeps lineage `food_provision ∈ {3 (base), 1, 0}` on the headline
config. The `0` point completes the attribution (full surround-mint retirement) and is marked
**destructive/disclosure-only**: it likely starves the lineage households (they have no other food path),
which is disclosed and classified as the attribution cell's cost, never presented as a viable regime. If the
price lifts off the floor only in reduced-surround cells, the `EarnedIncomeInsufficient` verdict carries the
attribution "surround-mint-floored" (pre-named, pointing at the next retirement) rather than an untyped
failure. This is a disclosure axis, not a tuned parameter: all three values reported.

### 3.5 Controls (the battery)
- **Mint-on reference** — the C3R.b viable cell (food=1, cap=2) re-run unchanged (must reproduce the landed
  StructurePersists+FlowCapped grid; BaseUnviable guard).
- **Stock-provisioning control (Slice B)** — mints retired, but the producer feeds members from its OWN
  bread stock (conserved stock transfer, no gold, no market purchase). Isolates "remove the flood" from
  "add the purchase loop": if stock-provisioning alone recovers flow, the circular-flow mechanism isn't the
  operative lever; if it starves (producer output can't cover members) the purchase loop's value-add shows.
- **No-provisioning control** — mints retired, no transfers: the starvation baseline (the C3R.b food=0
  bracket, now with the cushion also off).
- **C3R.a / tool-inheritance-denied / cap=1** — carried forward from the C3R.b battery where informative.

## 4. Anti-smuggling guards
1. **The P0 self-dealing rule is structural:** same-household sales never credit the ledger; the verdict
   `AccountingLoopOnly` additionally kills a class-level illusion (intra-producer-class dominance). `Flow
   Runs`-quality claims require genuinely-external revenue by construction.
2. **No demand nudge, no price help:** the mints are retired and the loop wired; nothing bids for, prices,
   or subsidizes bread on anyone's behalf. The member bids through its own existing want ladder.
3. **No pool, no household actor** (unified-households anti-smuggling rule): individual holdings + explicit
   conserved transfers only.
4. **The cold start is allowed to bite:** provisioning is capped to the producer's actual gold (earned-first
   FIFO, endowment share reported); no runway extension, no starter subsidy beyond the pre-existing
   `producer_gold=16` (which is disclosed as the prior-saving fund and measured).
5. **Not tuned:** no new free parameter — the transfer trigger is the member's own existing need mechanics;
   the surround-mint axis is a three-point disclosure sweep ({3,1,0}, the 0 point destructive/disclosure-only);
   everything else rides the landed C3R.b grid.
6. **Bounded-horizon honesty:** consumer-gold trajectory and late-window trade volume are first-class
   telemetry; a run that works early and dies with the consumer stock is `SavingsFundBridgesGap`, not
   success.

## 5. Conservation & determinism
No new goods or money flows are created: the ledger is pure observation of existing conserved trades; the
provisioning transfer is the existing conserved `transfer_gold`; the stock-provisioning control is a
conserved stock debit/credit pair; retiring the mints removes sources. Integer, deterministic (AgentId-
ordered transfer pass; the FIFO buckets deterministic). **Digest:** tag 29 = ON-only `{ push(29);
push(u8::from(earned_provisioning)) }`; all ledger/telemetry state runtime-only, OUT of `canonical_bytes`;
the retired mints are config values already serialized (demography/chain bytes) so the headline base is a
new scenario; `frontier`, `frontier_capital`, `frontier_mortal_producers`, `frontier_mortal_producers_
heritable` all byte-identical off — **a build DoD verified directly against the old bases (the C3R.b v2
rule), not a completed claim**. Telemetry: the ledger + class
split, `intra_household_sales`, `endowment_funded_provisioning`, `provisioning_transfers` /
`provisioning_gold`, `members_fed_by_purchase` vs `member_starvations`, `funded_but_unfilled` (transferred,
bid posted, did not clear — the belief-lag measurement, §3.2), consumer-gold trend (start/mid/final
total), late-window bread price/trades, the C3R.b structure/flow battery carried forward.

## 6. Slices
- **A — the ledger (observation only).** The spot-side attribution pass + buyer-class split + FIFO
  earned/endowed split, run on the UNCHANGED C3R.b base. *DoD: ledger invariants hold; the C3R.b landed grid
  reproduces byte-identically (no behavior change); the class split of the existing ~6k bread purchases is
  reported (who actually buys today — the gatherer question answered).*
- **B — the controls.** Stock-provisioning + no-provisioning cells on the mints-retired base. *DoD: the
  isolation cells run and classify.*
- **C — the headline.** `frontier_mortal_producers_earned` + the provisioning loop + the surround-mint
  sweep cell. *DoD: flag-off byte-identical; tag-29 split test; the transfer pass conserves; cold-start
  telemetry live.*
- **D — the suite.** `sim/tests/earned_provisioning.rs`: the cells × `SEEDS=[3,7,11,19,23]`, the §2 ladder
  printed per cell, never asserted. *DoD: suite green within budget (report any dropped slices explicitly);
  hard guards = invariants only.*

## 7. Acceptance suite (`sim/tests/earned_provisioning.rs`, new)
- **Cells:** headline (earned, mints retired) ×3 surround values ({3,1,0} — all three required, the 0 point
  destructive/disclosure-only); stock-provisioning control;
  no-provisioning control; mint-on C3R.b reference. Producer-house cap pinned at the C3R.b viable 2 (the
  inheritance-capable minimum); `RUN_TICKS=1600`.
- **Classifier, NOT asserted:** the §2 ladder. Flow read as C3R.b (price off floor + spread + output +
  bread-per-staffed-tick); structure as C3R.b (joint staffing to final window); the class split decides
  `AccountingLoopOnly`; `SavingsFundBridgesGap` vs full run is decided by the §2 three exact conjuncts (the consumer-gold trend is reported evidence only).
- **Hard guards (invariants only):** conservation, money, registry, `immortal_producer_count==0`, ledger
  invariants (§3.1), byte-identity of the four old bases + tag-29 split, the mint-on reference reproducing
  the C3R.b grid.

Build/verify: `cargo test -p sim --test earned_provisioning -- --nocapture`, full workspace, fmt, clippy
`-D warnings`; mortal_producers + mortal_producer_inheritance + producible_capital + g5b_frontier +
g4b_demography + share/wage/succession suites stay green; every prior digest unchanged.

## 8. Risks & open questions
1. **The cold start may bite immediately** (16 gold + buffers vs the first-sale lag) — first-class
   (`ColdStartGapBites`), and the doctrine's own prediction; the runway telemetry makes it a measurement.
2. **The price may stay floored even mints-retired** — the surround-mint sweep (§3.4) attributes it; the
   result is then `EarnedIncomeInsufficient(surround-mint-floored)`, naming the next retirement, not a
   mystery null.
3. **The demand base may be too internal** — if the class split shows intra-producer dominance,
   `AccountingLoopOnly` lands and the honest conclusion is that the surround (depleting consumers, gatherer
   income unknown) cannot anchor a circular flow — pointing at a durable-external-income milestone (the
   gatherers' recurring WOOD income is the one candidate already in the economy).
4. **Slice-A class split may reshape the design** — if today's ~6k purchases are dominated by a class we
   did not expect (e.g. producers), the headline interpretation adjusts before the build proceeds; Slice A
   is deliberately observation-first.
5. **Provisioning-rule determinism** — multiple hungry members / multiple producers: AgentId-ordered,
   clamped, conserved; the review should confirm no ordering hazard with the market pass (transfers happen
   in the provisioning phase before wants regenerate, or after — pin the tick-phase placement).
6. **Scope discipline:** C3R.c does NOT touch the lineage households' provisioning (beyond the disclosed
   three-point surround sweep), does NOT give consumers income, does NOT add any wage/rent mechanism —
   those are the *next* walls if the pre-named verdicts land there.

## 9. Falsifiable-bar summary
Four milestones died because no mortal earns the income that would constitute demand — every dependent is
fed by a mint, so production has nothing to be for and the money pools instead of circulating. C3R.c
retires the keystone's remaining producer-side mints and wires the cheapest faithful circular flow: the
producer's externally-earned revenue (P0-guarded against self-dealing and class-level recirculation)
provisions its own dependents through the existing market. If structure and flow both run on genuinely
external revenue (`StructureAndFlowRun`), the program has its first self-sustaining mortal economy. If the
cold-start fund is too small (`ColdStartGapBites`), the price stays floored (`EarnedIncomeInsufficient`,
with the surround-mint attribution), the flow proves to be an internal illusion (`AccountingLoopOnly`), or
it works only until the external stock depletes (`SavingsFundBridgesGap`) — each is a named, measured,
first-class finding that tells the keystone exactly which wall is next.
