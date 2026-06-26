# impl-34 — S22b: Occupational Stickiness via Bounded Cultivation Skill (does accumulated advantage turn fluid participation into a stable role split?)

Status: LANDED — verdict **NO STICKINESS DESPITE SKILL** across all SEEDS. The lever BITES (the
mandatory non-vacuity test passes: a SKILL_CAP cultivator harvests strictly more grain — 144 vs 72,
exactly 2× the per-trip haul — AND produces strictly more bread than a skill-0 cultivator on every
seed), and money + mortality + provenance + conservation all survive. But under the FLUID
hunger-driven participation S22a found, skill never matures: an agent cultivates only in short spells,
so per-tick decay erodes the gain between them and the steady-state skill stays low (max ~180 ≪ the
500 maturity, mature-cohort count 0). Churn therefore stays at the matched-seed S22a baseline (≈2.7,
no fall toward the 0.5× drop bar) and no persistent membership cohort forms — accumulated
harvest-efficiency advantage ALONE does not turn fluid self-provisioning into a stable occupation. The
no-decay control confirms the mechanism: with decay off, skill ratchets to the cap (mature cohort
forms, ~36% grain share — a diagnostic upper-bound approximation, attributed at horizon-maturity) yet it
is STILL NoStickinessDespiteSkill (no persistent cohort, churn unchanged, no monopolization). The robustness sweep maps the boundary — a starved grain node tips to
OSCILLATION (intense competition over a depleting commons), abundant grain stays NoStickinessDespiteSkill;
low-grain-flow correctly classifies CommuneCollapse (not faked success). All five tripwire goldens are
byte-identical; off the flag the chain is byte-identical to the S22a stream. The honest next boundary:
stickiness needs a stronger lock-in than a soft, decaying harvest-efficiency skill (heritable skill,
durable capital, or a profit-driven role chooser — the S22c+ scope) OR a participation regime that
holds an agent cultivating long enough for skill to mature.

Codex review-of-results: **PASS-WITH-CAVEATS** (no P1/P2 code-correctness issue — the per-trip
over-carry path is conserved: the node is debited, and death/estate collection drains arbitrary carried
quantities via `withdraw_agent_carry`/`remove_agent`; no leak/double-count/digest problem). Three P3
framing fixes folded in: the stale "~23" churn references reworded to the matched-seed `churn_per_capita`
≈2.7 baseline (the ~23 was a different S22a diagnostic); the non-vacuity claim reworded from "per
cultivating opportunity" to "matched conditions over the same horizon"; the grain-share scoped as a
horizon-maturity diagnostic upper-bound approximation (it only feeds the MONO_SHARE+damage cull check,
so it cannot fake the verdict).

