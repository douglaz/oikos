# Implementation Spec G2b: two-rate loop + delivery escrow (the `sim` crate)

## Purpose

G2a built the spatial substrate (`world`) in isolation; G2b makes space
**economically meaningful** by wiring it under the economy. It introduces the
`sim` crate — the two-rate orchestrator the game-spec (§4.1, §4.3) calls for —
which owns a `world::World`, per-colonist `life` need state, and an
`econ::Society`, and runs the fast loop (movement, gathering, hauling) under
the economic tick (scale regeneration, market clearing), with the §4.3
**delivery-escrow contract** governing the boundary.

The milestone proves two things, the DoD:

1. **Whole-system conservation is exact** — every good is accounted across
   its full spatial+economic lifecycle (node stock → hauler carry → exchange
   stockpile → econ stock → consumed), and the only sources are harvest yield
   and node regen, the only sink consumption. The escrow (goods in transit)
   is part of the ledger.
2. **Distance measurably affects realized prices** — a good gathered from a
   node FAR from the exchange realizes a higher price than the same good
   gathered NEAR, because travel eats the fast-tick budget so fewer units
   reach the market per economic tick. Sign only (the lab's
   direction-not-magnitude discipline).

This is the two-rate loop + haul-escrow for ONE settlement. It is NOT
multi-settlement / the Society-monolith extraction (G2c), NOT the debug
viewer or inspectors (G2d), NOT wage-labor escrow (the same §4.3 pattern, but
it needs spatial hiring/projects which arrive later — noted, not built), and
NOT a change to any `econ`, `world`, or `life` behavior (their suites and the
six econ goldens stay green and byte-identical).

## Verified Base Facts (2026-06-13, oikos @ `2d7ae45`, 732 tests green)

1. **`world` already models goods-in-transit** — an agent has a position, a
   carried inventory, and tasks (`GoHarvest`/`GoDeposit`/`GoTo`); `World::tick`
   advances movement and applies arrivals (harvest node→carry, deposit
   carry→stockpile) with a `TickReport` conservation total. So the §4.3
   "goods in transit are an escrowed claim" IS a hauler's carried inventory
   while it moves toward the exchange stockpile — no new world concept is
   needed. Public API: `generate`/`add_node`/`add_stockpile`/`add_agent`/
   `assign_task`/`tick`/`run` + `agent_pos`/`agent_carry`/`agent_carry_total`/
   `agent_task`/`agent_status`/`agent_blocked` (world/src/lib.rs:44-48).
2. **`life` is the reusable mechanism** — `NeedState`, `CultureParams`,
   `regenerate_scale`, and the `Camp` non-spatial driver (life/src/camp.rs).
   G2b REUSES `regenerate_scale`/`NeedState`/`CultureParams`; it does not
   modify `life`. `Camp` stays as the G1 non-spatial reference harness with
   its 11 tests intact (sim supersedes it as the driver going forward but
   does not delete it — keeps the G1 proof and the diff bounded).
3. **`econ::Society` clears a market and reports realized prices** —
   `Society::step()` runs the tick; `agents: AgentArena` is public; G1 added
   consumption read-back and `realized_food_price`-style accessors (camp.rs).
   sim drives `Society::step()` exactly as `Camp` does, plus the spatial
   transfer below.
4. **The §4.3 rule: no money mutation in the fast loop; all money mutates in
   the econ tick.** The fast loop (world) moves only physical goods; money
   moves only in `Society::step()`. G2b preserves this.
5. **Determinism is inherited and mandatory** — integer state, `Rng` at
   generation only, nothing drawn in either loop, id-ordered iteration,
   `BTreeMap`/`Vec` only, no `HashMap`. Same seed + same config →
   byte-identical run.

## The world→econ transfer seam (the load-bearing design)

A good has ONE owner at a time — never double-counted:

```
node stock (world)  --harvest-->  hauler carry (world)  --deposit-->
  exchange stockpile (world)  --[econ-tick transfer]-->  econ agent stock
  --trade/consume-->  (econ)
```

