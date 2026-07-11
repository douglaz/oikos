# impl-67 — C3R.e: Ignition and Withdrawal (can a finite intervention put the mortal economy into a state that survives the intervention's removal?)

Status (spec): **SPEC-READY (v4)** — three narrowing xhigh rounds. R3's remaining ladder defects folded
with the reviewer's prescriptions verbatim: the B support-era OBSERVATION grid ([0,160),[160,320));
IgnitionNeverIgnites requires exhaustion + ≥1 eligible window (cannot pre-empt ResidualNeverExhausted);
eligibility requires start > intervention_completion (the vacuous-exhaustion hole: A1's global held is 0
before the injection fires); earliest-witness rules; the too-few-passing-windows case named in the
catch-all; the order-preserving retag (adjacent-coalesce only); the §5/§0 residues purged; the B substrate
wording corrected. The remaining gates are the build panel and the result-review. Round 2: the ladder reordered
(HysteresisHolds FIRST among outcomes over exact absolute window grids; IgnitionNeverIgnites made
non-vacuous; exact relapse/EDD boolean predicates; transient patterns → the catch-all); the
SupportOnControlFails precondition made executable with absolute-interval reference comparisons derived
from single traces; the NEW P0 closed — B's cushion WOOD leg (untrackable by the bread-only ledger) is
DISABLED for the entire run, constant across eras, disclosed as the one divergence from the landed
substrate; criterion ii refined to "no economically accessible intervention inventory" (estate-to-commons
= a terminal sink); the market-retag seam pinned (retain drawn lots, set channel=Bought, re-credit
partitioned by origin; mixed-origin partial-draw + retag unit tests in the DoD); the remaining channel-
residue text purged; §8 corrected (six named outcomes + catch-all). Round 1 NEEDS-REVISION (4 P0s
+ P1s, all folded: the resale-proof `intervention` ORIGIN FLAG replacing channel-only tracking, with GLOBAL
exhaustion; ignition-observation-era semantics for the one-shot paths + eligible windows starting at
exhaustion + `ResidualNeverExhausted`; `RegimeUntrapsWithoutIgnition` pre-named and blocking relapse;
HysteresisHolds checked FIRST over ANY 5-window streak; B's support = the EXACT landed substrate
(food=1 AND cushion=4, both withdrawn); A1's dose detector + non-producer donors + a dedicated gate; A2's
bootstrap-sweep split; gatherers pinned 48 with derivation + the round-robin honesty note; the {current}
expectation removed; 7-window B arithmetic; the fixed injective tag-33 record; ladder scoped to
intervention cells with control reference-outcomes; zero-birth windows fail criterion iii; the drawn-lot
transfer gains a breakdown return). The keystone's endgame experiment, built exactly
as the second-opinion review amended and as the C3R.e-obs diagnosis selected: **direct stock provision as
the maximin intervention** (it bypasses all three measured proximate bottlenecks at once), tested as
**ignition with a strict withdrawal/hysteresis bar** — never assumed sufficient. Build base: **master @
the C3R.e-obs merge** (`cc27205` lineage — the consolidated tree now including the shared C3R test module
and the allocation trace). New scenario constructors + knobs (all ON-only, **tag 33** — next free after 32);
all prior bases byte-identical off. **"Multiple equilibria / big push" language is RESERVED for
`HysteresisHolds`** — an observed high state surviving withdrawal — and appears nowhere else.

Falsifiable bar (headline): C3R.a–d closed the keystone into a bootstrap trap and C3R.e-obs proved the trap
economic (microstructure 2–7%) and *correlated* across three proximate bottlenecks. The one gate-proven
fact (the C3R.d sufficiency control): possession of the four-loaf birth stock converts every otherwise-
eligible opportunity into a birth — *while the provision recurs*. The untested question is the development
question itself: **does a FINITE intervention — a one-time stock ignition, or a subsidy later withdrawn —
leave behind a self-sustaining economy, or does the trap reassert itself when the support ends?** The
honest default expectation is relapse (`IgnitesThenRelapses`); `HysteresisHolds` would be the program's
first demonstration that history, not parameters, can separate two trajectories of the same economy.

