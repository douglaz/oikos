# impl-52 — C5: The State — Taxation, Spending, Enforcement, Public Goods (does the chartalist circuit close through treasury spending?)

Status (spec): **DRAFT — pending Codex spec-review.** Layer C5 of the civilization-core roadmap
(`docs/spec-civ-core-roadmap.md`). Composes on **C1** (`wage_labor` tag 22) + the merged **M20/M21** tax
machinery. Flag `state_fiscal`, digest **tag 26**, ON-only.

Falsifiable bar (headline): does a fiat-only wage tax pull idle labor into taxed employment (M21, proven)
**and** does **treasury spending** fund a public work that would not otherwise be built — the M21
chartalist circuit *completed* through a spend loop — with the falsification twin (no tax / no spend)
leaving both undone?

## 0. Dependency & premise (read first)

C5 makes the state a coercive **fiscal-military institution**: it taxes, holds a treasury, **spends**,
and enforces the property/contract rules the institution layer (in the game, the player) sets. Its whole
value depends on there being **incomes to tax and a labor market to hire in** — so **C5 assumes C1
succeeded** (`CircularFlowForms`). M21 already proved the *pull* leg (a fiat-only tax draws a leisured
worker into fiat-wage labor and makes fiat circulate and return to the issuer, falsified by the tax-free
twin); what M21 **lacks** is the *spend* leg — receipts are **vaulted, never spent**
(`absorb_issuer_payment` → `issuer_gold_vault`/`issuer_fiat_unissued`, ledger.rs:819–850). C5 builds that
spend leg and closes the circuit. Provisional on C1's landed verdict per the discipline.

## 1. Praxeology — the state acts by the political means, not calculation

This is the milestone where the model must be **most** careful. The state is **not** a market actor and
does **not** compute an optimal allocation (methodological individualism; the Misesian calculation
boundary). It acts by the **political means** (Oppenheimer: coercion) — it sets **exogenous rules and
rates** and moves **conserved money** — and the market's **ordinal** responses do the rest. Interventionist
consequences must **emerge, not be computed**: a wage tax reduces labor supply at the margin (agents'
ordinal choices), fiat issuance transmits Cantillon effects, a price control creates shortages. The state
may **not** read agent scales to "optimize" anything. **Anti-smuggling:** treasury spends only money it
holds (never conjured — except the explicitly-modeled fiat-issuance channel, G8c). **Two distinct named
coercions must not be conflated (spec-review P1):** (a) a **legal-tender / media policy** dictates *which
media settle* a surface (`accepted_media`/tender enums, money.rs:196/239) — a real coercion forcing agents
to earn/hold a particular medium (e.g. a fiat-only tax), but it does **not** set prices; (b) a **price
control** is a *separate* named constraint — a legislated price band that makes out-of-band trades illegal
(removes those bids/asks from the order book before clearing) — which C5 specifies as its **own** mechanism,
**never** via `accepted_media` (which only filters media, market.rs:537) and **never** a `SetPrice`. Both
leave the price *mechanism* untouched and are modeled *as* coercion with emergent consequences (a media
mandate → Cantillon + labor-supply shift; a binding price band → shortage).

## 2. What already exists

- **Tax (M20/M21):** `apply_levy_tax` (society.rs:1497) creates a zero-principal `DebtContract`
  (`due = levy`, lender = issuer, `DebtPurpose::TaxLiability`); `settle_tax_debt_m3` (timemarket.rs:1901)
  debits the agent per `TaxReceivability` accepted media and **vaults** the receipt
  (`absorb_issuer_payment`, ledger.rs:819) — receipts are observability-only (`issuer.tax_receipts_*`),
  never spent. This is the exact seam C5 fills.
- **Cantillon:** `CantillonRouter::route` (cantillon.rs:8–55; `Agents | Sector | Helicopter`) +
  `CantillonReceipt` with `CreditSource` — the routing a treasury-spend uses (as `FiatPrint` already does,
  society.rs:1780–1866).
- **Tender / legal-tender policy (G8c):** `PublicSpotTender`/`LaborWageTender`/`PublicDebtTender`/
  `TaxReceivability` → `accepted_media()` (money.rs:178–237), mutated by `SetPublicSpotTender` etc.
  (scenario.rs:402). Trades outside accepted media simply cannot clear — the price-control-as-coercion
  mechanism, no `SetPrice`.
- **Public goods as projects:** `build_road_template` (project.rs:210, pure-labor, `output_qty=0`), the
  `project.rs` lifecycle + `apply_labor_trade` labor binding — a public work is a `Project` in `Forming`
  advanced by hired labor.
- **C1 labor market:** the state hires through it as a **funded bidder** (`impl-wage-labor.md` §3.1,
  wage from the treasury reserve, not conjured), never a wage-setter.
