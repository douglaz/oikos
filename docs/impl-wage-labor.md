# impl-48 — C1: Wage Labor and the Circular Flow of Income (does buyer income finally make the market live?)

Status (spec): **SPEC-READY** (independent second-model spec-review: round 1 high + rounds 2–3 xhigh folded in;
a final whole-document xhigh read added three cross-slice P1 fixes — own-funded wage appraisal §4.3, retained-earnings
ledger digested + death-routed §4.6/§5, verdict scaffold-before-vacuity ordering §2). Opens the CIVILIZATION-CORE arc
(`docs/spec-civ-core-roadmap.md`, layer C1). This is the **keystone** milestone: every layer above it
(firms, unified households, factor markets, the state, the credit cycle) depends on a demand side that
can *earn*.

Base: master `b7e6b0f`, composed with **S23d** (`mortal_landowner_demography`, tag 20) + **S23e**
(`rival_subsistence_commons`, tag 21) rebased forward from `feat/rival-commons-impl-rb` @ `e592854`
(see §3 for the sequencing decision). Flag `wage_labor`, digest **tag 22**, ON-only. Composes the
existing labor-market and ordinal-reservation machinery (`econ/src/factor.rs`) into the settlement
loop; adds a conserved wage **escrow** for the two-rate boundary.

Falsifiable bar (headline): on the S23e **marginal** commons (φ = 0.5) — the cell where money
promotes and owners hold sellable surplus but S23e measured `buyer_bought = 0` — does adding wage
labor turn `SubsistenceBoundDespiteScarcity` into a **sustained producer/buyer money market**
(`CircularFlowForms`), and is it genuinely wage-driven (not a fiat or subsidised scaffold)?

---

## 0. One-paragraph summary

The S23c→d→e strand proved the whole living-economy frontier is blocked on one thing: the non-owner
demand side has **no sustained purchasing power** — a one-time endowment that depletes, with no wage,
rent, or payment stream to renew it (`report-emergence-and-its-limits.md` §21). S23e isolated it
cleanly at the **marginal** commons: money promotes, owners produce and sell a surplus, the commons
is scarce — yet hungry non-owners buy **nothing** and simply go hungry. C1 adds the missing channel
in the most faithful, least-fiat form: a **voluntary wage-labor relation**. An owner of a plot or
tool with unmet output demand hires a hungry non-owner for a money wage, advanced from the owner's
*own* prior earnings; the worker's labor produces output the owner sells, and the wage is the
worker's recurring income, which he spends buying output — closing the circular flow. The wage is a
**price imputed** from the output's forecast value (Menger, reusing the S2 input-bid machinery); the
worker's acceptance is **ordinal** (the existing `reservation_labor_ask_...` — the least wage that
provisions a future-money want ranked above Leisure). C1 **reuses** the ordinal wage-ask, the
`LaborBook` matching, and `apply_labor_trade`'s conserved payment — but it is **not** merely wiring:
spec-review established five pieces of genuinely new code — (i) an explicit scarce-outside-option gate
(the scale does not read the commons), (ii) a new `appraise_labor_hire_for_money` adapter (labor is a
flow, not a stocked input), (iii) a worksite / labor-capacity / project bridge (chain agents have
`labor_capacity: 0`), (iv) explicit conserved escrow primitives (the generic money helper is not
escrow-safe), and (v) a wage-eligible retained-earnings ledger (a balance cap cannot tell earnings from
endowment). Classify-not-tune; the same conservation/digest/anti-fiat spine; the central trap is a
**subsidised wage** that would silently re-create the S23d scaffold, guarded by a separated control and
the provenance ledger.

---

## 1. Why this milestone, why this lever

The roadmap's dependency analysis (`spec-civ-core-roadmap.md` §2) is unambiguous: income in a market
economy is the proceeds of selling a factor service (labor, land-use, capital) plus entrepreneurial
profit. A colonist who owns neither land nor capital has exactly one factor to sell — **his labor** —
and the institution that lets him sell it is the **wage relation**. This is the natural next lever
after S22f (the one lever in the entire occupation arc that stabilized anything was a *voluntary
binding institution* the agent opted into under its own realized signal): a wage contract is a
voluntary binding exchange of labor for money, now with a paying counterparty.

Why it must come first: a state cannot tax incomes that do not exist (C5); classes cannot form
without factor incomes (C9); a credit cycle needs a wage structure to distort (C7). C1 is the gate.

Why it can succeed where S23e failed: S23e added scarcity but no income, so scarcity merely *starved*
buyers. C1 adds income on top of that scarcity. The scarcity is **necessary** — on an unlimited free
self-provision floor (S23d) a worker has no reason to accept any wage, because he can feed himself
for free; only when self-provision is **rivalrous and costly** (the S23e finite commons) does selling
labor for money to buy food become the ordinally-preferred path. That is exactly why C1 composes on
the S23e base rather than the S23d floor.

