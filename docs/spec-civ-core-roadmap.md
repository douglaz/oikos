# Spec: Toward a Realistic Civilization Core

*A praxeology-consistent architectural roadmap for the OIKOS simulation engine.*

> Status: design spec (roadmap tier), revision 1 (2026-07-03). This is **not** a single-milestone
> impl spec — it is the sequencing document from which each layer below becomes its own
> `impl-NN.md` and goes through the established pipeline (research → spec → Codex spec-review →
> rb-lite build → independent verification → Codex result-review → merge + record). It is the
> companion to `spec-civ-game-integration.md` (how a game wraps this core) and the successor-in-scope
> to the microfoundations recorded in `report-emergence-and-its-limits.md`.
>
> **Central claim.** OIKOS has already built the *microfoundations* of a market economy — Mengerian
> money (strong-bar, non-circular), the grain→flour→bread division of labor, producible capital,
> per-agent originary interest, entrepreneurial profit/loss, private land tenure, a land market, and
> tax receivability — each emergent, conserved, deterministic, and honestly scoped. The gap between
> *that* and a **realistic civilization simulator** is a specific, ordered stack of **institutions**,
> and the research has already located its keystone: the **demand-side income loop** (S23c→d→e).
> This document lays out the full stack and sequences it so the keystone comes first.

---

## 1. What "realistic civilization simulator" means under praxeology

A civilization, in Misesian terms, *is* the division of labor extended across **number, space, and
time** — "the great society" of people who have never met cooperating through exchange. Society is
not an organism that acts; it is the network of exchanges among individuals who each act to remove
felt uneasiness under scarcity (methodological individualism). A civilization simulator is therefore
not a set of aggregate dials — it is the **institutional stack that lets ever more individuals
cooperate through exchange**, plus the honest consequences when those institutions are absent,
malformed, or coercively distorted.

The engine already contains the atoms of that stack (verified, `sim/src/settlement.rs`,
`econ/src/*`). What is missing to reach civilization scale, in dependency order:

1. **A circular flow of income** so non-owners can participate at all (the S23c–e blocker).
2. **Firms** — persistent enterprises that organize production and employ labor.
3. **Unified households** that own, consume, reproduce, and bequeath — closing the "disjoint
   populations" wall (S23c) structurally.
4. **Complete factor markets** — wages, rent, and interest as the three factor incomes.
5. **A state** — a coercive fiscal-military institution that taxes, spends, and enforces rules.
6. **Technology** — cumulative knowledge that lengthens the structure of production.
7. **Money, credit, and the business cycle** at scale (the Austrian cycle as an emergent result).
8. **Space** — many settlements linked by trade and migration (the empire substrate).
9. **Classes and social mobility** — persistent economic strata that individuals move between.
10. **Conflict** — the "political means" (coercion) as an alternative to the "economic means"
    (production and exchange).

Each is a *layer* built as one or more gated milestones. None replaces the microfoundations; each
composes on them.

### 1.1 The discipline is carried forward verbatim (non-negotiable)

Every layer below inherits the project's method exactly (see `report-emergence-and-its-limits.md`
§2 and `game-spec.md` §4.5):

- **Conservation every tick.** The per-good identity
  `after = before + regen + endowment + produced − consumed_as_input − consumed − promoted − spoiled`
  (settlement.rs:17470) is asserted every tick. New channels (a wage, a rent, a tax, a tribute) are
  **named, conserved transfers**, never mint/burn. Money paid as a wage is money that left the
  employer's balance; income created "from nowhere" is a bug, not a feature.
- **Byte-identical goldens when the flag is off.** Each layer ships behind a default-off flag,
  canonicalized ON-only in `canonical_bytes()` (settlement.rs:20107) with a **distinct injective
  digest tag**. Merged tags run to 17 (S24c); tags 18/20/21 are held by the unmerged S23c/d/e
  branches; **new milestones take tags from 22 upward.**
- **Ordinal preference, no cardinal or interpersonal utility.** Decisions read the agent's ordinal
  value scale (`life/src/scale.rs`), never a cardinal welfare number and never an aggregate
  (`econ` purism is compiler-enforced: metrics modules are unimportable from decision modules).
- **Imputation, not computation.** Factor values are imputed *backward* from the realized value of
  output (Menger), exactly as the S2 project-input bid and S7 capital appraisal already do — never
  computed from a cardinal marginal-product formula.
- **No planner placement.** Nothing is allocated by a central optimizer. Every reallocation is a
  voluntary exchange or a *named coercive* transfer (the state, §5, is explicitly the exception, and
  its coercion is modeled *as* coercion with praxeologically-derivable consequences).
- **Classify-not-tune.** Parameters are set on principle and **swept, not searched**; a milestone
  reports a classified verdict against a predeclared falsifiable bar; negative findings are
  first-class.

If a proposed mechanism cannot be built inside these constraints, that tension is itself the
finding — it is reported, not papered over (this is exactly how S23c→d→e produced the keystone).

---

## 2. The keystone: the circular flow of income (why buyer-income is Layer 0)

