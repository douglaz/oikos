# impl-61 — P1.6 / C1S: Tenure Succession (does an heir assuming the estate's live contract turn transient clearing into standing tenure?)

Status (spec): **SPEC-READY** (Codex xhigh: round 1 NEEDS-REVISION [no P0; 2 P1 — the same-tick worker-death
liveness guard, and a succession-SPECIFIC classifier keyed on `final_open_succeeded`/`post_succession_renewals`
over the `succeeded_live_ids` set rather than colony-global renewals — plus withholding the pending contract
during finalize and explicit in-kind/partible/worker-death scope]; round 2 all RESOLVED, no new P0/P1 →
SPEC-READY, the renewal-hint data shape + `succeeded_live_ids` removal rule polished in). The probe the
whole wage/share arc points at:
C1R (share), P1.5 (forward-provisioning), and C1N (in-kind wage) all **clear then fade**, and P1.5
*measured* the dominant fade cause — contracts die at the **owner-death seam** before reaching a renewal
decision (term=24: 1,172 contracts, 55 live-live expiries). Succession attacks that wall directly: when a
mortal **owner** dies mid-contract, instead of the contract dissolving, the **heir who inherits the plot
assumes the owner side** — *if* the heir re-consents (the plot is still worth sharing rather than working
itself) and the worker re-consents. Build base: branch **`feat/in-kind-wage-impl-rb` @ `42dbbc8`** (the
decomposed C1/C1R/P1.5/C1N stack). Flag **`share_contract_succession`** (bool, composes on `share_tenancy`),
gated `share_contract_succession_active() = flag && share_tenancy_active()` (the `share_forward_provisioning`/
tag-24 precedent). Digest **tag 26** (code-verified free) — **flag byte only**: a succeeded contract is an
existing `ShareContract` with `owner = heir`, already digested under tag 23, so no new digest fields (the
tag-24 shape exactly).

Falsifiable bar (headline): P1.5 found tenure is mortality-bounded and named succession as the missing
precondition. Does letting the contract **survive the owner's death** — reaching a renewal decision under
the heir instead of dying at the seam — turn the transient/rotational institution into **standing tenure**
(contracts persisting into the final window, with a survival lift), or does it still fade — revealing that
the death seam was *not* the binding constraint and the fade is the worker-side satiation ("fed out") P1.5
also measured?

## 0. One-paragraph summary

Every voluntary-labor result so far clears and then fades, and P1.5 decomposed the fade into two causes:
worker **satiation** (the fed-out renewal fate — a term's income covers the next term, so the worker exits
the pool) and owner **mortality** (the contract dies when the owner dies, before any renewal decision).
Succession removes the second: on a mortal owner's death, rather than dissolving the live share contract,
the heir who inherits the plot **assumes the owner side** of it. The move is a **voluntary re-formation**,
not a forced ratchet — the heir re-consents through the same cap-waste owner gate (the plot must still be
at-cap wasted regen the heir would rather share than work itself) and the worker re-consents through the
same bread-ordinal accept; either declining dissolves the contract as today. Mechanically it is a single
in-place rewrite of `contract.owner` (dead → heir) plus re-establishing the worker's plot reservation that
the land transfer wipes, with the pending grain left inside the continuing contract (no estate double-count,
byte-inert with the flag off). Because the contract's owner slot stays live, `expire_share_contracts` fires
a normal renewal hint under the heir — so a succeeded contract reaches the exact P1.5 renewal decision it
otherwise never reached. The probe then reads whether that produces **standing** tenure (final-window
contracts + survival lift) or whether the worker-satiation fade dominates regardless — either a first-class
finding about what a persistent economic institution actually requires on a mortal base. Classify-not-tune,
honest nulls first-class, scoped to the impartible single-heir regime (the base).

## 1. Base facts (verified on the decomposed branch)

1. **Owner death dissolves the contract, before the plot passes to the heir.**
   `settle_share_tenancy_for_death(dead)` (share_tenancy.rs:675–694) drops every contract with
   `owner == dead || worker == dead` (settles the grain to the dying owner's estate, clears the reservation,
   does not re-push). It runs at both death seams (starvation `settle_death` mod.rs:12982; old-age
   `age_and_remove_elderly` mod.rs:13990) **before** `transfer_private_land_on_death` (mod.rs:12983 / 14013).
   This is exactly the mortality-bound wall.
