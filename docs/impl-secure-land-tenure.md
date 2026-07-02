# impl-45 — S23c: Secure private land tenure (was it *insecurity*, not private property, that broke S23a?)

Status: **LANDED (rb-lite) — honest NULL/DISQUALIFIED on the generational headline; engine verified by
probes.** The universal-heir order (age-desc child → same-household kin → household successor → colony
next-of-kin → unowned) and the partible fractional-share split (capacity + remaining availability
conserved; sub-floor shares stranded-and-logged; no node subdivision) are implemented default-off
(digest tag 18, goldens byte-identical off) and verified by three forced-mortality probes (impartible
one-heir, partible conservation+stranding, and a positive age-descending-then-id heir-order
assertion). **But on the pinned same-as-S23a base death→inheritance among owners is structurally
unreachable:** land is claimed by the immortal standalone cultivator roster (`lifespan == None`,
`household == None`), while the mortal lineage founders are hearth-fed consumers that never
cultivate/claim — so `inherit_win == 0` in every secure acceptance run, and those runs are
**DISQUALIFIED (`DisqualifiedNoInheritance`), not classified** (the §5 honesty guard: a frozen
immortal registry is not evidence about heritable tenure). The security-contrast controls are honest:
`StillThrashes` at `idle_forfeiture_horizon = 12`, `UniversalOwnershipNoBuyers` at density 48/96
(open homesteading absorbs everyone). The generational concentration/fragmentation question the
milestone leads with cannot be answered on this base without changing the demography — which the
pinned "ONLY security changes" isolation forbids and the honesty section forbids manufacturing (see
`.rb-lite/runs/s23c/challenges-round-2.md`). Spec below unchanged.

Status (spec): SPEC-READY (Codex spec-review: NEEDS-REVISION with a 6-item punch-list, all folded in — numeric SecureTenureStableClass/TenureInertStaticPin classifier, predeclared plot-density band {24,48}, same-base-as-S23a primary + S22f robustness-only, deterministic universal-heir order + conservation invariants, partible = fractional capacity shares on the same node (no subdivision), numeric finding-mode thresholds; bar-reframe confirmed legitimate). User decisions LOCKED: universal heirs + both `inheritance_regime ∈ {impartible, partible}` in one milestone. Base: master `855be95`. **Re-opens the S23 private-property arc**
with the honest counterfactual the S23a critique demands. Composes on the S23a machinery
(`private_land_tenure`: plot registry, homesteading claim, owner-only harvest, inheritance, the population-scaled
spatial layout) — changing exactly the thing whose realism was in doubt: **land tenure becomes SECURE.**

## 0. Why this milestone — the S23a realism critique

S23a modeled private land as **use-it-or-lose-it**: an owner who stopped cultivating a plot for `land_idle_limit`
(default **12**) consecutive un-worked ticks **forfeited** it, and it was immediately re-homesteadable by
**anyone** (default: not reserved for the prior owner), with the lapsed owner mechanically pushed to worse,
farther land. It **thrashed** — rapid claim → lose → reclaim, churn ~10× the commons, owner share 0.75–0.80,
no stable cohort, buyer tier collapsed. **But that forfeiture rule is not how real private property works.**
Fee-simple ownership is *not* use-it-or-lose-it: you keep land whether or not you farm it; adverse possession
takes *years* and requires someone openly occupying it. S23a's rule is closer to *customary/usufruct* tenure
("you hold land while you work it") with an aggressively short clock, and the ~10× churn is plausibly an
**artifact of that short clock + open reclaim by others**, not an inherent property of private land. Three
things combined to manufacture the thrash: (i) a short idle-forfeiture horizon, (ii) reclaim open to *others*
the instant a plot lapsed, (iii) an *immediate* spatial penalty (a nearer agent takes your good plot). S23a's
own `no_forfeit`/`free_reclaim` controls hinted at this (they removed the churn engine) but were reported only
as classifier-inert controls, never as the headline. **S23c makes secure tenure the headline** and asks the
counterfactual directly.

## 1. The central question

