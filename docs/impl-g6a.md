# Implementation Spec G6a: era detection (eras are earned, not timed)

## Purpose

The frontier society (G5b) already passes through institutional phases —
forage, barter, a money good emerges, producers specialize — but nothing
*names* the era. G6a adds the **era detector**: a read-only classification of
the settlement's institutional era from **measured** quantities, with
hysteresis, surfaced in the viewer. This is game-spec pillar 2 ("eras are
earned, not timed") and the "Phase is measured, never set" doctrine inherited
from the lab: the era is a derived statistic, never a state the engine sets or
a timer advances.

It is read-only — it classifies from existing accessors and changes no
`econ`/`sim` behavior (the six econ goldens are trivially safe; era detection
is a measurement layer). It is NOT research/tech-tier unlocking (G6b), NOT the
Credit/Modern eras (those need G8 finance — deferred rungs), and NOT a driver
of any decision (decisions never read the era — purism).

## Verified Base Facts (2026-06-15, oikos @ `c3b3e00`, 912 tests green)

1. **The measured signals already exist** (settlement.rs): `Vocation`
   (Gatherer/Consumer/Miller/Baker/Unassigned), the emergent money good
   (barter phase vs promoted), `population`/`living_count`, realized prices,
   and production throughput. The frontier run demonstrably transitions
   forage→barter→money→specialize (G5b viewer). Era detection reads these;
   it measures nothing new.
2. **The read-only discipline has a precedent** — econ's `metrics` module is
   a read-only measurement layer that decisions may not import. Era detection
   follows it: a `sim`-level read-only classifier, unimportable by any
   decision path.
3. **Determinism + no behavior change**: classification is a pure function of
   measured state; it draws no RNG, mutates nothing, and cannot change a run.
   Goldens byte-identical by construction (it only reads).

## The era ladder (measured triggers + hysteresis)

```
Forager     — no sustained exchange (negligible trade volume)
Barter      — sustained reciprocal exchange (barter trade volume over a window)
Money       — a money good has been promoted (current_money_good is Some)
Specialist  — a sustained division of labor (producer-role share over a window)
Capital     — sustained roundabout production (chain depth / tool capital
              per worker over a window)
```

Credit and Modern eras (chartered banks, state money) are **deferred** to
G8 (they need finance machinery that does not exist in the game yet).

Hysteresis (the load-bearing rule against flapping): an era is ENTERED only
when its trigger holds for a sustained window, and is not abandoned on a
single-tick dip — the detector tracks the highest era reached and only
advances (or regresses) when a trigger sustains/fails over the window. Eras
are ordered; the detector reports the current era and the tick it was first
reached.

## Milestone Boundary

G6a includes:

- a `sim` read-only `EraDetector` (or a `Region`/`Settlement` accessor): the
  measured era ladder above with hysteresis; reports current era + first-tick;
- viewer surfacing: an era banner/column in the dashboard (and the era
  timeline if cheap);
- acceptance tests in `sim/tests/g6a_eras.rs`;
- README + `engine-divergence.md` (era detection is measured/read-only; the
  Credit/Modern rungs deferred to G8).

G6a excludes:

- no research / tech-tier unlocking (G6b);
- no Credit/Modern eras (need G8 finance);
- no decision reads the era (purism: era is measurement-only, like the lab's
  metrics);
- no change to econ/sim behavior — six goldens byte-identical;
- no `HashMap` in logic; nothing drawn; no asserted magnitudes beyond the
  ordered-progression and hysteresis sign claims.

## Acceptance Tests

`sim/tests/g6a_eras.rs` (+ unit tests):

1. `era_detection_is_deterministic` — same `(seed, config)` → identical era
   timeline.
2. `frontier_progresses_through_eras` — the frontier run reaches, in order,
   Barter → Money → Specialist (→ Capital if the config sustains roundabout
   production); each era's first-tick is after the prior era's; the era is
   driven by the measured transitions (promotion tick ≈ Money era onset,
   role-adoption ≈ Specialist onset), not a timer.
3. `eras_do_not_flap` — hysteresis: a single-tick dip below a trigger does not
   regress the era; regression requires a sustained failure over the window.
4. `forager_and_barter_controls` — a no-exchange config stays Forager; a
   barter-only config (no promotion, e.g. the barter-camp control) reaches
   Barter but never Money.
5. `era_is_read_only` — era detection mutates nothing and is not imported by
   any decision path (a source-gate check like the metrics gate); running with
   vs without querying the era yields byte-identical runs.
6. `econ_unchanged` — full workspace suite; six econ goldens byte-identical;
   all prior tests green; `cargo clippy --workspace --all-targets -- -D
   warnings`; `cargo fmt --check`.

Manual check:

```bash
cargo test -p sim
cargo run -p viewer -- run frontier --ticks 80   # era banner advances with the economy
```

## Handoff Notes

- Era is MEASURED, never set: a pure read-only classification from existing
  accessors; no decision reads it (purism source-gate, test 5). It cannot
  change a run — goldens byte-identical by construction.
- Hysteresis is the anti-flap rule: enter on a sustained trigger, don't
  regress on a single-tick dip (test 3). Eras are ordered; track the reached
  era.
- The Credit/Modern rungs are deferred to G8 (finance) — do not invent
  placeholder finance to reach them.
- Reuse the measured signals (vocations, money good, population, throughput);
  measure nothing new in econ.
- `git add` new files; gitignore stray build artifacts.
