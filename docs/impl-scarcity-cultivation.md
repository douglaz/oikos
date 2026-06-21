# Implementation Spec: scarcity-driven cultivation — money & agriculture co-emerge under population pressure (S13)

> The owner's reframing of the S12 dead-end. S12 showed you cannot retire the minted
> bread hearth and keep money, because money emerged in a *provisioned* bread economy. The
> contrived fix ("make bread a preferred good") just re-seeds the preference money is
> supposed to explain. The authentic mechanism is **population pressure on a foraging
> commons**: foraging is free and easy at low population, so nobody cultivates (and
> shouldn't); but the forage commons has a **carrying capacity**, and once population
> presses past it the marginal hunger can only be met by the more-roundabout cultivated
> chain. Bread is demanded **because it is the marginal food after foraging is exhausted**
> (Menger's imputation; Boserup's intensification under population pressure), not because
> it is preferred. Cultivation, specialization, capital, and money then all become
> **endogenously motivated by scarcity** — and money emerges from **produced** bread,
> retiring the mint scaffold.

## The synthesis (owner + the engine facts)

- **Endogenous carrying capacity, not a knob.** Today population is bounded by an
  artificial `max_household_size` "blowup bound" (`demography.rs:81`). Replace that bound
  with a *resource-determined* one: raise the artificial cap and let the **forage commons +
  the birth-hunger gate** set the real ceiling.
- **The preventive check already exists.** `run_births` (`settlement.rs:6017`) reproduces a
  household only if **every member's hunger ≤ `birth_hunger_ceiling`** (`:6045`). So when
  forage scarcity raises hunger, **births stall** — Malthus's *preventive* check —
  bounding population at the food-determined carrying capacity **without mortality**.
  Mortality (the *positive* check — deaths) stays a **separate later milestone**;
  enabling it here would let population fall back below capacity and erase the pressure.
- **Recipe execution needs no money — the key unlock.** `execute_direct_recipe_for_agent`
  runs with `None` money context (`econ/src/sim.rs:702-718`): pure stock transformation
  (debit input, hold tool, consume labor, credit output). The money gate
  (`run_role_choice`, `settlement.rs:7071`) blocks only the *market-producer adoption
  appraisal*, not production. So a hungry tool-holder can cultivate for **own use**
  pre-money (the S12 own-labor pattern), and **barter already trades produced bread for
  SALT** (`generate_direct_barter_offers`, `econ/src/society.rs:2028`) — so SALT can
  monetize against produced (not minted) bread.

## Purpose & the honest bar

Make money and the grain→flour→bread division of labor **co-emerge from population
pressure on a foraging commons**: at low population the colony forages and neither
cultivates nor monetizes (correct); as population grows past the forage carrying capacity
the unfed surplus **cultivates bread by its own labor**, barters it for SALT (so **SALT
monetizes against produced bread**), and the chain takes off; population **plateaus at the
food-determined carrying capacity** via the birth-hunger preventive check. No minted food,
no seeded "bread is preferred", no designated money, no mortality. Success = that arc;
failure (also valid) = a documented reason it doesn't materialize (e.g. demography can't
grow to pressure, or cultivation can't bootstrap money from barter).

NOT mortality/starvation (the later positive-check milestone), NOT differentiated food
quality (the contrived fix this replaces), NOT minted food, NOT a change to existing
emergence/market clearing (S5–S12 scenarios + all goldens byte-identical; additive +
gated, default off).

## Verified Base Facts (oikos @ `68ddff7`)

1. **The forage commons primitive exists.** `ResourceNode { stock, regen_per_tick, cap }`
   (`world/src/node.rs:18`); harvest relocates `min(want, stock, carry_room)` and depletes
   stock (`:46`); regen is the only ledgered source (`world/src/world.rs:485`); node state
   is conserved + in `canonical_bytes` (`node.rs:62`, `world.rs:809`). **Many gatherers on
   one node → per-capita yield falls** (first-come from a fixed stock, agent-id order) — a
   natural diminishing-returns carrying capacity. **S12's forage is NOT this**: a 0/0/0
   marker node + a fixed `forage_yield` credit (`settlement.rs:3995`, `:6503`), independent
   of forager count — it must become a real node.
2. **The preventive check is wired.** `run_births` gates on every member's hunger ≤
   `birth_hunger_ceiling` (`settlement.rs:6045`) and a parent able to endow the child's
   staple — births stall when hungry. `max_household_size` is an artificial "blowup bound"
   in `canonical_bytes` (`:12079`); `frontier_probe` already raises it to 60 so "demography
   never binds … change comes from carrying capacity" (`:2468-2472`) — the precedent for
   making the cap non-binding.
3. **Population is FLAT today — the make-or-break risk.** Births ≈ deaths in equilibrium
   (`g4b_demography.rs:234`); birth rate lags old-age death (frontier lifespan 18–36 ticks,
   `birth_interval` 4). So even with the cap raised, population may **not grow** enough to
   press on forage. The milestone likely must tune the demography (faster births / longer
   life) so population *grows toward* the carrying capacity — else there is no pressure.
4. **Recipe execution is money-free (the unlock).** `execute_direct_recipe_for_agent`
   (`econ/src/sim.rs:702-718`) needs only input stock + tool + labor; the money gate
   (`settlement.rs:7071`) blocks adoption *appraisal*, not execution. Pre-money cultivation
   is therefore an *adoption-path* change, not a production-engine change.
5. **Barter already trades produced goods.** `generate_direct_barter_offers`
   (`econ/src/society.rs:2028`) posts BREAD↔SALT when a holder has surplus bread and wants
   SALT; the saleability tracker counts it; SALT promotes from real produced-bread volume.
6. **No colonist produces bread pre-money today.** Pre-promotion bread is the seeded
   `bread_buffer` + the demographic `food_provision` mint (`settlement.rs:2299`, `:5903`);
   producer_subsistence requires adoption (gated). This mint is the S12-identified
   monetization scaffold to retire.
7. Conservation/gating/determinism + accessors as established (`tools_built`,
   `producer_cash`, `whole_system_total`, `births_total`/`old_age_deaths_total`,
   `max_living_hunger`, the emergence probe, `bread_for_salt_volume`).

## The slices (build in order; each independently testable)

- **S13.1 — the forage commons.** Make FORAGE a real `ResourceNode` (stock/regen/cap =
  carrying capacity), harvested by the world path (depletes stock; per-capita yield falls
  with forager count); retire the fixed `forage_yield` credit. Gated. **Test:** with N
  foragers on a capped node, total forage ≈ regen (not N×yield); per-capita falls as N
  rises; conserved (node regen is the source); flag off → byte-identical.
- **S13.2 — population can grow to pressure.** In the gated scenario, raise
  `max_household_size` so it doesn't bind, and tune the demography (birth rate vs lifespan)
  so population **grows** while fed and **plateaus** when forage scarcity stalls births
  (the preventive check). **Test:** a low-forage-capacity colony's population rises then
  plateaus at a forage-determined level (not the artificial cap), with births stalling as
  hunger hits `birth_hunger_ceiling`; no mortality (deaths are old-age only).
- **S13.3 — pre-money own-use cultivation.** Add a gated own-use cultivation adoption path:
  a hungry colonist holding a chain tool (or able to obtain one) cultivates grain→flour→
  bread by its **own labor** (reuse the S12 own-labor pattern — adopt on hunger+tool, skip
  the money appraisal; execution is money-free, Base Fact 4), gathering its own grain
  first. Surplus bread is bartered (Base Fact 5). Retire the demographic food mint on this
  path. **Test:** under forage scarcity, a hungry colonist cultivates bread from own labor
  (booked `produced`, not minted), eats it and/or barters it; with no scarcity, nobody
  cultivates (forage suffices) — conserved, gated, flag off → byte-identical.
- **S13.4 — the co-emergent scenario + DoD.** `frontier_scarcity` (derive from the S11/S12
  base) composing S13.1–S13.3 with the food mint OFF. **Test:** the clean arc below.

## Acceptance Tests (the S13.4 DoD) — `sim/tests/scarcity_cultivation.rs`

1. `scarcity_run_is_deterministic` — byte-identical `(seed, config)`.
2. `low_population_forages_without_cultivating` — a small colony under a generous forage
   commons feeds by forage, does NOT cultivate (no flour/bread produced) and does NOT
   monetize (or only a documented weak outcome) — extensive foraging, correctly.
3. `population_pressure_is_endogenous` — population grows while fed and **plateaus at a
   forage-determined carrying capacity** (lower cap → lower plateau), with births stalling
   via the hunger gate (the preventive check); the plateau tracks the forage commons, not
   `max_household_size`; deaths are old-age only (no starvation).
4. `pressure_drives_produced_cultivation` — once population presses past the forage
   capacity, the unfed surplus **cultivates bread by own labor** (`produced[bread] > 0`
   from cultivation, NOT minted: `endowment[bread] == 0`); cultivation rises with the
   shortfall.
5. `money_emerges_from_produced_bread` — SALT promotes (promoted good IS SALT) and the
   pre-promotion bread-for-SALT volume comes from **produced** bread (the food mint is off);
   post-promotion the chain sustains with real market input trades — money emerges from
   produced, not minted, bread. (THE core claim.)
6. `scarcity_conserves` — whole-system conservation every tick (forage node regen; produced
   bread; no minted food).
7. `controls_bracket_the_mechanism` — uncap forage (huge regen) → nobody cultivates, SALT
   doesn't monetize; keep the artificial `max_household_size` cap low → population can't
   grow to pressure → no cultivation; re-enable the demographic food mint → reproduces the
   old S9 provisioned-money behavior (proving the new path doesn't secretly depend on it).
8. `goldens_unchanged` — with the scarcity flags off, S5–S12 scenarios + the six econ +
   g5a/g5b/coemergence goldens byte-identical; S5–S12 suites green; new state (FORAGE node,
   flags, cultivation path) in `canonical_bytes` with `canonical_bytes_include_*`
   regressions; clippy `-D warnings`; fmt `--check`.

(Principled-failure path: if population can't be made to grow to pressure, or cultivation
can't bootstrap money from barter, land `scarcity_pressure_does_not_materialize` — a
documented diagnostic with the swept reason — not a forced pass.)

Manual: `cargo run -p viewer -- run scarcity --ticks 2000`.

## Missing Interactions (the central risks)

- **Population growth is the make-or-break (Base Fact 3).** If the demography stays flat,
  pressure never builds and the whole arc is inert. The carrying capacity must be set below
  the population the colony *would* reach, AND births must outpace deaths enough to get
  there. If no demography setting both grows the colony and lets the preventive check bound
  it, that is the principled finding.
- **Pre-money cultivation must not perturb the money-gated path.** The own-use cultivation
  adoption is additive behind the flag; the existing market-producer adoption (S5–S11)
  stays money-gated and byte-identical. Verify the own-use path can't fire in the
  designated-money scenarios.
- **The bootstrap ordering.** A hungry colonist must be able to obtain a tool + grain
  before it can cultivate; sequence gather-grain → (hold/obtain tool) → mill → bake so the
  chain is causal pre-money. If tools are seeded-only and a would-be cultivator has none,
  this needs the S7 producible-capital path active (per-agent build) — note the dependency.
- **Don't re-introduce a scaffold.** No minted food, no seeded bread preference, no
  designated money — the colony must forage, then cultivate under pressure, on its own labor.
- **Conservation/digest.** FORAGE-as-node, the raised cap, demography tuning, and the
  cultivation path all → `canonical_bytes` + regressions; the forage node conserves like
  any node.

## Handoff Notes

- **Reuse, don't rebuild.** FORAGE → a real `ResourceNode` harvested by `GoHarvest` (drop
  the 0/0/0 marker + fixed credit); own-use cultivation → the S12 own-labor adoption
  pattern (hunger+tool, no appraisal) + the money-free `execute_direct_recipe_for_agent`;
  bread→SALT barter is already there. The novelty is composition, not new engines.
- **Endogenous carrying capacity = forage commons + the birth-hunger preventive check;
  raise (don't keep) the artificial `max_household_size`.** Mortality (positive check) is
  the *next* milestone — do not enable it here (it would erase the pressure).
- **Money from produced bread is the whole point** — test 5 is the tripwire; if SALT only
  monetizes with the food mint on, the milestone failed and that's the finding.
- **Population-growth risk is real** — prove the colony actually grows to pressure (test 3)
  before claiming the arc; if it can't, land the diagnostic honestly.
- Build S13.1→S13.4 as separate commits with their own tests; `git add` new files.
- **Follow-on:** mortality / the positive check (Malthusian deaths as a selection layer on
  top of the preventive check); per-agent richer cultivation choice; tool-making under
  pressure (S7 capital interacts).
