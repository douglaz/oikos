# impl-39 — S23a: Private Land Tenure (does scarce, excludable, losable *place* finally stabilize an occupation?)

Status (spec): DRAFT — pending Codex spec-review. Base: master `496566e` (S22f landed + the article at the
arc's turn). Opens the **S23 private-property arc**. Composes on S22a (`endogenous_cultivation_entry`) on the
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
- `LandMonopolyCull` — top plots feed a tiny owner elite (owner grain share ≥ `MONO_SHARE` = 0.75) AND the
  non-owner buyer cohort collapses (buyers die / post-promo bought < floor).
- `HardBarrier` — too few viable plots: non-owners cannot enter at all (no marginal land available, owner
  share → universal, or non-owner entry attempts all fail). A pin by scarcity, not an open institution.
- `CommonsEquivalent` — the controls show title is inert (ownership doesn't change harvest/behaviour); behaves
  like `property_off`.
- `MoneyFailureFromTenure` — tenure disrupts the pre-money barter bootstrap; SALT fails to promote / demonetizes.
- `NoStickinessDespiteLand` — tenure bites (plots claimed, lost, re-claimed worse) but churn persists > the
  bar AND no persistent owner cohort.
- `LandTenureStickySuccess` — all ten success clauses, not downgraded.

**Ordered classifier (top-down, first-match-wins — S21i non-gameability discipline):**
`TenureLeverInert` → `ConservationBroken`/`extinct` → `LandMonopolyCull` → `HardBarrier` →
`MoneyFailureFromTenure` → `CommonsEquivalent` → **then the explicit final gate:** `if ALL TEN success clauses
(§2.1–§2.10) pass { LandTenureStickySuccess } else { NoStickinessDespiteLand }`. Predeclare every threshold as
a `const`; do NOT fit.

## 3. Engine design (additive, default-off, conservation-safe)

1. **NEW default-off flag** `ChainConfig::private_land_tenure: bool` + fields: `land_idle_limit: u16` (ticks an
   owner may go without working a plot before it reverts), and the headline scenario's **grain-plot layout**
   (a set of grain `ResourceNode`s with a quality/distance gradient — good high-`regen`/`cap` plots near the
   centre, poor low ones far; **fewer good plots than eligible agents** so good land is scarce, but **enough
   total plots that marginal land remains available** so entry stays open). Helper
   `private_land_tenure_active(&self)` = flag on AND `endogenous_cultivation_entry_active()`. Canonicalize
   ON-only with the **next free flag-digest tag (13** unless master advanced) + `land_idle_limit` + the layout
   params. Off ⇒ byte-identical.

2. **Plot registry (sim-side, gated — keeps `world` and goldens untouched):** a per-plot ownership map keyed by
   `NodeId` over the grain nodes: `owner: Option<AgentId>` + a per-plot `last_worked_tick` (or idle counter).
   This **steers harvest behaviour**, so it MUST be serialized into `canonical_bytes` **ON-only** under the
   gate (the S22 discipline). Not a good — no conservation term; grain conservation is unchanged.

3. **Claim (homesteading — money-free, open):** when an eligible agent (S22a filter: `Consumer | Gatherer |
   Unassigned`, spatial, alive) reaches an **unowned** grain plot and performs a harvest/cultivation-labor
   tick on it, it **claims** it (registry `owner = Some(id)`, `last_worked_tick = now`). First-come by physical
   arrival — no top-down allocator, no quota. Proximity therefore matters (nearer agents reach good unowned
   plots first).

4. **Harvest gate (excludability):** a grain plot with `owner = Some(other)` may be harvested **only by its
   owner**; a non-owner targeting it is denied (and re-routes to an unowned/own plot). Unowned plots are
   harvestable by anyone (and claiming them is the act of harvesting). When the gate is off, harvest is the
   current commons behaviour exactly.

5. **Abandonment (loss-on-idle — the forfeiture):** if a plot's owner does not work it for `land_idle_limit`
   ticks, it **reverts to unowned** (registry `owner = None`) and becomes claimable by the next agent who
   reaches it. This is what makes *leaving* cost your place. (Brief satiation must not forfeit — `land_idle_limit`
   is long enough that normal short non-cultivating stretches keep the plot, short enough that a genuine exit
   to buying loses it; sweep it.)

6. **Inheritance:** on the owner's death, the plot transfers to the household heir if one is eligible (reuse
   the estate-routing seam), else reverts to unowned/commons. A conserved registry transfer; the plot set is
   finite and preserved.

7. **Targeting that produces the gradient:** the existing "go harvest grain" task picks a target node; under
   the gate, an agent prefers (a) a plot it owns, else (b) the **nearest reachable unowned** plot. So a lapsed
   farmer whose good central plot was taken finds only far/poor unowned plots — the spatial re-entry penalty
   falls out of nearest-unowned targeting + the quality gradient, with no foresight.

8. **Everything else is S22a unchanged** — the hunger-gated cultivate entry/exit is untouched; the only new
   thing is that *holding the means to cultivate well* now depends on tenure. NO fiat "owners must cultivate"
   rule, NO money land-market (S23b), NO Vocation mutation. The other S22 exit-cost levers (skill, profit-stay,
   capital, commitment) are OFF in the headline.

9. **Diagnostics (runtime-only):** plots claimed / lost-on-idle / re-claimed-by-other; owner ids + owner share;
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
- **Controls (each a test):**
  - **property_off** = the commons baseline (reproduces S22a fluid / no-stickiness).
  - **non_excludable_deed** (ownership recorded but the harvest gate is OFF — anyone still harvests any plot):
    must NOT produce stickiness (proves it's *exclusion*, not title bookkeeping).
  - **free_reclaim** (a lapsed owner can re-take its *same* plot at no spatial cost — e.g. abandoned plots are
    reserved for the prior owner): must NOT produce stickiness (proves it's the *loss + worse re-entry*, not
    merely owning).
  - **abundant_good_land** (good plots ≥ eligible agents — no scarcity): must NOT produce a scarce owner
    cohort.
  - **no_forfeit** (`land_idle_limit` effectively infinite — owner keeps plot while idle): isolates
    loss-on-exit (predict weaker/no stickiness — owning without losing isn't enough).
- **HARD GUARDS every run + cell:** grain conserves every tick; `bread_minted_max == 0`; provenance
  clean-or-disqualified; `!extinct`; the **plot-registry invariant** (each plot ≤1 owner; claim/abandon/inherit
  preserve the finite plot set; no plot owned by a dead agent); open-entry guard (unowned plots available
  unless classified `HardBarrier`).
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