2. **The heir is resolvable at the seam and is a valid landowner.** The dying agent is `alive=false` but
   still in `society.agents` (removed only later in `collect_estate`), so `secure_land_universal_heir_for`
   (mod.rs:13477 — live children → same-household kin → household heir → colony next-of-kin) resolves the
   heir at the share seam. On the base's **Impartible** regime (`frontier_secure_land_tenure`, single
   universal heir) `transfer_secure_private_land_on_death` sets `record.owner = heir` and
   `record.reserved_for = None` (mod.rs:13628/13630). *Partible* fans a plot to multiple co-heirs — no clean
   single owner for one `ShareContract`, so succession is **scoped to impartible/single-heir** (partible →
   dissolve as today; disclosed).
3. **`contract.owner` has exactly 9 read sites, all fresh, all in `share_tenancy.rs` + 1 digest** — so an
   in-place `owner` rewrite is consistent everywhere with no cached copies:
   `expire_share_contracts` liveness (:102, **the mechanism** — heir keeps `owner_live=true`) and the
   renewal-hint key `(worker, owner, node)` (:122); `settle_share_contract_grain` credit (:185, skipped for
   succession); `split_share_output` bread + provenance + acquisition credits (:867/871/875, now credit the
   live heir); `share_plot_reserved_against_owner` owner-exclusion (:763); the dissolution branch (:681);
   and the tag-23 digest (mod.rs:23008). The reservation is **worker-keyed** and harvest admission is
   **worker+node-keyed** — both owner-agnostic.
4. **The land transfer wipes the reservation** (`record.reserved_for = None`, mod.rs:13628/13721/13867) —
   so a surviving contract's worker loses harvest admission unless succession **re-establishes**
   `reserved_for = Some(worker)` after the transfer.
5. **Owner deaths mid-contract are frequent** (`secure_land_owner_old_age_deaths_total` in the dozens–
   hundreds/run over 1,600 ticks vs 12-tick terms) — the wall is real and often hit.
6. **Tag 26 free**; the 7-site flag template is `share_forward_provisioning`; the suite harness
   (`sim/tests/share_tenancy.rs`) has the `ForwardProvisioning` verdict block as the exact 1:1 template for
   a `Succession` block, with final-window baselining + `survival_lift` + the renewal-fate map.

## 2. The central question and pre-named outcomes

**Central question.** On the mortal-landowner share base at the marginal (and swept) commons, across
`SEEDS=[3,7,11,19,23]`, when a live share contract's owner dies and — if heir and worker both re-consent —
the heir assumes the owner side instead of the contract dissolving: do contracts **survive owner deaths**,
**reach renewal decisions** under the heir, and **persist into the final window** (with a survival lift) —
the first standing cross-generational institution — or do they still fade, the worker-satiation exit
dominating regardless of who owns the plot?

**Ordered verdict enum** (first-match; slots into the suite's classifier after the ForwardProvisioning
block):

```
Preconditions (disqualifying):
  BaseUnviable        — extinct / the base fails to reproduce (owners must die mid-contract; the
                        forward-provisioning comparative must reproduce its P1.5 verdict)
  ConservationBroken  — goods / commons / money / provenance / renewal-fate consistency failed a tick
  RegistryBroken      — plot-registry / S23d owner-identity / reservation / no-double-contract invariants
Outcome ladder (all keyed on SUCCESSION-SPECIFIC signals, not colony-global renewals/open — Codex P1 #2):
  SuccessionInert           — no contract ever survives an owner death (successions_total == 0): owner
                              deaths never coincide with a live contract both parties re-consent to
  SuccessionButStillTransient — contracts DO survive owner deaths (successions_total > 0) but the SUCCEEDED
                              relationships still fade — final_open_succeeded < MIN_FINAL_OPEN AND
                              post_succession_renewals < MIN_RENEWALS: the death seam was not the binding
                              constraint; the worker-satiation fade dominates
  StandingTenureForms       — a SUCCEEDED relationship persists — final_open_succeeded ≥ MIN_FINAL_OPEN OR
                              post_succession_renewals ≥ MIN_RENEWALS (a contract that survived an owner
                              death is still open in the final window, or re-chose at renewal) — no lift
  StandingTenureLifts       — standing tenure AND a material survival lift over the same-seed no-contract
                              control — the death seam WAS the binding constraint; the arc's first standing
                              cross-generational institution
```

Every rung is first-class. Given P1.5 measured the fade as *mostly* fed-out satiation (99.2% of renewal
fates), `SuccessionButStillTransient` is a live and important possible result (succession fixes mortality
but not satiation); `StandingTenureForms/Lifts` would be the arc's turn. `SuccessionInert` guards against
the machinery never engaging.

## 3. Mechanism

### 3.1 The three-step succession at the death seam
Because dissolution runs *before* the plot→heir transfer and the transfer wipes the reservation, succession
is staged across the existing death-path calls (no reordering of the base calls):

