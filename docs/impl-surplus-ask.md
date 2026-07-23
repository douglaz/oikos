# impl-76 — C3R.k: The satiated-surplus ask — does a marginal money bid on a costless surplus re-coordinate the mortal chain?

Status (spec): **v2 — REVISED (dual review folded; awaiting confirm pass).** Both reviews VERIFIED the
lever is buildable and crosses the wall end-to-end (probe: `Price(1)` is the exact limit of the existing
pricing rule; Bake margin clears 5/5 seeds), but returned NEEDS-REVISION with seven concrete misfire
paths — threading, companion flags, attribution, activation timing. See `## −0` (AUTHORITATIVE).
Successor to impl-75 (C3R.j Cut 1). The ONE pre-registered lever milestone the C3R arc's one-milestone
cap was reserved for — it closes the arc either way: the chain re-coordinates (the satiation wall was
the causal blocker), or the pre-registered null is pinned with a *tested* lever behind it.

## −0. v2 revision (AUTHORITATIVE — folds the dual review; supersedes §§1–8 on conflict)

Both reviews confirmed the mechanism is real (gated `Price(1)` at the `NoMoneyGain` exit → `ensure_ask`
posts → `fresh_input_ask` reads the raw reservation → Bake margin clears → adopted heir's flour bid
posts) and that `Price(1)` is the genuine limit of appending unit-granularity money wants to
`first_money_gain_price_at_or_above` (verified returning `Gold(1)` at gold 5/6/7/19/1000), so §1's
"faithful, not fake" holds — record it, honestly, as a **pre-registered discrete unit-granularity
money-demand assumption**, not a recovered marginal valuation. The revisions, by priority:

1. **[P0 — the threading seam, and its silent-failure mode]** `ChainConfig` lives only in `sim`; the ask
   rule is an `econ` `Agent` method with ~7 call sites. The flag must become a **parameter** of
   `reservation_ask_outcome`/`reservation_ask_for_money` (so the compiler forces every site to choose),
   carried on `Society` as a **construction-time bool** (precedent `society.rs:468-469`). Enumerate the
   sites that must pass it: `society.rs:3333` (`ensure_ask`), **`society.rs:3795` (the live-quote change
   detector — CRITICAL)**, `agent.rs:412`, `mod.rs:9793`, `10190` (`fresh_input_ask`), `13193-13194`,
   `13900`. **Silent-failure:** if the change detector recomputes the reservation WITHOUT the flag while
   `ensure_ask` posted WITH it, the gate-fired ask is cancelled every reconciliation pass — the lever
   flaps off invisibly. Acceptance: a test proving a gate-fired ask **survives a reconciliation pass**.
2. **[P0 — return AT the `NoMoneyGain` exit, bypassing post-price validation]** In the gate state
   `money_want_gained_at_or_above` is false, so routing `Price(1)` through the normal exit trips
   `debug_assert!(false)` (`agent.rs:578-589`). Return it at the `NoMoneyGain` branch (`agent.rs:551,
   1076`) directly.
3. **[P0 — the two companion flags are load-bearing; pin the ON-run config]** The appraisal reads
   `fresh_input_ask` ONLY when `stale_input_price_fix` is ON (`phases.rs:2305`), and an adopted Baker
   posts an input bid ONLY when `project_input_bids` is ON (`mod.rs:8553`). Both are ON in the census
   config (inherited via `frontier_endogenous→frontier_capital→frontier_mortal_producers_heritable`). Pin
   the ON-run config = the census config and STATE both companions are load-bearing: in any config
   lacking either, the lever is dead by construction and `INERT` would be a false economic null.
