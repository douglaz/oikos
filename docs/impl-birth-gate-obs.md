# impl-70 — DH.b-obs: the birth-gate-stock diagnostic

Status: SPEC-READY v4 (Codex xhigh, 4 rounds: R1 ×14, R2 ×10, R3 ×7, R4 = SPEC-READY with 5 NITs, all folded into this text)

Changelog v4 (R3): observation activates AT `WindowStart` (before `regenerate_scales`, whose
quote cancellations release staple reservations) and is drained/disabled after `Society::step`
(R3-1); Society receives the staple `GoodId` via `begin_staple_obs_window(staple, active)` and
filters on it, never learning household/class (R3-2); a live-emitter test battery proves the
atomic emission against `step_m1`/`apply_trade` (R3-3); `Production` ownership pinned to
`Settlement` (Society authoritative only for Consumption/SettledTrade/AskChange) (R3-4); the
inactive-vs-force-disable byte tests disambiguated (R3-5); `event_end = joined_events.len()`
(R3-6); `opportunities_by_cell` = `None` off / `Some(map)` on (R3-7).

Changelog v3 (R2): a `WindowStart` baseline snapshot + a per-opportunity gate cursor bound
the replay (R2-1); the tape records atomic LOGICAL economic events (settled trade = one
two-member group; consumption emitted after reservations restored) not raw writes (R2-2);
the classifier takes the recorded gate decision and emits `GateDecisionMismatch` (both
directions), drains select the LAST ≥q→<q transition (R2-3); two predicates —
`birth_gate_obs_configured()` for tag/allocation, `birth_gate_obs_active()` for event writes
— resolve the force-disable/tag conflict (R2-4); the tape is a generic `Society`-owned tape
joined in `Settlement` (R2-5); a `ClassifiedBirthGateOpportunity` envelope carries
peak/gap/total so the sole reducer supplies every field (R2-6); the harness result gains an
`opportunities_by_cell` field (R2-7); machine-enforced q=4 non-vacuity (R2-8); component-wise
Miller+Baker=global (R2-9); the production anchor corrected (R2-10).
Milestone: DH.b-obs — the income-vs-burden structural slice (pure observation; the
diagnostic the DH.b grid opened)
Digest tag: 35 (next free; ON-only, injective)
Base: the exact DH.b grid — `frontier_closed_circulation()` × q∈{0,1,2,3,4,8} × saving arm
{Off, On(Motive)} × the landed seeds [3,7,11,19,23], run under observation in ONE execution.
Template: C3R.e-obs (impl-66) — copy its digest-inertness shape AND its ordered-trace shape
(the `AllocationRecord` tape, econ/src/society.rs:106) verbatim.

Changelog v2 (R1): the ordered free-stock event trace replaces phase-seam sampling so the
true intra-tick peak and the exact below-q crossing event are observable (R1-2); the ladder
made total with a member/household drain distinction classified by the crossing EVENT, not a
heuristic (R1-1, R1-3, R1-5); the gate sample pinned inside `run_births` (R1-4); the
denominator recount made gate-replay-independent with the full equality set (R1-6); one typed
`BirthGateReport::from_traces` reducer forces every share through the classifier (R1-7); a
shared DH.b harness both suites call, deriving the grid not printing it (R1-8); zero-denominator
strata first-class as `NA` (R1-9); the impossible peak-histogram expectation removed, absolute
peak + gap reported (R1-10); honest-null interpretations narrowed to the same-tick
within-household counterfactual (R1-11); the exact force-disable test moved beside the closure
unit tests with dynamic re-checks (R1-12); non-vacuity phrasing fixed (R1-13); runtime disclosed
and the grid executed ONCE (R1-14).

## 1. Motivation and the preregistered question

DH.b (impl-69) found the reproduction wall is not the four-loaf constant: at burden q∈{3,4,8}
NO birth ever occurs (30 cells, `NoBirth`), while q∈{1,2} produce 6–14 births/run that still die
childless of lineage. The birth gate is an **atomic, single-member hold**: one member must hold q
FREE loaves of the staple simultaneously at the end-of-tick births phase (`parent_birth_food` →
`free_stock_after_all_reserves ≥ q`, mod.rs:23127-23132, 16018-16037), and that staple is the
SAME bread every member eats each tick, on a base with no bread mint (mod.rs:4136-4147, 4183).
C3R.d's sufficiency control proved the endowment gate is the SOLE binding gate once stock exists.
So the wall reduces to one measurable question:

