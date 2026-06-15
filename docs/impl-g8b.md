# Implementation Spec G8b: banks & credit

## Purpose

G8a put the settlement on M3 ledger money (specie). G8b adds the **bank**: a
chartered institution that takes **deposits** and lends **fiduciary credit**
(demand claims beyond reserves), gated by its reserve ratio and the regime.
This is the credit layer — the machinery the lab proved drives the Austrian
boom/bust, now in the spatial game on emerged/ledger money.

Scope: the banking **mechanism** on the G8a M3 settlement — charter, deposit,
fiduciary lending — with the lab's **100%-reserve control** (a full-reserve
bank lends zero fiduciary). The full ABCT credit-cycle *demonstration*
(boom→stop→bust) rides with the regime ladder in G8c (the cycle needs a
regime that enables then stops credit). It is NOT fiat / the regime ladder /
tender / taxation (G8c), and NOT a change to econ's bank/M3 behavior (the six
goldens stay byte-identical — bank logic reused unchanged).

## Verified Base Facts (2026-06-15, oikos @ `bffee5e`, 975 tests green)

1. **econ has the full bank machinery** (bank.rs): `Bank` balance sheet
   (reserves, deposits, `fiduciary_issued`, `reserve_ratio_bps`),
   `fiduciary_lend_capacity(regime)` (regime- and reserve-gated;
   `ReserveRatioBps::FULL` → zero capacity), `can_issue_fiduciary`. The
   issuer/credit machinery (issuer.rs) is likewise present. G8b REUSES this;
   it adds no bank logic to econ.
2. **G8a put the sim on the M3 ledger** (`MoneySystem`: specie / fiat /
   demand claims / bank reserves). Deposits move specie → bank reserves +
   demand claims; fiduciary lending creates demand claims beyond reserves —
   all already modeled in the M3 ledger. G8b wires the bank into the
   settlement and routes deposit/lend through the existing M3 paths.
3. **The lab's M3 invariants** (carried in econ) include
   `commodity_credit_is_cycle_neutral` and `hundred_pct_reserve_lends_no_fiduciary`
   — the 100%-reserve control G8b reuses as its falsification.
4. **Conservation**: the M3 ledger conserves specie + fiduciary exactly
   (`fiduciary <= demand_claims`; reserves back claims). G8b's whole-system
   invariant spans the M3 ledger (now with nonzero claims/reserves/fiduciary)
   + goods.
5. **Goldens byte-identical**: the bank/M3 logic is reused unchanged; chartering
   a bank in the spatial sim is game-only. Determinism inherited.

## Milestone Boundary

G8b includes:

- a bank as a settlement entity (chartered via config; the player-`Command`
  charter is G8c/UI): reserves, deposits, fiduciary lending, reserve ratio;
- deposits: colonists deposit M3 specie → bank reserves + demand claims they
  hold and spend (claims circulate as money via the M3 ledger);
- fiduciary lending: the bank lends demand claims beyond reserves (regime-
  and reserve-gated), credited to borrowers who spend them into the economy;
- whole-system conservation spanning the M3 ledger with nonzero
  claims/reserves/fiduciary (+ goods);
- a `bank` config (fractional-reserve bank lends fiduciary) and a
  `full-reserve` control (`ReserveRatioBps::FULL` → zero fiduciary) — the
  falsification twin;
- viewer surfacing of the M3 composition (specie / claims / reserves /
  fiduciary) and bank balance sheet;
- acceptance tests in `sim/tests/g8b_banks.rs`;
- README + `engine-divergence.md` (banks/credit in the sim; the full credit-
  cycle demonstration + fiat/regime/tender/tax deferred to G8c).

G8b excludes:

- no fiat, regime ladder, tender policies, or taxation (G8c);
- no full ABCT boom/bust DEMONSTRATION (needs the regime ladder to
  enable-then-stop credit — G8c); G8b proves the lending MECHANISM + the
  reserve control;
- no player-`Command` bank charter (config-chartered here; the Command/UI is
  G8c/G9);
- no change to econ bank/M3 BEHAVIOR — six goldens byte-identical; wiring is
  additive/game-only;
- no Credit/Modern era rungs yet (G8c);
- no `HashMap` in logic; nothing drawn; no asserted magnitudes beyond
  fiduciary-lent>0 (fractional) vs ==0 (full reserve) and exact M3
  conservation.

## Domain Semantics

### Bank, deposits, fiduciary lending

A chartered bank has reserves and a reserve ratio. Colonists deposit M3
specie: specie moves to the bank's reserves, and the depositor receives demand
claims (M3 ledger) they can spend — claims circulate as money. The bank lends
**fiduciary** credit: it issues demand claims beyond its reserves, up to
`fiduciary_lend_capacity(regime)` (zero at `ReserveRatioBps::FULL` or under a
regime that forbids fiduciary), credited to borrowers who spend them into the
sim economy. All of this runs through econ's existing M3 ledger/bank paths;
G8b routes the sim's deposit/lend actions into them.