## 0. One-paragraph summary

The research (probes aee1c817 + a5954e2d) plus the round-1 review established the design. **The load-bearing
new mechanism is an orthogonal `intervention` ORIGIN FLAG on ledger lots** — preserved through every
transfer *including the market-sale retag to Bought* — because channel-only tracking is defeated by resale
(an ignition loaf could leave, return as `Bought`, and falsely pass both the exhaustion and birth-funding
criteria). Exhaustion is a GLOBAL zero-check on origin-flagged holdings; eligible measurement windows begin
only after it. Three ignition paths are compared: **A1 redistribution** (one-shot at tick 50: the C3R.d
conserved injection machinery behind a dedicated ignition gate, donors restricted to NON-producer
households, dose telemetry with `ignition_injected_qty < 24 → IgnitionShortfall` — the driver's shortfall
counter alone cannot detect under-dosing), **A2 additive** (a generation-time producer-house staple
endowment via a dedicated knob — `founders: 0` makes `starting_food` unreachable — with the bootstrap sweep
splitting the endowed quantity into origin-flagged lots, since the ledger boots empty and sweeps all
initial bread to SeededMinted), and **B support-withdrawal** (**the EXACT landed C3R.b viable support
substrate: producer-house `food_provision = 1` AND `producer_subsistence = 4` — the round-1 catch: the
landed viable cell retained the cushion, so food=1 alone is an unproven subsidy — BOTH active while
`econ_tick < producer_support_until_tick = 400`, both zero after**). The demand axis: {current} as landed
(criterion v MAY fail late — the landed evidence is mixed: the surround mint keeps the market
floored-but-active, so no expected verdict is preregistered for the current-regime cell — the
`ExternalDemandDiedFirst` rung exists for whatever the data shows), and {durable} — a DISCLOSED different
economy: the producer WOOD mint retired (producers buy warmth from gatherers out of bread revenue) with
**gatherers pinned at 48** (exactly double the base 24 — derivation: doubles the one recurring-earned-income
class while keeping it a minority; a single pinned value, not swept), where the producer→gatherer→producer
circulation is **mechanically possible and MEASURED per window, not structurally guaranteed** (gatherers on
this base are round-robin across nodes; the WOOD pin belongs to the multi-good-money path). Persistence is
measured on `Vec<WindowTrace>` (160-tick windows ≈ 6 producer lifetimes; B leaves 7 full post-support
windows with an 80-tick tail dropped); births are attributed from the exact drawn FIFO lots (the transfer
gains a drawn-breakdown return); the six-criterion bar and the totality-guaranteed ladder are §2; the
all-goods↔bread-only classifier congruence lands here as a pure sum of existing counters.

## 1. Base facts (verified by the probes; NOTE: several mod.rs line numbers below pre-date the obs merge —
current-master anchors: FoodChannel ~8986, the per-tick driver ~12014, `transfer_birth_stock` ~19520; the
build re-verifies all cites)

1. **No precedent exists for a tick-pinned config cutover; the closest structural precedent is digested.**
   Tick-gated behavior is either recurring-interval (births 14764; land carrying 12938) or runtime
   thresholds — `escrow.release_tick <= econ_tick` (wage_labor.rs:600/612), stored as a plain LE-u64 IN the
   digest (24834). A `producer_support_until_tick: Option<u64>` on `ChainConfig` gates BOTH landed support
   sources (the producer-house hearth in `deliver_demography_provisions` — mint gate/per-house delivery/
   `is_producer_household` scoping — AND `run_producer_subsistence`'s cushion) as one added conjunct
   `econ_tick < until`; `Some(until)` serializes once inside the tag-33 record; the switch rides
   deterministic `econ_tick`. No digest hazard.
2. **The A1 machinery exists and is conserved:** `transfer_birth_stock` (18489 — debit/credit, exact-qty
   rollback 18513–18523, both provenance transfers 18526/18530); the recurring driver
   `run_birth_stock_sufficiency_control` (18536, called per tick at 11299) gates per household on
   eligibility + a ≥target suppression (18577–18581) and records `birth_stock_source_shortfalls` (18616)
   when the richest outside donor lacks the 4 units. **A one-shot needs only an `econ_tick == T` latch on
   the call site** (`birth_stock_ignition_at: Option<u64>`); within that tick the built-in gates still
   apply → up to 24 loaves. THE DOSE, not the driver's shortfall counter, is the detector (an ineligible
   household receives nothing without incrementing the counter — round-1): `ignition_injected_qty` is
   recorded and `< 24 → IgnitionShortfall`. The donor filter must additionally EXCLUDE producer households
   (the existing filter only excludes the recipient household and earlier recipients). Activation: the
   driver gains a dedicated ignition gate (`birth_stock_ignition_at`), independent of the SufficiencyControl
   mode (which currently guards the whole function).