**Why does no producer-household member ever hold q≥3 free loaves at the births-phase gate — and
how does that failure decompose across the endowment-gate failures, split by producer type, and
how does the decomposition shift from q∈{1,2} to q≥3?**

The mechanism is legible from the code (throughput=1 baker production ≈ ~1 loaf/member/tick
consumption ⇒ net≈0; millers produce flour not bread and buy bread losing the allocation
contest; transient surplus sold or eaten before the gate). What is NOT legible, and what only an
ordered-trace measurement settles, is the **share decomposition** — production/income starvation
vs intra-household atomicity vs drain-before-gate, by the EXACT event that crosses the household
below q — split Miller vs Baker. That decomposition is the finding and selects the next lever;
this slice measures it and asserts nothing about it.

Non-institutional (observes, changes no behavior — proven, §7) and non-vacuous at canonical q=4:
at q=4 every otherwise-eligible endowment-gate opportunity fails, and the full set of those
failures is observed (R1-13).

## 2. What is observed — the Society-owned atomic logical-event tape (runtime-only, digest-inert)

Phase-seam sampling is insufficient: consumption and all ordered trades occur INSIDE
`Society::step` (mod.rs:12172), so a member can buy up to q and dispose of it before any seam
fires (R1-2). Following C3R.e-obs's `AllocationRecord` tape, the observer records an ordered
event tape — but it must be a **generic tape owned by `econ::Society`** (R2-5), because the
mutation sites live in `Society`/`Agent`/`market`, which cannot call `Settlement` methods and do
not know household/class. `Settlement` joins the drained tape with household/producer-type
metadata it alone holds.

**The Society tape.** A runtime-only `Vec<StapleStockEvent>` on `Society`, keyed by `AgentId`,
with observation-only state `staple_obs_active: bool` and `staple_obs_good: Option<GoodId>` (R3-2
— Society cannot infer `Settlement::known.hunger`, which is dynamically interned and not
necessarily `FOOD`; the staple GoodId is passed IN via a seam `begin_staple_obs_window(staple:
GoodId, active: bool)`). The active bit and good are read ONLY by recording branches (never by a
decision). Society is authoritative for exactly three atomic **logical economic events** (R2-2,
R3-4), each filtered to `staple_obs_good`:
- `Consumption { agent }` — emitted AFTER `step_m1` restores reserved assets (econ/src/society.rs
  :850 temporarily strips reservations during the eat call; recording mid-strip would
  double-subtract), so the recomputed `free` is correct.
- `SettledTrade { seller, buyer }` — ONE group carrying BOTH members' post-states, with the
  fill's reservation release folded in (econ/src/market.rs:653 releases reservations + moves
  stock in separate statements — they are one logical event; a same-household transfer must not
  produce a phantom seller-dip/buyer-peak).
