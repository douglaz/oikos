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