The S23c→d→e strand is the reason this roadmap exists in this order. Its verified conclusion
(`report-emergence-and-its-limits.md` §20–21, result-reviewed ACCEPT-AS-HONEST-NULL):

> On this base, private property and scarcity do **not** force a market, because the non-owner buyers
> have **no sustained purchasing power** — a one-time money endowment that depletes, with no wage,
> rent, or payment stream to renew it — so when subsistence runs short they cannot buy the owners'
> surplus; they simply go hungry.

Praxeologically this is unsurprising and precise. In the pure market economy, income is exactly two
things: (a) the **proceeds of selling factor services** you control — your labor, the use of your
land, the services of your capital — and (b) **entrepreneurial profit** for bearing uncertainty
well. A colonist who owns no land and no capital has exactly one factor to sell: **his labor.** The
institution that lets him sell it is the **wage-labor relation**, and it is the keystone of the whole
civ core:

```
employer advances money  →  hires labor for a wage  →  worker earns recurring income
        ↑                                                         ↓
employer's sale revenue   ←  worker (now solvent) buys output  ←  worker
```

This closes the loop the simulation has never had. It builds on the lesson of **S22f** (the one lever
that stabilized an occupation was a *voluntary fixed-term commitment*): both a wage contract and the
S22f commitment are **voluntary binding institutions** an agent opts into under its own realized
signal. The analogy is a family resemblance, not an identity — S22f is a fixed-term *role-commitment*
device with no paying counterparty, whereas a wage contract is an *exchange of labor for money with an
employer who pays* — but the shared shape (a voluntary institution that overrides the moment-to-moment
exit) is why C1 is the right next lever rather than another incentive tweak. And it is textbook
Misesian catallactics of the wage: the employer offers a
wage imputed from the (forecast) sale value of the product minus his required profit; the worker
accepts ordinally when the wage-money-want outranks the leisure or self-provision he forgoes.

**Everything above Layer 0 depends on this.** A state cannot tax incomes that do not exist; classes
cannot form without factor incomes; a credit cycle needs a wage structure to distort. So the roadmap
leads with it, even though the user's framing is "broad roadmap, buyer-income as one milestone among
many" — it is one milestone, but it is the *first* one, and the rest are scoped around it.

---

## 3. The layered roadmap

Notation per layer: **Goal** · **Praxeology** (the grounding and the anti-smuggling guard) ·
**Mechanism** (in engine terms, with existing substrate cited) · **Conservation** · **Flag & tag** ·
**Falsifiable bar** (the predeclared classify-not-tune verdict) · **Depends on**.

The engine already contains substrate for most of these — the honest headline is that this is far
less "new economics" than "compose and complete what S5–S24 built." Each layer notes what exists.

### C1 — Wage labor and the circular flow of income  *(the keystone)*

- **Goal.** A non-owner can sell labor for money to an owner of land/capital, earn a recurring wage,
  and become a solvent buyer of the owner's output — so a producer/buyer money market forms and
  sustains, which no S23 milestone achieved.
- **Praxeology.** The wage is a price; it is *imputed* from the entrepreneur's appraisal of the
  product's sale value, not computed from a marginal-product formula. The worker's acceptance is an
  **ordinal** comparison on his own scale (wage → future-money wants vs. the `Leisure` want and the
  self-provision alternative he forgoes — the same rank-walk `life/src/scale.rs` already runs). The
  employer's offer is an **entrepreneurial forecast** (reuse S11 `expect.rs` price beliefs) that can
  be wrong and select through profit/loss. **Anti-smuggling:** the worker must *prefer* the wage to
  self-provision — so C1 is only meaningful once self-provision is *worse* than specializing (the
  S21g/S23d lesson: "subsistence redundancy must precede specialization"). The finite rival commons
  of S23e (branch `feat/rival-commons-impl-rb`, tag 21) is the scarcity substrate that makes this
  bite; C1 composes on it rather than on the unlimited own-labor floor. **Second anti-smuggling guard
  — employer-reserve provenance:** the wage fund must be the employer's *own* money, earned from
  prior sales or held as saved/retained balances (C2), and *advanced before* the product is sold
  (the entrepreneur bears the interval — originary interest, S10). It must **never** be a per-tick
  injected advance; a subsidised wage would silently re-create the S23d scaffold, so a
  `subsidised_wage` control must be separated exactly as `fiat_wage` is.
- **Mechanism.** Extend `econ/src/factor.rs` (`LaborBook`, already present) into the settlement loop
  as a labor-market phase inside `econ_tick` (settlement.rs:9273), clearing *before* the goods
  market: owners of a plot/tool with unmet output demand post wage bids (money for labor-hours);
  hungry non-owners post labor asks (reservation = the ordinal value of the leisure/self-provision
  forgone, generated from the same rank-walk the goods market uses — not a scalar threshold).
  Clearing pays the wage into **escrow** — the delivery-escrow contract of `game-spec.md` §4.3, which
  is **new implementation, not existing machinery**: today `LaborBook`/`apply_labor_trade`
  (`econ/src/factor.rs`) pays wages and advances projects *synchronously*, so C1 must **build** the
  two-rate escrow bucket (wage conserved, released pro-rata on labor actually delivered in the fast
  loop settlement.rs:9992, refunded to the employer on short delivery/death). The hired worker's
  labor advances the owner's recipe (the owner keeps the output and sells it); the worker holds money
  and buys in the goods market same tick or next.
