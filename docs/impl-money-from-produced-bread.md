# Implementation Spec: money from PRODUCED bread (S16) — closing the S12 finding

> The keystone. S12's deep analysis showed strong-bar money emergence (S9) was **not
> provision-autarkic**: SALT monetized against a **minted** demographic bread stream — the
> minted hearth was the load-bearing *supply* (counterparty) the SALT-holding consumers
> circulated against. S13→S15 built the missing piece: a spatial reproducing population
> (S13) that, under a real forage carrying capacity (S14), **cultivates bread by its own
> labor** (S15). S16 closes the loop: let that **produced** bread be **traded** for SALT, so
> **money emerges against produced (not minted) bread** — retiring the scaffold and earning
> the full-stack claim S9 could not. Either it works (produced supply monetizes SALT) or it
> is a deeper, characterized finding (produced surplus is insufficient/mistimed) — both are
> first-class.

## What S13–S15 + the research established

- **The money-emergence machinery exists** (`frontier_coemergent_strong`, `settlement.rs:3045`):
  consumers hold the SALT medium endowment (`consumer_medium_endowment 80`, seeded by vocation
  at `:11334`), a heterogeneous SALT direct-use seeds pre-money saleability
  (`salt_direct_use_*`), and the strong-bar Mengerian gate promotes on indirect breadth
  (`min_indirect_acceptances 12`, `min_indirect_acceptor_agents 6`, `min_indirect_target_goods 1`).
- **The minted bread is the scaffold to retire**: pre-promotion bread = seeded `bread_buffer`
  (`:2400`) + demographic `food_provision` + `producer_subsistence`, gated off by
  `!own_labor_subsistence_can_run()` (`:6525`, `:7079`).
- **The composition gap (the crux).** S14/S15 *strip* consumers (`gatherers=0, consumers=0`,
  `:3205-3206`) for isolation; the coemergent scenario keeps consumers but mints bread. S16's
  scenario must keep **both** — SALT-holding consumers AND cultivating spatial lineages (S13)
  with the mint OFF — so the only bread is **produced**. This is a scenario-composition choice,
  not a hard constraint.
- **The surplus→market seam already half-exists.** S15 cultivation produces to a labor budget and
  consumes to satiety, leaving **surplus bread free in stock** (`:7376`); `generate_direct_barter_offers`
  reads `positive_goods` as give-goods (`econ/src/society.rs:2028`), so surplus bread is offered for
  the cultivator's *normal* unsatisfied wants via the **existing S9 direct/indirect barter** — no
  special SALT want (Base Fact 7). The own-use consume runs after the market (`:5506`) and drains
  only `free_stock_after_all_reserves`, so the boundary is conserved.
- **Provenance needs a stock-origin ledger** (closing the S12 caveat): role/`cultivating`-state at
  trade time is unsound (bread is produced post-market and sold later, Base Fact 8) — so a per-agent
  **produced-bread balance** (credited on `produced`, debited when bread leaves stock) is what
  classifies a bread→SALT trade **produced vs minted**, not `vocation_of_id`.

## Purpose & the honest bar

On a gated scenario composing the coemergent SALT-holders + S13 spatial lineages + S14 forage
commons + S15 cultivation, with the **minted bread OFF** (own-labor path, buffer absent/
exhausted): let cultivators sell their **surplus produced bread** for SALT, and test whether
**SALT monetizes against produced bread**. Success = (a) SALT **promotes** under the strong-bar
gate, AND (b) a **seller-provenance trace** shows the pre-promotion bread-for-SALT volume is
**dominantly from cultivators** (produced), not minted/residual — i.e. money emerges against
endogenously produced bread, closing the S12 finding. Honest target: **test whether** produced
bread can do what minted bread did.

