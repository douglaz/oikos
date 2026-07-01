# impl-45 — S23c: Secure private land tenure (was it *insecurity*, not private property, that broke S23a?)

Status (spec): DRAFT — for Codex spec-review. Base: master `855be95`. **Re-opens the S23 private-property arc**
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

## 2. The design (headline + a realism spectrum) — OPEN QUESTIONS FLAGGED FOR CODEX

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

**REALISM DECISION on inheritance (Codex to rule):** in the model inheritance is **not universal** — only
lineage households have heirs. Real land inheritance is universal. If only lineages retain land, secure
heritable tenure will *mechanically* concentrate land in lineages — potentially an **artifact of the
demographic model**, not an economic result. Two options: **(a) extend inheritance to ALL households** (a
small, realistic engine change: every owner has a determinable heir, so newborns/next-of-kin inherit
regardless of lineage) so concentration is *emergent*, not built-in; or **(b) keep the lineage-only asymmetry
but disclose it prominently** and treat lineage-concentration as a known feature. Recommend (a) for a fair test
unless it destabilizes the base.

**OPEN DESIGN QUESTIONS (Codex to resolve in spec-review):**
0. **The inheritance realism decision above** — universal heirs (a) vs disclosed lineage-only (b) — and whether
   (a) is a safe, goldens-preserving engine change.
1. **The static-pin vs healthy-class distinction (the crux).** Under secure tenure, a fixed set of first-movers
   owns the plots permanently. Is that (a) the *desired* outcome — a **stable bounded owner-cultivator class**
   who cultivate their secure plots + a **surviving buyer tier** who buy the food they produce (the two-tier
   economy = SUCCESS), or (b) a degenerate **static pin** (owners never cultivate/trade, nothing moves)? What
   metric distinguishes a *healthy stable class* (owners actually cultivate + produce + buyers materially buy)
   from an *inert pin*? The success bar must reward the former and fail the latter.
2. **What bounds ownership to a MINORITY + keeps a buyer tier?** With secure tenure + open homesteading, if
   every agent homesteads a plot → universal ownership (`HardBarrier`, no buyers). The land-capacity axis
   (scarce-but-adequate plots vs population) is the lever: too many plots → everyone owns → no buyers; too few
   → HardBarrier. Is there a plot-density band where a **bounded owner minority + surviving buyer majority**
   emerges under secure tenure? (S23a's population-scaled axis is the tool.)
3. **Is the base "healthy" enough, or does it need S22f?** S23a's base (S22a on, population-scaled) had its
   buyer tier collapsed *by* the forfeiture thrash. On the *same* base with secure tenure, does the buyer tier
   *survive* (proving forfeiture was the culprit), or was the base already fragile? Should the healthy-base
   variant layer secure tenure on the **S22f two-tier-preserving** regime, or does that conflate land tenure
   with the commitment institution? (Codex ruling needed: same-base-as-S23a for a clean isolation, vs
   S22f-base for a guaranteed-healthy two-tier economy.)
4. **Relation to S23a's `no_forfeit → TenureLeverInert` result.** S23a already ran `no_forfeit` as a control
   and classified it inert *by S23a's churn-drop bar*. S23c's claim is that the RIGHT bar for secure tenure is
   not "churn drops" but "**a stable bounded owner-cultivator class + surviving buyer tier forms**" — a
   two-tier-economy bar, not a stickiness-churn bar. Codex to confirm this reframing is legitimate (not
   success-bar shopping) — i.e. secure tenure is a *different question* (does secure property build a class?)
   than S23a's (does forfeiture make occupation sticky?), warranting a different, predeclared bar.

## 3. Success / finding modes (DRAFT — to be pinned with Codex)

**Primary success = `SecureTenureStableClass`** (majority of seeds, within a disclosed plot-density band):
1. **Bounded owner minority** — owner share ∈ `[MIN, MAX]` (a minority own plots; not universal `HardBarrier`,
   not seed-only).
2. **Owners actually cultivate + produce** — the owner cohort persistently cultivates its secure plots and
   produces food (NOT an inert pin: cultivation + grain output materially above zero, sustained).
3. **A surviving non-owner buyer tier materially buys** — the non-owner majority survives and buys the food the
   owners produce (post-money bought ≥ floor) — the two-tier economy the S23a thrash destroyed.
4. **Stable membership** — the owner-cultivator cohort is persistent (low churn — secure tenure means owners
   don't rotate), a genuine *class*, not S23a's rotating thrash.
5. Money + mortality + provenance + conservation hold; goldens byte-identical off.

**Finding modes (DRAFT):**
- `TenureInertStaticPin` — owners hold plots but don't cultivate/produce/trade meaningfully (a degenerate pin,
  not a class).
- `UniversalOwnershipNoBuyers` — plot density too high, everyone owns, buyer tier gone (`HardBarrier`).
- `HereditaryConcentration` — over generations, land concentrates into a few long-lived lineages
  (owner-lineage Gini / top-k share rises across generations) AND the buyer tier is squeezed out — a
  landed-aristocracy collapse. (If concentration rises but buyers *survive*, that is a coexisting
  landed-class outcome, reported, not a failure.)
- `SeedClusterOnly` / `NoStableClass` — no bounded persistent owner-cultivator cohort forms.
- `StillThrashes` — (sanity, at the `idle_forfeiture_horizon=12` end of the sweep) reproduces S23a.
- `SecureTenureStableClass` — a bounded owner-cultivator class + surviving buyer tier forms and persists across
  generations under secure tenure (concentration bounded, not runaway).

**Generational tracking (required):** report, per generation, the owner set, owner-lineage share / land-Gini,
heir-inheritance vs heirless-recycle counts, and buyer-tier survival — so the result shows the *trajectory*
across generations (concentrating? stable? recycling?), which is the whole point of secure heritable tenure.

Contrast axis (the whole point): the `idle_forfeiture_horizon` sweep should show `StillThrashes` at 12 and
(hypothesis) `SecureTenureStableClass` at ∞, with the long-horizon in between — isolating the security variable.

## 4. Controls / verification (DRAFT — to be pinned with Codex)

- **`idle_forfeiture_horizon` sweep** {∞, long, 12} — the core contrast; must be outcome-driving (12 reproduces
  S23a thrash, ∞ tested for the stable class).
- **plot-density sweep** (the S23a population-scaled land axis) — locate the bounded-minority band.
- **`property_off`** (commons) baseline; **`non_excludable_deed`** (title but no owner-only harvest) — title
  alone vs exclusion.
- The `TenureInertStaticPin` guard: a stable owner set that does NOT cultivate/produce/support-buyers is a
  *finding*, not a success.
- HARD GUARDS: conservation every tick; provenance clean; money promotes; the plot-registry invariant (finite
  plots, ≤1 owner, inheritance conserved, no dead-owner plots); **goldens byte-identical off** (digest tag 15
  ON-only, or reuse the S23a tag surface if it composes cleanly — Codex to confirm).
- New suite `sim/tests/secure_land_tenure.rs`; `goldens_unchanged`; classify-not-tune; predeclared consts.

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
- **This is a DRAFT.** Next step: Codex spec-review to resolve the four open design questions (§2), pin the
  success bar + finding-mode thresholds + the base choice, then rb-lite build under the usual discipline.
- Follow repo conventions; NEVER add Claude/AI/assistant references in code, comments, or committed text.
