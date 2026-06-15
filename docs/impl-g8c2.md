# Implementation Spec G8c-2: tender policies (the acceptance levers)

## Purpose

G8c-1 gave the game the credit cycle. G8c-2 adds the **tender policies** the
lab built across M11–M17 — explicit rules for *which media must be accepted*
on each settlement surface (spot exchange, debt discharge, bank-loan and
issuer repayment, and **labor wages**) — as sim policy levers. The headline
ties directly back to G8c-1: **wage tender gates the credit cycle.** When fiat
wages must be accepted, fiat credit transmits into the boom; when wages are
specie-only, the same credit issuance is *inert* — no boom, no bust. This is
the lab's M17 result, now in the spatial cycle, as a player-facing policy
choice.

It is NOT tax receivability (the state's counter-lever — G8c-3), NOT a
multi-seed study, and NOT a change to econ's tender behavior (the six goldens
stay byte-identical — the M11–M17 machinery is reused unchanged).

## Verified Base Facts (2026-06-15, oikos @ `28212f6`, 1013 tests green)

1. **econ has the full tender layer** (money.rs / scenario.rs):
   `PublicSpotTender`, `LaborWageTender`, `PublicDebtTender` (and the
   bank-repayment / issuer-repayment tenders), with `SetPublicSpotTender` /
   `SetLaborWageTender` / etc. events. The lab proved each
   refusal-vs-acceptance pair (M11–M17). G8c-2 REUSES them; it adds no tender
   logic to econ.
2. **G8c-1 gives the credit cycle** on the spatial M3+banks+fiat settlement.
   The M17 result — wage tender gating whether fiat credit transmits to a
   boom — composes directly: route the settlement's wage settlement through
   `LaborWageTender`, and the cycle fires or doesn't accordingly.
3. **The lab's M17 pair** (`fiat-wage-legal-tender` boom vs
   `fiat-wage-refusal` no-cycle) is the template for the headline
   demonstration + control.
4. **Conservation/determinism** inherited: tender policies gate *which media
   settle*, never create/destroy money; the M3 ledger conserves; determinism
   preserved.
5. **Goldens byte-identical**: tender logic reused unchanged; applying it as a
   sim policy lever is game-only.

## Milestone Boundary

G8c-2 includes:

- the tender policies as sim config levers (`SetPublicSpotTender`,
  `SetLaborWageTender`, `SetPublicDebtTender`, bank/issuer-repayment
  tenders): each gates which media settle its surface, routed through the
  existing econ tender machinery (config-set; player-`Command` is G9);
- the **headline**: `LaborWageTender` gates the G8c-1 credit cycle —
  `FiatAndSpecie`/`ParAll` wages → fiat credit transmits → boom→bust;
  `SpecieOnly` wages → the same credit issuance is inert → no boom, no bust;
- the other surfaces wired as the same policy mechanism (spot/debt/repayment
  tenders gate their media), each with its refusal-vs-acceptance behavior;
- a `wage-tender-cycle` config (fiat wages → cycle) and a
  `wage-refusal-cycle` control (specie-only wages → no cycle) — the headline
  falsification twin tying tender to the cycle;
- conservation (tender changes media composition, never totals); determinism;
- viewer surfacing of the active tender policies + the cycle outcome;
- acceptance tests in `sim/tests/g8c2_tender.rs`;
- README + `engine-divergence.md` (tender policies as levers; the wage-tender
  ×cycle result; tax receivability is G8c-3).

G8c-2 excludes:

- no tax receivability (G8c-3); no multi-seed study;
- no player-`Command`/UI tender setting (config-set here; G9);
- no change to econ tender BEHAVIOR — six goldens byte-identical; game-only
  wiring;
- no `HashMap` in logic; nothing drawn; magnitudes SIGN only (cycle fires vs
  inert) + exact conservation.

## Domain Semantics

### Tender as media-acceptance gates

Each tender policy names which media (specie / fiat / bank claims) must be
accepted to settle its surface. The sim routes that surface's settlement
through the policy's `accepted_media()` (the existing econ machinery): a
refused medium cannot settle there even if held. Tender changes *composition*
(which medium pays), never *totals* (no money created/destroyed).

### The headline: wage tender gates the cycle

`LaborWageTender` decides whether a wage can be paid in fiat. In the G8c-1
credit cycle, the fiat-credit borrowers (would-be employers) hold fiat:

- **`FiatAndSpecie`/`ParAll` wages**: fiat wages settle → fiat credit reaches
  workers → demand → the boom transmits → (stop) → bust. The cycle fires.
- **`SpecieOnly` wages**: fiat wages are refused → the fiat-credit employers
  cannot hire → the credit never enters the real economy → no boom, no bust.
  The same credit issuance is *inert*.

This is the lab's M17 (`calculation_degrades`/wage-tender) result, now in the
spatial cycle: the wage surface is the transmission valve from credit to the
structure of production.

## Acceptance Tests

`sim/tests/g8c2_tender.rs` (+ unit tests):

1. `tender_run_is_deterministic` — same `(seed, config)` → byte-identical.
2. `fiat_wages_transmit_the_cycle` — `wage-tender-cycle` (fiat wages): the
   G8c-1 cycle fires (gap>0, boom, stop, bust, capital consumed).
3. `specie_only_wages_render_credit_inert` — `wage-refusal-cycle` (specie-only
   wages): the same fiat-credit issuance produces NO boom and NO bust — the
   credit never transmits. Paired with test 2, shows the wage surface is the
   transmission valve. (Sign only.)
4. `tender_gates_media_not_totals` — across the tender surfaces, a refused
   medium does not settle that surface (even if held), and the active medium
   does; money totals are unchanged by the policy (composition, not creation).
5. `spot_and_debt_tenders_gate_their_surfaces` — `PublicSpotTender` /
   `PublicDebtTender` (and the repayment tenders) each enforce their
   refusal-vs-acceptance on their surface (the M11–M14 results, in the sim).
6. `tender_conserves` — whole-system conservation holds under every tender
   policy (composition changes only).
7. `econ_unchanged` — full workspace suite; six econ goldens byte-identical;
   all prior G1–G8c-1 tests green; `cargo clippy --workspace --all-targets -- -D
   warnings`; `cargo fmt --check`.

Manual check:

```bash
cargo test -p sim
cargo run -p viewer -- run wage-tender-cycle --ticks 80    # fiat wages -> cycle
cargo run -p viewer -- run wage-refusal-cycle --ticks 80   # specie wages -> inert
```

## Handoff Notes

- REUSE econ's tender machinery (`accepted_media()` + the `SetXTender`
  events) unchanged; G8c-2 routes each settlement surface through its tender
  policy. Six goldens byte-identical (game-only); test 7 is the tripwire.
- The headline is wage tender × the G8c-1 cycle: fiat wages transmit the
  cycle, specie-only wages render the same credit inert (tests 2+3). The
  control is the proof the wage surface is the transmission valve — if the
  cycle fires under specie-only wages, the wage gate isn't actually routing
  settlement, fix that.
- Tender gates COMPOSITION (which medium settles), never TOTALS (no money
  created/destroyed) — test 4. Conservation holds under every policy (test 6).
- Scope: the tender surfaces (M11–M17) as levers + the wage×cycle headline.
  Tax receivability (the state's counter-lever, M21) is G8c-3; player-`Command`
  tender setting is G9.
- `git add` new files; gitignore stray build artifacts.