Three outcomes, all first-class (Codex): **(1) success** — SALT promotes AND the provenance ledger
+ breadth show produced bread drives it (closes S12); **(2) principled failure** — SALT does NOT
promote with only produced bread (surplus too small/late, or the division of labor doesn't form):
land `produced_bread_does_not_monetize` with the characterized reason (volume/timing/provenance),
sharpening S12; **(3) the false success** — SALT promotes but via WOOD/forage breadth with produced
bread **incidental**: this is a **FAILURE of the S16 claim**, caught by the causality test
(`produced_bread_drives_the_promotion_not_incidental`), NOT a pass. Do NOT rescue any of these by
re-minting bread.

NOT mortality / the positive check (later), NOT a new monetization mechanism (reuse the S9
strong-bar gate), NOT a change to existing emergence/goldens. Additive + gated; flag off →
S5–S15 + all goldens byte-identical.

## Verified Base Facts (oikos @ `ee0f446`)

1. **Strong-bar emergence** (`frontier_coemergent_strong`, `settlement.rs:3045`): consumer SALT
   endowment (`:11334`), `salt_direct_use_*` seeding pre-money saleability, the Mengerian
   thresholds (`:3062-3064`), `medium_want_qty 0` (no circular want). The promotion +
   `bread_for_salt_volume` + `promoted_at_tick` probe (`:10372`).
2. **Minted bread gating**: `deliver_demography_provisions` (`:6515`) + `run_producer_subsistence`
   (`:7066`), both gated off by `!own_labor_subsistence_can_run()` (`:6525`, `:7079`); seeded
   `bread_buffer` (`:2400`).
3. **The composition is a scenario choice**: S14/S15 strip consumers (`:3205-3206`); nothing
   prevents a scenario that keeps consumers AND makes lineages spatial+cultivating.
4. **The surplus seam**: S15 surplus stays free in stock (`:7376`); offers read `positive_goods`
   (`econ/src/society.rs:2028`); the cultivator offers surplus for its normal wants (no special SALT
   want — Base Fact 7); own-use consume is post-market + reserve-aware (`:5506`,
   `free_stock_after_all_reserves`).
5. **Provenance is a stock-origin balance, not a role lookup**: `BarterTrade{a, b, a_gives,
   b_gives, …}` records only sides/goods (`econ/src/barter.rs`); `bread_for_salt_volume` does NOT
   record the seller (the S12 caveat). The fix is a per-agent produced-bread balance (Base Fact 8),
   not `vocation_of_id`/`cultivating`-state at trade time.
6. **Conservation/gating/determinism** as established; the emergence accessors; the
   `own_use_cultivation` / `own_labor_subsistence` / `spatial_households` / `forage_commons`
   flags compose.
7. **Strong-bar SALT use is heterogeneous (Codex P1b).** Selected agents *directly* want SALT;
   non-selected agents accept it **indirectly** (`IndirectFor{target}`), and indirect offers skip
   agents that directly want the leader (`econ/src/society.rs:2102`). A non-SALT-wanter accepting
   SALT for bread is the Mengerian result — so cultivators need NO special SALT want; produced-bread
   trades must contribute to **indirect breadth**.
8. **S15 bread is produced post-market and sold later (Codex P1a).** `run_own_use_cultivation` runs
   after the market (`settlement.rs:5506`); surplus persists in stock and is sold on a *subsequent*
   tick when `cultivating` may be false — so provenance must be a **stock-origin balance**, not a
   role/state lookup at trade time. `BarterTrade` records only sides/goods (`econ/src/barter.rs:31`);
   the bread seller is `a_gives == bread ? a : b`.
9. **Spatial eligibility currently includes consumers (Codex P1c).** With `spatial_households` on,
   `spatial_member = household.is_none() || spatial_active` (`settlement.rs:7203`) makes
   Consumer/Gatherer/Unassigned forage/cultivate-eligible — including the seeded SALT consumers S16
   needs as buyers. S16 must scope cultivation to **lineage** spatial members so consumers stay the
   buy side.

## The slices (build in order; each independently testable)

- **S16.1 — the cultivated-bread→market surplus seam + the buy/sell split.** On a scenario with
  SALT-holders, let a cultivator that has eaten to satiety leave its **surplus produced bread free
  in stock** so it enters the existing barter via `positive_goods` (`econ/src/society.rs:2028`) —
  gated by a new `cultivation_sells_surplus` flag (default off). Do **NOT** add a special SALT
  want to cultivators (Codex P1b — that recreates circular demand): reuse the **existing S9
  direct/indirect barter** — the cultivator offers surplus bread for its *normal* unsatisfied wants
  (warmth, savings, …), SALT-direct-use consumers accept bread for SALT (`DirectWant`), and
  non-SALT-wanters accept SALT *indirectly* (`IndirectFor{target}`, `society.rs:2102`) — the
  Mengerian breadth. Surplus is offered (not posted as a reserving ask); the own-use consume stays
  reserve-aware (conserved). **The buy/sell split (Codex P1c):** scope cultivation/forage
  eligibility to **lineage spatial household members** (`household.is_some() && spatial_active`),
  so the **seeded SALT consumers stay the goods-poor BUY side** (they do NOT self-cultivate). **Test:**
  a satiated cultivating *lineage* member leaves surplus bread that is offered and traded for SALT
  (bread leaves stock, SALT enters); a SALT-holding *consumer* buys bread and does NOT cultivate
  (the buy-side holds); conserved; flag off → byte-identical.
- **S16.2 — the produced-bread provenance LEDGER (stock-origin, not role).** Role/`cultivating`-
  state at trade time is NOT sound (Codex P1a): S15 produces bread post-market and it is sold a
  *later* tick when `cultivating` may be false; a consumer could resell bought bread. Instead add a
  **per-agent produced-bread provenance balance** (two conserved counters per agent: produced-origin
  bread vs other-origin bread) — credited to the produced counter when `run_own_use_cultivation` (or
  a chain producer) books bread `produced`, and **debited FIFO** (produced-origin first, in
  stock-removal order) when that agent's bread leaves stock (own-use consume, birth endowment, or
  **sale**). A bread→SALT trade's bread is attributed **produced** to the extent the seller's debit
  draws from its produced counter, else **minted/residual** (seeded buffer / hearth). Pin the rule —
  FIFO produced-first, deterministic; no "proportional" ambiguity. Identify the bread seller correctly:
  `a_gives == bread ? trade.a : trade.b`. Expose a `bread_for_salt_volume_by_provenance`
  (produced vs minted). **Test:** in a minted-bread scenario the ledger attributes the bread→SALT
  volume to **minted**; in the S16 produced-bread scenario it attributes it to **produced**; a
  resold-bought-bread case is NOT mis-attributed to produced — the ledger distinguishes origin, not
  current role (closing the S12 caveat: provenance, not just volume).
