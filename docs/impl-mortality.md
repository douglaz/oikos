# Implementation Spec: mortality — the Malthusian positive check (S17)

> The last foundational praxeology piece. S14 gave the colony an endogenous carrying capacity
> via the **preventive** check (births stall when hunger rises); S15 let it intensify by
> cultivation. But action under scarcity still has no **survival** consequence — `hunger_critical`
> is set to `need_max + 1`, so starvation death can never fire. S17 turns the **positive** check
> back on: on the fed-and-plateaued cultivation colony, sustained critical hunger kills, so
> population is bounded by **births AND deaths both responding to the carrying capacity** — a full
> Malthusian system. Codex (next-step consult): "action under scarcity must have survival
> consequences, not just hunger counters and stalled births." Independent of the money question;
> the last mechanism-level piece of the arc.

## What the research established (the machinery exists; S17 un-dodges it)

- **The kill is live and streak-gated** (`update_needs_and_remove_dead`, `settlement.rs:6380`):
  each tick, if `need.is_critical` (`hunger >= hunger_critical`, `life/src/need.rs:141`) the
  colonist's `critical_streak` increments, else resets to 0; at `critical_streak >= death_window`
  (default 3) it dies (`settlement.rs:6421-6430`). A single non-critical tick resets the streak —
  built-in hysteresis, so one bad tick never kills.
- **The dodge** (`settlement.rs:2279/2320/2526`): `hunger_critical = need_max + 1`, which `hunger`
  (clamped `0..=need_max`) can never reach — so `is_critical` is never true and death never fires.
  Re-enabling is **config-only**: set `hunger_critical = need_max` (the `lab_default`,
  `need.rs:88`) — no edit to the death machinery.
- **Conservation is already handled**: a starvation death routes through the *same*
  `settle_death → collect_estate → commons/heirs` path as old-age (`settlement.rs:6469/6479/6495`),
  draining gold + stock + world escrow + pending deposits and removing the world agent, guarded by
  a `can_remove_agent` assertion. Nothing vanishes. (The whole `g4a_death.rs` suite is the proof
  harness — slot-reuse, cache-reconcile, estate-to-commons, determinism.)
- **The attribution gap (the one real code add):** there is **no `starvation_deaths_total`** — only
  `old_age_deaths_total` (`settlement.rs:3912`); `update_needs_and_remove_dead` returns its death
  count untracked, and `report.deaths` is starvation + old-age *combined* (`:3484`). S17 must add a
  `starvation_deaths_total` accumulator (mirror `old_age_deaths_total`) + accessor.
- **The dynamics damp endogenously**: `birth_hunger_ceiling` (8) < `hunger_critical` brackets a
  region — as scarcity rises, births stall *first* (preventive), death fires only if hunger climbs
  to the critical ceiling and stays `death_window` ticks; a death frees forage-commons flow →
  per-capita rises (`forage_carrying_capacity.rs:152` proves per-capita falls with forager count,
  so it rises as they fall) → survivors recover → births resume. Band vs spiral hinges on the
  ceiling↔critical gap and `death_window` vs regen speed.
- **Determinism holds at the horizon**: no live RNG in the loop (`deterministic_mix64`); S15 runs
  already prove byte-identical `digest` at 3000 ticks; counters `u64` saturating, `critical_streak`
  `u16`; births+deaths churn the arena slot but keep `world_id == econ_id` (the g4a slot-reuse
  tests cover it).

## Purpose & the honest bar

On a gated path (a `frontier_mortality` scenario = `frontier_cultivation` + the positive check on
at a **principled** threshold `hunger_critical = need_max`, `death_window = 3` — the lab defaults,
NOT tuned to manufacture a band): test what the Malthusian system does. The intended success:
population **oscillates around a carrying-capacity band**, with **births rising when hunger falls
AND starvation deaths rising when hunger persists** (both non-zero), bounded away from extinction;
more food (forage/grain flow) **raises the living band and lowers starvation frequency**;
cultivation-on yields a **higher viable band** than forage-only; deaths are **attributable**
(old-age vs starvation separate, starvation only after sustained critical hunger); conservation
holds on every death. Honest target: **characterize the positive-check dynamics**, reporting which
outcome occurs — do not tune parameters until a band appears.

