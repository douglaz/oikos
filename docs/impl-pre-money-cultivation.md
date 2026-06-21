# Implementation Spec: pre-money own-use cultivation — intensification under pressure (S15)

> The Austrian/Boserupian heart of the arc. S14 gave the colony a real **forage carrying
> capacity** (population plateaus where the land-capped forage commons can't feed more). S15
> is the **escape valve**: when foraging can't feed a colonist, it **cultivates bread by its
> own labor** (gather grain → produce bread → eat at home) — tapping the **more abundant grain
> node** via a **more roundabout, more laborious** process than foraging. So the colony
> **intensifies under population pressure** and its carrying capacity **rises** above the
> forage-only plateau (Boserup). Cultivation is *chosen only under scarcity* because it costs
> more labor than foraging — at low population, extensive foraging suffices and nobody
> cultivates. NO money (S16), NO mortality (later); the bread is produced for **own use**, not
> yet traded.

## What S14 + the research established

- **Forage is land-capped; the grain node is abundant.** The forage commons feeds a
  population bounded by its regen (~2/tick → plateau ~51 at S14's setting); the existing grain
  `ResourceNode` is far larger (`stock 8000, regen 64`, `settlement.rs:2399-2414`) but
  unharvested on the S14 path (gatherers stripped). Cultivation taps grain via a 2-step
  (gather → produce) process — more labor per unit than foraging's 1 step — so it relieves
  hunger the depleted forage commons can't, and raises the plateau toward the grain flow.
- **Recipe execution is money-free; only adoption is gated.** `execute_direct_recipe_for_agent`
  runs with `None` money (`econ/src/sim.rs:702-718`): debit input, hold tool, consume labor,
  credit output. `run_role_choice` gates only market-producer *adoption* on money
  (`settlement.rs:7071`). So an own-use cultivation phase calls the executor directly for a
  hungry non-adopted colonist — exactly the S12 own-labor pattern.
- **No 1-stage grain→bread exists.** Only Mill (grain→flour, needs mill) + Bake (flour→bread,
  needs oven) (`content.rs:131-150`); `GatherFood` shows a no-input/no-tool recipe shape
  (`econ/src/project.rs:122`). A new **no-tool `Cultivate`** recipe (grain→bread, low yield)
  avoids the 2-tool bootstrap and *signals* that own-use cultivation is more roundabout than
  the efficient specialized chain.
- **The cultivators are the spatial hungry lineages (S13).** Forage eligibility now admits
  spatial lineage Consumer/Unassigned (`settlement.rs:6859-6886`); the same set cultivates,
  gated by a **second tier** on the S12 forage hysteresis: *still hungry after foraging* →
  gather grain → cultivate. Per-colonist hunger tracked via the consume readback
  (`update_needs_and_remove_dead`, `:5857`).
- **Conservation already models produce.** `produced` + `consumed_as_input` are in the
  identity (`settlement.rs:3376-3394`); grain `consumed_as_input` → bread `produced` is a
  conserved transformation; grain node regen is the source. The gated-config pattern
  (`forage_commons: Option<…>`, `:833`) extends to an `own_use_cultivation` flag.

## Purpose & the honest bar

On a gated path composing S14 (forage commons + endogenous plateau) + a new
`own_use_cultivation`: when foraging can't feed a colonist (forage commons depleted under
population pressure), it **cultivates bread by own labor** (GoHarvest grain → a no-tool
`Cultivate` recipe → bread eaten at home), booked `produced`/`consumed_as_input` (NOT minted),
at a real labor opportunity cost (its world task that tick is grain-gathering, not foraging or
WOOD). Success = **intensification**: under forage scarcity the colony cultivates and its
population plateau **rises above the forage-only (S14) level**, tracking the grain flow; under
abundant forage **nobody cultivates** (extensive foraging suffices). Honest target: **test
whether** population pressure on the forage commons drives own-labor cultivation that raises
the carrying capacity.

Principled-failure (first-class): if cultivation can't bootstrap (grain unreachable, the
produce step doesn't relieve hunger, or the plateau doesn't rise above forage-only), land
`cultivation_does_not_intensify` with the swept reason — do NOT fake it by minting bread or
seeding food.

