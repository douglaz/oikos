# Implementation Spec: marketability — the durability/carry-cost lever (S21a)

> Slice 1 of the open-colony capstone, and a *prerequisite* for it (Codex): S20 fixed the *clearing*
> (the medium can round-trip), but not the *saleability race*. S18 showed that with on-market survival
> the universal necessity (food/WOOD, wanted by everyone every tick) **dominates saleability**, so the
> token never leads — and two-lane clearing is **orthogonal** to that. In reality money beats
> necessities not by topology but by **marketability**: it is durable, low-carrying-cost, less
> perishable. S21a adds that lever — *physically*, not as a taste — and demonstrates in a controlled
> barter setting that a **durable token (SALT) beats a perishable necessity as the indirect medium**.
> Without this slice, an open colony just reruns S18 with a better matcher.

## The crux (from research)

Today the model has **no time-between-acceptance-and-re-trade**, so durability cannot matter:

- **Saleability is purely acceptance-count share** (`econ/src/menger.rs:247`): a good's share = its
  accepted-trade count / total; `provisional_leader`/`leader_shares`/the strong-bar gate read only
  counts, distinct acceptors, distinct counterparts — **no durability/holding-cost term**. So a
  perishable but heavily-traded necessity ranks *more* saleable than a durable token, purely on volume.
- **The indirect-acceptance decision is durability-blind** (`would_accept_indirect_barter_swap_with_stock`,
  `econ/src/agent.rs:477`): it is a single-tick greedy fill over a static `Stock` snapshot — it confirms
  the medium *can* re-trade for the target, but never weighs that a *perishable* medium would **spoil
  before the re-trade clears**. An agent accepts a perishable as a means exactly as readily as a durable
  one. `Stock` (`econ/src/good.rs:71`) carries no per-good durability metadata.
- **Spoilage is `sim`-only** (`run_spoilage`, `sim/src/settlement.rs:8758`): a `ChainConfig.perishable_decay_bps`
  rate decays held stock above a free-storage floor; the perishable *set* is hardcoded
  (`:8776-8789` — staple/subsistence/grain; **WOOD, SALT, tools are explicitly exempt as durable**);
  `spoiled` is a conserved sink (`:3880`). The econ layer cannot see any of this.

**The authentic lever (route b, generalized to MARKETABILITY — not a metric redefinition):** add an
**econ-owned per-good marketability table** (`MarketabilityConfig`: per-good **durability/decay** and
**carrying cost** — NOT the sim hardcoded perishable set, which is not the source of truth) and make it
**visible to the econ acceptance decision**. When an agent considers accepting good *g* **as a means**,
it applies an **explicit holding rule**: over a `hold_horizon` (the minimum ticks until plausible
re-trade), would enough of *g* survive its decay AND is it cheap enough to hold (carry cost) to be worth
taking as a medium? If not, **decline** (binary — not a down-rank; the method already returns `bool`).
So a *perishable* food (spoils before re-trade) AND a *durable-but-bulky/high-carry* necessity like WOOD
are both bad media, while a durable, low-carry token is accepted. Marketability then **drives the
acceptance counts**, so SALT wins saleability *by realized acceptances* — no "prefer SALT" want, no
change to what saleability *means* (`menger.rs` untouched). Same discipline as S8→S9 (which removed the
circular `medium_want_qty` and re-derived emergence from real saleability). **This addresses the WOOD
problem (Codex P1b):** durability alone wouldn't (WOOD is durable + universally wanted) — but a *carry
cost* makes a bulky necessity a poor medium even though it doesn't spoil.

## Purpose & the honest bar

