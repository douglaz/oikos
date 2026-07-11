# impl-66 — C3R.e-obs: The Allocation-Contest Instrumentation (WHY does the saving bid lose — offer scarcity, priority loss, queue starvation, or gold reservation?)

Status (spec): **SPEC-READY (v4)** — three narrowing xhigh rounds (R1: the CDA-incoherent taxonomy, the
single-site trace, the consumption-log behavior trap, physical-stock attribution; R2: the pass-start window,
the domain and shared-predicate P0s, the total tick-keyed schema, the exact oracle; R3: the intra-pass
cancellation gap → PostExitConsumption in the Residual family + QuoteExit records, SelfAskOnly's ≥1 clause,
eligibility = LITERAL C3R.d snapshot membership, bid-post-time seq-keyed intent capture with one-tick-batch
bounded retention, fully-total QuoteAttempt outcomes). All round-3 fixes are the reviewer's own
prescriptions applied verbatim; the remaining gates are the build panel and the result-review. Round 2 (P0s): the window is
now the POST-CANCELLATION PASS-START snapshot through pass end, so pre-entry consumption is IN the window
and classified `CompetitiveLoss{PreEntryOrder}` (Microstructure family, regardless of the winner's price);
`SelfAskOnly` precedes NoExecutable; the multi-winner payload pinned to the first compatible lost unit in
(limit, seq) order; the opportunity DOMAIN restricted to money-priced spot-pass ticks (`no_spot_pass_ticks`
counted); the denominator and `Filled` share the ONE authoritative C3R.d attribution predicate. (P1s): the
trace schema made tick-keyed and TOTAL (both sides, all outcomes, carried resting metadata) at BOTH
money-priced loops with the scope claim narrowed to them; the five-seed oracle pinned with exact
(births, attributable, reached_four) metrics; stale v1 text purged; the explicit 16,000-tick budget with no
duplicate runs. Round 1 NEEDS-REVISION (4 P0s,
all folded: the taxonomy rebuilt as an exhaustive precedence-ordered OPPORTUNITY-WINDOW enum honoring the
CDA's immediate crossing — including the later-ask-crosses-resting-bid path and seq-vs-limit loss
attribution via two orthogonal sub-dimensions; the trace redesigned to THREE record kinds (tick-start book
snapshot / per-quote attempts from the loop where order_pos lives / seq-keyed executions) since ensure_order
alone misses the gold-bind early return, carried resting quotes, and who-consumed-what; the consumption log
made READ-ONLY — it is already construction-enabled and behavior-driving, so re-enabling would have erased
needs-phase state; stock-fall rebuilt on PHYSICAL stock with the corrected phase order. P1s: the accounting
unit = the quote-opportunity; explicit precedence; the five-seed pinned behavioral oracle; exact majority
formulas over families with a clean single enum; the reservation-recomputation demoted to advisory with
QuoteAttempt authoritative; Slice-0 anchor corrected to the mod.rs:32229 lot-semantics pattern). A
**pure-observation** slice — the standalone,
frozen instrumentation the C3R.d result-review and the C3R.e second-opinion review both made a prerequisite:
its outcome **selects the trap-escape lever** (no offerable surplus → ignition/granary; surplus-but-losing →
set-aside/priority; deterministic queue loss → a market-microstructure artifact, not a development
institution; gold-reservation bind → a scale/want-ordering question). Build base: **master @ `be566ce`**
(the consolidated tree — the first milestone built on integrated master). Flag **`saving_allocation_obs`**
(bool on `ChainConfig`), gated `saving_allocation_obs_active() = flag && birth_stock_saving_active()` (mode
1 — the motive must be on to observe its bids). Digest **tag 32** (verified free; no `push(32)` anywhere) —
**the single tag emission (two bytes: tag + flag) is the ONLY digest delta when ON** (a testable,
load-bearing DoD: pure observation adds no wants, no transfers, no behavior).
**Slice 0 folds in the C3R.d verified test debt:** the `non_bread_*` counter assertions.

Falsifiable bar (headline): C3R.d closed on an unresolved sub-diagnosis — "no uncommitted surplus reaches
saving" could mean **offer scarcity** (nothing price-compatible for sale), **allocation priority loss**
(bread exists but higher-limit bidders win it), **microstructure loss** (price-compatible asks existed but
arrival order alone decided — a deterministic artifact), or **gold-reservation
bind** (the saving bid was never even posted because higher-ranked wants reserved the gold). These imply
*different* next levers, and one of them (microstructure loss) would partially **disconfirm the economic trap
reading** in favor of a market-implementation bottleneck. This slice measures which, per seed, with exact
pre-named diagnosis rules — and changes nothing else.

## 0. One-paragraph summary

The research (probe aec94448) classified every required observable: all but one are **post-hoc
reconstructable in the settlement/test layer with zero engine changes** — the pure predicates the quote
path itself uses are side-effect-free and callable counterfactually (`reservation_bid_for_money`,
`reservation_ask_for_money`, `shade_bid`; `free_gold_after_all_reserves` / `free_stock_after_all_reserves`
are public), the resting order book is public (`Society::books`), the trades tape is public, and stock-fall
attribution decomposes over the settlement's deterministic phase boundaries (with the existing opt-in
`enable_consumption_log` for the hunger seam). The one genuinely mid-tick fact — whether a crossable ask
existed *at the instant the saving bid posted*, and whether earlier-ordered buyers starved the queue — is
not on the tape and not in the end-of-tick book, because the market is a **continuous double auction with
immediate crossing** (price-then-arrival priority). For that, ONE bounded inert addition in econ: an
**allocation trace** (three runtime-only record kinds — a tick-start book snapshot, per-quote attempts, and
seq-keyed executions; §3.1), enabled once by the settlement iff the obs flag is on (the
`enable_consumption_log` opt-in SHAPE — an explicit enable the goldens never call; note `payment_audit` is
an M3 post-execution payment record, a runtime-only-vector precedent but not a quote-trace precedent);
never serialized. Everything aggregates into per-run
**loss-reason shares** with exact pre-named diagnosis rules that select the lever. Verdicts are printed,
never asserted; the slice is frozen (reviewed → built → read) before any lever milestone is scoped.

