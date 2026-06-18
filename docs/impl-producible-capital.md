# Implementation Spec: producible capital goods (S7)

> The S6 milestone (`docs/impl-provisioning-at-scale.md`) scaled the *untooled*
> subsistence base and explicitly carved out the *tooled* grain→flour→bread chain:
> mills/ovens are seeded-only, never produced, and `latent` (the producer-eligibility
> gate) is write-once, so the chain's throughput is hard-capped at the fixed seeded
> tool count no matter how big the colony or how strong the bread demand. This
> milestone removes that cap by letting colonists **build** new mills/ovens — the
> Mengerian capital-formation step (a higher-order good produced from labor + a
> lower-order good, valued by imputation from the flour/bread it will help produce).

## Purpose & the honest bar

Make the **tooled chain's capacity respond to demand** instead of being frozen at the
seeded tool count: under sustained unmet bread demand a colonist **appraises that
building a mill/oven will pay** (imputed from the chain's expected proceeds, on its own
endowment), **invests labor + WOOD to build it** (a conserved project), then **adopts
the trade and produces** — so as the colony grows, bread output tracks demand rather
than flat-lining, with **no planner placement of tools or quota of producers**, and
**without runaway over-building** (capital formation stops when demand is met).

NOT firms/wage-labor, NOT banks/fiat/credit (that is the G8 stack / Credit era), NOT a
change to econ market-clearing (the conformance goldens stay byte-identical; every
edit additive and gated behind a default-OFF flag, proven by a disabled-phase
regression), NOT touching the `era` detector (pure instrumentation — gates nothing),
NOT raising `hunger_critical`. Tool **inheritance** on producer death is out of scope
because producers are non-lineage and never die of old age today (Base Fact 6) — the
estate machinery already conserves a held tool to commons/heir, but no living producer
mortality exercises it; redistribution-from-commons is a noted follow-on, not this
milestone.

## Verified Base Facts (oikos @ `cb06d99`)

1. **The labor-lifecycle PROJECT system is the build mechanism, and already produces
   a tool.** `ProjectTemplate { input_goods: Vec<(GoodId,u32)>, required_labor, output_good,
   output_qty, salvage_bps }` (`econ/src/project.rs:65-74`) has no era/tier/`enabled`
   field. Lifecycle: `start_project` removes inputs up front (`project.rs:196-237`),
   `advance_project`/`advance_project_by` add labor (`:239-258`),
   `complete_project_if_ready` sets Complete and `stock.add(output_good, output_qty)`
   (`:260-271`), `abandon_project` salvages `salvage_bps` of inputs (`:273-306`).
   `build_net_template` (`:143-153`) already outputs the **NET tool** (NET is itself a
   `required_tool` for FishWithNet, `:113`) — proof a project can mint a tool. (Set
   aside the unrelated market-capital system in `econ/src/capital.rs` — different
   mechanism.)
2. **Two build drivers exist; S7 needs the PER-AGENT one, not the pooled one.** The
   *individual* driver is the lab `World` path: `start_project` from the agent's own
   stock, `advance_project` with the agent's own labor, `complete_ready_projects`
   crediting the agent's own stock (`econ/src/sim.rs:412-433`, `:527-539`) — one builder,
   its own WOOD, its own labor/opportunity cost. The *pooled* driver is
   `sim/src/region.rs:road_step` (`:840-906`): community labor `Σ living ×
   labor_per_colonist` into a shared route fund — a **public good**, the wrong model for
   private capital formation. S7.2 reuses the project **lifecycle** (start/advance/
   complete/abandon) but the **per-agent** allocation, NOT the road's pooled labor. The
   road is cited only for its conservation booking (materials → `report.consumed_as_input`,
   `:882-889`); note it produces no good (`output_qty=0`), which is why it never needed a
   `produced` booking.
