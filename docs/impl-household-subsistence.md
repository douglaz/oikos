# Implementation Spec: household subsistence at scale (S12)

> S8/S9 made money emerge, but the co-emergent colony is **semi-hungry** (mean ~7.6) and
> kept alive by **exogenous hearths that MINT food/WOOD with no labor, time, or
> opportunity cost** (`producer_subsistence`, the demographic `food_provision`). Codex's
> direction: that minting is the artifact to retire — not the *existence* of household
> provision (primitive households do produce for own use alongside the market). Replace
> the minted food hearth with **own-labor subsistence PRODUCED by real labor**: a
> low-grade survival **floor**, while **bread stays the superior market good** that clears
> enough exchange to monetize SALT. This is the prerequisite for S13 (mortality
> selection) — you cannot honestly enable starvation on an artificially-hungry colony.

## The faithful framing (Codex) and the central risk

- **Floor, not substitute.** The subsistence good is **low-grade**: produced from the
  colonist's own labor/time (opportunity cost vs gathering WOOD-to-sell or leisure),
  consumed by its producer first, **ranked below bread**, low yield, possibly perishable. A
  colonist *forages to survive* but *prefers bread* when it can afford it — so bread
  demand (the bread-for-SALT trade that monetizes SALT) persists.
- **Retire the MINTING of hunger goods**, not provision per se: turn off
  `producer_subsistence`'s staple mint and the demographic `food_provision` mint;
  own-labor subsistence now comes from **`produced`** (labor), not **`endowment`**.
