# Implementation Spec G8c-1: fiat, the regime ladder, and the credit cycle

## Purpose

This is the climax of the economic engine: the **Austrian business cycle**, in
the colony game, from spatial first principles. G8a put the sim on M3 ledger
money; G8b added banks and fiduciary credit. G8c-1 adds **fiat** and the
**regime ladder** (SoundGold → FractionalConvertible → SuspendedConvertibility
→ Fiat), and demonstrates the cycle the lab proved (`emerged-gold-fiat-credit-
expansion`): cheap credit drives the market rate **below** the credit-disabled
shadow natural rate (a measured **gap**), capitalists over-invest in
roundabout production (the **boom**), credit **stops**, the rate reasserts, the
malinvested projects are **abandoned**, and **capital is consumed** (the
**bust**) — against a **sound-money control** that shows no gap and no cycle.

It also unlocks the **Credit** and **Modern** era rungs the G6a detector
deferred to G8.

G8c is sliced: **G8c-1 (this milestone) = fiat + regime ladder + the credit
cycle + the sound-money control.** G8c-2 = the tender policies (M11–M17) and
tax receivability as player levers. It is NOT the tender/tax layer (G8c-2),
NOT a multi-seed robustness study (deferred), and NOT a change to econ's
ABCT/regime/shadow behavior (the six goldens stay byte-identical — that
machinery is reused unchanged).

## Verified Base Facts (2026-06-15, oikos @ `675e6f6`, 993 tests green)

1. **econ has the complete ABCT machinery** (scenario.rs / issuer.rs /
   record.rs): `EventKind::SetRegime(Regime)` (the ladder),
   `FiatPrint`, `SetIssuerPolicy` / `StopIssuerCredit`, the boom/bust /
   abandonment / capital-consumption records, and the **credit-disabled
   shadow counterfactual** (the authoritative natural-rate benchmark). The
   lab's `emerged_gold_fiat_credit_expansion` is the headline cycle
   (gap-opens → boom → stop → bust → capital consumed). G8c-1 REUSES all of
   it; it adds no ABCT/regime/shadow logic to econ.
2. **G8a/G8b put the sim on M3 + banks**: the settlement runs ledger money
   with fiduciary credit. G8c-1 adds the regime (gating fiduciary/fiat) and
   the issuer (fiat printing / fiat-credit) as sim policy, and runs the
   shadow counterfactual to measure the gap.
3. **The shadow is the authoritative signal** (lab doctrine): the cycle's
   gap is `shadow_natural_rate − market_rate` (credit-disabled replay); the
   bust is the cluster of individually-rational abandonments when credit
   stops. Both are measured, never set.
4. **Roundabout production exists** (G3): the chain (grain→flour→bread, +
   tier-2) gives the structure that lengthens in the boom and shortens in the
   bust; capital consumption reuses the M2/M3 abandonment machinery.
5. **Goldens byte-identical**: the ABCT/regime/shadow logic is reused
   unchanged; running it in the spatial sim is game-only. Determinism
   inherited.

## Milestone Boundary

G8c-1 includes:

- the **regime ladder** as sim policy (`SetRegime`: SoundGold →
  FractionalConvertible → SuspendedConvertibility → Fiat), gating fiduciary
  and fiat;
- **fiat** issuance (the issuer prints fiat / extends fiat-credit) entering
  the sim economy under the Fiat regime;
- the **shadow counterfactual** run for the settlement: the credit-disabled
  natural rate, and the measured `gap = shadow_natural_rate − market_rate`;
- the **credit cycle demonstration** on a curated config: cheap credit →
  gap>0 → boom (roundabout structure lengthens / over-investment) → `stop`
  → rate reasserts → malinvested projects abandoned → capital consumed;
- a **sound-money control** (SoundGold / 100% reserve, no fiat): gap ≈ 0,
  no boom, no bust — the falsification twin;
- the **Credit / Modern era rungs** in the G6a detector unlock here (chartered
  bank credit circulating → Credit; state fiat the marginal medium → Modern);
- conservation across fiat issuance/retirement (the M3 ledger; fiat base =
  issued − retired); determinism;
- viewer surfacing: regime, the shadow gap, boom/bust indicators, capital
  consumed;
- acceptance tests in `sim/tests/g8c1_cycle.rs`;
- README + `engine-divergence.md` (the cycle in the game; tender/tax G8c-2;
  multi-seed robustness deferred).

G8c-1 excludes:

- no tender policies (M11–M17) or tax receivability (G8c-2);
- no multi-seed robustness STUDY of the cycle (deferred, like the lab's
  sweep was a separate milestone);
- no change to econ ABCT/regime/shadow BEHAVIOR — six goldens byte-identical;
  any econ edit additive/game-only;
