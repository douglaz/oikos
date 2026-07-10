# impl-65 — C3R.d: Saving Ahead of Need — the birth-stock motive (does an individual saving behavior restore intergenerational continuity?)

Status (spec): **SPEC-READY** (Codex xhigh, 2 rounds; the round-2 P1s folded with the reviewer's own
prescriptions: the sufficiency-control donor moved OUTSIDE the target household — the same-household form
was causally vacuous — with the exact eligible-opportunity definition, per-injection recording, and
mutually-exclusive mode bytes; the classifier made TOTAL (StockReachedBirthsStillBlocked + an exact
UnclassifiedMixed catch-all, endowment-counter movement demoted to reported); the attribution snapshot
pinned pre-market; the Slice-0 zero-transfer residue removed; both Miller AND Baker in the focused override
test). Round 1 NEEDS-REVISION (2 P0s
+ 5 P1s, all folded: the want block is the FULL 4-unit target every tick, not `4 − held` — the deficit form
would unreserve accumulating stock; tag 31 with an injective control-mode byte — tag 30 is taken;
`birth_interval=4` on this base + the `Option<u64>` next-eligible formula; the trigger simplified to
below-cap (the "approaching" window was tautological) with the hunger ceiling explicitly omitted; the
extension applied BEFORE `producer_scale_extension` + a focused active-producer-override test; the
sufficiency control made conclusive-or-inconclusive with counters/rollback/phase; every classifier rung an
exact per-seed rule with a defensible purchase-attribution definition; scales acknowledged as serialized;
variant B cut; the Slice-0 inertness proof corrected to ownership + verdict stability with an untracked==0
invariant). The fourth slice of C3R (the keystone: a mortal
production chain) and its first purely **behavioral** slice. Build base: branch
**`feat/earned-provisioning-impl-rb` @ `ec3417d`** (C3R.c landed: income feeds the living but never funds
reproduction). Flag **`birth_stock_saving`** (bool on `ChainConfig`), gated `birth_stock_saving_active() =
flag && earned_provisioning_active()` (composes on the C3R.c stack). Digest **tag 31** (tag 30 is ALREADY TAKEN by `producer_stock_provisioning_control`, mod.rs:24429 — verified) —
payload = the flag byte + the sufficiency-control mode byte (injective encoding for the control cell). New base `frontier_mortal_producers_saving()` = `frontier_mortal_producers_earned()` + the flag.
All prior bases byte-identical off. **Prerequisite repair folded in (Slice 0):** the C3R.c dormant ledger
provenance gap, fixed by lot-preserving reattachment — attribution-only by CODE OWNERSHIP (the buckets feed
statistics only — transfer amounts read actual free gold, mod.rs:18129–18140 — so reattaching lots can
change later earned/endowed statistics, never physical transfers or holdings), verified by re-running the
C3R.c suite with verdicts unchanged.

