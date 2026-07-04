# Spec: Integrating the Civilization Core into a Playable Game

*How to wrap the OIKOS engine in a praxeology-faithful, single-settlement-first civilization game.*

> Status: design spec (integration tier), revision 1 (2026-07-03). **Successor to `game-spec.md`**
> (rev 2, pre-G0, 2026-06-12), which it does not replace but *updates*: `game-spec.md` was written
> before the fork was actually built and before the S22–S24 and S23c–e findings existed. This spec
> reconciles that vision with (a) the engine as it now is (G0–G8, S5–S24 built and verified) and
> (b) what the research learned, and it re-centers v1 on **one deep settlement** per the project
> decision. Companion to `spec-civ-core-roadmap.md` (the core it wraps). Where this spec and
> `game-spec.md` agree, `game-spec.md` remains the detailed reference (architecture §4, escrow §4.3,
> determinism §4.4, legibility §8, testing §10); this document supersedes its *roadmap* (§11) and
> *core-model* assumptions in light of the findings.

---

## 1. The design thesis, now empirically earned

`game-spec.md`'s first pillar — **"colonists act; the player governs; no colonist is ever ordered to
trade, work, or value anything"** — was a purist aspiration in rev 2. The S24 institution-selection
arc has since turned it into an **empirical result**:

> Local welfare-imitation *cannot* select a division-of-labor institution, because its value is
> **non-local** — realized through exchange from producers to buyers across the market — so no local
> welfare observable ever makes the productive role look best. The institutional layer can be
> **supplied** and made to spread by ratchet, but under non-circular local welfare-imitation it does
> **not** self-select. (`report-emergence-and-its-limits.md` §19, S24a–c triad.)

