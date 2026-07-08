# impl-62 — C3R.a: Mortal Chain-Producers, No Succession (does the production chain survive producer death — or self-repair, or collapse?)

Status (spec): **SPEC-READY** (Codex xhigh, 4 rounds: R1 caught the immortal-replacement reservoir → mortal-only
formation gate + isolation; R2 the additive-hearth double-provision + thin-pool confound; R3 the dedicated-households
over-correction → thick-pool scaffold; R4 confirmed the v4 lifespan-only + existing-lineage-refill design sound, with
only three stale-text lines to fix — now fixed. Each round *simplified* toward the minimal experiment). Round 3 NEEDS-REVISION (no P0; 1 new P1: the
v3 dedicated-producer-households fix over-corrected into a *thick-pool + hearth scaffold* — 6 households ×
cap 8 → up to 48 members). v4 removes the producer households entirely: the seeded producers are
**lifespan-only mortals** (`household: None`, cushion kept — no hearth, no double-provision, no digest
change), and the refill pool is the **existing ≤16 mortal lineage** via the mortal-only formation gate,
reported as `mortal_builder_adopter_pool`. Simpler *and* sharper (tests open-profession refill vs the
payback horizon). §1.5 inheritance misstatement corrected. Round 2 NEEDS-REVISION (no P0; 3 P1s folded:
cushion contradiction; refill-pool confound; gating both build paths). Round 1 NEEDS-REVISION (no P0; the P1s all
turned on one thing: the naïve "attach lifespans to frontier_capital's producers" leaves a *large immortal
builder/adopter reservoir* (~68 agents) that would fake self-repair, and household membership couples to an
*additive* hearth that double-provisions / would swamp at roster scale). v2 (informed by probe a231fe61)
re-grounds the mechanism: mortalize only the seeded producers into the lineage (cushion dropped), **gate
producer formation to mortal-only** to close the reservoir, and **isolate** producer mortality against a
fixed immortal surround (gatherers/consumers) — the correct one-variable first slice. The first slice of
C3R (the program's keystone: a mortal production chain), scoped per the research maps to its minimal
falsifiable unit. Build base:
branch **`feat/succession-impl-rb` @ `325a24d`** (the decomposed C1..C1S stack). Flag
**`mortal_chain_producers`** (bool on `ChainConfig`), gated `mortal_chain_producers_active() = flag &&
demography.is_some() && chain.is_some()`. Digest **tag 27** (code-verified free) — **flag byte only**:
producer `household`/`age`/`lifespan` are already in the per-colonist digest (gated on
`demography.is_some()`), so making them mortal changes *values*, not layout; the flag rides tag 27 for the
byte-identical-iff-behaviour-identical contract, and every config with the flag off stays byte-identical
(producers keep `lifespan: None`).

Falsifiable bar (headline): the whole program has run its production chain (grain→flour→bread, Miller/Baker
+ the S7 mill/oven) on **immortal** producers — the wall is `lifespan: None` on the household-less producer
roster (mod.rs:8994), skipped every tick by `age_and_remove_elderly` (mod.rs:13988). C3R.a removes exactly
that: it makes the chain producers mortal lineage members and wires **nothing else** — no role succession,
no capital inheritance. The question: **does the production chain survive its producers dying?** Either it
**collapses** (the milling/baking stages empty as producers die, bread output falls, the measured Capital
era regresses — the null that motivates C3R.b/c), or it **self-repairs** via the *existing* S7
tool-acquisition path (a surviving latent producer or the tool-holding heir re-adopts the vacated role with
no new mechanism — a first-class surprise that would reshape b/c).

## 0. One-paragraph summary

The chain has never had to survive a producer death because producers never die: they are generated
non-lineage with `lifespan: None`, and old-age removal early-returns on colonists without a lifespan. C3R.a
attaches the demography's own mortality (a drawn lifespan, `household: None`, as a lifespan-only mortal)
to the chain's Miller/Baker producers on a `frontier_capital`-derived base — the one base that already runs
the full chain, the S7 producible mill/oven, *and* demography lineages side by side. It wires no succession
and no inheritance: when a producer dies, the existing death seam runs unchanged (estate to the household
heir if one exists, else to the commons), and the *vacated role* is filled only if the **existing**
`run_role_choice` path chooses to fill it (a tool-holding colonist re-adopting under the S7 gate). So C3R.a
is a clean, minimal test of a single question — is producer mortality *by itself* survivable by the chain,
using only mechanisms already in the engine? The answer is genuinely open: the S7 re-adopt path
(`recurring_motive` keeps a producer producing while the spread pays, and a tool-holder is re-admitted to
adoption each tick) *might* spontaneously repair the chain, or the roles/capital might drain away faster
than they are refilled and the chain collapse. Both are pre-named, first-class findings; the collapse
motivates the succession slices (C3R.b role, C3R.c capital), the self-repair reshapes them. No smuggling:
C3R.a adds mortality and observes; it does not add any new "fill the role" or "inherit the tool" mechanism.

## 1. Base facts (verified across the two research maps)

1. **The wall.** Chain producers are generated with `lifespan: None, household: None` (mod.rs:8994, the
   non-lineage roster path 8852–9015; vocation by id-band 8868–8964 including Miller 8899 / Baker 8906 /
   latent Mill·Bake 8911–8924). `age_and_remove_elderly` skips any colonist with `lifespan: None`
   (mod.rs:13988). Mortality ⟺ being a lineage member with `lifespan: Some(..)` + a household — currently
   only the G4b Consumers (founders mod.rs:9056/9095/9098; newborns 14293–14328).
2. **The base exists to derive from.** `frontier_capital()` (mod.rs:3917) runs the full chain + S7
   producible capital (`producible_capital=true`, `tool_acquisition_eligibility=true`,
   `capital_payback_cycles=16`, `tool_build_wood=6`) **and** demography lineages (`demography=Some`), with
   designated GOLD + `recurring_motive=true`. It is what `sim/tests/producible_capital.rs` runs. The
   demographic machinery (aging, old-age death, estate, births) is already active on it — it is simply not
   attached to the producer colonists.
3. **Producers avoid starvation by a cushion, not an exemption** (`producer_subsistence`, seeded
   `miller_grain_buffer`/`baker_flour_buffer` = 16, `bread_buffer`) — so `update_needs_and_remove_dead`
   (mod.rs:12904) already advances need for all colonists and could starve a producer. **Under the flag the
   mortal producers KEEP `producer_subsistence`** — they are lifespan-only mortals with `household: None`
   (§3.1.1), so they are not hearth-fed and there is no double-provision to avoid; their feed path is
   unchanged from the immortal producers'. Starvation exposure of producers is a *later* variable — old-age
   death is this slice's mortality under test.
4. **The vacated role is filled only by the existing S7 path.** `run_role_choice` (mod.rs:16431) admits a
   colonist that holds a mill/oven and `latent == None` to the adopt appraisal (16478–16487) and adopts
   Miller/Baker if `recipe_adoption_pays_for_money` clears; `recurring_motive` keeps a role while the
   recipe is profitable (mod.rs:16536–16556). This is the ONLY re-fill path in C3R.a — no death→role wiring
   is added.
5. **A dead producer's estate today.** `settle_death`→`settle_estate_to_heirs` (demography) credits the
   whole stock (incl. the mill/oven good) to `heir_for(id)` — and `heir_for` (mod.rs:13359) needs a
   `household`, so a C3R.a producer (`household: None`) is **heirless → its mill routes to commons**
   (`settle_estate_to_commons`). (`private_land_heir_eligible` excluding Miller/Baker, mod.rs:13377, is
   *land*-specific and does not gate stock inheritance — but it is moot here since the producer has no
   household.) An in-flight tool build whose builder dies is dropped, its WOOD forfeited (mod.rs:16785). No
   producer-role transfer exists. C3R.a **changes none of this** — it only makes producers eligible to die;
   the mill-to-commons + no-role-transfer is exactly the collapse pressure C3R.b/c later relieve.
6. **Digest.** Per-colonist `household`/`age`/`lifespan`/`seed` are already serialized when
   `demography.is_some()` (mod.rs:23434, 23765–23795) — producers currently emit `household=None→0,
   age=0, lifespan=None→0`. So C3R.a needs no new digest layout; tag 27 (free) carries only the flag byte.

## 2. The central question and pre-named outcomes

**Central question.** On a `frontier_capital`-derived base with the chain producers made mortal
(lifespan-only, `household: None`) and **no** succession or inheritance wired, across `SEEDS=[3,7,11,19,23]`: does the
production chain **survive** its producers dying — sustaining a staffed milling and baking stage and its
measured Capital era across producer deaths using only the existing S7 re-adopt path — or does it
**collapse**?

**Ordered verdict enum** (first-match):

```
Preconditions (disqualifying):
  BaseUnviable        — the mortal-producer base fails to run the chain at all even before deaths bite
                        (the flag-off frontier_capital control must climb to the Capital era; producers
                        must actually reach mortal age and die — mortal_producer_old_age_deaths > 0)
  ReservoirOpen       — any producer with lifespan: None exists (immortal_producer_count > 0): the
                        mortal-only formation gate failed and a self-repair could be an immortal artifact
  ConservationBroken  — goods / money / provenance / estate conservation failed a tick
  RegistryBroken      — colonist/household/estate invariants violated
Outcome ladder:
  ChainCollapsesOnProducerDeath — after the first producer die-off the milling/baking stage empties
                        (living Miller/Baker → 0 for a sustained window), bread output falls toward the
                        no-chain floor, and the measured era regresses out of Capital/Specialist. The
                        motivating null: producer mortality is NOT survivable without succession.
  ChainSelfRepairsWithoutSuccession — the existing S7 re-adopt path refills vacated roles as fast as
                        producers die (a tool-holder/heir re-adopts), both stages stay staffed and the era
                        does not regress, with NO new mechanism. The surprise: mortality is survivable on
                        the engine as-is (and C3R.b/c are reframed as robustness, not necessity).
  ChainRunsMortalAndBuilds — self-repairs AND a mortal producer still builds NEW capital (a fresh mill/oven
                        appraised and completed by a mortal builder) — the strongest positive, directly
                        probing the payback-horizon tension.
```

Every rung is first-class. Collapse is the expected result and motivates the succession slices;
self-repair is the informative surprise; the builds-under-mortality rung directly touches the
payback-horizon question C3R.c is built to answer.

## 3. Mechanism — isolate producer mortality, close the reservoir

The research probe (a231fe61) established three load-bearing facts that reshape the mechanism from the v1
"just attach lifespans" naïveté: **(i)** old-age death is *lifespan-only* gated (no household needed —
`age_and_remove_elderly` iterates all colonists and skips only `lifespan: None`, mod.rs:13988); **(ii)**
the hearth `deliver_demography_provisions` is **additive** (a fixed ration/tick, not top-up, mod.rs:14119),
so hearth-feeding the whole 74-agent roster would mint a huge food scaffold, and a lifespan-only roster
with *no* households simply **depopulates** (no births) — neither is a steady-state mortal chain; **(iii)**
the S7 builder/adopter pools have **no lifespan filter** (mod.rs:16974, 16478), so ~68 immortal
Consumers/Gatherers could build a mill and adopt Miller/Baker — a **large immortal-replacement reservoir**
that would fake "self-repair." C3R.a therefore does not mortalize the whole colony; it **isolates producer
mortality as the single variable and closes the reservoir at the formation gate.**

### 3.1 The two changes (both flag-gated)
When `mortal_chain_producers_active()`:
1. **The seeded producers become mortal by LIFESPAN ONLY** (no new households — Codex round-3 P1). The 6
   seeded latent Mill/Bake producers each get a `lifespan = demo.lifespan_ticks(seed)` + a real
   per-colonist `seed` + a staggered `age`, keeping `household: None`. Old-age removal is lifespan-only
   gated (mod.rs:13988 — no household required, probe fact i), so they die of old age; a heirless producer's
   mill routes to commons via the existing estate seam (unchanged). Because they are **not** household
   members they are **not** hearth-fed, so they **keep `producer_subsistence`** (no double-provision — the
   earlier "drop the cushion" was only forced by the rejected producer-household design). This is the
   minimal mortality change and touches neither the demography household set/digest nor the hearth scale.
   *Rejected (round 2→3): giving producers dedicated households — 6 households × `max_household_size=8` →
   up to 48 hearth-fed members, a thick-pool + hearth-scaffold confound symmetric to the thin-pool bug.*
2. **Producer *formation* is gated to mortal agents only — on BOTH build paths.** Under the flag, the
   builder gate in `run_capital_formation` — *both* the legacy loop (mod.rs:16966) **and** the per-agent
   subpath `start_per_agent_builds` (mod.rs:16843, used when `per_agent_capital` is on) — and the adopter
   gate in `run_role_choice` (mod.rs:16463) require `lifespan.is_some()`. This **closes the reservoir**: no
   immortal roster Consumer/Gatherer can build a mill or adopt a producer role, so every producer that ever
   exists is mortal. (The flag gates that subpath explicitly; it is not left silently un-gated.) Off-path
   byte-identical (the gate only narrows the pool when the flag is on).

### 3.2 The refill pool and the isolation (what stays fixed — why it is the right first slice)
The **refill pool is the pre-existing mortal lineage** — the frontier demography's own ≤16 lineage
Consumers (the 2 patient/present-biased households), who under the mortal-only gate (§3.1.2) are the agents
eligible to build a mill and adopt a vacated producer role. No new mortal population is added; this is the
bounded, already-in-the-config pool, reported each run as `mortal_builder_adopter_pool` so a collapse is
attributable (mortality-driven vs a thin-pool artifact, §7). C3R.a thus tests the **open-profession refill
against the payback horizon**: when a mortal producer dies and its mill sinks to commons, will a mortal
lineage member build a *fresh* mill (a 16-cycle payback against an 18–36-tick life) to refill the role? The
immortal roster gatherers (24) and consumers (44) **remain immortal and running** — a stable grain supply +
bread demand — so the **only** varied factor is producer mortality. This is the correct one-variable slice:
it tests "do mortal producers sustain the chain in a stable economy," not "does a fully-mortal colony run a
chain" (the fully-mortal surround, and the *guild* model where a producer's heir inherits role/capital, are
later extensions — the guild model is exactly C3R.b/c). No hearth swamp (no new households), no depopulation
(the immortal surround persists), no reservoir (formation is mortal-only).