3. **A2 must be a dedicated knob.** Producer houses are `founders: 0, starting_food: 0` (4053–4061);
   `HouseholdSpec.starting_food` reaches only founders (26588/9452). Producer subjects get generation
   stock from the shared `bread_buffer` (25859–25861). The precedent for a targeted class endowment is
   `gatherer_food_cushion` ("a dedicated knob… 0 for every existing config, so byte-identical",
   25846–25857) → `producer_house_starting_staple: u32`, applied to producer-household subjects only. The
   birth gate sees it (free stock); the saving motive adds wants, not reservations — no interference.
   **Ledger boot (round-1):** the acquisition ledger boots empty and sweeps ALL initial bread to
   `SeededMinted` at tick 0 — the A2 endowment is split out at that sweep: exactly the endowed quantity per
   producer subject is credited as origin-flagged lots.
4. **Residual tracking — the ORIGIN FLAG, not a channel (the round-1 P0).** The acquisition ledger
   (channels Bought/SeededMinted/SelfProduced/Foraged/Commons; conservation-asserted per tick; NOT
   digested) preserves channels through `transfer_preserve` — but **a market sale deliberately retags the
   buyer's lots `Bought` regardless of origin**, so channel-tracking is defeated by resale. The fix is an
   orthogonal `intervention: bool` on `FoodLot`, preserved through transfer_preserve, the Bought retag (order-preserving, §0),
   estate-to-heir, and birth transfers; estate-to-COMMONS consumes the flag (terminal, inaccessible). Criterion ii is a
   **GLOBAL** zero-check (every agent, every channel: origin-flagged held == 0), computed by a new
   global + producer-cohort accessor pair (the `non_lineage_acquisition_held_by_channel` pattern). A1
   stamps the moved units; A2 stamps at the bootstrap sweep; B stamps its subsidy mints. The ledger is
   un-digested, so the flag is digest-free. (A `FoodChannel::Ignition` variant is NOT used — the flag
   subsumes it and survives resale; the acquisition ledger must be explicitly ENABLED for every C3R.e
   cell.)
5. **Birth-funding attribution is exact:** the birth debit (14842–14887) is provenance-blind at the stock
   level BUT `transfer_preserve` at 14882 draws the parent's FIFO oldest-first via `draw_lots` (8422),
   returning the exact `Vec<FoodLot>` with channels. **A tally hook at the drawn-lot site** gives exact
   per-birth funding channels (`births_funded_by[channel]`) — no approximation needed.
6. **The multi-window machinery exists:** `WindowTrace` (earned_provisioning.rs:59–132) with `observe()` +
   the same `structure_runs()`/`flow_runs()` bars as the C3R.b/d suites (structure: min millers>0 AND min
   bakers>0 over the window; flow: price off floor + spread positive + bread ≥ 100 + per-staffed-tick ≥
   0.10). Generalization: `Vec<WindowTrace>` indexed `(tick − W)/N`. **Window N = 160 ticks ≈ 6 producer
   lifetimes** (lifespans {18,24,30,36}, mean ≈ 27 — demography.rs:163–166, frontier values 3529–3531).
   Era-entry streaks (era.rs DEFAULT_ERA_WINDOW=3) are NOT persistence windows — not used.