**Was it *insecurity*, not *private property*, that broke S23a?** When land tenure is **secure** — an owner
**keeps** their plot whether or not they cultivate it (no idle-forfeiture), it is theirs to harvest, and it is
inherited — does private land, on the *same* population-scaled base where S23a's insecure forfeiture thrashed,
instead **stabilize a bounded owner-cultivator class with a surviving non-owner buyer tier** (the working
two-tier economy the forfeiture-churn destroyed)? Or does secure tenure just go **static/inert** (a fixed set
of owners, no economic dynamics) — Codex's standing warning that "a never-lose title with no exercised exit
cost risks a static pin"?

The value: S23c is the **clean isolation of the tenure-*security* variable**. S23a (insecure) → thrash. If
S23c (secure) → a stable owner-cultivator class + surviving buyers, then S23a's collapse was the *forfeiture
rule's* fault (the realism critique is confirmed), and *secure private property* is what a division of labor in
land looks like. If S23c → inert/static or still no stable class, that is the honest finding that private land,
even when secure, does not by itself produce the two-tier economy (and the contrast with S22f's contract
stands).

## 2. The design (headline + a realism spectrum) — DECISIONS PINNED

**Headline `secure_land_tenure` (default off):** the S23a tenure, with `forfeit_on_idle = false` promoted to
the headline (secure title: own it forever, cultivate or not), everything else S23a (homesteading claim,
`harvest_gate` on = owner-only harvest, inheritance, the population-scaled layout, the spatial targeting
gradient). Same base and seeds as S23a `{3,7,11,19,23}`, same land-capacity axis (total plots {12,24,48,96}),
so it is a matched counterfactual to S23a's thrash.

**A realism spectrum (a sweep, not separate milestones)** — to place secure tenure against S23a's harsh clock:
`idle_forfeiture_horizon ∈ {∞ (secure, headline), long (e.g. 200 ticks ≈ realistic abandonment), 12 (= S23a)}`,
so the result reads as a *curve* from secure → long-horizon → S23a-harsh, showing exactly where thrash sets in.

**DEATH + INHERITANCE IS THE CENTRAL MECHANISM (not a reused footnote).** Under secure tenure, ownership only
ever turns over through **death → inheritance**, so the demography *is* the engine of the outcome. What OIKOS
already has: old-age + starvation mortality with several generations per ~1600-tick run (`old_age_onset_years`
~30 at `ticks_per_year` 6 ≈ 180 ticks/generation), and S23a's `settle_death` already transfers a dead owner's
plot to a **live, eligible household heir** (else it reverts to unowned). **The load-bearing asymmetry:** heirs
are **lineage-only** — non-lineage colonists (here, the SALT-rich buyers/consumers) die **heirless**, so their
plots **recycle** to the commons, while **lineage** plots stay in the family across generations. So secure +
heritable tenure will tend to **concentrate land in a few long-lived lineages** (a hereditary landed class),
with the frontier's openness set by how fast heirless plots recycle. This makes the sharpest question:
**does secure heritable tenure produce hereditary land CONCENTRATION over generations, and does it choke off or
coexist with the buyer economy?** — a distinctly private-property question S23a never reached (its 12-tick
churn destroyed plots before any generational dynamic could run).

**FERTILITY + THE INHERITANCE REGIME (partible vs impartible) — a first-class axis.** OIKOS already models
the **Malthusian preventive check**: households grow toward `max_household_size` only while every member's
hunger is under a ceiling (food-gated fertility), so "numerous children competing for a fixed food/land base"
is real. But land inheritance is currently **impartible**: `settle_estate_to_heirs` gives a plot to a *single*
household heir, and plots are **atomic** (a ResourceNode with fixed regen/cap — no splitting). Land is *not*
divided among children today. This is the primogeniture branch, and the two regimes give **different, both
historically-real** outcomes that S23c should be able to show via an `inheritance_regime` axis:
- **`impartible`** (primogeniture; runs on existing machinery): one heir keeps the whole plot; the *other*
  children inherit **nothing → a growing LANDLESS class**. Land scarcity manifests as *more landless people*,
  not smaller plots — and the landless younger children are exactly the **buyer/labor tier**, so impartible
  secure tenure could itself *produce* the two-tier economy (landed lineage + expanding landless buyers).
  Fertility (`max_household_size` / birth interval) dials how fast the landless class grows.