- no `HashMap` in logic; nothing drawn; magnitudes are SIGN/direction only
  (gap>0 vs ≈0; abandonments>0 vs 0; capital consumed>0 vs 0) — the lab's
  direction-not-magnitude discipline; conservation is exact.

## Domain Semantics

### The regime ladder + fiat

`SetRegime` walks SoundGold → FractionalConvertible → SuspendedConvertibility
→ Fiat (reusing econ's `Regime`). The regime gates credit:
fiduciary/fiat capacity is zero under SoundGold/full-reserve and opens as the
ladder descends. Under Fiat, the issuer prints fiat / extends fiat-credit
into the sim economy (the M3 ledger tracks fiat base = issued − retired). All
reused from econ; G8c-1 routes the sim's policy/issuance into it.

### The shadow gap and the cycle

The settlement runs a credit-disabled **shadow** replay to get the natural
rate; `gap = shadow_natural_rate − market_rate`. Cheap credit pushes the
market rate below the natural rate (**gap > 0**) — the live ABCT warning.
Capitalists, seeing cheap credit, start longer roundabout projects (the
measured structure lengthens — the **boom**). When credit **stops**
(`StopIssuerCredit` / regime tightening), the rate reasserts toward the
natural rate, the malinvested long projects no longer pencil out, and they
are **abandoned** — non-convertible embodied capital is **consumed** (the
**bust**), reusing the M2/M3 abandonment + capital-consumption machinery. The
bust is a cluster of individually-rational abandonments, not a global trigger.

### The sound-money control

The control runs SoundGold / 100% reserve with no fiat: fiduciary/fiat
capacity is zero, so `gap ≈ 0`, no boom, no abandonments, no capital
consumed — the falsification twin proving the cycle comes from credit
expansion, not from the production/spatial dynamics themselves.

## Acceptance Tests

`sim/tests/g8c1_cycle.rs` (+ unit tests):

1. `cycle_run_is_deterministic` — same `(seed, config)` → byte-identical run
   through boom, stop, and bust.
2. `fiat_credit_opens_a_shadow_gap` — under fiat credit, the market rate
   falls below the credit-disabled shadow natural rate: `gap > 0` during the
   boom. (Sign only.)
3. `expansion_then_stop_busts_and_consumes_capital` — boom (roundabout
   structure lengthens above the shadow baseline) → `stop` → malinvested
   projects abandoned (`abandonments > 0`) → `capital_consumed > 0`.
4. `sound_money_control_has_no_cycle` — the falsification twin: SoundGold /
   100% reserve, no fiat → `gap ≈ 0`, no boom, zero abandonments, zero
   capital consumed. Paired with tests 2–3, isolates the cycle to credit
   expansion.
5. `fiat_conserves` — fiat issuance/retirement conserves in the M3 ledger
   (fiat base = issued − retired; a default leaves the money stock changed by
   rule, not by leak); whole-system conservation holds across the cycle.
6. `credit_and_modern_eras_unlock` — the G6a detector reaches the **Credit**
   era when chartered bank credit circulates and **Modern** when state fiat
   is the marginal medium (measured, with hysteresis).
7. `econ_unchanged` — full workspace suite; six econ goldens byte-identical;
   all prior G1–G8b tests green; `cargo clippy --workspace --all-targets -- -D
   warnings`; `cargo fmt --check`.

Manual check:

```bash
cargo test -p sim
cargo run -p viewer -- run credit-cycle --ticks 80     # gap opens, boom, stop, bust
cargo run -p viewer -- run sound-money --ticks 80       # the control: no cycle
```

## Handoff Notes

- REUSE econ's ABCT/regime/shadow machinery unchanged; G8c-1 routes the sim's
  regime/issuance into it and runs the shadow replay for the settlement. Six
  goldens byte-identical (game-only); test 7 is the tripwire.
- The shadow counterfactual is the AUTHORITATIVE signal (credit-disabled
  natural rate); the gap and the bust are MEASURED, never set (lab doctrine).
- The sound-money control (test 4) is the proof the cycle is credit-driven,
  not an artifact of the spatial/production dynamics. If the control busts,
  the cycle isn't coming from credit — fix that, don't weaken the test.
- Magnitudes are SIGN only (gap>0 vs ≈0, abandonments/capital-consumed >0 vs
  0); conservation is exact (fiat base = issued − retired). The cycle config
  may need cast/credit sizing to fire (the lab's M3 scenario-sizing mandate);
  tune the config, never the assertions.
- Scope: fiat + regime ladder + the cycle + control. Tender policies (M11–M17)
  and tax receivability are G8c-2; the multi-seed robustness study is deferred.
- The Credit/Modern era rungs (G6a-deferred) unlock here.
- `git add` new files; gitignore stray build artifacts.
