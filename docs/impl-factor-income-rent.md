# impl-51 — C4: Complete Factor Markets — Rent and Capital Income (do idle owners lease instead of forfeit or hoard?)

Status (spec): **DRAFT — pending Codex spec-review.** Layer C4 of the civilization-core roadmap
(`docs/spec-civ-core-roadmap.md`). Composes on **C1** (`wage_labor` tag 22), **C2** (`firm_enterprise`
tag 23), **C3** (`unified_households` tag 24). Flag `factor_income_rent`, digest **tag 25**, ON-only.

Falsifiable bar (headline): with a **tenancy** (an owner leases the *use* of a plot it is not working to
a would-be cultivator for a periodic money rent) and **capital-service leasing** (a tool owner leases
tool-use), do idle owners **lease to productive tenants** — rent > 0, owner rent income funds owner
consumption/saving — rather than forfeit (S23a) or hoard, and does this **raise** the extent of
cultivation and the circular flow vs. a `no_tenancy` control?

## 0. Dependency & premise (read first)

C4 completes the three factor incomes: **wages** (C1, labor), **rent** (this milestone, land-use), and
**interest / capital-service income** (S10 originary interest, already per-agent + tool leasing here).
Rent is a second buyer-income channel — this time for *property owners* — parallel to C1's for laborers:
a landowner too old, too specialized, or too busy to work a plot can still earn from it by leasing, and
a landless-but-able cultivator can work land it does not own. That widens participation in the division
of labor.

**C4 assumes C1–C3 succeeded.** It needs (a) a money market that lives (C1 `CircularFlowForms`), (b)
owners who persist (C2 firms / C3 owner-households), and (c) the owner-household as the rent recipient
(C3). If C1 failed, rent income is as inert as every other channel on a dead market; C4 is provisional on
the upstream verdicts. Its base is the C1+C2+C3 stack (which itself lands S23a/S23b land tenure tags
13/14, already on master, plus S23c/S23d via C3).

## 1. Praxeology — rent and interest are prices, not formulas

Rent is the price of the *use* of a durable factor for a period; interest is the ordinal premium on
present over future goods (S10, `culture.rs` has no cardinal discount by design). **Both are discovered
by exchange, not computed.** The tenant values a plot's use because of the output it expects to draw from
it (imputation — Menger); the owner asks a rent that at least restores the income it forgoes by not
working the plot itself. **Anti-smuggling:** the S23b capitalized-rent estimate
(`land_market_rent_basis`, settlement.rs:10855 — rolling realized yield or a quality prior) is only an
**appraisal input / reservation anchor**, never the rent itself; the rent is what tenant bid and owner
ask *clear at*. No cardinal marginal-product formula sets it.

## 2. What already exists

- **The plot registry (S23a, tag 13):** `LandPlotRecord { owner, idle_counter, reserved_for }` keyed by
  `NodeId` (settlement.rs:6683), owner-exclusive harvest via `harvest_gate` (:1345), inheritance on death
  (:11566–11992). This is what a tenancy leases (use-right, owner retained).
- **The rent estimate (S23b, tag 14):** `land_market_rent_basis` (:10855, rolling yield / quality prior),
  `land_market_price_from_rent` (:10831, `cap_factor × rent`), `yield_history`, the conserved
  `land_fee_pool_salt` sink (:6302), carrying cost + foreclosure. C4 reuses the rent estimate as an
  appraisal input only.
- **Tool / capital ownership:** `CapitalBuild { builder, slot, template, project }` (:6792), owner-holds
  → runs-recipe gate (:1499); mills/ovens (S7), plow (S22d). This is what capital-service leasing leases.