- `AskChange { agent }` — a standalone reserve/cancel/expiry that moves free stock and is NOT
  part of a settlement (a posted-but-unfilled ask lowering free stock — the R1-3 case; this also
  covers `regenerate_scales`'s quote cancellations, see the window rule below).
Each record carries, per affected agent, `(physical, reserved, free)` computed AFTER the whole
logical event is applied; `free` = `free_stock_after_all_reserves(agent, staple)` (pure `&self`,
econ/src/society.rs:6603). `Production` and the gate/`BirthDebit` events are NOT Society events
(R3-4): `Settlement` appends exactly one `Production` immediately after each successful
`execute_direct_recipe_for_agent_checked` return in `run_production` (post-market, mod.rs:12317,
econ/src/society.rs:6487) — the acquisition-ledger credit is metadata only — and the gate/debit
events at `run_births`.

**Window and gate cursor (R2-1, R3-1).** The measurement window for a tick is from a `WindowStart`
baseline to the gate. `WindowStart` = a snapshot of every living producer-household member's
`(physical, reserved, free)` taken RIGHT AFTER the death/estate phase — so estate-to-heir staple
credits (mod.rs:14877) are folded into the baseline, not a mid-window event — at which point
Settlement calls `begin_staple_obs_window(staple, active)` to **clear the tick tape and ACTIVATE
observation** (R3-1). Observation is therefore live through `regenerate_scales` (mod.rs:11984,
whose `cancel_changed_live_quotes_for_agents` releases staple-ask reservations — these emit
`AskChange`) AND through `Society::step`; Settlement drains the Society tape and DISABLES it
immediately after the step, then appends `Production` and the gate events. Each gate opportunity
stores `event_end = joined_events.len()` captured immediately before the births-phase gate read
(R3-6); the classifier consumes `&events[..event_end]` and thereby EXCLUDES the post-gate
`BirthDebit` and the post-births `run_spoilage` (mod.rs:12412). Replay initializes per member from
`WindowStart` (a member already holding q is counted at-q from event 0) and hard-reconciles its
terminal per-member free vector against the authoritative gate state.

**Gate state.** For each producer household each tick, the observer records the per-member `free`
read taken INSIDE `run_births`, immediately before parent selection and the debit (the
authoritative `gate_phase`, R1-4; hard-asserted equal to `post_production` on the DH.b grid, where
the intervening control/ignition hooks are inactive), the recorded PASS/FAIL decision, and the
gate stratum (interval / non-empty / size-cap / hunger-ceiling all passed ⇒ the endowment gate was
reached).

**Two activation predicates (R2-4):**
- `birth_gate_obs_configured()` = `demography.is_some() && chain.birth_gate_obs` (a new
  `ChainConfig::birth_gate_obs: bool`, default false; on the closed base the closed marker is set,
  so this is just the flag). Drives tag-35 serialization AND construction-time tape allocation —
  UNAFFECTED by the closure force-disable, so both force-disable twins carry the identical tag.
- `birth_gate_obs_active()` = `birth_gate_obs_configured() && closure_active()`. Drives event
  WRITES only. Under the closure force-disable this goes false and the tape records nothing, while
  the tag byte is unchanged. Set the Society `staple_obs_active` bit from this predicate; the
  closure hook's contract (safe after construction, before the first tick — closure.rs:1076) is
  respected (no mid-run flip is required or claimed).

Not gated on q or the saving arm — the observer covers all 60 DH.b cells without perturbing them.

## 3. The classifier (pure, total, precedence over the ordered trace)

`classify_birth_gate_opportunity(window_start, events, gate_state, q) ->
ClassifiedBirthGateOpportunity`, a pure test-layer function (the `classify_saving_opportunity`
pattern, mod.rs:6844). Replaying `[window_start, gate cursor)` it maintains, after each logical
event, the max single-member free stock and the household-total free stock (`window_start` counts
as event 0, so a member already at q at the tick's start is at-q from the outset — R2-3). It
returns the envelope (R2-6):

```
ClassifiedBirthGateOpportunity {
    outcome: BirthGateOutcome,
    household_peak: u32,       // max household-total free over the window
    gap_to_q: u32,             // q.saturating_sub(household_peak)
    gate_household_total: u32, // household-total free at the gate
}
```

**Contradiction handling first (R2-3):** the classifier takes the recorded PASS/FAIL decision AND
recomputes the gate pass (terminal max member ≥ q). Classify `(recorded, recomputed)`:
- both PASS → `GatePassed`;
- either mismatch → `GateDecisionMismatch { recorded, recomputed }` — the observer/gate
  contradiction (replaces `UnexpectedGatePass`, now covering both directions). Hard guard: must be
  **zero** grid-wide; nonzero fails the suite, not a scientific outcome.
- both FAIL → the scientific ladder, first match:

1. `MemberDrainedBeforeGate { crossing: EventCause, member }` — some single member reached ≥q at
   some event but is < q at the gate; focal member tie rule (R1-5): highest peak, then earliest
   at-or-above-q event, then lowest AgentId; `crossing` = that member's **last** ≥q→<q transition
   before the gate (R2-3 — the transition that explains the final below-q state).
2. `SplitAtGate` — no single member ever reached ≥q, but the household TOTAL is ≥q at the gate
   (the pure atomicity wall: same-tick within-household aggregation at the gate instant clears it).
3. `HouseholdDrainedBeforeGate { crossing: EventCause }` — no single member ever reached ≥q and the
   household total is < q at the gate, BUT the household total reached ≥q at some event (R1-1's
   [2,1]→[1,1] case); `crossing` = the household total's **last** ≥q→<q transition before the gate.
4. `NeverReachedQ` — the household total never reached q at any event (peak/gap in the envelope).

`EventCause` enumerates the logical-event causes (Consumption / SettledTrade-sell / AskChange /
Production / …); because the tape is a sequence of atomic logical events, the crossing is the
SINGLE logical event whose post-value takes the tracked quantity below q — never a "larger wins"
heuristic (R1-3, R2-2). The `outcome` is total over {`GatePassed`, `GateDecisionMismatch`, 1–4};
table-driven unit tests cover every branch, the member/household precedence, baseline-at-q, the
crossing identification (AskChange-only drop with no settled trade; a same-household transfer; a
window with both a buy and a sell), the R1-1 split-drain case, both mismatch directions, and the
zero-`GateDecisionMismatch` invariant.

## 4. The report (printed, never asserted) — one typed reducer

`BirthGateReport::from_traces(opportunities) -> BirthGateReport` (R1-7): the SINGLE reducer, which
calls `classify_birth_gate_opportunity` exactly once per raw opportunity and derives EVERY count,
share, histogram, peak/gap distribution, crossing-cause breakdown, and type split from the returned
`ClassifiedBirthGateOpportunity` envelope (R2-6 — no separate trace replay). The printer accepts
only a `BirthGateReport`; it maintains no separate display fields. A plumbing test asserts that
mutating one opportunity's trace moves exactly the corresponding bucket (the C3R.e-obs false-green
guard).

Printed per `(q, arm, producer_type)` stratum aggregated across seeds (arms NOT merged — R1-9),
and per cell: raw `{opportunities, passes, failures}` counts ALWAYS, then the failure-conditional
outcome shares. Zero-denominator strata (every q=0 cell has zero endowment failures; some
type strata may be empty) print `NA (denominator=0)` via a typed `Option<Ratio>`, never `0%`
(R1-9). Report the absolute `household_peak` distribution and `gap_to_q` across ALL failure
opportunities from the envelope (R1-10, R2-6 — no directional histogram expectation), the
`MemberDrained`/`HouseholdDrained` `crossing`-cause breakdown, and the `SplitAtGate` share. Print
the q∈{1,2} → q≥3 shift as the headline contrast (a comparison of strata, not a preregistered
prediction).

## 4a. The behavioral oracle (asserted) — shared DH.b harness, one execution

The DH.b grid must reproduce byte-identically under observation (R1-8), and to prevent a
constants-printing false green the two suites share ONE real pipeline: extract from
`reproductive_burden.rs` a harness `run_burden_grid(observe: bool) -> BurdenGridResult` that runs
each cell through the real `classify_burden_cell` / `synthesize_burden_grid` functions, preserving
the landed loop order. `BurdenGridResult` carries `{ cells, births_by_cell, audit_rows, synthesis,
paired_table, opportunities_by_cell: Option<BTreeMap<CellKey, Vec<RawOpportunity>>> }` — the last
field `None` for `observe=false` and `Some(map)` for `observe=true` (a possibly-empty map — R3-7),
holding each cell's captured raw opportunities WITH producer-type metadata (R2-7). DH.b calls it with `observe=false` (its golden
unchanged); DH.b-obs calls it ONCE with `observe=true` (the `birth_gate_obs` flag set) and:
- DERIVES the 60 audit rows, synthesis, and paired table from the returned structure (never
  printed constants) and asserts them equal to the committed `reproductive_burden_cells.txt`
  golden and the landed synthesis/paired values;
