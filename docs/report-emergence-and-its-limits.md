# OIKOS — Emergence and Its Limits

*An experimental report on emergent money, capital, and survival in a praxeologic simulation.*

> Status: working research report (raw material for a future article). Covers the milestone arc
> through S24c (the money sub-arc, the full open-colony-capstone *supply* arc, the mortality-on cold-start finding, its resolution — a produced demand-side survival floor that lets money and mortality coexist — the robustness appendix that finds that coexistence is *band-qualified* (MIXED, load-bearing on WOOD scarcity and SALT-anchor density), and the role-topology arc: endogenizing cultivation entry (S22a — the food-producing class self-forms as *fluid* participation, not a stable class) and testing whether accumulated skill produces occupation (S22b — it does not; the lever bites but does not change the hunger-gated entry/exit) and whether a realized monetary stay-decision does (S22c — it does not either; the profit-stay signal bites with a genuine counterfactual exit-flip but retains only marginally) and whether sunk, owned, durable capital does (S22d — it does not either; the owner-exclusive cultivation tool bites hardest of all, owners take up to 71% of grain, yet capital can only be earned by already sustaining the fluid role so a rare few capitalize and dominate rather than a class forming) and whether capital given *up front and inherited* does (S22e — it does not either; an endowed minority of lineage households with plows that inherit down the line bites massively (641–681 plow→heir transfers per run, heirs cultivate) yet the cohort is flat 0/8 even at universal ownership because the hunger/profit *exit* rotates owners out regardless of who owns the means — a clean five-step negative whose consistent boundary is that occupation needs an explicit role-choice/assignment institution that overrides the exit, not capital of any provenance) — and then testing exactly such an institution (S22f — a *voluntary fixed-term cultivation commitment*: the arc's TURN; the formal predeclared aggregate-churn verdict stays NoStickinessDespiteCommitment, but for the first time the lever forms a persistent, renewing, **minority** committed cultivator core + a working **two-tier** division of labor with a surviving fluid buyer side — genuinely voluntary, not a re-pin (fiat-pin separates as RePinScaffold) — the churn clause failing precisely *because* the model now has a stable core plus a still-churning fluid tier; occupation needs an exit-overriding institution, and even then stabilizes a core, not the whole colony) — and then opening the PRIVATE-PROPERTY arc by testing whether scarce excludable LAND is such an exit cost (S23a — it is not, at least not as a use-it-or-lose-it forfeiture: with the capacity confound removed (the take-1 HardBarrier was a too-few-plots artifact, caught in review), owner-exclusive land lost-if-idle THRASHES — claim→forfeit→reclaim churn ~10× the commons, no stable cohort = NoStickinessDespiteLand — so the exit-cost INSTITUTION'S DESIGN matters: S22f's binding voluntary contract stabilizes a core, this involuntary forfeiture rule destabilizes; a non-forfeiting title / money land-market is deferred to S23b) — and then testing that money land-market (S23b — it does not stabilize either: a post-money alienable market with a genuinely endogenous capitalized-rent price (good plots ~86 SALT vs marginal ~1) goes *thin* — only 5–7 title trades, churn unchanged, no owner-cultivator cohort = LandMarketInert — and the honest caveat is that it is layered on S23a's already owner-collapsed base, so the S23 property arc delivers a clean pair of negatives: private land institutions, forfeiture OR market, fail to stabilize an occupation — forfeiture *thrashes*, the market goes *thin* over owner-dominance — unlike S22f's voluntary contract which preserved a two-tier market) — and then OPENING THE INSTITUTION-SELECTION arc by asking whether a *working* institution can itself EMERGE rather than be supplied (S24a — partly: gating S22f's commitment behind an adopts-norm bit, seeding a minority, and spreading it by generic local imitation of observed success yields a MIXED result — the norm genuinely spreads + recreates a core with all-generic copy drivers, but under sticky adoption usually over-spreads to a re-pin (3/5) or isn't separable from the random-imitation null (1/5 drift), with only 1/5 a clean bounded selection-driven success → a working institution CAN propagate by local selection but cleanly only sometimes; non-sticky/abandonable adoption deferred to S24b) — and then testing that abandonable adoption (S24b — it does not give the clean positive; it DISSOLVES the institution: with bidirectional generic-welfare imitation the norm dies back completely (NormDiesBack 5/5, final adopters 0) because the committed CULTIVATORS are not individually better off than the well-fed fluid BUYERS, so welfare-imitation flows AWAY from the productive role — a "tragedy of imitation": generic individual-welfare imitation does not preserve a division-of-labor institution when the producers who sustain it aren't better off than the buyers they feed. So generic-imitation institution selection is a KNIFE-EDGE: sticky over-spreads/drifts (S24a), abandonable dies back (S24b); the clean positive needs a role-crediting/group-payoff signal or explicit hysteresis = S24c) — and then testing group-payoff (S24c — it ALSO dissolves the institution: local GROUP-welfare imitation lands NormDiesBack 5/5 because the best-welfare GROUP is buyer-heavy, so the gradient selects away from adoption too; two rigor catches en route — a spatial-degeneracy artifact [groups collapsed to the whole population until keyed on a stable home-node anchor] and a verdict mislabel [GroupSignalVacuous→NormDiesBack, since the signal fires and selects away, not "no signal"]. This CLOSES the institution-selection arc as a clean TRIAD of negatives: sticky spreads-but-drifts / individual-abandonable dissolves / group dissolves — local welfare-imitation cannot SELECT a division-of-labor institution because its value is NON-LOCAL, realized through exchange from producers to buyers, so no local welfare observable makes the producer role look best; the institutional layer can be supplied and made to spread by ratchet but under non-circular local welfare-imitation it does not select))). Every result below
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

S22d named the remaining escape — capital that is *endowed/inherited* rather than earned from within —
and S22e tested it.

---

## 13. Even endowed, inherited capital does not produce occupation (S22e)

S22d's boundary was a chicken-and-egg: earned capital can't form a class because acquiring the lock-in
requires already sustaining the (un-sticky) role. S22e supplies the canonical escape — give the lock-in
**up front** and let it pass **down a lineage**. A default-off gate endows a minority of lineage
households with a durable plow at generation (deterministic hash selection, counted in the initial
conservation baseline) and adds a **plow estate-routing switch**: tools already inherit to the household
heir via the existing estate path, so the lever toggles whether plows keep that heir route (inheritance
on) or are forced to the commons (the falsifying control). Everything else is S22d/S22c unchanged — the
owner-exclusive haul boost, and stickiness only through the unmodified profit-stay (no exit edit, no fiat
flag). Endowment is restricted to lineage households (inheritance needs a household), so the question is
honestly about an endowed **dynastic/lineage** class, not a non-lineage occupation; the base is expanded
to `ROSTER_HOUSEHOLDS = 8` so the cohort bar (4 owner lineages) is reachable.

**Verdict: `NoStickinessDespiteEndowment` across all five seeds.** The lever bites *massively* — every
seed passes non-vacuity (the endowed owner out-produces a no-tool cultivator ~3×; **641–681 real
plow→living-heir inheritance transfers per run**, and those heirs cultivate; an owner enters the
retention signal) — and the precondition holds (the gate-off expanded base promotes SALT, sustains
mortality, keeps provenance clean, shows no cohort with high churn, buyers materially buy). Yet even
capital given **up front and inherited** does not retain: churn stays ~1× the matched baseline (not the
≤0.5× bar), cultivation share settles ~4%, and **no persistent owner-cultivator lineage cohort forms
(0/8)**. The result is genuine, not an unreachable bar (Codex verified the metric is feasible): across
the full `endowed_tool_count` sweep (1→2→4→8, owner-lineage share 0.12→0.25→0.50→**1.00**) the cohort is
**flat 0/8** and cultivation **flat ~4%** — and even at *universal* ownership (owner id-share 0.59, owner
grain share 0.79) owners do most of the cultivating but **no lineage persists** ≥50% of the final window;
they rotate. The binding constraint is the **hunger/profit exit**, which rotates owners out regardless of
whether the capital was earned, endowed, or inherited — not capital supply. Controls isolate it
(no-inheritance and productivity-only not sticky; too-many-tools → `UniversalOwnership`; no-endowment →
`EndowmentLeverInert`). Money + mortality + clean provenance + conservation + the tool-stock invariant
all hold. Codex review-of-results: **PASS-WITH-CAVEATS** (no P1/P2; bounded to *this endowed/inherited
plow institution in this configured WOOD-poor, mortality-on colony* — NOT a universal claim about capital
or inheritance).

**Through S22e the role-topology arc is a clean five-step negative** — hunger, accumulated skill, a profit
stay-incentive, sunk *earned* owned capital, and even *endowed + inherited* capital each *bite* but none
converts fluid participation into a durable division of labor. The decisive shift S22e adds: the boundary
is **not** the chicken-and-egg of *acquiring* capital (S22e removes that) — it is that the active
hunger/profit **exit regime rotates cultivation regardless of who owns the means**. The remaining named
condition is an explicit **role-choice / role-assignment institution** that overrides that exit — and
S22f tests exactly that.

---

## 14. An exit-overriding voluntary institution forms a stable occupational core — the arc's turn (S22f)

S22e named the boundary: not capital of any provenance, but the **exit** itself. S22f is the first lever to
touch it — a **voluntary fixed-term cultivation commitment** (Codex-scoped). Post-money, an eligible agent
whose own realized cultivation-return signal (a reusable predicate over the same S22c proceeds/outside data,
*not* the exit helper) clears its outside option may **choose** to commit to cultivation for a fixed term;
while committed, the hunger/profit exit **cannot** turn cultivation off; at expiry it re-decides from fresh
returns (a tracked renewal only if the signal still clears). It is the first and only exit edit in the arc,
gated entirely behind voluntary, signal-cleared, post-money entry; per-agent commitment state steers
`cultivating` (never `Vocation`), digest tag 12 ON-only; the headline runs on the expanded
`ROSTER_HOUSEHOLDS = 8` base with **no capital** (so any stickiness is the institution, not capital + a
contract).

**Formal verdict (by the predeclared aggregate-churn bar): `NoStickinessDespiteCommitment` 4/5 +
`TermTooShortFinding` 1/5 — a sixth classified negative.** But the formal label sits on top of the arc's
**first genuine positive sub-result**, and that is the real story. For the first time an institution
produces a **persistent, renewing, minority committed cultivator core with a working two-tier division of
labor**: 159–450 agents *voluntarily* commit per run (each traceable to its own cleared signal), ~1800–2100
eligible below-floor agents *decline*, 14–30 distinct committed ids cultivate ≥ ½ the final window (every
one has renewed from a fresh post-expiry signal — a bounded **minority**, commit-share 0.07–0.20), the
committed core takes 0.85–0.95 of grain, and a **fluid non-committed buyer majority survives and materially
buys** (17k–26k post-promotion). Money promotes, provenance is clean, mortality and conservation hold. The
**only** unmet success clause is aggregate per-ever-cultivating churn (2.67–2.75, not ≤ 0.5× baseline) —
and it fails *precisely because the model now has a two-tier structure*: a stable committed core plus a
fluid tier that still churns, and the aggregate (designed for the all-fluid S22a–e regimes) is dominated by
the fluid majority. Per classify-not-tune we keep the predeclared negative label rather than relabel it
`RoleStickySuccess` post hoc — but the honest scientific result is that **S22f is the arc's turn: the first
lever to stabilize an occupation at all.**

It is genuinely *not* a re-pin (Codex review-of-results: PASS-WITH-CAVEATS, no P0/P1): the `fiat_pin` control
classifies `RePinScaffold` (forced, signal=0, no fresh-signal renewals), `unprofitable_offer` gets zero
uptake (`CommitmentUnchosen`), `nonbinding_term` (=1) forms no persistence, and the voluntary headline has
~1800 below-floor decliners + a live fluid remainder. A separate composition observation: commitment **+**
capital (the earned/endowed secondary variants) tips into monopolization — the committed core takes all
grain and the lineage goes extinct — a cull regime, reported as its own finding, not the no-capital
headline. The honest frame for the whole arc: **five levers that leave the exit intact each fail to produce
a division of labor; the sixth, which overrides the exit by voluntary contract, finally produces a stable
occupational core and a two-tier market — though not a colony-wide fall in occupational churn.** Occupation,
in this model, needs an institution that changes the *exit*, and even then it stabilizes a *core*, not the
whole colony.

---

## 15. Private land tenure as an exit cost — a use-it-or-lose-it property rule *thrashes* (S23a)

S22f showed a *contract* (voluntary fixed-term commitment) can make the exit costly enough to stabilize a
core. S23 opens the **private-property arc** and asks whether the canonical real-world exit cost — **scarce,
excludable, losable land** — does the same. The whole S22 arc ran on a world where the means of production is
a *commons* (a `ResourceNode` has no owner; a lapsed farmer re-enters for free), so S23a switches on the
missing precondition: grain plots become **heterogeneous, excludable, claimed by money-free homesteading
labor, harvested owner-only, lost if left idle, inherited** — with the re-entry penalty made **spatial**
(abandon your good central plot and a nearer agent takes it; you can only re-homestead far, poor land), which
sidesteps the money-bootstrap trap. Only the ownership layer is new; the spatial substrate (heterogeneous
node quality, distance, pathfinding travel cost) already existed.

**The arc's first methodological lesson — a confound caught and corrected.** S23a take-1 returned
`HardBarrier` across the board, which would read as "private land closes entry." But the colony had ~48
agents and only ~12–24 plots, and the sweep never varied *total land* — so open entry was impossible *by
construction*, and the verdict was a **capacity artifact**, not an economic finding (caught in
review-of-results, not accepted). Take-2 added a **population-scaled total-land axis** (total plots
{12,24,48,96}); `HardBarrier` then **vanishes** once adequate viable marginal land exists, confirming the
take-1 result was an artifact. (A second catch: a verdict mislabel — the classifier had called the
churn-didn't-drop case `CommonsEquivalent`, but a regime with churn ~10× the commons baseline is the opposite
of "title inert"; corrected to `NoStickinessDespiteLand`.)

**Verdict (take-2, terminal): `NoStickinessDespiteLand` across {3,7,11,19,23}.** With adequate land and
genuinely open entry (viable marginal plots 6–10, observed non-owner claims, buyers survive and materially
buy, money promotes, conservation/provenance/registry guards hold, mechanism non-vacuous), private
owner-exclusive **use-it-or-lose-it** tenure still does **not** stabilize an occupation — it **thrashes**:
plots are claimed → lost-on-idle → reclaimed-by-another in rapid succession, so churn *explodes* to ~26–27
per ever-cultivator vs the matched commons baseline ~2.6 (~10×), owner share runs 0.75–0.80, and no
persistent bounded-minority cohort forms. No success window appears anywhere in the capacity grid. Controls
separate cleanly (`property_off`/`non_excludable_deed`/`free_reclaim`/`no_forfeit` → `TenureLeverInert`).
Codex review-of-results: PASS after the relabel.

**The lesson — the exit-cost institution's *design* matters.** S22f's *binding voluntary fixed-term*
commitment stabilized a core; S23a's *involuntary forfeiture* land rule **destabilizes** (thrash), even
though both make leaving costly. Making exit costly is necessary but not sufficient; the cost has to bind in
a way that doesn't itself churn. A non-forfeiting title, or a money land-market that capitalizes spatial rent
into a price (so land is *sold*, not lost-on-idle), is deferred to **S23b**.

---

## 16. A post-money land market goes *thin* over owner-dominance — the other property failure mode (S23b)

