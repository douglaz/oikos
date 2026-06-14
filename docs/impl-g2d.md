# Implementation Spec G2d: the debug viewer + inspectors (the `oikos` binary)

## Purpose

G2a/G2b built a living spatial economy but it is only reachable from tests —
there is no binary. G2d delivers the **first runnable artifact** and the
legibility layer the game-spec (pillar 3, §8) makes central: a headless debug
viewer that runs a settlement and renders its state, plus the two inspectors
the roadmap mandates for G2 — **price → the trades behind it** and **colonist
→ its value scale and why**. This is what turns "passing tests" into
"something you can watch."

It is read-only: it renders from existing accessors and changes no `econ`,
`world`, `life`, or `sim` behavior (all suites and the six econ goldens stay
green and byte-identical). It is NOT the Bevy UI (G9), NOT interactive
gameplay, NOT multi-settlement (G2c). It is a deterministic text CLI.

Taken with G2a (space) and G2b (space meets economy), G2d completes the
revised G2 except for the multi-settlement extraction (G2c), which follows.

## Verified Base Facts (2026-06-14, oikos @ `e209812`, 754 tests green)

1. **`sim::Settlement` exposes everything read-only the viewer needs**
   (settlement.rs): `generate(seed, &SettlementConfig)`, `econ_tick`,
   `run`, `realized_price(good)`/`realized_food_price`, `tracked_goods`,
   `whole_system_total`/`econ_stock_total`/`conserves` (via the report),
   `last_report`, `econ_tick_count`, `population`, `colonist_id(index)`,
   `vocation_of`, `is_alive`, `need_of`, `carry_of`, `living_count`/
   `living_total`, `world()`, `society()`, `exchange()`, `digest()`. Several
   ready-made configs exist: `SettlementConfig::viable()`, `price_probe()`,
   `with_food_node_distance(d)`.
2. **The trade tape for the price inspector** is `Society::trades:
   Vec<Trade>` (society.rs:240; `Trade` at market.rs:30) with
   `Society::good_name(good)` (society.rs:4916) for labels, reachable via
   `settlement.society()`.
3. **The value scale for the colonist inspector** is `Agent.scale:
   Vec<Want>` reachable via `settlement.society().agents` by `AgentId`;
   `need_of`/`vocation_of`/`is_alive`/`carry_of` give the rest of the
   "why".
4. **No binary exists yet** — the workspace is libs only (`econ`, `life`,
   `world`, `sim`); G2d adds the first `[[bin]]`. The praxsim `prax` CLI
   (hand-rolled `take()` arg dispatch, builtin-name resolution) is the
   style to mirror; it lives in the lab, not here.
5. **Determinism** — `Settlement` runs are deterministic (G2b), so a viewer
   that renders from a seeded run produces identical output for identical
   args. No RNG in the viewer itself.

## Milestone Boundary

G2d includes:

- a new thin binary crate (`viewer/`, producing the `oikos` binary;
  depends on `sim` + the libs it re-exports; pure std, no TUI/graphics deps);
- `oikos run` — runs a named scenario config for N econ ticks and prints a
  per-econ-tick dashboard (tick, population by vocation, realized prices per
  tracked good, conservation OK/violated, a needs summary, transfers/
  consumed for the period);
- `oikos inspect price` — for a good, print the realized price and the
  trades behind it (the price→trades inspector) at a chosen tick;
- `oikos inspect colonist` — for a colonist id/index, print its value scale
  (ranked wants), needs, vocation, alive/dead, carry — the "why";
- `oikos scenarios` / `oikos help` — list configs and usage;
- a deterministic-output contract and tests in `viewer/tests/`;
- README + `engine-divergence.md` (the first binary; the viewer is
  read-only and renders from accessors).

G2d excludes:

- no Bevy/TUI/graphics, no interactivity, no input-driven commands (G9);
- no multi-settlement (G2c);
- no new lib behavior — the viewer only READS; if an accessor is missing it
  is added to `sim` as an additive read-only accessor, proven harmless by
  the unchanged goldens;
- no balance tuning, no new economic mechanics;
- no `HashMap` in logic; no new external dependencies (text formatting is
  std only).

## Domain Semantics

### CLI shape (mirror the lab's `prax` dispatch)

```
oikos run <scenario> [--ticks N] [--seed S]
oikos inspect price <scenario> --good NAME [--at-tick T] [--ticks N] [--seed S]
oikos inspect colonist <scenario> --id N        [--at-tick T] [--ticks N] [--seed S]
oikos scenarios
oikos help
```

