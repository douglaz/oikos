# Implementation Spec: imperfect-double-coincidence money — the 3-good cycle (S19)

> The live frontier — the one structure where a durable token could emerge as money endogenously
> from a PRODUCED economy. S18 failed because its two-good division of labor had a **perfect** double
> coincidence (cultivators want exactly what woodcutters make, and vice versa) → bread↔WOOD clears by
> direct barter → the abundant necessity (WOOD) dominates saleability → SALT never leads. S19 builds
> the canonical Jevons/Menger structure: a **3-good production cycle** with **no pairwise double
> coincidence** — A makes X (needs Z), B makes Y (needs X), C makes Z (needs Y) — so NO pair clears
> directly and a medium becomes *necessary*. Demand is **derived from production** (each needs another's
> output as its input), not invented taste; survival is kept **off-market** so the universal
> hunger/warmth wants don't dominate; and SALT, the neutral token, wins the saleability race because the
> cycle goods are *bad direct media* (broken coincidence suppresses their direct acceptances). The
> decisive question (Codex): can the provisional-leader logic recognize SALT from **sparse** direct-use
> before the cycle stalls? Achievable, but likely another honest finding unless that holds.

## What the research + Codex established

- **The 3-cycle is the canonical construction** (Codex): A: Z→X, B: X→Y, C: Y→Z. Pairwise, no double
  coincidence — A has X wants Z, but C (who has Z) wants Y not X; etc. So each producer's output is
  wanted (as the next role's input) but never by the holder of what it needs → indirect exchange is
  *necessary*. Demand is **derived input demand** (the producer-input want), NOT consumption taste —
  this dodges the want-invention circularity.
- **Goods + recipes are data.** `GoodRegistry::intern` (`registry.rs:51`); a `ContentSet` constructor
  interns goods + builds `Recipe{input_good, required_tool, output_good, …}` (`content.rs:142`); the
  executor is data-driven (no per-variant match, `society.rs:5580`). Adding X/Y/Z + 3 recipes = a new
  `ContentSet` ctor + 3 `RecipeId` variants. New vocations extend additively (Scholar/Confectioner
  prove the surface: `production_specialty` `settlement.rs:13544`, `build_agent` arm `:12376`,
  production-phase arm `:7483`).
- **Derived input demand is the want-invention-free mechanism.** `production_specialty(vocation)` maps
  a role → (tool, input good); `producer_scale_extension` (`settlement.rs:13618`) injects the input as
  a `Horizon::Next` want (reserved for the recipe, never eaten) ranked below survival. **Pre-money this
  Next want IS the cycle demand** — it drives generic barter offers via `near_unsatisfied_goods`.
  `set_project_input_bid_overrides` (`:8261`) posts a real order-book bid only **after a money good
  exists** (`:8240`) → a POST-promotion seam (its hardcoded Miller/Baker arms `:8302` need 3 cycle
  arms for after promotion); do NOT rely on it for pre-money demand.
- **Leadership is a RELATIVE-share race, not absolute volume (the decisive feasibility, Codex):**
  `provisional_leader` (`menger.rs:201`) picks the highest `share_bps = acceptances·10000/total`, gated
  by `indirect_min_acceptance_share_bps` (30% share), `min_acceptor_agents` (3), `min_counterpart_goods`
  (2). So broken coincidence shrinks the cycle goods' acceptances AND the denominator → SALT's sparse
  anchor can be the largest *share*. The binding constraints are the **breadth floors** (≥3 distinct
  SALT-acceptors, ≥2 counterparts, ≥30% share); densifying the anchor (lower `salt_direct_use` period,
  without making it universal) is the lever if it can't clear them.
- **Survival must be fully off-market (the make-or-break, the S18 trap).** Universal hunger/warmth
  present-ladder wants (`scale.rs:194`) regenerate every tick *above* producer-input wants; if any
  colonist must BUY food/WOOD, those re-enter the book and dominate (S18). Off-market via
  `run_producer_subsistence` (hearth-minted food+WOOD, NOT from market, `settlement.rs:7653`) +
  own-use subsistence/cultivation. A satisfied scale want is skipped by `near_unsatisfied_goods`
  (`agent.rs:272`), so off-market-fed survival never becomes a barter offer → accrues no acceptances.
- **SALT is genuinely neutral** (`settlement.rs:1379`): not a node good (not gathered), not a recipe
  output (not produced), not a recipe input (not consumed-as-input). Its only acceptances come from
  the heterogeneous direct-use anchor + indirect re-trade → its saleability share is unpolluted.
- **The metric must be derived.** No direct-acceptance-share-by-good accessor; derive
  `direct = acceptances − indirect_acceptances` per good from `emergence_acceptances()`
  (`settlement.rs:11254`). `saleability_leader()`/`saleability_bps()` + the S18 `indirect_target_goods()`
  accessor + the round-trip ledger exist.

## Purpose & the honest bar

On a fresh gated scenario (`frontier_cycle`): a 3-good production cycle (A: Z→X, B: X→Y, C: Y→Z, no
pairwise double coincidence, demand **derived from the producer-input want**), survival isolated
off-market, SALT the neutral medium + the heterogeneous direct-use anchor, the strong-bar gate
UNCHANGED. Test whether money emerges endogenously from imperfect double coincidence. Success = SALT
**promotes** under the unchanged gate, with: pre-promotion direct barter among X/Y/Z near-zero (broken
coincidence); SALT the provisional leader BEFORE promotion; `indirect_target_goods(SALT) ⊇ {X, Y, Z}`
(it bridges all three cycle inputs); producers accept SALT `IndirectFor{their input}` and **round-trip**
it into that input (the traced ledger); production continues post-promotion; survival trade ≈ 0.

**Honest scope of the construction (Codex P1c/P2a — state plainly, do NOT overclaim):** this is an
**artificial exchange-topology test** — a laboratory cycle isolating the question "does a neutral token
emerge as the medium under imperfect double coincidence?" It is NOT "the first scaffold-free produced
economy": (a) the production loop is a closed input cycle (no terminal consumer — each good is wanted
only as the next role's input), an honest abstraction, not full Mengerian imputation from final wants;
(b) survival is **isolated off-market**, and the producer hearth that feeds it is **itself a survival
scaffold**. The earned claim is therefore "a neutral token monetizes in a produced cycle with survival
isolated off-market" — not scaffold-free. (The stronger versions — grounding the cycle in a final
survival/service sector, and produced [own-use cultivation/forage, S15] rather than hearth-minted
survival — are follow-ons, noted in Handoff.)

