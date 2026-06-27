# OIKOS — Emergence and Its Limits

*An experimental report on emergent money, capital, and survival in a praxeologic simulation.*

> Status: working research report (raw material for a future article). Covers the milestone arc
> through S22d (the money sub-arc, the full open-colony-capstone *supply* arc, the mortality-on cold-start finding, its resolution — a produced demand-side survival floor that lets money and mortality coexist — the robustness appendix that finds that coexistence is *band-qualified* (MIXED, load-bearing on WOOD scarcity and SALT-anchor density), and the role-topology arc: endogenizing cultivation entry (S22a — the food-producing class self-forms as *fluid* participation, not a stable class) and testing whether accumulated skill produces occupation (S22b — it does not; the lever bites but does not change the hunger-gated entry/exit) and whether a realized monetary stay-decision does (S22c — it does not either; the profit-stay signal bites with a genuine counterfactual exit-flip but retains only marginally) and whether sunk, owned, durable capital does (S22d — it does not either; the owner-exclusive cultivation tool bites hardest of all, owners take up to 71% of grain, yet capital can only be earned by already sustaining the fluid role so a rare few capitalize and dominate rather than a class forming — a clean four-step negative whose consistent boundary is that occupation needs an explicit role-choice/assignment institution, or endowed/inherited capital, not a lever earned from within the fluid regime)). Every result below
> was built additively behind a default-off flag, kept the prior conformance goldens byte-identical,
> conserved every tick, ran deterministically, and was reviewed by an independent second model (Codex)
> at both the spec and the result stage. Honest negative findings are reported as first-class results,
> not failures.
>
> **Central claim (sharpened through S21).** Across conserved, deterministic ABM experiments, Mengerian
> commodity money in OIKOS did not emerge from direct demand, produced supply, or multi-good trade
> alone. It emerged only when **four conditions aligned**: (1) a good with **real direct-use demand** as
> a *non-circular eligibility floor*; (2) **medium-saleability leadership** — the good most accepted in
> *indirect/re-trade* exchange, not the good most consumed; (3) a **tradeable pre-money surplus** for
> the medium to circulate against (whether seeded or, authentically, produced by pre-money
> household labor); and (4) an **exchange institution** that lets the medium be both *sold-for* and
> *spent*. The failures along the way were diagnostic, not dead ends: necessities dominate
> *consumption* metrics, produced supply alone yields *direct* trade, imperfect coincidence can make
> the token *lead* but one-offer clearing *deadlock*, and retiring the food scaffold *collapses* the
> pre-money market until production refills it. This is a model finding, not a theorem.

---

## 1. What OIKOS is, and what we set out to test

OIKOS is an agent-based simulation built to ask whether the core phenomena of Austrian/Misesian
economics — the division of labor, capital, money, interest, entrepreneurial profit and loss, and
Malthusian population dynamics — can **emerge** from individual action under scarcity, rather than
being assumed or hard-coded. The recurring discipline was *no smuggling*: a result only counts if it
arises from agents pursuing their own ends, not from a configured preference that assumes the
conclusion (e.g. "agents want money" before money exists, or "agents prefer bread" so the chain has a
reason to run).

The economy is grounded in physical conservation: a per-tick ledger
(`after = before + regen + endowment + produced − consumed_as_input − consumed − promoted − spoiled`)
is asserted every tick, so nothing is created or destroyed except by a named, accounted channel.
Goods are gathered from depleting/regenerating resource nodes or transformed by labor through
recipes. Determinism is strict (no live RNG in the loop; per-agent heterogeneity from a hashed seed),
so any run is a fixed, reproducible trajectory pinned by a digest of the canonical state.

---

## 2. Method: the milestone pipeline

Each milestone (Sxx) went through a fixed, deliberately adversarial pipeline:

1. **Research** — read-only agents map the relevant engine machinery and report grounded `file:line`
   facts (no design proposals).
2. **Spec** — a written implementation spec: purpose, an *honest falsifiable bar*, the slices, the
   acceptance tests, and the named principled-failure modes.
3. **Spec review (Codex)** — an independent second model reviews the spec for soundness and
   circularity, returning P0–P3 findings; iterated to SPEC-READY (often several rounds).
4. **Build** — for most implementation milestones a two-implementer / multi-reviewer loop (rb-lite)
   builds against the spec on a feature branch until the review panel is clean; a small, well-isolated
   fix (e.g. S21c) is sometimes implemented directly and gated by the same verification + Codex review.
5. **Independent verification** — the orchestrator re-runs the suite, the goldens, fmt/clippy, and
   reads the load-bearing test assertions directly.
6. **Result review (Codex)** — the second model adversarially reviews the *landed* result: is the
   finding genuine, was anything tuned to pass, is the claim honestly scoped?
7. **Merge + record** — on PASS, fast-forward merge, push, and record the result.

Two recurring guards proved decisive: **byte-identical goldens** (the prior milestones' canonical
digests must not move when a new flag is off) caught accidental coupling immediately; and the
**no-tuning discipline** (set principled parameters, *report* the outcome, sweep rather than search
for a passing value) kept the findings trustworthy — especially the negative ones.

The pipeline was not uniformly clean: several rb-lite runs had thin or failed reviewer panels (a
3-reviewer panel often degraded to 2-of-3 or 1-of-3, and two runs died on transient API
overload/rate-limit and were relaunched), and a few milestones were effectively gated by the
orchestrator's independent verification plus the Codex result-review rather than a full clean panel.
The constant was the **Codex spec-review and result-review on every milestone**, not panel uniformity.

---

## 3. The foundational mechanisms (each demonstrated or honestly bounded)

These established that the production, capital, demography, and survival mechanisms self-organize.
(Most were demonstrated with designated or already-emergent money present — they are "non-money-rule"
colony mechanisms, not money-free; the *emergence of money itself* is the separate sub-arc in §4.)

- **S5–S6 — The specialized chain and provisioning at scale.** A grain→flour→bread division of labor
  self-organizes from gatherers, millers, and bakers trading through an exchange; the colony sustains
  a specialized chain at population scale rather than collapsing to autarky.
- **S7 — Producible capital.** Tools (mill, oven) *can* be produced from wood + labor over time via a
  project lifecycle — the chain is not capped at the seed tools — so capital is a roundabout,
  time-consuming investment rather than a fixed endowment. (Scenarios may still seed some starting
  tools; S7 proves new ones are producible, not that the economy starts tool-free.)
- **S10 — Originary interest / intertemporal choice.** Per-agent time preference makes the capital
  decision an *ordinal* intertemporal choice (build now for later output), heritable and heterogeneous
  across the population — interest as a feature of valuation, not a configured rate.
- **S11 — Entrepreneurial error and profit/loss.** Per-agent price forecasts (fallible, biased) drive
  production decisions; a real shock perturbs the economy and the loss *selects* — through capital,
  not yet mortality — making entrepreneurial error falsifiable rather than noise.
- **S13 — Spatial households (a structural prerequisite).** The reproducing lineage population was
  non-spatial (econ-only, hearth-fed) while only the fixed roster could forage. S13 unified the model
  so lineage members are spatial and can forage — the population that *grows* can now *work the land*.
  The load-bearing subtlety: the econ agent arena reuses freed slots with bumped generations, so
  world/econ id coincidence is preserved by *mirroring the exact econ id* into the world, not by
  insertion-order lockstep (which breaks on the first birth-after-death).
- **S14 — Forage carrying capacity (the Malthusian preventive check).** Foraging became a real capped
  commons (per-capita yield falls as more forage it), so the colony's carrying capacity is
  *endogenous*: population grows while fed and **plateaus** when forage scarcity raises hunger past
  the birth-hunger ceiling and births stall. The plateau tracks the forage flow (lower flow → lower
  plateau), bounded by the *preventive* check — fewer births — with no deaths required.
- **S15 — Agricultural intensification (Boserup).** Under forage scarcity, the unfed surplus
  *cultivates* bread by its own labor — tapping the more abundant grain resource via a more
  roundabout, more laborious process — which *raises* the carrying capacity above the forage-only
  plateau. Crucially, cultivation is chosen *only* under pressure (abundant forage → no cultivation):
  the costlier method is adopted exactly when the cheaper margin no longer satisfies wants. This is
  the authentic driver of agriculture — population pressure, not a seeded preference for bread.
