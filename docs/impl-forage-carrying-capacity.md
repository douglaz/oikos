# Implementation Spec: forage carrying capacity — the endogenous population plateau (S14)

> The first slice of the scarcity arc, now buildable on S13. S13 made the reproducing
> population **spatial** (it can forage). S14 makes FORAGE a real **capped commons** and lets
> population **grow to press on it**, so the colony's carrying capacity becomes **endogenous**
> (forage-flow-determined) via Malthus's **preventive check** — the existing birth-hunger gate
> stalls births when forage scarcity raises hunger — instead of the artificial
> `max_household_size` knob. NO cultivation, NO money, NO mortality (S15/S16/later). The point
> is to demonstrate a population that **grows and plateaus at a forage-determined level**, the
> pressure the cultivation arc will exploit.

## What S13 + the research established

- **The reproducing population can now forage** (`spatial_households`, S13): lineage members
  have world agents and enter the forage eligibility set (`settlement.rs:6604`). S14 *exercises*
  the spatial-lineage **haul** cycle (GoHarvest→carry→deposit→transfer) that S13 left wired but
  unproven.
- **FORAGE is a marker, not a commons.** The forage node is created `0/0/0`
  (`settlement.rs:4023`), `GoForage` relocates nothing (`world.rs:661`), and
  `run_own_labor_subsistence` credits a **fixed** `forage_yield` (`settlement.rs:6652`) —
  independent of forager count. It must become a real `ResourceNode` (stock/regen/cap) harvested
  by depletion, so per-capita yield falls as population grows (the carrying capacity).
- **The preventive check binds on the staple, which today is bread (Codex P1a).** Both the
  parent-must-endow gate (`settlement.rs:6103-6111`) and the newborn's initial food
  (`:10399`) key on `self.known.hunger` (= **bread** on the frontier). If the colony eats
  **forage** (`known.subsistence`) with the hearth mint off, parents lack bread → births stall
  on a *bread* shortage, not forage scarcity, so the plateau would NOT be forage-determined.
  The child endowment must key on the colony's **actual food** (forage) on this path.