- While in `node stock`, `hauler carry`, or `exchange stockpile`, a unit is a
  **world** good (G2a's conservation owns it). Carry-while-moving is the
  **escrow**: committed to delivery, not yet tradable, conserved, retained
  (not destroyed) if the hauler is blocked or dies.
- At each econ tick, `sim` performs the **transfer**: units deposited into the
  exchange stockpile since the last econ tick are removed from the world
  stockpile and added to the depositing agent's `econ` stock, atomically and
  conservingly. After transfer the unit is an **econ** good (tradable,
  consumable).
- Consumption happens in `econ` (the existing FOOD-consumption path) and is
  the only sink. Node regen (world) and harvest yield are the only sources.

`sim`'s whole-system conservation invariant, checked every econ tick:

```
sum over goods of [ node stocks + hauler carries + exchange stockpile
                    + econ agent stock ]
  changes only by (+harvest is internal: node->carry, net 0)
                  (+regen, accounted in the world TickReport)
                  (-consumption, accounted in the econ tick)
```

The transfer is net-zero (world −n, econ +n). Harvest and deposit are
net-zero (relocation). Regen and consumption are the only non-zero deltas,
each independently accounted. This is the spatial+economic ledger invariant,
and it is the G2b conservation DoD.

## Milestone Boundary

G2b includes:

- a new `sim` workspace crate (depends on `world`, `life`, `econ`; pure std;
  deterministic);
- a `Settlement` (or `Sim`) orchestrator: owns a `World`, per-colonist
  `NeedState`/`CultureParams`, and a `Society`; configurable single
  settlement with one exchange stockpile and resource nodes at chosen
  positions;
- the two-rate loop: `FAST_TICKS_PER_ECON_TICK` fast `world` ticks per econ
  tick; the econ tick does transfer → scale regeneration → `Society::step()`
  → consumption read-back → need update → death tombstone (reusing the G1
  mechanism);
- the haul-escrow: in-transit (carried) goods accounted as escrow; arrival
  transfers to econ; non-arrival retains in carry;
- the world→econ transfer seam (additive `econ` accessors only — credit a
  good to an agent's stock; no econ economic-behavior change);
- a whole-system conservation check + a per-econ-tick report;
- realized-price accessors sufficient for the distance test;
- acceptance tests in `sim/tests/g2b_two_rate.rs`;
- README + `engine-divergence.md` (the `sim` crate; the transfer seam; the
  escrow-as-carry modeling; sim supersedes `Camp` as the driver).

G2b excludes:

- no multi-settlement, no `Society`-monolith extraction (G2c);
- no binary, viewer, or inspectors (G2d);
- no wage-labor escrow (same §4.3 pattern; needs spatial hiring/projects —
  noted for a later milestone, not built; G2b's escrow is the haul form);
- no deletion of `life::Camp` or changes to `econ`/`world`/`life` behavior;
  the six econ goldens and all G1/G2a tests stay byte-identical;
- no balance tuning or asserted magnitudes — conservation (exact) and the
  distance→price SIGN only;
- no RNG in either loop; no `HashMap` in logic; no new external deps.

## Domain Semantics

### The two-rate tick

```
Settlement::econ_tick():
  1. FAST: run world for FAST_TICKS_PER_ECON_TICK ticks
     (movement, harvest node->carry, deposit carry->exchange stockpile).
     No money moves. Dead colonists deliver nothing (G1 tombstone +
     world Blocked are short deliveries — escrow simply not fulfilled).
  2. TRANSFER: for each unit deposited into the exchange stockpile since the
     last econ tick, remove from the world stockpile and credit the
     depositing agent's econ stock (net-zero; conserved; recorded).
  3. NEEDS: update each living colonist's NeedState from last econ tick's
     consumption + activity; tombstone starvation deaths (G1 mechanism).
  4. SCALES: regenerate_scale for every living colonist (overwrite econ scale;
     cancel now-stale quotes, as G1 does).
  5. MARKET: Society::step() — the unchanged econ clearing (money moves here
     only); realized prices recorded.
  6. READ-BACK: read consumption for the next need update.
  7. ASSIGN: (re)assign world tasks for the next fast interval
     (gatherers -> their node, then -> exchange; deterministic).
```

Fast loop draws no RNG; econ tick draws no RNG; only `Settlement::generate`
consumes `Rng`. Iteration is always id-ordered.

### Roles in the G2b settlement (minimal division of labor)

- **gatherers**: assigned a node; each fast interval they harvest there and
  haul to the exchange stockpile; at the econ tick the transfer credits the
  hauled good to their econ stock; they sell it on the market.
- **consumers**: at/near the exchange; buy the good and consume it (the
  need/scale loop drives their bids).

This division is what makes a market exist (gatherers sell, consumers buy)
and what makes distance bite (a gatherer far from the exchange hauls less per
econ tick).

### Distance → price (the DoD mechanism)

Holding demand, yields, and population fixed, MOVE the gatherers' node
farther from the exchange. Travel consumes more of the
`FAST_TICKS_PER_ECON_TICK` budget, so fewer units are deposited and
transferred per econ tick → tighter supply at the market → higher realized
price. The test compares two runs (near vs far node placement, all else
identical) and asserts `realized_price(far) > realized_price(near)` — sign
only.

## Implementation Tasks

1. Add `sim` to the workspace; `sim/Cargo.toml` (world + life + econ path
   deps; pure std).
2. Additive `econ` accessor(s): credit a quantity of a `GoodId` to an
   agent's stock by `AgentId` (and read it back) — additive only; run the
   full econ conformance suite to prove goldens unchanged.
3. `sim/src/settlement.rs`: `Settlement`, `SettlementConfig` (grid, exchange
   pos, node specs incl. distance, gatherer/consumer rosters), `generate`,
   `econ_tick`, the transfer seam, the whole-system conservation report,
   realized-price + digest accessors.
4. The two-rate loop + haul-escrow per Domain Semantics; reuse G1
   `regenerate_scale`/needs/tombstone.
5. Tests (below).
6. README + `engine-divergence.md` updates.

## Acceptance Tests

`sim/tests/g2b_two_rate.rs` (+ unit tests):

1. `run_is_deterministic` — same `(seed, SettlementConfig)` → byte-identical
   run (digest) across two runs; nothing drawn in the loops.
2. `whole_system_conserves_every_econ_tick` — across a multi-period run, for
   every good: node + carry + stockpile + econ-stock totals change only by
   accounted regen (source) and consumption (sink); the transfer is
   net-zero; no unit created or destroyed at any boundary.
3. `in_transit_goods_are_escrow_not_lost` — a hauler carrying goods that does
   NOT reach the exchange within the fast interval keeps them in carry
   (counted as escrow), and they transfer on a later arrival; a hauler that
   dies mid-haul (tombstone) retains its carried goods frozen (conserved,
   not destroyed, not transferred).
4. `no_money_moves_in_the_fast_loop` — money/TMS totals are unchanged by the
   fast `world` ticks and change only across `Society::step()` within the
   econ tick.
5. `distance_raises_realized_price` — two runs identical but for the
   gatherers' node distance from the exchange: the far run's realized price
   for the good is strictly higher than the near run's. Sign only.
6. `far_node_delivers_fewer_units_per_econ_tick` — the supply mechanism
   behind test 5: units transferred per econ tick decrease as node distance
   increases (monotone, holding the fast-tick budget fixed).
7. `settlement_sustains_itself` — a viable config runs N econ-years without
   collapse (living gatherers + consumers > 0; needs bounded), smoke test
   only, deterministic.
8. `econ_world_life_unchanged` — the full workspace suite passes; all six
   econ goldens byte-identical; all G1 (`life`) and G2a (`world`) tests
   green; `cargo clippy --workspace --all-targets -- -D warnings`;
   `cargo fmt --check`.

Manual check:

```bash
cargo test -p sim
cargo test            # whole workspace incl. econ conformance + life + world
```

## Handoff Notes

- A good has ONE owner at a time: world (node/carry/stockpile) OR econ
  (agent stock). The transfer is the only crossing and it is net-zero. If a
  unit is ever counted in both, conservation is wrong — test 2 is the
  tripwire.
- Escrow is just carry-while-moving; do NOT invent a separate escrow ledger.
  Non-arrival (blocked/dead) RETAINS goods in carry — never destroys them.
- The fast loop moves goods only; ALL money moves in `Society::step()`
  (§4.3). Test 4 guards this.
- `econ`/`world`/`life` behavior is unchanged: the econ edit is an additive
  stock-credit accessor, proven harmless by the unchanged goldens; `world`
  and `life` are used as-is. Do not modify `life::Camp` or its tests.
- Determinism: integer, `Rng` at generation only, nothing in either loop,
  id-ordered, `BTreeMap`/`Vec`. Test 1 is the tripwire.
- Distance→price is SIGN only (two-run comparison). The first test pinning a
  price magnitude is out of scope (tuning is G2+/later).
- Perf items the G2a panel deferred (per-tick BFS, stockpile sums) get their
  real-load profiling here if they actually bite; do not pre-optimize
  against imagined scale — measure against the two-rate loop.
- `git add` new files; gitignore stray build artifacts.
