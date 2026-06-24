# impl-25 — S21b: Two-Layer Mengerian Saleability (direct-use eligibility floor + medium-saleability leadership)

Status: DRAFT (pre-Codex-spec-review)
Branch: `feat/two-layer-saleability`
Base: master @ `58b8779` (S21a landed)

## 0. Motivation — the S21a finding this resolves

S21a (the marketability/carrying-cost lever) landed as a **finding**: a *physical*
holding rule that makes an agent decline a perishable/high-carry good **as a means**
(`agent.rs:496-502`, `marketability.rs:52-56`) genuinely cuts the necessity's
*indirect* (re-trade) acceptance — but the necessity still **leads** the saleability
race. The reason, proven by `econ/tests/marketability.rs::marketability_finding`
(`marketability.rs:286-306`):

> The saleability metric — **total acceptance share** — conflates *consumption*
> acceptance with *medium* (re-trade) acceptance. Leadership is computed on the single
> combined counter `CandidateStats.acceptances` (`menger.rs:76`) via
> `share_bps`/`leader_shares`/`winner` (`menger.rs:247-304`), which is reason-blind.
> A universal necessity wins on sheer direct-consumption volume even after losing its
> medium role.

Money emerges because a good is more saleable **in exchange**, not because it is eaten
often. S21b makes the metric express that, **without** smuggling in circularity
("accepted as money because accepted as money").

## 1. The design in one sentence

Split saleability into two layers:

- **(i) Direct-use saleability = the non-circular ELIGIBILITY FLOOR.** A good must have
  real, broad-enough *direct* (non-monetary) demand to be a money candidate at all —
  the regression-theorem anchor (money traces back to a commodity with prior direct-use
  value). This is a *gate*, not a ranker; it does **not** pick the winner.
- **(ii) Medium-saleability = LEADERSHIP / promotion.** Among eligible goods, the leader
  is the one most accepted **as a means** — actual observed pre-money `IndirectFor`
  acceptances (volume + breadth across distinct targets and acceptors). This is what
  promotes to money.

Non-circularity argument: eligibility rests on direct-use demand (real, pre-monetary,
not derived from monetary status); leadership rests on **observed pre-promotion indirect
trades** — genuine re-trade behavior that occurs *before* the good is money, which is
exactly the Mengerian bootstrapping, not a tautology. The good is never "money because
it's money"; it is "the most-accepted-as-a-means good that also has real direct demand."

## 2. The hard part — the provisional-leader bootstrap (THE #1 RISK)

Leadership-by-indirect-share **alone deadlocks**, and the spec must address this head-on.

The chain (verified):
- Indirect offers are posted only for the **single provisional leader**:
  `generate_indirect_barter_offers` (`society.rs:2255`) and `post_first_medium_sell_offer`
  (`society.rs:2309`) both pin `receive_good = provisional_leader`.
- The provisional leader is chosen by **combined acceptance share**
  (`provisional_leader`, `menger.rs:201-214`; ranks by `leader_shares` on
  `stats.acceptances`).

So with a universal necessity present:
1. Necessity wins provisional leadership (combined share, dominated by direct volume).
2. All indirect offers target the necessity.
3. Agents decline the perishable necessity as a medium (S21a holding rule) → it
   accumulates ~no indirect acceptances.
4. The better medium (e.g. SALT) is never the provisional leader → gets **zero** indirect
   offers → its `indirect_acceptances` stays 0 → it can never win leadership-by-indirect.

**Self-locking.** Changing only the final `winner` metric is insufficient; the
**provisional-leader selection** is the bottleneck.

### Primary design (proposed): marketability-gated provisional leadership

When `two_layer_saleability` is ON, the provisional leader is chosen **among
direct-use-eligible candidates**, **excluding goods that fail the S21a marketability
holding rule as a medium** (i.e. compose with `durability_aware_acceptance`: a good that
no agent would accept as a means is not a medium candidate), then ranked by indirect
share with a **direct-use-breadth bootstrap seed** when no indirect data exists yet.

Walkthrough with the necessity + neutral commodity:
- Perishable necessity fails the holding rule → excluded from provisional leadership.
- Durable neutral commodity (broad-enough direct use) → becomes provisional leader →
  indirect offers target it → it accumulates indirect acceptances → it wins
  leadership-by-indirect AND passes the direct-use floor → promotes.

This composes S21a (which goods *can* be media) with S21b (leadership/promotion tracks
*medium-saleability* atop a direct-use floor). The capstone scenario runs S20
`multi_offer_medium` + S21a `durability_aware_acceptance` + S21b `two_layer_saleability`
together.

### Alternative (note for Codex, NOT primary): multi-candidate indirect offers

