# impl-28 — S21e: Finite Seeded-Surplus Probe (does a pre-promotion tradeable supply monetize SALT?)

Status: DRAFT (pre-Codex-spec-review)
Branch: `feat/seeded-surplus-probe`
Base: master @ `c2f42e4` (S21d landed)

## 0. What this milestone is — a bounded diagnostic counterfactual

S21d landed a Phase A finding: retiring the food mints collapses the pre-promotion barter
market to **zero trades** — SALT never promotes. The research for S21e located the *exact*
block, and it is NOT what the S21d narrative first implied:

> **The block is at offer GENERATION, not acceptance.** In S21d every agent's only bread was a
> *reserved near-want* (it wants to eat it), so `barter_swap_acceptable` (`econ/src/agent.rs:542`)
> rejects *giving* it — no sell lane can post — so SALT's spend lanes have no counterparty and the
> book clears nothing. The indirect-acceptance predicate
> (`would_accept_indirect_barter_swap_with_stock`, `agent.rs:478`) is **acquirability-blind** (it
> reads only the acceptor's own scale + post-swap stock, never the order book), so "no bread to
> later buy" was never the literal gate.

S21e is the **clean causal counterfactual** Codex's direction review prescribed: seed a finite,
decaying bread **surplus** (bread held *beyond* a class's own hunger want, so it is offerable
surplus, not reserved food) — i.e. **replace the recurring mint's bread supply with a one-time
finite seed** — and ask: *is a real pre-promotion tradeable food supply sufficient for SALT to
promote under S20/S21a/b/c, with the mints retired and mortality off?*

It is a **deliberately bounded diagnostic scaffold, not the final authentic mechanism** (that is
S21f, pre-money household production-for-barter). Every outcome is informative (the three-way
diagnostic):
- **SALT promotes** → confirmed: the missing piece was purely pre-promotion *supply*.
- **SALT promotes but post-promotion production deadlocks** → Phase B (the producer bootstrap) is
  the next blocker.
- **SALT still does not promote** → S21d was not "just supply"; the topology/medium-demand path is
  itself insufficient.

## 1. Why a surplus should unblock the collapse (the mechanism, verified)

- **Surplus posts a sell lane.** The give-side of every offer is the agent's full
  `positive_goods()` (`econ/src/good.rs:118`, no want filter); a surplus good clears
  `barter_swap_acceptable` precisely because giving it does not un-provision a reserved near-want
  (`preserved_near_allocations_above_target`, `agent.rs:876`). So bread held *beyond* the eating
  want DOES post a sell offer — the structural fix to the S21d collapse.
- **SALT monetizes via the SALT-rich consumer demand hub.** The consumers hold SALT and want
  bread+WOOD but offer no WOOD; so a surplus-bread seller that wants WOOD sells bread → SALT
  (`IndirectFor{WOOD}`) to a consumer, then spends SALT → WOOD on a gatherer (who sells WOOD →
  SALT `IndirectFor{bread}`). SALT accrues indirect breadth `{WOOD, bread}` — one non-food target.
  This is the imperfect-coincidence bridge the existing `two_lane_microtest_off_deadlocks_on_clears`
  (`econ/src/society.rs:8808`) already proves (off → 0 trades; on → 3 trades, 3 SALT `IndirectFor`).
- **Provisional-leader bootstrap.** Under two-layer, `provisional_media_candidates` filters only by
  the direct-use floor (`menger.rs:333`); SALT becomes a candidate as soon as it clears that floor,
  so sell lanes post. Final *promotion* is still gated by the S9 breadth floor in `base_eligible`
  (`menger.rs:306`) — disclose the exact values.

## 2. The engine change — one gated field (minimal)

Add `pub seeded_surplus_bread: u32` to `ChainConfig` (near the buffer fields, `settlement.rs:~1169`),
default `0` in all three constructors (~1280/~1377/~1466) for byte-identity. Apply it as a one-time
`stock.add(staple, chain.seeded_surplus_bread)` to the **bread-seller class(es) the retired mints
used to feed** — i.e. mirror the mint recipients: the lineage/household members (the demographic
`food_provision` recipients, seeded in `gen_household_member` ~`settlement.rs:14049`) and/or the
producer pool, whichever the mints-on control shows actually *sells* bread into the market. The
implementation must verify (via the bootstrap/ledger traces) that the seeded class posts sell lanes
(otherwise the surplus is held idle and the probe is vacuous).

**Two honesty constraints baked into the design:**
1. **The seed must exceed the spoilage free-storage floor.** `run_spoilage` (`settlement.rs:9457`)
   decays bread only *above* `FREE_STORAGE = 20` per holder (`:9499`). A seed ≤ 20/holder would be a
   **permanent, non-decaying scaffold** — defeating "finite." The seed per holder must clear 20 by a
   meaningful margin so the surplus genuinely depletes (the probe tests promotion *before*
   exhaustion). Assert the seeded channel actually decays over the run.
2. **Labeled `seeded`, automatically.** `maybe_init_acquisition_ledger` (`settlement.rs:10727`,
   called each tick at `:6517`) sweeps *all* generated bread into the `SeededMinted` channel on the
   first active tick; with mints retired that channel only depletes. No extra labeling code; assert
   the surplus shows up as `SeededMinted` (not produced/bought) and depletes.

## 3. The scenario — `frontier_seeded_surplus` (a `SettlementConfig`)

Derive from `frontier_open_survival` (`settlement.rs:3795`), changing ONLY:
- `seeded_surplus_bread = <S>` (a disclosed value > 20/holder margin).
- Everything else identical: `retire_food_mints = true`, no forage, mortality off, S20 two-lane +
  S21a marketability + S21b two-layer + S21c open-discovery, the bread⇄WOOD topology, the S9
  strong-bar gates (disclose values). This makes the *only* difference from the S21d collapse the
  finite seeded surplus — the cleanest possible counterfactual.

## 4. Falsifiable bar + controls (Codex direction)

Classify the run (`open_survival`-style harness, seed 7, 1600 ticks):
- **Pre-promotion barter volume > 0** (the collapse is lifted).
- **SALT promotes before the seeded surplus is exhausted** (`current_money_good() == Some(SALT)`
  while `seeded` channel still > 0; if it promotes only after, or never, classify accordingly).
- SALT is the **medium** leader (`medium_leader_shares().good == SALT`), FOOD may lead total.
- **Pre-promotion indirect breadth includes the non-food WOOD target** (not bread-only).
- **After promotion**: the seeded channel depletes AND real production / input trades begin
  (`bread_produced > 0` post-promotion; `bought`/`self_produced` consumption rises as `seeded`
  falls) — i.e. the economy transitions off the seed, not rides it. If production never starts,
  classify as a **Phase B finding** (supply was sufficient for promotion but the producer bootstrap
  still deadlocks — localize with the cross-tick `BootstrapTrace`).

Controls (each fails the right way — classify, never tune):
- **no seeded surplus** (`seeded_surplus_bread = 0`) → reproduces the S21d zero-trade collapse.
- **mints ON** → the old scaffolded market (not a capstone success).
- **money levers off** (two-layer / marketability / multi-offer) → no promotion.
- **seed-size SWEEP** → the result holds across a *window* of seed sizes (e.g. {small-but->20, mid,
  large}), proving it is not a single tuned point; report the boundary where it stops promoting.
- **no SALT direct-use anchor/seed** → no promotion (regression-theorem grounding).

## 5. Slices

- **S21e.0** — `seeded_surplus_bread` field (gated, default 0; applied to the mint-recipient
  seller class; byte-identical goldens; canonicalized ON-only if it enters future behaviour — it
  changes the starting stock, so gate it ON-only in the relevant `push_*_config_bytes` and add a
  `canonical_bytes_include_seeded_surplus_bread` regression).
- **S21e.1** — the `frontier_seeded_surplus` scenario + the seed-decays + seeded-channel-labeling
  assertions (the surplus is finite, decays, labeled seeded, and the seeded class posts sell lanes).
- **S21e.2** — the classification suite + the run: assert the bar OR classify (Phase B / still-no-
  promotion) with traces; the full control matrix incl. the seed-size sweep; determinism; a live run.

## 6. Determinism / golden contract

- `seeded_surplus_bread` default 0 ⇒ all 18 golden suites byte-identical; canonicalized ON-only
  (it changes the starting stock = future behaviour) with a digest regression; the acquisition
  ledger stays runtime-only (excluded from `canonical_bytes`).
- `cargo fmt --check` + `clippy --workspace --all-targets -- -D warnings` clean; conservation
  asserted every tick; deterministic.

## 7. Honest scope

S21e is a **bounded diagnostic counterfactual**, not the authentic mechanism: the seed is a
one-time scaffold (finite + decaying, and required to exceed the 20-unit free-storage floor so it
genuinely depletes — not a permanent floor). A positive result shows *supply sufficiency*, not that
the colony endogenously produces its pre-promotion food; the authentic follow-up is **S21f
(pre-money household production-for-barter)**. A Phase B result localizes the producer bootstrap as
the next gate. The direct-use anchor + thresholds remain configured and disclosed.

## 8. Pipeline

Codex spec-review → SPEC-READY → rb-lite `codex,claude` (S21e.0→.2) → independent verification
(workspace + all 18 goldens byte-identical + the new suite + a live run) → Codex review-of-results
→ merge + report/memory + pin.
