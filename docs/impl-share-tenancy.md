# impl-58 — C1R: Voluntary Output-Share Tenancy (does the no-advance labor contract clear where money wages could not?)

Status (spec): **SPEC-READY** (Codex xhigh spec-review round 1: 1 P0 [owner dominance → the cap-waste
gate] + 4 P1 + 5 P2 folded in; round 2: all findings RESOLVED, no P0, the single residual P1
[`cap_at_start` in the record sketch] + 2 polish notes [nodes start at cap; drawdown-check timing]
folded verbatim). P1 of the replan
(`docs/review-and-replan-2026-07.md`): the genetically-next labor institution after the C1 null. Base:
branch **`feat/wage-labor-impl-rb` @ `f372c12`** (the composed S23c tag 18 + S23d tag 20 + S23e tag 21 +
C1 `wage_labor` tag 22 stack — C1's default-off machinery stays for same-seed comparative cells). Flag
`share_tenancy`, digest **tag 23** (code-verified first-free on the branch: `canonical_bytes` pushes
…18,20,21,22 and nothing pushes 23; the C2 reference-spec reservation was paper-only), ON-only.

Falsifiable bar (headline): on the S23e **marginal** commons (φ=0.5) where C1 measured
`WageMarketVacuous`, does a **voluntary output-share contract** — a landless worker works an owner's
plot for a pre-agreed share of the realized bread, **no advance of money or goods on either side** —
**clear and renew** across seeds, with worker survival materially contract-funded and owner surplus
positive from otherwise-unworked land?

## 0. One-paragraph summary

C1 proved the money-wage market cannot open on this base: the owner's own-money willingness-to-pay sits
below the worker's reservation ask, exactly as the wages-fund doctrine predicts when no savings exist
(a wage is an *advance*). The share contract is the institution that needs **no advance at all**: the
worker is paid **out of the realized output itself**, after production, in kind — so it requires neither
accumulated money, nor an accumulated bread fund, nor even money's existence (share tenancy is
historically and praxeologically *prior* to monetary wages; C1R is therefore **not** post-promotion
gated). Both sides gain ordinally: the hungry landless worker (whose alternatives — the finite rival
commons, buying with SALT it doesn't earn, C1 wages that never clear — demonstrably fail it) gets
`s·N` bread for labor it values below that bread; the owner gets `(1−s)·N` bread harvested from
**regen its at-cap plot was wasting** (spec-review P0 correction: an unworked plot's regen *accrues in
the node up to cap*, so "unworked" alone is NOT zero-opportunity-cost — the owner could harvest that
stock later; the honest dominance gate shares out only **at-cap plots**, where marginal regen is
literally destroyed unharvested, and bounds the worker's draw to the regen rate so durable stock never
draws down — then, and only then, the owner's gain is genuinely something-for-nothing). Mechanically
C1R is a small delta on verified machinery: the plot-admission hook exists (`reserved_for`), the
production path exists (haul → `Cultivate` → own-use phase), the worker's outside-option gate is reused
verbatim (`forecast_commons_sufficiency`, goods-denominated), the contract record/term/renewal/death
patterns come from `WageEscrow` + S22f. The genuinely new pieces: a **bread-denominated ordinal
acceptance** for the worker, a **dominance acceptance** for the owner, and the **conserved,
provenance-exact output split**. Classify-not-tune: `share_bps` and term are pinned consts **swept**,
never searched; `ShareVacuous` (even the no-advance contract fails to clear) is a first-class null that
would mean the problem is deeper than the wages-fund gap.

## 1. The base facts this spec is built on (disclosed, load-bearing)

1. **Landlessness on this base is structural, and that is a *disclosed scope condition*.** Land is
   abundant (48 plots: 4 good regen-64/cap-8000, 44 marginal regen-12/cap-1000; settlement.rs:243–250)
   against ~8 lineage owner-households — but the ~48 non-lineage Consumers/Gatherers **cannot
   homestead**: `mortal_landowner_claim_eligible` requires a reproductive lineage actor
   (`household.is_some()`, :20600–20614), the gate that keeps the S23d owner-identity invariants true
   (letting the immortal roster own land would resurrect the S23c disjoint-populations wall). So the
   landless class *is* the excluded-from-ownership class, and C1R tests exactly the institution that
   lets that class participate in production **without owning**. The claim is scoped accordingly:
   *given a landless class* (here enforced by the base's owner-identity design), does voluntary share
   tenancy clear where money wages did not? Within the lineage class homesteading remains open, so the
   contract must still beat the worker's real alternatives (§4.2), not a rigged empty set.
2. **Owners hold unworked plots whose regen is wasted at cap — and only THAT waste is free.** An owner
   works **one** plot at a time (`private_land_target_for_agent` returns a single min-distance own
   plot, :11151–11209) while impartible inheritance consolidates a deceased owner's *every* plot onto
   one heir (:12890–12945) and depleted-plot claims stack. **But (spec-review P0): node stock accrues
   up to cap** (`world/src/node.rs:18–26, 52–59`; `world/src/world.rs:671–680`) — an unworked plot is
   a growing stock the owner could still harvest later, so "unworked" is not free. **At cap, however,
   further regen is destroyed** — that flow is a genuine deadweight the owner forgoes at zero
   opportunity cost. The honest owner gate (§3.1) therefore shares out only **at-cap** owned plots not
   currently owner-targeted/reserved, and bounds the contract draw to the regen rate. Detection uses
   the already-digested node state (stock == cap; regen), not diagnostic counters.
3. **The admission hook exists.** `reserved_for == Some(worker)` (or membership in `shares`) admits an
   agent to an owned plot in *both* seams — target selection (:11174) and the pre-tick exclusion
   (:11236–11244) — confirming the C4 finding on this branch. C1R reuses `reserved_for` as the
   contract-term admission (single worker per plot; semantics disclosed: it also blocks other
   claimants, which is inert here since owned plots are never claimable).
4. **The production path is the own-use cultivation machinery.** Haul (fast loop, `GoHarvest`
   carry_cap 6, :10877–10958) → transfer credits the hauler's stock (:10021–10113) → the no-tool
   `Cultivate` grain→bread conversion in `run_own_use_cultivation` (:14418–14595) booking
   `produced`/`consumed_as_input` exactly and crediting `SelfProduced` provenance. The split divides
   exactly this output.
5. **A share contract is not a monetary phenomenon.** Unlike C1 (post-promotion-gated), C1R activates
   with the S23e substrate only (`share_tenancy && rival_subsistence_commons_active`); it can clear
   pre-money. Whether share-workers' surplus sales then *feed* SALT promotion is an emergent
   composition effect — reported, never suppressed (§4.5).

## 2. The central question and pre-named outcomes

**Central question.** On the S23e marginal base where `WageMarketVacuous` landed, when a landless
hungry worker may **voluntarily** contract to work an owner's otherwise-unworked plot for a pinned,
swept output share `s` — no advance, no money required, both sides accepting **ordinally** — do share
contracts **clear and renew** across `SEEDS=[3,7,11,19,23]`, with worker survival materially
contract-funded, owner share income positive, the S23d owner-identity/inheritance preconditions intact,
and conservation (goods, commons, provenance, money) holding every tick — and is it genuinely voluntary
(a `forced_share` scenario separates as scaffold; `no_contract` reproduces the C1/S23e null)?

**Ordered verdict enum** (first-match, scaffold-before-vacuity per the C1 classifier discipline):

```
Base preconditions (disqualifying):
  BaseUnviable            — the S23e base fails to reproduce (no promotion on marginal, etc.)
  ConservationBroken      — goods / commons / money / provenance-ledger conservation failed a tick
  RegistryBroken          — plot-registry or owner-identity invariants (S23d) violated
Scaffold (by SCENARIO MODE, before vacuity):
  ShareScaffoldOnly       — the run IS a forced_share control (contracts imposed, not chosen)
Vacuity (headline voluntary run only):
  ShareVacuous            — < MIN_CONTRACTS voluntary contracts ever clear (the no-advance contract
                            ALSO fails ⇒ the wall is deeper than the wages-fund gap — a major finding)
Outcome:
  ShareTenancyClears      — success clauses §2.1 all hold (the headline positive)
  ShareClearsButNoLift    — contracts clear + renew, but no material worker-survival lift and/or no
                            positive owner gain (the lever bites but does not matter — honest null)
```

**§2.1 success clauses (all required for `ShareTenancyClears`):**
1. **Non-vacuity + voluntariness:** ≥ `MIN_CONTRACTS` voluntary contracts clear per run; some eligible
   workers/owners decline (both gates are real); every contract traceable to both ordinal acceptances.
2. **Renewal:** ≥ `MIN_RENEWALS` term-expiry re-decisions renew from fresh state (S22f pattern — chosen
   persistence, not one long binding).
3. **Worker lift:** contracted workers' final-window consumption is materially share-funded
   (`share_funded_consumption_share ≥ θ_SHARE`) **and** final-window survival of the landless class
   lifts over the matched `no_contract` run by ≥ `SURVIVAL_LIFT`.
4. **Owner gain:** cumulative owner share income `(1−s)·N > 0` booked from **at-cap plots the owner
   did not itself work that term** (verified against the cap-waste gate conditions + `WorkedLandEvent`),
   with contract draws never pulling a plot below its cap-at-contract-start (the regen bound held).
5. **Base intact:** S23d owner-identity counters hold (`immortal_roster_owned_plot_ticks==0`,
   `non_lineage_owner_plot_ticks==0`, `owner_old_age_deaths>0`), inheritance fires, conservation +
   provenance + commons rivalry every tick. **A share contract must never confer or imply title** —
   workers remain non-owners (clause 5 is the anti-title guard).

## 3. Mechanism

### 3.1 The contract and the match

`ShareContract { id, owner, worker, node, share_bps, term, opened_tick, renewals, cap_at_start }` —
the `WageEscrow` record minus every money field, plus `node`/`share_bps`/`term` and **`cap_at_start`**
(the plot's stock at contract open, the drawdown-guard baseline §2.1-4/§3.1; template :6986–7005). A gated
**pre-market phase at the C1 seam** (:9993; runs when `share_tenancy_active()`, no money gate):

- **Worker side.** Eligible = landless (`holds no plot`), non-lineage Consumer/Gatherer, hungry, and
  the reused **`forecast_commons_sufficiency` gate says the commons will NOT feed it this tick**
  (:15557 — goods-denominated, reused verbatim). Acceptance (**the genuinely new ordinal evaluator**):
  expected term output **`N̂` is derived purely from already-digested current state** — the contracted
  plot's `regen` × the term × the contract labor/haul budget (the cap-waste gate bounds the draw to
  regen, so regen *is* the expected flow; no realized-experience history is needed, which keeps the
  tag-23 digest complete — spec-review P1 resolved by construction). The worker's expected share is
  the **exact integer** `floor(N̂ · share_bps / 10_000)` — the *same* floor math the split uses (§3.3).
  Accept iff adding that bread to the worker's stock **newly provisions a `Good(BREAD)` want ranked
  above its first unsatisfied Now-Leisure want, preserving all higher-ranked wants** — the exact
  `reservation_labor_ask_from_claims_for_money` bitmap pattern (**econ**/src/factor.rs:539–597) with
  the money receipt swapped for a bread-into-stock receipt. Pre-promotion, SALT is just a good on the
  scale — the evaluator must not treat it as money before promotion (no money concept enters at all).
- **Owner side (the P0 fix — cap-waste dominance, honestly restored).** Eligible = live lineage owner
  holding ≥1 plot that is (a) **at cap** (`node.stock == node.cap` — further regen is being destroyed),
  (b) **not its current work target** and not otherwise reserved/escrowed, (c) `share_bps < 10_000`,
  (d) output headroom exists. The **contract draw is bounded to the plot's regen rate per tick**, so
  the worker harvests only the flow the cap was destroying and durable stock never draws down below
  cap-at-contract-start. Under these conditions — and *only* these — the owner's `(1−s)` share is a
  genuine something-for-nothing gain and a dominance check (no appraisal) is praxeologically sound.
  An owner whose plots are below cap has a real future-own-harvest opportunity cost; sharing such
  plots would need a full ordinal comparison against later own use — **out of scope for C1R,
  disclosed** (a `stock_opportunity_refusal` diagnostic counts these declines).
- **Match:** the C1 two-sided greedy shape (sorted deterministic order, one contract per worker, one
  worker per plot), with the C1 price machinery deleted — matching is on the two boolean acceptances.
  Admission: set `reserved_for = Some(worker)` on the contracted plot for the term; cleared on
  expiry/death. **Owner-exclusion guard (spec-review P1):** `reserved_for` admits the worker, but the
  *owner* can still be routed to its own reserved plot (holder admission wins in target selection
  :11168–11179; validation reroutes via the same selector; the work-event path accepts holder harvest
  before the reserved-by-other check :11334–11364) — so C1R adds an explicit guard: for the term, the
  owner's target selection **skips** its share-reserved plots, validation **reroutes** owner tasks
  aimed at them, and the work-event path rejects owner draws from them (a `reservation_collision`
  diagnostic counts reroutes). **Mutual exclusion:** a worker in a live wage escrow is not
  share-matchable and vice versa; a share-reserved plot is not wage-workable. The headline never runs
  both institutions in one cell (a both-on config is out of scope, guarded by a debug assertion).

### 3.2 The work (spatial, real — not abstracted)

The contracted worker is steered exactly like a cultivator **to the contracted plot** (the fast-loop
steering seam :10877–10958, with the C1-style contract hook replacing the escrow-idle hook :10882):
it hauls grain (`GoHarvest`, capped per tick at the plot's **regen** — the cap-waste bound, §3.1),
deposits, and the transfer credits **its own stock** with the contract-sourced grain
(`carried_grain_source` attributes the plot). **Deposit-attribution fix (spec-review P1):** the fast
loop only snapshots/attributes deposits for `foraging || cultivating || Gatherer` agents
(:10575–10590, :10631–10670, predicate :10825–10830) — a share worker steered as a Consumer could
harvest and deposit *unattributed* within one fast-loop interval. C1R therefore **adds
share-contracted workers to that attribution predicate** (via their digested contract state), so every
contract haul is attributed. In the own-use cultivation phase (:14418) the worker runs `Cultivate`
under a per-contract labor budget, producing `N` loaves. Unlike C1 (which idled the worker and
produced abstractly from employer stock), C1R's work is the real spatial production path — the
machinery already exists for cultivators.

### 3.3 The split (conserved, provenance-exact — the load-bearing new booking)

At conversion, `report.consumed_as_input[grain] += N_in` and `report.produced[bread] += N_out` are
booked **once**, exactly as today (:14521/:14535). The split then divides holdings, not bookings —
and **it runs immediately at conversion, BEFORE the own-use consume step** (:14563–14583; spec-review
P2: otherwise the worker could eat bread owed to the owner). Worker keeps
`floor(N_out · share_bps / 10_000)` — the **identical integer floor** the acceptance evaluator uses
(§3.1), remainder to the owner; rounding direction disclosed, not tunable. The owner's share moves
worker→owner as a **conserved stock transfer with origin-preserving provenance/acquisition follow** —
the machinery exists and is verified: `BreadProvenance::transfer` (:7391–7405), acquisition
`transfer_preserve` (:7768–7777), as used by birth endowments (:13482–13501). **Provenance attribution
(pinned):** the **worker** is the producer — `bread_provenance.credit_produced(worker, N_out,
non-lineage)` + `acquisition.credit(worker, SelfProduced, …)` — because its labor made the bread; the
owner's share arrives as a transfer whose `SelfProduced` origin is preserved. Both ledger finalizers
(:10234/:10241) must stay exact every tick. **Disclosed metric implication (worded precisely,
spec-review P2):** contracted workers thereby enter the `SelfProduced` **producer/provenance** set of
the pre-promotion monetization telemetry (:19119/:18644) — correct, since they genuinely produced —
while the **seller** metrics remain distinct (:18739–18777) and are affected only if a worker actually
sells its share; both effects are **reported**, never suppressed. The S23e owner-seller attribution
(`current_or_ever_landowner`) is unaffected for owners.

### 3.4 Term, renewal, death

Term = `SHARE_TERM` econ ticks (pinned, swept). At expiry both sides **re-decide from fresh state**
(the S22f `commitment_remaining`/`renewals` pattern, :14281–14335); renewals counted — persistence must
come from re-choosing. Death routing (the C1 `settle_wage_labor_for_death` fork, :16069, settled
**inside death removal**, before estate collection): worker dies → contract dissolves, realized split
stands, `reserved_for` cleared; owner dies → contract **dissolves**, the plot passes to the heir
(S23c), the heir may re-contract at the next match (simpler and honest; carrying the term to the heir
is future work). No money, so no escrow and no stranded funds by construction.

### 3.5 Pinned, swept parameters (classify-not-tune)

`share_bps ∈ {2500, 5000, 7500}` (headline **5000**), `SHARE_TERM ∈ {6, 12, 24}` (headline **12**),
both swept with per-cell verdicts reported. There is **no bargaining/price-discovery of `s` in C1R**
(a share-bargaining market is disclosed future work); the two-sided ordinal acceptance at a pinned
swept share is the honest v1, exactly as φ was in S23e.

## 4. Conservation & determinism

- **Goods:** input consumed once, output produced once, split is a conserved transfer; the commons
  rivalry, plot registry, and money/escrow invariants of the base are untouched and stay asserted.
  Provenance + acquisition ledgers stay exact through the origin-preserving transfer (§3.3).
- **Digest (tag 23, ON-only):** `if self.share_tenancy_active() { out.push(23); out.push(flag);
  share_mode_tag; share_bps; SHARE_TERM; next_share_contract_id; length-prefixed ShareContract records
  (id, owner, worker, node, share_bps, term, opened_tick, renewals, cap_at_start); per-colonist share
  task/attribution state (the steering + deposit-attribution marker, §3.2); renewal/expiry state }` —
  everything that steers matching, steering, splitting, renewal. **`N̂` needs no serialized history**
  (spec-review P1 resolved): it is a pure function of already-digested node state (regen, cap) + the
  contract record. Diagnostic-only sets/counters (`share_workers_ever`, `stock_opportunity_refusal`,
  `reservation_collision`, etc.) stay **out** (the C1 tag-22 lesson, :23127–23131).
  Off-path: nothing emitted → byte-identical to the branch goldens; with all of 18/20/21/22/23 off,
  byte-identical to master goldens.
- **Determinism:** matching is sorted-deterministic; `N̂` is a deterministic function of plot state;
  no live RNG; integer-only.

## 5. Slices

- **A — contract + match + admission.** `ShareContract`, the pre-market phase, both ordinal gates, the
  greedy match, `reserved_for` admission + mutual exclusion + debug guard. *DoD: voluntary contracts
  clear on the marginal base; both gates demonstrably decline someone; off-path byte-identical.*
- **B — work + split.** Contract steering in the fast loop; contract labor budget in own-use
  cultivation; the conserved provenance-exact split with pinned rounding. *DoD: split conserves and
  both ledger finalizers stay exact; worker and owner stocks receive exactly `floor(s·N)` / remainder.*
- **C — term/renewal/death + tag 23.** Expiry re-decision + renewals; death routing; the tag-23
  ON-only digest + byte-identity regressions. *DoD: renewals fire from fresh state; death cases clean;
  goldens byte-identical off.*
- **D — acceptance suite** (§6).

## 6. Acceptance suite (`sim/tests/share_tenancy.rs`)

Mirror `wage_labor.rs` (SEEDS, RUN_TICKS=1600, final window, sub-window velocity where relevant,
Metrics/verdict/line printer, goldens pinning, canonical-split test):

- **Predeclared thresholds (swept, printed, never asserted toward success):** `MIN_CONTRACTS`,
  `MIN_RENEWALS`, `θ_SHARE` (share-funded consumption share), `SURVIVAL_LIFT`, plus the
  `share_bps`/`SHARE_TERM` sweeps and the φ sweep {scarce, marginal, abundant} (headline marginal).
- **Scenario modes:** `NoContract` (reproduces the S23e/C1 null — `SubsistenceBoundDespiteScarcity`
  shape, `buyer_bought=0`, no lift), `Voluntary` (headline), `ForcedShare` (contracts imposed →
  `ShareScaffoldOnly`, classified by mode BEFORE vacuity), **`WageComparative`** (the same seed run in
  C1 `wage_labor` Voluntary mode → expect `WageMarketVacuous`, printed side-by-side with the share
  cell — the money-gap demonstration in one table), and a **`LineageWorker` diagnostic cell**
  (spec-review P2: a landless *lineage-household* worker, for whom homesteading IS open — does it
  still choose to sharecrop a good at-cap plot over homesteading the marginal frontier? Reported as a
  diagnostic beside the headline, so the "given the S23d landless exclusion" scoping is probed, not
  just asserted).
- **Interpretability diagnostics (spec-review P2 — so `ShareVacuous` is readable):** per-run counters
  `worker_declined` (bread-acceptance failed), `owner_no_atcap_plot` (no plot met the cap-waste gate),
  `stock_opportunity_refusal` (below-cap plots not offered — the disclosed out-of-scope margin),
  `reservation_collision` (owner rerouted off its reserved plot), `share_stock_drawdown` (any draw
  below cap-at-start — must be 0), `unattributed_share_deposit` (must be 0).
- **Mandatory non-vacuity + discrimination:** ≥ `MIN_CONTRACTS` clear, each traceable to both
  acceptances; at least one eligible worker below its bread-acceptance and at least one owner with no
  underworked plot decline; a real counterfactual — a landless worker alive-and-share-fed where the
  matched `NoContract` run starves it or leaves it commons-bound.
- **Hard guards every run:** `report.conserves()`, commons conservation, money invariant, provenance
  finalizers, plot-registry + S23d owner-identity invariants, **no contracted worker ever acquires
  title** (`non_lineage_owner_plot_ticks==0` still).
- **`goldens_unchanged` + `canonical_bytes_split_only_when_share_tenancy_active`.**

Build/verify: `cargo test -p sim --test share_tenancy -- --nocapture`, full workspace, fmt, clippy
`-D warnings`; the wage_labor + S23c/d/e suites stay green on the branch.

## 7. Anti-smuggling guards

1. **Voluntary both sides, ordinal both sides.** Worker: bread-denominated rank-walk above Leisure.
   Owner: dominance restricted to **at-cap waste** (regen being destroyed; draw bounded to regen) — the
   only regime where something-for-nothing is true; below-cap plots are declined and counted, never
   appraised away. No cardinal object anywhere; `ForcedShare` separates as scaffold by scenario mode.
2. **No advance, no credit, no money requirement.** Nothing is paid before output exists; the split
   happens at realized production. No escrow, no fund, no post-promotion gate.
3. **No title drift.** The contract grants term-scoped *access* (`reserved_for`), never ownership;
   the S23d owner-identity invariants are hard guards.
4. **The outside option is real and disclosed.** The worker gate reuses the S23e commons forecast; the
   structural homesteading exclusion (§1.1) is a disclosed scope condition of the base's
   owner-identity design, not a C1R construction; within-lineage homesteading remains open.
5. **Not tuned.** `share_bps`/`SHARE_TERM`/thresholds pinned + swept; the verdict test prints and
   classifies, never asserts success; `ShareVacuous` and `ShareClearsButNoLift` are first-class.
6. **Provenance honest.** The worker is the producer; the owner's share is a transfer; the emergent
   effect on SALT promotion telemetry is reported, not suppressed or engineered.

## 8. Risks & open questions

1. **`ShareVacuous` is live — but at-cap availability is favorable on this base.** Private-land nodes
   **start at cap** (settlement.rs:5183–5200; spec-review round 2), so any owned plot the owner does
   not work is at cap from tick 0 — the gate can fire immediately, and an emptied-then-abandoned plot
   refills in ~83 ticks (marginal, regen 12/cap 1000) to ~125 (good, regen 64/cap 8000). If the run
   still lands vacuous *for lack of at-cap land*, the `owner_no_atcap_plot` /
   `stock_opportunity_refusal` diagnostics make that a scoped finding about plot dynamics (and the
   below-cap ordinal-comparison extension becomes the named follow-on), distinguished from
   `worker_declined` acceptance failure. **Drawdown-check timing (round-2 polish):** node harvest runs
   *before* regen within a fast tick (world.rs:538–562; node.rs:46–59), so the
   `share_stock_drawdown == 0` guard is evaluated **after the tick's regen**, comparing end-of-tick
   stock against `cap_at_start` — the regen-bounded draw then genuinely never lowers it.
2. **`N̂` estimation.** Resolved by the cap-waste redesign: `N̂` is a pure deterministic function of
   already-digested state (plot regen × term × labor budget — the regen bound makes regen the expected
   flow); no realized-experience history, no optimism knob, nothing extra to digest.
3. **Split rounding.** `floor` to the worker means tiny `N` can zero the worker's share at low `s`;
   the share sweep exposes this rather than a tuned minimum.
4. **Composition with C1 metrics.** Share income must stay out of the C1 money-attribution buckets
   (separate namespaces) so the comparative cells remain independently readable.
5. **Bargaining deferred.** A pinned swept `s` is not price discovery; if `ShareTenancyClears`, an
   endogenous share-bargaining follow-on becomes the natural P1.5 — future work, named now.

## 9. Falsifiable-bar summary

On the exact base where the money-wage market provably cannot open, a voluntary output-share tenancy —
no advance, no money, worker paid out of realized production, owner paid from the **regen its at-cap
plot was wasting** (the only honestly-free gain), both acceptances ordinal, share and term pinned and
swept — should show whether the **no-advance labor contract is the institution that bootstraps
participation**: `ShareTenancyClears` (contracts clear and renew, workers materially share-fed with a
survival lift over `NoContract`, owners gain from cap-waste without durable stock ever drawing down,
title never drifts, everything conserved) versus the honest nulls `ShareVacuous` (the wall is deeper
than the wages-fund gap — or at-cap land is the binding scarcity, distinguishable by the diagnostics)
and `ShareClearsButNoLift` (the lever bites but does not matter) — with the same-seed `WageComparative`
cell printing the money-wage null beside the share result, making the "it was a money gap" claim, or
its refutation, visible in one table.