Let indirect offers be posted for **all** direct-use-eligible candidates rather than the
single provisional leader; agents' own holding-rule declines (S21a) then suppress bad
media, and leadership-by-indirect picks the survivor. Cleaner conceptually (no
marketability in the leader metric), but it touches the offer-generation hot path
(`generate_indirect_barter_offers` pins a single `receive_good`) and is a bigger, riskier
change. Deferred unless Codex judges the primary's S21a↔S21b coupling unacceptable.

## 3. Exact current-state map (from research, all `econ/src/` unless noted)

- Combined leadership metric: `share_bps` (`menger.rs:247-254`), `leader_shares`
  (`menger.rs:272-304`, numerator `stats.acceptances` at `:278`), `winner`
  (`menger.rs:185-199`), `provisional_leader` (`menger.rs:201-214`).
- Strong-bar eligibility: `base_eligible` (`menger.rs:256-270`) — already mixes a combined
  share floor (`:258`) with the S9 indirect floors (`:266-269`).
- Counting chokepoint (only place `BarterReason` is visible): `observe_acceptance`
  (`menger.rs:216-241`): combined bump at `:230`; indirect branch at `:236-240`.
- Accumulator struct: `CandidateStats` (`menger.rs:73-89`) — has `indirect_*` mirrors but
  **no `direct_*` mirror**. Read view `CandidateSaleability` (`menger.rs:62-71`); digest
  iterator `candidate_saleability` (`menger.rs:137-147`); snapshot `SaleabilitySnapshot`
  (`menger.rs:8-18`).
- `BarterReason::{DirectWant, IndirectFor{target}}` (`barter.rs:9-19`); preserved
  end-to-end into the tracker (`barter.rs:220-229` → `v2_observe_barter_trades`
  `society.rs:2508-2515` → `observe_trade` `menger.rs:149-158`).
- S21a holding rule: `would_accept_indirect_barter_swap_with_stock` (`agent.rs:478-524`,
  gate `:496-502`), `MarketabilityConfig::can_cover_holding_period`
  (`marketability.rs:52-56`).
- Config: `MengerianConfig` (`money.rs:89-133`), defaults (`money.rs:135-155`). Threading:
  extracted in `try_from_scenario` (`society.rs:371-381`), stored on `Society`
  (`society.rs:308-310`, assigned `:477-479`), consumed `if self.<flag>`.
- Digest ON-only gating: `push_mengerian_config_bytes` (`sim/src/settlement.rs:14877`,
  the `multi_offer_medium`/`durability_aware_acceptance` pattern at `:14897-14920`).
- Prior-art test infra: `candidate_counts` (derives `direct = acceptances − indirect`,
  `econ/tests/marketability.rs:77-92`), `saleability_leader` (`:94-102`),
  `marketability_finding` (`:286-306`), `goldens_unchanged` (`:344-361`).

## 4. Slices

### S21b.0 — Direct-use tally (tracking only, no behavior change)

Add to `CandidateStats` (`menger.rs:73-89`): `direct_acceptances: u64`,
`direct_acceptor_agents: Vec<AgentId>`. Count them in `observe_acceptance` on a new
`BarterReason::DirectWant` arm, symmetric to the existing `IndirectFor` arm
(`menger.rs:236-240`). Mirror in `CandidateSaleability` (`menger.rs:62-71`) and
`candidate_saleability` (`menger.rs:137-147`).

Rationale for a real field (not pure derivation): `direct_acceptances` is derivable as
`acceptances − indirect_acceptances`, but **distinct direct acceptor agents** (the breadth
the eligibility floor needs) is not derivable — hence the explicit `Vec<AgentId>`.

Invariant: `CandidateStats` is not serialized into `canonical_bytes` (only
`MengerianConfig` is — confirmed via `push_mengerian_config_bytes`), and no ranking reads
the new fields yet, so **all goldens stay byte-identical** and behavior is unchanged.
Test: a unit test asserting `direct_acceptances + indirect_acceptances == acceptances` and
that `direct_acceptor_agents` is the distinct-agent set on the `DirectWant` side.

### S21b.1 — The two-layer metric behind `two_layer_saleability`

Add to `MengerianConfig` (`money.rs:89-133`, default `false`/`0` in `Default`
`money.rs:135-155`):
- `two_layer_saleability: bool` (default `false`).
- `min_direct_use_acceptors: u16` (the direct-use eligibility floor — distinct agents who
  accept the good directly; default `0` = inert).

Thread through `try_from_scenario` (`society.rs:371-381`) → `Society` field
(`society.rs:308-310`, `:477-479`). Gate the digest ON-only in
`push_mengerian_config_bytes` (`sim/src/settlement.rs`, mirror `:14897-14920`):
```
if menger.two_layer_saleability {
    out.push(1);
    out.extend_from_slice(&menger.min_direct_use_acceptors.to_le_bytes());
}
```

When `two_layer_saleability` is ON:
1. **Eligibility floor** in `base_eligible` (`menger.rs:256-270`): add the conjunct
   `len_to_u16(stats.direct_acceptor_agents.len()) >= config.min_direct_use_acceptors`.
   (The non-circular regression anchor; both necessity and commodity typically pass — it
   is a floor, not the discriminator.)