Falsifiable bar (headline): C3R.c proved that with the mints retired **nobody starves and nobody
reproduces** — the birth gate debits four *saved* staple units from a parent's stock
(`child_food_endowment=4`), and agents whose only buy-side wants are present-hunger `Now` wants eat every
loaf they purchase; births collapse 357→1–5 and the chain dies of childlessness in a fed economy. The
prior-saving problem in its most elemental form. C3R.d supplies the missing **behavior** — a parent-facing
future-bread motive that purchases and *retains* the four-loaf birth stock through the existing market —
and asks: **does individual saving ahead of need restore intergenerational continuity** (births resume,
heirs exist, the chain's structure persists on earned provisioning) — or does the supply/timing race bite
(the stock cannot be assembled before the mortal founders die), or the motive stay inert, or continuity
return without flow? Each outcome pre-named, each first-class.

## 0. One-paragraph summary

The research (probes a59ef92e + a029e4ec) established that the mechanism composes from existing machinery
with **no new engine surface** and — decisively — that the **`Horizon::Next` want is the only correct
carrier**: `reservation_bid_for_money` forms bids ONLY for `Now|Next` wants (a `Later` buy-side want would
never clear — econ/src/agent.rs:389–392), and a `Next` want simultaneously (i) drives a real market bid,
(ii) **reserves** the held stock so `reservation_ask` will not re-list it (without this, a fed parent's
saved loaves leak straight back to the market to feed the standing `Later`-GOLD savings ladder), and (iii)
is never eaten by the consume path (`consume_now_wants` removes stock only for `Now` wants — survival
correctly outranks saving). Three landed precedents use exactly this shape: the producer input buffers
(`producer_scale_extension` — a Miller bids for grain from production purpose, not hunger), the
scholar/confectioner buffers ("neither dumped nor eaten"), and the medium-savings extension. C3R.d is one
scale extension: while a producer-household member's household is **below its cap**, emit the FULL
`child_food_endowment` (4) unit `Want{Good(staple), Horizon::Next}` wants every tick (the full-buffer
scholar/confectioner form — a deficit-count form would unreserve the stock as it accumulates, the review's
P0) — zero new parameters (4 is the existing `child_food_endowment`; the cap is the existing gate knob; the
interval gates the birth itself, not the saving). Everything else is already in place: purchased bread passes the birth gate identically
to minted (the debit at mod.rs:14734–14737 is provenance-blind), the `birth_block_endowment` telemetry landed in
C3R.c, spoilage cannot touch a 4-loaf stock (under the 20-unit free-storage floor), gold at birth is a
clamp not a gate, and affordability is trivial (~4 gold vs 16-gold endowments + earned revenue). The honest
open risk — the experiment's genuine question — is the **supply/timing race**: whether parents can find and
buy four surplus loaves on a barely-staffed market *before the mortal founders die*, and whether restored
births then sustain the structure. A **sufficiency control** (a conserved 4-loaf stock injection — a
`debit_stock`/`credit_stock` move of existing bread with provenance transfer, NOT a mint) proves the birth
gate is the sole blocker once stock exists.

## 1. Base facts (verified by the two probes; cites `sim/src/settlement/mod.rs` @ ec3417d unless named)

1. **The birth gate, in order** (`run_births`, 14647–14808): interval (**`birth_interval = 4` on this
   base** — set in `frontier()` mod.rs:3482–3516, unchanged by the C3R constructors; the next-eligible
   formula is `last_birth_tick.map_or(birth_interval, |t| t + birth_interval)` with `last_birth_tick:
   Option<u64>` initialized `None` — 14653–14658, 7003–7010) → extinct-skip → size cap (producer-house cap
   via tag-28, `birth_cap_for_household` 14619) → hunger ceiling (every member ≤ `birth_hunger_ceiling`=12;
   `birth_block_hunger_ceiling`) → **parent food endowment**: any member holding ≥4 *free* staple units
   (`free_stock_after_all_reserves`, 20707; most-free-gold tiebreak), debited raw at 14734–14737
   (`debit_stock(parent, staple, 4)`; 14738 is the defensive continue) —
   **provenance-blind: purchased bread passes identically** (provenance is transferred to preserve origin,
   14766–14790, never gated). Gold at birth is a **clamp, not a gate** (`child_gold_endowment.min(parent_
   gold)`, 14740). **The `birth_block_endowment` counter already landed in C3R.c** (incremented 14718, with
   `_interval`/`_size_cap`/`_hunger_ceiling` siblings) — the C3R.c failure is directly attributable to
   gate 5.
2. **What eats/sells a saved stock.** Consume removes stock only for `Now` wants (econ/src/agent.rs:750–773);
   `Next` wants reserve without consuming (743–748); a fed parent (deficit 0) emits no `Now` food want and
   holds bread indefinitely. Spoilage decays only the hoard above `FREE_STORAGE=20` (16855) — a 4-loaf
   stock is exempt. **BUT a fed parent's surplus bread is auto-listed as an ask**: `ensure_ask`
   (econ/src/society.rs:2996–3028) posts iff `reservation_ask_for_money` clears, and with no higher bread
   want the sale provisions the standing `Later`-GOLD savings ladder → **the saved stock leaks back to the
   market unless a `Next` want reserves it.** Retention is NOT free — it falls out of the same `Next` want
   that drives the bid.
3. **The want/bid asymmetry** (the decisive constraint): `reservation_bid_for_money` (agent.rs:378–413)
   forms a buy price only for `Now|Next` wants (389–392); `Later` wants never bid (the only existing
   `Later` want is the GOLD savings ladder, a *sell-side* motive). A future-bread motive MUST be `Next`.
4. **The exact template**: `producer_scale_extension` (27156) inserts `Want{Good(input), Horizon::Next}`
   unit wants at `scale_input_insert_position` — just below the `Now` survival block, above the `Later`
   savings ladder. A Miller bids for grain from production purpose, not hunger; a parent bidding for bread
   from a reproduction purpose is the same shape. Sibling precedents: the scholar/confectioner full-buffer
   reservation; `medium_scale_extension`.
5. **Trigger inputs all readable at want-gen time** (`regenerate_scales`, 14876 — tick phase 4, after the
   fast transfers/needs/deaths and before the market, mod.rs:10778–10847): household membership +
   `is_producer_household` (17907), the producer-house cap, own staple stock. Deterministic (id-ordered,
   committed state); births run last (11194) — no ordering hazard.
6. **Affordability is trivial; supply/timing is the question.** Price floored at 1 → the 4-loaf stock costs
   ~4 gold vs `child_gold_endowment=16` + earned revenue (59–199/run). The race: mortal founders
   (lifespans ~tens of ticks) must find 4 *surplus* loaves for sale on a barely-staffed market before dying;
   the lineage surround mint (headline keeps `lineage_food=3`) supplies some floored bread flow. Market
   microstructure: quantity-one orders, one quote pass per agent/good/tick → assembling four loaves takes
   ≥ 4 market ticks — a full `birth_interval`.
7. **The C3R.c ledger provenance gap** (the prerequisite): `run_earned_provisioning_market_attribution`
   debits the buyer's FIFO for every trade (18279) but the intra-household branch `continue`s (18301)
   without re-crediting the seller — recycled gold would drift to `untracked` → misclassified `Endowed`
   (18042–18048). **Fix (lot-preserving reattachment):** capture the debited lots via
   `debit_earned_provisioning_lots` (18008) and, in the intra branch (18290), `credit_earned_provisioning_
   lots(trade.seller, lots)` — earned→earned, endowed→endowed. Do NOT credit external revenue (self-dealing
   rule holds); do NOT relabel as `Earned` (that would launder endowed gold through a self-sale — an
   anti-subsidy break). Attribution-only → no behavior change → byte-identical; provably inert on the
   landed C3R.c grid (transfers were 0).
8. **Time preference today** gates only the GOLD savings ladder (count `save_units(time_preference_bps)`,
   life/src/scale.rs:369; urgency shading 286; depth 350) — per-colonist, inherited, readable at want-gen.

## 2. The central question and pre-named outcomes

**Central question.** On the C3R.c stack (mints retired, earned provisioning wired) with the birth-stock
saving motive added, across `SEEDS=[3,7,11,19,23]` (1600 ticks): **do births resume and the chain's
structure persist on earned provisioning** — intergenerational continuity restored by an individual saving
behavior — and if so, does flow follow?

**Ordered verdict enum** (first-match; structure/flow as C3R.b; the C3R.c ledger + class split carried
forward):

```
Preconditions (disqualifying):
  BaseUnviable        — the no-motive reference cells fail to reproduce the landed C3R.c result (pinned
                        per-seed expected facts, not a prose range), or the mint-on reference fails the
                        C3R.b grid (its exact structure rule: final-window min millers > 0 AND min bakers
                        > 0 — the landed bar)
  ReservoirOpen / ConservationBroken / RegistryBroken — incl. the ledger invariants, the Slice-0 inertness
                        check, and the intra-path untracked==0 invariant
Sufficiency gate (the control cell, evaluated first):
  SufficiencyControlInconclusive — injections_completed == 0 (no eligible opportunity was actually topped
                        up — donor shortfall): the control proves nothing this run; disclosed, and the
                        motive ladder is read WITHOUT the gate-is-sole-blocker premise confirmed.
  BirthGateNotSoleBlocker — injections_completed > 0 AND the corresponding births still failed: another
                        gate binds (the per-gate birth_block_* counters name it) — the C3R.d premise is
                        wrong and the motive result is reinterpreted accordingly.
Outcome ladder (headline cell; ALL rules exact per-seed vs the same-seed no-motive reference; TOTAL — a
terminal catch-all guarantees every run classifies):
  SavingMotiveInert   — birth_stock_wants_emitted == 0 (wiring/trigger never fired), OR wants_emitted > 0
                        AND attributable_purchases == 0 (emitted but never cleared — the two sub-reasons
                        printed). "Attributable purchase" (exact, with a PINNED SNAPSHOT PHASE): a bread
                        purchase clearing on a tick where the buyer had NO unprovided Now bread want, that
                        predicate snapshotted AFTER all pre-market provisions/transfers and BEFORE
                        society.step() — a post-market evaluation would falsely attribute hunger purchases
                        (the newly bought bread would mask the prior unprovided Now want).
  BirthStockRaceLost  — attributable_purchases > 0 AND reached_four_count == 0 (no parent ever reaches 4
                        gate-eligible free staple before its household's birth-or-death) AND
                        births_headline <= births_reference. The supply/timing race, measured.
  StockReachedBirthsStillBlocked — reached_four_count > 0 but births_headline <= births_reference: the
                        stock assembled yet births did not exceed the reference — the per-gate
                        birth_block_* counters at the block times name which gate now binds (ceiling /
                        interval / cap). A finding about the NEXT gate, not a race loss.
  BirthsResumeStructureStillDies — births_headline > births_reference (exact strict, per seed) AND the
                        C3R.b structure rule fails (final-window min millers > 0 AND min bakers > 0 — not
                        met). The endowment-counter movement is REPORTED, not a rung condition (the round-2
                        totality fix: births can exceed the reference without a strict counter fall).
  ContinuityRestored  — births_headline > births_reference AND the C3R.b structure rule holds to the final
                        window on earned provisioning. FLOW reported orthogonally (the landed C3R.b flow
                        bar + the C3R.c genuine-external ledger) — ContinuityRestored + FlowRuns on genuine
                        external revenue would be the program's first self-sustaining mortal economy.
  UnclassifiedMixed   — terminal exact catch-all (any combination not matched above): all quantities
                        printed; guarantees ladder totality.
```
Every rung is first-class: `BirthStockRaceLost` names the supply/timing wall honestly;
`BirthsResumeStructureStillDies` separates demographic from economic continuity; the sufficiency gate
protects the premise itself.

## 3. Mechanism

### 3.1 The saving motive (the one behavioral change)
When `birth_stock_saving_active()`, `regenerate_scales` appends a **birth-stock scale extension** for each
producer-household member (`is_producer_household`, scope §3.3) whose household is **below its cap**
(`birth_cap_for_household`). The trigger is deliberately that simple: the review established that any
"interval-approaching" window is tautological (immediately after a birth, `now + interval >=
last_birth_tick + interval`; and `last_birth_tick` is `Option<u64>` initialized `None`, with the gate's
next-eligible formula `None => birth_interval, Some(last) => last + interval` — mod.rs:14653–14658), so the
honest statement is **"save while the household can still grow"**; the birth gate itself applies the
interval. The **hunger ceiling is deliberately OMITTED from the trigger**: the motive persists while
hungry — `Now` survival wants sit structurally above it and win the gold first — and the ceiling then binds
the *birth* honestly (`birth_block_hunger_ceiling`), not the saving.

**The want block (the round-1 P0 fix): emit the FULL `child_food_endowment` (4) unit
`Want{Good(staple), Horizon::Next}` wants every tick while the motive applies — NOT `4 − held`.** The
deficit-count form would unreserve the stock as it accumulates (held 3 → one want reserves one loaf, two
leak to the ask ladder; held 4 → zero wants and all four lose protection before the market, defeating the
gate). The full-target form is the scholar/confectioner precedent exactly (the full buffer emitted every
tick, mod.rs:14962–14966): provisioning marks held units provided (no further bids once four are held) while
the standing wants keep all four reserved against both the consume path and the ask ladder.

**Placement:** the birth-stock extension is applied **BEFORE `producer_scale_extension`** in the extension
order, so the producer-input wants insert ahead of it at `scale_input_insert_position` (which returns
immediately after the last `Now` good — mod.rs:27141) — yielding the pinned, disclosed value-scale order:
`Now` survival > producer inputs > birth stock > `Later` GOLD savings. One review-mandated caveat: on this
base an ACTIVE Miller/Baker's generic input wants are suppressed by the `project_input_bids` override
(mod.rs:14942–14956, 16354–16375), so rank alone does not establish production-before-saving for actives —
the build must include a **focused test** that an active producer under the real override still posts the
birth-stock bid after its input-gold reservation (society reserves gold in GoodId order,
econ/src/society.rs:738–744). Market microstructure fact (disclosed): orders are quantity-one with one
quote pass per agent/good/tick (society.rs:3040–3048, 738–744), so assembling four loaves takes ≥ 4 market
ticks — and `birth_interval` on this base is **4** (set in `frontier()`, mod.rs:3482–3516; NOT 8), so the
purchase latency spans a full interval: the race is real and measured, not assumed away. The `Next` horizon
does the rest (§1.2–1.4): a real shaded bid (beliefs start expected 2 step 1 — clears a price-1 ask,
conditional on unreserved gold), reservation against the ask ladder, never eaten while fed. No transfer
changes, no new market path, no override; the C3R.c provisioning loop (gold transfers on unprovided `Now`
wants, `has_unprovided_now_bread_want` returning true only for `Now` — mod.rs:18091–18110) is UNCHANGED and
un-triggered by the below-`Now` block.

### 3.2 Conditioning: unconditional (variant B cut per review)
The headline motive is **unconditional** for eligible producer-household members — the producer-input
template exactly (a hardcoded reproduction *purpose*, structurally subordinated to survival and production
by scale position). The time-preference-scaled variant is **deferred to a follow-on**: `save_units` is
private with a 4–60 range and no principled mapping to a 0–4 birth target exists without a new policy
choice — exactly what this slice must not add (review P1). One behavior, one slice.

### 3.3 Scope: producer households only
The lineage households' hearth refills their parents' staple for free (their births never block on the
endowment), and the C3R.b/c comparison cells require the surround UNCHANGED. The motive is gated on
`is_producer_household` — the exact unit whose reproduction C3R.c proved unfundable.

### 3.4 The sufficiency control (proving the premise — conclusive-or-inconclusive by construction)
A control cell where, instead of the motive, a **conserved 4-loaf stock injection** places the birth stock
directly. **The donor is OUTSIDE the target household** (the round-2 fix: a same-household donor is
causally vacuous — any member with ≥4 free staple already passes the gate, and a household genuinely
blocked on the endowment can have no such donor). Deterministic donor: the settlement-wide agent with the
largest free staple stock outside the target household (lowest-slot tiebreak) — typically a hearth-fed
lineage member or another house's baker; a conserved `debit_stock`/`credit_stock` move of EXISTING bread
with both provenance transfers (`bread_provenance.transfer` / `acquisition.transfer_preserve`); **rollback
on any failed credit**. NOT a mint (no `report.endowment` booking; total bread conserved). **Executed
immediately before `run_births`, after production.** **Eligible opportunity (exact):** a household that
passes interval/extinct/cap/hunger and fails ONLY the endowment gate this tick. Each injection records
`(tick, household)` and its immediate birth result. Counters: `eligible_opportunities` /
`injections_completed` / `source_shortfalls`. `SufficiencyControlInconclusive` when
`injections_completed == 0`; `BirthGateNotSoleBlocker` ONLY when an injection completed and its immediate
birth still failed. **Mode exclusivity (injective):** tag-31 payload mode byte ∈ {0=off, 1=motive,
2=control}; mode 2 DISABLES the motive want emission — motive and control are mutually exclusive by
construction, never co-active.

### 3.5 Slice 0 — the ledger provenance fix (prerequisite, attribution-only)
The §1.7 lot-preserving reattachment, landed FIRST with its own DoD: attribution-only by code ownership
(transfer amounts read actual free gold, mod.rs:18129–18140; the buckets feed stats), the C3R.c suite
re-runs with **verdicts unchanged**, byte-identity holds, a new unit test exercises the intra-household sale
path (buy-side lots reattach to the seller with labels preserved; no external-revenue credit; no `Endowed`
misclassification), and an **invariant asserts the `untracked` remainder from `debit_earned_provisioning_
lots` is zero on the intra-household path** (never silently dropped).

## 4. Anti-smuggling guards
1. **A want, not a wire:** the motive only adds wants to the member's own scale; the bid, the clearing, the
   retention, and the birth all run through unmodified machinery. No stock is placed (headline), no bid
   overridden, no price helped.
2. **Parameter-free:** the want block = the existing `child_food_endowment` (full target, every tick);
   eligibility = the birth gate's own cap (the interval gates the birth itself; the hunger ceiling
   deliberately omitted from the trigger — §3.1). No new tuned constant anywhere.