Prior status: SPEC-READY — Codex spec-review NEEDS-REVISION → five decisions settled (§8) and the 6-item
punch-list folded in: a MANDATORY non-vacuity test + a distinct LEVER-INERT outcome (§2/§7); skill
credits ACTUAL realized cultivation output, not the `cultivating` flag (§3.2); cap-zero control is
behavior-identical, not byte-identical (§5/§7); churn compared to the MATCHED-SEED S22a baseline
(§2/§3.3/§7); pinned thresholds (§8); membership-based persistent cohort + dominance-AND-damage
monopolization (§2/§3.3). Round 2 (NEEDS-REVISION): the lever changed from harvest-WANT-scaling (inert —
`carry_room` is the binding cap) to **grain-haul-capacity** scaling (raise the effective
`carry_cap`/`carry_room` for the grain trip), so it actually moves more conserved grain; non-vacuity
test unchanged (§1/§3.2/§8). Round 3 (NEEDS-REVISION → conditionally pre-approved): pinned the exact
engine seam — a gated per-trip `Task::GoHarvestWithRoom(node, want, room_cap)` override for the
cultivating grain trip (NOT mutating the agent's permanent `carry_cap`), since `Task::GoHarvest` still
calls `agent.carry_room()` (§3.2).

## 0. One-paragraph summary

S22a relaxed the pinned cultivator-lineage privilege and found **fluid/rotating participation**, not a
stable occupation: ~5% cultivate at any instant but *all* non-lineage roles rotate through (churn
~23/agent) — "everyone occasionally self-provisions under acute hunger, then returns to buying." Codex's
read: hunger pressure alone produces *survival behavior, not occupation*; the minimal **authentic**
mechanism for stickiness is **role-specific accumulated advantage** (capital/skill lock-in), not a
generic switching penalty. S22b adds the cleanest such mechanism: a default-off, per-agent, **bounded
cultivation skill** that accumulates while an agent cultivates and slowly decays while it does not, and
that raises **grain harvested per cultivating tick** (the one conservation-safe lever — a faster draw on
the conserved grain-node regen, *not* a higher bread-per-grain ratio). The central question: **does mild
accumulated productivity turn S22a's fluid self-provisioning into a stable role split — a persistent
cultivator cohort plus persistent non-cultivating buyers — while preserving money, mortality, and
provenance?** It is **classify-not-tune**: reuse house-style bounded accumulate/decay params, predeclare
the stickiness thresholds, sweep them, and accept "no stickiness" / "commune" / "monopolization" /
"money failure" as first-class findings. This is still cultivation *participation with accumulated
advantage*, not the full vocation topology (profit-driven drift, Miller/Baker entry, the global role
chooser remain S22c+).

## 1. Why this milestone, why this lever

Codex's state review named S22b the highest value × tractability next step and the one result that most
strengthens the eventual paper: it directly answers the reviewer challenge after S22a ("you removed
assigned cultivators, but you still don't have durable specialization"). The lever choice is forced by
two constraints:

- **Conservation.** Cultivation is a fixed 1-grain→1-bread recipe (`GRAIN_PER_CULTIVATE = 1`,
  `BREAD_PER_CULTIVATE = 1`, `CULTIVATE_LABOR = 2`; `content.rs`). Multiplying bread *output* per recipe
  mints bread (`produced ≠ consumed_as_input`) — inflationary, breaks the per-tick conservation
  identity. The **only** conservation-safe productivity lever is **grain hauled per cultivating trip**:
  the grain node is a conserved source (`stock`/`regen`/`cap`), and harvest is
  `min(want, node.stock, carry_room)` (`world.rs`). NOTE (Codex spec-review): `carry_room` is the binding
  cap, so scaling the *want* alone is **inert** (it cannot exceed carry_room); the lever must scale the
  cultivating agent's effective **grain-haul capacity** (`carry_cap`/`carry_room` for the grain
  `GoHarvest`), so it moves more *conserved* grain per trip — still bounded by `node.stock`, a faster
  draw on the regen, no minting.
- **Authenticity.** Per Codex, stickiness must come from *accumulated advantage*, not fiat. A skilled
  cultivator out-harvests the shared grain regen, so cultivating is rewarding for the skilled and
  unrewarding for an unskilled entrant — persistence and a producer cohort can self-select. The existing
  engine precedent is **tools** (durable role-specific capital gating 3× recipes); skill is the softer,
  accumulating analogue.

## 2. The central question and pre-named outcomes

**Central question.** Does bounded per-agent cultivation skill (accumulate-while-cultivating,
decay-while-not, capping grain-harvest efficiency) turn S22a's **fluid** participation into a **stable**
role split — a persistent cultivator cohort *and* persistent non-cultivating buyers — while SALT still
promotes on clean `SelfProduced` bread, mortality is survived, and conservation holds?

**SUCCESS** (all of, across `SEEDS`, vs the S22a baseline):
1. **Churn falls materially** — per-ever-cultivating-agent enter/exit churn drops to ≤ `CHURN_DROP` ×
   the **matched-seed skill-off baseline** (§7), i.e. agents stop rapidly rotating. (NOTE: this suite's
   `churn_per_capita()` metric reads ≈2.7 on the skill-off baseline; the "~23/agent" figure in the S22a
   report was a different, un-normalized churn diagnostic — the comparison here is always matched-seed,
   same metric on both sides, so the two numbers are not directly comparable.)
2. **A persistent cultivator cohort forms** — a set of agents cultivates for ≥ `PERSIST_FRACTION` of the
   final window (sticky membership), distinct from a non-cultivating buyer cohort that persists too.
3. **Money survives** — SALT promotes on clean (`seeded_minted == 0`) `SelfProduced` bread; food is
   materially bought after promotion; the WOOD↔SALT lane clears.
4. **Mortality survived** — no demand-side cull / extinction; a living non-cultivating buyer cohort
   persists.
5. **No monopolization cull** — the skilled cohort does not starve the rest by monopolizing grain (the
   non-cultivators survive and buy; living non-lineage does not collapse).
6. **Guards hold** — conservation every tick, `bread_minted_max == 0`, provenance clean, `!extinct`.

**Finding modes (pre-named; each a first-class result):**
- **LEVER INERT** — skill does **not** measurably increase a high-skill cultivator's harvested grain /
  produced bread vs a skill-0 cultivator (output is bound by the labor ceiling / node regen / carry, not
  by harvest want), so the productivity differential the milestone rests on does not exist. This is
  detected by the **mandatory non-vacuity test** (§7) and is a *distinct* outcome from "no stickiness" —
  it says the lever itself is the wrong knob (the S21i wood_buffer-vacuity lesson), and the milestone
  must report it as such and pivot the lever (labor-cost) in S22b-redux rather than pretend inertness is
  an economic finding.
- **NO STICKINESS DESPITE SKILL** — the lever *does* bite (non-vacuity passes) but churn stays ~S22a
  (within `CHURN_DROP` of the matched-seed baseline) and no persistent cohort forms; skill at this
  magnitude is not enough to overcome the hunger dynamics. (A real economic result: harvest-efficiency
  advantage alone does not produce occupation.)
- **COMMUNE COLLAPSE** — skill makes cultivation so rewarding that most agents become cultivators,
  bought food collapses, money fails/irrelevant.
- **MONOPOLIZATION CULL** — early skilled incumbents monopolize the grain regen; unskilled entrants get
  too little and the non-cultivating side starves (an S21g-like cull returns by a new cause).
- **MONEY FAILURE FROM LOCK-IN** — roles persist but the persistence weakens the WOOD/bread exchange
  enough that SALT fails to promote (specialization without the medium).
- **OSCILLATION** — skill + decay interact to produce churn that never settles (the no-decay vs decay
  controls disambiguate).

**Precondition — LEVER INERT (checked before per-seed classification).** If the mandatory non-vacuity
test (§7) shows skill does not increase actual harvested grain + produced bread vs skill-0 under matched
conditions, the milestone headline verdict is **LEVER INERT** and the per-seed scenario classification is
reported but moot (the lever is the wrong knob; pivot to labor-cost in S22b-redux). Only if the lever
bites does the per-seed ordered classifier below assign the regime.

**Ordered classifier (mutually exclusive, top-down — the S21i/S22a non-gameability discipline):**
1. **BROKEN-INVARIANT / EXTINCT** — any guard fails.
2. **MONOPOLIZATION CULL** — top-skill-cohort grain share ≥ `MONO_SHARE` (75%) **AND** living
   non-lineage / material buyers below the survival/buyer floor (dominance **and** damage — dominance
   alone with surviving buyers is not a cull).
3. **COMMUNE COLLAPSE** — rolling cultivator share ≥ `COMMUNE_SHARE` AND post-promotion bought <
   `MATERIAL_BOUGHT_FLOOR`.
4. **MONEY FAILURE FROM LOCK-IN** — `SelfProduced` bread produced + sold but `current_money_good() !=
   SALT` at horizon.
5. **OSCILLATION** — churn ≥ `CHURN_LIMIT` AND no settled cultivator-share band in the final window.
6. **NO STICKINESS DESPITE SKILL** — money + mortality survive but churn > `CHURN_DROP × (matched-seed
   S22a baseline churn)` (no material reduction) AND no persistent **membership** cohort (the same agent
   ids do not cross `PERSIST_FRACTION`).
7. **SUCCESS** — none of the above AND §SUCCESS 1–6 hold (churn fell ≤ `CHURN_DROP ×` the matched-seed
   baseline, a persistent membership cohort of `PERSIST_COHORT` formed, money + mortality survive, no
   monopolization).

The verdict test computes this order, prints the verdict + deciding metrics, and **does not assert
SUCCESS**. All thresholds are pre-stated in §7, not fitted. Churn comparisons are always against the
**matched-seed** skill-off S22a run (seed effects must not masquerade as stickiness).

## 3. What gets built

### 3.1 The gate (additive, default-off, canonicalized ON-only)

- New `ChainConfig` flag `cultivation_skill: bool` (default `false`) + a `cultivation_skill_active(&self)`
  helper (active only when the flag is on AND `endogenous_cultivation_entry_active()` — S22b composes on
  S22a). Canonicalized **ON-only** with the next free digest tag (8 unless master advanced; tags 2–7
  used by S21d/e/f/h + S22a) + the skill parameters; per-agent skill state is serialized into
  `canonical_bytes` **only when the gate is on** (the same gate as the existing `cultivating` /
  `cultivate_pressure` per-colonist block). Off ⇒ byte-identical to the S22a stream.

### 3.2 The per-agent skill scalar (imitating cultivate_pressure / throughput / the cap idiom)

- A new per-`Colonist` field `cultivation_skill: u16` (born at 0 — skill is *earned*, not inherited;
  heritable skill is deferred). Pinned magnitudes: `SKILL_CAP = 1000`, `SKILL_GAIN = 50`,
  `SKILL_DECAY = 5` (`DECAY < GAIN`, so sustained cultivation builds a durable advantage that idleness
  erodes only gradually). Update rule each econ tick under the gate:
  - **Accumulate on ACTUAL productive experience, not the `cultivating` flag** (Codex P1): credit
    `skill = (skill + SKILL_GAIN).min(SKILL_CAP)` only on a tick where the agent **actually harvested
    grain and converted it to bread** (a realized own-use cultivation output > 0), so an agent that is
    flagged `cultivating` while walking, blocked, carrying, or sitting on a depleted node earns nothing.
    (Saturating, bounded — the `cultivate_pressure` / `tools_built.saturating_add` idiom, but gated on
    realized output.)
  - **Decay** on any tick without realized cultivation output: `skill = skill.saturating_sub(SKILL_DECAY)`.
- **Effect (the conservation-safe lever — grain-haul capacity, not just want):** a cultivating agent's
  effective **grain-haul capacity** for the grain `GoHarvest` is scaled by its skill —
  `haul = carry_cap + (carry_cap * skill / SKILL_CAP)`, capped at `2 × carry_cap` at full skill — and
  **both** the GoHarvest `want` AND the effective `carry_room` used for that grain trip are raised to
  `haul` (Codex P1: scaling the want alone is inert because `carry_room` is the binding cap; the haul
  capacity itself must rise). **Pinned engine seam (Codex round 3):** the current
  `Task::GoHarvest(node, want)` calls `agent.carry_room()` in `world.rs` (~650), so raising `want` alone
  stays inert — the milestone MUST add a **gated per-trip harvest-room override** for the cultivating
  grain trip: a new `Task::GoHarvestWithRoom(node, want, room_cap)` variant (or thread a per-trip
  `room_cap` into the existing grain-harvest path), used ONLY for a skilled cultivating agent's grain
  trip, with `want = room_cap = haul`. It **must NOT mutate the agent's permanent `carry_cap`** (it is a
  per-trip cap for that grain harvest only). The world harvest then becomes `min(want, node.stock,
  room_cap) = min(haul, node.stock)` — a skilled agent moves more of the conserved grain per trip,
  bounded by node stock. **No change to the 1:1 grain→bread recipe, no new `produced` term, the node is still debited** —
  conservation is untouched by construction (a hard test guard confirms it every tick). The *effective*
  bread gain is then capped by `min(grain_hauled, labor_budget / CULTIVATE_LABOR)` and may be **less**
  than 2× — which is exactly why the §7 non-vacuity test is mandatory (skill must be shown to move more
  grain AND produce more bread, else LEVER INERT).