NOT money/SALT (S16 — the bread is own-use, not traded), NOT mortality / the positive check
(later), NOT the money-gated specialized mill+oven chain (that stays post-money), NOT a change
to existing emergence. Additive + gated; flag off → S5–S14 + all goldens byte-identical.

## Verified Base Facts (oikos @ `7a1d865`)

1. **The money-free executor** `execute_direct_recipe_for_agent` (`econ/src/sim.rs:702-718`):
   debit input, hold (never consume) tool, consume labor, credit output; no money. Wrapped by
   `execute_direct_recipe_for_agent_checked` (`econ/src/society.rs:5541`). Adoption (not
   execution) is the money gate (`run_role_choice`, `settlement.rs:7071`).
2. **The S12 own-labor phase is the template** (`run_own_labor_subsistence`,
   `settlement.rs:6835-6911`): eligible hungry spatial colonists, the `foraging` flag + hunger
   hysteresis (`h_in`/`h_out`), output booked `produced` (not endowment). S14 made FORAGE a
   real commons via `GoHarvest`; the same haul machinery serves grain.
3. **Recipes** (`content.rs:131-150`): Mill/Bake are 2-stage, tool-required; no 1-stage
   grain→bread. `GatherFood` (`econ/src/project.rs:122`) is the no-tool/no-input shape. A new
   no-tool `Cultivate` (grain→bread, low yield) is the own-use recipe.