---

## 2. The central question and pre-named outcomes

**Central question.** On the S23e marginal-commons base (φ = 0.5: money promotes, owners hold sellable
surplus, `buyer_bought = 0` in S23e), when hungry non-owners may **voluntarily sell labor for a money
wage** to owners who advance that wage from their **own prior earnings**, does a **sustained
producer/buyer money market** form — non-owners earn wages and use them to buy owner output across the
whole final window — while money promotes on `SelfProduced` bread, mortality runs, provenance is
clean, and conservation (including the new escrow) holds every tick — **and** is it genuinely
wage-driven (not a fiat re-pin and not a subsidised-wage scaffold)?

**Primary success = `CircularFlowForms`** (by classified metrics over `SEEDS = [3, 7, 11, 19, 23]`):

1. **Non-vacuity — wages actually clear.** ≥ `MIN_HIRES` voluntary labor trades clear per run *after
   money promotes*, each traceable to a matched worker-ask/employer-bid; and at least some eligible
   hungry non-owners have a below-ask signal and are **not** hired (the labor decision is real).
2. **Sustained, wage-financed demand** (the strengthened bar; not a token late trade):
   - `wage_financed_buy_share ≥ θ_SHARE` — a material fraction of non-owner purchases in the final
     window is financed by wage income (measured: money a non-owner received as wages this window and
     then spent on output), not by the depleting one-time endowment.
   - `final_window_velocity ≥ V_MIN` sustained across the **whole** final window (not one spike) —
     money keeps circulating wage→buy→revenue→wage.
   - `circular_loop_turnovers ≥ K` — the wage→buy→revenue→wage loop completes ≥ K times over the run.
3. **The base preconditions hold** (inherited, must not regress): SALT promotes on `SelfProduced`
   bread; owners produce a real surplus (`produced_minus_consumed > 0`); mortality + inheritance fire
   (S23d: `inherit_eligible_owner_deaths ≥ N`); provenance clean; conservation + `bread_minted_max ==
   0` + the new **escrow conservation** hold every tick.
4. **Genuinely wage-driven (anti-scaffold controls all separate):**
   - `no_wage_offered` control (labor market off) reproduces the S23e `SubsistenceBoundDespiteScarcity`
     null (`buyer_bought ≈ 0`).
   - `fiat_wage` control (employment forced regardless of the ordinal ask/bid) classifies
     `WageScaffoldOnly`, never headline success.
   - `subsidised_wage` control (wage fund **injected** per tick rather than paid from the owner's own
     prior earnings) classifies `WageScaffoldOnly` — this is the central trap (a subsidised wage is
     the S23d subsidy in disguise).

**Ordered, mutually-exclusive verdict enum** (top-down, mirrors the S22f/S23e classifier shape):

```
BasePrecondition failures (checked first, disqualifying):
  BaseUnviable            — SALT never promotes / owners hold no surplus / mortality doesn't fire
                            (the S23e base itself didn't reproduce → C1 is untestable this seed)
  ConservationBroken      — whole-system or escrow/money conservation failed a tick
  EscrowUnbalanced        — escrow not fully released+refunded by horizon (funds stranded/lost)
  ProvenanceDisqualified  — bread provenance unclean / bread_minted_max > 0
Scaffold mode (checked by SCENARIO FLAG, before vacuity — spec-review round 4, P1):
  WageScaffoldOnly        — the run IS a fiat_wage or subsidised_wage control (employment forced, or
                            wage fund injected rather than paid from own earnings). Determined by the
                            scenario mode, NOT by counting hires — a forced/subsidised run with few
                            or zero voluntary retained-earnings hires must still classify here, never
                            as vacuous.
Signal / vacuity (HEADLINE voluntary run only):
  WageMarketVacuous       — in the voluntary headline scenario, < MIN_HIRES own-earnings-funded hires
                            after promotion (labor market inert). Scoped to the headline run so it
                            cannot preempt a scaffold control.
Outcome (the real question):
  CircularFlowForms       — success clauses 1–4 all hold (headline positive)
  WageInertDemandStillDead— voluntary wages clear from own earnings, yet demand stays dead
                            (buyer_bought ≈ 0 / velocity ≈ 0): a NEW, sharper null than S23e —
                            "even wage income does not make the market live, because <reported reason>"
```

**Ordering note (round 4, P1):** scaffold classification is keyed on the **scenario mode** (which
control is running) and is checked **before** voluntary-vacuity; `WageMarketVacuous` is scoped to the
headline voluntary run. This prevents a `fiat_wage`/`subsidised_wage` control — which may legitimately
have zero *voluntary retained-earnings* hires — from misclassifying as `WageMarketVacuous` and defeating
its own purpose.