7. **The demand evidence is MIXED (round-1 correction — no preregistered expectation for {current}):**
   the landed `savings_bridge` conjunct requires zero late genuine trades (earned_provisioning.rs:465–470),
   BUT report §29 records the market staying floored-but-ACTIVE while the lineage surround mint is present
   (it dies when the mint is reduced) — and {current} retains the landed surround. So criterion v on
   {current} may or may not hold late; the `ExternalDemandDiedFirst` rung exists for whatever the data
   shows, with NO expected verdict attached to the current-regime cell. Consumers deplete (finite gold, no
   income); **gatherers** hold the only recurring earned loop (87% of genuine external revenue landed).
8. **The durable regime, wire-able and disclosed:** retire the producer-house WOOD mint
   (`wood_provision=0`) so producers must BUY WOOD for warmth out of bread revenue (producers have ordinary
   WOOD/warmth wants — life/src/scale.rs:215), with **gatherers pinned at 48** (double the base 24;
   derivation: doubles the recurring-earned-income class while keeping it a minority; ONE pinned value, not
   swept — classify-not-tune). **Honesty correction (round-1): gatherers on this base are round-robin
   across nodes (the WOOD pin belongs to the multi-good-money path), so the producer→gatherer→producer
   circulation is mechanically POSSIBLE and MEASURED per window (Δ from_gatherers), never claimed
   structurally closed** — gatherer gold can still pool if bread does not clear. NOT wire-able: any
   conserved *consumer* income loop — a recurring consumer-gold mint is named and REJECTED (never run).
9. **Classifier congruence (the debt lands here):** ALL-GOODS genuine external =
   `genuine_external_revenue + non_bread_external_earned`; ALL-GOODS producer-class recirculation =
   `from_other_producer_households + non_bread_producer_class_earned` — all four counters exist
   (7426/7428/7437/7438). The C3R.e flow criteria use the all-goods figures; the legacy bread-only
   `AccountingLoopOnly` reading is untouched (preregistered in its own suite).
10. **The matched trapped reference is already landed:** the C3R.d `NoMotiveReference` cell + pinned facts
    (births [2,3,5,2,1]; structure dead) — reused as the criterion-vi anchor, re-run under each demand
    regime.

## 2. The central question and pre-named outcomes

**Central question.** Across the cell spine (§6), `SEEDS=[3,7,11,19,23]`, `RUN_TICKS=1600`: after ALL
exogenous support has ended and its inventory is provably exhausted **globally**, does the producer economy
hold structure AND flow, with births funded from the market, against active external demand, for **M = 5
consecutive eligible windows** (160 ticks each ≈ 6 producer lifetimes) — while the matched no-ignition cell
stays trapped?

**The intervention-origin dimension (the round-1 P0 fix):** every unit of intervention bread (A1's moved
loaves, A2's endowment, B's subsidy mints) carries an **orthogonal `intervention: bool` origin flag on its
ledger lot**, preserved through EVERY transfer — including the market-sale retag to `Bought` (the sale
changes the *channel*, never the origin flag; the round-1 P0: without this, an ignition loaf could leave
the cohort, return as `Bought`, and falsely pass exhaustion and birth-funding). The acquisition ledger is
un-digested, so the flag is digest-free.

**Eras and eligible windows (the round-1 era-semantics fix):**
- Path B: the SUPPORT era = ticks [0, W); ignition-observation = the support-era windows.
- Paths A1/A2: the IGNITION-OBSERVATION era = from the intervention (tick T / generation) until
  **exhaustion** — the first tick where GLOBAL intervention-origin held == 0.
- **Eligible (measurement) windows** = full 160-tick windows starting at or after BOTH (support ended) AND
  (global intervention-origin held == 0). Pre-exhaustion windows are SKIPPED, never classified (a window
  still eating the intervention teaches nothing about withdrawal). Window boundaries are half-open,
  `[start, start+160)`, anchored at the exhaustion tick rounded up to the next boundary; **for B, the
  post-support era (1600−400 = 1200 ticks) holds 7 full windows with an 80-tick tail DROPPED (disclosed)**
  — M = 5 of up to 7; for A, up to ⌊(1600 − exhaustion)/160⌋ windows.

**The six-criterion hysteresis predicate (all EXACT; evaluated per eligible window `w`):**

