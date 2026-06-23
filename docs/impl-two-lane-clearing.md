# Implementation Spec: the two-lane bilateral medium order book (S20)

> The direct sequel to the S19 barrier — the first design that gives the token the institutional
> room to function as money without a planner. S19 proved SALT **wins the saleability-leader race**
> under imperfect double coincidence (the cycle goods are bad direct media) but **never promotes**,
> because the **one-live-offer-per-agent** barter book + the indirect generator (which *replaces* an
> agent's direct *SALT-spend* offer with an indirect *SALT-receive* offer) leaves the book full of
> "give output → SALT" and missing the complementary "give SALT → input" side — so the medium can't
> round-trip and indirect exchange never clears. S20 replaces that artificially-impoverished
> institution with a **minimally realistic bilateral order book: two lanes per agent** — a *spend*
> lane (`give SALT → input`, DirectWant) and a *sell-for-medium* lane (`give output → SALT`,
> IndirectFor{input}). Ordinary **pairwise** matching does the rest; the seeded SALT circulates the
> ring bilaterally, via the medium. NOT a ring/clearing-house matcher (that would clear the A→B→C
> triangle *without* money — bypassing it, the opposite result). One-variable surgery on
> `frontier_cycle`: replace the crippled barter institution, change nothing else.

## What the research established (the change is tiny and confined to Society policy)

(Paths verified against the `econ` crate; Codex-confirmed. Use the function names as anchors —
some sub-line numbers are approximate.)
- **The barter BOOK already supports N offers per agent** (`econ/src/barter.rs`: the book is flat
  `Vec`s, per-offer dedup is by `seq` NOT by agent; existing tests post two offers/agent;
  `post_offer` ~`:93`). **Zero changes to `barter.rs`.**
- **The one-offer cap is PURELY a Society-layer policy** (`econ/src/society.rs`), in exactly three
  spots: `agent_has_live_barter_offer` (`:2222`); the direct-lane early-`continue` inside
  `generate_direct_barter_offers` (`:2033`, `if self.agent_has_live_barter_offer(agent_id) { continue }`);
  and `replace_live_barter_offers_for_agent_with` (`:2159`), which **cancels** the agent's prior
  offers before posting the indirect one — *this cancel is the S19 deadlock* (the indirect sell
  evicts the direct spend).
- **Reservations are already two-lane-correct** (`econ/src/barter.rs` reservation accounting): each
  offer reserves its own give-good; distinct give-goods (SALT vs output) reserve independently; the
  same give-good is auto-protected by `unreserved_stock` (the existing double-spend test). **No
  reservation change.**
- **`clear_matches_with_provisional_leader` is already strictly PAIRWISE and agent-count-agnostic**
  (`econ/src/barter.rs:173`): matches `(give,receive)` against the reciprocal bucket, skips self,
  `apply_swap` is a two-party swap. Adding a second offer/agent just adds bucket entries — **no
  clearing change, no ring matcher.** Determinism preserved (offers `seq`-sorted, `BTreeMap` matching).
- **Config threading precedent**: boolean scenario flags are derived in `try_from_scenario` and
  stored as Society fields (`v2_enabled`, `m2_enabled`/`m3_enabled`). The barter call site is the
  `V2Phase::Barter` branch (`generate_direct_barter_offers` → `generate_indirect_barter_offers` →
  `clear_matches_with_provisional_leader`). No `BarterConfig`/`multi_offer_medium` exists yet
  (greenfield).

## Purpose & the honest bar

Add a gated **two-lane medium order book** (`multi_offer_medium`, default off) so that — in the S19
3-good cycle — an agent holding SALT + its output and wanting its input can hold BOTH a *spend* offer
(`give SALT → input`, DirectWant) and a *sell-for-medium* offer (`give output → SALT`,
IndirectFor{input}) live at once. Pairwise matching then lets the seeded SALT round-trip the ring
bilaterally. Test whether this finally lets endogenous token money emerge in a produced exchange cycle (with survival isolated off-market — NOT a scaffold-free full colony; the S19 hearth survival + closed input-loop are carried-forward abstractions). Success = SALT
**promotes** under the **unchanged strong-bar gate** — it leads first (as in S19), then indirect SALT
acceptances actually **clear**, `indirect_target_goods(SALT) ⊇ {X, Y, Z}`, the round-trip ledger shows
`accepted > 0` AND `spent > 0` (the medium genuinely intermediates), and production continues
post-promotion on market-acquired inputs. Honest target: **test whether** the minimally-richer
bilateral institution lets the medium do its job — endogenous token money in a produced exchange cycle (survival still isolated off-market via the
hearth scaffold — NOT scaffold-free), or a characterized failure of exactly what still blocks it.

Authenticity is the whole point — money must remain the thing doing the work (Codex):
- It is a **bilateral** refinement (real order books let one actor hold a bid AND an ask). It is NOT
  a central ring-clearer; `clear_matches` stays strictly pairwise; do NOT add a triangle/ring matcher
  that clears A→B→C without money.
- Cap at **exactly two lanes** per agent (spend + sell) — not an arbitrary/combinatorial book.

Principled-failure modes, all first-class (Codex — and possible):
- **SALT still pools** — multi-offer clears some trades but the medium accumulates on one side.
- **A leg starves** — the matching order leaves one cycle edge unfilled.
- **Reservations over-cancel** — a lane is dropped too aggressively, re-breaking the round-trip.
- **A cycle good becomes leader** — multi-offer raises a cycle good's trade volume past SALT.
- **Breadth still short** — indirect acceptances clear but never span all three targets.
Land `two_lane_clearing_finding` with the characterized reason. Do NOT rescue it with a ring matcher,
minting, a universal anchor, or invented taste.

NOT a new money rule (reuse the strong-bar gate UNCHANGED — S20 only enriches the *barter
institution*), NOT perishability/mortality/final-consumer (unchanged from S19 — one-variable
surgery), NOT a `barter.rs`/`clear_matches` rewrite (Society-policy + a flag only). Additive + gated;
flag off → S5–S19 + all goldens byte-identical.

## Verified Base Facts (oikos @ `737a8d1`, crate `econ`)

1. **Book supports N offers/agent** (`econ/src/barter.rs`, dedup by `seq` not agent). No book change.
2. **One-offer cap = Society policy** only: `agent_has_live_barter_offer` (`econ/src/society.rs:2222`),
   the direct skip inside `generate_direct_barter_offers` (`:2033`), the cancel in
   `replace_live_barter_offers_for_agent_with` (`:2159`).
3. **Two lanes**: SPEND = `generate_direct_barter_offers` (`society.rs:2033`, `DirectWant`); SELL =
   `generate_indirect_barter_offers` (`society.rs:2095`, `IndirectFor{target}`, posts via the
   cancelling `replace_live_barter_offers_for_agent_with` `:2159`). The cancel is what forbids both
   coexisting today; the direct generator posting the FIRST acceptable leader-side offer (which can be
   `output → SALT`) is why the spend lane needs to be EXPLICIT (Codex P1, see S20.1).
4. **Reservations** per give-good, independent across lanes (`econ/src/barter.rs`). No change.
5. **`clear_matches` pairwise + agent-agnostic** (`econ/src/barter.rs:173`). No change.
6. **Flag threading**: `try_from_scenario` → Society field, like `v2_enabled`. The round-trip ledger +
   `indirect_target_goods` + `saleability_leader` + the cycle scenario (`frontier_cycle`) from S19.

## The slices (build in order; each independently testable)

- **S20.1 — the two-lane capability (EXPLICIT lanes) + a Society-level MICROTEST (Codex: do this
  first).** Add a gated `multi_offer_medium` flag (threaded like `v2_enabled`: scenario →
  `try_from_scenario` → Society field). In the gated path, post each agent **two distinct, explicit
  lanes** (Codex P1 — NOT a vague relaxation of the one-offer skip, which would still post the wrong
  first-acceptable leader-side offer):
  - **SPEND lane** — ONLY `give == leader (SALT) && receive == a non-leader target` (DirectWant).
  - **SELL lane** — ONLY `give == a non-leader surplus && receive == leader (SALT)` (IndirectFor{target}).
  - Cancel/replace **only same-lane** offers; keep **at most one of each lane** per agent; the SELL
    lane is posted ADDITIVELY (never cancels the SPEND lane — the S19 deadlock was exactly that cancel).
  Reservations + `clear_matches` unchanged (the book already supports two offers/agent, reserves per
  give-good, clears pairwise). **Microtest at the SOCIETY level (Codex P2 — the cap lives in Society
  offer-generation, NOT in `BarterBook`, so a plain `barter.rs` unit test can't prove the deadlock/fix):**
  a small Society/`MarketScenario` harness — 3 agents, 3 goods, SALT the provisional leader, each holds
  output + SALT + an input want — exercises `generate_direct_barter_offers` + `generate_indirect_barter_offers`
  + `clear_matches`: with `multi_offer_medium` OFF no indirect SALT trade clears (the S19 deadlock at
  the unit level); ON, both lanes post, pairwise SALT trades clear, and `IndirectFor` acceptances are
  recorded. **Test:** the Society microtest both ways; flag off → byte-identical.
- **S20.2 — wire into the cycle + prove the bilateral round-trip.** Compose `frontier_cycle` (S19) +
  `multi_offer_medium`. The seeded SALT now round-trips the ring bilaterally via the medium. **Test:**
  with the flag on, the round-trip ledger is material (`accepted > 0` AND `spent > 0`),
  `indirect_target_goods(SALT)` reaches `{X, Y, Z}`, and every pre-promotion cycle-input acquisition
  has SALT on one side (no direct X/Y/Z barter); flag off → the S19 deadlock (the load-bearing control).
- **S20.3 — the emergence scenario + DoD.** The `frontier_cycle` flagship with `multi_offer_medium`
  on (or a `frontier_cycle_cleared` sibling). Register the viewer scenario. **Test:** the acceptance
  suite below — SALT promotes (the success) OR the characterized `two_lane_clearing_finding`.

## Acceptance Tests (the S20.3 DoD) — `sim/tests/two_lane_clearing.rs` (+ a econ-crate Society-level microtest)

1. `two_lane_microtest_off_deadlocks_on_clears` (econ, Society-level) — 3 agents/3 goods/SALT leader: with
   `multi_offer_medium` off NO indirect SALT trade clears; on, pairwise SALT trades clear and
   `IndirectFor` acceptances record — the deadlock and its fix isolated at the unit level.
2. `two_lane_run_is_deterministic` — byte-identical `(seed, config)`.
3. `salt_promotes_from_the_cleared_cycle` — **the core claim**: SALT leads first (as S19) then
   **promotes** (promoted good IS SALT, `medium_want_qty=0`) under the unchanged strong-bar gate;
   `indirect_target_goods(SALT) ⊇ {X, Y, Z}` (the medium bridges all cycle inputs). (If it still
   doesn't promote → the documented `two_lane_clearing_finding`.)
4. `salt_round_trips_bilaterally` — the traced ledger: `accepted > 0` AND `spent > 0`; SALT accepted
   `IndirectFor{input}` is later spent on that input; production continues post-promotion.
5. `money_does_the_work_not_a_ring_matcher` — **the authenticity tripwire**: every pre-promotion
   cycle-input acquisition has SALT on one side (no direct X↔Y/Y↔Z/Z↔X barter clears); `clear_matches`
   is pairwise (two-party swaps only); there is no multi-party/ring clearing.
6. `two_lane_conserves` — whole-system conservation every tick (the two lanes reserve independently,
   no overbooking; SALT neutral; recipe input→output).
7. `controls_prove_money_is_load_bearing` — (a) `multi_offer_medium` OFF → the S19 deadlock (no
   promotion); (b) no SALT seed → no clearing; (c) `allow_indirect_acceptance=false` → no promotion;
   (d) SALT removed from candidates → no medium bridge. These prove the medium (not the mechanism
   alone) does the work.
8. `goldens_unchanged` — with `multi_offer_medium` off, S5–S19 scenarios + the six econ +
   g5a/g5b/coemergence + demographic (`lineages`) + g4a_death goldens byte-identical; S5–S19 suites
   green; the flag in `canonical_bytes` with a regression (or scenario-local so off → byte-identical);
   `barter.rs` + `clear_matches` untouched; clippy `-D warnings`; fmt `--check`.

(Principled-failure path: if SALT pools / a leg starves / a cycle good leads / breadth stays short,
land `two_lane_clearing_finding` with the characterized reason — NOT a ring-matcher/minting rescue.)

Manual: `cargo run -p viewer -- run cycle --ticks 3000` (with the flag on).

## Missing Interactions (the central risks)

- **Cap at exactly two lanes.** Spend + sell only — not an arbitrary book (combinatorial offers would
  be both unrealistic and a perf/determinism hazard). The gating must post at most these two.
- **Lane edge cases (Codex — pin these).** (a) The SPEND lane must scan non-leader targets EVEN when
  the agent also directly wants SALT — do not let the existing `target_goods.contains(leader)`
  suppression (which correctly gates the *indirect* lane) block the spend lane. (b) The SELL
  IndirectFor lane must still NOT be posted by an agent whose relevant want is DIRECTLY for SALT (else
  genuine direct SALT demand is mislabeled as indirect exchange — corrupting the breadth metric). (c)
  An agent holding no SALT this tick simply posts no spend lane (only the sell lane) — expected, not a
  bug; it gets SALT first, then spends next tick.
- **Don't over-cancel.** The S19 deadlock IS the cancel in `replace_live_barter_offers_for_agent_with`;
  the gated path must post the sell lane ADDITIVELY (or cancel only same-lane offers), leaving the
  spend lane live. Verify both lanes coexist (the microtest).
- **SALT pooling / leg starvation (the likely failure).** Even with both lanes, matching order or
  reservation timing could let SALT accumulate on one side, or leave one cycle edge unfilled. The
  round-trip ledger (`spent > 0`) is the tripwire; if it stays low, that's the finding.
- **Authenticity — no ring matcher.** `clear_matches` stays pairwise; `money_does_the_work_not_a_ring_matcher`
  asserts SALT is on one side of every pre-promotion input acquisition. A multilateral clearer would
  pass promotion while *disproving* the money thesis — explicitly forbidden.
- **Goldens / determinism.** The flag is off by default and `barter.rs`/`clear_matches` are untouched,
  so all existing scenarios are byte-identical; the two-lane offers are `seq`-ordered (deterministic).
- **No other variable.** Survival off-market, the cycle, the anchor, the SALT seed — all unchanged
  from S19. S20 changes ONLY the barter institution, so a promotion attributes cleanly to it.

## Handoff Notes

- **The change is tiny and confined**: a gated flag + the two `generate_*_barter_offers` methods in
  `econ/src/society.rs` (skip the direct-lane `agent_has_live_barter_offer` continue for the
  spend lane; post the indirect sell lane additively, no cancel of the spend lane). `barter.rs`,
  reservations, and `clear_matches` need NO changes (already agent-count-agnostic). Build the
  econ-crate Society-level microtest FIRST (Codex) — it isolates the deadlock and the fix before the full scenario.
- **Honest two-way DoD**: SALT finally promotes — endogenous token money in a produced exchange cycle
  (still on the disclosed S19 abstractions: closed input loop, off-market hearth survival) — OR a
  characterized `two_lane_clearing_finding`. Both are real; do not fake it with a ring matcher, minting,
  a universal anchor, or invented taste.
- **The proof is the round-trip + the pairwise-only authenticity tripwire** — promotion alone could be
  incidental (the S16 lesson) or could come from a ring-clear that bypasses money; `salt_round_trips_bilaterally`
  + `money_does_the_work_not_a_ring_matcher` are what earn "money emerged".
- **Gate everything**; the `lineages` + `g4a_death` goldens are the tripwires; `barter.rs`/`clear_matches`
  must stay byte-for-byte unchanged.
- Build S20.1→S20.3 as separate commits with their own tests; `git add` new files.
- **After S20:** if it promotes, the arc closes the money question fully (emergence needs a saleability
  lead AND a clearing institution that lets the medium round-trip); if it's a finding, the barter
  institution's exact remaining limit is mapped. Either way, remaining items (perishability; a 10k
  mortality smoke test) are optional add-ons.
