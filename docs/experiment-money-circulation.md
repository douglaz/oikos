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
revenue.

## Hypothesis 3 — the pure-consumer class causes it (REFUTED, but illuminating)

A pure-consumer class (agents that hold money, eat, and never produce) is
unrealistic for a subsistence age, and it segregates the money from the
producers. Hypothesis: remove it — make everyone a self-provisioning gatherer
who holds money — and the deadlock dissolves because money sits with the
producers.

**Test:** `no-consumers` (`frontier_no_consumers`) — fold the consumers into
the gathering labor force and move the SALT endowment onto the gatherers,
total supply preserved. The only controlled change vs `frontier` is who holds
the money.

**Result (refuted):** removing the consumers **broke money emergence**. The
run never leaves barter (`money = —`, era stalls at `barter`), the chain never
forms (the latent producers adopt only on a realized *money* spread that never
appears), and hunger still climbs to ~8. The consumers were **load-bearing for
monetization** — the SALT-rich, goods-poor saleability hub that lets SALT win
as money (game-spec's Mengerian emergence). And the colony still starves for
the deeper reason: as all-gatherers they harvest *grain* (inedible) and the
only path to the staple (bread) is the fragile chain — there is **no
directly-edible base food, no subsistence floor.** "Everyone produces" does not
help when what they produce cannot feed them.

## Synthesis: the robust ingredient is a directly-edible subsistence base

| Scenario | Money emerges | Chain runs | Long-run | Why |
| --- | --- | --- | --- | --- |
| `viable` (edible food, self-provision) | n/a (direct) | n/a | healthy | directly-edible subsistence base |
| `frontier` (chain, pure consumers) | yes | stalls ~t37 | chronic hunger | no subsistence fallback; producers cash-starve |
| `millisats` (finer money) | yes | stalls | chronic hunger | divisibility fixed money, not production |
| `no-consumers` (money to gatherers) | no | never | chronic hunger | consumers were the saleability hub; still no edible base |

`viable` is the only robust colony, and the only one with a directly-edible
subsistence base agents can fall back to. The fix is to make the grain→bread
chain **optional specialization on top of** a subsistence floor (survival-driven
gathering of edible food), not the sole food source. That is the next thing to
design and test — and it is also where the genuine savings → investment channel
(above) belongs: real savings capitalize the *specialized* structure, while the
subsistence base keeps anyone from starving when it fails.

## Experiment 4 — the subsistence floor (built; reveals a tradeoff)

A real subsistence floor was added as an additive, game-only feature
(`KnownGoods::subsistence`, `ChainConfig::subsistence_on_grain`,
`SettlementConfig::frontier_subsistence`, scenario `subsistence`): raw **grain**
— already over-gathered and piling up — becomes a directly-edible food ranked
just below bread. A colonist prefers bread but eats raw grain to survive, so the
grain→flour→bread chain is optional specialization on top of a subsistence base.
`None` by default, so every existing scenario stays byte-identical (verified:
full suite green, econ goldens byte-identical).

**Result (mechanically works; economically a tradeoff):**

- The fallback fires — `grain.eaten > 0` and hunger improves from frontier's
  chronic ~8 toward ~6.2. Nobody is one chain-stall from death.
- **But money no longer emerges** (`money = —`, stuck in `barter`) and the chain
  never forms. Easy subsistence **crowds out the bread trade that monetized
  SALT**: money emerges from the *extent of exchange* (Menger), so cheap
  self-sufficiency suppresses it. Subsistence and specialization trade off.
- Hunger settles at ~6.2, not the well-fed ~1.5 of `viable` — agents under-eat
  the abundant grain, a want-ranking/access tuning issue (the fallback is real
  but not yet fully exploited).

**Reading:** this is the central tension of the primitive→advanced arc, now
visible in the model. `frontier` got specialization only by *forcing* it (no
subsistence → fragile, deadlocks). A subsistence floor removes the fragility but,
if too cheap, removes the gains from trade that drive money and specialization
(stable but primitive barter). The civ-builder must **balance** the two: a floor
that prevents starvation while bread/specialization stays attractive enough to
sustain trade, monetization, and — capitalized by genuine savings → investment —
the roundabout structure. That balance (and fixing the grain under-eating) is the
next tuning step.

## Experiment 5 — the capital advance (Codex's test; an unrepaid subsidy backfires)

Codex's recommended falsification of the producer-working-capital thesis: from
`frontier_millisats` (so money concentration and the integer price floor are not
confounds), add a conserved capital-advance phase
(`ChainConfig::capital_advance`, `frontier_capital_advance`, scenario
`capital-advance`): after money emerges, top up cashless active producers from
the richest saver so they can buy inputs. Prediction: if missing working capital
is the binding cause, the chain keeps producing past the seed-exhaustion tick.

**A tooling gap surfaced first:** every conserved gold primitive
(`transfer_gold` / `credit_gold` / `debit_gold`) gates on
`uses_closed_gold_money()` (which requires a *designated* GOLD regime), so they
silently refuse on an *emergent* money even though the money lives in
`Agent.gold` post-promotion. The first run was a no-op (byte-identical to
`millisats`) — a false negative caught only by instrumenting the phase. The sim
now moves `Agent.gold` directly for the emergent regime (reservation guard
honored by capping at the donor's free gold; no `money_system` cache exists
there).

**Result with the transfer working — REFUTED, and counterproductive:** funding
the producers did not restart the chain; it *suppressed* it. Bread produced over
300 ticks collapsed from **585** (`millisats`) to **9** (`capital-advance`); the
latent producers stayed idle, the era stalled at `money` (never reached
`capital`), hunger climbed to ~8. Locked by
`unrepaid_capital_advance_is_counterproductive`.

**Mechanism (a clean praxeological point):** in an ordinal-value model, an actor
produces to satisfy an **unmet future-money want**. Handing a producer money
*satisfies* that want, so the role-choice appraisal
(`recipe_adoption_pays_for_money`, which adopts a role only when running the
recipe newly provisions an unmet money want) never adopts — or de-adopts — the
producer. **The subsidy removes the motive to produce.** This is exactly why the
faithful mechanism is a funded **loan with repayment**, not a gift: a loan keeps
the want unmet (the producer must still earn to repay), so it supplies working
capital *without* destroying the incentive. The no-repayment shortcut is what
backfired. It also confirms Codex's fallback prediction: with funded producers
still not producing, the culprit is the **role-choice gating** — production
follows the money spread / an unmet money want, not survival or physical demand.

**Next:** a capital advance *with a repayment obligation* (a real conserved
loan), so the producer's money want stays unmet and the working capital actually
funds production. If that still fails, the role-choice motivation itself
(profit-only, no survival/demand-driven production) is the thing to redesign.

## Experiment 6 — the revolving loan with repayment (the fix that works, to a point)

The `capital-advance` scenario now implements the faithful mechanism (replacing
the unrepaid gift): a **revolving working-capital loan**. Each enabled tick,
before the market, a cashless active producer borrows up to a small floor from
the richest saver (`run_capital_advance`, recorded in a `borrower -> (lender,
owed)` ledger); after the market, it repays from its sales
(`run_capital_repayment`). The producer stays **cash-light**, so its future-money
want stays UNMET and role-choice keeps it adopted — the loan supplies working
capital *without* the gift's de-adoption. Conserved (gold moves both ways; the
ledger is bookkeeping); a tooling helper `move_money_conserved` handles the
emergent regime (`transfer_gold` refuses there).