## 1. Base facts (verified by the probe; cites master @ be566ce)

1. **The market is a CDA with immediate crossing.** The one-quote-pass loop (`society.rs:738-746`, agent
   order × market goods, `ensure_bid`/`ensure_ask` → `ensure_order` at 3031) posts and **crosses each order
   immediately** (3070–3096; `match_bid` market.rs:370 walks resting asks in ascending `(limit, seq)`,
   executes at the resting limit). Allocation priority = price, then arrival (`(limit, seq)` /
   `(Reverse(limit), seq)` keys, market.rs:47-48). The unmatched remainder rests; the end-of-tick book holds
   only survivors.
2. **Post-hoc reconstructable (zero econ changes):** the saving bidder's reservation (`reservation_bid_for_
   money`, agent.rs:378 — pure, and it already computes "protected money" = gold reserved by higher-ranked
   wants at agent.rs:398), the shaded limit (`shade_bid`, expect.rs:36 — pure), free balances after ALL
   resting reserves (public, society.rs:6163/6172), offerable units (`reservation_ask_for_money`,
   agent.rs:419 — pure, callable counterfactually per agent), posted-vs-offerable (the public resting
   `books`, society.rs:246 / market.rs:47-50), fills (the public tape), seller class + provenance (the
   existing `BreadSellerProvenance` + `earned_buyer_class`), and the "no bid was even posted" loss reason
   (recompute the `ensure_bid` clamp, society.rs:2977-2982 — ADVISORY only; the QuoteAttempt records are the
   authoritative bid facts, since the live path evaluates an available_agent with other reserves removed
   and its own quote restored, society.rs:3149/6205).
3. **NOT post-hoc: the intra-tick trajectory** — ask-availability during the pass, pre-entry consumption,
   who consumed compatible units. Needs the §3.1 allocation trace (three tick-keyed record kinds pushed
   from the quote-pass loops and the crossing site — NOT a single ensure_order push, which would miss the
   gold-bind early return, carried resting quotes, and seq attribution), enabled once at construction (the
   `enable_consumption_log` opt-in shape, society.rs:4340; the log itself is read-only-copied for the
   hunger seam — never re-enabled/drained).
4. **Stock-fall attribution** decomposes at the settlement's deterministic phase seams — with the
   CORRECTED order (round 1): deaths run BEFORE scales/market (mod.rs:10934); consumption happens INSIDE
   `society.step` before matching (society.rs:721); production/own-use after the market, before births
   (mod.rs:11239/11292). Seams: birth debit (mod.rs:14842), hunger consumption (the ALREADY-ENABLED
   consumption log — read-only, never re-enable/clear: the needs phase depends on it, mod.rs:9684,
   13392/13405), sale (the tape), death-estate, transfers. Deltas are taken on **physical `agent.stock`**
   (free-stock confounds reservation changes: posting/cancelling an ask moves free stock without moving
   physical stock — society.rs:6172), reservations tracked separately, reconciled per phase.
5. **The inert-observation precedents to copy:** `BreadSellerProvenance` (runtime-only, documented
   "not serialized"), `last_capital_decisions` ("read-only, NOT serialized"), the C3R.d `birth_stock_*`
   counters (never serialized, public accessors, unit-tested). **The anti-pattern to avoid:** the
   `birth_block_*` counters are *conditionally serialized* under `forage_commons_active()` (mod.rs:25288) —
   this slice's counters must be purely runtime, never in the digest under any gate.
6. **Tag 32 is free** (grep-verified: no `push(32)`, nothing in `push(3[2-9])`). Tag 31 =
   `birth_stock_saving` (mod.rs:24956).
7. **Slice-0 seams exist:** the C3R.d attribution pass is unit-testable with hand-pushed tape trades (the
   landed pattern at mod.rs:32229 — note a hand-pushed tape moves no actual gold, so the assertions are on
   the earned-provisioning LOTS; multi-good flour/bread stock setup at 32634-32646); the `non_bread_*`
   counters sit at mod.rs:18740-18757.

## 2. The central question and pre-named diagnosis (this slice classifies a CAUSE, not a success)

**Central question.** On the C3R.d headline cell (the saving motive on, mints retired), across
`SEEDS=[3,7,11,19,23]` (1600 ticks): **for every saving quote-opportunity that did not convert into a
purchase, WHY not** — with an exhaustive, precedence-ordered outcome per opportunity — and which cause
family dominates?

**The accounting unit (v4): a saving quote-opportunity = `(tick, agent, good=staple)`** where the agent is
**literally a member of the captured C3R.d pre-market attribution snapshot for that tick** (mod.rs:18377 —
producer-household member, no unprovided `Now` bread want, carrying a `Next` staple want; NO additional
filter such as "target unmet" — round-3 P0: eligibility is snapshot membership, nothing else, so the
denominator and `Filled` share one predicate by construction); and the tick **enters a money-priced spot
quote pass** for the agent (round-2 P0: ticks without a spot pass — e.g. pre-promotion/barter phases — produce no
QuoteAttempts and are excluded from the domain, counted separately as `no_spot_pass_ticks`). One outcome per
opportunity (the engine posts at most ONE unit bid per agent/good/tick). The fill rate is reported
separately; the loss decomposition's denominator is UNFILLED opportunities.

**The opportunity WINDOW (v3, round-2 P0 fix):** from the **post-cancellation pass-start book snapshot**
through the end of the quote pass — so an ask resting at pass start that an earlier-ordered buyer consumes
BEFORE the saving agent's quote turn is IN the window (it was consumable that tick) and its loss is
classified, never mistaken for absence.

**The outcome enum (exhaustive, assigned by PINNED PRECEDENCE — first match wins):**

```
1. Filled              — the agent bought ≥1 staple this tick under the authoritative attribution predicate
                         (immediate cross at post, or a later same-tick ask crossing its resting bid).