- **Money conservation:** `BaseLedger`/`ClaimsLedger`, `tms()` (ledger.rs:64–97), the C1 `total_gold()` +
  money invariant.

## 3. Mechanism

### 3.1 The treasury as a dedicated non-household in-ledger agent (new)

Add the state treasury as a **dedicated non-household in-ledger agent** with its own `AgentId`. Because
ledger balances must correspond to live agents (ledger.rs:1031), the treasury must be a real agent so its
balance is valid for the money invariants and so it can post hire orders / pay wages through C1 — **but it
is excluded from every ordinary agent loop** (spec-review P1: no value scale, no consumption/need, not a
tax subject, no role-choice/vocation); it *only* holds money, receives tax receipts, and spends. Tax
settlement is **redirected**: the receipts M21 currently absorbs into the issuer vault / unissued-fiat
(ledger.rs:819, dead-vaulted) are instead transferred (conserved) into the treasury agent's **spendable**
balance. The treasury enters `whole_system_total`/`total_gold` and the C1 money invariant as this named
holder (M21 precedent: treasury in and spending out are conserved money transfers). No new money is created
by taxation — it is a coercive conserved transfer agent→treasury.

### 3.2 The treasury-spend loop (the honest new leg)

A gated `state_fiscal` spend phase (after the C1 labor market clears its private hires, so the state bids
against a real wage): the treasury **commissions public works** as `Project`s (roads, fortifications,
a court) and **funds their labor by hiring through the C1 labor market** — the treasury is the employer,
its **wage-eligible reserve is the treasury balance**, wages are paid from it (never conjured), and the
payment is **Cantillon-routed** to the hired contractors (`CreditSource::FiatFiscal`/a new `FiscalSpend`
tag on the receipt). Hired workers deliver labor at the state worksite via the existing `apply_labor_trade`
path; on completion the public good's service applies (e.g. a road cuts transit cost for all traders, C8;
a fort reduces raid success, C10). This is the circuit M21 could not close: **tax → treasury → spend →
wages → circulation → tax**.

### 3.3 Enforcement & public goods (minimal)

The state **enforces** the property/contract rules already in the engine (S23a ownership, C1 contracts,
C4 tenancy) — enforcement here means the rules the institution/player layer *sets* are the ones applied;
C5 adds no new coercion beyond taxation + the named tender/price-control constraints. Public goods are
non-excludable project services funded by the spend loop.

### 3.4 Interventionism as emergent consequence

