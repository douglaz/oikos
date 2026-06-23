# Implementation Spec: money from a produced MULTI-GOOD economy (S18)

> The deepest remaining milestone — closing the S16 reframing finding. S16 proved produced bread
> can supply a market but SALT never monetized: with only one produced market good, every bread↔SALT
> trade was **direct**, so SALT accrued **zero indirect-exchange breadth** and never crossed the
> strong-bar gate. Codex's prescription, now the design: a real **division of labor** with **two**
> produced/gathered goods and **role-separated cross-demand** — bread cultivators (sell bread, want
> WOOD) ⇄ woodcutters (sell WOOD, want bread) — so each accepts SALT as a **means** to the *other*
> good (`IndirectFor{target}`), SALT round-trips as the more-saleable intermediary, and money emerges
> from a produced multi-good economy. NOT "more bread", NOT a new craft good (WOOD already is a
> produced/gathered final good — a new good would risk the want-invention circularity we killed).

## What the research + Codex established

- **The lever is role-separated cross-demand, not a fed colony.** SALT gets `IndirectFor{target}`
  (`generate_indirect_barter_offers`, `econ/src/society.rs:2090`) only when SALT is the provisional
  leader, the agent has an unsatisfied target ≠ SALT, holds a surplus good ≠ SALT ≠ target, and the
  means-swap is acceptable (`would_accept_indirect_barter_swap_with_stock`, `econ/src/agent.rs:477`).
  Two distinct targets (bread AND WOOD) are produced only because the two roles want different goods.
- **WOOD is already a real produced good.** WOOD = `GoodId(2)`, a real `ResourceNode`
  (`good.rs:8`, node like grain/forage, `settlement.rs:730/4558`); today gatherers are assigned
  **round-robin over `config.nodes`** (`:4710/6394`), so S18 must add an explicit WOOD-node seam to
  make woodcutters land on WOOD (not split with grain). Warmth→WOOD demand is **universal + automatic** (every
  colonist's warmth depletes each tick, `Horizon::Now`; one shared `known`, `scale.rs:215`,
  `need.rs:118`). So cross-demand is free; the lever is endowments + patience.
- **The preemption trap (must avoid).** The one-offer-per-agent book offers an agent's *lowest-good-id*
  surplus first (`post_first_direct_barter_offer`, `society.rs:2061`); WOOD (id 2) preempts bread. S16
  worked around it by deleting WOOD surplus. **S18's fix is role separation** — each agent holds only
  ONE surplus good, so there's nothing to preempt.
- **SALT leads via the heterogeneous `salt_direct_use` anchor** (`settlement.rs:3076`, `:5322`):
  ~1-in-8 directly want SALT → it becomes the provisional leader (`menger.rs:201`); the rest are
  eligible to accept it *indirectly* (direct-wanters are excluded from posting indirect offers,
  `society.rs:2111`). If a produced good out-leads SALT on direct share, SALT never leads (the S16
  failure) — so the anchor + role separation must keep SALT the leader.
- **The WOOD mint is ungated** — only FOOD is gated by own-labor (`settlement.rs:6844`); WOOD
  (`:6860`) mints unconditionally. Neutralize with `HouseholdSpec.wood_provision = 0` so cultivators
  are genuinely WOOD-short and must *buy* WOOD (S16 used a wash-to-1; S18 wants 0).
- **The metric gap + the hoarding risk (two code adds).** The strong-bar gate counts
  `indirect_target_goods` but the sim probe collapses it to a *count* (`settlement.rs:10889`); the
  membership (`&[GoodId]`) exists on `CandidateSaleability` (`menger.rs:70`) — S18 must surface it to
  assert `target_goods ⊇ {bread, WOOD}`. And the gate counts an acceptance *at receipt*, not whether
  SALT is later **spent** (`observe_acceptance`, `menger.rs:236`); a self-sustaining loop needs SALT
  to **round-trip** (cultivators end up with WOOD, woodcutters with bread) — Codex's "means role
  incomplete" failure — so S18 needs a spent/round-trip guard, not just accept-side volume.

## Purpose & the honest bar

On a fresh gated scenario (`frontier_multigood` = `frontier_cultivation` + S9 strong-bar SALT +
the S16 cultivated-bread surplus seam/provenance + a WOOD node + a woodcutter group + minted bread
AND WOOD off + the buy/sell split + **mortality off**): test whether money emerges from a produced
multi-good economy. Success = SALT **promotes under the unchanged strong-bar gate** (`medium_want_qty
= 0`), driven by **two-sided produced indirect breadth** — bread sellers accept SALT
`IndirectFor{WOOD}` AND woodcutters accept SALT `IndirectFor{bread}`, distinct acceptor agents meet
the gate, the pre-promotion bread+WOOD traded are produced/gathered (not minted, by the provenance
ledger), AND the SALT **round-trips** (producers spend it on the target — not hoarded). Honest
target: **test whether** a role-separated produced multi-good division of labor monetizes SALT.