This is the game's foundation, not a footnote — **scoped precisely.** What S24 established is narrow
but load-bearing: in *every institution-selection mechanism the project has tested* — sticky (S24a),
abandonable (S24b), and group-payoff (S24c) **local welfare-imitation** — the sim did **not**
endogenously select the division-of-labor institution, because its value is realized non-locally
through exchange. It is **not** proven that no mechanism could: the report itself names
market-mediated / global-payoff signals, contribution accounting, and group-level
reproduction/selection as open future work. So the honest claim is: *under the selection mechanisms
tested so far, the institution had to be **supplied**.* That is exactly enough to justify the game's
design — **for v1 the player is the institution-selection layer** (property regime, wage law, tax,
money, charters), and whether that layer can eventually be *endogenized* is a live research question,
not a settled impossibility. The player is therefore not a convenience bolted onto an autonomous
economy; the player fills the selection role the sim has not (yet) been shown to fill itself. That is
both the game's core fantasy (you supply and select the institutions — and watch whether the emergent
economy they enable thrives or collapses by praxeological necessity) and its praxeological
justification (the player supplies rules; agents' own ordinal action supplies everything else).

**The keystone dependency.** There is no economy to govern until the **circular flow of income**
exists. S23c→d→e proved that without a buyer-income loop the settlement is subsidy-bound (everyone
self-provisions; no one buys; there is no market). So the game becomes *playable as an economy* only
once core layers **C1–C4** land (`spec-civ-core-roadmap.md`). Until then there is a survival colony
but not a catallaxy. This spec's roadmap (§7) makes that dependency the first gate.

### 1.1 Design pillars (carried from `game-spec.md` §1, reaffirmed)

1. **Colonists act; the player governs.** The anti-command discipline (§4.3) — no `SetPrice`,
   `SetWage`, `AssignJob`. Enforced by the type system.
2. **Eras are earned, not timed.** Institutional eras are *detected* (`era.rs`); tech tiers unlock
   what *can* be made (C6).
3. **Every number is auditable.** The conservation ledger and canonical tapes become the explanation
   UI (§6).
4. **Deterministic to the byte.** One seed + one command log = one identical run — the *target*
   property, achieved once Gb folds the command log into the canonical/digest surface (§5.2); the
   engine's determinism is inherited, but the command-log surface is new work.

---

## 2. What is already built vs. what the game still needs

`game-spec.md`'s roadmap (G0–G9) was written as if from a standing start. Most of it has since been
built as the S/G milestone arc. This reconciliation is the honest starting point:

| `game-spec.md` roadmap item | Status in the engine today |
|---|---|
| G0 fork & engine (`econ`/`world`/`life`/`sim`) | **Built.** Workspace `econ/ world/ life/ sim/` + `viewer/`. |
| G1 needs → wants (`regenerate_scale`) | **Built.** `life/src/scale.rs`, `NeedState` (hunger/warmth/rest). |
| G2 space + two-rate loop | **Built** (space + loop). `world/` grid/nodes/movement; `econ_tick` (settlement.rs:9273) over a 24-fast-tick loop. **The wage-delivery escrow bucket (`game-spec.md` §4.3) is NOT built** — `LaborBook`/`apply_labor_trade` (factor.rs) pays wages and advances projects *synchronously*; the two-rate escrow is C1/Ga work. |
| G3 production & construction (emergence) | **Built.** grain→flour→bread (content.rs), producible capital (S7), endogenous specialization (S5). |
| G4 demography | **Built.** households/lineage/aging/estate (G4b, demography.rs); Malthusian band (S14/S17). |
| G5 money at scale | **Built (single settlement).** Mengerian strong-bar emergence (S8/S9). Robustness-across-generated-worlds is **not** done — still the top risk. |
| G6 tech & eras | **Substrate built.** G6b `KNOWLEDGE`/research/tier-gating; `era.rs` detector. Full ladder = core C6. |
| G7 trade (multi-settlement) | **Substrate built.** G2c `region.rs` caravans. Trader AI/migration = core C8. |
| G8 institutions & finance | **Substrate built.** Banks, fractional reserve, tender/fiat (G8a/b/c), tax receivability (M20/M21). V2→M3 runtime bridge + treasury-spend loop = core C5/C7. |
| G9 game shell (UI, commands, save/replay UX) | **Not built.** This is the genuinely game-specific work. |

**Two things `game-spec.md` did not know it needed, now known:**

1. **The buyer-income loop (core C1–C4).** `game-spec.md` assumed specialization + capital + a labor
   market would compose into a living economy. The research showed the demand side has no recurring
   income, so the economy is subsidy-bound without an explicit wage/rent/payment loop. **This is the
   precondition for a playable economic game and did not exist in rev 2's plan.**
2. **The player-as-institution-selector justification (S24).** rev 2 argued the anti-command
   discipline from *purism*; the game can now argue it from a *result*.

So the game's real remaining work is: **(a)** the core buyer-income loop (delegated to
`spec-civ-core-roadmap.md` C1–C4, built headless and research-verified first), and **(b)** the game
shell — command protocol, save/replay UX, legibility inspectors, policy levers, and the
robustness-across-worlds gate.

---

## 3. Single-settlement v1 scope (the deep colony)

Per the project decision, v1 **centers on one settlement** (a Rimworld/Banished-shaped living
colony), with the empire/4X layer as the stated end-goal, staged after one settlement is deep and
playable. The player founds and governs a band → village → town.

**The core game loop:**

```
OBSERVE (legible tapes: prices, money good, era, hunger band, wage flows, firm sheets, classes)
   →  SET institutions / infrastructure / policy (the player's only levers)
   →  WATCH the emergent consequence propagate through ticks and space
   →  ADAPT
```

The player never touches an agent's choice. The player shapes the **environment** (zones, public
projects, exploration) and the **institutions** (property regime, wage-labor legality, tenure type,
tax surfaces and rates, money/tender policy, bank charters, research direction). Everything economic
— prices, wages, the money good, who works, who owns, who trades, which classes form — **emerges**.

### 3.1 Player levers, mapped to the core layers

Extending `game-spec.md` §7's `Command` protocol to the new core layers (`spec-civ-core-roadmap.md`).
Every command is validated at a tick boundary and returns `Applied | Rejected(reason)` (never a
silent no-op):

