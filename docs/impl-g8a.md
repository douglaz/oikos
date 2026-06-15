# Implementation Spec G8a: the M3-ledger money settlement (finance foundation)

## Purpose

G8 brings the late game — banking, credit, fiat, the regime ladder, tender
policies, taxation. All of it runs on econ's **M3 ledger money**
(`MoneySystem`: specie / fiat / demand claims / bank reserves), not the M1
closed-gold the sim has used so far. G8a is the foundation: run a spatial
settlement on **M3 ledger money** and resolve the **M3 estate routing** that
G4a/b deferred — *without* banks or fiat yet. It is the prerequisite for all
of finance.

G8 is sliced (it is the heaviest arc):

- **G8a (this milestone): M3-money settlement.** The spatial sim runs on the
  M3 `MoneySystem` (specie as money, no banks/fiat); deaths route M3 balances
  to the estate (resolving the G4a/b deferral); conservation spans the M3
  ledger.
- **G8b: banks & credit** (chartered banks, deposits, fiduciary lending).
- **G8c: fiat, the regime ladder, tender policies, taxation** (M11–M17 +
  tax-receivability as player policy levers; the era detector's Credit/Modern
  rungs unlock here).

It is NOT banks/credit (G8b), NOT fiat/regime/tender/tax (G8c), and NOT a
change to econ's M3 *behavior* (the six goldens stay byte-identical — the M3
banking/market logic is reused unchanged; G8a only routes the sim's flows
through the M3 ledger and resolves runtime removal).

## Verified Base Facts (2026-06-15, oikos @ `75fb2b5`, 962 tests green)

1. **econ has the full M3 `MoneySystem`** (ledger.rs:136) and M3 is selected
   by `ScenarioKind::MarketM3` → `m3_enabled` builds a `money_system`
   (society.rs:361,370); the M3 market/ledger flows (`ledger_credit`/
   `ledger_debit`, M3 spot execution) already exist (society.rs:2311+). G8a
   REUSES the M3 behavior; it does not change M3 market logic.
2. **The sim uses M1 closed-gold today** (`Agent.gold`, no `MoneySystem`).
   G8a constructs the settlement's Society as M3 (`MarketM3`) so money lives
   in the `MoneySystem` ledger, and routes the sim's money flows (wage
   escrow, the world→econ transfer settlement, trades, estate) through the
   M3 ledger instead of `Agent.gold`.
3. **The G4a/b deferral**: `remove_agent` gracefully REFUSES a funded M3
   agent (returns `None`), so a death with an M3 ledger balance cannot
   currently complete. G8a resolves it: `remove_agent` (M3 path) drains the
   agent's ledger balance into the returned `Estate` (conserved), so M3
   deaths settle like M1 deaths did.
4. **Goldens byte-identical by construction**: the lab's M3 scenarios run a
   fixed roster and never free an agent at runtime; the new M3-removal path
   is game-only. The M3 market/ledger logic the goldens exercise is unchanged.
5. **Conservation, determinism** inherited: the M3 ledger has its own exact
   conservation (specie/fiat/claims/reserves balance); G8a's whole-system
   invariant spans the M3 ledger + goods. Integer, `Rng` at generation only,
   nothing in the loops, `BTreeMap`/`Vec`, no `HashMap`.

## Milestone Boundary

G8a includes:

- a settlement mode on **M3 ledger money** (`MarketM3` Society with a
  `MoneySystem`; specie is the money; NO banks, NO fiat, NO claims issuance);
- routing the sim's money flows through the M3 ledger: the two-rate wage
  escrow, the world→econ transfer settlement, spot trades, and estate
  transfers all move M3 specie via the ledger (not `Agent.gold`);
- resolving the deferred M3 estate routing: `remove_agent` (M3) drains the
  ledger balance into the `Estate`; deaths/births conserve M3 balances;
- whole-system conservation spanning the M3 ledger (specie) + goods;
- an `m3-settlement` config (a settlement running on M3 specie money) and the
  demonstration it behaves like the M1 settlement economically (same trades,
  same outcomes — M3 specie with no banks/fiat is economically M1, but
  ledger-accounted);
- viewer surfacing of the M3 money composition (specie; fiat/claims zero);
- acceptance tests in `sim/tests/g8a_m3_money.rs` (+ econ M3-removal unit
  tests);
- README + `engine-divergence.md` (M3-money settlement; M3 estate routing
  resolved; banks/fiat/regime/tender deferred to G8b/c).

G8a excludes:

