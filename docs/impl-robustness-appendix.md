# impl-32 — S21i: Robustness Appendix (does the S21f/g/h capstone survive the parameter space, or is it a narrow band?)

Status: SPEC-READY — Codex spec-review NEEDS-REVISION → all four open questions settled and the
6-item punch-list folded in (§4 records the decisions; §2/§3/§5 carry the changes).

## 0. One-paragraph summary

The open-colony capstone landed as four regime claims, each demonstrated at a **single shipped
config** (mostly seed 7, the S21f/g/h scenarios): **S21f** = endogenous pre-money
production-for-barter monetizes SALT (success); **S21g** = mortality-on culls the non-cultivating
demand side before money forms (cold-start finding); **S21h.0** = a seeded consumed-only cushion
yields no *clean* demand-bridge success (knife-edge finding); **S21h.1** = a produced own-labor
emergency survival floor threads it — money + mortality coexist after a one-off cull (success). This
milestone asks the **robustness question Codex flagged as the #1 next step and the #1 credibility
risk**: do those four regimes hold across seeds and parameter bands, or does the S21h success (and
the S21h.0 negative) only hold in a narrow window? The deliverable is a new **test-additive**
acceptance suite that sweeps the disclosed axes, classifies every cell with the *same* 5-tuple
provenance/demand machinery the S21h suite already uses (no new metric, no tuning), and a report
**§8 Robustness Appendix** that states the verdict honestly. **Pre-named finding mode (Codex):** if
S21h.1 success holds only in a narrow window of one or more axes, the capstone headline is
**downgraded** to "money + mortality coexist as an *existence proof under a narrow survival-bridge
band*," not a broad result. This milestone classifies; it does not tune any result into existence.

## 1. Why this milestone, and why now

Codex's strategic evaluation (post-S21h) ranked this #1 by value/effort and named the matching #1
credibility risk: *a hostile reviewer will say "you saved the market by adding a configured no-input
bread floor to the exact agents mortality killed."* The honest defense is not to hide the floor but
to show **how wide the window is** in which it works — and to state the boundary plainly when it is
narrow. Every other roadmap item (endogenize role topology; the article) depends on knowing whether
the headline is robust or band-limited. So robustness comes before any further engine work or
writeup.

This is also the cheapest high-value milestone: almost every axis is an **existing config field**, so
the core suite is **purely test-additive** — which (per the golden discipline, §6) keeps all existing
goldens byte-identical *by construction* and carries near-zero engine risk.

## 2. The central question and the pre-named outcomes

**Central question.** Across seeds and disclosed parameter bands, do the four capstone regimes hold,
and in particular: is the S21h.1 emergency-floor SUCCESS a **broad** result or a **narrow-window**
result?

Each swept cell is classified into one of these regimes using the existing `Cell` 5-tuple
(`survived`, `demanded`, `promoted`, `bought_materially`, `provenance_clean`) plus the broken-invariant
guards (`conserved`, `bread_minted_max==0`, `!extinct`):

- **SUCCESS** — `is_success()` (all five): alive + still-hungry demand + SALT promoted + material
  buying + provenance clean.
- **CULL** — `!survived`: the positive check wiped the non-lineage demand side (the S21g regime).
- **SATED / TOO-STRONG** — `survived && !demanded`: alive but the bridge satiated bread demand out
  of the market (the over-cushion failure mode).
- **SURVIVED-NO-PROMOTE** — `survived && demanded && !promoted`: demand side persists but money never
  forms (a partial — distinguishes "demand alive but supply/anchor insufficient" from a cull).
- **DISQUALIFIED** — `!provenance_clean`: a promotion that sold seeded `SeededMinted` bread for SALT
  (the seeded-supply trap; never counted as a success).

**Per-axis robustness criterion (Codex spec-review Q4: two-step, not one — one step is gameable by
coarse band spacing since *we* choose the band).** For each CORE axis (§3.2), with the shipped value
at band index `i`:

- **ROBUST-on-axis** — the cells at `i-2, i-1, i, i+1, i+2` are ALL SUCCESS across every tested seed
  (two SUCCESS steps on each side), where the band has that many valid values.
