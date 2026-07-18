# Design: Mortal producers and the succession of a productive role

**Date:** 2026-07-17
**Status:** Design — approved direction, not yet specced to impl-NN
**Mode:** Research / genetic composition
**Supersedes nothing; extends** `docs/review-and-replan-2026-07.md` (§1.1 the composition gap, §3 the immortal-producer wall)

---

## 1. The problem, stated precisely

OIKOS today is two co-resident but **disjoint populations**: an immortal caste that
produces and accumulates, and a mortal caste that consumes and reproduces. The two never
overlap in the same agent. Verified structural facts (from the replan's code-level
fact-check):

- Every chain producer (Miller/Baker/Scholar) is constructed immortal: the roster loop sets
  `household: None`, `lifespan: None`; old-age death requires `lifespan: Some`; every chain
  config disables starvation (`hunger_critical = need_max + 1`).
- Every mortal lineage member — founders and newborns alike — is hardcoded `Consumer`, forever
  outside the chain.
- Capital accumulates only where death cannot reach. Frontier lineage lifespans average
  ~27 econ ticks; a single capital payback is 16 cycles plus the saving horizon, so a mortal
  agent cannot span its own investment by arithmetic necessity.
**Correction (verified 2026-07-17, supersedes the replan's framing).** The succession
machinery already exists, is landed, and is tested — the replan's "succession does not exist /
no heir ever assumes a producer role" is wrong at the code level:

- Producers are already tagged into dedicated households (`MORTAL_PRODUCER_HOUSEHOLDS = 6`,
  `generation.rs:560-567`) and are already mortal (`frontier_mortal_producers*` scenarios).
- Tool inheritance exists behind gates (`mortal_producer_inheritance` /
  `mortal_producer_tool_inheritance`, `mod.rs:9008-9022`); the estate routes the mill/oven to
  a household heir (`demography.rs:303-336, 421`).
- The heir **assumes the producer role** at the role-choice seam: `phases.rs:2360-2377` is a
  `Consumer → Baker/Miller` switch gated on an inherited, still-held tool, instrumented as
  `heir_tool_adoptions`. In the sweep this fires **200–750 times per run**.
- The whole subsystem is exercised by `sim/tests/mortal_producer_inheritance.rs`
  ("C3R.b v2 — capital inheritance for mortal chain producers"), 5 seeds × 1600 ticks, with
  `Control` / `FlagOffHeritable` control twins and conservation/registry invariants.

**What does NOT exist is a subsidy-free, healthy sustain.** Confirmed by running the sweep:

| regime | cells | outcome |
|---|---|---|
| `food_provision = 0` (no subsidy) | 10/10 | `StructureDoesNotPersist` — dies every seed |
| `food_provision = 1` (light subsidy) | ~6 | persists but `FlowCapped` — limps |
| `food_provision ≥ 2` | 20 | `SubsidyFloodsChainDies` — free food destroys bread demand |
| `FlowRuns` (healthy) in any cell | 1 of 80 | subsidized |

The anchor: `producer_mean_tenure ≈ 27 ticks` in **every** cell, because lifespan is fixed
across the whole existing 2-axis sweep (`food_provision` × `producer_house_cap`). Lifespan —
hence the life/payback ratio — is the one axis never moved.

**A civilization is precisely what happens when production must survive the producer.** The
machinery for the crossing exists and fires constantly; what has never happened is the
crossing *sustaining on its own*. That is the keystone, and it is now a precise, open question.

## 2. The reframing: lifespan is a precondition, not a realism knob

The immortal-producer wall has three levers, because the wall exists whenever a producer's
**life is shorter than a payback**: succession (capital survives the death), cheaper capital
(payback fits a short life), or **longer life (the life spans the payback)**. These are not
alternatives — they compose:

- **Longer life** lets a producer accumulate anything *at all* within one life.
- **Succession** lets that accumulation *cross* to the next generation.
- Lifespan alone → back toward the degenerate immortal case. Succession alone → cannot fire,
  because in a 27-tick life nothing accumulates to bequeath.

The 27-tick lifespan was set for the demography experiments (fast turnover to observe births
and deaths in short runs). It is pathologically short for *this* question. Realism cuts the
right way: real producers live many times longer than it takes to pay off a mill, with room
to train an heir.

**The control variable is therefore the life-to-payback ratio**, not "mortality on/off":

| ratio | outcome |
|---|---|
| ≈ 1 (≈ today) | producer barely recoups, dies with nothing to pass on → chain limps/collapses |
| ≈ 2–4 (hypothesis) | producer recoups, accumulates surplus, endows and trains an heir, dies → heir continues → **sustains across generations** |
| ≫ 1 | death rarely bites, succession almost never fires → degenerate immortal case |

The headline result becomes *"a mortal-producer chain sustains only inside a band of the
life/payback ratio"* — genetically sound, control-friendly, and the same shape of finding
OIKOS already produces well (the Malthusian band).

## 3. Locked premises

- **P1.** The composed economy cannot be built, only grown in genetic order; the taproot is
  the immortal-producer wall.
- **P2.** Breaking it needs three coupled pieces: producers *mortal* (caste split ends), their
  role + capital + skill *inheritable* (succession), and lifespan *long enough to span a
  payback*.
- **P3.** The control variable is the **life-to-payback ratio**; the headline is the band
  where the chain sustains across generations.
- **P4.** Proven with the existing discipline: an immortal-baseline control twin, per-tick
  conservation, multi-seed, digest-pinned goldens.

## 4. Milestone 1 — sweep the life/payback ratio on the existing subsystem

Re-scoped after the 2026-07-17 verification. M1 is **not** a build of new succession
machinery — ~70% of that is already landed (see §1). It is a targeted experiment on the
existing `mortal_producer_inheritance` subsystem: introduce the one axis nobody has moved and
find whether it produces the first subsidy-free healthy sustain.

### 4.1 What changes (small)

1. **Add a lifespan / life-payback-ratio axis to the existing harness.** The subsystem's sweep
   (`sim/tests/mortal_producer_inheritance.rs`) varies `food_provision` and
   `producer_house_cap`; it holds lifespan fixed (`producer_mean_tenure ≈ 27`). Add a third
   axis that scales the producer-household `lifespan_ticks` (via `DemographyConfig`) across
   ratio ≈ 0.5–8× the measured payback. Reuse `trace_run` / `classify` / the `Trace` verdicts
   wholesale — they already emit `StructurePersistsUnderInheritance` and `FlowRuns`.

2. **Sweep at `food_provision = 0`.** The subsidy is a confound: `SubsidyFloodsChainDies` shows
   free hearth food destroys bread demand, and `food = 0` is the clean regime (and the one that
   dies today). The headline is whether *any* lifespan makes `food = 0` reach
   `StructurePersistsUnderInheritance` + `FlowRuns`.

3. **Add the one thing the subsystem lacks: an asserting sustain test.** The existing sweep
   "prints classification without verdict assertions" — it is an observatory, pinning nothing.
   If a subsidy-free sustain band is found, add a test that *asserts* it at the winning ratio
   (a real, pinned, first-in-project result), plus the immortal-baseline control twin (reuse
   `Control` / `FlagOffHeritable`).

**Dropped from the earlier draft, with reason:**
- ~~Move baker roster→lineage~~ — producer households already exist (`MORTAL_PRODUCER_HOUSEHOLDS`).
- ~~Add succession path / heir assumes role~~ — exists (`phases.rs:2360`, fires 200–750×/run).
- ~~Skill transfers with decay~~ — bakers have **no** skill state (`cultivation_skill` is the
  only skill field on `Colonist`). Skill transfer is a new mechanic, not a reuse, and is not
  needed to answer the ratio question. Out of M1 scope; revisit only if the ratio band exists
  and skill turns out to gate healthy flow.

### 4.2 Milestone contract (non-negotiable, matches every prior milestone)

- **Off by default / byte-identical:** the new behavior lives on the new scenario only; every
  existing golden and digest is unchanged.
- **Control twin:** `frontier_mortal_baker` vs an identical `..._immortal_control` (baker
  immortal) — proves the difference is the mortality + succession, not the config.
- **Conservation:** the oven transfer and any gold endowment conserve whole-system; the money
  identity now spans the full tick (the recent oik-1ui fix), so the succession path is covered.
- **Digest guard:** any new behavior-steering field (inherited skill, the succession-enabled
  flag) is classified DIGESTED in the digest-coverage guard (`settlement/digest.rs`) and gets a
  `canonical_bytes_include_*` test — the compile-time guard will *force* this classification.
- **Determinism:** succession draws no randomness beyond what generation already consumes; heir
  selection is deterministic (existing heir-order rules).

### 4.3 The experiment

- Sweep the **lifespan** parameter (hence the life/payback ratio) across a range spanning
  ratio ≈ 0.5 to ≈ 8 on the chosen config, seed 1 first, then a multi-seed confirmation.
- **Primary metric:** does bread output sustain (not decay to zero) across ≥ K baker deaths?
  Report the ratio **band** where it sustains.
- **Secondary metrics:** count of successful handovers; ticks of zero-bread "gap" at each
  handover; whether skill decay across handovers erodes output over generations.

### 4.4 What Milestone 1 deliberately does NOT do

- **No new mechanism.** M1 adds a swept axis and an asserting test to existing code; it does
  not build succession, households, tool inheritance, or role adoption (all landed).
- **Money-emergence and the land market are not composed in** — the `frontier_mortal_producers*`
  base isolates producer succession. Buyer-income enters at the *next* domino.
- No claim that the composed north-star economy sustains — M1 answers only "does raising the
  life/payback ratio produce the first subsidy-free, healthy-flow mortal-producer chain, and
  in what band." (Both miller and baker are already mortal in this subsystem; this is the
  whole two-stage chain, not one role.)