- feeds `opportunities_by_cell` (the SAME single execution) into `BirthGateReport::from_traces` for
  the printed decomposition (R1-14 — the grid is executed once, not twice; disclosed runtime
  ≈ the DH.b ~105s plus the trace overhead, not doubled).

The oracle asserts: the DH.b rungs/synthesis/golden; the totality invariants (passes + failures =
independent opportunities; `GateDecisionMismatch == 0`; and Miller + Baker = global
COMPONENT-WISE for opportunities, passes, failures, every scientific outcome, and each
crossing-cause total, with every raw opportunity resolving to exactly one closure class — R2-9);
the `gate_phase == post_production` equality on the grid; and **machine-enforced non-vacuity
(R2-8): every q=4 cell has `independent_opportunities > 0` and `independent_failures ==
independent_opportunities`**. The share decomposition is printed, never asserted.

**Independent denominator recount (R1-6):** a second counter replays interval + non-empty +
size-cap + hunger-ceiling + the exact stock gate from a pre-`run_births` snapshot (stored
SEPARATELY from `BirthGateObs`), WITHOUT reading the observer or any `birth_block_*` counter, and
asserts: independent failures = observer failures = `birth_block_endowment` delta; independent
passes = observer passes = births delta; passes + failures = independent opportunities; Miller +
Baker = global (component-wise); and no gate PASS reaches the defensive post-selection
`debit_stock` failure path.

