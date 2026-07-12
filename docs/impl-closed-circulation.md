# impl-68 — DH.a: the closed circulation (demand-horizon slice 1)

Status: SPEC-READY v7 (Codex xhigh, 7 rounds: R1 ×16, R2 ×9, R3 ×6, R4 ×8, R5 ×7, R6 ×5,
R7 = SPEC-READY with 1 MAJOR + 1 NIT, both folded into this text as R7-1 and the
positive-consideration bootstrap-proof case)

Changelog v7 (R6 findings): the recount contract expanded to every tape-derived
per-window result (R6-1); variant-specific signed legs + the actor→class registry as the
only auxiliary replay input, both reducers consume the ordered tape (R6-2);
InitialHolding/A2FrontLoad disjointness defined and tested (R6-3); the two-case bootstrap
proof, 160 as inherited grid constant, CC1-or-CC2 wording, pipeline-level exclusion test
(R6-4); the two §4 readings rewritten inside the inference boundary (R6-5).

Changelog v6 (R5 findings): the bootstrap window [0,160) preregistered as
reported-but-excluded — closure is a POST-BOOTSTRAP claim, since all gold begins endowed
and the economy's first buyer must spend endowed gold (R5-1); the audit tape redefined as
a RAW upstream event stream (no origin labels) consumed by two independent reducers
(R5-2); the physical event table made mutually exclusive and exhaustive — settled spot
trade with both legs, A2 split from construction, one InitialHolding per balance, earned
provisioning removed (gold-only), committed deltas only (R5-3); the liquidation sequence
test (endowed sale → production → own sale) required at the shadow-reducer level (R5-4);
`commons_stock` spoilage acknowledged, `commons_goods_drain` defined as gross estate-seam
inflow (R5-5); the U row added to §4 (R5-6); `Gold::mul_qty` pinned (R5-7).

Changelog v5 (R4 findings): CC2 extended to physical endowed drawdown —
`endowed_physical_debits == 0` per class per window (R4-1); exhaustive physical event
table incl. spoilage, capital formation, A1 birth-stock transfer, and B-support runtime
credits classified `endowed` (R4-2); the `ClosurePhysicalEvent` audit tape and a
ledger-independent seed-3 reference shadow (R4-3); `commons_goods_drain` added to CC3 and
§2 corrected to name both commons pools (R4-4); CC1 pinned to monetary consideration via
the market's exact per-bucket payment arithmetic (R4-5); inertness comparison sequencing
corrected and the hook-placement constraint noted (R4-6); empty-grid-first check and the
class-payload convention pinned (R4-7); the §1 headline claim restated exactly (R4-8).
Milestone: DH.a — first slice of the demand-horizon frontier
Digest tag: 34 (ON-only, injective; encodes the `closed_circulation` marker, §3.5)
Oracle: `sim/tests/ignition_withdrawal.rs` — ladder re-run UNCHANGED on the new regime;
25 new experimental runs (5 arms × 5 seeds, 60 total); existing cells byte-identical
against a committed golden rendered by a canonical renderer.

Changelog v4 (R3 findings): physical-origin shadow inventory closes the
liquidation-laundering false green — CC1 earning counts only own-class-produced sale
portions, and sale proceeds of endowed-origin lots credit the ENDOWED gold bucket (R3-1);
verdict names corrected to `ConservationBroken`/`RegistryBroken` and the
authoritative-source escape hatch deleted (R3-2); the verification cell pinned to seed
VALUE 3 = `SEEDS[0]`, the first listed seed (R3-3); the inertness check pinned to a
test-only force-disable hook comparing `EconTickReport` + `canonical_bytes()` after
generation and after every tick (R3-4); the golden captured from UNMODIFIED master before
any source edit, and the renderer required to reproduce it (R3-5); leak precedence made
total for the no-CC0-failure case (R3-6).

Changelog v3 (R2 findings): total `ClosureClass` mapping over real vocations + CC0
presence defined (R2-1); provenance rules re-enumerated over the Closed-reachable gold
paths incl. earned-provisioning transfers, assertion timing scoped to post-market-batch
and end-of-tick (R2-2); CC1/CC2 pinned to settled market-purchase consideration and
event-time endowed debit (R2-3); post-econ_tick sampling, [t,t+1) event assignment,
strictly-before leak precedence, "maps to Holds" parenthetical deleted (R2-4); canonical
renderer + original-cell projection golden, landed CELLS untouched, seeds-outermost
retained (R2-5); §4 enumerates all 16 landed verdict variants exactly (R2-6);
false-green verification preregistered to Closed×NoIgnition seed 3 and the ledger-off
economic comparison made mandatory (R2-7); exact zero-provision assertion (R2-8); removed
gold corrected to 2,880 (R2-9). v2 changelog (R1 ×16) retained in git history.

