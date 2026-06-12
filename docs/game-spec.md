# Spec: OIKOS — transforming praxsim into a colony/civilization builder

Working title `OIKOS` (placeholder; rename freely). A colony sim / civ builder
that runs from a stone-age founding band to an advanced financialized
civilization, built around praxsim's emergent-economy engine. This document
specifies what carries over, what must be built, the architecture, the
gameplay-to-mechanism mapping, and a phased roadmap with definitions of done.

Status: design spec, revision 2 (pre-G0). Companion to `plan.md` (the lab's
architecture) and `impl-01..19.md` (the lab's milestones). The praxsim repo
remains the **laboratory**; the game is a **separate workspace** that forks
the engine. Revision 2 (2026-06-12) incorporates an independent codex spec
review; disposition of its findings is logged in §15.

---

## 1. Vision and design pillars

The one-line pitch: **the first colony builder where the economy is real** —
prices discovered by actual trades between colonists, money that *emerges*
from barter (a different money good per map), capital structure that
lengthens because colonists actually saved, and late-game financial crises
that follow from the player's own monetary policy by causal necessity, not
scripted event cards.

Pillars, in priority order:

1. **Colonists act; the player governs.** No colonist is ever ordered to
   trade, work, or value anything. The player shapes the environment
   (zones, public projects, exploration) and the institutions (laws, money,
   taxes, charters). Everything economic emerges through markets. This is
   praxsim's purism turned into the game's identity.
2. **Eras are earned, not timed.** "Bronze age", "banking age" are *detected*
   from what the colony actually does (praxsim's "Phase is measured, never
   set" doctrine) — tech unlocks what CAN be made; institutional eras emerge
   from how the economy organizes.
3. **Every number is auditable.** The audit-tape discipline becomes the
   explanation UI: click any price and see the trades behind it; click any
   coin and see the ledger path that put it there. Legibility is what makes
   an emergent economy *fun* instead of opaque.
4. **Deterministic to the byte.** One seed + one command log = one identical
   run. Saves are cheap, replays are free, desyncs are impossible, and the
   game's economy can be regression-tested exactly like the lab.

Non-goals (v1): combat as a tactical system (raids exist only as exogenous
capital-destruction shocks), diplomacy/multiple civs, multiplayer (the
determinism design keeps the door open), 3D.

---

## 2. Asset inventory — what carries over from praxsim

The labels below grade the MECHANISM (the algorithm/data model) separately
from the INTEGRATION (how it is wired into the engine today). The honest
headline: most mechanisms carry over intact; most integrations do not,
because `Society` is a global monolith — one runner owning all books, tapes,
projects, ledgers, events, and scenario-kind switches. Extracting
settlement-scoped services from it is a named refactor (G2), not
configuration. Where revision 1 said "as-is" it meant the mechanism; this
table now prices both.

| praxsim module | game role | mechanism | integration cost |
|---|---|---|---|
| `agent.rs` ordinal `Want` scale + rank-walk reservation pricing | colonist decision core | as-is | scale *generation* is new (§5.2); `AgentId(u32)` → generational arena is a wide migration (G0b) — the id is embedded in ledgers, tapes, debts, banks, metrics, barter, factor, market records |
| `market.rs` CDA order book, price-then-seq, resting-limit settlement | marketplace clearing | as-is | per-settlement instancing requires the Society-monolith extraction (G2) |
| `barter.rs` BarterBook + `would_accept_barter_swap` | pre-money exchange at the campfire | as-is | same instancing dependency |
| `menger.rs` SaleabilityTracker + `IndirectFor` + money promotion | the money-emergence set-piece | as-is | `IndirectFor` is the SEED of trader behavior only — route arbitrage, transport cost, capacity, and price memory across settlements are new (§5.4) |
| `money.rs` `MoneyRegime::current_money_good()`, tender enums | dynamic money good; policy levers | as-is | the dynamic seam exists, but the engine runs TWO paths (`step_v2` for emergent-money societies, the M3 path for designated money) joined today only by a static bridge seed — banking on runtime-emerged money needs the V2→M3 runtime bridge (§13) |
| `ledger.rs` BaseLedger/ClaimsLedger, integer conservation | the un-dupeable game economy | as-is | mining inflow is NEW work — `mining_per_period` exists in docs only, not in `src/` |
| `bank.rs`, `issuer.rs`, `cantillon.rs` | late-game finance | as-is | era gates are NOT enough for generated worlds: solvency/error rules for bank failure, dead borrowers, multi-bank interactions are new (G8) |
| `project.rs` (Crusoe) + `capital.rs` M2 lines (`apply_labor_trade`, abandonment, salvage) | construction sites & multi-tick production | as-is | spatial sites, multi-worker delivery failure, buildings as persistent service providers, stockpile ownership are all new (§5.5) |
| `factor.rs` LaborBook | the jobs market | as-is | per-settlement instancing (G2); wage escrow for the two-rate loop (§4.3) |
| `expect.rs` adaptive PriceBelief | colonist price memory | as-is | none significant |
| `timemarket.rs` loans, debt settlement, tender surfaces | credit gameplay | as-is | same G8 solvency caveat |
| `record.rs`/`report.rs` tapes | the inspector UI's data source | as-is | ring buffers + spill (§9); unbounded vecs fixed at G0a |
| `metrics.rs` Gini/Lorenz/dispersion/idle-labor | the statistics bureau | as-is | none significant (read-only discipline kept) |
| `scenario.rs` EventKind + event application | the player command protocol (§7) | pattern carries | lab events are scheduled mutations with SILENT no-ops; commands need result/error semantics — new contract (§7) |
| `shadow.rs` credit-disabled counterfactual | the "court economist" advisor (§8) | as-is | shadow replays from a `MarketScenario`, not live state; the advisor is implemented as command-log replay from genesis (§8), which determinism makes exact but not cheap |
| `rng.rs` xorshift64*, determinism discipline | world-gen + replay/save | as-is | none |
| `sweep.rs` | balance/robustness CI harness | as-is | none (headless) |

What is explicitly **left in the lab**: the built-in scenarios, the M0–M17
acceptance suites (they become the engine's conformance suite, §10), the
CLI, per-tick shadow attachment, and the source-gate purism *as a global
rule* (kept for the econ crate only, §4.5).

## 3. Gap inventory — what praxsim provides nothing for

1. **Space**: map, terrain, resource nodes, movement, adjacency, stockpiles.
2. **Population dynamics**: birth, death, aging, household formation,
   migration; stable identity over a changing cast.
3. **Needs**: hunger/warmth/shelter/rest/social state that *generates* the
   value scales praxsim treats as fixtures.
4. **Content**: goods catalog, recipes, buildings, tech tree — praxsim has
   ~6 goods and hand-built project lines.
5. **Time structure**: praxsim has one tick granularity; a game needs a fast
   loop (movement, needs) under a slower economic tick (§4.3).
6. **Player interface**: rendering, input, UX, pacing, tutorialization.
7. **Robust emergence**: every lab proof runs on a hand-tuned cast; the game
   needs the same mechanisms to fire across arbitrary generated worlds
   (§10.3 makes this a gated, measured requirement — it is the project's
   single biggest technical risk).

---

## 4. Architecture

### 4.1 Workspace layout (new repo, e.g. `~/p/oikos`)

```
oikos/
  Cargo.toml            # workspace
  econ/                 # FORK of praxsim-core: the economy engine (pure std)
  world/                # map, terrain, resources, pathfinding, stockpiles
  life/                 # needs, demography, households, culture params
  content/              # data-driven goods/recipes/buildings/tech (+ loader)
  sim/                  # the orchestrator: two-rate loop, command queue,
                        # save/replay, era detection; owns econ+world+life
  ui/                   # Bevy app: rendering, input, inspectors (float OK here)
  tools/                # headless runner, robustness sweeps, balance CI
```

Fork, don't depend: the game will mutate the engine (dynamic goods, agent
arena, tape retention) on its own schedule. The praxsim repo stays alive as
the clean-room where new *mechanisms* (tax receivability, endogenous panic)
are proven against the invariant suite before being ported into `econ/`
(§13).

### 4.2 The simulation core is headless and the UI is a client

`sim` exposes exactly two surfaces:

```rust
pub fn advance(&mut self, commands: &[Command]) -> TickReport;  // one econ tick
pub fn view(&self) -> WorldView<'_>;                            // read-only
```

The UI thread renders from `WorldView` snapshots and queues `Command`s; it
can never mutate state. This is praxsim's read-only-metrics boundary
promoted to the process architecture, and it is what keeps the sim testable
in CI without a GPU.

### 4.3 Two-rate time

- **Fast tick** (`world`+`life`, ~10/game-hour): movement, work execution,
  needs accrual, hauling. No economic decisions, no money mutation.
- **Economic tick** (= the praxsim tick, 1/game-day): scale regeneration,
  order placement, market/labor/loan clearing, project advancement, debt
  settlement, era detection, metrics capture.

All money state mutates ONLY inside the economic tick (conservation checks
stay per-econ-tick and exact). The fast loop produces inputs (labor actually
delivered at the worksite, goods hauled to the stockpile) that the econ tick
consumes; the econ tick produces assignments (you were hired at wage w at
site s) that the fast loop executes.

**The delivery-escrow contract** (the lab's labor settlement pays and
advances synchronously, which a two-rate loop breaks — so the boundary gets
an explicit contract, not a hand-wave):

1. When a labor trade clears at econ tick T, the wage is moved into ESCROW
   (a ledger bucket on the employer side; conserved, auditable, not
   spendable by either party). No project progress is recorded yet.
2. During the fast interval, the fast loop logs delivered labor units per
   assignment (path-blocked, interrupted, or dead colonists simply deliver
   less, possibly zero).
3. At econ tick T+1, settlement is pro-rata on delivery: delivered fraction
   → wage released to the worker and project progress advanced by exactly
   the delivered units; undelivered fraction → escrow refunded to the
   employer, no progress. Death and path-block are not special cases; they
   are short deliveries.
4. All escrow transitions happen inside econ ticks; the conservation
   invariant covers the escrow bucket; the wage tape records both legs.

The same escrow pattern covers hauling contracts and caravan trips (goods
in transit are an escrowed claim, released or refunded on arrival/loss).

### 4.4 Determinism contract

- Integer-only state in `econ`, `life`, `world` logic (fixed-point where
  fractions are needed, as `Gold(u64)` already does). Floats exist only in
  `ui`.
- One seed; `BTreeMap`/`Vec` only (praxsim rule); fixed iteration orders.
- Save file = world seed + content hash + command log + ENGINE SCHEMA
  VERSION (+ periodic state snapshots, which also carry the version).
  Replay verification is a CI golden test.
- Versioning policy (replay is version-scoped, by design): a command log
  replays exactly only under the engine version and content hash that
  produced it. Balance patches and bug fixes bump the schema version and
  break replay compatibility deliberately; cross-version loading goes
  through state snapshots plus a deterministic migration step, never
  through re-running an old log on a new engine. Derived caches
  (pathfinding flow fields etc.) are never serialized — they rebuild from
  state, so they cannot desync a load.
- `Date::now()`-class calls banned in sim crates.

### 4.5 Purism: where it stays law and where it relaxes

- `econ` keeps the full discipline: no decision reads an aggregate; metrics
  modules unimportable from decision modules; compiler-enforced.
- `world`/`life` may read anything (pathfinding obviously reads the map);
  they cannot *write* economic state except through the defined inputs.
- The shadow counterfactual is NOT run per tick (cost); it becomes an
  on-demand advisor query (§8).

### 4.6 Content is data, not code

Goods, recipes, buildings, and tech live in declarative files
(`content/*.toml`, same strict unknown-key-is-an-error stance as the lab's
parser). `GoodId(u16)` stays; a `GoodRegistry` interns names → ids at load.
Properties relevant to money emergence (durability, divisibility,
portability, perish rate) are good *attributes* that flow into carrying
costs and storage losses — saleability itself stays measured from realized
acceptance, never assigned.

---

## 5. Core model transformations

### 5.1 Colonist = Agent + body + place

`econ` keeps its `Agent` (scale, stock, money, beliefs, roles) in a stable
generational arena (`AgentId` = index + generation; praxsim's bare `u32`
Vec-index is insufficient once colonists die). `life` owns the body
(age, health, needs, skills, household, culture params); `world` owns
position. A `ColonistId` ties the three. Skills multiply labor delivered
per hour (an integer bps multiplier on the fast loop's delivery, not a new
econ concept).

### 5.2 Needs generate the value scale (the biggest semantic change)

praxsim scales are static fixtures. In the game, each econ tick regenerates
each colonist's scale from need state:

```rust
fn regenerate_scale(
    needs: &NeedState,        // hunger, warmth, shelter, rest, social, security
    culture: &CultureParams,  // time-preference bias, leisure weight, novelty
    knowledge: &KnownGoods,   // what this colonist knows can satisfy what
) -> Vec<Want>                // strict descending urgency, Now/Later horizons
```

Rules: more depleted need → higher rank; each marginal unit listed
separately (diminishing MU preserved); `Leisure` stays a first-class want
(labor supply stays emergent); time preference is *structural* — a
present-biased colonist ranks `bread@Now` above `bread@Later` — and the
bias parameter is heritable with mutation (§5.6), so patient and impatient
lineages diverge and capital accumulation has demographics. Money wants
enter the scale exactly as the lab does it post-promotion (money buys
future wants), so the rank-walk reservation machinery is untouched.

This function is pure, data-driven, and the single most important tuning
surface in the game. It gets property tests (satiation monotonicity, no
empty scales, leisure present) and a dedicated balance harness.

### 5.3 Inventory becomes spatial

Agent `stock` splits into carried inventory (small) and claims on stockpile
contents at known sites. Market and barter participation requires presence
at (or a household runner to) the settlement's exchange site. Perishables
rot in the stockpile by good attribute. The conservation invariant extends
to goods: produced − consumed − rotted == Δstock, exact, per econ tick.

### 5.4 Settlements instance the markets

Each settlement owns one `BarterBook` (pre-money), one `OrderBook` per
traded good (post-money), and one `LaborBook`. Market *access* is spatial;
prices in different settlements diverge naturally. Inter-settlement trade
is performed by agents in the existing `Role::Trader`. The M5
`IndirectFor{target}` pattern supplies the praxeological SEED — holding a
good instrumentally rather than as a final want — but trader AI proper is
new work: route selection over price-belief differentials across
settlements, transport cost and capacity, spoilage in transit, and trip
risk (G7 scope, priced as such). Caravans are trader trips on the fast
loop under the escrow contract of §4.3. Roads (public
projects) cut trip cost → price convergence along routes is an emergent,
measurable phenomenon (and a great visualization).

### 5.5 Buildings are Projects; capital is real

- A construction site is a praxsim `Project`: labor and inputs advanced
  over econ ticks, abandonable, salvage fraction, capital consumed if
  walked away from — the bust mechanics come free.
- A completed building is a capital good: it either unlocks recipes
  (workshop), boosts labor productivity (tools/mill, integer bps), reduces
  costs (granary cuts rot; road cuts haul), or provides services (shelter
  satisfies a need directly).
- Production chains = recipes whose inputs are other recipes' outputs.
  Chain *depth* is never stored; "structure of production" remains a
  derived statistic — now visible to the player as the supply-web view.

### 5.6 Population dynamics

Births when household need-security clears a threshold (food margin,
shelter); deaths by age/starvation/exposure. Children inherit
`CultureParams` with bounded mutation → cultural/time-preference drift
under selection. New colonists enter the arena; estate settlement has a
defined timing: the fast loop only RECORDS a death; at the next econ tick
the estate settles atomically — open hire/ask orders cancelled and
reservations released, escrowed wages resolved per §4.3, debts settled or
defaulted through the existing debt machinery, money and stockpile claims
passing to the household (or to the settlement commons when none exists).
No ownership changes hands in the fast loop. Target band stability is a G4 gate:
across 100 seeds, no extinction and no Malthusian blowup inside the tuned
parameter envelope.

### 5.7 Tech and knowledge

Research is a project line consuming labor (scholar time) and sometimes
goods, producing `Knowledge` units that unlock recipe/building defs from
`content/`. Tech TIERS gate what is buildable; they do not set prices,
wages, or any economic quantity. Diffusion: a recipe known to a settlement
spreads with trade contact (knowledge rides the caravan).

### 5.8 Eras are detected

Institutional eras are derived statistics with hysteresis, e.g.:

```
Forager      — no sustained exchange
Barter       — sustained reciprocal exchange (barter tape over window)
Money        — a good passed promotion (the lab's saleability rule, verbatim)
Specialist   — >X% of colonists earn majority of consumption via exchange
Capital      — production-chain depth ≥ k sustained; tool capital per worker
Credit       — a chartered bank's claims circulate (claims tape)
Fiat/Modern  — state paper is the marginal settlement medium (payment tape)
```

The era banner the player sees is a *measurement*. Tech tiers (stone,
bronze, iron, …) are the content axis and advance by research; the design
intends them to roughly co-move, but nothing enforces it — a colony can be
iron-age in tools and barter-age in institutions, and that divergence is
interesting, visible, and true.

### 5.9 Money, banking, and the state — the late game is the lab

- Money emergence: M5/M6 verbatim, per settlement, on whatever goods the
  map provided. Coinage: a mint building standardizes units (cuts
  verification cost, a small trade-friction attribute).
- Banking: the player charters a bank (or licenses an NPC one). Reserve
  ratio, convertibility, suspension — the M9/M10 machinery — become policy
  toggles with the lab's exact balance-sheet behavior, including
  deterministic redemption runs.
- The state: the player's government is an in-ledger agent with a treasury.
  Taxation requires the **tax receivability mechanism — now proven in the lab
  (M21)**: a per-agent tax is an ordinary issuer debt (zero principal, the levy
  as due) gated by a `TaxReceivability` policy, and the lab's headline closes a
  full chartalist circuit — a fiat-only tax pulls a leisured worker into
  fiat-wage labor and makes fiat circulate and return to the issuer, while its
  falsification twin (the levy removed) leaves the worker idle. What the lab has
  NOT yet built is the treasury-spending loop (receipts retire or vault, not
  fund works), broad/sectoral levies, or media-targeted acquisition (§13), and
  this still blocks more than late-game finance: the FIRST
  state-funded public work needs a funded treasury. The funding ladder is
  therefore explicit per era: Forager-era community projects use direct
  labor (no money exists); money-era public works are commissioned as
  community projects funded by voluntary project-purpose contributions (the
  M2.5 bundle machinery) or by a named private payer the player recruits;
  STATE-funded works — treasury spending, Cantillon-routed to named
  contractors the player picks — unlock only once taxation exists. The
  game never conjures a treasury from nowhere.
- Fiat and the cycle: the full regime ladder and the M11–M17 tender
  policies are late-game policy levers (seven surfaces, with one known
  hole: the wage surface ships fiat-only until the lab's bank-claim wage
  tender milestone lands, §13). The M17 result is, verbatim, a
  gameplay truth: mandate fiat wage acceptance and printing transmits into
  booms and busts; leave wages free and printing is inert. Busts present as
  abandoned construction sites — capital consumption the player can walk
  through.

---

## 6. The era arc as a player experience

1. **Founding band (Forager)**: M0-style direct production with spatial
   needs; player designates gathering zones, the fire, the first shelter
   (public projects). Drama: survival margins.
2. **Barter camp**: surplus appears; the barter book at the campfire;
   player sees the saleability panel start ranking goods.
3. **The money moment**: one good crosses promotion — a set-piece event
   (different per map: salt, shells, iron rings, silver). Prices appear;
   the colony has a unit of account. This is the game's signature scene.
4. **Specialization and capital**: workshops, tools, hiring; the labor
   market opens; roundabout chains deepen because savers fund them.
5. **Trade age**: second settlement, caravans, price-convergence play,
   roads as infrastructure investments with measurable returns.
6. **Credit age**: bank charter; deposits, notes, lending; the player sets
   reserve policy and learns what a run is.
7. **Modern age**: state money, taxation, fiscal works, fiat issuance —
   and the consequences. The endgame challenge is *keeping an advanced
   financialized economy stable*, with the ABCT as the live antagonist.

Pacing risk is real (markets are slow-burn); mitigation is the legibility
layer (§8) and survival/external pressure in early eras (weather cycles,
raid shocks destroying capital — exogenous in v1).

---

## 7. Player interaction: commands are events

praxsim's `EventKind` + apply-at-tick-boundary architecture is already a
command protocol (M0-ignored, shadow-preserved semantics included). The
game extends it:

```rust
enum Command {
    DesignateZone { kind, area },            // gather/farm/build/stockpile
    CommissionProject { def, site, budget }, // public works; player picks payer
    SetPolicy(PolicyEvent),                  // ALL M3..M17 events, era-gated
    CharterBank { reserve_ratio, .. },
    LevyTax { base, rate, receivable },      // needs lab mechanism (§13)
    Research { tech },
    Expedition { target },                   // explore/found settlement
    // never: SetPrice, SetWage, AssignJob — the anti-command list is part
    // of the spec and is enforced by the type system (no such variants).
}
```

The anti-command list is pillar 1 made concrete: the player cannot set a
price, a wage, an interest rate, or a colonist's job. They can always see
why the economy did what it did (§8) and reshape institutions/environment.

Commands differ from lab events in one load-bearing way: lab `EventKind`
application tolerates SILENT no-ops (an event naming a missing debt id
simply does nothing — fine for authored scenarios, unacceptable for player
input). Every `Command` application returns a result —
`Applied | Rejected(reason)` — validated at the tick boundary (era gate,
target existence, treasury balance, zone legality), and rejections surface
in the UI with the reason. No command ever silently does nothing.

---

## 8. Legibility: turning tapes into UX

The lab's audit tapes become the game's explanation engine — this is the
feature that makes an emergent economy playable:

- **Price inspector**: click a price → the trades behind it (tape query),
  the bid/ask ladder, the marginal pair.
- **Coin provenance**: click money → ledger path (minted/issued where,
  Cantillon receipt tags, which surface it last settled on).
- **Colonist "why"**: click a colonist → their current value scale, top
  unsatisfied wants, last rank-walk outcome ("refused the job: leisure
  outranked the wage").
- **The Statistics Bureau**: metrics.rs as an in-game building the player
  constructs to *unlock* aggregate dashboards (Gini, price dispersion,
  idle labor) — diegetic justification for the read-only layer.
- **The Court Economist (advisor)**: on demand, computes the credit-disabled
  counterfactual ("had there been no credit expansion, the natural rate
  would be ~X; your gap is Y") — the lab's authoritative ABCT signal as a
  hireable, era-gated feature. Implementation note: the lab's shadow
  replays from a `MarketScenario`, not from live state; the advisor is
  therefore implemented as a COMMAND-LOG REPLAY from genesis with credit
  origination disabled, which determinism makes exact. Cost scales with
  run length, so it runs async with a progress bar and an in-fiction
  cooldown ("the economist needs a season to prepare the report"), and
  memoizes up to its last computed tick.

---

## 9. Performance and scale plan

Target: 500 colonists, 3 settlements, 60 econ-ticks/min headless on one
core (UI separate). This target is a budget to be defended, not an
assumption: CI perf checks run from G3 onward (§11) so drift is caught per
milestone, not discovered at the end. Knowns from the lab (Concern 5 list)
and new costs:

- Per-settlement books bound matching cost; CDA is cheap (BTreeMap ladders).
- Scale regeneration is the hot loop: pure function over ~500 colonists
  once per econ tick — batched, cache-friendly, integer-only; budget 10ms.
- Tapes: ring buffers in memory (inspector window) + append-only spill to
  the save directory (full audit); `Society::{trades,loan_trades,…}`
  unbounded growth fixed at G0.
- Pathfinding on the fast loop is standard (flow fields per stockpile);
  not econ-coupled.
- Known O(N²) spots in the lab (input aggregation, per-candidate
  provisioning recompute) get fixed at G0 with the conformance suite as
  the safety net.

---

## 10. Testing and verification strategy

1. **Conformance suite**: the forked `econ` crate must keep passing the
   lab's ported unit/property tests (rank-walk, CDA, ledger conservation,
   tender behavior, promotion rule). The lab's *scenario* tests stay in the
   lab; a thin compat layer in `tools/` replays a handful of golden lab
   scenarios against `econ` to catch behavioral drift (M3 golden hash
   included) until the engines deliberately diverge — after which the
   game pins its own goldens.
2. **Property tests across seeds** (the lab's direction-not-magnitude
   philosophy at scale): money conserved exactly; goods conserved with
   rot; no insolvent fills; harvest shock → food price rises (sign only);
   tool adoption → output per worker rises; credit expansion with fiat
   wage tender → structure lengthens (and does NOT without it — the M17
   pair as a permanent game property test).
3. **Robustness gates** (the de-risking instrument for §3.7): the G5 gate
   is scoped to a DECLARED WORLDGEN ENVELOPE, so it cannot be gamed by
   cherry-picked worlds or failed by degenerate ones. The envelope is
   published as content parameters and versioned with the gate; the
   initial envelope: population 30–200; at least two goods with tradeable
   surplus beyond subsistence; barter-encounter rate above a stated floor
   (settlement layout cannot isolate every household); spoilage and
   transport friction below stated ceilings — initial envelope parameters
   to be refined per `docs/emergence-study.md` (M19). Inside the envelope: money
   emerges in ≥80% of 200 randomized worlds within the era-3 time budget,
   with ≥3 distinct winning goods across the corpus. Outside the envelope
   the REQUIRED result inverts where appropriate: worlds with no tradeable
   surplus must NOT promote a money good (true negatives are part of the
   gate). Promotion is one-way in the engine, so "flapping" is not a
   meaningful metric (rev-1 error); the stability metrics are instead the
   time-to-promotion distribution (no long degenerate tail inside the
   envelope) and zero spurious promotions outside it. Demotion/competing
   re-monetization is explicitly out of scope for v1. Failure here pauses
   content work and redirects to mechanism tuning — this gate is the
   project's go/no-go.
4. **Golden replays**: every milestone pins seed+command-log replays;
   byte-identical `TickReport` streams in CI.
5. **Balance CI**: `tools/` sweeps (the lab's sweep.rs) over content
   parameters; dose-response curves (like the credit sweep) become balance
   regression charts.

---

## 11. Roadmap

Each milestone is shippable headless (UI lags by design) and gated on its
DoD. Rough sizing assumes one focused developer + agent orchestration.

- **G0a — Fork & behavior-preserving cleanup.** Fork praxsim-core → `econ`;
  port the conformance suite + lab-golden compat harness; tape ring
  buffers + spill; fix known O(N²) spots (behavior-preserving only). NO
  data-model migrations in G0a. *DoD: lab goldens replay byte-identical
  through the fork.*
- **G0b — Engine migrations behind a compat layer.** Dynamic
  `GoodRegistry`; generational `AgentId` arena (priced honestly: the id is
  embedded in ledgers, tapes, debts, banks, metrics, barter, factor, and
  market records — this is a wide, mechanical migration); `Command`
  result/error semantics over the event machinery (§7). A COMPAT SHIM maps
  the lab's static goods and bare-u32 ids onto the new types so the golden
  harness keeps running through it — rev-1's G0 demanded byte-identical
  goldens AND the migrations that break them, which was self-contradictory.
  *DoD: conformance suite green natively; lab goldens byte-identical
  through the shim.*
- **G1 — Needs → wants (mechanism only).** `life` crate: NeedState,
  CultureParams, `regenerate_scale`; food/warmth/rest loop; death by
  starvation. Deliberately PRE-SPATIAL: the DoD asserts mechanism
  properties, not balance — balance tuning before space exists would tune
  a model G2 throws away. *DoD: scale-generation property tests (satiation
  monotonicity, leisure present, no empty scales, deterministic) across 50
  seeds; a 50-colonist headless camp survives 5 years as a smoke test
  only; harvest-shock food-price sign response. All numeric tuning
  deferred to G2+.*
- **G2 — Space + the monolith extraction.** `world` crate: tile map,
  resource nodes, movement, work delivery, stockpiles, one exchange site;
  two-rate loop with the §4.3 escrow contract. The named engine work:
  extract settlement-scoped market/labor/barter services from the
  `Society` monolith (the prerequisite for everything multi-settlement).
  The REQUIRED debug viewer ships here and includes the first two
  inspectors (price → trades, colonist → scale/why) — legibility is the
  fun-risk mitigation and cannot wait for G9. *DoD: spatial delivery
  reconciles through escrow with exact conservation; distance measurably
  affects realized prices; inspectors answer "why" from live tapes.*
- **G3 — Production & construction (an EMERGENCE milestone).** Buildings-
  as-Projects, recipes from `content/`, tools as productivity capital.
  Flagged honestly: the signature test demands entrepreneurship, input
  procurement, job selection, and pricing to compose — harder than
  anything before it, so it gates in two steps. *DoD: (G3a) a
  grain→flour→bread chain operates end-to-end with seeded recipe knowledge
  and a hand-placed first entrepreneur; (G3b) the same chain ARISES from
  price spreads with no scripted assignment on ≥60% of 50 seeds. Perf
  budget checks begin here (headless econ-ticks/min on a reference world,
  tracked in CI from G3 on).*
- **G4 — Demography.** Births/deaths/aging/households; culture inheritance;
  estate settlement per §5.6. *DoD: 100-seed population-stability band;
  patient-lineage capital accumulation measurable.*
- **G5 — Money at scale.** Menger machinery on generated worlds; mint.
  *DoD: the §10.3 envelope-scoped robustness gate, including the
  true-negative requirement. GO/NO-GO checkpoint.*
- **G6 — Tech & eras.** Research projects, tier unlocks, era detection with
  hysteresis. *DoD: stone→capital-era progression driven by measured
  saving, not timers, on ≥70% of seeds in budget.*
- **G7 — Trade.** Second settlement, trader AI seeded by `IndirectFor`
  (route choice, transport cost, capacity — new work, §5.4), caravans
  under escrow, roads. *DoD: price convergence along a built road is
  measurable and visualized.*
- **G8 — Institutions & finance.** Charters, credit, regime ladder, tender
  policies, taxation (after the lab proves receivability, §13), the
  advisor query (§8). Era gates are necessary but NOT sufficient: before
  any gate opens in generated worlds, the solvency/error ruleset must be
  specified and tested — bank insolvency and closure, issuer default,
  debts of dead agents (§5.6), multi-bank claim interactions, and what
  happens to depositors in each case. Requires the V2→M3 runtime bridge
  (§13) so banking runs on the money the world actually monetized. *DoD:
  the M17 wage-tender experiment reproducible in-game by a player using
  only Commands; a forced bank insolvency resolves by rule, not panic.*
- **G9 — Game shell.** Full Bevy UI over the G2 viewer, remaining
  inspectors (§8), save/replay UX, scenario starts, tutorial era. *DoD:
  a stranger reaches the money moment unassisted.*

---

## 12. Risks

1. **Emergence robustness (top risk)** — mitigated by the G5 gate and by
   keeping the lab alive for mechanism surgery.
2. **Fun/pacing** — emergent economies are slow; mitigated by legibility
   (§8), survival pressure early, and the era set-pieces; prototype fun
   checks start at G3, not G9.
3. **Scale tuning** — integer economics with ~unit-sized trades worked in
   the lab; the game needs quantity scaling (grain in hundreds). The
   fixed-point already supports it; content tuning must keep marginal
   units meaningful (G1/G3 balance harness).
4. **Scope** — this is a multi-year game. The roadmap is sequenced so G0–G5
   alone produce a publishable interactive artifact ("watch money emerge
   in a living colony") even if the full civ arc takes longer.

## 13. Relationship to the praxsim lab

The lab stays the clean-room: mechanisms are proven there against the
invariant suite, then ported into `econ`. Lab milestones the game needs,
in order of urgency:

1. **Tax receivability** — **DONE (M21).** The lab proved the chartalist
   circuit: a per-agent tax is an issuer debt gated by a `TaxReceivability`
   policy, and a fiat-only levy alone pulls a leisured worker into fiat-wage
   labor and makes fiat circulate and return to the issuer (falsified by the
   tax-free twin). What still blocks the first STATE-funded public work
   (§5.9's funding ladder) is the **treasury-spending loop**: M21 receipts
   retire or vault rather than fund works, so the Cantillon-routed
   treasury-spend mechanism — plus broad/sectoral levies and media-targeted
   acquisition — is the next lab rung for the state.
2. **Runtime V2→M3 bridge** (blocks G8): today the engine joins the
   emergent-money runtime (`step_v2`) to the banking/finance runtime only
   through a static bridge seed. The game's whole arc — the world
   monetizes a good, then banks form ON that money — requires the live
   handoff. Rev 1 omitted this item; it is the largest single piece of
   engine work hiding behind §2's money-row caveat.
3. **Population scaling / robustness harness** (feeds G5's gate directly).
4. **Endogenous acceptance/panic** (upgrades tender laws and bank runs from
   policy gates to behavior — the game wants this for realism; the lab
   wants it for the next falsification rung; also feeds G8's solvency
   ruleset).
5. **Bank-claim wage tender** (completes the wage surface; G8 polish).
6. **Mining inflow** (`mining_per_period` is a documented concept with no
   implementation in `src/` — small, but it must be built, not ported).

## 14. Open decisions (user input wanted, none blocking before G2)

- Working title.
- Engine: Bevy (recommended: Rust-native, ECS fits `ui`/`world`, active
  ecosystem) vs macroquad/egui-only for a leaner start.
- Art direction (top-down tiles vs isometric) — affects `world` only.
- Raids/external pressure: include as v1 shock source or cut entirely.
- New-repo name and whether `econ` should eventually be published as a
  standalone crate (the emergent-economy engine has standalone value).

## 15. Review log

**Codex spec review, 2026-06-12** (independent second-model review; codex
read the spec and spot-checked its claims against `praxsim-core/src`;
~555k tokens). Disposition:

- **Accepted and folded into revision 2:** the §2 "as-is" labels graded
  mechanism and integration separately (Society is a monolith; per-
  settlement instancing is the G2 extraction); `fill_labor_pair` corrected
  to `apply_labor_trade` (factor.rs:586); `mining_per_period` reclassified
  from carry-over to new work (docs-only, no implementation); generational
  `AgentId` repriced as a wide migration (G0b); G0 split into G0a/G0b with
  a compat shim — rev 1 demanded byte-identical goldens and the migrations
  that break them in the same milestone; the §4.3 delivery-escrow contract
  replacing the short-delivery hand-wave; estate-settlement timing
  (§5.6); the era-indexed public-works funding ladder and the earlier
  blocking scope of tax receivability (§5.9, §13); `Command`
  result/error semantics vs the lab's silent-no-op events (§7); the
  advisor reimplemented as command-log replay from genesis (§8); the G5
  gate scoped to a declared worldgen envelope with true-negative
  requirements, and the incoherent "promotion flapping" metric replaced
  (promotion is one-way) (§10.3); G3 reframed as an emergence milestone
  with a two-step gate; G8 gated on an explicit solvency/error ruleset,
  not era gates alone; save/replay schema versioning (§4.4); CI perf
  budgets from G3 (§9, §11); the V2→M3 runtime bridge added to §13 as
  the largest hidden engine dependency; `IndirectFor` demoted from
  "trader AI" to "seed of trader AI" (§5.4); inspectors pulled forward
  into the G2 debug viewer.
- **Refuted:** "`step_v2()` exists but is not reached" — the emergent-money
  scenarios (`menger-salt-money`, `menger-gold-money`) demonstrably run
  and promote through `step_v2` (`Society::step` routes there after
  `try_step` defers). The substance behind the misread — that banking
  never runs on runtime-emerged money — is real and is exactly the §13
  bridge item.
- **Pushed back with adjustment:** G1-before-G2 ordering kept, but G1's
  DoD narrowed to mechanism properties with all balance tuning deferred
  (codex's "tuning a fake model" risk was the real content of the
  objection); UI timing kept at G9 for the full shell, but the
  legibility-critical inspectors moved into the mandatory G2 viewer.
