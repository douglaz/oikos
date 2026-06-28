# impl-39 — S23a: Private Land Tenure (does scarce, excludable, losable *place* finally stabilize an occupation?)

Status (spec): REVISED per Codex spec-review rounds 1-2 (6 P1 + 3 P2, then 3 P1 + 1 P2 folded in, §7); pending confirmation. Base:
master `496566e` (S22f landed + the article at the arc's turn). Opens the **S23 private-property arc**. Composes on S22a (`endogenous_cultivation_entry`) on the
expanded `ROSTER_HOUSEHOLDS = 8` base; the other S22 exit-cost levers (skill, profit-stay, capital,
commitment) are **OFF** in the headline so land tenure is the only new exit-cost mechanism. Scoped by Codex
("Spec S23a as ResourceNode-owned spatial homesteading tenure …").

## 0. One-paragraph summary

The role-topology arc (S22a–f) found that occupation does not self-form under any lever that leaves the
**exit/re-entry cheap** — hunger, skill, profit, earned capital, inherited capital all failed; only S22f's
**voluntary commitment** (an institution that makes *leaving* costly) produced a stable core, and even that
only a core. But all of S22 ran on a world where the **means of production is a commons**: a `ResourceNode`
has no owner, so anyone walks up and harvests, and a lapsed farmer re-enters for free. S23a switches on the
missing Austrian precondition — **private property in scarce productive land** — and asks whether *that*,
rather than a contract, makes exit costly enough to stabilize an occupation. The mechanism is **spatial, not
monetary** (which dissolves the bootstrap trap that money-priced land would create): grain plots are
**heterogeneous in quality and distance** (good land near the centre, poor land far), a plot is **claimed by
homesteading labor** (money-free, always open to anyone who reaches and works unowned land), **harvested only
by its owner**, **lost if left idle** for `LAND_IDLE_LIMIT` ticks, and **inherited** on death. So leaving
farming costs you your *place*: abandon your good central plot and a nearer agent claims it; to return you can
only homestead **far, poor** land (low yield + long travel), which may not beat just buying food. The
re-entry penalty is **mechanical** (the land that's left is worse) — no money and no agent foresight required.
This pulls in Lockean homesteading + Ricardian differential rent (the extensive margin) + von Thünen
location, all on spatial machinery the engine **already has** (positioned nodes with `regen`/`cap` quality,
Manhattan distance, pathfinding/travel cost). The honest hypothesis: a scarce, excludable, losable spatial
asset can stabilize the cultivator occupation that skill/profit/tools/inherited-tools could not. **Central
trap:** making "owner" a *pin* (only deed-holders farm) would be circular — so entry stays **open** (anyone
can homestead unowned land), the result must be a **bounded minority** of owners with a **surviving
non-owner buyer cohort** and **viable marginal land still available**, and a battery of controls must show
the stickiness *vanishes* when exclusion / loss-on-idle / scarcity / the quality-gradient is removed.
Classify-not-tune; money + mortality + provenance + conservation must survive; the money land-market is
deferred to **S23b**.

## 1. Why this milestone, why this lever — and the grounding

S22 isolated the boundary to **costly exit**: every lever that left re-entry cheap failed. The deepest reason
re-entry was cheap is that **land was free** — the commons. The canonical economic source of occupational
persistence that is neither productivity nor a contract is **private property in a scarce productive factor**:
you hold a specific plot, leaving forfeits it, and getting back in is costly. This is the precondition the
whole Austrian frame presumes and OIKOS never implemented.

**Grounding (verified — the spatial substrate already exists; only ownership is new):**
- `world::ResourceNode { pos, good, stock, regen_per_tick, cap }` (world/src/node.rs) — **land quality is
  already per-plot tunable** (good land = high `regen`/`cap`, poor = low). It has **no `owner`** today: any
  agent harvests any node (the commons).
- `world::Grid` (world/src/grid.rs) — `Pos`, **Manhattan distance**, terrain/impassable, pathfinding/BFS;
  an existing milestone (G2b) already uses "distance affects price." Agents already pathfind to nodes, so
  **distant land already costs travel time** (ticks away from eating and from the exchange).
- So the **only new engine surface is excludable ownership** of a node (claim/harvest-gate/abandon/inherit),
  implemented **sim-side and gated** so the `world` crate and all goldens are untouched when off.

**Design decisions (Codex):**
- **Spatial scarcity, not money price** (dissolves the bootstrap: money needs pre-money production, which
  needs free homesteading). Money-priced land / sale / capitalized rent is **S23b**, not here.
- **The quality/distance gradient is load-bearing and in the headline**, not enrichment — it is what keeps
  entry *open* (you can always farm *somewhere*, just worse) rather than a hard slot cap ("only deed-holders
  farm").
- **Own the `ResourceNode`** (each grain node = a plot). Cell ownership / land improvement is S23b/c.

## 2. The central question and pre-named outcomes

**Central question.** When grain plots are **excludable, scarce, heterogeneous (good-near / poor-far),
claimed by homesteading labor, harvested only by their owner, lost if left idle, and inherited** — does a
**persistent owner-cultivator cohort** form (churn ≤ matched-commons baseline drop + a persistent membership
cohort that ARE the plot-owners), with re-entry made costly *spatially* (lapsed farmers measurably pushed to
worse/farther land) — while entry stays **open** (anyone may homestead unowned land), a **non-owner buyer
cohort survives and materially buys**, **viable marginal land remains available**, and money / mortality /
provenance / conservation all hold — AND is the stickiness genuinely from scarce-losable-place (proven by
controls), not a fiat "only owners farm" pin?

**Primary success = `LandTenureStickySuccess`** (all, across `SEEDS`, vs the matched-seed **commons
baseline** = same scenario with `private_land_tenure = false`):
1. **Tenure non-vacuous** — plots are actually claimed by homesteading, harvested owner-only, and ≥1 plot is
   lost-on-idle and re-claimed by a *different/nearer* agent (the mechanism bites).
2. **Churn falls materially** — per-ever-cultivating churn ≤ `CHURN_DROP` (0.5) × the matched-commons baseline.
3. **A persistent owner-cultivator cohort forms** — ≥ `PERSIST_COHORT` (4) distinct agent ids cultivate ≥
   `PERSIST_FRACTION` (0.5) of the final window **and are the plot-owners** (the sticky cohort is the
   landholding one, not a coincidental rotation).
4. **Ownership is a bounded MINORITY with open entry** — owner share ≤ `OWNER_SHARE_MAX` (0.6) of
   ever-cultivating/eligible agents, **and** unowned (marginal) plots remain available/used so a non-owner
   *could* still homestead (entry not closed). Universal/near-universal ownership ⇒ `HardBarrier`, not success.
5. **Spatial hysteresis is real** — lapsed owners who re-enter get, on average, **worse/farther** plots than
   stayers (lower `regen`/`cap` and/or greater distance-to-centre) — the trace that re-entry is costly.
6. **A surviving non-owner buyer cohort materially buys** — post-promotion bought food ≥
   `MATERIAL_BOUGHT_FLOOR`, living (the market is not just owners feeding themselves).
7. **Money survives** — SALT promotes on `SelfProduced` bread and remains money; food materially bought after.
8. **Provenance clean** — sold/pre-promotion bread is `SelfProduced`; `seeded_minted == 0`.
9. **Mortality + conservation + the plot-registry invariant hold** — grain conserves every tick
   (harvest/ownership is metadata, no good created/destroyed); each plot has ≤1 owner; every
   claim/abandon/inherit transition preserves the finite plot set.
10. **NOT downgraded by the controls (§4)** — `non_excludable_deed`, `free_reclaim`, `abundant_good_land`,
    `no_forfeit`, `property_off` each fail to reproduce the stickiness.

**Finding modes (pre-named; first-class; verdict test prints the classification, does NOT assert SUCCESS):**
- `TenureLeverInert` (precondition / non-vacuity failure) — plots never get claimed, or never lost+reclaimed,
  or ownership doesn't gate harvest (the mechanism didn't bite).
- `ConservationBroken` / `extinct` — any grain-conservation break, registry invariant break, or colony death.
- `HardBarrier` (entry is impossible — a pin by scarcity, Codex P2.3) — **no viable marginal entry exists**:
  `viable_marginal_plots == 0` for a sustained window (see the `VIABLE_MARGINAL` definition in §2a), or owner
  share → universal, or every observed non-owner claim *attempt* fails. Non-owners cannot enter at all.
- `LandMonopolyCull` (entry is possible but ownership damages the market) — viable marginal land *does* exist,
  BUT the owner grain share ≥ `MONO_SHARE` (0.75) AND the non-owner buyer cohort collapses (buyers die /
  post-promo bought < `MATERIAL_BOUGHT_FLOOR`). Distinct from `HardBarrier`: here entry is open, the harm is
  concentration.
- `CommonsEquivalent` — the controls show title is inert (ownership doesn't change harvest/behaviour); behaves
  like `property_off`.
- `MoneyFailureFromTenure` — tenure disrupts the pre-money barter bootstrap; SALT fails to promote / demonetizes.
- `NoStickinessDespiteLand` — tenure bites (plots claimed, lost, re-claimed worse) but churn persists > the
  bar AND no persistent owner cohort.
- `LandTenureStickySuccess` — all ten success clauses, not downgraded.

**Ordered classifier (top-down, first-match-wins — S21i non-gameability discipline):**
`TenureLeverInert` → `ConservationBroken`/`extinct` → `HardBarrier` (entry impossible) → `LandMonopolyCull`
(entry open but concentration culls buyers) → `MoneyFailureFromTenure` → `CommonsEquivalent` → **then the
explicit final gate:** `if ALL TEN success clauses (§2.1–§2.10) pass { LandTenureStickySuccess } else
{ NoStickinessDespiteLand }`. Predeclare every threshold as a `const`; do NOT fit.

## 2a. `VIABLE_MARGINAL` — the measurable open-entry definition (Codex P1.5)

A grain plot counts as **viable marginal land** at a tick iff: it is **unowned**, **reachable** by ≥1 live
non-owner (on the 1-D line every in-bounds plot is reachable), and its `regen`/`cap` are ≥ the declared
`VIABLE_REGEN_FLOOR` / `VIABLE_CAP_FLOOR` (a plot that yields ~nothing does NOT count). **Open entry** (success
clause §2.4) requires `viable_marginal_plots ≥ 1` through the final window **and** ≥1 *observed* non-owner
claim+production on a marginal plot during the run (entry is demonstrated, not merely theoretical). `HardBarrier`
is its negation: no viable marginal plot, or every non-owner claim attempt fails.

## 3. Engine design (additive, default-off, conservation-safe)

1. **NEW default-off flag** `ChainConfig::private_land_tenure: bool` + a gated **mode/config surface** (Codex
   P1.6 — every control needs an explicit field, all serialized ON-only under the gate): `land_idle_limit: u16`
   (the forfeiture clock, §3.6); `harvest_gate: bool` (default true; OFF = the `non_excludable_deed` control);
   `forfeit_on_idle: bool` (default true; OFF = the `no_forfeit` control); `reclaim_reserved_for_prior_owner:
   bool` (default false; ON = the `free_reclaim` control); plus the grain-plot **layout** (§3.2). Helper
   `private_land_tenure_active(&self)` = flag on AND `endogenous_cultivation_entry_active()`. Canonicalize
   ON-only with the **next free flag-digest tag (13** unless master advanced) + `land_idle_limit` +
   `harvest_gate` + `forfeit_on_idle` + `reclaim_reserved_for_prior_owner` + the layout params **and the
   steering state that persists across ticks: the plot registry (per-plot `owner` + `idle counter`, §3.3) and
   the per-agent `carried_grain_source` markers (§3.6)**. (The unowned-plot reservation, §3.5(b), is recomputed
   each pass from positions+tasks+registry — a pure function of already-serialized state, so it is NOT a
   separate digest field.) Off ⇒ byte-identical.

2. **Concrete grain-plot layout (Codex P1.4 — predeclared, 1-D, NOT tunable by rb-lite).** The world is a
   `width = 64, height = 1` line, exchange at `Pos::new(0, 0)`; distance-to-centre = `x` (Manhattan on the
   line). Replace the single rich grain node with a **gradient of grain plots** (each its own `ResourceNode`/
   plot):
   - **GOOD plots:** `GOOD_PLOTS` (default **4**) at `x ∈ {2,3,4,5}`, `regen = 64`, `cap = 8000` (the current
     rich-grain quality).
   - **MARGINAL plots:** `MARGINAL_PLOTS` (default **8**) at `x ∈ {12,18,24,30,36,42,48,54}`, `regen = 12`,
     `cap = 1000` (far + poor, but above the viability floor).
   - **Scarcity framing (Codex round-2 P1.3 — NOT tied to total eligible-agent count).** The 8-household roster
     has far more potential entrants (~dozens) than plots, but that is fine: most agents are *buyers*, and in
     S22 only ~4–5% cultivate at any instant. So the relevant scarcity is over **simultaneous would-be
     farmers**, not the whole roster. Pin it relationally instead: (a) good land is **contested** —
     `GOOD_PLOTS (4)` is small, on the order of the target persistent owner cohort (`PERSIST_COHORT = 4`), so
     good plots cannot host more than a handful of owners; (b) **entry stays open** — `MARGINAL_PLOTS (8)`
     greatly exceeds the typical number of simultaneous cultivators, so a viable marginal plot is essentially
     always free to homestead. Do **not** assert any `< eligible cultivators` inequality. `VIABLE_REGEN_FLOOR =
     8`, `VIABLE_CAP_FLOOR = 256` (the marginal plots clear it; §2a).
   - **Predeclared sweeps (no tuning):** `land_idle_limit ∈ {6, 12, 24, 48}` (default **12**); gradient
     steepness = marginal `regen ∈ {6, 12, 24}`; scarcity = `GOOD_PLOTS ∈ {2, 4, 6}`. The scarcity and
     idle-limit axes MUST be outcome-driving (too abundant → `CommonsEquivalent`/no cohort; too scarce →
     `HardBarrier`/`LandMonopolyCull`; a middle band → potential success). The **`abundant_good_land` control**
     (§4) is a *concrete high* `GOOD_PLOTS = 16` (good land no longer scarce — well above any plausible owner
     cohort), NOT a "≥ eligible agents" comparison.

3. **Plot registry (sim-side, gated — keeps `world` and goldens untouched):** a per-plot map keyed by `NodeId`
   over the grain plots: `owner: Option<AgentId>` + a per-plot **idle counter** (§3.6). It **steers harvest
   behaviour**, so it MUST be serialized into `canonical_bytes` **ON-only** under the gate. Not a good — no
   conservation term; grain conservation is unchanged.

4. **Harvest-event detection (Codex P1.2 — a concrete deterministic event source).** The fast loop already
   attributes deposits via carry diffs but has no per-agent/per-node harvest-completion signal. Add one:
   **snapshot each agent's `(task, carry)` immediately before each fast `world.tick`; after the tick**, if the
   agent's task was `GoHarvest{,WithRoom}(node)` for a grain plot AND its carry increased (it pulled grain),
   record `worked(agent, node, moved)` for this tick. This is the single deterministic source for both *claim*
   (§3.5) and *idle-reset* (§3.6).

5. **Pre-arrival validation + a single-targeter reservation + claim (Codex P1.1 + round-2 P1.1) — the gate must
   act BEFORE `world.tick`, not just at assignment.** Because `World::apply_arrival` harvests blindly, each fast
   tick runs a **deterministic pre-`world.tick` validation pass** over every agent whose task is
   `GoHarvest{,WithRoom}(grain node N)`, in a fixed order, doing two things:
   - **(a) Owned-by-other → reroute.** If `harvest_gate` is on and `N` is owned by **another** live agent
     (a competitor claimed it, or the agent's own plot reverted), cancel and re-route per §3.7.
   - **(b) Unowned-plot reservation — closes the same-tick "stampede" race (round-2 P1.1).** Pre-tick
     validation alone would let *many* agents target the same unowned nearest plot (e.g. all rush `x=2`) and
     all harvest it the same tick before the post-tick claim assigns ownership. So, in the validation pass, an
     **unowned** plot may be the live target of **at most one** agent: among all agents currently targeting
     unowned plot `N`, the **winner** is the one minimizing `(travel_cost = manhattan(agent, N), agent_id)`;
     every other agent is **rerouted** to its next-nearest *unreserved* unowned (or own) plot per §3.7. The
     reservation set is **recomputed deterministically from positions + tasks + the registry each pass**
     (it is a pure function of serialized state, so it adds **no** new digest surface). After the pass, at
     most one agent can harvest any given unowned plot, and the post-tick `worked` event claims it cleanly.
   - **Claim:** a `worked(agent, N, moved)` event (§3.4) on an **unowned** eligible plot sets `owner =
     Some(agent)` (homesteading — money-free, first-come by physical arrival, no allocator/quota). When
     `harvest_gate` is off (`non_excludable_deed` control), neither (a) nor (b) blocks/reroutes — the commons
     behaviour, to prove *exclusion* (not title) is what bites.

6. **Forfeiture = TRUE EXIT, not travel/deposit delay (Codex P1.3 + round-2 P1.2).** A far owner legitimately
   spends ticks travelling and depositing, so "idle = no harvest arrival" would wrongly forfeit active distant
   owners and *manufacture* churn. The per-plot idle counter advances only while the owner is **NOT engaged**
   with that plot — but "carry pulled from it still pending deposit" is **not** detectable from carry alone,
   which is keyed by *good*, not *source node* (round-2 P1.2). So add a **per-agent
   `carried_grain_source: Option<NodeId>` marker**: set to `N` on a `worked(agent, N, moved)` event; **kept**
   while that grain remains in carry / pending deposit (across the `GoDeposit` trip back to the exchange);
   **cleared** on the deposit transfer. It is per-agent steering state → serialized **ON-only** under tag 13.
   Then **engaged with plot `N` = any of**: a current `GoHarvest{,WithRoom}(N)` task, OR `carried_grain_source
   == Some(N)` (a deposit trip from `N` is in progress), OR a `worked(_, N, _)` event this tick. Any engagement
   **resets** the counter to 0; the plot reverts only after `land_idle_limit` consecutive **un-engaged** ticks
   — i.e. the owner genuinely stopped cultivating it (a real exit to buying), never mid-cycle. (`forfeit_on_idle
   = false` → the counter is inert, plot kept while idle: the `no_forfeit` control.)
   `reclaim_reserved_for_prior_owner = true` → on revert the plot is reserved for its prior owner to re-take at
   no spatial cost: the `free_reclaim` control (predicts no stickiness).

7. **Targeting that produces the gradient (Codex P2.1 — deterministic tie-breaks).** Under the gate, the
   "go harvest grain" target is chosen as: (a) the agent's **own** plot if it has one and it isn't exhausted;
   else (b) the **nearest reachable unowned** grain plot, sorted by `(travel_cost = manhattan(agent, plot),
   node_id)` — fully deterministic, no iteration-order ties. Because good plots sit at small `x` near the
   exchange, nearest-unowned naturally hands early/near agents the good land and pushes a lapsed re-entrant
   (whose good plot was taken) to far/poor plots — the re-entry gradient, with no allocator and no foresight.

8. **Inheritance (Codex P2.2 — pinned order):** during `settle_death`, **before** the dead-owner registry
   invariant check, each plot the dead colonist owned transfers to its household heir **iff** the heir is live
   and S22a-eligible; otherwise the plot reverts to unowned. Assert **no plot is owned by a dead agent** after
   every death phase. A conserved registry transfer; the finite plot set is preserved.

9. **Everything else is S22a unchanged** — the hunger-gated cultivate entry/exit is untouched; the only new
   thing is that *holding the means to cultivate well* now depends on tenure. NO fiat "owners must cultivate"
   rule, NO money land-market (S23b), NO `Vocation` mutation. The other S22 exit-cost levers (skill,
   profit-stay, capital, commitment) are OFF in the headline.

10. **Diagnostics (runtime-only):** plots claimed / lost-on-idle / re-claimed-by-other; owner ids + owner share;
   owner ∩ persistent-cohort overlap; per-owner plot quality (`regen`/`cap`) and distance-to-centre;
   **spatial-hysteresis trace** (re-entrant lapsed owners' plot quality/distance vs stayers'); non-owner buyer
   cohort + post-promo bought; count of available unowned (marginal) plots over time; churn vs commons baseline.

## 4. The new suite `sim/tests/private_land_tenure.rs`

- **MANDATORY NON-VACUITY TEST** (else `TenureLeverInert`): plots are actually claimed by homesteading;
  ownership actually gates harvest (a non-owner is denied a held plot); ≥1 plot is lost-on-idle and re-claimed
  by a **different** agent; and the **spatial-hysteresis trace** shows ≥1 lapsed re-entrant getting a
  worse/farther plot than a stayer.
- **The ordered classifier (§2)**, printed `--nocapture`; verdict test prints verdict + deciding metrics, does
  NOT assert SUCCESS.
- **Scenario:** `frontier_land_tenure` (HEADLINE) = the expanded `ROSTER_HOUSEHOLDS = 8` S22a base (endogenous
  entry on; skill/profit-stay/capital/commitment OFF) + a heterogeneous grain-plot layout (good-near/poor-far,
  good plots < eligible agents < total plots) + `private_land_tenure = true`. Matched baseline = the same with
  `private_land_tenure = false` (commons).
- **Controls (each a test; each flips ONE pinned config field from §3.1 so it's a clean one-variable falsifier):**
  - **property_off** (`private_land_tenure = false`) = the commons baseline (reproduces S22a fluid / no-stickiness;
    the matched churn baseline).
  - **non_excludable_deed** (`harvest_gate = false`): ownership recorded but never blocks harvest — anyone still
    harvests any plot. Must NOT produce stickiness (proves it's *exclusion*, not title bookkeeping).
  - **free_reclaim** (`reclaim_reserved_for_prior_owner = true`): a lapsed owner re-takes its *same* plot at no
    spatial cost. Must NOT produce stickiness (proves it's the *loss + worse re-entry*, not merely owning).
  - **abundant_good_land** (`GOOD_PLOTS = 16`, a concrete high value well above any plausible owner cohort —
    good land no longer scarce; NOT a "≥ eligible agents" comparison): must NOT produce a scarce owner cohort
    (→ `CommonsEquivalent`/no cohort).
  - **no_forfeit** (`forfeit_on_idle = false` — owner keeps plot while idle): isolates loss-on-exit (predict
    weaker/no stickiness — owning without *losing on exit* isn't enough).
- **HARD GUARDS every run + cell:** grain conserves every tick; `bread_minted_max == 0`; provenance
  clean-or-disqualified; `!extinct`; the **plot-registry invariant** (each plot ≤1 owner; at most one agent
  has a live harvest task on any unowned plot after the validation pass; `carried_grain_source` clears on
  deposit and is never stale; claim/abandon/inherit preserve the finite plot set; no plot owned by a dead
  agent); open-entry guard (viable marginal plots available unless classified `HardBarrier`).
- **goldens_unchanged** test pinning the five tripwire digests (copy from `voluntary_cultivation_commitment.rs`).
- **Robustness mini-sweep** over `land_idle_limit` + the gradient steepness (good/poor `regen` ratio) + good-plot
  scarcity, classified, no tuning. The scarcity + idle-limit axes MUST be outcome-driving (too abundant →
  CommonsEquivalent/no cohort; too scarce → HardBarrier/LandMonopolyCull; a middle band → potential success).

## 5. Verification (independent gate)

Redirect cargo to files; never pipe to head/grep (EPIPE → spurious exit 101).
- `cargo test -p sim --test private_land_tenure` passes (non-vacuity + the exclusion/loss/reclaim mechanics +
  the controls).
- `cargo test --workspace` passes; **all existing goldens byte-identical** (`goldens_unchanged` +
  voluntary_cultivation_commitment / endowed_inherited_capital / durable_cultivation_capital /
  profit_driven_retention / occupational_stickiness / endogenous_cultivation_entry / robustness_appendix /
  household_barter / mortality / open_colony_mortality / demand_survival_bridge).
- `cargo fmt --check` clean; `cargo clippy --workspace --all-targets -- -D warnings` clean; conservation holds.

## 6. Honesty and scope (state these in the result; do not let a SUCCESS overclaim)

- **A SUCCESS is the institutional/property claim, not "occupation without institutions."** Honest headline:
  *"a scarce, excludable, losable spatial productive asset can stabilize the cultivator occupation that skill,
  profit, tools, and inherited tools could not — occupation emerges from costly exit through land tenure, not
  from productivity alone."* It joins S22f: occupational persistence needs an institution that makes
  exit/re-entry costly — **a contract (S22f) or scarce private property (S23a)**.
- **Why land can succeed where the S22d/e plow failed** — land is **required, scarce, excludable, losable, and
  spatially differentiated**, where the plow was optional, abundant, kept-when-idle, and uncontested. State
  this contrast explicitly.
- **The controls + open-entry + bounded-minority + surviving-buyer guards are load-bearing** — without them a
  SUCCESS is indistinguishable from a fiat "only owners farm" pin. The classifier downgrades to `HardBarrier`
  / `CommonsEquivalent` / `LandMonopolyCull` if they don't separate.
- **Money land-market deferred to S23b** — S23a's claims are money-free homesteading only; the initial claim
  path MUST stay pre-money so the SALT bootstrap is preserved.
- **Bounded to this WOOD-poor, mortality-on, expanded-roster regime + this grain-plot layout** — like S21h/i,
  expect possible band-qualification; report the idle-limit / gradient / scarcity windows where it holds.
- Follow repo conventions; NEVER add Claude/AI/assistant references in code, comments, or committed text.

## 7. Codex spec-review resolutions (round 1)

- **P1.1 harvest gate vs the real world loop** — §3.5: the gate acts **pre-`world.tick`** (re-validate every
  `GoHarvest{,WithRoom}(grain node)` task against the registry and cancel/re-route if the plot is owned-by-other
  or reverted), so a blind `World::apply_arrival` can never harvest a plot the agent doesn't own.
- **P1.2 concrete claim event source** — §3.4: snapshot each agent's `(task, carry)` before each fast tick;
  after the tick, a `GoHarvest` task + carry increase = a `worked(agent, node, moved)` event — the single
  deterministic source for claim + idle-reset.
- **P1.3 idle forfeiture = true exit, not travel delay** — §3.6: the idle counter advances only while the owner
  is **not engaged** (no task targeting it, no pending carry from it, no `worked` event); any engagement resets
  it; reverts only after `land_idle_limit` consecutive un-engaged ticks. Prevents the gradient manufacturing churn.
- **P1.4 predeclared layout/constants** — §3.2: exact 1-D layout (64-wide, exchange at 0), GOOD_PLOTS=4 at
  x∈{2..5} (regen 64/cap 8000), MARGINAL_PLOTS=8 at x∈{12..54} (regen 12/cap 1000), VIABLE floors, and the
  predeclared sweep bands for idle-limit / gradient / scarcity.
- **P1.5 viable marginal land measurable** — §2a: `VIABLE_MARGINAL` = unowned + reachable + regen/cap ≥ floors;
  open entry requires ≥1 viable marginal plot through the final window AND ≥1 observed non-owner claim+production;
  `HardBarrier` is its negation.
- **P1.6 control config surface** — §3.1: explicit gated fields `harvest_gate` / `forfeit_on_idle` /
  `reclaim_reserved_for_prior_owner` (+ GOOD_PLOTS for abundance), all serialized ON-only under tag 13; §4 binds
  each control to exactly one field.
- **P2.1 deterministic targeting tie-breaks** — §3.7: own plot first, else nearest reachable unowned by
  `(manhattan, node_id)`.
- **P2.2 inheritance order** — §3.8: during `settle_death`, before the dead-owner invariant check; household heir
  if live+eligible else unowned; assert no dead-owner plots after each death phase.
- **P2.3 HardBarrier vs LandMonopolyCull** — §2: `HardBarrier` = no viable marginal entry (entry impossible);
  `LandMonopolyCull` = entry open but owner grain concentration ≥ MONO_SHARE culls buyers; classifier reordered
  HardBarrier → LandMonopolyCull.

### Round 2 (3 P1 + 1 P2)

- **P1.1 unowned-plot stampede race** — §3.5(b): a deterministic single-targeter reservation in the
  pre-`world.tick` pass — at most one agent may target any unowned plot (winner by `(manhattan, agent_id)`,
  others reroute); recomputed each pass from positions+tasks+registry (no new digest surface).
- **P1.2 carry source tracking** — §3.6: a per-agent `carried_grain_source: Option<NodeId>` (set on `worked`,
  kept through the deposit trip, cleared on deposit), serialized ON-only; "engaged" now includes
  `carried_grain_source == Some(N)` so a deposit trip is engagement and distant active owners never forfeit.
- **P1.3 plot-count invariant wrong for the expanded roster** — §3.2: dropped the `< eligible cultivators`
  inequality; reframed as good-land contested (`GOOD_PLOTS` ≈ `PERSIST_COHORT`) + open entry
  (`MARGINAL_PLOTS` ≫ simultaneous cultivators); `abundant_good_land` control = concrete `GOOD_PLOTS = 16`.
- **P2 registry/digest surface** — §3.1 + §4: the persistent steering state (plot registry + per-agent
  `carried_grain_source`) is serialized ON-only and added to the invariant; the reservation stays recomputed.
