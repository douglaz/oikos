# impl-27 — S21d: Open-Survival Money Probe (mortality off)

Status: SPEC-READY (Codex round 1 NEEDS-REVISION → round 2 NEEDS-REVISION [Phase A topology +
canonicalization] → addressed: bread⇄WOOD topology with WOOD the sole non-food target, a real
second non-food need scoped out as future work; retire_food_mints canonicalized ON-only; the
acquisition-ledger outflow invariant pinned)
Branch: `feat/open-survival-probe`
Base: master @ `7c208d9` (S21c landed)

## 0. What this milestone is — a PROBE, classify the outcome (likely a FINDING)

The open-colony capstone's first money-bearing slice. Compose the landed money machinery
(S20 two-lane + S21a marketability + S21b two-layer + S21c open-discovery) into a colony where
**agents survive by buying food on the market** — no off-market hearth mint, no own-labor
forage floor — with **mortality OFF** to isolate the money question from the demographic one.

This is a **probe**, and per the Codex direction + spec reviews it is **likely to land as a
finding, not a success**: two-layer saleability fixes the *metric* problem but not the
*production/bootstrap* problem. The deliverable is an honest open colony + instrumentation,
*run*, and the outcome **classified** with traces — either (a) SALT promotes AND the chain
bootstraps under market-financed survival (the capstone result), or (b) a clean deadlock with
the gate localized (a first-class finding: "exactly where the in-cycle result stops surviving
terminal consumption"). **Do not tune the result into existence** — disclose seeds/thresholds,
run the control matrix, classify the failure.

## 1. The two phases and their risks (the phase order is load-bearing)

`econ_tick` order matters and was mis-stated in the first draft (Codex P1). Within a tick:
project input-bid overrides are set **before** `society.step()` (`settlement.rs:6138`); the
market **consume** pass runs **inside** `society.step()` before spot-order generation
(`society.rs:778`). Consequences:

- **The producer bootstrap is necessarily CROSS-TICK.** Food bought in tick `t` cannot free
  money for the input-bid reservation in tick `t`; it is eaten at the start of `t+1`, and only
  then can the input bid post (if money remains). The spec pins this explicitly: *buy food `t`
  → eat `t+1` → input bid `t+1`*. The Exp-9 gate (`reservation_bid_for_money` →
  `allocated_money_before_rank`, `agent.rs:357/889`) bites whenever hunger is still unmet at
  the reservation point — so the question is whether market-financed survival keeps each
  producer fed *across* ticks well enough to ever free input money.
- **Recipe-input demand is POST-promotion only.** `run_role_choice` (`settlement.rs:8878`) and
  `set_project_input_bid_overrides` (`settlement.rs:8614`) both early-return while
  `current_money_good()` is `None`. So miller/baker input purchases — the obvious non-food
  indirect SALT use — **do not exist pre-promotion** and cannot be the source of the
  pre-promotion indirect breadth SALT needs to promote.

**Phase A — pre-promotion (does SALT promote at all?).** With the bread mint retired and no
roles yet, SALT's indirect breadth can come ONLY from **consumer cross-demand over seeded
stocks**. **Topology constraint (Codex round 2):** the value scale (`life/src/scale.rs:194`)
emits present-goods wants only for `known.hunger` (bread), `known.warmth` (WOOD), optional
`known.subsistence` (a food fallback — off here), plus the future savings want and the
heterogeneous SALT direct-use anchor. So **the only non-food terminal consumed good in the
current model is WOOD** — a seeded "craft good" would be *held but never wanted* (no want path).
Phase A therefore uses the existing **bread ⇄ WOOD** topology: cultivators sell bread / want
WOOD, woodcutters sell WOOD / want bread, SALT-anchor consumers buy both. SALT accrues indirect
acceptances when an agent takes SALT as a *means* (`IndirectFor{target}`) — a cultivator selling
bread→SALT to later get WOOD (target WOOD), a woodcutter selling WOOD→SALT to later get bread
(target bread) — so the pre-promotion indirect-target set is `{bread, WOOD}`: **one non-food
target (WOOD)** plus the food target. Two-layer (S21b) ensures SALT leads on *medium* share even
though bread dominates consumption. The bar requires SALT's pre-promotion breadth to **include
the non-food WOOD target** (proving SALT is not merely a bread-buying token); set
`min_indirect_target_goods` to the available topology (disclose the exact value). **Honest bound:
a richer multi-need topology (a genuine second non-food need — clothing/tools) is future work,
not this slice.** Risk: the seeded barter window (bounded by `perishable_decay_bps`) may not
generate enough breadth before the seed depletes → no promotion (a finding); and bread↔WOOD has
*near-perfect* coincidence between the two producer roles (the S18 hazard), so SALT's indirect
use must come from the timing/quantity/consumer-mediated gaps, not the producer pair — itself a
real test.

**Phase B — post-promotion (does the chain bootstrap under market survival?).** After SALT
promotes, roles adopt and producers must (i) buy food to eat and (ii) buy recipe inputs — both
with earned SALT — while the hearth is gone. This re-opens the circular-flow bootstrap of the
long-horizon-death arc, now cross-tick and hearthless. Risk (the likely gate): a producer that
cannot reliably buy food stays hungry, reserves its SALT for food, never bids inputs, the chain
never forms, and post-promotion indirect breadth/production collapses → the colony reverts to
the seeded barter remnant (a finding localized at the Exp-9 gate).

## 2. Engine pieces (small, isolated, gated; goldens byte-identical)

### S21d.0 — Explicit `retire_food_mints` flag (not the forage hack)

Do NOT reuse `own_labor_subsistence=true + with_forage() + forage_yield=0`: even with steering
off, interning FORAGE injects a subsistence good into scales/spoilage/market traces (Codex P1).
Add an explicit gated `ChainConfig::retire_food_mints: bool` (default `false`). When set, the
two staple-mint sites — demographic `food_provision` (`settlement.rs:7326`/`:7339`) and
producer staple (`settlement.rs:7950`/`:7974`) — are skipped, **independent of forage**
(`food_provision`/`producer_subsistence` quantities ignored for the staple). WOOD/warmth
provision is unaffected (warmth is out of scope for this probe; disclose it). No FORAGE good is
interned, so no forage steering/credit/scale pollution. Default `false` ⇒ all goldens
byte-identical. The suite asserts: with the flag on, the food-mint endowment term is zero and
no FORAGE good exists in the run.

**Canonicalization (Codex P2 — MANDATORY, not conditional):** `retire_food_mints` changes a
recurring future-behaviour source (the staple mint), so it **must** enter `canonical_bytes`
ON-only — append it (and only when `true`) in the relevant `push_*_config_bytes`, mirroring the
S20 `multi_offer_medium` / S21a / S21b gating, so every flag-off golden keeps its exact prior
byte layout. Add a `canonical_bytes_include_retire_food_mints` regression (off == explicit-off,
off != on).

### S21d.1 — Acquisition-channel provenance (sim-side, runtime-only)

Mirror `BreadProvenance` (the post-`society.step()` readback at `settlement.rs:9846`), NOT
econ-internal hooks. A sim-side, runtime-only per-agent ledger classifying each food unit by
**acquisition channel**: `bought` (entered stock via a `Society::trades`/`barter_trades`
record), `seeded/minted` (cold-start buffer or hearth), `self-produced` (chain/cultivation), or
`foraged` (n/a here). Updated each tick from the sim's own trade + production + endowment logs;
debited **FIFO** against the consumption-log readback so resale/mixed stock can't misattribute.
**Conservation invariant (Codex P2):** *every* outflow of the tracked food good debits the
channel ledger — consumption, sale/barter transfer, spoilage, estate settlement, and any
birth/endowment transfer — so the per-channel sum stays equal to held tracked-food and "bought
food consumed" cannot be overstated by an untracked outflow. Excluded from `canonical_bytes`
(like `starvation_deaths_total`, `settlement.rs:4242`) ⇒ no golden digest shift. The bar reads: **after warm-up, food consumed by survivors is
overwhelmingly `bought`**, `seeded/minted`+`foraged` ≈ 0, and buyers paid from prior-sale
proceeds.

## 3. The scenario — `frontier_open_survival` (a `SettlementConfig`)

Derive from `frontier_coemergent_strong` (`settlement.rs:3196`) — it has terminal bread for
survival, the real grain→flour→bread chain, WOOD as a second consumed good, and the
co-emergent sustain stack (`recurring_motive`, `project_input_bids`, `perishable_decay_bps`,
capital). Codex confirmed this is the right base (the 3-good cycle has no terminal consumer).
Changes:

- **Retire the hearths (market survival):** `retire_food_mints = true` (S21d.0). Survival of
  every agent — producers included — is now a market bread purchase.
- **Compose the money machinery:** `multi_offer_medium = true` (S20); `durability_aware_acceptance
  = true` + a marketability table (SALT durable/costless; FOOD perishable; WOOD high-carry)
  (S21a); `two_layer_saleability = true` + `min_direct_use_acceptors` (S21b); the S21c lane is
  already in. Keep the S9 strong-bar gates (disclose exact values).
- **Pre-promotion indirect breadth (Phase A) — the deliberate design point:** the seeded barter
  economy uses the **bread ⇄ WOOD** topology (the only non-food terminal good is WOOD; see §1).
  Role-separated so no good preempts another by id (the S16/S18 ordering fix). SALT's
  pre-promotion indirect-target set is `{bread, WOOD}`; the bar requires the non-food WOOD target
  present. (Recipe inputs cannot serve here — they are post-promotion.) A genuine second non-food
  need is explicitly out of scope (future work).
- **Mortality OFF:** inherit `hunger_critical = need_max + 1` (do NOT derive from
  `frontier_mortality`).
- **Disclosed cold-start seeds** (bounded by `perishable_decay_bps = 1500`): `bread_buffer`,
  `consumer_staple_buffer`, `consumer_medium_endowment` (SALT), producer input buffers,
  `latent_flour_seed`. Pin exact values in the scenario doc-comment; the suite reports seed
  depletion over time separately so a "seed-only" non-result is visible.

## 4. Falsifiable bar + controls (Codex direction + anti-fake additions)

Success (capstone result) = ALL hold in one run:
- No recurring food mint (food-mint endowment term zero); no forage good/steering.
- `current_money_good() == Some(SALT)` (promotes).
- FOOD/WOOD may win **total** acceptance, but SALT wins **medium** share
  (`medium_leader_shares().good == SALT`).
- Pre-promotion SALT indirect breadth includes the **non-food WOOD target** (target set
  `{bread, WOOD}`), proving SALT is not merely a bread-buying token.
- After warm-up, food **consumed** is **market-acquired** (acquisition ledger: bought ≫
  seeded/minted ≈ 0); buyers paid from prior-sale proceeds.
- **Production is genuinely post-promotion and self-sustaining** (chain output continues past
  seed depletion, not riding the seed).

Anti-fake assertions (Codex P2):
- **No pre-promotion production** while relying on seeded stock; **seed depletion reported
  separately** so a seed-only outcome cannot masquerade as success.
- **Bootstrap microtrace (S21d.2a):** a producer bought food in `t`, ate it in `t+1`, then did
  or did not post an input bid in `t+1` — directly localizing the Exp-9 gate.

Controls (each must fail the right way — classify, never tune):
- two-layer off → necessity dominates / no SALT promotion.
- marketability off → FOOD/WOOD dominates as medium.
- multi-offer off → round-trip clearing deadlock.
- no WOOD cross-demand (bread-only) → direct trade, no indirect breadth (the S16 control).
- no SALT direct-use anchor/seed → no promotion (regression-theorem grounding).
- mints ON (`retire_food_mints=false`) → the old scaffolded control, NOT a capstone success.

If the bar is not met, classify the gate (Phase A no-promotion vs Phase B bootstrap deadlock)
with the live traces and land it as a finding, as the long-horizon-death experiments did.

## 5. Slices (per Codex)

- **S21d.0** — `retire_food_mints` flag (engine, gated; goldens byte-identical; assert no
  food-mint endowment + no FORAGE good when on).
- **S21d.1** — acquisition-channel ledger (sim-side, runtime-only, FIFO at consume readback;
  excluded from `canonical_bytes`).
- **S21d.2a** — phase-order **bootstrap microtrace** harness + test (the cross-tick buy→eat→bid
  sequence; localizes the Exp-9 gate).
- **S21d.2b** — the `frontier_open_survival` scenario (compose flags + retire mints +
  ≥2-non-food cross-demand + disclosed seeds).
- **S21d.3** — acceptance suite + the run: assert the bar OR classify the gate; full control
  matrix; pre-promotion seed/indirect-breadth traces; determinism; a live `viewer run`.

## 6. Determinism / golden contract

- All new flags/instruments default OFF / runtime-only; **all 18 golden suites byte-identical**
  (`retire_food_mints` default-off is identity; the acquisition ledger is excluded from
  `canonical_bytes`; the scenario is new). `retire_food_mints` is canonicalized **ON-only**
  (mandatory, §2 S21d.0) exactly like the S20/S21a/b flags, with a dedicated digest regression.
- `cargo fmt --check` + `clippy --workspace --all-targets -- -D warnings` clean; conservation
  asserted every tick; deterministic (no live RNG).

## 7. Honest scope

Tests whether endogenous medium money survives **market-financed survival** in an open colony
with mortality OFF. It does NOT add the positive check (S21e) or claim demographic realism.
**A clean deadlock is the expected, publishable result** identifying where the in-cycle money
result stops surviving terminal consumption (most likely the Phase B cross-tick producer
bootstrap). Seeds, thresholds, and the direct-use anchor remain configured and disclosed. The
faithful response to a Phase B deadlock is NOT value-scale surgery (fake entrepreneurship) — it
is to report the gate and let a later slice address the institution (e.g. a wage/firm or a
genuine market food-supply path), exactly as the long-horizon-death arc proceeded.

## 8. Pipeline

rb-lite `codex,claude` (slices S21d.0→.3) → independent verification (workspace + all 18
goldens byte-identical + the new suite + a live run) → Codex review-of-results → merge +
report/memory + pin. Given the likely-finding framing, the review-of-results judges *honesty +
correct classification*, not "did money emerge."
