# OIKOS — From-Scratch Review and Replan (July 2026)

> Status: review + replan, supersedes the **build ordering** (§5) of `spec-civ-core-roadmap.md` — not its
> praxeology contract (§4), not its layer taxonomy, and not the game spec. Produced deliberately with
> fresh eyes after the C1 null: three independent inputs — a newcomer's critical review with no stake in
> the existing plans, a code-level fact-check of the structural hypothesis (verify-or-refute, not
> assume), and an unsteered clean-slate strategic review by the independent second model — plus this
> synthesis. All three converged on the same diagnosis and substantially the same plan.

---

## 1. What OIKOS actually is today (honest inventory)

**Proven, merged, load-bearing** (byte-identical goldens, adversarially reviewed):

- **The money result** — the crown jewel. Mengerian commodity money emerges non-circularly only when four
  separable conditions align (direct-use floor, medium-saleability *leadership*, tradeable pre-money
  surplus, a round-tripping exchange institution), each isolated by a matched failure (S8–S20, S21a–f);
  under open survival it needs a fifth (a demand-side survival floor), and the coexistence is honest-ly
  **band-qualified** (S21i: load-bearing on WOOD scarcity and anchor density).
- **The foundational mechanisms** — endogenous specialization with market-sourced inputs (S5/S6),
  producible capital (S7), per-agent ordinal originary interest (S10), entrepreneurial profit/loss
  selection (S11), and a genuine two-check Malthusian band (S14/S17).
- **The institutional negatives, which are the map, not noise** — occupation forms only under an
  exit-overriding voluntary contract, and then only as a minority core (S22a–f); property institutions
  in both tested forms fail ahead of their preconditions (S23a thrashes, S23b goes thin); local
  welfare-imitation cannot *select* a division-of-labor institution because its value is non-local
  (S24a–c); tax receivability closes the chartalist pull-leg (M20/M21).

