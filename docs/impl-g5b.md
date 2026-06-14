# Implementation Spec G5b: emergence composed with the full stack

## Purpose

Each emergent phenomenon has been proven in isolation: money emerges from
spatial barter (G5a), production roles emerge from price spreads (G3b),
population sustains under demographic selection (G4b). G5b is the
**integration** milestone — one settlement where **all three happen
together**: a barter camp where a money good emerges, then producers take up
milling/baking from the resulting price spreads, while births and deaths run
demographic selection — all conserving and deterministic.

This proves the simulation composes: the whole economic foundation
(G1 needs → G2 space/trade → G3 production → G4 demography → G5a money) runs as
one coherent society, not as separate demos.

It is NOT new mechanism (every piece is reused), NOT the multi-seed spatial
robustness study (deferred, like M18/M19), NOT multi-settlement composition
(later), and NOT a change to econ behavior (the six goldens stay
byte-identical).

## Verified Base Facts (2026-06-14, oikos @ `55e701d`, 899 tests green)

1. **`SettlementConfig` fields already compose** (settlement.rs): `demography:
   Option<DemographyConfig>` is an overlay; `BarterConfig` drives G5a
   emergence; production roles come from the chain/role config. They are
   independent fields, not mutually-exclusive variants — the *structure*
   supports combination. No existing config enables all three at once
   (`barter_camp` has no demography/production; `emergent_chain` has no
   barter/demography; `lineages` has no barter/production).
2. **The phase order exists** in the econ tick: G5a runs barter → promotion →
   money market; G3b's role-choice runs after scale regeneration; G4b's
   births/deaths run in the demography step. G5b orders all of them coherently
   in one tick and proves they coexist.
3. **Conservation is established per-flow** (G2b transfer, G3a recipe
   transformation, G4 birth/death transfer, G5a barter swap + promotion
   conversion). G5b's whole-system invariant must hold with ALL flows active
   simultaneously.
4. **Goldens byte-identical by construction**: the lab uses none of these
   overlays; the combined path is game-only. The no-overlay paths stay
   structurally unchanged.
5. **Determinism**: integer, `Rng` at generation only, nothing in the loops,
   deterministic mutation/role-choice/promotion; `BTreeMap`/`Vec`, no
   `HashMap`. The combined run must be byte-identical across two invocations.

## Milestone Boundary

G5b includes:

- a combined `frontier` config: barter-start emergence + production-role
  emergence + demography overlay, in one settlement;
- coherent econ-tick phase ordering so all three coexist (barter/promotion;
  role-choice once money prices exist; births/deaths) — fixing whatever
  interaction bugs the combination surfaces (e.g. role-choice or births during
  the barter phase; promotion interacting with active producers or a birth/
  death on the promotion tick);
- whole-system conservation with ALL flows active (barter swap, promotion
  conversion, recipe transformation, birth/death transfer) simultaneously;
- viewer `frontier` scenario surfacing the phase, money good, producer roles,
  and population together;
- acceptance tests in `sim/tests/g5b_frontier.rs`;
- README + `engine-divergence.md` (the composed society; robustness study and
  multi-settlement composition deferred).

G5b excludes:

- no new mechanism (every piece reused — G5b is composition + a combined
  config + interaction fixes);
- no multi-seed robustness STUDY (deferred);
- no multi-settlement composition (the Region with all overlays is later);
- no change to econ behavior — six goldens byte-identical; any econ edit
  additive;
- no `HashMap` in logic; nothing drawn in the loops; no asserted magnitudes
  beyond all-three-fire and exact conservation.

## Domain Semantics

### The combined econ tick