| Command | Maps to core | What it sets (exogenous) | What stays emergent |
|---|---|---|---|
| `DesignateZone{kind, area}` | world | where gathering/farming/building is *allowed* | who works it, and how hard |
| `CommissionProject{def, site, payer}` | C2/C5 | a public work exists to be funded | whether labor accepts the wage to build it |
| `SetPropertyRegime{tenure}` | C3/C4 | commons vs. private tenure vs. tenancy rules | whether owners form, lease, thrash (S23a/b live here) |
| `SetPolicy(WageLabor{legal})` | C1 | whether wage employment is permitted | whether firms hire and buyers earn |
| `LevyTax{surface, rate, receivable}` | C5 | tax rate on a surface (M21) | labor-supply and Cantillon responses |
| `SetTreasurySpend{budget, targets}` | C5 | a treasury *budget* + which works/roles to fund | the **wage still clears in the labor market** — the state is a funded *bidder*, never a wage-setter |
| `SetMoneyPolicy(tender)` | C7 | which media settle which surfaces (G8c) | whether money circulates or is inert |
| `CharterBank{reserve_ratio}` | C7 | a bank and its reserve rule exist | deposits, lending, runs, the cycle |
| `Research{direction}` | C6 | which knowledge the scholars pursue | whether surplus funds it (time preference) |
| `Expedition{target}` *(empire, deferred)* | C8 | explore/found a second settlement | trade, migration, convergence |
| **never** `SetPrice`/`SetWage`/`AssignJob`/`SetInterest` | — | — | these have **no command variant** (type-enforced) |

The anti-command list is pillar 1 made mechanical: the player can make wage labor *legal*, can tax a
wage, can charter a bank — but can never *set* a wage, a price, or an interest rate, nor order a
colonist into a job. This is exactly praxeologically faithful: the player wields the **political
means** (rules, coercion, public goods) and the market wields the **economic means** (production,
exchange, calculation).

**The Command surface is a narrow, curated enum — the game never exposes the engine's raw
`EventKind`.** The lab's `EventKind` (settlement.rs:23273; `econ` `apply_command`, society.rs:1290)
contains primitives that would shatter faithfulness if a player could fire them directly —
`SeedStock`, `FiatPrint`, `SetDebtDueTick`, `SeedCommodityDebt`, tender setters, reserve setters. The
game therefore maps each curated `Command` onto a *vetted subset* of engine effects and exposes
nothing else; the anti-command guarantee is only as strong as this narrowing, so it is a hard
architectural rule, not a naming convention. In particular `SetTreasurySpend{budget, targets}` sets a
**spending budget and offer policy** — state-funded work still hires through the C1 labor market where
the wage clears, so the state is a well-funded *bidder*, never a `SetWage` in disguise.

---

## 4. Praxeology-consistency mapping for game mechanics

This is the heart of the request — *"all rules compatible and consistent with praxeology."* Below,
each familiar civilization-game mechanic is mapped to its faithful implementation, and the
inconsistent shortcut it must **reject** is named. This table is the acceptance gate for every
game-facing feature; it operationalizes the §4 contract of `spec-civ-core-roadmap.md`.