**Branch-preserved nulls (unmerged)** — S23c (inheritance vacuous), S23d (demography fixed,
subsidy-bound), S23e (scarcity doesn't force a market), **C1 (no voluntary money-wage clears at all)**.

**Spec-only, zero code** — C2–C10 (ten reviewed impl specs), the entire game (Ga–Gg).

### 1.1 The overstatement to retract

The civ-core roadmap claimed OIKOS "has already built the microfoundations of a market economy" and that
the remaining gap is "compose and complete." That was wrong in the way that matters most:

- **Nothing has ever been composed.** Every mechanism was validated in isolation behind a default-off
  flag on a curated base. There is no run where money emergence + the production chain + demography +
  property + a buyer-income loop run together and sustain. The one time several strands were stacked
  (C1's base), the result was a null.
- **No mortal/open base has ever produced a sustained producer/buyer market.** S23d, S23e, and C1 are
  three consecutive nulls naming the same joint. The thing a civilization *is* — sustained exchange among
  mortal earners — has not yet emerged anywhere in the codebase.

The microfoundations are real as isolated mechanisms. Composition is not a sequencing detail; it is the
unsolved problem.

---

## 2. The meta-finding: institutions are vacuous ahead of their preconditions

Read across the whole null corpus, one shape repeats:

| Null | Institution supplied | Missing precondition |
|---|---|---|
| S23c | Heritable secure title | Owners who die and reproduce (disjoint populations) |
| S23b | Land market | Buyers with money (owner-collapsed base) |
| S23d | Owner demography | Any reason to trade (unlimited self-provision) |
| S23e | Scarcity of the outside option | Buyer income |
| C1 | Money-wage labor | An accumulated wages-fund (prior savings) |
| S24a–c | Institution selection by imitation | A local observable of non-local value |

And the successes have the complementary shape: money emerged only after *both sides* of monetary
exchange could exist (produced supply + surviving demand); the chain sustained only after working
capital, cold-start, and recurring motive were each in place.

**The finding:** OIKOS fails whenever a downstream institution is asked to bootstrap before its
complementary side can survive, pay, or persist — and succeeds when preconditions are built first. This
is itself a praxeological result (institutions have genetic preconditions; Menger's organic institutions
emerge in an order), and it indicts the C1→C10 ordering at the root: that roadmap was **teleological**
(working backwards from the modern circular flow — "wage labor is Layer 0") where it should have been
**genetic** (working forwards — what can emerge *next* from what exists *now*). C1's null is the
doctrine's own prediction: a wage is an advance out of prior saving (Böhm-Bawerk/Strigl); no accumulated
fund, no money-wage market. The keystone was empty *by Austrian theory*, and the plan's DAG even
contained the circularity (C1 needs the wages-fund; the fund accumulates in C2; C2 depended on C1).

---

## 3. The verified structural diagnosis: the immortal-producer wall

A code-level fact-check (verify-or-refute) confirmed the deepest structural fact of the codebase:

- **Every chain producer is constructed immortal and non-lineage.** Miller/Baker/Scholar come from a
  roster loop that unconditionally sets `household: None`, `lifespan: None`; old-age death requires
  `lifespan: Some`; every chain config disables starvation (`hunger_critical = need_max + 1`). No shipped
  config has ever killed a chain specialist.
- **Every mortal lineage member is hardcoded `Consumer`** (founders and newborns alike), forever outside
  the chain. The class boundary is stable by construction.
- **Capital accumulates only where death cannot reach.** Frontier lineage lifespans average ~27 econ
  ticks; a single capital payback is 16 cycles plus the saving horizon — a mortal agent cannot span it,
  so the immortal roster does all accumulating, by arithmetic necessity.
- **Succession does not exist.** Mill/oven inheritance is explicitly deferred ("needs producer mortality
  to matter"); no heir ever assumes a producer role; only the plow (subsistence layer) has an
  inheritance path. The one unification attempt (S23d) was on the cultivation axis — its base has *zero*
  millers/bakers — and was subsidy-bound.

So OIKOS today is two co-resident but **disjoint populations**: an immortal caste that produces and
accumulates, and a mortal caste that consumes and reproduces. Demography and economy co-occur in the
frontier configs, but never in the same agents. **A civilization is precisely what happens when
production must survive the producer** — when capital, roles, and knowledge have to cross a generation.
That crossing has never been attempted for the real economy, and it is the project's true long-horizon
keystone.

---

## 4. What survives unchanged

- **The method** — conservation every tick, byte-identical goldens, ON-only injective digest tags,
  ordinal-only decisions, classify-not-tune, honest nulls, adversarial two-model review. Untouched.
- **The praxeology contract** (`spec-civ-core-roadmap.md` §4) — untouched; it is the operational meaning
  of "all rules consistent with praxeology."
- **The C2–C10 specs** — demoted from build order to **reference taxonomy**. Each is re-derived from the
  landed findings when its preconditions actually exist (the discipline already written into their
  "Dependency & premise" sections — now exercised).
- **The C1 machinery** — the branch's ordinal asks/appraisals, matching, conserved escrow, provenance
  ledger, and controls are directly reusable by the next levers.
- **The game vision** — single settlement first, player-as-institution-supplier, the state as *optional
  content the player can try, never a requirement* (a stateless colony is a fully valid playthrough).
  Game *infrastructure* (command log, replay digests, inspectors) can proceed early; the game *economy*
  waits for a living base.

---

## 5. The replan (genetic order — each milestone classify-not-tune, each re-specced from findings)

**P1 — `C1R`: voluntary output-share tenancy (sharecropping).** *The next build.* A landless worker works
an owner's plot for a **share of the realized output** — no advance of any kind, so it needs neither a
wages-fund nor even money, and it directly attacks the exact gap C1 measured. Historically and
theoretically this is the labor institution of thin-capital economies. Base: the same S23e marginal
rival-commons mortal-landowner base (unchanged, so the only new variable is the contract). Bar: across
`{3,7,11,19,23}`, voluntary share contracts clear and renew; worker survival is materially
contract-funded; owner surplus stays positive; a `no_contract` control reproduces the S23e null; a
`forced_share` control separates as scaffold.

**P2 — `C1N`: natural (in-kind) wages.** The owner pays the worker in **bread** from its own produced
surplus. Tests whether C1's gap was purely a *money* gap (a hungry worker paid in bread has ~zero
reservation ask) — and whether a real-goods wages-fund suffices where the money fund did not. Cheap on
top of C1's machinery; may be built as P1's sibling variant if the build shapes align. An honest null
here (no present bread fund → no hires) is itself the wages-fund doctrine confirmed in kind.

**P3 — `C1A`: the accumulation horizon + money-wage re-entry.** On whichever of P1/P2 lives: can a real
wages-fund now form endogenously (accumulation across a lifetime, lifespan swept as an axis, not tuned)?
Then the Austrian sequencing test proper: money wages stay vacuous *before* reserves accumulate and
clear voluntarily *after* — the C1 re-run with its precondition finally present.

**P4 — `C3R`: the mortal economy (the true keystone).** Extend mortality to the *real* economy: chain
producers who age and die, mill/oven inheritance, and role succession (the long-deferred S7 follow-on).
Falsifiable bar: production survives the death of producers.

> **STATUS (2026-07-09) — decomposed into a 3-slice sub-sequence; slice 1 landed.** The original framing
> (compose "the winning voluntary income mechanism" into a generational base) is superseded: the
> voluntary-labor arc CLOSED without a standing-institution winner (§26 — the binding constraint is the
> worker's satiation exit), so C3R does not compose it. Instead C3R isolates producer mortality on the
> existing `frontier_capital` base, decomposed (per two research maps + 4 xhigh spec-review rounds) into:
> **C3R.a** producer mortality, no succession (the motivating null) → **C3R.b** role succession → **C3R.c**
> capital (mill/oven) inheritance.
> - **C3R.a — LANDED (impl-62, spec `docs/impl-mortal-producers.md`).** Verdict `ChainCollapsesOnProducerDeath`
>   ×5: the chain dies with its producers. Instrumented honest (reservoir provably closed
>   `immortal_producer_count=0`; not a thin-pool artifact; ~150 re-adoptions/run + 2–3 mortal builds/run — the
>   chain churns but never stabilizes; the payback horizon bites). Flag `mortal_chain_producers`, tag 27.
>   rb-lite clean in 6 rounds; RoR ACCEPT-AS-HONEST-FINDING no P0/P1. Report §27 + appendix. Preserved on
>   **`feat/mortal-producers-impl-rb`** (tip `d8e0ddc`).
> - **C3R.b / C3R.c — NEXT.** C3R.a names them precisely: the role is refilled only by frantic ad-hoc
>   re-adoption (→ role succession), and capital sinks to the commons on every death (→ capital inheritance,
>   the Böhm-Bawerk payback-across-generations question). Scoping decision (b-first vs c-first, or bundled)
>   pending — C3R.a's total collapse is itself an input to whether role succession alone can lift it.

**P5+ — re-derive the institutional stack on the living base.** Firms (C2R) and rent/factor markets
(C4R) once solvent counterparties exist; the classes/mobility measurement (C9) over whatever forms;
land-market re-runs. **State (C5) and credit (C7) enter as optional interventionist comparisons** — what
does a fiscal injection or a credit expansion *do* to a living voluntary base — and as player content,
never as the path to the base. C6 (tech ladder) is substrate-independent and may interleave; C8 (space)
follows the living base.

**Parallel engineering track (the price of admission for composition):**
- **Decompose `settlement.rs`** (~28k lines; C1 alone added ~3.6k). Behavior-preserving module split
  with the goldens as the conformance net, staged so every step is byte-identical. Composing further
  layers into the monolith is where correctness will otherwise die.
- **Game infrastructure early** (per the game spec's target property): the canonical command-log
  surface, replay-digest tests, and the first inspectors — none of it blocked on the economy.
- **Docs hygiene:** temper the README's "SUCCESS" framing toward the report's honesty; this document
  supersedes the roadmap's §5 sequencing (pointer added there).

---

## 6. Why this order should be trusted more than the last one

The C1→C10 ordering was authored top-down from theory-elegance and reviewed for *internal* soundness —
and its keystone was falsified by the first build. This ordering is derived bottom-up from the landed
null corpus (each next step is the cheapest untested lever whose preconditions demonstrably exist), it
was reached independently by three fresh-eyes reviews, and its first milestone (P1) is deliberately the
*smallest* possible delta on a verified base: one new voluntary contract form, no advance, no new
scarcity knob, no state, no credit. If P1 fails too, the finding is that the problem is deeper than the
wages-fund gap — and that would itself re-derive P2–P4, exactly as the discipline demands.
