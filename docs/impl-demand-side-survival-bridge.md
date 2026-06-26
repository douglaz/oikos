# impl-31 — S21h: Demand-Side Survival Bridge (does keeping the buyers alive bring money back under mortality?)

Status: IMPLEMENTED — Codex review-of-results PASS-WITH-CAVEATS (framing scoped below;
no P0/P1 result defect). **S21h.0 = the KNIFE-EDGE FINDING**: no seeded-cushion size yields a
clean demand-bridge success (too small → only 4–5 of 18 survive, too thin to monetize; too
large → sated out of the market then the cull lands anyway). On the equal-buffer diagonal SALT
never promotes at all (across sizes and seeds); off the diagonal there is likewise no clean
success — and the cells that *do* promote do so only by selling seeded `SeededMinted` cushion
bread for SALT (the seeded-supply-*disqualified* path, not an authentic demand bridge). The
hard per-cell `SeededMinted`-sold-for-SALT==0 invariant is what classifies those cells as
disqualified rather than as successes. **S21h.1 = the SUCCESS** (the produced near-critical
emergency BREAD floor threads it: 12 of 18 non-lineage roles survive AND still demand AND buy,
SALT promotes on the lineage's `SelfProduced` bread, `seeded_minted == 0` entirely, robust
across seeds {3,7,11,19,23} and the threshold sweep 7–11; durable to 10k ticks). The emergency
floor is a *configured own-labor survival institution* — a recurring no-grain-input subsistence
tier (the engine's established own-labor subsistence path), not ordinary grain→flour→bread
production and not the removal of all survival scaffolding. So the open colony has **money +
mortality together after a one-off cold-start cull** (6 of 18 non-lineage roles still die;
starvation then stops — a partial bridged band, not full demand-side survival nor an ongoing
positive-check band). All 21 existing goldens byte-identical; `fmt`/`clippy --workspace
--all-targets -D warnings` clean; conservation every tick; deterministic. Suite:
`sim/tests/demand_survival_bridge.rs` (19 tests).
Prior: SPEC-READY (Codex round 1 + round 2 NEEDS-REVISION → all addressed: no-input emergency fork settled; the emergency good pinned to the BREAD staple credited SelfProduced, immediately consumed, no offerable remainder [not FORAGE — avoids S12 pollution]; cushion 'consumed-only' overclaim fixed with a hard per-cell seeded-sold==0 invariant; the 5-tuple sweep classifier; demand-preservation test)
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

**Consumed-only ONLY up to the hunger reservation (Codex P1 — do not overstate).** The reservation
rule protects stock needed for the agent's Now/Next *hunger wants*
(`stock_reserved_for_near_wants_barter`, `econ/src/agent.rs:317`; `barter_swap_acceptable`,
`agent.rs:563`) — NOT arbitrary excess. So a cushion *within* the current hunger deficit is never
offerable, but a cushion **larger than the protected hunger allocation** (likely in the overpowered
sweep cells) has offerable excess that the agent *could* sell. That is a trap: a large cushion's
excess sold for SALT would be S21e-style **seeded supply** monetizing the token — a false S21h
"success" by the wrong mechanism.

**Ledger / the relaxed-but-pinned bar (HARD invariant on EVERY sweep cell).** The cushion sweeps to
`SeededMinted` (`maybe_init_acquisition_ledger`, `settlement.rs:11390`), eaten via `consume()`. So
S21h.0 **relaxes** "`seeded_minted` *consumed* == 0" (the cushion is eaten) but **keeps, as a hard
invariant asserted on every sweep cell, "`seeded_minted` *sold-for-SALT* == 0"** — SALT must promote
*only* on the lineage's `SelfProduced` bread, never on cushion bread. The SOLD-for-SALT probe
(`transfer_as_bought` + the cleared `SeededMinted` tally, `settlement.rs:11221-11242`) is the check;
any cell where cushion bread is sold for SALT is **disqualified** (it's seeded supply, not a
demand-survival result), not counted as success. (Non-lineage buyers are not in
`seeded_surplus_seller_class`, `settlement.rs:14404`, so they don't post the lineage `bread → SALT`
sell lane — but the invariant is asserted regardless, since offerable cushion excess could still clear
some other way.)

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
- **No-input emergency self-provisioning — the good is BREAD, not FORAGE (Codex round 2 P1).** Use a
  no-input (no grain), own-labor emergency that produces a **tiny amount of the tracked hunger staple
  (bread) directly**, NOT a FORAGE/subsistence good. Crediting FORAGE would reopen S12's
  `known.subsistence` scale effects *and* wouldn't fit the bread-centric acquisition ledger; crediting
  the staple keeps the demand-side roles non-cultivating *and* lets the existing acquisition proof
  work. Pin: the emergency emits bread to `report.produced` (own-labor source, conserved — not an
  endowment/mint), credits it `FoodChannel::SelfProduced` (the credit the fixed own-labor path at
  `settlement.rs:9113` currently lacks; cultivation has it at `:9277`), routes it through the
  **consumption-readback path so it is immediately self-consumed**, and is **capped so NO
  emergency-produced unit remains offerable after the tick** (no sellable remainder → it can never be
  sold for SALT, so it cannot fake supply). Grain-access is explicitly rejected (it would route the
  buyers/woodcutters to the grain node and collapse the demand side into cultivators, muddying the
  "survives but still buys" claim).
- **Scenario** `frontier_emergency_provision` = `frontier_open_colony_mortality` + the emergency seam
  on (no seeded cushion — `seeded_minted == 0` fully restored).

## 4. Falsifiable bar + controls

Classify each slice (mirror the `open_colony_mortality`/`household_barter` harnesses):
- **Non-vacuity (gate):** the bridge actually keeps the demand side alive — `living_non_lineage > 0`
  through the pre-money window (the S21g cull does NOT happen). Else the bridge is too weak (a
  classified outcome, not a bad probe).
- **Demand survives (the S21h success):** with mortality ON, the non-lineage buyers survive to
  promotion **and still demand bread** — measured directly (non-zero bread `Horizon::Now` wants among
  living non-lineage roles during the pre-money window; add a `bread Now-wants` probe, or a
  conservative proxy if none exists, per Codex P2). Then `current_money_good() == Some(SALT)`, medium
  leader, `{bread, WOOD}` breadth; pre-promotion bread-sold-for-SALT is `SelfProduced` with
  `SeededMinted`-sold == 0; and **after promotion the non-lineage roles' food is materially `Bought`**
  (a real demand side buying on the market), with the cushion/emergency food neither sold nor dominant
  in tail consumption (Codex P2 — the demand-preservation test that rules out a bridge that secretly
  satiates the buyers out of the market).
- **S21h.0 ledger:** cushion `SeededMinted` is *consumed* but never *sold*; S21h.1: `seeded_minted ==
  0` entirely.
- **The Malthusian band (if it survives):** starvation + births both bind, no extinction, no drift —
  or a clearly characterized partial band.
- **Conservation every tick.**

Controls (classify, never tune):
- **no bridge** (`frontier_open_colony_mortality`) → the S21g cull (no demand side, no money).
- **mortality off** (`frontier_household_barter`) → the S21f success (the positive control).
- **cushion-size / emergency-yield SWEEP — the knife-edge probe (central):** sweep the bridge
  strength and **classify every cell** by the full vector (Codex P2): (1) non-lineage *alive* through
  the promotion window? (2) bread Now-demand still present? (3) SALT promoted? (4) food materially
  *Bought* after promotion? (5) `SeededMinted`-sold-for-SALT == 0 (else the cell is **disqualified** as
  seeded supply, not a demand-bridge result). Expect three regimes: too-weak → cull (1=no); a middle
  window → all of 1–5 hold; too-strong → buyers sated → 2=no → no market (money dies by the opposite
  cause). Report all regimes. **If no cell satisfies 1–5 together, that knife-edge is the finding** —
  pre-stated as a first-class outcome: the model needs a finer survival/consumption structure (graded
  satiation, partial-provisioning roles) before money and mortality coexist in this colony.
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
