# impl-28 — S21e: Finite Seeded-Surplus Probe (does a pre-promotion tradeable supply monetize SALT?)

Status: LANDED @ `fd8f2aa` (+ follow-up) — a **successful bounded diagnostic** (a finite surplus on a
WOOD-poor seller class is sufficient: SALT promotes before exhaustion, ~99% SALT-mediated, production
replaces the seed). Codex review-of-results **PASS-with-caveats** (no P0/P1; the success is genuine,
not tuned — verified by the sweep window + cross-seed). P1/P2 addressed: a same-shape control
(WOOD-poor, seed removed → no promotion) isolates the seed as load-bearing; the SALT-share bar raised
(headline ≥9000 bps, observed 9947); the sweep window pinned (no non-exhausting size); the mislabeled
"marketability off" control corrected (it actually STILL promotes — the holding rule is *not*
load-bearing here, asserted as an honest nuance); an explicit post-promotion grain/flour input-trade
assertion added. Spec-review history: round 1 NEEDS-REVISION → round 2 SPEC-READY.
Branch: `feat/seeded-surplus-probe`
Base: master @ `c2f42e4` (S21d landed)

## 0. What this milestone is — a bounded diagnostic counterfactual

S21d landed a Phase A finding: retiring the food mints collapses the pre-promotion barter market
to **zero trades** — SALT never promotes. The block is at offer **generation**, not acceptance: in
S21d every agent's only bread is a *reserved near-want* (it wants to eat it), so `barter_swap_acceptable`
(`econ/src/agent.rs:542`) rejects *giving* it — no sell lane posts — so SALT's spend lanes have no
counterparty (the indirect-acceptance predicate is acquirability-blind, `agent.rs:478`).

S21e is the **clean causal counterfactual** Codex's direction review prescribed: seed a finite,
decaying bread **surplus** (bread held *beyond* a class's own hunger want, so it is offerable
surplus, not reserved food) — i.e. **replace the recurring mint's bread supply with a one-time
finite seed** — and ask: *is a real pre-promotion tradeable food supply sufficient for SALT to
promote under S20/S21a/b/c, mints retired, mortality off?*

It is a **deliberately bounded diagnostic scaffold, not the final authentic mechanism** (that is
S21f, pre-money household production-for-barter). The three-way diagnostic (every outcome informative):
- **SALT promotes and production replaces the seed** → supply was the missing piece; the open
  colony works on a finite (non-recurring) food supply.
- **SALT promotes on the seed but production never takes over / collapses at exhaustion** → Phase B
  (the producer bootstrap) is the next blocker.
- **SALT never promotes despite real sellers** → S21d was not "just supply"; the topology/medium-
  demand path is itself insufficient.
- **(guard) No sell lanes ever post** → the probe is VACUOUS / mis-built (the seeded class is not a
  real seller); this is a **bad-probe condition to fix, NOT an economic finding** (Codex P2.5).

## 1. Mechanism — a HYPOTHESIS to verify with settlement-level traces (not assumed)

The existing `two_lane_microtest_off_deadlocks_on_clears` (`econ/src/society.rs:8808`) proves the
barter book *can* clear the imperfect-coincidence shape (off → 0 trades; on → 3 trades, 3 SALT
`IndirectFor`) — but with *synthetic* agents carrying explicit `Horizon::Next` input wants + SALT
stock. The open-survival colony uses **need-scale** agents (hunger/warmth/SALT-direct), so the same
lanes are a **hypothesis that must be confirmed at the settlement level**, not a narrative
assumption (Codex P2.4). The hypothesized path: a surplus-bread seller that *wants WOOD* sells
bread → SALT (`IndirectFor{WOOD}`) to a SALT-rich consumer (who offers no WOOD), then spends
SALT → WOOD on a gatherer — SALT accruing indirect breadth `{WOOD, bread}`. The milestone must
*trace* this in the settlement, not infer it from the microtest.

Two preconditions the barter code imposes on a real seller (both must hold for the seeded class):
- **offerable surplus:** bread above the reserved hunger allocation, so giving it passes
  `barter_swap_acceptable` (`agent.rs:542`, `preserved_near_allocations_above_target` `:876`).
