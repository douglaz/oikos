# impl-25 — S21b: Two-Layer Mengerian Saleability (direct-use eligibility floor + medium-saleability leadership)

Status: SPEC-READY (revised after Codex spec-review round 1)
Branch: `feat/two-layer-saleability`
Base: master @ `58b8779` (S21a landed)

## 0. Motivation — the S21a finding this resolves

S21a (the marketability/carrying-cost lever) landed as a **finding**: a *physical*
holding rule that makes an agent decline a perishable/high-carry good **as a means**
(`agent.rs:496-502`, `marketability.rs:52-56`) genuinely cuts the necessity's *indirect*
(re-trade) acceptance — but the necessity still **leads** the saleability race. The reason,
proven by `econ/tests/marketability.rs::marketability_finding` (`marketability.rs:286-306`):

> The saleability metric — **total acceptance share** — conflates *consumption* acceptance
> with *medium* (re-trade) acceptance. Leadership is computed on the single combined counter
> `CandidateStats.acceptances` (`menger.rs:76`) via `share_bps`/`leader_shares`/`winner`
> (`menger.rs:247-304`), which is reason-blind. A universal necessity wins on sheer
> direct-consumption volume even after losing its medium role.

Money emerges because a good is more saleable **in exchange**, not because it is eaten often.
S21b makes the metric express that, **without** circularity ("accepted as money because
accepted as money").

## 1. The design in one sentence

Split saleability into two layers:

- **(i) Direct-use saleability = the non-circular ELIGIBILITY FLOOR.** A good must have real,
  broad-enough *direct* (non-monetary) demand — measured as the count of **distinct agents who
  accept it directly** (`DirectWant`) — to be a money candidate at all. The regression-theorem
  anchor (money traces to a commodity with prior direct-use value). A *gate*, not a ranker.
- **(ii) Medium-saleability = LEADERSHIP / promotion.** Among eligible candidates, the leader
  is the one most accepted **as a means** — observed pre-money `IndirectFor` acceptances,
  scored by **medium share = `indirect_acceptances / total_indirect_acceptances`** plus the
  S9 distinct-acceptor / distinct-target breadth. This is what promotes to money.

Non-circularity: eligibility rests on direct-use demand (real, pre-monetary, not derived from
monetary status); leadership rests on **observed pre-promotion indirect trades** that are open
to *every* eligible candidate (see §2) — genuine re-trade behavior occurring *before* the good
is money. The good is never "money because it's money"; it is "the most-accepted-as-a-means
good that also has real direct demand."

## 2. The bootstrap — multi-candidate indirect offers (PRIMARY design, per Codex round 1)

Leadership-by-medium-share **alone deadlocks**, because of how indirect offers are routed today:

- Indirect offers are posted only for the **single provisional leader**:
  `generate_indirect_barter_offers` (`society.rs:2209-2266`) and `post_first_medium_sell_offer`
  (`society.rs:2269-2317`) pin `receive_good = leader`; the validity gate
  `offer_has_valid_saleability_context` (`barter.rs:363-368`) requires `provisional_leader ==
  Some(offer.receive_good)`.
- The provisional leader is chosen by **combined acceptance share** (`provisional_leader`,
  `menger.rs:201-214`).

So with a universal necessity: it wins provisional leadership on direct volume → all indirect
offers target it → agents decline the perishable necessity as a medium (S21a holding rule,
already enforced in offer posting at `society.rs:2238-2248`/`2292-2302`) → it accrues ~no
indirect acceptances → the better medium is never the provisional leader → gets **zero**
indirect offers → its `indirect_acceptances` stays 0 → never wins. Self-locking.

**Codex round 1 (P1):** a *marketability-gated single provisional leader* (an earlier draft)
*preselects* the medium — "the physically-best-looking candidate gets the only chance to prove
itself." The faithful, non-circular design is **multi-candidate discovery**:

> The direct-use floor creates a **candidate set**; indirect offers may target **any** eligible
> candidate; the agents' own S21a holding-rule declines (already wired into offer posting)
> suppress bad media; observed `IndirectFor` trades then determine medium leadership.

Key realization that keeps this tractable: **marketability filtering is already per-agent inside
offer posting** (`would_accept_indirect_barter_swap_with_stock` is called before every indirect
offer, `society.rs:2238-2248` and `2292-2302`). So we do **not** put marketability in the
metric or the candidate set — we simply let each agent post sell/spend lanes toward *each*
eligible candidate, and the existing S21a gate makes them decline perishable candidates
automatically. Durable candidates accrue indirect acceptances; perishable ones don't; medium
share picks the survivor. This is exactly "discover marketability through actual trades."

Why this is required (not optional): in the S20 cycle (`frontier_cycle_cleared`) there is **no**
universal necessity, so SALT is already the single provisional leader and `.1` alone would
change nothing observable. The distinguishing scenario is the S21a necessity scenario, where the
necessity seizes provisional leadership — only multi-candidate offers let SALT receive indirect
offers there. Hence the leadership-metric slice (`.1`) and the multi-candidate slice (`.2`) are
both load-bearing.

## 3. Exact current-state map (verified; all `econ/src/` unless noted)

- Combined leadership metric: `share_bps` (`menger.rs:247-254`, denom `total_acceptances`),
  `leader_shares` (`menger.rs:272-304`, numerator `stats.acceptances` at `:278`), `winner`
  (`menger.rs:185-199`), `provisional_leader` (`menger.rs:201-214`).
- Eligibility: `base_eligible` (`menger.rs:256-270`) mixes a combined share floor (`:258`),
  combined acceptor/counterpart floors (`:259-260`), and the S9 indirect-breadth floors
  (`:266-269`).
- Counting chokepoint (the only place `BarterReason` is visible): `observe_acceptance`
  (`menger.rs:216-241`) — combined bump `:230-232`; indirect arm `:236-240`. `total_acceptances`
  bumped in `observe_trade` (`menger.rs:149-158`).
- Accumulator `CandidateStats` (`menger.rs:73-89`): has `indirect_*` mirrors, **no `direct_*`**.
  Read view `CandidateSaleability` (`menger.rs:62-71`); read iterator `candidate_saleability`
  (`menger.rs:137-147`); snapshot `SaleabilitySnapshot` (`menger.rs:8-18`, share field
  `acceptance_share_bps` = TOTAL share).
- **Digest serializes the runtime tracker state** (Codex P1 — confirmed): `push_emergence_runtime_bytes`
  (`sim/src/settlement.rs:14843-14875`) writes `total_acceptances` and, per candidate,
  `acceptances` + acceptor/counterpart member lists + the `indirect_*` fields — all into
  `canonical_bytes`. Adding `direct_*` to the serialized view **unconditionally would break every
  golden**; it must be appended ON-only.
- Config digest: `push_mengerian_config_bytes` (`sim/src/settlement.rs:14877+`); ON-only flag
  pattern for `multi_offer_medium`/`durability_aware_acceptance` at `:14897-14920`.
- Offer routing: `generate_indirect_barter_offers` (`society.rs:2209-2266`),
  `post_first_medium_sell_offer` (`society.rs:2269-2317`), lane classifiers
  `is_medium_spend_lane`/`is_medium_receive_leader_lane` (`society.rs:2346-2362`), validity gate
  `offer_has_valid_saleability_context` (`barter.rs:363-368`). The S21a marketability gate is
  already applied per offer (`society.rs:2238-2248`, `2292-2302`).
- Config: `MengerianConfig` (`money.rs:89-133`), `Default` (`money.rs:135-155`). Threading:
  `try_from_scenario` (`society.rs:371-381`) → `Society` fields (`society.rs:308-310`, `:477-479`)
  → consumed `if self.<flag>`.
- Prior-art test infra: `candidate_counts` (derives `direct = acceptances − indirect`,
  `econ/tests/marketability.rs:77-92`), `saleability_leader` (`:94-102`), `marketability_finding`
  (`:286-306`), `goldens_unchanged` (`:344-361`).

## 4. Slices

### S21b.0 — Metric plumbing (tracking + medium-share machinery; no behavior change; flag-off byte-identical)

1. `CandidateStats` (`menger.rs:73-89`): add `direct_acceptances: u64`,
   `direct_acceptor_agents: Vec<AgentId>`. Count on a new `BarterReason::DirectWant` arm in
   `observe_acceptance` (`menger.rs:236`), symmetric to the `IndirectFor` arm. (A real
   `direct_acceptances` field even though it equals `acceptances − indirect_acceptances`, because
   **distinct direct acceptor agents** — the breadth the floor needs — is not derivable.)
2. Tracker: add `total_indirect_acceptances: u64`, bumped in the `IndirectFor` arm.
3. Read APIs (Codex P1 denominator + P2 naming):
   - `medium_share_bps(good) = indirect_acceptances / total_indirect_acceptances`
     (`total_indirect_acceptances == 0` → `0`/no medium leader).
   - `medium_leader_shares()` — mirror of `leader_shares` over medium share.
   - Extend `CandidateSaleability` (`menger.rs:62-71`) + `candidate_saleability` with `direct_*`.
   - Add `medium_share_bps: u16` to `SaleabilitySnapshot` (`menger.rs:8-18`), distinct from the
     existing total `acceptance_share_bps`.
4. **Digest (Codex P1 — the critical golden-safety fix):** thread the config flag into
   `push_emergence_runtime_bytes` (`sim/src/settlement.rs:14843`) and append the new
   `direct_acceptances` + `direct_acceptor_agents` member bytes **only when
   `two_layer_saleability` is ON**, at the end of each candidate block (mirroring the indirect
   section). Off-path emits byte-for-byte the prior layout. `total_indirect_acceptances` is
   derivable from the already-serialized per-candidate `indirect_acceptances`, so it need not be
   serialized; if serialized, it is ON-only too.

Invariant: nothing reads the new fields for ranking yet, and the new bytes are ON-only, so **all
18 golden suites stay byte-identical and behavior is unchanged.** Tests: `direct_acceptances +
indirect_acceptances == acceptances`; `direct_acceptor_agents` is the distinct `DirectWant`
acceptor set; medium-share arithmetic incl. the zero-denominator case.

### S21b.1 — The two-layer eligibility + leadership metric behind `two_layer_saleability` (gated)

Add to `MengerianConfig` (default in `money.rs:135-155`):
- `two_layer_saleability: bool` (default `false`).
- `min_direct_use_acceptors: u16` (direct-use eligibility floor; default `0` = inert).

Thread `try_from_scenario` → `Society` → gate. Digest ON-only in `push_mengerian_config_bytes`
(mirror `:14897-14920`):
```
if menger.two_layer_saleability {
    out.push(1);
    out.extend_from_slice(&menger.min_direct_use_acceptors.to_le_bytes());
}
```

When `two_layer_saleability` is ON (Codex P2 — every gate classified, no hidden combined gate):
1. **Leadership** (`winner` `menger.rs:185-199`, `provisional_leader` `menger.rs:201-214`): rank
   by `medium_leader_shares()` instead of `leader_shares()`. `total_indirect_acceptances == 0`
   → no medium leader (`None`). `lead_margin_bps` / `tied_best` discipline preserved, applied to
   medium share.
2. **`base_eligible`** under two-layer becomes, explicitly:
   - **direct-use floor (NEW):** `direct_acceptor_agents.len() >= min_direct_use_acceptors`;
   - **medium-share floor:** `medium_share_bps(good) >= promotion_threshold_bps` (the promotion
     threshold now reads MEDIUM share, not combined);
   - **medium-breadth floors (S9, reinterpreted as medium-use):** `indirect_acceptances >=
     min_indirect_acceptances`, distinct indirect acceptors / targets `>= min_indirect_*`;
   - **liveness:** `total_acceptances >= min_total_acceptances` retained as a general liveness
     gate (not a discriminator).
   The legacy **combined** share floor and combined acceptor/counterpart floors
   (`menger.rs:258-260`) are **replaced** by the medium-share + direct-use floors above when the
   flag is on — they are NOT left silently active. Off-path: the exact current `base_eligible`.
3. Snapshots emit `medium_share_bps` so tests/diagnostics read the right number.

This makes leadership/promotion track medium-saleability *once candidates have indirect data* —
but in the necessity scenario the better medium still has 0 indirect data until `.2`.

### S21b.2 — Multi-candidate indirect offer generation (the bootstrap fix)

Add `provisional_media_candidates(config) -> Vec<GoodId>`: when `two_layer_saleability` is ON,
the goods passing the **direct-use eligibility floor** (`direct_acceptor_agents.len() >=
min_direct_use_acceptors`), in the tracker's sorted candidate order (deterministic). Not gated on
indirect data, so it is populated as soon as direct trades give breadth — which breaks the
bootstrap.

When the flag is ON, generalize offer routing from a single `leader` to the candidate set:
- `generate_indirect_barter_offers` / `post_first_medium_sell_offer`: iterate the candidate set;
  for each candidate, run the existing per-agent logic (including the S21a marketability gate at
  `society.rs:2238-2248`/`2292-2302`, unchanged) to post a sell lane (`give surplus → candidate`)
  and respect the spend lane (`give candidate → want`). Perishable candidates are declined
  per-agent and accrue nothing; durable ones accrue indirect acceptances.
- `offer_has_valid_saleability_context` (`barter.rs:363-368`): under two-layer, an `IndirectFor`
  offer is valid if `offer.receive_good` is in the candidate set (any eligible medium), not only
  the single provisional leader. Thread the candidate set to the validity site.
- Lane classifiers (`society.rs:2346-2362`): generalize "the leader" to "a candidate medium"
  under two-layer (a per-candidate spend/sell lane). The S20 book already supports N offers/agent;
  the candidate set is small (2–4), so the offer count stays bounded.

**Off-path (flag OFF) is the exact current single-leader machinery — byte-identical.** This is the
biggest/highest-risk slice; the disabled-flag golden regression is the tripwire.

### S21b.3 — Controlled scenario + falsifiable acceptance suite

Add `menger_two_layer_saleability` layering `two_layer_saleability: true` + `min_direct_use_acceptors`
on top of the S21a marketability scenario (`econ/src/scenario.rs:1197+`; necessity perishable,
neutral commodity durable with a heterogeneous direct-use anchor so it clears the floor). Suite in
`econ/tests/two_layer_saleability.rs` (mirror `marketability.rs` helpers):

- `two_layer_off_reproduces_the_finding`: flag off → `saleability_leader == NECESSITY` (the S21a
  `marketability_finding`).
- `two_layer_promotes_the_medium` (FALSIFIABLE BAR): flag on → the medium leads by medium share
  where the necessity led before (`medium_leader_shares().good == COMMODITY`).
- `the_commodity_promotes_to_money` (stronger): flag on → `current_money_good() == Some(COMMODITY)`.
  If the 2-good controlled scenario lacks the indirect breadth to satisfy the S9 medium-breadth
  floors and promote, compose with the S20 cycle for THIS assertion **and keep the leadership flip
  proven in the controlled scenario** — reported honestly, not forced.
- `multiple_candidates_received_indirect_offers`: assert >1 good accrued indirect acceptances (or
  received indirect offers) — proves discovery was open, not preselected (Codex P1).
- `eligibility_floor_is_non_circular`: a good with indirect acceptances but `direct_acceptor_agents
  < min_direct_use_acceptors` is NOT eligible/winner (proves direct-use is a real gate; guards
  against pure-medium circularity).
- `lever_off_is_byte_identical` + `two_layer_run_is_deterministic` (mirror
  `durability_aware_run_is_deterministic`).
- `canonical_bytes_include_two_layer_saleability` (sim test, mirror `two_lane_clearing.rs:318`):
  off == explicit-off, off != on, inert-config(off) == off.

## 5. Determinism / golden contract (must hold)

- Defaults `false`/`0`; all struct literals across `scenario.rs`/`menger.rs`/`worldgen.rs`/
  `barter.rs` keep compiling via `..Default::default()`.
- **Two ON-only digest gates**: (a) the new `direct_*` member bytes in
  `push_emergence_runtime_bytes` appended only when `two_layer_saleability`; (b) the config bytes
  in `push_mengerian_config_bytes` appended only when ON. Both append at the end of their section.
- All flag-OFF suites byte-identical: g4a_death, g4b_demography, g5a_emergence, g5b_frontier,
  money_coemergence, strong_bar_emergence, producible_capital, originary_interest,
  entrepreneurial_uncertainty, own_labor_subsistence, spatial_households,
  forage_carrying_capacity, pre_money_cultivation, money_from_produced_bread, mortality,
  multigood_money, cycle_money, two_lane_clearing, econ marketability — plus the pinned
  `goldens_unchanged` digest blocks (`two_lane_clearing.rs:376`, `cycle_money.rs:560`).
- All new ordering total + deterministically tie-broken (mirror `tied_best`/`lead_margin_bps`);
  no float; sorted-`Vec` accumulators only (no map-iteration leaks).
- `cargo fmt --check` + `cargo clippy --workspace --all-targets -- -D warnings` clean.

## 6. Honest scope (what S21b does and does NOT claim)

- DOES: make money leadership/promotion track **medium-saleability** atop a **non-circular
  direct-use eligibility floor**, with indirect-offer discovery open to **all** eligible
  candidates (no preselected medium); resolve the S21a conflation finding in a controlled
  scenario; compose with S21a marketability + S20 two-lane clearing.
- Does NOT claim: full open-colony integration (on-market survival + terminal consumption) — the
  next milestone. The direct-use anchor is still the S9-style configured heterogeneous use (the
  *use* is given, not itself emergent). Promotion (vs leadership) may require the richer cycle for
  S9 indirect breadth; if so, reported as such, not forced.

## 7. Pipeline

rb-lite `--implementer codex,claude` (slices S21b.0→.3, each its own commit + tests) →
independent verification (workspace + all 18 goldens byte-identical + the new suite + fmt/clippy)
→ Codex review-of-results → feat commit + merge + report/memory update + plan-hash pin.
