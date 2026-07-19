//! The determinism/digest surface.
//!
//! `canonical_bytes` — the order-stable byte serialization of the whole settlement
//! (the determinism tripwire) — plus its FNV-1a `digest` and every `push_*_bytes`
//! serialization helper. Extracted verbatim from `mod.rs` (pure code motion): the two
//! methods move into the `impl Settlement` block below; the helpers become `pub(super)`
//! and are re-imported by the parent via `use digest::*`. Keeping the entire digest
//! surface in one file makes the hand-maintained per-flag serialization auditable
//! (every field either digested here or deliberately absent).

use super::*;

impl Settlement {
    /// A canonical, order-stable byte serialization of the whole settlement —
    /// world, econ holdings, needs, and realized prices. Two settlements are
    /// byte-identical iff these are equal (the determinism tripwire).
    pub fn canonical_bytes(&self) -> Vec<u8> {
        let mut out = Vec::new();
        out.extend_from_slice(&self.econ_tick.to_le_bytes());
        out.extend_from_slice(&self.world.canonical_bytes());

        // Config-derived parameters that steer future ticks but are not otherwise
        // captured by the dynamic state below, so two settlements differing only
        // in one of them never digest equal — the tripwire stays honest for
        // non-equivalent configs, not only same-config reruns.
        out.extend_from_slice(&self.carry_cap.to_le_bytes());
        out.extend_from_slice(&self.exchange.0.to_le_bytes());
        push_dynamics_bytes(&mut out, &self.dynamics);
        // The role-choice phase (G3b) acts only on a latent pool; a settlement with
        // none (a plain config or a seeded G3a chain) runs it as a no-op. So the
        // role-choice-only knobs below extend the digest only when a latent pool is
        // present — without one they cannot steer a future tick, and including them
        // would make behaviour-identical states digest differently.
        let has_latent_pool = self
            .colonists
            .iter()
            .any(|colonist| colonist.latent.is_some());
        // S7.1: with tool-acquisition eligibility on, role-choice acts on a colonist
        // that merely HOLDS a tool — so the role-choice appraisal can steer a future
        // tick even with no seeded latent pool. Widen the role-choice digest gate to
        // "a latent pool OR S7 eligibility on", so the operating cost and the
        // per-colonist latent block below serialize in that case too. With the gate
        // off this is exactly `has_latent_pool`, so every pre-S7 stream is unchanged.
        let role_choice_active = has_latent_pool || self.tool_acquisition_can_run();
        // S11: whether per-agent forecasts steer the appraisals — gates the per-belief
        // `observed` flag and the per-colonist `forecast_bias_bps` below into the digest.
        // Off the flag neither is emitted, so the pre-S11 stream is byte-identical.
        let entrepreneurial_serialized = self.entrepreneurial_can_run();
        if let Some(chain) = &self.chain {
            out.extend_from_slice(&chain.throughput.to_le_bytes());
            // The G3b operating cost steers nothing but the role-choice appraisal, so
            // it is part of the future-behaviour identity only when that appraisal can
            // run — a latent pool OR S7 tool-acquisition eligibility (the widened
            // gate). Without either (a seeded G3a chain) two settlements differing only
            // in it behave identically, so it is omitted — keeping the tripwire's
            // "byte-identical iff future behaviour identical" contract honest rather
            // than splitting equivalent seeded chains apart.
            if role_choice_active {
                out.extend_from_slice(&chain.operating_cost.to_le_bytes());
            }
            // The S2/S5 endogenous knobs steer future ticks but never show up in the
            // generated holdings, so two chains differing only in one would collide
            // at generation and then diverge — they belong in the "byte-identical iff
            // future behaviour identical" identity, exactly like the operating cost.
            // `producer_subsistence` mints the local staple/WOOD floor for producers
            // each tick; `project_input_bids` switches input acquisition to the
            // project-aware market bid. Both are included unconditionally (not
            // latent-pool-gated like the operating cost): every chain config has
            // producers and a money path, so each always eventually steers a tick —
            // there is no behaviour-identical config pair they would falsely split.
            out.extend_from_slice(&chain.producer_subsistence.to_le_bytes());
            out.push(u8::from(chain.project_input_bids));
            // `recurring_motive` keeps an owner-operator adopted while the recipe
            // stays profitable, steering future role-choice ticks without ever
            // showing up in generated holdings — the same identity contract as the
            // two knobs above, so it joins them unconditionally.
            out.push(u8::from(chain.recurring_motive));
            // The capital-advance / in-kind-subsistence / in-kind-input / spoilage
            // knobs each gate a future settlement phase (run_capital_advance,
            // run_subsistence_advance, run_input_advance, run_spoilage) that runs for
            // any chain regardless of a latent pool, so two configs differing only in
            // one generate identically and then diverge — the same identity contract,
            // joined unconditionally. (perishable_decay_bps is the spoilage rate, not
            // a bool: 0 disables the phase, any other value steers it.)
            out.push(u8::from(chain.capital_advance));
            out.push(u8::from(chain.subsistence_advance));
            out.push(u8::from(chain.input_advance));
            out.extend_from_slice(&chain.perishable_decay_bps.to_le_bytes());
            // Outstanding capital-advance loans are live future-behaviour state under
            // the flag: the repayment phase drains them from post-market sales and the
            // advance phase skips borrowers that still owe, so two settlements equal in
            // every holding but differing in an owed balance diverge on later ticks.
            // Emitted only when the flag is armed (the same gated-block discipline as
            // the blocks below), so a capital-advance-OFF run stays byte-identical to
            // the pre-existing stream. BTreeMap iteration order is deterministic.
            if chain.capital_advance {
                out.extend_from_slice(&(self.capital_loans.len() as u32).to_le_bytes());
                for (borrower, (lender, owed)) in &self.capital_loans {
                    out.extend_from_slice(&borrower.0.to_le_bytes());
                    out.extend_from_slice(&lender.0.to_le_bytes());
                    out.extend_from_slice(&owed.0.to_le_bytes());
                }
            }
            // The S6 productive-re-entry state steers future ticks only while the
            // phase can actually feed a colonist: the gate is on, raw grain is the
            // subsistence fallback, AND a grain-yielding node exists. When it cannot
            // run, omit these bytes entirely (no marker) — like every other gated
            // block here (latent pool, research, the per-colonist home below) — so a
            // re-entry-OFF or inert config stays byte-identical to the pre-S6 stream
            // and two behavior-identical configs never digest apart.
            if self.productive_reentry_can_run() {
                out.extend_from_slice(&chain.reentry_hunger_in.to_le_bytes());
                out.extend_from_slice(&chain.reentry_hunger_out.to_le_bytes());
            }
            // S7.1: the tool-acquisition eligibility gate relaxes role-choice and adds
            // the acquired-tool scale anchor, steering every future tick for any chain
            // once a colonist comes to hold a tool. It is emitted only when on, so a
            // pre-S7 (flag-off) chain stays byte-identical to the pre-S7 stream — the
            // same gated-block discipline as the re-entry thresholds above.
            if self.tool_acquisition_can_run() {
                out.push(1);
            }
            // S7.2: the per-builder capital-formation phase + its appraisal knobs steer
            // every future tick once on, so they join the identity when the phase can
            // run. Emitted only when on (the same gated-block discipline), so a
            // producible-capital-OFF chain stays byte-identical to the pre-S7 stream.
            if self.producible_capital_can_run() {
                // S10: in the per-agent path the `per_agent_capital` flag steers every
                // future tick and `capital_payback_cycles` is behaviour-INERT — so
                // serialize the flag in its place (digesting the inert knob would split
                // behaviour-identical per-agent configs). The legacy heuristic path
                // serializes `capital_payback_cycles` and no flag, byte-identical to pre-S10.
                if self.per_agent_capital_can_run() {
                    out.push(1);
                } else {
                    out.extend_from_slice(&chain.capital_payback_cycles.to_le_bytes());
                }
                out.extend_from_slice(&chain.tool_build_wood.to_le_bytes());
                out.extend_from_slice(&chain.tool_build_labor.to_le_bytes());
                out.extend_from_slice(&chain.capital_build_hunger_max.to_le_bytes());
                out.extend_from_slice(&self.next_capital_project_id.to_le_bytes());
                // The in-flight per-builder builds are live state two runs through the
                // build can differ in (which builder, how much labor it has advanced),
                // so they are part of the future-behaviour identity. Serialized in the
                // stored (slot-ordered, deterministic) order. Each build's WOOD cost and
                // output are fixed by the template; labor_advanced is the progress.
                out.extend_from_slice(&(self.capital_builds.len() as u32).to_le_bytes());
                for build in &self.capital_builds {
                    out.extend_from_slice(&build.builder.0.to_le_bytes());
                    out.extend_from_slice(&build.project.id.0.to_le_bytes());
                    out.push(project_template_id_tag(build.project.template));
                    out.extend_from_slice(&build.project.started_at.0.to_le_bytes());
                    out.extend_from_slice(&build.project.output_good.0.to_le_bytes());
                    out.extend_from_slice(&build.project.output_qty.to_le_bytes());
                    out.extend_from_slice(&build.template.required_labor.to_le_bytes());
                    out.extend_from_slice(&build.project.labor_advanced.to_le_bytes());
                }
            }
            // S11: the entrepreneurial-forecasts gate steers every future tick once on
            // (each appraisal weighs a per-agent forecast instead of the realized price),
            // so it joins the identity when the phase can run. Emitted only when on (the
            // same gated-block discipline as S7/S10 above + the per-colonist forecast bias
            // and the per-belief `observed` flag below), so a flag-off chain stays
            // byte-identical to the pre-S11 stream.
            if self.entrepreneurial_can_run() {
                out.push(1);
            }
            // S12: the own-labor subsistence gate retires the food mints and steers the
            // forage phase + the per-colonist `foraging` state below. When it can run,
            // serialize a marker + the forage knobs (yield + the hysteresis band) that
            // steer how much FORAGE is produced and who forages. Emitted only when on
            // (the same gated-block discipline as S7/S10/S11 above), so a flag-off chain
            // stays byte-identical to the pre-S12 stream. (The FORAGE good id itself is
            // already captured by `known.subsistence` below and `good_entries`.)
            if self.own_labor_subsistence_can_run() {
                let commons_active = self.forage_commons_active();
                out.push(1);
                let forage_yield = if commons_active {
                    0
                } else {
                    chain.forage_yield
                };
                out.extend_from_slice(&forage_yield.to_le_bytes());
                out.extend_from_slice(&chain.forage_hunger_in.to_le_bytes());
                out.extend_from_slice(&chain.forage_hunger_out.to_le_bytes());
                // S14: the FORAGE-commons mode flag. The node's stock/regen/cap are
                // already in `world.canonical_bytes` (it is a real `ResourceNode`); this
                // marker pins the BEHAVIOR switch (harvest the depleting node + retire the
                // fixed credit + FORAGE child endowment) so a commons config never digests
                // equal to a `0/0/0`-marker config that happens to share node bytes.
                // Emitted only when on, so a marker-mode (commons-off) chain stays
                // byte-identical to the pre-S14 stream.
                if commons_active {
                    out.push(1);
                }
            }
            // S15: the own-use cultivation gate + its hysteresis tier + the per-tick
            // own-use bread draw. All steer future ticks (who escalates to cultivation,
            // and how much bread is eaten via the readback) only while the path is
            // active, so they are emitted ONLY when on (the gated-block discipline) — a
            // cultivation-off chain stays byte-identical to the pre-S15 stream. (The
            // `Cultivate` recipe itself is already in the recipe bytes above, and the
            // grain node's stock/regen/cap are in `world.canonical_bytes`.)
            if self.own_use_cultivation_active() {
                out.push(1);
                out.extend_from_slice(&chain.cultivate_hunger_in.to_le_bytes());
                out.extend_from_slice(&chain.cultivate_hunger_out.to_le_bytes());
                out.extend_from_slice(&chain.cultivate_consume.to_le_bytes());
                out.extend_from_slice(&chain.cultivate_patience.to_le_bytes());
            }
            // S16: the money-from-produced-bread gate steers BOTH the buy/sell split (who
            // forages/cultivates) and the provenance ledger, so it is part of the
            // future-behaviour identity whenever it can run. Emitted only when on (the same
            // gated-block discipline as S15 above), so a flag-off chain stays byte-identical
            // to the pre-S16 stream. (The per-agent provenance counters it maintains are
            // serialized with the colonist roster below.)
            if self.cultivation_sells_surplus_active() {
                out.push(1);
            }
            // S18: the multi-good money gate routes the non-lineage gatherers (the
            // woodcutters) to the WOOD node at generation — a real behavior switch in the
            // colonist roster — so it is part of the future-behavior identity whenever it can
            // run. Emitted only when on (the same gated-block discipline as S16 above), so a
            // flag-off chain stays byte-identical to the pre-S18 stream. (The runtime-only
            // multi-good instrumentation it also turns on is diagnostic and NOT digested.)
            if self.multigood_money_active() {
                out.push(1);
            }
            // S21d.0: the retire-food-mints gate skips the recurring demographic + producer
            // staple mints every tick — a future-behaviour change for any chain (its agents
            // must buy/produce food instead of being fed), so it joins the identity. Two
            // refinements keep the marker injective AND behaviour-faithful:
            //   * It is emitted ONLY when it actually retires a *live* mint — i.e. the flag is
            //     on AND own-labor subsistence is NOT already retiring the same two mint sites
            //     (both guarded by `!own_labor_subsistence_can_run() && !retire_food_mints()`,
            //     settlement.rs:7919/8574). When own-labor already runs, the flag is
            //     behaviour-inert, so emitting nothing keeps an own-labor config's digest
            //     unchanged (no false split for behaviour-identical configs).
            //   * The marker is a DISTINCT tag (`2`), not the bare `1` the adjacent
            //     `multigood_money_active()` block emits, so the two gated markers can never
            //     collide in the byte stream. (They are already mutually exclusive — multigood
            //     requires `own_labor_subsistence_can_run()` true while this marker requires it
            //     false — but the distinct tag makes the injectivity self-evident rather than
            //     resting on that cross-flag invariant.)
            // Off (every existing config) emits nothing, so a flag-off chain stays
            // byte-identical to the pre-S21d stream. (The S21d.1 acquisition ledger it pairs
            // with is a runtime-only diagnostic and is deliberately NOT digested — like
            // `starvation_deaths_total`.)
            if self.retire_food_mints() && !self.own_labor_subsistence_can_run() {
                out.push(2);
            }
            // S21e: a finite one-time seeded surplus changes initial holdings and
            // future behavior, but default 0 must preserve every existing byte stream.
            // The runtime traces that observe its depletion remain excluded.
            if chain.seeded_surplus_bread > 0 {
                out.push(3);
                out.extend_from_slice(&chain.seeded_surplus_bread.to_le_bytes());
            }
            // S21f: the household-barter cultivation seam activates cultivation steering +
            // the own-use cultivation phase + the multi-good woodcutter routing WITHOUT the
            // forage substrate — a future-behaviour change for the roster (lineage members
            // escalate to cultivation, produce bread, barter the surplus). Emitted only when
            // active (the same gated-block discipline as S16/S18/S21d above) with a DISTINCT
            // tag (`4`), so the gated markers stay injective and a flag-off chain stays
            // byte-identical to the pre-S21f stream.
            if self.household_barter_cultivation_active() {
                out.push(4);
            }
            // S21h.0: the non-lineage woodcutters' consumed-only bread cushion changes
            // their initial holdings (and so future behaviour). The differing gatherer
            // starting stock already shifts the colonist-roster bytes below, but a DISTINCT
            // tag (`5`) plus the value makes the gated marker self-evidently injective and
            // ON-only — default 0 emits nothing, so a flag-off chain stays byte-identical to
            // the pre-S21h stream. (The acquisition ledger that classifies the cushion as
            // `SeededMinted` is a runtime-only diagnostic and is deliberately NOT digested.)
            if chain.gatherer_food_cushion > 0 {
                out.push(5);
                out.extend_from_slice(&chain.gatherer_food_cushion.to_le_bytes());
            }
            // S21h.1: the emergency self-provisioning seam fires a near-critical own-labor
            // bread floor for the non-lineage roles whenever it is on — a future-behaviour
            // change for the roster (those roles produce + immediately eat emergency bread).
            // Emitted only when on (the same gated-block discipline as S15/S16/S21f above)
            // with a DISTINCT tag (`6`) plus the threshold, so the gated markers stay
            // injective and a flag-off chain stays byte-identical to the pre-S21h stream.
            if self.emergency_self_provision_active() {
                out.push(6);
                out.extend_from_slice(&chain.emergency_hunger_threshold.to_le_bytes());
            }
            // S22a: the endogenous cultivation-entry gate relaxes who is eligible to escalate
            // to cultivation (any spatial colonist, not just the lineage) — a future-behaviour
            // change for the roster (non-lineage agents enter cultivation under hunger). Emitted
            // only when active (the same gated-block discipline as S16/S18/S21d/f/h above) with a
            // DISTINCT tag (`7`), so the gated markers stay injective and a flag-off chain stays
            // byte-identical to the pre-S22a stream. (The production-time entrant-class provenance
            // split + the rolling cultivator/buyer diagnostics it pairs with are runtime-only and
            // deliberately NOT digested — like `starvation_deaths_total`.)
            if self.endogenous_cultivation_entry_active() {
                out.push(7);
            }
            // S22b: the bounded cultivation-skill gate changes who out-harvests grain (a skilled
            // cultivator hauls more per trip), a future-behaviour change for the roster. Emitted
            // only when active (the same gated-block discipline as S16/S18/S21/S22a above) with a
            // DISTINCT tag (`8`) plus the skill magnitudes that steer the haul/accumulate/decay,
            // so the gated markers stay injective and a flag-off chain stays byte-identical to the
            // pre-S22b stream. (The per-agent skill state is serialized with the colonist roster
            // below; the skill-distribution / grain-share / churn diagnostics it pairs with are
            // runtime-only and deliberately NOT digested.)
            if self.cultivation_skill_active() {
                out.push(8);
                out.extend_from_slice(&chain.skill_gain.to_le_bytes());
                out.extend_from_slice(&chain.skill_decay.to_le_bytes());
                out.extend_from_slice(&chain.skill_cap.to_le_bytes());
                out.extend_from_slice(&chain.skill_haul_ceiling.to_le_bytes());
            }
            // S22c: the profit-driven-retention gate makes the cultivation EXIT profit-modulated (a
            // currently-cultivating agent stays past the hunger exit on a clearing realized return)
            // — a future-behaviour change for the roster. Emitted only when active (the same
            // gated-block discipline as S16/S18/S21/S22a/b above) with a DISTINCT tag (`9`), so the
            // gated markers stay injective and a flag-off chain stays byte-identical to the pre-S22c
            // stream. (The per-agent rolling-return window it maintains IS digested — serialized
            // with the colonist roster below, ON-only — because it steers the next `cultivating`
            // flag; the counterfactual-flip / proceeds-distribution diagnostics it pairs with are
            // runtime-only and deliberately NOT digested.)
            if self.profit_driven_retention_active() {
                out.push(9);
                out.extend_from_slice(&chain.return_window.to_le_bytes());
                out.extend_from_slice(&chain.retention_margin_bps.to_le_bytes());
                out.extend_from_slice(&chain.retention_material_floor.to_le_bytes());
            }
            // S22d: the durable-cultivation-capital gate builds the owned plow + raises the
            // owner's grain-haul ceiling — a future-behaviour change for the roster (which
            // cultivators capitalize, and how much grain an owner draws). Emitted only when active
            // (the same gated-block discipline as S16/S18/S21/S22a/b/c above) with a DISTINCT tag
            // (`10`) plus the build/boost magnitudes (the sunk WOOD+labor cost, the build patience,
            // the owner haul ceiling) and the in-flight builds (live state two runs can differ in:
            // which builder, how much labor advanced), so the gated markers stay injective and a
            // flag-off chain stays byte-identical to the pre-S22d stream. (The per-agent
            // cultivation tenure it maintains IS digested — serialized with the colonist roster
            // below, ON-only — because it steers the build decision; the tool-owner / sunk-cost /
            // churn diagnostics it pairs with are runtime-only and deliberately NOT digested. The
            // plow good itself lives in the agent's stock, already serialized.)
            if self.durable_cultivation_tool_active() {
                out.push(10);
                out.extend_from_slice(&chain.tool_build_wood.to_le_bytes());
                out.extend_from_slice(&chain.tool_build_labor.to_le_bytes());
                out.extend_from_slice(&chain.tool_build_patience.to_le_bytes());
                out.extend_from_slice(&chain.cultivation_tool_haul_ceiling.to_le_bytes());
                out.push(u8::from(chain.cultivation_tool_non_durable));
                out.extend_from_slice(&self.next_cultivation_tool_project_id.to_le_bytes());
                out.extend_from_slice(&(self.cultivation_tool_builds.len() as u32).to_le_bytes());
                for build in &self.cultivation_tool_builds {
                    out.extend_from_slice(&build.builder.0.to_le_bytes());
                    out.extend_from_slice(&build.project.id.0.to_le_bytes());
                    out.push(project_template_id_tag(build.project.template));
                    out.extend_from_slice(&build.project.started_at.0.to_le_bytes());
                    out.extend_from_slice(&build.project.output_good.0.to_le_bytes());
                    out.extend_from_slice(&build.project.output_qty.to_le_bytes());
                    out.extend_from_slice(&build.template.required_labor.to_le_bytes());
                    out.extend_from_slice(&build.project.labor_advanced.to_le_bytes());
                }
            }
            // S22e: the endowed + inherited cultivation-capital gate seeds a minority of lineage
            // households with a plow at generation and gates whether plows inherit to the heir or
            // are FORCED to the commons. Emitted only when the active gate can steer behavior:
            // either at least one household is endowed, or the inheritance switch forces any later
            // built plows to the commons. The explicitly inert no-endowment / inheritance-on
            // control omits the marker so it stays byte-identical to the expanded S22d base.
            if self.endowed_cultivation_capital_digest_active() {
                out.push(11);
                out.extend_from_slice(&chain.endowed_tool_count.to_le_bytes());
                out.push(u8::from(chain.cultivation_tool_inheritance));
                out.extend_from_slice(&(self.endowed_households.len() as u32).to_le_bytes());
                for &household in &self.endowed_households {
                    out.extend_from_slice(&(household as u32).to_le_bytes());
                }
            }
            // S22f: the voluntary fixed-term cultivation commitment gate makes the cultivation EXIT
            // overridable for a chosen term (a committed agent cannot exit until the term expires) — a
            // future-behaviour change for the roster. Emitted only when active (the same gated-block
            // discipline as S16/S18/S21/S22a–e above) with a DISTINCT tag (`12`) plus the binding term,
            // the entry floor, and the fiat-pin count (which steer who commits, for how long, and
            // whether entry is voluntary or forced), so the gated markers stay injective and a flag-off
            // chain stays byte-identical to the pre-S22f stream. (The per-agent commitment state IS
            // digested — serialized with the colonist roster below, ON-only — because it steers the
            // next cultivation exit; the uptake/renewal/below-floor/exit-override diagnostics it pairs
            // with are runtime-only and deliberately NOT digested.)
            if self.voluntary_cultivation_commitment_active() {
                out.push(12);
                out.extend_from_slice(&chain.commitment_term.to_le_bytes());
                out.extend_from_slice(&chain.commitment_entry_floor.to_le_bytes());
                out.extend_from_slice(&chain.commitment_fiat_pin.to_le_bytes());
            }
            // S23a: private land tenure changes the grain-harvest target set and access rights —
            // a future-behaviour change for every cultivating agent. Emitted only when active
            // (the same gated-block discipline as S16/S18/S21/S22a-f above) with the next free tag
            // (`13`), the land knobs/layout, and the plot registry (owner + idle clock +
            // reclaim reservation) that steers the next validation/forfeiture pass. Per-agent
            // carried grain source is serialized in the colonist block below.
            if self.private_land_tenure_active() {
                out.push(13);
                out.extend_from_slice(&chain.land_idle_limit.to_le_bytes());
                out.push(u8::from(self.private_land_harvest_gate_active()));
                out.push(u8::from(self.private_land_forfeit_on_idle_active()));
                out.push(u8::from(chain.reclaim_reserved_for_prior_owner));
                out.extend_from_slice(&chain.land_good_plots.to_le_bytes());
                out.extend_from_slice(&chain.land_marginal_plots.to_le_bytes());
                out.extend_from_slice(&chain.land_marginal_regen.to_le_bytes());
                out.extend_from_slice(&(self.land_plots.len() as u32).to_le_bytes());
                for (&node, record) in &self.land_plots {
                    out.extend_from_slice(&node.0.to_le_bytes());
                    match record.owner {
                        Some(owner) => {
                            out.push(1);
                            out.extend_from_slice(&owner.0.to_le_bytes());
                        }
                        None => out.push(0),
                    }
                    out.extend_from_slice(&record.idle_counter.to_le_bytes());
                    match record.reserved_for {
                        Some(owner) => {
                            out.push(1);
                            out.extend_from_slice(&owner.0.to_le_bytes());
                        }
                        None => out.push(0),
                    }
                }
            }
            if self.secure_land_tenure_active() {
                out.push(18);
                out.push(u8::from(chain.secure_land_tenure));
                out.push(inheritance_regime_tag(chain.inheritance_regime));
                out.push(1); // universal-heir rule version
                out.extend_from_slice(&(self.land_plots.len() as u32).to_le_bytes());
                for (&node, record) in &self.land_plots {
                    out.extend_from_slice(&node.0.to_le_bytes());
                    out.extend_from_slice(&(record.shares.len() as u32).to_le_bytes());
                    for (&owner, share) in &record.shares {
                        out.extend_from_slice(&owner.0.to_le_bytes());
                        out.extend_from_slice(&share.regen.to_le_bytes());
                        out.extend_from_slice(&share.cap.to_le_bytes());
                        out.extend_from_slice(&share.available.to_le_bytes());
                    }
                    out.extend_from_slice(&record.stranded_regen.to_le_bytes());
                    out.extend_from_slice(&record.stranded_cap.to_le_bytes());
                }
            }
            if self.mortal_landowner_demography_active() {
                // Tag 19 is already occupied by `fixed_commitment_norm_active()` below in this same
                // gated chain, so S23d takes the next free tag (`20`) to keep the ON-only sections
                // injective when a config composes both.
                out.push(20);
                out.push(u8::from(chain.mortal_landowner_demography));
                out.push(1); // lineage-owner routing rule version
            }
            if self.rival_subsistence_commons_active() {
                out.push(21);
                out.push(u8::from(chain.rival_subsistence_commons));
                out.extend_from_slice(&chain.rival_subsistence_commons_phi_bps.to_le_bytes());
                out.extend_from_slice(&self.subsistence_commons_stock.to_le_bytes());
                out.extend_from_slice(&self.subsistence_commons_cap.to_le_bytes());
                out.extend_from_slice(&self.subsistence_commons_regen.to_le_bytes());
            }
            if self.wage_labor_active() {
                out.push(22);
                out.push(u8::from(chain.wage_labor));
                out.push(wage_labor_mode_tag(chain.wage_labor_mode));
                out.extend_from_slice(&self.wage_escrow_gold.0.to_le_bytes());
                out.extend_from_slice(&self.next_wage_contract_id.to_le_bytes());
                out.extend_from_slice(&(self.wage_escrows.len() as u32).to_le_bytes());
                for escrow in &self.wage_escrows {
                    out.extend_from_slice(&escrow.id.to_le_bytes());
                    out.extend_from_slice(&escrow.employer.0.to_le_bytes());
                    out.extend_from_slice(&escrow.worker.0.to_le_bytes());
                    out.extend_from_slice(&escrow.amount.0.to_le_bytes());
                    out.extend_from_slice(&escrow.wage.0.to_le_bytes());
                    out.extend_from_slice(&escrow.retained_funded.0.to_le_bytes());
                    out.extend_from_slice(&escrow.endowment_funded.0.to_le_bytes());
                    out.extend_from_slice(&escrow.qty.to_le_bytes());
                    out.extend_from_slice(&escrow.opened_tick.to_le_bytes());
                    out.extend_from_slice(&escrow.release_tick.to_le_bytes());
                    push_recipe_id_bytes(&mut out, escrow.recipe);
                    out.extend_from_slice(&escrow.output_good.0.to_le_bytes());
                    out.extend_from_slice(&escrow.output_qty.to_le_bytes());
                    match escrow.input {
                        Some((good, qty)) => {
                            out.push(1);
                            out.extend_from_slice(&good.0.to_le_bytes());
                            out.extend_from_slice(&qty.to_le_bytes());
                        }
                        None => out.push(0),
                    }
                    out.extend_from_slice(&escrow.delivered.to_le_bytes());
                }
                out.extend_from_slice(&(self.wage_retained_earnings.len() as u32).to_le_bytes());
                for (&agent, &amount) in &self.wage_retained_earnings {
                    out.extend_from_slice(&agent.0.to_le_bytes());
                    out.extend_from_slice(&amount.0.to_le_bytes());
                }
                out.extend_from_slice(&(self.wage_proceeds_buckets.len() as u32).to_le_bytes());
                for (&agent, bucket) in &self.wage_proceeds_buckets {
                    out.extend_from_slice(&agent.0.to_le_bytes());
                    out.extend_from_slice(&(bucket.len() as u32).to_le_bytes());
                    for lot in bucket {
                        out.extend_from_slice(&lot.amount.0.to_le_bytes());
                    }
                }
                // wage_workers_ever / wage_employers_ever are DIAGNOSTIC-ONLY (read solely by
                // wage_labor_stats for distinct_* counts; consulted by no matching / escrow /
                // decision path). Excluding them from canonical_bytes keeps the digest byte-identical
                // for two states that differ only in *historical* hire participants — preserving the
                // "byte-identical iff future behaviour identical" contract (spec-review P2).
            }
            if self.share_tenancy_active() {
                out.push(23);
                out.push(u8::from(chain.share_tenancy));
                out.push(share_tenancy_mode_tag(chain.share_tenancy_mode));
                out.extend_from_slice(&chain.share_bps.to_le_bytes());
                out.extend_from_slice(&chain.share_term.to_le_bytes());
                out.extend_from_slice(&self.next_share_contract_id.to_le_bytes());
                out.extend_from_slice(&(self.share_contracts.len() as u32).to_le_bytes());
                for contract in &self.share_contracts {
                    out.extend_from_slice(&contract.id.to_le_bytes());
                    out.extend_from_slice(&contract.owner.0.to_le_bytes());
                    out.extend_from_slice(&contract.worker.0.to_le_bytes());
                    out.extend_from_slice(&contract.node.0.to_le_bytes());
                    out.extend_from_slice(&contract.share_bps.to_le_bytes());
                    out.extend_from_slice(&contract.term.to_le_bytes());
                    out.extend_from_slice(&contract.opened_tick.to_le_bytes());
                    out.extend_from_slice(&contract.renewals.to_le_bytes());
                    out.extend_from_slice(&contract.cap_at_start.to_le_bytes());
                    out.extend_from_slice(&contract.grain_in_stock.to_le_bytes());
                    // The split residue steers every future split (cumulative-exact floor),
                    // so it is state, not a diagnostic — digested with the record.
                    out.extend_from_slice(&contract.split_remainder_bps.to_le_bytes());
                }
                out.extend_from_slice(&(self.colonists.len() as u32).to_le_bytes());
                for colonist in &self.colonists {
                    out.extend_from_slice(&colonist.id.0.to_le_bytes());
                    match colonist.carried_share_contract_id {
                        Some(contract_id) => {
                            out.push(1);
                            out.extend_from_slice(&contract_id.to_le_bytes());
                        }
                        None => out.push(0),
                    }
                }
            }
            if self.share_forward_provisioning_active() {
                out.push(24);
                out.push(u8::from(chain.share_forward_provisioning));
            }
            if self.in_kind_wage_active() {
                out.push(25);
                out.push(u8::from(chain.in_kind_wage));
                out.extend_from_slice(&self.next_in_kind_contract_id.to_le_bytes());
                out.extend_from_slice(&(self.in_kind_contracts.len() as u32).to_le_bytes());
                for contract in &self.in_kind_contracts {
                    out.extend_from_slice(&contract.id.to_le_bytes());
                    out.extend_from_slice(&contract.employer.0.to_le_bytes());
                    out.extend_from_slice(&contract.worker.0.to_le_bytes());
                    out.extend_from_slice(&contract.node.0.to_le_bytes());
                    out.extend_from_slice(&contract.wage_bread.to_le_bytes());
                    out.extend_from_slice(&contract.term.to_le_bytes());
                    out.extend_from_slice(&contract.opened_tick.to_le_bytes());
                    out.extend_from_slice(&contract.grain_in_stock.to_le_bytes());
                    out.extend_from_slice(&contract.split_remainder_bps.to_le_bytes());
                }
                out.extend_from_slice(&(self.colonists.len() as u32).to_le_bytes());
                for colonist in &self.colonists {
                    out.extend_from_slice(&colonist.id.0.to_le_bytes());
                    match colonist.carried_in_kind_contract_id {
                        Some(contract_id) => {
                            out.push(1);
                            out.extend_from_slice(&contract_id.to_le_bytes());
                        }
                        None => out.push(0),
                    }
                }
            }
            if self.share_contract_succession_active() {
                out.push(26);
                out.push(u8::from(chain.share_contract_succession));
            }
            if self.mortal_chain_producers_active() {
                out.push(27);
                out.push(u8::from(chain.mortal_chain_producers));
            }
            if self.mortal_producer_inheritance_active() {
                out.push(28);
                out.push(u8::from(chain.mortal_producer_inheritance));
                out.push(u8::from(chain.mortal_producer_tool_inheritance));
                out.push(chain.producer_house_cap);
            }
            if self.earned_provisioning_active() {
                out.push(29);
                out.push(u8::from(chain.earned_provisioning));
            }
            if self.producer_stock_provisioning_control_active() {
                out.push(30);
                out.push(u8::from(chain.producer_stock_provisioning_control));
            }
            if self.birth_stock_saving_active() || self.birth_stock_control_active() {
                out.push(31);
                out.push(u8::from(chain.birth_stock_saving));
                out.push(birth_stock_saving_mode_tag(chain.birth_stock_saving_mode));
            }
            // C3R.e-obs (impl-66): pure observation. The ENTIRE digest footprint is these
            // two bytes emitted ON-only — removing this block yields the OFF stream
            // byte-for-byte (a dedicated test pins it). No counter, trace, or aggregate is
            // ever serialized (avoiding the `birth_block_*` conditional-serialize
            // anti-pattern).
            if self.saving_allocation_obs_active() {
                out.push(32);
                out.push(u8::from(chain.saving_allocation_obs));
            }
            // C3R.e (impl-67): ONE fixed injective record — a presence-bit byte (which of the
            // three knobs is set), then the present fields' LE bytes in fixed order (absent field
            // = presence bit 0, no bytes). Gated ON-only, so with all three knobs off no byte is
            // emitted and every prior golden is byte-identical.
            if self.ignition_withdrawal_active() {
                out.push(33);
                let mut presence = 0u8;
                if chain.birth_stock_ignition_at.is_some() {
                    presence |= 0b001;
                }
                if chain.producer_house_starting_staple > 0 {
                    presence |= 0b010;
                }
                if chain.producer_support_until_tick.is_some() {
                    presence |= 0b100;
                }
                out.push(presence);
                if let Some(at) = chain.birth_stock_ignition_at {
                    out.extend_from_slice(&at.to_le_bytes());
                }
                if chain.producer_house_starting_staple > 0 {
                    out.extend_from_slice(&chain.producer_house_starting_staple.to_le_bytes());
                }
                if let Some(until) = chain.producer_support_until_tick {
                    out.extend_from_slice(&until.to_le_bytes());
                }
            }
            // DH.a (impl-68): the closed-circulation marker. ON-only, injective — the tag byte 34
            // then the marker byte (always 1 when active). The whole-population provenance ledger
            // and closure preamble it enables are pure OBSERVATION and never serialized; the entire
            // digest footprint is these two bytes, so removing this block yields the OFF stream
            // byte-for-byte (a dedicated identity test pins it).
            if self.closed_circulation {
                out.push(34);
                out.push(1);
            }
            // DH.b-obs (impl-70): pure observation. The ENTIRE digest footprint is these two bytes
            // emitted whenever CONFIGURED (demography + flag), UNAFFECTED by the closure
            // force-disable so both twins carry the identical tag — removing this block yields the
            // OFF stream byte-for-byte (a dedicated test pins ON = OFF + [35, 1]). No tape, event,
            // or aggregate is ever serialized.
            if self.birth_gate_obs_configured() {
                out.push(35);
                out.push(u8::from(chain.birth_gate_obs));
            }
            // S23b: the post-money land market extends S23a's registry with an endogenous-price
            // state, listings, last-sale anchors, and the non-agent fee sink. Emitted only when the
            // market composes on active private land tenure; with the flag off every S23a and older
            // golden keeps the exact byte stream.
            if self.land_market_active() {
                out.push(14);
                out.extend_from_slice(&chain.land_carrying_cost.to_le_bytes());
                out.extend_from_slice(&chain.land_price_cap_factor.to_le_bytes());
                out.extend_from_slice(&LAND_CARRYING_PERIOD.to_le_bytes());
                out.extend_from_slice(&LAND_RENT_WINDOW.to_le_bytes());
                out.extend_from_slice(&(LAND_MIN_RENT_HISTORY as u32).to_le_bytes());
                out.extend_from_slice(&LAND_SALE_HISTORY_WEIGHT_BPS.to_le_bytes());
                out.extend_from_slice(&(LAND_SALE_HISTORY_K as u32).to_le_bytes());
                out.extend_from_slice(&LAND_LIST_IDLE.to_le_bytes());
                out.extend_from_slice(&LAND_FORECLOSE_DISCOUNT_BPS.to_le_bytes());
                out.extend_from_slice(&LAND_PRICE_MIN.to_le_bytes());
                out.extend_from_slice(&self.land_fee_pool_salt.0.to_le_bytes());
                out.extend_from_slice(&(self.land_market_plots.len() as u32).to_le_bytes());
                for (&node, state) in &self.land_market_plots {
                    out.extend_from_slice(&node.0.to_le_bytes());
                    out.extend_from_slice(&state.price.to_le_bytes());
                    match state.listing {
                        Some(listing) => {
                            out.push(1);
                            out.extend_from_slice(&listing.ask.to_le_bytes());
                            out.push(match listing.kind {
                                LandListingKind::Idle => 1,
                                LandListingKind::Foreclosure => 2,
                            });
                        }
                        None => out.push(0),
                    }
                    match state.last_sale_price {
                        Some(price) => {
                            out.push(1);
                            out.extend_from_slice(&price.to_le_bytes());
                        }
                        None => out.push(0),
                    }
                    match state.last_sale_tick {
                        Some(tick) => {
                            out.push(1);
                            out.extend_from_slice(&tick.to_le_bytes());
                        }
                        None => out.push(0),
                    }
                    out.extend_from_slice(&(state.yield_history.len() as u32).to_le_bytes());
                    for entry in &state.yield_history {
                        out.extend_from_slice(&entry.tick.to_le_bytes());
                        out.extend_from_slice(&entry.qty.to_le_bytes());
                    }
                }
            }
            if self.commitment_norm_spread_active() {
                out.push(15);
                out.extend_from_slice(&chain.commitment_seed_share_bps.to_le_bytes());
                out.extend_from_slice(&chain.imitation_period.to_le_bytes());
                out.extend_from_slice(&chain.imitation_window.to_le_bytes());
                out.extend_from_slice(&chain.imitation_margin_bps.to_le_bytes());
                out.extend_from_slice(&chain.imitation_radius.to_le_bytes());
                out.extend_from_slice(&chain.imitation_max_models.to_le_bytes());
                out.extend_from_slice(&chain.food_window_target.to_le_bytes());
                out.push(u8::from(chain.no_imitation));
                out.push(u8::from(chain.random_imitation));
                out.push(u8::from(chain.salt_in_score));
            }
            if self.abandonable_norm_active() {
                out.push(16);
                out.push(u8::from(chain.abandonable_norm));
            }
            if self.group_payoff_imitation_active() {
                out.push(17);
                out.push(u8::from(chain.group_payoff_imitation));
                out.extend_from_slice(&(COMMITMENT_NORM_GROUP_MIN_SIZE as u16).to_le_bytes());
                out.extend_from_slice(&COMMITMENT_NORM_ADOPTER_SHARE_GAP_BPS.to_le_bytes());
                out.push(u8::from(COMMITMENT_NORM_SEED_CLUSTER));
            }
            if self.fixed_commitment_norm_active() {
                out.push(19);
                let prevalence = self
                    .fixed_commitment_norm_prevalence()
                    .expect("fixed mode has a prevalence");
                out.extend_from_slice(&prevalence.to_bits().to_le_bytes());
                out.extend_from_slice(&self.generation_seed.to_le_bytes());
                out.push(1);
            }
            // The staple mapping steers the next needs/scale phase for *any* chain,
            // role-choice or not, so it is included whenever a chain is active. The
            // G3b no-spread control shares the emergent config's physical state but
            // maps hunger to FOOD instead of bread, and that divergence must show.
            out.extend_from_slice(&self.known.hunger.0.to_le_bytes());
            out.extend_from_slice(&self.known.warmth.0.to_le_bytes());
            out.extend_from_slice(&self.known.savings.0.to_le_bytes());
            // `subsistence_on_grain` is realised at construction as
            // `known.subsistence` (a directly-edible staple fallback) and steers the
            // needs/scale phase (settlement.rs:4586, 5750) exactly like the three
            // mappings above, so it joins their identity. Encode the Option as a
            // presence byte plus the good id when set.
            match self.known.subsistence {
                Some(good) => {
                    out.push(1);
                    out.extend_from_slice(&good.0.to_le_bytes());
                }
                None => out.push(0),
            }
            let entries = chain.content.good_entries();
            out.extend_from_slice(&(entries.len() as u32).to_le_bytes());
            for (name, id) in entries {
                out.extend_from_slice(&id.0.to_le_bytes());
                out.extend_from_slice(&(name.len() as u32).to_le_bytes());
                out.extend_from_slice(name.as_bytes());
            }
            out.extend_from_slice(&(chain.content.recipes().len() as u32).to_le_bytes());
            for recipe in chain.content.recipes() {
                push_recipe_bytes(&mut out, recipe);
            }
            // G6b research/tech-tier dynamic state. Gated on a research chain, so every
            // pre-G6b chain config (no research recipes) is byte-identical. The
            // tier-2 threshold steers when future ticks unlock, and the Knowledge
            // counter plus unlock tick are independent state two
            // otherwise-equal runs can differ in, so all three belong in the
            // "byte-identical iff future behaviour identical" identity — the tick the
            // tier unlocks is part of the determinism contract (G6b test 1). (The
            // tier-2 recipe's `enabled` flip is already captured by the recipe bytes
            // above, since the unlock keeps `content` consistent with the society.)
            if chain.content.has_research() {
                out.extend_from_slice(&chain.tier2_threshold.to_le_bytes());
                out.extend_from_slice(&self.knowledge.to_le_bytes());
                match self.tier2_unlocked_at {
                    Some(tick) => {
                        out.push(1);
                        out.extend_from_slice(&tick.to_le_bytes());
                    }
                    None => out.push(0),
                }
            }
        }

        // The G5a emergent-money config + runtime. The config fields steer future
        // barter ticks even before they show up in holdings or tracker outputs
        // (`medium_want_qty`, endowments, and the Mengerian thresholds/candidates),
        // while the runtime fields capture the phase switch (the promoted good +
        // tick) and the FULL Mengerian emergence state — the saleability tracker's
        // accumulated per-candidate acceptances/acceptor-sets/counterpart-sets and
        // the promotion-timing latch. All of that steers the future promotion
        // decision, so it belongs in the "byte-identical iff future behaviour
        // identical" identity (the provisional leader the old layout captured is a
        // derived projection of it). Omitted entirely for non-emergent settlements,
        // so every G2b/G3/G4 canonical layout stays byte-identical.
        if let Some(barter) = &self.barter {
            push_barter_config_bytes(&mut out, barter);
            out.extend_from_slice(&self.known.savings.0.to_le_bytes());
            push_option_good_bytes(&mut out, self.current_money_good());
            match self.promoted_at_tick() {
                Some(tick) => {
                    out.push(1);
                    out.extend_from_slice(&tick.to_le_bytes());
                }
                None => out.push(0),
            }
            // A barter overlay always runs econ's Emergent money state (the two are
            // wired together in `generate`), so the emergence object is present
            // through every phase — `expect` documents that invariant rather than
            // silently dropping the runtime bytes if it were ever violated.
            let emergence = self
                .society
                .emergence()
                .expect("a barter-overlay settlement runs econ's Emergent money state");
            push_emergence_runtime_bytes(&mut out, emergence);
        }

        // The G8a M3 ledger-money runtime. Omitted entirely for pre-G8a settlements
        // so their canonical layout stays byte-identical; present for M3 so a
        // ledger-backed settlement never collides with the M1 state whose Agent.gold
        // cache happens to match at generation, and so future ledger composition
        // changes are part of the determinism surface.
        if let Some(money_system) = &self.society.money_system {
            out.push(1);
            push_money_system_bytes(&mut out, money_system);
        }

        // The G8b chartered-bank state. Omitted entirely for a bank-free settlement so
        // the pre-G8b canonical layout is byte-identical; present once a bank is
        // chartered so deposits and fiduciary lending — and every config/regime field
        // that steers the *next* bank phase — are part of the determinism surface. The
        // ledger block above already carries the system-level reserves/fiduciary; the
        // fields below are otherwise zero/default at generation, so two banked configs
        // that only diverge on tick one would collide without them.
        if let Some(bank_cfg) = &self.bank {
            // The deposit cadence steers how much specie each future tick moves into
            // reserves (and thus the whole claims/fiduciary trajectory); it lives only
            // in the config, so without it two banked configs differing only in it
            // collide at generation while diverging the next tick.
            out.extend_from_slice(&bank_cfg.deposit_per_tick.0.to_le_bytes());
            // The money regime gates `fiduciary_lend_capacity` (only
            // `FractionalConvertible` / `SuspendedConvertibility` permit fiduciary) and
            // the public spot tender decides whether the issued claims circulate — both
            // steer every future bank phase, so a divergence in either must show in the
            // digest (the G8c regime ladder will move these over time).
            out.push(regime_tag(self.society.regime()));
            out.push(public_spot_tender_tag(self.society.public_spot_tender));
            // Every chartered bank's full balance sheet AND lending policy, in `banks`
            // order (not just `BANK_ID`), so two runs differing in any bank field are
            // distinguishable even if a future settlement charters more than one.
            out.extend_from_slice(&(self.society.banks.len() as u32).to_le_bytes());
            for bank in &self.society.banks {
                push_bank_bytes(&mut out, bank);
            }
        }

        // The G8c-1 credit-cycle state. Omitted entirely for a non-finance settlement
        // (so every pre-G8c-1 canonical layout is byte-identical); present for a
        // finance settlement so the cycle trajectory — the regime rung, the issuer's
        // fiat base, and the per-tick boom/bust/structure/rate the M3 records carry —
        // is part of the "byte-identical iff future behaviour identical" identity. The
        // money_system + agent blocks above already carry the ledger and balances; the
        // ABCT records below are the cycle-specific state two runs through the
        // boom→stop→bust can otherwise differ in (the test-1 determinism tripwire).
        if let Some(cycle) = &self.cycle {
            push_cycle_runtime_bytes(&mut out, cycle);
            out.push(regime_tag(self.society.regime()));
            out.extend_from_slice(&(self.society.issuers.len() as u32).to_le_bytes());
            for issuer in &self.society.issuers {
                out.extend_from_slice(&issuer.fiat_issued.0.to_le_bytes());
                out.extend_from_slice(&issuer.fiat_retired.0.to_le_bytes());
                out.extend_from_slice(&issuer.fiat_credit_outstanding.0.to_le_bytes());
            }
            push_cycle_live_m2_bytes(&mut out, &self.society);
            out.extend_from_slice(&(self.society.m3_records.len() as u32).to_le_bytes());
            for record in &self.society.m3_records {
                out.push(regime_tag(record.regime));
                out.extend_from_slice(&record.public_specie.0.to_le_bytes());
                out.extend_from_slice(&record.public_fiat.0.to_le_bytes());
                out.extend_from_slice(&record.fiduciary.0.to_le_bytes());
                out.extend_from_slice(&record.boom_projects_started.to_le_bytes());
                out.extend_from_slice(&record.bust_abandoned_projects.to_le_bytes());
                out.extend_from_slice(&record.m2.structure_length_ticks_x100.to_le_bytes());
                out.extend_from_slice(&record.m2.market_rate_bps.unwrap_or(i64::MIN).to_le_bytes());
                out.extend_from_slice(&record.m2.capital_labor_consumed.to_le_bytes());
                out.extend_from_slice(&record.m2.capital_goods_consumed.to_le_bytes());
            }
        }

        // The G8c-2 tender-bench state. Omitted entirely for a non-bench settlement
        // (so every pre-G8c-2 canonical layout is byte-identical); present for a bench
        // so the surface + the tender-policy timeline its scenario carries are part of
        // the "byte-identical iff future behaviour identical" identity (a spot bench
        // and a debt bench, or two benches differing only in the surface tender, must
        // never collide). The agent/money blocks above already carry the live ledger +
        // balances; this adds the bench-specific policy steering.
        if let Some(bench) = &self.bench {
            out.push(bench_surface_tag(bench.surface));
            push_cycle_runtime_bytes_for_scenario(&mut out, &bench.scenario);
            out.push(public_spot_tender_tag(self.society.public_spot_tender));
            out.push(public_debt_tender_tag(self.society.public_debt_tender));
            out.push(bank_repayment_tender_tag(
                self.society.bank_repayment_tender,
            ));
            out.push(issuer_repayment_tender_tag(
                self.society.issuer_repayment_tender,
            ));
        }

        // The G8c-3 tax-overlay state. Omitted entirely for a non-tax settlement (so
        // every pre-G8c-3 canonical layout — and the tax-free cycle — is byte-identical);
        // present for a tax settlement so the configured + active receivability and the
        // issuer tax accounts (the levy/receipt/default outcome) are part of the
        // "byte-identical iff future behaviour identical" identity. The levy events are
        // already carried by the cycle scenario block above; this pins the settled
        // outcome the test-1 determinism tripwire reads back.
        if let Some(tax) = &self.tax {
            out.push(tax_receivability_tag(tax.receivability));
            out.extend_from_slice(&tax.levied.0.to_le_bytes());
            out.push(tax_receivability_tag(self.society.tax_receivability));
            out.extend_from_slice(&(self.society.issuers.len() as u32).to_le_bytes());
            for issuer in &self.society.issuers {
                out.extend_from_slice(&issuer.taxes_levied.0.to_le_bytes());
                out.extend_from_slice(&issuer.tax_receipts_fiat.0.to_le_bytes());
                out.extend_from_slice(&issuer.tax_receipts_specie.0.to_le_bytes());
                out.extend_from_slice(&issuer.taxes_defaulted.0.to_le_bytes());
            }
        }

        // Delivered exchange-stockpile units that are still awaiting econ credit
        // affect future transfers, so attribution belongs in the canonical state.
        out.extend_from_slice(&(self.pending_deposits.len() as u32).to_le_bytes());
        for (&(agent, good), &qty) in &self.pending_deposits {
            out.extend_from_slice(&agent.0.to_le_bytes());
            out.extend_from_slice(&good.0.to_le_bytes());
            out.extend_from_slice(&qty.to_le_bytes());
        }

        // The settlement commons (G4a dead-estate sink). It never feeds back into
        // stepping, so it is omitted entirely while empty — a no-death run's bytes
        // stay identical to the pre-G4a layout (the test-7 tripwire). Once a death
        // settles an estate here it becomes material public state two otherwise-equal
        // runs can differ in (e.g. a different starting gold leaves a different
        // settled balance), so it joins the digest, distinguishing post-death states
        // the live-agent block alone — which drops the freed colonist — would miss.
        // BTreeMap iteration is key-ordered, so the bytes are deterministic.
        let commons_nonempty =
            self.commons_gold > Gold::ZERO || self.commons_stock.values().any(|&qty| qty > 0);
        if commons_nonempty {
            out.extend_from_slice(&self.commons_gold.0.to_le_bytes());
            out.extend_from_slice(&(self.commons_stock.len() as u32).to_le_bytes());
            for (&good, &qty) in &self.commons_stock {
                out.extend_from_slice(&good.0.to_le_bytes());
                out.extend_from_slice(&qty.to_le_bytes());
            }
        }

        // The G4b demography runtime (the birth cadence + lifetime counters). It is
        // omitted entirely without a demography overlay, so a pre-G4b settlement's
        // bytes are unchanged; when present it steers future births, so it is part of
        // the future-behaviour identity. The per-household block is index-ordered
        // (deterministic). The per-colonist demography fields (household, age,
        // lifespan, seed) are appended in the colonist loop below, also gated.
        let is_demographic = self.demography.is_some();
        if let Some(demo) = &self.demography {
            push_demography_config_bytes(&mut out, demo);
            out.extend_from_slice(&self.birth_seq.to_le_bytes());
            out.extend_from_slice(&self.births_total.to_le_bytes());
            out.extend_from_slice(&self.old_age_deaths_total.to_le_bytes());
            // S14: the birth-block diagnostic counters are live run state, but they are
            // counted for ANY demography config — so serialize them ONLY on the
            // forage-commons path (where they are the load-bearing plateau diagnostic),
            // keeping every existing demography golden (`lineages`/frontier) byte-identical.
            if self.forage_commons_active() {
                out.extend_from_slice(&self.birth_block_interval.to_le_bytes());
                out.extend_from_slice(&self.birth_block_size_cap.to_le_bytes());
                out.extend_from_slice(&self.birth_block_hunger_ceiling.to_le_bytes());
                out.extend_from_slice(&self.birth_block_endowment.to_le_bytes());
            }
            out.extend_from_slice(&(self.households.len() as u32).to_le_bytes());
            for household in &self.households {
                match household.last_birth_tick {
                    Some(tick) => {
                        out.push(1);
                        out.extend_from_slice(&tick.to_le_bytes());
                    }
                    None => out.push(0),
                }
            }
        }

        // Econ agent state in id order, over the LIVE arena agents (a dead colonist
        // is freed by G4a real removal, so it drops out here). This includes every
        // mutable public field that can affect later stepping: holdings, labor, full
        // value scales, roles, and adaptive price beliefs.
        out.extend_from_slice(&(self.society.agents.len() as u32).to_le_bytes());
        for agent in self.society.agents.iter() {
            out.extend_from_slice(&agent.id.0.to_le_bytes());
            out.extend_from_slice(&agent.gold.0.to_le_bytes());
            out.extend_from_slice(&agent.labor_capacity.to_le_bytes());
            out.extend_from_slice(&agent.hunger_deficit.to_le_bytes());

            out.extend_from_slice(&(agent.roles.len() as u32).to_le_bytes());
            for &role in &agent.roles {
                push_role_bytes(&mut out, role);
            }

            out.extend_from_slice(&(agent.scale.len() as u32).to_le_bytes());
            for want in &agent.scale {
                push_want_kind_bytes(&mut out, want.kind);
                push_horizon_bytes(&mut out, want.horizon);
                out.extend_from_slice(&want.qty.to_le_bytes());
                out.push(u8::from(want.satisfied));
            }

            // A finance settlement (the G8c-1 credit cycle or a G8c-2 tender bench)
            // tracks no spatial goods (its goods live inside econ's own conserving
            // market/project machinery), yet its agents hold and trade goods that DO
            // steer the run — so serialize each agent's full (GoodId-sorted) stock
            // directly, with GOLD excluded (it is money, already serialized as
            // `agent.gold` + the money-system block). A spatial settlement keeps the
            // original path: every physical good an agent can hold is already in the
            // sorted `self.goods` (node goods ∪ starting goods; trade only relocates
            // them and no recipe mints a new one here), so serialize against it
            // directly, the debug check pinning that "complete and sorted" assumption.
            if self.cycle.is_some() || self.bench.is_some() {
                let mut held: Vec<(GoodId, u32)> = agent
                    .stock
                    .positive_goods()
                    .filter(|&good| good != GOLD)
                    .map(|good| (good, agent.stock.get(good)))
                    .collect();
                held.sort_by_key(|&(good, _)| good.0);
                out.extend_from_slice(&(held.len() as u32).to_le_bytes());
                for (good, qty) in held {
                    out.extend_from_slice(&good.0.to_le_bytes());
                    out.extend_from_slice(&qty.to_le_bytes());
                }
            } else {
                #[cfg(debug_assertions)]
                for good in agent.stock.positive_goods() {
                    debug_assert!(
                        good == GOLD || self.goods.contains(&good),
                        "agent holds an untracked good {good:?} the digest would miss"
                    );
                }
                out.extend_from_slice(&(self.goods.len() as u32).to_le_bytes());
                for &good in &self.goods {
                    out.extend_from_slice(&good.0.to_le_bytes());
                    out.extend_from_slice(&agent.stock.get(good).to_le_bytes());
                }
            }

            out.extend_from_slice(&(agent.expect.len() as u32).to_le_bytes());
            for belief in &agent.expect {
                out.extend_from_slice(&belief.expected.0.to_le_bytes());
                out.extend_from_slice(&belief.step.0.to_le_bytes());
                out.extend_from_slice(&belief.last_seen.to_le_bytes());
                // S11: the `observed` flag steers the grounded forecast (belief vs realized
                // fallback) and is NOT derivable from `last_seen` (0 is ambiguous between
                // never-observed and a tick-0 observation), so it is part of the
                // future-behaviour identity once forecasts run. Emitted only under the flag,
                // so a flag-off agent block is byte-identical to the pre-S11 stream.
                if entrepreneurial_serialized {
                    out.push(u8::from(belief.observed));
                }
            }
        }

        // Colonist need/liveness state in generation order.
        let has_estate_destinations = self
            .colonists
            .iter()
            .any(|colonist| colonist.estate_destination.is_some());
        // The S6 re-entry home (vocation+node) decides the revert target of a
        // displaced re-entrant, so it steers future ticks only while the phase can run.
        // Gate its per-colonist bytes on the same active-phase predicate as the
        // thresholds above: a re-entry-OFF or non-edible config never reads the home
        // and keeps its pre-S6 per-colonist layout byte-identical.
        let reentry_serialized = self.productive_reentry_can_run();
        // S12: the per-colonist `foraging` flag steers the next fast loop (forage vs
        // harvest WOOD) only while the own-labor phase can run; gate its byte on the same
        // active-phase predicate, so a non-own-labor config keeps its pre-S12
        // per-colonist layout byte-identical.
        let own_labor_serialized = self.own_labor_subsistence_can_run();
        // S15: the per-colonist `cultivating` flag steers the next fast loop (GoHarvest
        // grain vs forage/WOOD) only while the cultivation phase can run; gate its byte
        // on the active-phase predicate, so every pre-S15 config keeps its per-colonist
        // layout byte-identical.
        let cultivation_serialized = self.own_use_cultivation_active();
        // S22b: the per-colonist `cultivation_skill` scalar steers the next grain trip's haul
        // capacity only while the cultivation-skill phase can run; gate its bytes on the
        // active-phase predicate (which implies `cultivation_serialized`, so it nests inside the
        // cultivation block below), so every pre-S22b config keeps its per-colonist layout
        // byte-identical.
        let skill_serialized = self.cultivation_skill_active();
        // S22c: the per-colonist `cultivation_return_window` steers the next cultivation EXIT (the
        // profit-stay decision) only while the profit-driven-retention phase can run; gate its
        // bytes on the active-phase predicate (which implies `cultivation_serialized`, so it nests
        // inside the cultivation block below), so every pre-S22c config keeps its per-colonist
        // layout byte-identical.
        let retention_serialized = self.profit_driven_retention_active();
        // S22d: the per-colonist `cultivation_tenure` steers the next tool-build decision (a
        // cultivator invests once tenure ≥ `tool_build_patience`) only while the
        // durable-cultivation-capital phase can run; gate its byte on the active-phase predicate
        // (which implies `cultivation_serialized`, so it nests inside the cultivation block below),
        // so every pre-S22d config keeps its per-colonist layout byte-identical.
        let tool_serialized = self.durable_cultivation_tool_active();
        // S22f: the per-colonist commitment state (`commitment_remaining` / `commitment_renewals`)
        // steers the next cultivation EXIT (a bound agent cannot exit until the term expires) only
        // while the voluntary-commitment phase can run; gate its bytes on the active-phase predicate
        // (which implies `cultivation_serialized`, so it nests inside the cultivation block below), so
        // every pre-S22f config keeps its per-colonist layout byte-identical.
        let commitment_serialized = self.voluntary_cultivation_commitment_active();
        let commitment_norm_serialized = self.commitment_norm_gate_active();
        let abandonable_norm_serialized = self.abandonable_norm_active();
        let group_payoff_norm_serialized = self.group_payoff_imitation_active();
        // S23a: carried grain source steers the next idle-forfeiture pass while a cultivator is
        // hauling or awaiting transfer credit from a plot. Gate it with private land tenure, which
        // implies the cultivation block is active.
        let land_serialized = self.private_land_tenure_active();
        let secure_land_serialized = self.secure_land_tenure_active();
        out.extend_from_slice(&(self.colonists.len() as u32).to_le_bytes());
        for colonist in &self.colonists {
            out.extend_from_slice(&colonist.id.0.to_le_bytes());
            out.push(u8::from(colonist.alive));
            // The vocation tag (Consumer=0, Gatherer=1 — exactly G2b's
            // `u8::from(== Gatherer)` — plus Miller=2, Baker=3, and the G3b
            // Unassigned=4). Pre-G3a settlements only ever emit 0/1, so every
            // G2b/G2c digest is byte-identical; the producers extend the space.
            out.push(colonist.vocation.tag());
            out.extend_from_slice(&colonist.need.hunger.to_le_bytes());
            out.extend_from_slice(&colonist.need.warmth.to_le_bytes());
            out.extend_from_slice(&colonist.need.rest.to_le_bytes());
            out.extend_from_slice(&colonist.critical_streak.to_le_bytes());
            // Culture drives the next scale regeneration and the node the next
            // harvest target, so both belong in the future-behavior identity.
            out.extend_from_slice(&colonist.culture.time_preference_bps.to_le_bytes());
            out.extend_from_slice(&colonist.culture.leisure_weight_bps.to_le_bytes());
            // S11: the heritable forecast bias steers every future entrepreneurial
            // appraisal, so it joins the identity once forecasts run. Emitted only under
            // the flag (the same gated-block discipline as the per-belief `observed`
            // above), so a flag-off colonist block stays byte-identical to pre-S11.
            if entrepreneurial_serialized {
                out.extend_from_slice(&colonist.culture.forecast_bias_bps.to_le_bytes());
            }
            match colonist.node {
                Some(node) => {
                    out.push(1);
                    out.extend_from_slice(&node.0.to_le_bytes());
                }
                None => out.push(0),
            }
            if reentry_serialized {
                // The home vocation+node the colonist reverts to once fed
                // (`run_productive_reentry`). Two states with identical CURRENT
                // vocation/node but different homes diverge on the revert path, so the
                // home is part of the future-behaviour identity whenever re-entry runs.
                out.push(colonist.home_vocation.tag());
                match colonist.home_node {
                    Some(node) => {
                        out.push(1);
                        out.extend_from_slice(&node.0.to_le_bytes());
                    }
                    None => out.push(0),
                }
            } else if group_payoff_norm_serialized {
                // S24c group anchors prefer `home_node` over the current node, so the
                // home node steers future group membership whenever group-payoff imitation
                // is active. `home_vocation` is intentionally omitted here because S24c's
                // anchor does not read it.
                match colonist.home_node {
                    Some(node) => {
                        out.push(1);
                        out.extend_from_slice(&node.0.to_le_bytes());
                    }
                    None => out.push(0),
                }
            }
            if own_labor_serialized {
                // S12: whether the colonist is foraging — it steers the next fast loop
                // (forage the FORAGE node instead of harvesting WOOD). Two states with
                // identical current vocation/node but different foraging flags diverge
                // on the next task, so it is part of the future-behaviour identity
                // whenever the own-labor phase runs.
                out.push(u8::from(colonist.foraging));
            }
            if cultivation_serialized {
                // S15: whether the colonist is cultivating — it steers the next fast loop
                // (GoHarvest the grain node instead of foraging/WOOD) and the cultivation
                // phase. Mutually exclusive with `foraging`, so two states with identical
                // current vocation/node but different cultivating flags diverge on the
                // next task — part of the future-behaviour identity whenever the
                // cultivation phase runs.
                out.push(u8::from(colonist.cultivating));
                // S15: the cultivation pressure streak steers WHEN the colonist next
                // escalates to cultivation, so two states identical but for it diverge on
                // a future tick — part of the identity whenever the cultivation phase runs.
                out.extend_from_slice(&colonist.cultivate_pressure.to_le_bytes());
                // S15: delayed own-use grain drains are provenance-tracked. This latch
                // steers whether settled grain can still be converted after the visible
                // cultivating flag clears.
                out.push(u8::from(colonist.cultivation_stock_pending));
                // S22b: the cultivation skill scalar steers the next grain trip's per-trip haul
                // capacity, so two states identical but for it diverge on a future harvest —
                // part of the future-behaviour identity whenever the cultivation-skill phase
                // runs. Nested inside the cultivation block (the gate implies it), emitted only
                // under `skill_serialized`, so a pre-S22b colonist block stays byte-identical.
                if skill_serialized {
                    out.extend_from_slice(&colonist.cultivation_skill.to_le_bytes());
                }
                // S22c: the rolling cultivation-return window steers the next cultivation EXIT (a
                // clearing realized return keeps the colonist cultivating past the hunger exit), so
                // two states identical but for it diverge on a future tick — part of the
                // future-behaviour identity whenever the profit-driven-retention phase runs. Nested
                // inside the cultivation block (the gate implies it), emitted only under
                // `retention_serialized` (length-prefixed so the variable window is injective), so
                // a pre-S22c colonist block stays byte-identical.
                if retention_serialized {
                    let window = &colonist.cultivation_return_window;
                    out.extend_from_slice(&(window.len() as u32).to_le_bytes());
                    for entry in window {
                        out.extend_from_slice(&entry.tick.to_le_bytes());
                        out.extend_from_slice(&entry.cultivation_proceeds.to_le_bytes());
                        out.extend_from_slice(&entry.outside_proceeds.to_le_bytes());
                    }
                }
                // S22d: the cultivation tenure streak steers WHEN the colonist next invests in a
                // tool, so two states identical but for it diverge on a future build — part of the
                // identity whenever the durable-cultivation-capital phase runs. Nested inside the
                // cultivation block (the gate implies it), emitted only under `tool_serialized`, so
                // a pre-S22d colonist block stays byte-identical.
                if tool_serialized {
                    out.extend_from_slice(&colonist.cultivation_tenure.to_le_bytes());
                }
                // S22f: the commitment remaining/renewals steer the next cultivation EXIT (a bound
                // agent cannot exit until the term expires), so two states identical but for them
                // diverge on a future tick — part of the future-behaviour identity whenever the
                // voluntary-commitment phase runs. Nested inside the cultivation block (the gate
                // implies it), emitted only under `commitment_serialized`, so a pre-S22f colonist
                // block stays byte-identical.
                if commitment_serialized {
                    out.extend_from_slice(&colonist.commitment_remaining.to_le_bytes());
                    out.extend_from_slice(&colonist.commitment_renewals.to_le_bytes());
                }
                if commitment_norm_serialized {
                    out.push(u8::from(colonist.adopts_commitment_norm));
                    out.push(u8::from(colonist.commitment_norm_seed_adopter));
                    out.extend_from_slice(
                        &(colonist.commitment_norm_observations.len() as u32).to_le_bytes(),
                    );
                    for observation in &colonist.commitment_norm_observations {
                        out.extend_from_slice(&observation.tick.to_le_bytes());
                        out.extend_from_slice(&observation.hunger.to_le_bytes());
                        out.extend_from_slice(&observation.food_consumed.to_le_bytes());
                        out.extend_from_slice(&observation.salt_stock.to_le_bytes());
                        out.push(u8::from(observation.at_market));
                    }
                    if abandonable_norm_serialized {
                        match colonist.next_norm_bit {
                            Some(bit) => {
                                out.push(1);
                                out.push(u8::from(bit));
                            }
                            None => out.push(0),
                        }
                    }
                }
                if land_serialized {
                    match colonist.carried_grain_source {
                        Some(node) => {
                            out.push(1);
                            out.extend_from_slice(&node.0.to_le_bytes());
                        }
                        None => out.push(0),
                    }
                }
            }
            if role_choice_active {
                // The latent specialty (G3b) steers each tick's role-choice
                // re-appraisal, so it is part of the future-behavior identity. This
                // block is omitted entirely when role-choice cannot run (no latent pool
                // AND no S7 eligibility), preserving the pre-G3b canonical layout for
                // plain and seeded-only configs. Under S7 eligibility it serializes the
                // latent (mostly `None`) for every colonist, since role-choice now acts
                // on a tool-holder even with an empty seeded latent pool.
                match colonist.latent {
                    Some(recipe) => {
                        out.push(1);
                        push_recipe_id_bytes(&mut out, recipe);
                    }
                    None => out.push(0),
                }
            }
            if is_demographic {
                // The G4b demography fields steer aging, old-age mortality, the birth
                // roster, and culture inheritance, so they are part of the
                // future-behavior identity. Gated on a demography overlay, so the
                // pre-G4b canonical layout for every other config is unchanged.
                match colonist.household {
                    Some(h) => {
                        out.push(1);
                        out.extend_from_slice(&(h as u32).to_le_bytes());
                    }
                    None => out.push(0),
                }
                out.extend_from_slice(&colonist.age.to_le_bytes());
                match colonist.lifespan {
                    Some(life) => {
                        out.push(1);
                        out.extend_from_slice(&life.to_le_bytes());
                    }
                    None => out.push(0),
                }
                out.extend_from_slice(&colonist.seed.to_le_bytes());
                if secure_land_serialized {
                    match colonist.parent {
                        Some(parent) => {
                            out.push(1);
                            out.extend_from_slice(&parent.0.to_le_bytes());
                        }
                        None => out.push(0),
                    }
                }
            }
            if has_estate_destinations {
                match colonist.estate_destination {
                    Some(EstateDestination::Commons) => out.push(1),
                    Some(EstateDestination::Household { household, heir }) => {
                        out.push(2);
                        out.extend_from_slice(&(household as u32).to_le_bytes());
                        out.extend_from_slice(&heir.0.to_le_bytes());
                    }
                    None => out.push(0),
                }
            }
        }

        // S16: the produced-bread provenance ledger's per-agent counters. They steer the
        // future origin attribution of every bread→medium trade (a resold produced loaf
        // stays produced), so two settlements differing only in a per-agent produced balance
        // diverge on a future trade's attribution — part of the identity whenever the ledger
        // runs. Emitted ONLY on the active path (the gated-block discipline), so every
        // pre-S16 config keeps its canonical layout byte-identical. The whole-run
        // accumulators are pure functions of these counters plus the realized trades, so
        // they need no separate bytes (like the never-serialized emergence-probe diagnostics).
        if self.bread_provenance_active() {
            let produced = &self.bread_provenance.produced;
            out.extend_from_slice(&(produced.len() as u32).to_le_bytes());
            for (id, qty) in produced {
                out.extend_from_slice(&id.0.to_le_bytes());
                out.extend_from_slice(&qty.to_le_bytes());
            }
        }

        // Realized prices for the tracked goods.
        for &good in &self.goods {
            out.extend_from_slice(&good.0.to_le_bytes());
            match self.realized_price(good) {
                Some(price) => {
                    out.push(1);
                    out.extend_from_slice(&price.0.to_le_bytes());
                }
                None => out.push(0),
            }
        }
        out
    }
    /// A 64-bit FNV-1a digest of [`Settlement::canonical_bytes`] — a compact
    /// cross-run determinism check.
    pub fn digest(&self) -> u64 {
        let mut hash = 0xcbf2_9ce4_8422_2325u64;
        for byte in self.canonical_bytes() {
            hash ^= u64::from(byte);
            hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
        }
        hash
    }
}

