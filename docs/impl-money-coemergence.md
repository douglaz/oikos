# Implementation Spec: money co-emergence with the specialized economy (S8)

> The S5/S6/S7 economy runs on **designated GOLD** — it is handed a money good and
> only then does calculation, input bidding, retained earnings, and capital
> formation occur. In Mengerian/Misesian terms that bypasses the *origin* of
> economic calculation: money should emerge from the most-saleable commodity through
> indirect exchange (Mises's regression theorem needs a commodity-money origin).
> Money emergence exists in the engine (`barter_camp`/`frontier`) but has **never
> been composed with the sustained specialized chain**. This milestone composes
> them: run the S5/S6/S7 stack from barter into endogenous SALT money, then sustain
> specialization and capital formation on the *emerged* money.

## Purpose & the honest bar

Make money, prices, the grain→flour→bread division of labor, and capital formation
**co-emerge in one run**: start with no designated money; let SALT promote by
saleability from real indirect exchange; after promotion, all input bids, role
adoption, retained earnings, and capital builds use the *emerged* money — and the
chain **sustains** (it does not freeze at the barter→money cutover). A
no-saleability control must **fail** to monetize or to sustain.

**This milestone may fail for principled reasons, and that is informative.** Prior
experiments showed (a) a subsistence↔extent-of-exchange tension and (b) a
post-promotion barter shutoff. If the chain only sustains on designated gold, that
means the "resolved economy" depends on an exogenous money scaffold — a real finding
to report honestly, not to paper over. Do **not** relabel the model "authentic
praxeology" until this passes; until then S5/S6/S7 is a strong *engineered* colony
economy, not a proven Mengerian market order.

NOT firms/wage-labor, NOT banks/fiat/credit (Credit era / G8 stack), NOT a change to
econ market-clearing or the saleability/promotion machinery (the conformance goldens
incl. the G5a/G5b emergence goldens stay byte-identical; every edit additive/gated),
NOT designating or pre-seeding any money, NOT curated food/input/capital placement
(`subsistence_advance`/`input_advance`/`capital_advance` stay off).

## Verified Base Facts (oikos @ `5f3e58c`)

1. **The S5/S6/S7 phases are already money-good-agnostic — the great news.** Every
   appraisal/bid/build threads `current_money_good()`, not hard-coded GOLD: project
   input bids `set_project_input_bid_overrides` (`sim/src/settlement.rs:5998`, passes
   `money` into `imputed_input_reservation` `:6042`); role choice
   (`:6222`→`recipe_adoption_pays_for_money` `:6311`); capital build gated on
   `current_money_good().is_some()` (`:6515`) with prices in the market money unit
   (`capital_build_surplus` `:8872`). The only GOLD-hardcoded path is the unused
   `recipe_adoption_pays` wrapper (`:8800-8817`), which these phases do **not** call.
2. **Money moves already handle the emergent regime.** `move_money_conserved`
   (`:6163-6181`) tries `transfer_gold` (designated/M3) and falls back to a direct,
   reservation-checked `Agent.gold` move when `uses_closed_gold_money()` is false (the
   emergent case). The S5/S6/S7 money-moving phases all use it. Post-promotion the
   emerged money **is** `Agent.gold` (1:1 SALT→gold at promotion,
   `promote_v2_money_good` `econ/src/society.rs:2264-2303`), representationally
   identical to designated gold; only the *regime tag* differs.
3. **Conservation + digest already cover emergence.** The good→money conversion is
   booked into `report.promoted` (`:4387-4396`) so `conserves()` holds across the
   promotion tick; `canonical_bytes` serializes the full Mengerian runtime
   (`push_emergence_runtime_bytes` `:8157-8176`). No S5/S6/S7 state is keyed on the
   GOLD *good*.
4. **The regime is selected by ONE config dimension.** `(config.barter, config.m3)`:
   `Some(barter) ⇒ Emergent`; `None ⇒ Designated(GOLD)` (`:3825-3838`).
   `frontier_endogenous` forces gold with `cfg.barter = None` (`:2562`) and seeds
   non-zero gold (`starting_gold_* = 60`, `producer_gold = 16`, household/child
   16-60, `:2563-2589`); `frontier_endogenous_scaling`/`frontier_capital` inherit it.
   **A barter overlay asserts ZERO initial money everywhere** (`:3398-3425`) — so the
   emergent path requires zeroing every gold endowment and supplying a SALT medium.
5. **`frontier` (G5b) is the existing emergent composition** (`:2148-2296`):
   barter-start, SALT medium (`consumer_medium_endowment: 80`, candidates
   `[WOOD, bread, SALT]`), latent millers/bakers that adopt **only after promotion**
   (role choice early-returns while `current_money_good()` is `None`). It proves
   emergence + adoption **fire and conserve**, but only over a short horizon — it is
   the very scenario the 13 experiments showed **freezes ~tick 150**; it lacks the
   S5/S6/S7 sustain stack (`recurring_motive`, `project_input_bids`,
   `producer_subsistence`, threshold spoilage, re-entry, producible capital).
6. **TENSION A — subsistence crowds out monetization, AND S6 re-entry is coupled to
   it.** SALT promotes on the breadth and volume of acceptances
   (`econ/src/menger.rs:155-217`); bread-for-SALT and WOOD-for-SALT are the legs that
   monetize it. A grain subsistence floor (`subsistence_on_grain`, ranked just below
   bread, `life/src/scale.rs:65-69` via `known.subsistence` set at generation `:3296`)
   makes a hungry colonist eat cheap grain instead of buying bread → thinner
   bread-for-SALT trade → SALT may miss the promotion thresholds. This is *why*
   `subsistence_on_grain` is OFF in bare `frontier`. **Critically, S6 re-entry cannot
   substitute:** `run_productive_reentry` early-returns unless `known.subsistence ==
   Some(grain)` (`:6384`), so re-entry is INERT with the grain floor off and re-enables
   the crowd-out with it on. And `known.subsistence` is built once at generation
   (`life/src/scale.rs:160` reads it directly) — there is **no** dynamic post-promotion
   edibility switch; "stage subsistence on after promotion" requires NEW gated code
   (scale regeneration + a digest knob), not config. Therefore S8 does **not** use S6
   re-entry or staged subsistence — provisioning-at-scale under emergence is deferred
   (it is exactly where this tension bites).
7. **TENSION B — the promotion cutover.** On the promotion tick the barter book is
   wiped and SALT is delisted (`econ/src/society.rs:2280,2299-2300`); producers adopt
   only the *next* tick (role choice gated on the money phase) holding only the gold
   from their SALT→gold conversion. A freshly-adopted producer with little free gold,
   an unmet bread want, and a savings want **reserves its cash and its input bid never
   clears** → the chain shuts off right at the cutover (the diagnosed Exp-9 gate).
8. **The rejection list keeps promotion conserved** (`money_rejection_goods`
   `:2967-2985`, auto-built from nodes ∪ chain goods ∪ hearth `:3696-3726`); the sim
   takes `step_rejecting_v2_money_goods` only when `barter.is_some()` (`:4357-4359`).
   A new emergent builder must keep `barter = Some(..)` so this path runs and every
   renewable chain good is rejected as a money candidate.

## Scope (Codex spec-review): core co-emergence, NOT provisioning-at-scale

S8 proves **money + the specialized chain + capital co-emerge in one run**. It does
**not** attempt S6 provisioning-at-scale under emergence — that needs raw-grain
subsistence, which crowds out monetization (Base Fact 6), so it is a deferred
follow-on (S9). S8 therefore runs a **modest colony** (≈ the `frontier`/endogenous
size, not the S6 pop-87 scaling colony) with `subsistence_on_grain` and S6 re-entry
**OFF**; the household hearth + `producer_subsistence` (a local minted producer floor —
it feeds only producers, not consumers, and adds no raw-grain subsistence that would
crowd out consumer bread demand) keep producers solvent, while non-lineage
consumers/gatherers **buy bread**, which is exactly the bread-for-SALT trade that
monetizes SALT. A modest residual hunger tail is acceptable (it is the S9 problem).

## The slices (build in order; each independently testable)

- **S8.0 — instrumentation / emergence probe (build first).** Add the read-only
  diagnostics needed to tell a *principled* failure from a *tuning* one: per-vocation
  SALT and gold at the promotion tick (esp. latent/future Miller-Baker free gold),
  barter acceptances by candidate good, the promotion tick, bread-for-SALT barter
  volume, and the hunger/critical streak before promotion. Surface them via read-only
  accessors + the viewer dashboard. **Test:** a probe run records a promotion tick and
  non-empty per-candidate acceptance counts; deterministic.
- **S8.1 — the co-emergent base: money emerges WITH the chain present.** Add
  `SettlementConfig::frontier_coemergent` = the `frontier` barter-start emergent base
  (`barter = Some(..)`, SALT medium, **all gold endowments zero**) carrying the S5
  sustain stack (`recurring_motive`, `project_input_bids`, cold-start buffers,
  threshold spoilage), with `subsistence_on_grain` **OFF** and S6 re-entry **OFF** so
  the bread-for-SALT barter stays thick enough for SALT to promote. **Test:** SALT
  promotes endogenously (a real `promoted_at_tick` / the promoted good IS SALT, not
  merely `current_money_good().is_some()`), with no designated money and zero gold at
  generation, and the promotion tick conserves; **no active producer / no chain
  production before promotion**.
- **S8.2 — survive the cutover (Tension B).** Ensure a producer that adopts just after
  promotion holds enough **free** (non-reserved) gold to win its input bid, so the
  chain does not freeze at the barter→money discontinuity. The faithful lever is
  *earned* working capital: latent producers sell their seeded bread/flour/WOOD into
  the barter market during the emergence window, accumulating the SALT that converts
  1:1 to gold at promotion — no designated endowment, no curated advance. (Tune
  cold-start buffers / latent seeding only as a bootstrap, not a subsidy. If the
  `frontier` saleability hub concentrates SALT only in consumers so producers earn
  none — Base Fact 5 — adjust who trades what, not who is *given* money.) **Test:** the
  S8.0 probe shows ≥1 (latent/adopting) producer with positive converted gold and free
  gold to bid at promotion; and after promotion grain/flour are acquired by **real
  `Society::trade` records** (buyer an active Miller/Baker, seller ≠ buyer) and consumed
  as recipe inputs — the chain clears inputs across the cutover.
- **S8.3 — capital forms on emerged money (the full DoD).** Compose S7 producible
  capital onto the co-emergent base. **Test:** the clean metric below — bread sustains
  to t1600, ≥1 tool **built** (`produced`) under emerged-money prices, hunger bounded
  (modest tail allowed), conserved every tick, deterministic.
- **If S8.1/S8.2 fail for a principled reason** (no parameter band both monetizes and
  survives across seeds; or the chain freezes at the cutover): that is a **first-class
  deliverable** — land a *passing* diagnostic test that asserts the observed failure
  mode (e.g. `monetization_and_survival_windows_are_disjoint` using the S8.0 probe),
  plus a written finding. Do NOT force a pass with designated gold or curated advances,
  and do not leave only a vague "may fail" note.

## Acceptance Tests (the S8.3 DoD) — `sim/tests/money_coemergence.rs`

1. `coemergent_run_is_deterministic` — same `(seed, config)` → byte-identical
   (`canonical_bytes` + `digest`) through barter→promotion→money→production→capital.
2. `no_designated_money_and_zero_gold_at_generation` — the setup honesty check:
   `frontier_coemergent` generates with `barter = Some(..)`, **no designated money**,
   and **every gold endowment zero**; no curated phase (`*_advance`) is on.
3. `money_emerges_endogenously` — the **promoted good IS SALT** with a real
   `promoted_at_tick()` (not merely `current_money_good().is_some()`), and **no active
   Miller/Baker and no chain production occur before the promotion tick** (the chain
   waits on money, proving emergence drives it, not the reverse).
4. `no_saleability_control_does_not_monetize` — a **non-vacuous** control: keep a
   physical SALT stock AND the barter overlay, but remove the *asymmetry* that makes
   SALT widely accepted (e.g. spread the medium endowment evenly / drop the universal
   medium want, `barter_camp_control`-style — do NOT delete SALT or the medium). Money
   never emerges (`current_money_good()` stays `None`). Emergence is earned, not
   structural-by-accident.
5. `producers_hold_free_gold_at_promotion` (Tension B) — via the S8.0 probe, ≥1
   latent/adopting producer has positive converted gold AND free (non-reserved) gold
   able to fund an input bid at the promotion tick — earned in barter, not endowed.
6. `inputs_acquired_by_market_trade_after_promotion` — THE cutover metric: after the
   promotion tick there exist real `Society::trades` where an active Miller/Baker buys
   grain/flour from a different seller, later consumed as a recipe input — the chain
   survives the barter→money discontinuity.
7. `specialization_sustains_on_emerged_money` — `bread.made > 0` through tick 800 and
   1600, on emerged SALT money (not designated gold).
8. `hunger_bounded_under_coemergence` — hunger mean/p95 bounded and non-drifting over
   tail windows, no curated placement (a modest residual tail is allowed — S6
   provisioning-at-scale is deferred to S9).
9. `capital_forms_on_emerged_money` — at least one mill/oven is **built** (`produced`)
   **after promotion** under emerged-money prices, conserving (the S7 build runs on the
   emerged unit).
10. `coemergence_conserves` — whole-system conservation every tick, including the
    promotion sink (`report.promoted`) and all chain/capital flows.
11. `goldens_unchanged` — the G5a/G5b emergence goldens and the six econ conformance
    goldens are byte-identical; S5/S6/S7 suites green with their (designated) configs
    unchanged; new digest knobs have `canonical_bytes_include_*` regressions; clippy
    `-D warnings`; fmt `--check`.

(If the milestone lands the principled-failure path instead, tests 6/7/9 are replaced
by the diagnostic test that asserts the observed failure mode — see the slice note.)

Manual: `cargo run -p viewer -- run coemergent --ticks 1600` (watch the era column go
barter→money, then bread sustain; compare to `endogenous`).

## Missing Interactions (the central risks — track explicitly)

- **Tension A (subsistence ⊣ monetization) is the make-or-break, and it is why S8
  keeps the grain floor OFF.** Money emerges from the extent of exchange; the
  raw-grain floor (and the S6 re-entry coupled to it) would thin bread-for-SALT trade
  and starve promotion (Base Fact 6). S8 sidesteps this by NOT provisioning at scale —
  producers are fed by the local `producer_subsistence` hearth (which does not touch
  bread demand) and consumers buy bread. The residual risk: with the floor off, do the
  non-lineage poor starve before money emerges? Use the S8.0 probe (hunger/critical
  streak vs promotion tick). If no parameter band across seeds both monetizes AND keeps
  the colony alive to promotion, that is the **principled failure** — land it (S9, with
  a real staged/produced-subsistence mechanism, is where this is properly solved).
- **Tension B (the cutover freeze).** Producers must hold free gold the tick after
  promotion. Earned (sold-into-barter) working capital is the faithful source; verify
  it is not a disguised endowment. If the chain freezes at promotion, report it.
- **SALT must still win.** The candidate set is `[WOOD, bread, SALT]` and every
  renewable chain/hearth/node good is auto-rejected (`:3696-3726`); shifting exchange
  onto money-priced input purchases can thin SALT's barter breadth — keep SALT's
  universal-want / concentrated-holder asymmetry (`medium_want_qty`,
  `consumer_medium_endowment`) intact.
- **No hidden money.** The barter overlay's zero-money asserts (`:3398-3425`) are the
  tripwire; do not reintroduce gold endowments to "help" the cutover.

## Handoff Notes

- **The post-promotion phases are money-good-agnostic, so the work is config
  composition + the two tensions + the S8.0 probe (NOT new appraisal logic).** Start
  `frontier_coemergent` from `frontier` (not `frontier_endogenous`) to inherit the
  emergent base, then layer the S5 sustain stack + S7 capital; do NOT set
  `cfg.barter = None`, do NOT seed gold, keep `subsistence_on_grain` and S6 re-entry
  OFF (Base Fact 6 — they would kill monetization and re-entry is inert anyway).
- **Tension B (earned working capital) may require adjusting WHO TRADES WHAT in barter**
  so latent producers actually earn SALT (the `frontier` hub concentrates SALT in
  consumers; producers earn only if their seeded bread/flour/WOOD sells). Adjust the
  trade structure, never hand producers money. The S8.0 probe is how you verify it.
- **Keep emergence machinery untouched** (`econ/src/menger.rs`, `promote_v2_money_good`,
  the rejection-list step) — additive/gated; the G5a/G5b goldens are the tripwire.
- **Honest reporting:** if it fails for a principled reason (no monetization window, or
  cutover freeze), land the negative result as a **passing diagnostic/control test**
  that asserts the observed failure mode (using the S8.0 probe), plus a written
  finding — exactly as the experiment log did. Do not force a pass with designated gold
  or curated advances, and do not leave the result as an ignored/failing test.
- **Digest:** any new co-emergence knob → `canonical_bytes` + a `canonical_bytes_include_*`
  regression; the emergent surface is already serialized (Base Fact 3).
- Build S8.0→S8.3 as separate commits with their own tests; `git add` new files.
- **Follow-ons:** **S9 — provisioning-at-scale under emergence** (a real staged or
  household-*produced* subsistence good that does not crowd out bread-for-SALT trade,
  so S6 re-entry can run on the emergent path); per-agent intertemporal capital choice
  / emergent time preference; re-enabled starvation selection; entrepreneurial
  uncertainty (forecast prices, not last realized).
