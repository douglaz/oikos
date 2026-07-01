# Conditions for Emergent Money, and the Limits of Emergent Occupation

### A praxeologic agent-based study under a no-smuggling, conservation, and adversarial-review discipline

*Working paper distilled from the OIKOS experimental record (milestones S5–S23b). Companion to the
running report `report-emergence-and-its-limits.md`, which carries the per-milestone detail.*

---

## Abstract

We build an agent-based model (OIKOS) to ask whether the core phenomena of Austrian/Misesian economics —
the division of labor, capital, originary interest, entrepreneurial profit and loss, Malthusian
population dynamics, and money — can **emerge** from individual action under scarcity, rather than being
assumed. The model is grounded in strict physical conservation (a per-tick ledger asserted every tick)
and strict determinism (no live randomness; per-agent heterogeneity from a hashed seed), and every
milestone passed a deliberately adversarial pipeline: a written falsifiable spec, an independent
second-model review of the spec, an implement→review build loop, independent re-verification, and an
independent second-model review of the *landed* result. Two methodological guards did most of the work:
**byte-identical conformance digests** (a new, default-off feature must not move any prior result's
canonical state) and a **no-tuning discipline** (set principled parameters, *report* the outcome, sweep
rather than search for a passing value), which together make the **negative** findings trustworthy.

