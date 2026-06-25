# impl-31 — S21h: Demand-Side Survival Bridge (does keeping the buyers alive bring money back under mortality?)

Status: DRAFT (pre-Codex-spec-review)
Branch: `feat/demand-side-survival-bridge`
Base: master @ `b7beb39` (S21g landed)

## 0. What this milestone is — the S21g-localized step

S21g found that turning the positive check on over the S21f open-market money colony culls the
**non-cultivating demand side** (the SALT-rich buyers + specialist woodcutters — zero food, don't
cultivate) in a one-off cold-start cull at ~tick 7, *before* the money market forms; the self-feeding
cultivation lineage survives into a quiescent commune, SALT never promotes. The pre-money bootstrap
requires the demand side to survive a long hungry, foodless wait (~40–70 ticks pinned at `need_max`
until SALT promotes, *then* buy); mortality kills that patience.

S21h is the **demand-side survival bridge**: keep the non-lineage market roles alive long enough for
the S21f market to form *under mortality*, and ask whether endogenous money then reappears. Sliced like
the supply arc (S21e seeded → S21f produced):
- **S21h.0** — a bounded diagnostic: a finite **consumed-only** survival cushion for the non-lineage
  roles (eaten, never sold). *If the demand side simply survives, does S21f money come back under
  mortality?*
- **S21h.1** — the authentic mechanism: a produced, low-yield, **near-critical emergency
  self-provisioning** for the non-lineage roles that keeps them alive without satiating them into the
  commune (preserves bread demand), `SelfProduced` (no seed).

## 1. The knife-edge this milestone tests (Codex's predicted dilemma)

The research surfaced the exact tension Codex predicted, and it is the heart of S21h:
- **Too weak a bridge → the S21g cull.** A cushion/fallback that doesn't keep the buyers fed past the
  pre-money wait lets them starve before the market forms.
- **Too strong a bridge → demand dies, money never forms.** The food-want ladder is rebuilt each tick
  from *current hunger* (`life/src/scale.rs:194-259`): a buyer at hunger 0 emits **zero** Now bread
  wants. So a cushion large enough to fully sate the buyers removes their bread demand (and their
  bread reservation), the SALT-rich consumer demand hub disappears, and the market never forms — the
  same no-promotion outcome by the opposite cause.
- **The window in between** (residual hunger > 0 but below the death streak) is where the buyer both
  *survives* and *keeps demanding bread* until the lineage's `SelfProduced` surplus monetizes SALT and
  it can buy. S21h's job is to find whether that window exists — and if it does not, **the knife-edge
  is itself the finding** (the model needs a finer survival/consumption structure before money and
  mortality coexist).

## 2. S21h.0 — the consumed-only cushion (diagnostic)

**Lever (minimal).** Give the non-lineage roles a finite starting bread cushion:
- **Buyers (`Vocation::Consumer`, non-lineage):** raise the already-wired `consumer_staple_buffer`
  (`ChainConfig`, currently 0 in `frontier_household_barter`) — read directly in `build_agent`
  (`settlement.rs:14441`). No new field.
- **Woodcutters (`Vocation::Gatherer`, non-lineage):** they read the shared `bread_buffer` (the `_ =>`
  arm), which is also the lineage's — raising it would re-seed lineage bread (breaking the
  sold-for-SALT provenance). So add a **new gated `gatherer_food_cushion: u32`** (default 0) applied
  only to non-lineage Gatherers.

**Auto consumed-only (no extra wiring).** A cushion the agent's own hunger want reserves is never
offerable: `stock_reserved_for_near_wants_barter` (`econ/src/agent.rs:317`) protects Now/Next
hunger-wanted stock, and `barter_swap_acceptable` (`agent.rs:563`) only posts a give-bread offer that
`preserved_near_allocations_above_target`. So the cushion is eaten, never sold — confirmed by the
existing offerable-surplus accounting (`settlement.rs:11169`).

**Ledger / the relaxed-but-pinned bar.** The cushion sweeps to `SeededMinted`
(`maybe_init_acquisition_ledger`, `settlement.rs:11390`), eaten via `consume()`. So S21h.0 **relaxes**
the S21f/g "`seeded_minted` *consumed* == 0" (the cushion is eaten) but **keeps**
"`seeded_minted` *sold-for-SALT* == 0": non-lineage buyers are not in `seeded_surplus_seller_class`
(`settlement.rs:14404`) so they never post `bread → SALT`; the SOLD-for-SALT probe
(`transfer_as_bought` + the cleared tally, `settlement.rs:11221-11242`) stays at `SeededMinted == 0` —
SALT still promotes only on the lineage's `SelfProduced` bread.

**Scenario** `frontier_demand_cushion` = `frontier_open_colony_mortality` (mortality ON) +
`consumer_staple_buffer = C` + `gatherer_food_cushion = G` (disclosed; swept).

## 3. S21h.1 — produced emergency self-provisioning (authentic)

Replace the seeded cushion with a produced own-labor fallback for the non-lineage roles, triggered
only near starvation, low-yield, self-consumed, `SelfProduced` (no seed), demand-preserving:
- **Eligibility flip** (`run_own_labor_subsistence`, `settlement.rs:9092-9101`): under the buy/sell
  split the non-lineage roles are excluded; add an emergency branch making a non-lineage
  Consumer/Gatherer eligible **only when `hunger >= emergency_hunger_threshold`** (a *new* gated
  threshold, ~10–11, distinct from the lineage `cultivate_hunger_in = 6`; ordered below `need_max`/
  `hunger_critical = 12` and above the lineage trigger; mirror the validator ordering checks at
  `settlement.rs:6471-6510`).