- **Conservation.** Wage is a money transfer employer→escrow→worker, net-zero; the escrow bucket is a
  **new conserved holder** C1 adds to the conservation identity. Labor is not a conserved *good* but a
  flow — delivered-labor-units advance the recipe exactly as own-labor does today, so the produced/
  consumed_as_input booking is unchanged.
- **Flag & tag.** `wage_labor` (default off), next free tag (22), ON-only.
- **Falsifiable bar.** Predeclared and *sustained* (not a token late trade): with C1 on and the S23e
  commons scarce, does a **sustained producer/buyer money market** form — non-owner buyers earn a
  wage and use it to buy owner output across the **whole final window**, such that (a) wage income is
  a material share of non-owner purchases (`wage_financed_buy_share ≥ θ`), (b) money velocity stays
  bounded away from zero across the window (not one spike), and (c) the wage→buy→revenue→wage loop
  turns over ≥K times — **and** it is genuinely wage-driven: a `no_wage_offered` control reproduces
  the S23e null, a `fiat_wage` control that forces employment separates as scaffold, and the
  `subsidised_wage` control (injected wage fund) separates as scaffold? Thresholds `θ, K` set on
  principle and **swept**. Verdict names: `CircularFlowForms` / `WageInertDemandStillDead` /
  `WageScaffoldOnly`. This is the milestone that *reopens* the deferred S23c generational-tenure
  study.
- **Depends on.** S23e rival commons (tag 21); S22c realized-return signal; S11 forecasts.

### C2 — The firm as a going concern

- **Goal.** Production organized by a **persistent enterprise**: an entrepreneur-owner advances money
  (from savings) to buy factors (labor via C1, land-use via C4, inputs via the market), directs
  production over multiple ticks, sells output, and **retains earnings / bears losses** with a
  standing balance sheet — the natural unit of large-scale division of labor.
- **Praxeology.** A firm is not a planner and owns no aggregate objective; it is one acting man (the
  entrepreneur) making ordinal appraisals under uncertainty, using **monetary calculation** (Mises)
  to compare input costs against forecast revenue. Profit/loss (S11) is the *only* selection
  mechanism — a firm that misappraises consumes its capital and dissolves. **Anti-smuggling (the
  retracted-"needs firms" lesson, memory Exp 11 / Codex `e3df8c9`):** the earlier de-adoption wall
  was an *artifact* of bounded-satiable savings + one-off adoption motive + a fixed producer pool —
  **not** an economic necessity. C2 must therefore make the firm's continuation depend on **recurring
  profitability under recurring consumption** (S12 `recurring_motive` precedent), and hiring depend
  on **standing product demand**, so the firm does not satiate-and-retire. If it still collapses, the
  finding is that persistent enterprise needs X — reported, not tuned away.
- **Mechanism.** A firm is a lightweight record: owner `AgentId`, owned tools/plots, a money reserve
  (retained earnings, S3 working-capital-persistence precedent), a set of open wage bids (C1) and
  input bids (S2 project-input-bid override, settlement.rs:9478), and a running profit/loss tally.
  Reuse `econ/src/project.rs` capital lifecycle for the firm's production runs and
  `econ/src/capital.rs` abandonment/salvage for dissolution (bust mechanics come free).
- **Conservation.** The firm's reserve is an ordinary agent money balance; retained earnings are
  money not yet spent; dissolution routes assets through the existing estate/salvage machinery. No
  new sink/source.
- **Flag & tag.** `firm_enterprise` (default off), tag 23, ON-only.
- **Falsifiable bar.** Mechanical, to foreclose the old satiation artifact: over a run of ≥R econ
  ticks with a *recurring* consumption load and an owner whose own consumption **drains** his reserve
  each period, (a) a firm stays continuously staffed (≥1 hired worker) for ≥F of the final window
  *without* its continuation depending on a one-off savings want — i.e. it renews on **realized
  recurring profit**, not a filled MAX_SAVE want (the Exp-11 wall); (b) profit/loss **selects** — a
  `bad_forecast` control (owner fed a systematically wrong price belief) consumes reserve and
  dissolves, a `standing_demand_off` control (no recurring product demand) fails to sustain hiring,
  and a well-appraising firm under live demand grows. `R, F` principled and swept. `EnterprisePersists`
  / `SatiationRetiresFirm` / `NoProfitLossSelection`.
- **Depends on.** C1; S7 producible capital; S10 originary interest; S11 entrepreneurship; S3
  working-capital persistence.

### C3 — Unified households: own, consume, reproduce, bequeath

- **Goal.** Make the **owner, the consumer, and the reproducer the same household** — dissolving the
  S23c "disjoint populations" wall (immortal cultivator-owners vs. mortal consumer-lineages) at its
  root, and giving the economy the unit that carries property and occupation **across generations**.