## 4b. Digest inertness

The ONLY digest delta is tag 35, ON-only, keyed on `birth_gate_obs_configured()` (`out.push(35);
out.push(u8::from(chain.birth_gate_obs))`, the mod.rs:26965 pattern) — UNAFFECTED by the closure
force-disable, so both twins carry the identical tag (R2-4). Proven by a dedicated test that ON =
OFF + exactly `[35, 1]`. Two inertness tests (R3-5): (a) the CONFIGURED-BUT-INACTIVE test — a
marker-configured run on a non-closed config (`closure_active()` false) steps with identical
behavior/reports, an EMPTY Society tape, and canonical bytes = the flag-OFF bytes + `[35,1]` (NOT
literal equality — a configured run always carries the tag); (b) the FORCE-DISABLE twins test —
placed BESIDE the closure library unit tests (where `closure_ledger_force_disable_for_test` is
visible, closure.rs:1076, applied after construction before the first tick; R1-12) — asserting
that two marker-CONFIGURED runs, one with the closure ledger force-disabled, are LITERALLY
canonical-byte-identical (both configured ⇒ both carry `[35,1]`; the disabled twin's
`birth_gate_obs_active()` is false ⇒ no writes, no behavior change). Flags-off goldens
byte-identical.

## 5. Honest nulls (all publishable) — narrowed to what is measured (R1-11)

- `NeverReachedQ`-dominated at q≥3 → same-tick within-household pooling would NOT clear these
  observed opportunities; upstream supply, acquisition, reservation, and intertemporal mechanisms
  remain unresolved (not "no institution helps" — cross-household and persistent pooling are
  untested follow-ups).
- `SplitAtGate`-dominated → same-tick within-household aggregation would clear the stock threshold
  at the gate instant; behavior under an actual pooling institution remains untested.
- `MemberDrainedBeforeGate` / `HouseholdDrainedBeforeGate`-dominated → the double-duty drain; the
  `crossing`-cause breakdown says whether a stronger reservation, a distinct non-eaten birth good,
  or a sale-lock is the candidate lever (each a follow-up hypothesis, not a measured fix).
- Mixed / type-split (e.g. bakers drained, millers never-reached) → report both; the Miller/Baker
  split is itself the finding.

## 6. Mechanism inventory

Reuse (verify, do not rebuild): the `AllocationRecord` ordered-trace pattern
(econ/src/society.rs:106, enable-once/drain accessors :4690-4716); `free_stock_after_all_reserves`
(econ/src/society.rs:6603, pure); `birth_block_endowment` and siblings (mod.rs:16035, 25541); the
acquisition/closure/burden provenance (all `closure_active()`-gated); the DH.a ClosureClass
Miller/Baker mapping; the enable-once construction gate (mod.rs:11330); the tag-emission pattern
(mod.rs:26965); the DH.b harness + golden.

Build only:
1. `ChainConfig::birth_gate_obs: bool` (default false) + `birth_gate_obs_configured()` +
   `birth_gate_obs_active()` + tag 35 (keyed on configured).
2. The generic `Society` staple-stock event tape + `staple_obs_active`/`staple_obs_good` state.
   `begin_staple_obs_window(staple, active)` is called at `WindowStart` — BEFORE the first
   `regenerate_scales` — and observation stays live through all regenerations and `Society::step`,
   then Settlement drains and disables it immediately afterward (NIT-1). Society emits only
   `Consumption`, `SettledTrade`, and `AskChange` (atomic logical events at the real mutation
   sites: consumption after reservation-restore; settled trade as one two-member group; standalone
   ask change). `Settlement` then appends exactly one `Production` after each successful executor
   return, followed by the gate/`BirthDebit` events (NIT-2). The `Settlement`-side `BirthGateObs`
   join holds the `WindowStart` baseline (after death/estate), the drained Society tape, the
   appended events, and the in-`run_births` gate-state + `event_end` cursor capture.