pub(super) fn push_dynamics_bytes(out: &mut Vec<u8>, d: &NeedDynamics) {
    out.extend_from_slice(&d.need_max.to_le_bytes());
    out.extend_from_slice(&d.hunger_deplete.to_le_bytes());
    out.extend_from_slice(&d.warmth_deplete.to_le_bytes());
    out.extend_from_slice(&d.hunger_per_food.to_le_bytes());
    out.extend_from_slice(&d.warmth_per_wood.to_le_bytes());
    out.extend_from_slice(&d.rest_per_labor.to_le_bytes());
    out.extend_from_slice(&d.rest_recover.to_le_bytes());
    out.extend_from_slice(&d.hunger_critical.to_le_bytes());
    out.extend_from_slice(&d.death_window.to_le_bytes());
}
pub(super) fn push_barter_config_bytes(out: &mut Vec<u8>, barter: &BarterConfig) {
    push_mengerian_config_bytes(out, &barter.menger);
    out.extend_from_slice(&barter.medium_good.0.to_le_bytes());
    out.extend_from_slice(&barter.medium_want_qty.to_le_bytes());
    out.extend_from_slice(&barter.gatherer_medium_endowment.to_le_bytes());
    out.extend_from_slice(&barter.consumer_medium_endowment.to_le_bytes());
    // S9: the heterogeneous direct-use seed steers which colonists barter for SALT
    // pre-promotion (and thus the saleability the promotion reads), so both knobs
    // are part of the future-behaviour identity. Appended last so every pre-S9
    // barter config's prefix is unchanged.
    out.extend_from_slice(&barter.salt_direct_use_qty.to_le_bytes());
    out.extend_from_slice(&barter.salt_direct_use_period.to_le_bytes());
    // S19: zero is omitted so every pre-cycle barter scenario keeps its exact bytes;
    // nonzero cycle producer SALT seed must split the digest because it changes
    // generation holdings and future barter.
    if barter.cycle_producer_medium_endowment > 0 {
        out.extend_from_slice(&barter.cycle_producer_medium_endowment.to_le_bytes());
    }
}
pub(super) fn push_money_system_bytes(out: &mut Vec<u8>, money_system: &econ::ledger::MoneySystem) {
    out.extend_from_slice(&money_system.base.commodity_base.0.to_le_bytes());
    out.extend_from_slice(&money_system.base.fiat_base.0.to_le_bytes());
    out.extend_from_slice(&money_system.base.issuer_gold_vault.0.to_le_bytes());
    out.extend_from_slice(&money_system.base.issuer_fiat_unissued.0.to_le_bytes());
    out.extend_from_slice(&money_system.base.bank_reserves.0.to_le_bytes());
    out.extend_from_slice(&money_system.base.bank_fiat_reserves.0.to_le_bytes());
    out.extend_from_slice(&money_system.claims.demand_claims.0.to_le_bytes());
    out.extend_from_slice(&money_system.claims.fiduciary.0.to_le_bytes());
    out.extend_from_slice(&money_system.claims.time_deposits.0.to_le_bytes());
    out.extend_from_slice(&(money_system.balances.len() as u32).to_le_bytes());
    for balance in &money_system.balances {
        out.extend_from_slice(&balance.agent.0.to_le_bytes());
        out.extend_from_slice(&balance.public_specie.0.to_le_bytes());
        out.extend_from_slice(&balance.public_fiat.0.to_le_bytes());
        out.extend_from_slice(&(balance.demand_claims.len() as u32).to_le_bytes());
        for (bank, claim) in &balance.demand_claims {
            out.extend_from_slice(&bank.0.to_le_bytes());
            out.extend_from_slice(&claim.0.to_le_bytes());
        }
    }
}
/// Serialize the G8b chartered-bank balance sheet into the canonical digest. The
/// ledger block already carries the bank's reserves/fiduciary at the system level;
/// this adds the bank-owned fields (demand_deposits, loans_outstanding, the reserve
/// ratio, convertibility) so two runs that differ only in the bank's balance sheet
/// are distinguishable, plus the lending **policy** — which steers each tick's
/// `fiduciary_lend_capacity` (the per-tick cap, the one-unit loan template, the
/// enabled flag) yet is zero/default-free at generation, so two configs differing
/// only in it would otherwise collide before the first loan.
pub(super) fn push_bank_bytes(out: &mut Vec<u8>, bank: &Bank) {
    out.extend_from_slice(&bank.id.0.to_le_bytes());
    out.extend_from_slice(&bank.reserves.0.to_le_bytes());
    out.extend_from_slice(&bank.demand_deposits.0.to_le_bytes());
    out.extend_from_slice(&bank.time_deposits.0.to_le_bytes());
    out.extend_from_slice(&bank.loans_outstanding.0.to_le_bytes());
    out.extend_from_slice(&bank.fiduciary_issued.0.to_le_bytes());
    out.extend_from_slice(&bank.reserve_ratio_bps.0.to_le_bytes());
    out.push(u8::from(bank.convertible));
    push_bank_policy_bytes(out, &bank.policy);
}
pub(super) fn push_cycle_runtime_bytes(out: &mut Vec<u8>, cycle: &CycleRuntime) {
    out.push(cycle_kind_tag(cycle.kind));
    push_cycle_runtime_bytes_for_scenario(out, &cycle.scenario);
}
/// Encode a finance scenario's identity (name, seed, periods, money config, and the
/// full event timeline — including the G8c-2 `SetXTender` levers). Shared by the
/// cycle and the tender bench so a settlement's future behaviour is pinned by the
/// policy timeline it carries.
pub(super) fn push_cycle_runtime_bytes_for_scenario(out: &mut Vec<u8>, scenario: &MarketScenario) {
    out.push(scenario_name_tag(scenario.scenario));
    out.extend_from_slice(&scenario.seed.to_le_bytes());
    out.extend_from_slice(&scenario.periods.to_le_bytes());
    push_market_money_config_bytes(out, &scenario.money);
    out.extend_from_slice(&(scenario.events.len() as u32).to_le_bytes());
    for event in &scenario.events {
        out.extend_from_slice(&event.tick.0.to_le_bytes());
        push_event_kind_bytes(out, &event.kind);
    }
}
pub(super) fn push_cycle_live_m2_bytes(out: &mut Vec<u8>, society: &Society) {
    out.extend_from_slice(&(society.m2_projects.len() as u32).to_le_bytes());
    for project in &society.m2_projects {
        push_m2_project_bytes(out, project);
    }

    out.extend_from_slice(&(society.debts.len() as u32).to_le_bytes());
    for debt in &society.debts {
        push_debt_contract_bytes(out, debt);
    }

    out.extend_from_slice(&(society.project_funding_plans.len() as u32).to_le_bytes());
    for plan in &society.project_funding_plans {
        push_project_funding_plan_bytes(out, plan);
    }

    out.extend_from_slice(&(society.project_output_lots.len() as u32).to_le_bytes());
    for lot in &society.project_output_lots {
        push_project_output_lot_bytes(out, lot);
    }
}
pub(super) fn push_m2_project_bytes(out: &mut Vec<u8>, project: &M2Project) {
    out.extend_from_slice(&project.id.0.to_le_bytes());
    out.extend_from_slice(&project.owner.0.to_le_bytes());
    out.extend_from_slice(&project.line.0.to_le_bytes());
    out.push(m2_project_state_tag(project.state));
    out.extend_from_slice(&project.started_at.0.to_le_bytes());
    out.extend_from_slice(&project.maturity.0.to_le_bytes());
    out.extend_from_slice(&project.labor_advanced.to_le_bytes());
    out.extend_from_slice(&(project.input_goods_committed.len() as u32).to_le_bytes());
    for &(good, qty) in &project.input_goods_committed {
        out.extend_from_slice(&good.0.to_le_bytes());
        out.extend_from_slice(&qty.to_le_bytes());
    }
    out.extend_from_slice(&project.input_cost_basis.0.to_le_bytes());
    out.extend_from_slice(&project.advanced_gold.0.to_le_bytes());
    out.extend_from_slice(&project.expected_revenue.0.to_le_bytes());
    out.extend_from_slice(&project.output_good.0.to_le_bytes());
    out.extend_from_slice(&project.output_qty.to_le_bytes());
    out.extend_from_slice(&project.salvage_bps.to_le_bytes());
}
pub(super) fn push_debt_contract_bytes(out: &mut Vec<u8>, debt: &DebtContract) {
    out.extend_from_slice(&debt.id.0.to_le_bytes());
    push_credit_lender_bytes(out, debt.lender);
    out.extend_from_slice(&debt.borrower.0.to_le_bytes());
    out.extend_from_slice(&debt.opened_tick.0.to_le_bytes());
    out.extend_from_slice(&debt.due_tick.0.to_le_bytes());
    out.extend_from_slice(&debt.principal.0.to_le_bytes());
    out.extend_from_slice(&debt.due.0.to_le_bytes());
    out.extend_from_slice(&debt.paid.0.to_le_bytes());
    out.push(debt_state_tag(debt.state));
    push_debt_purpose_bytes(out, &debt.purpose);
    push_credit_source_bytes(out, debt.funding);
}
pub(super) fn push_project_funding_plan_bytes(out: &mut Vec<u8>, plan: &ProjectFundingPlan) {
    out.extend_from_slice(&plan.id.0.to_le_bytes());
    out.extend_from_slice(&plan.owner.0.to_le_bytes());
    out.extend_from_slice(&plan.line.0.to_le_bytes());
    out.extend_from_slice(&plan.created_tick.0.to_le_bytes());
    out.extend_from_slice(&plan.expires_tick.0.to_le_bytes());
    out.extend_from_slice(&plan.expected_revenue.0.to_le_bytes());
    out.extend_from_slice(&plan.input_cost_basis.0.to_le_bytes());
    out.extend_from_slice(&plan.required_labor.to_le_bytes());
    out.extend_from_slice(&plan.funding_horizon.to_le_bytes());
    out.extend_from_slice(&plan.borrowed_gold.0.to_le_bytes());
    out.extend_from_slice(&plan.future_due_committed.0.to_le_bytes());
    out.extend_from_slice(&plan.reserved_gold.0.to_le_bytes());
    match plan.started_project {
        Some(project) => {
            out.push(1);
            out.extend_from_slice(&project.0.to_le_bytes());
        }
        None => out.push(0),
    }
}
pub(super) fn push_project_output_lot_bytes(out: &mut Vec<u8>, lot: &ProjectOutputLot) {
    out.extend_from_slice(&lot.project.0.to_le_bytes());
    out.extend_from_slice(&lot.owner.0.to_le_bytes());
    out.extend_from_slice(&lot.good.0.to_le_bytes());
    out.extend_from_slice(&lot.qty_remaining.to_le_bytes());
    out.extend_from_slice(&lot.proceeds.0.to_le_bytes());
}
pub(super) fn push_market_money_config_bytes(out: &mut Vec<u8>, money: &MarketMoneyConfig) {
    match money {
        MarketMoneyConfig::Designated(money) => {
            out.push(0);
            out.extend_from_slice(&money.good.0.to_le_bytes());
        }
        MarketMoneyConfig::Emergent(menger) => {
            out.push(1);
            push_mengerian_config_bytes(out, menger);
        }
    }
}
pub(super) fn push_event_kind_bytes(out: &mut Vec<u8>, kind: &EventKind) {
    match kind {
        EventKind::DisableRecipe(recipe) => {
            out.push(0);
            out.push(recipe_id_tag(*recipe));
        }
        EventKind::SetRegime(regime) => {
            out.push(1);
            out.push(regime_tag(*regime));
        }
        EventKind::SetReserveRatio { bank, ratio } => {
            out.push(2);
            out.extend_from_slice(&bank.0.to_le_bytes());
            out.extend_from_slice(&ratio.0.to_le_bytes());
        }
        EventKind::SetBankConvertibility { bank, convertible } => {
            out.push(3);
            out.extend_from_slice(&bank.0.to_le_bytes());
            out.push(u8::from(*convertible));
        }
        EventKind::SetBankCreditPolicy { bank, policy } => {
            out.push(4);
            out.extend_from_slice(&bank.0.to_le_bytes());
            push_bank_policy_bytes(out, policy);
        }
        EventKind::StopBankCredit { bank } => {
            out.push(5);
            out.extend_from_slice(&bank.0.to_le_bytes());
        }
        EventKind::RedeemDemandClaims {
            bank,
            route,
            max_per_agent,
        } => {
            out.push(6);
            out.extend_from_slice(&bank.0.to_le_bytes());
            push_redemption_route_bytes(out, route);
            match max_per_agent {
                Some(max) => {
                    out.push(1);
                    out.extend_from_slice(&max.0.to_le_bytes());
                }
                None => out.push(0),
            }
        }
        EventKind::FiatPrint {
            issuer,
            amount,
            route,
        } => {
            out.push(7);
            out.extend_from_slice(&issuer.0.to_le_bytes());
            out.extend_from_slice(&amount.0.to_le_bytes());
            push_cantillon_route_bytes(out, route);
        }
        EventKind::ResetPublicSpotBook => out.push(8),
        EventKind::SetPublicSpotTender(tender) => {
            out.push(9);
            out.push(public_spot_tender_tag(*tender));
        }
        EventKind::SetPublicDebtTender(tender) => {
            out.push(10);
            out.push(public_debt_tender_tag(*tender));
        }
        EventKind::SetBankRepaymentTender(tender) => {
            out.push(11);
            out.push(bank_repayment_tender_tag(*tender));
        }
        EventKind::SetIssuerRepaymentTender(tender) => {
            out.push(12);
            out.push(issuer_repayment_tender_tag(*tender));
        }
        EventKind::SetLaborWageTender(tender) => {
            out.push(13);
            out.push(labor_wage_tender_tag(*tender));
        }
        EventKind::SetTaxReceivability(receivability) => {
            out.push(14);
            out.push(tax_receivability_tag(*receivability));
        }
        EventKind::LevyTax {
            agent,
            amount,
            due_tick,
        } => {
            out.push(15);
            out.extend_from_slice(&agent.0.to_le_bytes());
            out.extend_from_slice(&amount.0.to_le_bytes());
            out.extend_from_slice(&due_tick.0.to_le_bytes());
        }
        EventKind::SetDebtDueTick { debt, due_tick } => {
            out.push(16);
            out.extend_from_slice(&debt.0.to_le_bytes());
            out.extend_from_slice(&due_tick.0.to_le_bytes());
        }
        EventKind::SeedCommodityDebt {
            lender,
            borrower,
            principal,
            due,
            due_tick,
            purpose,
        } => {
            out.push(17);
            out.extend_from_slice(&lender.0.to_le_bytes());
            out.extend_from_slice(&borrower.0.to_le_bytes());
            out.extend_from_slice(&principal.0.to_le_bytes());
            out.extend_from_slice(&due.0.to_le_bytes());
            out.extend_from_slice(&due_tick.0.to_le_bytes());
            push_debt_purpose_bytes(out, purpose);
        }
        EventKind::SeedStock { agent, good, qty } => {
            out.push(18);
            out.extend_from_slice(&agent.0.to_le_bytes());
            out.extend_from_slice(&good.0.to_le_bytes());
            out.extend_from_slice(&qty.to_le_bytes());
        }
        EventKind::SetIssuerPolicy { issuer, policy } => {
            out.push(19);
            out.extend_from_slice(&issuer.0.to_le_bytes());
            push_issuer_policy_bytes(out, policy);
        }
        EventKind::StopIssuerCredit { issuer } => {
            out.push(20);
            out.extend_from_slice(&issuer.0.to_le_bytes());
        }
    }
}
pub(super) fn push_bank_policy_bytes(out: &mut Vec<u8>, policy: &BankPolicy) {
    out.extend_from_slice(&policy.max_new_fiduciary_per_tick.0.to_le_bytes());
    out.extend_from_slice(&policy.loan_present.0.to_le_bytes());
    out.push(policy.loan_horizon);
    out.extend_from_slice(&policy.loan_future_due.0.to_le_bytes());
    out.push(u8::from(policy.enabled));
}
pub(super) fn push_issuer_policy_bytes(out: &mut Vec<u8>, policy: &econ::issuer::IssuerPolicy) {
    out.push(u8::from(policy.fiscal_enabled));
    out.push(u8::from(policy.credit_enabled));
    out.extend_from_slice(&policy.max_fiscal_issue_per_tick.0.to_le_bytes());
    out.extend_from_slice(&policy.max_credit_issue_per_tick.0.to_le_bytes());
    out.extend_from_slice(&policy.loan_present.0.to_le_bytes());
    out.push(policy.loan_horizon);
    out.extend_from_slice(&policy.loan_future_due.0.to_le_bytes());
}
pub(super) fn push_redemption_route_bytes(out: &mut Vec<u8>, route: &RedemptionRoute) {
    match route {
        RedemptionRoute::Agents(agents) => {
            out.push(0);
            out.extend_from_slice(&(agents.len() as u32).to_le_bytes());
            for agent in agents {
                out.extend_from_slice(&agent.0.to_le_bytes());
            }
        }
        RedemptionRoute::AllClaimHolders => out.push(1),
    }
}
pub(super) fn push_cantillon_route_bytes(out: &mut Vec<u8>, route: &CantillonRoute) {
    match route {
        CantillonRoute::Agents(agents) => {
            out.push(0);
            out.extend_from_slice(&(agents.len() as u32).to_le_bytes());
            for agent in agents {
                out.extend_from_slice(&agent.0.to_le_bytes());
            }
        }
        CantillonRoute::Sector(sector) => {
            out.push(1);
            out.push(cantillon_sector_tag(*sector));
        }
        CantillonRoute::Helicopter => out.push(2),
    }
}
pub(super) fn push_debt_purpose_bytes(out: &mut Vec<u8>, purpose: &DebtPurpose) {
    match purpose {
        DebtPurpose::Consumption => out.push(0),
        DebtPurpose::ProjectFunding { plan, project } => {
            out.push(1);
            out.extend_from_slice(&plan.0.to_le_bytes());
            match project {
                Some(project) => {
                    out.push(1);
                    out.extend_from_slice(&project.0.to_le_bytes());
                }
                None => out.push(0),
            }
        }
        DebtPurpose::TaxLiability => out.push(2),
    }
}
pub(super) fn push_credit_lender_bytes(out: &mut Vec<u8>, lender: CreditLender) {
    match lender {
        CreditLender::Agent(agent) => {
            out.push(0);
            out.extend_from_slice(&agent.0.to_le_bytes());
        }
        CreditLender::Bank(bank) => {
            out.push(1);
            out.extend_from_slice(&bank.0.to_le_bytes());
        }
        CreditLender::Issuer(issuer) => {
            out.push(2);
            out.extend_from_slice(&issuer.0.to_le_bytes());
        }
    }
}
pub(super) fn push_credit_source_bytes(out: &mut Vec<u8>, source: CreditSource) {
    match source {
        CreditSource::Commodity => out.push(0),
        CreditSource::BankFiduciary(bank) => {
            out.push(1);
            out.extend_from_slice(&bank.0.to_le_bytes());
        }
        CreditSource::FiatFiscal(issuer) => {
            out.push(2);
            out.extend_from_slice(&issuer.0.to_le_bytes());
        }
        CreditSource::FiatCredit(issuer) => {
            out.push(3);
            out.extend_from_slice(&issuer.0.to_le_bytes());
        }
        CreditSource::Tax(issuer) => {
            out.push(4);
            out.extend_from_slice(&issuer.0.to_le_bytes());
        }
    }
}
/// Serialize an `Option<GoodId>` into the canonical digest: a present/absent tag
/// byte followed by the good id when present. Keeps the optional-good encoding
/// uniform across the emergent-money blocks.
pub(super) fn push_option_good_bytes(out: &mut Vec<u8>, good: Option<GoodId>) {
    match good {
        Some(good) => {
            out.push(1);
            out.extend_from_slice(&good.0.to_le_bytes());
        }
        None => out.push(0),
    }
}
/// Serialize the FULL Mengerian emergence runtime into the canonical digest: the
/// promotion-timing latch (the stable winner and how many consecutive ticks it
/// has led) and the saleability tracker's accumulated per-candidate state (the
/// running acceptance count plus the DISTINCT acceptor agents and counterpart
/// goods each candidate has been traded against). All of it steers the future
/// promotion decision — two barter states agreeing on holdings and the current
/// leader but differing in a stability counter or an acceptor set promote on
/// different future ticks — so it is part of the "byte-identical iff future
/// behaviour identical" identity. The member lists (not just their counts) are
/// serialized because a later acceptance only advances the eligibility counts
/// when its acceptor/counterpart is new. The tracker freezes once a good has
/// promoted (it stops observing), but is still serialized so the post-promotion
/// bytes stay a faithful function of the run. Candidate order is the tracker's
/// stored sorted order, so the bytes are deterministic.
pub(super) fn push_emergence_runtime_bytes(out: &mut Vec<u8>, emergence: &MengerianEmergence) {
    push_option_good_bytes(out, emergence.stable_winner());
    out.extend_from_slice(&emergence.stable_winner_ticks().to_le_bytes());
    let two_layer_saleability = emergence.config().two_layer_saleability;
    let tracker = emergence.tracker();
    out.extend_from_slice(&tracker.total_acceptances().to_le_bytes());
    let candidates = tracker.candidate_saleability();
    out.extend_from_slice(&(candidates.len() as u32).to_le_bytes());
    for candidate in candidates {
        out.extend_from_slice(&candidate.good.0.to_le_bytes());
        out.extend_from_slice(&candidate.acceptances.to_le_bytes());
        out.extend_from_slice(&(candidate.acceptor_agents.len() as u32).to_le_bytes());
        for agent in candidate.acceptor_agents {
            out.extend_from_slice(&agent.0.to_le_bytes());
        }
        out.extend_from_slice(&(candidate.counterpart_goods.len() as u32).to_le_bytes());
        for good in candidate.counterpart_goods {
            out.extend_from_slice(&good.0.to_le_bytes());
        }
        // S9: the indirect-exchange breadth (volume + distinct indirect acceptors +
        // distinct indirect targets) the strong-bar gate reads. A future acceptance
        // only advances the gate when its acceptor/target is new, so the member sets
        // — not just their counts — are part of the future-behaviour identity.
        out.extend_from_slice(&candidate.indirect_acceptances.to_le_bytes());
        out.extend_from_slice(&(candidate.indirect_acceptor_agents.len() as u32).to_le_bytes());
        for agent in candidate.indirect_acceptor_agents {
            out.extend_from_slice(&agent.0.to_le_bytes());
        }
        out.extend_from_slice(&(candidate.indirect_target_goods.len() as u32).to_le_bytes());
        for good in candidate.indirect_target_goods {
            out.extend_from_slice(&good.0.to_le_bytes());
        }
        if two_layer_saleability {
            out.extend_from_slice(&candidate.direct_acceptances.to_le_bytes());
            out.extend_from_slice(&(candidate.direct_acceptor_agents.len() as u32).to_le_bytes());
            for agent in candidate.direct_acceptor_agents {
                out.extend_from_slice(&agent.0.to_le_bytes());
            }
        }
    }
}
pub(super) fn push_mengerian_config_bytes(out: &mut Vec<u8>, menger: &MengerianConfig) {
    out.extend_from_slice(&(menger.candidate_goods.len() as u32).to_le_bytes());
    for good in &menger.candidate_goods {
        out.extend_from_slice(&good.0.to_le_bytes());
    }
    out.extend_from_slice(&menger.min_total_acceptances.to_le_bytes());
    out.extend_from_slice(&menger.promotion_threshold_bps.to_le_bytes());
    out.extend_from_slice(&menger.lead_margin_bps.to_le_bytes());
    out.extend_from_slice(&menger.min_acceptor_agents.to_le_bytes());
    out.extend_from_slice(&menger.min_counterpart_goods.to_le_bytes());
    out.extend_from_slice(&menger.stability_ticks.to_le_bytes());
    out.extend_from_slice(&menger.indirect_min_acceptance_share_bps.to_le_bytes());
    // S9 strong-bar gate: these steer the future promotion decision (they withhold
    // promotion until indirect breadth accrues / disable indirect acceptance), so
    // they are part of the future-behaviour identity. Appended last so every pre-S9
    // Mengerian config's prefix is unchanged.
    out.extend_from_slice(&menger.min_indirect_acceptances.to_le_bytes());
    out.extend_from_slice(&menger.min_indirect_acceptor_agents.to_le_bytes());
    out.extend_from_slice(&menger.min_indirect_target_goods.to_le_bytes());
    out.push(u8::from(menger.allow_indirect_acceptance));
    // S20 two-lane medium flag: it steers future ticks (whether agents post the
    // spend + sell medium lanes), so it is part of the future-behaviour identity
    // once set. Appended only when ON, so every flag-off Mengerian config (all the
    // S5–S19 + econ + emergence goldens) keeps its exact prior byte layout.
    if menger.multi_offer_medium {
        out.push(1);
    }
    // S21a durability-aware marketability: the per-good decay/carry table only
    // steers acceptance when the lever is ON (the agent gate fires solely on the
    // flag), so it is part of the future-behaviour identity only then. Appended
    // only when ON — mirroring `multi_offer_medium` — so every flag-off Mengerian
    // config (all the S5–S20 + econ + emergence + demographic goldens) keeps its
    // exact prior byte layout, and a behaviour-inert table edit (flag off) does
    // not split the digest.
    if menger.durability_aware_acceptance {
        out.push(1);
        out.extend_from_slice(&menger.marketability.hold_horizon.to_le_bytes());
        out.extend_from_slice(&(menger.marketability.goods.len() as u32).to_le_bytes());
        for (good, marketability) in &menger.marketability.goods {
            out.extend_from_slice(&good.0.to_le_bytes());
            out.extend_from_slice(&marketability.decay_bps.to_le_bytes());
            out.extend_from_slice(&marketability.carry_cost.to_le_bytes());
        }
    }
    if menger.two_layer_saleability {
        out.push(1);
        out.extend_from_slice(&menger.min_direct_use_acceptors.to_le_bytes());
    }
}
pub(super) fn push_demography_config_bytes(out: &mut Vec<u8>, demo: &DemographyConfig) {
    out.extend_from_slice(&(demo.households.len() as u32).to_le_bytes());
    for household in &demo.households {
        out.extend_from_slice(&household.founders.to_le_bytes());
        out.extend_from_slice(&household.time_preference_base_bps.to_le_bytes());
        out.extend_from_slice(&household.food_provision.to_le_bytes());
        out.extend_from_slice(&household.wood_provision.to_le_bytes());
        out.extend_from_slice(&household.starting_gold.to_le_bytes());
        out.extend_from_slice(&household.starting_food.to_le_bytes());
        out.extend_from_slice(&household.starting_wood.to_le_bytes());
    }
    out.extend_from_slice(&demo.ticks_per_year.to_le_bytes());
    out.extend_from_slice(&demo.old_age_onset_years.to_le_bytes());
    out.extend_from_slice(&demo.lifespan_span_years.to_le_bytes());
    out.extend_from_slice(&demo.birth_interval.to_le_bytes());
    out.extend_from_slice(&demo.birth_hunger_ceiling.to_le_bytes());
    out.extend_from_slice(&demo.max_household_size.to_le_bytes());
    out.extend_from_slice(&demo.child_food_endowment.to_le_bytes());
    out.extend_from_slice(&demo.child_gold_endowment.to_le_bytes());
    out.extend_from_slice(&demo.mutation_delta_bps.to_le_bytes());
    // S13: the spatial-households flag steers future ticks (whether newborns get a
    // world agent and whether lineage members can be assigned world tasks), so it is
    // part of the future-behaviour identity once it is set. Appended only when ON, so
    // every flag-off demography config (the `lineages`/frontier goldens) keeps its
    // exact pre-S13 byte layout. The founder/newborn world agents themselves are
    // already in `world.canonical_bytes`; this pins the flag's own identity.
    if demo.spatial_households {
        out.push(1);
    }
}
pub(super) fn push_role_bytes(out: &mut Vec<u8>, role: Role) {
    out.push(match role {
        Role::Household => 0,
        Role::Producer => 1,
        Role::Trader => 2,
        Role::Capitalist => 3,
        Role::Worker => 4,
        Role::Consumer => 5,
    });
}
pub(super) fn push_want_kind_bytes(out: &mut Vec<u8>, kind: WantKind) {
    match kind {
        WantKind::Good(good) => {
            out.push(0);
            out.extend_from_slice(&good.0.to_le_bytes());
        }
        WantKind::Leisure => out.push(1),
    }
}
pub(super) fn push_horizon_bytes(out: &mut Vec<u8>, horizon: Horizon) {
    match horizon {
        Horizon::Now => out.push(0),
        Horizon::Next => out.push(1),
        Horizon::Later(ticks) => {
            out.push(2);
            out.push(ticks);
        }
    }
}
pub(super) fn push_recipe_bytes(out: &mut Vec<u8>, recipe: &Recipe) {
    push_recipe_id_bytes(out, recipe.id);
    out.extend_from_slice(&(recipe.name.len() as u32).to_le_bytes());
    out.extend_from_slice(recipe.name.as_bytes());
    out.extend_from_slice(&recipe.labor.to_le_bytes());
    match recipe.input_good {
        Some((good, qty)) => {
            out.push(1);
            out.extend_from_slice(&good.0.to_le_bytes());
            out.extend_from_slice(&qty.to_le_bytes());
        }
        None => out.push(0),
    }
    match recipe.required_tool {
        Some(good) => {
            out.push(1);
            out.extend_from_slice(&good.0.to_le_bytes());
        }
        None => out.push(0),
    }
    out.extend_from_slice(&recipe.output_good.0.to_le_bytes());
    out.extend_from_slice(&recipe.output_qty.to_le_bytes());
    out.push(u8::from(recipe.enabled));
}
pub(super) fn push_recipe_id_bytes(out: &mut Vec<u8>, id: RecipeId) {
    out.push(match id {
        RecipeId::GatherFood => 0,
        RecipeId::CutWood => 1,
        RecipeId::FishWithNet => 2,
        RecipeId::Mill => 3,
        RecipeId::Bake => 4,
        // G6b content recipes; pre-G6b configs never serialize these, so existing
        // digests are byte-identical.
        RecipeId::Research => 5,
        RecipeId::Confect => 6,
        // S15 own-use cultivation; carried only by the gated cultivation content set,
        // so every pre-S15 config's recipe stream is byte-identical.
        RecipeId::Cultivate => 7,
        // S19 cycle recipes; carried only by the gated cycle content.
        RecipeId::CycleA => 8,
        RecipeId::CycleB => 9,
        RecipeId::CycleC => 10,
    });
}