If S23a's forfeiture rule was too *harsh* (you lose land by not working it), S23b tests the canonical,
gentler property institution: **alienable, scarce, illiquid land priced *after money exists*.** Pre-money the
colony homesteads as before (idle-forfeiture off from tick 0, so SALT still bootstraps from `SelfProduced`
barter); **post-promotion**, plots become assets **bought and sold for SALT** at an **endogenous** price
(capitalized from each plot's realized grain yield + local sale history), holding a plot costs a disclosed
**carrying cost**, leaving means **selling** (not forfeiting), and re-entry means **re-buying** — so a lapsed
farmer who sold its land and spent the proceeds on food is **priced out** of comparable land. This *budget
hysteresis* is a genuinely new stabilizer: not S22f's contract, not S23a's forfeiture, but the everyday
illiquid-asset exit cost. The central methodological risk (and the bulk of the spec) was making the price
**endogenous** — capitalized rent, never a hardcoded "expensive enough to stick" constant, which would be a
fiat pin.

**Verdict: `LandMarketInert` across {3,7,11,19,23}.** The price *is* genuinely endogenous — good (high-regen)
plots trade ~86 SALT against marginal plots' ~1 (a rent-tracking gap), trades occur only post-promotion,
foreclosures fire, carrying costs accrue, priced-out traces exist, and conservation/registry/SALT-accounting
guards hold every tick. But the market is **too thin to be load-bearing**: only **5–7 title trades per seed**
(below the predeclared non-vacuity floor of 8), churn stays ~20 against the matched no-market baseline's ~21
(no fall), and **no stable owner-cultivator cohort forms** anywhere in the price-cap/carrying-cost sweep. At a
zero price (free land) trades jump into the hundreds but the rent signal vanishes (no hysteresis) — also
inert. So neither extreme stabilizes.

**The honest caveat — and what it reveals.** This is *not* a clean test of "land markets in a functioning
two-tier colony": the matched no-market baseline on this population-scaled S23a base is **already
owner-dominant and buyer-thin** (one food-buyer, owners taking ~95% of grain) — because S23a's private tenure
*already* collapsed the buyer side. S23b therefore tests whether an alienable market can *rescue an
already-collapsed private-tenure regime*, and the answer is no: it trades too thinly to rebuild a buyer
economy that private property dissolved. That narrower reading is itself the finding. Taken with S23a, the
**S23 property arc** delivers a clean pair of negatives: **private land institutions, in both forms tested,
fail to stabilize an occupation — forfeiture *thrashes*, an alienable market goes *thin* over the
owner-dominance that private tenure produces** — in pointed contrast to S22f, where a *voluntary contract*
preserved a working two-tier market. A land market over a deliberately *non-collapsed* two-tier base is
clearly-scoped future work, not a re-run of S23b. (Process note: the rb-lite review panel was misconfigured
this milestone — it carried a stale reviewer checklist from an earlier milestone — so the result was gated on
independent verification plus an adversarial review-of-results instead; goldens remained byte-identical and the
verdict reproduced deterministically.)

---

## 17. Can an institution *spread*? Endogenous commitment-norm propagation — a mixed result (S24a)

Every institution to this point — the exchange rule, the SALT anchor, S22f's commitment contract, the S23
property regimes — was *supplied*: switched on globally by a flag the experimenter set. The natural final
question is whether the institutional layer itself can **emerge**: can a *working* institution stop being
experimenter-universal and **spread through the population on its own**? S24a opens the **institution-selection
arc** with the cleanest version: take S22f's commitment contract unchanged, but gate its availability behind a
per-agent `adopts_commitment_norm` bit; seed a small deterministic minority; and let the norm spread by
**generic local imitation of observed success** — periodically a non-adopter copies a nearby agent's norm bit
if that neighbour is doing better on a *generic* survival score (alive / low hunger / recently fed),
**never** on "is a committer" or "commitment pays." The hard part, and most of the spec, was keeping that
non-circular: if the score read commitment identity or profit it would be a disguised fitness oracle, so the
score is normalized over generic observables only, a score-purity invariant forbids reading institution
fields, and a `random_imitation` null (copy an outcome-*blind* random neighbour) must fail to reproduce the
result.

**Verdict: a mixed finding, honestly `{3 UniversalCommitmentRePin, 1 DriftNotSelection, 1
InstitutionSpreadSuccess}` across the five seeds.** The norm genuinely spreads — a ~12-agent seed grows to
~50 adopters, non-seed agents enter real commitments and renew, a committed core forms, every copy is driven
by a generic observable (SALT contributed to zero copies), and money/mortality/provenance/conservation hold.
But the spread is *not reliably clean*: in three seeds adoption over-shoots 60% and the fluid buyer tier
collapses (`UniversalCommitmentRePin` — effectively re-imposing S22f-for-all); in one seed the **outcome-blind
random null reproduces the same core**, so the spread there is drift, not selection (`DriftNotSelection`); only
one seed shows the target — bounded adoption (57%), surviving buyers, and a core that beats its own random null
(`InstitutionSpreadSuccess`). So a working institution *can* propagate by genuine local selection, but under
**sticky (never-abandoned) adoption** it usually over-spreads or cannot be separated from drift.

**Two honesty notes that the rigor surfaced.** First, the rb-lite build died in its first iteration (the
orchestrator process was swept on the shared machine and the implementer orphaned and hung *after* the suite
passed), so there was no review panel; the result was gated entirely on independent re-verification plus an
adversarial review-of-results, which returned **FAIL as originally classified**. The original classifier used
an *aggregate* drift test (random must reproduce the core on *all* seeds to count as drift) that masked the
single drift-contaminated success; the fix made the drift comparison **per-seed**, which correctly demoted that
seed. Second, the `unprofitable_seed` control (a non-binding `commitment_term=1`) still *spreads the bit* —
because even a one-tick commitment lets an adopter cultivate and eat once, conferring a transient generic food
advantage — yet forms no core, which is its own small finding: spread of the *practice* is not the same as
emergence of the *occupation*. The clean positive test — **non-sticky / abandonable adoption**, which should
make the drift null clean and let adoption settle below saturation — is the clearly-scoped next slice (S24b).

---

## 18. Abandonable adoption *dissolves* the institution — a tragedy of imitation (S24b)

S24a's mixed result had a single named cause: *sticky* adoption, a bit that once set never clears, so
outcome-blind copying ratchets it toward saturation. S24b removes the confound in the cleanest way — adoption
becomes **abandonable**: each imitation step is now *bidirectional*, copying the better-off observed
neighbour's norm bit whether that neighbour is an adopter or not, on the same generic welfare score (a
committer who is not doing better simply drops the norm; abandonment inside a binding commitment term is staged
and applied at expiry, so no term breaks mid-way). The hypothesis was that this turns an outcome-blind null
from a ratchet into a random walk and lets the genuine signal settle at a bounded equilibrium — the arc's
potential first *clean* positive.

**Verdict: `NormDiesBack` 5/5 — and the way it fails is the finding.** With abandonment on, the norm dies back
**completely**: final adopters reach zero on every seed, and the only norm-flips recorded are abandonments
(`adopt = 0`, `abandon = 7–10`). No non-adopter ever imitates *into* the committed role, and the seed adopters
drop it at expiry. The reason is economically pointed, and visible in the metrics: **on a generic
individual-welfare score the committed cultivators are not better off than the well-fed fluid buyers** (buyers
run 37–48 alive with 15k–32k units bought — they eat plentifully by *buying*, while the cultivators carry the
production burden). So welfare-imitation flows *away* from the productive role: everyone imitates the
comfortable buyer, and the role that *sustains* the division of labor is the one no one wants to copy. S24a's
institution only ever spread because sticky adoption **ratcheted** it (seed adopters could not leave); remove
the ratchet and generic-welfare selection dissolves it.

**The insight — a tragedy of imitation.** *Generic individual-welfare imitation does not preserve a
division-of-labor institution when the producers who sustain it are not individually better off than the
buyers they feed.* Taken together, S24a (sticky → over-spread, indistinguishable from drift) and S24b
(abandonable → die-back) show that generic-imitation institution selection is a **knife-edge**: neither the
never-abandon nor the freely-abandon extreme yields a clean, bounded, selection-driven equilibrium. This is
not a failure of the test but a result about *selection*: an institution can be collectively productive and
still lose under individual-welfare imitation, because the payoff to *joining* the productive role is not what
imitation rewards. (Verified independently — the null *does* adopt and the sticky reference *does* form cores,
so the result is genuine, not a dead copy path; rb-lite converged clean in three rounds, and Codex
review-of-results confirmed the label and the reading.) The clean positive now requires a genuinely different
mechanism — **role-crediting / group-payoff imitation** (imitate what the *productive* agents do, or what
groups that contain them achieve) or **explicit adopt/abandon hysteresis** — which is the next slice (S24c),
not a re-run.

---

## 19. Group-payoff imitation also dissolves it — closing the institution-selection arc (S24c)

S24b's diagnosis was that *individual*-welfare imitation rewards the comfortable buyer, not the committed
producer. The natural repair is to change the **unit** success is measured over: if agents imitate the
practice of the better-off *neighbourhood* rather than the better-off *individual*, a group that internalizes
the producers' contribution — they feed the group — might carry the norm. S24c keeps S24b's abandonable
adoption but scores imitation on **local group welfare**: an agent selects the best-off nearby group by
generic welfare aggregates (alive share, hunger relief, food — never any commitment field), then copies toward
that group's **adopter-share gradient** (adopt if the better-fed group has materially more adopters than its
own, abandon if fewer). The score never reads institution identity — the group is chosen on welfare; only the
copy *direction* follows the visible norm distribution.

**This milestone earned its result through two review catches, both the kind the discipline exists to force.**
First, the mechanism was *mechanically dead*: the base colony's non-hauling majority shares one exchange tile,
so every "group" collapsed to the whole population and **zero** group-copy events fired — a spatial-degeneracy
artifact the reviewers caught by instrumenting the run, not a finding. Keying group membership on each
colonist's stable economic anchor (its home node) restored genuine distinct neighbourhoods, and the mechanism
then fired for real (ten group-copy events per seed, ~5000 covariance samples). Second, the first verdict was
*mislabeled* `GroupSignalVacuous` — but a run with positive group-copy advantages every seed and a positive
welfare↔adopter-share covariance in four of five seeds is not "no signal"; a signal that fires and selects
*away* is `NormDiesBack`. The adversarial result review corrected the label.

**Verdict: `NormDiesBack` 5/5.** The group mechanism fires and the group signal is present — adopter-heavy
neighbourhoods even tend to eat *slightly* better (positive covariance in four of five seeds) — yet every copy
is an **abandonment** (`adoptions=0, abandonments=10`), because the **best-welfare group is buyer-heavy**: the
best-fed neighbourhood is dominated by buyers who eat plentifully by purchasing, so the adopter-share gradient
points away from adoption everywhere and even the seeded adopters drop the norm. Group-welfare imitation does
**not** rescue the institution. (Scope, disclosed: the mechanism fires cleanly for the anchored agents; a
synthetic anchor for the tile-sharing majority was tried and rejected because it broke the null and
unprofitable controls, so S24c tests group-payoff as far as this base's spatial structure allows — a fully
spatially-dispersed colony is future work.)

**This closes the S24 institution-selection arc as a clean triad of negatives.** A working institution, made
available behind an adopted-norm bit and left to spread by *local welfare imitation*, does not stably emerge:
**sticky** adoption spreads it but is indistinguishable from drift and over-shoots (S24a); **individual
abandonable** welfare imitation dissolves it (S24b); **group** welfare imitation also dissolves it (S24c). The
unifying reason is structural: the value of a division-of-labor institution is **non-local** — it is realized
through *exchange*, flowing from producers to buyers across the market — so no purely *local* welfare
observable, individual or group, ever makes the producer role look best, and imitation-of-success drifts away
from the very role that sustains the institution. Selecting such an institution would require a signal that
sees past local welfare — a market-mediated or global payoff, explicit contribution accounting, or
group-level reproduction/selection — each a genuinely new mechanism, and the honest boundary at which this
study leaves the question: **the institutional layer can be *supplied* and can even be made to *spread* by
ratchet, but under non-circular local welfare-imitation it does not *select*.**

---

## 20. Revisiting private property with mortality and inheritance — two honest nulls (S23c, S23d)

The property arc (§15–16) left an unanswered objection. S23a's use-it-or-lose-it tenure *thrashed*, but was that the
fault of private property or of **insecurity** — a title you lose the moment you stop working it? And the whole
property arc ran on a world where the people who *owned* land and the people who *died and had children* were
different populations, so the one mechanism most economists would reach for — **inheritance** — never even
engaged. This short coda returns to property with a generational lens, and produces two honest nulls that sharpen
the boundary rather than crossing it. Neither is merged; both are preserved on branches as findings.

**S23c — secure, heritable title: the engine works, but the base makes it vacuous.** S23c builds the honest
counterfactual to S23a: a **secure** (never-forfeited) title plus a full **universal-heir inheritance engine**
(every dying owner has a determinable heir by a pinned deterministic order — live child, then nearest kin, then
household successor, then colony next-of-kin, else revert unowned) and a **partible** regime that splits a plot
into conserved fractional shares among co-heirs. The engine is correct — probe-verified by forced mortality, with
exactly-one-successor and no-dead-owner-plot conservation invariants — but the study is **`DisqualifiedNoInheritance`**:
on the shipped base it *never fires*. The reason is structural and worth stating plainly, because it had been
implicit in the whole arc: OIKOS runs **two disjoint populations**. The persistent cultivator-owners are drawn from
an **immortal** standalone roster (they cultivate every tick, so under homesteading they win and keep the plots) —
and they never age or reproduce. The agents who *do* die and bear children are the mortal lineage households, who
are hearth-fed consumers and rarely hold a plot at death. Owners ∩ reproducers ≈ ∅, so land is never bequeathed —
zero inheritance events across three hundred death traces. The heir engine is a working answer to a question the
base never asks.

**S23d — make the owners mortal and reproducing: the vacuity is solved, but the base is subsistence-bound.** S23d
is not a lever test; it is a **base-building** milestone whose only job is to make the disjoint populations one, so
that a later re-run of S23c would be non-vacuous. It composes already-verified seams — household production-for-
barter (S21f), hunger-driven cultivation entry (S22a), the own-labor emergency survival floor (S21h), homesteading
claims (S23a), and S23c's secure ownership substrate with forfeiture off — so that the **mortal, reproducing lineage
households are themselves the persistent cultivator-owners**. A strict owner-identity invariant guards against the
obvious cheat: an owner must be a lineage *reproductive actor* (in the birth/kinship graph), not merely something
with a finite lifespan. And it works, on its own terms: owners are genuinely mortal lineage reproducers (no immortal
and no non-lineage owner residue for a single tick), they die of old age, born-in-sim children grow up and take
plots, money promotes on self-produced bread — and **inheritance fires endogenously**, twenty-plus eligible owner
deaths with living heirs and real heir transfers, in normal play with no forced mortality. **The S23c vacuity is
genuinely removed.**

But the composed colony is **`DemographyBaseUnviable`** across every seed, and the reason is the deeper finding.
The economy is **subsidy-bound**: in the final window *all* consumption comes from the own-labor emergency floor and
non-owner buyers purchase *nothing* through money. When every agent can feed itself by its own labor, the owners'
cultivated grain is never worth buying — there is no gain from trade, so no producer/buyer money economy forms.
This is not a wiring artifact (the floor runs *after* market clearing and nets out what the market already supplied;
it does not pre-empt a purchase anyone wanted to make), and it should not be dressed up: money *promotes* here, but
without sustained exchange that is monetary classification, not a two-tier market. It echoes the oldest result in
the project — S21g's lesson that **subsistence redundancy must precede specialization**; a division of labor needs
self-provision to be *worse* than specialize-and-exchange, and here it isn't.

**The honest boundary, and one risk we have not retired.** Making landowners mortal-and-reproducing dissolves the
inheritance vacuity but reveals that *this* base — with an unlimited own-labor survival floor — is subsistence-bound,
so the generational tenure study still has no viable ground to run on. A second caution rides underneath: adult
lifespans here are short (a mean on the order of tens of ticks), and while S23d proves mortality no longer blocks
*inheritance*, it does **not** prove that short-lived owners can accumulate and trade a marketable *surplus* before
they die — the metrics show frequent turnover and real replacement, but also a lineage-continuity count that touches
zero, so surplus formation under mortality is an open structural question, not a solved one. The disciplined next
step is therefore **not** to tune the floor downward until a market appears — that is the exact knob-turning the
"classify-not-tune" rule forbids — but to introduce an independently-justified scarcity of the *outside option* (a
finite, regenerating rival commons in place of the unlimited floor), with controls that make it fail where it
should, and with new instrumentation for owner tenure, surplus-sold-before-death, and inherited capital. Until such
a base is shown viable on its own terms, the S23c generational study stays deferred. Two clean nulls: the
inheritance engine works but the old base couldn't exercise it; the fixed base exercises it but cannot yet sustain
the market that would give tenure something to bite on.

