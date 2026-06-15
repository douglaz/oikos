# Implementation Spec G8c-3: tax receivability (the state's counter-lever)

## Purpose

G8c-2 gave the player the **private** acceptance levers (tender policies):
when the labor market refuses fiat wages, fiat credit is inert — the boom
never transmits. G8c-3 adds the **state's** counter-lever: **tax
receivability** (the lab's M21, chartalist). The state levies a tax and
declares which media discharge it. When the tax is **receivable only in
fiat**, agents must obtain and remit fiat to settle their liability — so fiat
gains a *compelled* demand through the **fiscal channel** even where the labor
market refused it. This is the chartalist answer to private refusal: the state
can drive fiat acceptance by taxing in it.

The headline ties straight back to G8c-2: in a settlement whose **wages are
specie-only** (G8c-2: fiat credit inert, no private fiat demand), a
**fiat-receivable tax** routes fiat through the fiscal channel
(`tax_receipts_fiat > 0`) — the state compels what the market refused. The
**control** is the same settlement with a **specie-receivable tax**
(`tax_receipts_specie > 0`, `tax_receipts_fiat == 0`): no compelled fiat
demand.

It is NOT a change to econ's M21 behavior (the six goldens stay
byte-identical — the tax machinery is reused unchanged), NOT a multi-seed
study, and NOT the player-`Command`/UI layer (G9). **G8c-3 is the last
economic milestone before the G9 graphical-UI hand-off.**

## Verified Base Facts (2026-06-15, oikos @ `ca40e74`, 1033 tests green)

1. **econ has the complete M21 tax layer**: `EventKind::SetTaxReceivability(
   TaxReceivability)` and `EventKind::LevyTax { .. }` (society.rs:1219,1223);
   `Society::apply_levy_tax` seeds the tax as a `DebtContract` with
   `principal = ZERO`, `purpose: DebtPurpose::TaxLiability`, `funding:
   CreditSource::Tax(issuer)`, owed to the single state issuer
   (society.rs:1403); `TaxReceivability { SpecieOnly (default), FiatOnly,
   FiatAndSpecie }` (money.rs:153); settlement is `settle_due_debts_m3`
   (timemarket.rs), **gated by `TaxReceivability`, never the credit tenders**;
   the issuer tracks `taxes_levied` / `tax_receipts_fiat` /
   `tax_receipts_specie` / `taxes_defaulted` (issuer fields). G8c-3 REUSES all
   of it; it adds no tax logic to econ.
2. **The M21 doctrine** (econ/tests/m21_tax_receivability.rs header): a tax is
   a `DebtContract` with `principal = ZERO` whose lender is the issuer; the
   payables view (`agent_debt_views`) is the demand mechanism; **tax receipts
   NEVER move `credit_retired` or `fiat_credit_outstanding`** (tax is fiscal,
   not credit). The declared **Known Seam**: payable-accounting labor pull is
   AMOUNT-based; media enter only at settlement (the worker is paid in fiat
   because the employer holds nothing else and the debit order is fiat-first —
   not media-aware planning). Preserve this seam; do not engineer around it.
3. **G8c-1 / G8c-2 give the spatial fiat settlement + the wage lever**: the
   settlement runs M3 + banks + fiat under the regime ladder, and
   `LaborWageTender` gates whether fiat reaches workers. G8c-3 adds the state
   levy + receivability as sim policy on that same settlement.
4. **Conservation/determinism inherited**: a tax moves money from the agent to
   the issuer (the liability discharged), never creating/destroying it; a
   default leaves the liability unmet by rule (`taxes_defaulted`), not by leak.
   Tax receipts do not touch the credit aggregates (fact 2). Determinism
   preserved.
5. **Goldens byte-identical**: the M21 tax logic is reused unchanged; levying
   it as a sim policy on the spatial settlement is game-only.

## Milestone Boundary

G8c-3 includes:

- the **state levy + receivability** as sim policy on the G8c-1/G8c-2
  settlement: `SetTaxReceivability` (SpecieOnly / FiatOnly / FiatAndSpecie)
  and `LevyTax` routed through econ's unchanged tax machinery (config-set;
  player-`Command` is G9);
- the **headline**: in a **specie-only-wage** settlement (G8c-2: fiat credit
  inert), a **fiat-receivable** tax compels fiat demand through the fiscal
  channel — `tax_receipts_fiat > 0`; fiat circulates via tax where wages
  refused it (the chartalist counter-lever);
- a **specie-receivable** control: `tax_receipts_specie > 0`,
  `tax_receipts_fiat == 0` — no compelled fiat demand (the falsification
  twin);
- the levy/receipt accounting (`taxes_levied`, `tax_receipts_fiat/specie`,
  `taxes_defaulted`) surfaced; tax is fiscal — it does NOT move
  `credit_retired` / `fiat_credit_outstanding` (fact 2);
- whole-system conservation across levy → settlement (tax moves money,
  never creates/destroys; a default is unmet-by-rule, not a leak); determinism;
- a `tax-in-fiat` config (fiat-receivable tax under specie-only wages →
  compelled fiat demand) and a `tax-in-specie` control;
- viewer surfacing: active tax receivability, taxes levied, fiat vs specie
  receipts, defaults;
- acceptance tests in `sim/tests/g8c3_tax.rs`;
- README + `engine-divergence.md` (tax receivability as the state's
  counter-lever; the chartalist channel; **G9 graphical UI is the next
  milestone and the hand-off point — not autonomously buildable on the
  headless test loop**).