// ---------------------------------------------------------------------------
// DIGEST-COVERAGE GUARD (compile-time exhaustiveness)
//
// `canonical_bytes` is a hand-maintained, per-flag serialization implementing
// "byte-identical iff future behavior identical". Its failure mode is SILENT:
// a new behavior-steering field omitted from the digest lets two divergent
// settlements digest equal, quietly voiding the determinism tripwire and every
// byte-identity regression.
//
// The functions below destructure each behavior-steering struct WITHOUT `..`.
// Adding a field to any of them is therefore a compile error on this file
// until the field is classified into one of the two groups — the digest-or-
// inert decision can no longer be skipped by accident, and the classification
// line lands in the same diff as the field, where review can see it.
//
// Scope notes:
// - Enums need no guard: every `match` in this file is wildcard-free, so a new
//   variant is already a compile error in its push_* helper.
// - `SettlementConfig` itself is deliberately absent: generate-time knobs are
//   captured by the digested initial state, and every knob that steers a LATER
//   tick does so through a `Settlement` field or a stored overlay config — all
//   of which are guarded below.
// - The per-flag `canonical_bytes_include_*` tests remain the value-level
//   check that DIGESTED fields actually reach the byte stream.
// ---------------------------------------------------------------------------

#[allow(dead_code)]
fn digest_coverage_settlement(v: &Settlement) {
    let Settlement {
        // DIGESTED — serialized (directly or via a push_* helper) by canonical_bytes:
        generation_seed: _,
        world: _,
        society: _,
        colonists: _,
        dynamics: _,
        known: _,
        exchange: _,
        carry_cap: _,
        goods: _,
        pending_deposits: _,
        chain: _,
        capital_builds: _,
        capital_loans: _,
        next_capital_project_id: _,
        bread_provenance: _,
        subsistence_commons_stock: _,
        subsistence_commons_cap: _,
        subsistence_commons_regen: _,
        wage_escrow_gold: _,
        wage_escrows: _,
        next_wage_contract_id: _,
        wage_retained_earnings: _,
        wage_proceeds_buckets: _,
        share_contracts: _,
        next_share_contract_id: _,
        in_kind_contracts: _,
        next_in_kind_contract_id: _,
        cultivation_tool_builds: _,
        next_cultivation_tool_project_id: _,
        endowed_households: _,
        land_plots: _,
        land_market_plots: _,
        land_fee_pool_salt: _,
        econ_tick: _,
        commons_gold: _,
        commons_stock: _,
        demography: _,
        households: _,
        birth_seq: _,
        births_total: _,
        old_age_deaths_total: _,
        birth_block_interval: _,
        birth_block_size_cap: _,
        birth_block_hunger_ceiling: _,
        birth_block_endowment: _,
        barter: _,
        knowledge: _,
        tier2_unlocked_at: _,
        bank: _,
        cycle: _,
        bench: _,
        tax: _,
        closed_circulation: _,
        // NOT DIGESTED — inherited baseline at guard introduction (each was
        // implicitly omitted by the hand-maintained digest; kept as-is so the
        // guard lands with zero byte-stream change). Every NEW field added below
        // must instead be classified deliberately: digest it, or move it here
        // with a one-line rationale for why it cannot steer a future tick.
        live_colonist_slots: _,
        colonist_slot_by_id: _,
        forage_node_id: _,
        // derived-inert: every living world agent digests its own move_speed
        // (world.rs), and births require a living parent, so this stored copy
        // can never be the only divergence between two digest-equal settlements.
        move_speed: _,
        money_rejection_goods: _,
        trader_ids: _,
        tools_built: _,
        mortal_producer_old_age_deaths: _,
        role_readoptions: _,
        mortal_capital_builds: _,
        producer_tool_inheritances: _,
        heirless_producer_deaths: _,
        heir_tool_adoptions: _,
        producer_house_hearth_food_minted: _,
        non_producer_hearth_food_minted: _,
        producer_house_births: _,
        producer_house_deaths: _,
        producer_house_person_ticks: _,
        producer_recipe_pay_rejections: _,
        producer_build_rejections: _,
        producer_adoption_rejections: _,
        producer_tool_inheritors: _,
        last_capital_decisions: _,
        peak_pre_promotion_hunger: _,
        critical_ticks_pre_promotion: _,
        multigood: _,
        acquisition: _,
        earned_provisioning: _,
        birth_stock_wants_emitted: _,
        birth_stock_attributable_purchases: _,
        birth_stock_below_target_agents: _,
        birth_stock_reached_agents: _,
        birth_stock_held_max: _,
        birth_stock_held_at_death: _,
        birth_stock_eligible_opportunities: _,
        birth_stock_injections_completed: _,
        birth_stock_source_shortfalls: _,
        ignition_injected_qty: _,
        ignition_gate_blocked_interval: _,
        ignition_gate_extinct: _,
        ignition_gate_blocked_cap: _,
        ignition_gate_blocked_hunger: _,
        ignition_gate_suppressed_at_target: _,
        ignition_gate_donor_shortfall: _,
        producer_birth_funded_by_channel: _,
        producer_birth_funded_intervention: _,
        birth_stock_injection_records: _,
        birth_stock_births_by_household: _,
        last_birth_stock_attribution_snapshot: _,
        // NOT DIGESTED: runtime-only diagnostics, including C3R.g's nested
        // role-choice counters and Baker-hold ticks; no decision path reads them.
        saving_allocation_obs: _,
        birth_gate_obs: _,
        saving_obs_stock_tick: _,
        saving_obs_pending_offerable: _,
        bootstrap_trace: _,
        bread_seller_trace: _,
        seeded_surplus_trace: _,
        seeded_minted_bread_sold_for_salt: _,
        emergency_bread_provisioned: _,
        subsistence_commons_phi_bps: _,
        subsistence_commons_drawn_total: _,
        subsistence_commons_unmet_total: _,
        subsistence_commons_depleted_ticks: _,
        subsistence_commons_shortfall_ticks: _,
        subsistence_commons_eligible_need_total: _,
        wage_workers_ever: _,
        wage_employers_ever: _,
        wage_hires_total: _,
        wage_hires_post_promotion: _,
        wage_below_ask_not_hired: _,
        wage_endowment_funded_wages: _,
        wage_financed_output_buys: _,
        wage_nonowner_output_buys: _,
        wage_circular_loop_turnovers: _,
        share_workers_ever: _,
        share_owners_ever: _,
        share_contracts_total: _,
        share_voluntary_contracts_total: _,
        share_forced_contracts_total: _,
        share_renewals_total: _,
        share_worker_bread_income: _,
        share_owner_bread_income: _,
        share_worker_declined: _,
        share_worker_unmatched: _,
        share_forward_only_eligibility: _,
        share_renewal_hints_total: _,
        share_renewal_fed_out: _,
        share_renewal_base_ineligible: _,
        share_renewal_owner_not_candidate: _,
        share_renewal_bread_declined: _,
        share_renewal_matched_elsewhere: _,
        share_owner_candidates_total: _,
        share_owner_no_atcap_plot: _,
        share_stock_opportunity_refusal: _,
        share_reservation_collision: _,
        share_stock_drawdown: _,
        share_unattributed_share_deposit: _,
        share_owner_grain_settled: _,
        share_successions_total: _,
        share_succession_heir_declined: _,
        share_succession_worker_re_declined: _,
        share_post_succession_renewals: _,
        share_succeeded_live_ids: _,
        in_kind_workers_ever: _,
        in_kind_employers_ever: _,
        in_kind_hires_total: _,
        in_kind_worker_advance_bread: _,
        in_kind_employer_bread_income: _,
        in_kind_expected_output_total: _,
        in_kind_worker_declined: _,
        in_kind_worker_unmatched: _,
        in_kind_owner_candidates_total: _,
        in_kind_owner_no_atcap_plot: _,
        in_kind_owner_insufficient_fund: _,
        in_kind_productivity_declined: _,
        in_kind_reservation_collision: _,
        in_kind_stock_drawdown: _,
        in_kind_unattributed_deposit: _,
        in_kind_employer_grain_settled: _,
        in_kind_endowment_funded_hires: _,
        in_kind_term_starvations: _,
        ever_landowner_ids: _,
        owner_first_claim_tick: _,
        owner_age_at_first_claim: _,
        owner_tenure_before_death: _,
        owner_bread_consumed: _,
        owner_surplus_sold_before_death: _,
        owner_inventory_at_death: _,
        inherited_stock_to_heirs: _,
        buyer_purchases_by_owner_age_cohort: _,
        owner_seller_attributed_bought: _,
        cultivation_skill_producers: _,
        cultivation_grain_harvested: _,
        cultivation_bread_produced: _,
        cultivation_proceeds_scratch: _,
        profit_retained_ids: _,
        profit_retained_ever: _,
        commitment_committed_ever: _,
        commitment_uptake: _,
        commitment_fiat_ever: _,
        commitment_below_floor_ever: _,
        commitment_exit_override_ids: _,
        commitment_exit_override_ever: _,
        commitment_norm_copy_events: _,
        commitment_norm_flip_events: _,
        commitment_norm_adoptions: _,
        commitment_norm_abandonments: _,
        commitment_norm_imitation_adopters: _,
        commitment_norm_group_covariance_sum: _,
        commitment_norm_group_covariance_count: _,
        cultivation_tool_producers: _,
        cultivation_tools_built: _,
        cultivation_tool_wood_consumed: _,
        cultivation_tools_destroyed: _,
        endowed_cultivation_tools_total: _,
        endowed_member_ids: _,
        cultivation_tool_inherited_total: _,
        cultivation_tool_inheritor_ids: _,
        secure_land_inheritance_events: _,
        secure_land_owner_old_age_deaths_total: _,
        secure_land_inherit_eligible_owner_deaths_total: _,
        secure_land_stranded_shares_total: _,
        land_claims_total: _,
        land_idle_losses_total: _,
        land_harvest_denials_total: _,
        land_owner_gate_denials_total: _,
        land_nonowner_harvest_of_owned_total: _,
        land_reclaims_by_other_total: _,
        land_marginal_nonowner_claims_total: _,
        land_lapsed_reentry_worse_total: _,
        land_plot_harvest_totals: _,
        land_lapsed_losses: _,
        land_lost_prior_owners: _,
        land_market_yield_this_tick: _,
        land_market_sales: _,
        land_market_trade_count: _,
        land_market_pre_promotion_trade_count: _,
        land_market_carrying_paid_total: _,
        land_market_pre_promotion_charges: _,
        land_market_foreclosure_listings_total: _,
        land_market_priced_out_total: _,
        land_market_lapsed_priced_out_total: _,
        land_market_ask_bid_gap_sum: _,
        land_market_ask_bid_gap_count: _,
        land_market_title_history: _,
        last_report: _,
        starvation_deaths_total: _,
        barter_medium: _,
        salt_direct_use: _,
        shadow_cycle_cache: _,
        closure: _,
        burden: _,
        #[cfg(test)]
            test_fault_mint_birth_gold: _,
    } = *v;
}

