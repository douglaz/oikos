# Implementation Spec G2c: multiple settlements + caravans (the `Region`)

## Purpose

The final G2 slice: **multiple settlements exist and trade**, completing the
revised G2 (G2a space, G2b space-meets-economy, G2d viewer, G2c here). The
game-spec frames this as "extract settlement-scoped services from the Society
monolith." We achieve the *end* — several independent settlement economies
that trade — by **composition, not internal surgery**: a `Region` holds N
self-contained `Settlement`s (each unchanged from G2b, each its own
`World` + `Society`), linked by abstract inter-settlement **routes**, with
**caravans** carrying a good from where it is cheap to where it is dear. This
preserves the six econ goldens *by construction* (no `Society` internal change)
and keeps every G2b `Settlement` test untouched.

It proves two things, the DoD:

1. **Region-wide conservation is exact** — every good and all gold are
   accounted across all settlements' Societies plus the in-transit caravan
   escrow; sources/sinks remain only harvest/regen and consumption.
2. **Trade converges prices** — with a caravan active between two settlements
   that price a good differently, the realized-price gap narrows over time
   versus a no-caravan control (the price-convergence-along-routes
   phenomenon; sign only).

It is NOT the Society-monolith internal refactor (we compose instead), NOT
intra-settlement-grid inter-settlement movement (routes are abstract
transits), NOT multi-good dynamic routing or many-settlement networks
(two settlements, one traded good; richer trade is later), NOT the Bevy UI,
and NOT a change to `econ`/`world`/`life`/`Settlement` behavior beyond
additive read/transfer accessors (goldens byte-identical).

## Verified Base Facts (2026-06-14, oikos @ `96ab177`, 773 tests green)

1. **`Settlement` (G2b) is self-contained** — owns its `World`, `Society`,
   colonists, exchange, conservation report (settlement.rs). A `Region` can
   hold `Vec<Settlement>` with each unchanged; G2b's tests stay valid.
2. **Runtime agent roster changes are the G4-deferred work** — `AgentArena`
   `free`/cache reconciliation is parked at G4 (engine-divergence.md). So
   caravans must NOT add/remove agents from a `Society` at runtime. Instead,
   a caravan is a **pair of permanent resident trader agents** (one per
   linked settlement, present from generation); the Region shuttles their
   wealth between them. No roster mutation.
3. **`econ` has `credit_stock` (G2b) but no debit/gold transfer accessors**
   (society.rs:3682). G2c adds additive `debit_stock`, `credit_gold`,
   `debit_gold` (or a combined checked transfer) — additive only, no market-
   logic change, golden-safe (the same discipline that made `credit_stock`
   safe; they reject unknown/tombstoned ids and never create/destroy money
   or goods, only move them, with the Region accounting the move).
4. **One shared money (gold)** across the region — the emerged-gold model.
   Caravan proceeds in settlement B are gold the caravan carries back; gold
   is conserved region-wide.
5. **Determinism** inherited: integer, `Rng` at generation only, nothing in
   the loops, id/settlement-ordered iteration, `BTreeMap`/`Vec`, no
   `HashMap`. Same seed + RegionConfig → byte-identical run.

## The caravan model (the load-bearing design)

A caravan linking settlement A and B is a **pair of resident trader agents**:
`trader@A` (an agent in A's Society) and `trader@B` (an agent in B's Society),
both created at generation. They are one logical operator; the Region moves
wealth between them along the route as escrow. No agent is ever added to or
removed from a Society at runtime (respecting the G4 deferral).

Lifecycle of one arbitrage cycle (good X dearer in B than A):

```
1. BUY @A:  trader@A is an active bidder in A's market this econ tick; it
            buys X at A's price (normal Society::step clearing — no special
            accessor; the trader is just an A-agent with gold and an X want).
2. DEPART:  the Region withdraws the bought X (and the gold to fund the
            return leg's purchases) from trader@A into ROUTE ESCROW
            (debit_stock/debit_gold on trader@A); the caravan is now
            "in transit A->B" for the route's transit-tick count.
3. TRANSIT: each econ tick decrements the remaining transit; escrow holdings
            are conserved (not at any settlement, counted in the Region roll-
            up). A configurable loss/risk is OUT OF SCOPE for G2c (no loss).
4. ARRIVE@B: the Region deposits the escrowed X (and gold) into trader@B
            (credit_stock/credit_gold); trader@B becomes an active seller of
            X in B's market.
5. SELL @B: trader@B asks X into B's market; it clears at B's price (normal
            Society::step). Proceeds (gold) accumulate on trader@B.
6. RETURN:  the Region escrows trader@B's gold (and any unsold X) back along
            the route to trader@A; repeat.
```

While a trader-identity is "not the active side" (its wealth is in transit or
at the other end), it places no orders (idle). The Region's per-econ-tick
order: advance each Settlement's econ tick (unchanged G2b), then run the
caravan step (decide/buy/escrow-move/sell) using additive transfer accessors,
then roll up conservation.

Region-wide conservation invariant, every econ tick:

```
for each good X:  sum over settlements [ whole_system_total_X ]  +  route_escrow_X
for gold:         sum over settlements [ society money total ]   +  route_escrow_gold
  change only by (+harvest/regen per settlement, already accounted)
                 (-consumption per settlement, already accounted)
  the caravan transfers are net-zero (debit one identity/escrow, credit the
  other by the same amount).
```