Principled-failure modes, all first-class (Codex — and several are likely):
- **SALT never becomes provisional leader** — direct bread/WOOD barter dominates its saleability share.
- **A produced good (bread or WOOD) becomes the rejected saleability leader** instead of SALT.
- **One-offer-per-agent mechanics still suppress indirect offers** even with role separation.
- **Means role incomplete** — agents accept SALT indirectly but **hoard** it (no round-trip), so the
  "money" never actually intermediates.
Land `multigood_money_finding` with the characterized reason (the provisional-leader trace, the
by-target breadth, the round-trip metric) if it isn't the success. Do NOT rescue it by minting or by
inventing a want.

NOT mortality (off in the flagship — proven S17; a robustness test later), NOT a new craft good
(WOOD is the second good), NOT a new monetization mechanism (reuse the S9 strong-bar gate unchanged —
S18 only supplies a real second produced good + role separation). Additive + gated; flag/scenario off
→ S5–S17 + all goldens byte-identical.

## Verified Base Facts (oikos @ `1a7f133`)

1. **Indirect offers** (`society.rs:2090`): leader-gated; skips target⊇leader (`:2111`); posts
   `give→leader` `IndirectFor{target}` if the means-swap is acceptable (`agent.rs:477`).
2. **Provisional leader** (`menger.rs:201`): direct-acceptance-share leader past the weak-bar
   thresholds; SALT leads via `salt_direct_use` (`settlement.rs:3076/5322`).
3. **WOOD** = `GoodId(2)` real `ResourceNode` (`good.rs:8`, `settlement.rs:730/4558/4710/6394`);
   warmth→WOOD want universal `Now` (`scale.rs:215`, `need.rs:118`).
4. **The preemption artifact** (`society.rs:2061`, lowest-good-id surplus first); S16 workaround
   (`settlement.rs:3389-3402`). Role separation (one surplus good/agent) avoids it.
5. **WOOD mint ungated** (`settlement.rs:6860`); neutralize via `wood_provision = 0`. FOOD mint gated
   (`:6844`). The buy/sell split `cultivation_sells_surplus_active` scopes cultivation to lineage
   spatial members (`:7554`), keeping non-lineage consumers as buyers. **WOOD seed buffers exist**
   (household `starting_wood`, chain `wood_buffer`, gatherer/consumer wood endowments,
   `settlement.rs:11902`) — zeroing the mint alone does NOT make traded WOOD provenance-clean (Codex
   P1a). **Gatherer→node assignment is round-robin over `config.nodes`** (`:4710`) — with both a grain
   and a WOOD node, non-lineage gatherers split, so an explicit woodcutter→WOOD seam is needed (P1b).
6. **The metric + the gate default**: gate (`menger.rs:266`) counts `indirect_acceptances`/
   `indirect_acceptor_agents`/`indirect_target_goods`; the S9 strong scenario sets
   `min_indirect_target_goods = 1` — S18 must set it to **2** for the two-sided bar (P1d). Membership
   on `CandidateSaleability.indirect_target_goods` (`menger.rs:70`) but the sim probe collapses to a
   count (`settlement.rs:10889`) — S18 adds the by-target accessor. `observe_acceptance` counts at
   receipt, not spend (`menger.rs:236`) — hence the traced pending-indirect-SALT round-trip ledger.
   The S16 provenance ledger + `bread_for_salt_volume_by_provenance` (bread only — WOOD needs its own
   source accounting).
7. Conservation/gating/determinism as established; `barter_camp` (`settlement.rs:2218-2297`) is the
   existing two-good (FOOD+WOOD) specialist-gatherer model to mirror.

## The slices (build in order; each independently testable)

