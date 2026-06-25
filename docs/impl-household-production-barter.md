# impl-29 — S21f: Endogenous Pre-Money Household Production-for-Barter

Status: IMPLEMENTED — a **SUCCESS**: endogenous cultivated (`SelfProduced`) bread monetizes
SALT with NO `SeededMinted` bread anywhere. The gated `household_barter_cultivation` seam runs
cultivation steering + the own-use phase + the woodcutter→WOOD routing WITHOUT the forage
substrate; the `frontier_household_barter` scenario (zeroed bread buffers + `starting_food`,
spatial lineages, grain commons 480/24/960) promotes SALT at tick ~71 (seed 7) on cultivated
supply, indirect breadth {bread, WOOD}, SALT-mediated share ~9777 bps (≥9000 headline bar),
robust across seeds {3,7,11,19,23}. Cold-start classification = Success (spatial lineage member
cultivates → hauls grain → produces `SelfProduced` bread → cleared pre-promotion `bread → SALT
IndirectFor{WOOD}` lane). All 19 goldens byte-identical (flag default off, canonicalized ON-only
with tag 4 + a digest-split regression). Controls: cultivation-off → S21d zero supply (no
promotion); no-WOOD-poor-lineage / two-layer-off / multi-offer-off → no promotion; buy/sell-split
off → STILL monetizes (classified finding — the WOOD market keeps SALT alive; the split only
scopes the supply to lineages, producing ~5× more bread when off). Grain-flow sweep: zero flow →
no bread/no promotion; a finite stock with NO regen (240/0/240) → 236 cultivated bread once
(produced == consumed grain, the grain-bounded identity) then dry, INSUFFICIENT to promote
(recurring flow needed, NOT seed exhaustion); recurring flow → promotes on `SelfProduced`. `cargo
fmt`/`clippy --workspace --all-targets -D warnings` clean; deterministic; conservation every tick.
Prior: SPEC-READY (Codex round 1 NEEDS-REVISION → round 2 NEEDS-REVISION [spatial_households + steering seam] → addressed: demography.spatial_households=true; the activation seam pinned across run_own_labor_subsistence + the config/runtime/multigood active predicates with FORAGE-code guarded; WOOD via role separation not node deletion; 3-way cold-start classification)
Branch: `feat/household-production-barter`
Base: master @ `6856297` (S21e landed)

## 0. What this milestone is — the authentic supply mechanism

S21e proved (bounded diagnostic) that a finite *seeded* pre-promotion bread supply is sufficient
for SALT to monetize under market-financed survival, then for production to take over. The seed was
a one-time scaffold. **S21f makes that pre-promotion supply ENDOGENOUS:** lineage households
*cultivate* bread by their own labor, eat what they need, and barter the surplus — *before money
exists* — so SALT emerges from barter over genuinely produced surplus (the Mengerian /
regression-theorem bootstrap: direct production-for-use + barter of surplus precedes money).

The decisive new bar vs S21e: **the pre-promotion bread sold for SALT must be `SelfProduced`
(cultivated), not `SeededMinted` (seeded).** The acquisition ledger already distinguishes these:
`run_own_use_cultivation` credits `FoodChannel::SelfProduced` (`settlement.rs:9042`), and
`transfer_as_bought` books the seller-origin breakdown on sale (`settlement.rs:5134`).

Unlike S21e, this is **recurring production from a real depleting grain commons**, not a one-time
seed — so there is **no "exhaustion" framing**; the honesty guard is that produced bread is *bounded
by real grain input* (`consumed_as_input[grain]` bounds `produced[bread]`), and that money rests on
`SelfProduced` supply.

## 1. The engine change — a gated cultivation-WITHOUT-FORAGE activation seam (Codex P1)

S21f is **not** scenario-only (the original draft was wrong): `own_use_cultivation_active()` /
`chain_runtime_own_use_cultivation_active` (`settlement.rs:10633`, `:14396`) requires
`own_labor_subsistence && content.forage().is_some()` **in addition to** `own_use_cultivation` +
the `Cultivate` recipe. So composing `with_cultivate + own_use_cultivation + cultivation_sells_surplus`
onto `frontier_open_survival` alone would **silently not cultivate**.

The fix is a **gated activation/steering seam, NOT a pre-market production seam, and NOT dummy FORAGE**
(dummy forage would pollute the value scale with `known.subsistence` and reopen the S12 confusion).
Add a gated `ChainConfig::household_barter_cultivation: bool` (default `false`); when set, pin ALL of
these (Codex round 2 P1 — the seam must cover *steering*, not only the active predicate):
- **Run the steering phase:** `run_own_labor_subsistence` (`settlement.rs:8786`) currently returns
  unless `own_labor_subsistence_can_run()`. Change its guard to run when
  `own_labor_subsistence_can_run() || household_barter_cultivation_active()`.
- **Guard the FORAGE-specific code:** load/use the `forage` good, `forage_yield`, completed-forage
  credit, and forage flags **only** on the own-labor/forage path — so the household-barter path runs
  cultivation steering with **no FORAGE good interned** (no `known.subsistence` pollution).
