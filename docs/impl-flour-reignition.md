# impl-74 — C3R.i: Post-death flour re-ignition (can the flour market re-price a de-staffed chain, so production survives the producer?)

Status (spec): **CENSUS BUILT — result `HolderWithoutAsk`; R2-as-specced closed (INTERVENTION-INVALID) before it was built.** The `flour_reignition_census` diagnostic landed (`sim/tests/flour_reignition_census.rs`, non-steering, byte-identical) and was hardened under a second adversarial Codex+Fable review. R2 is NOT built. See `## −0.9` for the result. Milestone is at its census-cut decision point; the next seam (decompose the holders' `None`) is scoped but unbuilt under the one-milestone cap.
Successor to impl-73 (C3R.h). Origin:
the impl-71 (C3R.f) redirect (`docs/impl-producer-lifespan-ratio.md` §−2) — a dual review proved
lifespan is *not* the lever; the mortal chain dies via a flour-market **re-ignition deadlock**. This
milestone attacks that deadlock directly. **Hard cap: ONE milestone.** If neither lever clears the
five-seed gate, pin the null and STOP the C3R wall-chasing — do not chase a ninth "obvious lever".

## −0.9. CENSUS RESULT — `HolderWithoutAsk` (built, dual-review-hardened; AUTHORITATIVE outcome)

The census specced in `## −0.5` was built and independently verified (full suite green, no golden
moved, `canonical_bytes_excludes_flour_census` proves it non-steering), then adversarially re-reviewed
by Codex + Fable over the landed diff. **Result at the first post-death Bake `InputPriceAbsent`
decline (seed 3, society tick 23, after 4 producer old-age deaths): `HolderWithoutAsk`.**

- Measured, not inferred: two Millers hold **33 flour each** and a Consumer **heir** holds a further
  **33** (>100 units present), gold 76–176, grain 0 — yet **every** living non-self holder's
  `reservation_ask_for_money(flour)` is `None`. The wall is a **phantom seller**, not phantom stock —
  the sibling of C3R.h's phantom price.
- Mechanism (faithful, not a bug): the holders are money-satiated — with 33–36 flour, removing one
  unit drops no allocation (`lost_rank = scale.len()`), and their gold already covers every money-want
  (`econ/src/agent.rs:476`, `950–975`), so the ordinal rule yields **no reservation price**. The heir's
  pre-adoption appraisal reads exactly that reservation (`fresh_input_ask`) and declines.

**What this settles (LICENSED):** **R2-as-specced is `INTERVENTION-INVALID`** — its dose is gated to
holders at/below one output batch and its offer leg needs a computable ask; the measured holders sit
far above the gate with no ask, so it engages nothing. This is exactly the outcome `## −0.5`
pre-registered; the census pre-empted a ninth over-read (building R2 against a non-stock-absence wall).

**What this does NOT license (the tenth over-read, caught by the adversarial review — do NOT write
these):** that stock-adding *in general* is inert (flour to a poor unmet-want colonist would likely
produce a real ask); that the chain is *deadlocked* or the wall *persists* (single tick, single seed);
that the refusal is a *defect* vs faithful satiation; or that the candidate is an inheriting *heir* vs
a surviving founder (the schema records neither vocation-of-candidate nor oven provenance — the `## −0.5`
schema deliberately subset the None-decomposition, which is fatal to *lever selection*, not to this
result). The honest claim is the narrow one: **abundant flour, no seller-ask, at this appraisal.**

**Next seam (scoped, UNBUILT under the one-milestone cap):** decompose *why* the holders have no
reservation price (expected: money-want satiation at `agent.rs:975`) with free-stock + posted-ask +
miller-restock-predicate fields, measure persistence and heir-identity across seeds, then choose among
(a) applying the C3R.h live-price fix **symmetrically** to `project_input_bids` (`mod.rs:8556` still
reads the frozen `realized_price(output)`), (b) a seller-side motive change, or (c) genuinely
speculative adoption. `## −0` items 3–9 (the R2 offer, the join telemetry, the no-death control, the
outcome tree) remain the design for that successor milestone, not this one.

## −0. v2 revision (AUTHORITATIVE — folds the Codex+Fable dual review; supersedes §§0–8 on conflict)

Both reviews verified against the code; the milestone is real but reshapes to **census-first**.

1. **[P0 — census before R2] The restock gate already grants one full output batch of slack.** A
   zero-flour miller passes the unsold-output guard because restocking stops only at `stock >=
   output_qty` = 3 flour (`mod.rs:8569–8581`, `content.rs:79`). So a one-batch R2 dose is
   *behaviorally inert* (byte-different, same behavior). **Do the branch census FIRST:** at the
   zero-holder tick, a non-steering microtrace — living Miller? output price? gold? flour/grain
   stock? imputed reservation? grain ask posted/filled? Mill execution? — to establish *why* no flour
   exists (miller absent / can't fund the grain bid / produces-but-hoarded / one batch insufficient)
   before naming R2's hook or cap. An R2 that never engages is **INTERVENTION-INVALID**, not
   DEADLOCK-PERSISTS.
2. **[P0 — CUT R1] Not computable.** `reservation_ask_for_money` requires held stock
   (`agent.rs:449`); a no-flour miller has no reservation ask, so R1 would *synthesize* the exact
   non-executable price the no-holder decline exists to prevent (`phases.rs:2302`). A later Bake
   guards the outcome but does not validate an invented quote at adoption. **Drop R1 and R2+R1.** A
   binding *forward order* (funded buyer, seller commitment, quantity, settlement) is a legitimate but
   *separate* untested mechanism — the one-milestone cap excludes it as governance, so the null must
   read "preregistered R2 failed," NOT "economic re-ignition cannot cross the wall."
3. **[P0 — R2 = production + a reservation-respecting OFFER, reconciling the two reviews]** Fable
   verified the auto-offer path exists (`ensure_ask` posts for any holder of unreserved stock,
   `society.rs:961,3292`); Codex verified it *may not actuate* (production is post-market so flour is
   offered next tick; the posted ask needs free stock + `reservation_ask_for_money` success and is
   belief-*shaded above* the raw reservation `fresh_input_ask` reads, `mod.rs:10059`). So R2 must
   **prove the normal ask path actuates** (the census shows it) or specify a **seller-authored,
   reservation-respecting** bounded offer — never a forced below-reservation ask or forced fill. The
   required manipulation trace: `paid grain fill → Mill → bounded holder stock → ordinary seller
   reservation → posted ask → same-heir paid flour fill → Bake`. **Conservation:** an ordinary project
   bid using the miller's OWN gold + a willing grain seller + unchanged `run_production` (consume 1
   grain + labor, produce 3 flour, `mod.rs:8440`, `phases.rs:795`); **never credit flour directly**;
   gold reserved/transferred, not consumed. **Cap (pending the census):** the smallest non-tunable
   relaxation — `held + output_qty <= 2 × output_qty` (6 flour here), count reserved stock, replenish
   only after sale. Plausible-but-unverified until the census proves the threshold is even the binding
   branch (it may only *prevent* the gap, not *re-ignite* — if R2 keeps flour always present the gap
   never forms; call that PREVENTION, not re-ignition, or activate R2 only after a verified deadlock).
4. **[P0 — the join telemetry records NOTHING on this base]** `BurdenToolInherited/RoleAdopted/
   StageExecution` emit only under `closure_active()` = `closed_circulation && !disabled`
   (`closure.rs:1069`), which the mortal base never sets; the existing join also accepts same-tick
   execution and has no exit event (`burden.rs:694`). **Add non-steering, closure-INDEPENDENT
   telemetry** built from the `Trade` tape (tick/good/buyer/seller/price/qty, `market.rs:29`), cohort
   = Baker/oven-owner deaths: `death → oven inheritance → same-heir adoption → same-heir PAID flour
   Trade → no role exit → Bake`. (Per-heir purchase is measurable because `Trade` carries buyer.)
5. **[P1 — no-death control is expressible TODAY without landing the impl-71 knob]** A GLOBAL all-house
   no-death (`old_age_onset_years = 1`, `lifespan_span_years = 0`, `ticks_per_year > horizon`)
   genuinely differs from the confounded immortal control because producer lifespans stay `Some` and
   mortal eligibility/tagging remain active (`phases.rs:2220`). (Producer-*only* isolation would need
   the per-house `lifespan_scale_bps` override at all three sites — deferred; not required for the
   control.)
6. **[P1 — lineage liquidity, buildable]** Freeze Baker-house IDs at generation (`generation.rs:560`),
   sample each house every econ tick via `lineage_stats` (`mod.rs:13348`), score extinct houses zero,
   report **per-house minima** over a window (an aggregate hides an extinct lineage).
7. **[P0 — outcome tree, disjoint per arm×seed]** `REIGNITION-SUFFICES` (every qualifying gap
   completes a paid flour Trade + Bake within pinned latency AND sustainability passes) /
   `REIGNITES-BUT-DEEPER` (every gap re-ignites but sustainability fails for a named non-deadlock
   reason) / `DEADLOCK-PERSISTS` (≥1 valid gap remains/recurs at the original no-holder wall) /
   `INTERVENTION-INVALID` (R2 inert — never engaged) / `INCONCLUSIVE` (control/guard failure or
   censoring); suite `MIXED` when seeds disagree. **Apply the hard one-milestone cap only after
   treatment validity is demonstrated** (R2 actually actuated).
8. **[P1/P2 — pin all placeholders before running]** exact base constructor (P2: §1 was pinned on
   C3R.b food=3, §5 says food=0 — reconfirm the histogram on the run base), horizon, final/liquidity
   windows, `N` completed joins, the qualifying-gap definition, the latency ceiling, the production
   floor, censoring, whether retention requires a strictly-later Bake, and the commons-held-flour
   residual counter (heirless death → `settle_estate_to_commons` makes flour invisible to
   `fresh_input_ask` forever — record it in the DEADLOCK-PERSISTS residual; it also names the
   estate/probate-liquidity lever the cap forecloses, keeping the STOP honest).
9. **[P2 — determinism]** R2's flag + numeric cap + any steering inventory/commitment state DIGESTED
   ON-only in `digest_coverage_chain_config`; the census/join/liquidity telemetry non-digested; prove
   default-off == explicit-off tick-by-tick, ON differs, tags don't alias, telemetry perturbation
   leaves `canonical_bytes` unchanged (`digest.rs:168,2034`, `baker_role_l2.rs:37`).

**Net:** census-first; R1 cut; R2 = real production + reservation-respecting offer with a census-set
cap; closure-independent paid-purchase join; global no-death control; per-house lineage-liquidity
minima; a 6-way disjoint outcome set with validity-gated STOP. §§0–8 below are the v1 rationale,
superseded by this section where they conflict.

## −0.5. v2.1 — second dual review converged: correct the census TRIGGER before R2 (AUTHORITATIVE over §−0 item 1)

A second Codex+Fable consult (both verified against the decision-path code, not the brief)
converged on ONE correction, and it is a real one. Fable → BUILD-CENSUS-FIRST-with-a-P1-schema-
amendment; Codex → REVISE-SPEC-FIRST. **Same action, different label:** amend the census so its
trigger and schema match the decision path, *then* build the census test; do NOT name R2 or its cap
until the census classifies the binding branch.

**The correction (P0). `InputPriceAbsent` proves "no computable non-self executable appraisal input,"
NOT "zero physical flour."** `fresh_input_ask` excludes the appraiser (`mod.rs:10112`) AND
`reservation_ask_for_money` returns `None` for a *holder* too — not only the genuine non-holder
(`agent.rs:449`), but also a provisioning break (`agent.rs:476`) and no money-want at/above
`lost_rank` (`agent.rs:486→975`). So the §−0 phrase **"at the zero-holder tick" is an unverified
converse** ("None ⇒ non-holder") and must be struck. This is the exact shape of over-reads #1–8:
a decision-path *count* (the 83–93% `InputPriceAbsent`, real) inflated into a *cause* ("nobody holds
flour," unverified). The existing suite only aggregates the count (`baker_role_l2.rs:201`); it never
joins the rejection to physical holder state.

**Census retrigger + inference-free schema (replaces §−0 item 1).** Trigger the row on the **first
post-death `Bake` appraisal that takes the `InputPriceAbsent` branch** (`phases.rs:2319`), seed 3,
stop at first failure. Each row must **classify without inference**: candidate ID + own flour held;
commons flour (`settle_estate_to_commons` is invisible to `fresh_input_ask` from the first heirless
death); every *other living* colonist's `(flour held, free stock, reservation-ask result + which
`None` branch of `agent.rs:449/476/975`, posted ask)`; and each miller's `(restock predicate,
output price, free gold, imputed reservation, grain-order outcome, executed Trade, actual Mill
execution)`. Then the outcome is read off, not inferred: genuine zero-holder / holder-without-
computable-ask / commons-locked / miller-side failure (absent, cashless, failed imputation, no
crossing grain seller, failed production).

**The asymmetric-fix lead both raised (candidate root cause; census must be able to name it).** The
miller's restock imputation still values its output at the **frozen** `realized_price(flour)`
(`mod.rs:8556`), and `continue`s (skips the grain bid) when that is stale/absent (`mod.rs:8566`,
`8590`). The L2 stale-price fix was applied to role choice (`fresh_input_ask`, live min-holder
reservation) but **NOT** to `project_input_bids`. If the census shows the binding branch is a
*holder-without-ask* or a *stale-price restock skip*, the wall is a **second instance of the
stale-price bug**, and the fix is "apply L2 symmetrically to `project_input_bids`" — a smaller,
different change than R2, which R2's inventory cap would entirely miss.

**Defer R2 and the cap (P1, downstream).** R2's `held + output_qty <= 2×output_qty` (6-flour) rule
changes behavior only at `held == 3` — the restock gate already permits restocking at flour
holdings 0–2 (`mod.rs:8569`) — so it cannot repair miller absence, no cash, failed imputation, no
crossing grain offer, or failed production. Name R2 and its cap (§−0 items 3 + −0.8) only AFTER the
census classifies the branch; the census may show R2 targets a non-binding state (→ over-read #9).

**Single next artifact.** A one-seed, stop-at-first-failure diagnostic test emitting the census row
directly at the decline branch (`phases.rs:2319`), non-steering / read-only / no digest / no new
flags (seams already exist: `BootstrapTrace` grain-bid counters `mod.rs:6802,8602`, `role_choice_diag`,
`lineage_stats`):
`cargo test -p sim --test flour_reignition_census first_post_death_input_absence -- --exact --nocapture`.

**Verdict (both): revise the census trigger/schema per this section, then build the census; R2 stays
unnamed until the census classifies the binding branch.**

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