`CircularFlowForms` would be the first *positive* of the entire generational strand and the event that
**reopens the deferred S23c generational-tenure study** (now a viable C3). `WageInertDemandStillDead`
would be an honest, sharper negative that names the *next* missing condition.

---

## 3. The base: what C1 composes on, and the sequencing decision

C1 needs a base where (a) money promotes, (b) owners hold a sellable surplus, and (c) self-provision
is **rivalrous and costly** so a wage can outrank it. That base is **S23e's marginal rival commons on
the S23d mortal-landowner demography** — exactly the cell S23e classified
`SubsistenceBoundDespiteScarcity`, whose *only* missing ingredient (result-reviewed, code-verified) was
buyer income.

Both are unmerged branch mechanisms (S23d tag 20 `feat/mortal-landowner-impl-rb` @ `d965d35`; S23e tag
21 `feat/rival-commons-impl-rb` @ `e592854`, which itself composes on S23d). They are additive,
ON-only, conserved, and verified on their branches — but master has moved to `1029223` since they
were cut.

**Decision (recommended): rebase-forward-then-compose.** Build C1 on a fresh branch that first rebases
the S23d + S23e commits onto current master (mechanical: additive `push(20)`/`push(21)` digest blocks,
new gated phases, no overlap with anything merged after them), verifies the composed base still
conserves and reproduces the S23e verdicts, then adds `wage_labor` (tag 22) on top. The composed stack
(tags 20 + 21 + 22) **merges to master as a unit iff C1 achieves `CircularFlowForms`**; if C1 is a
null, the whole composed branch is preserved like its S23c/d/e predecessors. Rationale: S23d and S23e
were nulls *standing alone*; C1 is the milestone that makes them **load-bearing infrastructure**, so
they land exactly when (and if) they earn their place — never as unmotivated null-mechanisms on
master.

Rejected alternatives: (a) *merge S23d/S23e to master first, independently* — puts unmotivated nulls
on master and violates the "nulls stay branch-preserved" discipline; (b) *build C1 on bare master with
an ad-hoc scarcity substitute* — re-litigates S23e and confounds the finding (was it the wage, or the
new scarcity knob?). The rebase-forward path keeps the S23e scarcity **exactly** as verified, so the
only new variable is the wage.

**Base-precondition gate.** Slice A's first job is to reproduce the S23e marginal-cell verdict
(`SubsistenceBoundDespiteScarcity`, `buyer_bought ≈ 0`) with `wage_labor` OFF on the rebased base. If
the rebase perturbs it, C1 is not yet testable — this is the `BaseUnviable` verdict and a
stop-and-fix, not a tune.

---

## 4. Mechanism — the labor market as a settlement phase

### 4.1 Insertion point and phase order

Insert a new gated phase **between phase 4e (`set_project_input_bid_overrides()`, settlement.rs:9478)
and phase 5 (the goods market `Society::step`/`step_v2`, ~9480)**. This ordering is load-bearing:

- **After** the subsistence floors (`run_own_labor_subsistence` ~9455, and `run_emergency_self_provision`
  runs later at ~9635) so the labor decision is made against the *scarce-commons* alternative, and
  after role-choice (~9399) so adopted owners are in place to post hire orders.
- **Before** the goods market so wage income is in hand when the worker bids for output the same tick.

The phase runs only when `wage_labor_active()` (gated). Off-path: byte-identical (no phase, no digest).

### 4.2 The worker's wage ask, and the explicit scarce-outside-option gate

The wage **floor** is a real ordinal quantity that already exists:
`reservation_labor_ask_from_claims_for_money()` (factor.rs:539–600) returns the least wage that newly
provisions a money want ranked **above** the worker's first unsatisfied `Leisure` want
(`labor_wage_candidates()` :732–758; `wage_needed_for_money_rank()` :760–783;
`money_required_through_rank()` :785–796), preserving higher-ranked wants. C1 reuses it unchanged as
the **ask price**, and the worker posts a `LaborOrder` (Work side, factor.rs:32–39) at it.

**Correction to an earlier draft (spec-review P0).** The *decision to supply labor at all* does **not**
fall out of the value scale: `regenerate_scale()` reads only needs/culture/known goods (scale.rs:160),
and `regenerate_scales()` passes only that state (settlement.rs:12344) — the finite-commons stock is
**not** in the scale. So the scarce outside option must enter through an **explicit ordinal gate in
the labor phase**, not implicitly through the scale. C1 adds:

