# Implementation Spec G3b: production roles emerge from price spreads

## Purpose

G3a proved the grain→flour→bread chain *operates* with **seeded** producer
roles. G3b removes the seeding: a colonist **chooses** to mill or bake because
the realized price spread pays — entrepreneurship, the praxeology-honest way
(ordinal appraisal against its own value scale, no scalar profit-maximization
smuggled in). This is the emergence half of G3.

Scope (mirroring how the lab proved money emergence): G3b proves the
**mechanism** on a curated config with a **falsification control** — when a
profitable spread exists, idle colonists take up milling/baking and the chain
forms; when the spread is removed, they do not. A multi-seed robustness
*study* (the "≥X% of N random worlds" gate, analogous to M18/M19 for money) is
deferred to a possible G3-study milestone; G3b is the mechanism + control, not
the robustness gate.

It is NOT a robustness study over random worlds (deferred), NOT scalar
profit-maximization (the choice is ordinal, reusing M2.5 appraisal), NOT new
recipe/market logic in `econ` (reused), and NOT a change to the six econ
goldens.

## Verified Base Facts (2026-06-14, oikos @ `00cf530`, 813 tests green)

1. **The choice machinery exists in `econ`.** `instrumental_rank_for_*`
   (sim.rs:810+) ranks producing a good/tool instrumentally because a scale
   want needs it; `appraise_project_bundle` / `_for_money` (bundle.rs:42,50)
   is the M2.5 ordinal entrepreneurial appraisal (compare a build/sell bundle
   against the agent's ordinal endowment, no scalar PV). G3b REUSES these for
   the "should I become a miller?" decision — it adds no scalar optimizer.
2. **G3a wired recipes into the sim** (settlement.rs): miller/baker vocations
   apply `Recipe`s, tool-gated, with transformation conservation. G3b changes
   how a colonist ACQUIRES a vocation (from prices, not seeding); the recipe
   execution + conservation are unchanged.
3. **Realized prices are observable** per good (`realized_price`,
   `society().trades`), so a colonist can see the flour−grain spread vs the
   milling labor/tool cost — the signal driving the choice.
4. **The chain + control configs** can reuse G3a's content; the control is the
   same world with the spread removed (e.g. bread demand absent / flour and
   grain priced equally), so no chain forms.
5. **Determinism + purism** inherited: integer, `Rng` at generation only,
   nothing in the loops, ordinal choice (no scalar maximize), `BTreeMap`/`Vec`,
   no `HashMap`. The praxeology gate (decisions never read an aggregate; choice
   is ordinal) is load-bearing.
6. **Bootstrap stock is not role seeding.** The emergent config may seed a small
   flour stock on latent millers so the first baker has flour to buy and the
   middle good can realize a price; the holder remains `Unassigned` until its
   own ordinal appraisal sees the flour−grain spread. This is a mechanism
   bridge for price discovery, not a hand-placed production vocation.

## The role-choice mechanism (the milestone)

Each econ tick, an UNASSIGNED colonist (no seeded vocation) appraises the
recipes it could run against its own ordinal scale and the realized prices it
can observe, and adopts at most one vocation when the appraisal favors it:

- the appraisal is ORDINAL — reuse `appraise_project_bundle`/instrumental
  ranking: "if I acquire grain + a mill and run the mill recipe, the flour I
  can sell buys more of my wants than the grain + labor + tool it costs me"
  — compared on the value scale, never by a scalar profit number;
- a colonist adopts milling only if the flour−grain spread (realized) clears
  its milling labor's leisure cost and the tool cost on its scale; baking
  likewise on the bread−flour spread;
- vocation adoption is sticky within a run but re-appraised if the spread
  collapses (a miller with no margin reverts to gathering/idle) — so roles
  track the spread, not a one-time coin flip;
- no central assignment, no scalar argmax over colonists; each colonist
  decides for itself in id order. This is the §pillar-1 "colonists act"
  rule applied to occupation.

## Milestone Boundary

G3b includes:

- ordinal role-choice in `sim`: unassigned colonists appraise and adopt
  miller/baker vocations from realized spreads (reusing `econ`'s
  `appraise_project_bundle`/instrumental ranking — no scalar optimizer);
- re-appraisal so a role reverts when its spread collapses;
- an `emergent-chain` config (no seeded producer roles) and a
  `flat-prices`/no-spread control config (the falsification twin);
- the chain forming from prices in `emergent-chain` and NOT forming in the
  control — the DoD;
- `engine-divergence.md` note: ordinal entrepreneurship for occupation,
  robustness study deferred;
- a viewer `emergent-chain` scenario (read-only) showing roles adopted +
  the three prices;
- acceptance tests in `sim/tests/g3b_emergence.rs`;
- README updates.

G3b excludes:

- no multi-seed robustness study / "≥X% of N worlds" gate (deferred to a
  G3-study milestone, like M18/M19 were for money emergence);
- no scalar profit-maximization (ordinal appraisal only — purism);
- no new `econ` recipe/market/appraisal logic (reuse);
- no demography (G4); no change to econ goldens (byte-identical); only
  additive `econ`/`sim` accessors;
- no `HashMap` in logic; nothing drawn in the loops; no asserted price
  magnitudes (role-formation is a presence/sign claim).

## Acceptance Tests

`sim/tests/g3b_emergence.rs` (+ unit tests):

1. `emergent_chain_run_is_deterministic` — same `(seed, config)` →
   byte-identical run.
2. `roles_emerge_from_the_spread` — in `emergent-chain` (no seeded roles),
   over a run at least one colonist adopts milling and at least one adopts
   baking, and bread is produced and consumed — the chain forms from prices
   alone.
3. `no_spread_no_roles` — the falsification control: with the spread removed
   (flat prices / no bread demand), NO colonist adopts a production vocation
   and no flour/bread is produced. Paired with test 2, this shows the spread
   is what creates the roles.
4. `role_choice_is_ordinal_not_scalar` — the adoption decision routes through
   the ordinal appraisal (reused `appraise_project_bundle`/instrumental
   rank); a unit test asserts a colonist declines a vocation whose output
   does not outrank its costs on its scale, and adopts when it does — no
   scalar profit threshold.
5. `role_reverts_when_spread_collapses` — a miller whose flour−grain spread
   collapses mid-run re-appraises and stops milling (roles track the spread).
6. `emergent_chain_conserves` — transformation conservation (G3a) still holds
   exactly under emergent roles.
7. `econ_unchanged` — full workspace suite; six econ goldens byte-identical;
   all G1/G2*/G3a tests green; `cargo clippy --workspace --all-targets -- -D
   warnings`; `cargo fmt --check`; the praxeology source-gate (no decision
   module reads an aggregate) still holds.

Manual check:

```bash
cargo test -p sim
cargo run -p viewer -- run emergent-chain --ticks 40
cargo run -p viewer -- run emergent-chain-control --ticks 40
```

## Handoff Notes

- The choice is ORDINAL — reuse `appraise_project_bundle`/instrumental
  ranking; do NOT add a scalar profit-maximizer or an argmax over colonists.
  Each colonist decides for itself in id order (pillar 1). Test 4 guards this.
- The control (test 3) is the proof: roles must NOT form without the spread.
  If both form roles, the mechanism is reading something other than the
  spread — fix that, don't weaken the test.
- This is the MECHANISM milestone; the multi-seed robustness gate is a
  deferred study (note it in `engine-divergence.md`). Do not chase a
  "≥X% of N seeds" number here.
- Reuse G3a's recipes/conservation unchanged; G3b only changes how a
  vocation is acquired.
- econ goldens byte-identical; determinism + purism source-gate intact.
- `git add` new files; gitignore stray build artifacts.