---

## 21. Scarcity of the outside option does not force the market — the missing piece is buyer income (S23e)

S23d's diagnosis pointed at one lever: the base was subsistence-bound because the own-labor survival floor was
*unlimited*, so self-provision was always adequate and no one needed to trade. The disciplined repair is to make
the outside option **scarce** — but by an independently-justified mechanism, not by turning the floor's knob down
until a market appears. S23e does exactly that: it replaces the unlimited floor with a **finite, regenerating,
non-excludable rival subsistence commons**, so subsistence access becomes *rivalrous* and private land finally has
a reason to be bought. The anti-tuning guard is load-bearing here and it held: the scarcity is a single fraction
`φ` of a **measured** baseline throughput `D0` (`r = φ·D0`; `D0 = 12768` is pinned to the S23d flag-off emergency
draw and guarded by a live-measurement test), and `φ` is **swept** across abundant, marginal, and scarce rather
than searched — so the result cannot be an artifact of a fitted parameter.

**No level of scarcity forms a market.** Abundant (`φ = 1.25`) reproduces the S23d subsidy-bound null (the control
behaves, survival ≈ 0.69). Scarce (`φ = 0.25`) is too harsh in the other direction — it collapses the money
bootstrap before any market can form (survival ≈ 0.17, money never promotes), a starvation-pressure regime rather
than the economic point. The informative cell is **marginal** (`φ = 0.5`): money *does* promote, the commons *is*
scarce, and the owners *do* produce and sell a surplus — and yet the living non-owner buyers buy **nothing**
(`buyer_bought = 0`, the whole diet still comes from the commons). Scarcity plus owner surplus, together, are not
sufficient.

**The finding names the true constraint — and it is neither scarcity, surplus, nor mortality.** An adversarial
result review confirmed with code evidence that this is not a wiring confound (the "hungry non-owner buys owner
bread to eat" path is real and reachable — it is simply not taken) and not the short-lifespan wall (owners
demonstrably accumulate and sell surplus). What is missing is a **demand-side income loop**: the non-owner buyers
have no sustained purchasing power — a one-time money endowment that depletes, with no wage, rent, or payment
stream to renew it — so when the commons runs short they cannot buy the owners' bread; they simply go hungry. On
this base, *replacing the unlimited outside option with a scarce one does not force a producer/buyer market,
because the demand side has no way to earn.* The property arc's generational strand has now walked its constraint
down to a single load-bearing joint: S23c (inheritance inert on disjoint populations) → S23d (populations fixed,
inheritance fires, but the base is subsidy-bound) → S23e (scarcity does not rescue it — the missing piece is buyer
income). The honest next step is not another property or scarcity lever but a **buyer-income / money-circulation
mechanism** (a labor market, or a wage/rent/payment loop) that gives non-owners recurring purchasing power; until
the demand side can earn, land tenure has no market to bite on. (Process note: this milestone's automated review
degraded — one of the two reviewers failed to launch — so the result rests on independent re-verification and the
second-model result review rather than the build panel; it is preserved as a branch-only null, not merged.)

---

## 22. The buyer-income lever, built — voluntary wage labor does not bootstrap the circular flow (C1)

S23e named the missing joint precisely: a **buyer-income / money-circulation mechanism**, a labor market or a
wage/rent/payment loop that gives non-owners recurring earnings. C1 builds exactly that — the first milestone of a
**civilization-core arc** (a C1–C10 roadmap; see `docs/spec-civ-core-roadmap.md`) — and takes it as far as a
voluntary wage market goes. On the S23e **marginal** rival-commons mortal-landowner base (φ = 0.5, the informative
cell: money promotes, owners hold a sellable surplus, buyers buy nothing), an owner may hire a hungry non-owner for a
money **wage paid *now* from the owner's own prior sale earnings** (an anti-subsidy retained-earnings ledger credited
only by realized bread sales). The worker's ask is the ordinal least wage that provisions a money want ranked above
Leisure; the employer's bid is a new **own-money** appraisal — the wage debited from present gold against the output's
expected sale proceeds as a dated receivable, with **no credit created** — imputed from the *last realized bread spot
price the poor buyers actually pay*. Everything is conserved (a dedicated money/escrow invariant beside the per-good
identity), post-money, byte-identical off-path.

The result, across all five seeds and the whole φ sweep, is a third, sharper null: **`WageMarketVacuous` — no
voluntary owner–worker wage contract clears at all.** Not `CircularFlowForms` (the hoped-for success), and not even
`WageInertDemandStillDead` (wages clear but demand stays dead): the wage market never opens, because the owner's
own-money willingness-to-pay sits *below* the worker's reservation ask. And the null is **economic, not a wiring
artifact** — a `FiatWage` control that *forces* hiring clears eight hires (with mortality and conserved escrow),
proving the matching and escrow machinery is fully reachable; the voluntary market simply does not cross. Nothing is
tuned: `circular_flow_forms = 0/5` at *every* swept threshold (the loosest — one hire, a 5 % wage-financed buy share,
any positive velocity) and at every φ (scarce collapses before money even promotes; abundant reproduces the null).
The two scaffold controls separate cleanly (`FiatWage`/`SubsidisedWage` → `WageScaffoldOnly`), and the no-wage control
reproduces the S23e null.

The reading is that the demand-side deficit is **self-reinforcing**, and voluntary bilateral wage contracting cannot
break it from the inside: an owner cannot pay a living wage because it cannot sell bread for enough SALT, because the
buyers are poor, because there are no wages — a bid–ask gap that will not close by itself. This is the employer-side
chicken-and-egg made concrete. A second-model result review confirmed it with code evidence (the employer bid is
genuinely own-money and grounded in the real realized price; ask and bid are commensurate total-wage bundles; no
smuggled credit; conservation and provenance faithful) and classified it **ACCEPT-AS-HONEST-NULL** with no
confound.

The scoping matters, and Austrian capital theory supplies it. C1 ruled out exactly **one** voluntary institution —
post-money **money-wage** contracts — on a base with no accumulated monetary savings. That is what the wages-fund
doctrine predicts (Böhm-Bawerk, Strigl): a wage is an advance of *present goods out of prior saving*, so where no
subsistence fund has accumulated, no money-wage market should form. Read correctly, the null **confirms the Austrian
sequencing** — capital accumulation precedes wage labor — rather than indicting voluntary exchange. The levers the
finding actually points to, untested and first in line, are therefore **voluntary**: **in-kind (natural) wages** —
the owner pays the worker in *bread*, collapsing what C1 measured as a *money* bid–ask gap, exactly how thin-capital
economies historically paid labor (the spec's "wages are a monetary phenomenon" gate was a modeling choice, and this
result suggests too restrictive a one); **share contracts / tenancy** — the worker works the owner's land for a
share of the output, requiring *no wage advance at all*, the institution that historically dominates precisely where
capital is too thin for advances (the C4 shape); and the **accumulation horizon** — whether a wages-fund can form
endogenously once lifespans and trade allow, after which money-wage labor has its Austrian preconditions. A state
that hires and spends first (a fiscal circuit atop the M21 result), or a credit advance against future output,
remain **interesting interventionist comparisons** — in the game the state is exactly that, an *optional
intervention the player can try*, never a requirement for the economy to live — but they are experiments beside the
voluntary levers, not the implied fix. C1 is preserved as a branch-only null (`feat/wage-labor-impl-rb`), not
merged — its spec merges only on `CircularFlowForms`, which it did not reach.

---

## 23. The no-advance contract clears — voluntary share tenancy opens, as transient scarcity relief (C1R)

The replan's first milestone (`docs/review-and-replan-2026-07.md` P1) built the labor institution that
needs no wages-fund at all: a **voluntary output-share tenancy**. A landless worker (the class the S23d
owner-identity design structurally excludes from homesteading — a disclosed scope condition) works an
owner's plot for a pinned, swept share of the realized bread. The owner side is honest by construction:
only **at-cap plots** may be shared — where further regen is literally being destroyed — with the
worker's draw bounded to the regen rate, so the owner's share is harvested deadweight, never a drawdown
of stock it could have used later (`share_stock_drawdown = 0` held everywhere). Both acceptances are
ordinal; no money is required (the phase is not promotion-gated — share tenancy is praxeologically
prior to monetary wages); nothing is advanced by anyone.

**The contract clears.** Across all five seeds at the marginal commons, 31–54 voluntary share contracts
per run form and move real bread — 600–6,250 loaves of worker share income per run, split at exactly the
pinned ratios across the share sweep — making this **the first voluntary labor institution ever to open
on a mortal base**, and doing so *pre-money-capable* and advance-free. The same-seed comparative cell
prints the money-wage result beside it: `WageMarketVacuous` on the identical base and seed. The C1
reading is thereby demonstrated, not just argued: the wage failure was an *advance/money* gap, not a
labor-exchange gap. Voluntariness shows twice — under an **abundant** commons uptake is exactly zero
(contracting is a genuine scarcity phenomenon, switched off by the worker's own outside option), and a
forced-share control separates as scaffold.

**But it clears only as transient relief — the honest other half.** No voluntary contract is ever
renewed, and none exists in the final window, so no survival lift over the no-contract control forms:
the classified verdict is **`ShareClearsButNoLift`** (clears, but does not renew or lift). The result
review traced the no-renewal to **economics, not wiring** (forced-share renewals prove the machinery
reachable): the binding margin is the worker's own outside-option gate — **the contract feeds the worker
out of the hungry eligibility pool by term end**, the same demand-preserving hunger cycle the base is
built on — and share bread **substitutes one-for-one for commons draws** up to immediate need *(corrected
by measurement in §24: substitution is real but small — see the correction note at the end of this
section)*, so consumption is re-sourced rather than raised. Labor demand on this base is *episodic by construction*:
workers cycle in when hungry, out when share-fed, back in when hunger returns (~2 contracts per worker
per run), and no persistent tenancy relationship forms. Disclosed caveat: the worker-side bread
acceptance is exercised but never binds once a worker is in the hungry pool; the observed binding
margins are the outside-option gate and owner at-cap availability.

The finding, scoped: *on this base the no-advance share contract does clear voluntarily and moves real
bread — but only as transient scarcity relief; hunger-gated eligibility plus immediate commons
substitution prevent persistent tenancy and survival lift.* Preserved as a branch finding
(`feat/share-tenancy-impl-rb`), not merged — its spec merges only on the full `ShareTenancyClears`. Two
follow-ons fall straight out of the traces: a **forward-provisioning persistence probe** (can a worker
*anticipate* returning hunger — an ordinal future want — so renewal is chosen *before* eligibility
lapses?), and **in-kind wages from the owner's bread fund** — because the share contracts have done
something quietly important: **owners now end runs holding a positive produced bread fund**, which is
precisely the wages-fund whose absence made the C1 money-wage market vacuous.

**Correction (P1.5, 2026-07-06).** Two statements above are revised by §24's measured telemetry.
(1) *Substitution:* share income does **not** substitute one-for-one for commons draws — the per-agent
attribution built in P1.5 measures cohort substitution at roughly **3–37%**; most share income was net
new consumption. C1R's missing lift is therefore explained by **income magnitude** (tens of contracts
moving small shares against a much larger commons flow), with crowd-out a minor contributor — the
fed-out mechanism above stands (now measured at ~100% of renewal fates), but the substitution half was
overweighted. (2) *A latent settlement bug:* C1R settled a dead party's pending contract grain only on
the **starvation** death path; an owner dying of **old age** mid-contract left the worker the full
pending grain instead of the estate's `(1 − s)` share. Fixed in P1.5 with a regression; C1R's verdict
and contract-clearing scale are unaffected (the forward-off control cells reproduce
`ShareClearsButNoLift`/`ShareVacuous` post-fix), but pre-fix worker-stock magnitudes carried a small
windfall bias, disclosed here.

---

## 24. The forward-looking worker rotates instead of renewing — provisioning for the future reshapes the institution it was meant to perpetuate (P1.5)

The persistence probe asked one question of C1R's no-renewal wall: is it **myopia**? The worker's
outside-option gate evaluated only *this tick* — a share-fed worker is briefly sated at expiry, exits
the eligible pool, and hunger's return finds the pairing dissolved. Since this engine's need law is
closed-form deterministic, the honest extension was available without smuggling anything: replace the
instantaneous question with a **term-horizon forecast of the worker's own need** — a pure integer
function of already-digested state (the hunger law, held-bread-first depletion, the engine-exact
commons recurrence over the eligibility roster), applied symmetrically to new contracts and renewals,
with a real ordinal leisure guard and no realized-history input. Alongside it, the milestone shipped
the telemetry C1R's review asked for: a per-cause **renewal-fate map** and per-agent **commons
attribution** (both slices behind `share_forward_provisioning`, digest tag 24, byte-inert off).

**The telemetry first — C1R's null decomposed into measured fact.** Every renewal hint's fate is now
classified; across all cells and both modes, **`renewal_fed_out` is ~100% of fates** (post-fix aggregate
5,495 of 5,537). And the substitution measurement *revised* C1R's story: crowd-out is 3–37%, not 1:1
(§23 correction). The C1R null stands, but its mechanism is now: episodic eligibility plus small
episodic income — measured, not inferred.

**The headline: the forward gate does not produce contiguous tenure — it produces rotation.** The
answer to "is it myopia?" is **no, and the proof is that removing the myopia does not create renewal**:
the fed-out fate *survives* the forward gate, because a worker ending a term holds its share income as
bread, and the term forecast honestly reports it covered for the coming term. **One term of work buys
one term of provisioned living — so contiguous renewal is never the chosen act, *rationally*.** What
the gate does instead is transform the institution's scale and shape: eligibility extends by tens of
thousands of worker-ticks (`forward_only` 2k–36k per run), contract volume rises 4–10× (54 → 460
typical), participation becomes near-universal (32 of 32 landless workers), worker share income rises
~10×, and worker consumption roughly doubles. The verdict distribution across all fifty forward cells:
**44 `RenewalStillDeclined`, 5 `ForwardGateInert`** (abundant commons — the gate honestly never fires
where the commons covers the term), **1 `StandingTenancyLifts`**. The emergent institution is a
**standing rotational labor market** — continuously re-contracting, rotating pairs — not standing
tenancies. Provisioning for the future, given to the worker as a decision faculty, reshaped the
institution rather than perpetuating its pairings: the praxeological texture of seasonal and rotational
labor, not of tenure.

**The second wall the probe exposed: tenure is mortality-bounded.** A renewal decision requires both
parties alive at expiry — and on this base they rarely are. In the long-term cells most contracts end
at the **death seam**, not the term boundary (term=24, seed 3: 1,172 contracts, only 55 live-live
expiries — owners die of old age mid-term). Even a worker who *wanted* contiguous renewal mostly has no
counterparty left to renew with. Contiguous tenure on a mortal base needs something no individual
contract can supply: **succession** — the heir assuming the relationship — which is exactly the shape
of the follow-on this names.

**The threshold-marginal positive, scoped precisely.** One cell — term=24, seed 3 — crosses the
pre-declared bar: `renewals_total = 1`, 19 standing final-window contracts, and **survival lift +8**
(32 non-lineage survivors vs 24 in the matched no-contract run). The bars (`MIN_RENEWALS = 1`,
`MIN_FINAL_OPEN_CONTRACTS = 1`, `MIN_SURVIVAL_LIFT = 1`) were set before any run, so the
`StandingTenancyLifts` label is honest — but it is a **threshold-marginal crossing inside a dominantly
rotational regime**, reported as such and not as robust standing tenancy. The lift itself appears in
two further cells (`survival_lift = 8` at seed 19, φ=marginal) and is real welfare: net-new production
from at-cap regen that the rotational volume mobilizes — eight colonists alive who starve in the
control.

**Build note (the pipeline working as designed).** The result review's first round REJECTED the
landing on a P1 it found in the *C1R base*: the old-age death path never settled a dead party's pending
contract grain (only starvation deaths did), a worker windfall that P1.5's death-dominated volume made
load-bearing. Fixed at the seam with a regression test; all traces re-derived post-fix; the
confirmation round accepted with no P0/P1. The classifier's honest-null routes did their job twice —
a mid-build `StandingTenancyLifts` headline was re-classified to `RenewalStillDeclined` by the *stricter*
final classifier, and the post-fix rerun then produced the single genuine bar-crossing.