Principled-failure modes, all first-class (Codex — several likely):
- **A cycle good becomes provisional leader anyway** (one good's acceptances dominate).
- **The SALT anchor is too sparse to clear the breadth floors** (≥3 acceptors / ≥2 counterparts / 30%
  share) even with broken coincidence — densifying it is the only non-circular lever, and may not suffice.
- **The one-offer-per-agent book suppresses the cycle** so no trade volume forms (the cycle stalls).
- **Producers hoard SALT or starve** instead of round-tripping (means role incomplete).
- **Survival leaks back into the market** and reproduces S18 dominance.
Land `cycle_money_finding` with the characterized reason (the provisional-leader trace, the by-good
direct/indirect split, the round-trip ledger). Do NOT crank `salt_direct_use` to a universal want
(which suppresses indirect acceptance), do NOT mint, do NOT invent a consumption taste for X/Y/Z.

NOT perishability/carrying-costs (the *second* lever — add only if imperfect coincidence alone fails;
one variable at a time), NOT mortality (off in the flagship), NOT a new monetization mechanism (reuse
the strong-bar gate; only `salt_direct_use` density may be tuned, disclosed). Additive + gated;
flag/scenario off → S5–S18 + all goldens byte-identical.

## Verified Base Facts (oikos @ `cf017d4`)

1. **Goods/recipes are data** (`registry.rs:51`, `content.rs:142`, executor `society.rs:5580`); new
   `RecipeId` variants + a `ContentSet` ctor; vocations extend additively (Scholar/Confectioner:
   `settlement.rs:13554`, `:12411`, `:7489`).
2. **Derived input demand, pre-money**: `production_specialty` (`:13544`) → `producer_scale_extension`
   (`:13618`, `Horizon::Next`, reserved not eaten) drives **generic barter offers** pre-money via
   `near_unsatisfied_goods` (`agent.rs:250`). `set_project_input_bid_overrides` (`:8261/8240`) runs
   ONLY after a money good exists → **post-promotion only** (hardcoded Miller/Baker arms `:8302`; add
   cycle arms there for after promotion). Do NOT rely on it for pre-money cycle demand.
3. **Leadership = relative share + breadth floors** (`menger.rs:201/247`): 30% share /
   `min_acceptor_agents 3` / `min_counterpart_goods 2`; no absolute-volume bar. The anchor:
   `salt_direct_use` (`settlement.rs:1394`, `:7437`, heterogeneous, regression-theorem-clean,
   `medium_want_qty=0`); universal direct-want would suppress indirect offers (`:1404`). **Buyer-side
   gap (Codex P1a):** the anchor makes agents *want* SALT; it does NOT make SALT-holders want X/Y/Z.
   The medium endowment defaults to Gatherer/Consumer (`:12456`), not producers — so SALT must be
   seeded to the cycle producers (who have the derived input demand) or no SALT trade accrues.
4. **Survival off-market**: `run_producer_subsistence` (`:7653`, hearth-minted off-market) + own-use
   (`run_own_labor_subsistence` `:7700` pre-market, `run_own_use_cultivation` `:7877` post-market,
   never offered); satisfied wants skipped (`agent.rs:272`).
5. **SALT neutral** (`settlement.rs:1379`): not gathered/produced/consumed-as-input; `medium_good`.
6. **Metric**: derive `direct = acceptances − indirect_acceptances` from `emergence_acceptances()`
   (`:11254`, `CandidateAcceptances` `:3749`); `saleability_leader/bps` (`:10036`),
   `indirect_target_goods` + the round-trip ledger (S18).
7. Conservation/gating/determinism as established; `barter_camp` two-good model; the S18 multigood
   scenario (the structure to escape — perfect coincidence).

## The slices (build in order; each independently testable)

- **S19.1 — the 3-good production cycle (goods/recipes/roles + derived input demand).** Add 3 goods
  X/Y/Z + 3 `RecipeId` variants + a `ContentSet` cycle ctor (A: Z→X, B: X→Y, C: Y→Z), 3 cycle
  vocations (enum + tags + `production_specialty` + `build_agent` + production-phase + `ChainConfig`
  counts), each seeded with an input bootstrap buffer. **Pre-money vs post-money demand (Codex P1b — do
  not conflate):** pre-promotion, each role's input demand is the **derived `Horizon::Next` want**
  (`producer_scale_extension`) that drives **generic barter offers** via `near_unsatisfied_goods` +
  `generate_(in)direct_barter_offers` — `set_project_input_bid_overrides` runs ONLY after a money good
  exists (`settlement.rs:8240`) and so is a **post-promotion** concern (add the 3 cycle arms there for
  after promotion, not for pre-money demand). Survival off-market for ALL colonists (hearth/own-use).
  **Test:** the 3 roles produce X/Y/Z and each derives a `Next` barter demand for the previous role's
  output (its input); NO pairwise double coincidence (the holder of a role's input does not want that
  role's output); the cycle generates barter offers pre-money (no project-input-bid dependency before
  promotion); food/WOOD/forage trade ≈ 0 (survival off-market); conserved; flag off → byte-identical.
- **S19.2 — SALT buyer-side + the anchor density sweep + the saleability-race metric.** **Seed the
  CYCLE PRODUCERS a small SALT balance (Codex P1a, the buyer-side path):** the `salt_direct_use` anchor
  makes agents *want to receive* SALT but does NOT make SALT-holders want X/Y/Z; the default medium
  endowment goes to Gatherer/Consumer (`settlement.rs:12456`), who have no cycle demand. So SALT must
  start in the hands of agents with **derived input demand** — each cycle producer holds a small SALT
  balance (commodity stock, not designated money) so B can bid SALT for X, A for Z, etc., and SALT
  acceptances actually accrue. SALT stays neutral (not gathered/produced/consumed-as-input). Add the
  heterogeneous `salt_direct_use` anchor. **The density sweep (Codex P2b — not one tuned value):** the
  1-in-8 anchor may not yield ≥3 distinct SALT-acceptors in a small cycle colony; pin a sweep over
  `salt_direct_use` period `{8, 6, 4, 3, 2}` across fixed seeds, with period `1` (universal) ONLY as
  the "universal anchor suppresses indirect breadth" control — and **classify the outcome per period**
  (leads+promotes / leads-but-no-breadth / never-leads), not "find the one that passes." Add the
  by-good **derived direct/indirect** accessor (`direct = total − indirect` from `emergence_acceptances`);
  reuse the S18 `indirect_target_goods` accessor + round-trip ledger. **Test:** pre-promotion the cycle
  goods X/Y/Z show LOW direct acceptances (broken coincidence) while SALT's share is the highest; the
  derivation + `saleability_leader()` agree; the round-trip ledger credits on `IndirectFor{input}`
  accept and decrements on the `SALT→input` spend; the density sweep reports the per-period outcome.
- **S19.3 — the cycle-money scenario + DoD.** `frontier_cycle` composing the cycle + off-market
  survival + SALT anchor + the unchanged strong-bar gate. Register the `cycle` viewer scenario.
  **Test:** the acceptance suite below.

## Acceptance Tests (the S19.3 DoD) — `sim/tests/cycle_money.rs`

1. `cycle_run_is_deterministic` — byte-identical `(seed, config)`.
2. `no_pairwise_double_coincidence` — for each cycle pair, the holder of a role's input does NOT want
   that role's output; pre-promotion **direct** barter among X/Y/Z is near-zero (derived
   `direct = total − indirect`) — direct barter genuinely cannot clear the cycle.
