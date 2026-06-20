# Finding: household subsistence at scale (S12) — NO MIDDLE BAND

> Outcome of implementing `docs/impl-household-subsistence.md`. The own-labor
> subsistence mechanism is built, gated, conserving, and feeds the colony — **but the
> milestone's falsifiable core is FALSIFIED**: once the food mints are retired (as the
> milestone requires), money no longer emerges at any forage yield. This is the
> principled-failure mode the spec anticipated ("low garden → starvation, high garden →
> no money, no middle band"), landed honestly as a first-class finding rather than
> papered over by re-minting food or raw-grain edibility.

## What was built (and works)

A new default-off `ChainConfig::own_labor_subsistence` path:

- **FORAGE**, a low-grade subsistence good interned into the chain content
  (`ContentSet::with_forage`), wired as `KnownGoods::subsistence` so the consume
  readback counts it as hunger relief, ranked **below bread** via the existing
  subsistence offset (`life/src/scale.rs`), perishable via the existing spoilage phase.
- **`Task::GoForage`**, a world task analogous to `GoHarvest`: a hungry, eligible,
  unprovisioned spatial non-lineage colonist with spare labor forages instead of
  harvesting WOOD (the structural opportunity cost — one world task per colonist per
  tick). It relocates **no** world good; the floor is produced and credited at the econ
  layer.
- **`run_own_labor_subsistence`**, the econ phase that credits `forage_yield` units of
  FORAGE into a forager's OWN stock, booked **`report.produced`** (own labor) — NOT
  `report.endowment` (a mint). One source line, no node-regen double-count.
- The **hunger-good mints retired** on this path: `producer_subsistence`'s staple line
  and the demographic `food_provision` are zeroed (the WOOD/warmth provision stays).

Verified: FORAGE is produced not minted (`report.endowment[bread] == 0`), whole-system
conservation holds every tick, the FORAGE good + `own_labor_subsistence` flag + the
per-colonist `foraging` state are in `canonical_bytes` with regressions, and with the
flag **off** every S5–S11 scenario + the econ/emergence goldens are byte-identical.

**The floor feeds the colony.** Against the semi-hungry S11 baseline (mean hunger ~8,
p95 12, max 12, 12 chronically hungry), the provisioned colony at any forage yield ≥ 1
drops to **mean ~4, p95 ≤ 6, max ≤ 6, 0 chronically hungry**.

## The finding: no middle band

The pinned sweep — forage-yield `{0, 1, 2, 3, 4, 6, 8}` carry/tick × seeds
`{1, 7, 0xC0FFEE}` × 1600 ticks (`subsistence_and_monetization_have_no_middle_band`) —
records per cell `{tail mean/p95/max hunger, promoted_at_tick, pre-promotion
bread-for-SALT volume, tail bread.made + active-producer input trades}`.

| forage yield | tail hunger (mean/p95/max) | SALT promoted? | bread-for-SALT |
|---|---|---|---|
| 0 (no forage) | 12 / 12 / 12 (starves) | **no** | 0 |
| 1–8 (fed)     | 3–4 / 4–6 / 4–6 (bounded) | **no** | 0 |

**No cell satisfies bounded hunger AND SALT promoted AND bread trades.** The forage
floor either is absent (the tail starves) or is present (the tail is fed) — but SALT
never monetizes once the food mints are retired. The milestone's central claim ("fed AND
money still emerges") does not hold for any forage configuration.

## Why — the mechanism (isolated)

Retiring the mints, then re-introducing them one at a time, localizes the cause:

- **Retire `producer_subsistence` only** (lineage `food_provision` stays): SALT still
  promotes (≈ tick 443), bread-for-SALT volume material. The producer staple mint is
  **not** load-bearing for emergence.
- **Retire `food_provision` only** (producer staple stays): SALT does **not** promote;
  bread-for-SALT volume collapses to 0. The **lineage food mint is load-bearing**.

The single hunger scalar couples two things the milestone needs to separate. The retired
`food_provision` minted **bread** — the very good SALT monetizes against — and minted it
*per tick*, sustaining the pre-promotion bread supply across the strong-bar gate's long
emergence window (the seeded bread buffer alone spoils under the carrying-cost decay
before SALT can accrue its indirect breadth). FORAGE is, by design, a **distinct
sub-bread good**: it relieves the same hunger scalar but adds no bread to the
bread-for-SALT trade. So:

- feed the colony with **bread** (the mint) → money emerges, but that is the mint the
  milestone must retire;
- feed it with **forage** (own labor) → hunger is bounded, but the bread demand and
  bread supply that monetize SALT are gone;
- feed it with **nothing** → the colony starves and hoards its SALT for its own unmet
  hunger, so SALT never circulates.

There is no setting of a one-dimensional hunger floor that frees the colony's money to
circulate **and** keeps bread the widely-traded superior good.

## The fix (out of scope for S12)

The finding is exactly the spec's predicted resolution: the **single-hunger-scalar food
model is too coarse**. A faithful fix needs **differentiated food quality/services** —
bread satisfying a preference (variety, a prepared-meal service, a status good) that
forage cannot — so that a colonist forages to survive yet still buys bread for a want
forage does not touch, preserving the bread-for-SALT trade that monetizes SALT. That is a
model change (new want dimension), deliberately **not** attempted here: re-minting food
or making raw grain edible would force a hollow pass.

## Consequence for S13

S13 (mortality selection) is **not** unblocked by a "fed AND money" provisioned colony,
because none exists under the one-scalar model. It is, however, unblocked in the narrower
sense the milestone cared about: the tail now has a real labor-based survival path
(forage), so enabling starvation on the **own-labor** path would select against genuinely
bad decisions rather than an artificial provisioning gap — at the cost of money
emergence, which the differentiated-food-quality follow-on must restore first.
