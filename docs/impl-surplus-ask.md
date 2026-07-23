# impl-76 — C3R.k: The satiated-surplus ask — does a marginal money bid on a costless surplus re-coordinate the mortal chain?

Status (spec): **BUILT + VERIFIED — result: `DOWNSTREAM_NULL` (the lever crosses the wall; re-ignition
starves).** Built via the loop, verified green (off-flag byte-identical, no golden moved), result
adversarially checked twice. See `## −0.9` for the outcome; `## −0.5`/`## −0` for the build design.
Successor to impl-75 (C3R.j Cut 1). The ONE pre-registered lever milestone the C3R arc's one-milestone
cap was reserved for — it closed the arc's re-coordination question with a *tested* null.

## −0.9. RESULT — `DOWNSTREAM_NULL` on all 5 seeds (AUTHORITATIVE outcome; report §39)

`sim/tests/surplus_ask.rs` (5 tests, green; off-flag tick-by-tick `canonical_bytes` == base, no golden
moved; ON-only digest tag). Adversarially checked twice — the first-draft finding OVERREACHED (drop the
gold-pooling/experiment-7 clause; correct the blast-radius mechanism; scope death to a probe). Recorded:

- **CAUSAL (established):** the lever crosses the C3R.j wall. A gate-only flour ask (exists *only*
  because the flag turned `None` → `Price(1)`) posts → fills → is bought by an oven heir on that heir's
  Baker-adoption tick (9–17 heirs/seed, exact order-sequence join). Settlement-wide flour-as-input =
  116–293 ON vs **0 OFF** (identical seeded state). The seller-satiation refusal *was* a real causal
  blocker of flour-quote delivery and heir adoption. `BUYER_WILLING` at `W`.
- **INSUFFICIENT (the null):** all 5 seeds `DOWNSTREAM_NULL` — zero bread in the final 160 ticks, zero
  living Bakers at tick 1600, lever active throughout. Re-ignition is transient.