4. **Grain node exists, abundant, unharvested on S14** (`settlement.rs:2399-2414`: `stock
   8000, regen 64`; `frontier_forage_capacity` strips gatherers so it's untouched). Cultivators
   GoHarvest it.
5. **Cultivators = spatial hungry lineages** (`settlement.rs:6859-6886`, S13); per-colonist
   hunger from the consume readback (`:5857`); one world task/tick is the opportunity cost
   (`world/src/world.rs:40-51`). Bread is `known.hunger` (the edible staple), so cultivated
   bread relieves hunger directly.
6. **Conservation + gating + plateau metric** as established: `produced`/`consumed_as_input`
   identity (`:3376-3394`); the `forage_commons: Option<…>` gated pattern (`:833`); the
   windowed-plateau + birth-block-reason accessors from S14
   (`forage_carrying_capacity.rs:42-107`, `birth_block_hunger_ceiling`).
7. **The birth-food selector keys to FORAGE today (Codex P1a, load-bearing).** S14's
   `birth_food()` returns `known.subsistence` (= forage) on the forage-commons path
   (`settlement.rs:8302`), and `run_births` blocks if no parent holds the *selected* good
   (`:6342`, counted as `birth_block_endowment`). So cultivation can lower hunger with **bread**
   yet births still **stall on a forage shortage** — the plateau would NOT rise. The
   cultivation path must broaden the child-food rule to **any edible food the parent holds**
   (prefer `known.hunger`/bread, then `known.subsistence`/forage).
8. **Own-use consumption needs an explicit readback seam (Codex P1b, load-bearing).** Hunger
   advances from `society.consumption_log_last_tick()` (`settlement.rs:5857`), NOT from raw stock
   debits — so merely debiting cultivated bread + booking `report.consumed` would *conserve* but
   **not feed** the colonist (hunger wouldn't fall). The fix: a `consume_own_use_stock` seam that
   (a) debits the cultivated bread, (b) **records the consumption in the readback log** so hunger
   advances, and (c) books `report.consumed`. This *also* enforces own-use: the bread is eaten
   before the market (`society.step()` at `:5203`, after own-labor at `:5169`), so it is never
   bartered/sold — "own-use only" is enforced by *consuming it through the readback*, not assumed.
9. **`RecipeId` is a closed enum with canonical tags (Codex P2).** `RecipeId`
   (`econ/src/project.rs:5`), with stable sim tags (`settlement.rs:12360`) +
   `push_recipe_id_bytes` (`:12645`). Adding `Cultivate` means a new enum variant, canonical
   tag + `canonical_bytes_include_*`, and updating exhaustive matches — not a content-local
   change.

## The slices (build in order; each independently testable)

- **S15.1 — the `Cultivate` recipe (enum + tags) + the own-use cultivation phase.** Add
  `RecipeId::Cultivate` (a new enum variant, `econ/src/project.rs:5`) with its canonical sim tag
  + `push_recipe_id_bytes` entry + exhaustive-match updates (Base Fact 9); a gated, no-tool
  grain→bread recipe whose primary knob is **higher labor/time cost** (more roundabout than
  foraging — that is the Austrian point; low yield is a secondary effect, not the lever). Add a
  `run_own_labor_cultivation` phase (template: `run_own_labor_subsistence`) that, for an eligible
  hungry colonist holding grain, calls the money-free executor and books bread `produced` + grain
  `consumed_as_input`. Behind `own_use_cultivation` (default off). **Test:** a hungry colonist
  with grain produces bread by own labor at the labor cost (`produced[bread] > 0`,
  `consumed_as_input[grain] > 0`, labor spent, hunger falls, `endowment[bread] == 0` — not
  minted); the `Cultivate` tag is digested (`canonical_bytes_include_*`); flag off → byte-identical.
- **S15.2 — the scarcity→cultivation decision (explicit, tick-lagged, mutually exclusive) +
  grain + the birth-food broadening.** Add **explicit mutually-exclusive steering** —
  `foraging` XOR `cultivating` per colonist per econ tick (one world task/tick; never both) —
  driven **tick-lagged** off the prior consumption readback (`settlement.rs:5855`): a colonist
  *still hungry after its last consumption* (forage couldn't keep hunger down) is steered to
  **GoHarvest the grain node** then cultivate; otherwise it forages. So cultivating costs the
  forgone forage/WOOD (the opportunity cost), and under abundant forage hunger stays low → nobody
  cultivates. **Birth-food broadening (Base Fact 7):** on the cultivation path, the child-food
  rule accepts **any edible food the parent holds** (prefer `known.hunger`/bread, then
  `known.subsistence`/forage), so cultivated bread can endow births (else the plateau can't rise).
  **Own-use guard via the readback seam (Base Fact 8):** cultivated bread is consumed through a
  `consume_own_use_stock` seam — debit bread → **record in the consumption readback so hunger
  actually falls** → book `report.consumed` — BEFORE the market step, so it feeds the cultivator
  and is never bartered/sold. **Test:** under forage scarcity a hungry spatial lineage member
  gathers grain + cultivates + eats its own bread (**hunger actually falls via the readback**, not
  just a stock debit); `birth_block_endowment` stays low (cultivated bread endows children); under
  abundant forage cultivation count ~0; `foraging`/`cultivating` are never both set the same tick;
  no cultivated-bread trades, no SALT promotion; conserved.
- **S15.3 — the intensification scenario + DoD.** `frontier_cultivation` (compose S14 forage
  capacity + `own_use_cultivation` + a reachable grain node). Register the `cultivation` viewer
  scenario. **Test:** the acceptance suite below.

## Acceptance Tests (the S15.3 DoD) — `sim/tests/pre_money_cultivation.rs`

1. `cultivation_run_is_deterministic` — byte-identical `(seed, config)`.
2. `cultivation_is_produced_not_minted` — cultivated bread is booked `produced` with grain
   `consumed_as_input` (a conserved transformation), `endowment[bread] == 0`; a cultivator's
   hunger falls from eating its own bread.
3. `cultivation_intensifies_the_carrying_capacity` — **the core claim**: with cultivation on,
   the population plateau **rises above the forage-only (S14) plateau** at the same forage flow
   (the colony feeds more by tapping the abundant grain node). And a **grain-regen sweep** shows
   the new plateau **responds to cultivated-grain flow** (higher grain regen → higher plateau,
   monotone) — not a one-off tuning bump. (Compare plateau `own_use_cultivation` on vs off at the
   same forage regen; sweep grain regen.)
4. `cultivation_raises_births_not_just_feeds` — with cultivation on, `birth_block_endowment`
   stays **low** (the broadened child-food rule lets cultivated bread endow children) and births
   resume/rise — proving cultivation actually lifts the plateau, not merely lowers hunger while
   births stall on a forage-endowment shortage (Base Fact 7).
5. `cultivation_is_own_use_not_traded` — on this path there is **no SALT promotion** and **no
   cultivated-bread barter/market trades**; hunger relief comes from the cultivator's **own
   stock** (the own-use guard, Base Fact 8). Money emergence is S16, not here.
6. `no_cultivation_without_scarcity` — under abundant forage (high commons regen) cultivation
   count is ~0: the colony forages (cheaper) and does not pay the cultivation labor cost — the
   escape valve fires only under pressure. `foraging` and `cultivating` are never both set for one
   colonist in one econ tick.
7. `cultivation_conserves` — whole-system conservation every tick (grain node regen the source;
   grain `consumed_as_input` → bread `produced`; no minted food, `endowment[staple] == 0`).
8. `controls_bracket_intensification` — disable cultivation → stuck at the forage-only plateau
   (S14 regime); enable it → higher plateau. With cultivation, the birth-block reasons shift
   (fewer hunger-ceiling stalls — more food is produced locally).
9. `goldens_unchanged` — with `own_use_cultivation` off, S5–S14 scenarios + the six econ +
   g5a/g5b/coemergence + demographic (`lineages`) goldens byte-identical; S5–S14 suites green;
   new state (the `Cultivate` recipe, the `own_use_cultivation` flag, the cultivation steering
   state) in `canonical_bytes` with `canonical_bytes_include_*` regressions; clippy `-D
   warnings`; fmt `--check`.

(Principled-failure path: if cultivation can't bootstrap or doesn't raise the plateau at any
setting, land `cultivation_does_not_intensify` with the swept reason — not a forced pass.)

Manual: `cargo run -p viewer -- run cultivation --ticks 3000`.

## Missing Interactions (the central risks)

- **Cultivation must be MORE LABORIOUS than foraging, or the model is wrong.** Foraging is 1
  world task; cultivation is gather-grain (a world task) + produce-bread (econ labor). The
  opportunity cost must be real (a cultivating colonist forgoes foraging/WOOD that tick), so
  cultivation is chosen *only* when forage can't feed (`no_cultivation_without_scarcity` is the
  guard). If cultivation is costless it would always run and the Boserup story is fake.
- **The grain node must be reachable but not free.** Cultivators GoHarvest grain (a real
  depleting node), so grain is conserved and finite (its own regen caps cultivation's ceiling).
  The plateau should track grain flow; if grain is effectively infinite the "carrying capacity"
  is meaningless — keep grain a real node.
- **No minted food / no money leakage.** Cultivated bread is `produced` from grain, own-use,
  not traded (SALT need not promote — that's S16). Assert `endowment[staple] == 0`; keep the
  money-gated mill+oven chain untouched (it stays post-money).
- **Don't double-feed.** A colonist that forages AND cultivates the same tick would break the
  one-world-task opportunity cost — steer it to one or the other per tick (forage first, fall
  back to cultivation when foraging can't keep hunger down).
- **The intensification metric must be a real comparison** (cultivation on vs off at the same
  forage flow), not an absolute number — the rise above the forage-only plateau is the Boserup
  signal. Use the S14 plateau helper.
- **Determinism.** The `Cultivate` recipe, the `own_use_cultivation` flag, and the cultivation
  steering state enter `canonical_bytes` only on the flag-on path; flag-off byte-identical.

## Handoff Notes

- **Compose, don't rebuild.** `run_own_labor_cultivation` mirrors `run_own_labor_subsistence`;
  the money-free executor + the GoHarvest haul + the grain node + the S14 plateau metric all
  exist. The new work is the `Cultivate` recipe, the cultivation phase, the second-tier
  scarcity steering, and the intensification comparison.
- **Boserup is the through-line:** cultivation taps the abundant grain node via a more
  laborious process, so it raises the carrying capacity *and* is chosen only under pressure.
  Both halves are testable (`cultivation_intensifies…` and `no_cultivation_without_scarcity`).
- **Own-use only** — the bread is eaten, not traded; SALT/money is S16. Don't let cultivation
  monetize here.
- **Gate everything** (`own_use_cultivation` default off, composed on S14's flags) so S5–S14 +
  all goldens stay byte-identical; the `lineages` golden is the tripwire.
- Build S15.1→S15.3 as separate commits with their own tests; `git add` new files.
- **Next:** S16 money from produced bread (the cultivated bread becomes a traded good → SALT
  monetizes against it, retiring the scaffold S12 exposed); then mortality (the positive check).
