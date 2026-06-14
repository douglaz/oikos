# Implementation Spec G5a: money emerges from spatial barter

## Purpose

Every sim settlement so far runs on econ's **designated GOLD** (M1) market —
money is assumed. G5 makes money **emerge**: a settlement starts in **barter**
(no designated money), colonists trade goods-for-goods at the exchange, and a
money good is **promoted** by the Mengerian saleability rule the lab proved
(M5/M6) and studied (M18/M19/M20) — but now driven by **spatial** trade. This
is the spatial counterpart of the lab's money-emergence result.

G5 is sliced (the integration is the biggest yet):

- **G5a (this milestone): money emerges in a minimal barter camp.** A curated
  barter settlement (gatherers + consumers, no production/demography) where a
  money good emerges from spatial barter and afterward trade is money-priced —
  with a **control** (a world that should not monetize). Mechanism + control.
- **G5b: compose emergence with the full stack** (production, demography,
  multi-settlement) — deferred.
- **G5-study: the multi-seed spatial robustness gate** (emergence rate under
  encounter/transport frictions) — deferred, analogous to M18/M19 for the
  lab's non-spatial money emergence.

It is NOT the robustness study (deferred), NOT emergence composed with
production/demography (G5b), NOT a change to econ's emergence *rule* (reused),
and NOT a change to the six econ goldens.

## Verified Base Facts (2026-06-14, oikos @ `b34c463`, 881 tests green)

1. **econ has the full V2 emergence machinery**: `BarterBook` (barter.rs:52),
   `SaleabilityTracker` + `MengerianEmergence` with `winner(config)` /
   `provisional_leader` / `current_money_good` / `promoted_at_tick`
   (menger.rs), `MengerianConfig` (money.rs), and
   `MarketMoneyConfig::Emergent` vs `::Designated`. The promotion rule and its
   adopted envelope (M20 skew default) are reused unchanged — G5a adds no
   emergence-rule logic to econ.
2. **The sim uses `MarketMoneyConfig::Designated(GOLD)` today**
   (settlement.rs:69) — money is pre-designated. G5a introduces a
   barter-start settlement that runs the V2 path until promotion, then
   money-priced trade.
3. **The lab's V2 runs in the non-spatial Society** (`step_v2`): barter
   clears, saleability is measured from realized barter trades, promotion
   fires once, then the society trades in the emerged money. G5a drives this
   machinery from the **spatial** Settlement: colonists present at the
   exchange barter there; the SaleabilityTracker sees those spatial barter
   trades; promotion transitions the settlement to money-priced exchange.
4. **The spatial substrate exists** (G2a/G2b): world, gather/haul, the
   exchange site, the two-rate loop, conservation. G5a reuses it; pre-
   promotion the exchange clears barter, post-promotion it clears the
   money market (the existing G2b path).