```
frontier econ_tick():
  FAST: world gather/haul (physical only).
  TRANSFER: arrived goods cross into econ stock.
  EXCHANGE: if pre-promotion -> spatial BARTER + saleability + promotion check
            (G5a); if post-promotion -> money market (G2b).
  PRODUCTION: producers apply recipes (G3a); unassigned colonists appraise and
              adopt miller/baker roles from spreads (G3b) — only meaningful
              once money prices exist (post-promotion), so role-choice is
              gated on the money phase.
  DEMOGRAPHY: needs update; old-age + starvation deaths via remove_agent (G4a);
              births via add_agent into households (G4b); estates to heirs.
  MEASURE: whole-system conservation over every pool + every flow.
```

Key interaction rules (the integration's correctness core):
- **role-choice is gated on the money phase**: appraising a flour−grain spread
  needs realized money prices, which exist only post-promotion. Pre-promotion
  (barter) there are no money spreads, so no production roles emerge yet —
  production roles emerge AFTER money does, which is the correct economic
  ordering (a division of labor presupposes a medium of exchange).
- **births/deaths conserve through every phase**, including on the promotion
  tick (a birth/death the same tick a good monetizes must still conserve).
- **the veto list** (G5a: node/renewable/demography-provision goods cannot be
  promoted) now genuinely matters, since demography is active — a demography-
  provisioned staple must not become money.

### Conservation with all flows

Per good X and gold, the whole-system delta is the sum of every accounted
flow: harvest/regen (+), recipe output (+) / input (−), consumption (−),
barter swap (relocation, net 0), promotion conversion (good→money, exact),
birth endowment (transfer), death estate (transfer). Every flow already
conserves individually (G2b/G3a/G4/G5a); G5b asserts they conserve together.

## Acceptance Tests

`sim/tests/g5b_frontier.rs` (+ unit tests):

1. `frontier_run_is_deterministic` — same `(seed, config)` → byte-identical
   run through barter, promotion, money, production, and demography.
2. `all_three_emergences_fire` — in one `frontier` run: a money good is
   promoted (G5a), at least one producer role is adopted afterward (G3b), and
   births and deaths both occur with population sustained (G4b).
3. `production_roles_emerge_only_after_money` — no production role is adopted
   during the barter phase; roles appear only post-promotion (the economic
   ordering: division of labor follows a medium of exchange).
4. `frontier_conserves_with_all_flows` — whole-system conservation holds every
   econ tick with barter, promotion, recipes, births, and deaths all active;
   no unit/coin unaccounted at any boundary, including a birth/death on the
   promotion tick.
5. `demography_provision_good_cannot_monetize` — a demography-provisioned
   staple is vetoed from promotion (G5a's veto list, now exercised because
   demography is active); money emerges on a non-renewable good or not at all.
6. `frontier_sustains` — the combined society runs many econ-years without
   collapse (money emerged, producers working, population in a band); smoke/
   sign, deterministic.
7. `econ_unchanged` — full workspace suite; six econ goldens byte-identical;
   all G1/G2*/G3*/G4*/G5a tests green; `cargo clippy --workspace --all-targets
   -- -D warnings`; `cargo fmt --check`.

Manual check:

```bash
cargo test -p sim
cargo run -p viewer -- run frontier --ticks 80   # money emerges, then roles, with demography
```

## Handoff Notes

- G5b is COMPOSITION, not new mechanism: reuse G5a emergence, G3b role-choice,
  G4b demography unchanged; the work is ordering them coherently in one tick,
  a combined config, and fixing interaction bugs the combination surfaces.
- The economic ordering is load-bearing: production roles emerge only AFTER
  money (a division of labor needs a medium of exchange) — gate role-choice on
  the money phase. Test 3 is the tripwire.
- Conservation must hold with ALL flows active simultaneously, including the
  awkward coincidences (a birth/death on the promotion tick). Test 4 is the
  tripwire.
- The demography-provision veto (G5a) finally bites here (demography is
  active); test 5 guards it.
- econ goldens byte-identical (overlays are game-only); determinism; nothing
  drawn in the loops.
- The multi-seed robustness study and multi-settlement composition remain
  deferred (note in `engine-divergence.md`).
- `git add` new files; gitignore stray build artifacts.