2. NoBidPosted         — the QuoteAttempt shows no live bid this tick (no post, no carried resting bid):
                         reservation None or the ensure_bid gold clamp zero. = GoldReservationBind.
                         (A positive-but-clamped limit proceeds below.)
3. SelfAskOnly         — every staple ask in the window was the agent's own (self-trades skipped,
                         market.rs:387). Counted under the ExecutionResidual family. (Precedes NoExecutable —
                         round-2 fix.)
4. NoExecutableAskInWindow — a bid was live but NO non-self staple ask existed anywhere in the window
                         (neither in the pass-start snapshot nor arriving during the pass).
5. AllAsksAboveLimit   — non-self asks existed in the window but every one's limit exceeded the saving
                         bid's limit. (= PricedOut.)
6. CompetitiveLoss     — a non-self ask compatible with the saving bid's limit existed in the window but its
                         units were consumed by other buyers. priority_basis (round-2 fix):
                           PreEntryOrder        — the compatible ask was consumed BEFORE the saving bid
                                                  entered the book (regardless of the winner's price):
                                                  quote-order artifact → the Microstructure family.
                           HigherLimit          — consumed while the saving bid was LIVE, by a winner whose
                                                  limit exceeded the saving bid's: a genuine price contest.
                           EqualLimitEarlierSeq — consumed while live, equal limits, arrival order decided:
                                                  → the Microstructure family.
                           PostExitConsumption  — the saving bid was CANCELLED/exited intra-pass
                                                  (society.rs:3143) and the compatible ask was consumed
                                                  after the exit: neither pre-entry nor while-live —
                                                  routed to the ExecutionResidual family (round-3 fix),
                                                  with the exit recorded by the trace.
                         PAYLOAD RULE (multiple compatible asks/winners): the payload is taken from the
                         FIRST compatible lost unit in (limit, seq) order — deterministic, pinned.
                         winner_intent ∈ { HungryNow, SavingNext, ProducerInputNext, Other } — derived from
                         the WINNER's first-unprovided bid target at ITS quote time (explicit precedence
                         Now-hunger > Next-saving > Next-input > Other; consumption inside society.step
                         precedes quoting, so quote-time provisioning is the reference) — interpretive
                         color only, never a rule input.
