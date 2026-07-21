# impl-74 — C3R.i: Post-death flour re-ignition (can the flour market re-price a de-staffed chain, so production survives the producer?)

Status (spec): **v1 — DRAFT** (pending Codex+Fable dual review). Successor to impl-73 (C3R.h). Origin:
the impl-71 (C3R.f) redirect (`docs/impl-producer-lifespan-ratio.md` §−2) — a dual review proved
lifespan is *not* the lever; the mortal chain dies via a flour-market **re-ignition deadlock**. This
milestone attacks that deadlock directly. **Hard cap: ONE milestone.** If neither lever clears the
five-seed gate, pin the null and STOP the C3R wall-chasing — do not chase a ninth "obvious lever".

## 0. One-paragraph summary

With the stale-input-price fix (`stale_input_price_fix`, impl-73 cut 1) the *immortal* chain
functions and stays solvent (`EITHER_SUFFICES`). The *mortal* chain still collapses — and lifespan
does not fix it (flow = 0 across a 16× lifespan range). The cause is localized: after the founder
bakers die, the chain enters an **absorbing state** — no baker ⇒ millers under working-capital
discipline (`project_input_bids`) stop producing flour ⇒ no living agent holds flour ⇒ the fix's own
no-holder-decline (`fresh_input_ask`, `mod.rs:10103`; decline at `phases.rs:2316`) rejects every
heir's bake appraisal as `InputPriceAbsent` (83–93% of rejections). It is the *sibling* of cut 1's
stale-price wall: cut 1 fixed a phantom *presence* (a frozen price), this fixes a phantom *absence*
(no price because no holder). The question: can the flour market **re-ignite** after a producer
die-off, so the baker role re-adopts and production survives across generations?

## 1. Base facts (verified 2026-07-21)

- **Endpoints pinned.** Immortal + L2 sustains 5/5 (impl-73 cut 2). Mortal + L2 + full succession
  (`mortal_chain_producers` + `mortal_producer_inheritance` + `mortal_producer_tool_inheritance`)
  collapses 5/5 at default life: 0 living bakers, 0 final-window bread, despite 173–218 deaths and
  165–202 heir-adoptions/run.
- **Mechanism, code-verified.** `fresh_input_ask` (`mod.rs:10103`) declines when no living non-self
  agent *holds* the input; millers stop producing flour with no baker buyer (`project_input_bids`,
  `scenarios.rs:272`); succession is not the problem (estate + tools transfer before same-tick role
  choice, `mod.rs:7183`, `demography.rs:366`; heirless deaths 3–5). The rejection is
  `InputPriceAbsent` 83–93%, `margin_nonpositive = 0`.

## 2. The central question and pre-named outcomes

**Q: Can a genuinely-produced flour supply (and/or speculative appraisal) let the flour market
re-price a de-staffed chain, so the baker role re-adopts and production sustains across real producer
deaths?** Pre-named, per-seed, exclusive:

- **REIGNITION-SUFFICES** — a lever makes the mortal chain staff, produce, and stay lineage-solvent
  across ≥ N measured death→inherit→adopt→**bake** joins on all five seeds; flour trading resumes
  after each die-off gap. The project's first sustained *mortal* producer chain.
- **REIGNITES-BUT-DEEPER** — the flour market re-ignites (heirs re-adopt and bake) but the chain
  still fails to sustain for a *different* reason (heir liquidity, demand thinness) — the deadlock is
  eliminated as the wall and the next one is named. A real localization.
- **DEADLOCK-PERSISTS** — neither lever re-ignites the market at any tested setting: pin the residual
  rejection histogram and **STOP** (the deadlock is a deeper wall; C3R wall-chasing ends here).

## 3. Levers (economics, not patches; default-off; tested one-at-a-time + combined)

**R2 — bounded speculative miller flour inventory (LEAD; both reviews endorse).** Let a miller under
`project_input_bids` produce and hold a **small, bounded** flour buffer even without a live baker
bid — entrepreneurial inventory speculation. It must **consume real grain + working capital**, be
**voluntarily saleable**, and be **bounded** (a cap ≪ a mint) so it cannot smuggle a subsidy or a
forced trade. Then flour *exists* for an heir to buy and appraise against, and the market re-ignites
economically. Default-off `ChainConfig` flag, ON-digested, conservation-safe (produced, never
minted).