### Conservation with credit

The M3 ledger conserves: public specie + bank reserves is conserved (deposits
move specie to reserves); demand claims = reserves-backed + fiduciary;
`fiduciary <= demand_claims`. The whole-system invariant now spans specie +
claims + reserves + fiduciary + goods. Fiduciary issuance is NOT minting
specie — it is credit expansion the ledger tracks explicitly (and the lab's
TMS distinction holds: fiduciary expands spendable claims without new specie).

### Control

The `full-reserve` bank (`ReserveRatioBps::FULL`) lends zero fiduciary — the
lab's `hundred_pct_reserve_lends_no_fiduciary`, reused as the falsification:
deposits still circulate as claims, but no credit is created.

## Acceptance Tests

`sim/tests/g8b_banks.rs` (+ unit tests):

1. `bank_run_is_deterministic` — same `(seed, config)` → byte-identical run
   with deposits and fiduciary lending.
2. `deposits_become_claims_backed_by_reserves` — a deposit moves specie to
   bank reserves and gives the depositor demand claims; the claims spend as
   money; specie + reserves conserved.
3. `fractional_bank_lends_fiduciary` — a fractional-reserve bank issues
   fiduciary credit (demand claims beyond reserves) to borrowers, who spend
   it into the economy; `fiduciary_issued > 0`.
4. `full_reserve_lends_no_fiduciary` — the control: a `ReserveRatioBps::FULL`
   bank lends zero fiduciary (`fiduciary_issued == 0`) while deposits still
   circulate. Paired with test 3, isolates credit creation to fractional
   reserve.
5. `m3_conserves_with_credit` — whole-system conservation holds with nonzero
   claims/reserves/fiduciary: `fiduciary <= demand_claims`, reserves back
   claims, specie conserved, goods conserved — every econ tick.
6. `fiduciary_is_not_minted_specie` — issuing fiduciary does not change the
   public specie total; it expands claims (the TMS distinction).
7. `econ_unchanged` — full workspace suite; six econ goldens byte-identical;
   all prior G1–G8a tests green; `cargo clippy --workspace --all-targets -- -D
   warnings`; `cargo fmt --check`.

Manual check:

```bash
cargo test -p sim
cargo run -p viewer -- run bank --ticks 40          # deposits, claims, fiduciary
cargo run -p viewer -- run bank-full-reserve --ticks 40
```

## Handoff Notes

- REUSE econ's `Bank`/M3 ledger paths unchanged; G8b charters a bank in the
  settlement and routes deposit/lend through them — no new bank logic in
  econ. Six goldens byte-identical (game-only wiring); test 7 is the tripwire.
- Conservation now spans specie + claims + reserves + fiduciary + goods;
  `fiduciary <= demand_claims` and reserves back claims (the M3 invariants).
  Fiduciary is credit, not minted specie (tests 5, 6).
- The `full-reserve` control (test 4) is the proof credit creation comes from
  fractional reserve, not from deposits alone — the lab's
  `hundred_pct_reserve_lends_no_fiduciary`, in the sim.
- Scope: the lending MECHANISM + reserve control. The full boom/bust
  DEMONSTRATION needs the regime ladder (enable-then-stop credit) and lands
  in G8c; do not build the regime ladder here.
- config-chartered bank; the player-`Command` charter is G8c/UI.
- `git add` new files; gitignore stray build artifacts.

## Amendment A1 (spec-owner, 2026-06-15) — claim-holder death settlement & the complete bank invariant