1. **Tentative retain** (in `settle_share_tenancy_for_death`, share_tenancy.rs:675): split the owner/worker
   branch. **Worker death → dissolve as today.** **Owner death, `share_contract_succession_active`, the
   worker still live (`private_land_live_agent(worker)` — Codex P1 #1: if owner and worker die in the same
   batch and the owner is processed first, `contract.worker != dead` but the worker is already marked
   non-live; a dead worker dissolves as worker-death, never `worker_re_declined`), and the plot's regime is
   impartible/single-heir →** do *not* settle the grain, do *not* clear the reservation; **take the contract
   OUT of `self.share_contracts`** (the existing `std::mem::take` pattern) and record it as a **pending
   succession** `(contract, worker, node)`. Withholding it is load-bearing: `share_owner_candidate_plots`
   excludes plots already under a live contract (share_tenancy.rs:451), so the heir's cap-waste re-consent
   in step 3 would self-exclude the plot if the contract were still in the vec (Codex P2 #3). (Partible,
   dead worker, or flag-off → dissolve as today — byte-inert off.)
2. **Land transfer** (`transfer_private_land_on_death`, unchanged): the heir becomes `record.owner`;
   `record.reserved_for` is wiped to `None`.
3. **Finalize succession** (a new step *after* the transfer, in both death paths): for each pending
   succession, now that the heir owns the plot, evaluate **both re-consents**:
   - **Heir re-consent:** the plot passes the heir's cap-waste owner gate — `share_owner_candidate_plots`
     from the heir's perspective (at cap, regen>0, and **not** a plot the heir would work itself,
     `share_plot_currently_owner_targeted` / `private_land_target_for_agent`). An heir who would rather
     cultivate the plot declines (takes it over — the honest "reason to prefer working it").
   - **Worker re-consent:** `share_worker_accepts_bread(worker, bread, node)` — owner-agnostic, so this is
     "does the worker still accept this plot's share." (For a forward-provisioning cell the worker's
     outside-option gate applies as in P1.5.)
   - **Both re-consent →** rewrite `contract.owner = heir` in place, re-establish
     `record.reserved_for = Some(worker)`, **re-push the contract into `self.share_contracts`** as live,
     count `successions_total += 1`, and **add `contract.id` to the runtime `succeeded_live_ids` set** (the
     mark that makes the classifier succession-specific — §7). If the dead owner had itself been a heir of an
     earlier succession the id is already tracked; the mark is threaded through renewals (§3.2).
   - **Either declines →** dissolve now (settle the grain to the estate as today, clear the reservation,
     drop — the contract was already taken out in step 1, so "drop" = simply do not re-push), counting
     `heir_declined` or `worker_re_declined` (mutually exclusive per event).

### 3.2 What continues unchanged (the point)
The succeeded contract is an ordinary live `ShareContract` with `owner = heir`. Every downstream read is
fresh (§1.3): the next `split_share_output` credits the heir's bread + provenance + acquisition; the plot
stays reserved to the worker; harvest admission is worker+node-keyed. And critically
`expire_share_contracts` now computes `owner_live = private_land_live_agent(heir) = true`, so at term end it
fires a **normal renewal hint** `(worker, heir, node)` — the succeeded contract reaches the exact P1.5
renewal decision it otherwise never reached. Succession + forward-provisioning thus compose: the death wall
is removed and the renewal is decided by the term forecast.

**Threading the succeeded mark through renewals (for the succession-specific classifier).** A renewal
expires the succeeded contract (id X ∈ `succeeded_live_ids`) and `open_share_contract` re-forms it as a
**new** id Y. To keep the classifier measuring *succeeded relationships* rather than the colony-global
renewal count (Codex P1 #2), the mark is carried. **Data shape (polish):** the current renewal hint value
is a bare `u16` renewals count keyed `(worker, owner, node)` (RenewalHintKey, mod.rs:7246) — that does not
carry a flag, so either widen the hint value to a small struct `{ renewals: u16, from_succeeded: bool }` or
keep a sidecar `BTreeSet<RenewalHintKey>` of flagged keys. When `expire_share_contracts` consumes a
contract whose id ∈ `succeeded_live_ids`, **remove X from the set** (the contract is gone) and flag its
hint; when `open_share_contract` opens from a flagged hint, **insert the new id Y** and increment
`post_succession_renewals`. **Removal rule (polish):** an id leaves `succeeded_live_ids` on *every*
consumption of that contract — expiry (flagged renewal re-inserts Y; unflagged/declined does not),
death-dissolution, or decline/drop — so the set holds exactly the currently-live succeeded contracts, and
`final_open_succeeded` = |{live contracts} ∩ succeeded_live_ids| at the final-window snapshot (no stale ids,
no leak). An ordinary contract that never survived an owner death is never inserted and can never inflate
either signal. The mark is a runtime diagnostic, never digested (§5).

### 3.3 Conservation
Skipping `settle_share_contract_grain` for a succeeded contract moves **zero** units — the worker's
`grain_in_stock` was already in the worker's econ stock (grain has no provenance channel) and simply stays
there, splitting to the heir at the next `split_share_output`. Retain **XOR** estate: a retained contract's
grain never enters the dead owner's estate (`collect_estate` sweeps only the dead owner's own stock), so no
double-credit. The reservation re-establishment is a registry write, not a good movement. The heir is a
live agent, so the provenance/acquisition transfers at the next split remain agent-to-agent and valid.
`whole_system_total` and the `bread_provenance` produced-identity are untouched.