## 1. Motivation and lineage

The keystone arc (C3R.a→e) closed with the finding that no finite intervention reaches a
measurable escape from the bootstrap trap because criterion v (ActiveExternalDemand) was
already dead in every window any cell reached (`ExternalDemandDiedFirst` @560, @160 in one
cell) — and the repaired gate decomposition showed the deepest form of the same fact:
`extinct=6` on every seed, no living producer household at the injection tick. The plan
(docs/review-and-replan-2026-07.md, C3R.e block) names the next frontier: *a genuinely
closed multi-class circulation — every class earns AND spends, no depleting endowments, no
mints — after which the ignition/withdrawal oracle re-runs as-is.*

DH.a builds that regime **subtractively**: no new markets, no new mechanisms, no parameter
tuning. The durable stack already contains a producer→gatherer spend leg (warmth
purchases), producer input purchases on the real order book, and an immortal earning buyer
class. What violates closure is the endowed, non-producing surround: the 44 chain
consumers and the 2 legacy lineage households with per-tick hearth provisions. DH.a
removes the surround, instruments closure exactly, and re-poses the ignition question.

**The preregistered question (stated exactly — R4-8, R5-1):** on a regime where, in
every window AFTER the bootstrap window, every class records positive own-production
sale consideration and positive market spending without endowed drawdown (monetary or
physical), what does the unchanged ignition/withdrawal ladder return? Closure is a
**post-bootstrap** claim by mathematical necessity: all gold begins construction-endowed,
so the economy's first buyer must spend endowed gold — window [0,160) is therefore
preregistered as the reported-but-excluded bootstrap window (§3.3). (The criteria do NOT
prove that the particular gold spent came exclusively from own production — the earned
bucket includes resale proceeds; they prove production income exists, spending exists,
and no endowment is being run down once the economy is underway.)

**Scope of inference (the contrast is UNPAIRED):** removing the surround also removes
2,880 gold of monetary stock (44 consumers × 60 + 4 lineage founders × 60), 48 mouths and
bidders, agent-id positions, and RNG draws (survivor renumbering + 48 fewer culture
draws). Closed-vs-durable differences therefore do NOT isolate "demand durability"
causally. Every §4 reading is a classification *of the closed regime*, phrased "consistent
with", never "confirms" or "isolates". Classify, don't tune; honest nulls first-class.

## 2. Research basis (probes 2026-07-12, corrected per R1/R2)

- **Gold is a closed stock.** No mid-run gold faucet; every inflow is a construction-time
  endowment (`starting_gold_gatherer` mod.rs:2558, `starting_gold_consumer` :2559,
  `ChainConfig::producer_gold` :1783, `HouseholdSpec::starting_gold` demography.rs:51);
  births are conserved transfers (mod.rs:15783-15784). The earned ancestry has
  `barter=None` and starts with designated GOLD (mod.rs:3956) — asserted in §3.6; no
  money-emergence machinery is in play.
- **The two commons pools (R4-4, R5-5):** `commons_gold` (mod.rs:7621) has NO debit
  path; `commons_stock` (goods; direct-commons estates mod.rs:14522 and unplaceable heir
  remainders :14663) receives estate goods and MAY SHRINK through end-of-tick spoilage
  (mod.rs:17983) — which is why `commons_goods_drain` is defined as the GROSS quantity
  routed into commons at the estate seam, independent of subsequent spoilage. Estates
  route heir-first (mod.rs:14647) with commons fallback (:14525-14540).
- **The hearth mints that survive in the earned base:** the two original lineage
  households retain `food_provision=3` (one also `wood_provision=3`), minted per tick
  (households mod.rs:3552; delivery :15547). The producer households' provisions are
  earned (C3R.c) with `wood_provision=0` in the durable stack.
- **The consumer cohort (corrected):** chain consumers are endowed
  (`starting_gold_consumer`, `chain.consumer_wood_buffer` — build path mod.rs:27381), and
  under `productive_reentry=true` (:3995, transition :18284) a hungry consumer can
  re-enter as a grain gatherer and earn. They are not provably non-earning; they are
  removed because DH.a removes *endowed scaffolding* wholesale (§3.1 rationale).