3. **The sufficiency control separates gate from market** — a motive failure cannot be spun as "the gate is
   unreachable," nor a gate problem as "the market failed."
4. **The supply/timing race is allowed to bite** (`BirthStockRaceLost` is first-class): no supply help, no
   founder lifespan extension, no early-market seeding.
5. **The provenance fix does not launder:** intra-household lots reattach with their original
   earned/endowed labels; external revenue and genuine-external stats untouched.
6. **Scope discipline:** lineage households and the immortal surround unchanged; the C3R.b/c reference
   cells must reproduce their landed results.

## 5. Conservation & determinism
The motive adds no flows (wants only); the sufficiency control moves existing bread conserved (rollback on
failed credit); the provenance fix is attribution-only. Integer, deterministic (the scale extension in
`regenerate_scales`'s existing id-ordered loop). **Digest:** tag 31 = ON-only `{ push(31);
push(u8::from(birth_stock_saving)); push(control_mode_byte) }` (tag 30 is TAKEN by
`producer_stock_provisioning_control`; the control mode rides the payload for injectivity). **Scales ARE
serialized into canonical bytes** (mod.rs:24778–24800, with an explicit test at 29205–29223) — the review
corrected the draft's "flag byte only" claim: the new `Next` wants change the digest ON-path (a new
scenario, correct), and flag-OFF bases stay byte-identical because their scales are unchanged. Byte-identity
DoD: `frontier`, `frontier_capital`, `frontier_mortal_producers`, `frontier_mortal_producers_heritable`,
`frontier_mortal_producers_earned` verified directly; tag-31 canonical-split test. **Telemetry
(runtime-only):** `birth_stock_wants_emitted`, `attributable_purchases` (the §2 exact rule),
`reached_four_count`, `birth_stock_held_max`/`_at_death`, the `birth_block_*` quartet, per-house `births`,
the control counters (`eligible_opportunities`/`injections_completed`/`source_shortfalls`), plus the C3R.c
battery carried forward (ledger + class split, structure/flow, price/trades, gold trends).

## 6. Slices
- **0 — the provenance fix** (§3.5). *DoD: attribution-only by ownership; C3R.c suite verdicts unchanged;
  byte-identity; the intra-sale unit test; the untracked==0 invariant.*
- **A — the flag + the motive.** `birth_stock_saving`, tag 31, the full-target scale extension
  (unconditional, applied BEFORE `producer_scale_extension`), the base `frontier_mortal_producers_saving`.
  *DoD: flag-off byte-identical (all five old bases direct); tag-31 split; flag-on wants emit for eligible
  members (counter > 0); the focused active-producer-override test (§3.1 — BOTH an active Miller AND an active Baker under the
  real `project_input_bids` override still post the birth-stock bid).*
- **B — the sufficiency control.** The conserved injection mode (§3.4) with its counters + rollback.
  *DoD: conserves (no endowment booking); inconclusive-vs-conclusive decidable; injective mode byte.*
- **C — the suite.** `sim/tests/birth_stock_saving.rs`: headline (motive) / sufficiency control / no-motive
  C3R.c reference (pinned per-seed expected facts — births, verdict — as executable regression anchors, not
  a prose range) / mint-on reference (the exact C3R.b structure rule); `SEEDS=[3,7,11,19,23]`,
  RUN_TICKS=1600, lineage surround at the base {3} (the C3R.c sweep stands — not re-swept; disclosed). The
  §2 ladder printed per cell, never asserted. *DoD: suite green in budget; drops logged.*

## 7. Acceptance suite (`sim/tests/birth_stock_saving.rs`, new)
- **Classifier, NOT asserted:** the §2 ladder — every rung an exact per-seed rule (strict integer
  comparisons vs the same-seed no-motive reference; the C3R.b structure/flow bars reused verbatim from the
  landed suite).
- **Hard guards (invariants only):** conservation, money, registry, `immortal_producer_count==0`, the
  ledger invariants + Slice-0 inertness + the intra-path untracked==0 invariant, byte-identity of the five
  old bases + the tag-31 split, the no-motive reference reproducing its pinned C3R.c facts and the mint-on
  reference passing the exact C3R.b structure rule.

Build/verify: `cargo test -p sim --test birth_stock_saving -- --nocapture`, full workspace, fmt, clippy
`-D warnings`; earned_provisioning + mortal_producer_inheritance + mortal_producers + producible_capital +
g5b_frontier + g4b_demography + share/wage/succession suites stay green; every prior digest unchanged.

## 8. Risks & open questions
1. **The supply/timing race is the genuine question** — a barely-staffed market may not offer four surplus
   loaves within a founder's lifespan (`BirthStockRaceLost`). Honest if so; the telemetry
   (`birth_stock_held_max`/`_at_death`, purchases) makes the race measurable.
2. **Bid competition inside the parent's own gold** — the birth-stock `Next` want competes with the
   producer-input `Next` wants and the earned-provisioning transfers for the same free gold
   (`allocated_money_before_rank` protects higher ranks; the pinned below-inputs placement decides).
   Disclosed; the review should confirm the placement can't starve the same-household provisioning loop.
3. **The hunger-ceiling gate may bind next** — if saving succeeds but births still block on the ceiling
   (members hungry in a mint-free economy), that surfaces as `birth_block_hunger_ceiling` dominance inside
   `BirthsResumeStructureStillDies`/`BirthGateNotSoleBlocker` — a finding, not a failure.
4. **Scope:** C3R.d does NOT touch the demand-durability question (the depleting consumer stock, the
   gatherers' 87%) — that re-runs after continuity is restored, per the C3R.c RoR.

## 9. Falsifiable-bar summary
C3R.c located the keystone's deepest wall: a mortal economy whose members demand only against present need
cannot fund its own reproduction — the four-loaf birth stock never accumulates, and the chain dies of
childlessness in a fed economy. C3R.d adds the single missing behavior — a parameter-free, market-cleared,
`Next`-horizon saving want keyed on the birth gate's own requirements — and lets the existing machinery do
everything else. If births resume and the structure persists (`ContinuityRestored`, with flow reported
orthogonally), an individual saving behavior is what stands between a fed-but-childless economy and a
self-perpetuating one — the first act of saving, located. If the market cannot supply the stock in time
(`BirthStockRaceLost`), the motive never bites (`SavingMotiveInert`), people return without the chain
(`BirthsResumeStructureStillDies`), or the gate was never the sole blocker (`BirthGateNotSoleBlocker`) —
each is a named, measured finding that tells the keystone exactly where the saving problem actually lives.