## Milestone Boundary

G2c includes:

- a new `Region` type in `sim` (holds `Vec<Settlement>` unchanged + routes +
  caravans + a region-wide conservation roll-up + realized-price-by-settlement
  accessors); `RegionConfig` (settlements, one route, one caravan, one traded
  good);
- abstract inter-settlement **routes** (a transit-tick count; "near"/"far"
  route lengths for the convergence contrast and the future road idea);
- the **caravan** operator per the model above (permanent trader pairs;
  Region-shuttled wealth as route escrow; deterministic buy-low/sell-high on
  a realized-price differential);
- additive `econ` accessors: `debit_stock`, `credit_gold`, `debit_gold` (or a
  checked transfer), reject unknown/tombstoned ids, conserve (move not mint);
- the region-wide conservation report + a no-caravan control mode;
- a `region` scenario in the `oikos` viewer (read-only): per-settlement prices
  + the convergence gap over time (surfacing the result);
- acceptance tests in `sim/tests/g2c_region.rs`;
- README + `engine-divergence.md` (multi-settlement by composition; the
  caravan-as-trader-pair model; why no Society internal extraction).

G2c excludes:

- no `Society` internal change (we compose N Societies; goldens byte-
  identical); no `Settlement` behavior change (G2b tests untouched);
- no runtime agent roster mutation (G4 deferral respected — trader pairs are
  permanent);
- no caravan loss/risk, no roads-as-projects, no >2 settlements, no multi-good
  or dynamic multi-hop routing (later);
- no balance tuning or asserted price magnitudes — convergence is SIGN only
  (gap-narrows vs control) and conservation is exact;
- no `HashMap` in logic; no new external deps; nothing drawn in the loops.

## Implementation Tasks

1. Additive `econ` accessors (`debit_stock`/`credit_gold`/`debit_gold` or a
   checked `transfer`), with unit tests; run the full conformance suite to
   prove the six goldens unchanged.
2. `sim/src/region.rs`: `Region`, `RegionConfig`, `Route`, `Caravan`;
   `generate`, `econ_tick` (advance settlements → caravan step → roll-up),
   region conservation report, per-settlement realized-price + gap accessors,
   a `caravans_enabled` toggle for the control.
3. The caravan operator per the model (trader pairs, route escrow, the
   deterministic arbitrage decision on the price differential).
4. Viewer `region` scenario (read-only price + gap rendering).
5. Tests (below).
6. README + `engine-divergence.md` updates.

## Acceptance Tests

`sim/tests/g2c_region.rs` (+ unit tests):

1. `region_run_is_deterministic` — same `(seed, RegionConfig)` →
   byte-identical run (digest) across two runs; nothing drawn in the loops.
2. `region_conserves_every_econ_tick` — for every good and for gold: sum over
   settlements + route escrow changes only by accounted regen (source) and
   consumption (sink); every caravan transfer is net-zero; no unit or coin
   created/destroyed at any boundary, including in transit.
3. `caravan_escrow_in_transit_is_conserved` — goods/gold mid-route are
   counted in the Region roll-up (not in any settlement), and arrive intact
   (G2c has no loss); a caravan that never completes a leg retains its
   escrow, never destroys it.
4. `caravan_narrows_the_price_gap` — two settlements pricing good X
   differently: with the caravan enabled, `|price_A(X) − price_B(X)|` at the
   end is smaller than at the start AND smaller than the no-caravan control's
   end gap. Sign only.
5. `no_caravan_control_keeps_the_gap` — with caravans disabled, the price gap
   does not converge (the falsification twin: the caravan is what closes it).
6. `trader_pairs_are_permanent_no_roster_mutation` — agent counts in each
   settlement's Society are constant across the run (no runtime add/remove);
   the caravan moves wealth, not agents.
7. `additive_accessors_are_conservative` — `debit_stock`/`credit_gold`/
   `debit_gold` reject unknown and tombstoned ids, never go negative, and
   move (not create/destroy) value; unit-level.
8. `econ_settlement_unchanged` — the full workspace suite passes; all six
   econ goldens byte-identical; all G1/G2a/G2b/G2d tests green; `cargo clippy
   --workspace --all-targets -- -D warnings`; `cargo fmt --check`.

Manual check:

```bash
cargo test -p sim
cargo run -p viewer -- run region --ticks 30        # per-settlement prices + gap
cargo run -p viewer -- run region-control --ticks 30 # the no-caravan twin
```

## Handoff Notes

- Compose, do NOT carve up `Society`. Multi-settlement = `Vec<Settlement>`,
  each unchanged. The six goldens are safe by construction; test 8 confirms.
- No runtime agent roster mutation (the G4 deferral). Caravans are permanent
  trader PAIRS whose wealth the Region shuttles; test 6 guards this.
- The additive accessors MOVE value, never mint or burn it — every caravan
  transfer is net-zero and the Region roll-up is the ledger (test 2).
- Escrow (route in-transit) is conserved and retained on non-completion,
  never destroyed (the G2b escrow ethos, now inter-settlement).
- Convergence is SIGN only, proven against the no-caravan control (tests 4
  and 5 together). No asserted price magnitudes.
- Determinism: integer, `Rng` at generation only, nothing in the loops,
  settlement/id-ordered, `BTreeMap`/`Vec`. Test 1 is the tripwire.
- `git add` new files; gitignore stray build artifacts.