4. **[P0 — scope the CAUSAL arm to flour; general rule is a SEPARATE blast-radius arm]** `ensure_ask`
   runs per agent × per market good per tick (`society.rs:961`), so a general rule opens satiated asks on
   grain/bread/wood/food too — a non-flour channel (satiated grain at 1 reviving millers) could produce
   `REIGNITES` and mis-attribute it to the flour wall. The pre-registered CAUSAL test gates the rule to
   **flour only**. Run the general (all-goods) rule as a **separate, separately-reported blast-radius
   arm**. (This inverts v1's "general with guardrail" — attribution demands the scoped arm lead.)
5. **[P0 — attribution telemetry, or the `REIGNITES` claim is an over-read]** Add per-good gate-fired ask
   count + gate-fired fill count. `REIGNITES` additionally REQUIRES the adopting heir's flour purchases
   to **trace to gate-fired asks**; absent that trace, weaken the pre-registered conclusion from "the
   satiation wall was causal" to "a satiation wall was involved".
6. **[P0 — activate treatment only AFTER the paired OFF run reaches the wall]** An always-on flag alters
   deaths/inventories BEFORE the wall and poisons attribution. Run the OFF control to the pinned
   post-death wall; **activate the flag at that tick**; **snapshot all cumulative counters at
   activation**; difference `accepts` (cumulative, `flour_holder_ask_census.rs:291-309`) over the
   post-activation window only.
7. **[P0 — the outcome tree makes invalid causal claims; split it]** Replace v1 §4's `INERT` with:
   `REIGNITES` (a gate-fired flour ask posts → crosses → fills a Bake candidate → an ACTUAL adoption
   transition occurs — note `Accepts` is recorded BEFORE the vocation switch can be rejected,
   `phases.rs:2369-2415`, so require the transition, not the `Accepts`) AND the sustained-chain lens
   holds; `NOT_DELIVERED` (no gate-fired flour ask posts/crosses/fills — tells us nothing about the
   causal hypothesis, NOT a null); `DOWNSTREAM_NULL` (asks fill + heir adopts but the chain still fails
   for a named downstream reason — the honest causal null); `DESTABILIZES` (blast-radius arm breaks a
   calibrated base). Add a **fill-latency lens** (ticks from first gate-fired ask to first fill; ask-limit
   trajectory) because `ensure_ask` shades the limit UP and `nudge_unfilled_ask` walks it down over ticks
   (`expect.rs:52-58`), so "ask never fills" in a short window can be belief walk-down, not refusal.
8. **[P1 — the buyer-willingness baseline (§3) is NOT buildable as written]** `live_quotes` is private
   (`society.rs:475`); no accessor exposes a live bid PRICE. Add a read-only `Society` accessor for the
   best live non-self bid on a good. Fix the DEPTH bug: Bake produces 3 bread but every quote is qty 1
   (`society.rs:3386`), so one 1-unit bread bid cannot back 3 units of imputed revenue — derive buyer
   willingness from **≥3 units of live bid depth (or the third-unit marginal bid)**: pre-register the
   formula (`bread_bid × output_qty − operating_cost, ÷ input_qty`) and the behavior when no live bread
   bid rests at the decline tick. Prove candidate free tender before counting a flour bid as executable.
9. **[P1 — gate hardening]** Reject zero-quantity money wants (`Want.qty == 0` counts as provided, so
   `in_range > 0` can be met by a vacuous want — require `cumulative_required > 0`). Name the
   `Horizon::Later` blind spot (`agent.rs:981-983`): a good held against a future want reads as costless
   surplus and would be sold at 1 — the gate flips the base rule's default from HOLD to SELL for
   future-wanted goods; either exclude goods with an unexpired `Later` want or accept explicitly and add
   it to the guardrail. (Tool anchors are SAFE — a top-ranked tool want gives `lost_rank=0 ⇒ in_range==0`,
   so heirs cannot dump inherited mills/ovens at 1.)
10. **[P1 — solvency lens]** Use the existing calibrated sustained-chain threshold, not `bread > 0`:
    living Baker at horizon, final-window Baker production `>= 300`, long-horizon solvency, AND the
    Miller/Baker gold distribution (`baker_roundtrip_2x2.rs:19-40,148-157`) — Baker gold alone misses
    known gold pooling in Millers.
11. **[P2 — determinism, confirmed correct]** ON-only digest tag (precedent `stale_input_price_fix`
    tag 36, `digest.rs:2455`); `digest_coverage_chain_config` destructures `ChainConfig` so the field
    cannot be left unclassified; parameter-threading with `false` is branch-identical off-flag. Prove
    tick-by-tick default-false == explicit-false `canonical_bytes` across all 5 seeds.

**Net:** the milestone becomes a properly controlled intervention — a flour-scoped causal arm activated
at the wall with counters snapshotted, attribution-traced `REIGNITES`, a four-way outcome tree that
separates non-delivery from a real downstream null, a separate general blast-radius arm, the two
companion flags declared load-bearing, and the buyer-willingness baseline made buildable with a real
live-bid accessor and depth-correct formula. §§1–8 below are the v1 rationale, superseded here on
conflict.

## 0. What C3R.j Cut 1 established (the wall this milestone tests)
The mortal chain's post-death wall is an actor-independent **money-satiation** wall (report §38,
`docs/impl-holder-ask-absence.md` §−0.9): every flour holder's gold already provisions every money want
on its ENTIRE value scale (`lost_rank == scale_len`), so the ordinal ask rule
(`econ/src/agent.rs::reservation_ask_for_money`, now a projection of `reservation_ask_outcome`) yields no
reservation price and `ensure_ask` posts none. The unit's sale drops **no current-period allocation** —
a *costless surplus* — and it still goes unpriced. A follow-up probe indicates the *buyer* side is
willing (executable bread price supports a flour price ~1 at each decline), so this is a satiated seller
declining to originate a quote a willing buyer would accept, not absent demand. It blocks surviving
seeded-latent founders and inheriting heirs alike.

