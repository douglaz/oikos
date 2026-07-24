# The C3R keystone — does production survive the producer?

*A consolidated write-up of the C3R arc (C3R.a–k). The per-milestone sections in
`report-emergence-and-its-limits.md` §§27–39 remain the incremental record and the source of every number
here; this document is the arc read as one argument.*

---

## The question

Every economy this program built up to C3R rested on a quiet exemption. The grain→flour→bread chain — the
miller and baker who own the mill and oven, the thing that makes the settlement more than a subsistence
camp — was generated **immortal**: `lifespan: None`, skipped every tick by the old-age reaper. Money
emergence, capital formation, the wage and share institutions, the whole voluntary-labor arc: all of it
stood on producers who never died.

C3R is the pivot the program was built toward. Make the producers mortal and ask the keystone question:

> **Can a division of labor in *production* survive its practitioners dying?**

This is not a demographic question. It is Böhm-Bawerk's question mechanized. Roundabout production is
production *through time*; capital is only rational if its payback horizon can be carried across the gap
between the individuals who create it. A structure that dissolves with each individual is not an economy —
it is a sequence of unrelated efforts. C3R asks whether the structure has continuity of its own.

The answer, after eleven milestones, is **no** — and *how* it fails is the result.

---

## The arc

Each milestone closed the wall the previous one named, and each fix revealed that the wall had moved rather
than fallen. That pattern is the finding.

**C3R.a — the chain dies with its producers.** Introduce producer mortality and nothing else. Uniform
across five seeds: `ChainCollapsesOnProducerDeath`. What makes it a finding rather than an artifact is what
it *isn't*: no hidden immortal reservoir (`immortal_producer_count` = 0 everywhere), no thin-pool artifact,
and emphatically not apathy — agents re-adopt the vacated role **125–171 times per run**. Mortal builders
even complete the 16-cycle-payback mill 2–3 times. The chain churns and never stabilizes: a producer dies
before its capital pays back, its mill sinks to the commons where no one can use it, and the next mortal
starts the roundabout investment from scratch. Two or three builds against ~150 deaths is the payback
horizon biting exactly as predicted.

**C3R.b — inheritance buys structure and costs flow.** Close the capital-destruction channel: give the
producers households so a dead producer's mill routes to a living heir through the existing estate seam. In
one narrow window (`food_provision = 1`, `cap = 2`, 4 of 5 seeds) the chain does something it never had —
**both stages jointly staffed for ~1,500 of 1,600 ticks**, the keystone's first structural positive. And on
flow the verdict is unanimous: **`FlowCapped`**, bread ≈ 9. The stages are manned continuously and produce
almost nothing. The inversion is measured, not asserted, and it is intrinsic: inheritance sustains structure
only by keeping producer households populated, populated households reproduce (357 births vs 16 in the
inheritance-denied control), and that reproduction carries the hearth subsidy that floods bread demand. Deny
inheritance and the chain runs on constant rebuilding instead — real bread (≈ 1,869) but no continuous
staffing. **Having capital continuously is not the same as using it productively**, and the two ways to run
a mortal chain are in tension.

**C3R.c — income feeds the living but never funds reproduction.** Retire the producer mints; fund the
households from externally *earned* revenue. The preregistered classifier says `EarnedIncomeInsufficient`,
but the causal finding is stranger: **nobody starves.** The provisioning transfer fires **zero times in all
fifteen runs** — with a 16-gold birth gift against a price of 1, the funding gap is never positive. What
fails is **reproduction**. Births collapse from 357 to 1–5, because the birth gate debits four *saved* food
units and present-hunger buying feeds but never saves: agents buy a loaf when hungry and eat it, no one
accumulates the four-loaf child-rearing stock, no heirs exist. The prior-saving problem C1 located in the
wages-fund relocates to its most elemental form — **a mortal economy whose agents demand only against
present need cannot fund its own reproduction.**

**C3R.d — saving works as built, and the trap holds.** Build the missing behavior: a parent-facing
future-bread want, no new parameters. A sufficiency control settles the gate question emphatically —
possession of four loaves was the final missing birth condition *every time* (`failed_injected_births = 0`;
births rise ~2 → 702–730). But the *motive* emits 384–540 wants per run and only **3–7 purchases clear**.
The circle closes: the chain is dead for want of heirs, heirs absent for want of birth stock, the stock
unassembled for want of winnable bread, and bread unmade for want of a chain. **An individual saving
behavior alone does not escape a low-level equilibrium this deep.**