Outcomes, all first-class (Codex) — and note the **expected** one:
- **Redundant / rare starvation (the EXPECTED result, Codex P3)** → with `birth_hunger_ceiling 8`
  below `hunger_critical 12` and cultivation as an escape valve, the **preventive** check likely
  absorbs most pressure before hunger reaches the critical ceiling for `death_window` ticks, so
  starvation deaths are zero/negligible. This is NOT a disappointment — it is the informative
  praxeological finding that **the preventive check is the binding Malthusian mechanism** in a
  colony with a working escape valve; the positive check is latent. Land it as such.
- **The band (the hoped-for success)** → births AND starvation deaths both phase-track hunger;
  population oscillates around a bounded band (the full Malthusian system).
- **Collapse / extinction** → the positive check is too harsh or the food-response loop too slow.
- **Conservation/estate breakage** → an implementation failure, NOT an economic finding (must not
  happen).
Land the honest outcome (`mortality_dynamics_finding` with the characterized trajectory) if it
isn't the band; do NOT tune `hunger_critical`/`death_window`/the gap to force the band.

NOT the money question (independent — mortality sits on the demographic plateau, not on SALT), NOT
the multi-good economy (separate), NOT a change to existing emergence/goldens. Additive + gated;
flag/scenario off → S5–S16 + all goldens byte-identical (the existing scarcity configs KEEP
`hunger_critical = need_max + 1`, untouched).

## Verified Base Facts (oikos @ `574b1ab`)

1. **Death seam** (`settlement.rs:6380-6471`): streak-gated kill at `critical_streak >= death_window`
   (`:6428`); `is_critical = hunger >= hunger_critical` (`need.rs:141`); reset on non-critical (`:6426`).
2. **The dodge** (`settlement.rs:2279/2320/2526`): `hunger_critical = need_max + 1` (unreachable).
   `dynamics: NeedDynamics` is a `pub` config field (`:1483`); `hunger_critical`, `death_window`,
   `need_max` are `NeedDynamics` fields (`need.rs:55/69/71`).
3. **Counters**: `births_total` (`:3910`), `old_age_deaths_total` (`:3912`, written only by
   `age_and_remove_elderly` `:6781`). **No starvation counter.** `report.deaths` is combined (`:3484`).
4. **Estate/conservation on death**: shared `collect_estate` (`:6495`); `settle_death` dispatches
   heirs-or-commons (`:6479`); `can_remove_agent` guard (`:6453`). `g4a_death.rs` is the harness
   (conservation every tick, slot-reuse, determinism).
5. **Measurement**: windowed-plateau helpers (`forage_carrying_capacity.rs:49-107`,
   `pre_money_cultivation.rs:54`); accessors `living_total` (`:10463`), `population` (`:10285`),
   `births_total` (`:10480`), `old_age_deaths_total` (`:10485`), `max_living_hunger` (`:10812`),
   `birth_block_*` (`:10491-10514`), `need_of` (`:10314`).
6. **Knobs to sweep** (all `pub`, reachable from tests as S14/S15 do): `forage_commons` regen
   (`:848`, `frontier_forage_capacity` `:3249`), the grain node (`frontier_cultivation` `:3333`),
   `own_use_cultivation` (`:858`), `dynamics.hunger_critical`, `birth_hunger_ceiling` (`demography.rs:79`).
7. **Determinism**: no live RNG (`deterministic_mix64`); 3000-tick `digest` byte-identical proven
   (`pre_money_cultivation.rs:25/67`); g4a death path byte-identical (`g4a_death.rs:277`).

## The slices (build in order; each independently testable)