- **`partible`** (fragmentation; needs a bounded engine change): a plot's capacity is **partitioned among all
  heirs** (reuse `regen`/`cap` + `LAND_VIABLE_REGEN_FLOOR` — once a share falls below the floor the plot is
  non-viable). Over generations, holdings shrink → *morcellement* → Malthusian collapse (pre-famine Ireland /
  partible-France). Land scarcity manifests as *smaller plots*.
**DECIDED (user): BOTH regimes ship in this milestone** as the `inheritance_regime` axis — `impartible`
(existing atomic plots; landless non-heir children → buyer tier) AND `partible` (plot-capacity partition among
heirs + `LAND_VIABLE_REGEN_FLOOR`; fragmentation → morcellement). The partible branch's bounded plot-divisibility
(capacity split; no new node creation — reduce the inherited plot's effective capacity per co-heir) is in scope.
Track **land-per-capita** across generations for both — the whole point is to watch it fall under fixed land +
growing population.

**REALISM DECISION on inheritance (Codex to rule):** in the model inheritance is **not universal** — only
lineage households have heirs. Real land inheritance is universal. If only lineages retain land, secure
heritable tenure will *mechanically* concentrate land in lineages — potentially an **artifact of the
demographic model**, not an economic result. Two options: **(a) extend inheritance to ALL households** (a
small, realistic engine change: every owner has a determinable heir, so newborns/next-of-kin inherit
regardless of lineage) so concentration is *emergent*, not built-in; or **(b) keep the lineage-only asymmetry
but disclose it prominently** and treat lineage-concentration as a known feature. **DECIDED (user): (a) UNIVERSAL HEIRS** — every owner has a determinable heir (next-of-kin / newborn), so a
plot passes on death regardless of lineage and land concentration is an EMERGENT outcome of a fair rule, not a
built-in lineage asymmetry. Keep it goldens-preserving (gated behind the secure-tenure mode, digested ON-only).

**PINNED DECISIONS (Codex spec-review — all open questions resolved):**
0. **Universal heirs — LOCKED (a).** Every owner has a determinable heir (§2 above); the deterministic heir
   order + conservation invariants are pinned in §4.
1. **Static-pin vs healthy-class — PINNED numerically (§3).** A stable owner set that does NOT cultivate/produce
   /support-buyers is `TenureInertStaticPin` (a *finding*, not success); the healthy two-tier economy is
   `SecureTenureStableClass`. Secure tenure does NOT trivially win — low churn is *necessary but not sufficient*.
2. **Plot-density band — PINNED `{24,48}` primary** (predeclared, chosen before results). `12` = scarcity /
   seed-cluster boundary probe; `96` = expected `UniversalOwnershipNoBuyers`/`HardBarrier` (open homesteading
   absorbs too many into ownership). Success = within `{24,48}`, ≥1 density passes in ≥3/5 seeds, and adjacent
   densities classify honestly.
3. **Base — PINNED same-base-as-S23a as the primary** (the clean isolation: same seeds, same population-scaled
   layout, same machinery, ONLY security changes). "Healthy once forfeiture is off" is testable on this base
   with the §3 metric. **S22f is a labeled robustness appendix/control ONLY, not the headline** — if S23c only
   works on S22f, the finding is "secure land composes with the commitment institution", not "forfeiture broke
   S23a".
4. **Bar reframe — CONFIRMED legitimate (not success-bar shopping), because predeclared exactly as §3.** S23a
   asked "do tenure rules reduce churn/stickiness"; S23c asks "does secure heritable exclusion create a
   functioning owner-cultivator + buyer economy" — a different question. **Honesty guard: still report S23a's
   old churn/drop metrics, as NON-success metrics** (low churn necessary for secure title, not sufficient for
   success).

## 3. Success / finding modes (PINNED — Codex spec-review)

**Evaluation window:** the final 50% of the run, spanning ≥3 completed inheritance generations where possible.
All shares/thresholds are consts; do NOT fit.

**Primary success = `SecureTenureStableClass`** (in ≥3/5 seeds, at ≥1 density in the predeclared `{24,48}`
band):
1. **Bounded owner minority** — `owner_share ∈ [0.10, 0.45]`.
2. **Buyer majority** — `non_owner_share ≥ 0.50`.
3. **Owners actually cultivate** — `≥ 0.60` of owned viable plots cultivated at least once per generation AND
   `≥ 0.50` of owner-households have positive grain production in the window (NOT an inert registry).
4. **Material production** — owner-produced grain `≥ 0.35` of all grain acquired/consumed by non-owners.
5. **Material buyer tier** — non-owner post-money food bought `≥ 0.25` of non-owner food intake, AND non-owner
   survival not collapsing below `0.60` of its mature-window mean.
6. **Stable class** — title churn *excluding death/inheritance* `≤ 0.05` of plots per generation (inheritance
   turnover is allowed + reported separately).
7. **Bounded concentration** — land Gini does not trip the `HereditaryConcentration` collapse threshold (§below).
8. Money promotes; `seeded_minted == 0`; provenance clean; mortality + conservation hold every tick; goldens
   byte-identical off.

**Finding modes (PINNED thresholds):**
- `TenureInertStaticPin` — owner share stable BUT (owner cultivation `< 0.30`) OR (owner grain production
  `< 0.20` of buyer food intake) OR (non-owner bought food `< 0.10`): a static title registry without a working
  economy. (This is why secure tenure can't trivially "win" — reducing churn alone fails.)
- `UniversalOwnershipNoBuyers` — `owner_share ≥ 0.75` OR `non_owner_share < 0.25` (`HardBarrier`; expected at
  density 96).
- `HereditaryConcentration` — land Gini rises by `≥ 0.15` from the first mature generation to the final, OR the
  top-10% of owners hold `≥ 0.50` of viable land — **classified a FAILURE only if** buyer material purchases
  fail or landless starvation rises; if concentration rises but the buyer tier *survives*, it is a reported
  *coexisting landed-class* outcome, not a failure.
- `LandlessProletariat` — (`impartible`) `landless_share ≥ 0.60` and rising across ≥2 generations. Healthy
  variant if the buyer-purchase floor passes; FAILURE variant if non-owner survival `< 0.60` or the bought-food
  floor fails.
- `FragmentationCollapse` — (`partible` only) `≥ 0.35` of inherited shares below `LAND_VIABLE_REGEN_FLOOR`, OR
  median viable land per owner falls by `≥ 0.40`, plus food/survival decline (morcellement → Malthusian collapse).
- `SeedClusterOnly` — `owner_share < 0.10` OR the owner count stays within the initial seed cohort with no
  generational inheritance expansion.
- `StillThrashes` — (sanity, at the `idle_forfeiture_horizon = 12` end of the sweep) reproduces S23a's churn.
- `SecureTenureStableClass` — passes the full healthy-class classifier above without tripping concentration/
  fragmentation collapse.

**Generational tracking (required):** report, per generation, the owner set, owner-lineage share / land-Gini,
**land-per-capita** (owned land / population — the Malthusian pressure), the **landless share** (agents with no
plot + no heir-claim), heir-inheritance vs heirless-recycle counts, plot-viability (under `partible`), and
buyer-tier survival — so the result shows the *trajectory* across generations (concentrating? fragmenting?
producing a landless buyer tier? collapsing?), which is the whole point of secure heritable tenure under a
growing population.

Contrast axis (the whole point): the `idle_forfeiture_horizon` sweep should show `StillThrashes` at 12 and
(hypothesis) `SecureTenureStableClass` at ∞, with the long-horizon in between — isolating the security variable.

## 4. Mechanics / controls / verification (PINNED — Codex spec-review)

**Universal-heir order (PINNED deterministic; Codex §5).** On an owner's death, choose the heir by the first
that yields a live agent:
1. a live **child in the owner's household**, deterministic by **age descending, then stable agent id**;
2. else the nearest live **kin** by household/lineage relation (if available);
3. else a designated live **household successor** from the owner's household;
4. else a deterministic **colony next-of-kin**: the live non-owner minimizing `(household_distance,
   spatial_distance, agent_id)`;
5. else (no live agents qualify) the plot **reverts to unowned**.
**Conservation invariants:** exactly ONE successor per impartible plot; NO dead-owner plots after settlement;
NO duplicate ownership; every inheritance event logs `{deceased id, heir id, plot id, regime, pre/post effective
capacity}`.

**Partible = fractional beneficial interests on the SAME node (PINNED; no spatial subdivision — Codex §6/§8).**
The one `ResourceNode` stays; `effective_regen` + `effective_cap` are **split among co-heirs**; the sum of heir
shares **equals** the pre-death effective plot capacity (modulo explicit floor/writeoff accounting — a share
that falls below `LAND_VIABLE_REGEN_FLOOR` is classified **non-viable/stranded and LOGGED**, never silently
created/destroyed). Each co-heir has a holding share; harvest is capped by their share. **Real node subdivision
is explicitly forbidden in S23c** (it would add spatial/pathing churn unrelated to inheritance).

**Axes / controls (each a test):**
- **`inheritance_regime ∈ {impartible, partible}`** — the headline axis (both ship this milestone).
- **plot-density band {24,48} primary; 12, 96 boundary probes** — predeclared; success within the band.
- **`idle_forfeiture_horizon` sweep** {∞ (secure headline), long ≈200, 12 (= S23a)} — the security contrast;
  must be outcome-driving (12 reproduces S23a's thrash; ∞ tested for the stable class).
- **`property_off`** (commons) baseline; **`non_excludable_deed`** (title, no owner-only harvest) — title alone
  vs exclusion.
- **S22f-base robustness appendix** (labeled control, NOT headline) — secure tenure on the known-healthy
  two-tier base, to separate "forfeiture broke S23a" from "secure land needs the commitment institution".
- **`fixed_partition` writeoff guard** — the partible capacity split conserves (sum of shares + stranded =
  pre-death capacity).

**HARD GUARDS every run + cell:** conservation every tick; `bread_minted_max == 0`; provenance
clean-or-disqualified; money promotes; **the plot-registry invariant** (finite plots, ≤1 owner per atomic plot
/ conserved shares per partible plot, inheritance conserved, NO dead-owner plots); **per-generation tracking**
(§3) reported. New default-off flag `secure_land_tenure` (+ `inheritance_regime`) canonicalized ON-only under
the **next free flag-digest tag (18** unless master advanced — S23a used 13); **goldens byte-identical off.**

New suite `sim/tests/secure_land_tenure.rs`; `goldens_unchanged`; classify-not-tune; predeclared consts.

## 5. Honesty / scope (DRAFT)

- A **SUCCESS** = *secure* private property (unlike S23a's insecure forfeiture) stabilizes a bounded
  owner-cultivator class + surviving buyer tier — i.e. **S23a's collapse was caused by the *forfeiture rule*,
  not by private property per se** (the realism critique confirmed), and secure title is a genuine
  division-of-labor institution in land. That would materially revise the S23 arc's conclusion.
- A **clean fail** (`TenureInertStaticPin` / `NoStableClass`) = even *secure* private land does not by itself
  build the two-tier economy — strengthening the S23 negative (it wasn't just the harsh clock) and leaving
  S22f's contract as the lone stabilizer.
- The `idle_forfeiture_horizon` sweep makes the result a **curve**, not a single point — the honest way to
  present "how much of S23a's thrash was the clock."
- **THE BIGGEST TRAP (Codex §9), load-bearing:** do NOT let "stable ownership" stand in for "a working land
  economy." Secure title MECHANICALLY reduces churn — that proves almost nothing. The honest success is ONLY
  the full §3 healthy-class classifier (owners cultivate + produce, buyers materially buy, non-owner survival
  holds, concentration bounded); low churn is necessary, never sufficient. S23a's old churn/drop metrics are
  reported as NON-success diagnostics.
- **This is a DRAFT.** Next step: Codex spec-review to resolve the four open design questions (§2), pin the
  success bar + finding-mode thresholds + the base choice, then rb-lite build under the usual discipline.
- Follow repo conventions; NEVER add Claude/AI/assistant references in code, comments, or committed text.