## 5. The genetic roadmap (dominoes, in order)

1. **M1 — single-role succession wedge** (this doc): baker crosses the generation; find the
   ratio band. *Unlocks:* capital can accumulate in a mortal lineage — the precondition every
   downstream null was missing.
2. **M2 — succession as a market** (Approach C): a dead producer's mill/oven goes to the
   existing asset market; any saved-up mortal agent buys it and assumes the trade. *This is
   where buyer-income enters the genetic order* — it needs the wages-fund M1 makes possible.
3. **M3 — unification + composition** (Approach B): caste split removed at the root, general
   role succession, composed with money + land. The north-star run. Earned by walking M1→M2
   first, not attempted big-bang.

## 6. Risks

- **The wedge sustains too easily.** A single-role handover on an otherwise-curated base may
  sustain trivially without stressing the circular flow — a "sustain" that doesn't generalize.
  *Mitigation:* the control twin + the ratio sweep; a trivial sustain shows as sustain across
  the *entire* ratio range (no band), which is itself a diagnostic that the config is too soft.
- **Skill decay dominates.** If inherited-skill decay is too steep, output ratchets down every
  generation regardless of the ratio — a collapse misattributed to mortality. *Mitigation:*
  make decay a swept parameter too, or set it to a measured baker learning curve.