- **Downstream mechanism UNRESOLVED (do NOT over-read):** the committed test records only role
  transitions + endpoint state, not per-adopter exits. The "starved-out" reading is **contradicted by the
  config**: starvation death is disabled (`hunger_critical = need_max + 1`, mod.rs:3710 — only old-age
  removal), and active/latent producers are fed to a floor before the market (`producer_subsistence = 4`,
  inherited; `run_producer_subsistence`, phases.rs:944), so `food_provision = 0` on the households does
  NOT mean a re-entering baker lacks subsistence. The short tenures are old-age exits or de-adoption, not
  starvation; *why re-adoption ceases* (adopter age / oven-heir-lineage exhaustion / de-adoption) is not
  measured. [This bullet corrected a committed over-read — the arc's 14th price/proxy near-miss.]
- **NOT a gold seizure (over-read dropped):** endpoint Baker gold 0 is vacuous (no living Bakers to sum);
  the surviving Miller's 2531–3428 gold matches the OFF control AND a functioning chain (~4000) — benign
  base condition, opposite signature to Experiment-7. Removed from the finding.
- **Blast-radius:** the AllGoods arm fails the calibrated immortal solvency lens on all 5 seeds via
  Baker-class **insolvency** (Baker gold → 0), NOT window collapse (output stays above the 300 floor).
  Flour-scoping was required for clean **attribution**; it is NOT proven a *safe* remedy on the immortal
  base (no flour-vs-all-goods immortal comparison was run).

**Net:** the one-milestone cap is spent, and it bought a clean tested answer — the satiation wall is
genuine, the marginal-money-demand correction crosses it and restarts baking, and that is INSUFFICIENT:
production does not durably survive producer turnover on this base. The keystone question closes in the
negative with the pricing wall's causal role demonstrated. The downstream persistence mechanism is
UNRESOLVED — naming a specific successor lever (subsistence, lifespan, …) would repeat the corrected
over-read. The honest next step if the arc reopens is an **observation-only exit-attribution** probe on
this harness (per gate-fired heir-adopter: exits-while-alive vs `age == lifespan` vs starvation-counter,
with `producer_subsistence`/`hunger_critical` asserted), to name the real seam before any intervention.

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

## −0.5. v2.1 — confirm pass (AUTHORITATIVE over §−0 items 1/4/6 — the flag design)

Both confirm passes converged: **NEEDS-REVISION on item 1's flag representation only**; all other §−0 items
verified clean (item 2 bypass required; item 3 companions correct; item 8 accessor buildable; the
threading list complete for production). The construction-time bool of item 1 genuinely cannot be
wall-activated (item 6) nor express the flour-vs-all scope (item 4), and a *runtime* debug setter would
escape `digest_coverage_chain_config` (it destructures `ChainConfig` — a non-config knob is invisible to
it) and could never carry the ON-only digest tag item 11 requires. The buildable design that satisfies
items 1/4/6/11 at once, from existing precedent, with **NO runtime mutation** (supersedes those items):

1. **`ChainConfig` gains an activation-tick + scope, not a bool.** `satiated_surplus_ask_at: Option<u64>`
   (`None` = off) and `satiated_surplus_ask_scope` (`Flour` | `AllGoods`). Precedent: `birth_stock_ignition_at:
   Option<u64>` (`mod.rs:1824`), evaluated per-tick behind a latch (`phases.rs:3087-3096`) and digested
   ON-only *including the tick value* (`digest.rs:632-642`). Classify both DIGESTED in
   `digest_coverage_chain_config` with an ON-only tag block mirroring `stale_input_price_fix` tag 36
   (`digest.rs:173`): push tag + `at` bytes + scope byte only when `Some`.
2. **`Society` gains a construction-time `satiated_surplus_ask: Option<(Tick, Scope)>`** (precedent
   `multi_offer_medium`/`durability_aware_acceptance`, `society.rs:468-469`; populated from chain config at
   generation, `generation.rs:1249`). Society owns `self.tick`, so the two Society-internal sites —
   `ensure_ask` (`society.rs:3333`) and the change detector (`society.rs:3795`) — both compute
   `self.satiated_surplus_ask.is_some_and(|(t, s)| self.tick >= t && scope_admits(s, good))` and pass it as
   the new rule parameter. They read the **same per-tick value by construction**, so item 1's flap-off
   acceptance test holds at every tick including the activation tick.
3. **Threading is compiler-forced by the parameter.** The STEERING reads are `ensure_ask` (3333), the
   change detector (3795), and `fresh_input_ask` (`mod.rs:10190`); the census-row reads (`mod.rs:13193-13194,
   13900`) pass `None`/off (non-steering); the `agent.rs:483/495/1677` wrappers are test-only and the
   compiler forces them to choose. No omitted-site flap-off path. Assert the settlement tick and
   `Society.tick` agree at the `fresh_input_ask` boundary so the appraisal and the market activate on the
   same tick.
4. **Item 6 becomes PAIRED RUNS, not a mid-run flip.** Run the OFF control (`satiated_surplus_ask_at =
   None`) to the wall; record the wall tick `W` per seed. Construct the ON run with `Some(W)` +
   `scope = Flour` (the causal arm) or `Some(W)` + `scope = AllGoods` (the separate blast-radius arm). Because
   parameter-threading with an inactive flag is branch-identical, the ON run's `[0, W)` prefix is
   **byte-identical to the OFF control** — a free extra determinism assertion — and "snapshot counters at
   activation" = read the cumulative `bake.accepts` at `W` (monotone, `flour_holder_ask_census.rs:307`) and
   difference the post-activation window. Determinism: `None` never diverges (item 11's tick-by-tick proof
   unchanged); `Some(W)` diverges only at/after `W`, expected and now prefix-checkable against the control.

Three close-outs on other items:
- **Item 3 (config precision):** `stale_input_price_fix` is set EXPLICITLY by the census `config()`
  (`flour_holder_ask_census.rs:28`), NOT inherited (it defaults false) — only `project_input_bids` is
  inherited. Build the ON-run config from that `config()`, not from `frontier_mortal_producers_heritable()`
  alone (which yields `stale_input_price_fix = false` → a dead lever → a FALSE `INERT`).
- **Item 8 (buyer formula):** `bread_bid` is the **third-unit marginal** resting bid price (sum resting
  non-self bids to depth ≥ 3, since every quote is `qty:1`, `society.rs:3392`), NOT best-bid × 3 (which
  overstates). The formula `bread_bid × output_qty − operating_cost, ÷ input_qty` matches
  `imputed_input_reservation` (`mod.rs:15737`). When no live bread bid rests (or depth < 3) at the decline
  tick, record **"no executable buyer surplus measured"** for that seed — a distinct baseline datum, NOT
  counted as buyer-willing (the buyer-willingness claim is then unproven for that seed, reported honestly).
- **Item 9 (Later-horizon): DECIDED — EXCLUDE.** The lever fires only when the holder has **no unexpired
  `Horizon::Later` want for the good** (`agent.rs:981-983`). A good saved for a future want is not a
  costless *surplus*; excluding it preserves the base rule's HOLD default and bounds blast radius. (Tool
  anchors already give `lost_rank = 0 ⇒ in_range == 0`, so this is belt-and-suspenders for future-wanted
  consumables.)

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