- **an unsatisfied non-SALT target:** `generate_candidate_indirect_barter_offers`
  (`econ/src/society.rs:2374`) posts a sell-for-medium lane only if the agent has a near
  unsatisfied target good (WOOD). A bread-surplus holder that is *also WOOD-satisfied posts
  nothing.*

## 2. Engine + scenario changes

### S21e.0 — identify the real seller, then seed it (Codex P1)

**First, instrument & pin the seller from the mints-on control.** Add a runtime-only
seller-provenance trace (or extend `BootstrapTrace`/the barter trace) that records, in the
**mints-on control**, *which* agents sell bread, with what `BarterReason`, and to whom. Pin that
class (by vocation / lineage path) — do NOT punt to "whichever sells." Research shows the
non-consumer `bread_buffer` holders are the intended barter-window sellers, but they also receive
`wood_buffer = 48` (`settlement.rs:13724`) so they may be WOOD-satisfied; the demographic mint
recipients (`gen_household_member` ~`:14049`) also get WOOD provision. The trace resolves which is
actually the seller.

**Then seed that exact class so it is a REAL seller:** add `pub seeded_surplus_bread: u32` to
`ChainConfig` (default 0 in all three constructors for byte-identity), applied as a one-time
`stock.add(staple, seeded_surplus_bread)` to the pinned seller class, AND ensure that class is
**WOOD-poor enough to have an unsatisfied WOOD target** in the promotion window (reduce its WOOD
buffer for this scenario if the trace shows it is WOOD-satisfied). This WOOD-poor change means S21e
is a **second diagnostic config**, not "only `seeded_surplus_bread` differs" (Codex P2.2 — disclose
it explicitly).

**Pre-promotion non-vacuity assertion (mandatory):** at some pre-promotion tick, **≥ N seeded
sellers simultaneously hold offerable bread surplus AND an unsatisfied WOOD target, and ≥ 1
`bread → SALT IndirectFor{WOOD}` lane is live or cleared.** If this fails, the run is a *bad probe*
(redesign), not a finding.

### Honesty: "finite" = offerable-surplus EXHAUSTION, not "some decay" (Codex P1)

`run_spoilage` (`settlement.rs:9457`) decays bread only *above* `FREE_STORAGE = 20` per holder
(`:9499`); the bottom 20/holder is permanent. "Decays" ≠ "ceases to scaffold." Define exhaustion
explicitly and assert it:
- `seeded_offerable_surplus_exhausted_tick` = first tick at which **no holder has seeded-origin
  bread above its protected hunger allocation** (i.e. no seeded *offerable* surplus remains; the
  residual ≤ floor that can only be eaten is not a market scaffold).
- **Pin the computation precisely (Codex round 2 P2).** "Offerable" must mean *removable under the
  real barter preservation rule*, not a loose approximation: a seeded-origin bread unit is offerable
  iff giving it still passes `barter_swap_acceptable` / `preserved_near_allocations_above_target`
  (`econ/src/agent.rs:542`/`:876`). Since the sim ledger tracks seeded *lots* separately while the
  value scale protects *undifferentiated* bread, compute this via a small econ helper (or read it
  off the actual offer/lane trace) — do NOT re-derive provisioning logic loosely in the sim.
- A seed sized so large it never reaches that state within the run is a **hidden permanent mint** —
  the sweep (below) must include sizes that DO exhaust, and the headline result must promote at a
  size that exhausts.

Labeling is automatic: `maybe_init_acquisition_ledger` (`settlement.rs:10727`) sweeps all generated
bread into the `SeededMinted` channel; assert the surplus is `SeededMinted` and that the channel
depletes.

## 3. The scenario — `frontier_seeded_surplus` (a `SettlementConfig`)

Derive from `frontier_open_survival` (`settlement.rs:3795`); the disclosed differences:
- `seeded_surplus_bread = <S>` (S > 20/holder margin, sized to exhaust within the run).
- the pinned seller class made WOOD-poor enough to want WOOD (disclosed second diagnostic axis).
- everything else identical: `retire_food_mints = true`, no forage, mortality off, S20 + S21a +
  S21b + S21c, bread⇄WOOD topology, the S9 strong-bar gates (disclose values).

The primary counterfactual is **vs the S21d collapse** (`frontier_open_survival`, no surplus →
0 trades). The **mints-on case is a positive control**, not the primary comparison (it restores a
recurring provision path, `settlement.rs:7919`/`:8574`).