#[allow(dead_code)]
fn digest_coverage_saving_allocation_obs(v: &SavingAllocationObs) {
    let SavingAllocationObs {
        // NOT DIGESTED: C3R.g runtime-only observation; no decision path reads it.
        role_choice_diag: _,
        filled: _,
        no_bid_posted: _,
        self_ask_only: _,
        no_executable_ask_in_window: _,
        all_asks_above_limit: _,
        competitive_loss: _,
        execution_residual: _,
        no_spot_pass_ticks: _,
        drops: _,
        phys_produced: _,
        phys_consumed: _,
        phys_net_delta: _,
        phys_within_phase_ambiguous: _,
        death_phase: _,
        pre_market_phase: _,
        market_phase: _,
        production_own_use_phase: _,
        birth_phase: _,
        end_of_tick_phase: _,
        supply_series: _,
    } = *v;
}

#[allow(dead_code)]
fn digest_coverage_chain_config(v: &ChainConfig) {
    let ChainConfig {
        // DIGESTED — serialized (directly or via a push_* helper) by canonical_bytes:
        content: _,
        operating_cost: _,
        subsistence_on_grain: _,
        forage_yield: _,
        forage_hunger_in: _,
        forage_hunger_out: _,
        cultivate_hunger_in: _,
        cultivate_hunger_out: _,
        cultivate_consume: _,
        cultivate_patience: _,
        cultivation_skill: _,
        return_window: _,
        retention_margin_bps: _,
        retention_material_floor: _,
        skill_gain: _,
        skill_decay: _,
        skill_cap: _,
        skill_haul_ceiling: _,
        tool_build_patience: _,
        cultivation_tool_haul_ceiling: _,
        cultivation_tool_non_durable: _,
        endowed_tool_count: _,
        cultivation_tool_inheritance: _,
        commitment_term: _,
        commitment_entry_floor: _,
        commitment_fiat_pin: _,
        abandonable_norm: _,
        group_payoff_imitation: _,
        fixed_commitment_norm_prevalence: _,
        commitment_seed_share_bps: _,
        imitation_period: _,
        imitation_window: _,
        imitation_margin_bps: _,
        imitation_radius: _,
        imitation_max_models: _,
        food_window_target: _,
        no_imitation: _,
        random_imitation: _,
        salt_in_score: _,
        land_idle_limit: _,
        reclaim_reserved_for_prior_owner: _,
        land_good_plots: _,
        land_marginal_plots: _,
        land_marginal_regen: _,
        secure_land_tenure: _,
        inheritance_regime: _,
        mortal_landowner_demography: _,
        rival_subsistence_commons: _,
        rival_subsistence_commons_phi_bps: _,
        wage_labor: _,
        wage_labor_mode: _,
        share_tenancy: _,
        share_tenancy_mode: _,
        share_forward_provisioning: _,
        share_contract_succession: _,
        in_kind_wage: _,
        mortal_chain_producers: _,
        mortal_producer_inheritance: _,
        mortal_producer_tool_inheritance: _,
        producer_house_cap: _,
        earned_provisioning: _,
        producer_stock_provisioning_control: _,
        birth_stock_saving: _,
        birth_stock_saving_mode: _,
        saving_allocation_obs: _,
        birth_gate_obs: _,
        share_bps: _,
        share_term: _,
        land_carrying_cost: _,
        land_price_cap_factor: _,
        retire_food_mints: _,
        capital_advance: _,
        perishable_decay_bps: _,
        subsistence_advance: _,
        input_advance: _,
        recurring_motive: _,
        project_input_bids: _,
        producer_subsistence: _,
        reentry_hunger_in: _,
        reentry_hunger_out: _,
        per_agent_capital: _,
        capital_payback_cycles: _,
        tool_build_wood: _,
        tool_build_labor: _,
        capital_build_hunger_max: _,
        throughput: _,
        seeded_surplus_bread: _,
        tier2_threshold: _,
        gatherer_food_cushion: _,
        emergency_hunger_threshold: _,
        birth_stock_ignition_at: _,
        producer_house_starting_staple: _,
        producer_support_until_tick: _,
        // NOT DIGESTED — inherited baseline at guard introduction (each was
        // implicitly omitted by the hand-maintained digest; kept as-is so the
        // guard lands with zero byte-stream change). Every NEW field added below
        // must instead be classified deliberately: digest it, or move it here
        // with a one-line rationale for why it cannot steer a future tick.
        millers: _,
        bakers: _,
        latent_millers: _,
        latent_bakers: _,
        bread_is_staple: _,
        own_labor_subsistence: _,
        forage_commons: _,
        own_use_cultivation: _,
        cultivation_sells_surplus: _,
        multigood_money: _,
        household_barter_cultivation: _,
        endogenous_cultivation_entry: _,
        profit_driven_retention: _,
        durable_cultivation_tool: _,
        endowed_cultivation_capital: _,
        voluntary_cultivation_commitment: _,
        commitment_norm_spread: _,
        private_land_tenure: _,
        harvest_gate: _,
        forfeit_on_idle: _,
        land_market: _,
        acquisition_ledger: _,
        productive_reentry: _,
        tool_acquisition_eligibility: _,
        producible_capital: _,
        entrepreneurial_forecasts: _,
        miller_grain_buffer: _,
        baker_flour_buffer: _,
        latent_flour_seed: _,
        bread_buffer: _,
        consumer_staple_buffer: _,
        wood_buffer: _,
        consumer_wood_buffer: _,
        producer_gold: _,
        scholars: _,
        confectioners: _,
        scholar_grain_buffer: _,
        confectioner_flour_buffer: _,
        cycle_a_producers: _,
        cycle_b_producers: _,
        cycle_c_producers: _,
        cycle_a_input_buffer: _,
        cycle_b_input_buffer: _,
        cycle_c_input_buffer: _,
    } = *v;
}