```
i.   SupportEnded        — config: path A tick > T; path B tick ≥ producer_support_until_tick. Structural.
ii.  ResidualExhausted   — GLOBAL intervention-origin held == 0 at w's start (every agent, every channel —
                           the origin flag survives resale). Eligibility precondition for w by definition.
iii. MarketFundedBirths  — births in w ≥ 1, AND every birth's drawn lots carry NO intervention-origin flag,
                           AND every drawn lot's channel ∈ {Bought, SelfProduced} (a parent producing its
                           own bread IS the economy working; Commons/Foraged/SeededMinted are not market
                           funding). Zero-birth windows FAIL iii (a childless window is not persistence).
iv.  StructureAndFlow    — WindowTrace.structure_runs() AND flow_runs() for w (the landed bars) AND the
                           ALL-GOODS revenue leg (algebraic): Δ(genuine_external_revenue +
                           non_bread_external_earned) > 0 within w.
v.   ActiveExternalDemand— Δ genuine_external_bread_trades > 0 AND Δ from_gatherers > 0 within w.
vi.  MatchedCellTrapped  — the same-seed no-ignition cell (same regime) fails structure_runs() in the
                           same-numbered window.
```

**Window grids (exact):** B measurement: `[400+k·160, 400+(k+1)·160)`, k=0..6 (7 full; 80-tick tail
dropped); **B observation (support era): `[0,160)`, `[160,320)`** (2 full; `[320,400)` dropped — the
round-3 fix: the measurement grid begins at 400, so the support era needs its own grid). A1: `[50+k·160,
…)` (observation = the pre-exhaustion windows; measurement = the eligible ones). A2: `[k·160, …)`.
**Eligible** windows = grid windows with `start > intervention_completion` (A1: start > 50 — the round-3
vacuous-exhaustion fix: global held is 0 BEFORE the injection fires, so the intervention-tick window must
be excluded; A2: generation precedes tick 0, so start ≥ 0 is safe — held > 0 from tick 0 until consumed;
B: start ≥ 400) AND global origin-held == 0 at `start`. Reference (matched) cells are compared over the
SAME ABSOLUTE intervals (views derived from one reference trace — no extra runs).

**Exact predicates:** for eligible window `w`: `pass(w) := iii(w) ∧ iv(w) ∧ v(w) ∧ vi(w)` (i–ii hold by
eligibility). `streak := ∃k: pass holds for 5 consecutive eligible windows starting at k`.

**Ordered verdict enum (first-match; applies to the INTERVENTION cells — A1/A2/B-withdrawn; controls carry
reference/guard outcomes, §6; printed, never asserted):**

```
Preconditions: BaseUnviable / ReservoirOpen / ConservationBroken / RegistryBroken (the verbatim quartet),
               + IgnitionShortfall (A1: ignition_injected_qty < 24 at T),
               + SupportOnControlFails (durable intervention cells only: the same-seed support-on control
                 (cell 4) passes structure_runs() in ZERO absolute-grid windows over [400, 1600) — the
                 support era itself is unviable on this regime, voiding every same-regime withdrawal
                 verdict; an explicit executable precondition, not prose).
HysteresisHolds{first_window, windows_survived} — `streak` (checked FIRST among outcomes — an empty or
                        failed ignition-observation era CANNOT suppress a later qualifying streak). The
                        ONLY rung where multiple-equilibria / big-push language may be used.
IgnitionNeverIgnites  — NON-VACUOUS form: exhaustion occurred AND ≥1 eligible window exists (so this rung
                        can never pre-empt ResidualNeverExhausted — round-3) AND the ignition-observation
                        era contains ≥1 full observation-grid window AND none of them passes
                        structure_runs() AND no streak exists.
ResidualNeverExhausted — no eligible window exists (inventory persists to run end, or exhaustion leaves no
                        full grid window): withdrawal UNDETERMINED, not relapse.
RegimeUntrapsWithoutIgnition{window} — ¬vi(w) for some eligible w; the witness = the EARLIEST such w
                        (the matched cell passed structure in the same absolute interval): persistence is
                        not attributable to ignition history.
ExternalDemandDiedFirst{window} — no streak, vi held throughout, and `¬v(w)` for some eligible w at or
                        before the earliest `¬(iii ∧ iv)` window (or no v-alive eligible window exists at
                        all); the witness = the EARLIEST ¬v window: UNDETERMINED, not relapse.
IgnitesThenRelapses{relapse_window} — no streak; relapse_window = the EARLIEST eligible w with
                        `¬(iii(w) ∧ iv(w))` (either predicate failing) where `v(u) ∧ vi(u)` held for EVERY
                        eligible u ≤ w. The honest default expectation.
UnclassifiedMixed     — exact terminal catch-all: transient recover-then-fail with no 5-streak; ALL
                        eligible windows passing but fewer than 5 exist (too-short measurement — printed
                        with the count); anything else. All quantities printed (ladder totality).
```

