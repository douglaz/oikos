# impl-49 — C2: The Firm as a Going Concern (does persistent enterprise organize production without satiating and retiring?)

Status (spec): **DRAFT — pending Codex spec-review.** Layer C2 of the civilization-core roadmap
(`docs/spec-civ-core-roadmap.md`). Composes on **C1** (`impl-wage-labor.md`, `wage_labor` tag 22).
Flag `firm_enterprise`, digest **tag 23**, ON-only.

Falsifiable bar (headline): a firm — an entrepreneur-owner who advances money from **retained earnings**,
hires labor (C1), runs a multi-tick production project (M2Project), sells output, and keeps its earnings
— **persists** as a going concern across a long horizon **without satiating and retiring** (the Exp-11
wall), and **profit/loss selects**: a firm fed a bad forecast consumes its reserve and dissolves, a
well-appraising one under standing demand grows.

## 0. Dependency & premise (read first)

**C2 assumes C1 succeeded.** Its whole point — persistent enterprise organizing hired labor — is only
meaningful if C1 produced `CircularFlowForms` (buyers earn wages and spend, so firms have both a labor
supply to hire and a product demand to sell into). **If C1 lands as `WageInertDemandStillDead`, C2's
premise fails** and this spec must be revisited (the firm would have no solvent customers). Per the
project's discipline (findings steer the next spec), C2 is written now as a buildable plan but is
**provisional on C1's landed verdict**; its base is the C1 stack (`S23d` tag 20 + `S23e` tag 21 +
`wage_labor` tag 22) once C1 merges. Nothing here weakens that dependency.

**The retracted-"needs firms" caution.** OIKOS once concluded it "needs firms/employment" and then
*retracted* it (memory Exp 11 / Codex `e3df8c9`): the producer de-adoption that motivated firms was an
**artifact** of bounded-satiable savings + a one-off adoption motive + a fixed pool, **not** an economic
necessity. C2 therefore must **not** re-introduce firms as a fix for a fake problem. Its justification is
different and forward-looking: at *civilization scale* the firm is the natural **organizing unit of
large-scale division of labor** — the going concern that lets an owner coordinate hired labor, capital,
and inputs into sustained roundabout production and bear the profit/loss that selects good coordination
from bad. C2 is a **structural** milestone (does a persistent enterprise form and get selected?), not a
patch.

## 1. Praxeology — the firm is one acting man

A firm is **not** a planner and has **no** aggregate objective (methodological individualism). It is one
acting man — the entrepreneur-owner — making **ordinal** appraisals under uncertainty, using **monetary
calculation** (Mises: comparing money magnitudes — reserve, wage bill, input cost, forecast revenue) to
decide what to produce and whom to hire. The only selection is **entrepreneurial profit/loss**: a firm
that misappraises consumes its capital and dissolves; one that appraises well accumulates and grows.
Nothing cardinal enters a decision: hiring reuses C1's ordinal `appraise_labor_hire_for_money`;
continuation reuses the ordinal role-choice / recurring-profitability gate. The "balance sheet" is money
magnitudes (already conserved gold), not a utility number.

## 2. What already exists (reused primitives — the economics, not the code)

- **The production lifecycle = the M2Project state machine** (`econ/src/capital.rs:13–49`):
  `Forming → Waiting → Mature → Sold` with `Abandoned` for dissolution. `start_project` (:144),
  `advance_project` (:194, Forming→Waiting when `labor_advanced ≥ required_labor`), `mature_project`
  (:215, Waiting→Mature at maturity), `record_project_sale` (:229, Mature→Sold), `abandon_project`
  (:274, salvage at `salvage_bps`; `advanced_gold` is unrecovered loss). This IS a firm's production +
  dissolution mechanics; C1's worksite bridge already wraps an owner's recipe in a `Forming` M2Project.
- **The reserve / retained earnings = C1's wage-eligible retained-earnings ledger** (`impl-wage-labor.md`
  §4.6) over `Agent.gold` (`econ/src/agent.rs:95`, persistent across ticks). C1 credits it from realized
  output-sale proceeds and debits wages from it. C2 makes it the firm's **balance sheet**.
- **Profit/loss inputs**: S11 `PriceBelief`/`forecast_output_price` (`expect.rs:5`, settlement.rs:21312);
  realized proceeds via `record_project_sale` (capital.rs:229); the S22c rolling return window
  (settlement.rs:9560) as a per-producer realized-margin accumulator.
- **Anti-satiation gate**: `recipe_is_profitable` (settlement.rs:22042) + `recurring_motive`
  (settlement.rs:1442) — keeps a producer adopted while the recipe is simply profitable at realized
  prices, bypassing the one-off-savings-want de-adoption (`recipe_adoption_pays_for_money` :22719 /
  `soonest_savings_horizon` :22816).
- **Role machinery**: `run_role_choice` (:14320), `run_production` (:12500).