- **S16.3 — the co-emergence scenario + DoD.** `frontier_money_from_cultivation` (coemergent
  SALT-holders + S13 spatial lineages + S14 forage commons + S15 cultivation +
  `cultivation_sells_surplus`, minted bread OFF, `bread_buffer` absent/minimal). Register the
  `money-from-cultivation` viewer scenario. **Test:** the acceptance suite below.

## Acceptance Tests (the S16.3 DoD) — `sim/tests/money_from_produced_bread.rs`

1. `money_from_cultivation_run_is_deterministic` — byte-identical `(seed, config)`.
2. `salt_monetizes_against_produced_bread` — **the core claim**: in the scenario where the only
   bread is produced (minted off, buffer absent), SALT **promotes** (the promoted good IS SALT)
   under the strong-bar gate, with material pre-promotion bread-for-SALT volume.
3. `the_monetizing_bread_is_produced_not_minted` — the **provenance ledger** shows the
   pre-promotion bread-for-SALT volume is **dominantly produced** (drawn from cultivators'
   produced-bread balance); the **minted contribution is provably zero** (mint off) and the seeded
   buffer is negligible/absent; a resold-bought-bread unit is NOT mis-attributed. This is what
   closes the S12 finding.
4. `produced_bread_drives_the_promotion_not_incidental` — **causality (Codex P2 + the third
   outcome)**: produced-bread→SALT trades are **material before `promoted_at_tick`**, AND bread is
   actually **in the indirect-acceptance breadth** that fires the strong-bar gate (bread appears as
   an `IndirectFor{target=bread}` / direct bread-for-SALT path in the promoting saleability, not
   just WOOD/forage with bread incidental). If SALT promotes but produced bread is incidental,
   that is a **FAILURE of the S16 claim** (the third outcome), caught here — not a pass.
5. `cultivation_division_of_labor_forms` — under forage pressure, spatial **lineages** cultivate a
   surplus and sell it; SALT-holding **consumers buy** bread and do **NOT self-cultivate** (the
   buy/sell split, Codex P1c) — a produced-bread market forms (not just own-use); sale volume is
   non-trivial and sustained pre-promotion.
6. `money_from_cultivation_conserves` — whole-system conservation every tick (grain regen the
   source; grain `consumed_as_input` → bread `produced` → consumed/traded; no minted food,
   `endowment[staple] == 0`).