**C3R.e-obs — why the saving bid loses.** Instrument the allocation contest before intervening. Every failed
saving opportunity gets exactly one pinned cause. The split is **economic, three ways** (≈⅓ no bid posted,
≈⅓ outbid by hungry buyers, ⅕–⅖ no purchasable offer) with the family built to *disconfirm* that reading —
arrival-order microstructure — explaining only **2–7%**. Correlated economic bottlenecks; microstructure
minor. So the next lever should bypass all three at once rather than bet on one margin.

**C3R.e — the escape experiment, and the horizon that closes it.** Three finite interventions against a
six-criterion hysteresis bar. Neither escape nor relapse: the ladder returns **undetermined**, and that
refusal is the finding. The redistribution failed its dose precondition on every seed because **every
original producer founder is dead by tick 36**; and in every eligible window on either regime, **external
demand was already dead**. The keystone closes not with an answer to the hysteresis question but with the
demonstration that *the question cannot yet be posed in this economy* — the demand base dies before any
escape can be evaluated.

*(Interlude — the demand-horizon frontier, DH.a/DH.b: structure dies before the closure question can be
posed; the reproductive burden persists at one loaf, not only four; and the wall is income, not atomicity.)*

**C3R.f–h — the turn: a real bug, and the immortal chain works.** The arc then found something it had been
walking past. Role-choice valued a recipe's *input* at a **frozen last-trade price** — `realized_price` has
no recency gate, so margins computed from it can be phantom. Fixed to read a live minimum non-self holder
reservation (declining explicitly when no holder exists, never `None → 0`), and the **immortal chain
functions**. In the same stroke, lifespan was proven *not* to be the lever: cross-stage flow stayed at zero
across a 16× lifespan sweep. So the mortal wall was never mortality-as-such — it was something the live
price now let us see.

**C3R.i — the flour is *there*.** A census, run *before* building the obvious fix, measuring physical state
rather than the reason code. At the first post-death bake decline, **flour is abundantly present** — two
millers and a consumer heir holding 33 units each — while **not one holder posts a computable ask**. The
wall is a phantom *seller*, the exact sibling of the phantom *price* C3R.h had just removed. The specced
stock-adding intervention was closed **INTERVENTION-INVALID before it was built**: it would have engaged
nothing.

**C3R.j — the wall is actor-independent money satiation.** Decompose the absent ask. Every holder's gold
already provisions **every money want on its entire value scale**, so the ordinal ask rule yields no
reservation price and no ask is posted. The sharpest form: the sale drops *no allocation* — the flour is
**costless to part with** — and it still goes unpriced. And it blocks surviving founders and inheriting
heirs *alike*: not an heir-identity failure, an actor-independent one.

**C3R.k — the lever crosses the wall, and the re-ignition does not persist.** The one pre-registered lever
the arc's cap reserved: post a marginal `Price(1)` ask on the costless surplus, flour-scoped, activated at
the measured wall tick, with a byte-identical pre-activation prefix and order-sequence attribution. It
**works, causally**: gate-only asks post, fill, and are bought by oven heirs on their own Baker-adoption
tick — 116–293 flour consumed as input versus **0** in the paired control. And it is **insufficient**: all
five seeds end `DOWNSTREAM_NULL`, no bread in the final window, no living bakers at the horizon. The probed
mechanism is old-age adopters on a late succession clock dying before a chain can re-establish, while
`food_provision = 0` has stopped producer-house **reproduction** — the birth-endowment gate needs a member
holding four staple. Both cheap subsistence remedies fail *in opposite directions*: restoring the hearth
restores births but drives baked flour to zero (the mint destroys the bake margin), and a conserved advance
is inert because a collapsed chain has no fund to advance from.

---

## The answer

**Production does not durably survive the producer on this base** — and the arc's shape is the substance of
the answer. Every wall it broke revealed the same deeper one:

| fix | wall it revealed |
|---|---|
| mortality introduced (a) | capital sinks to the commons; payback horizon unbridgeable |
| inheritance (b) | structure persists, flow capped — the subsidy that supplies heirs floods demand |
| earned income (c) | nobody starves; **reproduction** is what fails |
| saving behavior (d) | bootstrap/allocation trap — no uncommitted surplus reaches saving |
| allocation instrumented (e-obs) | correlated economic bottlenecks, not microstructure |
| escape attempted (e) | demand dies before the question can be posed |
| live input price (f–h) | the immortal chain works; the mortal wall is elsewhere |
| census (i) / decomposition (j) | a genuine, actor-independent **pricing** wall — flour present, unpriced |
| the pricing wall crossed (k) | **reproduction financing, again** |

