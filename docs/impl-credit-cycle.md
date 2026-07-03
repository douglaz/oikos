# impl-54 — C7: Money, Credit, and the Business Cycle at Scale (does the Austrian cycle emerge on emergent money?)

Status (spec): **DRAFT — pending Codex spec-review.** Layer C7 of the civilization-core roadmap
(`docs/spec-civ-core-roadmap.md`). Composes on **C1** (`wage_labor` tag 22, a wage structure to distort),
**C2** (`firm_enterprise` tag 23, the investors), the merged **G8a/b/c** bank/credit/tender stack, and
**S10** originary interest. Flag `credit_cycle`, digest **tag 28**, ON-only.

Falsifiable bar (headline): the M17 pair, at civ scale and on **emergent** money — credit expansion
**with** fiat-wage tender lengthens the production structure and then **busts** (abandoned projects rise =
visible capital consumption); **without** it (free wages), issuance is inert.

## 0. Dependency & premise + a path caveat (read first)

C7 delivers bank credit that funds entrepreneurial investment and, as the payoff, the **Austrian
business cycle as an emergent phenomenon**: fiduciary credit unbacked by real saving pushes the loan rate
below the natural rate (set by ordinal time preference, S10), induces **malinvestment** in overly-roundabout
projects, and the correction is a **bust** — abandoned projects the player can walk through. It composes
on C1 (a wage structure the boom distorts) and C2 (firms that invest); **provisional on their landed
verdicts.**

**Path caveat (must resolve in Slice A):** the seam research for C7 was mapped against the **praxsim lab
source** (`praxsim-core/src/*`), which OIKOS's `econ/` crate was **forked from**. The architecture
(bank balance sheet, loan/time-market, agio schedules, the credit-disabled shadow, Cantillon, the
boom-project + abandonment) is expected to exist in `econ/src/{bank.rs, timemarket.rs, agio.rs, shadow.rs,
metrics.rs, cantillon.rs, issuer.rs, capital.rs, ledger.rs}`, **but the exact line numbers below are lab
citations. Spec-review (2a) has since CONFIRMED the substrate is present in the oikos `econ/` fork** —
banks (`econ/src/bank.rs`), fiduciary loans (`timemarket.rs`), `agio.rs`, the credit-disabled `shadow.rs`
(+ its measurement-only import guard), the `metrics.rs` natural-rate proxy, and `credit_boom_long_line`
(`capital.rs`) all exist — so **C7 is NOT a port of that machinery.** Slice A still re-pins the §2
citations to `econ/src/`, but the real new work is elsewhere: **the live V2→M3 emergent-SALT bridge does
not exist** (§3.1) and is the milestone's dominant engine task, not a re-scope risk.

## 1. Praxeology — the cycle must emerge, and the shadow must only measure

- **Natural vs. loan rate.** The natural rate reflects real time preference — the intertemporal exchange
  ratio agents will voluntarily lend/borrow at, derived **ordinally** from each agent's multi-horizon
  wants (the `agio` schedule, S10). Bank fiduciary credit pushes the *loan* rate below it.
- **Malinvestment and bust.** The below-natural rate makes overly-roundabout projects look profitable;
  when real savings prove insufficient (or the expansion stops and the rate rises back), those projects
  become unviable and are **abandoned** — partial salvage, the rest is visible **capital consumption**.
- **Anti-smuggling (the crux):** the cycle must **emerge** from bank/issuer balance-sheet policy
  (exogenous/player) *interacting with agents' ordinal intertemporal choices* — it must **not** be
  scripted. The **shadow credit-disabled counterfactual** is the *measurement* of the natural-rate gap and
  the malinvestment — it must be **measurement-only, never read by any decision** (the lab enforces this
  with a test that forbids decision modules from importing `shadow`; C7 must keep that guard).

## 2. What already exists (expected in the `econ/` fork — verify in Slice A)

- **The bank stack (G8b):** a `Bank` balance sheet (`reserves`, `demand_deposits`, `loans_outstanding`,
  `fiduciary_issued`, `reserve_ratio_bps`), `fiduciary_lend_capacity()`, `record_fiduciary_loan()` (the
  point of fiduciary creation), `retire_fiduciary_principal()` on repayment; `run_bank_phase`
  (settlement.rs ~9760). Credit is **named** (`CreditSource::BankFiduciary` / `FiatCredit` / `FiatFiscal`
  / `Commodity` / `Tax`), never silent.
- **The loan/time market:** `LoanOrderBook::add_order_m3` crosses lend/borrow orders → issues demand
  claims + records the fiduciary loan; `DebtContract` with a `CreditSource`. Bank debt settlement retires
  fiduciary principal.
