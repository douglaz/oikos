# OIKOS

Working title. A colony sim / civ builder that runs from a stone-age founding
band to an advanced financialized civilization, built around an
emergent-economy engine forked from the [praxsim](../praxsim) laboratory.

The design constitution is [`docs/game-spec.md`](docs/game-spec.md)
(revision 2, reviewed). The short version of the pitch: the first colony
builder where the economy is real — prices discovered by actual trades
between colonists, money that *emerges* from barter (a different money good
per map), capital structure that lengthens because colonists actually saved,
and late-game financial crises that follow from the player's own monetary
policy by causal necessity.

## Workspace

```
econ/    the economy engine — fork of praxsim-core (pure std, deterministic)
docs/    the game spec and design documents
```

Future crates per the spec's §4.1: `world/` (map, movement, stockpiles),
`life/` (needs, demography), `content/` (data-driven goods/recipes/tech),
`sim/` (orchestrator, two-rate loop, commands), `ui/` (Bevy client),
`tools/` (headless runners, balance CI). They arrive with their milestones
(G1, G2, …) — empty scaffolding is not kept ahead of need.

## Provenance and the lab relationship

`econ/` was forked at praxsim commit `0729227` (post-M21: the full M0–M21
lab ladder — ordinal value scales, CDA markets, Mengerian money emergence,
banking/fiat/ABCT, the eight-surface tender thread including tax
receivability, and the emergence-robustness instrument with the adopted
M20 envelope). The fork carries the lab's complete test suite as the
engine's **conformance suite**, including the four byte-exact series
goldens (M0/M1/M2/M3) and the M18/M20 emergence goldens — these are the
proof that the fork preserves behavior, and they gate every change to
`econ/` until the engines deliberately diverge (game-spec §10.1).

The praxsim repo stays alive as the clean-room: new economic *mechanisms*
are proven there against its invariant suite, then ported here
(game-spec §13).

## Status: G0b (engine migrations) — complete

Per game-spec §11. G0a forked the lab verbatim; G0b is the first deliberate
divergence — three migrations, each behind a compatibility surface that keeps
the lab goldens byte-identical. Every deliberate divergence is recorded in
[`docs/engine-divergence.md`](docs/engine-divergence.md).

G0a (fork):

- [x] fork `praxsim-core` → `econ`; conformance suite green; lab goldens
      replay byte-identical through the fork
- [x] `aggregate_input_goods` O(N²) scan → order-preserving index map
      (identical output by construction; golden-guarded)
- [ ] per-tick provisioning recompute (Concern-5) — DEFERRED: a real
      caching refactor, not a drop-in; belongs with the G2 perf budget
- [ ] tape retention design — DEFERRED to the inspector/WorldView work
      (G2): an in-memory ring buffer changes test-visible tape contents,
      so it is not a behavior-preserving G0a change; the design decision
      is recorded here rather than smuggled in

G0b (migrations behind compatibility):

- [x] dynamic `GoodRegistry` — goods become data; `lab_default()` interns the
      exact lab set in the exact id order; the `GoodId` constants and
      `good_name` stay as lab-compat surface
- [x] generational `AgentId` — `u32 → u64` packing `(generation, index)`;
      generation-0 ids are byte-identical in ordering and formatting
- [x] `AgentArena` — stable-identity storage replacing `Vec<Agent>` +
      id-resolution; id-ordered, deterministic, no `HashMap`; slot reuse and
      generation bumping unit-tested (no engine path frees yet)
- [x] `Command` result/error semantics — additive `apply_command` returning
      `Applied | Rejected(reason)`, sharing the event path's logic; the
      scenario event path keeps its silent-tolerance semantics

The conformance suite stays green natively and all goldens are byte-identical;
see `econ/tests/g0b_engine_migrations.rs` for the migration acceptance tests.

## Build and test

```bash
cargo test          # full conformance suite incl. goldens
cargo clippy --workspace --all-targets -- -D warnings
cargo fmt --check
```