3. `classify_birth_gate_opportunity` (pure, envelope-returning) + `BirthGateReport::from_traces`
   (the single reducer) + the printed decomposition split by producer type.
4. The shared `run_burden_grid(observe)` harness extracted from `reproductive_burden.rs` with the
   `opportunities_by_cell` field.
5. `sim/tests/birth_gate_obs.rs`: the shared-harness behavioral oracle (byte-identical
   verdicts/synthesis/golden, derived not printed); the classifier table tests (all branches,
   baseline-at-q, both mismatch directions, the split-drain and same-household-transfer cases); the
   plumbing move-a-trace-moves-a-bucket test; the independent denominator recount; the
   `GateDecisionMismatch == 0` + q=4 non-vacuity guards; the tag-35 ON=OFF+2 test; the
   configured-but-inactive byte test. Plus the library-unit force-disable-twins byte test beside
   the closure tests.
6. **A LIVE-EMITTER test battery (R3-3)** — proving the ATOMIC emission against the real
   `step_m1`/`apply_trade` semantics (constructed classifier tables + terminal reconciliation
   cannot catch a phantom intermediate peak): reserved-stock consumption emits exactly one
   post-restore `Consumption` (no mid-strip dip); an immediately-filled ask emits NO standalone
   reservation dip and exactly one two-member `SettledTrade`. SEPARATE exact-sequence/free-value
   tests are required for each distinct ask path (NIT-3): unfilled posting, ordinary/rewrite
   cancellation, TTL expiry (which releases via `OrderBook::purge_expired`), and the
   `regenerate_scales` cancellation — each emits `AskChange`. The same-household case uses a
   RESTING staple ask followed by an incoming bid, proving the release-only seller peak in
   `apply_trade` (econ/src/market.rs:653) never escapes as an event. Each successful recipe
   application emits exactly one post-executor `Production`. All assert the emitted event SEQUENCE
   and per-event free values on real micro-runs, not constructed traces.

## 7. Acceptance criteria

1. Flags-off goldens byte-identical; conservation every tick; ALL landed suites reproduce —
   including the DH.b grid byte-identical under observation (verdicts, synthesis, `reproductive_
   burden_cells.txt` golden), derived through the shared harness.
2. The ONLY digest delta is tag 35 ON-only, keyed on `configured`: a configured run is
   behavior-inert and tape-empty when `closure_active()` is false, with `[35,1]` as the sole
   configured-vs-OFF byte delta; the two configured force-disable twins are literally
   canonical-byte-identical.
3. The observer is behavior-inert (the DH.b oracle proves it) and never read by a decision path:
   event payloads are computed through pure reads (`free_stock_after_all_reserves`), and emission
   mutates only runtime observer state — never economic, decision, or canonical state.
4. `classify_birth_gate_opportunity` pure, total, envelope-returning; `BirthGateReport::from_traces`
   the sole path to every printed share; table tests over every branch/precedence/crossing-event/
   baseline/mismatch/boundary; the plumbing test; the LIVE-emitter battery (§6.6, R3-3) proving the
   atomic emission against `step_m1`/`apply_trade`; `GateDecisionMismatch == 0` and q=4 non-vacuity
   grid-wide; the independent recount's full component-wise equality set; the WindowStart→gate
   replay reconciles with the authoritative gate state.
5. The decomposition printed per `(q, arm, producer_type)` and per cell with raw counts always and
   `NA` for zero denominators; the peak/gap distribution and crossing-cause breakdown; never
   asserted.
6. Runtime disclosed; the grid executed ONCE; fmt/clippy/test gates green.

## 8. Out of scope

Any institution or behavior change (pure observation). Any change to the birth gate, the closed
base, throughput, consumption, or the DH.b grid. The next lever the decomposition selects
(pooled-stock/pooled-heir, income-side, or reservation-strengthening) is DH.b-obs+1, chosen by
THIS slice's measured shares.