**R1 — speculative appraisal against a producible-input quote (SECONDARY; contested — handle with
care).** Extend `fresh_input_ask` so a would-be baker holding an inherited oven appraises flour at a
*miller's reservation ask* even when no agent currently *holds* flour (Misesian appraisal of a
future price). **Codex's caution is load-bearing:** this risks a "free appraisal" that smuggles the
coordination the experiment tests — a baker adopting against flour it *cannot actually buy*. So R1 is
judged **only on realized bake execution** (did the adopted baker acquire flour and bake?), never on
appraisal acceptance; if R1 only raises appraisal-accepts without raising realized bakes, it is
manufacturing unbuyable adoption and is a null, not a fix.

## 4. Metrics — the §−2 confound fixes (do NOT reuse the churn-unstable / proxy signals)

- **Instrument the real join**, not proxy counters: per producer death, record heir selection → oven
  transfer → same-heir Baker adoption → *retention* → *subsequent Bake execution* (the burden event
  shapes at `burden.rs:83` exist for this). Assert ≥ N *completed* joins, not `heir_tool_adoptions`.
- **Lineage liquidity, not current-vocation class gold.** Cut 2's `baker_class_gold` reads 0 during
  staffing gaps and loses estate gold on not-yet-promoted heirs. Use fixed *producer-house lineage*
  liquidity (sum over living members of the baker producer houses) as a per-tick minimum over a
  window.
- **A real no-death control** (per both reviews): keep **all** mortal/inheritance/tool/tagging + L2
  plumbing on and give producer houses a checked lifespan **beyond the horizon** — the confounded
  "immortal control" admits an adopter pool the mortal base lacks (`phases.rs:2220`) and changes
  tagging (`generation.rs:560`), so it measures pool restriction, not `life = ∞`.
- **Re-ignition latency:** ticks from a die-off gap to the next resumed flour trade + Bake execution
  — the direct signal the deadlock broke.

## 5. Acceptance suite (new, `sim/tests/flour_reignition.rs`)

Mortal base (`stale_input_price_fix = true` + all succession flags, `food_provision = 0`, cap held),
`SEEDS = [3,7,11,19,23]`, one common horizon. Arms: base / R2 / R1 / R2+R1, per seed, plus the real
no-death control. A "sustains" arm shows, on all five seeds: both stages staffed to the final window,
attributed production sustained, producer-house lineage liquidity positive over the window, ≥ N
completed death→…→bake joins, and flour trading resumed after each gap — asserted, with conservation
/ digest / no-immortal-reservoir guards. Classify the §2 outcome per seed; suite label only when all
five agree.

## 6. The one-milestone cap (both reviews, load-bearing)

If neither R2 nor R1 (nor combined) clears the five-seed gate, **pin the residual histogram and
STOP.** The C3R keystone then closes as: the mortal production chain fails at a flour re-ignition
deadlock that entrepreneurial inventory/appraisal does not cross — an honest, localized negative,
and the end of the wall-chase. Do NOT open a ninth lever.

## 7. Conservation & determinism

R2's buffer is **produced** (grain + labor consumed, booked; no mint); R1 changes only the appraisal
input source (serialized-state-derived, as cut 1). Both flags are behavior-steering → **DIGESTED
ON-only** (off byte-identical, coverage-guard classified). The join/liquidity/latency telemetry is
**non-steering, non-digested** (impl-72 pattern). Conservation and the money identity asserted per
tick.

## 8. Falsifiable-bar summary

**Pass (either sign):** an asserting suite pins one §2 outcome per seed on the mortal base with the
real no-death control separating, on the *real* join + lineage-liquidity metrics (not the proxy
counters), with R1 gated on realized bakes not appraisal-accepts. **Fail:** a curated buffer that
smuggles coordination (unowned/forced/mint), R1 credited on appraisal acceptance, reuse of the
churn-unstable class-gold or `StructurePersists`/`FlowRuns` proxies, or opening a second milestone to
chase the wall further after a clean 5/5 null.
