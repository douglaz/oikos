# impl-26 — S21c: Fix the two-layer open-discovery path-dependence

Status: DRAFT
Branch: `feat/open-discovery-lane`
Base: master @ `e1bc22b` (S21b landed)

## 0. Motivation

S21b's Codex review-of-results flagged a P1 (an overclaim, not a defect): under
`two_layer_saleability` + `multi_offer_medium` candidate mode, indirect-offer
discovery is **path-dependent**. Once `provisional_media_candidates()` is non-empty,
`generate_candidate_direct_barter_offers` (`econ/src/society.rs:2119-2156`) posts
direct/spend offers **only for goods already in the candidate set**, and
`cancel_non_candidate_lane_barter_offers_for_agent` (`society.rs:2525-2534`) cancels
everything else. A good with real direct-use demand that crosses the direct-use floor
**late** is therefore starved of the direct acceptances it would need to ever become a
candidate — discovery closes the moment the first candidate appears.

This is the prerequisite first slice of the open-colony capstone sub-arc (S21c→S21f):
the open colony has many goods, so late-crossing candidates must not be starved.

## 1. The fix (minimal, candidate-mode only)

Add a **legacy direct-discovery lane** for *below-floor* goods, alongside the candidate
spend/sell lanes, and preserve it across ticks:

1. New lane classifier `is_legacy_direct_discovery_lane(offer, candidates)` — a
   `DirectWant` offer whose `give_good` and `receive_good` are **both not in**
   `candidates`. (A purely below-floor↔below-floor direct barter; it cannot spend or
   acquire a candidate medium, so it does not bypass the medium.)
2. `cancel_non_candidate_lane_barter_offers_for_agent` (`society.rs:2525`): preserve
   this lane too — cancel only offers that are neither a candidate spend lane, nor a
   candidate receive lane, nor a legacy direct-discovery lane.
3. `generate_candidate_direct_barter_offers` (`society.rs:2119`): after the per-candidate
   loop, post one legacy direct-discovery offer via `post_first_direct_barter_offer`
   restricted to **below-floor** receive/give goods (filter out `candidates`), gated so
   each agent carries at most one such lane (mirroring the one-offer-per-lane discipline).

INDIRECT/medium routing is untouched — `generate_candidate_indirect_barter_offers` and
`post_first_medium_*` still iterate `candidates` only, so media discovery stays
exclusively through direct-use-eligible candidates. The only added behavior is direct
barter for below-floor goods (the starved population), which lets them accrue direct
acceptances and cross the floor late.

## 2. Determinism / golden contract

- Candidate mode requires `two_layer_saleability && multi_offer_medium` (both off in
  every golden), so **all 18 golden suites stay byte-identical** — this fix cannot touch
  any flag-off path.
- The only behavioral surface is two-layer scenarios (the S21b econ suite + the future
  open-colony scenarios). The S21b suite must still pass (more-open discovery only helps;
  `lever_off_is_byte_identical` is unaffected; the on-path promotion tests still hold).
- New lane posting must be deterministic (sorted give/receive order, one lane/agent).
- `cargo fmt --check` + `cargo clippy --workspace --all-targets -- -D warnings` clean.

## 3. Test (falsifiable bar for this slice)

`econ/tests/two_layer_saleability.rs::late_floor_crosser_is_not_starved` (Codex's
"FOOD crosses first, SALT crosses later only if the discovery lane exists"): a controlled
scenario where one good's direct-use demand only reaches the floor *after* another good is
already a candidate; assert the late good still accrues direct acceptances and becomes a
candidate (and, where the scenario supports it, can still win/promote on medium share).
A companion assertion or the existing suite confirms the prior S21b results are unchanged.

## 4. Pipeline

Direct implementation (small, isolated) → verification (all 18 goldens byte-identical +
S21b suite + the new test + fmt/clippy + workspace) → Codex review-of-results → merge +
docs/memory + pin.