- **Low-yield, self-consumed, demand-preserving:** cap the emergency production so it makes *just
  enough to not die* and **no sellable/satiating surplus** (contrast the lineage's deliberate ~6×
  surplus via `OWN_USE_CULTIVATION_LABOR_BUDGET`, `settlement.rs:128/9256`). Capping production at ≈
  the survival floor (no leftover after eating) preserves bought-bread demand: the role still prefers
  to buy and only self-provisions to avoid death.
- **Input fork (settle in spec-review):** (a) **emergency cultivation** — give the role grain access
  (route to the grain node / GoHarvest), reusing the input-haul machinery; or (b) **no-input
  emergency forage** — a fixed small own-labor credit (the S12 forage pattern, `settlement.rs:9168`),
  simpler for roles with no grain node, but the forage-credit path must also credit
  `FoodChannel::SelfProduced` (it currently books `produced` without the ledger credit — a gap to
  close). Both conserve via `report.produced`; both must stay `SelfProduced`, never `SeededMinted`.
- **Scenario** `frontier_emergency_provision` = `frontier_open_colony_mortality` + the emergency seam
  on (no seeded cushion — `seeded_minted == 0` fully restored).

## 4. Falsifiable bar + controls

Classify each slice (mirror the `open_colony_mortality`/`household_barter` harnesses):
- **Non-vacuity (gate):** the bridge actually keeps the demand side alive — `living_non_lineage > 0`
  through the pre-money window (the S21g cull does NOT happen). Else the bridge is too weak (a
  classified outcome, not a bad probe).
- **Demand survives (the S21h success):** with mortality ON, the non-lineage buyers survive to
  promotion **and still demand bread** (non-zero bread Now-wants / they actually BUY post-promotion);
  `current_money_good() == Some(SALT)`, medium leader, `{bread, WOOD}` breadth; pre-promotion
  bread-sold-for-SALT is `SelfProduced` with `SeededMinted`-sold == 0; after promotion the buyers'
  food is materially `Bought`, not mostly cushion/fallback.
- **S21h.0 ledger:** cushion `SeededMinted` is *consumed* but never *sold*; S21h.1: `seeded_minted ==
  0` entirely.
- **The Malthusian band (if it survives):** starvation + births both bind, no extinction, no drift —
  or a clearly characterized partial band.
- **Conservation every tick.**

Controls (classify, never tune):
- **no bridge** (`frontier_open_colony_mortality`) → the S21g cull (no demand side, no money).
- **mortality off** (`frontier_household_barter`) → the S21f success (the positive control).
- **cushion-size / emergency-yield SWEEP — the knife-edge probe (central):** sweep the bridge
  strength. Expect: too-weak → cull (no money); a middle window → buyers survive + demand persists +
  SALT promotes; too-strong → buyers sated → zero bread demand → no market (money dies by the opposite
  cause). Report all three regimes. **If no middle window promotes, that knife-edge is the finding.**
- **overpowered bridge** → explicitly show demand crowd-out (zero bread wants, no promotion).

Cross-seed robustness on the headline result/finding.

## 5. Slices

- **S21h.0** — `consumer_staple_buffer` cushion for buyers + a gated `gatherer_food_cushion` for
  woodcutters; `frontier_demand_cushion` scenario; the cushion-size sweep + the relaxed/pinned ledger
  bar; classify (window found → success / knife-edge → finding / too-weak → cull).
- **S21h.1** — the gated emergency self-provisioning seam (eligibility flip at
  `emergency_hunger_threshold`, low-yield self-consumed, `SelfProduced`; the input fork settled in
  review); `frontier_emergency_provision` scenario; the emergency-yield sweep; classify.

(If S21h.0 shows no window exists — the knife-edge is real — S21h.1 may be unnecessary or becomes the
test of whether a *produced* near-critical floor can thread it where a seeded cushion can't. Decide
after S21h.0's result.)

## 6. Determinism / golden contract

- All new flags/fields default off/0; new scenarios additive. The `gatherer_food_cushion` /
  `emergency_hunger_threshold` flags canonicalized ON-only with digest regressions; acquisition
  ledger stays runtime-only. **All 21 existing golden suites byte-identical.**
- `cargo fmt --check` + `clippy --workspace --all-targets -- -D warnings` clean; conservation every
  tick; deterministic.

## 7. Honest scope

S21h tests whether a demand-side survival bridge lets endogenous money coexist with the positive
check in the open colony. A SUCCESS = the buyers survive *and* keep demanding *and* SALT promotes —
the open colony finally has money + mortality together. A FINDING = the knife-edge (no bridge
strength both keeps buyers alive and preserves demand) — a deep result that the model needs a finer
survival/consumption structure (e.g. graded satiation, partial-provisioning roles) before the two
coexist. It remains: a configured grain commons + 3-role topology + SALT anchor + the disclosed
bridge lever; mortality ON. Does NOT claim emergent role topology or clearing institution.

## 8. Pipeline

Codex spec-review (settle the input fork + the cushion/emergency sizing approach) → SPEC-READY →
rb-lite `codex,claude` (S21h.0, then S21h.1 if warranted) → independent verification (workspace + all
21 goldens byte-identical + the new suite + a live run) → Codex review-of-results → merge +
report/memory + pin.