- **Demand death on the durable regime is a supply shadow:** gatherers are immortal
  (non-demography ⇒ no lifespan, mod.rs:15451-15457; starvation disabled :3640-3642) with
  recurring earned income (~87% of genuine external revenue); criterion v dies because
  the producer class goes extinct (`extinct=6`), not because buyers die or deplete.
- **All markets needed already exist:** good-generic CDA (econ/src/market.rs), live WOOD
  trade, producer input bids (`set_project_input_bid_overrides` mod.rs:17489; proven in
  sim/tests/project_aware_input_bids.rs).
- **R2 verified:** lineage removal is a config-level operation; `consumers=0` breaks no
  id-band, node-assignment, or indexing assumption; tag 34 is unused.

## 3. Design

### 3.1 The closed regime

New constructor `SettlementConfig::frontier_closed_circulation()`, derived from the
durable stack exactly as the oracle builds it (`frontier_mortal_producers_earned()` +
producer-house `wood_provision=0` + `gatherers=48`), with these edits and NOTHING else:

1. `consumers = 0`; `starting_gold_consumer = 0`; `consumer_wood_endowment = 0` (inert but
   zeroed for explicitness — all three appear in the identity test, §3.7).
2. The two legacy lineage households are removed: the demography household list contains
   ONLY the 6 mortal producer households. Their hearth mints go with them; §3.6 asserts
   all six retained household specs have `food_provision == 0 && wood_provision == 0`.
3. `closed_circulation = true` (the new default-false marker field, §3.5).

Rationale for (2): a household that produces nothing and earns nothing cannot exist in a
closed economy; the lineage surround (like the consumer cohort) is endowment scaffolding
from earlier arcs. DH.a removes scaffolding rather than converting it — conversion (wage
labor, earning consumers) is DH.b material (§7).

Resulting population: 48 gatherers (grain/WOOD, round-robin node split), 6 mortal producer
households (millers/bakers + demography).

**ClosureClass — a total, stable accounting mapping (R2-1), NOT current vocation:**
- Every non-household colonist → `ClosureClass::Gatherer` (in this regime all
  non-household colonists are gatherers; asserted).
- Every household member (founder, spouse, newborn, heir — regardless of momentary
  `Vocation`, which is `Unassigned` for seeded producers and `Consumer` for newborns,
  mod.rs:15856) → the class of its household: `Miller` or `Baker`, fixed at construction
  from the household's seeded latent recipe, immutable thereafter, inherited by all
  descendants.
- A debug assertion (marker-on) checks every living agent maps to exactly one
  ClosureClass at every window boundary.

CC0 presence (decision, preregistered): a required class is present in a window iff it
has **≥1 living member** (by the mapping above) at every post-`econ_tick` sample in the
window — living membership, not momentary producer activity; whether living members still
*trade* is exactly what CC1 measures, and conflating the two would double-count.

No change to: estate routing, the intervention-origin flag, `subsistence_on_grain`,
`productive_reentry`, prices, endowment magnitudes of retained classes, preferences,
lifespans, or any market mechanism. Grain self-feeding stays: autarkic consumption of own
product is legitimate action, not a faucet; closure means **no class funds demand from a
stock it cannot replenish by its own earning**, not "everyone buys everything".

### 3.2 The all-class gold-provenance ledger (new instrumentation, runtime-only)

The existing earned/endowed buckets cover producer-house members only (init mod.rs:19139,
attribution gate :20126). DH.a adds a **whole-population provenance ledger**, active only
when `closed_circulation=true` (ON-only; observation-only — it never alters a settlement).

Per-agent `(earned, endowed)` buckets with invariant `earned + endowed == agent.gold`.
**Assertion timing (R2-2, wording R6-2):** the invariant is debug-asserted (marker-on)
after the market batch of each tick — the ledger consumes the tick's ordered
`ClosurePhysicalEvent` tape entries (whose trade events carry buyer, seller, price, qty,
trade identity in settlement order, reconciled against the authoritative trade records) —
and again at end of tick after all non-market transfers; NOT "after every mutation"
(spot settlement happens inside `Society::step`).

