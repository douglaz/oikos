# impl-50 — C3: Unified Households (does inheritance finally fire on a living economy?)

Status (spec): **DRAFT — pending Codex spec-review.** Layer C3 of the civilization-core roadmap
(`docs/spec-civ-core-roadmap.md`). Composes **C1** (`wage_labor` tag 22) + **S23d**
(`mortal_landowner_demography` tag 20, branch `feat/mortal-landowner-impl-rb` @ `d965d35`) + **S23c**
(`secure_land_tenure` inheritance tag 18, branch `feat/secure-land-tenure-impl-rb` @ `c6c4689`). Flag
`unified_households`, digest **tag 24**, ON-only. **This is the S23c generational-tenure study re-run** —
the whole point of the S23c→d→e strand, now testable because C1 gives buyers income.

Falsifiable bar (headline): with the owner, the consumer, and the reproducer unified into one mortal
lineage household that **earns wages (C1), buys output, and bequeaths (S23c)**, does the S23c
inheritance engine finally **fire on a living market economy** — born-in-sim households persistently own
land/capital, buy through money, and pass estates to heirs who continue the occupation — rather than
staying `DemographyBaseUnviable` (S23d) or `DisqualifiedNoInheritance` (S23c)?

## 0. Dependency & premise (read first)

C3 is the milestone the entire generational strand was deferred *for*. Its premise chain:

- **S23c** built a correct universal-heir inheritance engine (`settle_estate_to_heirs`,
  `transfer_secure_private_land_on_death`, `heir_for`, partible fractional shares) but landed
  `DisqualifiedNoInheritance` — **vacuous** because OIKOS ran two disjoint populations (immortal
  cultivator-owners vs. mortal consumer-lineages); owners ∩ reproducers ≈ ∅.
- **S23d** fixed the demography (mortal reproducing lineage households *are* the persistent
  cultivator-owners; owner-identity invariants `immortal_roster_owned_plot_ticks==0`,
  `non_lineage_owner_plot_ticks==0`, `owner_old_age_deaths>0` all hold), so inheritance **fires** — but
  landed `DemographyBaseUnviable`: **subsidy-bound** (100% own-labor floor, `buyer_bought=0`), because
  there was no way for buyers to earn.
- **C1** adds exactly that missing income loop.

So **C3 = S23d demography + S23c inheritance + C1 wages, composed** — and it asks the strand's terminal
question: *once owners are mortal lineage households that can also earn wages and buy, does secure
heritable private tenure sustain a cross-generational owner-cultivator cohort on a living market?*

**C3 assumes C1 succeeded** (`CircularFlowForms`). If C1 is `WageInertDemandStillDead`, the base is still
subsidy-bound and C3 remains a re-run of S23d's null — the spec would then report that inheritance fires
but the economy is still dead, a sharper restatement, and the strand stays blocked upstream. Provisional
on C1's landed verdict, per the discipline.

## 1. Praxeology — the household without aggregate utility

The household is the primitive locus of consumption and reproduction, but it is **not** an actor and has
**no** aggregate utility. Only its members act, each on its **own ordinal value scale**
(`life/src/scale.rs:regenerate_scale`, per-`Colonist`, `NeedState`-driven). The household is a *name for
a pattern*: the unit that (a) holds property (land/tools/lineage estate), (b) whose members sell factor
services (labor via C1), (c) reproduces (`run_births`), and (d) bequeaths (S23c estate engine). Time
preference is per-member and heritable (`time_preference_base_bps`, mutated at birth). **Anti-smuggling
(the Codex caveat, carried from prior review):** a household "budget" must be **access control over
conserved gold**, never a utility aggregator — members still choose ordinally and bid individually
(`Colonist` id, not household id). C3's headline therefore keeps consumption fully individual; a shared
money pool is an *optional, separately-gated sub-experiment* (§3.3), not the default.

## 2. What already exists (C3 is mostly composition + a rebase)

- **The inheritance engine (S23c, tag 18, branch):** `heir_for` (settlement.rs:12140, first surviving
  household member in slot order), `private_land_heir_eligible` (:12158), `settle_estate_to_heirs`
  (:11954, conserved gold+stock→heir, overflow→commons, bread-ledger split), `transfer_secure_private_land_on_death`
  (:12361, impartible/partible fractional shares), the newborn `parent` field. Conserved end-to-end;
  tag-18 ON-only.
- **The mortal-landowner demography (S23d, tag 20, branch):** mortal reproducing lineage households as
  persistent owners; the owner-identity invariants; staggered founder ages; old-age lifespans (72–120
  ticks); `age_and_remove_elderly` (:12006) firing `settle_death`→estate.