## 1. The economic claim under test (why this lever is faithful, not fake)
The refusal is faithful to a **finite ordinal money-want ladder** — but that ladder is a modeling
primitive the project has already overridden twice as un-economic (adoption-side: `recurring_motive`;
food-hoard-side: threshold `run_spoilage`). Mengerian/Misesian **cash-balance money demand does not
satiate**: a holder of a *costless* surplus weakly gains from *any* positive money price. So this
milestone tests a specific correction of that primitive at the ask surface — NOT an arbitrary forced
ask. Pre-registered epistemics: if the correction re-coordinates the chain, the finite ladder was the
load-bearing artifact and the satiation wall was causal; if it does not (or it destabilizes calibrated
bases), the honest null is pinned and the arc STOPS. Either outcome is a result.

## 2. The lever (default-off, digested ON-only, minimal)
A `ChainConfig` flag (e.g. `satiated_surplus_ask`). When ON, `reservation_ask_outcome` — and therefore
its `reservation_ask_for_money` projection — returns `Price(1)` (the minimal money unit) INSTEAD of the
`NoMoneyGain` `None`, exactly when the measured universal state holds:
`lost_rank == scale_len` (the sale drops no allocation — a costless surplus) AND
`provided_wants == in_range_money_wants` with `in_range_money_wants > 0` (fully money-satiated, not
want-less). This is the money-want-ladder extension **in its limit**: appending a fine-grained recurring
unit money want makes the existing shortfall rule (`first_money_gain_price_at_or_above`) return exactly
1, so it is a *stated pricing rule* (satisfies `impl-holder-ask-absence.md` §−0 item 5) without
rewriting any scale or touching the bid side. The posted quote still flows through `ensure_ask`'s normal
netting/shading — no forced raw ask, no forced fill.

**Conservation / faithfulness invariants:** the flag changes only the reservation *price* the holder is
willing to accept; it does not credit goods, move gold, alter allocation, or change any bid. A sale that
results still executes through the ordinary market at the cleared price.