## 4. Falsifiable bar + controls

Classify (seed 7, 1600 ticks):
- **Non-vacuity (gate):** the §2 pre-promotion seller assertion holds (real sellers exist + a
  cleared `bread→SALT IndirectFor{WOOD}` lane). If not → bad probe, fix it.
- **Pre-promotion barter volume > 0** (collapse lifted).
- **SALT promotes** (`current_money_good() == Some(SALT)`), is the **medium** leader, with indirect
  breadth including the **non-food WOOD** target.
- **Success rests on SALT-mediated volume, not direct barter (Codex round 2 P2).** Making sellers
  WOOD-poor can make a *direct* bread↔WOOD swap attractive via the S21c legacy direct-discovery lane
  (when bread/WOOD are below the candidate floor). Report **direct bread↔WOOD volume vs
  SALT-mediated bread/WOOD volume**, and require the SALT-mediated share to be material — a "success"
  must not be mostly direct barter with a token amount of SALT.
- **Promotes before seed exhaustion** (`promotion_tick < seeded_offerable_surplus_exhausted_tick`).
- **Production replaces the seed (the tail bar, Codex P2.3):** after
  `seeded_offerable_surplus_exhausted_tick` — `bread_produced > 0`, real input trades occur, tail
  food consumed is `bought`/`self_produced` (not `seeded_minted`), and (for Phase B success)
  `bids_posted_after_recent_buy > 0`. If promotion happens but the tail rides/collapses-without the
  seed → **Phase B finding** (localize with the cross-tick `BootstrapTrace`).

Controls (classify, never tune):
- **no seeded surplus** → S21d zero-trade collapse.
- **mints ON** → old scaffolded market (positive control, not success).
- **money levers off** (two-layer / marketability / multi-offer) → no promotion.
- **seed-size SWEEP** → promotion holds across a *window* of exhausting seed sizes (report the lower
  boundary where it stops promoting and the upper boundary where the seed stops exhausting); proves
  a window, not a tuned point, and that no size is a hidden permanent mint.
- **no SALT direct-use anchor** → no promotion (regression-theorem grounding).

## 5. Slices

- **S21e.0** — seller-provenance trace (runtime-only) + pin the seller class from the mints-on
  control; the `seeded_surplus_bread` field (gated, default 0; canonicalized ON-only — it changes
  starting stock = future behaviour — with a `canonical_bytes_include_seeded_surplus_bread`
  regression) + the WOOD-poor seller adjustment.
- **S21e.1** — the `frontier_seeded_surplus` scenario + the non-vacuity precondition, the
  seeded-channel labeling, and the offerable-surplus-exhaustion instrument & assertion.
- **S21e.2** — the classification suite + the run: assert the bar OR classify (Phase B /
  still-no-promotion / bad-probe) with settlement-level lane/trade traces; the full control matrix
  incl. the seed-size sweep; determinism; a live run.

## 6. Determinism / golden contract

- `seeded_surplus_bread` default 0 + the WOOD-buffer change scoped to the new scenario ⇒ all 18
  golden suites byte-identical; `seeded_surplus_bread` canonicalized ON-only with a digest
  regression; the seller-provenance trace + acquisition ledger stay runtime-only (excluded from
  `canonical_bytes`).
- `cargo fmt --check` + `clippy --workspace --all-targets -- -D warnings` clean; conservation every
  tick; deterministic.

## 7. Honest scope

S21e is a **bounded diagnostic counterfactual**, not the authentic mechanism: the seed is a
one-time scaffold that must (a) be *finite* — proven by offerable-surplus exhaustion, not mere
decay (the bottom 20/holder is permanent and must not be load-bearing) — and (b) actually create
sellers (the non-vacuity gate). It is a **second diagnostic config** (WOOD-poor sellers), disclosed.
A positive result shows *supply sufficiency*, not endogenous pre-promotion production; the authentic
follow-up is **S21f (pre-money household production-for-barter)**. A Phase B result localizes the
producer bootstrap. A no-promotion-with-real-sellers result reopens the topology question. The
direct-use anchor + thresholds remain configured and disclosed.

## 8. Pipeline

rb-lite `codex,claude` (S21e.0→.2) → independent verification (workspace + all 18 goldens
byte-identical + the new suite + a live run) → Codex review-of-results → merge + report/memory + pin.