- **Households + births + estate timing (master):** `Settlement::households: Vec<HouseholdRuntime>`
  (:6572, currently only `last_birth_tick`), `run_births` (:12136, endowment conserved), `settle_death`
  (:11566), the econ-tick order (needs+death ~9357, aging+death ~9362, births ~9654).
- **C1 (tag 22):** the wage-labor loop the households plug into as workers and owner-employers.

So C3's new code is small: the **rebase-forward composition** (§2.1), the **unification wiring** (make
the same households own + earn + buy + bequeath), and the tag-24 gate.

### 2.1 The rebase-forward composition (the load-bearing engineering)

S23c (`c6c4689`, parent pre-master) → S23d (`d965d35`, parent `c6c4689`) form a stack that is
**mechanically independent** of intervening work but has **not** been rebased onto current master, and
whose spec conflicts land on the `Settlement` struct + `econ_tick` order (both heavily edited by C1).
Slice A performs, in order: rebase S23c onto the C1 branch, rebase S23d onto that, resolve the
`Settlement`/`econ_tick` conflicts, and verify each reproduces its own branch goldens/verdicts
(`secure_land_tenure` suite, `mortal_landowner_demography` suite) on the composed base. **If S23d does
not reproduce `DemographyBaseUnviable` with wages off, the rebase drifted — a stop-and-fix, not a tune.**

## 3. Mechanism

### 3.1 Unification (owner = worker = consumer = reproducer)

On the composed base, the mortal lineage households (S23d) that own plots and reproduce now also:
- **Earn wages** — household members are hire-eligible non-owners in C1's labor market when they are not
  the owner working their own plot; an owner-household short of labor hires from other households (C1).
- **Buy output** — with wage income (C1 wage-proceeds bucket), through the ordinary goods market.
- **Bequeath** — on member death, `settle_estate_to_heirs` + `transfer_secure_private_land_on_death`
  route gold, tools, and land shares to the household heir (S23c), who continues.

No new decision machinery — this is the S23d base with C1's income channel switched on and S23c's estate
engine composed. The strand's question is answered by *observation*, not a new lever.

### 3.2 Digest tag 24 (ON-only) + landing tags 18/20

`if self.unified_households_active() { out.push(24); out.push(u8::from(flag)); ... }`. Because C3 *lands*
S23c (tag 18) and S23d (tag 20) as its base, those tags become part of the composed digest; with
`unified_households` off (but tags 18/20 on) the run must reproduce the S23c/S23d branch goldens, and
with all of 18/20/24 off, byte-identical to master. Any C3-specific steering state (e.g. the optional
pool, §3.3) is serialized ON-only under tag 24.

### 3.3 Optional shared budget pool (separately gated sub-experiment, NOT headline)

A `household_budget_pool` sub-flag (default off, distinct digest) lets a household hold a **shared gold
pool** that members draw from when their *individual* ordinal bid clears — access control only, never an
aggregate scale. It is tested *separately* to answer "does pooling change the outcome?" and must pass the
same anti-aggregate guard (each member's consumption is still its own `regenerate_scale`; the pool is a
conserved shared holding, `Σ pool = Σ member draws`). The **headline C3 uses individual holdings** so the
generational-tenure result is not confounded by a pooling mechanism.

## 4. Praxeology / anti-smuggling guards

1. **No aggregate utility.** Consumption is per-member ordinal; the household never has a scale. The
   optional pool is access control (conserved), asserted by an anti-aggregate test.
2. **Inheritance is conserved, not minted.** Reuse S23c's `endowed+built−destroyed==total` invariants;
   estate gold+stock+land shares route heir-or-commons with no leak.
3. **Owners are lineage reproducers.** Carry S23d's owner-identity invariants unchanged
   (`immortal_roster_owned_plot_ticks==0`, `non_lineage_owner_plot_ticks==0`, `owner_old_age_deaths>0`)
   so "owner=reproducer" is enforced, not assumed.
4. **Wages from own earnings.** C1's retained-earnings ledger + anti-subsidy controls carry over; an
   owner-household hires from its own sale proceeds.
5. **Base not tuned to pass.** Slice A reproduces S23c's and S23d's *nulls* on the composed base with
   `unified_households`/`wage_labor` off — the only new variable is the composition itself.

## 5. Conservation & determinism

Estate/inheritance transfers are conserved (S23c invariants); the optional pool is a conserved shared
holding; C1's money/escrow invariant carries over. Determinism: inheritance order is deterministic
(`heir_for` slot order), births deterministic (hashed seeds). Tag-24 ON-only; off-path byte-identical.