- **The outside-option gate (pre-market forecast).** A hungry non-owner posts a Work ask only when
  self-provision from the finite rival commons is forecast **insufficient this tick**. *Temporal
  correction (spec-review round 2, P0):* S23e's realized commons draw runs **after** market/production
  (branch `e592854` settlement.rs ~10018 / ~14606–14717), while the labor phase is **pre-market**, so
  it cannot read the realized draw — "the same draw this tick" would be circular. Instead the gate runs
  a deterministic **pre-market dry-run**, `forecast_commons_sufficiency(worker)`, applying S23e's
  rationing math to the *current* `subsistence_commons_stock`/regen and the count of currently-eligible
  claimants, to estimate whether the commons will pull this worker's hunger to target. Ordinal reading:
  if the forecast says yes, the worker prefers free commons food (self-provision outranks the wage
  detour) and does **not** sell labor; if the forecast says the commons is depleted/rationed below its
  need, its next-best hunger relief is a wage → it posts the Work ask. The realized S23e draw still
  happens later in its own phase; the labor decision uses the forecast, and any forecast error is honest
  expectational error (S11-like), not a hidden knob. The dry-run **reuses S23e's rationing math on
  pre-market state** — no new scarcity parameter.

### 4.3 The employer's wage bid — a new labor-hire appraisal adapter (genuinely new code)

**Correction to an earlier draft (spec-review P0): this is NOT a free reuse of the input-bid path.**
`imputed_input_reservation()` (settlement.rs:22657–22717) prices a *stocked recipe input* and divides
by `input_good` quantity — it does not apply to labor (a flow); and `appraise_project_bundle_for_money()`
(bundle.rs:97) *carries* a `required_labor` field (bundle.rs:50) but does **not** use it in the
acceptance test. So C1 must **define a new adapter — `appraise_labor_hire_for_money`** — modeled on the
bundle appraisal but pricing labor:

- Forecast the output the hired labor will produce with `forecast_output_price()` (settlement.rs:21312–21327),
  capped at the observed realized price by `project_input_bid_limit()` (:22620) so the forecast cannot
  inflate the offer.
- **Own-funded, not financed (spec-review round 4, P1).** Do **not** reuse
  `appraise_project_bundle_for_money()` directly: that path leaves the owner's current `gold` unchanged
  and models the advance as a future *due/payable* (bundle.rs:109) — i.e. a **financed** project, which
  would smuggle credit into C1 (credit is C7) and could approve a hire without testing the real
  present-money sacrifice. `appraise_labor_hire_for_money` must be a **true own-money adapter**: the wage
  is paid **now** out of the owner's wage-eligible retained earnings (§4.6), so the appraisal compares
  the owner's ordinal provisioning at **current gold** against provisioning at **(gold − wage_now)** plus
  the *expected future sale proceeds* of the output — no synthetic payable, no fake credit.