- **Originary interest (S10):** per-agent `time_preference_base_bps` (:2308), `appraise_capital_tool_bundle_for_money`
  (:1521), the multi-horizon savings ladder. Interest income = the return on advanced capital (a C2
  firm's reserve), already ordinal.
- **Reservation machinery:** `reservation_bid_for_money` (agent.rs:378), `reservation_ask_for_money`
  (:419) — the ordinal rank-walk. C4 reuses the *pattern*, but plot-use is not a stocked `Good` want, so
  it needs a bundle appraisal (§3.2), not a direct `Good` reservation.
- **Conserved money transfer:** `move_money_conserved` (settlement.rs:14269) / the C1 escrow path — rent
  is a pure conserved money redistribution tenant→owner.

## 3. Mechanism

### 3.1 The tenancy (land-use rent)

Gated `factor_income_rent`, post-money. Each econ tick, before the goods market: an owner **not currently
working** a plot it owns (idle by the S23a engagement test) may **offer it for lease** rather than let it
sit idle/forfeit; a landless-but-able cultivator (a C3 household member with labor to spare and no plot
of its own) may **bid to rent** it. A matched tenancy is a record
`LandTenancy { owner, tenant, node, rent_per_period, last_paid, term }` on the plot. While leased:
- The **tenant** gets owner-equivalent harvest access to the plot (the `harvest_gate` admits the tenant
  for the term); the owner retains title.
- Each `TENANCY_PERIOD` the **rent is paid** tenant→owner (conserved money move); on non-payment the
  tenancy ends and the plot reverts to the owner (arrears → eviction, not a new forfeiture regime).
- The tenancy is distinct from S23b **sale of title** (title never moves); an owner can lease *or* sell,
  not both simultaneously (a listed-for-sale plot is not leasable, and vice versa, to avoid double-use).

### 3.2 Rent discovery (ordinal, via a new tenancy appraisal adapter)

Plot-use is not a `Good` in the scale, so C4 defines a new adapter, `appraise_tenancy_for_money`:
- **Tenant bid — own-money, no credit (spec-review P1).** As C1's wage appraisal established
  (`impl-wage-labor.md` §4.3), the existing bundle appraisal models a future *payable* and leaves present
  gold unchanged (`bundle.rs:109`) — reusing it would smuggle credit. So `appraise_tenancy_for_money` is a
  **true own-money adapter**: the rent is paid **now** from the tenant's own gold, and the appraisal
  compares the tenant's ordinal provisioning at **current gold** vs **(gold − rent_now)** plus the
  *expected* grain→food/output the plot yields it over the term (imputed from plot quality /
  `land_market_rent_basis` as the yield prior, capped at realized; forecast error is honest expectational
  error, not a knob). **No payable, no loan.** The tenant bids the highest rent that still newly provisions
  one of its own future wants while preserving higher wants.
- **Owner ask — over an explicit feasible-alternative set (spec-review P1).** Plot-use is not a `Good`, so
  `reservation_ask_for_money` (which prices stocked goods, agent.rs:419) does **not** apply directly, and
  the S23b estimate must **not** become a formulaic rent floor. The owner ask is the least rent that makes
  **leasing rank ordinally ≥ the owner's best feasible alternative for the plot this term**, where the
  alternatives are enumerated explicitly: (a) **own-cultivate** it (only if the owner has spare labor and
  cultivating out-ranks its other uses of that labor — its net own-use yield), (b) **list it for sale**
  (S23b; mutually exclusive with leasing — expected sale proceeds amortized over the term), (c) **leave it
  idle** (value 0). Ask = the least rent whose income makes leasing ≥ best of those on the owner's own
  ordinal scale; if none is feasible (idle owner, no spare labor, not listed), the ask floor is the idle
  value (0) and rent is whatever the tenant clears at. The S23b `land_market_rent_basis` anchors only the
  *tenant's yield prior*, never the owner ask.
- **Clear** iff bid ≥ ask (clearing at the ask, like the land market). Rent is the cleared price, **not**
  a formula.

Capital-service leasing (tool-use) is the identical shape with a tool instead of a plot: a tool owner not
using its mill/oven/plow leases tool-use for a rent; the tenant-producer's bid is imputed from the recipe
output the tool enables. Interest income needs no new mechanism — it is the return a C2 firm's advanced
reserve already earns (S10).

### 3.3 Digest tag 25 (ON-only) + conservation

`if self.factor_income_rent_active() { out.push(25); out.push(u8::from(flag)); /* per-plot/tool LandTenancy
records: node (plot/tool id, serialized EXPLICITLY as the existing land digest idiom does,
settlement.rs:20486/20523, so different leased plots/tools cannot collide), owner, tenant,
rent_per_period, last_paid, term */ }`. Rent moves tenant→owner via the **fail-closed rent primitive**
defined below — **not** raw `move_money_conserved` (which can debit before crediting a missing recipient,
settlement.rs:14279); no goods created, the use-right is metadata. Tenancy records
**steer** harvest access and payment, so they are digested ON-only. Off-path (`factor_income_rent` false):
nothing emitted → byte-identical. Death routing: a dead tenant's tenancy ends (plot reverts to owner); a
dead owner's leased plot carries the tenancy to the heir (S23c) — the tenant keeps the term, rent now
flows to the heir. **Payment ordering (spec-review P2):** each tick, tenancy **reconciliation runs BEFORE
any rent payment or harvest-access grant** — settle/refund and update-or-retire every record whose tenant
or owner died or whose term expired — so a rent transfer never targets a missing agent. Rent moves via an
explicit conserved primitive that **fails closed** (both legs or neither): unlike raw `move_money_conserved`
(which can debit without crediting when the recipient id is missing, settlement.rs:14269), the rent
primitive verifies both parties are live-and-resolved before moving; otherwise the tenancy is reconciled
(eviction / heir-reroute) and **no partial transfer occurs**.

## 4. Praxeology / anti-smuggling guards

1. **Rent is discovered, not decreed.** The S23b estimate is an appraisal input; the cleared rent is
   bid∧ask. A `hardcoded_rent` control (rent set to the S23b estimate directly, no bid/ask) must be
   separated as scaffold.
2. **Ordinal both sides.** Tenant bid = imputed bundle appraisal; owner ask = forgone-own-use
   restoration. No cardinal marginal product.
3. **Conserved money.** Rent is a conserved tenant→owner transfer; no mint; the use-right is metadata.
4. **Voluntary, revocable.** Tenancy is opt-in both sides; arrears end it (eviction), not a coercive
   forfeiture; it is distinct from sale.
5. **Post-money.** Rent is monetary → gated post-promotion.
6. **Not a relabeled forfeiture.** S23a forfeiture (thrash) is OFF here; leasing is the *alternative* to
   idleness/forfeiture, and the `no_tenancy` control isolates its effect.

## 5. Slices

- **Slice A — the tenancy record + payment loop.** `LandTenancy` on plots, harvest-gate admission of the
  tenant for the term, conserved periodic rent, arrears→eviction, death routing. *DoD: a leased plot is
  worked by the tenant, rent flows conserved, reverts correctly; off-path unchanged.*
- **Slice B — rent discovery.** The `appraise_tenancy_for_money` adapter (tenant bid imputed, owner ask =
  forgone own-use), bid∧ask clearing with the S23b estimate as anchor. *DoD: rent is endogenous
  (varies with plot quality/yield), not the hardcoded estimate; `hardcoded_rent` separates as scaffold.*
- **Slice C — capital-service leasing + tag 25.** Tool-use leasing (same shape); tag-25 ON-only digest +
  byte-identical off-path. *DoD: tool leasing clears; goldens byte-identical off.*
- **Slice D — acceptance suite + controls** (§7).

## 6. Acceptance suite (`sim/tests/factor_income_rent.rs`)

`SEEDS=[3,7,11,19,23]`, long horizon.

- **Predeclared thresholds (swept):** `MIN_TENANCIES`, `RENT_ENDOGENEITY` (rent spread across plot
  qualities), `CULTIVATION_LIFT` (extent-of-cultivation vs `no_tenancy`), owner-rent-income floor.
- **Ordered verdict enum:** base-precondition (C1 `CircularFlowForms` / conservation) → scaffold
  (`hardcoded_rent` mode) → outcome: `TenancyClears` (idle owners lease to productive tenants, rent > 0
  and endogenous, owner rent income funds consumption, cultivation extent **rises** vs `no_tenancy`) /
  `RentInert` (few/no tenancies clear, or no cultivation lift).
- **Mandatory non-vacuity:** ≥`MIN_TENANCIES` clear post-money; rent varies with plot quality (not the
  hardcoded estimate); a counterfactual — an idle plot worked by a tenant that the matched `no_tenancy`
  run leaves idle/forfeit.
- **Controls:** `no_tenancy` (baseline — plots idle/forfeit as in S23a/b); `hardcoded_rent` (rent = S23b
  estimate, no bid/ask → scaffold); `factor_income_rent_off` matched base.
- **`goldens_unchanged()`:** with `factor_income_rent` off, byte-identical to the composed-base goldens.

Build/verify: `cargo test -p sim --test factor_income_rent -- --nocapture`, `cargo test --lib`, fmt,
clippy `-D warnings`, workspace green.

## 7. Risks & open questions

1. **Upstream dependency.** Inert on a dead market (C1 failed) or without persistent owners (C2/C3).
2. **Buyer-thin base (the S23b confound).** S23b found the land economy goes *thin over owner-dominance*;
   C4's tenancy is meant to *widen* participation (landless cultivators rent in), which is the exact
   antidote — but if the base is still owner-collapsed, tenancies may not clear (`RentInert`), a scoped
   finding rather than a general claim.
3. **Rent vs. wage double-counting.** A cultivator who rents a plot *and* hires labor must not be both
   tenant and employer in a way that double-books; keep tenancy (land-use) and C1 (labor) as separate
   conserved channels.
4. **Plot-use imputation quality.** The tenant's bid depends on a forecast of the plot's yield; forecast
   error is honest expectational error (S11-like), not a knob.
5. **Interest income scope.** C4 treats interest as the already-existing S10 return on a C2 reserve; an
   explicit loan market (lending at interest) is C7, not C4 — do not smuggle credit here.

## 8. Falsifiable-bar summary

Adding a voluntary, ordinally-priced land-use tenancy (and capital-service leasing) on the C1–C3 living
base should let idle owners **earn rent** by leasing to landless-but-able cultivators rather than
forfeiting or hoarding, with the rent **discovered** by tenant bid ∧ owner ask (the S23b estimate only an
anchor) and **conserved** tenant→owner — raising the extent of cultivation and widening the circular flow
(`TenancyClears`), separated from a `hardcoded_rent` scaffold and beating a `no_tenancy` control. The
honest alternative is `RentInert` (tenancies don't clear / no cultivation lift on this base) — a scoped
finding about whether a land-use market can widen participation once the demand side can earn.
