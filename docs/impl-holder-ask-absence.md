# impl-75 — C3R.j: Why the flour has no price — decompose the holder's absent reservation ask

Status (spec): **v2 — REVISED** (Codex+Fable dual review folded; both NEEDS-REVISION, no blockers — every
seam exists). The milestone survives but Cut 1 changes shape and lever (b) is CUT. See `## −0`.
Successor to impl-74 (C3R.i). Origin: the C3R.i
census (`docs/impl-flour-reignition.md` §−0.9, report §37) proved the mortal wall is **`HolderWithoutAsk`**
— at the first post-death Bake decline, living holders sit on >100 units of flour yet **not one** has a
computable `reservation_ask_for_money(flour)`. R2-as-specced was closed INTERVENTION-INVALID before it
was built. **This milestone does NOT pull a lever until Cut 1 measures why.**

## −0. v2 revision (AUTHORITATIVE — folds the Codex+Fable dual review; supersedes §§0–5 on conflict)

Both reviewers verified against the code. **No blockers** — every seam exists or is a trivial `&self`
addition. But the spec had a test that would never fire, an outcome tree that leaks, a bucket that
conflates three different economies, and **two instances of over-read #11**. Revisions, by priority:

1. **[P0 — the acceptance base would NEVER TRIGGER]** §5 named `baker_role_diagnostic::config(false)`,
   which never enables `stale_input_price_fix` — and `InputPriceAbsent` is *unreachable* with that flag
   off (`phases.rs:2305`). The landed census sets it `true` (`flour_reignition_census.rs:24`). **Use the
   census's own config and ASSERT `stale_input_price_fix == true`**, else the five-seed run reaches its
   tick cap observing nothing.
2. **[P0 — the tree is not exhaustive across seeds]** C3R.i pinned `HolderWithoutAsk` on **seed 3 only**
   (`flour_reignition_census.rs:150–158`). On other seeds the first post-death decline may classify as
   C3R.i's sibling buckets (`ZeroHolder`/`CommonsLocked`/`MillerSide`, `flour_reignition_census.rs:29–34`),
   which have **no bucket** in this tree. Add **`OtherWall(<C3R.i bucket>)`** and gate the decomposition
   per-seed on `HolderWithoutAsk`, so a seed-7 `MillerSide` is reportable instead of unassertable.
3. **[P0 — the `975` bucket conflates THREE economies]** `None` at `agent.rs:975` means *any* of:
   **(i) no money want in the scale at all**, **(ii) every money want already provided**, **(iii)
   unprovided but covered by gold on hand**. These need *different* levers — a Consumer heir with **no
   money want was never a "satiated saver"**, so §3's parallel is simply FALSE for (i). Cut 1 must
   report, per holder, **money-want count / provided count / gold vs cumulative-required** and split the
   bucket three ways. Otherwise a recurring-money-want lever gets built and is inert because the holders
   never had a money want to recur.
