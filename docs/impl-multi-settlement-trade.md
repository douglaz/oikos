# impl-55 — C8: Multi-Settlement Space — Trade, Migration, Comparative Advantage (does a region specialize and converge?)

Status (spec): **DRAFT — pending Codex spec-review.** Layer C8 of the civilization-core roadmap
(`docs/spec-civ-core-roadmap.md`) — the substrate an empire/4X layer needs. Composes on the merged
**G2c** region + **G7** roads substrate, on **C1–C4** (a living economy per settlement), and softly on
**C6** (knowledge diffusion). Flag `multi_settlement_trade`, digest **tag 29**, ON-only.

Falsifiable bar (headline): does building a road between settlements produce **measurable price
convergence** (G7 already shows this) **and** do settlements **specialize by endowment** — each nets
exports of its locally-cheap good under comparative advantage?

## 0. Dependency & premise (read first)

C8 turns the single settlement into **one node of many**, linked by caravan trade, labor migration, and
specialization by comparative advantage — the empire substrate. It is **provisional on C1–C4** (each
settlement's economy must live for inter-settlement trade to mean anything) and composes the existing
region substrate rather than inventing it. Migration is the one genuinely hard piece and may be a deferred
sub-slice (§3.5).

## 1. Praxeology — voluntary bilateral exchange across distance

Inter-regional trade is **voluntary bilateral exchange** bearing **transport cost**; settlements
specialize by **comparative advantage** (Ricardo / Mises' law of association); migration is an **ordinal
choice** (move if the expected real wage net of moving cost and forgone ties ranks higher). **Anti-smuggling:**
**no gravity-model aggregate flows** — caravans are individual trader trips (`Role::Trader`, seeded by the
S9 `IndirectFor` instrumental-holding pattern), priced by realized price-belief differentials; migration is
each colonist's own ordinal comparison; transport cost is real (distance); spoilage/loss in transit is a
**conserved named sink**.

## 2. What already exists (G2c + G7 — a lot)

- **The region (G2c):** `Region { settlements: Vec<Settlement>, caravan, route, road }` (region.rs:442),
  `econ_tick` with a per-tick **regional conservation roll-up** (`RegionTickReport::conserves`,
  region.rs:364), `regional_total(good) = Σ settlement.whole_system_total + route_escrow + road_fund`
  (:913), `canonical_bytes`/`digest` (:1069).
- **The caravan:** a phase machine `BuyA→ToB→SellB→ToA→Hold` (region.rs:684), **route escrow** for goods +
  gold in transit (conserved, held at no settlement, released on arrival — the delivery-escrow pattern
  already realized here), a **price-differential gate** `haul_is_favourable` (`price_A ≤ price_B`,
  region.rs:821). Traders are permanent pairs (no runtime roster mutation), `debit_stock`/`credit_stock`/
  `debit_gold`/`credit_gold` conserved seams (society.rs:4429).
- **Roads (G7):** `RoadProject` + `road_step` (region.rs:837) cut `transit_ticks` on completion → measurable
  **price convergence** (G7 test) — the headline's first leg already works.
- **Spatial substrate:** deterministic `travel_cost(grid, a, b)` (world/src/path.rs:67) — the distance a
  transport cost should map from.

## 3. Mechanism (what C8 adds; the G2c model is hard-coded and single-good)

### 3.1 Trader route selection (endogenous, ordinal, IndirectFor)
Extend `caravan_step` (region.rs:684) from the hard-coded A→B single flow to **route/good selection by
each trader's own ordinal reservations**. **Correction (spec-review P1): this must NOT be a scalar
max-`(ask − bid − cost)` optimization** — a cardinal/aggregate decision criterion is forbidden
(spec-civ-core-roadmap.md §4). Instead a trader hauls a good on a leg only if buying it at the origin and
reselling at the destination *provisions one of its own future-money wants* — reusing the ordinal
`reservation_bid_for_money`/`reservation_ask_for_money` rank-walk (the same the goods market uses), with
the destination price a **forecast** (S11 belief) and transport cost + capacity entering as a reduction in
the deliverable quantity, not as a maximized margin. It is the same accept/reject the S9 `IndirectFor`
instrumental-holding already expresses, with a **deterministic tie-break** (good-id then route-id order)
when several legs clear. Individual trips, not aggregate flows.

### 3.2 The regional data model + transport cost (new)
**Correction (spec-review P1): the current `Region` cannot express multi-settlement routing as-is** —
`RegionConfig` is exactly `settlement_a`/`settlement_b` + one `Route` + one `good` (region.rs:126),
`Region` holds one route/caravan/good (:441), and `Route` is just `transit_ticks` (:79); `travel_cost`
needs a `Grid` + start/goal `Pos` (world/src/path.rs:67) the region does not carry. So Slice A must
generalize the region to a **`Vec<Settlement>` with per-settlement coordinates**, a **route table** (which
pairs are linked, each with its own `transit_ticks` and per-route caravan state), and an **endpoint→`Pos`
mapping** so `travel_cost(grid, pos_a, pos_b)` yields the route distance. Then map that **distance →
`transit_ticks`** and add a **per-trip capacity** ceiling gating how much a caravan accumulates in `BuyA`.
Transport cost is the real friction that bounds arbitrage and lets differentials persist (converging only
as roads cut cost).

### 3.3 Multi-good caravans
Lift the single-good route escrow (region.rs:407) to a multi-good `BTreeMap<GoodId,u32>` escrow and
multi-good trader scales, so a region can trade its whole basket — the precondition for comparative
advantage to show as specialization across goods.

### 3.4 Spoilage / trip risk in transit (conserved sink)
Perishable goods decay during `ToB`/`ToA` (a conserved **named sink**, the `run_spoilage` precedent);
trip risk is deterministic per seed (a hazard that destroys/loses a fraction — conserved to the sink,
never minted). This prices distance and risk into the trade.

### 3.5 Labor migration (the hard slice — may defer)
Migration is an **ordinal choice**: a colonist moves to another settlement if its expected real wage there
net of moving cost and forgone ties ranks higher on its own scale. **Engineering blocker (disclosed):** the
region composes settlements as a `Vec` with **no runtime agent transfer** between them (G2c traders are
permanent pairs precisely to avoid roster mutation; runtime relocation is blocked until the G4 arena
supports free/insert across settlements). So C8 migration is either (a) a **deferred sub-slice** until the
arena supports cross-settlement transfer, or (b) modeled first via a **cross-settlement proxy** (like the
permanent trader pattern) that shifts *labor supply* without moving the agent object. Slice E decides; the
headline (trade + specialization) does **not** depend on migration.

### 3.6 Comparative advantage (the emergent result)
With multi-good routed trade + transport cost, settlements with different local endowments should **net
export their locally-cheap good and import the dear one** — comparative advantage as a *measured* outcome,
not assigned.

## 4. Praxeology / anti-smuggling guards

1. **Individual trips, no gravity model.** Trade is per-trader arbitrage seeded by `IndirectFor`; a test
   asserts no aggregate flow is scripted.
2. **Transport cost real.** Distance→cost from `travel_cost`; a `zero_transport` control collapses the
   differential (arbitrage instant) — the anti-smuggling twin showing cost is load-bearing.
3. **Spoilage/risk conserved.** In-transit loss is a named sink (conserved), never a mint; a regional
   conservation test covers it.
4. **Migration ordinal.** If built, migration is each colonist's own ordinal wage-net-of-cost comparison,
   not a flow rate.
5. **Specialization measured.** Comparative advantage is read from net trade, never assigned.

## 5. Conservation & determinism

- **Conservation.** **Correction (spec-review P1): the regional roll-up is not yet the full settlement
  identity** — `RegionTickReport` aggregates only `regen/endowment/produced/consumed_as_input/consumed`
  (region.rs:616) and its identity omits `promoted` and `spoiled` (:355), while settlement conservation
  includes them (settlement.rs:5580/5591). **Before** adding C8's in-transit spoilage sink, Slice A must
  bring `RegionTickReport` to mirror the **full** settlement ledger (add `promoted` and `spoiled`). Then
  C8's multi-good escrow + in-transit spoilage extend the completed regional identity (each good's regional
  total conserved; the sink named). Money conservation spans the region (route gold escrow already
  conserved).
- **Digest (tag 29, ON-only).** Extend `Region::canonical_bytes` (region.rs:1069): `if multi_settlement_trade
  { ... route-selection state, per-good multi-escrow, transport-cost/capacity params, migration state if
  built }`. Everything that **steers** routing/hauling/migration is digested; off-path (G2c single-good
  hard-coded caravan) byte-identical.
- **Determinism.** Route selection is deterministic (price-then-margin); trip risk is a deterministic
  per-seed hazard (no live RNG); `travel_cost` is a pure function. Integer-only.

## 6. Slices

- **Slice A — transport cost + capacity.** Map `travel_cost`→`transit_ticks`; per-trip capacity ceiling.
  *DoD: distance measurably bounds arbitrage; roads still converge (G7 preserved); off-path unchanged.*
- **Slice B — multi-good caravans.** Multi-good route escrow + trader scales. *DoD: a region trades a basket
  conserved; single-good G2c goldens preserved off.*
- **Slice C — endogenous route selection + spoilage/risk.** IndirectFor-seeded route/good choice by margin;
  conserved in-transit spoilage/hazard sink. *DoD: traders pick profitable routes; in-transit loss
  conserved.*
- **Slice D — comparative advantage measurement + tag 29.** Measure net exports by endowment; tag-29 digest.
  *DoD: settlements specialize by comparative advantage; goldens byte-identical off.*
- **Slice E — labor migration (deferred/gated).** Ordinal cross-settlement wage migration, via proxy or once
  the arena supports transfer. *DoD (if built): a colony with higher real wage draws migrants; conserved.*

## 7. Acceptance suite (`sim/tests/multi_settlement_trade.rs`)

`SEEDS=[3,7,11,19,23]`, multi-settlement region.

- **Predeclared thresholds (swept):** price-convergence along a road (G7 metric), specialization index (net
  export concentration by endowment), transport-cost elasticity of the differential, migration flow (if
  built).
- **Ordered verdict enum:** base-precondition (regional conservation / C1-living settlements) → outcome:
  `TradeConverges` (a road produces measurable convergence **and** settlements specialize by comparative
  advantage — each net-exports its cheap good) / `AutarkyPersists` (transport cost too high or no gains
  from trade → settlements stay autarkic).
- **Mandatory non-vacuity:** caravans clear inter-settlement trades; net-export specialization is measurable;
  a real counterfactual — convergence along a built road vs. the `no_road` control.
- **Controls:** `no_road` (G7 negative — no convergence); `zero_transport` (differential collapses — cost is
  load-bearing); `autarky` (caravans off); `multi_settlement_trade_off` reproduces G2c/G7.
- **`goldens_unchanged()`:** with `multi_settlement_trade` off, byte-identical to G2c/G7 region goldens.

Build/verify: `cargo test -p sim --test multi_settlement_trade -- --nocapture`, `cargo test --lib`, fmt,
clippy `-D warnings`, workspace green; the G2c/G7 suites green.

## 8. Risks & open questions

1. **Migration is blocked (top).** No runtime cross-settlement agent transfer exists; migration is deferred
   or proxied (§3.5). The headline avoids depending on it.
2. **Upstream dependency.** Empty trade on dead per-settlement economies (C1–C4).
3. **Trip-risk determinism.** A per-seed hazard must stay deterministic (no live RNG) — model as a fixed
   per-route-leg fraction, not a draw.
4. **Multi-good escrow complexity.** Lifting single-good escrow region-wide touches the conservation
   roll-up; the regional test is the guard.
5. **Comparative-advantage requires heterogeneity.** Settlements must have *different* endowments for
   specialization to show — the scenario must seed heterogeneous regions, swept.

## 9. Falsifiable-bar summary

Extending the G2c/G7 region substrate with distance-based transport cost + capacity, multi-good routed
caravans (IndirectFor-seeded), and conserved in-transit spoilage/risk should make a region **specialize and
converge**: a built road produces measurable price convergence (G7, extended) and settlements net-export
their comparative-advantage good (`TradeConverges`), with `zero_transport` and `no_road` controls proving
cost and infrastructure are load-bearing — while labor migration is a disclosed, blocked sub-slice
(deferred or proxied). The honest alternative is `AutarkyPersists` (gains from trade too small / cost too
high) — a first-class finding about when a region integrates.