- **S17 — Mortality (the Malthusian positive check).** Re-enabling starvation death on the fed,
  plateaued cultivation colony, at principled lab-default thresholds, produced a genuine
  *carrying-capacity band*: births and starvation deaths both phase-track hunger
  (at seed 1 over a 3000-tick measurement window: `corr(hunger, deaths) ≈ +0.65`,
  `corr(hunger, births) ≈ −0.68`), the population oscillates around a bounded band without drift or
  extinction. The insight (which beat the prior expectation of "the
  preventive check absorbs everything"): the preventive check binds on *potential births* while the
  positive check binds on *already-living marginal mouths* — so both operate at once.

---

## 4. The money sub-arc — the spine of the project

The hardest and most revealing thread was money. Mengerian theory says a *medium of exchange* emerges
because some commodity is more *saleable* than others, letting agents trade indirectly when no direct
"double coincidence of wants" exists. We tried to make a neutral token (SALT) emerge as money from
real exchange — and the arc became a progressive isolation of *exactly what that requires*.

### 4.1 S8/S9 — Emergence under a scaffold

The strong-bar emergence milestone made SALT promote to money without a circular "medium want": SALT
had a real, heterogeneous direct *use* (the regression-theorem anchor), and it had to clear a genuine
*indirect-acceptance breadth* gate (accepted as a means to reach other goods, by enough distinct
agents). This passed — SALT emerged from real saleability, not a renamed medium want — **within that
provisioned ecology** (S12 below shows the provisioning was itself load-bearing; do not quote this
result without that scope).

### 4.2 S12 — The first deep finding: emergence rested on a *minted* scaffold

Trying to retire the colony's food *mints* (hearths that produced bread with no labor) exposed that
the S9 emergence was **not provision-autarkic**. A deeper analysis isolated *why*: the minted
demographic bread hearth was the load-bearing **supply** — the counterparty the SALT-holding consumers
circulated *against*. Removing it didn't restore bread demand; it removed the bread *seller*, and the
one-offer barter book turned "no bread seller" into a circulation choke. The honest sharpening:
*strong-bar SALT emergence is genuine within a **provisioned** bread economy, but not yet within a
fully **produced** subsistence economy.* This reframed the whole question and motivated S13–S20.

### 4.3 S16 — Produced bread supplies a market, but money doesn't emerge

S13–S15 built a genuinely produced economy (spatial population, forage carrying capacity, own-labor
cultivation). S16 let the *produced* bread be traded for SALT. The market formed and scaled — the
bread that traded was 100% produced (a stock-origin provenance ledger proved minted contribution was
zero) — **yet SALT never promoted**: it accrued *zero* indirect-exchange breadth. The reason: the
bread-for-SALT trades that formed were **direct final-good trades** (agents acquiring bread to eat),
which give the durable token no *indirect* (re-trade) breadth — and with the mint retired the
hunger-stressed colony directs its trading at food, not at building up a medium. (This is *not* a
claim that "food is the money"; it is that produced supply alone yields direct food trade, not a
monetized medium.) **Produced bread alone was not sufficient *in this S16 single-good setting*** —
though note the later reconciliation: S21d/e/f show that *supply generation* (a real pre-promotion
tradeable surplus) WAS in fact the missing piece for the open-survival path; S16's lesson is the
narrower one that a single produced good gives only *direct* trade, not a monetized medium.

### 4.4 S18 — A produced multi-good economy, perfect coincidence: the necessity beats the token

We added a second produced/gathered good (WOOD) and a real division of labor: bread cultivators ⇄
woodcutters. But this two-good economy has a *perfect* double coincidence of wants (each side wants
exactly what the other makes), so bread↔WOOD clears by **direct barter**. The abundant,
universally-wanted necessity (WOOD) dominated the saleability race (>10× SALT) **under the legacy
total-acceptance metric** — so SALT never even led. (S21b later reframes *why* that metric was too
crude: total acceptance conflates consumption with medium use; but at S18 the deeper point stands —
with perfect coincidence no medium is needed at all.) The finding: *money is not created by "two
produced goods" alone* — a medium is only needed to bridge
**absent** double coincidence (Menger/Jevons).

### 4.5 S19 — Imperfect double coincidence: the token finally *leads*, but exchange deadlocks

We built the canonical 3-good cycle with **no pairwise double coincidence** (A makes X needs Z; B
makes Y needs X; C makes Z needs Y), demand derived from production (not invented taste), survival
isolated off-market so the necessities don't flood the book. This worked *exactly as theory predicts*:
the cycle goods are bad direct media, so the neutral token **won the saleability-leader race** —
`saleability_leader() == Some(SALT)`, the step beyond S18. **But it still didn't promote.** Indirect
SALT offers were posted, yet *no indirect SALT trade cleared*: the **one-live-offer-per-agent** book,
plus an indirect-offer generator that *replaced* an agent's direct "spend SALT" offer with an indirect
"receive SALT" offer, left the book full of "give output → SALT" and missing the complementary "give
SALT → input" side. The arc had now isolated the remaining barrier all the way down: **not the want
structure, not the saleability race — the barter-clearing institution itself.**

### 4.6 S20 — The resolution: a two-lane bilateral order book

The fix was to enrich the *exchange institution*, not the money rule. A gated two-lane order book lets
each agent hold **both** a *spend* offer (`give SALT → input`, a bid) and a *sell-for-medium* offer
(`give output → SALT`, an ask) at once — exactly what a real order book permits and the one-offer book
artificially forbade. Ordinary **pairwise** matching then lets the seeded SALT round-trip the ring
bilaterally: A's "spend SALT for Z" matches C's "sell Z for SALT," C then spends that SALT on Y from
B, B on X from A. **SALT promoted.** It led first (as in S19), then cleared the *unchanged* strong-bar
gate; the medium genuinely round-trips (accepted *and* spent), and indirect breadth spans all three
cycle goods.

The authenticity guards held: the matcher stayed strictly **pairwise** (no central ring/clearing-house
that would clear the triangle *without* money — which would have disproven the thesis); `barter.rs` and
the clearing code were byte-for-byte unchanged; every pre-promotion cycle-input acquisition had SALT on
one side. And money was proven *load-bearing* by controls: with the same ecology but the two-lane flag
off, the **S19 deadlock returns**; remove the SALT seed, and nothing clears.

---

## 5. What we discovered

1. **In this model, money emergence had separable necessary conditions — and the "saleability" one
   split in two.** A token became money only when (a) it had **real direct-use demand** (a
   non-circular eligibility floor — the regression-theorem anchor), (b) it won the **medium-saleability**
   race — most accepted in *indirect/re-trade* exchange, *not* most consumed (S21a/S21b: the original
   single "saleability = total acceptance share" metric conflated consumption with medium use, and a
   universal necessity dominates consumption; splitting the metric is what let the durable token lead),
   *and* (c) the **clearing institution** let the medium be both *sold-for* and *spent* (an agent can
   hold both sides of the monetary strategy). The arc separated these: S18/S21a showed (b) fails under
   a conflated metric; S19 achieved the saleability lead but failed (c); S20 added (c) and money
   emerged; S21b/c sharpened (a)+(b) and confirmed the durable medium promotes over the necessity in a
   controlled scenario. No one condition sufficed. **Honest caveat:** this is a *model* finding,
   not a universal theorem — and condition (b) is partly a genuine economic insight (a market needs an
   institution expressive enough for monetary round-tripping; real economies have many) and partly the
   repair of a *self-imposed* modeling constraint (our one-offer-per-agent barter book artificially
   forbade holding a bid and an ask at once). The defensible statement is institutional, not
   mechanism-specific: *Mengerian money required both a saleability leader and an exchange institution
   capable of monetary round-tripping* — not "money needs a two-lane order book."

2. **A medium is only needed to bridge *absent* double coincidence.** Where wants coincide directly
   (S18's two-good perfect-coincidence economy), direct barter clears and the durable token is
   out-competed by the most-wanted necessity. This is the Menger/Jevons foundation, demonstrated
   negatively then positively.

3. **Apparent emergence can rest on a hidden scaffold.** S9's "money emerges" was real on its terms but
   depended on a minted bread *supply* (S12). Provenance ledgers and isolation controls — not headline
   volume — are what distinguish genuine emergence from a scaffolded artifact.

4. **Population dynamics decompose into Malthus's two checks, on two different populations.** The
   preventive check (fewer births) bounds the carrying capacity via the *potential* population (S14);
   the positive check (deaths) bounds it via the *already-living* marginal population (S17); both
   operate at once, producing a genuine oscillating band.

5. **Agricultural intensification is driven by scarcity, not taste (Boserup).** Cultivation — the more
   roundabout, more laborious path — is adopted *only* when foraging can no longer satisfy hunger, and
   it raises the carrying capacity. The chain's existence is endogenously motivated by population
   pressure, not a seeded preference.

6. **Negative findings were the most informative results.** S12, S16, S18, and S19 each *failed* to
   produce money — and each failure, honestly characterized, isolated the next variable. The arc's
   value is as much in the precisely-bounded "no"s as in the final "yes."

---

## 6. Honest scope and limitations

- **S20's money is in a produced *exchange cycle*, not a scaffold-free full colony.** Survival is still
  isolated off-market (fed by a hearth scaffold) and the production loop is closed (no terminal
  consumer) — these are deliberate S19 abstractions that isolate the money question. The earned claim
  is "endogenous token money in a produced exchange cycle," not "scaffold-free colony money."
- **SALT is shown load-bearing, not uniquely destined.** The no-SALT control proves a medium is
  required; it does not prove only SALT could ever monetize — another neutral commodity could under
  the same institution.
- **Several non-money mechanisms remain parameter-supported** (cold-start buffers, the SALT direct-use
  anchor density, the cultivation subsistence cap). These are disclosed, swept where they matter, and
  not tuned to force outcomes — but they are not themselves emergent.
- **The strong-bar gate's thresholds are configured**, not derived; they encode "what counts as
  monetary breadth." S20 left them unchanged precisely so the result attributes to the institution.

### Threats to validity (what a skeptic will attack first)

- **Configured strong-bar thresholds** — `min_indirect_acceptances`/`acceptor_agents`/`target_goods`
  encode "what counts as monetary breadth"; they are set, not derived.
- **Configured SALT direct-use anchor and producer SALT seed** — the regression-theorem seed and the
  bootstrap commodity balance are parameters set, not derived; the no-seed/no-anchor controls fail.
  Their *sizes* are swept in the robustness appendix below (promotion holds across the pinned seed-size
  and anchor-density bands), but the values remain configured, not emergent.
- **Closed input-loop, no terminal consumer** (S19/S20) — the cycle goods are wanted only as each
  other's inputs; there is no final consumption sink.
- **Survival off-market via a hearth scaffold** (S19/S20) — survival is deliberately isolated so the
  necessities don't dominate saleability; that hearth is itself a (disclosed) scaffold.
- **Saleability metric (refined, now open-colony-proven for this topology)** — the original
  "saleability = total acceptance share" was too crude (it conflated consumption with medium use);
  S21a added a durability/carrying-cost dimension and S21b split direct-use eligibility from
  medium-saleability leadership. This fix held first **in a controlled econ scenario** and then
  **in the full open colony with market-financed survival** (S21e seeded supply, S21f endogenous
  production) — so the two-layer metric is no longer merely controlled-scenario. The residual scope:
  it is proven for *this configured open-market topology* (a grain commons, a 3-role WOOD-poor
  cultivator/woodcutter/consumer split, the direct-use SALT anchor + thresholds), and mortality is
  still OFF.
- **S20 fixes a self-imposed artifact** — the one-offer book was our constraint; part of the S19→S20
  result is institutional insight, part is repairing a modeling limitation. Stated honestly, still a
  result; stated as a universal law, an overclaim.
- **Robustness — established for the in-cycle result (S20-R / S17-R).** The credibility gaps are now
  closed by a robustness appendix (test-only, goldens byte-identical): S20's promotion holds **across
  RNG seeds** {1, 7, 19, 23, 42}, **across producer-seed sizes** {12, 18, 24, 36, 48} (not a knife-edge
  at the shipped 12), and at the shipped **and denser anchor densities** (`salt_direct_use_period`
  ∈ {2, 3, 4}); the S17 Malthusian band **persists to a 10k-tick horizon** (no drift or collapse —
  late-window mean within ±40% of the early-window mean — both checks active, no extinction). Caveat:
  these sweeps confirm robustness *of the in-cycle result* (closed loop, off-market survival); they do
  not extend it to an open colony — that is the open-colony integration milestone, not a robustness gap.
  (Sparser anchors, like the off-path S19 sweep, can still fail the acceptor floor; the *clearing* fix
  is robust, the *saleability-lead* condition still needs a dense-enough anchor — consistent with §5.1.)

### The bounded headline

The single most defensible claim: **"Endogenous money in OIKOS required both a saleability leader and a
market institution capable of monetary round-tripping."** Anything broader (e.g. "money needs a
two-lane order book," or "scaffold-free colony money") overstates what the runs show.

The open-colony arc extends this one notch — *durable token money in an open mortal colony required
direct-use eligibility, medium-saleability leadership, bilateral monetary round-tripping, pre-money
produced supply, **and** demand-side survival through the bootstrap* — but the robustness appendix (§8)
bounds even that: the money+mortality coexistence is **MIXED / band-qualified**, seed-robust but
load-bearing on WOOD scarcity and SALT-anchor density. So the honest frame for the whole arc is
**condition decomposition** (which conditions are necessary, and how wide each one's window is), not a
claim of spontaneous open-colony order.

---

## 7. The open-colony arc (supply question closed; mortality-on resolved via a demand-side survival floor) and open directions

The capstone — embedding the working two-lane money into an *open* colony (on-market survival +
terminal consumption, removing the S19 abstractions) — was built as a slice arc S21c→S21h and **its
supply question is now closed** (S21f: endogenous pre-money production-for-barter monetizes SALT, no
seed/no mint, for this configured open-market topology), then stress-tested under the positive check
(S21g — a cold-start finding: mortality culls the non-cultivating demand side before the market forms),
and the finding **then resolved** (S21h — a produced demand-side survival floor lets money and mortality
coexist after a one-off cold-start cull; see below). The arc began with a deep finding that re-planned
the rest:

- **S21a — Marketability / carrying cost (a finding).** Two-lane clearing (S20) fixed the *round-trip*
  but not the *saleability race*; with on-market survival, S18's universal-necessity dominance would
  return. So S21a added a *physical* marketability lever (route b): per-good durability + carrying cost
  made visible to the indirect-acceptance decision, so an agent **declines a perishable or high-carry
  good *as a means*** (it can't carry to re-trade) — no taste, no change to the saleability metric. The
  lever *works* at the individual level (perishable food and high-carry WOOD are declined as media,
  durable SALT accepted; the SALT-also-bad-medium control flips it back). **But it is not enough for the
  token to lead**, and the reason is the deepest money insight of the arc: **the saleability metric —
  total acceptance share — conflates *consumption* acceptance with *medium* (re-trade) acceptance.** The
  lever correctly cuts the necessity's *indirect* (as-a-means) acceptance, but its sheer *direct
  consumption* acceptance still dominates the share, so it keeps the lead. Money emerges because a good
  is more marketable *in exchange*, not because it is eaten often — and total-acceptance-share, a useful
  early proxy, is too crude to express that.
- **S21b — Two-layer Mengerian saleability (the resolution of the S21a finding).** Saleability is
  split into two layers: (i) *direct-use saleability* — the count of **distinct agents who accept a
  good for itself** (`DirectWant`) — is the **non-circular eligibility floor** (the regression-theorem
  anchor: a good needs real non-monetary demand to be a candidate); (ii) *medium-saleability* —
  `indirect_acceptances / total_indirect_acceptances`, the observed `IndirectFor`/re-trade volume —
  drives *leadership and promotion*. The circularity trap ("money because money") is avoided because
  eligibility rests on pre-monetary direct demand while leadership rests on **observed pre-promotion
  indirect trades that are open to every eligible candidate** (a `SaleabilityContext::Candidates` set,
  not a single preselected leader; agents' own S21a holding-rule declines suppress bad media). **Result
  (controlled scenario):** with a perishable universal necessity present, the necessity keeps the
  *total*-acceptance lead but the durable commodity becomes the *medium* leader and **promotes to
  money** — the exact failure mode of S18/S21a, now inverted. A hand-built test proves the floor is a
  real gate (a medium leader is withheld from promotion until it clears the direct-use floor), so the
  separation is genuinely non-circular. *Honest scope (Codex review-of-results, PASS-with-caveats):*
  this is a controlled econ-level proof of the **metric**, on a deliberately permissive promotion
  scenario; it is **not** yet fully-open discovery in a rich colony — candidate-mode direct discovery
  is **path-dependent** (once the eligible candidate set is non-empty, a good that crosses the
  direct-use floor *late* can be starved of further direct offers). That path-dependence is a
  documented limitation to fix where it actually bites: open-colony integration.
- **The open-colony capstone (a 6-slice sub-arc — supply question closed at S21f, mortality-on a
  cold-start finding at S21g, resolved via a demand-side survival floor at S21h).** Embedding the
  money machinery into a rich *open* colony — where agents survive by **buying food on the market**
  (terminal consumption financed by production/sales, no off-market hearth mint, no own-labor forage)
  — was built S21c→S21f with **mortality off** to isolate the money/supply question, then S21g turned
  the positive check on (the capstone stress test). A direction review established that **two-layer saleability only
  *partly* dissolves the S12 collision**: it removes the *metric* collision (food can dominate
  consumption while the durable medium leads on medium use), but the retired bread mint was also a
  real *supply/counterparty* scaffold, and **produced bread alone is still direct exchange** — the
  open colony needs a *second produced/consumed good or input-demand loop* so the medium is accepted
  for ends other than food. The slices: **S21c — fix the open-discovery path-dependence** *(landed:
  a legacy direct-discovery lane lets a below-floor good still cross the floor late; a regression test
  confirmed non-vacuous — it fails without the fix; all 18 goldens byte-identical)*; then **S21d —
  open survival, mortality off** *(landed as a **Phase A finding**: see below)*; **S21e — finite
  seeded-surplus probe** *(landed as a **diagnostic Success**: a one-time finite tradeable bread
  supply IS sufficient — SALT promotes and production replaces the seed; see below)*; and **S21f —
  endogenous pre-money production-for-barter** *(landed as the **authentic-mechanism Success**: lineage
  households cultivate `SelfProduced` bread and barter the surplus pre-money, monetizing SALT with no
  seed/no mint; see below)*; and **S21g — mortality-on over the open-market colony** *(landed as a
  **cold-start finding**: the positive check culls the non-cultivating demand side before the market
  forms, so money fails under mortality — a spec pre-named outcome; see below)*; and **S21h — the
  demand-side survival bridge** *(landed as the **resolution Success**: a produced own-labor emergency
  survival floor keeps the demand side alive-and-hungry, so money and mortality coexist after a one-off
  cold-start cull; a seeded one-time cushion does NOT — the knife-edge finding; see below)*. Mortality went last so
  a monetary failure could not be masked by a demographic wipeout, and it was meaningful precisely
  because the colony finally had a working clearing market to survive in — which is exactly what the
  finding isolates: the *demand side* of that market cannot survive the positive check's cold-start. The
  bar throughout: market-sourced food *provenance* + medium promotion + real pre-promotion indirect
  breadth, under a full control matrix.
- **S21d — Open survival (mortality off): the supply-scaffold finding.** Compose the full money
  machinery (S20 + S21a/b/c) onto the strong co-emergent colony with the food hearths **retired**
  (an explicit `retire_food_mints` engine flag, not the forage hack) so survival is a *market* bread
  purchase. Add a runtime-only **acquisition-channel ledger** (per-agent FIFO over bread:
  bought/seeded-minted/self-produced/foraged, conserved across every outflow) and a **cross-tick
  bootstrap microtrace** (buy → eat → bid, localizing the Exp-9 gate). **Result: a clean Phase A
  deadlock — SALT never promotes because the pre-promotion barter market clears _zero trades_.**
  The pre-promotion *tradeable* bread supply **depended on the food scaffold**; with production
  post-promotion only and the mint gone, every agent holds its seed bread to *eat* (its only food,
  mortality off), the universal bread want has no market supply, and the book cannot clear — so SALT
  accrues no saleability. A mints-ON control (all else identical) restores the bread market (431
  trades), localizing the gate at the retired scaffold, not the money machinery; the collapse holds
  across seeds. This **confirms the direction-review's own hypothesis**: the retired bread mint was a
  real *supply/counterparty* scaffold for this monetization path, and two-layer saleability fixes the
  *metric* but not the *supply*. (Scoped claim: the strong/open-survival frontier path depended on the
  scaffold — not every money result in the arc; the controls show the bread *market* was
  mint-dependent.) The faithful next step is an institution that supplies a pre-promotion terminal
  good (a wage/firm or seeded producers selling into the barter window), not value-scale surgery. All
  engine pieces default-off; all 18 goldens byte-identical.
- **S21e — Finite seeded-surplus probe (mortality off): the supply-sufficiency Success.** The clean
  causal counterfactual the S21d direction review prescribed: *replace the retired recurring food
  mint with a one-time, finite, decaying bread **surplus*** — bread held *beyond* a class's own
  hunger want, so it is **offerable** (passes the barter preservation rule), not reserved food —
  seeded onto the exact seller classes a **mints-on provenance trace** pins (latent `Unassigned`
  bread-buffer holders + demographic household consumers), and made WOOD-poor (the disclosed *second*
  diagnostic axis: `wood_buffer` 48→12, household WOOD zeroed) so they post real
  `bread → SALT IndirectFor{WOOD}` lanes. All else identical to S21d (mints retired, mortality off,
  S20 + S21a/b/c, bread⇄WOOD topology). **Result: a finite real supply IS sufficient.** The seed lifts
  the S21d zero-trade collapse; a mandatory non-vacuity gate confirms ≥1 real seeded seller and a
  cleared `bread→SALT IndirectFor{WOOD}` lane; **SALT promotes (tick 37) as the medium-share leader
  with indirect breadth {WOOD, bread}, and ~99% of the bread/WOOD volume is SALT-mediated, not direct
  barter.** The seed's *offerable surplus* then exhausts (tick 44) — promotion precedes exhaustion,
  and a seed-size sweep (128–2048 all promote-and-exhaust; 32/64 exhaust without promoting, fixing
  the lower boundary) confirms no size is a hidden permanent mint — after which **endogenous
  production replaces it** (4755/4773 loaves produced *after* exhaustion; the post-exhaustion tail is
  consumed entirely `bought`/`self_produced`, zero `seeded_minted`). Robust across seeds 3/7/11/19/23.
  This localizes the S21d block squarely at supply **generation**: given a tradeable pre-promotion
  food supply, the S20/S21a/b/c topology monetizes SALT and the open colony survives on a finite
  (non-recurring) food endowment. **Attribution (a same-shape control isolates the seed):** the
  WOOD-poor adjustment *alone*, with the seed removed, clears only a trickle (~11 trades) and **never
  promotes** — so the finite seed is the load-bearing change, and the honest claim is "a finite
  surplus *on a WOOD-poor seller class* is sufficient." The control matrix *classifies, never tunes*:
  no-seed → the S21d zero-trade collapse; mints-on → the scaffolded positive control; two-layer /
  multi-offer / SALT-direct-use-anchor off, and *SALT-itself-perishable*, → no promotion. One honest
  nuance: turning the durability **holding rule** off (`durability_aware_acceptance=false`) still
  promotes — the per-good holding rule is *not* load-bearing here; what is, is SALT's own durability
  (SALT-perishable breaks it) plus two-layer leadership + the supply. *Honest scope:* it is a **bounded
  diagnostic scaffold, not the authentic mechanism** — the seed is a one-time scaffold proven *finite*
  by **offerable-surplus exhaustion** (target-independent: removable above the protected hunger floor,
  so the permanent ≤20/holder spoilage floor is never load-bearing); the authentic follow-up is
  **S21f (endogenous pre-money production-for-barter)**. The `seeded_surplus_bread` field defaults 0
  (canonicalized ON-only); the seller-provenance + seeded-surplus traces are runtime-only; all 18
  goldens byte-identical.
- **S21f — Endogenous pre-money household production-for-barter: the supply question CLOSED.** The
  authentic mechanism S21d→e pointed to, replacing S21e's *seed* with genuine production. Lineage
  households **cultivate** bread by their own labor (the S15 `Cultivate` grain→bread recipe), eat what
  they need, and barter the surplus — *before money exists* — so the pre-promotion tradeable supply is
  **endogenous (`SelfProduced`), with the food mints retired and every bread buffer zeroed (no
  `SeededMinted` bread ever enters)**. The one engine piece is a gated *cultivation-without-FORAGE
  activation seam* (`household_barter_cultivation`): it runs the cultivation steering on either the
  own-labor/forage path *or* this flag, guards the FORAGE-specific code so **no `known.subsistence`
  good is interned** (no S12 pollution), sets `cultivating` from sustained hunger for spatial lineage
  members, and leaves specialized chain production still money-gated (`run_role_choice` unchanged — this
  is *unspecialized* household production, the Mengerian pre-money kind). `Cultivate` stays post-market,
  so the surplus sells **cross-tick**. **Result (SUCCESS):** SALT promotes as the medium leader with
  `{WOOD, bread}` breadth, dominant SALT-mediated share, round-tripping; the pre-promotion bread sold
  for SALT is **100% `SelfProduced`, zero `SeededMinted`** — the open colony bootstraps money from
  pre-money production-for-barter, *no seed, no mint*. **Honest scope:** what promotion needs is a
  *sufficient cumulative grain supply* — a recurring grain commons, a pure regen flow (zero initial
  stock), or even a large-enough finite grain stock all promote, while a small/medium finite stock does
  not; in every case the supply is **grain-bounded** (`produced[bread] == consumed[grain]` in the
  cultivation-only regime, never a bread mint). Controls classify, never tune (cultivation off → the
  S21d collapse; buy/sell-split off → consumers self-feed → buy-side collapse; no WOOD target /
  two-layer off / multi-offer off → no promotion; S21e seeded surplus is the positive control). This
  **closes the supply question for this open-market topology**: a real pre-money *produced* supply is
  sufficient for endogenous money emergence. Remaining scope (disclosed every time): mortality OFF, a
  configured grain commons, a configured 3-role WOOD-poor cultivator/woodcutter/consumer split, and the
  SALT direct-use anchor + thresholds — so this is "supply solved *in this configured open colony*,"
  not "scaffold-free spontaneous colony money." All gated default-off; all 20 goldens byte-identical.
- **S21g — Mortality-on over the open-market colony: the cold-start cull finding.** Turn the S17
  positive check ON over the exact S21f money colony and ask the capstone question: does the working
  money/food market survive real positive-check pressure (a Malthusian band — births *and* starvation
  both binding, no extinction, no drift — *while SALT still circulates on `SelfProduced` bread*)? The
  scenario `frontier_open_colony_mortality` derives `frontier_household_barter` with **only two
  disclosed deltas (neither tuned):** `hunger_critical = need_max` (13→12, the *lab-default* positive
  check, the exact `frontier_mortality` flip) and `birth_hunger_ceiling = 8` (12→8, the *S17/S14 band
  value*, restoring the
  **preventive** arm *below* the positive one so the two checks bind at distinct hungers — the genuine
  Malthusian structure, not a degenerate positive-only band). No new engine code (the S17 mortality
  wiring and the S21f cultivation seam both already exist); reverting both deltas is byte-identical to
  `frontier_household_barter` (the additive proof). **Result: a cold-start FINDING — money fails under
  mortality pressure (a spec pre-named outcome), NOT the hoped-for band.** At seed 7 the colony starts 4
  lineage founders + 18 **non-self-provisioning** market roles (the SALT-rich buyers + the specialist
  woodcutters, who hold no food and do not cultivate). The positive check culls **all 18** of those
  roles in a **one-off cold-start cull** (~tick 7) — `starvation_deaths_total` plateaus at exactly 18
  and **never rises again over 10k+ ticks** (a single cull, *not* an ongoing band). The self-feeding
  cultivation lineage survives exactly as the spec's cold-start budget predicted: the runtime-only
  **`cold_start_timing_trace`** (Codex spec-review P2 — an explicit ordered timing trace, not just
  no-extinction) pins the seed-7 chain `(cultivate 3, grain-deposit 4, own-use-consume 4, hunger-drop
  5, critical 5, first-death 7)`, so `first_hunger_drop (5) < first_starvation_death (7)` with survivors
  — the founders eat their own first cultivated bread before the cull. **SALT never promotes**
  (`current_money_good() == None` forever), **no food is ever bought**, `pre_promotion_bread_for_salt`
  is `(0, 0)`; what remains is a quiescent **subsistence-cultivation commune** that feeds itself
  (`SelfProduced` only, zero `SeededMinted`) and churns through births + old age, but trades nothing.
  **Why (the economic content):** the pre-money bootstrap *requires the demand side to survive a
  prolonged hungry, foodless wait* for the market to form — in S21f (mortality off) the SALT-rich
  consumers sit pinned at `need_max` for ~40–70 ticks until SALT promotes, *then* buy. The positive
  check kills exactly that patience, so the market never forms. **Controls localize the cause, never
  tune:** the **mortality-off positive control** (= S21f) keeps all 18 roles alive and promotes SALT,
  so the cause is the positive check, not the scenario; the spec's two endorsed provenance-clean rescue
  levers (faster *first production*) **cannot rescue it — both now tested** (Codex result-review P1): a
  grain flow swept to 10× leaves money dead and the demand side culled, *and* faster `cultivate_*`
  timing (triggering at the validator-floor hunger with no patience) likewise fails — because the
  dying roles **do not cultivate** (faster bread helps only the cultivators, who already survive);
  seed bread is the spec's *forbidden* last resort (it would break the `seeded_minted == 0` provenance
  the milestone rests on). A
  degenerate `birth_hunger_ceiling = 12` control shows the *same* collapse (the cull is cold-start,
  independent of the preventive arm's position). This **localizes precisely where the working colony
  stops surviving the positive check:** the **non-cultivating demand side + specialists**, in the
  pre-money window — essentially, why subsistence redundancy must precede specialization. Robust across
  seeds {3, 7, 11, 19, 23}; a 10k-persistence smoke confirms no late die-off and money stays dead. All
  gated default-off; `starvation_deaths_total` stays out of `canonical_bytes`; all 20 goldens
  byte-identical (mirroring `frontier_mortality`).
- **S21h — Demand-side survival bridge: money and mortality coexist (the S21g resolution).** Keep the
  non-cultivating buyers/woodcutters alive long enough for the market to form under mortality, sliced
  like the supply arc (S21e seeded → S21f produced). Codex predicted the dilemma: a bridge *weak* enough
  to preserve demand may fail to keep buyers alive; one *strong* enough to keep them alive may sate them
  out of the bread market and kill money. **The result splits exactly along that dilemma.** *S21h.0 — the
  consumed-only cushion (the bounded diagnostic):* a finite starting bread cushion for the two culled
  roles (buyers' `consumer_staple_buffer`, woodcutters' new `gatherer_food_cushion`), eaten but never
  sold. **The knife-edge finding: no cushion size yields a *clean* demand-bridge success.** Too small →
  only 4–5 of 18 survive (too thin a demand hub to monetize SALT); too large → the cushion sates the
  buyers out of the bread market while it lasts, then runs out and the full cull lands anyway. On the
  equal-buffer diagonal SALT never promotes at all (across sizes and seeds); off the diagonal there is
  likewise no clean success — and the cells that *do* promote do so **only by selling seeded
  (`SeededMinted`) cushion bread for SALT**, the seeded-supply-*disqualified* path (the S21e/S21f trap),
  not an authentic demand bridge; the hard per-cell `SeededMinted`-sold-for-SALT==0 invariant is what
  classifies those cells as disqualified rather than as successes. A one-time stock cannot keep the
  demand side both *alive* and *hungry* through the pre-money wait. *S21h.1 — produced emergency
  self-provisioning (the authentic mechanism):* a **configured own-labor survival institution** — a
  produced, **no-grain-input**, low-yield, self-consumed own-labor bread floor (the engine's established
  own-labor subsistence tier, *not* ordinary grain→flour→bread production and *not* the removal of all
  survival scaffolding) that fires only near starvation and pulls hunger to one notch below the trigger
  — a recurring near-critical floor, immediately eaten, **no offerable remainder** (so it can never be
  sold for SALT or fake supply). **This threads the knife-edge:** it keeps 12 of the 18 non-lineage roles
  alive *and* hungry (so they still demand and buy bread), and **SALT promotes on the lineage's
  `SelfProduced` bread** (`seeded_minted == 0` entirely; the pre-promotion bread that monetizes SALT is
  `SelfProduced`, not minted or emergency). So the open colony finally has **money + mortality together —
  after a one-off cold-start cull** (6 of 18 non-lineage roles still die; starvation then *stops*: a
  partial bridged band, durable to 10k ticks, **not** full demand-side survival nor an ongoing
  positive-check band). A recurring produced floor sits in the window a one-time stock cannot: it keeps
  the role alive *without satiating it*. Robust across seeds {3,7,11,19,23} and the threshold sweep 7–11;
  every new field/flag defaults off, canonicalized ON-only with injective digest tags, conservation every
  tick; all 21 prior goldens byte-identical. *(Codex review-of-results: PASS-WITH-CAVEATS — no P0/P1
  result defect; the cushion negative is scoped to "no clean success in the tested sweep" and the
  coexistence claim to the partial one-off cull, per the wording above.)* **The robustness appendix
  (S21i, §8) later showed this coexistence is seed-robust but *band-qualified* (MIXED): load-bearing on
  WOOD scarcity (the lineage must be at the WOOD floor) and on the SALT-anchor density — not a broad
  result.**
- **S21i — the robustness appendix (done; see §8).** The S21f/g/h capstone is **MIXED**: seed-robust
  and grain-flow-robust, but load-bearing on WOOD scarcity (`lineage wood_provision`) and SALT-anchor
  density (`salt_direct_use_period`). The coexistence headline is real but **band-qualified**.
- **S22a — endogenize cultivation entry (done; see §9).** The first slice of role-topology
  endogenization: relaxing the lineage cultivation *privilege* to any spatial colonist under hunger
  pressure. Result = SUCCESS but **fluid/rotating participation**, not a stable occupational class.
- **Then / later:** occupational stickiness + profit-driven role drift (S22b+); endogenizing the
  clearing institution (the two-lane book is still configured); then the article (framed as *condition
  decomposition*, per §8's verdict). A follow-up **S21i-b** could decouple the emergency-floor
  target-pull depth (deferred from S21i as the one engine knob).

---

## 8. The robustness appendix (S21i): the capstone is MIXED — band-qualified, not broad

The whole S20→S21h arc demonstrated each regime at a **single shipped config** (mostly seed 7). Codex's
post-S21h evaluation named the matching credibility risk plainly: *a hostile reviewer will say you saved
the market by adding a configured no-input bread floor to the exact agents mortality killed.* S21i is
the honest answer — not hiding the floor, but measuring **how wide the window is** in which it works.
It sweeps the three open-colony scenarios across seeds and disclosed parameter bands and classifies
every cell with the **same** 5-tuple provenance/demand machinery the S21h suite uses (`survived`,
`demanded`, `promoted`, `bought_materially`, `provenance_clean`), under a verdict criterion designed to
be *auditable rather than tunable*: a CORE axis is **ROBUST** only with two SUCCESS steps on each side
of the shipped value (one step is gameable by coarse band spacing), every 1-D cell is classified across
three seeds, and the verdict test prints the bands + shipped index rather than asserting the answer.
The suite is purely test-additive (no engine change), so all prior goldens are byte-identical.

**Headline regimes are seed-robust.** Across 12 seeds `{3,7,11,19,23,29,31,37,41,43,47,53}` the three
regimes are stable: S21f SUCCESS, S21g CULL, S21h.1 SUCCESS — and the S21h.1 non-lineage survivor count
is **12 of 18 for every wide seed** (reported, not pinned). So the coexistence is not a seed-7 artifact.

**CORE axes (the capstone verdict):**

| Axis | Band (shipped\*) | Criterion | Note |
|------|------------------|-----------|------|
| `grain_regen` (pre-money food flow) | {12, 18, 24\*, 36, 48} | **ROBUST** | two SUCCESS steps each side |
| `emergency_hunger_threshold` | {7, 8, 9, 10, 11\*} | **BOUNDED-BY-AXIS** | shipped 11 = top validator bound (`< hunger_critical`); four SUCCESS steps on the low side |
| `lineage wood_provision` (WOOD scarcity) | {0\*, 1, 2, 3, 4} | **NARROW** | shipped 0 = the hard WOOD floor (low side bounded); **one notch (`=1`) flips every cross-seed cell to SURVIVED-NO-PROMOTE** — bread buying collapses ~6.6k → ~50 as the relaxed WOOD want stops driving the bread→SALT `IndirectFor{WOOD}` lane |
| `salt_direct_use_period` (anchor density) | {4, 6, 8\*, 12, 16} | **NARROW** | {4,6,8,16} SUCCESS but **period 12 is a non-monotonic SURVIVED-NO-PROMOTE hole** across all three cross-seeds (demand alive, provenance clean — SALT just fails to lead) |

**SENSITIVITY axes** (classified but excluded from the core verdict): strong-bar acceptors {1,2,3} all
SUCCESS; `min_indirect_target_goods=3` is infeasible in the two-target `{bread, WOOD}` topology
(skipped, logged — a failure there would be the topology, not S21h narrowness); role-count bands
(gatherers/consumers/founders) and mortality-timing bands (`birth_hunger_ceiling {8,10}`,
`death_window {2,3,4}`) all SUCCESS where feasible (`birth_hunger_ceiling=6` skipped infeasible). Both
interaction-map shipped cells (threshold×grain, WOOD-poor×anchor) are SUCCESS.

**Verdict: MIXED** — the headline is seed-stable and the CORE axes split (grain flow robust, emergency
threshold hard-bounded, but WOOD scarcity and anchor density narrow). The honest reading: **money and
mortality coexist in the open colony as an existence proof within a disclosed envelope, not as a broad
result.** It is *load-bearing on two conditions*: (i) the lineage sellers must be at the **WOOD floor**
— one unit of WOOD relief relaxes their unsatisfied WOOD want and the bread→SALT medium lane collapses
(answering the spec's own question, "how WOOD-poor must the colony be for SALT to lead?" — *maximally*);
and (ii) the regression-theorem direct-use **anchor density** has a non-monotonic hole (period 12 fails
where 8 and 16 succeed). This sharpens rather than retracts the capstone: the coexistence is real and
seed-robust, but it sits on a narrow ridge in WOOD-scarcity and anchor-density space — exactly the kind
of precondition a *condition-decomposition* account (not an "authentic spontaneous order" claim) should
state plainly.

---

## 9. Endogenizing the role topology (S22a): cultivation becomes a fluid survival behavior

Through S21, the open colony's **food-producing class was pinned** — a pre-identified cultivator
*lineage* supplied the `SelfProduced` bread; non-lineage buyers and woodcutters never cultivated. That
lineage privilege was the biggest remaining scaffold against "division of labor arises from *choice*,
not placement." S22a is the first slice of endogenizing it (Codex-scoped as the smallest meaningful
step): a default-off gate relaxes cultivation eligibility from "spatial household lineage member" to
**any spatial colonist under sustained hunger pressure**, reusing the *existing* S15/S21f
pressure/patience hysteresis — no profit optimizer, no new threshold; the opportunity cost is structural
(a cultivating tick cannot use the world-task slot). It is a steering-flag change (no vocation
mutation), additive and default-off, so all goldens stay byte-identical.

**Verdict: SUCCESS — but the honest result is *fluid/rotating participation*, not a stable
occupational class.** On the headline scenario (mortality on), across all five seeds: SALT promotes on a
clean (`seeded_minted==0`) supply, food is materially bought after promotion, a living non-cultivating
buyer cohort (7–10 of 18) persists, the WOOD↔SALT lane clears — **money and mortality survive the
relaxed producer identity.** But the cultivation is fluid: at any instant only ~5% cultivate (rolling
share settled), yet the membership rotates rapidly — churn ≈ 23–24 enter/exit transitions per
ever-cultivating non-lineage colonist over 1600 ticks, and *every* non-lineage role dips in at some
point. So the honest reading is *"everyone occasionally self-provisions under acute hunger, then returns
to buying,"* not *"a food-producing class self-forms."* S22a **dissolves the lineage privilege** —
cultivation is an endogenous survival behavior available to all under pressure — but it does **not**
demonstrate a stable, sticky division of labor (that is deferred to S22b+, along with profit-driven role
drift, specialized-producer entry, and a global role chooser). This endogenizes cultivation
*participation*, not the full vocation topology.

**Controls** (classify, not tune): the pinned-topology baseline still succeeds (the S21h 12/18);
money-machinery-off fails to promote; low/no grain-flow does not fake success (everyone cultivates,
nothing trades — a commune collapse); the mortality-off sanity variant succeeds. **Two control
findings** (Codex review-of-results, reported not forced): (i) the **no-hysteresis control creates no
distinct failure regime** — the headline already churns far above the oscillation limit, so removing
most of the hysteresis leaves aggregate stability intact while per-agent churn stays high in both; the
hysteresis is *not* load-bearing for aggregate stability here; (ii) the **no-emergency-floor control
does not reproduce the S21g cull** under endogenous entry — relaxing eligibility makes cultivation
itself a survival path, so the emergency floor is no longer the sole demand-side bridge. Both are honest
findings about how the relaxed topology changes the colony's survival structure. Codex review-of-results:
PASS-WITH-CAVEATS (no P1 code defect; the "stable class" framing was downgraded to fluid participation,
per the wording above).

---

## 10. Accumulated advantage alone does not produce occupation (S22b)

S22a showed cultivation is *fluid* participation, not an occupation. Codex's read was that hunger
pressure produces survival behavior, not specialization, and that the minimal **authentic** mechanism
for stickiness is **role-specific accumulated advantage** (capital/skill lock-in), not a generic
switching penalty. S22b tests the cleanest such mechanism: a default-off, per-agent **cultivation
skill** (born 0) that accumulates on *realized* cultivation output and slowly decays otherwise, and that
raises only the **grain hauled per cultivating trip** — the one conservation-safe lever (a bigger
debited draw on the conserved grain node, routed through a gated per-trip harvest-room override that
never mutates the agent's permanent carry capacity; the 1:1 grain→bread recipe is untouched). The whole
question hinges on the lever actually *biting*, so the suite carries a mandatory non-vacuity test.

**Verdict: `NoStickinessDespiteSkill` across all seeds — and it is a deep, well-isolated finding.** The
lever bites: the non-vacuity test passes on every seed (a max-skill cultivator harvests strictly more
grain — 144 vs 72, the 2× per-trip haul — and produces strictly more bread, ~124 vs ~72, than a skill-0
cultivator, under matched conditions over the same horizon). Money, mortality, provenance, and
conservation all survive. **But accumulated harvest-efficiency advantage does not change the
entry/exit dynamics:** under S22a's fluid hunger-driven participation, agents still rotate into and out
of cultivation on hunger regardless of skill, so per-capita churn stays at the matched-seed S22a
baseline (no fall toward the pre-registered 0.5× drop bar) and no persistent membership cohort forms.

The controls **isolate the mechanism rather than tune it**: even where skill *matures* —
`skill_gain=100` drives skill to ~995 (a mature cohort of 4–5 taking ~40% of harvested grain, a
diagnostic upper-bound), and the **no-decay** control ratchets skill to the cap (~39% grain share) — the
verdict is *still* `NoStickinessDespiteSkill`: no persistent cohort, churn unchanged, no monopolization.
So it is not that skill cannot accumulate; it is that **productivity-while-cultivating does not make an
agent decide to *stay* a cultivator** when hunger is what gates entry and exit. The boundary is named
precisely: occupational stickiness needs a mechanism that changes the *decision to stay* — heritable
skill, durable capital, or a profit-driven role chooser (S22c+) — or a participation regime that holds
an agent in cultivation long enough for advantage to matter. The robustness sweep maps the edges (a
starved grain node tips to oscillation; low/no grain-flow correctly classifies commune collapse, not
faked success). All goldens byte-identical; conservation preserved by construction. Codex
review-of-results: PASS-WITH-CAVEATS (no P1/P2 code-correctness defect — the over-carry path is
conserved; three P3 framing fixes folded in, incl. clarifying the churn metric).

This is the paper-strengthening outcome Codex predicted either way: a clean negative that **names the
next necessary condition** for an authentic division of labor.

---

## 11. A realized monetary stay-decision is not enough either (S22c)

S22b named the next condition: occupation needs a mechanism that changes the **decision to stay**, not
just productivity-while-in (the cultivation *exit* was hunger-only). S22c is the smallest authentic such
mechanism: a default-off **profit-driven retention** rule that, **only after money exists**, lets a
cultivating agent remain past the normal hunger-exit when its **recent realized cultivation-sale return
≥ its outside option**. Entry stays hunger-gated — *hunger discovers the role; money makes it
(potentially) persistent.* The signal is per-agent **cultivation-sale proceeds** attributed at sale-time
to the original producer (drawn from the seller's own `produced_lots`, ignoring resold/minted bread) over
a rolling 48-tick window, compared as a per-tick *rate* to the agent's realized outside rate (or a colony
reference). The hard anti-circularity guard: the rule is inert until `current_money_good() == Some(SALT)`
and the return windows fill only from post-promotion spot sales.

**Verdict: `NoStayDespiteProfit` across all seeds (skill-off headline and skill-on variant) — and,
critically, the signal genuinely BITES.** The mandatory non-vacuity test shows a real *counterfactual
exit flip* (a post-money agent past its hunger exit is retained where the matched flag-off run exits),
the cultivation-proceeds signal discriminates across ~20–24 agents, 4–7 agents are retained at any time,
and the retained cohort's grain share rises to 0.06–0.26 — all while SALT promotes on clean
`SelfProduced` bread, mortality is survived, and conservation holds. **But the realized monetary
stay-decision still does not produce occupation:** per-ever-cultivating churn falls only ~2.7 → ~2.4
(short of the pre-registered 0.5× drop bar) and *no persistent membership cohort forms*. A return-window
sweep (12/24/48/96) and a permissive sensitivity all stay `NoStayDespiteProfit`. The controls isolate
the mechanism cleanly: `signal-inert-pre-money` (every retention is post-money — anti-circularity holds);
`zero-returns` → SignalVacuous (the *signal*, not the rule's mere presence, drives any retention);
flag-off reproduces S22a; low-grain → SignalVacuous/commune. Codex review-of-results: PASS-WITH-CAVEATS
(no P1/P2 defect; the attribution, anti-circularity gate, and counterfactual flip all verified sound).

So the role-topology arc has a clean three-step shape: **hunger discovers the role (S22a, fluid) →
accumulated productivity doesn't change the exit (S22b, no stickiness) → even a realized profit *stay
incentive* retains only marginally (S22c, no-stay-despite-profit).** Each lever *bites* yet none yields a
durable cultivator class — the consistent signal is that occupation needs **durable lock-in** (heritable
craft across generations, or sunk role-specific capital that makes *leaving* costly), not a stronger
in-the-moment incentive. S22d then tested the durable-lock-in hypothesis itself.

---

## 12. Even sunk, owned capital does not produce occupation (S22d)

S22c named the candidate: durable lock-in (sunk, asset-specific capital). S22d builds it — a default-off,
buildable, **durable, agent-owned, role-specific cultivation tool** (a "plow"): a cultivator that
sustains realized output (a new tenure counter) invests a sunk WOOD+labor cost into a durable owned tool
(a dedicated good + `BuildCultivationTool` template + a separate pre-money build phase, never the
money-gated producer-capital path). The tool raises **only its owner's** grain-haul ceiling **while it
cultivates** (asset-specific, owner-exclusive, conservation-safe — a bigger debited node draw, the 1:1
recipe and `produced` accounting untouched), so the owner's durable comparative advantage flows through
the *unmodified* S22c profit-stay — no fiat flag, no exit edit.

**Verdict: `NoStickinessDespiteCapital` (4/5 seeds; `CapitalLeverInert` on the 5th where no tool
happened to form).** The lever bites *hardest* of the arc: the non-vacuity test passes every seed (a
tool owner harvests 7200 vs 2400 grain and bread under matched opportunity), and owners durably
out-produce — owner bread ≈6.4k vs ≈71 for the transient non-owners, owner grain share up to **0.71**.
Yet it still does not produce occupation: per-ever-cultivating churn falls only ~2.5→~2.3 (short of the
0.5× bar) and no persistent owner-cohort of four forms. **The WOOD-poverty confound is resolved by the
sweep:** the headline already uses a *cheap* tool (`tool_build_wood=1`, so capital forms), and across
`tool_build_wood ∈ {0,1,4,16}` × `haul_ceiling ∈ {1,2,3,6}` owner-share stays a tiny minority
(0.00–0.10) with no four-owner cohort even when owners take 41–71% of the grain — and at the highest
boost the buyer side collapses toward **monopolization** rather than a healthy split. The real boundary
is a **chicken-and-egg**: the lock-in asset can only be *earned by already sustaining* the fluid role, so
a rare one or two agents capitalize and dominate, never a class. Controls isolate it cleanly:
productivity-only (same boost to all, no owned asset) is not sticky; non-durable/rented (tool consumed
after one use) is not sticky; zero-build-input and the capital-alone (profit-stay-off) variant are
`CapitalLeverInert` across all seeds. Conservation, `bread_minted_max==0`, the tool-stock accounting
invariant (`built−destroyed==stock`), and a `plow_never_trades` guard all hold. Codex review-of-results:
PASS-WITH-CAVEATS (no P1; the negative is honest if **bounded** to *capital earned from within this
fluid, WOOD-poor regime* — it does not rule out pre-built/endowed, credit-financed, or
inheritance-heavy capital producing a class).

**The role-topology arc is now a clean four-step negative** — hunger, accumulated skill, a profit
stay-incentive, and even sunk owned capital each *bite* but none converts fluid participation into a
durable division of labor. The repeatedly-named next condition is an explicit **role-choice /
role-assignment institution** (or capital that is *endowed/inherited* rather than earned from within the
fluid regime), which is exactly the boundary the eventual article's division-of-labor section can rest
on as a sequence of falsified sufficiency claims.

---

## Appendix — milestone index

| Sxx | Title | Outcome |
|-----|-------|---------|
| S5/S6 | Specialized chain + provisioning at scale | mechanism |
| S7 | Producible capital | mechanism |
| S8/S9 | Money emergence (strong-bar, scaffolded) | mechanism (scaffold-dependent, per S12) |
| S10 | Originary interest / intertemporal choice | mechanism |
| S11 | Entrepreneurial error, profit/loss selection | mechanism |
| S12 | Retire the food mint | finding: emergence rested on a minted bread *supply* scaffold |
| S13 | Spatial households | structural prerequisite |
| S14 | Forage carrying capacity | mechanism: endogenous plateau (preventive check) |
| S15 | Pre-money cultivation | mechanism: Boserup intensification |
| S16 | Money from produced bread | finding: produced supply ≠ money (zero indirect breadth) |
| S17 | Mortality | mechanism: the full Malthusian band (positive check) |
| S18 | Produced multi-good money | finding: perfect coincidence → necessity beats the token |
| S19 | Imperfect-double-coincidence cycle | finding: token *leads* but clearing deadlocks |
| S20 | Two-lane bilateral order book | **resolution: endogenous token money emerges** |
| S21a | Marketability / carrying cost (open-colony slice) | finding: lever cuts *as-a-means* acceptance, but total-saleability conflates consumption with medium use → two-layer metric needed |
| S21b | Two-layer Mengerian saleability | **resolution: direct-use eligibility floor + medium-saleability leadership → the durable medium promotes; non-circular (controlled scenario)** |
| S21c | Open-discovery lane (capstone slice 1) | fix: a legacy direct-discovery lane lets a below-floor good cross the direct-use floor late (the open-colony prerequisite); all goldens byte-identical |
| S21d | Open survival, mortality off (capstone slice 2) | finding (Phase A): retiring the food scaffold collapses the pre-promotion barter market to zero trades — production is post-promotion only, so there is no pre-promotion food supply for the medium to circulate against; two-layer fixes the metric, not the supply; all goldens byte-identical |
| S21e | Finite seeded-surplus probe, mortality off (capstone slice 3) | finding (Success): a one-time finite tradeable bread supply is sufficient — SALT promotes (tick 37) as medium leader with {WOOD,bread} breadth before the seed's offerable surplus exhausts (tick 44), then production replaces it (zero seeded_minted in the tail); localizes the S21d block at supply *generation*; bounded diagnostic scaffold (S21f is the authentic mechanism); all goldens byte-identical |
| S21f | Endogenous pre-money household production-for-barter (capstone slice 4) | **SUCCESS — the supply question closed: lineage households cultivate bread (`SelfProduced`, zero `SeededMinted`) and barter the surplus pre-money; SALT promotes on it — money bootstraps from genuine pre-money production-for-barter, no seed/no mint** (gated cultivation-without-FORAGE seam; grain-bounded; mortality off); all goldens byte-identical |
| S21g | Mortality-on over the open-market colony (capstone, the positive check) | finding (money fails under mortality, a spec pre-named outcome): turning the S17 positive check ON (`hunger_critical=need_max`, `birth_hunger_ceiling=8`, the S17 deltas) over the S21f money colony culls all 18 non-self-provisioning market roles (SALT-rich buyers + woodcutters) in a one-off cold-start cull (~tick 7) before any market forms — SALT never promotes, no food is ever bought; the self-feeding cultivation lineage survives (cold-start timing trace: `first_hunger_drop 5 < first_starvation_death 7`) into a quiescent subsistence commune. Mortality-off control = S21f money works; neither endorsed rescue lever (grain-flow nor cultivate-timing) can rescue (the dying roles don't cultivate); robust across seeds; all goldens byte-identical |
| S21h | Demand-side survival bridge (capstone slice 6, the S21g resolution) | **SUCCESS — money and mortality coexist:** a produced no-grain-input own-labor emergency survival floor (a configured subsistence institution) keeps 12 of 18 non-lineage roles alive *and* hungry, so SALT promotes on the lineage's `SelfProduced` bread (`seeded_minted == 0`) under the positive check — after a one-off cold-start cull (6/18 still die, then starvation stops: a partial bridged band, durable to 10k). The bounded diagnostic (a finite *seeded* consumed-only cushion, S21h.0) is the **knife-edge finding**: no cushion size threads it cleanly — too small culls, too large sates out of the market; the diagonal never promotes, off-diagonal promotions are seeded-supply-disqualified. Robust across seeds + threshold sweep; all goldens byte-identical (Codex review-of-results: PASS-WITH-CAVEATS, no P0/P1) |
| S21i | Robustness appendix — does the S21f/g/h capstone survive the parameter space? | **MIXED (band-qualified):** sweeps the three scenarios across 12 seeds + parameter bands, classifying every cell with the same 5-tuple machinery (test-additive, all goldens byte-identical). Headline regimes seed-robust (S21f SUCCESS / S21g CULL / S21h.1 SUCCESS; 12/18 survivors every wide seed). CORE axes: `grain_regen` ROBUST; `emergency_hunger_threshold` BOUNDED-BY-AXIS (shipped = top validator bound); `lineage wood_provision` NARROW (shipped 0 = WOOD floor — one notch of WOOD relief collapses the bread→SALT lane); `salt_direct_use_period` NARROW (non-monotonic period-12 promotion hole). So money+mortality coexistence is real and seed-robust but **load-bearing on WOOD scarcity and SALT-anchor density** — an existence proof within a disclosed envelope, not a broad result (Codex review-of-results: PASS-WITH-CAVEATS, no P1) |
| S22a | Endogenize cultivation entry (role-topology slice 1) | **SUCCESS, fluid participation:** a default-off gate relaxes cultivation eligibility from "lineage member" to "any spatial colonist under sustained hunger pressure" (reusing the existing pressure/patience hysteresis, steering-flag only, all goldens byte-identical). Money + mortality survive the relaxed producer identity — SALT promotes on clean `SelfProduced` bread, a living buyer cohort persists. But it is **fluid/rotating participation, not a stable class**: ~5% cultivate at any instant (settled) yet all 18 non-lineage roles rotate through (churn ~23/agent) — "everyone occasionally self-provisions under hunger, then buys." Dissolves the lineage *privilege*, not a sticky division of labor (S22b+). Control findings: the hysteresis is not load-bearing for aggregate stability; the emergency floor is no longer the sole survival bridge (Codex review-of-results: PASS-WITH-CAVEATS, no P1) |
| S22b | Occupational stickiness via cultivation skill (role-topology slice 2) | **FINDING — accumulated advantage alone does NOT produce occupation:** a default-off bounded per-agent cultivation skill (born 0, accumulates on realized output, decays otherwise) raises only grain-haul capacity per cultivating trip (conservation-safe per-trip room override; goldens byte-identical). The lever BITES (non-vacuity passes: max-skill cultivator harvests 2× grain + more bread vs skill-0, every seed) and money/mortality/provenance/conservation survive — but skill does not change the hunger-gated entry/exit, so churn stays at the matched-seed S22a baseline and no persistent membership cohort forms. Even where skill matures (no-decay / high-gain → ~40% grain share) it is STILL no-stickiness. Names the next condition: occupation needs a mechanism that changes the decision to STAY (heritable skill / durable capital / profit-driven chooser, S22c+) (Codex review-of-results: PASS-WITH-CAVEATS, no P1/P2) |
| S22c | Profit-driven cultivation retention (role-topology slice 3) | **FINDING — a realized monetary stay-decision bites but does NOT produce occupation:** a default-off rule lets a post-money cultivator stay past the hunger-exit when its recent realized cultivation-sale return ≥ its outside option (per-agent proceeds attributed at sale-time to the original producer via `produced_lots`; rolling 48-tick rate; inert pre-money). The signal is genuinely non-vacuous — a real counterfactual exit-flip fires, it discriminates across ~20-24 agents, 4-7 are retained, grain share rises to 0.06-0.26 — and money/mortality/provenance/conservation all survive. But churn falls only ~2.7→~2.4 (short of the 0.5× bar) and no persistent membership cohort forms; a window sweep + permissive sensitivity stay NoStay. Controls: signal-inert-pre-money (anti-circularity), zero-returns→SignalVacuous, flag-off→S22a. Completes the 3-step arc (hunger discovers → skill doesn't change exit → profit-stay retains only marginally) → occupation needs durable lock-in, not an in-the-moment incentive (S22d+) (Codex review-of-results: PASS-WITH-CAVEATS, no P1/P2) |
| S22d | Durable role-specific cultivation capital (role-topology slice 4) | **FINDING — even sunk, owned capital does NOT produce occupation:** a default-off durable agent-owned cultivation tool ("plow"; new good + `BuildCultivationTool` template + separate pre-money build phase) raises only its owner's grain-haul ceiling while cultivating, flowing through the unmodified S22c profit-stay (owner-exclusive, conservation-safe, no fiat flag). Verdict NoStickinessDespiteCapital (4/5; CapitalLeverInert on the 5th). The lever bites hardest of the arc (owner 7200 vs 2400 grain matched; owner grain share up to 0.71) but churn falls only ~2.5→~2.3 and no 4-owner cohort forms. WOOD-poverty confound resolved by the sweep (cheap wood + big boost still no cohort; high boost → buyers collapse toward monopolization). The boundary is chicken-and-egg: the lock-in can only be EARNED by already sustaining the fluid role, so a rare 1-2 capitalize and dominate, never a class. Controls: productivity-only + non-durable not sticky; zero-build + capital-alone CapitalLeverInert. Completes a clean 4-step negative → occupation needs an explicit role-choice/assignment institution (or endowed/inherited capital), not a lever earned from within. (Codex review-of-results: PASS-WITH-CAVEATS, no P1; bounded to earned-from-within capital) |