4. **[P0 — over-read #11a: the `MoneySatiation → spoilage` wiring is DELETED]** §2b read "holders at
   33–36 flour, above `FREE_STORAGE = 20`, in the one good the pressure never touches" as causal support
   for the carrying-cost lever — while the spec's *own* trap paragraph, now code-confirmed by both
   reviewers, proves that lever invalid under exactly that outcome. Anticipated decay never enters
   `reservation_ask_for_money` (it receives only scale/stock/gold/good/qty/money-good, `agent.rs:443`);
   staple spoilage works through **returning hunger** (`phases.rs:2102–2103`), a demand-side channel
   flour does not have; and at 1500 bps integer flooring grinds 33 flour to 26 and stops
   (`phases.rs:2123`). **Delete (b)-as-spoilage entirely.** The physical asymmetry is real but is not a
   lever.
5. **[P0 — the surplus-ask lever is self-contradictory as written]** "a satiated holder still offers
   genuine surplus … never a forced below-reservation ask" cannot both hold: for a holder whose money
   wants are all provided, **any** sale price fails the ordinal gain test (`agent.rs:489–497`), so any
   posted ask is *definitionally* below-reservation **until the money-want ladder itself is extended**.
   So lever (b) in ANY form means extending the money-want ladder (the true ask-side analogue of
   `recurring_motive`, `phases.rs:2351`), and **its pricing rule must be specified before it is a
   candidate** — otherwise defer (b) out of this milestone.
6. **[P0 — over-read #11b: inventory magnitude read as cause]** §0 said removing one flour unit drops no
   allocation *because* holders have 33–36 units. The real reason is that these vocations' scales
   **contain no flour want at all** (bread, grain, WOOD, leisure, money; a Miller's production extension
   adds its *input* grain, not its *output* flour). The predicted `975` outcome still holds, but the
   stated chain is wrong and would break the moment a scale gained a flour want. **Measure `lost_rank`
   and before/after allocation; never infer them from stock size.**
7. **[P0 — "resolves" is undefined, so the persistence STOP is unsafe]** The capture fires only on a Bake
   `InputPriceAbsent` decline (`phases.rs:2324`), so **row-absence is ambiguous** between "ask appeared",
   "a different rejection", and "no Bake candidate appraised at all". Add an **unconditional `&self` row
   accessor** plus per-tick deltas of the `role_choice_diag` bake histogram (`mod.rs:13056`), and define
   **resolves = the Bake reason reaches `Accepts` within the window** — not merely exiting
   `InputPriceAbsent` (a state that exits into a persistent `MarginNonpositive` would otherwise STOP the
   milestone while the chain stays dead). **Pin the window numerically.**
8. **[P0 — `AskPostedButUnseen` needs a STOP-and-escalate arm]** The bucket is substantively right (the
   posting side nets stock **and gold** via `available_agent` and shades the limit, `society.rs:3313/3333/
   3344/3599–3624`, while `fresh_input_ask` reads the raw reservation, `mod.rs:10142–10166`). But Cut 2
   has **no arm for it**, and the obvious lever — make the appraisal read the book — is **forbidden by
   the determinism contract**: `live_quotes` is not in `canonical_bytes`, so a DIGESTED flag may not
   depend on it (`mod.rs:10110–10113`). Add an explicit **STOP-and-escalate** arm naming that conflict.
   Also require a **live-limit counterfactual** (would Bake pay at the posted limit?) before calling this
   bucket causal — a live ask at 100 is not "the wrong read" if the candidate would decline anyway.
9. **[P0 — the tree's axes are orthogonal, not one list]** Reason × market-state × provenance × temporal
   are independent; `TransientOnly` and `NotPostDeathHeir` can co-occur with every reason bucket. Replace
   the flat list with **explicit axes plus a stated classification precedence** (identity → persistence →
   dominant reason) and **define "dominates"**. `StockReserved` is *not* a raw-`None` cause at all — the
   census evaluates on full physical stock while the market evaluates a reservation-netted clone
   (`mod.rs:13104` vs `society.rs:3599`) — so it belongs on the market-state axis, not the reason axis.
10. **[P1 — row fields the diagnosis actually needs]** Add **holder gold** (`FlourCensusColonist` has
    none today, `mod.rs:4212`) and **reserved gold** (`market.rs:108`) — in the predicted branch the
    raw-vs-netted divergence behind `AskPostedButUnseen` is **gold-side, not stock-side**. Free stock
    already exists (`free_stock_after_all_reserves`, `society.rs:6839`; `reserved_stock`, `market.rs:114`).
11. **[P1 — name the live-ask accessor]** `Society::live_quotes` (`society.rs:475`) and `find_live_quote`
    (`society.rs:6671`) are **private**. The spec must name the new `pub &self` accessor it adds (or read
    the public order book, `market.rs:68`) — do not assume the read exists.
12. **[P1 — the `None` taxonomy must be total, and must not drift]** Cover **every** return path, not just
    `449/476/975`: the defensive removal, `checked_add`, and post-price validation exits
    (`agent.rs:456/487/489`; `500` is formally unreachable since adding gold is monotone in
    `provisioning_with_optional_money`, `agent.rs:861–905`) via a **debug-asserted catch-all** rather than
    silently folding them into a real bucket. Make the production `reservation_ask_for_money` a **thin
    `Some/None` projection of the new diagnostic**, with byte-identity proven, so the two cannot diverge.
13. **[P1 — provenance is three/four-way, not binary]** `producer_tool_inheritors` already records
    inherited `(heir, tool)` (`mod.rs:5413`, written `demography.rs:423`) — **no new state needed**. But
    this base has `producible_capital = true` (`scenarios.rs:375`), so an oven may be **self-built**.
    Classify: `SeededLatent` (`colonist.latent == Some(Bake)`) / `Inherited` (inheritor-set) /
    `SelfBuilt` / `Other` — else a self-builder is mislabeled "seeded" and the identity STOP misfires.
14. **[P1 — Cut 2(a): fix the citations and name the missing helper, or drop it]** The frozen read is the
    **output** price at `mod.rs:8595` with skip-on-`None` at `8605–8607` (NOT the `8566/8590` this spec
    cited). Its live analogue is a **bid-side** roster proxy — `reservation_bid_for_money` exists, but a
    `fresh_output_bid` helper does **not**. Moreover Cut 1 as specced captures no output-price/imputation/
    bid/execution fields (and `BootstrapTrace` only records while the acquisition ledger is active,
    `mod.rs:13129`), so **Cut 1 cannot select lever (a)**. Either add miller-restock telemetry to Cut 1
    **and** name the helper, or drop Cut 2(a) from this milestone.
15. **[P2 — §3 is gated on the wrong thing]** Gate the satiation-parallel claim on the **three-way split**
    of item 3, not the aggregate bucket; and note that "zero carrying cost" applies to **every** ask (no
    anticipated decay is modeled anywhere), not specifically to flour.

**Net:** Cut 1 becomes a per-seed-gated, axis-structured decomposition with a total `None` taxonomy, gold
fields, a defined "resolves", and an escalation arm; lever (b) is CUT as specced (spoilage is a trap; the
surplus-ask needs a money-want-ladder extension with a stated pricing rule); lever (a) is contingent on
adding restock telemetry. §§0–5 below are the v1 rationale, superseded here on conflict.

## 0. The three assumptions C3R.i left UNLICENSED (this is the whole reason for Cut 1)

The adversarial review of the landed census was explicit that the result licenses only *"abundant flour,
no seller-ask, at this appraisal."* Three things were **assumed, not measured**, and each independently
changes which lever is correct:

1. **Which `None` branch fires.** `reservation_ask_for_money` (`econ/src/agent.rs:443`) returns `None`
   from three places: non-holder/money-good (`449`), provisioning break (`476`), or no money-want at or
   above `lost_rank` (`486→975`). The *expected* answer is money-want satiation at `975` (with 33–36
   flour held, removing one unit drops no allocation ⇒ `lost_rank = scale.len()` at `474`, so `476`
   cannot fire and the `950–975` scan is maximal ⇒ `None` means every money want is already provided or
   covered by gold on hand — and the holders carry 76–176 gold). **Expected ≠ measured.** If instead
   `476` fires, the wall is a provisioning break and the lever is entirely different.
2. **Persistence.** C3R.i captured ONE tick on ONE seed and stopped. "The wall persists" is inference.
   A state that resolves next tick is not a wall.
3. **Heir identity.** Role choice appraises Bake for any colonist that is `latent = Some(Bake)` **or**
   holds an oven (`sim/src/settlement/phases.rs:2241–2254`). The census records neither the candidate's
   vocation nor whether its oven was **inherited**. So "an heir cannot re-ignite" is not established —
   the decliner may be a surviving founder or an original latent baker that never adopted.

## 1. Cut 1 — the decomposition (DIAGNOSTIC ONLY, no intervention, non-steering)

Extend the existing `flour_reignition_census` seam (`sim/tests/flour_reignition_census.rs`,
`build_flour_census_row` in `sim/src/settlement/mod.rs`). Same discipline as C3R.i: default-off,
digest-excluded, `&self`-pure capture, proven byte-identical.

- **Per holder, the `None` sub-branch.** Add a read-only diagnostic that reports WHICH branch fired
  (`MoneyGoodOrNonHolder` / `ProvisioningBreak` / `NoMoneyWantAtRank` / `Some(price)`). Prefer a
  `&self` diagnostic variant beside `reservation_ask_for_money` that returns the reason, so the census
  never re-derives (and cannot drift from) the real rule.
- **Per holder, free vs reserved stock** and **whether a live ask is posted** for flour in the order
  book. This separates three distinct states the single `None` currently collapses: *won't sell*
  (no reservation price), *can't sell* (stock reserved), *is selling but the appraisal can't see it*
  (`fresh_input_ask` reads a raw reservation, NOT the netted/shaded posted quote —
  `econ/src/society.rs:3313,3333`).
- **Candidate identity:** vocation, oven holding, and oven **provenance** (inherited vs seeded), so
  "heir" becomes measured.
- **Persistence:** do not stop at the first row. Sample the decline state every tick over a pinned
  window after the first post-death decline, across the 5 canonical seeds (`3,7,11,19,23`), and report
  per-seed whether the no-ask state **recurs, resolves, or oscillates**.

**Cut 1 outcome tree (disjoint, per seed):** `MoneySatiation` (`975` dominates) / `ProvisioningBreak`
(`476` dominates) / `StockReserved` / `AskPostedButUnseen` (a live quote exists while the raw
reservation is `None` — the appraisal is reading the wrong thing) / `MixedBranch` / `TransientOnly`
(state resolves within the window) / `NotPostDeathHeir` (candidate is not an oven-inheriting heir).
Suite `MIXED` when seeds disagree.

**STOP conditions (carry the C3R one-milestone discipline forward):** if Cut 1 returns `TransientOnly`
or `NotPostDeathHeir`, the C3R.i *framing itself* is wrong — **re-frame and STOP; do not pull a lever.**

## 2. Cut 2 — the lever, CHOSEN BY CUT 1 (do not pre-commit)

Two **verified structural asymmetries** are the leading candidates. Both have the same shape as the
C3R.h bug that started this arc: *a fix applied at one stage but not its sibling stage.*

- **(a) The price asymmetry.** The C3R.h live-price fix went into role choice (`fresh_input_ask`) but
  **not** into the producer's own restock imputation: `sim/src/settlement/mod.rs:8556` still values the
  miller's output at the **frozen** `realized_price(output)` and skips the grain bid on `None`
  (`8566`, `8590`). Lever = apply the live-price fix **symmetrically** to `project_input_bids`.
  Indicated if Cut 1 shows the miller side failing to restock on a stale/absent output price.
- **(b) The carrying-cost asymmetry.** `run_spoilage` (`sim/src/settlement/phases.rs:2080–2123`) exists
  precisely to break satiation-withdrawal — its own comment: *"Targeting the satiation hoard is the
  point: when the staple decays, hunger returns and the holder must re-enter the market"* — and it
  pressures **staple + subsistence + grain**, each threshold-protected at `FREE_STORAGE = 20`. It does
  **NOT** include **flour**. The measured no-ask holders sit at **33–36 flour**, above that threshold,
  in the one good the anti-satiation pressure never touches. Indicated if Cut 1 returns
  `MoneySatiation`.

  **Do NOT assume (b) works — it must be measured.** Spoilage *destroys stock*; it does not by itself
  *create an ask*. If `reservation_ask_for_money` is purely current-provisioning-based (it appears to
  be), decay pressure may destroy flour **without** producing a reservation price — making things
  strictly worse. Cut 2 for (b) must therefore first answer: *does anticipated decay enter the
  reservation computation at all?* If not, the faithful lever is not spoilage but the **ask-side
  analogue of `recurring_motive`** — a satiated holder still offers genuine **surplus** (holdings above
  its own working/provisioning need), which is a *seller-motive* change, never a forced
  below-reservation ask or a forced fill.
- **(c) Speculative adoption** (the old R1) stays **out of scope as governance** — it needs a binding
  forward order (funded buyer, seller commitment, quantity, settlement), a separate untested mechanism.
  If Cut 1 indicates only (c), pin the null and STOP.

## 3. Why this is the same wall the early arc already hit (hypothesis, to be confirmed by Cut 1)

The unifying root across experiments 1–7 was: **a bounded savings want + zero carrying cost ⇒ satiated
agents withdraw, holding goods AND money; the rest cannot re-enter.** That was fixed on the *adoption*
side by `recurring_motive` (produce while the recipe is simply profitable, because consumption recurs)
and on the *hoard* side by threshold spoilage. C3R.i's finding is structurally the same failure on the
**ask** side: money-satiated millers hold flour and post no price. If Cut 1 returns `MoneySatiation`,
this is not a new wall — it is the original satiation wall surfacing in the one good and the one
direction its two known mitigations were never applied to. State this only if Cut 1 confirms it.

## 4. Determinism obligations

Cut 1 is non-steering telemetry: NOT digested, `&self`-pure, default-off, with a
`canonical_bytes`-exclusion test that provably captures a row (non-vacuous), mirroring
`canonical_bytes_excludes_flour_census`. Any Cut 2 behavior flag is **DIGESTED ON-only** and classified
in `digest_coverage_chain_config`, with off-flag byte-identity proven tick-by-tick and no golden moved.

## 5. Acceptance

Cut 1: the decomposition runs on the pinned mortal base (`config(false)` of
`sim/tests/baker_role_diagnostic.rs`) across the 5 canonical seeds, prints the per-seed classification,
and **asserts the measured bucket** (as C3R.i learned to do — an unasserted census result silently
regresses). Full workspace `cargo test` green (plain cargo, no nix), `clippy --all-targets -D warnings`
clean, `fmt --check` clean, no golden/digest moved.
