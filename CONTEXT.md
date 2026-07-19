# CONTEXT — glossary

Precise domain language for OIKOS. Glossary only: definitions and the distinctions
that matter, no implementation detail, no specs.

## Producer role occupancy

An agent nominally *holding* a chain-producer vocation (Baker/Miller) — because it was
seeded in the role or inherited a producer tool and adopted it. Occupancy says nothing
about whether the chain produces anything.

## Chain function

The mill→oven→bread pipeline actually *running*: producers apply recipes, bread is
produced and clears in the market, and (via the demand signal) capacity can grow.

**Why the distinction matters:** these are independent, and conflating them misreads the
data. In the C3R.b mortal-producer sweep a cell can score "structure persists" (both
stages continuously *occupied* via inherited tools + a survival subsidy) while producing
~9 loaves in 1600 ticks (no *function*). The immortal control on the same base shows the
opposite is possible — full function (13,068 loaves) — so occupancy without function is a
real, measurable failure mode, not a boundary case. A result must state which it means.

## Chain bootstrap time

The continuous run length a functioning chain needs to reach self-sustaining bread
*clearing* — the point past which the demand signal unlocks new capacity. Distinct from a
tool's cost *payback*: the binding timescale for a mortal producer chain is bootstrap
time (can a tenure outlast the cold start?), not how long a mill takes to pay for itself.

## Era (`EraDetector` label)

A settlement-development label. **Not a monetization or chain-function signal:** a run can
sit at `Forager` while producing 13,068 loaves. Do not read chain health off the era.

## Realized price

`Society::realized_price(good)` is the **last trade's price, persisted forever** (no recency
gate; `econ/src/society.rs`). It is NOT a live/contemporaneous clearing price: if a good stops
trading, its realized price stays frozen at the last trade indefinitely. Role-choice and
producer appraisals read it raw, so a **margin computed from realized prices can be a phantom**
— e.g. a baker rejected because flour "costs 12" when 12 was frozen from an early boom and no
flour can currently clear above ~2. To reason about a good's *current* market, look at recent
**trades** (and live bid/ask limits, order ages), never the realized-price snapshot alone. This
distinction has repeatedly produced wrong diagnoses; treat realized-price arithmetic as a
starting hypothesis to confirm with trade-level evidence.