- **S17.1 — enable starvation + attribute it.** Add a `starvation_deaths_total` accumulator
  (mirror `old_age_deaths_total`) incremented from the death count in `update_needs_and_remove_dead`
  (`:6471`), with an accessor. **It is RUNTIME-OBSERVABILITY ONLY — NOT in `canonical_bytes`
  (Codex P1).** Rationale: `old_age_deaths_total` *is* digested today, and existing configs already
  have live starvation (`g4a_death`, `starved_hauler`), so adding this counter to the digest would
  break their goldens (and even a zero `u64` shifts every layout). Determinism is already pinned by
  the existing `digest` (the deaths live in the colonist liveness/estate state); the counter is a
  pure diagnostic the S17 tests read via the accessor. (The asymmetry with `old_age_deaths_total`
  is intentional — avoid golden churn.) Enable the positive check on the S17 scenario via
  `hunger_critical = need_max` (principled, config-only); do NOT touch the existing scarcity/frontier
  configs (they keep `need_max + 1`; existing live-starvation harnesses like `g4a_death` /
  `starved_hauler` are unaffected because the new counter is not digested). **Test:** a starving colonist dies after `death_window` consecutive
  critical ticks (not before); `starvation_deaths_total` increments and is **distinct from
  `old_age_deaths_total`**; the estate settles conserving (reuse the g4a harness — gold+stock+escrow
  to commons/heirs, slot freed, `report.conserves()` every tick); the existing goldens (incl. g4a)
  are **byte-identical** (the counter isn't in the digest).
- **S17.2 — the carrying-capacity band (the core characterization).** On `frontier_cultivation` +
  mortality, over a long window (≥3000 ticks), measure the trajectory. A genuine Malthusian band
  needs **windowed phase behavior, not just nonzero churn** (Codex P2): (a) **high-hunger windows
  are FOLLOWED by starvation deaths**, and **lower-hunger windows are FOLLOWED by births** (deaths
  and births track hunger with the right phase — the negative feedback, not random churn); (b)
  windowed `min_living` is **bounded away from 0** (no extinction); (c) the population is **NOT
  drifting downward** — the late-window mean ≈ the early-window mean (oscillating/flat, not a slow
  collapse); (d) `max_living_hunger` oscillates around `hunger_critical`. **Test:** the band by
  (a)–(d) — OR the named principled-failure: `extinction` (`living_total → 0` / downward drift while
  conservation holds); `redundant` (`starvation_deaths_total == 0` or negligible — preventive check
  absorbs all pressure, the **expected** outcome, see Purpose). Report which; do not tune to force
  the band.
- **S17.3 — the Malthusian response + DoD.** `frontier_mortality` scenario; register the
  `mortality` viewer scenario. **Test:** the carrying-capacity response sweep + the controls below.

## Acceptance Tests (the S17.3 DoD) — `sim/tests/mortality.rs`

1. `mortality_run_is_deterministic` — byte-identical `(seed, config)` at ≥3000 ticks (the band is a
   fixed, reproducible trajectory; a twin-run digest pins it).
2. `starvation_is_streak_gated_and_conserved` — a colonist dies only after `death_window`
   consecutive critical ticks; the estate settles conserving on every starvation death
   (`report.conserves()` each tick; the slot frees + reuses) — the g4a guarantees under the positive
   check.
3. `deaths_are_attributable` — `starvation_deaths_total` and `old_age_deaths_total` are **separate**
   and both meaningful; starvation deaths occur only after sustained critical hunger; their sum
   relates to `report.deaths` as expected.
4. `population_settles_in_a_carrying_capacity_band` — **the core claim**, by windowed PHASE
   behavior (not mere nonzero churn): population does NOT go extinct (`min_living > 0`) and does NOT
   drift downward (late-window mean ≈ early-window mean), with **high-hunger windows followed by
   starvation deaths AND lower-hunger windows followed by births** (the negative-feedback phase),
   `max_living_hunger` oscillating around `hunger_critical` — the full Malthusian dynamic, not the
   preventive-only plateau and not a slow collapse with occasional births. (If instead
   extinction/redundant, this becomes the documented `mortality_dynamics_finding`.)
5. `more_food_raises_the_band_and_cuts_starvation` — sweeping forage/grain flow up **raises** the
   living band AND **lowers** starvation frequency (the carrying-capacity response); cultivation-on
   yields a **higher viable band** than cultivation-off (the S15 control, now with mortality).
6. `mortality_conserves` — whole-system conservation every tick across births + starvation +
   old-age deaths (estate to commons/heirs; no minted food; no leak).
7. `goldens_unchanged` — S5–S16 scenarios + the six econ + g5a/g5b/coemergence + demographic
   (`lineages`) + **g4a_death** goldens byte-identical (the new `starvation_deaths_total` is
   runtime-only, NOT in `canonical_bytes`, so it cannot shift any digest — the tripwire for P1);
   S5–S16 suites green; clippy `-D warnings`; fmt `--check`. (The S17 enabled `hunger_critical`
   lives only in the new `frontier_mortality` scenario; existing scarcity configs keep `need_max + 1`,
   and existing live-starvation harnesses stay byte-identical because the counter is not digested.)

(Principled-failure path: if the colony collapses/extincts or shows zero starvation deaths at the
principled threshold, land `mortality_dynamics_finding` with the characterized trajectory — NOT a
tuned band.)

Manual: `cargo run -p viewer -- run mortality --ticks 4000`.

## Missing Interactions (the central risks)

- **Don't tune the band into existence (Codex, the #1 risk).** Use principled thresholds
  (`hunger_critical = need_max`, `death_window = 3` — the lab defaults; `birth_hunger_ceiling`
  as-is). Run and report. If a *disclosed* sweep is needed to characterize the response, log it; do
  not search parameters for the one band-producing config and call it success.
- **Death spiral.** If the ceiling↔critical gap is too tight or `death_window` too short relative to
  forage regen recovery, deaths outrun the per-capita recovery and cascade to extinction. That is a
  *finding* (positive check too harsh), not a bug — but distinguish it from a conservation/estate
  bug (which IS a bug). The g4a `dieoff_config` shows the marginal-supply collapse shape.
- **Redundancy.** If births already stall hard enough (preventive ceiling) that hunger never reaches
  the critical ceiling for `death_window` ticks, starvation deaths stay 0 and S17 adds nothing — the
  honest "preventive check absorbs all pressure" finding.
- **Conservation on the churn.** Births + deaths both firing every window churns the arena slot
  rapidly; verify `world_id == econ_id` holds and no estate/cache dangles over the long run (extend
  the g4a slot-reuse coverage to the sustained regime).
- **Scope.** Mortality is independent of money/SALT (do not touch the S16 path) and of the
  multi-good economy. The existing scarcity/frontier configs keep `need_max + 1`; existing
  live-starvation harnesses (`g4a_death`, `starved_hauler`) stay byte-identical because the new
  counter is not digested (untouched → byte-identical).
- **Determinism.** `starvation_deaths_total` is runtime-only (NOT in `canonical_bytes`) so it
  shifts no digest; the enabled `hunger_critical` lives only in the new `frontier_mortality`
  scenario. Existing configs (incl. g4a) → byte-identical. The S17 band is a fixed reproducible
  trajectory pinned by a twin-run `digest` at ≥3000 ticks (the existing pattern).

## Handoff Notes

- **This is the last foundational piece** — the positive check completing the Malthusian system the
  preventive check (S14) started. The machinery (kill, estate, conservation, determinism) all
  exists; the new work is the `starvation_deaths_total` counter, enabling `hunger_critical` on the
  S17 scenario, and the long-window band characterization.
- **Reuse g4a_death.rs** for the conservation/determinism/slot-reuse harness; reuse the S14/S15
  windowed-plateau helpers for the band; reuse the S14/S15 sweep pattern for the food response.
- **Honest characterization over a forced band** — the band is the hoped-for success, but
  extinction / redundant are first-class findings; the principled-threshold discipline is what makes
  whichever outcome occurs trustworthy.
- **Gate everything** so S5–S16 + all goldens stay byte-identical; the `lineages` + `g4a_death`
  goldens are the tripwires.
- Build S17.1→S17.3 as separate commits with their own tests; `git add` new files.
- **After S17:** the arc is "praxeologically complete enough" at the mechanism level (money
  emergence, production, capital, time preference, entrepreneurial error, scarcity, carrying
  capacity, intensification, survival selection — all demonstrated or honestly bounded). The
  remaining gap is *richness* (the produced multi-good economy for the strong money claim), not a
  foundational mechanism.