So C2 adds **little new *economics*** — but **substantial new code** (spec-review P2, not "just wiring"):
a **project scheduler** (rolling the owner through a sequence of M2Projects), a **dissolution/bankruptcy
path** (reserve-exhaustion → `abandon_project` → de-adopt), and **firm accounting** (reserve credit/debit,
owner-consumption drain, P/L window). It binds the existing primitives into a *persistent going concern
with a balance sheet and a dissolution rule*, and proves persistence + profit/loss selection.

## 3. Mechanism

### 3.1 The firm record (new, lightweight)

A gated per-owner `Firm { owner: AgentId, reserve_ledger (C1 retained-earnings bucket), owned_capital:
Vec<tool/plot>, active_projects: Vec<M2Project>, pl_window: rolling realized (proceeds − costs) }`. The
reserve is *not* a second money balance (it is the owner's own gold, tagged by C1 provenance), and
`owned_capital` references existing tool/plot ownership. **Correction (spec-review P1): the firm's
production projects are NOT already-digested state.** Today `M2Project` state is serialized only through
the cycle-specific digest path (settlement.rs:20715/23173); a firm's ordinary production projects are new
instances that path would not capture. So tag 23 must **fully serialize every firm-owned project's steering
state** (`state`, `advanced_gold`, `labor_applied`, `maturity`, `output_good/qty`, abandonment) alongside
the reserve provenance, the P/L window, and the adopted/dissolved status (§5) — anything less is a
determinism hole.

### 3.2 The going concern (persistence across projects)

Today an owner runs one recipe per tick. C2 makes the owner run a **rolling sequence of M2Projects**
funded from the firm reserve: on `Sold`, proceeds credit the reserve (C1 ledger); the firm immediately
appraises the *next* project (same ordinal `recipe_adoption` / `recurring_motive` gate), hiring labor
for it via C1 if the appraisal clears at the current wage. Retained earnings roll forward, so the firm
is a **standing entity**, not a one-shot. Working capital persistence reuses the S3 retained-earnings +
reserve precedent (no per-tick planner loan).

### 3.3 Profit/loss selection and dissolution (the falsifiable core)

- **Credit**: realized sale proceeds (`record_project_sale`) → reserve.
- **Debit**: wage bill (C1 escrow releases), input cost, operating cost → reserve.
- **Owner consumption drain**: the owner is a consumer — its own consumption draws down the reserve each
  period (so a firm that merely breaks even on paper still faces the owner's recurring needs; this is
  what makes profitability *have* to recur).
- **Dissolution**: if the reserve cannot fund the next project's wage+input advance (losses exhausted
  it), the firm **abandons** its in-flight project (`abandon_project`, salvage returned, `advanced_gold`
  lost) and **de-adopts** (owner reverts to Unassigned / re-enters the labor market as a worker). This is
  bankruptcy by rule, conserved, not a panic.
- **Growth**: a firm whose reserve accumulates hires more labor / runs larger or parallel projects, up to
  standing product demand.

### 3.4 Anti-satiation (the Exp-11 guard, made mechanical)

Continuation is keyed on **recurring profitability under recurring consumption**, not a filled savings
want: the firm keeps producing while (a) the recipe `recipe_is_profitable` at realized prices AND (b)
there is standing product demand (recent sales > 0), with the owner's consumption drain keeping the
money want reopening. It must **not** de-adopt merely because a bounded savings want is full (the S12
`recurring_motive` path is the mechanism). If a firm still satiates-and-retires under recurring
consumption, that is the `SatiationRetiresFirm` finding — reported, not tuned away.

## 4. Praxeology / anti-smuggling guards

1. **Individualist.** The firm's every "decision" is the owner's ordinal appraisal; no aggregate is read.
2. **Ordinal hiring & continuation.** Reuse C1's `appraise_labor_hire_for_money` (own-funded, present
   gold debited) and the ordinal `recipe_is_profitable`/`recurring_motive` gate; no cardinal
   marginal-product, no cardinal profit-maximization — profit/loss is realized money magnitudes that
   select, not a maximand read by a decision.
3. **Profit/loss is the only selection.** No firm is preserved by subsidy; dissolution is by reserve
   exhaustion. A `bad_forecast` control must dissolve; a `no_owner_consumption` control (drain off) tests
   whether the consumption drain is what forces recurring profitability.
4. **Conservation.** The reserve is the owner's own conserved gold (C1 ledger provenance, no second
   store); dissolution routes salvage/assets through the existing `abandon_project` + estate machinery.
   No mint, no planner placement.
5. **Post-money.** Firms are a monetary-calculation phenomenon → gated post-promotion, like C1.

## 5. Conservation & determinism

- **No new money holder.** The firm reserve is `Agent.gold` tagged by the C1 retained-earnings ledger
  (already digested by tag 22). C2 adds no gold sink/source; the existing `total_gold()` + money
  invariant (C1 §5) cover it.