## 6. Slices

- **Slice A — rebase-forward + precondition.** Rebase S23c→S23d onto the C1 branch, resolve
  `Settlement`/`econ_tick` conflicts, reproduce both branch suites' verdicts on the composed base
  (S23d must still be `DemographyBaseUnviable` with wages off). *DoD: composed base green; both branch
  goldens reproduced; conservation holds.*
- **Slice B — unification.** Switch on C1 income for the lineage households (workers + owner-employers);
  wire estate/inheritance to fire on the now-earning owner-households. *DoD: born-in-sim households own,
  earn wages, buy, and bequeath in one run; inheritance events fire post-money.*
- **Slice C — tag 24 digest + optional pool.** Tag-24 ON-only; the separately-gated
  `household_budget_pool` with its anti-aggregate test. *DoD: goldens byte-identical off; pool conserved
  and non-aggregating.*
- **Slice D — acceptance suite + controls** (§7).

## 7. Acceptance suite (`sim/tests/unified_households.rs`)

`SEEDS=[3,7,11,19,23]`, long horizon (S23d used ~1600 ticks + cold-start).

- **Predeclared thresholds (swept):** `INHERIT_EVENTS_MIN` (from S23d), `OWNER_COHORT_MIN` (persistent
  cross-generational owner lineages), `BUYER_BOUGHT_MIN` (final-window wage-financed purchases),
  generations-min.
- **Ordered verdict enum:** base-precondition (composed base didn't reproduce S23c/S23d verdicts →
  `BaseUnviable`; conservation/anti-aggregate broke) → outcome:
  `GenerationalTenureLives` (inherit-eligible owner deaths ≥ N **and** `final_buyer_bought > 0` **and** a
  cross-generational owner-cultivator cohort persists ≥ the window) / `StillSubsidyBound` (inheritance
  fires but `buyer_bought≈0` — C1 didn't rescue the demand side, i.e. C1 was `WageInertDemandStillDead`)
  / `InheritanceStillVacuous` (owners still don't reproduce/inherit — the S23c disjoint-population wall
  somehow persists).
- **Mandatory non-vacuity:** real heir transfers fire post-money (plots + gold + tools to living heirs);
  owner-households both earn wages and buy output; a cross-generational lineage owns across ≥2 deaths.
- **Controls:** `unified_households_off` (reproduces S23d subsidy-bound null); `wage_labor_off` on the
  composed base (isolates that wages are what changed); `household_budget_pool` variant reported
  separately; an anti-aggregate test (pooling doesn't cardinalize a member's scale).
- **`goldens_unchanged()`:** with `unified_households` off, byte-identical to the composed-base goldens;
  with tags 18/20/24 all off, byte-identical to master.

Build/verify: `cargo test -p sim --test unified_households -- --nocapture`, `cargo test --lib`, fmt,
clippy `-D warnings`, workspace green; plus the S23c/S23d suites green on the composed base.

## 8. Risks & open questions

1. **C1-outcome dependency (top).** `StillSubsidyBound` is the expected verdict if C1 failed — C3 cannot
   rescue a dead demand side; it only tests inheritance-on-a-living-economy once C1 lives.
2. **Rebase drift (top engineering risk).** S23c/S23d predate heavy C1 edits to `Settlement`/`econ_tick`;
   the rebase is the largest task and Slice A gates on reproducing their verdicts, not just compiling.
3. **The disjoint-population wall must stay closed.** S23d's owner-identity invariants are the guard;
   C3 must not let the immortal roster creep back as owners once C1 adds worker roles.
4. **Lifespan vs. surplus accumulation.** S23d noted adult lifespan (~27 ticks) may be too short to
   accumulate surplus; with C1 wages the accumulation channel changes — watch whether owner-households
   can now build an estate worth inheriting, or whether the horizon is still too short (a scoped finding).
5. **Pool confound.** Kept out of the headline precisely so it can't confound the generational result.

## 9. Falsifiable-bar summary

Composing the S23d mortal-landowner demography + the S23c universal-heir engine + C1 wage income into one
class of mortal lineage households that own, earn, buy, and bequeath should — *if C1 lives* — finally let
the S23c inheritance engine **fire on a living market economy**: born-in-sim owner-households persist
across generations, buy through wage income, and pass estates to heirs who keep cultivating
(`GenerationalTenureLives`). The honest alternatives are `StillSubsidyBound` (C1 didn't rescue demand, so
inheritance fires into a dead market — the strand stays blocked upstream) or `InheritanceStillVacuous`
(the disjoint-population wall reappears) — each a first-class finding that closes or re-scopes the
S23c→d→e strand.