### 3.3 What is deliberately NOT wired (falsification integrity)
No role succession (a dead producer's vocation is not transferred to its heir), no mill/oven inheritance
beyond the existing estate route (a dead producer's mill → heir if eligible else commons, unchanged), no
mortality term in the capital-build appraisal. A vacated producer role is refilled **iff** the existing
`run_role_choice` + S7 tool-holding path does so on its own — now necessarily by a **mortal** agent (§3.1.2).
So `ChainSelfRepairsWithoutSuccession`, if it occurs, is genuine mortal rebuild on the existing engine, not
an immortal-replacement artifact and not smuggled succession. Bundling any fill/inherit mechanism into
C3R.a would hide whether the chain would have collapsed — defeating the falsification.

## 4. Anti-smuggling guards
1. **Add mortality + close the reservoir, observe — add no repair.** C3R.a introduces two capabilities
   (producers can die of old age; producer formation is mortal-only) and zero new role/capital mechanisms.
   Self-repair, if it happens, is the *existing* `run_role_choice`/S7 engine, necessarily by a mortal agent.
2. **The reservoir is provably closed** — `immortal_producer_count == 0` is a hard guard (no producer with
   `lifespan: None` ever exists under the flag), so a self-repair verdict cannot be an immortal-replacement
   artifact.
3. **The flag-off control is the immortal chain** — the same base with `mortal_chain_producers=false` must
   climb to the Capital era (BaseUnviable guards a broken base), isolating producer mortality as the only
   varied factor against the fixed immortal surround.
4. **Deaths must actually bite** — `mortal_producer_old_age_deaths > 0` (not a generic death count) is
   required for any non-precondition verdict (else the "survival" is vacuous).
5. **Not tuned** — lifespans are the demography's own drawn distribution (no new free parameter); the
   collapse/self-repair outcome is reported, not searched. `ChainCollapsesOnProducerDeath` is first-class.

## 5. Conservation & determinism
No new goods flows: producer death routes through the existing estate seam (heirless producer → commons),
which already conserves; nothing is minted or dropped (the cushion is kept, no households are added).
Integer, deterministic (lifespan = the existing pure `demo.lifespan_ticks(seed)`; iteration AgentId-ordered).
**Digest:** tag 27 = ON-only `{ push(27); push(u8::from(mortal_chain_producers)) }`; producer mortality
state (age/lifespan/seed; household stays `None`) rides the existing per-colonist digest block (already
serialized when `demography.is_some()`), so the flag changes *values* on the mortal-producer scenario only,
not layout — flag-off `frontier`/`frontier_capital` stay byte-identical (producers keep `lifespan: None`).
All telemetry (`mortal_producer_old_age_deaths`, `immortal_producer_count`, `living_millers`/`living_bakers`
over the final window, `role_readoptions`, `mortal_capital_builds`, `mortal_builder_adopter_pool` — the
count of live mortal agents *eligible* to build/adopt a producer role, the population-artifact guard —
`era_final`) is runtime-only, out of `canonical_bytes`. Off-path: byte-identical to the decomposed branch
goldens (and, all civ flags off, to master).

## 6. Slices
- **A — the base + the two flag-gated changes.** `frontier_mortal_producers()` deriving from
  `frontier_capital()`; the `mortal_chain_producers` flag (7-site template); (1) the generation change
  giving the seeded producers a `lifespan` (lifespan-only; `household: None`, cushion kept); (2) the
  mortal-only gate on both build paths (`run_capital_formation` legacy loop + `start_per_agent_builds`
  subpath) and `run_role_choice`; tag 27. *DoD: flag-off byte-identical (goldens_unchanged incl. frontier &
  frontier_capital); tag-27 split test; flag-on, `mortal_producer_old_age_deaths > 0` and
  `immortal_producer_count == 0`; the flag-off control climbs to the Capital era.*
- **B — the observation harness + telemetry.** `mortal_producer_old_age_deaths`, `immortal_producer_count`
  (the reservoir-closed guard), per-tick/final-window `living_millers`/`living_bakers`, bread output, the
  measured era (reuse `EraDetector`), `role_readoptions` (mortal S7 re-adopts after a producer death),
  `mortal_capital_builds` (a mortal builder completing a fresh mill/oven). *DoD: the collapse-vs-self-repair
  signal is measurable and deterministic; a mortal-built tool is distinguishable from a pre-seeded one.*
- **C — the suite.** `sim/tests/mortal_producers.rs`: the `MortalProducers` cell + the flag-off immortal
  control + the §2 verdict ladder; `SEEDS=[3,7,11,19,23]`. *DoD: suite green; the control reproduces the
  immortal-chain Capital era; verdicts printed, never asserted.*

## 7. Acceptance suite (`sim/tests/mortal_producers.rs`, new)
- **Predeclared thresholds (swept where load-bearing):** the final-window `living_producer` floor that
  separates collapse from staffed, the era-regression window. No tuned lifespan (demography's own draw).
- **Cells:** `MortalProducers` (flag on, `frontier_mortal_producers`); the flag-off immortal control (must
  climb to Capital); a starvation-exposed producer variant deferred (old-age mortality is this slice's
  variable).
- **Classifier, NOT asserted:** `mortal_producer_old_age_deaths == 0` → `BaseUnviable`;
  `immortal_producer_count > 0` → `ReservoirOpen`; sustained `living_producers → 0` after deaths + era
  regression → `ChainCollapsesOnProducerDeath`; staffed stages + era held → 
  `ChainSelfRepairsWithoutSuccession`; + a `mortal_capital_builds > 0` fresh tool → `ChainRunsMortalAndBuilds`.
  **Population-artifact disclosure:** a `ChainCollapsesOnProducerDeath` is reported with
  `mortal_builder_adopter_pool` — a collapse while the pool is non-trivial is mortality-driven (the finding
  that motivates C3R.b/c); a collapse while the pool is ~empty is flagged as a population-cap artifact
  (`CollapseFromThinMortalPool`), a signal to disclose the thin pool or bump the existing lineage size as a
  pinned parameter — NOT to add a producer-lineage that would scaffold the result — not a clean mortality result.
- **Hard guards (invariants only):** conservation, money invariant, colonist/household/estate registry,
  the reservoir-closed guard (`immortal_producer_count == 0`), the flag-off control climbing to Capital
  (base viability), `mortal_producer_old_age_deaths > 0` (non-vacuity).
- **`goldens_unchanged` + the tag-27 canonical-split test** (frontier & frontier_capital byte-identical off).

Build/verify: `cargo test -p sim --test mortal_producers -- --nocapture`, full workspace, fmt, clippy
`-D warnings`; the g5b_frontier + producible_capital + g4b_demography + share/wage suites stay green; every
prior digest unchanged.

## 8. Risks & open questions
1. **Self-repair may be partial/seed-dependent** — some seeds collapse, some self-repair (like C1N's
   φ-dependence). The verdict is then reported per-seed; a mixed result is honest and still motivates b/c
   for the collapsing seeds.
2. **The immortal surround is an isolation, not a limitation to hide** (§3.2) — C3R.a tests mortal
   producers in a stable (immortal gatherer/consumer) economy, the correct one-variable slice; a
   fully-mortal-surround economy is a disclosed later extension. The refill pool is the *existing* ≤16
   mortal lineage (no new households), reported as `mortal_builder_adopter_pool` so a collapse is
   attributable to mortality vs a thin pool. If that pool proves too thin to be conclusive, the honest
   response is to disclose it (or bump the existing lineage size as a pinned parameter), NOT to add a large
   producer-lineage that would scaffold the result.
3. **The payback horizon may suppress *new* builds under mortality even if existing tools keep running** —
   that is exactly the `ChainRunsMortalAndBuilds`-vs-not distinction, and it foreshadows C3R.c; C3R.a only
   *observes* it, does not fix it.
4. **Era regression as the collapse signal** relies on `EraDetector` being read-only and already validated
   (g6a) — reused, not modified.

## 9. Falsifiable-bar summary
Removing the one line that makes chain producers immortal — and wiring nothing else — asks whether the
production chain can survive its own producers dying on the engine as it stands. The milestone shows
whether it **collapses** (`ChainCollapsesOnProducerDeath` — mortality is not survivable without succession,
motivating C3R.b role succession and C3R.c capital inheritance) or **self-repairs** via the existing S7
re-adopt path (`ChainSelfRepairsWithoutSuccession`, or the stronger `ChainRunsMortalAndBuilds` if a mortal
producer also builds fresh capital against the 16-cycle payback) — each named before the run, each
first-class, and each decisive for how the rest of the keystone is built.