3. `survival_stays_off_market` — food/WOOD/forage trade volume ≈ 0 (survival met by hearth + own-use);
   the universal necessities accrue ≈ zero saleability acceptances (the S18 trap is avoided).
4. `salt_leads_then_promotes_from_imperfect_coincidence` — **the core claim**: SALT becomes the
   provisional leader BEFORE promotion (highest share, clears the breadth floors), then **promotes**
   (promoted good IS SALT, `medium_want_qty=0`), and `indirect_target_goods(SALT) ⊇ {X, Y, Z}` — the
   medium bridges all three cycle inputs. (If instead a cycle good leads / SALT can't clear the floors /
   the cycle stalls, this becomes the documented `cycle_money_finding`.)
5. `salt_round_trips_through_the_cycle` — the traced ledger: SALT accepted `IndirectFor{input}` is later
   **spent on that input**; production continues post-promotion on market-acquired inputs (the means
   role completes; the cycle keeps turning on money).
5b. `anchor_density_sweep_classifies_the_outcome` — over `salt_direct_use` period `{8,6,4,3,2}` × fixed
   seeds (period `1` as the universal-anchor control), each cell is classified leads+promotes /
   leads-but-no-breadth / never-leads — the result is reported across the band, NOT a single tuned
   density; the universal (`period 1`) control shows indirect breadth suppressed (the anchor can't be
   made universal). The disclosed density is whatever the sweep shows, not searched-for-pass.
