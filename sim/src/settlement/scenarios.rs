//! Frontier scenario preset constructors.
//!
//! The `SettlementConfig::frontier_*` constructors — the named scenario presets each
//! milestone pins its goldens against. Extracted verbatim from `mod.rs` (pure code
//! motion) into this sibling `impl SettlementConfig` block; all were already `pub`
//! inherent methods, so every call site resolves unchanged with no re-import.

use super::*;

impl SettlementConfig {
    /// S13 — **spatial households**: the G5b [`Self::frontier`] (never mutated) with
    /// the demography overlay's `spatial_households` flag flipped on. Every lineage
    /// member (founders at generation + newborns at birth) now gets a **world agent**
    /// at its exact econ id, so the reproducing population is spatial and *can* be
    /// assigned forage/gather/haul tasks — the structural unification that unblocks the
    /// scarcity arc (S14+).
    ///
    /// It is **purely structural**: there is no forage scarcity yet (the food hearth
    /// still feeds the lineages), so the motivation to forage is absent and the
    /// demography behaves exactly as `frontier`'s — the spatial world agents sit Idle.
    /// The milestone adds the *capability*, not a behavior change. Derived by flipping
    /// a single flag, so with it reverted it is byte-identical to `frontier`.
    pub fn frontier_spatial_households() -> Self {
        let mut cfg = Self::frontier();
        cfg.demography
            .as_mut()
            .expect("the frontier carries a demography overlay")
            .spatial_households = true;
        cfg
    }
    /// EXPERIMENTAL (progression probe — not a golden path): `frontier` with the
    /// whole productive bundle scaled by `scale` — the food supply (grain/WOOD
    /// node regen, cap, stock), the gathering labor force, and the chain
    /// processing throughput — under a fixed-generous demographic headroom held
    /// CONSTANT across scales (so demography is never the binding cap and is not
    /// the variable under test). It answers one question: is the colony's
    /// long-run equilibrium carrying-capacity-bound (output and sustained
    /// population rise ~linearly with the productive bundle) or pinned by a fixed
    /// cap (they saturate)? Additive and game-only; the six econ goldens and
    /// every existing scenario are untouched.
    pub fn frontier_probe(scale: u32) -> Self {
        let scale = scale.max(1);
        let mut cfg = Self::frontier();
        for node in &mut cfg.nodes {
            node.regen = node.regen.saturating_mul(scale);
            node.cap = node.cap.saturating_mul(scale);
            node.stock = node.stock.saturating_mul(scale);
        }
        let scale_u16 =
            |n: u16| -> u16 { ((n as u32).saturating_mul(scale)).min(u16::MAX as u32) as u16 };
        cfg.gatherers = scale_u16(cfg.gatherers);
        cfg.consumers = scale_u16(cfg.consumers);
        if let Some(chain) = cfg.chain.as_mut() {
            chain.throughput = chain.throughput.saturating_mul(scale);
            chain.millers = scale_u16(chain.millers);
            chain.bakers = scale_u16(chain.bakers);
            chain.latent_millers = scale_u16(chain.latent_millers);
            chain.latent_bakers = scale_u16(chain.latent_bakers);
            chain.bread_buffer = chain.bread_buffer.saturating_mul(scale);
            chain.latent_flour_seed = chain.latent_flour_seed.saturating_mul(scale);
        }
        if let Some(d) = cfg.demography.as_mut() {
            // Generous, constant headroom: demography never binds, so any change
            // in the equilibrium across scales comes from carrying capacity, not
            // from a demographic ceiling.
            d.max_household_size = 60;
        }
        cfg
    }
    /// EXPERIMENTAL (millisats / divisibility probe — not a golden path):
    /// `frontier` redenominated into a `precision`-times-finer money unit (the
    /// Lightning-millisat idea — same real economy, many more money units). It
    /// scales every money-denominated SUPPLY/WANT in the barter config (the SALT
    /// endowments and the medium want) by `precision`, leaving goods, recipes,
    /// labor, and demography identical. The point: the post-promotion savings
    /// demand is a count of single money-unit wants capped at `MAX_SAVE_UNITS`
    /// (life::scale) — a NOMINAL, unit-denominated demand. With only a few
    /// hundred money units in the base `frontier`, a handful of patient savers
    /// corner the whole supply and circulation freezes. A finer unit gives the
    /// economy enough units that the same capped nominal savings demand can no
    /// longer absorb the supply. Additive and game-only; econ goldens untouched.
    pub fn frontier_millisats(precision: u32) -> Self {
        let precision = precision.max(1);
        let mut cfg = Self::frontier();
        if let Some(b) = cfg.barter.as_mut() {
            b.consumer_medium_endowment = b.consumer_medium_endowment.saturating_mul(precision);
            b.gatherer_medium_endowment = b.gatherer_medium_endowment.saturating_mul(precision);
            b.cycle_producer_medium_endowment =
                b.cycle_producer_medium_endowment.saturating_mul(precision);
            b.medium_want_qty = b.medium_want_qty.saturating_mul(precision);
        }
        cfg
    }
    /// EXPERIMENTAL (no-pure-consumer probe — not a golden path): `frontier`
    /// with the pure-consumer class removed. The "consumers" — agents that hold
    /// the money, eat, and never produce — are folded into the gathering labor
    /// force, and the SALT money endowment is moved onto the gatherers (total
    /// supply preserved). The controlled variable vs `frontier` is ONLY who
    /// holds the money: working gatherers instead of a non-producing consumer
    /// class; the chain, food model, nodes, and demography are otherwise
    /// identical. It tests whether segregating money from production (the
    /// consumer class) is what causes the circular-flow cold-start deadlock —
    /// the producer-working-capital finding in `docs/experiment-money-
    /// circulation.md`. Additive and game-only; econ goldens untouched.
    pub fn frontier_no_consumers() -> Self {
        let mut cfg = Self::frontier();
        let ex_consumers = u32::from(cfg.consumers);
        // Fold the removed consumers into the gathering labor force so the
        // population and the number of mouths are preserved; only their role
        // (idle money-holder -> producing gatherer) changes.
        cfg.gatherers = cfg.gatherers.saturating_add(cfg.consumers);
        cfg.consumers = 0;
        if let Some(b) = cfg.barter.as_mut() {
            // Move the SALT endowment from the removed consumers onto the
            // gatherers, preserving the total supply: money is now held by
            // producers, not an idle consumer class.
            let total_salt = ex_consumers.saturating_mul(b.consumer_medium_endowment);
            let gatherers = u32::from(cfg.gatherers).max(1);
            b.gatherer_medium_endowment = total_salt / gatherers;
            b.consumer_medium_endowment = 0;
            b.cycle_producer_medium_endowment = 0;
        }
        cfg
    }
    /// EXPERIMENTAL (subsistence floor — not a golden path): `frontier` with raw
    /// grain made a directly-edible subsistence food (`subsistence_on_grain`),
    /// so the grain→flour→bread chain is **optional specialization on top of a
    /// subsistence base** rather than the sole food source. Colonists prefer
    /// bread but eat the raw grain they already over-gather to survive when the
    /// chain stalls. Tests whether a subsistence floor keeps the colony fed (no
    /// chronic-hunger collapse) over a long horizon while specialization still
    /// emerges — the synthesis of `docs/experiment-money-circulation.md`.
    /// Additive and game-only; econ goldens untouched.
    pub fn frontier_subsistence() -> Self {
        let mut cfg = Self::frontier();
        if let Some(chain) = cfg.chain.as_mut() {
            chain.subsistence_on_grain = true;
        }
        cfg
    }
    /// EXPERIMENTAL (capital-advance probe — not a golden path): `frontier` on a
    /// finer money unit (built from [`Self::frontier_millisats`] so concentration
    /// and the integer price floor are not confounds), plus the conserved
    /// capital-advance phase ([`ChainConfig::capital_advance`]). It isolates the
    /// producer-working-capital thesis: after promotion, cashless active
    /// producers are funded from the richest saver so they can buy inputs. If
    /// missing working capital is the binding cause, the chain keeps producing
    /// past the seed-exhaustion tick instead of stalling. Additive and game-only;
    /// econ goldens untouched.
    pub fn frontier_capital_advance() -> Self {
        let mut cfg = Self::frontier_millisats(1_000);
        if let Some(chain) = cfg.chain.as_mut() {
            chain.capital_advance = true;
        }
        cfg
    }
    /// EXPERIMENTAL (spoilage / inventory carrying cost — not a golden path):
    /// `frontier_capital_advance` (the revolving working-capital loan) PLUS
    /// per-tick spoilage on the perishable chain foods. Codex's primary fix for
    /// the distribution-seizure halt: with the loan supplying working capital and
    /// spoilage forcing satiated holders' bread/grain hoards back into
    /// circulation (hunger returns, raw grain must sell before it rots), the test
    /// is whether production sustains past the ~tick-300 halt without the colony
    /// bifurcating into a hoarding consumer class and a starving producer class.
    /// Additive and game-only; econ goldens untouched.
    pub fn frontier_spoilage() -> Self {
        let mut cfg = Self::frontier_capital_advance();
        if let Some(chain) = cfg.chain.as_mut() {
            chain.perishable_decay_bps = 2_000;
        }
        cfg
    }
    /// EXPERIMENTAL (in-kind subsistence advance — not a golden path): the
    /// revolving working-capital loan (`frontier_capital_advance`) PLUS an in-kind
    /// staple-food advance to hungry producers (`ChainConfig::subsistence_advance`).
    /// The live order-book trace (Experiment 9) proved a loan-funded but hungry
    /// miller posts no grain bid because its money is reserved for its own unmet
    /// bread want; feeding it in kind frees that money so it buys grain and the
    /// chain runs. The faithful fix: a saver advances both money (loan) and
    /// present goods (food) to the producer, keeping each worker's value scale
    /// intact. Additive and game-only; econ goldens untouched.
    pub fn frontier_in_kind() -> Self {
        let mut cfg = Self::frontier_capital_advance();
        if let Some(chain) = cfg.chain.as_mut() {
            chain.subsistence_advance = true;
        }
        cfg
    }
    /// EXPERIMENTAL (in-kind INPUT advance — not a golden path): the in-kind
    /// subsistence colony (`frontier_in_kind`: loan + food in kind, so the colony
    /// stays fed) PLUS the in-kind **input** advance
    /// ([`ChainConfig::input_advance`]) — a capitalist buys each producer's recipe
    /// input in kind and places it in its hands, so production runs without the
    /// producer having to out-rank its own savings to bid for inputs (the residual
    /// blocker from Experiment 10). Tests whether placing inputs makes the
    /// production chain self-sustain past the halt. Additive and game-only; econ
    /// goldens untouched.
    pub fn frontier_input_advance() -> Self {
        let mut cfg = Self::frontier_in_kind();
        if let Some(chain) = cfg.chain.as_mut() {
            chain.input_advance = true;
        }
        cfg
    }
    /// EXPERIMENTAL (the subsistence→specialization arc — not a golden path): the
    /// full in-kind capital-advance colony (`frontier_input_advance`, which
    /// advances the loan, food, and inputs in kind on a fed subsistence base) plus
    /// the recurring owner-operator motive ([`ChainConfig::recurring_motive`]). The
    /// recurring motive stops producers retiring once their savings fill (the
    /// satiation wall that collapsed Experiment 11), so — with inputs placed and
    /// the colony fed — specialization that emerges from the subsistence base can
    /// *sustain*: a self-employment economy, no firms. Tests the whole arc:
    /// subsistence, emergent money, then sustained specialized production. Additive
    /// and game-only; econ goldens untouched.
    pub fn frontier_economy() -> Self {
        let mut cfg = Self::frontier_input_advance();
        if let Some(chain) = cfg.chain.as_mut() {
            chain.recurring_motive = true;
        }
        cfg
    }
    /// EXPERIMENTAL (ablation — `economy` minus the in-kind INPUT advance): loan +
    /// food-in-kind + recurring motive, but producers must acquire inputs through
    /// the **market** (no `input_advance`). If tail production collapses relative
    /// to `frontier_economy`, the sustained chain was mostly scripted input
    /// placement, not market coordination (Codex's sharp ablation). Game-only.
    pub fn frontier_economy_no_input() -> Self {
        let mut cfg = Self::frontier_in_kind();
        if let Some(chain) = cfg.chain.as_mut() {
            chain.recurring_motive = true;
        }
        cfg
    }
    /// EXPERIMENTAL (endogenous ablation — recurring motive ALONE): the divisible-
    /// money base plus only `recurring_motive` — NO curated capital/food/input
    /// advances. The market, latent producers, and subsistence wants are as in the
    /// base colony. Tests whether specialization sustains **endogenously** (inputs
    /// acquired by market trade, not placed). The falsification of the "economy"
    /// being self-organizing: if this does not sustain, `frontier_economy` is
    /// scaffolded, not endogenous. Game-only; econ goldens untouched.
    pub fn frontier_recurring_only() -> Self {
        let mut cfg = Self::frontier_millisats(1_000);
        if let Some(chain) = cfg.chain.as_mut() {
            chain.recurring_motive = true;
        }
        cfg
    }
    /// EXPERIMENTAL (the ENDOGENOUS economy — the genuine test): divisible money,
    /// a revolving working-capital loan, the recurring owner-operator motive, and
    /// project-aware input bids — but **NO curated food or input placement**. The
    /// producer feeds and supplies itself through the market: it borrows working
    /// capital, **buys** its input at an imputed price from a willing seller
    /// (`project_input_bids`), mills/bakes, and sells the output. If the chain
    /// sustains here, specialization is **self-organizing**, not scaffolded — the
    /// falsification of the Experiment-12 "scaffolded" verdict. Game-only; econ
    /// goldens untouched.
    pub fn frontier_endogenous() -> Self {
        // The ENDOGENOUS economy (the S5 DoD): the grain→flour→bread division of
        // labor emerges atop a HOUSEHOLD/SUBSISTENCE base and sustains on REAL
        // MARKET TRADE, with NO chain-specific global placement.
        //
        // Base (local/household allocation — allowed, not scaffolding):
        // - the household demography hearth feeds the consumer lineages their staple
        //   + WOOD (`deliver_demography_provisions`), and reproduces/ages them;
        // - each chain producer feeds from its OWN local hearth
        //   (`producer_subsistence`: staple + WOOD) so its money frees ENTIRELY for
        //   recipe inputs rather than its own subsistence;
        // - raw grain is a directly-edible subsistence floor (`subsistence_on_grain`)
        //   — the roundabout bread chain is OPTIONAL specialization on top of it.
        //
        // Coordination (S1–S4, all real market trade):
        // - producers BUY their recipe inputs on the real order book at the imputed
        //   bundle reservation (`project_input_bids`, S1/S2), restocking only as they
        //   clear their output (working-capital discipline, S3);
        // - `recurring_motive` keeps an owner-operator producing while profitable;
        // - the cold-start buffers (`latent_flour_seed`, `bread_buffer`) seed the
        //   first realized flour/bread prices so latent millers→bakers adopt in
        //   pipeline order (S4).
        //
        // NO curated scaffolds: NO per-tick planner loan (`capital_advance` off —
        // working capital is real retained earnings), NO global food redistribution
        // (`subsistence_advance` off), NO global input placement (`input_advance`
        // off). A designated-GOLD market (`barter = None`) so the study is the chain,
        // not money emergence (that is G5a/G5b); the money supply is the colonists'
        // starting gold, which circulates rather than pooling.
        let mut cfg = Self::frontier();
        cfg.barter = None;
        cfg.starting_gold_gatherer = 60;
        cfg.starting_gold_consumer = 60;
        if let Some(chain) = cfg.chain.as_mut() {
            chain.recurring_motive = true;
            chain.project_input_bids = true;
            chain.subsistence_on_grain = true;
            chain.producer_subsistence = 4;
            chain.producer_gold = 16;
            // Smaller bread/flour bootstrap than the barter frontier (no barter
            // window to bridge): enough to seed the first prices, not so much that
            // reshuffling the buffer drowns out new production.
            chain.bread_buffer = 8;
            chain.consumer_staple_buffer = 2;
            chain.latent_flour_seed = 12;
            // Threshold carrying cost on the staple + raw-grain HOARDS (working
            // stock under the free-storage floor is exempt): a satiated holder's
            // bread/grain pile decays, so hunger recurs and the holder re-enters
            // the market — keeping demand (and the chain) running rather than
            // letting bread/grain accumulate unbounded. Bounds every stock, so the
            // colony is genuinely stationary, not slowly hoarding.
            chain.perishable_decay_bps = 1_500;
        }
        if let Some(demo) = cfg.demography.as_mut() {
            demo.child_gold_endowment = 16;
            for household in &mut demo.households {
                household.starting_gold = 60;
            }
        }
        cfg
    }
    /// THE SCALING ECONOMY (the S6 DoD): the endogenous economy
    /// ([`Self::frontier_endogenous`]) plus **productive re-entry** turned ON, so no
    /// hungry, unprovisioned colonist is left permanently stranded. A hungry spatial
    /// non-lineage colonist — an idle consumer or a WOOD-mis-allocated gatherer —
    /// adopts edible-grain gathering on its own value scale and feeds itself, and a
    /// fed re-entrant resumes WOOD gathering (the S6.2 hysteresis keeps WOOD alive).
    /// To exercise *growth* (not just the fixed stranded set), it also seeds a
    /// **larger colony** (more consumers + gatherers) and raises the household-size
    /// cap so population climbs further while provisioning keeps pace. Everything else
    /// is the endogenous economy: the grain→flour→bread chain still self-organizes and
    /// sustains on real market trade, with NO chain-specific global placement.
    pub fn frontier_endogenous_scaling() -> Self {
        let mut cfg = Self::frontier_endogenous();
        if let Some(chain) = cfg.chain.as_mut() {
            // The gated phase, ON. Re-enter at chronic hunger (the stranded tail sits
            // at the need ceiling) and revert once comfortably fed — a wide band so a
            // re-entrant holds its node for many ticks rather than thrashing.
            chain.productive_reentry = true;
            chain.reentry_hunger_in = 8;
            chain.reentry_hunger_out = 4;
        }
        // A larger non-lineage base — the stranded set re-entry must provision — so
        // the metric is exercised at scale, not only on the fixed 4 consumers + 4 WOOD
        // gatherers of the endogenous roster.
        cfg.consumers = 8;
        cfg.gatherers = 12;
        // Let the lineages grow further so total population climbs above the
        // endogenous plateau (the "tracks a growing population" half of the DoD).
        if let Some(demo) = cfg.demography.as_mut() {
            demo.max_household_size = 8;
        }
        cfg
    }
    /// THE CAPITAL ECONOMY (the S7 DoD): the scaling economy
    /// ([`Self::frontier_endogenous_scaling`]) plus **producible capital goods** — the
    /// tooled grain→flour→bread chain can now GROW, not just the untooled gathering
    /// base. Under the larger colony's sustained unmet bread demand a fed, non-latent
    /// colonist appraises that building a mill/oven will pay, invests its own saved
    /// WOOD + labor in a conserved build (S7.2), then — holding the new tool — is
    /// admitted to the adoption appraisal (S7.1), adopts, buys its input on the real
    /// market, and produces. So bread output tracks demand rather than flat-lining at
    /// the seeded tool count, with NO planner placement of tools and NO over-building
    /// (capital formation stops when demand is met). Everything else is the scaling
    /// economy: re-entry still provisions the untooled tail, and the chain still
    /// self-organizes on real market trade.
    pub fn frontier_capital() -> Self {
        let mut cfg = Self::frontier_endogenous_scaling();
        if let Some(chain) = cfg.chain.as_mut() {
            // S7.1: a colonist that holds a mill/oven is admitted to the adoption
            // appraisal (and anchors the tool so it is never sold before it adopts).
            chain.tool_acquisition_eligibility = true;
            // S7.2: the per-builder BuildMill/BuildOven phase, on. A modest WOOD/labor
            // cost (a WOOD-gatherer or a hearth-provisioned lineage member can save it)
            // amortized over a generous payback window, so building pays under genuine
            // unmet bread demand yet stops once the spread thins (the overinvestment
            // guard). A fed colonist (hunger at/below the comfortable revert level)
            // invests; a hungry one feeds first.
            chain.producible_capital = true;
            chain.capital_payback_cycles = 16;
            chain.tool_build_wood = 6;
            chain.tool_build_labor = 4;
            chain.capital_build_hunger_max = 4;
        }
        // A larger consumer base than `scaling` — more mouths than the seeded
        // grain→flour→bread chain (3 latent millers + 3 latent bakers) can feed — so
        // bread demand genuinely OUTRUNS the seeded tool count and there is real room
        // for built capital to raise output. Without producible capital this same colony
        // (the test control) leaves bread demand unmet; with it, builders add the
        // bottleneck producers until demand is met. The extra WOOD gatherers keep the
        // builders' WOOD (and the warmth battery) supplied as capital is committed.
        cfg.consumers = 44;
        cfg.gatherers = 24;
        cfg
    }
    /// C3R.a — the capital frontier with mortal seeded chain producers and no
    /// succession. This is exactly [`Self::frontier_capital`] plus the mortality gate:
    /// seeded latent mill/bake producers get lifespan-only demography state, and only
    /// mortal agents may form producer roles or build fresh chain capital.
    pub fn frontier_mortal_producers() -> Self {
        let mut cfg = Self::frontier_capital();
        if let Some(chain) = cfg.chain.as_mut() {
            chain.mortal_chain_producers = true;
        }
        debug_assert!(config_mortal_chain_producers_active(&cfg));
        cfg
    }
    /// C3R.b — C3R.a's mortal producer chain plus bounded reproducing producer
    /// households. The six latent producer subjects are assigned one-per-house at
    /// generation, so completed mill/oven tools can pass through the existing estate
    /// heir route and the heir can re-adopt through the existing S7 tool-holder path.
    pub fn frontier_mortal_producers_heritable() -> Self {
        let mut cfg = Self::frontier_capital();
        if let Some(chain) = cfg.chain.as_mut() {
            chain.mortal_chain_producers = true;
            chain.mortal_producer_inheritance = true;
            chain.mortal_producer_tool_inheritance = true;
            chain.producer_house_cap = MORTAL_PRODUCER_HOUSE_CAP_DEFAULT;
        }
        let tp_base = cfg.consumer_time_preference_base_bps;
        if let Some(demo) = cfg.demography.as_mut() {
            for _ in 0..MORTAL_PRODUCER_HOUSEHOLDS {
                demo.households.push(HouseholdSpec {
                    founders: 0,
                    time_preference_base_bps: tp_base,
                    food_provision: 3,
                    wood_provision: 3,
                    starting_gold: 0,
                    starting_food: 0,
                    starting_wood: 0,
                });
            }
        }
        debug_assert!(config_mortal_producer_inheritance_active(&cfg));
        cfg
    }
    /// C3R.c — earned provisioning headline. Derives from C3R.b's heritable
    /// mortal producer base, retires the two producer-side FOOD mints, and turns
    /// on conserved GOLD transfers from active producers to hungry same-household
    /// members. The lineage surround remains unchanged.
    pub fn frontier_mortal_producers_earned() -> Self {
        let mut cfg = Self::frontier_mortal_producers_heritable();
        if let Some(chain) = cfg.chain.as_mut() {
            chain.earned_provisioning = true;
            chain.producer_subsistence = 0;
        }
        if let Some(demo) = cfg.demography.as_mut() {
            let start = demo
                .households
                .len()
                .checked_sub(MORTAL_PRODUCER_HOUSEHOLDS)
                .expect("heritable base appends producer households");
            for household in &mut demo.households[start..] {
                household.food_provision = 0;
            }
        }
        debug_assert!(cfg
            .chain
            .as_ref()
            .is_some_and(chain_config_earned_provisioning_active));
        cfg
    }
    /// DH.a (impl-68) — THE CLOSED CIRCULATION: the durable stack MINUS the endowed non-producing
    /// surround. Built subtractively from [`Self::frontier_mortal_producers_earned`] exactly as the
    /// `ignition_withdrawal` oracle builds its `{durable}` regime (producer `wood_provision = 0` +
    /// `gatherers = 48`), then the §3.1 edit list and NOTHING else:
    ///
    /// - `consumers = 0`; `starting_gold_consumer = 0`; `consumer_wood_endowment = 0` (the last two
    ///   are already 0 on this path — zeroed for explicitness, all three in the §3.7 identity test).
    /// - the 2 legacy lineage households are removed: the demography household list is ONLY the 6
    ///   mortal producer households (their per-tick hearth mints go with them).
    /// - `closed_circulation = true` (the ON-only marker, digest tag 34).
    ///
    /// A household that produces nothing and earns nothing cannot exist in a closed economy; the
    /// lineage surround (like the consumer cohort) is endowment scaffolding from earlier arcs. DH.a
    /// removes scaffolding rather than converting it. No parameter tuning, no new mechanism: every
    /// market the regime needs already exists. See `docs/impl-closed-circulation.md`.
    pub fn frontier_closed_circulation() -> Self {
        // The {durable} regime exactly as the oracle builds it: the earned trap base + the retired
        // producer WOOD mint + gatherers pinned at 48 (double the base 24).
        let mut cfg = Self::frontier_mortal_producers_earned();
        cfg.gatherers = 48;
        if let Some(demo) = cfg.demography.as_mut() {
            let start = demo
                .households
                .len()
                .checked_sub(MORTAL_PRODUCER_HOUSEHOLDS)
                .expect("earned base appends producer households");
            for household in &mut demo.households[start..] {
                household.wood_provision = 0;
            }
        }
        // The DH.a subtractive edits (§3.1): remove the endowed non-producing surround.
        cfg.consumers = 0;
        cfg.starting_gold_consumer = 0;
        cfg.consumer_wood_endowment = 0;
        if let Some(demo) = cfg.demography.as_mut() {
            // Keep ONLY the 6 mortal producer households (drop the 2 legacy lineage households at
            // the head of the list). Their hearth mints leave with them.
            let start = demo
                .households
                .len()
                .checked_sub(MORTAL_PRODUCER_HOUSEHOLDS)
                .expect("earned base appends producer households");
            demo.households.drain(..start);
        }
        cfg.closed_circulation = true;
        cfg
    }
    /// C3R.d — C3R.c plus a producer-household motive to save the existing
    /// four-loaf child endowment through ordinary `Next`-horizon market wants.
    pub fn frontier_mortal_producers_saving() -> Self {
        let mut cfg = Self::frontier_mortal_producers_earned();
        if let Some(chain) = cfg.chain.as_mut() {
            chain.birth_stock_saving = true;
            chain.birth_stock_saving_mode = BirthStockSavingMode::Motive;
        }
        debug_assert!(cfg
            .chain
            .as_ref()
            .is_some_and(chain_config_birth_stock_saving_active));
        cfg
    }
    /// THE CO-EMERGENT ECONOMY (the S8 DoD): money, the grain→flour→bread division
    /// of labor, and capital all CO-EMERGE in one run — with NO designated money and
    /// NO curated placement. Unlike [`Self::frontier_endogenous`] (which is HANDED
    /// designated GOLD and only then calculates, bids, and builds), this starts from
    /// the barter-start emergent base [`Self::frontier`] — `barter = Some(..)`, the
    /// SALT medium, **every gold endowment zero** — and lets SALT promote by
    /// saleability from real indirect exchange. After promotion the (money-good-
    /// agnostic) S5 sustain stack and the S7 capital phase run on the EMERGED unit.
    ///
    /// What it adds to [`Self::frontier`] (all of which thread `current_money_good`,
    /// never hard-coded GOLD — Base Fact 1):
    /// - the S5 sustain stack: `recurring_motive` (an owner-operator keeps producing
    ///   while profitable), `project_input_bids` (producers BUY recipe inputs at the
    ///   imputed reservation, S1/S2), and threshold spoilage (`perishable_decay_bps`)
    ///   so a satiated holder's bread/grain pile decays and demand recurs;
    /// - the local `producer_subsistence` hearth set to a **partial** floor (2, not
    ///   the endogenous economy's 4): enough to free a producer's emerged money for
    ///   inputs across the cutover, but NOT so much that a fully-fed producer hoards
    ///   its whole margin and drains the (scarce) emerged money out of circulation —
    ///   the balance that lets the chain *sustain* on emerged money rather than freeze
    ///   once the post-promotion money pulse is absorbed (see the S8 finding below);
    /// - a **lean demographic hearth** (`food_provision`/`wood_provision` = 1, not 3):
    ///   a hearth-fed lineage that mints a large staple/WOOD *surplus* sells it for the
    ///   emerged money and — being fed — hoards the cash, a money sink that scales with
    ///   the supply and starves the productive loop. Trimming the surplus keeps the
    ///   scarce emerged money circulating (the colony is still hearth-fed and still
    ///   reproduces; it just no longer pumps money into idle savings).
    ///
    /// What it deliberately does NOT do (Base Fact 6 / the two tensions):
    /// - `subsistence_on_grain` stays OFF — a raw-grain floor would thin the
    ///   bread-for-SALT trade that monetizes SALT (Tension A), so it would starve
    ///   promotion; S6 productive re-entry stays OFF too (it is inert without the
    ///   grain floor and would re-enable the crowd-out with it);
    /// - it seeds NO money: `producer_gold` stays 0 and every gold endowment is zero
    ///   (the barter overlay asserts it at generation). A producer's working capital
    ///   across the cutover is EARNED — it sells its seeded cold-start output into the
    ///   real money market post-promotion, no curated advance (the S8.2 finding: the
    ///   `frontier` saleability hub concentrates SALT in consumers who barely spend it
    ///   before the fast promotion, so producers earn ~no *barter* SALT — Base Fact 5 —
    ///   yet the chain survives the cutover on these post-promotion earnings anyway).
    /// - NO curated placement (`subsistence_advance`/`input_advance`/`capital_advance`
    ///   all off). A modest colony (≈ the endogenous size, not the S6 scaling colony):
    ///   provisioning-at-scale under emergence is deferred (S9).
    pub fn frontier_coemergent() -> Self {
        let mut cfg = Self::frontier();
        if let Some(chain) = cfg.chain.as_mut() {
            // The S5 sustain stack on emerged money (the only GOLD-hardcoded path,
            // the unused `recipe_adoption_pays` wrapper, is never called here).
            chain.recurring_motive = true;
            chain.project_input_bids = true;
            // Threshold carrying cost on the staple + raw-grain HOARDS (working stock
            // under the free-storage floor is exempt): satiated piles decay so hunger
            // recurs, demand stays alive, and stocks stay bounded — the same lever the
            // endogenous economy uses, here keeping the post-promotion chain churning.
            chain.perishable_decay_bps = 1_500;
            // A PARTIAL local producer hearth (staple + WOOD): each active producer
            // feeds mostly from its own hearth so its emerged money frees for recipe
            // inputs — but not fully (2, vs the endogenous 4), so it still buys some
            // food and recirculates its margin rather than hoarding it. And (unlike
            // `subsistence_on_grain`) it adds no raw-grain consumer floor that would
            // crowd out the bread-for-SALT trade (Base Fact 6).
            chain.producer_subsistence = 2;
            // S8.3 — the S7 producible-capital phase, composed onto the EMERGED money:
            // a colonist holding a mill/oven is admitted to adoption (S7.1) and a fed
            // colonist can BUILD a mill/oven from its own WOOD + labor (S7.2), all
            // priced in the emerged unit. The build cost is DEARER than the S7 scaling
            // colony's (12 WOOD, not 6) over a longer payback window (32, not 16): the
            // small co-emergent colony's high producer margins would otherwise justify
            // runaway building, so a higher real-resource bar keeps capital formation
            // modest and demand-anchored (a few tools, then it stops) rather than an
            // over-build that drains WOOD and destabilizes the chain.
            chain.tool_acquisition_eligibility = true;
            chain.producible_capital = true;
            chain.capital_payback_cycles = 32;
            chain.tool_build_wood = 12;
            chain.tool_build_labor = 4;
            chain.capital_build_hunger_max = 4;
            // `subsistence_on_grain`, `productive_reentry`, `producer_gold = 0`, and
            // every gold endowment stay at the `frontier` (barter-start) defaults.
        }
        // Trim the demographic hearth to a lean floor (1, not 3): the hearth still
        // feeds and reproduces the lineage, but no longer mints a large surplus the
        // fed lineage sells and hoards — the money sink that otherwise drains the
        // scarce emerged supply out of the productive loop.
        if let Some(demo) = cfg.demography.as_mut() {
            for household in &mut demo.households {
                household.food_provision = 1;
                household.wood_provision = 1;
            }
        }
        cfg
    }
    /// S9 — the STRONG-BAR emergence experiment, derived from
    /// [`Self::frontier_coemergent`] (never mutating it). It removes the remaining
    /// circularity Codex flagged in S8: SALT promoted there only because every
    /// colonist was configured to want SALT *as a medium* (`medium_want_qty`), i.e.
    /// to desire it as money before it was money. Here that pre-monetary medium want
    /// is **off** (`medium_want_qty = 0`); instead SALT is given a modest,
    /// **heterogeneous real direct use** (a `Good(SALT)/Now` consumption want on a
    /// subset of colonists, `salt_direct_use_period = 8` → ~1-in-8), and promotion is
    /// gated on genuine **indirect-exchange breadth** — a good monetizes only after
    /// enough indirect acceptances, by enough distinct indirect acceptors, for at
    /// least one end other than the good's own use. The Mengerian chain runs forward:
    /// heterogeneous direct use → saleability → provisional leader → indirect
    /// acceptance by the OTHERS → breadth gate → promotion. No designated money, no
    /// seeded gold, no re-added medium want.
    ///
    /// Observed result (`docs/impl-strong-bar-emergence.md`): money EMERGES — SALT
    /// promotes from real saleability across seeds, then the S8 chain + capital
    /// sustain on the emerged unit. The indirect demand concentrates on the staple
    /// (bread) — the one near-universal unmet want the colony re-trades SALT to reach
    /// — so the realized indirect-target breadth is one dominant end, which the gate
    /// requires (`min_indirect_target_goods = 1`) while the distinct-acceptor floor
    /// (`6`) rules out a few-agent churn.
    pub fn frontier_coemergent_strong() -> Self {
        let mut cfg = Self::frontier_coemergent();
        if let Some(barter) = cfg.barter.as_mut() {
            // Remove the circular pre-monetary medium want — SALT is no longer wanted
            // AS money before it is money. (Its physical endowment stays, so SALT is
            // still present to circulate and to convert 1:1 at promotion.)
            barter.medium_want_qty = 0;
            // The real, heterogeneous direct use that replaces it: one fixed
            // `Good(SALT)/Now` consumption want on ~1-in-8 colonists (the band that
            // both seeds saleability and leaves enough non-wanters to accept SALT
            // indirectly — Base Fact 6; a denser want would suppress indirect offers).
            barter.salt_direct_use_qty = 1;
            barter.salt_direct_use_period = 8;
            // The strong-bar promotion gate: real indirect-exchange breadth.
            // Withholds promotion (which the weak S8 bar fires by ~tick 19 on direct
            // churn alone) until SALT has accrued sustained indirect volume, spread
            // across distinct acceptors, for an end other than its own use.
            barter.menger.min_indirect_acceptances = 12;
            barter.menger.min_indirect_acceptor_agents = 6;
            barter.menger.min_indirect_target_goods = 1;
            // Indirect acceptance stays ON (the headline path); the
            // `allow_indirect_acceptance = false` control derives from here.
        }
        cfg
    }
    /// S10 — THE ORIGINARY-INTEREST ECONOMY (the flagship): the strong-bar co-emergent
    /// colony ([`Self::frontier_coemergent_strong`], never mutated) with **per-agent
    /// intertemporal capital choice** on. Money still EMERGES (SALT promotes from real
    /// indirect-exchange breadth), then the chain + capital sustain on the emerged unit —
    /// but capital now forms through a **per-colonist ORDINAL** decision instead of S7's
    /// settlement-level planner: each eligible colonist appraises, on its OWN value scale,
    /// whether committing present WOOD + forgone leisure to build a durable mill/oven whose
    /// recipe-margin receipt stream provisions one of its OWN future-money savings wants is
    /// worth it ([`appraise_capital_tool_bundle_for_money`]). Capital formation then tracks
    /// each colonist's `time_preference_bps` — patient colonists invest in the roundabout
    /// tooled chain, present-biased ones do not — with NO cardinal discount (originary
    /// interest expressed ordinally via the multi-horizon savings ladder), no global stage
    /// choice, and no first-eligible-builder assignment.
    ///
    /// Derived from the strong-bar base (only the build seam changes): `per_agent_capital`
    /// is switched on (which leaves `capital_payback_cycles` inert), the build's WOOD cost
    /// is trimmed from the co-emergent 12 to 6 so a colonist that has saved a modest WOOD
    /// surplus can fund it from its OWN endowment (the per-agent decision is the brake now,
    /// not a dear real-resource bar), and gatherers carry a slightly larger WOOD battery so
    /// a fed, rested saver actually accumulates the build's WOOD. Everything that makes SALT
    /// emerge (the heterogeneous direct use, the breadth gate, the WOOD-poor consumer hub)
    /// is untouched, so money still emerges from real saleability.
    pub fn frontier_coemergent_strong_originary() -> Self {
        let mut cfg = Self::frontier_coemergent_strong();
        if let Some(chain) = cfg.chain.as_mut() {
            // The per-agent ordinal decision replaces S7's build planner. The S7 gates
            // (tool-acquisition eligibility + producible capital) stay on — per-agent mode
            // steers the SAME per-builder substrate.
            chain.per_agent_capital = true;
            // Trim the build's WOOD cost (12 → 6): the per-agent appraisal is the brake
            // now (a colonist builds only when the tool provisions its own deep savings
            // want without breaking a higher one), so the build need not be made dear to
            // hold capital formation in check — it just has to be fundable from a saver's
            // own WOOD surplus.
            chain.tool_build_wood = 6;
            // A larger WOOD battery so a fed builder accumulates the build's WOOD without
            // running its own warmth short (removing the committed WOOD must not break a
            // higher-ranked warmth want, or the appraisal declines — the WOOD's present
            // use is one of the costs the future gain must outrank).
            chain.wood_buffer = 64;
        }
        cfg
    }
    /// S11 — THE ENTREPRENEURIAL-UNCERTAINTY ECONOMY (the flagship): the S10 originary
    /// base ([`Self::frontier_coemergent_strong_originary`], never mutated) with
    /// **per-agent fallible forecasts** on. Every entrepreneurial appraisal — the
    /// role-choice adopt, the per-agent capital build, the project input-bid — now weighs
    /// its OUTPUT-revenue estimate against the colonist's OWN grounded forecast (its
    /// adaptive [`PriceBelief`] tilted by the heritable
    /// [`CultureParams::forecast_bias_bps`]) instead of the shared last realized price.
    /// The market still clears at the REAL price, so an over-optimist that adopts/builds
    /// on an inflated forecast earns the real (lower) revenue: its committed WOOD/inputs
    /// are sunk and it ends with LESS capital to invest, while an accurate/conservative
    /// forecaster accumulates and expands — **profit/loss selection through capital, not
    /// mortality** (`hunger_critical` stays disabled). Money still EMERGES and the S10
    /// multi-horizon ladder + per-agent capital choice are intact (the originary base is
    /// untouched); only the appraisal's price expectation becomes individual and fallible.
    ///
    /// Derived from the originary base by flipping a single flag
    /// ([`ChainConfig::entrepreneurial_forecasts`]) — so with that flag reverted it is
    /// byte-identical to `frontier_coemergent_strong_originary`. The per-colonist forecast
    /// biases come from the heritable jitter around the neutral base
    /// ([`SettlementConfig::forecast_bias_base_bps`], left neutral here).
    pub fn frontier_coemergent_strong_entrepreneurial() -> Self {
        let mut cfg = Self::frontier_coemergent_strong_originary();
        if let Some(chain) = cfg.chain.as_mut() {
            chain.entrepreneurial_forecasts = true;
        }
        cfg
    }
    /// S12 — THE PROVISIONED ECONOMY (the flagship): the S11 entrepreneurial co-emergent
    /// colony ([`Self::frontier_coemergent_strong_entrepreneurial`], never mutated) with
    /// **own-labor subsistence** on and the food mints retired. The exogenous food
    /// hearths that minted bread/staple with no labor (the producer-subsistence staple
    /// floor and the demographic `food_provision`) are gone; instead a hungry, eligible,
    /// unprovisioned colonist with spare labor **forages** a low-grade survival floor
    /// (the FORAGE good) from its OWN labor — booked `produced`, eaten at home, ranked
    /// BELOW bread. Everything that makes SALT emerge and the chain + capital + forecasts
    /// sustain is untouched, so the test is whether the colony can be both
    /// **bounded-hunger** (the forage floor feeds the surviving spatial tail) AND keep
    /// money emerging (bread stays the superior good that monetizes SALT). Derived by flipping the
    /// own-labor flags (and interning the FORAGE good into the content), so with them
    /// reverted it is byte-identical to the entrepreneurial base.
    ///
    /// The `forage_yield` is the default diagnostic yield the S12 sweep probes: enough
    /// to pull sustained spatial-tail hunger below the semi-hungry S9/S11 baseline, but
    /// not enough to rescue money emergence under the one-scalar food model.
    pub fn frontier_coemergent_strong_provisioned() -> Self {
        let mut cfg = Self::frontier_coemergent_strong_entrepreneurial();
        if let Some(chain) = cfg.chain.as_mut() {
            // Intern the FORAGE subsistence good (no recipe — produced from labor) and
            // turn on the own-labor path: the food mints retire and a hungry forager is
            // credited `forage_yield` FORAGE after completing a forage task.
            chain.content = chain.content.clone().with_forage();
            chain.own_labor_subsistence = true;
            // The survival-floor knob used by the S12 no-middle-band diagnostic.
            chain.forage_yield = 3;
            // Forage when hunger reaches the band's top, stop once comfortably fed —
            // wide enough that a gatherer does not thrash between foraging and WOOD.
            chain.forage_hunger_in = 6;
            chain.forage_hunger_out = 2;
        }
        cfg
    }
    /// S14 — **forage carrying capacity** (the endogenous population plateau). Composes
    /// the S12 own-labor path (hearth food MINT OFF — forage IS the food) + S13 spatial
    /// households (lineages forage) + the S14.1 capped FORAGE commons + a S14.2
    /// growth-capable demography that endows children from FORAGE. The colony's
    /// population GROWS while the commons can feed it and PLATEAUS when it cannot — the
    /// plateau set by the forage flow (regen/cap), bounded by the birth-hunger
    /// **preventive check** (births stall when a member's hunger exceeds the ceiling).
    /// Deaths are **old-age only** (no mortality; `hunger_critical` stays disabled).
    ///
    /// Derived from [`Self::frontier_coemergent_strong_provisioned`] (never mutated): the
    /// non-lineage chain colonists and the bread-chain bootstrap buffers are stripped so
    /// the only population is the spatial **lineages** foraging the commons, and the
    /// demography is retuned for growth — `max_household_size` raised to 24 (so the
    /// artificial knob does not bind; the forage flow does), a long lifespan (so the
    /// demographic ceiling sits far above the forage-bound plateau), and the
    /// birth-hunger ceiling set to 8 (below `need_max` 12, so forage scarcity can push a
    /// member over it and stall births — the preventive check). The hearth food mint is
    /// off (own-labor), so `endowment[staple] == 0` and the plateau is forage-determined.
    /// SALT does not promote and the chain does not run — S14 is purely demographic /
    /// ecological. Gated: with the S14 flags off it reduces to the unchanged S5–S13
    /// stream, so every existing scenario/golden is byte-identical.
    pub fn frontier_forage_capacity() -> Self {
        let mut cfg = Self::frontier_coemergent_strong_provisioned();
        // Lineage-only: strip the non-lineage chain colonists and the bread-chain
        // bootstrap buffers so the reproducing spatial lineages are the only foragers
        // pressing on the commons (a clean carrying-capacity signal). The chain never
        // runs and SALT never promotes here — S14 is demographic/ecological only.
        cfg.gatherers = 0;
        cfg.consumers = 0;
        if let Some(chain) = cfg.chain.as_mut() {
            chain.millers = 0;
            chain.bakers = 0;
            chain.latent_millers = 0;
            chain.latent_bakers = 0;
            chain.bread_buffer = 0;
            chain.consumer_staple_buffer = 0;
            chain.latent_flour_seed = 0;
            // Isolate S14 from the inherited inventory-carrying-cost experiment:
            // the carrying-capacity signal must come from forage flow, not spoilage.
            chain.perishable_decay_bps = 0;
            // Forage aggressively (start at hunger 4, hold until comfortably fed) so a
            // fed lineage stays below the birth ceiling and a scarce commons is what
            // pushes it over — the preventive check, not a foraging-threshold artifact.
            chain.forage_hunger_in = 4;
            chain.forage_hunger_out = 1;
            // The capped commons: regen 2/fast-tick is the carrying capacity the
            // population presses on. Tuned so the forage-bound plateau (low 50s) sits
            // below the size cap (3 × 24 = 72) and the demographic ceiling (~100), so
            // the plateau is forage-determined (the controls bracket it: uncap the
            // forage → it rises to the size cap; drop `max_household_size` → the knob
            // binds instead).
            chain.forage_commons = Some(ForageCommons {
                stock: 90,
                regen: 2,
                cap: 300,
            });
        }
        if let Some(demo) = cfg.demography.as_mut() {
            demo.spatial_households = true;
            // Three lineages, founders 2 each (start pop 6), food mint off (food is
            // foraged); WOOD still hearth-provisioned for non-lethal warmth, which does
            // not gate births). Children are endowed from FORAGE (the birth-food
            // selector), so births stall on FORAGE scarcity, not a bread shortage.
            demo.households.clear();
            for k in 0..3u16 {
                demo.households.push(HouseholdSpec {
                    founders: 2,
                    // A little spread in time preference so the lineages are not clones.
                    time_preference_base_bps: 400 + k * 300,
                    food_provision: 0,
                    wood_provision: 3,
                    starting_gold: 0,
                    starting_food: 6,
                    starting_wood: 6,
                });
            }
            demo.birth_interval = 4;
            // The preventive check: a member over hunger 8 (< need_max 12) stalls the
            // household's birth, so forage scarcity is what bounds the population.
            demo.birth_hunger_ceiling = 8;
            // Long lifespan: lifespan ∈ {180, 186, 192, 198} ticks, so old-age deaths are
            // slow and the demographic ceiling (~100) sits far above the forage plateau —
            // forage scarcity binds first. Deaths are old-age only (no starvation).
            demo.old_age_onset_years = 30;
            demo.lifespan_span_years = 3;
            demo.ticks_per_year = 6;
            // The artificial knob, raised high so it does NOT bind on the main path (the
            // forage flow does). The controls vary this to bracket "endogenous vs knob".
            demo.max_household_size = 24;
            demo.child_food_endowment = 4;
            demo.child_gold_endowment = 0;
        }
        cfg
    }
    /// S15 — the **own-use cultivation** scenario: the S14 forage-capacity colony with
    /// the escape valve enabled. Composed from [`Self::frontier_forage_capacity`] (never
    /// mutated structurally) plus: the no-tool `Cultivate` recipe on the content set, the
    /// `own_use_cultivation` gate + its hysteresis tier, a binding **grain node** the
    /// cultivators GoHarvest (a real depleting resource, so the intensified plateau
    /// tracks the grain FLOW), and headroom on the size cap / lifespan so the new plateau
    /// is grain-flow-bound, not knob- or demographic-ceiling-bound.
    ///
    /// Under forage scarcity a *still-hungry* forager escalates to cultivation — hauling
    /// grain and making bread by its own labor, eaten at home — so the colony
    /// **intensifies** and its carrying capacity rises above the forage-only (S14)
    /// plateau (Boserup). Under abundant forage nobody is still hungry, so nobody pays
    /// the cultivation labor cost (the escape valve fires only under pressure). NO money
    /// (the bread is own-use, never traded) and NO mortality (deaths stay old-age only).
    /// Gated: with `own_use_cultivation` off it reduces to the S14 stream, so every
    /// existing scenario/golden is byte-identical.
    pub fn frontier_cultivation() -> Self {
        let mut cfg = Self::frontier_forage_capacity();
        if let Some(chain) = cfg.chain.as_mut() {
            // The no-tool grain → bread recipe the own-use cultivation phase applies.
            chain.content = chain.content.clone().with_cultivate();
            chain.own_use_cultivation = true;
            // The second hysteresis tier: escalate above the forage band (`forage_hunger_in`
            // 4) and below the birth-hunger ceiling (8), so a still-hungry forager
            // cultivates and pulls its hunger back under the preventive check (lifting the
            // plateau) rather than stalling births.
            chain.cultivate_hunger_in = 6;
            chain.cultivate_hunger_out = 3;
            // The per-tick own-use bread draw (eaten through the readback); the rest of a
            // tick's cultivated bread stays in stock to endow children (broadened rule).
            chain.cultivate_consume = 4;
            // Sustained-hunger gate: a colonist must be hungry (>= cult_in) for this many
            // CONSECUTIVE ticks before cultivating, so a transient forage-haul spike never
            // triggers it (cultivation fires only under real, persistent scarcity).
            chain.cultivate_patience = 2;
        }
        // Turn the inherited grain node into a binding "commons" the cultivation taps: a
        // real depleting resource whose REGEN sets the cultivated-grain flow (so the
        // intensified plateau tracks it — the sweep). Lower than the frontier's 8000/64
        // so it binds at a population the colony actually reaches.
        let grain = cfg.chain.as_ref().expect("chain").content.grain();
        for node in cfg.nodes.iter_mut() {
            if node.good == grain {
                node.stock = 120;
                node.regen = 4;
                node.cap = 300;
            }
        }
        if let Some(demo) = cfg.demography.as_mut() {
            // Raise the size cap and the demographic ceiling (longer lifespan) well above
            // the forage-only plateau so the INTENSIFIED plateau is grain-flow-bound, not
            // capped by the artificial knob or by old-age turnover.
            demo.max_household_size = 100;
            demo.old_age_onset_years = 60;
        }
        cfg
    }
    /// S16 — **money from PRODUCED bread** (the keystone): the S15 cultivation colony with
    /// a SALT-holding consumer BUY side restored and the `cultivation_sells_surplus` path
    /// on, so the cultivators' **surplus produced bread** is traded for SALT and the test
    /// is whether **money emerges against produced (not minted) bread** — closing the S12
    /// finding. Composes the strong-bar SALT machinery (the medium endowment + the
    /// heterogeneous direct use + the indirect-breadth gate, all inherited unchanged) +
    /// S13 spatial lineages + S14 forage commons + S15 own-use cultivation + the S16 flag.
    ///
    /// Derived from [`Self::frontier_cultivation`] (never mutated): the **minted bread is
    /// OFF** (the own-labor path retires the hearth food mints, so the only bread is the
    /// lineages' cultivated bread) and the bread bootstrap buffers are absent (inherited 0
    /// from `frontier_forage_capacity`). The single composed change is the
    /// `cultivation_sells_surplus` flag (which turns on the buy/sell split + the
    /// provenance ledger) plus restoring the consumers `frontier_forage_capacity` stripped
    /// — the goods-poor, SALT-rich (`consumer_medium_endowment 80`) buyers that, under the
    /// buy/sell split, do NOT self-cultivate, so a produced-bread market must form. With
    /// `cultivation_sells_surplus` reverted it is byte-identical to `frontier_cultivation`.
    pub fn frontier_money_from_cultivation() -> Self {
        let mut cfg = Self::frontier_cultivation();
        if let Some(chain) = cfg.chain.as_mut() {
            // The S16 gate: the buy/sell split (lineage-only cultivation, so the consumers
            // below stay the buy side) + the produced-bread provenance ledger.
            chain.cultivation_sells_surplus = true;
        }
        // Restore the SALT-holding consumer BUY side that S14 (`frontier_forage_capacity`)
        // stripped to isolate the carrying-capacity signal. These non-lineage consumers
        // hold the inherited `consumer_medium_endowment` (SALT) and — under the buy/sell
        // split — never forage or cultivate, so their only food path is BUYING the
        // lineages' surplus produced bread with SALT. The inherited bread/consumer-staple
        // buffers are 0, so they are goods-poor and buy from the first ticks. No mortality
        // (`hunger_critical` disabled upstream), so a consumer that cannot buy yet survives
        // the bootstrap. Gatherers stay 0 — the spatial lineages do all the foraging and
        // cultivating; the consumers are the pure demand side.
        cfg.consumers = 6;
        // Let the cultivators' PRODUCED surplus actually reach the barter. These knobs do
        // NOT force promotion (the labor/grain-flow sweep shows SALT never promotes at any
        // setting); they remove a barter offer-ordering ARTIFACT so the real economic
        // question — does produced bread monetize SALT — can be observed rather than masked.
        // The one-offer-per-agent barter offers an agent's LOWEST-good-id surplus first
        // (`post_first_direct_barter_offer`), and WOOD (id < bread) would otherwise always
        // preempt bread: a fed lineage that warms from a hearth surplus offers WOOD, never
        // its bread. So: drop the WOOD node (no idle-harvest WOOD flood) and size the hearth
        // warmth to a clean wash — `warmth_per_wood = 1` with `wood_provision = 1` means each
        // tick's WOOD is consumed for that tick's warmth (no WOOD surplus accrues), the
        // cultivator stays warm AND holds no WOOD, so its only offerable surplus is bread.
        cfg.nodes.retain(|n| n.good != WOOD);
        cfg.dynamics.warmth_per_wood = 1;
        if let Some(demo) = cfg.demography.as_mut() {
            for household in &mut demo.households {
                household.wood_provision = 1;
            }
        }
        cfg
    }
    /// S18 — **money from a produced MULTI-GOOD economy** (the deepest milestone; closes the
    /// S16 reframing finding). S16 proved produced bread can supply a market but SALT never
    /// monetized: with ONE produced good every bread↔SALT trade was DIRECT, so SALT accrued
    /// ZERO indirect-exchange breadth. S18 supplies a real **division of labor** with TWO
    /// produced/gathered goods and **role-separated cross-demand** — bread CULTIVATORS (sell
    /// bread, want WOOD) ⇄ WOODCUTTERS (sell WOOD, want bread) — so each accepts SALT as a
    /// MEANS to the OTHER good (`IndirectFor{target}`), SALT round-trips as the intermediary,
    /// and the two-sided indirect breadth `{bread, WOOD}` can cross the strong-bar gate.
    ///
    /// Derived from [`Self::frontier_money_from_cultivation`], it composes the THREE roles:
    /// the SALT-anchor consumers (the inherited non-lineage `consumer_medium_endowment`
    /// holders, buy BOTH bread and WOOD), the bread CULTIVATORS (the inherited lineages,
    /// `cultivation_sells_surplus` + `own_use_cultivation`, want WOOD/warmth), and the
    /// WOODCUTTERS (non-lineage `Gatherer`s producing + selling WOOD, wanting bread/food).
    /// The composed changes: (1) **re-add the WOOD node** S16 dropped —
    /// with role separation WOOD no longer preempts bread, so a real WOOD market forms;
    /// (2) **the woodcutter group** (`gatherers`), pinned to the WOOD node by the
    /// `multigood_money` seam (NOT the round-robin, so grain never draws them off into a
    /// third surplus); (3) **WOOD market-supplied AND provenance-clean** — `wood_provision =
    /// 0` (no mint) AND every initial WOOD buffer zeroed (`starting_wood`, `wood_buffer`,
    /// `consumer_wood_buffer`, the plain endowments), so traded WOOD can ONLY come from
    /// node-gathering (`endowment[WOOD] == 0`). Both food (own-labor) AND WOOD mints are now
    /// off. Mortality stays OFF (proven S17; a robustness test later). Each role's only
    /// SURPLUS is its produced good (no `post_first_direct_barter_offer` preemption). Existing
    /// scenarios remain byte-identical because they do not opt into this structural scenario
    /// or the `multigood_money` routing flag.
    pub fn frontier_multigood() -> Self {
        let mut cfg = Self::frontier_money_from_cultivation();
        // Re-add the WOOD node S16 dropped. Generous flow (the inherited 8000/64) so the WOOD
        // supply is never the bottleneck — the woodcutters always have WOOD to sell, and the
        // economic question is the monetization, not a WOOD scarcity race.
        cfg.nodes.push(NodeSpec {
            good: WOOD,
            pos: Pos::new(3, 0),
            stock: 8_000,
            regen: 64,
            cap: 8_000,
        });
        // The woodcutter group: non-lineage `Gatherer`s. The `multigood_money` flag routes
        // them to the WOOD node (Codex P1b) instead of the round-robin over `config.nodes`,
        // so grain (the cultivators' input) never draws them off into a third surplus. With
        // the buy/sell split on (inherited from S16) they do NOT forage/cultivate — their
        // only surplus is the WOOD they gather, and their only unsatisfied want is bread.
        cfg.gatherers = 6;
        if let Some(chain) = cfg.chain.as_mut() {
            chain.multigood_money = true;
            // WOOD must be provenance-clean (Codex P1a): zero every seeded WOOD buffer so the
            // only WOOD that can ever be traded was gathered at the node. The chain buffers
            // seed the non-lineage chain colonists (the 6 consumers + the woodcutters).
            chain.wood_buffer = 0;
            chain.consumer_wood_buffer = 0;
        }
        // The plain (non-chain) WOOD endowments — unused on the chain path, zeroed for a
        // consistent provenance-clean read.
        cfg.gatherer_wood_buffer = 0;
        cfg.consumer_wood_endowment = 0;
        // `wood_provision = 0` (no per-tick WOOD mint, was 1 in S16) and `starting_wood = 0`
        // (no founder WOOD seed): the cultivators are genuinely WOOD-short and must BUY WOOD,
        // and no minted/buffered WOOD can ever reach the market. The WOOD mint is now off too.
        if let Some(demo) = cfg.demography.as_mut() {
            for household in &mut demo.households {
                household.wood_provision = 0;
                household.starting_wood = 0;
            }
        }
        // Gate-align the two-sided bar (Codex P1d): S9's strong scenario uses
        // `min_indirect_target_goods = 1`; S18 claims emergence from the {bread, WOOD}
        // two-target breadth, so the honest gate is 2 — SALT cannot promote until BOTH targets
        // are present as indirect goods. The mechanism (the S9 strong-bar gate) is unchanged;
        // only this scenario parameter is stricter.
        if let Some(barter) = cfg.barter.as_mut() {
            barter.menger.min_indirect_target_goods = 2;
        }
        cfg
    }
    /// S19 — **imperfect-double-coincidence cycle money**: an artificial produced
    /// 3-good input loop with no pairwise double coincidence. Role A consumes Z and
    /// produces X, B consumes X and produces Y, C consumes Y and produces Z. X/Y/Z are
    /// demanded only as producer inputs (`Horizon::Next`), not as consumption goods.
    ///
    /// This is a closed exchange-topology test, not a scaffold-free economy: there is
    /// no terminal consumer, and survival is isolated by the producer hearth. SALT is a
    /// neutral commodity stock seeded to the cycle producers, with no medium want and
    /// no designated money. The heterogeneous direct-use anchor is period 4, the best
    /// disclosed density from the pinned sweep: SALT becomes the provisional leader but
    /// the run remains a finding because no indirect `IndirectFor` trade clears, so the
    /// strong-bar breadth and round-trip stay empty.
    pub fn frontier_cycle() -> Self {
        let mut cfg = Self::viable();
        cfg.width = 4;
        cfg.height = 1;
        cfg.exchange = Pos::new(0, 0);
        cfg.nodes.clear();
        cfg.gatherers = 0;
        cfg.consumers = 0;
        cfg.starting_gold_gatherer = 0;
        cfg.starting_gold_consumer = 0;
        cfg.gatherer_food_buffer = 0;
        cfg.gatherer_wood_buffer = 0;
        cfg.consumer_food_buffer = 0;
        cfg.consumer_wood_endowment = 0;
        cfg.demography = None;
        cfg.m3 = false;

        let mut chain = ChainConfig::three_good_cycle();
        chain.project_input_bids = true;
        // For cycle producers this is an ON/OFF gate, not a quantity: `run_producer_subsistence`
        // early-returns when it is 0, then overrides the per-good target to
        // `CYCLE_PRODUCER_SUBSISTENCE_CAP` (=4). So `1` means "subsistence on"; cycle producers
        // get up to the cap, not 1 unit.
        chain.producer_subsistence = 1;
        let (x, y, z) = chain
            .content
            .cycle_goods()
            .expect("the cycle content carries X/Y/Z");
        cfg.chain = Some(chain);
        cfg.barter = Some(BarterConfig {
            menger: MengerianConfig {
                candidate_goods: vec![x, y, z, SALT],
                min_indirect_acceptances: 12,
                min_indirect_acceptor_agents: 3,
                min_indirect_target_goods: 3,
                ..MengerianConfig::default()
            },
            medium_good: SALT,
            medium_want_qty: 0,
            gatherer_medium_endowment: 0,
            consumer_medium_endowment: 0,
            cycle_producer_medium_endowment: 12,
            salt_direct_use_qty: 1,
            salt_direct_use_period: 4,
        });
        cfg
    }
    /// S20 — the S19 3-good cycle with the gated two-lane medium book enabled.
    ///
    /// The S19 builder remains the flag-off control. This sibling changes only the
    /// barter-institution gate, so the existing cycle finding stays byte-identical
    /// while the viewer can run the cleared cycle directly.
    pub fn frontier_cycle_cleared() -> Self {
        let mut cfg = Self::frontier_cycle();
        cfg.barter
            .as_mut()
            .expect("cycle barter overlay")
            .menger
            .multi_offer_medium = true;
        cfg
    }
    /// S17 — **mortality** (the Malthusian positive check): the S15
    /// [`Self::frontier_cultivation`] colony with starvation death turned back on at the
    /// **principled** lab-default threshold `hunger_critical = need_max` (the others keep
    /// the `need_max + 1` dodge). On the fed-and-plateaued cultivation colony, sustained
    /// critical hunger now kills, so the population is bounded by **births AND deaths**
    /// both responding to the carrying capacity — the full Malthusian system the S14
    /// preventive check started.
    ///
    /// The ONLY change from `frontier_cultivation` is `dynamics.hunger_critical`; the death
    /// machinery (streak-gated kill, `settle_death → collect_estate → commons/heirs`) is
    /// reused unchanged. With it reverted to `need_max + 1` this is byte-identical to
    /// `frontier_cultivation`. Independent of money (the bread stays own-use). Whether the
    /// positive check is binding (a band), latent (the preventive check absorbs all
    /// pressure — the expected outcome), or too harsh (collapse) is the characterization,
    /// NOT tuned: the threshold and `death_window` (3) are the lab defaults.
    pub fn frontier_mortality() -> Self {
        let mut cfg = Self::frontier_cultivation();
        // Turn ON the positive check: `hunger` clamps at `need_max`, so `hunger_critical =
        // need_max` is the lowest reachable critical ceiling — the principled threshold,
        // config-only, no edit to the death machinery.
        cfg.dynamics.hunger_critical = cfg.dynamics.need_max;
        cfg
    }
    /// S21d — the **OPEN-SURVIVAL money probe** (impl-27): the strong co-emergent colony
    /// ([`Self::frontier_coemergent_strong`], never mutated) with the food hearths RETIRED and
    /// the full money machinery composed, so survival is a MARKET bread purchase. The deliverable
    /// is an honest probe + instrumentation, *run and classified* — likely a FINDING, not a
    /// success (two-layer saleability fixes the metric, not the production/bootstrap problem).
    ///
    /// Changes from the strong base (each disclosed, none tuned into a result):
    /// - **Retire the hearths** ([`ChainConfig::retire_food_mints`] = true): the demographic
    ///   `food_provision` and the producer staple floor no longer mint bread, so every agent —
    ///   producers and lineages included — must BUY (or produce) its food. WOOD/warmth provision
    ///   is unaffected (out of scope; disclosed). Unlike the S12 forage hack, NO FORAGE good is
    ///   interned.
    /// - **Compose the money machinery on the barter overlay:** `multi_offer_medium` (S20, the
    ///   two-lane book), `durability_aware_acceptance` + a marketability table (S21a: SALT
    ///   durable/costless, bread perishable, WOOD high-carry — so a perishable staple cannot
    ///   masquerade as the medium), `two_layer_saleability` + `min_direct_use_acceptors` (S21b:
    ///   direct-use is an eligibility floor, leadership ranks on medium share). The S21c
    ///   open-discovery lane rides on `two_layer_saleability && multi_offer_medium`. The S9
    ///   strong-bar gates (`min_indirect_acceptances = 12`, `min_indirect_acceptor_agents = 6`)
    ///   are inherited unchanged.
    /// - **Pre-promotion indirect breadth (Phase A) — bread ⇄ WOOD topology:** WOOD is the only
    ///   non-food terminal consumed good in the model (`life::scale` emits present-goods wants
    ///   only for hunger=bread and warmth=WOOD), so a *genuine* second non-food need is OUT OF
    ///   SCOPE (future work). SALT's pre-promotion indirect-target set is `{bread, WOOD}`; the bar
    ///   requires the non-food WOOD target present, so the gate is set to the available topology:
    ///   `min_indirect_target_goods = 2` (disclosed). A bread seller accepting SALT to later get
    ///   WOOD (target WOOD) and a WOOD seller accepting SALT to later get bread (target bread)
    ///   are the two legs.
    /// - **Mortality OFF:** `hunger_critical = need_max + 1` is INHERITED from the strong base
    ///   (NOT derived from `frontier_mortality`), isolating the money question from the
    ///   demographic one.
    /// - **Acquisition-channel ledger ON** ([`ChainConfig::acquisition_ledger`] = true): the
    ///   runtime-only per-agent FIFO ledger that proves survivors eat MARKET-acquired food after
    ///   warm-up (`bought` ≫ `seeded/minted`). Diagnostic, never digested.
    ///
    /// **Disclosed cold-start seeds** (bounded by `perishable_decay_bps = 1500`, inherited from
    /// the strong base): `bread_buffer = 64` (the barter-window bread the non-consumers sell),
    /// `consumer_staple_buffer = 2` (consumers start nearly bread-empty, so they buy),
    /// `consumer_medium_endowment = 80` SALT (the SALT-rich consumer hub), `latent_flour_seed = 12`
    /// (the first baker's flour finds a seller), and the inherited producer input buffers
    /// (`miller_grain_buffer = 0`, `baker_flour_buffer = 0`). The acceptance suite reports seed
    /// depletion separately so a "seed-only" non-result cannot masquerade as success.
    pub fn frontier_open_survival() -> Self {
        let mut cfg = Self::frontier_coemergent_strong();
        let bread = cfg
            .chain
            .as_ref()
            .expect("the strong co-emergent base carries a chain")
            .content
            .bread();
        if let Some(chain) = cfg.chain.as_mut() {
            // S21d.0: survival of EVERY agent is now a market bread purchase (no off-market
            // hearth mint, no own-labor forage floor — own-labor subsistence stays off here).
            chain.retire_food_mints = true;
            // S21d.1: the acquisition-channel ledger proves the food survivors eat is bought.
            chain.acquisition_ledger = true;
        }
        if let Some(barter) = cfg.barter.as_mut() {
            // S20: the two-lane medium book (a spend lane + a sell lane per agent).
            barter.menger.multi_offer_medium = true;
            // S21a: physical marketability — a prospective medium must carry through the holding
            // horizon. SALT is durable/costless (a money good); bread is perishable (it cannot
            // be the medium even though it dominates consumption); WOOD carries at a cost.
            barter.menger.durability_aware_acceptance = true;
            barter.menger.marketability = MarketabilityConfig {
                hold_horizon: 1,
                ..MarketabilityConfig::default()
            }
            .with_good(
                bread,
                GoodMarketability {
                    decay_bps: 10_000,
                    carry_cost: 0,
                },
            )
            .with_good(
                WOOD,
                GoodMarketability {
                    decay_bps: 0,
                    carry_cost: 1,
                },
            )
            .with_good(
                SALT,
                GoodMarketability {
                    decay_bps: 0,
                    carry_cost: 0,
                },
            );
            // S21b: two-layer saleability — direct-use breadth is an eligibility floor, medium
            // leadership/promotion ranks on indirect (re-trade) share. The direct-use floor is a
            // modest `2` distinct direct acceptors (the heterogeneous SALT direct-use anchor,
            // period 8, supplies them) — disclosed, not tuned.
            barter.menger.two_layer_saleability = true;
            barter.menger.min_direct_use_acceptors = 2;
            // The {bread, WOOD} two-target breadth bar: SALT cannot promote until it is taken as
            // a means toward BOTH the food (bread) and the non-food (WOOD) end — proving it is a
            // general medium, not merely a bread-buying token. Set to the available topology.
            barter.menger.min_indirect_target_goods = 2;
        }
        cfg
    }
    /// S21e — finite seeded-surplus diagnostic: derive from
    /// [`Self::frontier_open_survival`] and replace the recurring food mint's
    /// pre-promotion tradeable supply with a one-time finite bread surplus on the
    /// pinned mints-on seller classes. Disclosed differences only:
    ///
    /// - `seeded_surplus_bread = 512` on latent `Unassigned` bread-buffer holders
    ///   and demographic household consumers;
    /// - those same seller classes are made WOOD-poor enough to have an unsatisfied
    ///   WOOD target (`wood_buffer` reduced from 48 to 12, household WOOD zeroed),
    ///   so they can post `bread -> SALT IndirectFor{WOOD}` lanes.
    ///
    /// The food mints remain retired, mortality remains off, and the S20/S21a/b/c
    /// money machinery plus the bread/WOOD topology are inherited unchanged.
    pub fn frontier_seeded_surplus() -> Self {
        let mut cfg = Self::frontier_open_survival();
        if let Some(chain) = cfg.chain.as_mut() {
            chain.seeded_surplus_bread = 512;
            chain.wood_buffer = 12;
        }
        if let Some(demo) = cfg.demography.as_mut() {
            for household in &mut demo.households {
                household.starting_wood = 0;
                household.wood_provision = 0;
            }
        }
        cfg
    }
    /// S21f — **endogenous pre-money household production-for-barter**: the open-survival
    /// money base ([`Self::frontier_open_survival`], never mutated) with the pre-promotion
    /// tradeable bread supply made ENDOGENOUS — lineage households *cultivate* bread by
    /// their own labor (no forage substrate), eat what they need, and barter the surplus
    /// *before money exists*, so SALT emerges from barter over genuinely produced
    /// (`SelfProduced`) surplus rather than the S21e one-time seed. The Mengerian /
    /// regression-theorem bootstrap: direct production-for-use + barter of surplus precedes
    /// money.
    ///
    /// Composed changes vs the open-survival base, each disclosed (none tuned into a result):
    /// - **No seed, no cold-start bread (so NO bread enters as `SeededMinted`):**
    ///   `seeded_surplus_bread = 0` (inherited), `bread_buffer = 0`, `consumer_staple_buffer
    ///   = 0`, and the lineage founders' `starting_food = 0`. With the food mints retired and
    ///   every bread holding zeroed, the acquisition ledger credits ZERO `SeededMinted` bread,
    ///   so the pre-promotion bread that monetizes SALT can only be `SelfProduced`.
    /// - **Spatial households on** (`demography.spatial_households = true`): without it lineage
    ///   members never become spatial cultivators (eligibility needs `household.is_some() &&
    ///   spatial_active`) and the milestone is inert.
    /// - **Endogenous cultivation on:** the no-tool `Cultivate` recipe (`with_cultivate`),
    ///   `own_use_cultivation = true`, the §1 activation seam `household_barter_cultivation =
    ///   true` (cultivation without forage), `cultivation_sells_surplus = true` (the lineage-
    ///   only buy/sell split keeps the SALT-rich consumers a pure demand side), and the S15
    ///   cultivation knobs (`cultivate_hunger_in = 6`, `cultivate_hunger_out = 3`,
    ///   `cultivate_consume = 4`, `cultivate_patience = 2`). With the cold-start bread zeroed
    ///   the colony is hunger-stressed (mortality off, so agents stay hungry, don't die), so
    ///   the hunger hysteresis fires and lineage members cultivate.
    /// - **Pinned role topology (mirror S18 `frontier_multigood`, NOT S16):**
    ///   - **lineage members = the cultivators / bread sellers**, made WOOD-poor (`wood_buffer
    ///     = 12`, every household `starting_wood`/`wood_provision` zeroed) so their only
    ///     unsatisfied want is WOOD and their only offerable surplus is bread → they post
    ///     `bread → SALT IndirectFor{WOOD}`;
    ///   - **non-lineage `Consumer`s = the SALT-rich buy side** (`consumer_medium_endowment =
    ///     80`, inherited), not cultivation-eligible (the buy/sell split keeps them pure
    ///     demand);
    ///   - **`Gatherer`s = woodcutters**, pinned to the WOOD node by `multigood_money = true`
    ///     (NOT the round-robin, so grain — the cultivators' input — never draws them off into
    ///     a third surplus), so WOOD is genuinely gathered and sold.
    ///   - The WOOD node is KEPT (S21f needs the live WOOD market so cultivators can reach
    ///     WOOD via SALT); the WOOD<bread offer-ordering artifact is neutralized by role
    ///     separation + the zeroed lineage WOOD surplus, not by removing WOOD (the S16
    ///     artifact fix is wrong here).
    /// - **Grain flow (the disclosed recurring-supply axis):** the grain node sets the
    ///   cultivated-bread flow — a real depleting commons, recurring by design. Pinned to a
    ///   disclosed value (the base inherits a generous 8000/64/8000); the grain-flow sweep
    ///   proves promotion needs a real flow window (and that produced bread is grain-bounded —
    ///   recurring production, NOT seed exhaustion).
    ///
    /// The S21d/e money machinery (`retire_food_mints`, `acquisition_ledger`,
    /// `multi_offer_medium`, `durability_aware_acceptance` + the marketability table,
    /// `two_layer_saleability` + `min_direct_use_acceptors`, the S9 strong-bar gates,
    /// `min_indirect_target_goods = 2`) and mortality-OFF are inherited unchanged. With
    /// `household_barter_cultivation` reverted the cultivation seam is inert; the scenario is
    /// additive, so every existing golden stays byte-identical.
    pub fn frontier_household_barter() -> Self {
        let mut cfg = Self::frontier_open_survival();
        if let Some(chain) = cfg.chain.as_mut() {
            // No seed, no cold-start bread → NO bread enters as `SeededMinted`.
            chain.seeded_surplus_bread = 0;
            chain.bread_buffer = 0;
            chain.consumer_staple_buffer = 0;
            // The endogenous-cultivation stack: the no-tool grain→bread recipe + the gate +
            // the §1 activation seam (cultivation without the forage substrate) + the lineage
            // buy/sell split + the woodcutter→WOOD routing.
            chain.content = chain.content.clone().with_cultivate();
            chain.own_use_cultivation = true;
            chain.household_barter_cultivation = true;
            chain.cultivation_sells_surplus = true;
            chain.multigood_money = true;
            // The S15 cultivation hysteresis: escalate to cultivation at sustained hunger 6,
            // drop back below 3, eat 4 own-use bread/tick, after 2 consecutive hungry ticks.
            // `cultivate_hunger_in = 6 < birth_hunger_ceiling = 12` (the preventive check).
            chain.cultivate_hunger_in = 6;
            chain.cultivate_hunger_out = 3;
            chain.cultivate_consume = 4;
            chain.cultivate_patience = 2;
            // The cultivators (lineage) are made WOOD-poor so their only unsatisfied want is
            // WOOD and their only offerable surplus is bread (the non-lineage chain colonists'
            // WOOD buffer, mirror S21e). Their warmth is non-lethal (mortality off), so the
            // unmet WOOD want simply drives the `bread → SALT IndirectFor{WOOD}` lane.
            chain.wood_buffer = 12;
        }
        if let Some(demo) = cfg.demography.as_mut() {
            // Lineage members become spatial cultivators.
            demo.spatial_households = true;
            for household in &mut demo.households {
                // No cold-start bread for the lineage (so no `SeededMinted` bread can be
                // sold), and WOOD-poor (an unsatisfied WOOD target → the medium lane). The
                // food mint is retired, so `food_provision` is already a no-op; zeroing
                // `starting_food` makes the founders hunger-stressed from tick 0, which is
                // exactly the cold-start trigger for cultivation.
                household.starting_food = 0;
                household.starting_wood = 0;
                household.wood_provision = 0;
            }
        }
        // Pin the grain node to a disclosed recurring-supply flow (the base inherits a
        // generous 8000/64/8000). A real depleting commons whose regen sets the
        // cultivated-bread flow — the grain-flow sweep brackets the promotion window.
        let grain = cfg
            .chain
            .as_ref()
            .expect("the household-barter chain carries a grain good")
            .content
            .grain();
        for node in cfg.nodes.iter_mut() {
            if node.good == grain {
                node.stock = 480;
                node.regen = 24;
                node.cap = 960;
            }
        }
        cfg
    }
    /// S21g — **mortality-ON over the open-market colony** (the Malthusian band on a
    /// working money market): the endogenous household-barter colony
    /// ([`Self::frontier_household_barter`], never mutated) with the S17 positive check
    /// (starvation) turned ON, asking the capstone question — does the working money/food
    /// market survive real positive-check pressure? i.e. does the colony settle into a
    /// Malthusian band (births AND starvation deaths both binding, no extinction, no drift)
    /// *while SALT still emerges and circulates on `SelfProduced` bread*?
    ///
    /// A scenario composition, NO new engine code: the mortality wiring (S17) and the
    /// household-barter cultivation seam (S21f) both already exist. The ONLY two deltas vs
    /// S21f — both the S17 lab-default values, both disclosed, neither tuned into a result:
    /// - **Turn on the positive check** (`dynamics.hunger_critical = dynamics.need_max`,
    ///   inherited 13 → 12) — the exact analogue of [`Self::frontier_mortality`]'s only delta:
    ///   `hunger` clamps at `need_max`, so `hunger_critical = need_max` is the lowest reachable
    ///   critical ceiling, the principled threshold. `death_window = 3` is the inherited lab
    ///   default, untouched.
    /// - **Restore the S17 Malthusian-band structure (the preventive arm):**
    ///   `demography.birth_hunger_ceiling = 8`. S21f inherited `12` from the co-emergent base,
    ///   which *equals* the new critical ceiling — so births would stall and deaths fire at the
    ///   *same* hunger (a degenerate, positive-check-only band). S17 uses `8 < 12` so the
    ///   **preventive** check (births slow) binds *below* the **positive** check (starvation) —
    ///   the genuine Malthusian structure. The invariant `cultivate_hunger_in (6) <
    ///   birth_hunger_ceiling (8)` still holds (cultivation triggers before births are blocked).
    ///
    /// Everything else is identical to S21f: `retire_food_mints`,
    /// `household_barter_cultivation`, `cultivation_sells_surplus`, `multigood_money`,
    /// `spatial_households`, `bread_buffer = 0`, `consumer_staple_buffer = 0`, the lineage
    /// founders' `starting_food = 0`, the grain commons (480/24/960), and the full S20+S21a/b/c
    /// money machinery. **No cold-start cushion** (`starting_food` stays 0): the zeroed buffers
    /// and the retired mints are what keep the `SelfProduced` / `seeded_minted == 0` claim clean
    /// (`child_food_endowment = 4` is a conserved provenance-preserving parent→child transfer,
    /// never `SeededMinted`), and the cold-start trigger *is* the starting hunger. The
    /// provenance-clean lever if a cold-start die-off appears is grain-flow / `cultivate_*`
    /// timing (faster first production), NOT seed bread.
    ///
    /// Determinism: `hunger_critical` and `birth_hunger_ceiling` are both digested, but only
    /// THIS new scenario's digest changes (no existing golden moves, mirroring
    /// `frontier_mortality`); `starvation_deaths_total` stays runtime-only (NOT in
    /// `canonical_bytes`). With `hunger_critical` reverted to `need_max + 1` AND
    /// `birth_hunger_ceiling` back to `12` this is byte-identical to `frontier_household_barter`.
    pub fn frontier_open_colony_mortality() -> Self {
        let mut cfg = Self::frontier_household_barter();
        // Turn ON the positive check (the S17 delta): `hunger` clamps at `need_max`, so
        // `hunger_critical = need_max` is the lowest reachable critical ceiling — the
        // principled threshold, config-only, no edit to the death machinery.
        cfg.dynamics.hunger_critical = cfg.dynamics.need_max;
        // Restore the preventive arm BELOW the positive one (the S17 Malthusian structure):
        // births stall at hunger 8, starvation kills at 12, so the two checks bind at distinct
        // hungers (not the degenerate preventive=positive band S21f's inherited `12` would give).
        if let Some(demo) = cfg.demography.as_mut() {
            demo.birth_hunger_ceiling = 8;
        }
        cfg
    }
    /// S21h.0 — the **consumed-only demand-side survival cushion** (the bounded diagnostic):
    /// [`Self::frontier_open_colony_mortality`] (the S21g colony where the positive check
    /// culls the non-cultivating demand side before the money market forms) plus a finite
    /// STARTING bread cushion for the two non-lineage market roles that S21g wipes out:
    /// - the SALT-rich buyers (`Vocation::Consumer`): the already-wired `consumer_staple_buffer`;
    /// - the specialist woodcutters (`Vocation::Gatherer`): the new dedicated
    ///   `gatherer_food_cushion` (NOT the shared `bread_buffer`, which would re-seed the
    ///   lineage/seller bread and break the sold-for-SALT provenance).
    ///
    /// The cushion is **consumed-only**: it is swept to the `SeededMinted` acquisition channel
    /// at generation and eaten over the run — so S21h.0 RELAXES the S21g "`seeded_minted`
    /// *consumed* == 0" bar (the cushion IS eaten) but KEEPS the hard per-cell invariant
    /// "`seeded_minted` *sold-for-SALT* == 0" ([`Self::seeded_minted_bread_sold_for_salt`] /
    /// [`Self::pre_promotion_bread_for_salt_by_provenance`]): SALT must promote only on the
    /// lineage's `SelfProduced` bread, never on cushion bread (a cell that sells cushion bread
    /// for SALT is a seeded-supply result, not a demand-survival one — disqualified).
    ///
    /// The two cushion sizes are the disclosed knife-edge sweep axis (the acceptance suite
    /// brackets the regimes: too-weak → the S21g cull; a middle window → buyers survive AND
    /// still demand bread AND SALT promotes; too-strong → buyers satiated out of the market →
    /// no promotion). This default is one disclosed headline cell; every other delta is
    /// inherited from `frontier_open_colony_mortality`. Default off (both buffers 0) reverts
    /// to `frontier_open_colony_mortality` byte-for-byte.
    pub fn frontier_demand_cushion() -> Self {
        let mut cfg = Self::frontier_open_colony_mortality();
        if let Some(chain) = cfg.chain.as_mut() {
            // The headline cell (the suite sweeps both axes). A finite consumed-only cushion
            // for the two culled non-lineage roles — the buyers and the woodcutters.
            chain.consumer_staple_buffer = 16;
            chain.gatherer_food_cushion = 16;
        }
        cfg
    }
    /// S21h.1 — **produced emergency self-provisioning** (the authentic mechanism): instead of
    /// a finite seeded cushion, the non-lineage demand-side roles keep themselves alive by a
    /// produced, no-input, low-yield, self-consumed own-labor BREAD floor that fires only near
    /// starvation ([`Self::run_emergency_self_provision`], gated by
    /// [`ChainConfig::emergency_hunger_threshold`]). Derived from
    /// [`Self::frontier_open_colony_mortality`] with the emergency seam on and NO seeded
    /// cushion — so `seeded_minted == 0` is fully restored (the S21g provenance), and the only
    /// bread the non-lineage roles ever hold is the emergency `SelfProduced` floor they
    /// immediately eat (never offerable, never sold for SALT).
    ///
    /// The threshold is the disclosed yield-sweep axis: pinned at `11` here (~10–11 per the
    /// spec — above the lineage `cultivate_hunger_in = 6` trigger and strictly below
    /// `hunger_critical = 12`, so it fires within the alive-but-lethal-pressure window). The
    /// floor pulls projected hunger to `threshold - 1`, a near-critical level that keeps the
    /// role alive WITHOUT satiating it — so it still demands and prefers to BUY bread (the
    /// demand-preserving property a one-time cushion cannot guarantee). Default off (threshold
    /// 0) reverts to `frontier_open_colony_mortality` byte-for-byte.
    pub fn frontier_emergency_provision() -> Self {
        let mut cfg = Self::frontier_open_colony_mortality();
        if let Some(chain) = cfg.chain.as_mut() {
            // No seeded cushion (seeded_minted stays 0); the emergency floor is the only
            // demand-side survival lever. `11` is ~10–11, above cultivate_hunger_in (6) and
            // below hunger_critical (12) — the validated alive-but-lethal-pressure window.
            chain.emergency_hunger_threshold = 11;
        }
        cfg
    }
    /// S22a — **endogenous cultivation entry over the demand-bridged money colony** (the
    /// headline): [`Self::frontier_emergency_provision`] (the S21h.1 colony where money +
    /// mortality coexist on the pinned cultivator lineage) with the **only** change being
    /// `endogenous_cultivation_entry = true`. That relaxes cultivation eligibility from the
    /// spatial lineage to ANY spatial colonist (the `Consumer|Gatherer|Unassigned` vocation
    /// filter preserved), so the food-producing class can self-form from sustained hunger via
    /// the existing S15/S21f pressure/patience hysteresis rather than assigned lineage identity.
    ///
    /// Everything else is inherited unchanged: the S20+S21a/b/c money machinery, the emergency
    /// floor (`emergency_hunger_threshold = 11`), the grain commons (480/24/960), the WOOD-poor
    /// cultivator topology, `multigood_money`, `retire_food_mints`, `acquisition_ledger`, the
    /// `seeded_minted == 0` provenance, and mortality ON (`hunger_critical = need_max = 12`,
    /// `birth_hunger_ceiling = 8`). The central S22a question: does cultivation participation
    /// ENDOGENIZE (non-lineage agents enter under hunger and sell `SelfProduced` bread) while
    /// the open colony still promotes SALT and survives the positive check — or was the pinned
    /// producer lineage load-bearing (pinned-lineage necessity / commune collapse / etc.)?
    ///
    /// Determinism: `endogenous_cultivation_entry` is canonicalized ON-only (digest tag 7), so
    /// only THIS scenario's digest changes; with the flag reverted to `false` it is
    /// byte-identical to `frontier_emergency_provision`. The entrant-class provenance split, the
    /// rolling cultivator/buyer diagnostics, and the per-agent bought counter are all
    /// runtime-only (never digested).
    pub fn frontier_endogenous_cultivation() -> Self {
        let mut cfg = Self::frontier_emergency_provision();
        if let Some(chain) = cfg.chain.as_mut() {
            chain.endogenous_cultivation_entry = true;
        }
        cfg
    }
    /// S22a — **mortality-off endogenous-entry sanity variant** (diagnostic only): the S21f
    /// household-barter money colony ([`Self::frontier_household_barter`], mortality OFF) with
    /// `endogenous_cultivation_entry = true`. It proves the entry seam fires and the
    /// food-producing class can self-form WITHOUT the positive check confounding the read (no
    /// cold-start cull, no emergency floor) — the clean control for "does relaxing the producer
    /// identity admit non-lineage cultivators at all?" Not the headline (mortality is off);
    /// byte-identical to `frontier_household_barter` with the flag reverted.
    pub fn frontier_endogenous_cultivation_no_mortality() -> Self {
        let mut cfg = Self::frontier_household_barter();
        if let Some(chain) = cfg.chain.as_mut() {
            chain.endogenous_cultivation_entry = true;
        }
        cfg
    }
    /// S22b — **occupational stickiness via bounded cultivation skill** (the headline):
    /// [`Self::frontier_endogenous_cultivation`] (the S22a fluid-participation money colony,
    /// mortality on) with the **only** change being `cultivation_skill = true`. Skill is born at
    /// 0 and accumulates ([`SKILL_GAIN`]) while an agent actually cultivates (grain harvested AND
    /// converted to bread), decaying ([`SKILL_DECAY`]) while it does not, saturating at
    /// [`SKILL_CAP`]; it raises ONLY the per-trip grain-haul capacity of a cultivating agent's
    /// grain trip (the [`world::Task::GoHarvestWithRoom`] override, `≤ SKILL_HAUL_CEILING ×
    /// carry_cap`), a conservation-safe faster draw on the conserved grain node — never the
    /// bread-per-grain ratio.
    ///
    /// Everything else is inherited unchanged from S22a: the relaxed cultivation eligibility, the
    /// S20+S21a/b/c money machinery, the emergency floor, the grain commons, the WOOD-poor
    /// topology, the `seeded_minted == 0` provenance, and mortality on. The central S22b
    /// question: does mild accumulated productivity turn S22a's FLUID self-provisioning into a
    /// STABLE role split — a persistent cultivator cohort plus persistent non-cultivating buyers
    /// — while preserving money, mortality, and provenance? Or does it produce no stickiness /
    /// commune / monopolization / money failure (each a first-class finding)?
    ///
    /// Determinism: `cultivation_skill` is canonicalized ON-only (digest tag 8 + the skill
    /// parameters + the per-colonist skill state), so only THIS scenario's digest changes; with
    /// the flag reverted to `false` it is byte-identical to `frontier_endogenous_cultivation`.
    /// The skill-distribution / grain-share / persistent-cohort / churn diagnostics are all
    /// runtime-only (never digested).
    pub fn frontier_occupational_stickiness() -> Self {
        let mut cfg = Self::frontier_endogenous_cultivation();
        if let Some(chain) = cfg.chain.as_mut() {
            chain.cultivation_skill = true;
        }
        cfg
    }
    /// S22c — **profit-driven cultivation retention** (the HEADLINE):
    /// [`Self::frontier_endogenous_cultivation`] (the S22a fluid-participation money colony, skill
    /// OFF, mortality on) with the **only** change being `profit_driven_retention = true`. That
    /// makes the cultivation EXIT profit-modulated: a currently-cultivating agent remains
    /// cultivating past the normal hunger exit when, **only after money exists**
    /// (`current_money_good() == Some(SALT)`), its realized cultivation-sale return over a rolling
    /// [`RETURN_WINDOW`] clears both a small material floor and its outside option. Entry stays
    /// hunger/pressure-gated (S22a unchanged) — *hunger discovers the role; money makes it
    /// occupationally persistent.* Skill OFF isolates the stay decision as the sole new lever.
    ///
    /// Everything else is inherited unchanged from S22a: the relaxed cultivation eligibility, the
    /// S20+S21a/b/c money machinery, the emergency floor, the grain commons, the WOOD-poor
    /// topology, the `seeded_minted == 0` provenance, and mortality on. The central S22c question:
    /// does a realized monetary stay-decision turn S22a's FLUID participation into a STABLE role
    /// split — a persistent cultivator cohort plus persistent non-cultivating buyers — while
    /// preserving money, mortality, and provenance? Or does the signal not discriminate
    /// (SIGNAL VACUOUS), or stay without a cohort, or collapse the market (each a first-class
    /// finding)?
    ///
    /// Determinism: `profit_driven_retention` is canonicalized ON-only (digest tag 9 + the
    /// per-agent rolling-return window, which steers the next `cultivating` flag), so only THIS
    /// scenario's digest changes; with the flag reverted to `false` it is byte-identical to
    /// `frontier_endogenous_cultivation`. The counterfactual-flip / proceeds-distribution
    /// diagnostics it pairs with are runtime-only (never digested).
    pub fn frontier_profit_retention() -> Self {
        let mut cfg = Self::frontier_endogenous_cultivation();
        if let Some(chain) = cfg.chain.as_mut() {
            chain.profit_driven_retention = true;
        }
        cfg
    }
    /// S22c — **profit-driven retention composed with bounded skill** (the skill-ON variant, a
    /// composition read): [`Self::frontier_occupational_stickiness`] (the S22b skill colony) with
    /// the **only** change being `profit_driven_retention = true`. Skill (S22b) may raise the
    /// cultivation surplus a skilled cultivator can sell, but the stay is still mediated by
    /// *realized* gain, not by "skilled" — they compose, not conflate. Compared to its matched
    /// no-retention baseline `frontier_occupational_stickiness` (S22b), not the skill-off S22a
    /// baseline. Byte-identical to `frontier_occupational_stickiness` with the flag reverted.
    pub fn frontier_profit_retention_skill() -> Self {
        let mut cfg = Self::frontier_occupational_stickiness();
        if let Some(chain) = cfg.chain.as_mut() {
            chain.profit_driven_retention = true;
        }
        cfg
    }
    /// S22d — **durable role-specific cultivation capital** (the HEADLINE):
    /// [`Self::frontier_profit_retention`] (the S22c profit-stay money colony, skill OFF, mortality
    /// on) with the **only** changes being `durable_cultivation_tool = true` and the durable
    /// cultivation-tool good ([`content::CULTIVATION_TOOL`]) interned onto the content set. A
    /// sustained-producing cultivator may invest a SUNK cost (`tool_build_wood` WOOD +
    /// `tool_build_labor` labor) into a durable, OWNED plow it then keeps; the plow raises ONLY
    /// its owner's grain-haul ceiling ([`ChainConfig::cultivation_tool_haul_ceiling`] × `carry_cap`)
    /// **while it cultivates** (asset specificity). The owner's higher realized cultivation return
    /// then flows through the UNMODIFIED S22c profit-stay exit — no stay flag is added, no exit
    /// branch edited — so any stickiness comes from durable OWNERSHIP, not raw productivity.
    ///
    /// Everything else is inherited unchanged from S22c/S22a: the relaxed cultivation eligibility,
    /// the S20+S21 money machinery, the emergency floor, the grain commons, the WOOD-poor topology,
    /// the `seeded_minted == 0` provenance, profit-stay on, and mortality on. The central S22d
    /// question: does a durable, owned, role-specific cultivation tool finally turn the fluid regime
    /// into a stable role split — a persistent cohort of tool-owning cultivators plus persistent
    /// non-owner buyers — while money/mortality/provenance/conservation survive AND the stickiness
    /// is durability/ownership, not raw productivity (the controls falsify it)?
    ///
    /// Determinism: `durable_cultivation_tool` is canonicalized ON-only (digest tag 10 + the build
    /// params + the in-flight builds + the per-colonist cultivation tenure) and the plow good is
    /// interned only on THIS content set, so only this scenario's digest changes; every existing
    /// golden is byte-identical (the gate + good are confined to the new scenario). The tool-owner /
    /// sunk-cost / churn diagnostics it pairs with are runtime-only (never digested).
    pub fn frontier_cultivation_capital() -> Self {
        let mut cfg = Self::frontier_profit_retention();
        if let Some(chain) = cfg.chain.as_mut() {
            chain.durable_cultivation_tool = true;
            chain.content = chain.content.clone().with_cultivation_tool();
            // The WOOD-poor cultivation colony's cultivators are food-focused and hold little
            // WOOD (they buy it for warmth and consume it), so the producer-capital default
            // (`tool_build_wood = 6`) leaves the lever inert (almost no cultivator can ever
            // afford it). `1` is the modest sunk cost at which a sustained-producing cultivator
            // that transiently holds WOOD CAN invest — non-vacuous yet still a MINORITY (only a
            // few ever-cultivators capitalize; the sweep raises it to show the build-out boundary).
            // NOT tuned to a cohort target; the verdict test never asserts SUCCESS.
            chain.tool_build_wood = 1;
        }
        cfg
    }
    /// S22d — **durable cultivation capital with the profit-stay EFFECT neutralized** (the
    /// profit-stay-OFF variant, a control read): [`Self::frontier_cultivation_capital`] with the
    /// retention material floor set impossibly high so [`Settlement::profit_stay_active`] always
    /// returns false — the durable-capital gate stays active (it composes on the
    /// profit-driven-retention path, so the flag must remain set) and the tool still builds + boosts
    /// the owner's haul, but NO agent is ever retained by profit, so the only cultivation exit is
    /// hunger. Tests whether capital ALONE (without the profit-stay exit) moves the hunger-only exit
    /// — expected: no (the durable advantage needs the stay decision to bite). Reported as a
    /// control, not the headline.
    pub fn frontier_cultivation_capital_no_stay() -> Self {
        let mut cfg = Self::frontier_cultivation_capital();
        if let Some(chain) = cfg.chain.as_mut() {
            chain.retention_material_floor = u64::MAX;
        }
        cfg
    }
    /// S22d — the **non-durable / rented-tool CONTROL**: [`Self::frontier_cultivation_capital`] with
    /// `cultivation_tool_non_durable = true`, so a built plow is consumed after the one cultivation
    /// opportunity it boosts (no persistent stock). Same per-use owner-only productivity as the
    /// durable headline, but NO durable ownership — it isolates *durability*: it must NOT produce
    /// stickiness (else the durable headline's stickiness was not from persistence). A control.
    pub fn frontier_cultivation_capital_non_durable() -> Self {
        let mut cfg = Self::frontier_cultivation_capital();
        if let Some(chain) = cfg.chain.as_mut() {
            chain.cultivation_tool_non_durable = true;
        }
        cfg
    }
    /// S22d — the **productivity-only CONTROL**: [`Self::frontier_profit_retention`] (the S22c
    /// profit-stay colony, durable capital OFF) with bounded skill turned on and its haul ceiling
    /// raised to the tool's ([`CULTIVATION_TOOL_HAUL_CEILING`]). Driven with EVERY cultivator's
    /// skill pinned to the cap each tick (the test re-applies it), every cultivating agent draws the
    /// SAME boosted haul the tool confers — but with NO buildable, owned, durable asset (skill is
    /// earned, not owned, and pinned uniformly here). If this colony-wide productivity bump still
    /// produces stickiness, the durable headline's stickiness was raw output, not capital
    /// (PRODUCTIVITY ONLY). A control, never the headline.
    pub fn frontier_cultivation_capital_productivity_only() -> Self {
        let mut cfg = Self::frontier_profit_retention();
        if let Some(chain) = cfg.chain.as_mut() {
            chain.cultivation_skill = true;
            chain.skill_haul_ceiling = CULTIVATION_TOOL_HAUL_CEILING;
        }
        cfg
    }
    /// S22e — **the EXPANDED S22d base** (gate OFF): [`Self::frontier_cultivation_capital`] (the
    /// S22d durable-capital money colony) expanded to [`ENDOWED_ROSTER_HOUSEHOLDS`] lineage
    /// households. This is BOTH the matched-seed churn baseline AND the precondition colony for the
    /// S22e suite — the gate-off expanded base must reproduce S22d `NoStickiness` (money promotes,
    /// mortality coexists, no owner-lineage cohort, churn high). The headline flips the endowment
    /// gate on top of exactly this roster. Byte-identical to itself with the gate reverted (the gate
    /// is off here).
    pub fn frontier_endowed_capital_expanded_base() -> Self {
        Self::frontier_cultivation_capital().expanded_endowment_roster()
    }
    /// S22e — **endowed + inherited cultivation capital** (the HEADLINE):
    /// [`Self::frontier_endowed_capital_expanded_base`] (the expanded S22d money colony) with the
    /// **only** changes being `endowed_cultivation_capital = true`, a MINORITY
    /// `endowed_tool_count = ENDOWED_TOOL_COUNT_DEFAULT`, and `cultivation_tool_inheritance = true`.
    /// A minority of lineage households start with a plow at generation (a conservation-safe initial
    /// endowment, no earning required), and plows pass to the household heir on death (the existing
    /// estate routing). Everything else reuses S22d unchanged: the owner-exclusive haul boost, and
    /// stickiness arising ONLY through the unmodified S22c profit-stay (no exit edit, no fiat "stay"
    /// flag). The central S22e question: does a persistent owner-cultivator LINEAGE cohort finally
    /// form — and survive the founder's death (an inherited-tool heir in the sticky cohort) — while
    /// money + mortality + provenance + conservation survive, and is it a genuinely
    /// inheritance-stabilized class rather than a static re-pin (`EndowmentOnlyScaffold`) or an owner
    /// dynasty that starves the market (`InheritedMonopoly`)?
    ///
    /// Determinism: `endowed_cultivation_capital` is canonicalized ON-only when behavior-active
    /// (digest tag 11 + the endowed count + the inheritance switch + the granted household ids), so
    /// only THIS scenario's digest changes vs the expanded base; with the gate reverted it is
    /// byte-identical to `frontier_endowed_capital_expanded_base`. The endowment/inheritance-transfer
    /// diagnostics are runtime-only (never digested).
    pub fn frontier_endowed_capital() -> Self {
        let mut cfg = Self::frontier_endowed_capital_expanded_base();
        if let Some(chain) = cfg.chain.as_mut() {
            chain.endowed_cultivation_capital = true;
            chain.endowed_tool_count = ENDOWED_TOOL_COUNT_DEFAULT;
            chain.cultivation_tool_inheritance = true;
        }
        cfg
    }
    /// S22e — the **no-inheritance CONTROL** (the falsifying control that isolates whether
    /// inheritance is load-bearing): [`Self::frontier_endowed_capital`] with
    /// `cultivation_tool_inheritance = false`, so plows are FORCED to the commons on a holder's
    /// death even when the rest of the estate goes to the heir. Evaluated over the SAME
    /// post-founder-death window as the headline: if the headline owner-lineage cohort persists past
    /// the founders but this one does NOT, inheritance is load-bearing; if the headline only matches
    /// this within one generation, the headline is `EndowmentOnlyScaffold`.
    pub fn frontier_endowed_capital_no_inheritance() -> Self {
        let mut cfg = Self::frontier_endowed_capital();
        if let Some(chain) = cfg.chain.as_mut() {
            chain.cultivation_tool_inheritance = false;
        }
        cfg
    }
    /// S22e — the **no-endowment CONTROL**: [`Self::frontier_endowed_capital`] with
    /// `endowed_tool_count = 0` (inheritance still on). No household is endowed, so tools must be
    /// EARNED (the build path) exactly as S22d — expected `EndowmentLeverInert` / `NoStickiness` on
    /// the expanded base and byte-identical to it at generation. Isolates the endowment as the
    /// load-bearing lever.
    pub fn frontier_endowed_capital_no_endowment() -> Self {
        let mut cfg = Self::frontier_endowed_capital();
        if let Some(chain) = cfg.chain.as_mut() {
            chain.endowed_tool_count = 0;
        }
        cfg
    }
    /// S22e — the **too-many-tools CONTROL**: [`Self::frontier_endowed_capital`] with
    /// `endowed_tool_count` raised to the whole roster (universal ownership). Ownership is then NOT a
    /// minority (owner-share > `OWNER_SHARE_MAX`), so the classifier must return `UniversalOwnership`
    /// (topology, not an occupation), never `LineageStickySuccess`. The outcome-driving end of the
    /// `endowed_tool_count` axis.
    pub fn frontier_endowed_capital_too_many_tools() -> Self {
        let mut cfg = Self::frontier_endowed_capital();
        if let Some(chain) = cfg.chain.as_mut() {
            chain.endowed_tool_count = ENDOWED_ROSTER_HOUSEHOLDS as u16;
        }
        cfg
    }
    /// S22e — the **productivity-only CONTROL** on the expanded base:
    /// [`Self::frontier_cultivation_capital_productivity_only`] (the S22c profit-stay colony with
    /// bounded skill on and its haul ceiling raised to the tool's, durable capital + endowment OFF)
    /// expanded to [`ENDOWED_ROSTER_HOUSEHOLDS`]. Driven with every cultivator's skill pinned to the
    /// cap, so every cultivating agent draws the SAME boosted haul the plow confers — but with NO
    /// owned/endowed/inheritable asset. If this colony-wide productivity bump still clears the
    /// stickiness bars, the headline downgrades to `ProductivityOnly`.
    pub fn frontier_endowed_capital_productivity_only() -> Self {
        Self::frontier_cultivation_capital_productivity_only().expanded_endowment_roster()
    }
    /// S22f — **the EXPANDED no-capital S22c base** (the matched baseline + the precondition colony):
    /// [`Self::frontier_profit_retention`] (the S22a + S22c fluid money colony, skill OFF, durable +
    /// endowed capital both OFF, no plow content) expanded to [`ENDOWED_ROSTER_HOUSEHOLDS`] lineage
    /// households via the SAME proportional rescale the S22e base uses — but applied to the no-capital
    /// S22c base, NOT to `frontier_cultivation_capital`, so the headline carries no plows of any kind
    /// (Codex P1 #1). This is BOTH the `commitment_off` matched churn baseline AND the precondition
    /// colony for the S22f suite — the commitment-off expanded base must reproduce S22c/S22e
    /// no-stickiness. The headline simply flips the commitment gate on top of exactly this roster.
    /// Byte-identical to itself with the gate reverted (the gate is off here).
    pub fn frontier_profit_retention_expanded() -> Self {
        Self::frontier_profit_retention().expanded_endowment_roster()
    }
    /// S22f — **voluntary fixed-term cultivation commitment** (the HEADLINE):
    /// [`Self::frontier_profit_retention_expanded`] (the expanded no-capital S22c money colony) with
    /// the **only** change being `voluntary_cultivation_commitment = true`. Post-money, an eligible
    /// agent whose OWN realized cultivation-return signal clears the entry floor vs its outside option
    /// may VOLUNTARILY opt in to a cultivator commitment of [`COMMITMENT_TERM_DEFAULT`] econ ticks;
    /// while the term runs the normal hunger/profit exit cannot turn its cultivation off; at expiry it
    /// re-decides from FRESH realized returns (a renewal only if the signal still clears). The
    /// institution is configured; the UPTAKE (who commits, when) is endogenous and signal-gated. NO
    /// capital of any kind (durable + endowed both OFF), so any stickiness is the commitment institution
    /// alone, not "capital + a contract" (Codex P1 #1).
    ///
    /// The central S22f question: does an institution that changes the EXIT itself finally turn S22c's
    /// FLUID participation into a STABLE role split — a persistent VOLUNTARILY-committed cultivator
    /// cohort plus a fully-fluid non-committed buyer side — while money + mortality + provenance +
    /// conservation survive, AND is it a genuinely voluntary institution (not a fiat re-pin), with
    /// persistence across terms driven by renewals from fresh signals? Or is the offer unchosen
    /// (`CommitmentUnchosen`), uptake universal (`UniversalCommitment`), a disguised pin
    /// (`RePinScaffold`), or sticky-but-failing (`NoStickinessDespiteCommitment`)?
    ///
    /// Determinism: `voluntary_cultivation_commitment` is canonicalized ON-only (digest tag 12 + the
    /// term/floor/fiat-pin params + the per-colonist commitment state, which steers the next exit), so
    /// only THIS scenario's digest changes vs the expanded base; with the gate reverted it is
    /// byte-identical to `frontier_profit_retention_expanded`. The uptake/renewal/below-floor/
    /// exit-override diagnostics it pairs with are runtime-only (never digested).
    pub fn frontier_voluntary_commitment() -> Self {
        Self::frontier_profit_retention_expanded().with_voluntary_commitment()
    }
    /// S22f — the **unprofitable-offer CONTROL**: [`Self::frontier_voluntary_commitment`] with
    /// `commitment_entry_floor = u64::MAX`, so NO agent's realized cultivation-return signal ever
    /// clears the entry floor → ZERO uptake → `CommitmentUnchosen`. Proves the uptake is voluntary /
    /// signal-gated (not an automatic yes the moment the institution is offered).
    pub fn frontier_voluntary_commitment_unprofitable() -> Self {
        let mut cfg = Self::frontier_voluntary_commitment();
        if let Some(chain) = cfg.chain.as_mut() {
            chain.commitment_entry_floor = u64::MAX;
        }
        cfg
    }
    /// S22f — the **nonbinding-term CONTROL**: [`Self::frontier_voluntary_commitment`] with
    /// `commitment_term = 1`, so a "commitment" binds only the one tick on which it forms (and the
    /// agent re-decides every tick). A one-tick term should reproduce S22c MARGINAL retention —
    /// proving the binding *term*, not the act of committing, is what matters.
    pub fn frontier_voluntary_commitment_nonbinding() -> Self {
        let mut cfg = Self::frontier_voluntary_commitment();
        if let Some(chain) = cfg.chain.as_mut() {
            chain.commitment_term = 1;
        }
        cfg
    }
    /// S22f — the **fiat-pin CONTROL** (the key anti-repin falsifier):
    /// [`Self::frontier_voluntary_commitment`] with `commitment_fiat_pin = COMMITMENT_FIAT_PIN_DEFAULT`,
    /// so the voluntary signal-gated entry is BYPASSED and a configured minority of eligible agents
    /// are FORCE-committed from the first post-money tick and re-pinned on expiry. It must classify
    /// `RePinScaffold` and NEVER count as headline success: the forced commits record no signal-gated
    /// uptake, leave no below-floor non-committer set, and earn no fresh-signal renewals, so the
    /// voluntary headline is distinguishable even when both show low churn.
    pub fn frontier_voluntary_commitment_fiat_pin() -> Self {
        let mut cfg = Self::frontier_voluntary_commitment();
        if let Some(chain) = cfg.chain.as_mut() {
            chain.commitment_fiat_pin = COMMITMENT_FIAT_PIN_DEFAULT;
        }
        cfg
    }
    pub fn frontier_commitment_norm_spread() -> Self {
        let mut cfg = Self::frontier_voluntary_commitment();
        if let Some(chain) = cfg.chain.as_mut() {
            chain.commitment_norm_spread = true;
            chain.abandonable_norm = false;
            chain.commitment_seed_share_bps = COMMITMENT_SEED_SHARE_BPS_DEFAULT;
            chain.imitation_period = COMMITMENT_NORM_IMITATION_PERIOD_DEFAULT;
            chain.imitation_window = COMMITMENT_NORM_IMITATION_WINDOW_DEFAULT;
            chain.imitation_margin_bps = COMMITMENT_NORM_IMITATION_MARGIN_BPS_DEFAULT;
            chain.imitation_radius = COMMITMENT_NORM_IMITATION_RADIUS_DEFAULT;
            chain.imitation_max_models = COMMITMENT_NORM_IMITATION_MAX_MODELS_DEFAULT;
            chain.food_window_target = COMMITMENT_NORM_FOOD_WINDOW_TARGET_DEFAULT;
            chain.no_imitation = false;
            chain.random_imitation = false;
            chain.salt_in_score = false;
        }
        cfg
    }
    pub fn frontier_abandonable_norm() -> Self {
        let mut cfg = Self::frontier_commitment_norm_spread();
        if let Some(chain) = cfg.chain.as_mut() {
            chain.abandonable_norm = true;
        }
        cfg
    }
    pub fn frontier_group_payoff_imitation() -> Self {
        let mut cfg = Self::frontier_abandonable_norm();
        if let Some(chain) = cfg.chain.as_mut() {
            chain.group_payoff_imitation = true;
        }
        cfg
    }
    /// S22f — the **earned-capital composition variant** (SECONDARY, never required for the headline
    /// verdict): [`Self::frontier_cultivation_capital`] (the S22d durable-capital colony) expanded to
    /// [`ENDOWED_ROSTER_HOUSEHOLDS`] with the voluntary commitment gate on top. Tests whether the
    /// commitment institution composes with EARNED capital; reported separately so the headline must
    /// succeed/fail WITHOUT any capital (Codex P1 #1).
    pub fn frontier_voluntary_commitment_earned_capital() -> Self {
        Self::frontier_cultivation_capital()
            .expanded_endowment_roster()
            .with_voluntary_commitment()
    }
    /// S22f — the **endowed-capital composition variant** (SECONDARY, never required for the headline
    /// verdict): [`Self::frontier_endowed_capital`] (the S22e endowed + inherited capital colony) with
    /// the voluntary commitment gate on top. Tests whether the commitment institution composes with
    /// ENDOWED capital; reported separately so the headline must succeed/fail WITHOUT any capital.
    pub fn frontier_voluntary_commitment_endowed_capital() -> Self {
        Self::frontier_endowed_capital().with_voluntary_commitment()
    }
    /// S23a — **private land tenure** (the HEADLINE): [`Self::frontier_endogenous_cultivation`]
    /// expanded to the eight-household base, with the only new exit-cost mechanism being scarce,
    /// losable, owner-exclusive grain land. Good-near plots and poor-far marginal plots replace the
    /// grain commons at generation; claim is by first successful homesteading harvest, loss is by
    /// idle forfeiture, and inheritance follows the household heir. S22b-f exit/capital/commitment
    /// levers remain OFF in this headline.
    pub fn frontier_private_land_tenure() -> Self {
        let mut cfg = Self::frontier_endogenous_cultivation().expanded_endowment_roster();
        if let Some(chain) = cfg.chain.as_mut() {
            chain.private_land_tenure = true;
            chain.land_idle_limit = LAND_IDLE_LIMIT_DEFAULT;
            chain.harvest_gate = true;
            chain.forfeit_on_idle = true;
            chain.reclaim_reserved_for_prior_owner = false;
            chain.land_good_plots = LAND_GOOD_PLOTS_DEFAULT;
            chain.land_marginal_plots = LAND_MARGINAL_PLOTS_DEFAULT;
            chain.land_marginal_regen = LAND_MARGINAL_REGEN_DEFAULT;
        }
        cfg
    }
    /// S23c — **secure private land tenure**: the S23a finite, owner-exclusive plot
    /// registry over the same expanded S22a base, but title never lapses from idle use.
    /// Ownership turns over only through death inheritance, with the inheritance regime set by
    /// [`ChainConfig::inheritance_regime`].
    pub fn frontier_secure_land_tenure() -> Self {
        let mut cfg = Self::frontier_private_land_tenure();
        if let Some(chain) = cfg.chain.as_mut() {
            chain.secure_land_tenure = true;
            chain.harvest_gate = true;
            chain.forfeit_on_idle = false;
            chain.inheritance_regime = InheritanceRegime::Impartible;
            debug_assert!(chain_config_secure_land_tenure_active(chain));
        }
        cfg
    }
    /// S23d — **mortal-landowner demography base**: the verified S23c secure-title
    /// substrate over the S21f/S21h/S22a money-and-mortality colony, with homesteading
    /// title routed only to mortal reproducing lineage households. This is a fixed base
    /// composition, not a tenure comparison; the land market remains off and idle
    /// forfeiture remains off through secure tenure.
    pub fn frontier_mortal_landowner_demography() -> Self {
        let mut cfg = Self::frontier_secure_land_tenure();
        if let Some(chain) = cfg.chain.as_mut() {
            chain.mortal_landowner_demography = true;
            chain.land_market = false;
            chain.forfeit_on_idle = false;
            debug_assert!(chain_config_mortal_landowner_demography_active(chain));
        }
        cfg
    }
    /// S23b — **post-money alienable land market** (the HEADLINE): S23a's finite,
    /// owner-exclusive, heterogeneous plot registry over the S22a population-scaled base, with the
    /// land-market institution enabled. The market disables idle forfeiture from tick 0, but buying,
    /// selling, carrying costs, and foreclosure listings activate only after SALT promotes.
    pub fn frontier_land_market() -> Self {
        let mut cfg = Self::frontier_private_land_tenure();
        if let Some(chain) = cfg.chain.as_mut() {
            chain.land_market = true;
            chain.land_carrying_cost = LAND_CARRYING_COST_DEFAULT;
            chain.land_price_cap_factor = LAND_PRICE_CAP_FACTOR_DEFAULT;
            chain.land_marginal_plots = LAND_MARKET_TOTAL_PLOTS_DEFAULT - chain.land_good_plots;
            debug_assert!(chain_config_land_market_active(chain));
        }
        cfg
    }
}