#[allow(dead_code)]
fn digest_coverage_tender_policy(v: &TenderPolicy) {
    let TenderPolicy {
        // DIGESTED — serialized (directly or via a push_* helper) by canonical_bytes:
        spot: _,
        wage: _,
        debt: _,
        // NOT DIGESTED — inherited baseline at guard introduction (each was
        // implicitly omitted by the hand-maintained digest; kept as-is so the
        // guard lands with zero byte-stream change). Every NEW field added below
        // must instead be classified deliberately: digest it, or move it here
        // with a one-line rationale for why it cannot steer a future tick.
        bank_repayment: _,
        issuer_repayment: _,
    } = *v;
}

#[allow(dead_code)]
fn digest_coverage_tax_policy(v: &TaxPolicy) {
    let TaxPolicy {
        // DIGESTED — serialized (directly or via a push_* helper) by canonical_bytes:
        receivability: _,
        // NOT DIGESTED — inherited baseline at guard introduction (each was
        // implicitly omitted by the hand-maintained digest; kept as-is so the
        // guard lands with zero byte-stream change). Every NEW field added below
        // must instead be classified deliberately: digest it, or move it here
        // with a one-line rationale for why it cannot steer a future tick.
        levies: _,
    } = *v;
}

#[allow(dead_code)]
fn digest_coverage_barter_config(v: &BarterConfig) {
    let BarterConfig {
        // DIGESTED — serialized (directly or via a push_* helper) by canonical_bytes:
        menger: _,
        medium_good: _,
        medium_want_qty: _,
        gatherer_medium_endowment: _,
        consumer_medium_endowment: _,
        cycle_producer_medium_endowment: _,
        salt_direct_use_qty: _,
        salt_direct_use_period: _,
    } = *v;
}