6. `cycle_conserves` — whole-system conservation every tick (recipe transforms input→output; SALT
   neutral; no minted cycle goods; survival hearth-minted is accounted).
7. `controls_close_the_finding` — (a) **collapse the cycle to a 2-good perfect coincidence** → S18-style
   no promotion; (b) **indirect acceptance disabled** → no promotion; (c) **no SALT anchor** → SALT
   doesn't lead; (d) **survival forced on-market** → food/WOOD dominate, SALT doesn't lead (the S18
   reproduction); (e) **no SALT seeded to the cycle producers** (buyer-side removed, Codex P1a) → no
   SALT trade accrues, SALT doesn't lead. These bracket "imperfect coincidence + off-market survival +
   a producer-held neutral token is what monetizes SALT".
8. `goldens_unchanged` — with the `cycle` scenario absent, S5–S18 scenarios + the six econ +
   g5a/g5b/coemergence + demographic (`lineages`) + g4a_death goldens byte-identical; S5–S18 suites
   green; new state (the cycle goods/recipes/vocations, the scenario) in `canonical_bytes` with
   regressions; the derived-accessor + round-trip ledger read-only/runtime-only; clippy `-D warnings`;
   fmt `--check`.

(Principled-failure path: if SALT never leads / a cycle good wins / the cycle stalls / producers hoard /
survival leaks, land `cycle_money_finding` with the characterized reason — NOT a tuned pass; the only
disclosed lever is `salt_direct_use` density, never a universal want or a minted shortcut.)