- **Praxeology.** The household is the primitive locus of consumption and reproduction; it holds
  property, its members sell factor services, it consumes jointly, saves per its (heritable) time
  preference (S10), and bequeaths. Nothing here is cardinal — the household budget is just the money
  its members earn; its consumption is the members' ordinal scales; its bequest is the S23c
  universal-heir engine (`settle_estate_to_heirs`, branch `feat/secure-land-tenure-impl-rb`, tag 18)
  that already works but had no owners who reproduced. **Anti-smuggling:** S23d
  (`feat/mortal-landowner-impl-rb`, tag 20) already built the mortal-reproducing-landowner base and
  proved the *demographic* fix works (owners age, die, and their heirs inherit endogenously); C3
  lands that base on master **composed with C1**, so the households that own and reproduce now also
  *earn wages and buy* — the piece S23d lacked.
- **Mechanism.** Compose S23d's mortal-landowner demography (tag 20) + S23c's secure-tenure
  inheritance (tag 18) + C1 wage labor onto master. Households (already `Settlement::households`,
  demography.rs) gain a shared money view for budgeting; membership, birth endowment, and estate
  succession reuse G4b + S23c.
- **Conservation.** Inheritance and endowment are conserved transfers (S23c/S23d already assert
  this: `endowed + built − destroyed == whole-system total`). No new channel.
- **Flag & tag.** `unified_households` composing tags 18+20 with C1; new composite tag 24, ON-only.
- **Falsifiable bar.** With C1+C3 on: do **born-in-sim households** persistently own land/capital,
  earn wages, buy output, **and** pass estates to heirs who continue the occupation — i.e. does the
  S23c inheritance engine finally **fire on a living economy** (`inherit_eligible_owner_deaths ≥ N`
  **and** `final_buyer_bought > 0` **and** a cross-generational owner cohort persists)? This is the
  **S23c re-run** the whole strand deferred. `GenerationalTenureLives` / `StillSubsidyBound` /
  `InheritanceStillVacuous`.
- **Depends on.** C1; S23c (tag 18); S23d (tag 20); G4b demography.

### C4 — Complete factor markets: rent and capital income

- **Goal.** Round out the three factor incomes — **wages** (C1), **land rent** (the price of using a
  plot), and **interest / capital-service income** (S10 originary interest is already per-agent) — so
  that owning land or capital yields a *recurring income stream*, not just a one-off harvest
  advantage.
- **Praxeology.** Rent is a **price discovered by exchange** — it emerges from tenant bids against
  owner asks, exactly as any market price does; it is *not* computed from a cardinal "marginal
  product" formula. The S23b land market's rolling realized-yield price
  (`base_price = cap_factor × rolling realized yield`, settlement.rs:11247) is an **appraisal input**
  (a quality prior / reservation anchor the parties may consult), never the rent itself. C4 turns the
  latent capitalized rent into an actual **periodic payment** for *use* (a tenancy), cleared by
  bid/ask and distinct from *sale* of title.
  Interest is not a configured rate (`culture.rs:5` has no cardinal discount by design); it is the
  ordinal premium on present over future goods (S10). **Anti-smuggling:** rent must be paid by a
  tenant who values the plot's use above the rent (ordinal), and must be *income to the owner that
  funds his consumption/saving* — closing another buyer-income channel for property owners, parallel
  to C1's for laborers.
- **Mechanism.** Add a **tenancy** to the land layer: an owner who is not using a plot leases its use
  to a would-be cultivator for a per-period rent (money owner←tenant, conserved), reusing the S23b
  rent estimate as the reservation price and the S23a plot registry for enforcement. Capital-service
  income = a tool owner leasing tool-use to a producer (same shape). Interest income is already the
  return on advanced capital (C2 firm reserve, S10).
- **Conservation.** Rent is a conserved money transfer tenant→owner; no goods created.
- **Flag & tag.** `factor_income_rent` (default off), tag 25, ON-only.
- **Falsifiable bar.** Do idle owners lease to productive tenants (rent > 0, owner income funds
  owner consumption) rather than forfeit/hoard, and does this **raise** the extent of cultivation vs.
  a `no_tenancy` control? `TenancyClears` / `RentInert`.
- **Depends on.** C1; C2; **C3** (owner-households are the recipients of recurring rent/capital
  income); S23a/S23b land tenure & market (tags 13/14); S10.

### C5 — The state: taxation, spending, enforcement, public goods