**The physical-origin shadow inventory (R3-1, taxonomy R4-2).** Monetary provenance
alone is launderable: selling construction-endowed *goods* yields gold labelled "earned"
without any production. The ledger therefore also maintains, per agent per tradeable
good, a shadow decomposition of inventory into three origin buckets — `endowed`,
`own_produced`, `acquired` — observation-only, riding the real event streams, with
invariant (sum of buckets == real holding) asserted on the same schedule as the gold
invariant. Every physical debit consumes buckets in the FIXED anti-false-green order
**endowed → acquired → own_produced**, and the endowed portion of every
non-bucket-preserving debit is recorded per class per window as
`endowed_physical_debits` (feeds CC2 — R4-1).

**Exhaustive, mutually exclusive physical event table (R4-2, R5-3). Rows cover
COMMITTED semantic deltas only — never rollback intermediates. Every committed per-agent
holding delta must match exactly one row; an unmatched delta fails the invariant:**

| Event (one subtype each) | Shadow effect |
|---|---|
| `InitialHolding` — exactly one per generated per-agent per-good balance, defined as the FINAL generated holding MINUS the known A2 component (covers construction buffers incl. `bread_buffer=8`, `latent_flour_seed=12`) | credit `endowed` |
| `A2FrontLoad` — exactly the A2 component of the generated holding (disjoint from `InitialHolding` by construction; a unit test asserts the two sum ONCE to the real generated holding — R6-3) | credit `endowed` |
| `BSupportCredit` — the Closed B arms' runtime support deliveries (`food_provision=1`/`producer_subsistence=4`, ignition_withdrawal.rs:166) | credit `endowed` (support is not production) |
| `GatherDeposit` — settled gathered-node deposits | credit `own_produced` (gathering is production) |
| `SettledSpotTrade` — BOTH legs of one trade: seller stock debit (per debit order; decomposition recorded) AND buyer credit | seller debit; buyer credit `acquired` |
| `RecipeProduction` — input consumed (debit order), output credited | output → `own_produced` (purchased flour in, baker's bread out is own-produced; an ENDOWED input debit counts in `endowed_physical_debits` and fails CC2 that window) |
| `CapitalFormation` (WOOD in, mill/oven out — mod.rs:18475) | input debit per order; tool credit `own_produced` |
| `Consumption` — eating/warmth (market-phase consumption applies BEFORE the tick's ordered trades — match the real phase order) | debit per order |
| `Spoilage` (`perishable_decay_bps=1500`, mod.rs:17923) | debit per order; recorded sink (per-agent stock only; commons spoilage is NOT a per-agent event — R5-5) |
| `HouseholdTransfer` — birth transfer, A1 `transfer_birth_stock`, estate-to-heir (earned provisioning moves GOLD only, mod.rs:19339 — it does NOT appear in this table) | bucket-preserving |
| `EstateToCommons` — direct commons estates and unplaceable heir remainders | buckets removed; gross quantity recorded in `commons_goods_drain` (R4-4/R5-5) |

Each sale's debit decomposition is recorded at event time — it drives CC1 and the
gold-crediting rule 2 below. All consideration arithmetic (both reducers) calls
`Gold::mul_qty` (`Trade.price.mul_qty(bucket_qty)`, econ/src/good.rs:47) directly — one
arithmetic implementation, no independent rounding (R4-5, R5-7).

**The raw audit tape and the two reducers (R4-3, R5-2, contract R6-2):**
`ClosurePhysicalEvent` is a totally ordered RAW event stream emitted at the upstream
mutation seams. Each record carries tick/order, event subtype, and **variant-specific
signed committed legs** — e.g. `RecipeProduction { agent, input: (good, qty debited),
output: (good, qty credited) }`, `SettledSpotTrade { seller, buyer, good, qty, price,
trade_id }` — never a flattened `actors/good/qty` triple that cannot distinguish legs.
It carries **no origin buckets, no decompositions, no aggregates** — origin
classification is a REDUCER's job. There are two independent reducers — the production
shadow ledger and the seed-3 reference shadow (§3.3) — and BOTH consume the ordered tape
events. Their only shared inputs besides the tape: (a) the actor→ClosureClass registry
(total; built at construction, maintained at births; the ONLY auxiliary replay input),
(b) the raw event types themselves, and (c) `Gold::mul_qty`. They may NOT share
bucket-consumption, origin-classification, or aggregation helpers — a misclassification
in one reducer cannot propagate into the other. Market events on the tape are reconciled
against the authoritative consumption and trade logs; non-market events are emitted
atomically at their mutation seams.

**Classification rules — exhaustive over the Closed-reachable gold paths (R2-2):**
1. Construction endowment → `endowed`.
2. Spot-trade sale proceeds (from the tick's ordered trade records) → split by the
   sale's shadow decomposition (R3-1): the pro-rata portion of proceeds attributable to
   `endowed`-origin quantity credits the seller's `endowed` gold bucket (liquidation is
   not income); the `own_produced` and `acquired` portions credit `earned` (production
   income and resale/arbitrage income respectively).
3. Spot-trade purchase → buyer debits `earned` first, then `endowed` (matches the
   existing producer-house rule, mod.rs:8231-8234); the endowed portion of each purchase
   debit is recorded at event time (feeds CC2).
4. Earned-provisioning transfers (`run_earned_provisioning_transfers`, mod.rs:19325;
   producer → household member) → **bucket-preserving, non-spending**: debit the source
   earned-first, credit the recipient into the same buckets the debit consumed.
5. Birth endowment (parent→child, mod.rs:15783-15784) → bucket-preserving, non-spending
   (same rule as 4).
6. Estate-to-heir (`credit_estate_gold_to_heir`, mod.rs:14802) → bucket-preserving,
   non-spending.
7. Estate-to-commons (mod.rs:14525-14540 and the heir-path fallbacks) → the dead agent's
   buckets are removed; the amount is recorded in the window's `commons_drain` (CC3).

Any other gold-mutation family is proven UNREACHABLE in this regime by the §3.6
assertions: `m3` off, bank/cycle/tender/tax surfaces absent, land market inactive,
wage-labor mode inactive, `capital_advance`/`input_advance`/`subsistence_advance`
transfer mechanisms off. If implementation discovers a reachable path outside 1–7, that
is a spec bug and blocks landing (enumerate it in the writeup and stop).

Bucket-preserving transfers (4–6) and estate removal (7) are **excluded from CC1 outflow
and CC2 drawdown** (R2-3); commons loss belongs only to CC3.

### 3.3 Closure criteria and verdict (computed, preregistered)

**Trace:** closure is evaluated on the Closed regime's `NoIgnition` run per seed.
Intervention runs print the same per-window diagnostics for context but do NOT contribute
to the closure verdict.

**Sampling and window grid (R2-4, bootstrap R5-1):** samples are taken post-`econ_tick`,
matching the landed oracle's observation phase (ignition_withdrawal.rs:279); an event at
tick `t` belongs to window `[t, t+1)`'s containing grid window. The grid is the oracle's
existing absolute 160-tick grid, unchanged: [0,160), [160,320), … to the landed horizon.
**Window [0,160) is the preregistered BOOTSTRAP window (R5-1, proof R6-4):** its full
CC0–CC3 diagnostics are computed and printed, but it is EXCLUDED from the input to
`classify_closure`. Proof that passing [0,160) is impossible, two cases: (i) if the
window contains any POSITIVE-CONSIDERATION settled purchase, the globally FIRST such
purchase spends endowed gold (all gold begins construction-endowed; no prior sale
proceeds exist) → CC2 fails; (ii) otherwise CC1's positive-spend leg fails
(zero-price trades cannot satisfy it). Either way the window fails — so including
it would make H unreachable by construction and the classification vacuous, not honest.
The mathematics forces exclusion of the FIRST GRID CELL; its 160-tick duration is
inherited from the frozen oracle grid, not chosen. Closure is evaluated on [160,320)
onward. A PIPELINE-level test (not a `classify_closure` table test — the pure function
receives the already-filtered slice) asserts [0,160) is printed, the classifier input
begins at [160,320), and an endowed debit in any later window still fails CC2.

**Required class set (fixed):** {Gatherer, Miller, Baker} under the §3.1 mapping.

Per window (evaluation order CC0→CC1→CC2→CC3; within a criterion, lowest class ordinal):

- **CC0 (structure):** every required class present (§3.1 presence rule).
- **CC1 (earn+spend):** every required class has window **own-production sale
  consideration** > 0 — the sum over its sales of price × `own_produced` bucket-qty
  under the market's exact payment arithmetic (R4-5; endowed liquidation and resale do
  NOT qualify as earning) — AND window **settled market-purchase consideration** > 0
  (rule-3 debits only; transfers excluded — R2-3).
- **CC2 (no endowment drawdown, monetary AND physical — R4-1):** every required class
  has window sum of **endowed portions of purchase debits** (recorded at event time
  under rule 3) == 0 AND window `endowed_physical_debits` == 0 (the endowed portion of
  every non-bucket-preserving physical debit: sales, consumption, recipe/capital inputs,
  spoilage). Exact, threshold-free. Mixed-origin sequence tests are required AT THE
  SHADOW-REDUCER LEVEL (R5-4 — `classify_closure` receives completed aggregates and
  cannot test sequencing): (a) consumption/input use of endowed stock before a later
  own-produced sale; (b) the original laundering path — an agent SELLS endowed stock,
  later produces the same good, and sells again: the first sale must increment
  `endowed_physical_debits`, credit ENDOWED gold, and contribute nothing to CC1; only
  the second sale contributes own-production consideration.
- **CC3 (no drain):** window `commons_drain == 0` AND `commons_goods_drain == 0` (R4-4)
  AND `wage_escrow_gold == 0` AND `land_fee_pool_salt == 0` at every window boundary.

**Verdict — pure function** `classify_closure(&[ClosureWindow]) -> ClosureVerdict`
(empty input is checked FIRST and returns `ClosureUndeterminedNoWindow`; `ClosureHolds`
requires a non-empty grid — R4-7):
- `ClosureHolds` — non-empty grid, every window passes CC0–CC3.
- `ClosureStructureAbsent { first_window, class }` — decided by the first failing
  window when that window's failure is CC0.
- `ClosureLeaks { first_window, criterion: CC1|CC2|CC3, class: Option<ClosureClass> }` —
  the earliest CC1/CC2/CC3-failing window, when either NO CC0 failure exists anywhere in
  the grid, or that window is **strictly before** the first CC0-failing window (R3-6;
  within a single window CC0 precedes, so a window both structure-absent and leaking
  reads as CC0). Payload convention (R4-7): `class = Some(lowest failing class ordinal)`
  for CC1/CC2; `class = None` for CC3 (a global drain). Unit tests cover all three
  cases: leak with stable structure, leak before CC0 failure, leak at/after CC0 failure.
- `ClosureUndeterminedNoWindow` — empty grid (defensive; unreachable at the landed
  horizon; exists so no code path defaults to Holds).

**False-green guards (preregistered, not post-selected — R2-7):**
- The printed closure verdict is exclusively the enum returned by `classify_closure`.
- **Gold-reducer sequence tests (R7-1):** the `earned+endowed==gold` invariant proves
  conservation, not attribution — an earned/endowed swap during a purchase would
  preserve it while falsely passing CC2. Required unit tests on the gold reducer:
  purchases strictly below, exactly at, and strictly above the buyer's earned balance
  (asserting the resulting buckets AND the endowed-purchase aggregates); origin-specific
  sale credits (endowed-origin portion → endowed gold, own-produced/acquired → earned);
  and one bucket-preserving transfer.
- Table-driven unit tests cover every variant, the precedence rules (incl. same-window
  CC0+leak, leak-before-CC0, leak-after-CC0), zero-activity windows, and mid-window
  extinction.
- **Preregistered recount cell: Closed × NoIgnition × seed VALUE 3** — `SEEDS[0]`, the
  first listed seed of the landed `[3, 7, 11, 19, 23]` (R3-3; fixed now, before any
  run): the reference reducer — built SOLELY from the RAW `ClosurePhysicalEvent` tape
  plus the actor→class registry (§3.2; never calling the production ledger) — must
  byte-match the production ledger on EVERY tape-derived per-window result (R6-1):
  boundary origin inventories, sale decompositions and own-production consideration,
  purchase consideration, `endowed_physical_debits` by class and event family, and
  gross `commons_goods_drain`. Monetary gold-bucket fields stay OUTSIDE this comparison
  (no raw gold events ride the tape; their guard is the invariant + the sale-split rule
  driven by the compared decompositions).
- **Mandatory observation-inertness check, same cell (R3-4, sequencing R4-6):** via a
  test-only force-disable hook on the ledger, run TWO settlements with the marker
  enabled — hook off vs hook on — comparing `canonical_bytes()` immediately after
  generation (generation returns no `EconTickReport`), then after EVERY tick the tick's
  returned `EconTickReport` (derives `Eq`) followed by `canonical_bytes()`; the final
  digest is supplementary only. If the hook is `#[cfg(test)]`, this test lives in a
  library unit-test module (the hook is invisible to an integration-test build). Both
  checks are verification outside the 60 experimental cells; their runtime is disclosed
  separately in the writeup.

### 3.4 The oracle re-run

`sim/tests/ignition_withdrawal.rs` gains `Regime::Closed` with the full arm set the
classifier requires: Closed × {NoIgnition, BWithdrawn, BNeverWithdrawn, A1, A2} × the same
5 seeds = **25 new experimental runs, 60 total**. `support_on_control` and the
criterion-vi matched-reference lookups extend to Closed identically to the existing
regimes. The ladder, windows, eligibility (A1 strict floor at the injection tick; A2/B
none), criteria, M=5, and the A1 gate-decomposition print run **unchanged** — no criterion
added, removed, or reweighted. Closed cells additionally print the closure preamble
(per-window CC0–CC3 diagnostics + the base-trace closure verdict) BEFORE the ladder
verdict.

**Existing-cell byte identity — executable mechanism (R2-5, sequencing R3-5):**
1. **From UNMODIFIED master, before any source edit,** capture the suite's cell-only
   output projection (each existing cell's verdict line, criterion payloads, window
   reports, gate decomposition — exactly the current `println!` text, no run-count
   headers/footers) into the fixture
   `sim/tests/goldens/ignition_withdrawal_pre_dh_a.txt`.
2. The first branch commit introduces a **canonical renderer** that builds each cell's
   report as a `String` and prints it unchanged, plus the fixture from step 1 and an
   `include_str!` assertion that the renderer's existing-cell projection equals the
   fixture byte-for-byte — the renderer is validated against master's output, never
   against itself.
3. Closed cells are then appended (landed `CELLS` order, seeds-outermost loop retained,
   the 5 Closed cells in the landed intervention order); the step-2 assertion on the
   existing seven-cell projection remains in force unchanged.

### 3.5 The marker and digest tag

New field `SettlementConfig::closed_circulation: bool`, default false everywhere except
the new constructor. Runtime copies it; digest tag 34 encodes it ON-only with a fixed
encoding (tag byte 34 + the marker), injective against all existing tags. Flags-off
goldens byte-identical (default-false field; the ledger and preamble activate only under
the marker).

### 3.6 Base assertions (executable)

Unit test on the constructor asserts: `barter.is_none()`; designated money good is GOLD
from generation; wage-labor mode inactive; land market inactive; bank/cycle/tender/tax
surfaces absent; `m3` off; `capital_advance`, `input_advance`, `subsistence_advance` off;
`consumers == 0`; `starting_gold_consumer == 0`; `consumer_wood_endowment == 0`;
household list length == 6 with **every spec `food_provision == 0 && wood_provision ==
0`** (R2-8); `gatherers == 48`; `closed_circulation == true`. Runtime (already in CC3):
`wage_escrow_gold` and `land_fee_pool_salt` remain 0 at every window boundary.

### 3.7 Config-identity test

Asserts `frontier_closed_circulation()` equals the durable stack as the oracle builds it
(`frontier_mortal_producers_earned()` + producer `wood_provision=0` + `gatherers=48`) with
EXACTLY this edit list applied: `consumers=0`, `starting_gold_consumer=0`,
`consumer_wood_endowment=0`, household list reduced to the 6 producer specs,
`closed_circulation=true`. Any other field differing fails the test.

## 4. Preregistered classification space (total over the landed 16-variant enum — R2-6)

Closure verdicts: H = ClosureHolds, S = ClosureStructureAbsent, L = ClosureLeaks,
U = ClosureUndeterminedNoWindow (defensive).

The landed verdict enum (ignition_withdrawal.rs:516) has 16 variants, grouped:

- **Precondition/void variants (6):** `BaseUnviable`, `ReservoirOpen`,
  `ConservationBroken`, `RegistryBroken`, `IgnitionShortfall`,
  `SupportOnControlFails` — the cell is VOID; permitted inference is only "the
  precondition failed on the closed regime", reported with the closure verdict alongside.
  (`IgnitionShortfall` voids; it is NOT evidence about the trap. `SupportOnControlFails`
  appears both as a control-cell result and as the intervention-cell precondition it
  induces.)
- **Substantive variants (7):** `HysteresisHolds`, `IgnitionNeverIgnites`,
  `ResidualNeverExhausted`, `RegimeUntrapsWithoutIgnition`, `ExternalDemandDiedFirst`,
  `IgnitesThenRelapses`, `UnclassifiedMixed`.
- **Control-result variants (3):** `SupportOnControlViable`, `MatchedReferenceTrapped`,
  `MatchedReferenceUntrapped` — emitted by control cells (`NoIgnition`,
  `BNeverWithdrawn` are ARMS, not verdicts; their cells emit these control variants).

Permitted inference for the headline joint readings (all "consistent with", none causal):

| Joint | Reading |
|---|---|
| H × HysteresisHolds | Consistent with the demand horizon having been the binding constraint; a closed economy admits ignition. |
| H × IgnitionNeverIgnites | The trap survives a demand base that remained structurally present and closed throughout the classified horizon — consistent with an earning/allocation-side constraint. The strongest available statement of the trap. |
| H × ExternalDemandDiedFirst | Criterion v can still die with the last baker even under closure — consistent with EDD being a supply shadow. |
| S × anything | The closed regime cannot maintain its own class structure (producer extinction outruns closure) — the trap restated at the regime's foundation; the ladder reading is subordinate to S. |
| L × anything | The closure claim was wrong in a specific, instrumented way (named criterion, class, window). Honest null; the leak is the finding. |
| U × anything | No closure evaluation window was produced — a measurement failure; report it and make no substantive inference (R5-6). |
| Void cells / control variants | Reported per cell; no substantive inference. |

Every (closure verdict × ladder/control variant) combination is reportable; the table
privileges none — the writeup reports whatever obtains, per seed.

## 5. Honest nulls and budget

- The bootstrap window's guaranteed CC1-or-CC2 failure is disclosed by construction
  (§3.3 two-case proof, R6-4) — it is printed, excluded, and never counts toward or
  against closure.
- Likely outcome per prior evidence: producer extinction near tick 50 would fail CC0
  from the first CLASSIFIED window [160,320) → `ClosureStructureAbsent` on every seed.
  First-class result (S-row).
- Bread may fail to clear with 44 fewer buyers → CC1 leak; gatherer gold pools —
  first-class L result.
- Heirless producer death drains to commons → CC3 leak; first-class, praxeologically
  meaningful (the mortal economy drains its own money stock absent heirs).
- **Budget:** 60 experimental runs, landed cell ordering (seeds-outermost, landed CELLS
  order with the 5 Closed cells appended). Expected same-runner wall clock ≈115s (linear
  from 66s/35 runs) + the two seed-3 verification runs (disclosed separately); landed
  timeout convention kept; if wall clock exceeds 2× the estimate the writeup says so.

## 6. Acceptance criteria

1. Flags-off goldens byte-identical; conservation every tick; the gold-bucket AND
   shadow-inventory invariants asserted post-market-batch and end-of-tick under the
   marker (§3.2).
2. Existing seven cells byte-identical against the committed pre-change golden via the
   canonical renderer (§3.4).
3. Closed cells print: closure preamble (per-window CC0–CC3 diagnostics, base-trace
   closure verdict from `classify_closure`) then the unchanged ladder verdict; A1 cells
   include the gate decomposition.
4. Table-driven `classify_closure` tests (every variant, empty-grid-first, all three
   leak-precedence cases, same-window ties, zero-activity, mid-window extinction); the
   PIPELINE-level bootstrap-exclusion test (§3.3, R6-4); shadow-REDUCER sequence tests
   incl. the endowed-sale → production → own-sale laundering case (§3.3, R5-4); the
   GOLD-reducer sequence tests (§3.3, R7-1); the InitialHolding/A2FrontLoad
   disjointness test (§3.2, R6-3); the preregistered seed-value-3 raw-tape recount over
   every tape-derived per-window result (§3.3, R6-1); the mandatory per-tick
   observation-inertness comparison via the force-disable hook (§3.3).
5. Config-identity test (§3.7), base assertions (§3.6), and the total-ClosureClass
   mapping assertion (§3.1) green.
6. Digest tag 34 injective ON-only; `cargo fmt --check`, `cargo clippy --workspace
   --all-targets -- -D warnings`, `cargo test --workspace` green.
7. No change to estate routing, the origin flag, `subsistence_on_grain`,
   `productive_reentry`, or any retained price/endowment/preference parameter; the
   ledger/preamble are observation-only, PROVEN by criterion 4's inertness comparison.

## 7. Out of scope (future slices)

DH.b candidates, contingent on DH.a's joint classification: estate recirculation
(escheat-to-market), carrying costs on hoards, producers as wage employers (the landed
wage overlay currently admits only landowners — wage_labor.rs:136), re-introducing a
consumer class WITH an earning stream, heir-continuity reforms if S dominates. None are
prejudged; DH.a's (closure × ladder) reading selects.
