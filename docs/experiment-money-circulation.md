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

## Experiment 7 — the market-gate trace localizes the halt (the economy bifurcates)

A per-vocation stock/gold trace across the halt (`market_gate_trace_at_the_halt`,
via the new `Settlement::stock_by_vocation`) settles it. At t240–350 the colony
has **bifurcated**:

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

This is the same root that surfaced at every layer — **a bounded savings want +
zero carrying cost ⇒ satiated agents withdraw holding goods + money** — now
localized exactly: the satiated *consumers* corner the bread and the money, and
the producers starve with unsold grain piling beside them. Locked by the halt
signature (gatherer grain large, miller grain zero, consumers hoard bread).

**Next (the real redesign, per the Codex read):** give the model the
counter-pressures real economies have — inventory carrying cost / spoilage so a
satiated holder still offers surplus; a subsistence/household path so the hungry
producers feed themselves and stay solvent; and entrepreneurial production keyed
to *live unmet demand* (standing bids) rather than only a realized spread. The
loan fixed the supply of working capital; the remaining problem is that a
satiated, frictionless hoard never recirculates — a demand/turnover problem, not
a credit one.