- **Set `cultivating` directly from sustained hunger** for eligible lineage members on the
  household-barter path (reuse the `cultivate_hunger_in`/`cultivate_patience` hysteresis,
  `settlement.rs:8903-8917`), scoped lineage-only via the existing `cultivation_sells_surplus`
  buy/sell split (so the SALT-rich non-lineage consumers stay pure demand).
- **Include the flag in every cultivation/multigood active predicate** so validation, runtime, and
  generation-time WOOD pinning do not diverge: `chain_config_own_use_cultivation_active`,
  `chain_runtime_own_use_cultivation_active` (`settlement.rs:10633`/`:14396`), and
  `config_multigood_money_active`.
- **`Cultivate` stays POST-market** — the surplus sells **cross-tick** (cultivate tick `t` → surplus
  persists in stock → barter tick `t+1` posts the sell lane). No pre-market production seam.
- Default off ⇒ all 19 goldens byte-identical; canonicalized ON-only (it changes production) with a
  digest regression.

## 2. The trigger (cold-start) — the open colony supplies its own hunger pressure

With the food mints retired and the cold-start bread buffers zeroed (§3), the colony is
hunger-stressed: hunger climbs past `cultivate_hunger_in (6)` (mortality off → agents stay hungry,
don't die), so the hunger hysteresis fires and lineage members cultivate. The spec must **verify
cultivation actually runs** (a non-vacuity assertion: cultivators produce `SelfProduced` bread), not
assume it. If cultivation never fires (buffers keep hunger < 6), tune the buffers down or the
hunger-in threshold (disclosed), not the result.

## 3. The scenario — `frontier_household_barter` (a `SettlementConfig`)

Derive from `frontier_open_survival` (`settlement.rs:3802`); the disclosed differences:
- **Keep** the S21d/e money machinery: `retire_food_mints = true`, `acquisition_ledger = true`,
  `multi_offer_medium` (S20), `durability_aware_acceptance` + the marketability table (S21a),
  `two_layer_saleability` + `min_direct_use_acceptors` (S21b), the S9 strong-bar gates,
  `min_indirect_target_goods = 2`, mortality OFF.
- **No seed, no cold-start bread (Codex P1):** `seeded_surplus_bread = 0`, **`bread_buffer = 0`,
  `consumer_staple_buffer = 0`** — so NO bread enters as `SeededMinted`. (Protected non-bread
  startup goods are fine; the claim is specifically that pre-promotion *bread* supply is endogenous.)
- **Enable spatial households (Codex round 2 P1):** `demography.spatial_households = true`
  (`frontier_open_survival` inherits `false` from `frontier`, `settlement.rs:2645`); without it
  lineage members never become spatial cultivators (eligibility needs `household.is_some() &&
  spatial_active`, `:8856`) and S21f is inert.
- **Turn on endogenous cultivation:** `content.with_cultivate()`, `household_barter_cultivation = true`
  (the §1 activation seam), `cultivation_sells_surplus = true` (lineage-only buy/sell split), the S15
  cultivation knobs (`cultivate_consume`, `cultivate_hunger_in/out`, `cultivate_patience`).
- **Pinned role topology (Codex P2 — pin, don't describe; mirror S18 `frontier_multigood`, NOT S16):**
  - lineage household members = the **cultivators / bread sellers** (WOOD-poor: `wood_buffer = 12`,
    lineage `starting_wood`/`wood_provision` zeroed → an unsatisfied WOOD target → they post
    `bread → SALT IndirectFor{WOOD}`);
  - non-lineage `Consumer`s = the **SALT-rich buy side** (`consumer_medium_endowment = 80`),
    **not cultivation-eligible** (the buy/sell split keeps them pure demand);
  - `Gatherer`s = **woodcutters**, present and pinned to the **WOOD node** via `multigood_money = true`
    so WOOD is genuinely gathered and sold.
  - **Do NOT delete the WOOD node** (that was S16 `frontier_money_from_cultivation`'s artifact fix and
    is wrong here — S21f needs the live WOOD market so cultivators can reach WOOD via SALT). Neutralize
    the WOOD<bread lowest-good-id offer-ordering artifact by **role separation + zeroed lineage WOOD
    surplus** (so a lineage cultivator's only offerable surplus is bread), not by removing WOOD.
- **Grain flow (the disclosed recurring-supply axis):** the grain node `regen`/`stock`/`cap` sets the
  cultivated-bread flow. The base inherits a generous grain node (~8000/64/8000); S21f should pin a
  disclosed value (the S15 commons is 120/4/300). This is a *real depleting commons*, recurring by
  design — the sweep (below) proves promotion needs a real flow (not "no permanent mint").

## 4. Falsifiable bar + controls

Classify (seed 7, 1600 ticks):
- **Non-vacuity (gate) + 3-way cold-start classification (Codex round 2 P2):** assert the full chain
  — at least one lineage member is **spatial**, becomes `cultivating`, **hauls grain**, and produces
  `SelfProduced` bread — then classify:
  - cultivation **never starts** (no spatial cultivator / no `cultivating`) → **bad probe /
    activation failure** (fix the seam or buffers; NOT an economic finding);
  - cultivation starts but **no offerable surplus** (all bread eaten) → **production-flow finding**
    (grain flow / `cultivate_consume` too tight);
  - surplus exists but **no cleared `bread → SALT IndirectFor{WOOD}` lane** → **barter/topology
    finding** (the WOOD-target / role separation didn't compose).
  A success requires the chain through to ≥1 cleared pre-promotion `bread → SALT IndirectFor{WOOD}`
  lane whose bread is `SelfProduced`.
- **Cross-tick non-vacuity test (Codex P2):** a cultivator produces `SelfProduced` bread at tick `t`;
  at tick `t+1` its above-reserve bread is visible as a live or cleared `bread → SALT
  IndirectFor{WOOD}` offer (proves the post-market→next-tick sale path works).
- **Endogenous supply (the core claim):** pre-promotion bread sold for SALT is `SelfProduced`, with
  **`SeededMinted` bread sold pre-promotion == 0** (no seed, no mint, zeroed buffers).
- **SALT promotes** as medium leader, indirect breadth includes the non-food WOOD target,
  SALT-mediated share dominant (reuse S21e's `HEADLINE_MIN_SALT_SHARE_BPS`).
- **Production is grain-bounded, not minted (Codex P2):** `produced[bread]` is bounded by
  `consumed_as_input[grain]` (real commons input), and `SeededMinted`/`Foraged` food consumed ≈ 0 in
  the tail — production is genuinely from cultivation, recurring, never a mint.
- **Self-sustaining:** food consumed is `SelfProduced`/`Bought` through the run; conservation every
  tick. (Stronger, optional: post-promotion specialized chain roles adopt + buy inputs by market.)

Controls (classify, never tune):
- **cultivation off** (`household_barter_cultivation=false`) → the S21d zero-trade collapse.
- **seeded-surplus (S21e)** → positive control (seeded supply works); S21f matches it with
  *cultivated* supply.
- **buy/sell split off** (`cultivation_sells_surplus=false`) → consumers self-cultivate → buy-side
  collapse → no monetization.
- **no WOOD-poor target** → cultivators don't post the medium lane → no/weaker monetization.
- **two-layer off / multi-offer off** → no promotion.
- **grain-flow sweep (Codex P2):** zero grain flow → no produced bread / no promotion; low flow →
  cultivation but insufficient monetization; a middle window → promotion on `SelfProduced` bread;
  high flow reported but not used to define the claim. Assert produced bread tracks
  `consumed_as_input[grain]` (real node input bounds it) — recurring production, NOT seed exhaustion.

## 5. Slices

- **S21f.0** — the gated `household_barter_cultivation` activation seam (cultivation without the
  FORAGE/own-labor substrate; lineage hunger-triggered; `Cultivate` stays post-market). Default off;
  goldens byte-identical; canonicalized ON-only with a digest regression.
- **S21f.1** — the `frontier_household_barter` scenario (compose the money machinery + cultivation +
  zeroed bread buffers + pinned roles + grain flow) + the cross-tick non-vacuity test.
- **S21f.2** — the classification suite + the run: assert the bar (endogenous `SelfProduced` supply
  monetizes SALT) OR classify; the control matrix incl. the grain-flow sweep; determinism; cross-seed
  robustness; a live run.

## 6. Determinism / golden contract

- `household_barter_cultivation` defaults off; the new scenario is additive; **all 19 golden suites
  byte-identical**. The flag is canonicalized ON-only (it changes production) with a digest
  regression; cultivation/acquisition traces stay runtime-only.
- `cargo fmt --check` + `clippy --workspace --all-targets -- -D warnings` clean; conservation every
  tick; deterministic.

## 7. Honest scope

S21f is the **authentic supply mechanism** the S21d→e arc pointed to: endogenous pre-money
production (cultivated `SelfProduced` bread), not a seeded scaffold. It still: keeps mortality OFF;
treats the grain node as a disclosed configured commons (the sweep proves promotion needs a real
flow window, and produced bread is grain-bounded — recurring production, not a mint or a seed); keeps
the direct-use SALT anchor + thresholds configured. Specialized chain production still waits on money
(`run_role_choice` unchanged — the household/cultivation path is the *unspecialized* pre-money
production Menger describes). It does NOT add the positive check (mortality-on is the next milestone).
If SALT monetizes on cultivated supply, the open colony bootstraps money from genuine pre-money
production-for-barter — the capstone of the supply question; a clean failure localizes the gate
(e.g. cultivation flow insufficient, or cross-tick latency too slow for the strong-bar window).

## 8. Pipeline

rb-lite `codex,claude` (S21f.0→.2) → independent verification (workspace + all 19 goldens
byte-identical + the new suite + a live run) → Codex review-of-results → merge + report/memory + pin.