On a gated path (`durability_aware_acceptance`, default off): plumb an econ-owned per-good
**marketability** table (durability/decay + carrying cost) into the acceptance decision and apply an
explicit **holding rule** — over a `hold_horizon`, would enough of a prospective medium survive its
decay and be cheap enough to hold to be worth taking as a means? Demonstrate, in a **controlled econ
`MarketScenario` barter setting** (the canonical SALT-emergence microtest's home), that when a
necessity is universally wanted **but a bad medium** (perishable, or durable-but-high-carry like WOOD)
while SALT is **durable + low-carry**: with the lever ON, **SALT wins the saleability-leader race**;
with it OFF, the necessity dominates (the S18 reproduction). Honest target: **test whether** physical
marketability — not taste — lets a token out-saleability a wanted-but-poorly-marketable necessity.

Authenticity is the whole point:
- The token wins because it **holds value to re-trade**, a *physical* property — NOT a configured
  preference. No `medium_want`/"prefer SALT" is added; the saleability metric's *definition*
  (acceptance share) is unchanged; only the *acceptance behavior* gains spoilage-risk awareness.
- Controls must show the lever and the durability are load-bearing: lever OFF → necessity dominates;
  make SALT *also* perishable → it loses its edge.

Principled-failure modes, all first-class (and possible):
- **Durability can't tip it** — even penalizing perishable-as-means, the necessity's sheer volume keeps
  it the saleability leader (durability is too weak a lever in pure acceptance-count terms).
- **The lever becomes a disguised preference** — the only way to make SALT win is a penalty so strong
  it is effectively "refuse all non-SALT media," i.e. taste-smuggling. That would be a finding (the
  honest lever isn't enough), not a pass.
- **Cross-crate plumbing is not clean** — durability can't be made visible to `econ` without coupling
  that breaks layering or determinism.
Land `marketability_finding` with the characterized reason; do NOT force the pass with a "prefer SALT"
want or a metric redefinition.

NOT the open colony itself (S21b/c — survival on-market + terminal consumption compose later), NOT a
new money rule (the strong-bar gate is unchanged), NOT mortality. Additive + gated; flag off → all
S5–S20 + econ + the goldens byte-identical.

## Verified Base Facts (oikos @ `9a80bac`)

1. **Saleability = acceptance share, durability-blind** (`menger.rs:247/201/256`).
2. **Indirect-acceptance is durability-blind, returns `bool`** (`would_accept_indirect_barter_swap_with_stock`,
   `agent.rs:477-515`; single-tick, static `Stock` snapshot; binary accept; `Stock` `good.rs:71` has no
   durability). Called from barter offer generation (`society.rs:2226`/`:2276`) — the threading seam.
3. **Spoilage is sim-only** (`run_spoilage`, `settlement.rs:8758`; `perishable_decay_bps`
   `ChainConfig` `:929`; hardcoded perishable set `:8776`, **SALT/WOOD/tools exempt as "durable"** —
   so WOOD stays a durable universally-wanted good, exactly the S18-dominance risk a *carry cost*, not
   spoilage, must address; `spoiled` sink `:3880`). Econ cannot see any of it → the econ-owned table.
4. **The controlled-barter harness** is the econ `MarketScenario` (`econ/src/scenario.rs:335`,
   builders `:665`); the canonical money microtest `menger_salt_money` (`scenario.rs:1066`) +
   `econ/tests/m5_menger.rs` — hand-built agents, explicit stocks/wants, candidate goods, asserts SALT
   promotes. This is where to add the S21a demonstration. (It has no spoilage loop — fine: S21a tests
   the *acceptance* lever reading per-good durability, not the sim spoilage run, which composes in
   S21b/c.)
5. **The discipline precedent**: S8→S9 removed the circular `medium_want_qty` and re-derived emergence
   from real saleability (`strong_bar_emergence.rs:1-26`) — S21a follows the same physical-not-taste rule.

## The slices (build in order; each independently testable)

- **S21a.0 — the econ-owned marketability config + plumbing (Codex P2a).** Add an **econ-owned**
  `MarketabilityConfig`/`GoodMarketability` (per-good `decay_bps` + `carry_cost` — `carry_cost` a deterministic
  quantity-equivalent attrition/burden over the hold horizon, NOT a money price; default
  empty/durable-everything → off) carried on the scenario/`MengerianConfig`, stored on `Society`, and
  passed into the indirect-acceptance call sites (`society.rs:2226`/`:2276`). `econ` does NOT depend on
  `sim`; later `sim` translates its `ChainConfig`/spoilage policy into this econ config. Gated behind a
  `durability_aware_acceptance` flag, default off. **Test:** default config is empty → acceptance is
  byte-identical to today; the table is readable by the agent path.
- **S21a.1 — the explicit holding rule in the acceptance decision (Codex P1a, binary).** In
  `would_accept_indirect_barter_swap_with_stock` (`agent.rs:477`), when the flag is on, apply the
  holding rule to the `receive_good` (the prospective medium): over `hold_horizon` ticks, compute
  expected surviving quantity from `decay_bps` and the carry cost; **decline** (binary) if the means
  good cannot plausibly survive/afford the minimum holding period. A durable, low-carry good passes; a
  perishable or high-carry one fails. Keep `menger.rs` **unchanged**. **Test:** with the flag on, an
  agent accepts a durable low-carry good as a means but declines a perishable one AND declines a
  durable-but-high-carry one (the WOOD analogue); flag off → accepts any (durability-blind, as today);
  no `medium_want`/taste added; flag off → byte-identical.
- **S21a.2 — the controlled-barter demonstration (Codex P1c, pinned metrics — un-gameable).** Add a
  `MarketScenario` (beside `menger_salt_money`) where a universally-wanted necessity is a **bad medium**
  (perishable food; and a durable-but-high-carry WOOD-analogue) while SALT is durable + low-carry;
  candidate goods include both. **Test (assert ALL):** (i) lever OFF → the necessity LEADS the
  saleability race; (ii) lever ON → SALT leads; (iii) the necessity STILL has material **direct**
  acceptance volume (it is genuinely wanted/consumed — the scenario is not rigged to barely trade it);
  (iv) the ON-vs-OFF change is specifically a **drop in INDIRECT** acceptance of the necessity *as a
  means* (not its direct demand); (v) making SALT **also perishable/high-carry** removes its edge (the
  necessity leads again) — OR the documented `marketability_finding`.

## Acceptance Tests (the DoD) — `econ/tests/m5_*` / a new `econ/tests/marketability.rs`

1. `durability_aware_run_is_deterministic` — byte-identical `(seed/scenario)`.
2. `bad_medium_declined_good_medium_accepted` — the acceptance lever (binary): with the flag on, an
   agent accepts a **durable low-carry** good as an indirect means but declines a **perishable** one
   AND a **durable-but-high-carry** one (it can't survive/afford the `hold_horizon`); with the flag
   off, it accepts any (marketability-blind, as today).
3. `good_medium_beats_bad_medium_necessities` — **the core claim**: in the controlled scenario with a
   universally-wanted necessity that is a *bad medium* (perishable food + a high-carry WOOD-analogue)
   and a durable low-carry SALT, the lever ON makes SALT win the saleability-leader race; the necessity
   does NOT lead despite its consumption demand.
4. `it_is_marketability_not_taste` — authenticity: no `medium_want`/"prefer SALT" is configured; the
   saleability metric (`menger.rs`) is unchanged; making SALT **also perishable/high-carry** removes
   its edge (the necessity leads again) — the win is *physical marketability*, not a preference.
5. `the_necessity_still_trades_directly_and_the_drop_is_indirect` — un-gameable (Codex P1c): the
   necessity retains **material direct acceptance volume** (genuinely consumed), and the ON-vs-OFF
   difference that demotes it is a **drop in its INDIRECT (as-a-means) acceptance**, not its direct
   demand — so SALT does not win by the necessity simply not trading.
6. `lever_off_reproduces_necessity_dominance` — control: flag off → the necessity dominates saleability
   and SALT does not lead (the S18 result), so the lever is load-bearing.
7. `goldens_unchanged` — with `durability_aware_acceptance` off (and an empty marketability table),
   S5–S20 scenarios + the six econ + g5a/g5b/coemergence + demographic (`lineages`) + g4a_death goldens
   byte-identical; all suites green; `menger.rs` + the saleability metric + `Stock` layout untouched
   off-path; the flag/config in `canonical_bytes` with a regression (or scenario-local); clippy
   `-D warnings`; fmt `--check`.

(Principled-failure path: if durability can't tip the saleability race without an effectively-"prefer
SALT" penalty, or perishables still dominate, land `marketability_finding` with the characterized
reason — NOT a forced pass.)

## Missing Interactions (the central risks)

- **Don't redefine saleability; don't add a taste.** The lever is *behavioral* (acceptance weighs
  spoilage-risk-to-re-trade), not a durability term bolted onto the saleability metric and not a
  `medium_want`. `it_is_marketability_not_taste` (the SALT-also-perishable control) is the tripwire.
- **The cross-crate layering.** Durability lives in `sim` today; S21a must make a *minimal* per-good
  durability signal visible to `econ`'s acceptance path without `econ` depending on `sim`. Mirror the
  existing config-threading (a field/set passed into `Society`/the agent decision), not a back-reference.
- **Strength calibration (the likely failure).** The penalty must be strong enough that a durable
  medium is preferred, weak enough that it isn't "refuse everything but SALT." If no setting threads
  that needle without becoming a disguised preference, that is the honest finding.
- **Determinism + gating.** The flag is off by default; `menger.rs`/the saleability metric and all
  existing scenarios stay byte-identical; the durability-aware branch is deterministic.
- **Scope.** S21a is the controlled *acceptance* demonstration (econ level). The full sim spoilage loop
  + on-market survival + terminal consumption are S21b/c — do not pull them in here.

## Handoff Notes

- **The lever is route (b): durability in the acceptance decision, not the saleability metric.** Keep
  `menger.rs` untouched; the win must come through *realized* acceptances driven by durability.
- **Demonstrate at the econ `MarketScenario` level** (the controlled barter setting) — it's where the
  canonical money microtest lives and where the lever can be isolated from the full colony.
- **Honest two-way DoD**: durable token beats perishable necessity as the medium *by physical
  durability* (success — the prerequisite for open-colony money) OR a characterized
  `marketability_finding` (durability too weak / only a disguised-preference penalty works). Both real.
- **Gate everything**; flag off → all goldens byte-identical (`lineages` + g4a_death the tripwires);
  the saleability metric and `Stock` layout unchanged off-path.
- Build S21a.0→S21a.2 as separate commits with their own tests; `git add` new files.
- **Next:** S21b (open survival + two-lane: retire the hearth, food/WOOD produced+traded under
  two-lane) → S21c (full open-colony money: compose produced survival + terminal consumption +
  marketability + two-lane — the capstone pass/finding).