| Civ-game trope | Faithful implementation | **Rejected** shortcut (and why) |
|---|---|---|
| **Tech tree** | Knowledge produced by scholars from real inputs (C6); unlocks *possibility*; capital + time preference decide adoption. | Free timed unlocks that *raise output directly*. Tech without capital is inert (Mises); productivity is never a gift. |
| **Build queue / production orders** | Player *commissions* a project or *zones* land; colonists choose whether to build/work it for a wage. | Directly ordering agents to produce (central planning). The player cannot `AssignJob`. |
| **Tax slider** | Tax surfaces (M21), a rate on wages/spot/debt; conserved coercive transfer to a treasury. | A slider that just adds gold to a treasury. Tax is a *transfer from someone*, conserved, with labor-supply/Cantillon consequences. |
| **National treasury** | An in-ledger agent funded **only** by taxes/tribute/debasement; spends conserved money (C5). | A treasury that generates money per turn. Money from nowhere is a conservation violation. |
| **Happiness / approval meter** | Revealed preference: migration, reproduction, participation, and physiological **bands** (hunger/warmth). | A cardinal "happiness" number that agents maximize. There is **no** cardinal utility or interpersonal welfare (praxeology's hard line). |
| **Gold reward / instant income** | Income = proceeds of selling a factor service or entrepreneurial profit (C1/C4). | "Complete quest → +100 gold." Every coin has a conserved ledger path. |
| **Global instant policy effects** | Effects propagate through the tick loop and space (Cantillon: new money reaches near receivers first). | A policy that changes all prices at once. Adjustment is a *process* in time, not an equilibrium jump. |
| **Interest-rate dial** | The loan rate emerges from the credit market vs. the natural rate (S10/C7); the player can only charter banks and set reserve/tender rules. | Setting "the interest rate." Interest is imputed from ordinal time preference, never decreed. |
| **Prices set by supply/demand curves** | Prices discovered by actual bilateral trades in the order book (`market.rs`); no curve is stored. | Computing a clearing price from cardinal demand functions. Value is subjective and ordinal. |
| **Population growth rate** | Births when household need-security clears a threshold; deaths by age/starvation (G4b, S14/S17). | A growth-rate parameter. Demography is an outcome of the food/shelter margin. |
| **Victory points / score** | Depth and breadth of the division of labor: population × specialization × real output per capita, and how long the colony sustains it against scarcity (§8). | An arbitrary score. The challenge is *sustaining and deepening cooperation under scarcity*, which is the real thing civilization is. |
| **War for resources** | Conflict is capital destruction + coerced transfer (C10); negative-sum for the whole even if positive for the raider. | War that *creates* wealth. The political means produces nothing; it seizes and destroys. |
| **Difficulty setting** | Scenario parameters: initial endowment, resource scarcity/regen, map, shock frequency — all *config*, byte-identical machinery. | Difficulty that bends the *rules* (e.g., cheaper AI production). Difficulty changes the world, never the economics. |
| **Random events** | Exogenous shocks already modeled (S11 price shocks, raid = capital-destruction shock) parameterized as scenario content. | Scripted event cards that inject/remove money or force choices. Shocks perturb; they do not conjure. |

**The single rule behind the table:** the player and the scenario set **initial conditions, rules,
rates, and coercive transfers**; agents' **ordinal action under scarcity** produces every economic
outcome; and **conservation** holds throughout. Any mechanic that cannot be expressed that way does
not ship in *faithful mode* (§5.2).

---

## 5. Integration architecture: wrapping the deterministic core

### 5.1 The core stays headless; the game is a client

Exactly as `game-spec.md` §4.2: `sim` exposes two surfaces —

```rust
pub fn advance(&mut self, commands: &[Command]) -> TickReport;  // one econ tick
pub fn view(&self) -> WorldView<'_>;                            // read-only snapshot
```

The game/UI renders from `WorldView` and queues `Command`s; it can never mutate state. This is the
research engine's read-only-metrics boundary (`econ` purism) promoted to the process architecture,
and it is why the game is testable in CI without a GPU. The `viewer/` crate is already a read-only
inspector and is the seed of this client.

### 5.2 Player decisions become gated config mutations — the target property (design, not inherited)

This is the crucial integration insight — stated honestly as a **design target**, because the
mechanism it relies on **does not exist in the engine today** and is Gb work (§5.4). Today
`canonical_bytes()` (settlement.rs:20107) serializes settlement state, config, scenario events
(settlement.rs:23147), finance-policy timelines, and ON-only milestone tags — but there is **no game
`Command` log or command-schema surface**. So the property below is what Gb must *build*, not a fact
the engine already gives for free:

**Design intent — a player command is a config/policy mutation applied at a tick boundary,
structurally analogous to toggling a milestone flag.** Setting a property regime, a tax rate, a tender
policy, or a bank charter is the same *kind* of gated, digest-tagged, conservation-safe state change
the milestone system already handles — so if Gb encodes commands into the canonical/digest surface,
the game inherits the research discipline. **What Gb must specify and build for this to hold:**

- A **canonical command-log encoding** folded into `canonical_bytes()`/the digest, with
  **rejected-command side-effect rules** (a rejected command must leave *no* trace in the digest) and
  **schema-version bumps** on any command-set change.
- **Replay-digest golden tests**: a scenario + a scripted command log replays byte-identical in CI;
  and — the tripwire — *with no commands and flags off, a game run reproduces the research goldens
  exactly.* Once built, **the game cannot silently diverge from the faithful engine — the digest
  catches it.**
- **Save = seed + content hash + command log + engine schema version** (`game-spec.md` §4.4): the
  economy replayable to the byte, desyncs impossible, a corrupted/"cheating" save failing the
  replay-digest check.

Once that surface exists, **faithful mode == the research engine driven by player input** instead of a
fixed scenario config, and the whole game is regression-testable like the lab (balance changes bump
the schema version and break replay deliberately; never re-run an old log on a new engine).
**Difficulty and events stay config, not rule-bending** (§4) — same machinery, different parameters,
so a "hard" world is still a *faithful* world.

A **game-only presentation/scenario layer** (UI, art, tutorial, difficulty presets, scripted
scenarios) sits strictly *outside* the faithful core and never bends its rules; it only chooses
initial config and reads `WorldView`. This is the boundary that keeps the research goldens intact
while the game grows.

### 5.3 Time control, escrow, and the two-rate loop

Game speed maps to the two-rate loop (`game-spec.md` §4.3): the **fast tick** (24/econ-tick,
`FAST_TICKS_PER_ECON_TICK`, settlement.rs) runs movement/needs/hauling/labor-delivery; the **econ
tick** runs scale regeneration, market/labor/loan clearing, project advancement, era detection, and
the conservation assertion. The player pauses at econ-tick boundaries to set policy; money mutates
only inside econ ticks. The **delivery-escrow contract** (game-spec §4.3) is exactly what C1's wage
payment needs — wages escrow at clearing and release pro-rata on labor actually delivered in the fast
loop (death/path-block are short deliveries, not special cases). **This escrow bucket is not yet
built:** today `LaborBook` settles wages and advances projects *synchronously* in `apply_labor_trade`
(factor.rs), so the two-rate escrow is C1/Ga implementation work, not existing machinery.

### 5.4 What the game must add to the engine (the honest new work)

- **The command protocol with result/error semantics** (game-spec §7): the engine's `EventKind`
  apply-at-tick-boundary pattern exists (`scenario.rs`), but game commands need
  `Applied | Rejected(reason)` validation (era gate, target existence, treasury balance, zone
  legality), not the lab's silent no-ops — **and** the game must expose only a *narrow curated
  `Command` enum*, never the raw `EventKind` (§3.1).
- **The canonical command-log surface + replay-digest tests** (§5.2) — the property that makes the
  game faithful is *built here*, not inherited.
- **The two-rate wage-delivery escrow bucket** (§5.3, C1/Ga) — `LaborBook` is synchronous today.
- **Save/replay UX + schema versioning** (game-spec §4.4).
- **The legibility inspectors** (§6).
- **The V2→M3 runtime bridge and treasury-spend loop** — shared with core C5/C7 (`game-spec.md` §13),
  needed before the credit and state ages are playable.
- **Robustness across generated worlds** (§8) — the top risk, shared with core.

---

## 6. Legibility: the tapes are the game (carried from `game-spec.md` §8, extended)

An emergent economy is only *fun* if it is *legible*. The conservation ledger and canonical tapes
become the explanation engine. `game-spec.md` §8 specifies the core inspectors; the new core layers
add their own views:

- **Price inspector** — click a price → the trades behind it, the bid/ask ladder, the marginal pair.
- **Coin provenance** — click money → its ledger path (where minted/issued, Cantillon receipt tags,
  which surface it last settled on).
- **Colonist "why"** — click a colonist → current ordinal value scale, top unsatisfied wants, last
  rank-walk outcome ("refused the job: leisure outranked the wage"). *This is where the ordinal model
  is shown honestly — revealed preference, never a utility number.*
- **Wage & income view** *(new, C1)* — for a household: income by source (wages / profit / rent /
  interest), and whether it is a solvent buyer. This visualizes the keystone loop directly.
- **Firm balance sheet** *(new, C2)* — a firm's reserve, wage bill, input costs, forecast vs.
  realized revenue, running profit/loss — the entrepreneurial calculation made visible.
- **Class & mobility view** *(new, C9)* — the income-source composition of the population and the
  transition matrix (who moved from wage labor to ownership this generation).
- **The Court Economist / natural-rate advisor** *(C7)* — the credit-disabled shadow counterfactual
  (`shadow.rs`) as an on-demand, async advisor: "had there been no credit expansion, the natural rate
  would be ~X; your malinvestment gap is Y." The authoritative ABCT signal, computed as a
  command-log replay from genesis with credit origination disabled (game-spec §8) — exact because
  deterministic.

---

## 7. Roadmap: single-settlement first, empire deferred

Reframed from `game-spec.md` §11 to reflect what is *already built* (§2). Each stage ships headless
first (UI lags by design) and is gated on its DoD. The core-layer dependencies point at
`spec-civ-core-roadmap.md`.

- **Ga — The living economy (core C1–C4, headless, research-verified).** The buyer-income loop:
  wage labor (C1) on the scarce commons, unified reproducing owner-households (C3, landing S23c/S23d
  from branch onto master), rent/factor income (C4). *DoD: a sustained producer/buyer money market
  forms — `final_buyer_bought > 0`, money circulates, the deferred S23c generational-tenure study
  finally runs — verified across `{3,7,11,19,23}` with the classify-not-tune bar.* **This is the gate
  to a playable economic game.**
- **Gb — The command wrapper.** `Command` protocol with result/error semantics; save/replay + schema
  versioning; the deterministic player-driven run verified to reproduce research goldens with an
  empty command log. *DoD: a scripted command log replays byte-identical in CI; an illegal command is
  rejected with a reason, never a silent no-op.*
- **Gc — Legibility.** The §6 inspectors over live tapes: price→trades, colonist→scale/why, the new
  wage/income and firm-sheet views. *DoD: a player can answer "why did the colony do that?" for any
  price, wage, and coin from the UI.*
- **Gd — The state & policy layer (core C5).** Property-regime, wage-legality, tax, tender, and
  treasury-spend commands; the treasury-spend loop and V2→M3 bridge. *DoD: the M21 chartalist circuit
  is reproducible in-game by a player using only Commands, and a treasury-funded public work gets
  built; a bad policy visibly harms the economy by necessity, not script.*
- **Ge — Tech, eras, and the robustness gate (core C6 + G5 risk).** The knowledge ladder and era
  detection surfaced to the player; **the envelope-scoped robustness gate** (game-spec §10.3): inside
  a declared worldgen envelope, the living economy (Ga) forms in ≥80% of randomized worlds, with
  true-negative worlds (no tradeable surplus) correctly failing to form a market. *DoD: the go/no-go
  robustness checkpoint passes; era progression is measured, not timed.*
- **Gf — The game shell.** Full UI over the Gc viewer; scenario starts; the "money moment"
  set-piece; tutorial era; difficulty as scenario config. *DoD: a stranger reaches the money moment
  and runs a solvent wage economy unassisted.*
- **Gg+ — Empire (deferred: core C8, C10).** Multi-settlement space, trader AI, migration, roads and
  price convergence, and conflict as the political means. Only after one settlement is deep and
  playable. *DoD: a second settlement, caravan trade with measurable convergence, and raiding that is
  individually tempting but negative-sum for the region.*

**The publishable v1 is Ga–Gf: a single living settlement where the economy is *real* — money emerges
per map, buyers earn wages and spend them, firms hire and bear profit/loss, owner-households
reproduce and bequeath, the player governs by institution and infrastructure alone, and every number
is auditable.** That is a shippable artifact ("watch a praxeologically-real economy live and grow")
even if the empire arc takes longer.

---

## 8. Non-negotiables, risks, and open decisions

**Non-negotiables (the faithful-mode contract):**

- Faithful mode preserves the research goldens: no commands + flags off ⇒ byte-identical to the lab.
- Conservation asserted every econ tick, including the treasury, escrow, and any new money channel.
- The anti-command discipline is type-enforced (no `SetPrice`/`SetWage`/`AssignJob`/`SetInterest`).
- No cardinal utility, no interpersonal welfare, no aggregate in any decision path — the UI shows
  revealed preference and physiological bands, never a happiness number.
- Game-only content (scenarios, difficulty, art, tutorial) lives outside the faithful core and only
  chooses config + reads `WorldView`.

**Risks (from `game-spec.md` §12, re-weighted by the findings):**

1. **The living-economy gate itself (new top risk).** Everything downstream assumes Ga succeeds — but
   Ga *is* the open research question the buyer-income loop poses (`spec-civ-core-roadmap.md` C1). If
   wage labor does not produce a sustained market, the game's economic core is delayed until it does.
   Mitigated by building Ga headless and research-verified *before* any shell work.
2. **Emergence robustness across generated worlds (Ge).** The lab's proofs run on a curated cast; the
   game needs them to fire across randomized worlds inside a declared envelope. The single biggest
   de-risking task; the go/no-go gate.
3. **Fun / pacing.** Emergent economies are slow-burn; mitigated by legibility (§6), early survival
   pressure, and the era set-pieces — prototype fun-checks start at Gc, not Gf.
4. **Scope.** Multi-year. Sequenced so Ga alone (the living economy) is a result, and Ga–Gf is a
   shippable single-settlement game, before any empire work.

**Open decisions (none blocking before Gb; carried from `game-spec.md` §14):**

- Whether the game is a **separate workspace** that forks `econ` (game-spec §4.1) or a game crate
  **in this repo** alongside `sim`/`viewer`. Given the target property (§5.2) — once Gb makes player
  commands gated config folded into the digest, the faithful engine and the game share one core — an
  **in-repo game crate** that drives the *same* `sim` (rather than a fork) is now the stronger option
  than rev 2's fork, and would keep faithful mode and research mode literally the same engine.
  Recommend re-deciding this at Gb.
- UI toolkit (Bevy vs. macroquad/egui); art direction (top-down vs. isometric) — affects the shell
  only.
- Raids/external pressure in v1 (exogenous shock) vs. deferred to the empire layer with C10.
- Working title / whether `econ` is eventually published as a standalone emergent-economy crate.

---

## 9. Summary

The engine already is, in all but name, the hard 90% of a praxeology-faithful civilization simulator:
it grows money, capital, interest, entrepreneurship, a division of labor, property, demography, and
taxation from individual ordinal action, conserved and deterministic. Two things stand between it and
a *game*: the **buyer-income loop** that makes the economy live (core C1–C4, the S23c–e blocker), and
the **game shell** that lets a player govern it by institution and infrastructure alone. The S24
finding makes that player role necessary rather than decorative — under every selection mechanism
tested so far the sim did not select its own institutions, so for v1 the player does — and the
determinism discipline is what lets the game *become* as auditable as the lab: **once Gb folds a
player command into the canonical/digest surface as a gated config mutation, faithful mode is the
research engine with a human in the loop.** Build the living economy headless and verified first,
wrap it in the command protocol and the tape-driven legibility UI, stage the state and credit ages on
top, and defer the empire of many settlements until one settlement is deep — a colony builder where,
for the first time, the economy is real all the way down.
