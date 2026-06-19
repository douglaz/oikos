# Implementation Spec: strong-bar money emergence (S9)

> S8 removed designated gold — money, the chain, and capital co-emerge from a barter
> start. But Codex's review found a remaining circularity: SALT promotes because every
> colonist is given a **configured universal medium want** (`medium_want_qty`), i.e.
> agents want SALT *as a medium before SALT is money*. That assumes what it claims to
> explain. Menger/Mises run the other way (the regression theorem): a commodity with a
> real **direct use** and superior **saleability** comes to be accepted *indirectly*
> because actors learn it is easier to trade away. This milestone removes the
> pre-monetary medium want and makes SALT monetize only after real direct-use trade and
> real indirect-exchange volume.

## Purpose — this is an EXPERIMENT, not a guaranteed milestone

The question: **does money emerge from real saleability when no agent is configured to
want SALT as a medium?** On a new gated path: remove the configured universal medium
want; give SALT a modest, **heterogeneous** real direct (non-monetary) use; let SALT
accrue saleability from direct trades; and require promotion to need genuine
**indirect-exchange breadth** (enough trades, by enough distinct acceptors, for enough
distinct targets, where a side accepted SALT for an end *other* than SALT's own use).
Money should emerge **because actors discover SALT is the most marketable good**, not
because config says they want it — then the S8 chain + capital sustain on the emerged
unit.

**Both outcomes are valid, landed results — frame this as a discovery, not a forced
pass.** It may turn out that with no configured want, SALT never accrues enough
direct-use saleability to become the provisional medium, or a *different* commodity
wins. Each is a real finding ("no commodity monetizes without a pre-monetary
direct-use/saleability structure", or "the best-direct-use good wins") to be landed as
a **passing diagnostic** + a written conclusion — never papered over by restoring the
medium want or designated gold. The DoD is "we learned which of these happens, honestly
and reproducibly", not "SALT must promote".

NOT a change to existing emergence behavior (`barter_camp`/`frontier`/`frontier_coemergent`
and the g5a/g5b/coemergence goldens stay byte-identical — strictly additive + gated),
NOT designated/seeded money, NOT firms/credit, NOT provisioning-at-scale (still S10).

## Verified Base Facts (oikos @ `e630d3a`)

1. **The circular medium want is localized and cleanly removable.** `medium_scale_extension`
   (`sim/src/settlement.rs:9506-9522`) appends `medium_want_qty` copies of
   `WantKind::Good(SALT) / Horizon::Next` to each scale, called from `regenerate_scales`
   only pre-promotion (`:5726-5730`). `medium_want_qty = 0` (or unsupplied medium) makes
   the want simply absent (`:9507`, `:4165-4169`); the S8 control already does this
   (`sim/tests/money_coemergence.rs:39`) and nothing monetizes. (A separate `Later`
   savings want also binds to SALT, `:3446/3468/3485`, but "a `Later` savings want alone
   never barters" `:3462` — it does not drive monetization.)
2. **A direct SALT use is a FIXED, HETEROGENEOUS `Good(SALT)/Now` want — NOT a new
   need.** `WantKind` is only `Good(GoodId) | Leisure` (`econ/src/agent.rs:52-64`); a
   direct-consumption want is `Good(g) / Horizon::Now`. A *real need* (like warmth) is
   the heavy path: `NeedState` has only `hunger, warmth, rest` (`life/src/need.rs:13`),
   so a SALT need would require new `NeedState` + `NeedDynamics` + `NeedIntake` + scale
   generation + intake-update plumbing. **Avoid that.** Use the lighter path: a gated,
   **fixed** direct-service want — a modest `Good(SALT)/Now` want injected like the
   medium extension (`medium_scale_extension` pattern, `:9506`) but representing
   *consumption*, given to **only a heterogeneous subset of colonists** (Base Fact 6 —
   not universal). It is consumed by the existing `Horizon::Now` arm of
   `consume_now_wants_with_allocations` (`econ/src/agent.rs:720-743`) and booked into the
   existing `consumed` bucket — no new bucket, no consume-phase change. Call it a fixed
   direct-service want, not a "need ladder" (there is no life-feedback state behind it).
3. **SALT is a clean slate.** `SALT = GoodId(4)` (`econ/src/good.rs:10`) has zero direct
   use today; it is endowed, non-renewable, non-spoilable, barred from being a node/chain/
   hearth good (`sim/src/settlement.rs:3503-3531`) — which is what lets promotion convert
   its whole stock 1:1. An added direct use is purely additive and is exactly the
   "real direct use" that replaces the circular want.
4. **Indirect acceptance is ALREADY tagged on every trade — but discarded.**
   `BarterReason::{DirectWant, IndirectFor { target }}` (`econ/src/barter.rs:8-18`) rides
   on `BarterTrade.a_reason`/`b_reason` (`:39-40`), stamped at clear time
   (`:219-228`). An indirect offer is generated only when the agent does NOT directly
   want the leader and the leader is re-tradable for a good it does want
   (`econ/src/society.rs:2090-2144`, `econ/src/agent.rs:477-515`). **But
   `SaleabilityTracker::observe_trade` (`econ/src/menger.rs:124-128`) ignores the reason**
   — it counts only undifferentiated acceptances, distinct acceptors, counterpart goods
   (`CandidateStats`, `:64-70`). So the data to require indirect volume exists at clearing
   and is simply not recorded.
5. **No promotion threshold requires real indirect exchange.** `MengerianConfig`
   (`econ/src/money.rs:88-113`) and `base_eligible` (`menger.rs:212-217`) gate on total
   acceptances / share / breadth / lead / stability only. `indirect_min_acceptance_share_bps`
   (despite the name) is NOT an indirect-volume gate — it only sets the share floor for a
   good to become the `provisional_leader` that agents may accept indirectly
   (`menger.rs:171-184`). So a genuine "K indirect acceptances" promotion gate must be added.
6. **The indirect-offer machinery works under `medium_want_qty=0` — BUT a *universal*
   direct want would suppress it (the key design constraint).** Once SALT is the
   provisional leader (reached via its DIRECT trades), agents accept it *indirectly*
   (`society.rs:2090-2144`) without the medium want. HOWEVER `generate_indirect_barter_offers`
   **skips any agent whose unsatisfied targets include the leader** (`society.rs:2103`):
   an agent that *currently directly wants* SALT will not post an `IndirectFor` offer for
   it. So if the direct SALT use were universal and recurrent, everyone would directly
   want SALT while it leads and **nobody would accept it indirectly** → no indirect
   volume → no strong-bar promotion. Therefore the direct use MUST be **heterogeneous /
   satiable**: some colonists directly want SALT (seeding its saleability), while *others*
   — currently without an unsatisfied SALT want but wanting another good — accept SALT
   instrumentally to re-trade. Heterogeneity is what both (a) enables indirect acceptance
   and (b) prevents the direct want from becoming the new circularity. The Mengerian
   chain (heterogeneous direct use → saleability → provisional leader → indirect
   acceptance by the others → breadth gate → promotion) is *wireable*, but only with this
   heterogeneity; the tracking + the gate are also still missing (Base Facts 4-5).
7. **All additive/gated keeps goldens byte-identical.** New `BarterConfig`/`MengerianConfig`
   fields default inert; a new builder derives from `frontier_coemergent` (the
   derive-don't-mutate rule the `goldens_unchanged`/`econ_unchanged` tests enforce —
   `money_coemergence.rs:696-755`, `g5a_emergence.rs:391-437`). New tracker/config fields
   must be added to `canonical_bytes` (`push_emergence_runtime_bytes` `:10165-10198`,
   `push_mengerian_config_bytes` `:10200-10212`, `push_barter_config_bytes` `:9560-9567`)
   with regression tests.

## The slices (build in order; each independently testable)

- **S9.1 — SALT's heterogeneous direct use (the real seed).** Add a gated, modest,
  **fixed direct-service `Good(SALT)/Now` want** (NOT a new need — Base Fact 2) injected
  like the medium extension but for *consumption*, given to **only a heterogeneous subset**
  of colonists (Base Fact 6 — universal would suppress indirect acceptance), consumed into
  the `consumed` bucket, active **only pre-promotion** (gate like the medium want at
  `:5726`; post-promotion SALT is delisted to money). Add inert `BarterConfig` field(s)
  (`salt_direct_use_qty` + a way to make it heterogeneous, e.g. a fraction/which-vocations,
  default off) + digest bytes. **Test:** with the direct use on and `medium_want_qty=0`,
  SALT is traded **directly** pre-promotion (`BarterReason::DirectWant`), consumption is
  booked and conserves, AND at least some colonists do NOT currently want SALT directly
  (so they remain eligible to accept it indirectly); default → existing scenarios
  byte-identical.
- **S9.2 — record indirect acceptances + a BREADTH gate (not just a count).** Extend
  `observe_trade`/`observe_acceptance` to read `a_reason`/`b_reason` (pair each accepted
  good with that side's own reason — Base Fact 4) and record per-candidate
  `indirect_acceptances`, **distinct indirect acceptor agents, and distinct indirect
  targets**. A raw count is gameable by repeated trades among a few agents (Codex), so the
  gate must require breadth. Add to `MengerianConfig` (inert defaults): `min_indirect_acceptances`,
  `min_indirect_acceptor_agents`, `min_indirect_target_goods`, enforced in `base_eligible`.
  Add all to the digest. **Test:** the leader's indirect acceptances/acceptors/targets are
  recorded; with the breadth thresholds set, promotion is withheld until genuine breadth
  accrues, and a repeated-pair run does NOT satisfy them; defaults → goldens byte-identical.
- **S9.3 — the strong-emergence scenario + DoD/experiment.** Add `frontier_coemergent_strong`
  (derive from `frontier_coemergent`): `medium_want_qty = 0`, heterogeneous SALT direct use
  on, the indirect breadth gate on. **Test:** the clean metric below — and if it does not
  monetize, the principled-failure diagnostic.
- **If promotion does not fire:** land it as a **first-class passing diagnostic**
  (`strong_bar_does_not_monetize_without_configured_want`, via the S8.0 probe +
  `candidate_saleability`) + a written finding — do not restore the medium want or
  designated gold.

## Acceptance Tests (the S9.3 DoD) — `sim/tests/strong_bar_emergence.rs`

1. `strong_run_is_deterministic` — byte-identical `(seed, config)` through the arc.
2. `no_medium_want_and_salt_has_direct_use` — the setup honesty check:
   `frontier_coemergent_strong` has `medium_want_qty == 0` AND a non-zero SALT direct-use
   need; no designated money, zero gold at generation.
3. `salt_is_traded_directly_before_it_monetizes` — pre-promotion there exist barter trades
   accepting SALT with `BarterReason::DirectWant`; AND some colonists do NOT directly want
   SALT (so the heterogeneity that permits indirect acceptance holds).
4. `promotion_requires_indirect_breadth` — the strong gate: SALT promotes ONLY after the
   indirect acceptances AND distinct indirect acceptors AND distinct indirect targets all
   clear their thresholds; a variant with the breadth thresholds high does not promote at
   the tick the weak bar would; a repeated-pair micro-run does not satisfy the breadth gate.
5. `money_emerges_then_chain_sustains` — SALT promotes (promoted good IS SALT), the chain
   waits on money, then bread sustains at a real rate through t1600 on the emerged unit,
   ≥1 tool built after promotion — conserving, deterministic.
6. `no_indirect_acceptance_control_does_not_monetize` — a clean, separate control: a gated
   `allow_indirect_acceptance = false` (NOT lowering the leader floor, which would disable
   leadership itself). Prove SALT still reaches provisional leader and has direct trades,
   but with indirect acceptance off it does NOT monetize.
7. `no_direct_use_control_does_not_monetize` — remove SALT's direct use AND the medium want:
   with no demand for SALT at all, SALT does NOT monetize (`current_money_good()` None).
8. `alternate_winner_is_a_valid_outcome` (diagnostic, only if applicable) — if a good other
   than SALT promotes, assert that winner ALSO satisfies the indirect-breadth gate (a real
   Mengerian winner, not an artifact); document it.
9. `strong_emergence_conserves` — conservation every tick incl. SALT direct-use
   consumption and the promotion sink.
10. `goldens_unchanged` — g5a/g5b/coemergence emergence goldens + the six econ conformance
    goldens byte-identical; S5–S8 suites green; new digest fields have
    `canonical_bytes_include_*` regressions; clippy `-D warnings`; fmt `--check`.

(EXPERIMENT framing: if SALT does not monetize, tests 4/5/8 are replaced by
`strong_bar_does_not_monetize_without_configured_want` — a passing diagnostic that
records, via the probe + `candidate_saleability`, that no commodity cleared the breadth
gate. That is a valid landed result.)

Manual: `cargo run -p viewer -- run strong-emergence --ticks 1600`.

## Missing Interactions (track explicitly)

- **The narrow band (the central design tension): heterogeneity must be enough for SALT to
  (a) accrue saleability yet (b) leave enough non-wanters to accept it indirectly.** Too
  *universal* a direct want → everyone wants SALT directly → nobody posts `IndirectFor`
  (Base Fact 6) → no indirect breadth → no promotion. Too *sparse* → SALT never reaches the
  provisional-leader share floor against food/WOOD barter → no indirect offers at all → no
  promotion. The faithful band between "enough direct demand to lead" and "enough
  non-wanters to re-trade it" may be narrow or empty — that is the experiment. Tune the
  heterogeneous direct-use fraction/intensity; if no band works across seeds, land the
  principled finding.
- **A different commodity could win.** With a real direct-use field, whichever candidate
  has the best direct-use saleability promotes — it might not be SALT. That is a *more*
  authentic outcome, not a bug; assert and document which good wins.
- **SALT direct use is pre-promotion only** (post-promotion SALT is delisted to money).
  Disclose this honestly: real commodity money retains its non-monetary use; here the
  engine collapses SALT-the-good into money at promotion, so the direct use is the
  pre-monetary saleability seed and ends at monetization (it is NOT the circular
  want-it-as-money demand — it is a real consumption need).
- **Conservation of SALT consumption.** Direct use removes SALT from the closed stock
  pre-promotion (booked `consumed`); promotion then converts the *remaining* stock 1:1 —
  both conserve. Verify `conserves()` every tick across direct-use + promotion.
- **Digest.** EVERY new field steers future behaviour → `canonical_bytes` + regressions
  (Base Fact 7): `salt_direct_use_qty` and its **heterogeneity selector/fraction**,
  `allow_indirect_acceptance`, `min_indirect_acceptances`, `min_indirect_acceptor_agents`,
  `min_indirect_target_goods`, and the per-candidate `indirect_acceptances` count + the
  distinct indirect-acceptor and indirect-target sets.

## Handoff Notes

- **Mostly additive — the machinery exists.** `IndirectFor` is already tagged
  (`barter.rs:8-18`); `observe_trade` just needs to read it. SALT's direct use is a
  **fixed, heterogeneous `Good(SALT)/Now` want** (injected like `medium_scale_extension`
  but for consumption), NOT a new `NeedState` need (Base Fact 2) — keep it light.
- **Derive-don't-mutate:** `frontier_coemergent_strong` from `frontier_coemergent`; default
  every new field inert so g5a/g5b/coemergence + econ goldens are byte-identical (the
  tripwire tests already enforce this).
- **Keep the Mengerian causality intact:** direct use → saleability → provisional leader
  → indirect acceptance → K indirect trades → promotion. Do NOT re-add a want that makes
  agents desire SALT as money before it is money.
- **Honest reporting:** if it does not monetize (or a different good wins), that is the
  deliverable — a passing diagnostic + finding, never a forced pass.
- Build S9.1→S9.3 as separate commits with their own tests; `git add` new files.
- **Follow-ons:** S10 provisioning-at-scale under emergence; per-agent intertemporal
  capital / emergent time preference; re-enabled starvation selection; entrepreneurial
  uncertainty.
