# impl-60 — C1N: In-Kind (Bread) Wages — the fixed-advance twin of share tenancy (does the wage contract clear, and does the owner prefer it to a share?)

Status (spec): **SPEC-READY** (Codex xhigh: round 1 NEEDS-REVISION [P0: original grain-conversion mechanism
vacuous — 1 bread/bundle ≤ worker's minimal accept → reframed as the fixed-wage twin of C1R sharecropping
on the cap-waste substrate], round 2 NEEDS-REVISION [P0: escrow-till-release starves the worker → advance
paid up front, dissolving the bread-escrow machinery], round 3 NEEDS-REVISION [P1 precision: complete
C1R-machinery reuse — grain_in_stock, own-use loop gate, self-produced-filtered transfer, unconditional
digest, term-survival hard test], round 4 all RESOLVED, no new P0/P1 → SPEC-READY, polish folded). Codex
round 1 returned NEEDS-REVISION
with a decisive **P0**: the original "convert the owner's held grain" mechanism was vacuous by construction
— on this branch cultivation yields exactly **1 bread per labor bundle** and a hungry worker's minimal
accept is **≥1 bread**, so a single labor act produces exactly its own subsistence and the owner's
productivity gate `Q>W` can never pass; and the base is **grain-flow-bound, never labor-bound**
(mod.rs:121), so "hiring adds labor capacity" is false. This version re-grounds C1N where productivity
actually exists — the **at-cap-plot wasted regen** C1R already exploited, harvested over a **term** — making
C1N the **fixed-wage twin of C1R sharecropping** on the identical substrate. Round 2 then fixed a second
**P0** — the advance must be paid **up front** (the worker gives all product to the owner and is fed only by
the wage, so escrowing it to release-tick starves the worker before it can eat), which *dissolves* the
round-1 bread-escrow machinery: both bread flows become plain conserved transfers, and C1N reduces to C1R's
contract with an up-front advance + a worker share of 0. C1N of the wage/share arc
(replan P2), on the decomposed base **`feat/settlement-decomp` @ `567bc69`**. Flag **`in_kind_wage`** (bool,
composes on `share_tenancy`'s cap-waste substrate — see §1.6), gated `in_kind_wage_active()`. Digest **tag
25** (code-verified free), own ON-only block; every prior golden stays byte-identical.

Falsifiable bar (headline): C1's money-wage market was vacuous (no fund). C1R showed a landless worker will
take an **output share** to cultivate an owner's at-cap plot. C1N asks the wages-fund question in its
sharpest form: will the owner instead advance a **fixed bread wage** out of its real produced-bread fund to
hire the same labor — i.e. does the **advance-based** contract clear where the money wage cleared zero —
and, run beside the share contract on the same seed and plot, **which form does each side prefer**?

## 0. One-paragraph summary

The Austrian wages-fund doctrine: a wage is an advance of present goods out of prior saving, and it is paid
because the labor commanded yields **more** than the advance. C1's money-wage null showed the advance
needs a fund; C1R built the fund (owners hold produced bread) and showed the *share* contract clears. C1N
tests the **fixed-advance** form directly, in kind. The productive roundabout is not "convert the owner's
grain" (labor is ample, so that adds nothing) but the same one C1R found: the owner's plot regenerates to
cap and the **flow is wasted** for want of labor; a worker excluded from adequate self-provisioning (the
commons outside-option fails) cultivates that plot over a fixed term. Under a **share** contract (C1R) the
worker keeps `s·Q` and the owner gets `(1−s)·Q`. Under a **wage** contract (C1N) the owner advances a fixed
`W` bread out of its `SelfProduced` fund, takes the whole term product `Q`, and nets `Q−W`. The owner
advances iff the fund covers `W` **and** the roundabout is productive over the term (`Q>W` — now
achievable because `Q` is the term's regen-bread, not one bundle). The worker's acceptance reuses C1R's
bread-ordinal evaluator + P1.5's term forecast (the wage must cover the worker's whole-term subsistence,
since it is fed only by the advance). No smuggling: the advance is a conserved **transfer** of the owner's
own `SelfProduced` surplus straight to the worker (paid up front, so the worker survives the term); the
worker converts the plot's grain to bread and transfers 100% of it to the owner; both flows are plain
conserved transfers with no escrow; endowment/inherited/bought/minted-funded wages are disqualified.
The suite runs C1N **beside** the same-seed C1R share cell and the C1 money-wage null — the three contract
outcomes in one table. Pre-named verdicts, classify-not-tune, honest nulls first-class.

## 1. Base facts (verified on the decomposed branch)

1. **The P0's root — labor produces no per-bundle surplus.** `Cultivate` outputs exactly 1 bread per
   contract labor bundle (content.rs:108/124/397), released as one `output_qty` by the wage path
   (wage_labor.rs:42/454). `share_worker_accepts_bread_now` flips no want at `W=0` (share_tenancy.rs:344),
   so `W_min≥1`. Per **bundle**, `Q=1 ≤ W_min` — no surplus. The surplus is only a **term** phenomenon.
2. **The base is grain-flow-bound, not labor-bound** (mod.rs:121): own-use cultivation converts free grain
   up to a generous budget and carries pending stock forward (mod.rs:14850). So hiring to *convert* owned
   grain adds nothing; the productive scarce factor is **grain harvested from a plot**, and the wasted
   at-cap regen is the only source of added product — exactly C1R's `share_owner_candidate_plots`
   cap-waste gate (share_tenancy.rs, `plot.stock==cap && regen>0`, not owner-targeted).
3. **The term product `Q` is real and exceeds subsistence.** C1R's own trace: a contracted worker produced
   `worker_income_total` ≈ 2× `worker_consumed` (e.g. 12,543 vs 6,719) — labor over a term yields roughly
   twice its subsistence. So `Q` (the term's regen-bread) `> W` (term subsistence) is achievable; C1R's
   `share_expected_term_output(node, term)` (the regen×fast-ticks×term/budget bound, "N̂") is exactly the
   digested `Q` the owner gate needs.
4. **The worker side is reused** — `share_worker_accepts_bread_now(worker, bread, W)` (share_tenancy.rs:327),
   `pub(super)` and callable from a sibling submodule (both descendants of `settlement`), parameterized
   only by a bread qty; monotone in `W` (more bread never un-provisions a want), so `W_min` is a clean
   smallest-accept search. The **outside-option** gate (`share_worker_outside_option_fails` /
   `forecast_commons_sufficiency`, now in `rival_commons.rs`/`share_tenancy.rs`) makes the contract
   non-trivial: the worker takes a wage only when its own commons provisioning fails — else it self-harvests.
5. **The C1R contract machinery is reused (NOT the C1 wage escrow).** C1N is C1R's plot contract with
   worker-share 0 + an up-front advance, so it reuses the `ShareContract`-shaped record, the cap-waste
   candidacy, the convert-then-split hook in `run_own_use_cultivation` (mod.rs:14832/14893), the harvest
   steering + admission, the carry/deposit attribution, and the death dissolution — **all the `share_*`
   methods, given `in_kind_*` twins** (§6). The C1 money-wage escrow lifecycle (`release_wage_escrow_inner`,
   `release_due_wage_escrows`) and the money-fungibility apparatus (retained-earnings, FIFO proceeds) are
   **not** used — there is no escrow (the wage is paid up front) and no money.
6. **Composition + gating.** C1N runs on the C1R substrate: `frontier_mortal_landowner_demography` + rival
   commons + `share_tenancy` **on** (it needs the cap-waste plots and the outside-option gate). The flag
   `in_kind_wage` selects, **per candidate plot**, the wage form instead of the share form (see §3.2 —
   they do not double-contract the same plot). Gate: `in_kind_wage_active() = in_kind_wage &&
   share_tenancy_active()`. Cross-exclusion: a worker holding an in-kind contract must be barred from a
   share contract and a money-wage escrow, and vice-versa — the eligibility helpers
   (`wage_worker_available_labor` wage_labor.rs:111, `share_worker_base_eligible` share_tenancy.rs:267)
   **must be extended to see the `InKindWageContract`** (Codex round-1 P1 #4).
7. **Tag 25 free**; `share_forward_provisioning`/tag-24 is the plumbing template; the death seams are
   `settle_wage_labor_for_death` (both paths via `collect_estate`) + the old-age `age_and_remove_elderly`
   (mod.rs:13548 — the P1.5 bug seam) + a horizon flush.

## 2. The central question and pre-named outcomes

**Central question.** On the C1R cap-waste substrate at the marginal commons, across `SEEDS=[3,7,11,19,23]`,
when an owner may hire a worker to cultivate its at-cap plot under a **fixed bread-wage advance** `W` drawn
from its `SelfProduced` fund (taking the whole term product `Q`) instead of an output share: do voluntary
in-kind **wage** contracts clear (real hires, real fund drawdown, conserved) where the same-seed money wage
clears zero; and run beside the same-seed **share** cell, **which contract form does each side realize more
from**?

**Ordered verdict enum** (first-match):

```
Preconditions (disqualifying):
  BaseUnviable        — the substrate fails to reproduce (the money comparative must land WageMarketVacuous;
                        the share comparative must land its C1R verdict; owners hold a positive SelfProduced fund)
  ConservationBroken  — goods / commons / money conservation or the bread-provenance identity failed a tick
  RegistryBroken      — plot-registry / S23d owner-identity / no-double-contract (a plot with both forms) invariants
  FundIsScaffold      — endowment/minted-funded wages > 0 (the advance was not real prior saving)
Outcome ladder:
  InKindWageVacuous     — no wage contract clears (final in-kind hires < MIN_HIRES): fund and/or Q>W gate never opens
  InKindWageClears      — voluntary fixed-wage cultivation contracts clear, owner fund drawn down, all
                          conserved — the wages-fund advance confirmed in kind, where money wages clear zero
  InKindWageClearsAndLifts — clears AND a material survival lift over the same-seed no-contract control
```

Plus a **reported (non-verdict) contract-choice comparison** on matched plots/seeds: owner realized
`Q−W` (wage) vs `(1−s)·Q` (share); worker realized `W` (wage) vs `s·Q` (share) — which side prefers which
form, reported, never asserted (the Cheung question, observed not predicted).

Every rung is first-class. `InKindWageVacuous` is a real result (the fixed advance cannot compete with the
share form, or the fund/productivity gate never opens); `InKindWageClears` is the doctrine's positive.

## 3. Mechanism

**Framing (Codex round 2):** structurally C1N is **C1R's contract with two changes** — an **up-front
advance** paid to the worker at open, and a **worker share of 0** (100% of the term product goes to the
owner). It therefore reuses C1R's whole plot-contract apparatus (cap-waste candidacy, plot reservation,
harvest steering, harvest admission, the convert-then-transfer split, death dissolution, reservation-
collision guards), specialized to `share_bps_worker = 0` plus the advance. This is why the decomposition
matters: the machinery is the isolated `share_tenancy.rs` seam, extended, not re-derived. There is **no
bread escrow** — the wage is a plain up-front transfer, so the round-1 `whole_system_total`/provenance-
escrow-bucket complexity **dissolves**; both bread flows are conserved by the existing transfer machinery.

### 3.1 Worker side — the advance must cover the term (reused P1.5 forecast)
Labor-eligible by the extended gate (§1.6). Because the worker gives **all** its product to the owner and
is fed **only** by the advance, the wage must cover the worker's whole-term subsistence or it starves
mid-term (Codex round-2 P0). So `W_min = forecast_term_need_unmet(worker, bread, term)` (share_tenancy.rs:876)
— P1.5's pure term-horizon need forecast, the bread the worker needs to survive the term net of held stock
and the commons. The worker accepts iff the outside option fails (`!forecast_commons_sufficiency`) **and**
`W ≥ W_min` (`share_worker_accepts_bread_now` confirms the advance ordinally provisions the hunger want
above Leisure). Monotone in `W`, so `W_min` is a clean floor. **Survival is not *guaranteed* by the wage**
(Codex round-3 P1): the advance is ordinary stock, and only Now/Next wants are protected from market asks
(agent.rs:288) — a whole-term reserve is not. So term survival of the contracted worker is a **hard test
invariant** the suite asserts (a contracted worker must not starve before term end at `Q>W`), not a
mechanism guarantee; if it fails, that is a reportable finding about the advance's adequacy, not a silent leak.

### 3.2 Owner side (the binding margin)
Candidate plots are C1R's cap-waste set (`share_owner_candidate_plots` — at cap, regen>0, live owner, not
owner-targeted, not already under a share/wage contract). For each candidate the owner advances iff:
- **Fund gate:** `owner_free_self_produced_bread(owner) ≥ W` — the wage draws only the owner's **own**
  `SelfProduced` free lots (`ProducedLot.producer == owner` ∩ free stock, §4.1), not produced-origin bread
  it bought or inherited (Codex round-2 P2 #6) — real prior saving.
- **Productivity gate:** `Q > W`, where `Q = share_expected_term_output(node, term)` (C1R's N̂, the regen-
  bounded **term** product — Q is the term's bread, not one bundle; this is the round-1 P0 fix).
`W = W_min`. **No plot carries both a wage and a share contract**: the in-kind phase runs **before** the
C1R share match and reserves the plot, so `share_owner_candidate_plots` skips it (a guard symmetric to
`share_plot_reserved_against_owner`); the registry invariant enforces it as a hard guard. (Disclosed: the
headline runs the wage form; the same-seed *share* comparative is a **separate cell** with `in_kind_wage`
off, so wage-vs-share is compared across matched cells, not a within-run race.)

### 3.3 The advance and the product — two plain conserved transfers (no escrow)
A first-class in-kind contract record `Vec<InKindWageContract>{ id, employer, worker, node, wage_bread,
term, opened_tick, grain_in_stock, split_remainder_bps }` on the `Settlement` struct — **the full
`ShareContract` shape** (Codex round-3 P1: `grain_in_stock` is load-bearing — C1R's split uses it to
identify contract-sourced grain, share_tenancy.rs:816), digested ON-only under tag 25. Bread never sits in
limbo:
- **Open (advance, up front):** owner `−W` → worker `+W` by a direct conserved transfer that draws
  **specifically the owner's own `SelfProduced` lots** — a new `bread_provenance.transfer_self_produced(owner,
  worker, W)` filtered on `producer == owner` (Codex round-3 P1: the existing `transfer` draws produced lots
  FIFO, mod.rs:7563, which could move bought/inherited produced bread even when the fund gate passed), plus
  `debit_stock`/`credit_stock` and `acquisition.transfer_preserve`. Booked in neither `report.produced` nor
  `report.consumed` (a relocation of already-produced bread — the `split_share_output` template, reversed).
  The worker can now eat it; the plot is reserved `reserved_for = worker` (C1R admission).
- **Harvest+convert (the term):** the worker cultivates the reserved plot; grain→bread converts to the
  **worker's own** stock via the existing own-use path (`run_own_use_cultivation`, mod.rs:14860 — the
  cultivator is credited, not the owner directly — Codex round-2 P1). **The own-use loop gate must admit the
  in-kind worker** (Codex round-3 P1/finding C): today the loop enters on `cultivating || stock_pending ||
  share_contract.is_some()` (mod.rs:14832) — extend it to `|| in_kind_contract.is_some()`, and the
  split hook (mod.rs:14893) to call the in-kind split, or a wage worker (share 0) is never processed. Then,
  **before the worker's own-use consume**, the contract transfers **100%** of that batch to the owner
  (`split_share_output` shape with worker share 0: `debit_stock(worker)`, `credit_stock(owner)`,
  `bread_provenance.transfer(worker,owner)`, `acquisition.transfer_preserve`; the cumulative-exact floor in
  `split_remainder_bps` carries the sub-unit remainder, and `grain_in_stock` scopes the split to
  contract-sourced grain exactly as C1R). So the worker nets **only** the advance `W` (which it eats); the
  owner nets the term product `Q`.
- **No double-count:** the advance is a transfer of pre-existing bread; each converted batch is genuine
  production (`credit_produced(worker)` at conversion, as today) then transferred whole to the owner —
  produced once, relocated twice, every unit in a live agent's stock or the commons at all times, so the
  per-tick `report.conserves()` and the `bread_provenance` produced-identity hold with **no new bucket**.

### 3.4 Death & horizon
The contract dissolves at both death seams exactly as C1R's does (`settle_share_tenancy_for_death` is the
template; hooked at the starvation `settle_death` and the old-age `age_and_remove_elderly` — the P1.5 bug
seam) and at the horizon: any un-transferred converted product settles to the owner, the reservation
clears, the contract drops. There is **nothing to refund** — the advance was paid up front and is the
worker's property; if the worker dies mid-term the owner has lost that advance and keeps only the product
transferred so far (the owner's real risk in advancing, disclosed and conserved). No escrow balance to flush.

### 3.5 Dropped
Retained-earnings + FIFO wage-proceeds (wage_labor.rs:321–358,575–724) — money-fungibility only, irrelevant
to an in-kind contract. C1N's death path is C1R's (`settle_share_tenancy_for_death`-shaped), keyed on the
in-kind contract record, **not** `settle_wage_labor_for_death` or the gold retained maps — so no dependence
on money-wage state (Codex round-1 P1 #5, now moot since there is no bread escrow).

## 4. Anti-smuggling guards
1. **Real prior saving** (`FundIsScaffold` tripwire): the advance must draw the owner's **own**
   `SelfProduced` free bread — an exact helper `owner_free_self_produced_bread(owner)` over the
   `ProducedLot`s with `producer == owner` (mod.rs:7415) ∩ free stock, **not** the flat holder-produced
   balance `BreadProvenance.produced[owner]` (which includes produced-origin bread the owner bought or
   inherited — Codex round-2 P2 #6). An endowment/inherited/bought/minted-funded advance is counted
   (`in_kind_endowment_funded_hires`) and disqualifies to `FundIsScaffold`.
2. **Both bread flows are conserved transfers, never new production** (§3.3): advance owner→worker and
   product worker→owner both via `bread_provenance.transfer`; the only `credit_produced` is the worker's
   own conversion (as today). No new provenance bucket, no `whole_system_total` arm — bread is always in a
   live agent's stock or the commons.
3. **No owner cardinal appraisal**: the gate is the fund inequality + `Q>W`, both integer, both digested.
4. **Worker accept is the existing ordinal evaluator + P1.5 term forecast + outside-option** (all reused);
   no new want kind.
5. **Not tuned**: `W=W_min` (the term-need forecast); `Q` is the pinned N̂; no free wage parameter.
   `InKindWageVacuous`, `FundIsScaffold` first-class. Swept: φ, `share_term`, a Subsidised-advance control.

## 5. Conservation & determinism
No new sources/sinks and **no escrow**: the advance is a transfer, the product is a transfer of the
worker's own conversion, and every bread unit is in a live agent's stock or the commons at all times — so
the existing per-tick `report.conserves()` and the `bread_provenance` produced-identity hold **unchanged**,
with no `whole_system_total` arm and no new provenance bucket (the round-1 escrow machinery is dissolved by
paying the wage up front). Money invariant untouched (no gold moves). Integer, deterministic. **Digest:**
tag 25 = ON-only `{ push(25); flag byte; next_in_kind_contract_id; the InKindWageContract records —
id/employer/worker/node/wage_bread/term/opened_tick/grain_in_stock/split_remainder_bps; and the
per-colonist carried-in-kind-contract-id }`, digested exactly as the C1R `ShareContract` block under tag 23
— **`grain_in_stock` and the carried-contract-id are steering state, digested unconditionally, not
diagnostics** (Codex round-3 P1 D: C1R proves both steer future splits). `*_ever` sets and hire counters
stay **out** of `canonical_bytes`. Off-path: byte-identical to the decomposed branch goldens (and, all civ
flags off, to master).

## 6. Slices
- **A — plumbing + honest advance.** Flag (7-site template), the `InKindWageContract` record + state,
  tag 25, the `in_kind_wage.rs` submodule (added to the g6a source-gate), and the **first-class in-kind
  contract steering/admission** paralleling C1R (Codex round-2 P1): `in_kind_worker_has_contract`,
  `in_kind_contract_task` (steer the worker to harvest the reserved plot), carry/deposit attribution,
  `in_kind_worker_admitted_to` (admit the worker to the owner's reserved plot), reservation-collision
  guards, and the extension of `wage_worker_available_labor` + `share_worker_base_eligible` so the in-kind
  contract is seen everywhere a share contract is. The up-front advance transfer with the
  `owner_free_self_produced_bread` gate. *DoD: flag-off byte-identical; tag-25 split test; a single advance
  + a single product transfer both conserve (goods + provenance) with no new bucket.*
- **B — the hire loop.** Owner cap-waste + two-gate (fund + `Q>W`) decision, no-double-contract guard
  (in-kind phase before share match, reserves the plot), worker term-need accept + outside-option,
  harvest→worker-converts→100%-transfer-to-owner, and the C1R-shaped death + horizon dissolution.
  *DoD: in-kind hires clear; the worker survives the term on the advance; conservation + money invariants
  hold over a full run; no plot double-contracts.*
- **C — the suite.** `sim/tests/in_kind_wage.rs`: the `InKindWage` headline cell, the same-seed **share**
  comparative (C1R) and **money-wage** comparative (`WageMarketVacuous`), the §2 verdict ladder, the
  contract-choice comparison, the φ + Subsidised sweeps, the fund-provenance guard, the survival-lift
  re-test. *DoD: suite green; comparatives reproduce C1R and C1's verdicts; verdicts printed, never asserted.*

## 7. Acceptance suite (`sim/tests/in_kind_wage.rs`, new)
- **Predeclared thresholds (swept):** `MIN_HIRES`, `MIN_SURVIVAL_LIFT`, the φ band. No wage-magnitude
  threshold — `W=W_min`, reported.
- **Cells:** `InKindWage` (headline, φ=marginal); same-seed **share** comparative (`in_kind_wage=false,
  share_tenancy=Voluntary` → its C1R verdict) and **money-wage** comparative (`wage_labor=true,
  in_kind_wage=false, share off` → `WageMarketVacuous`); `NoContract` control (lift baseline); φ sweep;
  `SubsidisedInKind` scaffold control.
- **Classifier, NOT asserted (Codex P2 #7):** `final_in_kind_hires < MIN_HIRES` **routes to**
  `InKindWageVacuous` (never an assertion that ≥1 hire occurred); the non-vacuity *evidence* (a hire whose
  wage traces to owner cultivation surplus and whose same-seed money cell cleared zero) is a **reported
  trace** qualifying `InKindWageClears`.
- **Hard guards (invariants only):** conservation (goods + the `bread_provenance` produced-identity — both
  unchanged, no new bucket), money invariant, registry/owner-identity, no-double-contract (a plot never
  carries both a wage and a share contract), no double-count (the advance and the product are transfers,
  neither a fresh `credit_produced`), `in_kind_endowment_funded_hires==0` (else `FundIsScaffold`), and
  **term-survival** — measured at the starvation death seam, *before* the contract dissolves: fail if the
  dying worker holds a live in-kind contract with `econ_tick < opened_tick + term` and `Q > W` (§3.1).
- **`goldens_unchanged` + the tag-25 canonical-split test.**

Build/verify: `cargo test -p sim --test in_kind_wage -- --nocapture`, full workspace, fmt, clippy
`-D warnings`; wage_labor + share_tenancy + S23c/d/e suites stay green; every prior digest unchanged.

## 8. Risks & open questions
1. **`InKindWageVacuous` live** — if, for every candidate plot, `Q ≤ W_min` (the term product barely
   exceeds one worker's subsistence at this regen/term), the fixed advance never pays and the owner keeps
   the share form. Then the finding is that the **fixed-wage form cannot compete with the share form** on
   this base — a sharp Cheung-style result, readable via the two-gate decline counters + the contract-
   choice comparison.
2. **Fund depletion** — advancing bread up front (before the product returns) draws the owner's surplus;
   the final-window baselining captures whether hiring persists or is a transient of the initial fund
   (a `ClearsButNotSustained` shading, reported per window).
3. **Double-count** — the largest correctness risk; §3.3 pins advance=transfer, product=transfer of the
   worker's own single conversion, with the per-tick `report.conserves()` + the `bread_provenance`
   produced-identity as the net (no escrow, no new bucket).
4. **No-double-contract** — a plot must never carry both a share and a wage contract; the symmetric
   reservation guard + the registry invariant enforce it (a hard guard).
5. **Term-survival adequacy** — the advance is ordinary stock, not a protected reserve (§3.1); if a
   contracted worker starves before term end despite `Q>W`, the hard test flags it — a reportable finding
   about advance adequacy, not a conservation leak.

## 9. Falsifiable-bar summary
Grounding the advance where productivity actually exists — the owner's wasted at-cap regen, over a term —
turns C1N into the **fixed-wage twin of C1R sharecropping**: the owner advances a fixed bread wage `W` out
of real `SelfProduced` saving and takes the term product `Q`, hiring iff the fund covers `W` and the
roundabout is productive (`Q>W`). The milestone shows whether that advance **clears** — voluntary in-kind
wage contracts drawing down a real fund where money wages cleared zero (`InKindWageClears`, `…AndLifts` if
it raises survival) — with the same-seed share and money cells printed beside it and the owner/worker
contract-choice reported; or the honest alternatives — `InKindWageVacuous` (the fixed advance cannot
compete with the share form, or the productivity/fund gate never opens) and `FundIsScaffold` (the fund was
not real saving) — each named before the run, each first-class.
