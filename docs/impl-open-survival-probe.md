# impl-27 — S21d: Open-Survival Money Probe (mortality off)

Status: DRAFT (pre-Codex-spec-review)
Branch: `feat/open-survival-probe`
Base: master @ `7c208d9` (S21c landed)

## 0. What this milestone is — a PROBE, classify the outcome

The open-colony capstone's first money-bearing slice. Compose the landed money machinery
(S20 two-lane clearing + S21a marketability + S21b two-layer saleability + S21c
open-discovery lane) into a colony where **agents survive by buying food on the market** —
no off-market hearth mint, no own-labor forage floor — with **mortality OFF** to isolate the
money question from the demographic one.

This is a **probe**: the deliverable is an honest open colony plus instrumentation, *run*,
and the outcome **classified** — either (a) SALT monetizes under market-financed survival
(the capstone result), or (b) it deadlocks, with the gate identified (a first-class finding,
per the direction review: "if it fails cleanly, that shows exactly where the in-cycle result
stops surviving contact with terminal consumption"). **Do not tune the result into
existence** — disclose seeds/thresholds, use the control matrix, classify the failure.

## 1. The core tension this probe tests (from research)

1. **The S12 collision (partly dissolved, not gone).** Retiring the demographic bread mint
   killed SALT emergence under the *single-layer* metric because that mint was the
   load-bearing pre-promotion bread supply (`own_labor_subsistence.rs` ablation). S21b's
   two-layer metric removes the *metric* half (food can dominate consumption while SALT
   leads on medium use), but **produced bread alone is direct exchange** — so SALT's
   medium-saleability must come from **non-food** indirect demand (recipe inputs + WOOD),
   not bread-for-SALT.
2. **The hungry-producer bootstrap gate (THE #1 risk).** Proven (Exp-9,
   `experiment-money-circulation.md:380-387`): a hungry producer reserves its money for its
   own food want and posts **no input bid** — `reservation_bid_for_money`
   (`econ/src/agent.rs:357`) returns `None` because `allocated_money_before_rank`
   (`agent.rs:889`) protects cash earmarked for the higher-ranked hunger want. The faithful
   fix has always been *feed the producer* (both `frontier_endogenous` and
   `frontier_coemergent_strong` keep `producer_subsistence > 0`). Market-financed survival
   must satisfy hunger **by purchase** so the producer frees money for inputs — which
   re-opens the circular-flow bootstrap of the long-horizon-death arc. **Within-tick
   sequencing matters:** if the input-bid reservation is computed while hunger is still
   unmet, the producer posts no bid even if it could buy food that tick. This is the gate to
   design against / measure.
3. **Roles wait on money** (`run_role_choice` early-returns while `current_money_good()` is
   `None`, `settlement.rs:8886`): no production before promotion. The pre-promotion barter
   warm-up must generate SALT's indirect breadth from the seeded stocks alone.

## 2. Engine pieces required (small, isolated, gated)

### S21d.0 — Decouple forage *steering* from mint retirement (config-correctness)

`own_labor_subsistence_can_run()` (`settlement.rs:9770`) = `own_labor_subsistence &&
forage_present` gates BOTH (a) mint retirement (`settlement.rs:7326`, `:7950`) and (b) forage
behavior (`detect_forage` `:6596`, steering `:6659`/`:6870`, credit `run_own_labor_subsistence`
`:8034`). Setting `forage_yield = 0` retires the mints but **still steers eligible agents to
`Task::GoForage`** (walk + harvest zero) — wasted labor that confounds a market-survival
probe. Minimal decouple (`own_labor_subsistence_fields_active`, `settlement.rs:12937`): keep
the two mint sites on `_can_run()`, but gate the three forage-*behavior* sites additionally on
`forage_yield > 0` (or a new explicit `forage_enabled`/`forage_floor_can_run()` predicate). At
`forage_yield = 0`: mints retired, **no forage steering**, survival is market-only. Default
behavior (yield > 0) unchanged → all goldens byte-identical (gate off-path identical).

### S21d.1 — Acquisition-channel provenance (the market-sourced-food instrument)

The existing `BreadProvenance` (`settlement.rs:4509`) tracks *produced-vs-minted* origin of a
good **sold** for the medium; it does **not** prove that food an agent **consumed** was
**market-acquired** vs minted/foraged/self-produced (a transfer preserves the seller's
origin). The falsifiable bar needs an **acquisition-channel** tag at the consume event:
classify each unit of food eaten as `bought` (entered stock via a `Society::trade`),
`self-produced`, `minted` (hearth/seed), or `foraged`. Minimal: a per-agent acquisition ledger
updated on market execution (`market.rs:574`) and read at the consume event
(`agent.rs:738`); runtime-only (excluded from `canonical_bytes`, like `starvation_deaths_total`)
so it shifts no golden digest. The bar: **after warm-up, the food consumed by survivors is
overwhelmingly `bought`**, with `minted`/`foraged` ≈ 0.

## 3. The scenario — `frontier_open_survival` (a `SettlementConfig`)

Base on the chain coemergent economy (it has terminal **bread** consumption for survival, the
sustain stack, an **input-demand** structure giving SALT non-food uses, and WOOD as a second
consumed good — richer than a two-good bread↔WOOD economy, which Codex warned reproduces S18).

Derive from `frontier_coemergent_strong` (`settlement.rs:3196`) and change:
- **Retire the hearths (market survival):** `own_labor_subsistence = true` + `with_forage()`
  interned + `forage_yield = 0` (S21d.0 ⇒ mints off, no forage floor, no forage steering).
  This sets both the demographic `food_provision` mint and the `producer_subsistence` staple
  to off.
- **Compose the money machinery:** `multi_offer_medium = true` (S20), `durability_aware_acceptance
  = true` + a marketability table (SALT durable/costless, FOOD perishable, WOOD high-carry)
  (S21a), `two_layer_saleability = true` + `min_direct_use_acceptors` (S21b); S21c lane is
  already in. Keep the S9 strong-bar gates (disclose the exact values).
- **Terminal food consumer + non-food indirect demand:** ensure roles cross-demand so SALT
  bridges *non-food* coincidence gaps (recipe-input purchases: miller buys grain, baker buys
  flour; plus WOOD for warmth). The pre-promotion indirect breadth must span ≥2 non-food
  targets, not just bread.
- **Mortality OFF:** inherit `hunger_critical = need_max + 1` (do NOT derive from
  `frontier_mortality`).
- **Disclosed cold-start seeds** (carried forward, bounded by `perishable_decay_bps = 1500`):
  `bread_buffer`, `consumer_staple_buffer`, `consumer_medium_endowment` (SALT), producer
  input buffers, `latent_flour_seed`. List exact values in the scenario doc-comment.

## 4. Falsifiable bar + control matrix (Codex direction)

Success (capstone result) = ALL hold in one run:
- No recurring food mint (`food_provision`/producer staple off); no forage floor
  (`forage_yield = 0`, no steering).
- `current_money_good() == Some(SALT)` (SALT promotes).
- FOOD/WOOD may dominate **total** acceptance, but SALT wins **medium** share
  (`medium_leader_shares().good == SALT`).
- Pre-promotion SALT indirect breadth spans **≥2 target goods, not only bread** (non-food
  ends).
- After warm-up, food **consumed** is **market-acquired by the acquisition ledger** (bought ≫
  minted/foraged ≈ 0); buyers paid with proceeds of prior sales.

Controls (each must fail the right way — classify, don't tune):
- two-layer off → necessity dominates / no SALT promotion.
- marketability off → FOOD/WOOD dominates as the medium.
- multi-offer off → round-trip clearing deadlock.
- no second-good/input loop → direct food trade but no indirect breadth.
- no SALT direct-use anchor/seed → no promotion (regression-theorem grounding).
- mints ON → the old scaffolded control (NOT a capstone success).

If the bar is NOT met, classify the gate (most likely the §1.2 bootstrap: producers reserve
money for unbought food → no input bids → chain never forms → no indirect SALT breadth) and
land it as a **finding** with the live trace, exactly as the long-horizon-death experiments did.

## 5. Slices

- **S21d.0** — forage-steering decouple (engine, gated; goldens byte-identical).
- **S21d.1** — acquisition-channel provenance instrument (runtime-only; goldens byte-identical).
- **S21d.2** — the `frontier_open_survival` scenario (compose the flags + retire hearths +
  cross-demand structure + disclosed seeds).
- **S21d.3** — acceptance suite + the run: assert the bar OR classify the gate; the full
  control matrix; determinism; a live `viewer run` trace for the headline numbers.

## 6. Determinism / golden contract

- All new flags/instruments default OFF / runtime-only; **all 18 golden suites byte-identical**
  (the forage decouple is identity at `forage_yield > 0`; the acquisition ledger is excluded
  from `canonical_bytes`; the scenario is new).
- `cargo fmt --check` + `clippy --workspace --all-targets -- -D warnings` clean; conservation
  asserted every tick; deterministic (no live RNG).

## 7. Honest scope

This probe tests whether endogenous medium money survives **market-financed survival** in an
open colony with mortality OFF. It does NOT add the positive check (S21e) or claim demographic
realism. A clean deadlock is a valid, publishable result identifying where the in-cycle money
result stops surviving terminal consumption. Seeds, thresholds, and the direct-use anchor
remain configured and disclosed.

## 8. Pipeline

Codex spec-review → revise to SPEC-READY → rb-lite `codex,claude` (slices S21d.0→.3) →
independent verification (workspace + all 18 goldens byte-identical + the new suite + a live
run) → Codex review-of-results → merge + report/memory + pin.