- Acceptance (ordinal): hire at wage `w` only if, after debiting `w` from present gold, the owner's
  soonest future-money savings want (`soonest_savings_horizon()` :22816) is still newly provisioned by
  the expected sale proceeds while all higher-ranked wants are preserved. Binary-search the highest `w`
  that clears — that is the wage ceiling. (If credit is ever wanted here, it must be introduced
  explicitly as a C7 dependency, never implied by the bundle's payable semantics.)

No cardinal marginal-product is ever computed; the ceiling is the rank-walk break-even, the same
discipline the input bid uses, with the wage cost entering `present_advance` instead of an input
price. The employer posts a hire order (`LaborOrder`, Hire side) at `min(wage_ceiling, wage_eligible_reserve)`
(§4.6) for the labor its unmet output demand warrants. **This adapter is new code (Slice B).**

### 4.4 The worksite / labor-capacity / project bridge (new machinery, Slice B)

**Correction to an earlier draft (spec-review P0): the existing labor machinery will not compose
without a bridge.** `apply_labor_trade()` (factor.rs:603) requires an `M2Project` in `Forming` state
(:619, :636); settlement chain agents are constructed with `labor_capacity: 0` (settlement.rs:21553);
and `LaborReservations` reject a Work order from a zero-capacity agent (factor.rs:103). C1 must build
an explicit settlement-side bridge:

- **Pre-match labor capacity (spec-review round 2, P0).** `reservation_labor_ask_...` and
  `LaborReservations` reject a Work order from a zero-capacity agent (factor.rs:547–549, :107–112;
  society.rs:4173–4232), and chain agents are built with `labor_capacity: 0` (settlement.rs:21553) —
  so granting capacity *at contract time* is too late; it is needed to post/reserve the ask. Gated by
  `wage_labor`, at the **start of the labor phase, before matching**, each hire-eligible non-owner is
  assigned its available per-tick **labor capacity** = the labor-hours it is willing to sell this tick
  (its labor budget net of what Leisure and any retained self-provision claim — the same rest/leisure
  readback `allocate_labor` uses). This is a real quantity (the worker genuinely has labor to sell),
  set pre-match so the Work ask and reservation are accepted, reset each tick, and spent by the fast
  loop as labor is delivered. Off-path (`wage_labor` off): unchanged — capacity stays 0.
- **The owner's production as a fundable project:** route the owner's recipe run as (or wrap it in) an
  `M2Project` in `Forming` so `apply_labor_trade`'s labor advances it; **output ownership stays with the
  owner** — the worker is paid a wage and does not own the output.
- **Assignment + delivery:** the hired worker is directed to the owner's worksite; the fast loop
  (`run_fast_loop` ~9992) logs delivered labor per contract (path-block / interruption / death = short
  delivery); the existing rest/leisure readback (`allocate_labor`, sim.rs) still governs work-vs-rest.

This bridge is the **largest piece of genuinely new code** in C1.

### 4.5 Clearing and the conserved wage escrow (new escrow primitives, Slice C)

Clearing reuses `LaborBook` matching (`match_hire`/`match_work`, factor.rs:360–470; resting order wins
at the intersection wage). The payment path changes: today `apply_labor_trade` pays + advances
synchronously (factor.rs:649/685/691). Under `wage_labor` C1 uses the two-rate delivery-escrow
(`game-spec.md` §4.3):

1. At labor-clearing (econ tick T), the wage moves employer → **escrow**; the worker is assigned to the
   owner's worksite; no project progress yet.
2. The fast loop of T+1 delivers labor at the worksite (short delivery on block/interruption/death).
3. T+1's labor phase settles **pro-rata**: delivered → wage released to worker + project advanced by
   the delivered units; undelivered → refunded to employer.

**Correction to an earlier draft (spec-review P0): the escrow is NOT primitive-safe via
`move_money_conserved()`** — that helper moves agent→agent and, if the recipient id is missing, **debits
without crediting** (settlement.rs:14269); `escrow_gold` is not an agent. C1 must **define explicit
escrow primitives**:

- `debit_to_escrow(employer, amount, contract)` and `credit_from_escrow(contract, → worker | employer)`
  — conserved by construction (agent balance ↔ `escrow_gold`), backed by an escrow record
  `{ employer, worker, contract_id, amount, delivered_so_far }`.
- **Death routing (explicit, all cases; spec-review round 2, P0):**
  - Dead **worker, before delivering:** the full escrowed wage refunds to the (living) employer at T+1.
  - Dead **worker, after delivering but before the T+1 release:** the **delivered portion is earned
    income** and routes to the worker's estate (heir, else commons) via the existing estate settlement;
    the undelivered portion refunds to the employer.
  - Dead **employer:** the escrowed wage has *already left the agent balance* into escrow, so it cannot
    flow through the balance-based estate logic — C1 settles it explicitly: release the delivered portion
    to the worker, route the remainder to the employer's estate at death.

**Build notes (from the GO/NO-GO review; non-blocking but fix during Slice C):**
- **Death-hook ordering.** Death settlement removes agents (`update_needs_and_remove_dead` ~9357,
  `age_and_remove_elderly` ~9362) *before* the proposed T+1 labor-settlement phase — so the escrow
  death hook must run **at/inside death removal** (settling the dying agent's escrow obligations there),
  not in the later labor phase, or a worker/employer removed at T+1 would never settle its escrow.
- **Debit exactly once.** When the wage moves into escrow at clearing, the matched hire **reservation**
  (`LaborReservations`, factor.rs) must be **released/consumed** in the same step, so the owner is
  debited once (into escrow) and not twice (reservation hold + escrow).

**Simpler synchronous fallback (noted, not recommended):** pay for labor already delivered in this
tick's fast loop, no escrow — but it cannot represent short delivery and needs prior-tick assignment. If
the escrow + bridge over-scope the milestone, Slice B ships the synchronous variant and the escrow
becomes C1.5. The escrow is the faithful two-rate contract and the default plan.

### 4.6 Wage-fund provenance: the retained-earnings ledger (the anti-subsidy guard, Slice C)

**Correction to an earlier draft (spec-review P0): capping the wage at the owner's current balance does
NOT prove the wage came from output sales** — the balance may be the starting endowment, which would be
the S23d subsidy in disguise. C1 enforces provenance with a ledger:

- A per-owner **wage-eligible retained-earnings** bucket, credited **only** by the owner's realized
  output-**sale** proceeds (attributable via the existing bread/provenance/acquisition ledgers,
  settlement.rs:17031/7285), and wages are **debited from this bucket**.
- Wages that could only be funded from endowment (bucket empty) are disallowed at the headline, or
  tallied separately as `endowment_funded_wages`; the `subsidised_wage` control (which injects the
  bucket) must classify `WageScaffoldOnly`.
- Any cold-start owner reserve (Risk 2) is a **declared scaffold**, classified as such, never counted
  toward the headline circular flow.
- **Provenance-only, capped, and death-routed (spec-review round 4, P1).** The bucket is a *provenance
  tag* on already-conserved gold, **never a second store of value**: it is capped at the owner's actual
  spendable gold (you can't pay a wage the balance can't cover even if the tag says otherwise), so it
  cannot double-count or mint capital. On owner **death**, the bucket is simply **discarded** — the
  owner's gold routes through the normal estate machinery, and a **heir does NOT inherit wage-eligible
  status**: inherited/endowment gold is not "earned," so an heir must re-earn wage-eligibility through
  its own sales before it can fund wages (this is what stops endowment gold from becoming earned wage
  capital across a generation).

This makes "wages come from real earnings" a **checkable invariant**, not a hope. Because the bucket
**steers** hiring, it is digested ON-only (see §5).

### 4.7 Money-regime handling (the emergent-money gotcha)

The escrow primitives (§4.5) must use the **same conserved gold path the S23d/S23e base already uses**.
The trap (`report-emergence-and-its-limits.md`; mapper): post-promotion emergent money makes
`uses_closed_gold_money()` false while `money_system` is `None`, so the *closed-gold* primitives are
blocked — the escrow debit/credit must move `Agent.gold` directly (the emergent-regime path the
rival-commons branch already exercises) rather than route through the closed-gold helpers. Pre-promotion
(barter) there is no money to pay wages in, so C1's labor market is **post-promotion only** (gate on
`current_money_good() == Some(SALT)`, matching S22c/S22f anti-circularity) — also correct
praxeologically: wages are a monetary phenomenon.

---

## 5. Conservation and determinism

**Escrow holder.** Add `escrow_gold: Gold` to `Settlement` (parallel to `commons_gold`, ~6313).
**Correction (spec-review P1): money conservation is separate from `report.conserves()`** — the
`EconTickReport::conserves()` identity is per-*good* only and does **not** check money
(settlement.rs:5580). Gold is conserved via `total_gold()`, checked before/after the fast loop and
after the market (settlement.rs:9304 / 9580 / 17719). C1 must therefore (a) add `escrow_gold` into
`total_gold()` so those existing checks cover it, and (b) add an **explicit money/escrow invariant** —
asserted each tick and exercised by a dedicated test — that `Σ agent.gold + commons_gold + escrow_gold`
is constant except at the named promotion channel. Every wage entering escrow must be released or
refunded by horizon end; `EscrowUnbalanced` (stranded/lost funds) is a hard test failure. Death routing
is handled explicitly by the escrow primitives (§4.5), **not** the estate machinery — the wage has
already left the agent balance, so the estate path cannot see it.

**Digest (tag 22, ON-only).** In `canonical_bytes()` (settlement.rs:20107+), following the tag-13..17
idiom exactly: `if self.wage_labor_active() { out.push(22); out.push(u8::from(flag)); /* wage_margin_bps,
market params */ ; /* escrow_gold; escrow records; per-owner retained-earnings ledger; per-worker
wage-proceeds bucket; per-colonist employment state */ }`. **Every piece of state that steers behavior
must be digested (spec-review round 4, P1)** — this explicitly includes the **retained-earnings ledger**
(it gates wage bids, §4.6) and the **wage-proceeds bucket** (it drives the attribution metric, §7), not
just `escrow_gold` and per-colonist employment state (current wage-contract remaining, escrowed amount,
assigned worksite). All serialized ON-only inside the gate, no marker, conditional-omit — exactly as
S22f's `commitment_remaining` (settlement.rs:21091) and S22b's skill (:21058). **Off-path (`wage_labor`
false): nothing emitted → byte-identical.** (Diagnostic-only counters that never steer a decision —
e.g. `endowment_funded_wages` if used purely for reporting — stay out of the digest, per the S22
runtime-only convention.)

**Determinism.** No live RNG; the labor market clears deterministically (price-then-seq like the goods
book). All new state integer-only.

---

## 6. Slices

Each slice is a separate commit with its own tests; the milestone gates on the full acceptance suite.

- **Slice A — base compose + precondition gate.** Rebase S23d + S23e forward onto master `1029223`;
  verify the composed base conserves, digests (tags 20/21 byte-identical to their branch goldens), and
  **reproduces the S23e marginal-cell verdict** (`SubsistenceBoundDespiteScarcity`, `buyer_bought ≈ 0`)
  with `wage_labor` off. Establishes the control baseline C1 must beat. *DoD: composed base green,
  S23e verdicts reproduced, `BaseUnviable` not triggered on any seed.*
- **Slice B — the labor-market phase + bridges.** The gated phase between 4e and 5: the explicit
  scarce-outside-option gate (§4.2, **pre-market commons-sufficiency forecast**); the worker ask (reuse
  `reservation_labor_ask_...`); the **new `appraise_labor_hire_for_money` adapter** (§4.3, wage cost into
  `present_advance`); the **worksite / labor-capacity / project bridge** (§4.4 — **pre-match per-tick
  labor capacity**, owner-recipe-as-`M2Project`-in-`Forming`, worksite assignment, fast-loop delivery
  log, owner keeps output); and `LaborBook` clearing. Ships with the synchronous payment path first (escrow deferred to
  Slice C). *DoD: voluntary hires clear post-promotion only; non-vacuity + discrimination + counterfactual
  tests pass; goods market unaffected when off.*
- **Slice C — the conserved wage escrow, provenance ledger, money invariant, tag 22 digest.** The
  explicit escrow primitives `debit_to_escrow`/`credit_from_escrow` + escrow records + two-tick
  release/refund + both-sides death routing (§4.5); the per-owner **wage-eligible retained-earnings
  ledger** (§4.6); `escrow_gold` added to `total_gold()` with the **explicit money/escrow invariant**
  asserted and tested (§5, since `report.conserves()` does not cover money); tag 22 ON-only serialization
  + the byte-identical off-path regression. *DoD: escrow fully balances every run; the money invariant
  holds every tick; wages are provably from sale proceeds (endowment-funded wages tallied separately);
  goldens byte-identical off.*
- **Slice D — the acceptance suite + controls** (§7). *DoD: suite green across SEEDS; verdict reported;
  all controls separate; 5 goldens pinned.*

---

## 7. Acceptance suite (`sim/tests/wage_labor.rs`)

Mirror the S22f/S23e house structure (mapper 3; `sim/tests/support/mod.rs` `SEEDS = [3,7,11,19,23]`,
`PROBE_TICKS = 1_600`):

- **Predeclared thresholds** (a priori, principled, **swept** — never fitted): `MIN_HIRES`,
  `θ_SHARE` (wage-financed buy share), `V_MIN` (final-window velocity floor), `K` (loop turnovers),
  `N` (inherit-eligible owner deaths, inherited from S23d). Sweep each on its load-bearing axis and
  report the outcome curve, so a positive can't be a single tuned value and a negative can't be a
  failure-to-search.
- **Ordered verdict enum** (§2) reported via `println!` under `--nocapture`, **not** asserted (the
  finding is classified, not forced).
- **Metrics struct** per run: seed, conserved, escrow_balanced, bread_minted_max, extinct,
  provenance_clean, promoted, hires_post_promotion, distinct_workers, distinct_employers,
  wage_financed_buy_share, final_window_velocity, circular_loop_turnovers, buyer_bought,
  owner_surplus, inherit_eligible_owner_deaths.
- **Wage-financed purchase attribution (concrete accounting rule; spec-review P1).** Money is fungible,
  so the acquisition ledger (settlement.rs:17031/7285) proves a good was *bought* but not that the money
  was *wage-derived*. C1 maintains a per-worker **wage-proceeds bucket**: credited by wage released from
  escrow, debited **FIFO** as that worker spends on output. Then `wage_financed_buy_share` =
  (wage-bucket debits on output in the final window) / (total non-owner output purchases in the final
  window), and `circular_loop_turnovers` counts completed employer→escrow→worker→(buy)→owner-revenue
  cycles. This makes the headline metric a checkable quantity, not a fungibility hand-wave.
- **Hard guards asserted every run:** `report.conserves()`, escrow balances, `bread_minted_max == 0`,
  `!extinct`, provenance clean.
- **Mandatory non-vacuity + discrimination:** ≥ `MIN_HIRES` voluntary post-promotion hires each
  traceable to a matched ask/bid; some eligible hungry non-owners below-ask are **not** hired; a real
  **counterfactual**: a non-owner who buys owner output *because* it earned a wage, where the matched
  `no_wage_offered` run leaves it buying nothing.
- **Controls (each its own scenario, must separate):** `no_wage_offered` → reproduces S23e null;
  `fiat_wage` (forced hiring) → `WageScaffoldOnly`; `subsidised_wage` (injected fund) →
  `WageScaffoldOnly`; `wage_labor_off` matched base → S23e verdict; the φ sweep {0.25, 0.5, 1.25}
  reported (headline = 0.5).
- **`goldens_unchanged()`:** with `wage_labor` off, pin 5 byte-identical digests across the standard
  configs (lineages 300/800, frontier, spatial-households, viable) exactly as
  `voluntary_cultivation_commitment.rs:851–887`; **and** confirm tags 20/21 reproduce the S23d/S23e
  branch goldens on the composed base.

Build/verify (plain cargo, no nix): `cargo test -p sim --test wage_labor -- --nocapture`,
`cargo test --lib`, `cargo fmt --check`, `cargo clippy -- -D warnings`, full workspace green.

---

## 8. Anti-smuggling / anti-circularity guards

1. **No cardinal utility.** The worker's ask is the existing ordinal `reservation_labor_ask_...`
   (rank-walk above Leisure); the employer's bid is the ordinal bundle-appraisal break-even. No
   marginal-product number, no cardinal welfare, no aggregate is read in either decision (econ purism,
   compiler-enforced).
2. **No planner placement.** Hiring is a voluntary bilateral match in the `LaborBook`; no agent is
   assigned a wage or a job. The wage is discovered by ask/bid clearing, not set.
3. **Anti-subsidy (the central trap).** The wage fund is the owner's own realized sale proceeds,
   enforced by the wage-eligible retained-earnings ledger (§4.6) — a balance cap alone is insufficient
   (it can't tell earnings from endowment); the `subsidised_wage` control and the separate
   `endowment_funded_wages` tally prove the headline is not the S23d subsidy in disguise.
4. **Anti-circularity.** Post-promotion only (`current_money_good() == Some(SALT)`); a `pre_money`
   inertness check confirms zero wage activity before money exists.
5. **Anti-fiat re-pin.** The `fiat_wage` control (forced employment) classifies `WageScaffoldOnly`,
   never headline success — the wage relation must be voluntary on both sides.
6. **Conservation is not weakened.** The escrow is a conserved holder in the explicit money invariant
   (§5, `total_gold()` extended; `report.conserves()` is per-good only); no wage is minted or destroyed;
   short delivery refunds, and death is routed explicitly by the escrow primitives (§4.5), not the
   estate path.
7. **The base is not tuned to pass.** Slice A must reproduce S23e's *negative* with wages off; the
   only new variable between the S23e null and a C1 positive is the wage.

---

## 9. Risks and open questions

1. **The satiation echo (top risk).** If a hired worker earns a wage, satiates its bounded savings
   want, and withdraws (the Exp-11 wall), demand could stall again. Mitigation: recurring consumption
   under the scarce commons keeps the money want reopening (S12 `recurring_motive` precedent); if it
   still stalls, that is `WageInertDemandStillDead` and the next finding — reported, not tuned.
2. **The employer-side chicken-and-egg.** The owner needs prior earnings to advance a wage, but needs
   to sell output to have earnings. On the S23e base owners already sell *some* surplus (S23e:
   `owner_sold ≤ 106`), which makes a self-funded bootstrap **plausible but not guaranteed** — the
   surplus may be too thin to fund a wage large enough to matter. If so, C1 may need a
   cold-start seeded owner reserve (disclosed, like the S5 cold-start buffers) — classified as a
   **declared scaffold** in the retained-earnings ledger (§4.6) and excluded from the headline circular
   flow, never a per-tick subsidy.
3. **Escrow scope.** The two-rate escrow plus the worksite/capacity bridge (§4.4) are the largest new
   mechanisms; if they balloon the milestone, Slice B ships the synchronous fallback (§4.5) and the
   escrow becomes C1.5. Decide at Slice B review.
4. **Base rebase drift.** S23d/S23e were cut before `b7e6b0f`; the rebase must be verified to
   reproduce their goldens/verdicts (Slice A). If master moved something they depend on, that surfaces
   in Slice A as a mechanical fix, not a C1 design change.
5. **Which φ is the honest headline.** Marginal (0.5) is the informative cell (money + surplus +
   scarcity, only income missing). Report the full sweep; if only φ = 0.25 or only 1.25 flips, that is
   itself a scoped finding about *how much* scarcity the income loop needs.

---

## 10. Falsifiable-bar summary (one paragraph)

On the S23e marginal rival-commons base (φ = 0.5), where S23e measured money promoting, owners holding
surplus, and `buyer_bought = 0`, adding a **voluntary, own-earnings-funded, ordinally-cleared wage
labor market** should turn that null into a **sustained producer/buyer money market**: non-owners earn
wages and spend a material share of them on owner output across the whole final window, money velocity
stays bounded away from zero, the wage→buy→revenue→wage loop turns over ≥ K times, the S23d
mortality/inheritance and SALT promotion preconditions hold, and conservation including the new escrow
holds every tick — with `no_wage_offered` reproducing the S23e null and both `fiat_wage` and
`subsidised_wage` separating as scaffold. Success is `CircularFlowForms` (the strand's first positive,
reopening the S23c generational-tenure study as C3); the honest alternative is
`WageInertDemandStillDead`, a sharper null naming whatever condition income alone still cannot supply.