- **Population is flat and the household cap is binding.** Frontier demography
  (`settlement.rs:2340-2346`): `birth_interval 4`, lifespan ∈ {18,24,30,36} ticks, `max_household_size 5`
  — births ≈ deaths, and the size cap binds before any carrying capacity is reached
  (`frontier_probe` raises it to 60 "so change comes from carrying capacity, not the demographic
  ceiling", `:2470-2497`). To grow then plateau, raise the cap so it doesn't bind and tune so
  births outpace old-age deaths during the growth phase.

## Purpose & the honest bar

On a gated path composing S12 own-labor forage (hearth food mint OFF — forage IS the food) +
S13 spatial households (lineages forage) + a **capped forage commons** + a **forage child
endowment** + a **non-binding, growth-capable demography**: demonstrate that the colony's
population **grows while the forage commons can feed it and plateaus when it cannot** — the
plateau set by the forage flow (regen/cap), bounded by the birth-hunger **preventive check**
(births stall when hunger exceeds the ceiling), with deaths **old-age only** (no mortality).
Success = a forage-determined plateau that **tracks the carrying capacity** (lower forage flow →
lower plateau); the *endogenous* part is the population **response** to scarce flow (the cap/regen
are still parameters — say so). Honest target: **test whether** population plateaus this way.

Principled-failure (first-class): if no demography setting both **grows** the colony AND lets the
preventive check **bound** it (e.g. lifespan too short for births to outpace deaths, or the
plateau collapses instead of stabilizing), land `forage_pressure_does_not_plateau` — a documented
diagnostic with the swept reason — not a forced pass. Do NOT rescue it by re-minting food.

NOT cultivation (S15), NOT money/SALT (S16), NOT mortality / the positive check (later), NOT a
change to existing emergence. Additive + gated; flag off → S5–S13 + all goldens byte-identical.

## Verified Base Facts (oikos @ `189e2c4`)

1. **`ResourceNode { stock, regen_per_tick, cap }`** (`world/src/node.rs:18`); `harvest` relocates
   `min(want, stock, carry_room)` and depletes stock (`node.rs:42`); regen is the only ledgered
   source (`world.rs` regen → `report.regenerated`); node state conserved + in `canonical_bytes`.
   Many foragers on one node → per-capita yield falls (first-come from a fixed stock).
2. **Forage today is a fixed credit, not a commons** (`settlement.rs:4023` marker `0/0/0`,
   `world.rs:661` GoForage relocates nothing, `settlement.rs:6652` fixed `forage_yield`). The
   real gatherer haul cycle (GoHarvest deplete → GoDeposit → `transfer_pending_deposits`) already
   exists and is conserved — foragers must use it.
3. **The birth gate is the preventive check** (`run_births`, `settlement.rs:6017`): births require
   every member's hunger ≤ `birth_hunger_ceiling` (`:6091`) AND a parent holding
   `child_food_endowment` of `known.hunger` (`:6103-6111`); newborn seeded with `known.hunger`
   (`:10399`); founders seeded with `known.hunger` too (`build_demography_agent`, `:10359`).
   `known.hunger` is derived per scenario (bread on frontier, `:3837`) and is the staple — it
   threads through consumption/the chain. So the fix is a **birth-food selector** (forage on this
   path) applied at the parent gate, debit, newborn, and founder seeds — NOT a mutation of
   `known.hunger`. `known.subsistence` already reduces hunger (`:5632`) and ranks below bread.
4. **Demography knobs** (`settlement.rs:2340-2346`, `demography.rs:62-102`): frontier
   `ticks_per_year 6`, `old_age_onset_years 3`, `lifespan_span_years 3` → lifespan ∈ {18, 24, 30,
   36} ticks (NOT ~18–21); `birth_interval 4`, `birth_hunger_ceiling 12`, `max_household_size 5`
   (the binding cap; `frontier_probe` shows 60 is safe headroom). All in `canonical_bytes` (gated
   scenario stays byte-identical).
5. **Conservation/gating/determinism** as established; `births_total`/`old_age_deaths_total`,
   `max_living_hunger`, per-colonist `need_of`, `whole_system_total`, the population accessors.

## The slices (build in order; each independently testable)

- **S14.1 — FORAGE as a real capped commons (a DISTINCT gated mode).** Add a **new** gated
  forage-commons mode/config (e.g. `forage_commons { stock, regen, cap }`, default off) — do NOT
  repurpose S12's fixed-credit path (Codex P1): with the commons flag off, S12's `0/0/0` marker +
  fixed `forage_yield` credit (`settlement.rs:4006`, `:6651`) stays **byte-identical**. When on:
  create a real `ResourceNode` (stock/regen/cap) for FORAGE **outside `config.nodes`** (so ordinary
  gatherer round-robin doesn't target it — only the forage path does, like the S12 marker today),
  and route foragers through the **depleting** harvest cycle (GoHarvest on the forage node → carry
  → deposit → transfer) instead of the fixed credit; per-capita yield falls with forager count.
  **Deposit-attribution fix (Codex P1, load-bearing):** `run_fast_loop` records carry/deposit
  deltas only for `Vocation::Gatherer` (`settlement.rs:5398`, `:5444`), but forage eligibility
  includes `Consumer`/`Unassigned` via the `foraging` flag (`:6625`) — so a foraging Consumer's
  harvested FORAGE would carry/deposit but never transfer to econ. Extend attribution to
  `colonist.foraging || vocation == Gatherer` (the chosen fix) so foraged FORAGE is transferred.
  This exercises the spatial-lineage haul cycle. **Test:** N foragers on a capped node draw ≈
  regen total (not N×yield); per-capita falls as N rises; a spatial lineage **Consumer**'s foraged
  FORAGE is attributed + transferred to its econ stock and relieves hunger (the attribution fix);
  conserved; commons flag off → byte-identical (S12 fixed-credit intact).
- **S14.2 — forage child endowment (a SELECTOR) + a growth-capable demography + birth
  diagnostics.** Add a **birth-food selector** (e.g. `birth_food()` = `known.subsistence` when the
  forage-commons path is active, else `known.hunger`) and route the parent-endow gate
  (`settlement.rs:6103-6111`), the debit, and the newborn's initial stock (`:10399`) through it —
  do NOT mutate `known.hunger` globally (Codex P2): it is the staple and threads through
  consumption/the chain/sales. Founders' starting food (`build_demography_agent`, `:10359`) must
  use the **same selector** on this path (else founders start with a bread buffer — either route it
  through the selector or disclose the buffer as a bootstrap artifact; prefer the selector). So
  births stall on **forage** scarcity, not a bread shortage. Raise `max_household_size` so it
  doesn't bind and tune the demography (birth_interval / lifespan) so population **grows** while
  fed. **Birth-block diagnostics (Codex P2):** add counters/accessors for *why* a birth was
  skipped (size cap / hunger-ceiling / parent-endowment / interval) so the principled-failure is
  interpretable. **Test:** with abundant forage, population grows past the old `max_household_size
  5` toward the raised cap; the child endowment + founder food are satisfiable from forage (births
  don't stall on bread; `known.hunger` unchanged so consumption/chain are intact); the birth-block
  counters attribute stalls correctly; flag off → byte-identical.
- **S14.3 — the endogenous plateau scenario + DoD.** `frontier_forage_capacity` (compose S12
  own-labor [mint off] + S13 spatial + S14.1 commons + S14.2 demography). Register the
  `forage-capacity` viewer scenario. **Test:** the acceptance suite below.

## Acceptance Tests (the S14.3 DoD) — `sim/tests/forage_carrying_capacity.rs`

1. `forage_capacity_run_is_deterministic` — byte-identical `(seed, config)`.
2. `forage_commons_depletes_and_regenerates` — the forage node depletes under harvest and
   refills by regen; total forage drawn per tick is bounded by regen+stock, NOT forager count
   (per-capita yield falls as foragers rise) — a real commons, not a fixed credit.
3. `population_grows_then_plateaus` — with the forage commons feeding it, population **rises**
   past the old size cap and **plateaus**; the plateau is set by the forage flow, with births
   **stalling via the birth-hunger gate** as foragers can't keep hunger below the ceiling (the
   preventive check is the bound); deaths are **old-age only** (no starvation).
4. `plateau_tracks_carrying_capacity` — lower forage regen/cap → lower population plateau; higher
   → higher plateau (monotone). The plateau tracks the **forage flow**, not `max_household_size`.
5. `forage_capacity_conserves` — whole-system conservation every tick (forage node regen the only
   source; harvest/deposit/transfer relocations; no minted food on this path —
   `endowment[staple] == 0`).
6. `controls_bracket_the_plateau` — uncap forage (huge regen) → population grows to the raised
   household cap (forage no longer binds); keep `max_household_size` low → population can't grow to
   pressure (the artificial cap binds, the old regime). These bracket "endogenous vs knob".
7. `births_stall_on_forage_not_bread` — with the forage-commons path on, a fed-by-forage colony
   reproduces (the birth-food selector lets parents endow children from forage; `known.hunger`
   unchanged so consumption/chain intact), and as forage scarcity bites the **birth-block
   diagnostics** attribute the stall to the **hunger-ceiling** gate (not the parent-endowment or
   size-cap gate) — proving the preventive check, not a bread shortage, is the bound.
8. `goldens_unchanged` — with the S14 flags off, S5–S13 scenarios + the six econ +
   g5a/g5b/coemergence + demographic (`lineages`) goldens byte-identical; S5–S13 suites green;
   new state (forage node stock/regen/cap, the forage-commons flag, the birth-food selector,
   demography values, birth-block counters) in `canonical_bytes` with `canonical_bytes_include_*`
   regressions; clippy `-D warnings`; fmt `--check`.

(Principled-failure path: if population can't be made to grow-then-plateau on forage —
collapses, runs away, or stays flat at any setting — land `forage_pressure_does_not_plateau` with
the swept reason, not a forced pass.)

Manual: `cargo run -p viewer -- run forage-capacity --ticks 3000`.

## Missing Interactions (the central risks)

- **Growth is the make-or-break (Base Fact 4).** If births can't outpace old-age deaths during
  the growth phase (lifespan ∈ {18,24,30,36} ticks is short relative to `birth_interval 4`),
  population never grows to press on forage and the plateau is degenerate. May require longer
  lifespan / faster births. The **birth-block diagnostics** (test 7) distinguish "can't grow"
  (interval/endowment-bound) from "bounded by scarcity" (hunger-ceiling): a genuine plateau shows
  hunger-ceiling stalls, a degenerate one shows it never grew. If no setting both grows and bounds
  via the hunger ceiling, that is the finding (`forage_pressure_does_not_plateau`) — do not fake it.
- **The hearth mint must be OFF on this path** (own_labor_subsistence semantics) — else forage
  scarcity never binds (the mint feeds them) and the plateau isn't forage-determined. Assert
  `endowment[staple] == 0` on the S14 path.
- **Child endowment must use the actual food** (S14.2) or births stall on a bread shortage
  (Base Fact 3) — the plateau would be bread-endowment-bound, not forage-bound. This is the P1a
  fix; test it directly.
- **Plateau, not collapse.** The preventive check should *stall* births at the carrying capacity,
  giving a stable plateau — not oscillate or crash. If it collapses (hunger spikes kill via some
  path), that's a finding; mortality is explicitly NOT enabled here.
- **No money/cultivation leakage.** S14 is purely demographic/ecological; SALT need not promote
  and the chain need not run. Keep the forage-commons + demography additive and gated; the
  `frontier_forage_capacity` scenario must not perturb S5–S13.
- **Determinism.** Forage node state, the endowment-good selection, and demography values enter
  `canonical_bytes` only on the flag-on path; flag-off byte-identical.

## Handoff Notes

- **Compose, don't rebuild.** FORAGE → a real `ResourceNode` on the existing GoHarvest haul cycle
  (drop the `0/0/0` marker + fixed credit); the preventive check + demography already exist; S13
  made lineages spatial. The new work is the commons conversion, the forage child endowment, and
  the growth tuning.
- **Endogenous = the population plateau's *response* to scarce forage flow**, not the cap itself
  (a param). State it honestly; the controls (uncap → knob-bound; low-cap → can't grow) prove the
  distinction.
- **Mortality stays out** — the preventive check (fewer births) is the bound here; the positive
  check (deaths) is a later milestone (and would erase the pressure if added now).
- **Gate everything** (compose `spatial_households` + `own_labor_subsistence` + a forage-capacity
  config) so S5–S13 + all goldens stay byte-identical; the `lineages` golden is the tripwire.
- Build S14.1→S14.3 as separate commits with their own tests; `git add` new files.
- **Next (the rest of the arc):** S15 pre-money own-use cultivation (the unfed surplus cultivates
  bread by own labor); S16 money from produced bread; then mortality (the positive check).