#[allow(dead_code)]
fn digest_coverage_demography_config(v: &DemographyConfig) {
    let DemographyConfig {
        // DIGESTED — serialized (directly or via a push_* helper) by canonical_bytes:
        households: _,
        ticks_per_year: _,
        old_age_onset_years: _,
        lifespan_span_years: _,
        birth_interval: _,
        birth_hunger_ceiling: _,
        max_household_size: _,
        child_food_endowment: _,
        child_gold_endowment: _,
        mutation_delta_bps: _,
        spatial_households: _,
    } = *v;
}

#[allow(dead_code)]
fn digest_coverage_mengerian_config(v: &MengerianConfig) {
    let MengerianConfig {
        // DIGESTED — serialized (directly or via a push_* helper) by canonical_bytes:
        candidate_goods: _,
        min_total_acceptances: _,
        promotion_threshold_bps: _,
        lead_margin_bps: _,
        min_acceptor_agents: _,
        min_counterpart_goods: _,
        stability_ticks: _,
        indirect_min_acceptance_share_bps: _,
        min_indirect_acceptances: _,
        min_indirect_acceptor_agents: _,
        min_indirect_target_goods: _,
        allow_indirect_acceptance: _,
        multi_offer_medium: _,
        durability_aware_acceptance: _,
        two_layer_saleability: _,
        min_direct_use_acceptors: _,
        marketability: _,
    } = *v;
}