### 3.3 Diagnostics (runtime-only, never digested)

Reuse the S22a diagnostics + add, for the stickiness verdict:
- **per-agent skill distribution** at horizon (max / mean / count above a maturity threshold).
- **persistent cultivator cohort (MEMBERSHIP, not aggregate share)** — the count of distinct agent
  **ids** that each individually cultivate in ≥ `PERSIST_FRACTION` of the final-window ticks. A stable
  aggregate ~5% share with *rotating* members is S22a again, NOT a cohort (Codex P2) — the metric tracks
  per-id persistence, so the same agents must keep cultivating.
- **skilled-cohort grain share** — fraction of harvested grain taken by the top-skill cohort over the
  final window (the monopolization probe; paired with non-lineage survival for the cull verdict).
- **churn vs matched-seed S22a baseline** — the headline stickiness metric: each treatment seed's churn
  is compared to its OWN skill-off S22a run (not a hard-coded ~23), so seed effects can't masquerade as
  stickiness.

### 3.4 The scenario

`frontier_occupational_stickiness` deriving from `frontier_endogenous_cultivation` (S22a): identical
except `cultivation_skill = true`. Mortality on, S20/S21a/b/c money, emergency floor, WOOD-poor topology.

## 4. The skill dynamics (bounded, house-style, not tuned)