- **Monolith friction.** Even one role touches `generation.rs`, `demography.rs`, `phases.rs`,
  `scenarios.rs`, `digest.rs`. *Mitigation:* the recent decomposition (11 modules) and the
  digest-coverage guard both directly de-risk exactly this.

## 7. The Assignment (do this before writing any succession code)

**Measure the actual payback period on the chosen config.** You cannot set lifespan — the
numerator of the control variable — without the denominator. Instrument the *existing*
immortal-baker run to log, in econ ticks, the interval from "baker begins saving for the oven"
to "oven paid off and producing surplus." No new mechanism, no succession code — a half-day
instrumentation task that grounds the entire ratio sweep in a measured number instead of a
guess, and de-risks the milestone before a line of it is written. That single number tells you
whether today's 27-tick lifespan is ratio ≈ 0.8 or ≈ 1.5, and where to center the sweep.

---

## 8. Grill outcome (2026-07-18) — the wall is upstream of lifespan

A grill-with-docs pass drove the plan down its dependency tree against the data and bottomed
out somewhere neither the office-hours framing nor the impl-71 spec anticipated. Three findings,
in order:

1. **The substrate barely functions even immortally.** The demand-viability pre-check
   (`FlagOffHeritable`, immortal producers on the same base) reaches a functioning chain
   (13,068 loaves, `FlowRuns`) on **only 1 of 5 seeds** (seed 3). The other four immortal seeds
   collapse the baker stage (`bakers=0`, ~400 loaves). The earlier "the substrate works" was an
   over-read of that single seed.