## 3. The durable two-sided baseline (folds in Codex's census — OFF-flag, always asserted)
The C3R.j finding's two-sided and actor-independence legs currently rest on deleted probes. This
milestone DURABLY asserts them in the committed test, OFF-flag (extending / beside
`sim/tests/flour_holder_ask_census.rs`; reuse `build_flour_census_row`):
- **Buyer willingness (durably pinned):** at the decline, each Bake-eligible candidate's max flour bid
  derived from an **executable live non-self bread bid** — NOT the frozen `realized_price(bread)`
  (`phases.rs:2286`), which is *the arc's oldest over-read*. Assert a positive buyer surplus exists
  (candidate would pay ≥ 1 while the seller loses no allocation).
- **Actor-independence (durably pinned):** assert that recorded oven-inheritor heirs appraise Bake at
  the decline ticks, and that in the seeds where they had adopted `Baker` they did so before the wall —
  not merely `NotPostDeathHeir` on the first captured candidate.
- **Per-holder satiation:** assert `MoneySatiated` per holder (not only the modal reason).

## 4. Pre-registered outcomes (BEFORE running — no post-hoc bucket invention)
Run ON-flag across the 5 canonical seeds `[3,7,11,19,23]`.
- **`REIGNITES` (lever causal):** Bake `accepts > 0` AND final-window bread produced > 0 AND the C3R.h
  solvency lens holds (baker-class gold floors > 0), on the pre-registered majority of seeds.
- **`INERT` (null, STOP):** asks post but the wall merely moves — the decline reclassifies to
  `MarginNonpositive`, or the ask never fills, or no `accepts`. Pin the honest null: the satiation wall
  was not the causal blocker; the chain's death survives a priced costless surplus.
- **`DESTABILIZES` (over-reach, STOP):** the flag breaks a calibrated base — see §5. The lever is too
  broad to be faithful; pin that and STOP.
- Suite `MIXED` → report per-seed; a milestone-level STOP unless a clean majority `REIGNITES`.

## 5. Blast-radius guardrail (the strongest risk — measure, do not assume)
The rule fires for **every** money-satiated holder of **any** costless-surplus good, not just flour — a
satiated gatherer could post grain at 1 (cf. the C3R.h `EITHER_SUFFICES` reversal when levers stack).
Pre-register, ON-flag: measure the IMMORTAL-chain scenario and the solvency floor under the flag, not
only re-ignition. If the immortal base regresses (production collapses, solvency floor breached), that is
`DESTABILIZES`. (Design question for review: start GENERAL and let the guardrail expose over-breadth, or
scope the flag to chain intermediates from the start? Leaning general-with-guardrail — a general break is
itself the finding that the finite ladder is load-bearing elsewhere.)

## 6. Determinism
`satiated_surplus_ask` is behavior-steering → **DIGESTED ON-only**, classified in
`digest_coverage_chain_config`; off-flag byte-identical proven tick-by-tick (no golden moved off-flag);
on-flag moves goldens (expected, pinned). The durable baseline telemetry (§3) is non-steering /
not-digested.

## 7. Scope guard / NON-GOALS
- Build ONLY the `lost_rank == scale_len && fully-satiated` ask=1 rule. NOT a general seller-motive
  framework, NOT a bid-side change, NOT spoilage-on-flour, NOT forward orders / governance.
- Do NOT touch `project_input_bids` (lever (b), proven inert at this wall — the restock gate
  short-circuits before the frozen-price read and `realized_price(flour)=Some(1)`).
- Buyer-willingness telemetry MUST read executable live bread demand, never `realized_price(bread)`.
- Pre-register §4/§5 buckets and the solvency lens BEFORE running; no post-hoc bucket invention.
- Size budget: the `econ` rule is a few lines; the test + baseline telemetry the bulk. Plain `cargo`.

## 8. Acceptance
Off-flag byte-identical (goldens hold, digest guard green); on-flag classified into a pre-registered
bucket across 5 seeds with the durable two-sided baseline asserted; immortal-base + solvency measured
under the flag; full `cargo test` green, `clippy --all-targets -D warnings` clean, `fmt --check` clean.