- `SKILL_GAIN`, `SKILL_DECAY` (with `DECAY < GAIN`), `SKILL_CAP`, and the harvest-multiplier ceiling are
  **predeclared constants** chosen to be modest (a small per-tick gain, a slow decay, a bounded ≤2×
  harvest ceiling) — NOT hand-tuned until "just enough" stickiness appears. The robustness mini-sweep
  (§5) sweeps them and reports the regime map; commune/monopolization/no-stickiness outcomes are
  findings, not bugs.
- Skill is bounded (`.min(SKILL_CAP)`), saturating, deterministic (no RNG; the `cultivate_pressure`
  pattern), and born at 0. The opportunity cost stays structural (a cultivating tick still consumes the
  one world-task slot).

## 5. Controls (classify, never tune)

- **Skill-off control** = `frontier_endogenous_cultivation` (S22a) — must reproduce the fluid-participation
  baseline (the matched-seed `churn_per_capita()` ≈2.7 on this suite's metric, success-but-fluid),
  establishing the comparison every treatment seed is measured against.
- **Stickiness ON** = `frontier_occupational_stickiness` — the treatment, classified vs the S22a baseline.
- **No-decay vs decay** — `SKILL_DECAY = 0` vs the shipped decay: isolates whether decay is what makes
  the cohort *stable* (with no decay, skill ratchets monotonically — does that over-lock or monopolize?).
- **Cap-zero control** — `SKILL_CAP = 0` (or the multiplier ceiling = 1×): skill has no productivity
  effect ⇒ must reproduce S22a's **behavior/metrics** (churn, cultivator share, promotion, buying), the
  no-op proof the lever is the cause. NOTE (Codex P2): this is **behavior/metrics-identical, not
  byte-identical** — with the skill flag ON the gate still serializes config/state into the digest, so
  the digest legitimately differs from S22a; only a flag-OFF chain is byte-identical.
- **Exaggerated-cap sensitivity** — a large harvest multiplier (e.g. 4×): marked SENSITIVITY (likely
  monopolization/commune), excluded from the core verdict — shows the boundary.
- **Low/no grain-flow control** — starved grain node: skill must NOT fake success (provenance + buying
  collapse; everyone cultivates a depleted node — CommuneCollapse).

## 6. Determinism & goldens (the safety argument)

One additive default-off gate + one new per-`Colonist` field (born 0, serialized ON-only under the gate)
+ runtime-only diagnostics. Off ⇒ no tag 8, no skill bytes, the new field is 0 and never read ⇒ **all
five pinned goldens byte-identical** (`lineages()`@300/@800, `frontier()`@300,
`frontier_spatial_households()`@300, `viable()`@60), asserted in the new suite. The new scenario's digest
is new (it gates a real behavior change). Conservation is preserved **by construction** (skill only
scales the cultivating agent's grain-haul capacity — a bigger debited draw on the conserved grain node,
bounded by `node.stock`; the 1:1 recipe and all `produced`/`consumed_as_input` accounting are unchanged)
— a hard per-tick `conserves()` guard confirms it on every swept cell.

## 7. Acceptance criteria (independent verification)

- **MANDATORY non-vacuity test (Codex P1 — the milestone's premise):** under matched scenario
  conditions, a high-skill cultivator (skill driven to `SKILL_CAP`) must harvest **strictly more grain**
  AND produce **strictly more bread per cultivating opportunity** than a skill-0 cultivator. If it does
  not, the headline verdict is **LEVER INERT** (the lever is the wrong knob — pivot to labor-cost in a
  redux), NOT "no stickiness." This test gates the economic interpretation of the whole milestone.
- New suite `sim/tests/occupational_stickiness.rs`: classifies the treatment via the §2 ordered
  classifier across `SEEDS`, diagnostics printed, verdict not asserted SUCCESS.
- **Pre-stated thresholds (a priori, pinned in §8):** `CHURN_DROP = 0.5` (treatment churn ≤ 0.5 × the
  **matched-seed** skill-off S22a baseline ⇒ "churn fell materially"); `PERSIST_FRACTION = 0.5` (an agent
  id is a persistent cultivator if it cultivates ≥ 50% of the final window); `PERSIST_COHORT = 4`
  distinct such ids, ≥2 non-lineage; `MONO_SHARE = 0.75`; plus the reused `COMMUNE_SHARE`, `CHURN_LIMIT`,
  `MATERIAL_BOUGHT_FLOOR`. All frozen before running.
- The skill-off control reproduces the S22a baseline; the cap-zero control reproduces S22a's
  **behavior/metrics** (NOT byte-identical — the ON gate still digests config/state; only flag-off is
  byte-identical) as the no-op proof the lever is the cause; the other controls behave as classified
  (reported, not forced).
- Guards on every run + every swept cell: conservation, `bread_minted_max == 0`, provenance
  clean-or-disqualified, `!extinct`.
- Robustness mini-sweep over `SKILL_GAIN`/`SKILL_DECAY`/`SKILL_CAP`/harvest-ceiling + grain flow,
  classified, no tuning to pass; regime map reported.
- Workspace: all tests pass; **all existing goldens byte-identical**; `cargo fmt --check` and
  `cargo clippy --workspace --all-targets -- -D warnings` clean.

## 8. Resolved decisions (Codex spec-review)

1. **Lever → grain-haul-capacity efficiency** (defer labor-cost reduction). Skill scales the cultivating
   agent's effective grain-haul capacity (`carry_cap`/`carry_room` for the grain `GoHarvest`), NOT just
   the want (Codex round 2: want-scaling alone is mechanically inert because `carry_room` is the binding
   cap — `min(want, stock, carry_room)`). Conservation-safe (the node is still debited; harvest stays
   bounded by `node.stock`). **It must be proven to BITE** — the mandatory non-vacuity test (§7) and the
   LEVER-INERT outcome (§2): if a high-skill cultivator does not move more grain AND produce more bread
   than skill-0, the verdict is LEVER INERT (pivot to labor-cost reduction in a redux).
2. **Stickiness thresholds:** `CHURN_DROP = 0.5` of the **matched-seed** S22a baseline (each treatment
   seed compared to its own skill-off run, not a hard-coded ~23); `PERSIST_FRACTION = 0.5` of the final
   window; `PERSIST_COHORT = 4` (≥4 agents whose **ids** cultivate ≥50% of the final window), of which
   **≥2 are non-lineage**, with a material non-cultivating buyer cohort still present.
3. **Skill heredity → born at 0** (defer heredity — this tests *earned* advantage, not inherited craft).
4. **Skill magnitudes (pinned):** `SKILL_CAP = 1000`, `SKILL_GAIN = 50`, `SKILL_DECAY = 5`, max raw
   harvest want `≤ 2 × carry_cap`. Effective bread gain is capped by
   `min(harvest, node_stock, carry_room, labor_budget / CULTIVATE_LABOR)` and may be **less** than 2×.
5. **Monopolization → final-window top-skill-cohort grain share + DAMAGE.** MONOPOLIZATION CULL requires
   BOTH the top cohort taking **≥75%** of harvested grain AND living non-lineage / material buyers
   falling below the survival/buyer floor (dominance alone is not a cull if buyers still survive + buy).

## 9. Scope boundary (deferred to S22c+)

Profit-driven Consumer/Gatherer drift (opportunity-cost role choice); Miller/Baker specialized-producer
entry (money-gate / capital path); a global role chooser; heritable skill; endogenizing the clearing
institution. S22b adds **accumulated advantage to cultivation participation**, testing whether it
produces occupational persistence — not the full vocation topology.

## 10. Risks (and how the spec pre-empts them)

- **Conservation break (the worst engine risk).** Mitigated by construction: skill scales only the
  cultivating agent's grain-haul capacity (a bigger debited draw on the conserved grain node, bounded by
  `node.stock`), never the recipe ratio or a `produced` term; a hard per-tick `conserves()` guard on
  every cell.
- **Tuning trap.** Adjusting skill magnitudes until "just enough" stickiness appears makes the result
  worthless. Mitigation: predeclared modest constants, the cap-zero no-op control, the sweep, and a
  verdict test that never asserts SUCCESS.
- **Overclaim trap.** This is "occupational persistence from accumulated advantage in cultivation," not
  "endogenous division of labor" in full. The spec/commit/report say so; the deferred list (§9) stays
  visible. If churn does not fall, the honest verdict is NO STICKINESS DESPITE SKILL — a finding that
  names the next boundary, not a failure to tune away.

## 11. Pipeline

Codex spec-review (settle §8) → SPEC-READY → rb-lite `claude,codex` → independent verification
(workspace + all goldens byte-identical + the new suite + the verdict run) → Codex review-of-results →
merge + report-update + memory + pin.