`<scenario>` resolves to a named `SettlementConfig` (at minimum `viable`,
`price-probe`, `far-node`/`near-node` for the distance contrast). Unknown
scenario / flag is an error with a usage hint (not a silent default).

### The dashboard (`run`)

Per econ tick, one readable block/row: tick number; living colonists by
vocation (gatherers / consumers); realized price per tracked good (or `—`
if no trade cleared); conservation status (`OK` or a loud `VIOLATED` with
the offending good — drawn from the report's `conserves()`); a compact needs
summary (e.g. max/mean living hunger); transferred and consumed totals for
the period. Output is plain text, column-aligned, deterministic.

### Inspectors (the legibility payoff)

- **price → trades**: run to `--at-tick`, then for `--good` print the
  realized price and each `Trade` behind it that tick (buyer, seller,
  price, qty) from `society().trades`, with `good_name` labels. The answer
  to "why is bread N?" is the list of real bilateral trades.
- **colonist → scale/why**: for `--id`, print the colonist's ranked value
  scale (each `Want`: kind via `good_name`/`Leisure`, horizon, satisfied),
  its `NeedState`, vocation, alive/dead, and carried inventory. The answer
  to "why did this colonist do that?" is its ordinal scale and needs.

## Implementation Tasks

1. Add `viewer/` to the workspace; `viewer/Cargo.toml` (`sim` path dep;
   pure std); `viewer/src/main.rs` with hand-rolled arg dispatch.
2. Scenario registry: name → `SettlementConfig` (reuse the existing
   constructors; add `near`/`far` distance variants if not present, via
   `with_food_node_distance`).
3. `run` dashboard renderer (deterministic text).
4. `inspect price` and `inspect colonist` renderers.
5. `scenarios` / `help`.
6. Any additive read-only `sim` accessor the renderers need (e.g. iterate a
   colonist's scale) — additive only; goldens unchanged.
7. Tests (below).
8. README + `engine-divergence.md` updates.

## Acceptance Tests

`viewer/tests/g2d_viewer.rs` (+ unit tests; render to a `String`, not
stdout, so output is testable):

1. `run_output_is_deterministic` — same `(scenario, ticks, seed)` → byte-
   identical dashboard text across two invocations.
2. `run_dashboard_has_expected_shape` — the dashboard for `viable` over N
   ticks has one row per econ tick, shows population and at least one
   realized price, and reports conservation `OK` every tick (it must never
   print VIOLATED for a conserving scenario).
3. `price_inspector_matches_the_trade_tape` — the trades the inspector
   prints for a good at tick T are exactly those in `society().trades` for
   that good/tick; the printed realized price matches
   `realized_price(good)`.
4. `colonist_inspector_matches_state` — the scale/needs/vocation/carry the
   inspector prints for a colonist equal what `sim` reports for it; a dead
   colonist is shown as dead with an emptied scale.
5. `distance_contrast_is_visible` — running the `far` scenario shows a
   higher realized price for the good than the `near` scenario (the G2b
   result, now surfaced in the viewer; sign only).
6. `unknown_scenario_and_flags_error` — unknown scenario / unknown flag /
   missing required `--good`/`--id` produce an error + usage, not a panic
   or a silent default.
7. `inspectors_are_read_only` — running any viewer command does not change
   the lib suites: the full workspace suite passes, all six econ goldens
   byte-identical, all G1/G2a/G2b tests green; `cargo clippy --workspace
   --all-targets -- -D warnings`; `cargo fmt --check`.

Manual check:

```bash
cargo run -p viewer -- run viable --ticks 20
cargo run -p viewer -- inspect price price-probe --good food --at-tick 10
cargo run -p viewer -- inspect colonist viable --id 1 --at-tick 10
cargo run -p viewer -- scenarios
```

## Handoff Notes

- The viewer is READ-ONLY: it renders from `sim`/`econ` accessors and must
  not change any lib behavior. A needed accessor is added to `sim` as an
  additive read-only method, proven harmless by the unchanged goldens.
- Render to `String` (not directly to stdout) so output is unit-testable;
  `main` prints the returned string.
- Determinism is the contract: the run is seeded and the viewer draws no
  RNG; same args → same bytes (test 1).
- Errors are loud: unknown scenario/flag/missing-arg → message + usage, never
  a silent default or panic (mirrors the lab CLI and the G0b command-result
  ethos).
- Keep it text-only and dependency-free; no TUI, no color crates, no graphics
  — that is G9.
- `git add` new files; gitignore stray build artifacts (the `check`-binary
  lesson).