The finding, scoped: *giving the worker a pure term-horizon forecast of its own need — provisioning for
the future, derived not configured — does not create contiguous tenancy on this base: it creates a
standing rotational labor market (near-universal participation, 4–10× volume, ~10× worker income, real
survival lift in pockets), because one term's share rationally covers the next term's need, and because
mortal counterparties rarely survive to a renewal decision. Contiguous tenure now has two named
preconditions this base lacks: a reason to prefer the same pair (none exists — plots and workers are
interchangeable here), and succession across owner death.* Preserved on
`feat/forward-provisioning-impl-rb`. The follow-ons fall straight out: **C1N in-kind wages** on this
richer base (owners now hold large bread funds — `owner_grain_settled` alone reaches ~70k in the
term=24 cells), and a **tenure-succession probe** (does the heir assuming the estate's live contract
turn rotation into tenure?).

---

## 25. The advance clears where money could not — in-kind wages confirm the wages-fund doctrine, scarcity-dependently (C1N)

C1's money-wage market was vacuous, and §22 scoped that to the Austrian **wages-fund doctrine**: a wage is
an advance of present goods out of *prior saving*, and the C1 base had no monetary fund (the money fund
exists only after SALT promotes and the owner *sells* bread). C1R then built a fund of a different kind —
owners came to hold produced **bread**. C1N asks the doctrine's question in its cleanest form: pay the
wage **in kind**, out of that real bread fund, and see whether the advance-based contract clears where the
*money* wage cleared zero. Mechanically C1N is the **fixed-wage twin of C1R sharecropping**: the same
cap-waste plot substrate, but instead of an output share the owner advances a fixed bread wage **up front**
(its floor is P1.5's own term-need forecast — the bread the worker needs to survive the term, since it is
fed only by the advance), the worker cultivates the reserved plot, and **100% of the product** goes to the
owner. Two plain conserved transfers, no escrow. Built on the decomposed base, reusing the isolated
`share_tenancy.rs` contract machinery and P1.5's forecast — the prior two milestones' seams composing
directly.