Manual: `cargo run -p viewer -- run cycle --ticks 3000`.

## Missing Interactions (the central risks)

- **The acceptor-floor balance (the decisive risk).** SALT leads only if ≥3 distinct selected colonists
  transact it AND it clears 30% share AND ≥2 counterparts. The 1-in-8 anchor may not produce 3 distinct
  acceptors in a small cycle colony; densifying it (lower `period`) is the lever, but a *universal*
  anchor (`period=1`) suppresses indirect acceptance (agents who directly want the leader are skipped).
  There is a window between "too sparse to clear the floor" and "so dense it kills indirect breadth" —
  if no density fits, that is the finding. Disclose the density used; do not search for the one that
  passes and call it untuned.
- **Survival off-market is make-or-break.** EVERY colonist's food/WOOD must be hearth/own-use fed; any
  on-market survival buyer re-floods the book and reproduces S18 dominance. Test 3 is the tripwire.
- **The cycle must actually turn.** No pairwise coincidence + the one-offer-per-agent book could stall
  the cycle (no volume) before SALT can lead. Bootstrap input buffers + the project-input bids must let
  the cycle clear via SALT once SALT leads; if it deadlocks pre-money, that is the finding.
- **Don't invent consumption taste for X/Y/Z.** Demand is derived input demand only (the producer needs
  it to run its recipe). The cycle is internally self-demanding (B needs X, C needs Y, A needs Z); no
  terminal consumer is required, but say plainly the cycle is a production loop (honest abstraction).
- **No second lever yet.** Perishability/carrying-costs are out of scope (the *next* test if imperfect
  coincidence alone fails) — one variable at a time, so the result attributes cleanly to coincidence.
- **Determinism.** New goods/recipes/vocations/scenario in `canonical_bytes` with regressions; the
  derived accessor is read-only; the round-trip ledger runtime-only (like S17/S18). Flag off →
  byte-identical.

## Handoff Notes

- **This is the arc's hardest, most open milestone** — the first real shot at endogenous token money in
  a produced economy. Reuse everything: the producer project/input-demand machinery, the strong-bar
  gate + emergence probe, the S18 `indirect_target_goods` accessor + round-trip ledger, the hearth/
  own-use survival, `barter_camp`/the chain as the production reference. The new work is the 3-good
  cycle (goods/recipes/roles + the 3 input-bid arms), the off-market-survival composition, the derived
  direct/indirect accessor, and the scenario.
- **Honest two-way DoD**: SALT monetizes from imperfect double coincidence (success — a neutral token
  emerging as the medium in a produced cycle with survival isolated off-market; NOT "scaffold-free" —
  the hearth survival and the closed input-loop are disclosed abstractions) OR a characterized
  `cycle_money_finding` (a cycle good leads / the anchor can't clear the floors / the cycle stalls /
  hoarding / survival leaks). Both are real and important; do not fake the success by a universal
  anchor, minting, or invented taste. The stronger claims (final-want grounding; produced not minted
  survival) are follow-ons.
- **The proof is the by-good direct/indirect split + the cycle-spanning `indirect_target_goods ⊇
  {X,Y,Z}` + the round-trip** — promotion alone could be incidental (the S16 lesson); the medium must
  *demonstrably bridge the broken coincidence*.
- **Gate everything**; the `lineages` + `g4a_death` goldens are the tripwires.
- Build S19.1→S19.3 as separate commits with their own tests; `git add` new files.
- **After S19:** whichever way it lands, the money-emergence question is fully mapped — emergence under
  a scaffold (S9), the produced-economy boundaries (S16/S18), and imperfect-coincidence (S19, success
  or the precise threshold finding). Remaining items (perishability as a 2nd lever; a 10k mortality
  smoke test) are optional add-ons, not foundational.