3. **CONSERVATION GOTCHA (the #1 risk).** A good-emitting project creates the good ex
   nihilo via `stock.add` on completion (`project.rs:267`). The conservation identity
   `after == before + regen + endowment + produced − consumed_as_input − consumed`
   lives in the settlement/region report (`sim/src/region.rs:368-373`,
   `EconTickReport::conserves()`). So a built mill MUST be booked into the **`produced`**
   bucket, and the consumed WOOD into **`consumed_as_input`**, or conservation breaks.
   The road sidesteps this only by producing nothing.
4. **Eligibility is `latent`-only and write-once — the immutable *eligibility* blocker**
   (phase order, the tool-anchor, and realized prices also gate a freshly-tooled colonist
   — Base Fact 5 + the phase-order trap in Missing Interactions).
   `run_role_choice` re-appraises only latent colonists: `let Some(latent) =
   colonist.latent else { continue }` (`sim/src/settlement.rs:5996-6000`); it never
   inspects stock/tools. `Colonist.latent: Option<RecipeId>` is set only in
   generation struct literals (`:3456`, recipe at `:3412/:3419`) and newborns are
   hard-coded `latent: None` (`:5238`); there is **no `.latent =` assignment anywhere**
   post-generation. A non-latent colonist holding a mill can never become a Miller today.
5. **Everything except eligibility is already mutable and tool-correct mid-run.** The
   tool is plain `Agent.stock` (mutable). The adoption appraisal
   `recipe_adoption_pays_for_money` (`:8169-8248`) + the S6 `recurring_motive`
   `recipe_is_profitable` (`:8065-8080`) read only prices + the agent's own
   endowment, never `latent` — so they would correctly decide for a freshly-tooled
   colonist *if it reached the appraisal*. Production (`run_production` `:5328`) and the
   input-bid + tool-anchor scale extension (`producer_scale_extension` `:8359-8389`)
   key off the adopted vocation and the held tool, already mutable. The tool-anchor is
   a top-ranked `Next` want for the tool good (`:8372-8378`) so a producer never sells
   its capital.
6. **In the S5/S6 configs producers never die; the cap is purely the tool count.** Only
   lineage members have a lifespan (`age_and_remove_elderly` skips `lifespan: None`,
   `:5028`), so non-lineage chain producers are not culled by old age; and frontier's
   hunger-resilient dynamics (`hunger_critical = need_max+1`) disable starvation death.
   (The engine *can* remove any colonist — `update_needs_and_remove_dead` `:4695` — if a
   config makes critical hunger reachable; "never die" is scoped to these scenarios, not a
   universal engine claim.) Tools are durable
   (checked via `can_remove`, never consumed; `econ/src/sim.rs:791-794`) and the
   whole-system tool count is constant — the golden `sim/tests/g3a_production.rs:286-291`
   asserts `produced_of(mill)==0` and `consumed_as_input_of(mill)==0`. So exactly N
   millers + M bakers can ever exist; nothing grows the chain.
7. **`era` is derived instrumentation (gates nothing)** (`sim/src/era.rs:1-17`); the
   only runtime recipe-enable gate is the Knowledge-threshold `maybe_unlock_tier_two`
   → `set_recipe_enabled` (`sim/src/settlement.rs:5409-5432`) — a reusable pattern if
   tool-building should be tier/knowledge-gated. Conservation/determinism/goldens as
   before; tool *holdings* are already in `canonical_bytes` via econ stock, but any new
   eligibility state or build counter must be added to the digest with a regression
   test (mirror `recurring_motive` at `:7307`). No `latent`/eligibility accessor exists
   — add one for tests.

## The slices (build in order; each independently testable)

- **S7.1 — tool-acquisition eligibility (the keystone, smallest).** Behind a default-OFF
  flag, relax the `run_role_choice` entry gate so a colonist that **holds the required
  tool** is admitted to the adoption appraisal even when `latent` is `None`. (Prefer
  gating on "holds the tool" over making `latent` mutable — tool stock is already
  canonical state, Base Fact 7.) The existing appraisal (Base Fact 5) then decides.
  **Two integration musts:** (i) **phase order / anchor** — a freshly-eligible
  tool-holder must get its producer scale (incl. the tool-anchor) refreshed *before* the
  market step of the same tick, or it may post the just-acquired tool as surplus and sell
  it (the tool-anchor only protects *adopted* producers; see Missing Interactions);
  (ii) **digest** — because role-choice would now act without a latent pool, the
  `has_latent_pool` gate on the role-choice digest block (`canonical_bytes`
  `settlement.rs:7276-7279`) must widen to "latent pool OR S7 eligibility on", and the S7
  flag + any new state get a `canonical_bytes_include_*` regression. Add a read-only
  eligibility accessor (none exists). **Unit test:** a non-latent colonist handed a mill
  mid-run gets its scale refreshed, does NOT sell the mill, adopts Miller, and actually
  produces flour; with the flag OFF it stays a non-producer (byte-identical).
- **S7.2 — the BuildMill / BuildOven project (capital formation).** Add `ProjectTemplate`s
  that output a mill / oven from `input_goods=[(WOOD,n)]` + `required_labor`,
  `salvage_bps: 0` (no partial-build salvage, so conservation needs no WIP source —
  Missing Interactions). Drive them **per-agent** (Base Fact 2): one builder commits its
  own WOOD via `start_project`, advances with its own labor over several ticks, and the
  completed tool credits its own stock — NOT the road's pooled labor. Gated default-OFF.
  **Book conservation (Base Fact 3): WOOD → `consumed_as_input` at project START
  (`start_project` removes it up front); the built tool → `produced` at completion.** The
  **build decision is an individual entrepreneurial appraisal on the builder's own value
  scale**, not a planner quota: a colonist starts a build only when the tool's expected
  multi-period proceeds repay its build cost. Define the amortization concretely — a
  durable tool is multi-period capital, so the one-cycle `recipe_adoption_pays_for_money`
  is insufficient: introduce `capital_payback_cycles: N` and require
  `expected_margin_per_run × N > WOOD_build_cost + labor_opportunity_cost (+ first input)`,
  where `expected_margin_per_run` reuses the bundle appraisal's per-cycle spread. (Infinite
  tool life must NOT imply near-zero cost — the WOOD + the waiting/labor is the real cost
  imputed against the discounted flour/bread stream.) **Unit test:** under sustained unmet
  bread demand a builder completes a BuildMill from its own WOOD, whole-system mill count
  rises by 1, `produced_of(mill)>0`, WOOD booked to `consumed_as_input`, the tick
  conserves, and the builder then adopts + produces. **Guard overinvestment:** with demand
  already met (margin below the payback bar), no tool is built (the appraisal declines).