Policy levers are **exogenous** (tax rate/surface, tender policy, a price-control band, fiat-issuance
rate). Their consequences are **produced by the market**, not computed: a wage tax shifts labor supply
(ordinal), fiat issuance's Cantillon path advantages near-receivers, a binding price control (banned
trades) yields a shortage. C5 does **not** model these as formulas — it sets the rule and *observes* the
emergent result (which is the point, and the game's payoff).

## 4. Praxeology / anti-smuggling guards

1. **No calculation (compile-enforced; spec-review P2).** The state sets rules+rates and moves conserved
   money; it never reads scales to allocate. Enforce this the way `econ` purism / the `shadow` import guard
   already do — put the fiscal logic in a **named module forbidden (by an import-guard test) from importing
   the value-scale / metrics / aggregate types** — not a broad grep that would miss violations or ban
   legitimate treasury/project bookkeeping. Treasury *spending* choices are exogenous policy (which works to
   commission, at what budget), not scale-derived optimizations.
2. **Spend ≤ hold.** Treasury spends only its balance (conserved); fiat issuance is the explicit G8c
   channel (named, not silent). A `spend_exceeds_treasury` attempt must be rejected, not conjured.
3. **Coercion is named.** Taxes and price controls are modeled *as* coercion (conserved transfer / banned
   trades), with emergent consequences — the interventionist result is the finding, not tuned away.
4. **Wages via the market.** State-funded work hires through C1 at the cleared wage; the state is a
   bidder, not a wage-setter (no `SetWage`).
5. **Post-money.** Fiscal operations are monetary → gated post-promotion.
6. **Falsification twin.** No tax / no spend leaves the public work unbuilt and idle labor idle — the
   control that proves the circuit, not a scaffold.

## 5. Conservation & determinism

- **Treasury as named holder** in `whole_system_total`/`total_gold`; tax-in and spend-out conserved; the
  C1 money invariant extended to include the treasury balance. Fiat issuance is the explicit named channel.
- **Digest (tag 26, ON-only).** `if self.state_fiscal_active() { out.push(26); out.push(u8::from(flag));
  /* treasury balance; commissioned-project state; spend-phase routes; active tender/price-control policy
  if C5-set */ }`. The treasury balance and commissioned-project state **steer** spending → digested;
  tax counters stay observability-only (as in M21). Off-path (`state_fiscal` false): byte-identical.
- **Determinism.** Cantillon routing is deterministic; spend decisions are deterministic functions of the
  treasury balance + commissioned works. Integer-only.

## 6. Slices

- **Slice A — the treasury holder.** In-ledger treasury `AgentId`; redirect M21 receipts to it (spendable);
  add it to the conservation identity + money invariant. *DoD: taxes accumulate a spendable, conserved
  treasury balance; off-path unchanged.*
- **Slice B — the spend loop.** Commission a public-works `Project`; the treasury hires through C1
  (funded bidder, wage from treasury), Cantillon-routed; project completes and its service applies.
  *DoD: a treasury-funded public work is built by hired labor, conserved; the falsification twin leaves it
  unbuilt.*
- **Slice C — intervention policy + tag 26 digest.** C5-set **tender/media** policy via `accepted_media`
  (real media coercion) AND a **distinct price-control band** — its own new named order-book constraint that
  removes out-of-band orders before clearing, **not** `accepted_media`; tag-26 ON-only + byte-identical
  off-path. *DoD: a media mandate forces a medium; a price-control band produces an emergent shortage
  (banned trades), not a set price; goldens byte-identical off.*
- **Slice D — acceptance suite + controls** (§7).

## 7. Acceptance suite (`sim/tests/state_fiscal.rs`)

`SEEDS=[3,7,11,19,23]`, long horizon.

- **Predeclared thresholds (swept):** `TAX_PULL_MIN` (idle→taxed-employment, from M21), `SPEND_WORKS_MIN`
  (public works built only under spending), fiat-circulation/velocity.
- **Ordered verdict enum:** base-precondition (C1 `CircularFlowForms` / conservation / spend≤hold broke) →
  outcome: `FiscalCircuitCloses` (fiat-only wage tax pulls idle labor **and** treasury spending funds a
  public work the no-spend twin leaves unbuilt, fiat circulating tax→treasury→spend→wage→tax) /
  `TreasurySpendInert` (spending doesn't change what gets built / no circulation completion).
- **Mandatory non-vacuity:** ≥`TAX_PULL_MIN` agents enter taxed employment; ≥`SPEND_WORKS_MIN` treasury-
  funded works built; a real counterfactual — a public work built under spending that the matched
  `no_spend` run leaves unbuilt.
- **Controls:** `no_tax` (M21 falsification twin — worker stays idle); `no_spend` (receipts vault as M21,
  no works); `price_control_band` (emergent shortage via banned trades, reported); `state_fiscal_off`
  matched base.
- **`goldens_unchanged()`:** with `state_fiscal` off, byte-identical to the composed-base goldens; M18/M20
  tax goldens still byte-identical.

Build/verify: `cargo test -p sim --test state_fiscal -- --nocapture`, `cargo test --lib`, fmt,
clippy `-D warnings`, workspace green.

## 8. Risks & open questions

1. **Upstream dependency.** Inert without incomes to tax (C1). Non-negotiable premise.
2. **The calculation boundary (top methodological risk).** C5 must resist letting the state optimize; the
   guard is exogenous-rules + conserved-money + emergent-response, asserted by the no-scale-read test.
3. **Treasury representation.** Reusing the M21 issuer vault vs. a distinct treasury agent is a build
   choice; the treasury needs an `AgentId` to hire via C1, so a dedicated in-ledger treasury agent is the
   likely form — verify it composes with the money invariant and the issuer's fiat accounting.
4. **Public-good service modeling.** A road's transit-cut needs C8 (space); a fort's raid-reduction needs
   C10. Until those land, C5's public works can be tested via a simpler measurable service (e.g. a court
   that reduces a modeled enforcement cost) or a pure-labor road whose *service* is deferred — disclose
   which.
5. **Fiat vs specie treasury.** The spend loop must respect tender policy (which media the treasury can
   pay in) — reuse `LaborWageTender`; don't let the state pay wages in a medium the worker can't use.

## 9. Falsifiable-bar summary

Adding a named in-ledger treasury that receives M21 tax receipts as spendable and a spend loop that
commissions public works funded by hiring through the C1 labor market (state as a funded bidder,
Cantillon-routed, never conjured) should **close the chartalist circuit M21 left open**: a fiat-only wage
tax pulls idle labor into taxed employment, treasury spending builds a public work the no-spend twin
leaves unbuilt, and fiat circulates tax→treasury→spend→wage→tax (`FiscalCircuitCloses`) — with `no_tax`
and `no_spend` twins falsifying each leg and a price-control band producing an *emergent* shortage via
banned trades (not a set price). The honest alternative is `TreasurySpendInert` — a first-class finding
that the spend leg does not change real outcomes on this base.
