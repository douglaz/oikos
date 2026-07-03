# impl-57 — C10: Conflict and the Political Means (is predation individually tempting yet negative-sum for the whole?)

Status (spec): **DRAFT — pending Codex spec-review.** Layer C10 of the civilization-core roadmap
(`docs/spec-civ-core-roadmap.md`). Composes on **C5** (the state funds defense/raids) + **C8** (multiple
settlements) + the exogenous-shock substrate. Flag `conflict_political_means`, digest **tag 30**, ON-only.

Falsifiable bar (headline): is raiding ever **individually "profitable"** for the raider, and does it
**lower total real output** for the region (capital destroyed + labor diverted to raiding/defense) vs. a
peaceful control — i.e. does the model reproduce that the **political means is negative-sum for the whole
even when positive for the raider**?

## 0. Dependency & premise (read first)

C10 models **violence as an alternative to exchange**: raiding, conquest, tribute, and
defense-as-public-good — the "political means" beside the "economic means" (Oppenheimer/Rothbard). It is
**provisional on C5** (defense is a state-funded public good; tribute is a coerced tax by an outside
state) and **C8** (raiding is inter-settlement). It deliberately does **not** build a tactical combat
system (game-spec non-goal); outcomes are deterministic resource/probability functions of committed force.

## 1. Praxeology — wealth by seizure, not production

Wealth is acquired either by production-and-exchange (the **economic means**) or by coercive seizure (the
**political means**). Conflict is **capital destruction + coerced transfer**, never creation: a raider's
"gain" is exactly the victim's loss **minus** the deadweight destroyed in the taking and the labor both
sides divert from production. A conqueror who taxes a productive population is a stationary bandit (the
state, C5, at the limit). Defense is a **public good** funded by taxation (C5). **Anti-smuggling:** war
must produce **no** wealth — every seized unit is a **conserved transfer** (victim→raider), every destroyed
unit a **named sink** (like spoilage), and money is transferred but never minted; the region-wide total
strictly falls by the destruction + the forgone production.

## 2. What already exists

- **The conserved-sink precedent:** `run_spoilage` (settlement.rs:14179–14260) is the canonical **named
  sink** — decayed units debited from stock (`debit_stock`) and credited to `report.spoiled[good]`, kept
  in the conservation identity, with provenance/acquisition-ledger sinks. C10's **destruction** copies this
  exactly.
- **The conservation identity + report:** `EconTickReport` (settlement.rs:5437–5500) with
  `regen/endowment/transferred/consumed/consumed_as_input/promoted/spoiled` and the per-tick assertion
  (:9693). C10 adds a `destroyed` sink to both.
- **Conserved transfer primitives:** `move_money_conserved` (settlement.rs:14269), `debit_stock`/
  `credit_stock` (society.rs:4429) — the seizure moves.
- **Inter-settlement topology (C8/G2c):** `Region { settlements, route, caravan }` (region.rs:442) with a
  region-wide conservation roll-up — where a raid_phase between settlements lives.
- **Defense as a project (C5):** `project.rs` public-works lifecycle; a defense work reduces raid success.
- **Exogenous-shock substrate:** raids exist in v1 as exogenous capital-destruction shocks (game-spec §6);
  C10 promotes this to an *inter-settlement, choice-driven* interaction — no new RNG (deterministic per
  seed).

## 3. Mechanism

### 3.1 The raid (inter-settlement destructive transfer)
A gated `conflict_political_means` raid phase in `Region`; the outcome is a **deterministic per-seed
function** of `(raider_force, victim_defense, distance/transit_cost)` — no live RNG. On success:
- **Force is a real, pre-committed opportunity cost (spec-review P1).** `Region::econ_tick` advances each
  settlement (production) *before* the caravan/road phases (region.rs:612), so a force debit booked *after*
  production would be a mere audit label, not a real cost. The raider's diverted labor/capital must be
  **committed before its own production phase** (so it genuinely does not produce this tick) or **debited
  from next-tick capacity**, and that commitment is **digested** — the diversion is a real reduction in the
  raider's output, not a bookkeeping tag.