- **S7.3 — the scaling scenario + DoD.** Compose S7.1/S7.2 onto the S6
  `frontier_endogenous_scaling` base as `frontier_capital` / a `capital` scenario (larger
  / growing colony). **Test:** the clean metric below.

## Acceptance Tests (the S7.3 DoD)

`sim/tests/producible_capital.rs`:

1. `capital_run_is_deterministic` — same `(seed, config)` → byte-identical
   (`canonical_bytes` + `digest`).
2. `tool_acquisition_makes_a_colonist_eligible` — THE keystone: a colonist that comes to
   hold a mill (handed one, or buys one) adopts Miller and **produces flour** within N
   ticks; with S7.1 OFF it never does. (Proves the gate relaxation, not a relabel.)
3. `acquired_tool_is_not_sold_before_adoption` — the phase-order/anchor guard: a colonist
   that gains a tool does NOT sell it on the next market step before adopting (whole-system
   tool count does not drop; the would-be producer still holds it and adopts). Falsifies
   the phase-order trap in Missing Interactions.
4. `capital_is_built_under_demand_and_conserves` — with sustained unmet bread demand a
   per-agent BuildMill/BuildOven completes: whole-system tool count rises, `produced_of(tool)>0`,
   WOOD booked to `consumed_as_input`, and **conservation holds every tick** across the
   build.
5. `building_is_individual_not_planned` — the build is a per-colonist appraisal: no global
   quota, no tool placement/transfer, the builder pays the WOOD + labor from its **own**
   endowment, mints nothing it didn't build. (Assert the decision path is per-agent, not a
   planner phase handing tools out, and not the road's pooled labor.)
6. `bread_scales_with_capital` — THE clean metric, with a hard "new capital entered the
   chain" assertion: vs a no-build control on the same growing colony, the build-enabled
   run ends with **more tools, more active producers, and higher (and non-declining
   per-capita) bread output** — AND a formerly non-latent colonist must have
   built/acquired a **produced** tool (not seeded), adopted, **bought its input by a real
   `Society::trade`**, and transformed it (so "more tools + more bread" cannot be a seeded
   or placement artifact).
7. `no_overinvestment_in_capital` — real-resource investment responds to demand and
   **stops**: tools are not built without bound; once bread demand is met (per-run margin
   below the payback bar) the tool count and producer count stabilize (no idle-mill
   overbuild) and whole-system WOOD is not drained by speculative building. (Entrepreneurial
   over-investment, self-correcting because it is real saved WOOD/labor at stake — not a
   credit-fueled ABCT cluster.)
8. `s5_s6_unchanged` — with S7 OFF, the `endogenous` and `scaling` scenarios + their
   suites are byte-identical; the six econ conformance goldens are byte-identical
   (incl. `produced_of(mill)==0` in the golden scenarios — S7 gated off there); the new
   digest knobs have `canonical_bytes_include_*` regressions; clippy `-D warnings`; fmt
   `--check`.

Manual: `cargo run -p viewer -- run capital --ticks 1600` (and compare bread output +
tool count vs `scaling`).

## Missing Interactions (track explicitly)

- **Conservation of the built tool (Base Fact 3) is the highest risk** — get the
  `produced`/`consumed_as_input` booking right or `conserves()` fails. The S7 scenario
  has `produced_of(mill)>0`; the *golden* scenarios must stay at 0 via gating.