- no banks, deposits, fiduciary, or credit (G8b);
- no fiat, regime ladder, tender policies, or taxation (G8c);
- no change to econ M3 market/ledger BEHAVIOR — six goldens byte-identical;
  the M3-removal path is additive/game-only;
- no Credit/Modern era rungs yet (they unlock at G8c);
- no `HashMap` in logic; nothing drawn; no asserted magnitudes beyond
  economic-equivalence-to-M1 (specie, no banks) and exact M3 conservation.

## Domain Semantics

### M3 money flows in the sim

The settlement's Society is `MarketM3` with a `MoneySystem`. Money is M3
**specie** (no fiat, no claims — those need G8c/G8b). Every sim money flow
routes through the M3 ledger:

- wage escrow (the §4.3 two-rate contract): the escrowed wage is an M3-ledger
  amount, released/refunded via the ledger;
- the world→econ transfer settlement (G2b): the depositor is paid in M3
  specie via the ledger;
- spot trades: cleared by the existing M3 market path (reused);
- estate: a dead agent's M3 specie balance drains to the estate (G8a's new
  `remove_agent` M3 path), then to heirs/commons (G4 routing) — conserved.

Because there are no banks or fiat, M3 specie behaves economically exactly
like the M1 gold did — the difference is that it is **ledger-accounted**
(specie / fiat / claims / reserves), which is what banks and fiat (G8b/c)
will build on. G8a proves the spatial sim + demography run correctly on the
M3 ledger, conserving.

### M3 estate routing (resolving the deferral)

`Society::remove_agent` on an M3 agent (G8a): drain the agent's ledger
balance (specie) into the returned `Estate` (conserved — the ledger's public
specie total is unchanged in aggregate; it moves from the dead agent to the
estate/heir/commons), then free + reconcile as G4a does. This replaces G4a's
graceful refusal with actual M3 settlement; the lab never triggers it
(goldens safe).

## Acceptance Tests

`sim/tests/g8a_m3_money.rs` (+ econ unit tests):

1. `m3_settlement_run_is_deterministic` — same `(seed, config)` →
   byte-identical run on M3 money.
2. `m3_settlement_conserves` — whole-system conservation spans the M3 ledger
   (specie) + goods every econ tick; the M3 ledger's own conservation holds;
   no specie/good created or destroyed.
3. `m3_specie_is_economically_equivalent_to_m1` — an M3 specie settlement
   (no banks/fiat) produces the same economic outcomes (trades, prices,
   provisioning) as the M1 equivalent — M3 here is M1, ledger-accounted.
4. `m3_death_routes_ledger_balance_to_estate` — a death with an M3 specie
   balance settles: the balance drains to the estate/heirs (conserved), the
   slot frees, caches reconcile; `remove_agent` no longer refuses a funded
   M3 agent. (The G4a deferral, resolved.)
5. `m3_birth_endows_from_ledger` — a birth's endowment is a conserved M3
   ledger transfer (from household/commons), not a mint.
6. `m3_money_has_no_fiat_or_claims` — the M3 composition is pure specie
   (fiat, demand claims, reserves all zero) — banks/fiat are G8b/c.
7. `econ_unchanged` — full workspace suite; six econ goldens byte-identical;
   all prior G1–G7 tests green; `cargo clippy --workspace --all-targets -- -D
   warnings`; `cargo fmt --check`.

Manual check:

```bash
cargo test -p sim
cargo test -p econ          # M3-removal unit tests
cargo run -p viewer -- run m3-settlement --ticks 40   # M3 specie composition
```

## Handoff Notes

- REUSE econ's M3 market/ledger behavior unchanged; G8a routes the SIM's
  flows through the M3 ledger and resolves runtime M3 removal — it does not
  change how M3 clears markets. The six goldens are byte-identical by
  construction (M3-removal is game-only); test 7 is the tripwire.
- The M3-estate resolution is the deferred G4a/b piece: `remove_agent` (M3)
  drains the ledger balance into the `Estate`, conserved — replacing the
  graceful refusal. Test 4 is the tripwire.
- M3 specie with no banks/fiat IS M1, ledger-accounted (test 3) — that
  equivalence is the proof the wiring is correct before banks/fiat add real
  M3 behavior (G8b/c).
- Conservation now spans the M3 ledger (specie/fiat/claims/reserves) + goods;
  every sim flow is a ledger move, not an `Agent.gold` mutation. Test 2.
- Scope: specie-only M3. Banks/credit (G8b), fiat/regime/tender/tax (G8c),
  and the Credit/Modern era rungs are deferred — do NOT pull them in.
- `git add` new files; gitignore stray build artifacts.