7. `controls_close_the_finding` — (a) **re-enable the minted bread** → reproduces old S9/S12
   behavior (proving the new path doesn't secretly depend on the mint); (b) **disable cultivation**
   (forage only) → SALT does NOT monetize (the S12 finding — no produced supply). These bracket
   "produced supply is what monetizes". (No "no-SALT-want" control — that misreads the strong-bar
   mechanism: non-wanters accepting SALT indirectly IS the result.)
8. `goldens_unchanged` — with the S16 flags off, S5–S15 scenarios + the six econ +
   g5a/g5b/coemergence + demographic (`lineages`) goldens byte-identical; S5–S15 suites green;
   new state (the `cultivation_sells_surplus` flag, the provenance trace state) in
   `canonical_bytes` with `canonical_bytes_include_*` regressions; clippy `-D warnings`; fmt
   `--check`.

(Principled-failure path: if SALT can't monetize against produced bread at any setting, land
`produced_bread_does_not_monetize` with the characterized reason — the provenance trace + the
volume/timing — sharpening, not faking, the S12 finding.)

Manual: `cargo run -p viewer -- run money-from-cultivation --ticks 3000`.

## Missing Interactions (the central risks)

- **The composition must actually produce a tradable surplus in time.** SALT promotion needs a
  *sustained pre-promotion* bread-for-SALT volume over the long strong-bar window (S12's lesson).
  The cultivation surplus must be large enough and early enough — if cultivators barely feed
  themselves (subsistence, no surplus), there's nothing to sell and S16 fails (the finding). The
  forage pressure + grain abundance + labor budget must leave a real surplus.
- **The division of labor must form**: lineages cultivate (under forage pressure), consumers buy
  (they hold SALT, don't cultivate). If consumers also cultivate (foraging/cultivation eligibility
  includes them), no one needs to buy bread and SALT doesn't circulate — scope eligibility so
  there's a buy-side. (Echoes the S12 demand/supply balance.)
- **Provenance must be honest** (Codex's S12 caveat): the trace must actually attribute the
  monetizing volume to produced bread, not assume it — `the_monetizing_bread_is_produced_not_minted`
  is the tripwire. Minted contribution must be provably zero (mint off), buffer negligible.
- **Own-use vs sale boundary stays conserved**: surplus is sold only after eating to satiety, only
  free (unreserved) stock, never double-counted (eaten AND sold). Reuse the S15 reserve-aware
  consume.
- **No new monetization mechanism** — reuse the S9 strong-bar gate unchanged; S16 only changes
  the bread *supply* (produced not minted), not how SALT promotes. If the gate needs tuning to
  promote on produced volume, that's a finding about timing, not a new mechanism.
- **Determinism.** The `cultivation_sells_surplus` flag + the provenance trace enter
  `canonical_bytes` only on the flag-on path; flag-off byte-identical.

## Handoff Notes

- **This closes the arc's central question** — money emerges against PRODUCED bread, retiring the
  minted-bread scaffold S12 exposed. Reuse everything: the S9 strong-bar gate + emergence probe,
  the S13 spatial lineages, the S14 forage commons, the S15 cultivation + surplus-in-stock, the
  barter offer generation. The new work is the surplus→market offer (existing direct/indirect
  barter), the produced-bread provenance ledger, the lineage-only cultivation eligibility, and the
  composed scenario.
- **Honest three-way DoD**: (1) SALT monetizes *via produced bread* (success, closes S12); (2)
  `produced_bread_does_not_monetize` (sharpens S12 — own-use surplus insufficient/late); (3) SALT
  monetizes but produced bread is *incidental* (a FAILURE of the S16 claim, not a pass). The
  provenance ledger + the breadth-causality test separate (1) from (3). Do not fake (1) by re-minting.
- **Don't tune the labor budget until it passes (Codex).** Instrument: first produced surplus tick,
  first produced-bread→SALT trade tick, `promoted_at_tick`, produced-surplus inventory over time,
  and SALT distribution by role. **Sweep** the cultivation labor budget / grain flow and **report
  the sensitivity** — a genuine success is robust across a band; a knife-edge that only passes at one
  budget is really outcome (2) or (3).
- **The provenance ledger is the proof** — volume alone repeats the S12 ambiguity; the stock-origin
  attribution (not role-at-trade-time) is what earns the claim.
- **Gate everything** (`cultivation_sells_surplus` default off, composed on S13–S15's flags) so
  S5–S15 + all goldens stay byte-identical; the `lineages` golden is the tripwire.
- Build S16.1→S16.3 as separate commits with their own tests; `git add` new files.
- **Next:** mortality (the Malthusian positive check) on the now fed-and-monetized-by-produced-
  bread colony — the last praxeology piece.