7. ExecutionResidual   — overflow/invalid-settlement rejections (market.rs:502) or any unreconciled case —
                         counted and printed, never silently absorbed. (SelfAskOnly is reported within this
                         family.)
```

**Per-run diagnosis (exact majority rules over UNFILLED opportunities; families of atomic outcomes; printed
per seed, never asserted):**

```
family OfferScarcity      = NoExecutableAskInWindow + AllAsksAboveLimit
family AllocationPriority = CompetitiveLoss with priority_basis = HigherLimit
family Microstructure     = CompetitiveLoss with priority_basis ∈ {EqualLimitEarlierSeq, PreEntryOrder}
family GoldBind           = NoBidPosted
family Residual           = ExecutionResidual + SelfAskOnly + CompetitiveLoss{PostExitConsumption}

OfferScarcityDominates      iff share(OfferScarcity)      > 1/2
AllocationPriorityDominates iff share(AllocationPriority) > 1/2
MicrostructureDominates     iff share(Microstructure)     > 1/2   ← the trap-narrowing outcome
GoldBindDominates           iff share(GoldBind)           > 1/2
MixedDiagnosis              otherwise — ALL family shares printed, ranked; the lever decision (made in the
                            NEXT milestone's spec, not here) takes the top-two FAMILIES.
Ties at exactly 1/2: MixedDiagnosis (disclosed).
```

`winner_intent` is reported as interpretive color on the CompetitiveLoss families (e.g. HigherLimit losses
to HungryNow buyers read as "present need outbids future need" — the economically meaningful contest),
never as a rule input. Every diagnosis is decision-mapped in advance; `MicrostructureDominates` would be
the honest "the trap is partly an implementation artifact" result the second opinion asked us to be able
to find.

## 3. Mechanism (pure observation)

### 3.1 The econ-side trace (v3 schema — three record kinds, tick-keyed, total)
The trace is three record kinds, ALL carrying the tick key, with an explicit per-tick batch lifecycle
(begin-pass marker → records → end-pass marker), gated on one enable
(`Society::enable_allocation_trace()`, called ONCE at settlement construction iff
`saving_allocation_obs_active()`; zero rows when disabled; record-only):
- **`TickBookSnapshot`** — taken at pass start, AFTER expiry/cancellation purging (the post-cancellation
  snapshot the §2 window is defined on): every live resting order — side, agent, good, limit, `seq`.
- **`QuoteAttempt`** — pushed from the quote-pass loop (where `order_pos` is in scope), **TOTAL over both
  sides and all outcomes**: agent, good, side, `order_pos`, and outcome ∈
  `NoBid{reservation_none | clamp_zero | no_want | available_agent_failure | post_failure}` /
  `Posted{seq, limit, reservation}` / `RestingUnchanged{seq, carried_limit, carried_reservation}` (the
  carried metadata round-2 required) / `NoAsk{reason}` for the ask side — TOTAL means every early return in
  the quote path maps to an outcome, including the available_agent and post/reservation failure exits
  (round-3). Instrumented at BOTH money-priced spot loops (the M1 pass,
  society.rs:738-746, AND the money-phase V2 pass, 784-790). **Scope claim (narrowed, round-2):** the trace
  covers the MONEY-PRICED SPOT passes only — the M2/M3 loops are out of this slice's domain (the §2
  opportunity domain is money-priced spot ticks by construction), stated explicitly.
- **`QuoteExit`** — at each intra-pass cancellation/exit of a live quote (society.rs:3143): agent, good,
  side, `seq`, tick — required to route PostExitConsumption (round-3).
- **`Execution`** — at each crossing: incoming `seq`, resting `seq`, buyer, seller, qty, bid limit, ask
  limit, and any rejection (the market.rs:502 class → ExecutionResidual).
- **Intent capture (round-3 P1):** `winner_intent` is recorded AT BID-POST TIME, keyed by `seq` (a
  `seq → intent` side map carried with live orders) — the poster's first-unprovided bid target at its own
  quote instant; settlement pre-market snapshots cannot reconstruct intent after earlier intra-pass trades
  or for carried bids. Bounded retention: ONE tick-batch, drained after each pass; the carried
  `seq → intent` map is bounded by live orders; no cap-and-drop (the batch is the cap).
The §2 assignment is a deterministic join in the TEST layer (snapshot + attempts + executions + the
settlement's want-class snapshots for `winner_intent`, derived by the pinned §2 precedence). No decision
path reads any of it. Honest overhead claim: zero rows/allocations when disabled; record-only when enabled.

### 3.2 The settlement-side observation pass
When active, per tick: (a) record the per-member pre-market counterfactuals via the pure predicates
(reservation / shaded limit / free balances / offerable units per potential seller) — with the disclosed
caveat (round-1 P1) that the LIVE bid path evaluates an `available_agent` with other reserves removed and
its own existing quote restored (society.rs:3149, 6205), so the counterfactual recomputation is
**advisory color**; the AUTHORITATIVE bid facts (posted / not / limit / reservation) come from the
`QuoteAttempt` records, not the recomputation. (b) **READ-ONLY copy** of `consumption_log_last_tick()`
after the market — the log is ALREADY enabled at construction (mod.rs:9684) and the needs phase READS it to
advance hunger and deaths (mod.rs:13392/13405), so this slice must NEVER re-enable (which clears it) or
drain it — the round-1 P0. (c) **Physical-stock snapshots** (`agent.stock`, not the reservation-confounded
free-stock) at the pinned phase seams — start/pre-death, post-death, pre-market, post-market,
post-production/own-use, post-birth, end-of-tick (the CORRECTED phase order: deaths run before scales and
the market, mod.rs:10934; consumption happens INSIDE `society.step` before matching, society.rs:721;
production/own-use after the market and before births, mod.rs:11239/11292) — with reservation deltas
tracked separately; each phase's delta reconciles against its seam records (birth debit mod.rs:14842, the
consumption-log copy, the tape, the death/transfer sites), and only the post-reconciliation residual is
reported `WithinPhaseAmbiguous`. (d) accumulate the run aggregates. All state runtime-only fields on
`Settlement` with public accessors (the `BreadSellerProvenance`/`last_capital_decisions` pattern; NEVER
serialized — avoiding the `birth_block_*` conditional-serialize anti-pattern).

### 3.3 What is deliberately NOT here
No new wants, no transfers, no config-value changes, no lever. The trace records and never branches.
**Digest inertness (exact):** the ON canonical bytes are exactly TWO bytes longer than OFF, and removing
the single `[32, 1]` tag emission yields the OFF bytes byte-for-byte — asserted by a dedicated test (a new,
stronger shape than the existing window-presence tests, per review). **Behavioral inertness (pinned
oracle):** the C3R.d headline cell re-runs with obs ON and must reproduce the five LANDED per-seed
verdicts + key metrics, pinned as exact expected facts in the suite (seed 3 BirthsResumeStructureStillDies,
7 BaseUnviable, 11 BirthStockRaceLost, 19/23 StockReachedBirthsStillBlocked, with the landed births/
attributable/reached-four numbers).

### 3.4 Slice 0 — the D1 counter assertions (C3R.d verified test debt)
Two focused unit tests on the landed attribution pass (the **mod.rs:32229** hand-pushed-tape pattern — the
earlier-cited 32390 block tests birth-stock attribution, not this pass): an external-buyer flour sale
(gatherer buys from a producer-house Miller) and a producer-class flour sale (a member of ANOTHER producer
household buys). Each asserts: the exact counter increment; the OTHER counter unchanged; the seller's
`Earned` **earned-provisioning lot** credited; the buyer's **lot** FIFO debited (a hand-pushed tape moves
no actual gold — credit/debit here means the provenance lots, stated explicitly); bread totals AND bread
provenance unchanged.

## 4. Anti-smuggling / honesty guards
1. **Pure observation, provably:** the exact digest-delta test (ON = OFF + the single two-byte `[32,1]`
   emission, byte-for-byte after its removal); the pinned five-seed behavioral oracle (§3.3); all counters
   runtime-only; the trace opt-in, record-only, never read by any decision path; the consumption log
   read-only-copied, never re-enabled/drained.
2. **Diagnosis, not verdict:** the §2 rules classify a CAUSE with exact >1/2 share rules and a
   MixedDiagnosis catch-all; nothing is asserted; the lever selection is a documented mapping, applied in
   the NEXT milestone's spec, not auto-acted-on here.
3. **The disconfirming outcome is first-class:** `MicrostructureDominates` would reframe part of
   the trap as an implementation artifact — that is a pre-named, welcome result, per the second opinion's
   disconfirmation list.
4. **No new parameters:** the loss taxonomy is exhaustive by construction (posted/not-posted × offer
   existence × price compatibility × who won); the 1/2 thresholds are majority rules, disclosed.
5. **Frozen before the lever:** this spec is reviewed → built → its diagnosis READ, and only then is the
   C3R.e lever milestone scoped (per the amended sequence).

## 5. Conservation & determinism
Nothing moves: no goods, no gold, no wants. Deterministic: the trace records the already-deterministic
quote pass; the counterfactual predicates are pure (`&self`, cloned state — verified; the recomputation is
advisory, the QuoteAttempt records authoritative). **Digest:** tag 32 ON-only `{ push(32);
push(u8::from(saving_allocation_obs)) }` and nothing else — the dedicated test asserts ON is exactly two
bytes longer and equals OFF after removing that emission. Telemetry (all runtime-only): the per-opportunity
outcome table + family shares, the fill rate, `offerable_bread_series` (per-tick offerable units by seller
class), `posted_asks_series`, the QuoteAttempt-derived bid facts, the CompetitiveLoss
(priority_basis × winner_intent) matrix, physical-stock phase deltas + causes + the reconciled
`WithinPhaseAmbiguous` residual, the birth-gate-at-four record, plus the C3R.d battery carried forward.

## 6. Slices
- **0 — the D1 assertions** (§3.4). *DoD: both flour-sale tests green on the landed counters (lot
  semantics explicit).*
- **A — the econ allocation trace.** The three record kinds + `enable_allocation_trace()` + accessors.
  *DoD: unit test — enabled, a hand-driven sequence records snapshot/attempts/executions with correct
  order_pos/seq/limits incl. a GoldReservationBind early-return, a RestingUnchanged carry, and a
  later-ask-crosses-resting-bid execution; disabled (default), zero rows; not serialized.*
- **B — the settlement observation pass + the §2 join.** *DoD: the per-opportunity assignment is exhaustive
  (every unfilled opportunity gets exactly one outcome — a bookkeeping-totality invariant); the exact
  digest-delta test passes; the five-seed pinned oracle passes (obs ON reproduces the landed C3R.d headline
  verdicts + metrics).*
- **C — the suite.** `sim/tests/saving_allocation_obs.rs`: the C3R.d headline cell (obs on, ALL FIVE seeds)
  + the no-motive reference as an inactive identity cell (ALL FIVE seeds; the gate makes obs inert there —
  kept only to anchor the reference facts; the mint-on cell is NOT required, per review). **Budget
  (explicit): 2 cells × 5 seeds × 1600 ticks = 16,000 settlement ticks (~half the C3R.d suite's 4-cell
  grid; target < ~2 min wall incl. the join).** The Slice-B oracle REUSES Slice C's headline runs (no
  duplicate execution). The §2 family shares + diagnosis printed per seed. *DoD: suite green in budget;
  family shares sum to 1 over unfilled opportunities per seed; `no_spot_pass_ticks` reported; drops
  logged.*

## 7. Acceptance suite (`sim/tests/saving_allocation_obs.rs`, new)
- **Hard guards (invariants only):** conservation/money/registry (unchanged), the exact two-byte
  digest-delta test, the outcome-totality invariant, byte-identity of all prior bases (flag off), and the
  **pinned five-seed oracle** — the C3R.d headline cell with obs ON reproduces the landed per-seed verdicts
  AND exact metrics `(births, attributable, reached_four)`: seed 3 BirthsResumeStructureStillDies (3,3,3);
  7 BaseUnviable (0,5,0); 11 BirthStockRaceLost (0,7,0); 19 StockReachedBirthsStillBlocked (1,6,1);
  23 StockReachedBirthsStillBlocked (1,3,1).
- **Diagnosis, printed never asserted:** the §2 family shares + classification per seed.

Build/verify: `cargo test -p sim --test saving_allocation_obs -- --nocapture`, the birth_stock_saving +
earned_provisioning suites unchanged, full workspace, fmt, clippy `-D warnings`.

## 8. Risks & open questions
1. **The trace's join complexity** — assigning CompetitiveLoss requires replaying compatible-ask consumption
   within the tick from the trace; the trace is designed to make this a bookkeeping join (best-opposite at
   post + subsequent entries), but the build should keep the join in the TEST layer if possible.
2. **Two-cause ticks in stock-fall attribution** — resolved by phase-boundary deltas; residual ambiguity
   inside a single phase is reported as `WithinPhaseAmbiguous`, not guessed.
3. **The diagnosis may be seed-heterogeneous** — per-seed classifications reported; a mixed picture feeds a
   mixed lever decision honestly.
4. **Scope discipline:** no lever, no demand-regime change, no classifier congruence work (that lands in
   the ignition spec) — this slice only answers WHY the bid loses.

## 9. Falsifiable-bar summary
C3R.d ended on a disjunction: the saving bids almost never win, for one of four causes with four different
remedies. This slice makes the disjunction exclusive and measured — per opportunity, per seed, with one
bounded inert trace as the only engine surface and a digest that provably differs by the single two-byte tag emission.
Whatever dominates — scarcity, priority, queue order, or gold reservation — the next milestone inherits a
selected lever instead of a guess, and if the dominant cause is the deterministic queue, the trap's
economic reading is honestly narrowed before anything is built on it.
