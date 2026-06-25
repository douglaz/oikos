# impl-30 — S21g: Mortality-On over the Open-Market Colony (the Malthusian band on a working money market)

Status: DRAFT (pre-Codex-spec-review)
Branch: `feat/open-colony-mortality`
Base: master @ `fa9451e` (S21f landed)

## 0. What this milestone is

S21f closed the *supply* question: an open colony where agents survive by buying food on the market
bootstraps endogenous money from pre-money household cultivation-for-barter — **with mortality off**.
S21g turns the **S17 positive check (starvation) ON** over that exact colony and asks the remaining
capstone question: **does the working money/food market survive real positive-check pressure?** i.e.
does the colony settle into a Malthusian band (births + starvation deaths both binding, no extinction,
no drift) *while SALT still emerges and circulates on `SelfProduced` bread*?

This was deferred to last (Codex's sequencing: mortality goes last so a *monetary* failure is not
masked by a *demographic* wipeout) and is now meaningful precisely because the colony finally has a
clearing market to survive in.

## 1. The change — a scenario composition, no new engine code

The mortality wiring (S17) and the household-barter cultivation seam (S21f) both already exist. S21g
is a **new scenario + a band/money test suite**, no engine change.

`frontier_open_colony_mortality` derives from `frontier_household_barter` (`settlement.rs:3971`) with
the single S17-style mortality flip plus one band-shaping knob:
- **Turn on the positive check:** `dynamics.hunger_critical = dynamics.need_max` (inherited 13 → 12),
  the exact analogue of `frontier_mortality`'s only delta (`settlement.rs:3783`). `death_window = 3`
  is the inherited lab default — untouched, not tuned.
- **Restore the S17 Malthusian-band structure (the preventive arm):** set
  `demography.birth_hunger_ceiling = 8`. S21f inherited `12` from the co-emergent base, which *equals*
  the new critical ceiling — so births would stall and deaths fire at the *same* hunger (a degenerate,
  positive-check-only band). S17 uses `8 < 12` so the **preventive** check (births slow) binds *below*
  the **positive** check (starvation) — the genuine Malthusian structure. The invariant
  `cultivate_hunger_in (6) < birth_hunger_ceiling (8)` still holds (cultivation triggers before births
  are blocked). Disclosed second knob; both are the S17 values.
- **Everything else identical** to S21f: `retire_food_mints`, `household_barter_cultivation`,
  `cultivation_sells_surplus`, `multigood_money`, `spatial_households`, `bread_buffer=0`,
  `consumer_staple_buffer=0`, `starting_food=0`, the grain commons (480/24/960), and the full
  S20+S21a/b/c money machinery.

**No cold-start cushion (keep `starting_food=0`).** The zeroed buffers + retired mints are what make
the `SelfProduced`/`seeded_minted==0` claim clean, and `child_food_endowment=4` is a conserved
provenance-preserving parent→child transfer (never `SeededMinted`). The cold-start trigger *is* the
starting hunger. Adding bread `starting_food` would break the `seeded_minted==0` assertions, so it is
a **last resort** only — the provenance-clean lever if a cold-start die-off appears is grain-flow /
`cultivate_*` timing (faster first production), not seed bread.

## 2. The cold-start survival risk (the #1 risk to verify, not assume)

Quantified tick budget (need.rs defaults: `hunger_deplete=2`, `hunger_per_food=3`, `need_max=12`,
`death_window=3`; S21f cultivation: `cultivate_hunger_in=6`, `cultivate_patience=2`,
`cultivate_consume=4`): from hunger 0, cultivation triggers ~tick 4, the first cultivated bread is
eaten and hunger drops at the **tick-6 needs phase**, while hunger first reaches the critical ceiling
(12) at tick 6 and the earliest death is tick 8 (3-tick streak). **So founders survive the cold-start
with a ~2-tick margin** — but it is thin and hinges on the first grain harvest + conversion landing on
the first cultivation tick. The suite must **verify no extinction** (the colony bootstraps), and if a
cross-seed die-off is observed, classify it (cold-start finding) and address it via grain-flow/cultivate
timing, not seed food.