#[allow(dead_code)]
fn digest_coverage_bank_policy(v: &BankPolicy) {
    let BankPolicy {
        // DIGESTED — serialized (directly or via a push_* helper) by canonical_bytes:
        max_new_fiduciary_per_tick: _,
        loan_present: _,
        loan_horizon: _,
        loan_future_due: _,
        enabled: _,
    } = *v;
}

#[allow(dead_code)]
fn digest_coverage_issuer_policy(v: &econ::issuer::IssuerPolicy) {
    let econ::issuer::IssuerPolicy {
        // DIGESTED — serialized (directly or via a push_* helper) by canonical_bytes:
        fiscal_enabled: _,
        credit_enabled: _,
        max_fiscal_issue_per_tick: _,
        max_credit_issue_per_tick: _,
        loan_present: _,
        loan_horizon: _,
        loan_future_due: _,
    } = *v;
}

#[allow(dead_code)]
fn digest_coverage_need_dynamics(v: &NeedDynamics) {
    let NeedDynamics {
        // DIGESTED — serialized (directly or via a push_* helper) by canonical_bytes:
        need_max: _,
        hunger_deplete: _,
        warmth_deplete: _,
        hunger_per_food: _,
        warmth_per_wood: _,
        rest_per_labor: _,
        rest_recover: _,
        hunger_critical: _,
        death_window: _,
    } = *v;
}