- **Digest (tag 23, ON-only).** `if self.firm_enterprise_active() { out.push(23); out.push(u8::from(flag));
  /* per-firm: adopted/dissolved status, P/L window, reserve provenance, AND each firm-owned project's full
  steering state (state, advanced_gold, labor_applied, maturity, output_good/qty, abandonment) */ }`,
  following the tag-13..17 idiom (settlement.rs:20419+). **Correction (spec-review P1): the firm's
  production projects are NOT already digested** — the existing M2Project serialization is the
  cycle-specific path only (settlement.rs:20715/23173), so every firm project/output/funding field that
  steers scheduling, dissolution, sale, or continuation must be serialized under tag 23 or determinism
  breaks. The reserve is the owner's own gold (already covered by C1's tag-22 provenance), not
  re-serialized as a second balance. Off-path (`firm_enterprise` false): nothing emitted → byte-identical.
- **Determinism.** No live RNG; project sequencing and dissolution are deterministic functions of realized
  prices and reserve. Integer-only.

## 6. Slices

- **Slice A — the firm record + going concern.** The gated `Firm` derived state; roll a sequence of
  M2Projects funded from the C1 reserve; retained earnings roll forward. *DoD: an owner runs consecutive
  projects across ticks funded by its own accumulating reserve; off-path unchanged.*
- **Slice B — profit/loss selection + dissolution.** Reserve credit/debit incl. owner consumption drain;
  `abandon_project`-based dissolution on reserve exhaustion; growth when reserve accumulates. *DoD: a
  loss-making firm dissolves conserved; a profitable one persists and grows.*
- **Slice C — anti-satiation + digest tag 23.** Continuation on recurring profitability under recurring
  consumption; the tag-23 ON-only serialization + byte-identical off-path regression. *DoD: a profitable
  firm under recurring demand does not retire; goldens byte-identical off.*
- **Slice D — acceptance suite + controls** (§7).

## 7. Acceptance suite (`sim/tests/firm_enterprise.rs`)

Mirror the house structure (`SEEDS=[3,7,11,19,23]`, `PROBE_TICKS=1_600`).

- **Predeclared thresholds (swept):** `R` (persistence horizon), `F` (fraction of final window a firm is
  continuously staffed), `MIN_FIRMS`, growth/deficit margins.
- **Ordered verdict enum:** base-precondition failures (C1 `CircularFlowForms` did not reproduce →
  `BaseUnviable`; conservation/money-invariant broke) → scaffold controls by scenario mode → outcome:
  `EnterprisePersists` (a firm stays continuously staffed ≥F of the final window, renewing on realized
  recurring profit, with owner-consumption drain active) / `SatiationRetiresFirm` (firms retire despite
  recurring consumption) / `NoProfitLossSelection` (bad-forecast firms do not dissolve, or good ones do
  not grow).
- **Mandatory non-vacuity + discrimination:** ≥`MIN_FIRMS` firms run consecutive projects post-promotion;
  a real counterfactual — a `bad_forecast` firm consumes reserve and dissolves where the matched
  well-appraising firm persists.
- **Controls:** `bad_forecast` (systematically wrong price belief → must dissolve); `standing_demand_off`
  (no recurring product demand → must fail to sustain hiring); `no_owner_consumption` (drain off → tests
  whether satiation reappears without it); `firm_enterprise_off` matched base (reproduces C1's outcome).
- **`goldens_unchanged()`:** with `firm_enterprise` off, pin 5 byte-identical digests across the standard
  configs; confirm tags 20/21/22 reproduce the C1-stack goldens on the composed base.

Build/verify (plain cargo): `cargo test -p sim --test firm_enterprise -- --nocapture`, `cargo test --lib`,
`cargo fmt --check`, `cargo clippy -- -D warnings`, workspace green.

## 8. Risks & open questions

1. **C1-outcome dependency (top).** If C1 is `WageInertDemandStillDead`, C2 has no solvent customers and
   must be re-scoped. Non-negotiable premise.
2. **The satiation echo, again.** The firm form could re-hit the Exp-11 wall if continuation quietly keys
   on a savings want; the owner-consumption drain + recurring-profitability gate are the guard, and
   `no_owner_consumption` is the control that proves it. If it still satiates, that is the finding.
3. **Single-owner firm only.** C2's firm is one entrepreneur; multi-owner/partnership/joint-stock is out
   of scope (a later layer). Disclosed.
4. **Dissolution realism.** `abandon_project` salvage + de-adoption is the bankruptcy model; debts to
   workers are already settled through C1 escrow, so a dissolving firm owes no stranded wages — verify
   this composes (a firm dissolving mid-contract must still honor the escrowed wage for delivered labor).

## 9. Falsifiable-bar summary

On the C1 `CircularFlowForms` base, binding the existing M2Project lifecycle + C1 retained-earnings
reserve + the ordinal recurring-profitability gate into a persistent single-owner firm should yield a
**going concern that survives many ticks hiring labor and selling output without satiating** (renewing on
realized recurring profit under an owner-consumption drain), while **profit/loss selects** (bad-forecast
firms dissolve via conserved abandonment; well-appraising firms grow) — with `bad_forecast`,
`standing_demand_off`, and `no_owner_consumption` controls separating and conservation holding every tick.
Success is `EnterprisePersists`; the honest alternatives are `SatiationRetiresFirm` (the Exp-11 wall is
real even under recurring consumption) or `NoProfitLossSelection` (the model doesn't select on
entrepreneurial error) — each a first-class finding.