- **The natural rate (S10, ordinal):** the `agio` schedule derives each agent's lending/borrowing quotes
  from its own multi-horizon money wants (not an exogenous rate); `metrics` builds a credit-disabled
  **proxy** natural rate from those schedules; the tick computes `market_rate` (actual trades) and
  `rate_gap = natural − market` (the credit-expansion signal), stored in the M3 record — **read by no
  decision**.
- **The shadow counterfactual:** `run_credit_disabled_shadow` replays the scenario with credit disabled
  (banks full-reserve, fiat stripped) to output the natural rate + structure length; the
  `decision_modules_do_not_import_shadow` test enforces measurement-only.
- **Cantillon / fiat issuance:** `FiatPrint` → `CantillonRouter::route` injects new money to named
  first-receivers with `CreditSource` receipts; `issuer.fiscal_issue` (free) vs `record_credit_loan`
  (repayable).
- **Malinvestment substrate:** an over-roundabout boom project (`credit_boom_long_line`: more labor, longer
  horizon, low salvage_bps) + `abandon_project` salvage (capital.rs) = visible capital consumption;
  `abandon_unviable_projects` triggers on the correction; `boom_projects_started`/`bust_abandoned_projects`
  counters.
- **Tender (M17):** `LaborWageTender` gates whether fiat-wage projects are viable — fiat-wage tender lets
  a boom persist; free wages make injection inert.

## 3. Mechanism — the V2→M3 bridge is the load-bearing new work