Two headline results. First, **money emergence decomposes into separable necessary conditions**: a token
became Mengerian money only when it had (1) real direct-use demand as a non-circular eligibility floor,
(2) leadership in *medium* (re-trade) saleability rather than in consumption, (3) a tradeable pre-money
supply to circulate against, and (4) an exchange institution permitting monetary round-tripping. Each
condition was isolated by a matched failure; no single one sufficed. Second, and symmetrically, **a
durable division of labor does *not* self-form** from a fluid participation base under any **incentive or
capital** lever: hunger pressure, accumulated skill, a realized profit stay-incentive, sunk *earned* owned
capital, and even *endowed + inherited* capital given up front each demonstrably *bites* yet **none**
produces a persistent occupational class — because every one leaves the hunger/profit **exit** intact, and
that exit rotates agents out regardless of who owns the means. The boundary it names is an institution that
overrides the **exit** itself; a sixth lever supplies exactly that — a **voluntary fixed-term cultivation
commitment** (opt in under one's own realized return, bound for a term, re-decide at expiry) — and it is the
arc's **turn**: for the first time the model forms a persistent, renewing, *minority* committed cultivator
core with a working **two-tier** division of labor (a stable committed core plus a surviving fluid buyer
side), genuinely voluntary rather than a re-imposed pin. By the predeclared aggregate-churn criterion this
still classifies as no-stickiness — the colony-wide churn does not fall, *because* the fluid tier keeps
churning around the stable core — so the honest statement is that an exit-overriding institution stabilizes
an occupational **core**, not the whole colony. A third strand tests the obvious counter-hypothesis — that
the canonical real-world exit cost is **private property in land**, not a contract — and finds it does
**not** rescue occupation: a *use-it-or-lose-it* tenure rule makes the exit cost itself churn (entry thrashes
at roughly ten times the commons baseline), and a post-money *alienable land market* with a genuinely
endogenous, rent-tracking price goes **thin** over the owner-dominance that private tenure produces, never
forming an owner-cultivator class; what stabilized an occupation here was contract, not property — the
design of the exit-cost institution decides the outcome. We report all of these as **model findings, not
theorems**, and disclose the configured scaffolds that remain.

---

## 1. Introduction

Mengerian theory holds that a medium of exchange arises spontaneously because some commodity is more
*saleable* than others, letting agents trade indirectly when no direct double coincidence of wants
exists. Austrian theory more broadly treats the division of labor, capital, and interest as emergent
features of purposeful action under scarcity. OIKOS is a testbed for taking those claims literally: can
each phenomenon arise from agents pursuing their own ends, with **no smuggling** — no configured
preference that assumes the conclusion (no "agents want money" before money exists; no "agents prefer
bread" so the production chain has a reason to run)?

The contribution is twofold. Substantively, we offer a **condition decomposition**: rather than a single
"money emerged" event, a sequence of matched successes and failures isolates *which* conditions money
emergence requires, and a parallel sequence isolates *which* the emergence of a stable occupation does
**not** get for free. Methodologically, we offer a discipline for making such claims credible in an ABM
— conservation, determinism, byte-identical regression digests, adversarial second-model review at both
the design and result stages, and the treatment of precisely-bounded negative findings as first-class
results.

---

## 2. Model and method

**The economy.** Goods are gathered from depleting/regenerating resource nodes or transformed by labor
through recipes. Physical conservation is enforced by a per-tick identity asserted on every good every
tick:

```
after = before + regen + endowment + produced − consumed_as_input − consumed − promoted − spoiled
```

so nothing is created or destroyed except through a named, accounted channel. The simulation is strictly
deterministic — no live RNG in the loop; per-agent heterogeneity comes from a hashed seed — so each run
is a fixed, reproducible trajectory pinned by a digest of canonical state.

**The pipeline.** Each milestone went through: (1) read-only research mapping the relevant machinery to
grounded `file:line` facts; (2) a written spec stating purpose, an honest *falsifiable bar*, the slices,
the acceptance tests, and the named principled-failure modes; (3) an independent second-model
**spec review** (Codex) iterated to readiness; (4) a build (typically a two-implementer / multi-reviewer
loop) against the spec until the review panel is clean; (5) independent re-verification of the suite,
the digests, and the load-bearing assertions; (6) an independent second-model **result review** of the
*landed* artifact — is the finding genuine, was anything tuned to pass, is the claim honestly scoped;
(7) merge and record. The pipeline was not uniformly clean — reviewer panels often degraded, and runs
occasionally died on transient API failure or external interference and were relaunched — and a few
milestones were gated by the orchestrator's independent verification plus the result review rather than a
full clean panel. The invariant was the second-model spec- and result-review on *every* milestone.

**Two decisive guards.** (a) **Byte-identical digests**: every new capability is added behind a
default-off flag and canonicalized "on-only," so the prior milestones' digests must remain identical
when the flag is off; this caught accidental coupling immediately and underwrites every "all goldens
byte-identical" claim. (b) **No tuning**: parameters are set on principle, the outcome is *reported*, and
where a value could be load-bearing it is *swept*, not searched — so a negative result cannot be a
failure to find a passing setting, and a positive result cannot be an artifact of one.

---

## 3. Foundational mechanisms

Before money, the production, capital, demographic, and survival mechanisms were each shown to
self-organize (most demonstrated with money present — they are non-money-rule colony mechanisms; the
*emergence of money itself* is §4, and whether a *division of labor self-forms from a fluid base* — as
opposed to a chain running on assigned roles — is §6). The roles here are designated or emergent-by-tool;
the self-formation of an occupational class is the separate, negatively-answered question of §6:

- **Specialized chain & provisioning (S5–S6).** A specialized grain→flour→bread production chain *runs
  and sustains at population scale* among gatherers, millers, and bakers trading through an exchange,
  under designated or already-emergent roles. (Whether a *division of labor self-forms* from a fluid
  base — rather than running on assigned roles — is the separate question of §6, which answers it
  negatively.)
- **Producible capital (S7).** Tools are produced from wood + labor over time via a project lifecycle —
  capital is a roundabout, time-consuming investment, not a fixed endowment.
- **Originary interest (S10).** Per-agent time preference makes the capital decision an ordinal
  intertemporal choice — interest as a feature of valuation, not a configured rate.
- **Entrepreneurial profit/loss (S11).** Fallible, biased per-agent price forecasts drive production;
  a real shock perturbs the economy and loss *selects* through capital — error made falsifiable.
- **Spatial households (S13).** The reproducing lineage is unified with the spatial world so the
  population that *grows* can *work the land*.
- **Forage carrying capacity — the Malthusian preventive check (S14).** Foraging is a capped commons
  (per-capita yield falls with crowding); population grows while fed and *plateaus* when scarcity
  raises hunger past the birth ceiling and births stall — an endogenous carrying capacity, no deaths
  required.
- **Agricultural intensification — Boserup (S15).** Under forage scarcity the unfed surplus *cultivates*
  bread (a more roundabout, more laborious tap on the abundant grain resource), *raising* carrying
  capacity. Cultivation is adopted *only* under pressure — the authentic driver is population pressure,
  not a seeded preference for bread.
- **Mortality — the Malthusian positive check (S17).** Re-enabling starvation on the plateaued colony
  at principled thresholds yields a genuine oscillating carrying-capacity *band* (births and deaths both
  phase-track hunger, no drift, no extinction). The insight: the preventive check binds on *potential*
  births while the positive check binds on *already-living* marginal mouths — both operate at once, on
  two different populations.

---

## 4. Money emergence: a decomposition into necessary conditions

Money was the hardest thread, and the arc became a progressive isolation of *exactly what emergence
requires*. We tried to make a neutral token (SALT) emerge from real exchange.

- **Emergence under a scaffold (S8/S9).** SALT promotes to money without a circular "medium want": it
  has a real, heterogeneous direct *use* (the regression-theorem anchor) and must clear a genuine
  *indirect-acceptance breadth* gate. This passed — **within a provisioned ecology**.
- **The first deep finding: a minted-supply scaffold (S12).** Retiring the colony's food *mints*
  (hearths producing bread with no labor) showed the S9 emergence was not provision-autarkic: the minted
  hearth was the load-bearing *supply* — the counterparty the SALT-holders circulated *against*.
  Removing it removed the bread *seller*, and the one-offer barter book turned "no seller" into a
  circulation choke. Strong-bar emergence is genuine in a *provisioned* bread economy, not yet in a
  fully *produced* one.
- **Produced supply alone yields direct trade, not a medium (S16).** On a genuinely produced economy,
  letting produced bread trade for SALT formed and scaled a market (100% produced bread, by a provenance
  ledger) — **yet SALT never promoted**: the bread-for-SALT trades were *direct* final-good purchases
  (acquiring bread to eat), giving the token no *indirect* re-trade breadth. Produced supply alone gives
  direct food trade, not a monetized medium. (This is not "food is the money.")
- **Perfect coincidence: the necessity beats the token (S18).** Add a second good (WOOD) and a real
  bread⇄WOOD division of labor, and the economy has a *perfect* double coincidence — it clears by direct
  barter, and the abundant universal necessity dominates the (legacy total-acceptance) saleability race.
  A medium is only needed to bridge *absent* double coincidence (Menger/Jevons).
- **The token leads, but clearing deadlocks (S19).** A 3-good cycle with *no* pairwise double
  coincidence makes the cycle goods bad direct media, so the neutral token finally **wins the
  saleability-leader race** — yet still does not promote: a **one-live-offer-per-agent** book left every
  agent posting "give output → SALT" while missing the complementary "give SALT → input," so no indirect
  SALT trade cleared. The remaining barrier was isolated all the way down to the **clearing institution**.
- **The resolution: a two-lane order book (S20).** Enriching the *exchange institution* (not the money
  rule) so each agent can hold **both** a bid (`give SALT → input`) and an ask (`give output → SALT`) at
  once lets the seeded SALT round-trip the ring by ordinary **pairwise** matching. **SALT promoted** — it
  led, then cleared the *unchanged* strong-bar gate; the medium genuinely round-trips. Authenticity held:
  the matcher stayed strictly pairwise (no central clearing-house that would settle the triangle
  *without* money), the barter/clearing code was byte-for-byte unchanged, and controls proved money
  load-bearing (flag off → the S19 deadlock returns; no SALT seed → nothing clears).

**The four conditions.** A token became money only with: **(1)** real direct-use demand (a non-circular
eligibility floor); **(2)** *medium*-saleability leadership — most accepted in re-trade, not in
consumption; **(3)** a tradeable pre-money surplus to circulate against; **(4)** an exchange institution
permitting monetary round-tripping. The failures were diagnostic: necessities dominate *consumption*
metrics (S18), produced supply alone yields *direct* trade (S16), imperfect coincidence lets the token
*lead* but one-offer clearing *deadlocks* (S19), and retiring the minted food *seller* removes the
counterparty the medium circulates against (S12). Under open survival that same retirement *collapses*
the pre-money market to zero trades (S21d) until endogenous production refills it (S21e/f). **Honest caveat:** condition (4) is partly a genuine economic
insight (a market needs an institution expressive enough for monetary round-tripping) and partly the
repair of a *self-imposed* constraint (our one-offer book artificially forbade holding a bid and an ask
at once); the defensible statement is institutional — *money required both a saleability leader and an
exchange institution capable of monetary round-tripping* — not "money needs a two-lane order book."

---

## 5. The open-colony capstone: supply, mortality, and a band-qualified result

S20's money lives in a closed *exchange cycle* with off-market survival. The capstone embedded it in an
*open* colony, removing those abstractions: the first slices make survival **market-financed** (agents
buy food), and the later mortality-on coexistence result then requires a *disclosed emergency
subsistence bridge* (S21h below), not pure market survival:

- **Two-layer saleability (S21a/b).** A first slice found the original "saleability = total acceptance
  share" metric conflates *consumption* with *medium* use — which is why the universal necessity won
  S18. Splitting it into a **direct-use eligibility floor** (the non-circular regression anchor) and
  **medium-saleability leadership** (observed re-trade breadth) lets the durable medium promote over the
  necessity **in a controlled scenario**. In the open colony this fixed the *metric*, not the *supply*
  gate — promotion there still waited on the supply result below.
- **The supply question, closed (S21c–f).** Retiring the food scaffold under open survival first
  *collapsed* the pre-money market to zero trades (S21d) — confirming supply *generation*, not the money
  machinery, was the gate. A one-time **seeded** tradeable surplus was then shown *sufficient* (S21e),
  and finally **endogenous pre-money production-for-barter** (lineage households cultivate `SelfProduced`
  bread and barter the surplus, no seed and no mint) monetized SALT (S21f) — money bootstrapping from
  genuine pre-money labor.
- **Mortality: a cold-start finding (S21g).** Turning the positive check on over the open-market money
  colony *culls* the non-cultivating demand side (the SALT-rich buyers + specialist woodcutters) in a
  one-off cold-start cull before the market can form: money **or** mortality, not both — the pre-money
  bootstrap needs the demand side to survive a long hungry foodless wait, and mortality kills that
  patience.
- **Resolution: a demand-side survival floor (S21h).** A produced, no-grain-input, self-consumed
  own-labor emergency floor (a *configured subsistence institution*, immediately eaten, no offerable
  remainder) keeps the demand side **alive and still hungry**, so SALT promotes on the lineage's
  `SelfProduced` bread and **money and mortality coexist** — after a one-off cull (a partial bridged
  band, durable to 10k ticks). A one-time *seeded* cushion does **not** thread it (the knife-edge
  finding): a static stock either culls (too small) or sates demand out of the market (too large); only
  a recurring near-critical *produced* floor sits in the window.
- **Robustness: the capstone is MIXED (S21i).** A test-additive sweep classifies the coexistence across
  12 seeds and parameter bands with the same provenance/demand machinery. The headline regimes are
  seed-robust, but the result is **band-qualified**: robust on grain flow, hard-bounded on the emergency
  threshold, but **load-bearing on WOOD scarcity** (the lineage must sit at the WOOD floor — one notch
  of relief collapses the bread→SALT medium lane) and on **SALT-anchor density** (a non-monotonic hole).
  Money+mortality coexistence is real and seed-robust but an *existence proof within a disclosed
  envelope*, not a broad result.

The honest frame for the whole money arc is therefore **condition decomposition** — which conditions are
necessary, and how wide each one's window is — not a claim of spontaneous open-colony order.

---

## 6. The division of labor: five exit-preserving negatives, then an exit-overriding turn

S21's open colony still *pins* the food-producing class: a pre-identified cultivator lineage supplies the
bread while non-lineage buyers and woodcutters never cultivate. The role-topology arc relaxes that
privilege and asks whether a stable division of labor *self-forms*. The first five levers are a clean
negative — each leaves the hunger/profit **exit** intact, and each fails; the sixth overrides the exit and
**turns** the result. Every lever was shown to genuinely **bite** (a mandatory non-vacuity test).
Stickiness was measured the same way throughout — a material drop in
enter/exit churn versus a matched-seed baseline, *and* a persistent **membership** cohort (the same
agents — or, for the inheritance step, the same lineages — staying in the role).

- **Hunger discovers the role, but only fluidly (S22a).** Relaxing cultivation eligibility from "lineage
  member" to "any spatial colonist under sustained hunger pressure" keeps money and mortality alive — but
  the result is **fluid, rotating participation**, not a class: only ~5% cultivate at any instant yet
  *every* non-lineage agent rotates through (high churn). "Everyone occasionally self-provisions under
  hunger, then returns to buying." The lineage *privilege* dissolves; a sticky occupation does not appear.
- **Accumulated skill does not change the exit (S22b).** A per-agent cultivation skill that raises grain
  haul (conservation-safe; a faster draw on the conserved node) *bites* — a maxed cultivator out-hauls a
  novice 2× — yet produces **no stickiness**: the exit is hunger-gated, so agents leave as soon as hunger
  eases regardless of skill; churn is unchanged. Even with skill driven to its cap (≈40% grain share)
  there is no cohort. Productivity-while-in does not make an agent decide to *stay*.
- **A realized profit stay-incentive retains only marginally (S22c).** Letting a cultivator *stay* past
  the hunger exit when its realized post-money cultivation-sale return clears its outside option (a
  non-circular, realized-proceeds signal, inert before money) *bites*: a genuine counterfactual exit-flip
  fires and the signal discriminates across agents. Yet churn barely moves and no cohort forms — a stay
  *incentive* is not enough.
- **Even sunk, owned, durable capital does not produce a class (S22d).** A buildable, durable,
  agent-owned, role-specific cultivation tool (a sunk WOOD cost; raises *only its owner's* haul *only
  while cultivating* — asset-specific; flowing through the unmodified S22c stay-decision, no fiat flag)
  *bites hardest of all* (on 4 of 5 seeds; one seed inert, with no tool formed): owners durably
  out-produce and take up to **71%** of harvested grain. Yet occupation still does not form: owner-share
  stays a tiny minority and no four-owner cohort appears — and a parameter sweep *probes the obvious
  WOOD-cost confound and finds it does not explain the result* (cheap tools and large boosts still
  produce no cohort; pushing the boost higher drives the buyer side toward *monopolization*, not a
  healthy split).
  The boundary is a **chicken-and-egg**: the lock-in asset can only be *earned by already sustaining* the
  fluid role, so a rare one or two agents capitalize and dominate rather than a class forming.
- **Even endowed, inherited capital does not produce a class (S22e).** The chicken-and-egg suggests the
  escape: give the lock-in *up front* and let it pass *down a lineage*. A minority of lineage households is
  **endowed** with a plow at generation, and plows **inherit** to the household heir (a switch the
  falsifying control flips to force them to the commons instead). The lever bites *massively* — **641–681
  real plow→living-heir inheritance transfers per run**, heirs cultivate, owners out-produce ~3× — yet
  occupation *still* does not form: cultivation settles ~4%, churn stays ~1× baseline, and **no
  owner-lineage cohort appears (0/8)**. It is not an unreachable bar: across the full endowment sweep
  (owner-lineage share 0.12→1.00) the cohort is **flat 0/8**, and even at *universal* ownership — where
  owners do most of the cultivating — no lineage persists in the role; they rotate. The decisive reading:
  removing the acquisition chicken-and-egg changes nothing, because the binding constraint is the
  hunger/profit **exit**, which rotates owners out *regardless of who owns the means*. (Honest scope: a
  dynastic/institutional-sufficiency test bounded to this endowed/inherited plow institution in this
  configured colony — not a universal claim about capital or inheritance.)
- **An exit-overriding voluntary institution turns it — a stable occupational core forms (S22f).** The five
  negatives all left the exit intact, so the sixth lever changes the exit itself: a **voluntary fixed-term
  cultivation commitment**. Post-money, an eligible agent whose *own* realized cultivation-return signal
  clears its outside option may **choose** to commit for a fixed term; while committed the hunger/profit
  exit cannot turn cultivation off; at expiry it re-decides from fresh returns (a renewal only if the signal
  still clears). For the **first time in the arc** a persistent occupational cohort forms: 159–450 agents
  *voluntarily* commit per run (each traceable to its own cleared signal) while ~1800–2100 eligible
  below-floor agents *decline*; 14–30 committed ids cultivate ≥½ the final window, every one renewed from a
  fresh signal, a bounded **minority** (commit-share 0.07–0.20); the committed core takes 0.85–0.95 of grain
  while a **fluid non-committed buyer majority survives and materially buys** — a working **two-tier**
  division of labor. It is genuinely *not* a re-pin: a `fiat_pin` control (forced commitment) classifies
  separately, an unprofitable offer gets zero uptake, and a one-tick term forms no persistence. **The one
  thing that does not happen:** colony-wide per-agent churn does *not* fall (the predeclared success bar), so
  the formal classification stays *no-stickiness* — but it fails *because* the model now has a stable core
  plus a still-churning fluid tier, which the aggregate metric (built for the all-fluid regimes above)
  cannot register. (Composition note: commitment *plus* capital tips into monopolization — the core takes
  all grain and the lineage goes extinct — reported as a separate cull finding, not the no-capital headline.)

**The pattern.** Five levers that leave the **exit** intact — hunger, accumulated skill, a realized profit
incentive, sunk *earned* owned capital, and *endowed + inherited* capital — each *bite* but **none**
converts fluid participation into a division of labor; the binding constraint is the hunger/profit **exit**,
which rotates agents out regardless of incentive or ownership. The sixth lever **overrides the exit** by
voluntary contract, and it is the arc's **turn**: a persistent, renewing, minority committed cultivator
**core** and a two-tier market finally form — the first stable occupation in the arc — though *colony-wide*
churn stays high because the fluid tier persists around the core. Honestly stated: occupation in this model
needs an institution that changes the **exit**, and even then it stabilizes a **core**, not the whole
colony. That is a sequence of five falsified sufficiency claims followed by a sixth that succeeds at
core-formation but not colony-wide stabilization — a decomposition, not a single yes/no.

---

## 7. Private property in land does not rescue occupation

S22f stabilized an occupational core by making the *exit* costly through a voluntary contract. That raises
the obvious counter-hypothesis: the canonical real-world exit cost is not a contract but **private property
in land** — scarce, excludable, owned, costly to re-acquire once given up. The whole division-of-labor arc
above ran on a world where the means of production is a *commons* (a resource node has no owner; a lapsed
cultivator re-enters for free). The S23 arc switches on the missing precondition and asks whether private
land is the exit cost that produces a class. Two institutional designs were tested, and both fail — in
opposite ways.

**Forfeiture (S23a) *thrashes*.** When grain plots are excludable, heterogeneous (good-near, poor-far),
claimed by homesteading labor, harvested owner-only, **lost if left idle**, and inherited — with a *spatial*
re-entry penalty (abandon your good central plot and a nearer agent takes it; you can only re-homestead far,
poor land) — occupation does not stabilize; it churns *harder*. Plots are claimed, lost-on-idle, and
reclaimed by another in rapid succession, so per-cultivator churn runs about **ten times** the commons
baseline, owners take 75–80% of grain, and no stable cohort forms. (A first run returned a clean *hard
barrier* to entry; review caught it as a capacity artifact — far fewer plots than agents, with total land
never swept — and a population-scaled land axis made the barrier vanish, leaving the thrash as the real
finding.) Making the exit costly is necessary but not sufficient: an *involuntary forfeiture* rule makes the
cost itself churn.

**An alienable market (S23b) goes *thin*.** The gentler, more authentic institution is illiquid land priced
*after* money exists. Post-promotion, plots become assets bought and sold for SALT at an **endogenous**
price — capitalized from each plot's realized yield (good plots trade ~86 SALT against marginal plots' ~1),
never a hardcoded constant — with a carrying cost, sale-or-foreclosure instead of forfeiture, and a budget
hysteresis (a lapsed farmer who sold its land and spent the proceeds is priced out of re-buying). The price
machinery works and conserves, but the market is **too thin to be load-bearing**: only a handful of title
trades clear, churn does not fall, and no owner-cultivator cohort forms. The honest caveat sharpens the
result: the no-market baseline on this base is *already* owner-dominant and buyer-thin — because S23a's
private tenure has *already collapsed the buyer side* — so the market is being asked to rescue a regime that
private property has already hollowed out, and it cannot.

**The pattern.** Across both designs, **private property in land does not rescue occupational formation, and
it collapses the buyer economy that a functioning division of labor needs** — forfeiture by thrashing entry,
the market by going thin over the owner-dominance that private tenure produces. This is the same lesson as
the division-of-labor arc, seen from the property side: *the design of the exit-cost institution decides the
outcome.* A voluntary contract (S22f) made leaving costly in a way that bound without churning, and a
two-tier market survived; the two property institutions made leaving costly in ways that either churn
(forfeiture) or hollow out the demand side (market). What stabilized an occupation here was contract, not
property.

---

## 8. Discussion

The two arcs are mirror images under the same discipline. For **money**, we found a set of conditions and
*met* them, and emergence followed — but only inside a disclosed envelope, and partly by repairing a
self-imposed institutional constraint. For **occupation**, we probed five exit-preserving sufficiency
mechanisms and *falsified* each — the consistent shape of the failures naming the missing condition (an
exit-overriding institution) — then supplied that institution (S22f) and got the arc's first stable
occupational core, though not colony-wide stabilization. A third strand then tested whether **private
property in land** — the canonical real-world exit cost — could substitute for the contract, and found it
could not: forfeiture tenure thrashes, an alienable market thins out, and both collapse the buyer economy a
division of labor needs. The unifying lesson across the occupation and property strands is that *the design
of the exit-cost institution decides the outcome* — making leaving costly is necessary but not sufficient;
the cost must bind without itself churning or hollowing out demand, which here only a voluntary contract did.
In all cases the value is the decomposition: the model converts vague claims ("money emerges,"
"specialization emerges," "property creates a propertied class") into a graded ledger of *which* conditions
are necessary and how robust each is.

Two things make the negatives credible rather than mere absences. First, every lever was held to a
**non-vacuity** standard: we proved the mechanism materially changed agent behavior (skill raised haul,
the profit signal flipped a real exit, the tool raised owner output) before concluding it failed to
produce a *class* — so "no occupation" is never "the lever did nothing." Second, **isolating controls**
distinguished the intended cause from confounds: a productivity-only control and a non-durable control
showed S22d's (absent) stickiness would not have come from raw output; the WOOD-poverty confound on the
capital result was ruled out by a sweep. This is the same logic that, on the money side, distinguished
genuine emergence (S20) from a scaffolded artifact (S9/S12) via provenance ledgers and removal controls.

Methodologically, the study suggests that ABM claims about emergence are most trustworthy when (i) the
substrate conserves and is deterministic, (ii) regression digests make accidental coupling impossible to
miss, (iii) parameters are swept rather than searched, (iv) an independent model adversarially reviews
both the design and the result, and (v) negative findings are reported as first-class, precisely-bounded
results.

---

## 9. Limitations and threats to validity

We state the scaffolds plainly; a hostile reader should attack these first.

- **Configured strong-bar thresholds and SALT anchor.** What counts as "monetary breadth," the
  regression-theorem direct-use anchor, and the bootstrap commodity seed are *set, not derived*; the
  no-seed / no-anchor controls fail by construction. Their *sizes* were swept: the **in-cycle** (S20)
  promotion is robust across the pinned seed-size and anchor-density bands, but the **open-colony**
  coexistence is *narrow* in anchor density — including a non-monotonic period-12 promotion hole (S21i) —
  and the values themselves remain configured.
- **The S20 money is in a produced exchange *cycle*, not a scaffold-free colony.** Survival is isolated
  off-market and the input loop is closed (no terminal consumer) — deliberate abstractions that isolate
  the money question. The earned claim is "endogenous token money in a produced exchange cycle."
- **SALT is load-bearing, not uniquely destined.** Controls prove a medium is required; they do not
  prove only SALT could monetize.
- **The open-colony coexistence is band-qualified (S21i).** Money+mortality coexistence is seed-robust
  but load-bearing on WOOD scarcity and anchor density — an existence proof within a disclosed envelope.
- **The occupation results are bounded** to these levers in this fluid, WOOD-poor, mortality-on regime
  (and, for the inheritance step, to an endowed/inherited *plow* institution over lineage households).
  "Exit overrides ownership" (S22a–e) is a finding about *this* hunger/profit exit, not a law about all
  exits. **S22f's positive is a bounded, qualified one:** an exit-overriding *voluntary fixed-term*
  commitment forms a stable occupational **core** + two-tier market, but does **not** lower colony-wide
  churn, and does **not** generalize to a colony-wide occupational order; it is one institution design over
  this configured colony, classified by the predeclared bar as no-stickiness with a reported positive
  sub-result, not relabeled a success. Other institution/endowment designs (assignment, guild, credit) and
  why the core does not spread remain open.
- **Part of S20 repairs a self-imposed artifact.** The one-offer book was our constraint; the S19→S20
  result is part institutional insight, part removal of a modeling limitation. Stated as such it is a
  result; stated as a universal law it would be an overclaim.
- **All of the above are model findings, not theorems.** The contribution is a disciplined decomposition
  within one simulator, not a proof about economies in general.

---

## 10. Future work

- **Why the committed core does not spread colony-wide.** S22f's exit-overriding institution stabilizes an
  occupational *core* but leaves a churning fluid majority, so aggregate churn stays high. The live question
  is what would convert the two-tier structure into colony-wide occupational order — a stronger or
  differently-targeted commitment, or a structural reason the fluid tier is irreducible here. There is a
  legitimate measurement point too: colony-wide churn penalizes the *deliberately* fluid buyer tier, so a
  **two-tier-stability metric** (e.g. churn restricted to committed-eligible agents, or core persistence)
  may be the right measure of core-formation. But to avoid success-bar repair it must be introduced as a
  **predeclared, out-of-sample measurement appendix with its own predictions** — not a retroactive
  conversion of S22f's verdict. Under the discipline of this paper, S22f stands as a classified negative
  with a reported positive sub-result; re-measuring it is future work, not a relabel.
- **Other exit-overriding institution designs** beyond the voluntary fixed-term contract — assignment, a
  guild with a standing membership benefit, self-raised exit thresholds — as alternative paths to the same
  core, and a check on whether they avoid the commitment+capital monopolization/cull regime S22f surfaced.
- **Endogenizing the remaining money scaffolds** — the two-lane clearing institution and the SALT
  direct-use anchor — and broadening the open-colony robustness beyond the WOOD-load-bearing envelope.
- **An alienable land market over a deliberately *non-collapsed* two-tier base** — S23b was confounded by
  testing a market on top of S23a's already owner-collapsed tenure; the clean version starts from S22f's
  two-tier-preserving contract and adds the market, asking whether land ownership becomes *independently*
  load-bearing or whether the contract remains the real stabilizer (with a commitment-off control to rule out
  the market merely inheriting the thin/collapsed regime). Deferred, because attribution is messy when the
  contract is already solving the exit problem.
- **Endogenous institution selection — the natural next frontier, now opened.** The study has shown that
  *specific* hand-designed institutions make or break emergence (which exchange rule, which exit-cost
  institution). The next question is not another hand-designed institution but whether agents or populations
  can **select among institutions** based on their own survival and trade outcomes — i.e. whether the
  institutional layer the experimenter has so far supplied can itself emerge. Two slices in, the answer
  is a sharp negative pair. S24a (gate S22f's commitment behind an adopted-norm bit and let it **spread by
  generic local imitation of observed success** — a generic survival score, never institution identity, with a
  score-purity invariant and an outcome-blind null) gives a *mixed* result: the institution propagates and
  recreates a core, but under *sticky* adoption it usually over-spreads or cannot be separated from drift. S24b
  makes adoption **abandonable** (bidirectional imitation, so the norm can be dropped) — and the institution
  **dies back completely**, because on a generic individual-welfare score the committed *producers* are not
  better off than the *buyers* they feed, so imitation-of-success flows *away* from the productive role. So
  generic-imitation institution selection is a **knife-edge** — sticky over-spreads, abandonable dissolves —
  and the deeper result is a *tragedy of imitation*: **generic individual-welfare imitation does not preserve a
  division-of-labor institution when the producers who sustain it are not individually better off than the
  buyers they feed.** The clean positive now requires a genuinely different selection signal — **role-crediting
  / group-payoff imitation** (imitate what the productive agents do, or what groups containing them achieve) or
  **explicit adopt/abandon hysteresis** — and then genuine *choice among* competing institutions. Those are the
  open slices.

---

## 11. Conclusion

OIKOS turns two slogans into ledgers. Mengerian money in this model did **not** emerge from direct demand,
produced supply, or multi-good trade alone; it emerged only when a direct-use eligibility floor, medium-
saleability leadership, a tradeable pre-money supply, and a round-tripping exchange institution aligned —
and even then inside a disclosed, band-qualified envelope. A durable division of labor, by contrast, did
**not** self-form from a fluid base under any of five **incentive or capital** levers — hunger, skill, a
profit stay-incentive, sunk *earned* owned capital, or even *endowed + inherited* capital given up front —
because each leaves the hunger/profit *exit* intact and that exit rotates agents out regardless of incentive
or ownership. Only the sixth lever, which **overrides the exit** by voluntary fixed-term commitment, turns
the result: a stable, renewing, minority committed cultivator *core* and a two-tier market form — the arc's
first stable occupation — though colony-wide churn stays high because the fluid tier persists, so by the
predeclared bar it remains a classified negative with a real positive sub-result. Occupation here needs an
institution that changes the *exit*, and even then stabilizes a core, not the whole colony. And the obvious
substitute for the contract — **private property in land** — does not rescue occupation in either form
tested: a forfeiture rule makes the exit cost churn, an alienable market thins out over owner-dominance, and
both collapse the buyer economy a division of labor needs; what stabilized an occupation here was contract,
not property, because the *design* of the exit-cost institution decides the outcome. Across all three arcs
the method is the message: emergence claims in an ABM become trustworthy when the substrate conserves, the
runs are deterministic and regression-pinned, parameters are swept rather than searched, an independent
model audits design and result, and both the precisely-bounded "no" and the carefully-qualified "partial
yes" are treated as real findings.

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
| S16 | Money from produced bread | finding: produced supply → *direct* trade, no monetized medium |
| S17 | Mortality | mechanism: the full Malthusian band (positive check) |
| S18 | Produced multi-good money | finding: perfect coincidence → necessity beats the token |
| S19 | Imperfect-double-coincidence cycle | finding: token *leads* but one-offer clearing deadlocks |
| S20 | Two-lane bilateral order book | **resolution: endogenous token money emerges (in-cycle)** |
| S21a/b | Two-layer Mengerian saleability | resolution: direct-use floor + medium-saleability leadership |
| S21c–f | Open colony, supply question | **closed: endogenous pre-money production-for-barter monetizes SALT** |
| S21g | Mortality over the open colony | finding: cold-start cull of the non-cultivating demand side |
| S21h | Demand-side survival floor | **resolution: money + mortality coexist after a one-off cull** |
| S21i | Robustness appendix | **MIXED: coexistence band-qualified (load-bearing on WOOD scarcity + anchor density)** |
| S22a | Endogenize cultivation entry | finding: cultivation self-forms as *fluid* participation, not a class |
| S22b | Cultivation skill | finding: accumulated productivity does not change the hunger-gated exit |
| S22c | Profit-driven retention | finding: a realized stay-incentive bites but retains only marginally |
| S22d | Durable role-specific capital | finding: even sunk owned capital → dominant few, not a class (chicken-and-egg) |
| S22e | Endowed + inherited capital | finding: even capital given up front + inherited → owners rotate, no lineage cohort (the *exit* binds, not provenance) |
| S22f | Voluntary fixed-term commitment | the turn: an exit-overriding voluntary contract forms a persistent, renewing, minority committed **core** + a two-tier market (first stable occupation); formal verdict still no-stickiness (colony-wide churn unchanged), not a re-pin |
| S23a | Private land tenure (forfeiture) | finding: use-it-or-lose-it land *thrashes* (churn ~10× commons, no cohort); the exit cost itself churns. (A first run's hard-barrier was a capacity artifact, caught in review) |
| S23b | Post-money alienable land market | finding: genuinely endogenous rent-tracking price, but the market goes *thin* (few trades, churn unchanged, no owner-cultivator cohort) over S23a's already owner-collapsed base; property does not rescue occupation, contract does |