- **S18.1 — the woodcutter role + a market WOOD supply (the second produced good).** In the new
  scenario: a woodcutter group (non-lineage `Gatherer`s) produces+sells WOOD and (universally) wants
  bread; cultivators (lineage, `cultivation_sells_surplus`) produce+sell bread and want WOOD.
  **Pin the woodcutter→WOOD-node assignment (Codex P1b):** gatherer assignment is round-robin over
  `config.nodes` (`settlement.rs:4710`), but S18 has BOTH a grain node (cultivators' input) AND a
  WOOD node — so add an explicit node-assignment seam routing the non-lineage gatherers to the WOOD
  node (not round-robin), or grain would draw off "woodcutters" into a third surplus. **WOOD must be
  market-supplied AND provenance-clean (Codex P1a):** set every `HouseholdSpec.wood_provision = 0`
  (no per-tick mint) AND zero every initial WOOD buffer (`starting_wood`, chain `wood_buffer`,
  gatherer/consumer wood endowments, `settlement.rs:11902`) so traded WOOD can ONLY come from
  node-gathering; add a WOOD source check (traded WOOD bounded by WOOD node→econ transfer volume — or
  extend the S16 provenance ledger to WOOD). Keep cultivators OFF the WOOD node + `wood_provision=0`
  so each role's only *surplus* is its produced good (no preemption). Both food AND WOOD mints off.
  **Test:** a cultivator holds bread surplus + zero WOOD (+ an unsatisfied WOOD/warmth want); a
  woodcutter holds WOOD surplus + zero bread (+ an unsatisfied bread/food want); woodcutters are on
  the WOOD node (not grain); traded WOOD is gathered (bounded by node transfers, no minted/buffered
  WOOD: `endowment[WOOD] == 0` AND zero WOOD buffers); conserved; flag off → byte-identical.
- **S18.2 — indirect-breadth instrumentation: the by-target accessor + a TRACED round-trip ledger.**
  Add a public accessor surfacing the SALT candidate's `indirect_target_goods` **membership** (the
  `&[GoodId]`, not just the count, `menger.rs:70`). For the round-trip guard, **net-acquiring the
  target is too weak (Codex P1c)** — it can come from direct barter, buffers, or estate. Instead trace
  the actual sequence with a **per-agent pending-indirect-SALT ledger**: when an agent accepts SALT as
  `IndirectFor{target}`, credit `pending[agent][target] += qty`; when that agent later trades
  `SALT → target`, decrement it. The round-trip metric = the fraction of indirect-accepted SALT
  subsequently **spent on its target** (means-role-complete). The ledger is **runtime-only** (NOT in
  `canonical_bytes` — diagnostic/proof state, like S17's `starvation_deaths_total`), so it shifts no
  digest; determinism is pinned by the existing digest. **Test:** the by-target accessor returns
  the target set; the round-trip ledger credits on indirect accept and decrements on the SALT→target
  spend; a **hoarding control** (no counterparty for the target) shows the round-trip fraction ≈ 0
  while accept-side volume > 0 (the guard detects incomplete means, not net stock noise).
- **S18.3 — the multi-good emergence scenario + DoD.** `frontier_multigood` composing the three roles
  (SALT-anchor consumers + bread cultivators + woodcutters), mints off, buy/sell split, mortality off,
  S9 strong-bar. **Gate-align the two-sided bar (Codex P1d):** S9's strong scenario uses
  `min_indirect_target_goods = 1`; S18 claims emergence from `{bread, WOOD}` two-target breadth, so set
  this scenario's gate to `min_indirect_target_goods = 2` (mechanism unchanged — a stricter, honest
  parameter: SALT cannot promote until BOTH targets are present as indirect goods). Register the
  `multigood` viewer scenario. **Test:** the acceptance suite below.

## Acceptance Tests (the S18.3 DoD) — `sim/tests/multigood_money.rs`

1. `multigood_run_is_deterministic` — byte-identical `(seed, config)`.
2. `two_clean_surplus_goods_no_preemption` — pre-promotion, no agent **offers** from both surplus
   classes (Codex P2: after round-tripping a cultivator may *hold* bought WOOD, but it never *offers*
   WOOD as surplus), so the lowest-good-id preemption can't fire and BOTH bread and WOOD reach the
   barter book; woodcutters are on the WOOD node, not grain.
3. `salt_promotes_via_two_sided_indirect_breadth` — **the core claim**: SALT promotes (promoted good
   IS SALT) under the strong-bar gate with `medium_want_qty = 0` and `min_indirect_target_goods = 2`,
   and the by-target accessor shows pre-promotion `indirect_target_goods ⊇ {bread, WOOD}` — bread
   sellers accept SALT `IndirectFor{WOOD}` AND woodcutters accept `IndirectFor{bread}`, distinct
   acceptor agents meeting the gate. SALT must lead BEFORE the second target appears (not promote on
   one target then backfill).
4. `the_traded_goods_are_gathered_not_minted` — pre-promotion bread volume is produced (the S16
   ledger) and pre-promotion WOOD volume is **gathered** (bounded by WOOD node→econ transfers, with
   all WOOD buffers + the mint zeroed: `endowment[WOOD] == 0` and no seeded WOOD) — neither is minted.