The pricing detour (f–k) is not a digression — it is the arc's control. It found a *real* causal wall,
built the minimal faithful correction, demonstrated the correction *works*, and landed back on the same
terminal constraint the earlier milestones had already named three times over. That convergence is what
licenses the conclusion:

> **You cannot finance producer reproduction and re-entry from inside a chain that requires those producers
> to already exist.** The prior-saving problem — a wage is an advance out of a fund saved beforehand — holds
> at the *reproductive* level: the fund that would carry a producer lineage across the generational gap must
> come from outside the chain, and this economy has no outside.

That is the keystone's answer, and it is a negative one with a demonstrated mechanism rather than an
unexplained failure.

---

## Honest scope

What the arc **establishes**: the collapse under producer mortality (five seeds, instrumented against the
obvious artifacts); that inheritance buys structure at the cost of flow, with the trade-off's mechanism
measured; that reproduction, not consumption, is the binding demographic failure; that the birth gate is
locally sufficient given stock, and that an individual saving motive does not assemble that stock; that the
allocation failure is economic rather than microstructural; that a stale input price was suppressing the
*immortal* chain, and that lifespan is not the mortal lever; that the mortal wall is an actor-independent
money-satiation refusal to price a costless surplus; and that a marginal money-demand correction *causally*
crosses that wall and is *insufficient*.

The C3R.k collapse mechanism is now **partly test-backed**: the committed harness durably asserts, on all
five seeds, that starvation death never occurs, that removal is old age, and that every gate-fired
heir-adopter exits by *death, not de-adoption* — so "the re-ignition dies out rather than re-satiating or
starving" rests on a test. What remains **probe-indicated**, not durable, is the *reproduction* half of the
mechanism (that `food_provision = 0` dries the heir stream via the birth-endowment gate, and that both
cheap subsistence remedies fail).

What the arc does **not** establish: that these results generalize beyond the tested bases and seeds; that
the reproduction-financing half of C3R.k's mechanism is proven rather than probe-indicated; that
flour-scoping the ask is *safe* rather than merely necessary for attribution; or that any of the untried
institutions — a granary, a priority set-aside, a binding forward order, endogenous reproduction finance —
would fail. Several milestones close with named,
undischarged verification debt, disclosed in their sections.

---

## How the arc was run, and what that cost

The method is as much the result as the findings. Every milestone was pre-registered — outcome buckets and
their evidence named *before* the run — so that a null could be pinned rather than explained away, and so
that an intervention that never engaged would classify as `INTERVENTION-INVALID` instead of as a failed
economy. Diagnostics were built **before** interventions: C3R.i's census closed a specced intervention
before a line of it was written, and C3R.j's decomposition redirected the lever entirely.

The dominant hazard was not bugs. It was **over-reading proxies** — a run of **fourteen** near-misses in
which an aggregate, a snapshot, or a plausible mechanism was mistaken for ground truth: realized prices read
as live, reconstructed order-book *intent* read as executable liquidity, global trades read as one class's
sales, an `InputPriceAbsent` count read as "nobody holds flour," a single seed generalized to all, and — the
last and most instructive — a *probe's attribution* accepted without checking it against the configuration
that made it impossible. That final one reached a committed draft: C3R.k's collapse was written up as
producer *starvation* before a review caught that starvation death is disabled on this base and producers
are fed to a floor. The rule the arc converged on, and the reason the ledger is in the record at all:

> Aggregate and snapshot data tell you **where** to look. Only decision-path code and trade-level evidence
> tell you **why**. And the interpretation deserves the same suspicion as the data — audit the *conclusion*
> against what the measurement licenses, not only the measurement against the code.

---

## What this opens

The arc closes here, at its cap, with a tested answer. It leaves one genuinely new question — not a lever to
try next, but a research programme of its own:

> **Endogenous financing of producer reproduction across re-entry** — can a chain accumulate, from within
> and conserved, the fund that carries a producer lineage through the gap where the chain is de-staffed?

Everything C3R measured says that fund is the binding object and that neither a subsidy (which destroys the
margin), nor a transfer (which has no source in a collapsed chain), nor an individual saving motive (which
loses the allocation contest) supplies it. That is a different arc, and it deserves its own pre-registered
cap rather than an extension of this one.