**Result — the loan fixes what it targets, and beats every prior variant:**

| Variant | bread.made (300t) | producers | healthy window |
| --- | --- | --- | --- |
| `millisats` (no advance) | 585 | stall ~t37 | ~37 ticks |
| unrepaid gift, floor 100 | 9 | de-adopt | none |
| unrepaid gift, floor 5 | 624 | de-adopt ~t75 | ~50 ticks |
| **revolving loan, floor 20** | **732** | **stay adopted (to t700+)** | **~300 ticks** |

The loan keeps producers **in role for the whole run** (the de-adoption fix) and
unblocks the cold start, raising total production to the most of any variant and
extending the well-fed window from ~37 to ~300 ticks. Locked by
`repaid_capital_advance_sustains_roles_and_raises_production`. Conservation holds
throughout.

**But it reveals the next layer.** Over a long horizon, production still halts
(~tick 300): producers stay *adopted* (`mill=3, bake=3` to t700) and funded, yet
make no bread, and hunger climbs to ~8. So working capital was a **real binding
constraint** — now removed — but not the only one. Funded, in-role producers that
nonetheless stop producing point downstream: a **demand/market-coordination**
limit (why hungry consumers holding money don't sustain bread purchases), not a
supply-of-working-capital one.

**Next:** investigate the downstream halt — why funded, adopted producers stop
selling/producing while the colony is hungry and the buyers hold money. Candidate
culprits (per the Codex analysis): the role/production decision still being
gated on a realized money spread rather than standing demand, market-matching or
quote-timing on the one-unit machinery, or a stockpile/again-satiation dynamic on
the consumer side.

## Experiment 7 — the stock/gold trace at the halt (the economy bifurcates)

A per-vocation **stock/gold** trace across the halt
(`stock_and_gold_trace_at_the_halt`, via the new `Settlement::stock_by_vocation`)
shows what is *held* at the halt — strong evidence for hoarding + input
starvation, though (per the Codex review) it does **not** log live bids/asks, so
it cannot by itself distinguish "miller posts no grain bid" from "miller's bid
doesn't fill." At t240–350 the colony has **bifurcated**:

| Class | grain | flour | bread | gold | state |
| --- | --- | --- | --- | --- | --- |
| Consumers | 0 | 0 | **3.4k → 5.0k (growing)** | **~319,800 (≈all)** | satiated, well-fed, withdrawn |
| Gatherers | **34k → 50k (growing)** | 0 | 0 | ~200 | hungry, hold unsold grain |
| Millers | 0 | 230 (idle) | 0 | 0 | hungry, no grain to mill |
| Bakers | 0 | 0 | 0 | 0 | hungry, no flour to bake |

The chain dies of **input starvation**: grain piles with the gatherers and never
reaches the millers (`grain.input = 0`). The deeper cause is a **distribution
seizure**, not a working-capital one:

- The **consumers hold the output *and* the money** and are satiated, so they
  withdraw — they don't buy more and don't recirculate. Nothing (no spoilage,
  storage cost, tax, or rent) forces their bread/money hoard back into the market.
- Grain has **no effective buyer**: the only would-be buyers are the millers, but
  the millers are *hungry* (hold 0 bread), so their **present bread want outranks
  their grain-input want** — they chase bread (which the consumers hoard and won't
  sell) instead of buying grain. The consumers who *do* hold money don't eat grain.
- So the hungry producer class can never re-enter: too poor and too hungry to buy
  inputs, and locked out of the hoarded output.

This is consistent with the root that surfaced at every layer — **a bounded
savings want + zero carrying cost ⇒ satiated agents withdraw holding goods +
money**: the satiated *consumers* hold the bread and the money, and the producers
starve with unsold grain piling beside them. (Strong evidence, not a live-order
proof — see the Experiment-8 caveats.) Locked by the halt signature (gatherer
grain large, miller grain zero, consumers hoard bread).

**Next (the real redesign, per the Codex read):** give the model the
counter-pressures real economies have — inventory carrying cost / spoilage so a
satiated holder still offers surplus; a subsistence/household path so the hungry
producers feed themselves and stay solvent; and entrepreneurial production keyed
to *live unmet demand* (standing bids) rather than only a realized spread. The
loan fixed the supply of working capital; the remaining problem is that a
satiated, frictionless hoard never recirculates — a demand/turnover problem, not
a credit one.

## Experiment 8 — inventory carrying cost / spoilage (Codex's primary fix; partial)

Codex's primary counter-pressure for the distribution seizure: make the staple
**perish** so a satiated agent can't hoard its way out of the market. Built as
an additive, game-only, conserved sink: `ChainConfig::perishable_decay_bps`, a
`run_spoilage` phase, an `EconTickReport::spoiled` term added to the conservation
identity (`after = before + sources − consumed − … − spoiled`), and
`frontier_spoilage` / the `spoilage` scenario. Conservation holds every tick
(verified).

**Two findings — the *shape* of the carrying cost is decisive:**

1. **Flat percentage spoilage backfires.** Decaying a flat fraction of *all*
   holdings collapsed production to 33 bread/800 at every rate (2%–20%) — it rots
   the bakers' fresh output and the bootstrap seed buffers along with the hoard.
2. **Threshold spoilage (decay only holdings *above* a free-storage floor)
   works — partially.** With working stock and fresh output under the floor and
   exempt, spoilage curbs only true hoards. Result: total production rises to
   **951 bread/800 — the most of any variant** (vs 732 capital-advance, 585
   millisats), and the well-fed window is healthier (hunger ~1.1 through t~100).
   Locked by `threshold_spoilage_raises_production_and_conserves`.

**But it does not achieve sustained production.** The halt still returns
~tick 150–200 and hunger climbs to ~8. `grain.input → 0` while grain is
available, so the millers are not getting grain. Adding grain to the spoiled set
made no difference (951, identical halt).

### Caveats (Codex review — claims downgraded)

A code-grounded review flagged that several earlier conclusions outran the
evidence. Corrected:

- The residual blocker is on the **producer/input side**, but it is **not yet
  isolated** to "value-scale ordering." The ranking is real (present `bread-now`
  outranks the producer's `grain-input` want — pinned by a g3a test), **but**
  `reservation_bid_for_money` (econ/src/agent.rs) does *not* simply suppress the
  grain bid because of an unsatisfied higher bread want — it protects only
  *already-provisioned* higher wants and money above the target. So "hungry miller
  ⇒ no grain bid" is **not proven**; bid-suppression from the ranking is an
  unconfirmed hypothesis.
- The Experiment-7 trace is a **stock/gold trace**, not a live order-book trace.
  It shows millers hold no grain, but cannot distinguish "miller posts no grain
  bid" from "miller's bid does not fill" — and that distinction changes the fix.
- "Grain spoilage made no difference ⇒ grain hoarding isn't the blocker" is too
  strong: this spoilage is an **ex-post** sink, so it never enters a gatherer's
  reservation-*ask* decision; it does not test whether *anticipated* storage cost
  would make gatherers sell.
- "Working capital is solved" means **money** working capital. The loan advances
  money, not the present goods (food/inputs) a worker needs — **in-kind**
  maintenance/input advance is not addressed.

### What's actually next (per the review)

**First build a live order-book / reservation trace** around the halt (does the
miller post a grain bid? does the gatherer post a grain ask? do they cross?) to
identify *which* gate it is, before any behavioral fix. Then, almost certainly,
the faithful fix is **institutional and in-kind**: a saver/entrepreneur advances
*subsistence and/or inputs in kind* to active producers, bears risk, and takes
repayment or an output claim after sale — keeping each worker's own value-scale
ordering intact (do **not** make a starving miller prefer grain-for-production
over bread; that would be fake entrepreneurship by value-scale surgery).
Falsifiable success: `grain.input > 0` past tick 300, bread production through
tick 800, hunger materially below the ~8 plateau, producers still adopted,
conservation intact, no fiat/fiduciary issuance.

## Experiment 9 — the live order-book trace (the gate is producer-side, proven)

Built the instrument the review asked for: `Settlement::order_stats_by_vocation`
reconstructs each living colonist's live BID/ASK intent for a good from its pure
`reservation_bid_for_money` / `reservation_ask_for_money` (the same functions the
market uses), grouped by vocation, with best bid/ask. The `live_order_trace_at_
the_halt` test reads the grain book across the halt. This distinguishes the four
candidate gates Codex named.

**Result — case #1, the producer-side gate, confirmed:**

- At the halt, **gatherers post grain ASKS** (4 askers @ price 1) — the seller
  side works.
- **No vocation posts a grain BID** (`bidders = 0` everywhere) — including
  market-time instrumentation (snapshot right after the loan tops the millers
  up): a **loan-funded miller still posts no grain bid.**

So grain has sellers and **no buyers**: the would-be buyer (the miller) doesn't
bid even when funded, and the money-holding consumers don't want grain. Grain
never trades → `grain.input = 0` → the chain is input-starved. Locked by the halt
signature (grain askers > 0, grain bidders = 0).

**Why the funded miller posts no grain bid** — and it vindicates the *mechanism*
I'd earlier asserted without proof: `reservation_bid_for_money` returns `None`
because the miller's gold is **reserved for its higher-ranked own bread want**
(`allocated_money_before_rank` protects money earmarked for wants above the
grain-input rank). The hungry miller mentally holds its cash for bread it can't
get, so it never bids for grain. This is individually rational — the fix is NOT
to reorder the scale (fake entrepreneurship) but to remove the hunger: feed the
producer.

**This rules out the money-only advance** (a funded miller still doesn't bid) and
confirms the **in-kind advance** as the faithful fix: a saver/entrepreneur
advances *subsistence and/or inputs in kind* to active producers (bearing risk,
taking repayment / an output claim), so the producer's bread want is met, its
money is freed, and it bids for grain — or, equivalently, the inputs are placed
directly in its hands. Falsifiable as above (`grain.input > 0` past t300, bread
through t800, hunger ≪ 8, conserved, no fiat). That is the next build.

## Experiment 10 — the in-kind subsistence advance (the first non-starving colony)

Built the faithful fix: `ChainConfig::subsistence_advance` + a `run_subsistence_
advance` phase (before the market) that feeds each hungry active producer up to a
small staple floor by transferring staple food **in kind** from the richest
food-holder (a saver, keeping its own floor). Conserved (food moves
holder→producer, then is eaten — a transfer, not a mint). `frontier_in_kind` /
the `in-kind-advance` scenario combine it with the revolving loan. Locked by
`in_kind_advance_feeds_producers_and_conserves`.

**Result — a major welfare win, a partial production result:**

- **Hunger collapses from ~8 to ~2.7 and stays there through tick 1600**
  (conserved throughout). This is the **first long-horizon colony that does not
  starve** — Codex's "hunger materially below 8" criterion, met and sustained.
  The mechanism: the otherwise-starving producers (not in lineages, so they get
  no household provision) are fed in kind from the provisioned lineages, so the
  whole colony stays fed.
- **But the production chain still does not self-sustain.** `bread.made` is ~0 in
  the tail (total 546, *below* capital-advance's 732); `grain.input → 0`. The
  colony is fed by *sharing the lineages' subsistence provision*, not by reviving
  the grain→bread market chain.

**Why feeding didn't restart production** — the blocker moved one rank up. Feeding
provisions the producer's *bread* want, freeing its money — but a fed producer's
money is then reserved for its still-unmet **savings (future-money) want**, which
also outranks the grain-input want. So the funded, fed miller *still* doesn't bid
for grain. The grain-input want simply sits below both present consumption **and**
savings on the value scale, so a producer with limited money never reaches it.

**Where this leaves the arc.** Welfare is solved: a sustainable, non-starving
colony (the in-kind advance). Working capital is solved (the loan). The
demand-hoard is curbable (threshold carrying cost). The one remaining problem is
a **self-sustaining production chain** — gated by the grain-input want ranking
below both consumption and savings. The faithful next step is **not** scale
surgery (don't make a saver prefer grain over its own savings) but the *other*
in-kind form Codex named: **advance the inputs themselves in kind** (a
capitalist buys grain with real saved money and places it in the producer's
hands, taking an output claim), so production never depends on the producer
out-ranking its own savings to buy inputs. That, or accept the fed-but-
redistributive colony as the playable baseline and build the production economy
on the finance stack (banks/credit channeling savings into capitalist input
purchase) — the G8 machinery, now with a concrete job to do.

## Experiment 11 — the in-kind INPUT advance (refuted: the satiation wall returns)

Built the other in-kind form: `ChainConfig::input_advance` + a `run_input_advance`
phase (before production) where a capitalist (richest money-holder) **buys each
active producer's recipe input in kind** from the holder with the most of it
(grain for millers from gatherers, flour for bakers from millers), paying the
seller real money and placing the input in the producer's hands. Conserved (money
cap→seller, input seller→producer). `frontier_input_advance` / the `input-advance`
scenario stack it on the in-kind food colony. Locked by
`input_advance_conserves_but_triggers_satiation_de_adoption`.

**Result — refuted, and worse than in-kind food alone.** Production bursts early
(bread.made 9 at t30–40) but the producers then **de-adopt** (`mill 3→0,
bake 3→0` by t100–200), the era falls back to `money`, and hunger climbs to ~8.
Total bread 555 — *below* even the in-kind food colony, which stayed well-fed.

**Why — the bounded-savings satiation wall, again.** Placing inputs lets the
producer make and *sell* output, so it **earns**; its capped savings want
(`MAX_SAVE_UNITS`) fills; role-choice then sees no unmet money want and
de-adopts it. The revolving loan had avoided this only by sweeping producers
cash-light (which capped production low); boosting production via input placement
re-triggers the satiation. So the two pull against each other: keep producers
cash-light → they stay adopted but barely produce; let them earn → they satiate
and quit.

**Why — the bounded-savings satiation, again.** Placing inputs lets the producer
make and *sell* output, so it **earns**; its capped savings want
(`MAX_SAVE_UNITS`) fills; role-choice then sees no unmet money want and
de-adopts it. The revolving loan had avoided this only by sweeping producers
cash-light (which capped production low); boosting production via input placement
re-triggers the satiation.

### Caveats (Codex review — "needs firms" retracted)

A code-grounded review flagged my Exp 10–11 conclusions as overstated. Corrected:

- **"The faithful fix is a firm/employment structure" — retracted.** "Society
  needs companies to sustain production" is the same kind of overclaim as the
  earlier "needs banks." The de-adoption is an artifact of **three modeling
  choices**, not an economic necessity: (a) savings is **bounded/satiable**
  (`MAX_SAVE_UNITS = 60`) — but real consumption *recurs*, so an artisan never
  permanently retires; (b) role adoption is keyed only to a new *future-money*
  want (`recipe_adoption_pays_for_money` → `bundle.rs`), not to recurring
  consumption or standing demand; (c) the producer pool is **fixed** — a hungry
  gatherer/consumer never becomes the replacement baker.
- **"Once satiated, retires forever" — too strong.** The code permits
  re-adoption (spend savings → reopen the want → re-adopt). Exp 11 only *looked*
  permanent because producers earned, savings filled, food was advanced without
  draining their money, and no new artisan could enter.
- **"Any mechanism that makes a producer earn makes it retire" — only shown for
  this bounded-savings / fixed-pool / future-money-keyed setup.**
- **"Welfare is solved" → producer *feeding* is solved in that scenario.** Exp 10
  is a valid **subsistence** result, not a solved production economy.
- **"Input advance refuted" → refuted *as implemented*, not as an institution.**

### The bigger reframe: the goal was over-specified

The most important correction: a self-sustaining grain→bread *market chain* is
likely the **wrong target for a primitive colony**. A stable subsistence /
household base *should* precede market specialization; the chain should matter
only once bread carries a real premium, there is genuine surplus, and the
division of labor beats household subsistence. So **Experiment 10 (the fed
subsistence colony) is not a failure — it is the correct stone-age baseline**, and
the production chain is a later-era feature, not the primitive default.

### The faithful fix (when specialization *is* warranted) — no firms

A **recurring owner-operator motive**, keeping self-employment: a producer adopts
if the recipe's expected net proceeds can fund its *next-period* subsistence need
(live prices), not only if it fills a new savings want — plus **replacement /
re-entry** so a hungry colonist can take up an unserved trade. Falsifiable:
`grain.input > 0` past t300, bread through t800, hunger ≪ 8, producers stay
adopted **or churn with replacements**, conserved, no fiat. (Raising the savings
cap merely *hides* the retirement wall — not the real fix.) The current best
playable baseline remains the in-kind subsistence colony (Experiment 10):
sustainably fed and conserved.

## Experiment 12 — the recurring motive: the arc sustains (no firms)

The faithful self-employment fix, built: `ChainConfig::recurring_motive` +
`recipe_is_profitable`. With it, `run_role_choice` keeps a producer adopted while
the recipe is simply **profitable** at realized prices (`output·qty > input·qty +
operating_cost`), not only while it newly provisions a one-off future-money
savings want. That is the recurring owner-operator motive — a real artisan keeps
producing because consumption **recurs** — and it removes the satiation/retirement
that collapsed every prior variant. No firms, no value-scale surgery.

Composed with the fed subsistence base and the in-kind capital advances
(`frontier_economy` / the `economy` scenario: loan + food + inputs in kind +
recurring motive), **the whole subsistence→specialization arc finally sustains:**

| Metric | prior best | `economy` |
| --- | --- | --- |
| bread.made / 800 | 951 (spoilage) | **7011** |
| tail production (t400–800) | ~0 | **9 bread/tick, steady** |
| producers adopted | de-adopt | **mill 3 / bake 3, to t1600** |
| era | falls back | **holds `capital`** |
| hunger | climbs to ~8 | **~2.7 (early), ~6 (late)** |
| conserved | yes | **yes, every tick** |

Production runs continuously at full chain throughput (9 bread/tick, 3 grain/tick)
through tick 1600, producers stay adopted, era holds `capital`, conserved.
Locked by `scaffolded_economy_sustains_production`.

### Ablations (Codex review): the result is SCAFFOLDED, not endogenous

A skeptical Codex review forced the honest test — strip the supports and see what
holds. The ablations are decisive:

- **`economy` minus only `input_advance`** (loan + food + recurring motive, but
  producers must *buy* inputs at market): bread/800 **collapses 7011 → 546**,
  production dies ~tick 150. So the 9 bread/tick was **almost entirely the
  scripted input placement** (`run_input_advance` puts grain/flour directly in
  producers' hands each tick), not market coordination.
- **`recurring_only`** (recurring motive alone, no curated advances): bread/800 =
  585, also collapses ~tick 150, hunger → 8. Producers stay *adopted*
  (mill 3/bake 3) but produce nothing — they still cannot acquire inputs through
  the market (the Experiment-9 bid gate persists).

Locked by `economy_collapses_without_input_advance`.

### Claims retracted (Codex review)

- **"Self-sustaining specialized economy" / "grown from a subsistence base" —
  overclaimed.** The honest claim: *with curated money, food, and input advances
  plus recurring role retention, the configured chain can be kept producing at
  full throughput for long horizons while conserving.* The economy is **sustained
  under scaffolding**, not self-organizing.
- **"Closes the hunt" — false (another premature "settled").** What is shown: the
  scaffold prevents the old halt. What is NOT shown: specialization emerging and
  sustaining **endogenously** from prices, surplus, and agent choice — there is
  still **no configuration that sustains the chain without curated advances.**
- **"Recurring motive = the faithful self-employment fix" → a profitability-
  retention heuristic**, and a scalar one *outside* the ordinal appraisal: it
  reads `recipe_is_profitable` on `realized_price` (the *last* trade price, which
  can go stale and keep a role alive), and treats a missing input price as zero
  cost. Plausible heuristic, not a clean praxeological resolution.
- **"Hunger drift is throughput/scaling" — unproven hypothesis**, not established
  (could be managed decline; needs tail diagnostics — population trend,
  per-capita food, donor stocks, would-starvation-occur).

### The real remaining problem (re-opened)

The core is still unsolved and unchanged from Experiment 9: **producers do not
acquire inputs through the market.** The in-kind input advance bypasses this with
a planner-like allocator; remove it and the chain dies. Endogenous emergence
requires producers to actually *buy* inputs — still blocked by their money being
reserved for higher-ranked consumption/savings wants over the grain-input want.

**Next (the genuine test):** make specialization emerge and sustain **without the
curated advances** — e.g. let producers acquire inputs by market trade (the input
want must be reachable), specialization triggered by real surplus + price
signals, with a producer pool that scales/churns. Falsifiable: money emerges,
producers adopt, `grain.input > 0` and `flour.input > 0` past t300 **via market
trades (no transfer phase)**, bread sustained through t800, hunger bounded (not
drifting), conserved. Until that passes, the honest status is: *a sustainable
subsistence colony (Experiment 10), plus a chain that can be hand-fed
indefinitely (Experiment 12) — but not yet a self-organizing specialized
economy.*

## Experiment 13 — project-aware input bids (faithful mechanism; does NOT yet sustain)

Built Codex's specced endogenous fix: `ChainConfig::project_input_bids` +
`run_project_input_bids`. An active producer **buys** one unit of its recipe input
through a real market trade, with its **own** money, at a price *imputed* from the
output (Menger: the input's value is the output's realized value minus the
operating cost), matched against the cheapest *willing* seller at that seller's
own ask. Unlike the curated `input_advance` (a planner *places* inputs), the
producer pays from its own purse — endogenous acquisition, conserved, voluntary on
both sides. `frontier_endogenous` / the `endogenous` scenario combines it with the
loan and the recurring motive, with **no** curated food/input placement.

**Result — honest negative.** It does not yet self-organize: bread/800 = 666 (vs
the curated scaffold's 7011), production collapses ~tick 150, tail `grain.input =
flour.input = 0`, hunger → ~8 — only marginally above the no-mechanism ablations
(546 / 585). The producer does buy *some* inputs early via the market (the
mechanism works and conserves), but the chain still dies. Locked by
`endogenous_input_bids_conserve_but_do_not_yet_sustain`.

**Why it's hard (the tangle).** The producer needs working capital on hand to buy
inputs, but the revolving loan's repayment sweeps it cash-light; moving the bid
before the market (so it spends fresh loan gold) instead broke the miller
cold-start (no flour price → no adoption). The cold-start sequencing, the
loan/repayment timing, and the grain→flour→bread pipeline lag interact in a way
that a single per-tick project bid doesn't resolve. So **the faithful mechanism
is built and conserved, but a self-organizing specialized economy is still not
achieved** — only the curated scaffold (Experiment 12) sustains.

**Honest standing of the whole arc.** Two real results: (1) a sustainable
**subsistence** colony (Experiment 10) — the right stone-age baseline; (2) a chain
that can be **hand-fed** indefinitely (Experiment 12 scaffold). The endogenous
self-organizing transition — specialization emerging from surplus and sustaining
on market trade alone — is **not yet demonstrated**. This is genuinely milestone
territory (working-capital persistence across ticks, cold-start bootstrapping, a
scaling/churning producer pool, and likely posting the project-aware bid into the
real order book rather than a sim phase), not a one-shot experiment.