- **Goal.** A coercive **fiscal-military institution**: it levies taxes, holds a treasury, **spends**
  (soldiers' pay, public works, transfers), and **enforces** the property/contract rules that the
  institution layer (or, in the game, the player) sets.
- **Praxeology.** This is where the model must be *most* careful. The state is **not** a market actor
  and does **not** calculate an optimal allocation — it acts by the **political means** (Oppenheimer:
  coercion) as opposed to the economic means (production and exchange). Taxation is a **coercive
  conserved transfer** (already proven: M20/M21 `TaxReceivability`, the chartalist circuit —
  `game-spec.md` §5.9/§13). Its consequences are praxeologically derivable and must be *modeled as
  consequences, not optimizations*: a tax on wages reduces labor supply at the margin; fiat issuance
  transmits Cantillon effects (S-stack `cantillon.rs`); the state can consume capital. A **price
  control is a *named coercive legal constraint*** — a law that makes trades outside a legislated band
  *illegal* (removes them from the order book), **not** a `SetPrice` that overwrites the market's
  discovered price: the price mechanism is untouched, the state merely forbids some voluntary trades,
  and the shortage/surplus follows by necessity. (This is why the game keeps a blanket no-`SetPrice`
  rule while still allowing a modeled price-control *policy* — the state bans trades, it never names a
  price.) **Anti-smuggling (the calculation boundary):** the state may *not*
  read agent scales to "optimize" anything — it sets rules and rates (exogenous policy), collects and
  spends conserved money, and the market's ordinal responses do the rest. The one honest new piece
  M21 lacks is the **treasury-spending loop** (`game-spec.md` §13 item 1): today tax receipts *retire
  or vault*; C5 must let the treasury **spend** money it holds (Cantillon-routed to named
  contractors) to fund public works and pay soldiers — the loop that makes a "government" real.
- **Mechanism.** The state is an **in-ledger agent** with a treasury (game-spec §5.9). Tax surfaces
  (M21 `TaxPolicy`: spot/wage/debt/bank/issuer) already levy conserved money; add a spending phase
  that commissions projects (reuse `project.rs`) and pays wages (C1) to state-hired labor, funded
  **only** from treasury balance (never conjured). Public goods (defense, a court that enforces
  tenure) are projects whose service is non-excludable within the settlement.
- **Conservation.** Taxes in and spending out are conserved money transfers through the treasury
  balance, which enters the conservation identity as a named holder (M21 precedent). Fiat issuance,
  where enabled, is the explicit modeled channel (G8c), not silent minting.
- **Flag & tag.** `state_fiscal` (default off) with sub-flags for the spending loop; tags 26+,
  ON-only.
- **Falsifiable bar.** Does a fiat-only wage tax pull idle labor into taxed employment **and** does
  treasury spending fund a public work that would not otherwise be built, with the falsification twin
  (no tax / no spend) leaving both undone — the M21 circuit *completed* through spending?
  `FiscalCircuitCloses` / `TreasurySpendInert`.
- **Depends on.** C1; **C2** (the treasury commissions work through the same `project.rs`/firm
  machinery and hires via the labor market); M20/M21 tax (merged; note the current levy records
  issuer debt + receipts/defaults and does **not** yet spend — the treasury-spend loop is the new
  work); G8c tender/fiat; `cantillon.rs`.

### C6 — Technology and the knowledge ladder

- **Goal.** Cumulative **knowledge** that unlocks more productive, more **roundabout** methods — the
  "tech tree" of a civ game, done faithfully.
- **Praxeology.** Technological knowledge is a *precondition* of production, but it is **not** the
  binding constraint — **capital is** (Mises: knowing a more productive, longer process is useless
  without the saved capital to embody it and sustain the workers through its longer waiting time).
  So tech unlocks **possibility**; capital accumulation (S7) and time preference (S10) decide whether
  a society actually *takes* the longer road. **Anti-smuggling:** knowledge must be **produced by
  labor and capital** (a research sector, funded out of real surplus at the expense of present
  consumption — an intertemporal choice, S10), not handed out on a timer; and a tech unlock must not
  *set* any price, wage, or quantity — it only enables a recipe/building definition.
- **Mechanism.** The substrate exists: **G6b** already has `KNOWLEDGE` as a non-traded accumulator, a
  `Research` recipe (labor + input, `LIBRARY` tool), and **tier-gated recipes** (`PASTRY` via
  `ATELIER`, enabled on a knowledge threshold — content.rs:39). C6 generalizes this into a ladder:
  scholars (a vocation funded by surplus) produce knowledge; knowledge thresholds unlock new
  recipes/tools from `content/`; diffusion rides trade contact (C8). Tech tiers (stone→bronze→…) are
  the **content axis**; institutional eras (§`era.rs`) are the **measured** axis — they co-move
  loosely but nothing forces it (game-spec §5.8: a colony can be iron-age in tools and barter-age in
  institutions).
- **Conservation.** Research **consumes conserved inputs** (labor, grain), so it has a genuine
  opportunity cost. **Knowledge itself is *not* a conserved good** — matching the code: G6b drains
  `KNOWLEDGE` *out* of the goods conservation ledger and reports it on its own non-conserved line
  (content.rs:39; `run_production` settlement.rs:12577; the conservation identity deliberately
  excludes it). It is a **deterministic, non-traded, monotone accumulator** that gates recipes without
  being consumed (non-rival once produced). C6 must preserve exactly this treatment; describing
  knowledge as "conserved" would be wrong.
- **Flag & tag.** `tech_ladder` (default off) generalizing G6b; tags 27+, ON-only.
- **Falsifiable bar.** Does a society with **lower time preference** (S10) invest surplus in research,
  climb the ladder, and reach higher-productivity recipes, while a present-biased one does not —
  i.e. is tech adoption **capital/time-preference gated**, not free? `RoundaboutClimbs` /
  `TechWithoutCapitalInert`.
- **Depends on.** G6b research substrate; S7 capital; S10 time preference; C2 firms fund research.

### C7 — Money, credit, and the business cycle at scale

- **Goal.** Bank credit that funds entrepreneurial investment, and — as the payoff — the **Austrian
  business cycle** as an *emergent* phenomenon: credit expansion unbacked by real saving lengthens
  the structure of production beyond what time preference warrants, and the correction is a bust
  (abandoned projects = visible capital consumption).
- **Praxeology.** The natural rate of interest reflects real time preference (S10); a bank that
  expands fiduciary credit (G8b fractional reserve exists) pushes the loan rate below the natural
  rate, inducing **malinvestment** in overly-roundabout projects that cannot be completed when real
  savings prove insufficient — ABCT. **Anti-smuggling:** the cycle must *emerge* from the interaction
  of credit policy (exogenous/player) and agents' ordinal intertemporal choices — it must **not** be
  scripted. The "court economist" shadow counterfactual (credit-disabled replay, `shadow.rs`) is the
  *measurement* of the natural-rate gap, never an input to agent decisions.
- **Mechanism.** The substrate is large: **G8a/b/c** already have banks, fractional-reserve lending,
  fiduciary credit, tender/fiat, and `cantillon.rs`. The missing engine piece is the **V2→M3 runtime
  bridge** (`game-spec.md` §13 item 2): banking currently runs only on *designated* money, joined to
  the emergent-money runtime (`step_v2`) by a static seed. C7 builds the live handoff so banks form
  **on the good the world actually monetized** (S8/S9 emergent SALT), and credit expansion distorts
  the *emergent* money economy.
- **Conservation.** Fiduciary credit is modeled on the bank balance sheet (claims vs. reserves, G8b);
  fiat issuance is the explicit G8c channel. No silent money creation — the *point* is that credit
  creation is a **named, auditable** balance-sheet event whose consequences follow by necessity.
- **Flag & tag.** `credit_cycle` (default off); tags 28+, ON-only.
- **Falsifiable bar.** The M17 pair, at civ scale and on emergent money: credit expansion **with**
  fiat-wage tender lengthens the production structure then busts (abandoned projects rise); **without**
  it, issuance is inert. `CycleEmerges` / `CreditInertOnFreeWages`.
- **Depends on.** C1 (a wage structure to distort — the M17 fiat-wage-tender path); C2 firms (the
  investors); S10; G8a/b/c; the V2→M3 bridge. **Split by money regime:** the *bank-only fiduciary
  credit* cycle stands on C1+C2+G8b alone; the *state-money / fiat-issuance* path additionally
  depends on **C5** (issuer/tender policy is a state act).

### C8 — Space: multi-settlement trade, migration, comparative advantage

- **Goal.** Turn the single settlement into **one node of many**, linked by caravan trade, labor
  migration, and specialization by local endowment — the substrate an empire/4X layer needs.
- **Praxeology.** Inter-regional trade is voluntary bilateral exchange across distance bearing
  **transport cost**; settlements specialize by **comparative advantage** (Ricardo/Mises' law of
  association); migration is an **ordinal choice** (move if the expected real wage net of moving cost
  and forgone ties ranks higher). **Anti-smuggling:** no gravity-model aggregate flows — caravans are
  individual trader trips (`Role::Trader`, seeded by the S9 `IndirectFor` instrumental-holding
  pattern), priced by realized price-belief differentials, under the delivery-escrow contract
  (game-spec §4.3, §5.4).
- **Mechanism.** The substrate exists: **G2c** `region.rs` already does multi-settlement caravan
  trade. C8 completes trader route selection (transport cost, capacity, spoilage-in-transit, trip
  risk — flagged new work in game-spec §5.4), labor migration between settlements, and price
  convergence along roads (public-works investment with a measurable return).
- **Conservation.** Goods in transit are escrowed claims (conserved, released/refunded on
  arrival/loss); the conservation identity spans settlements (a whole-region total).
- **Flag & tag.** `multi_settlement_trade` (default off); tags 29+, ON-only.
- **Falsifiable bar.** Does building a road between two settlements produce **measurable price
  convergence** along it, and do settlements **specialize** by endowment (each nets exports of its
  cheap good)? `TradeConverges` / `AutarkyPersists`.
- **Depends on.** G2c region substrate; C1 (wages to migrate toward); C4. *Not* a hard dependency on
  C6 — trade and migration stand alone; **only tech diffusion** rides trade contact, so C6 is
  optional/composable, not a prerequisite.

### C9 — Classes, social mobility, and long-run demography

- **Goal.** Persistent economic **strata** (laborers, owners/entrepreneurs, rentiers) that
  individuals and lineages **move between** — the social structure of a civilization — as an
  *emergent* outcome of C1–C4, not a caste system.
- **Praxeology.** Class here is a **catallactic** category (Mises), not Marxian: it is one's *current*
  source of income (wages vs. profit vs. rent/interest), fluid by construction — a saving laborer
  buys capital and becomes an owner; a misappraising owner consumes his capital and returns to wage
  labor; heirs dissipate or grow estates. Mobility is the market's ongoing re-sorting; there is no
  fixed class. **Anti-smuggling:** classes must be *measured* from realized income sources, never
  *assigned*; mobility must be driven by saving/profit/loss and inheritance, not a scripted
  transition.
- **Mechanism.** No new decision machinery — C9 is largely a **measurement + long-horizon study**
  over C1–C4 running together: classify each household by dominant income source per window, measure
  the transition matrix (mobility), and check whether the S22 occupation arc *finally resolves at
  scale* (a durable division of labor now embedded in firms/households with property, which every
  in-the-moment incentive lever S22a–e could not produce). The Malthusian band (S14/S17) governs
  population.
- **Conservation.** Measurement only; nothing minted.
- **Flag & tag.** No new decision flag; a gated **study harness** + metrics; the outcome is a report,
  not a merged mechanism.
- **Falsifiable bar.** Over a long horizon with C1–C4 on: do **durable classes with real mobility**
  form (a non-trivial, non-frozen income-source transition matrix; a persistent owner/entrepreneur
  stratum **and** upward moves from wage labor)? `ClassesFormWithMobility` /
  `FrozenStrata` / `NoStableStructure`. This is the arc-closing answer to S22.
- **Depends on.** C1; C2; C3; C4; S14/S17 demography.

### C10 — Conflict and the political means

- **Goal.** Model **violence** as an alternative to exchange: raiding, conquest, tribute, and
  defense-as-public-good — the "political means" beside the "economic means."
- **Praxeology.** Oppenheimer/Rothbard: wealth is acquired either by production-and-exchange (the
  economic means) or by coercive seizure (the political means). Conflict is **capital destruction and
  coerced transfer**, not creation; a conqueror who taxes a productive population is a stationary
  bandit (the state, C5, at the limit). **Anti-smuggling:** war produces *no* wealth — it destroys
  capital and coercively transfers the rest; the "gains" to the victor are exactly the losses to the
  victim plus deadweight destruction, all conserved. Defense is a **public good** the state (C5)
  funds by taxation.
- **Mechanism.** In v1 the substrate is already there minimally: raids exist as **exogenous
  capital-destruction shocks** (game-spec §6, non-goal to make combat tactical). C10 promotes this to
  an **inter-settlement** interaction (C8): a settlement can raid another (destroy/seize capital,
  conserved transfer minus destruction), exact tribute (a coerced periodic transfer — a tax levied by
  an outside state), or fund defense (a C5 public work that reduces raid success). No tactical combat
  system — outcomes are resource/probability functions of committed force, deterministic per seed.
- **Conservation.** Seizure is a conserved transfer; destruction is a **named sink** (like spoilage)
  in the conservation identity. Nothing created by violence.
- **Flag & tag.** `conflict_political_means` (default off); tags 30+, ON-only.
- **Falsifiable bar.** Is raiding ever individually "profitable" in this model, and does it **lower
  total real output** (capital destroyed + labor diverted to raiding/defense) vs. a peaceful control —
  i.e. does the model reproduce that the political means is negative-sum for the whole even when
  positive for the raider? `PredationPaysButShrinks` / `DefenseDeters`.
- **Depends on.** C5 (the state funds defense/raids); C8 (multiple settlements); the exogenous-shock
  substrate.

---

## 4. The cross-cutting praxeology-consistency contract

Every layer above — and every future one — must pass this checklist. It is the operational meaning of
"all rules compatible and consistent with praxeology," and it is the acceptance gate the game spec
(`spec-civ-game-integration.md` §4) inherits verbatim:

1. **Methodological individualism.** Only individuals act. Households, firms, and states are *names
   for patterns of individual action* — a firm's "decision" is its owner's ordinal appraisal; a
   state's "policy" is exogenous rules plus conserved coercive transfers. No aggregate ever chooses.
2. **Ordinal value only.** Decisions read the agent's ranked scale; no cardinal utility, no
   interpersonal comparison, no aggregate welfare number in any decision path (`econ` purism,
   compiler-enforced).
3. **Imputation, not computation.** Factor prices (wages, rent, interest) are imputed backward from
   forecast output value; they are never computed from a cardinal production function.
4. **Monetary calculation is the firm's tool, not the planner's.** Firms (C2) and the state (C5)
   compare money magnitudes to appraise — but the *state never optimizes an allocation*; it sets
   rules and rates and lets the market's ordinal responses follow.
5. **Conservation.** Every value flow is a named, conserved channel. Income is earned by selling a
   factor service or by profit; it is never conjured. Violence and spoilage destroy (named sinks);
   only regen, endowment, and production are sources.
6. **No planner placement.** Reallocation is voluntary exchange or *named coercion*. The state's
   coercion is modeled *as* coercion, with its consequences (Cantillon, shortages, capital
   consumption) following by necessity — the interventionist result is the finding, not a bug to
   tune out.
7. **Entrepreneurial profit/loss is the only selection.** Persistence (of firms, occupations,
   classes) must survive profit/loss and recurring consumption, not a one-off motive or a subsidy
   (the retracted-"needs firms" and S23d subsidy-bound lessons).
8. **Emergence over assignment; measurement over decree.** Money good, prices, eras, classes, and the
   division of labor are *measured* from what agents did, never set. Institutions may be **supplied**
   (and, in the game, selected by the player) — the S24 triad proved the sim cannot select them
   endogenously by local welfare-imitation — but their *effects* must emerge.
9. **Classify-not-tune, findings first-class.** Predeclared falsifiable bars; swept not searched
   parameters; honest negatives; nothing relabeled post-hoc.

---

## 5. Sequencing, dependencies, and the tag plan

Dependency DAG (→ = "depends on"):

```
C1 wage labor ──► C2 firms ──► C6 tech ····► C8 space   (C6→C8 is diffusion-only, dotted = soft)
   │               │  │                          │
   │               │  └──► C7 credit cycle        │  (C7 fiat-wage path also ← C5)
   ├──► C3 unified households ──┐                  ▼
   │        (⇒ S23c re-run)     ├──► C4 rent/capital income ──► C9 classes ◄── C8
   │                            │        (C4 ← C1,C2,C3)
   └──► C5 state fiscal ◄── C2 (project machinery) ──► C10 conflict ◄── C8
```

Edges (explicit): C2←C1 · C3←C1 · C4←C1,C2,C3 · C5←C1,C2 · C6←C2 · C7←C1,C2 (bank-only) and
additionally ←C5 (state-money/fiat path) · C8←C1,C4 and soft ←C6 (diffusion only) · C9←C1–C4,C8 ·
C10←C5,C8.

**Critical path to a living economy:** C1 → C3 → C4 (wages + reproducing owner-households + factor
incomes) is the minimum that makes the settlement self-sustaining as a *market* rather than a
subsidy-bound autarky. That is also the gate the game (`spec-civ-game-integration.md`) needs before
there is any economy for a player to govern.

**Tag plan.** Merged milestones occupy digest tags through 17 (S24c). Tags **18, 20, 21** are held by
the unmerged branches S23c (`feat/secure-land-tenure-impl-rb`), S23d
(`feat/mortal-landowner-impl-rb`), S23e (`feat/rival-commons-impl-rb`); tag 19 is reserved
(`fixed_commitment_norm` control). **New master milestones take tags from 22 upward**, in build
order. C3 explicitly *lands* the S23c/S23d branch mechanisms (tags 18/20) on master by composing them
with C1 — the deferred generational-tenure study becomes viable exactly when C1 gives buyers income.

**Robustness is a first-class gate, not an afterthought.** Each layer that claims an *emergent
positive* (C1 circular flow, C7 cycle, C8 convergence, C9 classes) must clear it across the seed set
`{3,7,11,19,23}` and be swept on its load-bearing parameter — the same bar S22–S24 held — before it
is called a result.

---

## 6. Risks and open questions

1. **The satiation/subsidy wall (top economic risk).** Every prior attempt to make production
   *persist* hit either bounded-savings satiation (Exp 11) or a subsidy-bound autarky (S23d). C1/C2
   must make persistence rest on **recurring profitability under recurring consumption with a real
   scarcity of the outside option** (C1 composes on the S23e scarce commons for exactly this reason).
   If it still fails, that is the next finding.
2. **The calculation boundary for the state (top methodological risk).** C5 must resist the pull to
   let the state "optimize." The discipline: the state sets **exogenous rules/rates** and moves
   **conserved money**; it never reads scales to allocate. Interventionist consequences are the
   payoff, and they must be *derived by the market*, not computed by the state.
3. **Emergence robustness across seeds (top empirical risk).** The lab's proofs run on a curated cast;
   civ-scale mechanisms must fire across the seed set and, ultimately (for the game), across generated
   worlds — the single biggest de-risking task (game-spec §10.3, §12).
4. **Scope.** This is a multi-year arc. It is sequenced so that **C1–C4 alone** produce the
   project's first genuinely *living* economy (buyers earn and spend, owners reproduce and bequeath,
   the S23c study finally runs) — a publishable result on its own and the gate to the game.

---

## 7. How this feeds the game

`spec-civ-game-integration.md` wraps **this** core in a playable single-settlement game. The mapping
is direct: the player's institutional levers are exactly the flags and policies of C1–C7 applied at
tick boundaries (property regime, wage-labor legality, tax surfaces, money/tender, bank charters,
research direction); the game becomes *playable as an economy* once C1–C4 land; the credit age is C7;
the state/modern age is C5+C7; and the empire layer is C8+C10, deferred until one settlement is deep.
Crucially, the S24 finding makes the player's role **necessary, not decorative**: the sim cannot
select institutions endogenously, so the player *is* the institution-selection layer — which is the
game's core fantasy and its praxeological justification at once.