## 3. Mechanism

### 3.1 The three ignition paths (each a pinned, finite intervention; every intervention unit origin-flagged)
- **A1 — redistribution (conserved total):** `birth_stock_ignition_at = Some(50)`; at exactly tick 50 the
  injection machinery runs once behind its OWN ignition gate (independent of the SufficiencyControl mode
  guard), donors restricted to **non-producer households** (richest-first, the existing deterministic
  picker otherwise), moving up to 6×4 = 24 loaves via the conserved rollback-safe transfer with every moved
  unit **origin-flagged**. `ignition_injected_qty` is recorded; `< 24 → IgnitionShortfall` (the dose is the
  detector). Aggregate bread unchanged — a pure allocation intervention.
- **A2 — additive (aggregate raised):** `producer_house_starting_staple = 4` per producer-house subject at
  generation; the tick-0 bootstrap sweep splits exactly that quantity into **origin-flagged** lots (the
  ledger boots empty and would otherwise sweep everything to SeededMinted). No mid-run machinery. The
  A1↔A2 contrast separates allocation from aggregate scarcity.
- **B — support-withdrawal (the exact landed STAPLE pair values, with the disclosed WOOD-leg divergence):** producer-house
  `food_provision = 1` AND the `producer_subsistence = 4` cushion's STAPLE leg (the landed viable cell
  retained the cushion; food=1 alone is an unproven subsidy — round-1 catch) — both active while
  `econ_tick < producer_support_until_tick = Some(400)` (~15 lifetimes), both zero after (one field gates
  both; `food_provision` feeds all producer-house members while the cushion is producer/latent-scoped —
  disclosed). **The cushion's WOOD leg is DISABLED for the ENTIRE run (round-2 P0):** the acquisition ledger is
  bread-only, so subsidized WOOD cannot be origin-tracked — rather than let untracked WOOD survive
  "exhaustion," the B cells run the cushion staple-only from tick 0, constant across both eras (nothing
  WOOD-shaped changes at W; no WOOD residual exists). Disclosed as the ONE minimal divergence from the
  landed substrate. Every support-minted staple unit is **origin-flagged** (resale-proof).
All pins (T=50, W=400, 24 = 6 × `child_food_endowment`, the B substrate = the landed viable values) derive
from landed values and lifetime arithmetic — none searched.

### 3.2 The demand-regime axis
- **{current}:** the C3R.d base as landed. The demand evidence is mixed (§1.7) — NO expected verdict is
  preregistered for this cell; the `ExternalDemandDiedFirst` rung exists for whatever the data shows.
- **{durable}:** a new scenario constructor — the C3R.d base with (a) the producer-house WOOD mint retired
  (`wood_provision = 0`; producers buy warmth out of revenue — the circulation MEASURED, not guaranteed:
  §1.8) and (b) **gatherers = 48** (one pinned value with its derivation, §1.8 — not swept). Honest
  labeling: a DIFFERENT ECONOMY (a regime); every cross-regime comparison says so. No consumer-gold mint
  exists in any cell.