2. **Lifespan did not restore sustain in the tested probe.** A colony-wide lifespan probe
   (×1→×8) moved bread 54 → 389 but never sustained the baker stage at any tested life. "Weak
   secondary lever" is the read, but calling it weak by comparison with the single lucky
   13,068-loaf seed is itself unverified — the honest claim is only that the tested probe did not
   restore a functioning chain.
3. **The wall is in role adoption, not mortality — mechanism is a hypothesis, not established
   (corrected after Codex review, 2026-07-18).** The failing immortal seeds end with the *Baker*
   role rejected while *Miller* persists (endpoint role counts, e.g. millers/bakers = 7/0 —
   end-of-run state, not an adoption tally). Producers here are latent agents holding their tools
   that adopt via `run_role_choice`; the money and mortality gates are ruled out, so the rejection
   is the role-choice **recipe-profit test** — `recipe_adoption_pays_for_money` + (under
   `recurring_motive`) `recipe_is_profitable` (`phases.rs:2298-2318`) — returning `false` for
   baking. This is **NOT** `capital_build_surplus` (a separate settlement-level tool-BUILD
   heuristic; role-choice has no payback bar), and the earlier `bread_price − flour_price` margin
   was wrong: both stages yield 3 per input (`BREAD_PER_BAKE = FLOUR_PER_MILL = 3`,
   `content.rs:80,90`), so the baker margin is `3·P_bread − P_flour − operating_cost` and a bread
   price of ~1 proves nothing without the flour price. **Yield-aware margin starvation of the
   baking role is the leading hypothesis; its price path and structural cause are unmeasured.**
   (The active bread market on failing seeds does not isolate baking: hearths and
   `producer_subsistence` mint staple bread pre-market and agents consume owned bread before
   posting asks, so ~4,300 traded against ~400 baker-produced loaves is mixed provenance, not
   proof of baker demand. And the "seed 3 overproduces but doesn't trade" contradiction I first
   cited was an artifact — 13,068 is cumulative over 1,600 ticks; `late_bread_trades` covers only
   the final 160.)

**Consequence:** lifespan and succession (impl-71 / C3R.f, and the whole C3R line) are downstream
of a chain whose final (baking) role is rejected by the role-choice profit test even without
mortality. Sweeping lifespan on it builds on sand. **impl-71 is BLOCKED** — and, per Codex, does
not unblock until an immortal five-seed viability gate passes (a functioning chain on all five
seeds, not one). **impl-72 / C3R.g** traces the actual role-choice recipe-profit decision to
localize why baking is rejected; the §7 payback idea is dropped (role-choice has no payback bar).

---

*Design produced via office-hours (2026-07-17) and re-diagnosed via grill-with-docs (2026-07-18).
Next step is impl-72 (C3R.g): the per-tick baker-margin appraisal trace, not the lifespan sweep.*