5. `salt_round_trips_not_hoarded` — the **traced** round-trip ledger is material: a substantial
   fraction of SALT accepted `IndirectFor{target}` is later **spent on that same target** (pending
   ledger credited on accept, decremented on the SALT→target spend) — the means role completes, SALT
   actually intermediates (not net-stock noise, not hoarded).
6. `multigood_conserves` — whole-system conservation every tick (grain + WOOD node regen the sources;
   produced bread; no minted food/WOOD).
7. `controls_close_the_finding` — (a) **no WOOD market** (drop the woodcutters) → S16-style
   single-good, SALT does NOT promote; (b) **indirect acceptance disabled** → no promotion; (c) **no
   SALT direct-use anchor** → SALT does not lead/promote; (d) **no role separation** (one group holds
   both surpluses) → preemption / no two-sided breadth. These bracket "two-sided produced breadth is
   what monetizes SALT".
8. `goldens_unchanged` — with the S18 scenario absent, S5–S17 scenarios + the six econ +
   g5a/g5b/coemergence + demographic (`lineages`) + g4a_death goldens byte-identical; S5–S17 suites
   green; new state (the scenario flag, the by-target accessor is read-only, any round-trip counters)
   in `canonical_bytes` with regressions (or runtime-only, like the S17 counter — pick and state);
   clippy `-D warnings`; fmt `--check`.

(Principled-failure path: if SALT never leads, a produced good wins, indirect offers stay suppressed,
or agents hoard, land `multigood_money_finding` with the characterized reason — NOT a tuned pass.)

Manual: `cargo run -p viewer -- run multigood --ticks 3000`.

## Missing Interactions (the central risks)

- **Keeping SALT the leader is the crux.** With two produced goods now both traded, either could
  out-lead SALT on direct-acceptance share and become the (rejected) saleability leader — then no
  indirect offers fire. The `salt_direct_use` anchor must be strong enough (heterogeneous) that SALT
  leads, but not so strong it suppresses indirect acceptance (direct-wanters are excluded from
  indirect offers). This balance is the likely failure point; characterize it, don't tune past it.
- **The hoarding / round-trip risk (Codex).** Indirect acceptance is counted at receipt; the gate can
  pass on accept-side volume while SALT pools on one side and never round-trips. `salt_round_trips_not_hoarded`
  is the guard — without it, "money emerged" would be an overclaim.
- **Role separation must be clean.** If any agent holds both bread and WOOD surplus, the good-id
  preemption resurfaces and one good never reaches the market. wood_provision=0 + cultivators off the
  WOOD node + the buy/sell split must jointly guarantee one-surplus-good-per-agent (test 2).
- **No new monetization mechanism / no minted shortcut.** Reuse the S9 strong-bar gate unchanged; S18
  only adds a real second produced good + role separation. Bread+WOOD must be produced/gathered
  (provenance), SALT promotion via genuine two-sided breadth.
- **Determinism.** The by-target accessor is read-only; any round-trip counter is runtime-only (like
  S17's starvation counter) or gated — flag/scenario off → byte-identical.

## Handoff Notes

- **This closes the S16 finding** — money from a produced multi-good economy via a real division of
  labor. Reuse: the S9 strong-bar gate + emergence probe, the S16 cultivation surplus seam +
  provenance ledger, the WOOD node + gatherer haul, `barter_camp`'s two-good specialist structure, the
  buy/sell split. The new work is the woodcutter role + WOOD-market scenario, the by-target breadth
  accessor, the round-trip guard, and the composed three-role scenario.
- **Honest two-way DoD**: SALT monetizes via two-sided produced indirect breadth (success, closes
  S16) OR a characterized `multigood_money_finding` (SALT never leads / a good wins / offers
  suppressed / hoarded). Both are real; do not fake the success by minting or want-invention.
- **The by-target breadth + the round-trip metric are the proof** — promotion alone could be incidental
  (the S16 third-outcome lesson); `target_goods ⊇ {bread, WOOD}` + SALT actually round-tripping is what
  earns "money from a multi-good economy".
- **Gate everything**; the `lineages` + `g4a_death` goldens are the tripwires. Mortality off in the
  flagship; a mortality-on robustness test is a follow-on.
- Build S18.1→S18.3 as separate commits with their own tests; `git add` new files.
- **After S18:** the arc covers the produced multi-good money question too; remaining items are
  robustness add-ons (a 10k-tick mortality smoke test, a mortality-on multi-good run), not new
  foundational mechanisms.