## 4. Anti-smuggling guards
1. **Voluntary, not a ratchet:** succession requires **both** re-consents (heir cap-waste gate + worker
   bread accept), each evaluated fresh after the heir owns the plot; either declining dissolves the
   contract. Succession never *pins* a contract past a party's refusal.
2. **The heir's real alternative is preserved:** an heir who would work the plot itself declines (the
   `share_plot_currently_owner_targeted` / target exclusion) — succession does not force an heir to be a
   rentier.
3. **No new preference or want kind:** reuses the existing cap-waste owner gate and bread-ordinal worker
   accept; no configured "prefer to inherit a contract."
4. **Conservation exact, off-path byte-identical:** flag-off dissolves exactly as today; the tag-26 block is
   the flag byte only; succeeded state rides tag 23's existing `owner` field.
5. **Not tuned:** no new free parameter; the re-consent gates are the existing pinned ones. `SuccessionInert`
   and `SuccessionButStillTransient` are first-class classifier routes. Swept: φ, and succession on the
   plain Voluntary base **and** on the forward-provisioning base (does removing the death wall on top of
   P1.5's forward gate produce standing tenure?).

## 5. Conservation & determinism
No new sources/sinks: succession changes a contract's owner and a reservation, moves no goods. Integer,
deterministic (the heir resolution is the existing pure `secure_land_universal_heir_for`; iteration
`AgentId`-ordered). **Digest:** tag 26 = ON-only `{ push(26); push(u8::from(share_contract_succession)) }`;
a succeeded contract's `owner = heir` is carried by the existing tag-23 `ShareContract` block; the
succession telemetry (`successions_total`, `heir_declined`, `worker_re_declined`, `post_succession_renewals`,
`final_open_succeeded`, and the runtime `succeeded_live_ids: BTreeSet<u64>` that threads the succeeded mark
through renewals) are all **runtime-only diagnostics, out of `canonical_bytes`** (the tag-22/23/24/25
discipline — the mark steers no future tick; a contract behaves identically whether or not it was
succeeded). Off-path: byte-identical to the decomposed branch goldens (all civ flags off → master goldens).

## 6. Slices
- **A — the succession seam.** The flag (7-site template), tag 26, the owner/worker branch split in
  `settle_share_tenancy_for_death` + the pending-succession record, the post-transfer `finalize_succession`
  with both re-consent gates and the in-place owner rewrite + reservation re-establishment, the telemetry
  fields (+ `ShareTenancyStats`). Partible → dissolve; worker-death → dissolve. *DoD: flag-off byte-identical
  (goldens_unchanged); tag-26 split test (on splits, off/off-substrate inert); a single owner-death
  succession conserves (goods + provenance) and leaves a live contract owned by the heir with the worker
  admitted.*
- **B — composition + renewal.** Succeeded contracts reach the renewal decision under the heir (the
  `expire_share_contracts` hint fires); compose with forward-provisioning; the reservation-collision /
  no-double-contract / owner-identity invariants hold across a full run. *DoD: successions_total > 0 on the
  base; renewals reachable; conservation + registry invariants hold over 1,600 ticks.*
- **C — the suite.** `ScenarioMode::Succession` (on the forward-provisioning base) + a `SuccessionVoluntary`
  variant (plain share), the same-seed forward-provisioning comparative (its P1.5 verdict) and no-contract
  control, the §2 verdict ladder, φ sweep. *DoD: suite green; the comparative reproduces P1.5; verdicts
  printed, never asserted.*

## 7. Acceptance suite (`sim/tests/share_tenancy.rs`, extended — `ScenarioMode::Succession`)
- **Predeclared thresholds (swept):** `MIN_SUCCESSIONS` (non-vacuity of the mechanism),
  `MIN_FINAL_OPEN_CONTRACTS`, `MIN_RENEWALS`, `MIN_SURVIVAL_LIFT`, the φ band. No new tuned parameter.
- **Cells:** `Succession` (headline: share + forward-provisioning + succession, φ=marginal); a
  `SuccessionVoluntary` cell (share + succession, no forward gate); the same-seed forward-provisioning
  comparative (must reproduce its P1.5 verdict — `RenewalStillDeclined`/rotational); the `NoContract` lift
  baseline; the φ sweep.
- **Classifier, NOT asserted (succession-specific inputs — Codex P1 #2):** `successions_total == 0`
  **routes to** `SuccessionInert`; `successions_total > 0 && final_open_succeeded < MIN_FINAL_OPEN_CONTRACTS
  && post_succession_renewals < MIN_RENEWALS` **routes to** `SuccessionButStillTransient`;
  `final_open_succeeded ≥ MIN_FINAL_OPEN_CONTRACTS || post_succession_renewals ≥ MIN_RENEWALS` routes to
  `StandingTenureForms` (+ `survival_lift ≥ MIN_SURVIVAL_LIFT` → `StandingTenureLifts`). These signals are
  measured only on contracts that survived a succession (the `succeeded_live_ids` set, §3.2), so an ordinary
  contract that never survived an owner death can never inflate them. The non-vacuity evidence (a specific
  succeeded contract still open in the final window) is a **reported trace**, never an assertion.
- **Hard guards (invariants only):** conservation (goods + provenance identity), money invariant,
  registry/owner-identity, reservation validity (every succeeded contract's plot is reserved to its worker),
  no-double-contract, renewal-fate consistency, and a **succession-conserves** check (a succession moves no
  goods; the estate never double-credits a retained contract's grain).
- **`goldens_unchanged` + the tag-26 canonical-split test.**

Build/verify: `cargo test -p sim --test share_tenancy -- --nocapture`, full workspace, fmt, clippy
`-D warnings`; the wage_labor + in_kind_wage + S23c/d/e suites stay green; every prior digest unchanged.

## 8. Risks & open questions
1. **`SuccessionButStillTransient` is the most likely result and is first-class.** P1.5 measured the fade as
   ~99% fed-out satiation, not mortality — so removing the death wall may leave the worker still exiting the
   pool at term end. That is a sharp finding (persistence needs overriding the *worker* exit, not the owner
   seam) and the succession-vs-forward comparative isolates it.
2. **Heir re-consent may rarely fire.** An impartible heir who inherits the plot and has no other may prefer
   to work it (declines succession) — `heir_declined` telemetry measures this; if it dominates, succession is
   inert for an economic reason (the heir becomes a cultivator, not a rentier), itself a finding.
3. **The reservation-re-establishment ordering** is the main correctness risk (the transfer wipes it); the
   finalize-after-transfer step + the reservation-validity hard guard pin it.
4. **Scarce-φ tipping.** C1N sustained+lifted at scarce φ; succession at scarce φ (where the worker is less
   readily satiated) is where `StandingTenureForms/Lifts` is most plausible — the φ sweep probes it.
5. **In-kind wage succession is out of scope (disclosed).** The C1N in-kind contract also dissolves at
   employer death (in_kind_wage.rs:388) but is a **fixed-term contract with no renewal path** (no
   `expire`/renewal-hint machinery, unlike the share contract) — so "the heir assumes the owner side and the
   contract renews" is not defined for it. This milestone scopes succession to the **renewing share
   contract** (with/without forward provisioning); in-kind wage succession is a separate future question.
   Worker-death → dissolve (worker succession out of scope: a landless worker has no plot for an heir to
   inherit-with-a-contract). Partible inheritance → dissolve (no single owner slot). All three disclosed,
   not silent gaps.

## 9. Falsifiable-bar summary
Letting the heir who inherits the plot **assume the owner side** of a live share contract — a voluntary
re-formation gated by the heir's cap-waste consent and the worker's re-acceptance, a single in-place owner
rewrite that keeps the contract alive across the owner-death seam — removes the mortality bound P1.5
measured and lets a contract reach a renewal decision under the heir. The milestone shows whether that turns
the transient/rotational institution into **standing tenure** (`StandingTenureForms/Lifts` — the arc's first
persistent cross-generational institution, most plausibly at scarce φ), or whether it still fades
(`SuccessionButStillTransient` — the death seam was not the binding constraint; the worker-satiation exit
is), with `SuccessionInert` (the mechanism never engages — heirs prefer to work their inheritance) as the
disclosed floor. Each named before the run, each first-class.