G8c-3 excludes:

- no change to econ M21 BEHAVIOR — six goldens byte-identical; game-only
  wiring; the Known Seam (fact 2) preserved, not engineered around;
- no player-`Command`/UI tax setting (config-set here; G9);
- no multi-seed study; no multi-issuer tax (econ's M21 is single-issuer —
  fact 1; do not add issuer-id to the levy);
- no `HashMap` in logic; nothing drawn; magnitudes SIGN only
  (`tax_receipts_fiat > 0` vs `== 0`; defaults > 0 vs 0) + exact conservation.

## Domain Semantics

### Tax as a chartalist demand for the receivable medium

A `LevyTax` seeds a zero-principal `DebtContract` (the liability) owed to the
single state issuer; `TaxReceivability` declares which media discharge it. The
payables view pulls the agent's labor to cover the **amount**; at **settlement
time** (`settle_due_debts_m3`) the receivability gate decides which media may
remit. Under `FiatOnly`, only fiat settles the tax: an agent must hold (hence
acquire) fiat to discharge it — the tax *manufactures demand* for fiat. This
is the chartalist driver: the state's willingness to receive a medium for
taxes is what gives that medium its compelled acceptance. Tax is **fiscal**,
not credit: receipts move into the issuer's tax accounts and never touch
`credit_retired` / `fiat_credit_outstanding` (fact 2).

### The counter-lever to G8c-2

G8c-2 showed that **specie-only wages** render fiat credit inert — the private
labor market refuses fiat, so the boom never transmits. G8c-3's headline runs
that same specie-only-wage settlement and adds a **fiat-receivable tax**: now
agents must obtain fiat to pay the state, so fiat circulates through the
**fiscal** channel even though the **labor** channel refused it
(`tax_receipts_fiat > 0`). Tax receivability is thus the state's answer to
private refusal — the chartalist lever sitting opposite the M17 wage lever.
The **specie-receivable control** (same settlement, tax in specie) compels no
fiat demand (`tax_receipts_fiat == 0`), proving the fiat circulation comes
from the receivability policy, not from the levy or the spatial dynamics.

## Acceptance Tests

`sim/tests/g8c3_tax.rs` (+ unit tests):

1. `tax_run_is_deterministic` — same `(seed, config)` → byte-identical run
   through levy and settlement.
2. `fiat_tax_compels_fiat_demand` — `tax-in-fiat` (fiat-receivable tax under
   specie-only wages): the tax settles in fiat (`tax_receipts_fiat > 0`) —
   fiat circulates through the fiscal channel where the labor market refused
   it. (Sign only.)
3. `specie_tax_compels_no_fiat_demand` — `tax-in-specie` control:
   `tax_receipts_specie > 0`, `tax_receipts_fiat == 0`. Paired with test 2,
   isolates the compelled fiat demand to the receivability policy (the
   counter-lever, not the levy or the spatial dynamics).
4. `tax_is_fiscal_not_credit` — tax levy/receipt does NOT move
   `credit_retired` or `fiat_credit_outstanding` (fact 2); the levy is a
   zero-principal liability owed to the issuer.
5. `tax_settlement_conserves` — levy → settlement moves money from agent to
   issuer (never creates/destroys); a default is unmet-by-rule
   (`taxes_defaulted`), not a leak; whole-system conservation holds.
6. `tax_receivability_gates_the_tax_surface` — a medium not in the active
   `TaxReceivability` cannot discharge the tax even if held; the receivable
   medium does (the M21 gate, in the sim).
7. `econ_unchanged` — full workspace suite; six econ goldens byte-identical;
   all prior G1–G8c-2 tests green; `cargo clippy --workspace --all-targets --
   -D warnings`; `cargo fmt --check`.

Manual check:

```bash
cargo test -p sim
cargo run -p viewer -- run tax-in-fiat --ticks 80     # fiat-receivable tax -> fiat demand compelled
cargo run -p viewer -- run tax-in-specie --ticks 80   # the control: no compelled fiat demand
```

## Handoff Notes

- REUSE econ's M21 tax machinery (`apply_levy_tax`, `settle_due_debts_m3`
  gated by `TaxReceivability`, the issuer tax accounts) unchanged; G8c-3
  routes the sim's levy/receivability into it. Six goldens byte-identical
  (game-only); test 7 is the tripwire.
- Preserve the declared **Known Seam** (fact 2): the labor pull is
  amount-based, media enter only at settlement. Do NOT engineer media-aware
  planning into the tax path — the headline's fiat receipts come from the
  receivability gate + the fiat-first debit order, exactly as the lab's M21.
- Tax is **fiscal, not credit**: receipts must NOT move `credit_retired` /
  `fiat_credit_outstanding` (test 4). The counter-lever framing is the point —
  the fiscal channel circulates fiat that the labor channel (G8c-2) refused.
- The specie-receivable control (test 3) is the proof the compelled fiat
  demand is the receivability policy. If the control shows fiat receipts, the
  gate isn't routing settlement — fix that, don't weaken the test.
- Single-issuer only (econ's M21 — fact 1); do not add an issuer-id to the
  levy. Magnitudes SIGN only; conservation exact (a default is
  unmet-by-rule, not a leak).
- **After G8c-3, the next milestone is G9 (the Bevy graphical UI). G9 cannot
  be driven by the headless rb-lite + golden-test loop and is the explicit
  hand-off point to the user.** Record this in `engine-divergence.md`.
- `git add` new files; gitignore stray build artifacts.