- **NARROW-on-axis** — SUCCESS holds at the shipped value but fails (any non-SUCCESS regime) within
  two steps on at least one side.
- **BOUNDED-BY-AXIS** — the shipped value sits at a hard physical/semantic bound of the axis (e.g. the
  validator floor/ceiling), so "interior" is undefined on that side; reported as bounded, **not**
  claimed robust.

The suite must **print the exact band values and the shipped value's index** for every axis, so the
criterion is checkable, not asserted by fiat.

**Capstone verdicts (stated before running; the suite reports which obtains):**

- **ROBUST.** Every CORE axis is ROBUST-on-axis (or BOUNDED-BY-AXIS on a hard bound) AND the three
  headline regimes hold across all `WIDE_SEEDS`. → The capstone headline stands as written (still "in
  this configured topology").
- **NARROW-BAND FINDING.** Any CORE axis is NARROW-on-axis, OR a headline regime fails for some seed.
  → The capstone is **downgraded** in the report to an existence proof under a narrow band, with the
  limiting axis/axes named.
- **MIXED.** Robust on some CORE axes, narrow on others → report the per-axis map; the verdict names
  which axes are load-bearing.

This is a classification, not a pass/fail gate: **a narrow-band finding is a first-class, publishable
outcome**, exactly as S21d/S21g were. The verdict test (§3.2 C) **must not assert "ROBUST"** — it
asserts the regimes are correctly classified and the guards hold, and prints whichever verdict the
data supports. The non-gameability comes from: (i) the two-step criterion over author-chosen bands;
(ii) every 1-D cell run across `CROSS_SEEDS`, not just the shipped seed; (iii) the printed band +
index making any band-spacing trick visible.

## 3. What gets built

### 3.1 A shared classification module (DRY; reuse, do not re-derive)

The `Cell` struct, `is_success()`, `classify(seed, cfg, ticks)`, and the helpers (`living`,
`living_lineage`, `living_non_lineage`, `bread_good`, `with_cushion`, `with_cushion_split`,
`with_emergency`, `MATERIAL_BOUGHT_FLOOR`, `SEEDS`, `PROBE_TICKS`) currently live inside
`sim/tests/demand_survival_bridge.rs`. Extract them verbatim into a shared test-support module so the
robustness suite reuses them rather than copying:

- New file `sim/tests/support/mod.rs` (Rust integration tests share code via a `mod support;`
  declaration with `#[path = "support/mod.rs"]`, or the conventional `tests/common/mod.rs` pattern —
  implementer picks whichever the workspace already prefers; if no precedent exists, use
  `tests/support/mod.rs` with an explicit `#[path]` include in each consumer).
- `demand_survival_bridge.rs` is refactored to `use` the shared module instead of defining the
  machinery inline. **Hard constraint:** its 19 tests, their assertions, and their `eprintln!` output
  semantics stay behaviorally identical — this is a pure move, verified by the suite staying green and
  all goldens byte-identical. **No silent fallback (Codex Q2/P2):** the shared-module extraction is
  the required path; if it cannot be done cleanly, that is a documented implementation finding stated
  in the result and the report, **not** an invisible reversion to a duplicated copy.

Add to the shared module a small `Regime` enum + `Cell::regime(&self) -> Regime` (mapping the 5-tuple
+ guards to the five labels in §2) and a one-line `Cell` formatter for the per-cell `eprintln!` maps,
so every sweep in both suites prints a uniform regime line.

### 3.2 The robustness suite `sim/tests/robustness_appendix.rs`

A new acceptance suite. It does **not merely duplicate** the sweeps the S21 suites already cover
(cushion diagonal/off-diagonal, S21g cross-seed cull, S21e size band); it **extends** coverage to the
CORE/SENSITIVITY axes that were demonstrated at a single config, runs the emergency-threshold window
across the wider `CROSS_SEEDS` (extending the S21h single-seed [7..11] sweep, not re-running it
verbatim), and cross-checks the three headline regimes across a wider seed set. Structure:

**(A) Seed robustness of the three headline regimes (the spine).** For an extended seed set
(§5: `WIDE_SEEDS`, ≥10 seeds), classify the shipped scenarios and assert the regime is stable:
- `frontier_household_barter` (S21f) → SUCCESS for every seed (mortality off).
- `frontier_open_colony_mortality` (S21g) → CULL for every seed (`living_non_lineage == 0`,
  `current_money_good() == None`).
- `frontier_emergency_provision` (S21h.1) → SUCCESS for every seed; record `living_non_lineage`
  (the 12/18 figure is seed-7-specific — report the per-seed survivor count, do not assert a fixed
  12 across seeds). The pre-named claim: SALT promotes on `SelfProduced` bread (`seeded_minted==0`)
  and the demand side survives-and-demands for every seed; the *count* is reported, not pinned.

**(B) Per-axis 1-D sweeps over the S21h.1 scenario (the window maps).** For each axis below, sweep a
band around the shipped value and classify **every cell across `CROSS_SEEDS`** (§5; Codex Q3/P1 —
*not* primary-seed-only with a central cross-check: a single-seed 1-D map can miss seed-dependent
boundaries, so every band cell is run on all `CROSS_SEEDS`). Print one regime line per (cell, seed),
and compute the per-axis criterion (§2: ROBUST-on-axis / NARROW-on-axis / BOUNDED-BY-AXIS) treating a
cell as SUCCESS only if it is SUCCESS for *every* `CROSS_SEEDS` seed. The broken-invariant guards
(`conserved`, `bread_minted_max==0`, provenance-clean-or-disqualified) are a **hard assert on every
cell**, regardless of regime or seed — they never depend on the sweep.

The axes are split into **CORE** (feed the ROBUST/NARROW capstone verdict) and **SENSITIVITY**
(reported and classified, but excluded from the core verdict because they change the topology's
structure or hit hard feasibility bounds — Codex P1/P2).

**CORE axes (the capstone verdict, all existing config fields):**

  1. **Emergency hunger threshold** `chain.emergency_hunger_threshold` — band **`{7,8,9,10,11}`**
     (Codex P1: the validator requires `cultivate_hunger_in (6) < threshold < hunger_critical (12)`,
     so only `{7..11}` are valid classifiable cells; `4`, `6`, `12` are NOT regime cells). Shipped
     value 11 sits at the **top valid bound** → on the high side this axis is BOUNDED-BY-AXIS, so the
     two-step criterion applies only on the low side. Maps the fire-and-relieve window.
  2. **Grain flow** `nodes[grain].regen` — band scaling pre-money food supply down and up around the
     shipped 24, e.g. `{12, 18, 24, 36, 48}`. Too little starves the lineage; too much over-feeds and
     may collapse bread demand.
  3. **WOOD-poor magnitude** `chain.wood_buffer` — band around the shipped 12, e.g. `{4,8,12,24,48}`.
     The S21e second diagnostic axis: WOOD scarcity is what makes bread→SALT IndirectFor{WOOD} lanes
     form; how WOOD-poor must the colony be for SALT to lead?
  4. **SALT anchor density** `barter.salt_direct_use_period` (1-in-N carry the direct SALT want) —
     band around the shipped period 8, e.g. `{4,6,8,12,16}` (smaller period = denser anchor). How
     dense must the regression-theorem direct-use anchor be?

**SENSITIVITY axes (reported + classified, NOT part of the core ROBUST/NARROW verdict — Codex P1/P2):**

  5. **Strong-bar thresholds** `menger.min_direct_use_acceptors` (band `{1,2,3}`) and
     `menger.min_indirect_target_goods`. **`min_indirect_target_goods=3` is INFEASIBLE in the current
     `{bread, WOOD}` two-target topology** — a failure there means the topology supplies only two
     medium targets, NOT that S21h is narrow; it is marked infeasible-by-topology and excluded from
     the verdict. Acceptors `{1,2,3}` is a sensitivity map.
  6. **Role counts** `gatherers`, `consumers`, lineage founder count — bands around the shipped
     (8, 4, 4). **Structural, not merely parametric (Codex P2):** changing counts shifts ID layout,
     demand-hub size, production capacity, and effective SALT distribution, so the spec does not pin
     how SALT supply/thresholds scale with population — reported as a structural sensitivity map only.
  7. **Mortality timing** `demography.birth_hunger_ceiling` (band `{6,8,10}`), `dynamics.death_window`
     (band `{2,3,4}`). A sensitivity map of the Malthusian band shape (`hunger_critical` is the
     mortality on/off switch and is held at the shipped `need_max`).

**(B′) Interaction maps (classification-only; Codex P2 — 1-D sweeps miss combined boundaries).** Two
small 2-D maps over low/shipped/high values, classified across at least seed 7 + one `CROSS_SEEDS`
seed, printed as regime grids (reported, asserted only for the guards + that the shipped (×,×) cell is
SUCCESS):

  - **emergency threshold × grain flow** (does a leaner food supply narrow the threshold window?)
  - **WOOD-poor magnitude × SALT anchor density** (does a denser anchor compensate for less WOOD
    scarcity, or do they jointly gate SALT leadership?)

**(C) The verdict test.** A single `capstone_robustness_verdict` test aggregates the **CORE-axis**
per-axis criteria (ROBUST-on-axis / NARROW-on-axis / BOUNDED-BY-AXIS) and the `WIDE_SEEDS`
headline-regime stability into the §2 capstone verdict (ROBUST / NARROW-BAND / MIXED), printing a
summary table (axis, band values, shipped index, per-side margin, criterion). It **does not assert
"ROBUST"** — it asserts the regimes/criteria are correctly computed and the guards hold, and prints
whichever verdict the data supports; the SENSITIVITY axes and interaction maps are printed alongside
but excluded from the core verdict. The report §8 is written from this test's output.

### 3.3 No silent caps (Codex discipline)

Every bound the suite imposes — which axes are single-seed vs cross-seed, band endpoints, any cell
skipped (e.g. threshold==hunger_critical) — is `eprintln!`-logged and stated in the report §8, so a
reader sees exactly what was and was not swept. Truncating coverage silently would read as "we
covered everything" when we did not.

## 4. Resolved decisions (Codex spec-review)

1. **Emergency target-pull axis → DEFER (test-additive).** The pull depth stays hardcoded
   `target = threshold.saturating_sub(1)` (`settlement.rs:9532`); window-width is probed via the
   `emergency_hunger_threshold` band `{7..11}`. The decoupled pull-depth is a disclosed limitation —
   if threshold robustness turns out narrow, a follow-up **S21i-b** adds a default-preserving
   `emergency_target_pull` knob (digest tag 7) to sweep it. The core milestone is **zero-engine-change**.
2. **Shared classifier module → EXTRACT (no silent fallback).** Move `Cell`/`Regime`/`classify`/
   mutators/constants/formatters to `sim/tests/support/mod.rs`; both S21h and S21i `use` it. Inability
   to extract cleanly is a documented finding, never an invisible duplication (§3.1).
3. **Seeds/runtime → 12 wide seeds, default suite, no `#[ignore]`.**
   `WIDE_SEEDS = [3, 7, 11, 19, 23, 29, 31, 37, 41, 43, 47, 53]`; `CROSS_SEEDS = [3, 7, 19]` (one
   headline seed + two independent). **Every 1-D band cell is run across all `CROSS_SEEDS`** (not
   primary-seed-only). Runtime is acceptable for a robustness appendix; the main evidence is not gated.
4. **Verdict threshold → TWO-step interior for ROBUST.** One success step each side is too weak (we
   choose the bands). ROBUST-on-axis requires two SUCCESS steps each side where the band has the
   values; one-side/one-step margin ⇒ NARROW; hard-bounded sides ⇒ BOUNDED-BY-AXIS, not robust (§2).
   The suite prints exact band values + shipped index so the criterion is auditable.

## 5. Constants

- `PROBE_TICKS = 1_600` (reuse the S21 horizon).
- `SEEDS = [3, 7, 11, 19, 23]` (the existing set; reused for parity tests).
- `WIDE_SEEDS = [3, 7, 11, 19, 23, 29, 31, 37, 41, 43, 47, 53]` — the extended (12-seed) robustness
  set for part (A), the headline-regime spine.
- `CROSS_SEEDS = [3, 7, 19]` — every 1-D band cell in part (B) is classified across all three (one
  headline seed + two independent); a cell counts SUCCESS only if SUCCESS for all three.
- Interaction maps (B′) classified across at least seed 7 + one other `CROSS_SEEDS` seed.

## 6. Determinism & goldens (the safety argument)

This milestone (target-pull deferred, §4) adds **no new config field and no engine code** — it only
calls existing scenario constructors with swept values of existing config fields and reads existing
runtime-only metrics. Per the digest discipline (`canonical_bytes` serializes only colonist
roster/estate/liveness + ON-only digested config; `starvation_deaths_total`, the acquisition ledger,
the bread-Now-wants probe, `seeded_minted_bread_sold_for_salt`, `emergency_bread_provisioned`, and
`pre_promotion_*` are all runtime-only, never digested), **every existing golden is byte-identical by
construction.** The `goldens_unchanged` tripwire (the five pinned digests:
`lineages()`@300/@800, `frontier()`@300, `frontier_spatial_households()`@300, `viable()`@60) is
asserted in the new suite as well, matching the S21h pattern. (The deferred follow-up **S21i-b** would
add `emergency_target_pull` digested ON-only with tag 7 — default 0 emits nothing, same additive
guarantee — but that is out of scope here.)

## 7. Acceptance criteria (independent verification)

- The new `sim/tests/robustness_appendix.rs` suite passes; every cell (every CORE/SENSITIVITY band
  value × every seed run, plus the interaction-map cells) satisfies the broken-invariant guards
  (`conserved`, `bread_minted_max==0`, provenance-clean-or-disqualified) regardless of regime.
- The three headline regimes are stable across `WIDE_SEEDS` (S21f SUCCESS, S21g CULL, S21h.1 SUCCESS),
  with per-seed survivor counts reported (not a pinned 12).
- Each CORE-axis 1-D map is classified across all `CROSS_SEEDS`, printed with exact band values, the
  shipped index, the per-side SUCCESS margin, and the ROBUST/NARROW/BOUNDED-on-axis criterion.
- SENSITIVITY axes and the two interaction maps are printed and classified, explicitly excluded from
  the core verdict (with `min_indirect_target_goods=3` marked infeasible-by-topology).
- The `capstone_robustness_verdict` test prints ROBUST / NARROW-BAND / MIXED with the limiting axes
  named; it does NOT assert ROBUST. The report §8 reflects exactly that verdict (no overclaim — if
  narrow, say narrow).
- `demand_survival_bridge.rs` stays green with identical behavior after the shared-module extraction
  (its 19 tests + assertions unchanged); all existing goldens byte-identical.
- Workspace: all tests pass; **all existing goldens byte-identical**; `cargo fmt --check` and
  `cargo clippy --workspace --all-targets -- -D warnings` clean; conservation holds every tick.

## 8. Report & memory deliverables

- Report `docs/report-emergence-and-its-limits.md`: add **§8 Robustness Appendix** — the per-axis
  regime maps (compact tables), the seed-robustness of the three headline regimes, and the verdict.
  If NARROW-BAND, update §7's S21h bullet and the §5 "strongest defensible claim" / §6 scope to carry
  the band qualifier; if ROBUST, state that the headline is now seed- and band-robust within the
  disclosed envelope (still "in this configured topology"). Update the appendix index with an S21i row.
- Memory `oikos-long-horizon-finding.md` + `MEMORY.md` hook: append the S21i verdict; pin the
  implementation commit hash.

## 9. Pipeline

Codex spec-review (settle the four open questions) → SPEC-READY → rb-lite `codex,claude` →
independent verification (workspace + all goldens byte-identical + the new suite + the verdict run) →
Codex review-of-results → merge + report/memory + pin.
