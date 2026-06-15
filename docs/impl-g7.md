# Implementation Spec G7: roads — infrastructure cuts trip cost

## Purpose

G2c delivered the substance of "trade": multiple settlements, caravans, and
price convergence. G7 adds the one genuinely-new trade mechanic the game-spec
reserved for this slot — **roads**: a public-works project built from real
labor that, once complete, **cuts a route's transit cost**, so caravans cycle
faster and prices converge faster. Infrastructure investment with a
measurable return — and the first **public works** in the game.

Funding follows the game-spec §5.9 ladder: state taxation does not exist yet
(G8), so a G7 road is **community-funded by labor** — colonists contribute
labor to the road project; it is not a state-treasury expenditure.

Scope: ONE road on the G2c route, with a **no-road control** proving the
road is what speeds convergence. It is NOT state-funded public works (G8),
NOT a road network / pathable roads on the intra-settlement grid (routes are
abstract, per G2c), and NOT a change to econ behavior (the six goldens stay
byte-identical).

## Verified Base Facts (2026-06-15, oikos @ `a4b3048`, 944 tests green)

1. **The route carries `transit_ticks`** (region.rs:80) — econ ticks a
   caravan spends in transit per leg. A road reduces this; the G2c doc even
   anticipates it ("the future road idea"). Fewer transit ticks → faster
   caravan cycles → more trade per horizon → faster convergence (the G2c
   mechanism, now accelerated).
2. **Project/labor machinery exists** (G3): a road is a public project with a
   labor cost that colonists contribute to; on completion it applies its
   effect (cut `transit_ticks`). Reuse the labor/contribution path; no new
   econ project machinery.
3. **Caravans + convergence are proven** (G2c): `caravan_narrows_the_price_gap`
   + the `no_caravan_control`. G7 adds a road that makes that narrowing
   faster, with its own control.
4. **Conservation**: the road consumes real labor (and optionally conserved
   goods), accounted; building a road creates no goods and destroys none
   beyond the labor/inputs spent. The route effect (transit cut) is a config
   change, not a flow.
5. **Determinism + goldens**: road build/effect are deterministic and
   game-only (the lab has no Region/roads); six econ goldens byte-identical
   by construction.

## Milestone Boundary

G7 includes:

- a **road** public-works project on a `Region` route: a labor cost,
  community-contributed (colonists supply labor); on completion the route's
  `transit_ticks` is reduced (a defined fraction/amount);
- the build as a conserved labor expenditure (accounted; optional conserved
  good inputs);
- the effect: post-completion caravans transit faster → the price gap
  converges faster (fewer ticks to a given gap / tighter gap at a fixed
  horizon) than the no-road control;
- a `roads` config (road gets built, convergence accelerates) and a
  `no-road` control (same region, no road — slower convergence);
- viewer surfacing: road build progress, route transit, the convergence gap;
- acceptance tests in `sim/tests/g7_roads.rs`;
- README + `engine-divergence.md` (community-labor public works; state-funded
  works deferred to G8; road networks / grid-pathable roads deferred).

G7 excludes:

- no state-funded public works / taxation (G8 — roads are community labor);
- no road networks, no grid-pathable roads (routes stay abstract per G2c);
- no >2 settlements / multi-route topology (one road on the one route);
- no change to econ behavior — six goldens byte-identical; any econ edit
  additive;
- no `HashMap` in logic; nothing drawn; no asserted magnitudes beyond
  road-speeds-convergence (sign vs control) and conserved labor.

## Domain Semantics

### The road project

A road on a route has a labor cost (and optionally a conserved good cost).
Colonists contribute labor each econ tick (reusing the project-labor path)
until the cost is met; on completion the route's `transit_ticks` drops by a
defined amount (e.g. halved, floored at the route minimum). The contributed
labor is a real expenditure (the contributing colonists' labor that tick goes
to the road instead of their own production/leisure) — accounted in
conservation. The road is one-way (once built, stays); building is
deterministic.

### The convergence effect + control

With the road built, caravans complete cycles in fewer econ ticks, so over a
fixed horizon more goods cross between settlements and the realized-price gap
narrows FASTER (or to a tighter final gap) than the `no-road` control (same
region, road never built). The control is the proof that the road — not time
or the caravan alone (G2c already had the caravan) — is what accelerates
convergence.

## Acceptance Tests

`sim/tests/g7_roads.rs` (+ unit tests):

1. `roads_run_is_deterministic` — same `(seed, config)` → byte-identical run
   incl. the road completion tick and the transit change.
2. `road_is_built_from_labor` — the road completes only after enough
   contributed labor; the labor is a conserved expenditure (accounted); the
   route's `transit_ticks` drops at completion.
3. `road_accelerates_convergence` — with the road, the price gap reaches a
   given threshold in fewer econ ticks (or is tighter at a fixed horizon)
   than the `no-road` control. Sign only.
4. `no_road_control_converges_slower` — the falsification twin: same region
   without the road converges slower / to a wider gap. Paired with test 3,
   shows the road is the cause.
5. `road_build_conserves` — building the road creates no goods and destroys
   none beyond the labor/inputs spent; whole-system conservation holds across
   the build.
6. `road_is_one_way` — once built the transit reduction stays (no flapping).
7. `econ_unchanged` — full workspace suite; six econ goldens byte-identical;
   all prior tests green; `cargo clippy --workspace --all-targets -- -D
   warnings`; `cargo fmt --check`.

Manual check:

```bash
cargo test -p sim
cargo run -p viewer -- run roads --ticks 60       # road builds, convergence speeds up
cargo run -p viewer -- run roads-control --ticks 60
```

## Handoff Notes

- A road is COMMUNITY LABOR (colonists contribute), not a state expenditure —
  taxation/treasury-funded works are G8. Reuse the project-labor path; the
  contributed labor is a conserved expenditure (test 5).
- The effect is a route `transit_ticks` reduction (reuse the G2c field); the
  caravan/convergence machinery is otherwise unchanged.
- The control (test 4) is the proof: without the road, convergence is slower.
  If both converge identically, the road isn't actually cutting transit — fix
  that, don't weaken the test.
- Scope: ONE road on the ONE G2c route; no networks, grid-pathable roads, or
  multi-route topology (deferred). State-funded public works are G8.
- econ goldens byte-identical (Region/roads are game-only); determinism.
- `git add` new files; gitignore stray build artifacts.