Review surfaced a split on claim-holder starvation deaths (an always-on
`assert!` vs a unified M3 estate drain). Ruling, reconciled against the G8b
**Milestone Boundary** and **Handoff Notes** — which this milestone holds
authoritative ("no change to econ bank/M3 BEHAVIOR; REUSE econ's `Bank`/M3
ledger paths unchanged; six goldens byte-identical; wiring additive/game-only"):

- **The unified claim/fiat estate drain is deferred to G8c, not G8b.** Routing
  a dead agent's demand claims and fiat to its estate requires changing econ's
  M3 removal path — `remove_agent`/`can_remove_agent` and the `Estate`
  composition — which is exactly the econ M3 BEHAVIOR change the G8b boundary
  excludes. Claims-estate routing rides with the G8c tender/tax/regime finance
  work (alongside the player-`Command` charter), where the rest of the
  demand-claim machinery lands. For **G8b**, `remove_agent` drains **specie
  only** (the G8a resolution); a balance holding demand claims or fiat is still
  refused by `can_remove_agent`, and the bank configs are **no-death by
  design**: `Settlement::generate` rejects `bank && demography` and any
  non-curated (starvation-prone) banked layout, so no funded-with-claims death
  arises. The starvation-death path keeps its fail-loud `assert!` — every
  colonist reaching the death window must be settle-able (`can_remove_agent`) —
  as the backstop for any future claim/fiat holder introduced outside those
  generation guards. This is the design the shipped code, tests, README, and
  `engine-divergence.md` already implement coherently.
- **The conservation gate is complete (G8b, shipped):**
  `invariants_hold_with_banks` asserts the sum of bank `demand_deposits` equals
  the ledger's aggregate `demand_claims` (not only that reserves reconcile) — so
  a bank balance-sheet drift from the ledger is caught by the reconcile gate the
  G8b tests use.

Note: an earlier draft of this amendment proposed performing the unified
specie+claims+fiat drain (a `drain_m3_estate` over the full composition,
`can_remove_agent` accepting claim/fiat holders, banked demography enabled)
*inside* G8b. That is withdrawn: it conflicts with the milestone's "reuse econ's
M3 paths unchanged / no econ M3 behavior change" boundary, which G8b holds
authoritative over this amendment. The unified drain lands in G8c with the rest
of the claims-estate routing. Only the complete bank invariant above is in G8b.

## Amendment A2 (implementer, 2026-06-15) — depositor-death settlement by deposit withdrawal

Round-8 review (and direct reproduction — `run bank --seed 6 --ticks 500`)
falsified A1's premise that the curated banked layout is "no-death by design." The
underlying viable economy is viable only over a **bounded horizon**: its depositing
consumers eventually starve once their finite WOOD income is exhausted — this holds
**with or without a bank** (the bank-free `viable`/`m3-settlement` colony loses its
consumers at a similar horizon). A depositing colonist therefore *does* reach the
starvation-death window still holding the demand claims its deposits created, so the
A1 generation guards do **not** prevent a funded-with-claims death, and the
fail-loud `assert!` fires on a public CLI run.

Ruling, reconciled against the Milestone Boundary and Handoff Notes (which remain
authoritative — "reuse econ's `Bank`/M3 ledger paths unchanged; no econ M3 behavior
change; six goldens byte-identical; wiring additive/game-only"):

- **A dying depositor's bank deposit is withdrawn before removal**, in the sim, with
  **no econ change and no claims-estate routing** (both still G8c). The fix is the
  natural deposit⇄withdrawal symmetry: `Settlement::liquidate_bank_deposit_on_death`
  redeems the dying colonist's demand claims for specie through econ's **existing**
  `MoneySystem::redeem_demand_claim_for_specie` path (the bank pays specie from its
  reserves, retiring reserves + demand deposits so both the ledger and the A1
  conservation gate stay reconciled), after which the colonist holds **only specie**
  and settles as the ordinary G8a specie estate. `remove_agent` still drains specie
  only and `can_remove_agent` still refuses claim/fiat balances — both **unchanged** —
  and the death-window `assert!` stays as the fail-loud backstop for any residual a
  reserve-bounded withdrawal could not cover.
  This supersedes A1's "no-death by design + bare assert" only on the mechanism: A1's
  goal — no panic, a settled death, no econ change — is met, by withdrawing the
  deposit rather than by assuming the death never arises.
- **The generation guards stay, re-scoped.** `Settlement::generate` still rejects
  `bank && demography` (old-age/heir settlement of claims is unhandled — only the
  starvation path withdraws) and still limits banked configs to the curated M3 layout,
  now framed as **milestone scope** (G8b ships only the `bank` / `bank-full-reserve`
  controls; broader banked layouts are G8c), not a no-death claim.
- **The unified claim/fiat estate drain remains G8c**, exactly as A1 ruled — A2 adds
  no estate composition change, no `can_remove_agent` relaxation, and no banked
  demography. `banked_depositor_death_is_settled_by_deposit_withdrawal` (test 8) pins
  the settled death, the cross-death ledger reconcile, and conserved specie.

## Amendment A3 (implementer, 2026-06-15) — reserve headroom and exact G8b charters

Round-9 review found two G8b boundary bugs after A2:

- A fractional bank that lent to the exact reserve-ratio limit could redeem a dying
  depositor's claims 1:1 from reserves and demand deposits, stay M3-ledger reconciled,
  but fall below its configured reserve ratio (for example, `run bank --seed 6 --ticks
  321` ended below the 20% charter).
- The curated-layout guard still admitted custom bank ratios, even though G8b ships only
  the `bank` and `bank-full-reserve` controls.

Ruling: G8b remains game-only wiring over econ's unchanged bank/M3 paths. The sim-side
bank phase now lends **up to** econ's `Bank::fiduciary_lend_capacity`, capped by a
deterministic reserve-headroom buffer for protected depositor-death redemptions. The
buffer preserves enough excess reserves that the existing `redeem_demand_claim_for_specie`
withdrawal can settle a dying depositor without taking the bank below its configured
reserve ratio. G8b generation also rejects any `BankConfig` other than the exact shipped
fractional and full-reserve charters; broader bank finance remains G8c.