- **The V2→M3 runtime bridge (the #1 engine dependency; game-spec §13 item 2).** Today banking runs only
  on *designated* money (a static GOLD seed at society init, `from_agents_with_banks`), joined to the
  emergent-money runtime (`step_v2`, which promotes SALT) by a **static bridge seed** — banks never form
  on runtime-emerged money. C7 builds the **live handoff**: after V2 promotes SALT, (a) snapshot the
  emergent-money supply, (b) revalue/rebase the bank reserves + `commodity_base` from GOLD to the emerged
  SALT base, (c) route bank policy/capacity off the emerged base, **and (d) transition the runtime so
  institutional/bank events apply on the emerged money** — today V2 **rejects institutional events** except
  recipe disables (society.rs:6751) and the V2/M3 stepping paths are separate (society.rs:758/885), so the
  bridge must hand off into the M3 machinery (which accepts bank charters, tender, fiat) now keyed to the
  emerged SALT base — so **banks form on the good the world actually monetized** and credit expansion
  distorts the *emergent* money economy. This conserved handoff is the milestone's **dominant** new code
  (spec-review 2a), not a small hookup.
- **Credit expansion → boom → bust (compose the substrate).** With the bridge live, a chartered bank
  expands fiduciary credit (player/exogenous reserve-ratio policy); the below-natural loan rate funds
  boom projects (C2 firms take longer-roundabout M2Projects); when the expansion cannot be sustained by
  real saving, `abandon_unviable_projects` busts them (salvage loss = capital consumption). The shadow
  measures the natural-rate gap throughout.
- **The M17 gate.** Route the boom through `LaborWageTender`: fiat-wage tender transmits the injection into
  the wage structure (boom persists, then busts); free-wage tender leaves it inert.

## 4. Praxeology / anti-smuggling guards

1. **Emergent, not scripted.** The cycle arises from balance-sheet policy × ordinal agio/project choices;
   no boom/bust is hardcoded. **`rate_gap` is ex-post measurement, never a live gate (spec-review P2):** the
   *decision* is each agent's ordinal agio/project choice; that `boom_projects_started` rises with
   `rate_gap>0` is a **record-analysis / falsification** check computed after the fact from the M3 record —
   not a predicate any decision reads (the `shadow` import guard, shadow.rs:351, stays).
2. **Shadow is measurement-only.** Preserve the `decision_modules_do_not_import_shadow` guard; no decision
   reads the natural rate / rate gap / structure length.
3. **Credit is named, never silent.** All credit flows carry a `CreditSource`; the money ledger reconciles
   every tick (`money_ledgers_reconcile`), so no fiduciary/fiat is minted off-book.
4. **Natural rate is ordinal.** The agio schedule is derived from each agent's own multi-horizon wants
   (S10), not an exogenous rate; a `flat_time_preference` control should flatten the cycle's amplitude.
5. **The bust is real capital.** Malinvestment shows as `abandon_project` salvage loss, a named
   consumption, not a metric relabel.
6. **Post-money.** Banking is post-promotion (the whole point of the V2→M3 bridge).

## 5. Conservation & determinism

- **Conservation.** Fiduciary credit lives on the bank balance sheet (claims vs. reserves); fiat issuance
  is the explicit named channel; the ledger reconciles every tick (`base.commodity_base + fiat_base ==
  public_specie + public_fiat + demand_claims + bank_reserves`). The V2→M3 rebase must be a **conserved
  revaluation** (no money created by the handoff itself). C1's money invariant + the M3 reconciliation
  both hold.
- **Digest (tag 28, ON-only).** `if self.credit_cycle_active() { out.push(28); ... bank balance-sheet
  state, active loan contracts, the bridge's emerged-base snapshot, boom/bust project state }` — every
  field that steers lending/abandonment. The shadow/rate-gap are **runtime-only diagnostics** (like S8
  emergence probes) and stay **out** of the digest (they never steer). Off-path (`credit_cycle` false):
  byte-identical.
- **Determinism.** Loan crossing is price-then-seq; Cantillon routing deterministic; the bridge rebase is
  a deterministic function of the emergence snapshot. Integer-only.

## 6. Slices

- **Slice A — verify the fork + the V2→M3 bridge.** Confirm which of the bank/agio/shadow/cantillon/boom
  machinery `econ/src/` actually has (re-pin citations); build the live V2→M3 handoff (banks form on
  emerged SALT, conserved revaluation). *DoD: a chartered bank operates on runtime-emerged SALT; the
  ledger reconciles every tick; goldens byte-identical off.*
- **Slice B — credit expansion → boom.** Fiduciary credit at a below-natural rate funds boom (longer-
  roundabout) C2-firm projects; the shadow measures the rate gap. *DoD: `boom_projects_started` rises with
  `rate_gap>0`; shadow natural rate computed, read by no decision.*
- **Slice C — the bust + the M17 gate + tag 28.** `abandon_unviable_projects` on the correction (salvage
  loss = capital consumption); the `LaborWageTender` gate (fiat-wage → transmits; free-wage → inert);
  tag-28 ON-only digest. *DoD: the M17 pair reproduces at civ scale on emergent money; goldens
  byte-identical off.*
- **Slice D — acceptance suite + controls** (§7).

## 7. Acceptance suite (`sim/tests/credit_cycle.rs`)

`SEEDS=[3,7,11,19,23]`, long horizon.

- **Predeclared thresholds (swept):** reserve-ratio (credit capacity), boom-project count, bust-abandonment
  count, rate-gap magnitude, structure-length change.
- **Ordered verdict enum:** base-precondition (V2→M3 bridge live / ledger reconciles / C1+C2 base) →
  outcome: `CycleEmerges` (credit expansion **with** fiat-wage tender lengthens the production structure
  then busts — `boom_projects_started` then `bust_abandoned_projects` rise, salvage loss booked — and the
  shadow rate-gap tracks it) / `CreditInertOnFreeWages` (free-wage tender → issuance inert, no boom/bust —
  the M17 negative leg).
- **Mandatory non-vacuity:** fiduciary credit actually issued on emerged SALT; boom projects started and
  busted; the shadow natural-rate gap non-trivial; a real counterfactual — the fiat-wage vs free-wage pair.
- **Controls:** `free_wage_tender` (M17 negative — inert); `full_reserve` (no fiduciary credit → no cycle);
  `flat_time_preference` (flattens amplitude); `credit_cycle_off` matched base; the
  `decision_modules_do_not_import_shadow` guard test.
- **`goldens_unchanged()`:** with `credit_cycle` off, byte-identical; the emergence (G5) + econ goldens
  still byte-identical.

Build/verify: `cargo test -p sim --test credit_cycle -- --nocapture`, `cargo test --lib`, fmt, clippy
`-D warnings`, workspace green; the G8 + emergence suites green.

## 8. Risks & open questions

1. **The V2→M3 bridge (top engineering risk).** The largest single piece of new engine work; if the
   `econ/` fork lacks the shadow/agio stack, C7 becomes a substantial port and must be re-scoped (Slice A
   surfaces this).
2. **Upstream dependency.** No wage structure to distort (C1 failed) or no firm investors (C2) → no cycle.
3. **Conserved revaluation.** The GOLD→SALT rebase must create no money; verify the ledger reconciles
   across the handoff tick.
4. **Emergent-money credit realism.** Banks forming on a barter-emerged medium is novel; the cycle's shape
   on emergent money may differ from the lab's designated-money result — a scoped finding, reported.
5. **Path/citation drift.** All §2 citations are lab-source; they MUST be re-pinned to `econ/src/` in
   Slice A before build.

## 9. Falsifiable-bar summary

Building the live V2→M3 bridge so banks form on runtime-emerged SALT, then composing the existing
fiduciary-credit + ordinal-agio-natural-rate + boom-project + abandonment substrate, should make the
**Austrian business cycle emerge on emergent money**: credit expansion with fiat-wage tender lengthens the
production structure and then busts (visible capital consumption), the shadow measuring the natural-rate
gap throughout, while free-wage tender leaves issuance inert (`CycleEmerges` vs `CreditInertOnFreeWages` —
the M17 pair at civ scale). The honest alternatives are a re-scope (if the fork lacks the natural-rate
stack) or an inert result — each a first-class finding. This spec's citations are lab-source and must be
re-verified in `econ/src/` first.