### 3.3 The four machinery additions (all runtime-only except the config fields)
(1) `Vec<WindowTrace>` eligible-window machinery (§2); (2) the `intervention` origin flag on `FoodLot`,
preserved through every transfer incl. the market retag, + the GLOBAL and producer-cohort origin-held
accessors (§1.4); (3) the drawn-lot birth-funding tally — `transfer_preserve` (or a birth-specific variant)
returns the drawn breakdown (it currently returns unit — round-1); (4) per-window deltas of the
genuine-external + gatherer counters + the all-goods sums (§1.9). Config fields: `birth_stock_ignition_at`,
`producer_house_starting_staple`, `producer_support_until_tick`, the durable-regime constructor — **tag 33
as ONE fixed injective record: `push(33)` then a presence-bit byte then every field in fixed order
(absent = presence bit 0, no bytes)** — the round-1 injectivity fix; the tick-switch rides deterministic
`econ_tick` (the digested escrow.release_tick precedent).

### 3.4 What is deliberately NOT here
No recurring support in any measured cell (the recurring C3R.d control mode is not a C3R.e cell — it
proved gate sufficiency, not ignition). No demand nudge beyond the disclosed durable REGIME. No consumer
income mint. No tuning of ignition size/timing toward `HysteresisHolds` (single pinned values; the honest
default is relapse). No "multiple equilibria" language anywhere below the `HysteresisHolds` rung.

## 4. Anti-smuggling guards
1. **Withdrawal is real and verified:** criterion i is config; criterion ii proves NO economically
   accessible intervention inventory remains (origin-flag-tracked GLOBALLY, resale-proof; estate-to-commons
   is a terminal, economically inaccessible sink and counts as exhausted — the round-2 refinement), so a
   "persistence" cannot be the support still being eaten.
2. **Births must be market-funded (iii)** — exact drawn-lot attribution; a high state living off residual
   subsidy lots cannot classify HysteresisHolds.
3. **The matched trapped cell (vi)** makes persistence attributable to ignition history, not the regime:
   same regime, same seeds, no ignition, must stay structurally dead in the same windows.
4. **The support-on control** (B never withdrawn, durable regime) isolates withdrawal as the tested
   variable: if even permanent support fails on the durable regime, every withdrawal result is void
   (BaseUnviable-style guard for the era).
5. **The demand caveat is a verdict, not a footnote:** `ExternalDemandDiedFirst` is UNDETERMINED and
   precedes the relapse rung — a relapse can only be claimed while external demand demonstrably lived.
6. **A1's feasibility is detected, not assumed:** `IgnitionShortfall` disqualifies rather than silently
   under-igniting.
7. **Pinned, derived values throughout;** the gatherer count is the one REGIME parameter, disclosed and
   swept only if load-bearing.

## 5. Conservation & determinism
A1 moves existing bread (conserved, rollback-safe); A2 is a generation-time initial stock (booked like all
initial stocks); B is the existing mint switched off at a deterministic tick; the durable regime removes a
mint. The `intervention` origin flag lives on the un-digested acquisition ledger. **Digest:** tag 33 ON-only —
the §3.3 fixed injective record; all prior bases byte-identical off;
the tick-switch rides deterministic `econ_tick` (the digested-threshold precedent). Telemetry (runtime-
only): the window series (structure/flow/demand deltas per window), `births_funded_by[channel]` per window,
the cohort per-channel held series, `birth_stock_source_shortfalls`, the all-goods congruent revenue
figures, the C3R.d battery carried forward. The acquisition ledger is explicitly ENABLED in every C3R.e cell.

## 6. Slices & cells
- **Slice A — the knobs + channels:** the three config fields, the durable-regime constructor,
  the `intervention` origin flag (FoodLot field + stamping at the three intervention sites + preservation through the market-retag seam: the retag must map each drawn lot IN ORIGINAL FIFO ORDER to channel=Bought preserving the flag, coalescing only ADJACENT equal-origin lots (a partition would reorder mixed FIFOs and corrupt later exhaustion/birth attribution — round-3); mixed-origin partial-draw and market-retag unit tests), tag 33. *DoD: flag-off
  byte-identical (all prior bases direct); tag-33 split; each path fires exactly as pinned (one-shot at 50;
  hearth off at 400; endowment at generation).*
