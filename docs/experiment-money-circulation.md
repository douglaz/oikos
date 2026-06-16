# Experiment: why the long-horizon colony stops producing

A diagnostic investigation (not a milestone) into why `frontier` reaches a
living-but-degenerate state over long horizons: after the money-emergence demo
(~tick 30) the grain→flour→bread chain stops producing, grain piles up
unprocessed, and the population only survives because the demo disables
starvation. This note records what was tested, what was refuted, and the
verified root cause. All probes are **additive and game-only**; the six econ
conformance goldens stay byte-identical (`econ/` untouched).

## Tooling added

- `SettlementConfig::frontier_probe(scale)` — `frontier` with the productive
  bundle (food supply, gatherers, throughput) scaled, under constant generous
  demographic headroom. Scenarios `progress-probe-{1x,2x,4x}`.
- `SettlementConfig::frontier_millisats(precision)` — `frontier` redenominated
  into a `precision`×-finer money unit (the Lightning-millisat idea: same real
  economy, many more money units). Scenarios `millisats-1x`, `millisats`.
- `Settlement::gold_by_vocation()` — read-only diagnostic: living-colonist gold
  grouped by vocation.
- `sim/tests/producer_working_capital.rs` — the verification test.

## Hypothesis 1 — money concentration (REFUTED)

Initial read: a fixed, finite money good (SALT) plus time-preference selection
lets the patient lineage hoard the entire money supply; with all liquidity in
one non-spending hoard, exchange collapses. At `frontier` scale the patient
lineage does end holding 100% of the 320 SALT units.

**Test:** `millisats` — redenominate into a 1000×-finer unit (320,000 units,
same real economy). If concentration were the cause, finer money would
de-concentrate and restart the economy.

**Result:** millisats *did* de-concentrate (money split ~evenly across
lineages instead of 100% to one) and *did* un-stick the price level (bread
price rises smoothly instead of flatlining at the integer floor of 1). **But
production still stalled identically** — `bread.made → 0` after ~tick 37 in
both the coarse and the fine run. Concentration and the integer price floor
were symptoms, not the cause. (A measurement correction surfaced here too: the
earlier "money stops circulating" reading leaned on `salt.xfer`, which is zero
post-promotion *by construction* — SALT is converted to `gold` and dropped from
tracked goods. It never measured money circulation.)

## Hypothesis 2 — producer working-capital starvation (VERIFIED)

The tell in the millisats run: bread price climbs to ~1278 (strong unmet
demand) while production is zero and grain keeps arriving unprocessed. That
points at the producers, not the money.

**Test:** `gold_by_vocation()` over a millisats run (money de-concentrated, so
any starvation is a flow problem, not a corner-the-supply artifact).

**Result (verified, `producers_cash_starve_while_savers_accumulate`):** the
chain producers (Miller/Baker) hold **~0 gold for the entire run** (exactly 0
from t≈200). The money — **~99.9% of it** — sits with the **final consumers**
(≈319,838 of 320,000), who barely spend (≈160 units leave their purses in 250
ticks). It is not the savers/gatherers either (≈160).

## Root cause: a circular-flow cold-start deadlock

- Consumers hold the money and *want* bread (they bid its price up), but no
  bread is produced to sell them, so their money never flows upstream.
- A producer can only buy inputs with cash it already holds, and it holds none.
  The chain ran only while *seeded* input buffers lasted (~tick 37), gave the
  producers a trickle, then — with the seed gone — they had no cash to rebuy
  inputs, so they could not produce, could not sell, and never earned cash.
  Self-locking.
- The money is stuck at the *wrong end* of the production structure.

## Praxeological reading and the implied fix

This is the missing **capitalist advance / working capital**. In Austrian
capital theory (Böhm-Bawerk, Mises) the entrepreneur advances present goods —
out of **real savings** — to the factors of production *ahead* of selling the
output, recouping from future revenue. The model has no such advancing
mechanism, so idle final-consumer savings never capitalize the producers.

The implied fix is therefore **neither banks/fiduciary credit nor money
divisibility** — both were refuted or shown insufficient here. It is the
genuine **savings → investment** channel: real saved balances advanced to
capitalize the production structure (the credit-*disabled* natural-rate
mechanism the engine already models), so producers can buy inputs ahead of
revenue. That is the next thing to design and test.
