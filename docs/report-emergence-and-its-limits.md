# OIKOS — Emergence and Its Limits

*An experimental report on emergent money, capital, and survival in a praxeologic simulation.*

> Status: working research report (raw material for a future article). Covers the milestone arc
> through S20. Every result below was built additively behind a default-off flag, kept the prior
> conformance goldens byte-identical, conserved every tick, ran deterministically, and was reviewed
> by an independent second model (Codex) at both the spec and the result stage. Honest negative
> findings are reported as first-class results, not failures.

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
4. **Build (rb-lite)** — a two-implementer / multi-reviewer loop builds against the spec on a feature
   branch until the review panel is clean.
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
zero) — **yet SALT never promoted**: it accrued *zero* indirect-exchange breadth. The reason: with the
mint retired the colony is hunger-stressed, so *food itself* becomes the dominant saleable good, and
the durable token never becomes the saleability hub. **Produced supply was not the missing
ingredient.**

### 4.4 S18 — A produced multi-good economy, perfect coincidence: the necessity beats the token

We added a second produced/gathered good (WOOD) and a real division of labor: bread cultivators ⇄
woodcutters. But this two-good economy has a *perfect* double coincidence of wants (each side wants
exactly what the other makes), so bread↔WOOD clears by **direct barter**. The abundant,
universally-wanted necessity (WOOD) dominated the saleability race (>10× SALT), so SALT never even led.
The finding: *money is not created by "two produced goods" alone* — a medium is only needed to bridge
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

1. **In this model, money emergence had two separable necessary conditions.** A token became money
   only when (a) it won the **saleability** race — more saleable than the ordinary goods, which
   happens when those goods are bad direct media (imperfect double coincidence), *and* (b) the
   **clearing institution** let the medium be both *sold-for* and *spent* (an agent can hold both
   sides of the monetary strategy). The arc separated these: S19 achieved (a) and failed (b); S20
   added (b) and money emerged. Neither alone sufficed. **Honest caveat:** this is a *model* finding,
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
  bootstrap commodity balance are parameters; the no-seed/no-anchor controls fail, but the *sizes* are
  configured (not yet swept under the two-lane book).
- **Closed input-loop, no terminal consumer** (S19/S20) — the cycle goods are wanted only as each
  other's inputs; there is no final consumption sink.
- **Survival off-market via a hearth scaffold** (S19/S20) — survival is deliberately isolated so the
  necessities don't dominate saleability; that hearth is itself a (disclosed) scaffold.
- **Acceptance-share saleability** — "saleability" here is acceptance-share + breadth, not the full
  Mengerian notion (durability, divisibility, transportability, perishability, holding cost).
- **S20 fixes a self-imposed artifact** — the one-offer book was our constraint; part of the S19→S20
  result is institutional insight, part is repairing a modeling limitation. Stated honestly, still a
  result; stated as a universal law, an overclaim.
- **Robustness not yet established** — single-seed promotion; no S20 seed-size sweep, no two-lane
  anchor-density sweep, no 10k-tick horizon. These are the credibility gaps to close before publishing.

### The bounded headline

The single most defensible claim: **"Endogenous money in OIKOS required both a saleability leader and a
market institution capable of monetary round-tripping."** Anything broader (e.g. "money needs a
two-lane order book," or "scaffold-free colony money") overstates what the runs show.

---

## 7. Open directions (to be refined with review)

- **Open-colony integration (the natural capstone).** Embed the working two-lane money into a richer
  *open* colony with on-market survival and terminal consumption, removing the S19 abstractions — does
  money survive contact with the universal-necessity pressure that dominated S18?
- **Endogenizing the clearing institution.** The two-lane book is configured. Can the *richness* of the
  exchange institution itself emerge, or be selected for, rather than switched on?
- **Perishability / carrying-cost as a second saleability lever** (durability advantage for the token),
  the axis deliberately deferred from S19.
- **Robustness:** a long-horizon (10k-tick) mortality smoke test; mortality-on money runs.

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