- **Slice B — the measurement machinery:** `Vec<WindowTrace>`, the cohort held accessor, the drawn-lot
  tally, the per-window demand deltas, the all-goods congruent figures. *DoD: deterministic; the tally's
  per-birth channel counts sum to `child_food_endowment`; the accessor matches the ledger invariant.*
- **Slice C — the suite** `sim/tests/ignition_withdrawal.rs`. **The cell spine (7 × 5 seeds = 35 runs;
  drops logged):**
  1. `{durable, B-withdrawn}` — the headline hysteresis cell.
  2. `{durable, A1-redistribution}` — allocation-only ignition.
  3. `{durable, A2-additive}` — aggregate-raising ignition.
  4. `{durable, B-never-withdrawn}` — the support-on control.
  5. `{durable, NoIgnition}` — the matched trapped reference (criterion vi).
  6. `{current, B-withdrawn}` — the current-regime cell (no preregistered expectation).
  7. `{current, NoIgnition}` — its matched pair.
  **The §2 withdrawal ladder applies to the INTERVENTION cells (1, 2, 3, 6) only** (round-1 totality fix);
  the controls print reference/guard outcomes instead: cell 4 → `SupportOnControlViable`/`SupportOnControl
  Fails` (the era guard: if even permanent support fails on the durable regime, every same-regime
  withdrawal verdict is void); cells 5/7 → `MatchedReferenceTrapped`/`MatchedReferenceUntrapped` (feeding
  criteria vi and the RegimeUntrapsWithoutIgnition rung). Everything printed per cell × seed, never
  asserted; landed C3R.d facts pinned as executable anchors where reused.
- **Hard guards (invariants only):** conservation/money/registry, the reservoir guard, byte-identity +
  tag-33 split, the ladder-totality invariant, the drawn-lot sum invariant, the support-on control's
  era guard (§4.4), and the pinned-anchor reproductions.

Build/verify: `cargo test -p sim --test ignition_withdrawal -- --nocapture`, all landed suites unchanged,
full workspace, fmt, clippy `-D warnings`.

## 7. Risks & open questions
1. **Ignition may simply not ignite from stock alone** (A1/A2: 24 loaves fund one heir cohort; whether the
   chain restarts within their lifetimes is the experiment) — `IgnitionNeverIgnites` is first-class and
   informative against B's contrast.
2. **The durable regime may itself change the trap** — producers buying WOOD adds a new expense; the
   matched no-ignition cell (5) feeds the pre-named `RegimeUntrapsWithoutIgnition` rung (checked BEFORE
   relapse; relapse additionally requires the matched cell trapped in every considered window) — the
   honest conclusion would be that the regime, not the history, mattered: a first-class finding about the
   demand wall.
3. **Path B's residual check is the ORIGIN FLAG, not the SeededMinted channel** (resale-proof, §1.4); the
   birth-food-floor credit remains disclosed as a non-origin SeededMinted source (irrelevant to the flag).
4. **Window arithmetic (corrected, round-1):** path B's post-support era = 1200 ticks = 7 full 160-tick
   windows + an 80-tick tail (dropped, disclosed); M = 5 consecutive of up to 7. Path A's eligible windows
   start at exhaustion — if exhaustion is late, few windows remain (`ResidualNeverExhausted` covers the
   degenerate case).
5. **Scope:** no game-track work, no article surgery in this milestone; the recurring-vs-depleting demand
   question is ANSWERED only within the two regimes as built.

## 8. Falsifiable-bar summary
Four slices built the trap; the diagnosis proved it economic and selected the escape lever; this experiment
runs the escape with the strictest bar the program has preregistered. A finite ignition either leaves
behind an economy that feeds, breeds, produces, and sells on its own — for five consecutive
six-lifetime windows, against live external demand, with its births paid for on the market and no
economically accessible intervention inventory remaining — or one of the other named outcomes lands:
the intervention never ignites, its inventory never exhausts (undetermined), the regime untraps without
it, the demand base dies first (undetermined), the trap reasserts itself (the honest default), or the
pattern fits none and says so. Six substantive named outcomes plus an exact catch-all; only
`HysteresisHolds` graduates the keystone's claim from "a trap we can describe" to "a trap we have
watched an economy escape and stay out of."