- **The central risk (Codex, HIGH).** With a single hunger scalar, forage and bread
  reduce the *same* hunger — near-perfect substitutes at the margin. The passing band is
  narrow: forage productive enough that nobody starves, low enough that bread is still
  bought. **If a parameter sweep shows "low garden → starvation, high garden → no money,
  no middle band," that is NOT an implementation failure** — it means the one-scalar food
  model is too coarse (needs differentiated food quality/services, e.g. bread satisfying a
  preference forage can't). That is a **first-class landable finding**, not a forced pass.

## Purpose & the honest bar

On a new gated path: replace the minted food hearths with a **produced own-labor
subsistence floor** (real labor, booked `produced`, own-consumption-first, sub-bread rank), so
the co-emergent colony stays **bounded-hunger and well-enough-fed at scale** WITHOUT
killing money emergence — bread still monetizes SALT and the chain still sustains.
Success = a config in the middle band; failure = a documented "no middle band → food
model too coarse" finding.

NOT mortality/starvation (that's S13, which this unblocks), NOT a monetization-anchor
redesign (C — wrong move now; S9 proved bread monetizes SALT), NOT minting-with-labels
(B — a "production receipt" with no labor given up is still a scaffold), NOT a change to
emergence/market clearing (S5–S11 scenarios + all goldens byte-identical; additive +
gated, default off).

## Verified Base Facts (oikos @ `8f8f1b6`)

1. **The hearths MINT, they don't produce.** `run_producer_subsistence`
   (`sim/src/settlement.rs:6244-6280`) mints staple(bread)+WOOD up to `producer_subsistence`
   per producer, booked to `report.endowment` (`:5787`) — no labor. `deliver_demography_provisions`
   (`:5745-5765`) mints `food_provision`+`wood_provision` to `household.is_some()` members,
   also `endowment`. In `frontier_coemergent` these are lean (`producer_subsistence=2` `:2828`;
   `food/wood_provision=1` `:2853-2854`) — the scaffold to retire (for the hunger good).
2. **The subsistence↔monetization tension is real, and S6 re-entry is coupled to it.**
   `subsistence_on_grain` makes raw grain directly edible (`known.subsistence`,
   `:3698/3704`), which crowds out bread demand; `run_productive_reentry` is inert unless
   `known.subsistence == Some(grain)` (`:6931-6932`). So the existing S6 gather-and-eat
   path can't be reused under emergence without killing money — S12 needs a **distinct,
   sub-bread, labor-produced** subsistence good, not raw-grain edibility.
3. **No household-production path exists.** Hearths are pure endowment mints; gathering
   (`assign_idle_gatherer_tasks`/`Task::GoHarvest`, `:5343-5361`) and the miller/baker
   chain both route through the **market**. A `GatherFood` recipe exists in the lab
   (`econ/src/project.rs:122-130`, output FOOD, labor 1) but is unused in frontier configs.
   The produced-from-labor, produced-from-labor, own-consumption-first path is new.
4. **The hungry tail is the non-lineage poor.** Lineage members are hearth-fed; producers
   are hearth-fed; the **consumers + WOOD-node gatherers** have no endogenous food and must
   monetize WOOD→SALT→BREAD, so they sit hungry while the bread market cold-starts
   (`:5745-5765` lineage, `:6244-6280` producers, market-only for the rest). `hunger_critical
   = need_max+1` keeps them alive-but-pinned (`:2364`).
5. **The consume + ranking machinery supports a sub-bread floor.** `push_present_ladder` /
   `KnownGoods` rank goods on the hunger ladder; the staple (bread) outranks the
   `subsistence` fallback (`life/src/scale.rs`), and `Horizon::Now` goods are consumed
   (`econ/src/agent.rs:720-743`). A new subsistence good ranked below bread reuses this —
   bread outranks forage at each marginal hunger unit (the subsistence offset interleaves; colonists are not forced to exhaust bread first).
6. **Conservation + gating + accessors as established.** A produced good is booked to
   `report.produced` (conservation identity already covers it); default-off `ChainConfig`
   flag pattern; spoilage (`perishable_decay_bps`/`run_spoilage`) if forage is perishable;
   the S8.0 emergence probe (`promoted_at_tick`, bread-for-SALT volume), hunger accessors
   (`max_living_hunger`, per-colonist `need_of`), `whole_system_total`, `producer_cash`.

## The mechanism (concrete) — OWN-LABOR subsistence, not "household" (Codex P1)

The hungry tail is the **non-lineage spatial poor** (`household: None` — consumers,
mis-allocated WOOD gatherers, idle/latent producers), NOT G4b households. So key the
production to **any hungry unprovisioned colonist with spare labor**, and call it
**own-labor subsistence** (not household production). After the staple mint is retired,
this set explicitly includes producers/latent producers (an actively-baking colonist eats
its own bread; an idle/hungry producer or miller forages).

- **A new low-grade subsistence good `FORAGE`, wired as `known.subsistence` (Codex P1).**
  It must `known.subsistence = Some(FORAGE)` in the S12 path so it is **read back as hunger
  relief** (the consume readback only counts a fallback when `Some(good) == known.subsistence`,
  `settlement.rs:5381`) and ranked **below bread at each marginal hunger unit** via the
  existing subsistence offset (`scale.rs:201,241`). FORAGE must be in `content` / tracked
  goods / `canonical_bytes` (else it is eaten in econ but reduces no hunger and evades
  conservation coverage). Reusing raw grain instead would repeat the S4/S6 crowd-out trap —
  a distinct good is required.
- **The production seam — a WORLD-TASK forage with real opportunity cost (Codex P1, pinned).**
  A hungry eligible colonist is assigned a `Task::GoForage(forage_node, carry_cap)` (a new
  task analogous to `GoHarvest`, `settlement.rs:5343`) — it forages a FORAGE node/plot,
  carries+deposits FORAGE to its **own** stock, eaten at home. The opportunity cost is
  **structural and mutually exclusive**: that tick it forages *instead of* `GoHarvest`-ing
  WOOD (its market income) — one world task per colonist per tick, so foraging costs the
  forgone WOOD-to-sell. Output is booked to **`report.produced`** (made from labor on a
  node, conserved like any gathered good), NOT `endowment`. This is direct production for
  own consumption, not minting-with-labels.
- **Low productivity (the floor knob).** FORAGE `carry_cap`/yield per forage tick is the
  tuning lever: high enough that sustained tail hunger drops to a bounded band, low enough
  that colonists still want bread (the superior good) so the bread-for-SALT trade survives.
- **Retire the HUNGER-good mint only (Codex P3, scoped honestly).** In the S12 path,
  `producer_subsistence`'s **staple** mint and the demographic **food_provision** mint are
  zeroed; food now comes from FORAGE (own labor) + bread (market). The **WOOD/warmth
  provision stays an endowment** for now (retiring it is a noted follow-on) — so this
  retires the food scaffold, not the whole hearth; state that plainly.

## The slices (build in order; each independently testable)

- **S12.1 — own-labor subsistence (retire the food mint).** Add the gated `FORAGE` good
  (wired `known.subsistence = Some(FORAGE)`, in `content`/tracked/`canonical_bytes`,
  sub-bread rank) + the `Task::GoForage` world path (produced from own labor, mutually
  exclusive with WOOD `GoHarvest` that tick, eaten at home) and a default-off
  `own_labor_subsistence` flag; in that path zero the hunger-good mint
  (`producer_subsistence` staple + demographic `food_provision`). Eligible = any hungry
  unprovisioned colonist with spare labor (the `household:None` tail + idle/latent
  producers). **Unit test:** a hungry non-lineage colonist is assigned `GoForage`, deposits
  FORAGE **from its own labor** (booked `produced`, not `endowment`), and its hunger
  **actually falls** (FORAGE is `known.subsistence`, so the readback counts it) — with NO
  food minted (`report.endowment[staple] == 0`); flag off → byte-identical.
- **S12.2 — the balance: fed AND money still emerges (the falsifiable core).** Compose onto
  the co-emergent base. **Tests:** (a) tail hunger bounded (mean / p95 / max / chronic-count
  all below the semi-hungry S9 baseline); (b) SALT still promotes under the S9 indirect
  breadth gate and bread still monetizes it (pre-promotion bread-for-SALT volume material;
  post-promotion bread production + input trades continue in the tail); (c) **two controls
  bracket the band** — a no-own-labor-production control stays hungry; an overpowered-forage
  control crowds out bread and money fails to emerge / chain dies. The passing config sits
  strictly between, for a legible reason.
- **S12.3 — the flagship scenario + DoD.** Add `frontier_coemergent_strong_provisioned`
  (derive from the S11 entrepreneurial base) with `own_labor_subsistence` on, food mint off.
  **Test:** the clean metric below.
- **If no middle band exists** (the principled-failure mode): land a **passing diagnostic**
  `subsistence_and_monetization_have_no_middle_band` — a **pinned parameter sweep** (Codex
  P2): forage-yield grid `{0, 1, 2, 3, 4, 6, 8}` carry/tick × seeds `{1, 7, 0xC0FFEE}` ×
  1600 ticks, recording per cell {tail mean+p95+max hunger, `promoted_at_tick`,
  pre-promotion bread-for-SALT volume, tail bread.made + active-producer input trades}.
  PASS-the-milestone = ≥1 cell with **bounded hunger AND SALT promoted AND tail bread/input
  trades present**. If NO cell satisfies all three (every low cell starves the tail; every
  high cell kills money), land that as the finding (single-hunger-scalar food model too
  coarse → differentiated food quality is the real fix). Do NOT force a pass by re-minting
  food or by raw-grain edibility.

## Acceptance Tests (the S12.3 DoD) — `sim/tests/own_labor_subsistence.rs`

1. `provisioned_run_is_deterministic` — byte-identical `(seed, config)`.
2. `subsistence_is_produced_not_minted` — FORAGE consumed by the tail comes from
   `report.produced` (own labor), and `report.endowment` of the hunger staple is ZERO
   (the mint is retired); conserves every tick.
3. `tail_hunger_is_bounded_at_scale` — vs the S9/S11 semi-hungry baseline, tail hunger mean
   **and** p95 **and** max **and** chronic-hungry count all drop and stay non-drifting; no
   colonist is permanently stranded without a food path.
4. `money_still_emerges_and_bread_monetizes_it` — SALT promotes under the indirect-breadth
   gate (promoted good IS SALT); pre-promotion bread-for-SALT exchange is material; post-
   promotion bread production + active-producer input trades continue in the tail (the chain
   is not crowded out).
5. `no_own_labor_production_control_stays_hungry` — with `own_labor_subsistence` off (and the
   mint off), the tail is hungrier — proving the produced floor is what feeds it.
6. `overpowered_forage_crowds_out_money` — with forage yield cranked high, bread demand
   collapses and SALT does NOT monetize (or the chain dies) — proving the band is real and
   the passing config sits between this and test 5.
7. `provisioning_conserves` — whole-system conservation every tick (FORAGE produced from
   labor; spoilage if perishable booked to `spoiled`).
8. `goldens_unchanged` — with `own_labor_subsistence` off, S5–S11 scenarios + the six econ +
   g5a/g5b/coemergence goldens byte-identical; S5–S11 suites green; new good/flag/state has
   `canonical_bytes_include_*` regressions; clippy `-D warnings`; fmt `--check`.

(If the principled-failure path lands, tests 3/4 are replaced by
`subsistence_and_monetization_have_no_middle_band` per the slice note.)

Manual: `cargo run -p viewer -- run provisioned --ticks 1600`.

## Missing Interactions (track explicitly)

- **The middle band is the whole game (Codex).** Too little forage → tail starves; too much
  → bread demand and money die. The two controls (tests 5/6) bracket it; the passing config
  must sit between for a legible reason. If no band exists across seeds, that is the
  principled finding (food model too coarse) — land it.
- **Opportunity cost must be real.** Forage labor must cost the colonist (forgone
  WOOD-gathering income / leisure, like S6/S11), or it is minting-with-labels (option B).
- **Bread must stay superior.** FORAGE ranked strictly below bread at each marginal hunger
  unit, so bread keeps being demanded for the top of the hunger ladder — this preserves the
  bread-for-SALT trade → SALT monetization.
- **Conservation: don't double-count FORAGE (Codex).** `Task::GoForage` must book FORAGE as
  exactly ONE source line in `report.produced` — do not also let it flow through the normal
  node `regen` bucket, or conservation double-counts.
- **Producer food path (Codex).** Retiring the producer staple mint must leave an
  actively-producing miller a feasible food path (buy bread, or forage when idle/too
  hungry to produce); add an explicit **producer-hunger sanity assertion** so a regression
  here is easy to diagnose (not just inferred from tail input trades).
- **Don't regress S9/S10/S11.** Money emergence (S9), per-agent capital (S10), and forecasts
  (S11) all run in the flagship — verify their tests hold with own-labor subsistence on.
- **Digest.** The FORAGE good, the `own_labor_subsistence` flag, and any new per-colonist
  forage state → `canonical_bytes` + regressions; spoilage already digested.

## Handoff Notes

- **Retire the MINT, add real PRODUCTION.** Zero the hunger-good mint in the S12 path; the
  floor must come from `report.produced` via own labor with opportunity cost — not a
  relabeled endowment (that is option B, rejected).
- **Floor not substitute:** low yield, sub-bread rank, own-consumption-first, possibly perishable;
  let high self-use reservation make sale rare rather than hard-banning it.
- **Keep bread the monetizing good:** the passing band depends on bread staying widely
  bought; tests 4/6 are the guardrails.
- **Honest failure is a deliverable:** if there is no middle band, land the diagnostic +
  finding (single-hunger-scalar too coarse → differentiated food quality is the real fix),
  not a forced pass.
- **Gate everything** (default off) so S5–S11 + goldens stay byte-identical.
- Build S12.1→S12.3 as separate commits with their own tests; `git add` new files.
- **This unblocks S13** (mortality selection): once the tail has a feasible labor-based
  survival path, enabling starvation selects against genuinely bad decisions, not an
  artificial provisioning gap.
- **Follow-ons:** differentiated food quality/services (if the band is too narrow);
  produced WOOD/warmth (retire the WOOD mint too); S13 mortality selection.