**The advance clears — and money's does not.** On the identical base and seed, the same-seed money-wage
comparative cleared **zero** hires (`WageMarketVacuous`, C1's null reproduced), while the in-kind advance
cleared **3–47 contracts per seed** at the marginal commons, moving real bread — the owner advancing its
own `SelfProduced` surplus (never minted, never endowment: `endowment_funded_hires = 0` everywhere), the
worker cultivating and surviving the term on the advance (`term_starvations = 0`), the owner capturing the
product. This is the wages-fund advance/money-gap reading **demonstrated a third time** (after C1's null
and C1R's share), now in its sharpest form: *the same labor exchange that cannot clear for money clears in
kind, because the in-kind fund exists and the money fund does not.* And the binding margin is the fund
itself: the dominant decline is `owner_insufficient_fund` (the productivity gate `Q>W` essentially never
binds) — **the wages-fund constraint biting exactly as the doctrine predicts**, the advance bounded by the
size of prior saving.

**Whether it *persists* is scarcity-dependent — the honest headline.** The classified verdict keys on the
final window (does hiring *sustain* into the last 400 ticks), and by that measure the marginal-commons
cells read `InKindWageVacuous` — but that is a **sustain** label, not a "no contract ever cleared" null
(47 contracts cleared mid-run in the very cells so labelled). The φ-sweep tells the real story: at the
**scarce** commons, 2 of 5 seeds land `InKindWageClearsAndLifts` — hiring **sustains** into the final
window (12–13 final hires) **and raises survival** (+2–3 non-lineage survivors over the matched same-φ
no-contract baseline: net-new bread from mobilized at-cap regen keeping colonists alive who otherwise
starve). At the **marginal** commons it clears transiently then fades; at the **abundant** commons no
contract forms at all (no scarcity, no advance demanded). So the honest statement is *not* "vacuous": **the
in-kind advance clears and confirms the wages-fund mechanism; it sustains and lifts survival where scarcity
is sharp enough, clears transiently where it is milder, and vanishes under plenty.** As with C1R and P1.5,
continuity then collapses — the same fund-exhaustion and death-seam attrition that fades the share form
fades the wage form — which is why the arc's next question is the death-seam/tenure wall, not another
contract variant.

**Wage vs share, honestly bounded.** Run beside the share contract on matched marginal cells, both
voluntary forms clear under scarcity (in-kind 3–47 hires, share 33–74 contracts) and both fade; the
scarce-φ sustain+lift is measured for the wage form but the suite does not pair it with a scarce-φ share
cell, so this is **not** a dominance claim. The honest reading: *both the fixed-advance and the
output-share form of voluntary labor clear on this base under scarcity; neither is shown to dominate the
other from this suite.* The Cheung contract-choice question — which form each side prefers — is reported as
matched-cell accounting, not adjudicated.

The finding, scoped: *paying the wage in kind out of the owner's real prior saving makes the advance-based
labor contract clear where the money wage — lacking a money fund — cleared zero, confirming the wages-fund
doctrine a third time; its persistence is scarcity-dependent (sustained with a survival lift at the scarce
commons, transient at the marginal, absent at the abundant), and the binding margin is the size of the
fund, exactly as the doctrine predicts.* Preserved on `feat/in-kind-wage-impl-rb` (built on the decomposed
`feat/settlement-decomp`). All guards hold (conservation, the bread-provenance identity, money invariant,
registry/owner-identity, the anti-scaffold `endowment_funded_hires = 0`, term-survival); goldens
byte-identical off; four xhigh spec-review rounds (two P0s caught before any build), rb-lite clean in 5,
result-review ACCEPT-AS-HONEST-FINDING with the framing correction folded here. The next probe the traces
name: the **succession / death-seam** wall — *why* the fund and the final-window continuity collapse, and
whether an heir assuming the estate's live contract turns transient clearing into standing tenure.

---

## 26. Succession removes the death wall — and standing tenure still does not form: the arc closes on the worker exit (C1S)

Four voluntary labor institutions now clear and then fade — the output-share contract (C1R), its
forward-looking variant (P1.5), the fixed in-kind wage (C1N) — and P1.5 had *measured* the dominant reason
they fade: contracts die at the **owner-death seam** before ever reaching a renewal decision (at term=24,
1,172 contracts against only 55 live-live expiries). C1S is the direct test of that wall. On a mortal
owner's death, instead of the live share contract dissolving, the **heir who inherits the plot assumes the
owner side of it** — a single in-place rewrite of the contract's owner (dead → heir) — provided the heir
re-consents (the plot is still at-cap wasted regen it would rather share than work) and the worker
re-consents (the same bread-ordinal acceptance). A voluntary re-formation at the death seam, not a forced
pin; conservation is a no-op (the contract's grain stays with the continuing relationship); it reuses the
now-decomposed `share_tenancy.rs` machinery and rides tag 23's already-digested owner field, so tag 26
carries only a flag byte.

**The mechanism works — heirs genuinely inherit the going concern.** Across all five seeds at both the
scarce and marginal commons, succession engages robustly: 1–76 contracts per run survive an owner's death
into heir ownership (not the disqualifying `SuccessionInert`), the reservation is re-established so the
worker keeps harvesting the plot, and every conservation, registry, and owner-identity invariant holds.
The two re-consent gates both bite and are genuinely voluntary: heirs decline 1–9 times a run (preferring
to work the inheritance themselves rather than rent it out — the honest "reason to prefer working it"), and
workers decline re-consent 5–138 times a run.

**And standing tenure still does not form.** On *every* succession cell — both φ, all seeds — **no
succeeded relationship reaches the final window** (`final_open_succeeded = 0`; `post_succession_renewals =
0`), so the classified verdict is uniformly the pre-named first-class null **`SuccessionButStillTransient`**.
The reason is written plainly in the renewal-fate ledger: `renewal_fed_out == renewal_hints` in every cell
— **100% of expiring contracts fed the worker out of the eligible pool**, exactly the satiation exit P1.5
first isolated. Removing the mortality bound changed the *cause of dissolution* (contracts now survive
owner death) but not the *outcome* (they still fade at the next term boundary, because the worker no longer
needs the contract). **The owner-death seam was never the binding constraint; the worker's satiation exit
is.** A survival lift of +4 appears at the scarce commons — but it is the same transient scarcity relief
C1N and P1.5 give, measured against the matched no-contract baseline, and the succession-specific
classifier correctly refuses to call it standing tenure: it keys on succeeded relationships persisting or
renewing (`final_open_succeeded`), not on colony-global open-contract counts, so a seed with 137 open
contracts and a lift of 8 is still, correctly, `SuccessionButStillTransient`.

**The arc closes.** This is the fifth and decisive elimination in a single sustained inquiry into what a
*persistent* voluntary economic institution requires on this base. The occupation strand (S22) ruled out
every **incentive and capital** lever — hunger, skill, realized profit, sunk owned capital, endowed and
inherited capital — because each left the hunger/profit **exit** intact. The contract strand then ruled
out, one at a time, the candidate fixes for the exit itself: an output share clears but the worker is fed
out (C1R); extending the worker's horizon to a *term* forecast does not change that, it only rotates the
market faster (P1.5, ruling out **myopia**); paying a fixed advance in kind clears where money cannot but
fades identically (C1N); and letting the relationship **survive the owner's death** — ruling out
**mortality** — does not rescue it either (C1S). Every lever that leaves the worker free to exit when its
present need is met yields the same result: the institution *clears* (the exchange is real — the money-wage
nulls were a fund gap, not a labor-exchange gap) but does not *persist*. **The single binding constraint on
a standing occupational institution here is the worker's satiation exit** — the freedom to stop
contracting once fed. What would override it is, by construction, something that removes that freedom
(coercion, debt-lock, or an exogenous standing obligation) — which the study will not smuggle in under the
voluntary banner; naming that boundary precisely is the arc's result.

The finding, scoped: *succession makes a share contract genuinely survive its owner's death into the heir's
ownership — the mechanism engages robustly and conserves — yet produces no standing tenure, because the
worker-satiation renewal exit binds regardless of who owns the plot; mortality is thereby ruled out as the
persistence constraint, after myopia (P1.5) and the incentive/capital levers (S22), leaving the worker's
voluntary exit as the located, single binding constraint for this arc's institutions.* Preserved on
`feat/succession-impl-rb`. All guards hold; goldens byte-identical off; two xhigh spec-review rounds (a
same-tick death-race guard and a succession-specific classifier folded in), rb-lite clean in 2 rounds,
result-review ACCEPT-AS-HONEST-FINDING. Two disclosed P3s (a conservative same-batch heir-recompute that
can only under-count successions; an unexercised renewal-threading path — the primary signal is exercised).
The arc's forward door is no longer another owner-side contract lever: it is either an explicitly
exit-overriding institution (which must be framed as coercion/lock-in, not voluntary) or the pivot the
whole program was built toward — **C3R, mortal chain-producers** — now unblocked, since the wage/share
question it depended on is answered.

---

## 27. The production chain dies with its producers — the keystone's first slice, an honest collapse (C3R.a)

The whole voluntary-labor arc (§§22–26) ran on a base with one quiet exemption: the grain→flour→bread
producers — the miller and baker who own the mill and oven — were **immortal**. They were generated as
non-lineage roster colonists with `lifespan: None`, skipped every tick by the old-age reaper. So the
economy that emerged, and the wage/share institutions probed on top of it, all rested on producers who
never died. C3R is the pivot the program was built toward: make the chain **mortal** and ask whether a
division of labor in *production* — the thing that makes an economy more than subsistence — can survive its
practitioners dying. C3R.a is its first, minimal slice: introduce producer mortality and *nothing else* —
no succession, no inheritance — and observe.

The design is deliberately one-variable. Under flag `mortal_chain_producers` (tag 27, flag byte only) the
seeded chain producers become **lifespan-only** mortals (they draw a lifespan from the demography's own
distribution; they keep `household: None` and their subsistence cushion, so no hearth coupling and no
digest change beyond the flag). The one companion change closes a confound the spec-review process
hammered through four rounds: producer *formation* — building a mill/oven, or adopting the miller/baker
role — is gated to **mortal agents only**, on both capital-build paths and role adoption. Without that
gate, the ~68 immortal gatherers and consumers of `frontier_capital` could quietly rebuild the chain and
*fake* a self-repair; with it, `immortal_producer_count` is provably zero, so any repair that happens is
genuinely mortal. The surrounding economy (immortal gatherers supplying grain, consumers buying bread) is
held fixed as a stable backdrop, isolating producer mortality as the single varied factor.

The result is uniform across all five seeds and unambiguous: **`ChainCollapsesOnProducerDeath`.** Producers
die of old age abundantly (130–174 deaths per run); the milling and baking stages cannot stay jointly
staffed and dwindle to a single surviving producer; bread output falls toward the floor. What makes the
finding *honest* rather than an artifact is the instrumentation around it. It is not a hidden immortal
reservoir (`immortal_producer_count` max = 0 everywhere). It is not a thin-population artifact (the mortal
builder/adopter pool stays healthy at 5–6, so the pre-declared `CollapseFromThinMortalPool` label is
correctly *not* triggered). It is not apathy — agents re-adopt the vacated role **125–171 times per run**,
frantically. And it is not a blanket investment freeze — mortal builders do complete the 16-cycle-payback
mill 2–3 times per run. The chain churns through re-adoption and the occasional rebuild but never
*stabilizes*: a producer dies before its capital pays back, each dead producer's mill sinks to the commons
(where no one can use it), and the next mortal must start the roundabout investment from scratch. Two to
three builds against ~150 deaths is the payback horizon biting exactly as Böhm-Bawerk would predict —
capital under mortality is only rational if it can be carried across the gap between generations, and here
it cannot.

One phrasing precision, per the result-review: on this designated-money base the *era rung* sits at Forager
throughout even when the chain is healthy (a known property of `frontier_capital`'s rung thresholds), so
the collapse is read off the **Capital-stage staffing** signal (both stages continuously manned) dropping,
not off an era regression — the classifier reads the stage trigger directly and the write-up says so.

This is the keystone's motivating null, and it names its own next two slices precisely. The chain cannot
persist because (a) the *role* is refilled only by frantic ad-hoc re-adoption, never smooth transfer —
**C3R.b, role succession** — and (b) the *capital* sinks to the commons on every death instead of passing
to a successor, destroying the payback horizon — **C3R.c, capital inheritance**. The satiation exit that
closed the voluntary-labor arc (§26) does *not* govern here: a chain producer is profitability-locked, not
satiation-gated, so the constraint C3R exposes is genuinely new — mortality and the payback horizon, not
the freedom to stop once fed. Spec impl-62, SPEC-READY after four xhigh spec-review rounds (each *removing*
mechanism until the minimal experiment remained), built over six rb-lite rounds, result-review
ACCEPT-AS-HONEST-FINDING with no P0/P1. Preserved on `feat/mortal-producers-impl-rb`.

---

## 28. Inheritance preserves the chain's structure but the subsidy it rides caps the flow — capital continuity is not productive use (C3R.b)

C3R.a's diagnosis was that the chain dies because a dead producer's mill sinks to the commons — capital
destruction. C3R.b closes exactly that channel with the cheapest faithful mechanism: give the mortal
producers reproducing households so a dead producer's mill routes to a live **heir** through the *existing*
estate seam, and the heir — now holding the mill — re-adopts the role through the *existing* mortal-gated S7
tool-holder path. No new succession code; the research confirmed the estate and re-adopt machinery already
compose, the only missing piece being that C3R.a's producers had no household for `heir_for` to resolve.

The first build of C3R.b was **rejected by result-review** — and the rejection is itself the lesson. The
mechanism worked perfectly (every mill inherited, every heir adopted) yet the chain produced zero bread, so
"inheritance is insufficient" looked true. But the response variable was *floored by the precondition*: the
producer-household **hearth mints free food, and at the pinned subsidy that mint floods bread demand and
kills the market before inheritance can be evaluated.** Pinning that subsidy was a classify-not-tune
violation. The revision sweeps it — the producer-house hearth `food_provision ∈ {0,1,2,3}` and the
producer-house cap `∈ {1,2,3}` — and the confound becomes the finding.

Across the sweep (SEEDS=[3,7,11,19,23], 1,600 ticks; conservation, registry, money, and
`immortal_producer_count = 0` all hold; the older bases stay byte-identical): at high subsidy
(`food_provision ≥ 2`) the hearth mint floods and *every* cell dies (`SubsidyFloodsChainDies`, bread 0, the
hearth-mint scaling 36k→55k tracking the collapse). At the un-subsidised end (`food_provision = 0`)
reproduction is too thin to supply heirs and the structure fails. In one **narrow viable window**
(`food_provision = 1`, `cap = 2`), in **4 of 5 seeds**, capital inheritance does something the chain has
never done on mortal producers: it keeps **both the milling and baking stages jointly staffed** for ~1,500
of 1,600 ticks — the keystone's first *structural* positive. The split verdict names it precisely:
`StructurePersistsUnderInheritance` (a staffing claim that deliberately does **not** require output),
orthogonal to a `FlowVerdict`. And on flow the verdict is unanimous: every persisting seed is
**`FlowCapped`** — bread ≈ 9, bread-per-staffed-tick ≈ 0.006. The stages are manned continuously and produce
almost nothing.

The mechanism of the inversion is measured, not asserted, and it is an *intrinsic bind*. Inheritance
sustains the chain's structure only because it keeps the producer households populated, and populated
households reproduce — 357 births in the viable cell versus 16 in the inheritance-denied control. But that
reproduction is exactly what carries the hearth subsidy that floods bread demand (the producer-house
hearth-mint telemetry, scoped to producer households, rises with the sustained population; the market
counters show a floored bread price and thin trade). The matched control makes the trade-off legible as a
clean contrast rather than a disqualifier: deny tool inheritance and the producer households die back, so
the chain runs on constant *re-building* instead — which produces real bread (≈ 1,869) but cannot hold the
stages continuously staffed (`StructureDoesNotPersist`). And the `cap = 1` no-reproduction control seals the
necessity: with no room for a child-heir there are no heirs, and inheritance cannot preserve structure at
all. So the two ways to run a mortal chain are in tension — **inheritance buys continuous structure at the
cost of flow; re-building buys flow at the cost of structure** — and inheritance's structural success is
inseparable from the demographic subsidy that suppresses its output.

This is the Böhm-Bawerkian point made mechanical from the other side: *having* capital continuously (the
tool and the role never lapse) is not the same as *using* it productively. Capital destruction was
necessary but not sufficient; fixing it exposes a demand-side wall. The finding is a **narrow, honestly
reported positive** — one subsidy point, 4/5 seeds, flow-capped throughout — not a broad claim, and it
redirects the keystone: the binding next constraint is not role succession (inherited tools already reach
heirs and re-adopt in the viable cell) but the **demand side** — breaking the coupling by which the
reproduction that supplies heirs also floods the market. Spec impl-63, v2 after the result-review reject
(the v1 mechanism was SPEC-READY in one round; the confound was a scientific-inference gap the *result*
review caught, and the v2 sweep-and-split revision passed a focused confirmation round); rb-lite clean in 2
rounds; result-review ACCEPT-AS-HONEST-FINDING. Preserved on `feat/mortal-producer-inheritance-v2-impl-rb`.

---

## 29. Income feeds the living but never funds reproduction — the prior-saving problem relocates to the family provisioning fund (C3R.c)

C3R.b left the keystone facing its demand-side bind: the reproduction that supplies heirs rides a hearth
subsidy that floods the market. C3R.c attacked it with the cheapest faithful circular flow — retire both
producer-side mints and provision the producer households from the producer's **externally-earned** bread
revenue. The design carried the plan review's P0 as a structural rule: an `external_earned_revenue` ledger
credited only by cross-household sales and split by buyer class, with **genuine** external revenue defined
as consumers + gatherers + lineage (producer-class recirculation tracked but excluded — so a circular-flow
*illusion* over the producers' own finite gold was pre-named `AccountingLoopOnly`, not success). The
provisioning loop itself was deliberately modest and un-smuggled: on a member's own *unprovided* bread want,
the producer transfers conserved gold to fund one loaf at the last realized price, and the member bids
through its own unmodified machinery. No pool, no bid override, no demand nudge; the cold start allowed to
bite.

The preregistered classifier returned **`EarnedIncomeInsufficient` in every headline cell** — but the
result-review required, correctly, that the formal label be kept distinct from the causal finding, which is
sharper and stranger. **Nobody starves.** With both mints retired, adults feed themselves at the floored
bread price out of their birth-gift gold — the headline transfer mechanism executed **zero times in all
fifteen eligible runs**. It is mechanically reachable (the branch is live by inspection; with a 16-gold
birth gift against a price of 1, the funding gap is simply never positive) — an economy that never demands
the mechanism built for it, the same shape as C1's unclearable wage. And it is also a revealed design
mismatch: reactive consumption finance could never have addressed the wall that actually binds, because
**what fails is reproduction**. Births collapse from 357 (hearth-on) to 1–5: the birth gate debits four
*saved* food units from the parent's stock, and present-hunger buying feeds but never *saves* — agents buy
a loaf when hungry and eat it; no one accumulates the four-loaf child-rearing stock, so no children are
born, no heirs exist, and the chain's structure dies exactly as C3R.b's no-hearth bracket did. The
prior-saving problem that C1 located in the wages-fund (a wage is an advance out of prior saving) relocates
here to its most elemental form: **a mortal economy whose agents demand only against present need cannot
fund its own reproduction.** Meanwhile the price hypothesis resolves negatively in both directions: with
the lineage surround mint present the market stays floored-but-active; with it reduced, the late-window
market dies outright — though producer extinction truncates supply first, so the ultimate
recurring-vs-depleting demand question remains open. Two genuinely positive fragments: flood retirement
*did* lift output while the chain was staffed (42 vs the mint-on reference's 9), and the observation-first
ledger slice answered a standing question — the **gatherers**, the one class with recurring earned income,
are the dominant genuine external buyers (87% of genuine revenue in the reference), not the depleting
consumers.

Disclosed limits (per the result-review): zero transfers cannot by themselves distinguish "gap never
positive" from "producer unfunded" (the affordability explanation is arithmetic-supported, not per-member
measured); the vocation-aggregated buyer-gold series is not clean external-consumer-depletion evidence;
`members_fed_by_purchase` counts purchase events, not distinct dependents; and a dormant ledger provenance
gap (same-household purchases skip without reattaching earned provenance) must be repaired before any
future *positive* provisioning run is trusted. The finding names its own next slice precisely — and it is a
*behavioral* one, the first the keystone has needed: **individual saving ahead of need** — a parent-facing
future-bread motive that purchases and retains the four loaves through the existing market (purchased stock
already passes the birth gate; nothing structural is missing). Spec impl-64, SPEC-READY after three xhigh
rounds; rb-lite clean in 2 rounds (codex + claude); result-review ACCEPT-AS-HONEST-FINDING with the
reframed headline. Preserved on `feat/earned-provisioning-impl-rb`.

---

## 30. Saving works as built and the trap holds — the keystone closes into a bootstrap/allocation trap (C3R.d)

C3R.c named the missing behavior; C3R.d built it, as minimally as the engine allows. While a producer
household can still grow, its members carry the full four-unit `Next`-horizon bread want block — the same
scale-extension shape as a producer's input buffer: a real market bid through the member's own machinery,
plus reservation of the held stock against both the consume path and the ask ladder (without which a fed
parent's saved loaves would be auto-listed to feed the standing gold-savings want). No new parameter
anywhere: the count is the birth gate's own `child_food_endowment`, the trigger its own size cap. A
**sufficiency control** injects the four loaves directly — a conserved move of existing bread from the
richest out-of-household holder, rolled back on failure, mode-exclusive with the motive — to test whether
the birth gate is really the sole demographic blocker.

**The control's answer is emphatic, within its bounds.** At 690–730 otherwise-eligible opportunities per
run, possession of four loaves was the final missing birth condition every single time
(`failed_injected_births = 0`; the endowment block vanishes and the household size cap becomes the binder;
births rise from ~2 to 702–730 as cap-limited replacement throughput — *not* unbounded growth). The claim
is deliberately bounded: this is recurring out-of-household *redistribution*, not endogenous stock
formation — it proves the gate is locally sufficient, not that the market can produce or allocate the
stock, and the donors' class was not recorded.

**The motive, meanwhile, works as built and still cannot restore continuity.** It emits 384–540 wants per
run; only **3–7 attributable purchases** clear (attribution pinned to a pre-market snapshot so hunger
purchases cannot masquerade as saving); the stock reaches four 0–3 times; births never materially exceed
the no-motive reference. The four evidentiary seeds walk three different rungs of the pre-named ladder —
births resume but the structure still dies (seed 3), the race is lost outright (seed 11), the stock
assembles but births stay blocked (seeds 19, 23) — while the fifth (seed 7) is excluded by a pinned
precondition (its mint-on reference never persisted in the landed C3R.b either; a disclosed loss of one
seed, per the preregistered guard). The honest diagnosis is a **bootstrap/allocation trap**: final-window
chain production is dead, and the hearth bread that does survive in the economy does not become
reproductive stock in producer households — but the present telemetry cannot yet separate *insufficient
offerable surplus* from *priority loss to hungry buyers* (the market still trades ~590 loaves a run at the
floored price while the saving bids almost never win). "No uncommitted surplus reaches saving" is the
precise statement — not, literally, that nothing exists to save. Either way the circle holds: the chain is
dead for want of heirs, heirs absent for want of birth stock, the stock unassembled for want of winnable
bread, and bread unmade for want of a chain. **An individual saving behavior alone does not escape a
low-level equilibrium this deep.**

Process disclosures (required by the result-review): the build's review panel ended in a **consensus
failure** after seven rounds over one finding — the implementer had extended the earned-ledger to credit
cross-household *non-bread* sales (fixing a real C3R.c gap: a Miller's entire income is flour, and the
landed ledger never credited it), which the reviewer objected changed the preregistered bread-only
contract and could let producer-to-producer flour revenue bypass the accounting-loop guard. The
orchestrator adjudicated: the credit stands (income is income) but is now **class-tracked and disclosed**
as a post-preregistration, measurement-only ledger amendment — and the disclosed values settle the concern
empirically (producer-class non-bread revenue is 0–14 gold per run against 510–881 external; the bypass was
real in principle, negligible in practice). The `AccountingLoopOnly` verdict itself remains bread-only as
preregistered. The round-7 claude panel reviewer degraded to noise, so the gate ran on the codex review,
the orchestrator's independent verification, and this result-review. Known follow-up debt (verified,
non-blocking): assertions for the new counters, donor-class telemetry, a synthetic classifier table, and
ask/bid-loss telemetry to resolve the offer-scarcity-vs-allocation diagnosis.

The finding names its next lever precisely, and for the first time in the keystone it is **not another
individual behavior**: instrument the allocation contest (offerable bread, seller class, losing
birth-stock bids, competing `Now` demand), then test a coordination or transition institution — a household
or collective granary, a priority set-aside, the C3R.b hearth subsidy re-read as a *trap-escape*
intervention withdrawn after ignition to test hysteresis, or a finite initial stock bridge (which the
recurring control does *not* already prove sufficient). Spec impl-65, SPEC-READY after two xhigh rounds;
built via rb-lite over seven rounds ending consensus-failure, adjudicated and independently verified;
result-review ACCEPT-AS-HONEST-FINDING conditional on this narrowed framing. Preserved on
`feat/birth-stock-saving-impl-rb` (tip `45a922e`).

---

## 31. Why the saving bid loses — the trap is economic, correlated, and survives its sharpest challenge (C3R.e-obs)

Before building any trap-escape, the program did what both the C3R.d result-review and the second-opinion
review demanded: it *instrumented the allocation contest*. A pure-observation slice — provably inert twice
over (the canonical digest with observation ON equals OFF plus a single two-byte tag emission, and a pinned
oracle re-runs the full C3R.d four-cell classification under observation and asserts the landed verdicts
and metrics reproduce exactly) — assigns every failed saving quote-opportunity exactly one cause, by pinned
precedence, from a four-record allocation trace of the market's own quote pass.

The diagnosis, on every seed, is a **three-way split of economic causes with the microstructure share
negligible**: roughly a third of failures are `NoBidPosted` under the bidder's own reservation/posting
logic (the GoldBind family — *not* proven literal gold scarcity; the reason sub-counts are not retained), a
third lose the price contest to higher-limit hungry bidders (AllocationPriority), and a fifth to two-fifths
find no purchasable offer (OfferScarcity — which includes priced-out asks, not only empty windows). The
family built to *disconfirm* the economic reading — arrival-order artifacts of the deterministic
double-auction queue — explains only **2–7%**. No family reaches the preregistered majority on any seed, so
the formal classification is `MixedDiagnosis` five times, honestly. The endorsed conclusion, verbatim from
the result-review: **"correlated economic bottlenecks, microstructure minor, direct provision as the
maximin next intervention."** The three families are mutually-exclusive *proximate* failure states, not
proven independent walls — near-zero upstream chain output could jointly generate all three — which is
exactly why the selected next lever is the one that bypasses all three at once (direct stock provision,
already gate-proven by the C3R.d sufficiency control) rather than a bet on any single margin, tested as
*ignition with a withdrawal/hysteresis bar* rather than assumed sufficient.

The slice also earned its keep procedurally. The first submission was **rejected** by the result-review for
a false-green hard guard — the five-seed oracle printed its expected verdict labels instead of computing
them (three pinned metrics underdetermine a four-cell classifier). The repair extracted the real C3R.d
classifier into a shared test module and asserts its computed verdicts, added table-driven tests for every
outcome branch including the payload-selection rule, and cross-checks the opportunity denominator with an
independent per-tick recount — all with the diagnosis shares byte-identical before and after, as pure guard
repair must be. Spec impl-66, SPEC-READY after three xhigh rounds (each catching a way the diagnosis could
have been *confidently wrong* — a temporally impossible taxonomy for an immediate-crossing market, a window
that would misfile the queue-loss signal, an eligibility filter that disagreed with the fills); rb-lite
clean in 5 rounds + a 2-round repair run; result-review REJECT → repair → ACCEPT-AS-HONEST-FINDING.
Preserved on `feat/saving-allocation-obs-impl-rb` (tip `06e3180`).

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
| S22e | Endowed + inherited cultivation capital (role-topology slice 5) | **FINDING — even capital given UP FRONT and inherited does NOT produce occupation:** a default-off gate endows a minority of lineage households with a plow at generation (hash selection, conservation-baseline counted) + a plow estate-routing SWITCH (tools already inherit to the household heir; the lever toggles heir-route vs forced-commons), flowing through the unmodified S22c profit-stay (no exit edit). Verdict NoStickinessDespiteEndowment ×5. Lever bites massively (owner out-produces ~3×; 641–681 plow→living-heir transfers/run, heirs cultivate) but churn stays ~1× baseline, cultivation ~4%, cohort 0/8. Genuine, not an unreachable bar (Codex-verified): across the endowed_tool_count sweep (share 0.12→1.00) the cohort is flat 0/8 — even at universal ownership (owner id-share 0.59) no lineage persists ≥50%; they rotate. The binding constraint is the hunger/profit EXIT, not capital supply or provenance. Controls: no-inheritance + productivity-only not sticky; too-many-tools → UniversalOwnership; no-endowment → EndowmentLeverInert. Completes a clean 5-step negative → occupation needs an explicit role-choice/assignment institution that overrides the exit, not capital of any provenance. (Codex review-of-results: PASS-WITH-CAVEATS, no P1/P2; bounded to this endowed/inherited plow institution in this configured colony) |
| S22f | Voluntary fixed-term cultivation commitment (role-topology slice 6 — the TURN) | **FORMAL: NoStickinessDespiteCommitment 4/5 + TermTooShortFinding 1/5 (predeclared aggregate-churn bar). REAL RESULT: the arc's first stable occupational core.** A default-off gate lets a post-money eligible agent VOLUNTARILY commit to cultivation for a fixed term when its own realized cultivation-return signal (a reusable S22c-data predicate, not the exit helper) clears its outside option; while committed the hunger/profit exit can't turn cultivation off; at expiry it re-decides from fresh returns (renewal only if the signal still clears). Per-agent commitment state steers cultivating (not Vocation), digest tag 12 ON-only; headline on the expanded base with NO capital. For the FIRST time the lever forms a persistent, renewing, MINORITY committed cultivator core (14–30 ids ≥½ final window, commit-share 0.07–0.20, all renewed) + a working TWO-TIER division of labor (core takes 0.85–0.95 grain; fluid buyer majority survives + materially buys 17k–26k); money promotes, provenance clean, mortality+conservation hold. The ONLY unmet clause is aggregate churn (2.67–2.75 vs ≤0.5× bar) — it fails because the model now has a stable core + a still-churning fluid tier. Genuinely NOT a re-pin: fiat_pin → RePinScaffold (signal=0, no renewals), unprofitable_offer → CommitmentUnchosen (0 uptake), nonbinding_term(=1) → no persistence, ~1800 eligible decline; the commitment_term sweep {12,24,48,96} is outcome-driving. Separate observation: commitment + capital (secondary variants) → grain monopoly + lineage extinction (cull). Per classify-not-tune the predeclared negative label stands; the stable core is reported as the first positive sub-result, not relabeled SUCCESS. (Codex review-of-results: PASS-WITH-CAVEATS, no P0/P1; "the arc's turn — the first lever to stabilize an occupation"; rb-lite ended review_panel_failed on reviewer-infra timeouts, result independently verified) |
| S23a | Private land tenure (private-property arc, slice 1) | **FINDING — scarce excludable use-it-or-lose-it land does NOT stabilize occupation; it THRASHES.** Grain plots become excludable, heterogeneous (good-near/poor-far), claimed by money-free homesteading, harvested owner-only, lost-if-idle, inherited; re-entry penalty is spatial (lose your place → re-homestead far/poor land); sim-side gated plot registry + pre-world.tick single-targeter reservation + carried_grain_source; digest tag 13 ON-only; goldens byte-identical off. TWO methodological catches in review (not rubber-stamped): take-1's HardBarrier was a CAPACITY ARTIFACT (~48 agents vs ~12-24 plots; the sweep never varied total land) — take-2's population-scaled total-land axis {12,24,48,96} makes HardBarrier VANISH once adequate viable land exists, confirming the artifact; and a verdict mislabel (churn-didn't-drop → CommonsEquivalent) corrected to NoStickinessDespiteLand. Verdict NoStickinessDespiteLand ×5 {3,7,11,19,23}: with adequate land + open entry (viable margin 6-10, buyers survive+buy, money promotes, non-vacuous, guards hold) owner-exclusive lost-if-idle tenure THRASHES — claim→lost-on-idle→reclaimed-by-other → churn ~26-27 vs commons ~2.6 (~10x), owner share 0.75-0.80, no persistent cohort; no success window anywhere. Controls separate (property_off/non_excludable_deed/free_reclaim/no_forfeit → TenureLeverInert). Lesson: the exit-cost INSTITUTION'S DESIGN matters — S22f's binding voluntary contract stabilizes a core; this involuntary forfeiture rule destabilizes. Non-forfeiting title / money land-market deferred to S23b. (Codex review-of-results: PASS after the relabel; take-1 rb-lite run externally killed mid-round-2, cause unconfirmed, landed diff verified independently) |
| S23b | Post-money alienable land market (private-property arc, slice 2) | **FINDING — a post-money alienable land market goes THIN over owner-dominance; does NOT stabilize occupation.** Pre-money homesteading unchanged (idle-forfeiture off from tick 0, SALT bootstraps); POST-promotion plots become alienable assets bought/sold for SALT at an ENDOGENOUS capitalized-rent price (good plots ~86 SALT vs marginal ~1, gap_bps=850000 — strongly rent-tracking, NOT a constant); deterministic listing/bidding/matching (single sorted pairwise sweep, clear at ask); carrying cost + foreclosure as conserved transfers via an explicit land_fee_pool_salt sink; budget hysteresis (sold + spent → priced out of re-buying). Composes on S23a; digest tag 14 ON-only; goldens byte-identical off. Verdict LandMarketInert ×5 {3,7,11,19,23}: the market is physically real (prices track rent, foreclosures fire, priced-out traces exist, post-money-only, guards hold) but TOO THIN to be load-bearing — only 5-7 title trades/seed (< MIN_LAND_TRADES=8), churn ~20 ~ the matched no-market baseline ~21 (no drop), no owner-cultivator cohort, no success anywhere in the cap{0,1,2,3}/carrying sweep (free land → 100s of trades but rent signal vanishes → also inert). CONFOUND (disclosed, honest): the no-market baseline on this population-scaled S23a base is ALREADY owner-dominant + buyer-thin (buyers~1, owners~95% grain), so S23b tests whether a market can rescue an already owner-collapsed private-tenure regime (it cannot) — narrower than "land markets fail generally"; buyers=0/owner_grain~98% are the disclosed regime, not a conservation bug. Per classify-not-tune LandMarketInert is correct + non-post-hoc (per-seed non-vacuity fails 5-7<8; "non-vacuity passes" = aggregate endogeneity evidence only). S23 property arc = a clean PAIR of negatives (forfeiture THRASHES S23a / market goes THIN S23b) vs S22f's contract preserving a two-tier market. (Codex review-of-results: PASS-WITH-CAVEATS; rb-lite panel misconfigured with a stale S22d reviewer checklist — caught, run stopped at the round-4 codex-reviewer-clean state, gated on independent verification + Codex review-of-results) |
| S23c | Secure heritable land tenure (private-property arc, generational slice) | **NULL — `DisqualifiedNoInheritance` (deferred, unmerged; engine verified).** The honest counterfactual to S23a's forfeiture thrash: a SECURE (never-forfeited) title + a universal-heir inheritance engine (`settle_estate_to_heirs`/`transfer_secure_private_land_on_death`; pinned heir order live-child→nearest-kin→household-successor→colony-next-of-kin→revert; exactly-one-successor + no-dead-owner-plot conservation) + a partible fractional-share regime (conserved split among co-heirs). Digest tag 18 ON-only; goldens byte-identical off. The engine is CORRECT (probe-verified by forced mortality) but VACUOUS on the shipped base: OIKOS runs TWO DISJOINT populations — an immortal standalone cultivator roster (`lifespan=None`) that owns land but never dies/reproduces, and mortal reproducing lineage households (hearth-fed consumers) that never persistently hold a plot at death. Owners ∩ reproducers ≈ ∅ → 0 inheritance events across 306 death traces. Names the prerequisite: a base where the land-owning cultivators are themselves mortal and reproducing (S23d). Preserved on `feat/secure-land-tenure-impl-rb`. |
| S23d | Mortal-landowner demography base (private-property arc, generational slice) | **NULL — `DemographyBaseUnviable` ×5 (deferred, unmerged); the inheritance vacuity is SOLVED but the base is SUBSIDY-BOUND.** A base-building milestone (not a lever test; Codex-scoped option B) composing S21f + S22a + S21h floor + S23a claim + the S23c secure substrate (forfeiture off, no market) so the mortal reproducing lineage households ARE the persistent cultivator-owners; tenure-INDEPENDENT viability bar; strict P0 owner-identity (owner must be a lineage reproductive actor in the birth/kinship graph, not merely `lifespan=Some`; enforced by `immortal_roster_owned_plot_ticks==0` AND `non_lineage_owner_plot_ticks==0` AND `owner_old_age_deaths>0`). Digest tag 20 ON-only; goldens byte-identical off. The demographic fix WORKS: owners are mortal lineage reproducers (no immortal/non-lineage residue), die of old age (21–28), inheritance fires ENDOGENOUSLY in normal play (`inherit_eligible` 21–28 ≥20, real heir transfers, no forced mortality), money promotes (`seeded_minted=0`), born-in-sim agents own (19–26) — the S23c blocker is removed. BUT the economy is SUBSIDY-BOUND: final-window `floor=12768/12768=1.000` (100% own-labor emergency floor), `buyer_bought=0` — everyone self-provisions so no producer/buyer money economy forms (money promotes without sustained exchange). Not an artifact (floor runs after market clearing, nets out market consumption, doesn't pre-empt purchases). Echoes S21g (subsistence redundancy must precede specialization). Disclosed open risk: short adult lifespan (~27 ticks) — mortality no longer blocks inheritance, but surplus formation under mortality is unproven (`min_lineage_after_cold_start=0` = continuity still fragile). Honest next step = a NEW LEVER (finite rival outside-option scarcity that FORCES exchange, with predeclared controls), NOT tuning the floor to pass; S23c re-run stays deferred until a viable base exists. Verified: workspace green, goldens byte-identical off, fmt/clippy clean, suite 13/13; one Codex P1 folded in (`min_lineage` cold-start-cutoff units bug, verdict unchanged). Codex review-of-results ACCEPT-AS-HONEST-NULL. Preserved on `feat/mortal-landowner-impl-rb`. |
| S23e | Finite rival subsistence commons (private-property arc, scarcity lever) | **NULL — NO `φ` forces a market (unmerged); the binding constraint is BUYER INCOME, not scarcity.** The lever S23d pointed to (Codex-scoped a2): replace the unlimited own-labor emergency floor with a finite regenerating non-excludable RIVAL commons so subsistence access is rivalrous. Anti-smuggling done right: scarcity anchored to the MEASURED S23d baseline `D0=12768` (pinned + guarded by `s23d_baseline_reproduced_for_d0`), `r=φ·D0/window`, `φ` SWEPT {abundant 1.25, marginal 0.5, scarce 0.25}, never searched. Flag `rival_subsistence_commons`, digest tag 21 ON-only; goldens byte-identical off. Result: no `φ` forms a producer/buyer market — abundant→`AbundanceReproducesNull` (control ✓, reproduces S23d subsidy-bound, survival 0.69); marginal (the informative cell)→`SubsistenceBoundDespiteScarcity` (money promotes, owners produce+sell surplus, commons scarce — yet alive non-owner `buyer_bought=0`, survival 0.34); scarce→`MoneyFailure` (bootstrap collapse, survival 0.17). CORRECTLY-SCOPED FINDING (Codex review-of-results ACCEPT-AS-HONEST-NULL, code-verified NO confound — the `bought_food_of`←`Bought` buy path is reachable, just not taken; NOT the mortality wall — owners have surplus `produced_minus_consumed` 330–779, `owner_sold`≤106): on this base a finite rival commons does NOT force a sustained market because **hungry non-owner demand has no sustained purchasing-power loop** (a one-time money endowment that depletes, no wage/rent/payment stream) — scarcity + owner surplus are not sufficient. Metric caveat: `owner_sold` lifetime-cumulative vs `buyer_bought` final-window/alive-non-owner (no contradiction). Verified: workspace 40 suites green, goldens byte-identical off, fmt/clippy clean, suite 11/11; rival conservation + after-market S21h routing faithful. Process caveat: rb-lite converged in 1 round and the second (claude) reviewer failed to launch (`claude: command not found`) — gated on independent verification + Codex review-of-results, not the panel. Next step = a **buyer-income / money-circulation lever** (labor market / wage-rent-payment loop), NOT another scarcity tweak; the generational-land thread is blocked on the demand-side income loop. Preserved on `feat/rival-commons-impl-rb`. |
| C1 | Wage labor / circular flow of income (civilization-core arc, keystone; the buyer-income lever S23e named) | **NULL — `WageMarketVacuous` (unmerged); voluntary wage labor does not BOOTSTRAP the circular flow.** On the S23e marginal base (φ=0.5), an owner hires a hungry non-owner for a money wage paid *now* from the owner's OWN prior bread-sale earnings (anti-subsidy retained-earnings ledger). Ordinal worker ask (`reservation_labor_ask`, above Leisure) + new OWN-MONEY employer bid (`appraise_labor_hire_for_money`: wage debited from present gold vs expected proceeds as a dated receivable — no credit) imputed from the LOW realized bread price the poor buyers pay; pre-market commons-sufficiency outside-option gate; conserved wage escrow with full death routing + money/escrow invariant; per-worker FIFO wage-proceeds attribution. Flag `wage_labor`, digest tag 22 ON-only, goldens byte-identical off. Result across 5 seeds + whole φ sweep: **no voluntary wage clears** — the owner's willingness-to-pay sits BELOW the worker's reservation ask (`circular_flow_forms=0/5` at every swept threshold and every φ — not tuned). The null is ECONOMIC not artifact: the `FiatWage` control FORCES 8 hires (machinery reachable); controls separate (`NoWageOffered`→S23e null; `FiatWage`/`SubsidisedWage`→`WageScaffoldOnly`). A third, deeper outcome than anticipated — not `CircularFlowForms`, not even `WageInertDemandStillDead` (wages never clear at all). The demand deficit is self-reinforcing; voluntary bilateral wage contracting can't self-start the loop (the employer-side chicken-and-egg). Codex review-of-results ACCEPT-AS-HONEST-NULL, no confound. Verified: workspace green (0 failed), wage_labor 6/6, goldens byte-identical off, fmt/clippy clean. Built via rb-lite (full 8 rounds, healthy 2-reviewer panel). Scoping (the Austrian wages-fund): C1 ruled out MONEY-WAGE contracts on a base with no accumulated savings — the Böhm-Bawerk/Strigl prediction (a wage is an advance out of prior saving), NOT an indictment of voluntary exchange. Next levers = the VOLUNTARY ones (in-kind/natural wages — the measured gap is a *money* gap; share contracts/tenancy — no advance needed, the C4 shape; the accumulation horizon), with state-fiscal/credit injections as optional interventionist comparisons (in the game, the state is an optional player intervention, never required). C2/C3/C4 deferred pending a living flow. Preserved on `feat/wage-labor-impl-rb`. |
| C1R | Voluntary output-share tenancy (sharecropping — the no-advance contract; P1 of the replan) | **FINDING — `ShareClearsButNoLift` (unmerged): the no-advance contract CLEARS voluntarily — the first voluntary labor institution to open on a mortal base — but only as TRANSIENT scarcity relief.** On the C1 stack (tags 18/20/21/22), flag `share_tenancy` tag 23 ON-only: a landless worker (structurally excluded from homesteading by the S23d owner-identity design — disclosed) works an owner's **at-cap** plot (regen otherwise destroyed; draw bounded to regen; `share_stock_drawdown=0` everywhere) for a pinned swept share of realized bread. No advance, no money requirement (not promotion-gated); worker acceptance = bread-denominated ordinal rank-walk above Leisure (exact integer floor shared with the split); owner acceptance = cap-waste dominance; split booked once, before the own-use consume, owner share via origin-preserving conserved transfer, worker attributed producer. Result (φ=marginal, 5 seeds): 31–54 voluntary contracts/seed clear and move 600–6,250 loaves of worker share income at exact split ratios, with the same-seed `WageComparative` cell printing `WageMarketVacuous` beside it — the C1 advance/money-gap reading DEMONSTRATED. Voluntariness ×2: abundant commons → `ShareVacuous` (zero uptake — contracting is a scarcity phenomenon); `ForcedShare` separates as scaffold. BUT: `renewals_total=0`, `final_contracts=0` on every voluntary cell → `survival_lift=0`. Review-of-results (ACCEPT-AS-HONEST-FINDING, no P0/P1/P2): the no-renewal is ECONOMIC not wiring (forced renewals=2281 prove reachability) — the binding margin is the worker's own outside-option gate (**the contract feeds the worker out of the hungry eligibility pool by term end**), and share bread **substitutes 1:1 for commons draws** up to need, so consumption is re-sourced, not raised; labor demand is episodic by construction (~2 contracts/worker, cycling). Caveat: the worker bread-acceptance never binds on the hungry pool (binding margins = outside option + owner at-cap). All guards hold (conservation/commons/money/anti-title/S23d counters/provenance); goldens byte-identical off; clears=0/5 at every φ/share/term cell (not tuned). Verified: workspace green, share_tenancy 10/10; rb-lite clean in 5 rounds (round-5 panel 1-of-2; round-4 full 11-crux claude review no P0/P1/P2). Preserved on `feat/share-tenancy-impl-rb`. Follow-ons the traces name: renewal-fate + commons-substitution telemetry, a **forward-provisioning persistence probe** (renewal chosen BEFORE hunger returns), and **in-kind wages from the owner bread fund** — the share contracts leave owners holding a positive produced bread fund, i.e. the wages-fund whose absence made C1 vacuous. |
| P1.5 | Forward-provisioning persistence probe (term-horizon worker forecast + renewal-fate/substitution telemetry) | **FINDING — the forward-looking worker ROTATES instead of renewing (unmerged): removing the myopia does not create tenure — it transforms the institution into a standing rotational labor market.** On the C1R stack, flag `share_forward_provisioning` (tag 24 ON-only): the worker's outside-option + acceptance questions extended from this-tick to a TERM-HORIZON pure forecast of its own deterministic need (closed-form hunger law, held-bread-first depletion, engine-exact cap-before-draws commons recurrence over the eligibility roster; symmetric new/renewal; real ladder-derived leisure guard; no history, no prospective income). Slice A telemetry (the C1R RoR ask): per-cause renewal-fate map + per-agent commons attribution. Results (post-fix, 50 forward cells): 44 `RenewalStillDeclined`, 5 `ForwardGateInert` (abundance), 1 `StandingTenancyLifts` (term=24 seed=3: renewals=1, 19 standing final contracts, survival_lift=+8 — crosses the PRE-DECLARED bars exactly; scoped threshold-marginal inside a rotational regime). Fed-out = 99.2% of all renewal fates (5,495/5,537) — the C1R null now MEASURED and it SURVIVES the forward gate: one term's share income rationally covers the next term (satiation, not myopia). The gate transforms scale instead: 4–10× contracts, near-universal participation (32/32), ~10× worker income, doubled consumption, survival_lift=8 in pockets (net-new production from mobilized at-cap regen). Second wall exposed: tenure is MORTALITY-BOUNDED — most contracts end at the death seam (term=24: 1,172 contracts, 55 live-live expiries), so contiguous tenure needs SUCCESSION. Substitution MEASURED at ~3–37%, not 1:1 → C1R §23 correction (no-lift was income-magnitude, not crowd-out). RoR round 1 REJECT (P1: old-age deaths never settled pending contract grain — a C1R-era gap made load-bearing by death-dominated volume) → seam fix + regression → round 2 ACCEPT-AS-HONEST-FINDING, no P0/P1. Spec impl-59, 2 xhigh rounds to SPEC-READY; rb-lite clean in 3 rounds. Preserved on `feat/forward-provisioning-impl-rb`. Follow-ons: C1N in-kind wages (owner bread funds now large — owner_grain_settled ~70k in term=24 cells); a tenure-succession probe (heir assumes the estate's live contract). |
| C1N | In-kind (bread) wages — the fixed-wage twin of share tenancy (advance-based labor paid from the owner's produced-bread fund) | **FINDING — the advance CLEARS where money could not; the wages-fund doctrine confirmed, scarcity-dependently (unmerged).** On the DECOMPOSED base (`feat/settlement-decomp`), flag `in_kind_wage` (tag 25 ON-only): C1R's cap-waste plot contract with an UP-FRONT bread advance (floor = P1.5's `forecast_term_need_unmet`) + worker-share 0 (worker cultivates the reserved plot, transfers 100% product to owner, fed only by the advance); two plain conserved transfers, no escrow; reuses the isolated `share_tenancy.rs` machinery + P1.5's forecast. Result (5 seeds, guards all hold — conserved/registry/commons/money/provenance_identity, `endowment_funded_hires=0`, `term_starvations=0`): the same-seed MONEY-wage comparative clears ZERO (`WageMarketVacuous`) while the in-kind advance clears **3–47 hires/seed** at marginal φ (real advance bread, owner `SelfProduced` fund drawn down) — the advance/money-gap reading DEMONSTRATED a third time; dominant decline is `owner_insufficient_fund` (productivity gate never binds) — the wages-fund constraint biting as predicted. φ-DEPENDENT persistence: at **scarce** φ 2/5 seeds land `InKindWageClearsAndLifts` (sustained final hires 12–13, survival_lift +2–3 vs matched no-contract baseline — real net-new bread from mobilized at-cap regen); at **marginal** clears transiently then fades (`InKindWageVacuous` — a FINAL-WINDOW SUSTAIN label, NOT "no contract cleared": 47 cleared mid-run); at **abundant** no contracting. Wage-vs-share reported as matched-cell accounting, NOT a dominance claim (both voluntary forms clear under scarcity, both fade). Spec impl-60, **4 xhigh rounds** to SPEC-READY (two P0s caught before any build: the original grain-conversion mechanism was vacuous — 1 bread/bundle ≤ worker's accept → reframed as C1R's fixed-wage twin; escrow-till-release starved the worker → advance paid up front, dissolving the escrow). rb-lite clean in 5 rounds; RoR ACCEPT-AS-HONEST-FINDING (framing correction: lead with real clearing + φ-dependence, not the marginal "vacuous" label). Preserved on `feat/in-kind-wage-impl-rb`. Next probe: the succession / death-seam wall (why the fund + final-window continuity collapse). |
| C1S | Tenure succession — the heir assumes the estate's live share contract at owner death (the death-seam probe; P1.6) | **FINDING — succession removes the death wall, but standing tenure STILL does not form; the arc closes on the worker exit (unmerged).** On the DECOMPOSED base (`feat/in-kind-wage-impl-rb`), flag `share_contract_succession` (tag 26 — flag byte only; the succeeded owner=heir rides tag 23's existing owner field): on a mortal owner's death the heir who inherits the plot assumes the owner side of a live share contract (in-place `contract.owner` rewrite) IF heir re-consents (cap-waste gate) AND worker re-consents (`share_worker_accepts_bread`); else dissolve. Voluntary re-formation, not a ratchet; conservation is a no-op (grain stays with the continuing contract); reuses the isolated `share_tenancy.rs` machinery; scoped to impartible/single-heir (partible/worker-death/in-kind → dissolve). Result (5 seeds × φ, guards all hold — conserved/registry/commons/money/anti-title/identity/succession_registry, fates_consistent): the mechanism ENGAGES robustly (**1–76 successions/seed** survive owner death into heir ownership — NOT `SuccessionInert`; `heir_declined` 1–9 = heirs preferring to work the plot; `worker_re_declined` 5–138 — both voluntary gates bite), but lands uniformly on the pre-named null **`SuccessionButStillTransient`**: NO succeeded relationship reaches the final window (`final_open_succeeded=0`, `post_succession_renewals=0` in EVERY cell) because the worker-satiation fade dominates (`renewal_fed_out == renewal_hints`, 100% fed-out). The owner-death seam was NOT the binding constraint — the worker's satiation exit is. Survival_lift=+4 at scarce φ is the same transient relief (matched-baseline), correctly NOT `StandingTenure` (the succession-specific classifier keys on succeeded relationships, not colony-global open contracts — validated against a seed where open=137/lift=8 would mislabel). **ARC CLOSE:** the fifth elimination — after S22's incentive/capital levers and P1.5's myopia and C1N's money-gap, mortality is now ruled out; every voluntary lever that leaves the worker free to exit when fed yields clears-but-does-not-persist. The single binding constraint on a standing occupational institution here is the **worker's satiation exit**. Spec impl-61, 2 xhigh rounds to SPEC-READY (same-tick death-race guard + succession-specific classifier); rb-lite clean in 2 rounds (two rate-limit interruptions absorbed by retry); RoR ACCEPT-AS-HONEST-FINDING, no P0/P1; two disclosed P3s (conservative same-batch heir-recompute; unexercised renewal-threading). Preserved on `feat/succession-impl-rb`. Next: an explicitly exit-overriding institution (coercion/lock-in — not voluntary) OR pivot to C3R (mortal chain-producers), now unblocked. |
| C3R.a | Mortal chain-producers, no succession (civilization keystone, slice 1 — the mortal production chain) | **FINDING — `ChainCollapsesOnProducerDeath` ×5 (unmerged): the production chain dies with its producers; the motivating null for role succession + capital inheritance.** The whole prior arc ran on IMMORTAL producers (`lifespan: None` non-lineage roster, skipped by the old-age reaper); C3R makes the grain→flour→bread chain mortal. Slice 1 introduces producer mortality and NOTHING else. Flag `mortal_chain_producers` (tag 27, flag byte only), new base `frontier_mortal_producers()` = `frontier_capital` + the flag: the seeded latent mill/bake producers become LIFESPAN-ONLY mortals (demography's own lifespan draw; `household: None` kept, `producer_subsistence` cushion kept → no hearth coupling, no digest delta beyond the flag byte); byte-identical flag-off (`frontier` + `frontier_capital` goldens unchanged). The one companion change closes the confound the 4-round spec-review hammered out: producer FORMATION (build a mill/oven, or adopt Miller/Baker) is gated to MORTAL agents only on both capital-build paths (`run_capital_formation` legacy loop + `start_per_agent_builds`) and role adoption (`run_role_choice`), so the ~68 immortal gatherers/consumers cannot rebuild the chain and fake a self-repair — `immortal_producer_count == 0` is a hard guard. Immortal gatherers/consumers stay as a fixed surround (stable grain supply + bread demand), isolating producer mortality as the single variable. NO role succession, NO capital inheritance, NO mortality term in the capital appraisal (those are C3R.b/c). Result (SEEDS=[3,7,11,19,23], RUN_TICKS=1200, guards all hold — conserved/registry, `immortal_producer_count` max=0): uniform `ChainCollapsesOnProducerDeath` — 130–174 producer old-age deaths/run, milling+baking stages cannot stay jointly staffed (Capital-STAGE staffing collapses to a single surviving producer; era rung sits at Forager on this designated-money base regardless), bread 27–48. The null is instrumented HONEST, not artifact: reservoir provably closed (immortal_max=0), NOT a thin-pool artifact (mortal builder/adopter pool healthy at 5–6, so `CollapseFromThinMortalPool` correctly NOT triggered), NOT apathy (role re-adoptions 125–171/run), NOT an investment freeze (mortal builders complete the 16-cycle-payback mill 2–3×/run). The chain churns through re-adoption + occasional rebuilds but never STABILIZES: a producer dies before its capital pays back and each dead producer's mill sinks to the commons — the payback horizon biting as Böhm-Bawerk predicts (2–3 builds vs ~150 deaths). The satiation exit that closed the voluntary-labor arc does NOT govern here (a chain producer is profitability-locked, not satiation-gated) — the constraint is genuinely new: mortality + the payback horizon. Directly motivates C3R.b (role succession — smooth role transfer instead of frantic re-adoption) + C3R.c (capital inheritance — the mill passes to an heir instead of sinking to commons, preserving payback across generations). Spec impl-62, **4 xhigh spec-review rounds** to SPEC-READY (each REMOVING mechanism toward the minimal experiment: R1 caught the immortal-replacement reservoir → mortal-only formation gate; R2 the additive-hearth double-provision + thin-pool confound; R3 the dedicated-households over-correction → thick-pool scaffold, removed; R4 confirmed the lifespan-only + existing-lineage-refill design). rb-lite clean in 6 rounds; RoR ACCEPT-AS-HONEST-FINDING, no P0/P1 (one non-blocking phrasing note: "Capital-stage staffing collapses; era rung stays Forager", not "regresses from Capital"). Preserved on `feat/mortal-producers-impl-rb`. |
| C3R.b | Capital inheritance for mortal chain-producers (civilization keystone, slice 2 — does the mill passing to an heir lift the C3R.a collapse?) | **FINDING — inheritance preserves chain STRUCTURE in a narrow subsidy window but every persisting seed is FLOW-CAPPED; capital continuity ≠ productive use (unmerged; v2 after a result-review reject).** C3R.a's mill sank to commons on death (capital destroyed). C3R.b gives the mortal producers bounded REPRODUCING households (6 producer houses × 1 producer, cap-swept) so a dead producer's mill routes to a live HEIR via the EXISTING estate seam, and the heir (holding the mill) re-adopts via the EXISTING mortal-gated S7 tool-holder path — NO new succession code (research confirmed estate+S7 already compose; the only missing piece was C3R.a producers having `household: None`). Flag `mortal_producer_inheritance` (tag 28), new base `frontier_mortal_producers_heritable`. **v1 result-review REJECT (the lesson):** the mechanism worked perfectly (294/294 tools inherit, 0 heirless, ~291 heir-adoptions) yet bread=0 — but the producer-household HEARTH floods bread demand at the PINNED `food_provision=3`, flooring the response variable BEFORE inheritance is evaluable (a classify-not-tune violation). **v2 sweeps the confound:** producer-house hearth `food_provision ∈ {0,1,2,3}` (test-level axis mutating the appended HouseholdSpecs — no config knob, no digest change) × producer-house `cap ∈ {1,2,3}` (cap=1 = no-reproduction necessity control); `producer_subsistence` restored (diagnostic ruled it out as the confound); SPLIT verdict `StructureVerdict` (BaseUnviable / SubsidyFloodsChainDies / StructurePersistsUnderInheritance / StructureDoesNotPersist) orthogonal to `FlowVerdict` (FlowCapped / FlowRuns), the v1 `ControlDidNotCollapse` disqualifier removed (the inheritance-denied control is now the matched re-building contrast). Result (SEEDS=[3,7,11,19,23], **1600 ticks**, guards hold — conserved/registry/money, `immortal_producer_count=0`, old bases byte-identical): at `food_provision ≥ 2` the hearth-mint (scaling 0/9387/36612/54924 with the subsidy) FLOODS and all cells die (`SubsidyFloodsChainDies`, bread 0); at `food_provision=0` reproduction is too thin for heirs (`StructureDoesNotPersist`); in the **narrow viable window `food_provision=1, cap=2`, 4/5 seeds → `StructurePersistsUnderInheritance`** — both milling+baking stages jointly staffed ~1500/1600 ticks (the keystone's first STRUCTURAL positive) — but **every persisting seed is `FlowCapped`** (bread ≈ 9, bread-per-staffed-tick ≈ 0.006; seed 7 doesn't persist). The INVERSION is measured not asserted + an intrinsic bind: inheritance sustains structure only by keeping the producer households populated → they reproduce (357 births vs the control's 16) → the reproduction carries the hearth subsidy that floods bread demand (floored bread price, thin trade) → output capped. The matched control (deny tool inheritance → households die back → chain runs on constant RE-BUILDING) makes it a clean contrast: re-building produces real bread (≈1869) but can't hold the stages staffed (`StructureDoesNotPersist`); the `cap=1` no-reproduction control proves inheritance CANNOT persist structure without heirs. **Inheritance buys continuous structure at the cost of flow; re-building buys flow at the cost of structure** — capital continuity sustains occupancy, not productive use (Böhm-Bawerk from the other side). Capital destruction was necessary-but-NOT-sufficient; the binding next constraint is the DEMAND side (breaking the reproduction↔hearth-subsidy coupling), NOT role succession (inherited tools already reach heirs + re-adopt in the viable cell). Honestly narrow: one subsidy point, 4/5 seeds, flow-capped throughout — reported across the sweep (informative L-slice: food at cap=2, cap at food=1), not a broad claim. Spec impl-63; v1 SPEC-READY 1 round, result-review REJECT (confound); v2 focused-confirmation SPEC-READY, rb-lite clean in 2 rounds, result-review ACCEPT-AS-HONEST-FINDING. Preserved on `feat/mortal-producer-inheritance-v2-impl-rb`. |
| C3R.c | Earned provisioning — the circular flow (civilization keystone, slice 3; the attack on the recurring demand wall S23d/S23e/C1/C3R.b) | **FINDING — formal classifier `EarnedIncomeInsufficient` (every headline cell); causal finding: income feeds the LIVING but never funds REPRODUCTION — the prior-saving problem relocates from the wages-fund to the family provisioning fund (unmerged).** Retires both producer-side mints (producer-house `food_provision=0`, `producer_subsistence=0`) and provisions the producer households from the producer's EXTERNALLY-EARNED bread revenue: on a member's own UNPROVIDED Now bread want, a conserved gold transfer funds one loaf at the last realized price and the member bids via its own unmodified machinery (no pool, no bid override, no demand nudge; belief-lag measured via `funded_but_unfilled`). The P0 ledger credits cross-household sales only, split by buyer class — **genuine external = consumers + gatherers + lineage**, producer-class recirculation excluded (`AccountingLoopOnly` pre-named); FIFO earned/endowed gold buckets spend earned-first (`endowment_funded_provisioning` disclosed). Controls: stock-provisioning, no-provisioning, mint-on C3R.b reference (reproduces the landed grid); lineage surround-mint sweep {3,1,0} (0 destructive/disclosure-only). Flag `earned_provisioning` (tag 29), base `frontier_mortal_producers_earned`; all prior bases byte-identical off. Result (SEEDS=[3,7,11,19,23], 1600 ticks; conservation/money/registry hold, `immortal_producer_count=0`, workspace green): **nobody starves** — adults feed themselves at the floored price out of birth-gift gold, and the headline transfer mechanism executed **ZERO times in all 15 eligible runs** (mechanically reachable — branch live by inspection, 16-gold birth gift vs price 1 → gap never positive; a C1-shaped null AND a revealed design mismatch: reactive consumption finance cannot create the proactive stock that binds). **What fails is reproduction**: births 357 (hearth-on) → 1–5; the birth gate debits FOUR SAVED food units from parent stock, and present-hunger buying feeds but never SAVES → no children → no heirs → the structure dies as C3R.b's no-hearth bracket. **A mortal economy whose agents demand only against present need cannot fund its own reproduction.** The price never recovers: surround mint present → floored-but-active (Some(1), ~580 trades); reduced {1,0} → the late-window market dies outright (price None, trades 0) — though producer extinction truncates supply first, leaving the recurring-vs-depleting demand question open. Positives: flood retirement lifted output while staffed (42 vs the reference's 9); Slice-A answered the durability question — **the gatherers (recurring WOOD income) are 87% of genuine external revenue**, not the depleting consumers (one-time 2,640-GOLD endowment). Disclosed limits (RoR): zero transfers ≠ per-member-measured affordability (arithmetic-supported; no branch counters); vocation-aggregated buyer-gold not clean depletion evidence; `members_fed_by_purchase` counts events not dependents; a DORMANT ledger provenance gap (same-household purchases skip without reattaching earned provenance) must be fixed before trusting any future POSITIVE run. Next slice (RoR-endorsed): **individual saving ahead of need** — a parent-facing future-bread motive purchasing + retaining the four loaves through existing market machinery (purchased stock already passes the birth gate; nothing structural missing). Spec impl-64, SPEC-READY in 3 xhigh rounds (P0 self-dealing guard structural; funded-query rule pinned to existing observables); rb-lite clean in 2 rounds; RoR ACCEPT-AS-HONEST-FINDING with the reframed headline + formal label retained for auditability. Preserved on `feat/earned-provisioning-impl-rb`. |
| C3R.d | Saving ahead of need — the birth-stock motive (civilization keystone, slice 4; the first purely BEHAVIORAL slice) | **FINDING — the saving motive works as built and the trap holds: a bootstrap/allocation trap that individual saving cannot escape (unmerged; panel consensus-failure adjudicated).** C3R.c's named missing behavior, built minimally: while a producer household is below cap, members carry the FULL 4-unit `Want{Good(staple), Horizon::Next}` block (the producer-input/scholar-buffer pattern — a real bid + reservation against both eating and the ask ladder; the full-target form is load-bearing, a deficit-count would unreserve accumulating stock); parameter-free (count = `child_food_endowment`, trigger = the gate's own cap). Slice 0 landed the C3R.c ledger provenance repair (lot-preserving reattachment, attribution-only, untracked==0 invariant). A conserved OUT-of-household stock-injection **sufficiency control** (mode-exclusive, rollback, richest-donor deterministic). Flag `birth_stock_saving`, tag 31 (flag + injective mode byte; tag 30 taken); five prior bases byte-identical (scales are digest-serialized, so ON-path digests change by design). Result (SEEDS=[3,7,11,19,23], 1600 ticks; conservation/money/registry hold): **the control confirms the premise emphatically within bounds** — at 690–730 otherwise-eligible opportunities/run, four loaves were the final missing birth condition EVERY time (`failed_injected_births=0`, endowment block → 0, the size cap becomes the binder; births ~2 → 702–730 as CAP-LIMITED replacement throughput, and the claim is bounded: recurring out-of-household REDISTRIBUTION proving local gate sufficiency, NOT endogenous stock formation; donor class unrecorded). **The motive is mechanically alive but cannot restore continuity**: wants 384–540/run, only **3–7 attributable purchases** clear (attribution pinned to a pre-market snapshot), stock reaches four 0–3×, births never materially exceed the reference. Four evidentiary seeds walk three ladder rungs (BirthsResumeStructureStillDies / BirthStockRaceLost / StockReachedBirthsStillBlocked ×2); seed 7 excluded by the pinned mint-on precondition (it never persisted in landed C3R.b — a disclosed one-seed loss per the preregistered guard). Honest diagnosis: a **BOOTSTRAP/ALLOCATION TRAP** — final-window chain output is dead and surviving hearth bread never becomes reproductive stock in producer households; the telemetry cannot yet separate insufficient offerable surplus from priority loss to hungry buyers (~590 trades/run at the floor while the saving bids almost never win). Precise statement: "no uncommitted surplus reaches saving," NOT literally "nothing to save." The circle: no chain → no bread winnable → no birth stock → no heirs → no chain. **An individual saving behavior alone does not escape a low-level equilibrium this deep.** Process (disclosed): rb-lite ended CONSENSUS_FAILURE after 7 rounds over one codex P1 (the implementer's non-bread Earned credits fixed a real C3R.c Miller-income gap but bypassed the bread-only class split); orchestrator-adjudicated — credit kept, CLASS-TRACKED + disclosed as a post-preregistration measurement-only amendment (empirically: producer-class non-bread revenue 0–14 gold vs 510–881 external — the bypass negligible in practice; `AccountingLoopOnly` stays bread-only as preregistered); the round-7 claude reviewer degraded to noise → gated on codex review + independent verification + RoR. Follow-up debt (verified, non-blocking): counter assertions, donor-class telemetry, a synthetic classifier table, ask/bid-loss telemetry. **Next lever — NOT another individual behavior**: instrument the allocation contest, then a coordination/transition institution (granary / priority set-aside / the C3R.b subsidy as a trap-escape with a hysteresis withdrawal test / a finite initial stock bridge — which the recurring control does NOT already prove sufficient). Spec impl-65 SPEC-READY in 2 xhigh rounds; RoR ACCEPT-AS-HONEST-FINDING conditional on the narrowed framing. Preserved on `feat/birth-stock-saving-impl-rb` (tip `45a922e`). |
| C3R.e-obs | The allocation-contest instrumentation (pure observation; the prerequisite both reviews demanded) | **DIAGNOSIS — `MixedDiagnosis` ×5: three correlated economic bottlenecks at ~⅓ each, microstructure 2–7% — the bootstrap trap SURVIVES its sharpest disconfirmation challenge (unmerged; RoR REJECT→repair→ACCEPT).** For every saving quote-opportunity (LITERAL C3R.d attribution-snapshot membership on money-priced spot-pass ticks) that fails, exactly ONE outcome by pinned precedence — NoBidPosted / SelfAskOnly / NoExecutableAskInWindow / AllAsksAboveLimit / CompetitiveLoss{PreEntryOrder→Micro regardless of price | HigherLimit | EqualLimitEarlierSeq | PostExitConsumption→Residual, × bid-post-time winner intent} / ExecutionResidual — from a four-record tick-keyed allocation trace (post-cancellation pass-start book snapshot; TOTAL quote attempts from both money loops; intra-pass quote exits; seq-keyed executions), enabled once, record-only, never serialized, never read by any decision path. INERT TWICE: digest ON = OFF + the single two-byte tag-32 emission (byte-for-byte after removal); the pinned oracle re-runs the FULL C3R.d four-cell classification under observation and asserts the landed verdict enums + exact metrics. Shares over unfilled opportunities (SEEDS=[3,7,11,19,23]): OfferScarcity .22–.38 (includes priced-out), AllocationPriority .25–.35 (higher-limit hungry bidders win), GoldBind .28–.38 (**= NoBidPosted under reservation/posting logic, NOT proven literal gold scarcity** — reason sub-counts not retained), Microstructure .02–.07, Residual ≤.06; no family ≥ ½ anywhere. ENDORSED CONCLUSION (RoR verbatim): "correlated economic bottlenecks, microstructure minor, direct provision as the maximin next intervention" — the families are mutually-exclusive PROXIMATE states, not proven independent walls (near-zero upstream output could generate all three), which is why the selected lever bypasses all three at once (direct stock provision, gate-proven by the C3R.d control) tested as IGNITION + WITHDRAWAL/HYSTERESIS, not assumed sufficient. PROCESS: first submission REJECTED for a false-green guard (the oracle PRINTED expected verdicts; 3 metrics underdetermine the 4-cell classifier) → repaired (shared real classifier asserted; table-driven branch tests incl. the first-(limit,seq) payload rule; an independent per-tick opportunity recount) with the diagnosis byte-identical before/after. Slice 0 cleared the D1 counter-assertion debt. Spec impl-66, 3 xhigh rounds to SPEC-READY; rb-lite clean 5 rounds + 2-round repair; flag `saving_allocation_obs`, tag 32. Preserved on `feat/saving-allocation-obs-impl-rb` (tip `06e3180`). |
| S24a | Endogenous commitment-norm spread (institution-selection arc, slice 1) | **MIXED FINDING — a working institution CAN propagate by generic local imitation of observed success, but cleanly only sometimes.** Gates S22f's commitment behind a per-agent adopts_commitment_norm bit; a deterministic minority seeds it; non-adopters copy a better-off observed neighbour's norm bit scored ONLY on a normalized GENERIC alive/hunger/food score (SALT excluded; score-purity invariant forbids reading institution/profit fields; S22f mechanism otherwise unchanged). Composes on S22f+S22c; digest tag 15 ON-only; goldens byte-identical off. Per-seed tally {3,7,11,19,23}: 3 UniversalCommitmentRePin (3,7,19 — adoption over-spreads past 0.6, fluid buyer tier collapses), 1 DriftNotSelection (11 — its matched random-imitation null also reproduces the core), 1 InstitutionSpreadSuccess (23 — bounded 0.57, surviving buyers, beats its random null). The norm genuinely spreads (seed ~12 → ~50 adopters), non-seed agents commit+renew, a core forms, all copy drivers generic (salt=0), money/mortality/provenance/conservation hold — but under STICKY (never-abandoned) adoption it usually over-spreads or isn't separable from drift; clean bounded selection-driven spread is the minority. Anti-smuggling held (generic score + score-purity + per-seed random null). PROCESS: the rb-lite run died in iteration 1 (main swept, codex implementer orphaned+hung after the suite passed; NO panel ran) → gated on independent verification + Codex review-of-results, which returned FAIL-as-classified → two test-only fixes (per-seed drift check [an aggregate gate had masked seed 11's drift]; honest unprofitable_seed reporting [term=1 still spreads the bit via transient food advantage but forms no core]) → post-patch tally matches Codex's predicted honest outcome. Clean positive (non-sticky/abandonable adoption) deferred to S24b. Workspace 97 suites, all goldens byte-identical (28 goldens_unchanged), commitment_norm_spread 12/12, fmt+clippy clean. |
| S24b | Abandonable commitment-norm adoption (institution-selection arc, slice 2) | **FINDING — abandonable adoption DISSOLVES the institution; generic welfare-imitation doesn't preserve a division-of-labor institution (a "tragedy of imitation").** Makes exactly one change to S24a: imitation is BIDIRECTIONAL/abandonable — every agent each IMITATION_PERIOD copies the better-off observed neighbour's adopts_commitment_norm bit (adopter OR non-adopter) on the same generic alive/hunger/food score (SALT excluded; score-purity invariant); abandonment inside a binding S22f term is staged in next_norm_bit + applied at expiry before renewal (no mid-term break). Composes on S24a; digest tag 16 ON-only; goldens byte-identical off. Verdict NormDiesBack 5/5 {3,7,11,19,23}: the norm dies back COMPLETELY (final adopters=0 every seed; only flips are abandonment, adopt=0 abandon=7-10). MECHANISM (Codex-confirmed): on the generic welfare score the committed CULTIVATORS are not better off than the well-fed fluid BUYERS (buyers 37-48 alive, post_bought ~15k-32k — buyers eat plentifully by buying; cultivators bear the burden), so welfare-imitation flows AWAY from the productive role — nobody imitates into the committed role. S24a's institution only spread because sticky adoption RATCHETED it; remove the ratchet and generic-welfare selection dissolves it. NOT an artifact: the random null DOES adopt + sticky_reference forms cores (false→true path live); the 3 candidate artifact-bugs checked absent. INSIGHT: generic individual-welfare imitation does not preserve a division-of-labor institution when the producers who sustain it aren't individually better off than the buyers they feed. Together S24a (sticky→over-spread/drift) + S24b (abandonable→die-back) = generic-imitation institution selection is a KNIFE-EDGE; clean positive needs role-crediting/group-payoff imitation OR explicit hysteresis = S24c. rb-lite converged CLEAN in 3 rounds (fresh S24b reviewers file); Codex review-of-results PASS-WITH-CAVEATS. Workspace 98 suites, all goldens byte-identical (29 goldens_unchanged), abandonable_norm suite passes, fmt+clippy clean. |
| S24c | Group-payoff imitation (institution-selection arc, slice 3 — ARC-CLOSING) | **FINDING — group-welfare imitation ALSO dissolves the institution; closes the S24 local-welfare-imitation arc as a clean TRIAD of negatives.** Reuses S24b abandonable adoption but scores imitation on local GROUP welfare: an agent selects the best-off nearby group by GENERIC welfare aggregates (alive share / mean hunger-relief / mean food; SALT excluded; group score-purity forbids reading any member's commitment identity), then copies toward that group's ADOPTER-SHARE GRADIENT (adopt if the better-welfare group has materially more adopters, abandon if fewer — the welfare picks the group, the share only sets copy direction). Group membership keyed on each colonist's economic anchor (home_node, canonicalized ON-only, digest-safe). Composes on S24b; digest tag 17 ON-only; goldens byte-identical off. Verdict NormDiesBack 5/5 {3,7,11,19,23}: the mechanism genuinely fires (group_copy_events=10/seed, covariance_samples ~5000) and the group signal is present (positive_group_copy_advantages=10; welfare↔adopter covariance positive in 4/5 seeds, adverse in 1) — but every copy is an ABANDONMENT (adoptions=0, abandonments=10) because the best-welfare GROUP is BUYER-heavy (buyers 37-48 alive, post_bought 15k-32k), so the gradient selects AWAY from adoption and even the seeded adopters drop the norm. TWO rigor catches: (1) a spatial-degeneracy ARTIFACT — rounds 1-2 keyed groups on literal position; the non-hauling majority shares the exchange tile so every group collapsed to the whole population (zero group-copy events); reviewers caught it empirically; round-3 keyed on home_node → groups genuinely differ. (2) a VERDICT MISLABEL (Codex review-of-results, S23a/S24a pattern) — GroupSignalVacuous was routed whenever no ALIGNED adoption occurred, but the spec defines vacuous as "no signal observed", contradicted by positive_group_copy_advantages=10 + positive covariance; test-only classifier fix → NormDiesBack (a signal that fires and selects away is not "no signal"; a negative covariance is an adverse signal). Disclosed scope: the mechanism fires for anchored agents; a synthetic anchor for the tile-sharing majority was rejected (broke the null/unprofitable controls), so S24c tests group-payoff as far as this base's spatial structure allows. ARC CONCLUSION: local welfare-imitation — sticky (S24a) / individual-abandonable (S24b) / group (S24c) — cannot SELECT a division-of-labor institution, because its value is NON-LOCAL (realized through exchange from producers to buyers), so no local welfare observable makes the producer role look best. Future work explicitly new: market-mediated/global signals, contribution accounting, or group reproduction/selection. rb-lite converged CLEAN in 3 rounds (fresh S24c reviewers file, 2nd clean run running); Codex review-of-results PASS-WITH-CAVEATS. Workspace 99 suites / 1581 tests, all goldens byte-identical (30 goldens_unchanged), group_payoff_imitation 14/14, fmt+clippy clean. |