## 3. Falsifiable bar + controls

Classify a long run (mirror `mortality.rs`; warmup + measure window; plus a 10k-persistence smoke):
- **Both checks bind (the Malthusian band):** `starvation_deaths_total` substantial, `births_total`
  substantial, `old_age_deaths_total > 0`; starvation deaths **rise with hunger** and births **fall
  with hunger** (the `corr` signs, |corr| > 0.3); max hunger oscillates across the critical ceiling.
- **No extinction, no drift:** min windowed living population bounded well above 0; late-window mean
  population within a band of the early-window mean (stationary, neither collapse nor unbounded growth).
- **Money survives the positive check (the S21g-specific bar):** with mortality ON,
  `current_money_good() == Some(SALT)`; pre-promotion bread sold for SALT is `SelfProduced`
  (`pp_produced>0, pp_minted==0`); `acquisition_credited_by_channel().seeded_minted == 0`; food
  consumed is `SelfProduced` + `Bought` (zero `seeded_minted`/`foraged`); indirect breadth `{bread,
  WOOD}`; SALT-mediated share ≥ the S21f headline bar.
- **Conservation every tick** (deaths/births are conserved transfers).

Controls (classify, never tune):
- **mortality off** (`frontier_household_barter`, hunger_critical=13) → the S21f success, zero
  starvation deaths (the positive control: money works, no positive check).
- **birth_hunger_ceiling = 12** (the degenerate preventive=positive band) → report the band shape vs
  the `=8` version (shows the preventive arm matters), not necessarily a failure.
- **more grain flow → higher band / fewer starvation deaths** (the S17 carrying-capacity sweep) →
  confirms the band tracks the food supply, and that starvation is binding (not incidental).
- (If observed) **cold-start extinction at some seed** → a finding (mortality too harsh for
  zero-starting-food cultivation lag); the provenance-clean fix is faster first production.

Cross-seed robustness: the band + money result holds across several seeds (one seed is one seed).

## 4. Slices

- **S21g.0** — the `frontier_open_colony_mortality` scenario (derive `frontier_household_barter` +
  `hunger_critical=need_max` + `birth_hunger_ceiling=8`); a non-vacuity assertion that mortality
  actually fires (starvation_deaths_total > 0) and the colony does not go extinct.
- **S21g.1** — the band+money classification suite (mirror `mortality.rs` band template + the S21f
  money assertions kept alive under mortality), the control matrix (mortality-off positive control,
  the ceiling and grain-flow sweeps), cross-seed robustness, the 10k-persistence smoke, and a live run.

## 5. Determinism / golden contract

- New scenario only; `starvation_deaths_total` is excluded from `canonical_bytes`; `hunger_critical`
  IS digested but only this new scenario's digest changes (no existing golden moves, mirroring
  `frontier_mortality`). **All 20 existing golden suites byte-identical.**
- `cargo fmt --check` + `clippy --workspace --all-targets -- -D warnings` clean; conservation every
  tick; deterministic.

## 6. Honest scope

S21g tests whether the endogenous open-market money colony survives the Malthusian positive check.
A SUCCESS = the full band (preventive + positive) over a *working money market* (SALT still emerges
on `SelfProduced` bread). It remains: a configured grain commons + 3-role WOOD-poor split + SALT
direct-use anchor (carried from S21f); the band thresholds are the S17 lab defaults (disclosed, not
tuned). A clean *finding* (cold-start extinction, or money failing under mortality pressure, or a
redundant-check degeneracy) is equally publishable — it localizes where the working colony stops
surviving the positive check. This does NOT claim emergent role topology or an emergent clearing
institution (those remain future work).

## 7. Pipeline

Codex spec-review → SPEC-READY → rb-lite `codex,claude` (S21g.0→.1) → independent verification
(workspace + all 20 goldens byte-identical + the new suite + a live run) → Codex review-of-results →
merge + report/memory + pin.