5. **Determinism + conservation** inherited: integer, `Rng` at generation
   only, nothing in the loops, `BTreeMap`/`Vec`, no `HashMap`; barter and
   promotion are conserved (a swap relocates goods, a promotion converts a
   good's stock to money units exactly as the lab does).

## Milestone Boundary

G5a includes:

- a barter-start `Settlement` mode: `MarketMoneyConfig::Emergent` driving the
  V2 barter/saleability/promotion machinery within the spatial two-rate loop
  (colonists barter at the exchange pre-promotion; money-priced trade after);
- the spatial→saleability wiring: realized spatial barter trades feed the
  `SaleabilityTracker`; the Mengerian `winner` rule promotes (reused);
- promotion transition: on promotion, the emerged good's stock converts to
  money units (the lab's conserved promotion) and subsequent ticks use the
  G2b money market;
- a curated `barter-camp` config that monetizes and a `no-surplus`/symmetric
  control that does not (the falsification twin);
- conservation across barter swaps and the promotion conversion; determinism;
- viewer surfacing of the barter phase, the saleability leader, and the
  promotion tick;
- acceptance tests in `sim/tests/g5a_emergence.rs`;
- README + `engine-divergence.md` (spatial money emergence; G5b and the
  robustness study deferred).

G5a excludes:

- no emergence composed with production or demography (G5b — the G5a config
  is a plain gatherer/consumer barter camp);
- no multi-seed spatial robustness STUDY (deferred, like M18/M19);
- no change to econ's emergence rule / `MengerianConfig` defaults (reused);
- no multi-settlement emergence (later);
- no change to econ MARKET/emergence behavior — six goldens byte-identical;
  any econ edit additive;
- no `HashMap` in logic; nothing drawn in the loops; no asserted magnitudes
  beyond promotion-happens / control-doesn't (sign) and exact conservation.

## Domain Semantics

### Barter-start settlement and the two-rate loop

A barter-start settlement config carries `MarketMoneyConfig::Emergent`. Its
econ tick, pre-promotion:

```
1. FAST: world movement / gather / haul (unchanged G2b) — physical goods only.
2. TRANSFER: arrived goods cross into econ stock (unchanged).
3. BARTER: colonists present at the exchange clear goods-for-goods via the
   BarterBook (the V2 path), gated by their ordinal counterfactual; realized
   barter trades feed the SaleabilityTracker.
4. PROMOTION CHECK: if the Mengerian winner rule fires, promote — convert the
   winning good's stock to money units (conserved), and from the next tick the
   settlement runs the money-priced market (G2b path).
5. needs / death / etc. (unchanged).
```

Post-promotion the settlement is an ordinary G2b money settlement. No money
moves in the fast loop; promotion and barter are conserved transformations
accounted in the whole-system roll-up (a barter swap is a relocation; a
promotion converts good→money units exactly).

### Emergence + control

The `barter-camp` config seeds gains-from-trade and a saleability
differential (per the M20 finding: asymmetry drives emergence) so a money
good promotes. The `no-surplus`/symmetric control removes the differential
(no gains from trade / uniform demand) so nothing promotes — the falsification
twin proving emergence is driven by the trade structure, not by luck.

## Acceptance Tests

`sim/tests/g5a_emergence.rs` (+ unit tests):

1. `barter_camp_run_is_deterministic` — same `(seed, config)` →
   byte-identical run through barter, promotion, and money-priced phases.
2. `money_emerges_from_spatial_barter` — in `barter-camp`, a money good is
   promoted from realized spatial barter trades; `current_money_good`
   transitions from `None` to the winner at a definite tick; afterward trade
   is money-priced.
3. `no_surplus_control_does_not_monetize` — the falsification twin: with no
   gains-from-trade / no saleability differential, NO good promotes and the
   settlement stays in barter. Paired with test 2, shows emergence is driven
   by the trade structure.
4. `barter_and_promotion_conserve` — every barter swap is a conserved
   relocation; the promotion converts the winning good's stock to money units
   exactly (the lab's conserved promotion); whole-system conservation holds
   across the phase transition.
5. `promotion_transitions_to_money_market` — after promotion the settlement
   runs the G2b money-priced market (realized money prices appear; the barter
   book is quiescent).
6. `emergence_reuses_the_lab_rule` — the promotion decision routes through
   econ's `MengerianEmergence::winner` (reused), not a sim-local
   reimplementation; the adopted M20 envelope/config is used.
7. `econ_unchanged` — full workspace suite; six econ goldens byte-identical;
   all G1/G2*/G3*/G4* tests green; `cargo clippy --workspace --all-targets
   -- -D warnings`; `cargo fmt --check`.

Manual check:

```bash
cargo test -p sim
cargo run -p viewer -- run barter-camp --ticks 40    # barter -> promotion -> money
cargo run -p viewer -- run barter-camp-control --ticks 40
```

## Handoff Notes

- REUSE econ's emergence machinery (`BarterBook`, `SaleabilityTracker`,
  `MengerianEmergence::winner`, the conserved promotion); add NO emergence
  rule to econ. G5a is the spatial WIRING + a curated config + control.
- The control (test 3) is the proof: no monetization without a trade
  structure that supports it. If both monetize, the wiring is reading
  something other than realized barter — fix that, don't weaken the test.
- Conservation spans the phase transition: barter swaps relocate, promotion
  converts good→money units EXACTLY (the lab's rule). Test 4 is the tripwire.
- This is the MECHANISM slice: a plain gatherer/consumer barter camp.
  Composition with production/demography is G5b; the multi-seed robustness
  gate is a deferred study (note both in `engine-divergence.md`). Do not pull
  them in here.
- econ goldens byte-identical; determinism; nothing drawn in the loops.
- `git add` new files; gitignore stray build artifacts.