- **The golden `produced_of(mill)==0`** (`g3a_production.rs:286-291`) must remain true in
  the conformance scenarios — only the gated S7 scenario produces tools.
- **WOOD is a closed battery drained by warmth (S6 finding); building tools also spends
  WOOD.** Ensure tool-building does not starve warmth provisioning — track whole-system
  WOOD across the run (it must stay bounded; the build appraisal competes with warmth on
  the builder's scale, which is the faithful brake).
- **Phase-order / tool-anchor trap (Codex P1).** The tool-anchor scale want protects only
  *adopted* producers; a non-latent colonist that newly holds a tool has no anchor until
  role-choice adopts it and scales regenerate. If the tool appears after scale/role-choice
  but before the market step, it can be sold as surplus. S7.1 must complete build /
  eligibility BEFORE scale + role-choice in the tick (or refresh scale immediately on
  acquisition). Test 3 guards it.
- **Over-investment (entrepreneurial error, NOT ABCT).** Without a demand-anchored
  appraisal, colonists could build idle mills (wasted real WOOD + labor). This is ordinary
  entrepreneurial over-investment and self-corrects (real saved resources at stake); it is
  *not* an Austrian credit cycle, which requires fiduciary credit distorting the appraisal
  (out of scope, G8 stack). Test 7 guards it; the appraisal must be demand/price-driven.
- **S6 re-entry vs. building (labor allocation).** S6 sends hungry colonists to gather
  food; S7 building also wants labor. A hungry colonist should feed itself first (gather)
  and only a fed colonist with surplus should invest in capital — the builder's own value
  scale (hunger above building) is the faithful arbiter. Watch that enabling both does not
  starve the tail or stall all building.
- **Eligibility digest.** The "holds-tool eligibility" gate (or a mutable `latent`)
  changes `has_latent_pool` / future behaviour — widen that gate and add the S7 state to
  `canonical_bytes` with a regression (Base Fact 7).

## Handoff Notes

- **Highest risk: conservation of a produced tool.** Mirror the road's
  `consumed_as_input` booking for the WOOD input, and ADD a `produced` booking for the
  completed tool (the road never needed this). Assert `conserves()` every tick in the
  build test.
- **Per-agent, not pooled:** use the project lifecycle (`start_project`/`advance_project`/
  `complete_project_if_ready`) with the **single-builder** allocation (own WOOD, own
  labor) — the lab `World` BuildNet path (`econ/src/sim.rs:412-433`), NOT the road's
  pooled community labor (`region.rs:road_step`).
- **Amortize the tool cost:** a durable mill is multi-period capital; the build appraisal
  must compare `expected_margin_per_run × capital_payback_cycles` against `WOOD cost +
  labor opportunity cost (+ first input)`. Do not let infinite tool life imply ~zero cost.
  Reuse the bundle appraisal for the per-run margin; document `capital_payback_cycles`.
- **Keep the build praxeological:** the entrepreneurial decision is an individual
  colonist appraising expected proceeds vs build cost on its own value scale — not a
  planner quota or scenario-scripted placement.
- **Eligibility before capital, and before the market in-tick:** S7.1 is the prerequisite —
  a built tool is useless if holding it doesn't make the builder eligible, and the
  acquisition + scale refresh must precede the market step so the tool isn't sold (the
  phase-order trap). Build S7.1 first and test it alone (incl. the not-sold test).
- **Digest:** widen the `has_latent_pool` gate (`settlement.rs:7276-7279`) to also fire
  when S7 eligibility is on, so the role-choice digest block + S7 knobs are serialized
  even with no seeded latent pool.
- **Gate everything** behind a default-OFF flag so conformance/lab scenarios are inert
  and the goldens (incl. `produced_of(mill)==0`) stay byte-identical (the tripwire).
- **Digest discipline:** any new steering knob / eligibility state → `canonical_bytes` +
  a `canonical_bytes_include_*` test (cf. the S5/S6 digest fixes).
- **Observability for tests 5/6:** tying a *produced* tool to a *formerly non-latent*
  adopter may need a small read-only accessor/counter/event (e.g. a per-colonist
  "built/acquired-tool" marker or a built-tools counter) if existing state makes it
  awkward — add one (read-only, digest-covered if it steers behaviour).
- Build S7.1→S7.3 as separate commits with their own tests; `git add` new files.
- **Follow-on (separate, later):** tool inheritance/redistribution from commons to a
  living would-be producer (needs producer mortality to matter) and a richer
  time-preference / roundaboutness appraisal for the build decision.