- **Seizure — a Region-level paired debit/credit (spec-review P1).** `move_money_conserved` is a *private
  `Settlement`* method and `Society::transfer_gold` requires both agents in the **same** society
  (society.rs:4524) — raids are **cross-settlement**, so C10 must use a **Region-level paired
  debit/credit** in the caravan's pattern (`debit_stock`/`debit_gold` on the victim → region escrow →
  `credit_stock`/`credit_gold` on the raider, region.rs:684), **not** `move_money_conserved`. A fraction of
  the victim's *free* (unreserved) goods + gold moves victim→raider, booked in the **regional**
  `transferred`.
- **Destruction:** a fraction of the seized/contested goods is **destroyed in the taking** — debited and
  credited to a **new `report.destroyed[good]` sink** (the spoilage pattern), a deadweight loss. Money is
  transferred, never destroyed.

### 3.2 Tribute (coerced periodic transfer)
A conqueror can impose **tribute** — a coerced periodic transfer from the victim, modeled as a **tax
levied by an outside state** (reuse C5's tax-as-conserved-transfer, with the tribute recipient the
conquering settlement's treasury). No new coercion primitive.

### 3.3 Defense (public good, C5-funded)
A settlement funds **defense** as a C5 public work (treasury-commissioned `Project`); on completion it
**raises `victim_defense`**, lowering raid success — the raider must weigh the (now higher) cost against
the loot. Defense is non-excludable within the settlement.

### 3.4 The negative-sum result (the finding)
Because seizure is conserved and destruction is a named sink, and both sides divert labor from production,
**regional real output strictly falls** with conflict vs. a peaceful control — even if the *raider's* own
balance rises. That asymmetry (privately tempting, socially destructive) is exactly the political-means
result C10 tests.

## 4. Praxeology / anti-smuggling guards

1. **No wealth from violence.** Seizure conserved (victim loss = raider gain), destruction a named sink,
   money never minted; a regional conservation test asserts (full identity, including `produced`)
   `after = before + regen + endowment + produced − consumed − consumed_as_input − promoted − spoiled − destroyed`.
2. **Negative-sum for the whole.** A `peace` control must show higher regional real output than the raid
   scenario (destruction + diverted labor); if raiding *raised* total output, that would be a bug/smuggle.
3. **Individually tempting is allowed.** The raider's private balance may rise — the point is the split
   between private gain and social loss.
4. **Defense is a funded public good.** Reuse C5's treasury→project; a raider facing funded defense may
   find raiding unprofitable (`DefenseDeters`).
5. **No tactical combat / no RNG.** Outcome is a deterministic per-seed function of committed force; no
   live randomness (determinism preserved).
6. **Coercion is named.** Seizure and tribute are modeled *as* coercion (transfer/tax), not exchange.

## 5. Conservation & determinism

- **The `destroyed` sink.** Add `pub destroyed: BTreeMap<GoodId,u64>` to `EconTickReport` (parallel to
  `spoiled`, settlement.rs:5472) **and to `RegionTickReport`** (spec-review P1: raids run in the `Region`,
  so the sink must exist at the region level too, not only per-settlement), wired into the conservation
  identity + assertion (settlement.rs:9693; the **full** identity includes `produced`, settlement.rs:5591)
  and the regional roll-up (region.rs:364). **Prerequisite (shared with C8, P1):** the regional roll-up must
  first be brought to mirror the *full* settlement ledger (it currently omits `promoted`/`spoiled`,
  region.rs:355) before the `destroyed` sink is added. Seizure is a **Region-level paired debit/credit**
  (§3.1), booked in the regional `transferred`; money is transferred (conserved), never destroyed.
- **Digest (tag 30, ON-only).** `if self.conflict_political_means_active() { out.push(30); ... raid
  commitments, defense-project state, tribute obligations }`. The `destroyed` sink and raid state **steer**
  behavior → digested. Off-path (`conflict_political_means` false): `destroyed` stays empty across all ticks
  → **byte-identical**.
- **Determinism.** Raid outcomes are deterministic per-seed functions (no live RNG); Integer-only.

## 6. Slices

- **Slice A — the destruction sink + seizure (single→inter-settlement).** `report.destroyed` in the
  conservation identity; a conserved seizure (transfer) + destruction (sink) between two settlements.
  *DoD: a raid moves wealth conserved and destroys a booked deadweight fraction; regional conservation
  holds; off-path byte-identical (`destroyed` empty).*
- **Slice B — force/defense outcome + tribute.** Deterministic outcome from committed force vs. C5-funded
  defense; tribute as a coerced C5-tax transfer. *DoD: defense lowers raid success; tribute flows
  conserved.*
- **Slice C — the negative-sum measurement + tag 30.** Regional real-output comparison raid-vs-peace;
  tag-30 ON-only digest. *DoD: conflict lowers regional real output vs the `peace` control; goldens
  byte-identical off.*
- **Slice D — acceptance suite + controls** (§7).

## 7. Acceptance suite (`sim/tests/conflict_political_means.rs`)

`SEEDS=[3,7,11,19,23]`, multi-settlement region.

- **Predeclared thresholds (swept):** raider private-gain, regional real-output drop vs peace, defense
  deterrence threshold, tribute flow.
- **Ordered verdict enum:** base-precondition (regional conservation incl. `destroyed` / C5+C8 base) →
  outcome: `PredationPaysButShrinks` (raiding is individually profitable for the raider **and** lowers
  regional real output vs the `peace` control — privately tempting, socially negative-sum) / `DefenseDeters`
  (C5-funded defense makes raiding unprofitable, so it doesn't occur) / `PredationUnprofitable` (raiding
  never pays even undefended — a scoped finding about this base's spoils vs. costs).
- **Mandatory non-vacuity:** a raid actually seizes + destroys (conserved); a real counterfactual — a
  region whose real output is lower under raiding than the matched `peace` run.
- **Controls:** `peace` (no raids — the negative-sum baseline); `funded_defense` (C5 defense on → deters);
  `zero_destruction` (destruction fraction 0 → pure transfer, isolates the deadweight component);
  `conflict_political_means_off` matched base.
- **`goldens_unchanged()`:** with `conflict_political_means` off, byte-identical (the `destroyed` sink stays
  empty); the spoilage/conservation regressions still hold.

Build/verify: `cargo test -p sim --test conflict_political_means -- --nocapture`, `cargo test --lib`, fmt,
clippy `-D warnings`, workspace green.

## 8. Risks & open questions

1. **Upstream dependency (top).** Needs C5 (defense/tribute) + C8 (settlements); without them C10 reduces
   to an exogenous shock, not choice-driven conflict — disclosed, and the headline requires both.
2. **Force commitment as opportunity cost.** The raider must genuinely divert labor/capital from production
   (else "raiding" is free and the negative-sum test is trivial); the diversion must be real and conserved.
3. **Determinism of outcome.** No live RNG — model the success as a deterministic per-seed function; do not
   introduce a draw.
4. **Endogenous vs. scripted raids.** v1 may start with a *scripted* raid trigger (who raids whom) while
   keeping the *outcome/accounting* faithful; fully endogenous raid *decisions* (a settlement chooses to
   raid because it expects private gain) are the harder step — disclose which is built.
5. **Scope.** No tactical combat, no unit micro; conquest/occupation beyond tribute is future work.

## 9. Falsifiable-bar summary

Modeling a raid as a **conserved seizure** (victim→raider) plus a **named destruction sink** (deadweight),
with force as a real diverted-labor opportunity cost and defense as a C5-funded public good, should
reproduce the political-means result: raiding can be **individually tempting** (the raider's balance rises)
yet is **negative-sum for the region** (real output falls vs. a `peace` control by the destruction + forgone
production) — `PredationPaysButShrinks` — while funded defense can make it unprofitable (`DefenseDeters`).
The honest alternative is `PredationUnprofitable` (spoils never beat costs on this base) — each a
first-class finding that violence produces no wealth, only transfers and destroys it.