2. **Leadership numerator** = medium-saleability: when the flag is on, `leader_shares` /
   `share_bps` / `winner` rank candidates by `indirect_acceptances` (medium use) instead
   of combined `acceptances`. Keep the combined path verbatim when the flag is off.
3. **Provisional leader** (`provisional_leader`, `menger.rs:201-214`): when the flag is on,
   select among direct-use-eligible candidates, **exclude goods that fail the marketability
   holding rule as a medium** (compose with `durability_aware_acceptance`; needs the
   marketability table reachable here — pass it in or read off `Society`), rank by indirect
   share, and **seed by direct-use breadth** when all candidates have zero indirect
   acceptances (bootstrap). Off-path stays byte-identical.

Determinism: all new ordering must be total and tie-broken deterministically (mirror the
existing `tied_best`/`lead_margin_bps` discipline at `menger.rs:185-199`); no float, no
map iteration order leaks (use the sorted `Vec` accumulators).

### S21b.2 — Controlled scenario + falsifiable acceptance suite

Extend the S21a controlled `MarketScenario` (the `menger_marketability_*` family,
`econ/src/scenario.rs:1197+`) or add `menger_two_layer_saleability` layering
`two_layer_saleability: true` + `min_direct_use_acceptors` on top of the S21a marketability
scenario (necessity perishable, neutral commodity durable). Acceptance suite in
`econ/tests/two_layer_saleability.rs` (mirror `marketability.rs` helpers):

- `two_layer_off_reproduces_the_finding`: flag off → `saleability_leader == NECESSITY`
  (reproduces `marketability_finding`).
- `two_layer_promotes_the_medium` (THE falsifiable bar): flag on →
  `saleability_leader == COMMODITY` (the medium leads where the necessity led before).
- `the_commodity_promotes_to_money` (stronger): flag on → `current_money_good() ==
  Some(COMMODITY)` (leadership + strong-bar gate → promotion). If the 2-good controlled
  scenario lacks the indirect breadth to promote, compose with the S20 cycle for this
  assertion and keep the leadership flip as the primary bar (call this out, do not fake it).
- `eligibility_floor_is_non_circular`: a candidate with indirect acceptances but **below**
  `min_direct_use_acceptors` direct demand is **not** eligible (proves direct-use is a real
  gate; guards against pure-medium circularity).
- `lever_off_is_byte_identical` / determinism: flag off reproduces the prior trace exactly;
  flag on is deterministic across two runs (mirror `durability_aware_run_is_deterministic`).
- `canonical_bytes_include_two_layer_saleability` (sim test, mirror
  `two_lane_clearing.rs:318`): off == explicit-off, off != on, inert-config(off) == off.

## 5. Determinism / golden contract (must hold)

- Default `false`/`0`; every struct literal across `scenario.rs`/`menger.rs`/`worldgen.rs`/
  `barter.rs` keeps compiling via `..Default::default()`.
- New config enters `canonical_bytes` **only when ON** (append at end, inside
  `if menger.two_layer_saleability`).
- Adding `direct_*` fields to `CandidateStats` changes no digest (not serialized).
- All flag-OFF suites stay byte-identical: g4a_death, g4b_demography, g5a_emergence,
  g5b_frontier, money_coemergence, strong_bar_emergence, producible_capital,
  originary_interest, entrepreneurial_uncertainty, own_labor_subsistence,
  spatial_households, forage_carrying_capacity, pre_money_cultivation,
  money_from_produced_bread, mortality, multigood_money, cycle_money, two_lane_clearing,
  econ marketability. Plus the pinned `goldens_unchanged` digest blocks
  (`two_lane_clearing.rs:376`, `cycle_money.rs:560`).
- `cargo fmt --check` + `cargo clippy --workspace --all-targets -- -D warnings` clean.

## 6. Honest scope (what S21b does and does NOT claim)

- DOES: make money leadership/promotion track **medium-saleability** atop a **non-circular
  direct-use eligibility floor**, resolving the S21a conflation finding in a controlled
  scenario; compose with S21a marketability + S20 two-lane clearing.
- Does NOT claim: full open-colony integration (on-market survival + terminal consumption)
  — that remains the next milestone after this. The direct-use anchor is still the
  S9-style configured heterogeneous use (the *use* is given, not itself emergent). The
  provisional-leader marketability gate is the chosen composition, not the only one (the
  multi-candidate alternative is deferred). Promotion (vs leadership) may require the
  richer cycle for indirect breadth; if so it is reported as such, not forced.

## 7. Pipeline

Codex spec-review (this doc) → revise to SPEC-READY → rb-lite `--implementer codex,claude`
→ independent verification (workspace + all 18 goldens byte-identical + the new suite +
fmt/clippy) → Codex review-of-results → feat commit + merge + report/memory update +
plan-hash pin.
