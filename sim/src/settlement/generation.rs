//! The settlement generator.
//!
//! `Settlement::generate` — the seed+config bootstrap that builds the world, places
//! colonists, seeds holdings/beliefs, and validates the config (every `expect`/panic
//! guard the generate_rejects_* tests pin) — plus its `generate_finance` variant for
//! finance-chain scenarios (called only from `generate`, so it keeps its privacy).
//! Extracted verbatim from `mod.rs` (pure code motion) into this sibling
//! `impl Settlement` block; `generate` was already `pub`, so all call sites resolve
//! unchanged with no re-import.

use super::*;

impl Settlement {
    /// Generate a settlement from `seed` and a [`SettlementConfig`]. All
    /// randomness (per-colonist culture) is drawn here; neither loop draws any.
    /// Deterministic: same `(seed, config)` → byte-identical settlement.
    pub fn generate(seed: u64, config: &SettlementConfig) -> Self {
        // G8c-1/G8c-2/G8c-3: a finance settlement (the credit cycle, a tender bench, or
        // the tax overlay on the cycle) is built from econ's unchanged scenario, not a
        // spatial colony — branch before the spatial setup. The guards live in
        // `generate_finance`.
        if config.cycle.is_some() || config.tender_bench.is_some() || config.tax.is_some() {
            return Self::generate_finance(seed, config);
        }
        let effective_config;
        let config = if config_private_land_tenure_active(config) {
            effective_config = config.with_private_land_layout();
            &effective_config
        } else {
            config
        };
        assert!(
            config.gatherers == 0 || !config.nodes.is_empty(),
            "a config with gatherers must define at least one resource node to harvest"
        );
        // Money (GOLD) is not a physical good: it never enters `self.goods`, so it
        // is excluded from deposit attribution, the transfer, and the conservation
        // report. A GOLD node would be harvested and deposited by the fast loop yet
        // never transferred or tracked — a silent world-side money leak. Reject it
        // at the seam rather than let the §4.3 "no money in the fast loop" rule and
        // whole-system conservation go blind to it.
        assert!(
            config.nodes.iter().all(|spec| spec.good != GOLD),
            "a resource node cannot harvest the money good (GOLD); money is not a \
             physical good and never crosses the world→econ transfer seam"
        );
        assert!(
            !config_multigood_money_active(config)
                || config.nodes.iter().any(|spec| spec.good == WOOD),
            "active multigood_money requires a WOOD resource node"
        );
        let dynamics = config.dynamics;
        // The need→good mapping. A plain settlement uses the lab default
        // (hunger ↔ FOOD). The G3a chain and the G3b emergent config make **bread
        // the staple** (hunger ↔ bread) so the chain's final good is what colonists
        // eat to live, and that demand prices bread. The G3b no-spread control sets
        // `bread_is_staple = false`, keeping hunger ↔ FOOD so bread is never demanded
        // (and so never prices, and so no role forms). Warmth stays WOOD.
        // The directly-edible subsistence fallback a bread-staple chain ranks below
        // bread. S12 own-labor subsistence wires the FORAGE good (the labor-produced
        // floor); otherwise the legacy `subsistence_on_grain` raw-grain edibility (off
        // by default). Both are `KnownGoods::subsistence`, read back as hunger relief
        // and interleaved below the staple by the subsistence offset (`scale.rs`).
        let chain_subsistence = |chain: &ChainConfig| -> Option<GoodId> {
            if chain.own_labor_subsistence {
                // A flag set without a forage good degrades to off (None), matching
                // `own_labor_subsistence_can_run` (the per-tick gate) — so a misconfigured
                // flag is treated as off everywhere rather than panicking in this path
                // while the per-tick gate silently disables it.
                chain.content.forage()
            } else {
                chain.subsistence_on_grain.then(|| chain.content.grain())
            }
        };
        let known = match (&config.chain, &config.barter) {
            // G5b **frontier**: a bread-staple chain composed with the barter-start
            // medium. Hunger ↔ bread (the chain's demand pulls the chain into being),
            // warmth WOOD, and savings is the **emergent medium** (e.g. SALT) — the
            // good that monetizes. Post-promotion the money market provisions that
            // store-of-value want with the emerged money exactly like the plain barter
            // camp, and the role-choice appraisal targets that same future-money want
            // (threaded with the current money good, not assumed to be GOLD).
            (Some(chain), Some(barter)) if chain.bread_is_staple => KnownGoods {
                hunger: chain.content.bread(),
                warmth: WOOD,
                savings: barter.medium_good,
                subsistence: chain_subsistence(chain),
            },
            (Some(chain), _) if chain.bread_is_staple => KnownGoods {
                hunger: chain.content.bread(),
                warmth: WOOD,
                savings: GOLD,
                subsistence: chain_subsistence(chain),
            },
            // The G5a barter camp (no chain) eats gathered FOOD, warms with WOOD,
            // and **saves the emergent medium** (e.g. SALT). Saving the good that
            // becomes money is what the lab's emergence scenarios do, and it is
            // load-bearing for the money phase: the promotion converts the medium
            // stock to gold while leaving the medium's place on every scale, so the
            // money market provisions those store-of-value wants with gold and
            // colonists trade FOOD/WOOD for money exactly like a designated-money
            // settlement. (Pre-promotion the medium is also demanded as a NEAR want
            // via a separate scale extension; that is what drives the barter for
            // it — a `Later` savings want alone never barters.)
            (None, Some(barter)) => KnownGoods {
                hunger: FOOD,
                warmth: WOOD,
                savings: barter.medium_good,
                subsistence: None,
            },
            // A barter-start chain whose bread is NOT the staple (hunger stays FOOD,
            // the no-spread control's shape) still circulates and is endowed the
            // emergent medium: `build_agent` always adds `barter.medium_good` under a
            // barter overlay and the post-promotion market runs `step_rejecting_v2_*`,
            // so the savings want MUST be that medium too. Falling through to
            // `lab_default` (savings GOLD) would save GOLD while the agent holds and
            // the market clears a non-GOLD medium, and `run_role_choice`'s
            // `soonest_savings_horizon(money_good)` would then find no matching want and
            // never adopt a role. No shipped config reaches this arm today (the
            // frontier is bread-staple; the no-spread control has no barter), but every
            // barter-start chain must keep its savings coherent with its medium.
            (Some(_), Some(barter)) => KnownGoods {
                hunger: FOOD,
                warmth: WOOD,
                savings: barter.medium_good,
                subsistence: None,
            },
            // The control (chain present, bread not the staple) eats seeded FOOD;
            // every plain settlement eats gathered FOOD, warms with WOOD, saves GOLD.
            (Some(_), None) | (None, None) => KnownGoods::lab_default(),
        };
        // The G5a barter overlay was the MECHANISM slice: a plain gatherer/consumer
        // camp. G5b **composes** it with production (a chain) and demography (the
        // `frontier` config), so that mutual-exclusion is lifted. What still holds is
        // that the emergent medium must be **non-renewable**: a good the settlement's
        // own substrate keeps minting (a gathered node good, a recipe output, or a
        // demography-provisioned staple) cannot be the money good, because future
        // minting would create physical units of it *after* econ removed it from the
        // money-priced market, breaking the conserved promotion. The promotion
        // rejection list (`money_rejection_goods`) enforces that at the step boundary;
        // these asserts reject the unsupportable medium loudly at generation.
        if let Some(barter) = &config.barter {
            assert!(
                config
                    .nodes
                    .iter()
                    .all(|spec| spec.good != barter.medium_good),
                "the emergent medium must not be a gathered node good (the world would \
                 regenerate the money good, breaking the conserved promotion)"
            );
            // A chain's goods (the gathered raw, the recipe outputs, the durable tools)
            // are all renewable or capital — none can be the money good. Reject a medium
            // that names one rather than ship a config whose chain would re-mint the
            // money good after promotion.
            assert!(
                config
                    .chain
                    .as_ref()
                    .is_none_or(|chain| !chain.content.goods().contains(&barter.medium_good)),
                "the emergent medium must not be a production-chain good (a recipe output \
                 or raw input the chain keeps producing, breaking the conserved promotion)"
            );
            // The demography household hearth provisions the hunger staple and WOOD every
            // tick — both renewable sources. The medium must be neither, or the promotion
            // would convert a stock the provision keeps refilling.
            assert!(
                config.demography.is_none()
                    || (barter.medium_good != WOOD && barter.medium_good != known.hunger),
                "the emergent medium must not be a demography-provisioned good (the \
                 household hearth would keep minting the money good after promotion)"
            );
            // The emergent medium is a PHYSICAL good that circulates as barter stock
            // before promotion, so it must not be GOLD: GOLD is the money ledger, not
            // a physical good — it never enters `self.goods`, the deposit attribution,
            // the transfer, or the conservation report. A GOLD medium endowment would
            // mint stock the digest and whole-system ledger never track (a silent
            // money leak), and the promotion's good→money conversion is meaningless
            // when the "good" is already money. Reject it at the seam.
            assert!(
                barter.medium_good != GOLD,
                "the emergent medium cannot be GOLD; GOLD is the money ledger, not a \
                 physical good, so an endowed GOLD medium would create untracked stock \
                 the conservation report and digest never see"
            );
            assert!(
                config.starting_gold_gatherer == 0 && config.starting_gold_consumer == 0,
                "a barter-start camp holds no money before promotion (econ's V2 path \
                 requires zero initial money balances)"
            );
            // The G5b frontier composes the camp with a production chain and demography,
            // each of which has its OWN gold endowment knob. The V2 promotion converts
            // each agent's medium stock to gold and refuses to commit if ANY agent
            // already holds gold (`NonZeroMoneyBalance`), so every gold source — the
            // producers' working capital, the household founders' starting gold, and the
            // newborn gift — must also be zero before promotion. Reject a composed config
            // that seeds money loudly here rather than silently never-promote.
            assert!(
                config
                    .chain
                    .as_ref()
                    .is_none_or(|chain| chain.producer_gold == 0),
                "a barter-start frontier holds no money before promotion: a chain's \
                 producer_gold must be 0 under a barter overlay"
            );
            assert!(
                config.demography.as_ref().is_none_or(|demo| {
                    demo.child_gold_endowment == 0
                        && demo.households.iter().all(|h| h.starting_gold == 0)
                }),
                "a barter-start frontier holds no money before promotion: demography \
                 starting_gold and child_gold_endowment must be 0 under a barter overlay"
            );
        }
        if let Some(chain) = &config.chain {
            assert!(
                chain.operating_cost >= 1,
                "chain operating_cost must be at least 1"
            );
            assert!(
                matches!(
                    (chain.birth_stock_saving, chain.birth_stock_saving_mode),
                    (false, BirthStockSavingMode::Off)
                        | (true, BirthStockSavingMode::Motive)
                        | (false, BirthStockSavingMode::SufficiencyControl)
                ),
                "birth-stock motive and sufficiency control must be mutually exclusive"
            );
            if let Some(prevalence) = chain.fixed_commitment_norm_prevalence {
                assert!(
                    prevalence.is_finite() && (0.0..=1.0).contains(&prevalence),
                    "fixed_commitment_norm_prevalence must be finite and in [0, 1]"
                );
            }
            // A producer's throughput becomes that many input wants on its value scale
            // each regeneration; bound it so a config cannot drive the scale (and the
            // market that iterates it) to an unbounded size. See [`MAX_CHAIN_THROUGHPUT`].
            assert!(
                chain.throughput <= MAX_CHAIN_THROUGHPUT,
                "chain throughput {} exceeds the sanity bound {MAX_CHAIN_THROUGHPUT}",
                chain.throughput
            );
        }
        // The G4b demography overlay provisions the **hunger staple** as the household
        // hearth (`deliver_demography_provisions`, the birth food gate, and the newborn
        // endowment all use [`KnownGoods::hunger`]). A plain/`lineages` settlement maps
        // hunger ↔ FOOD, so it provisions FOOD exactly as G4b did (byte-identical); the
        // G5b frontier maps hunger ↔ bread, so the same path provisions bread — members
        // are always fed the good they eat, so the pre-G5b "non-FOOD staple starves the
        // household" hazard cannot arise and needs no generation guard.
        let mut rng = Rng::new(seed);

        // ---- world: grid, exchange stockpile, resource nodes ----
        let grid = Grid::new(config.width, config.height);
        let mut world = World::new(grid);
        let exchange = world
            .add_stockpile(Stockpile::new(config.exchange, config.exchange_cap))
            .expect("exchange lands on a passable tile");
        let mut node_ids = Vec::with_capacity(config.nodes.len());
        for spec in &config.nodes {
            let id = world
                .add_node(ResourceNode::new(
                    spec.pos, spec.good, spec.stock, spec.regen, spec.cap,
                ))
                .expect("node lands on a passable tile");
            node_ids.push(id);
        }
        // S12/S14: the FORAGE node. Placed OUTSIDE `config.nodes` (so the gatherer
        // round-robin never targets it — only the forage path does) at the exchange tile
        // ("eaten at home"), only when own-labor subsistence is on, so every other config
        // adds no node and stays byte-identical.
        //
        // - S12 (no `forage_commons`): a pure `0/0/0` location marker for `GoForage`,
        //   which relocates nothing — the floor is credited at the econ layer (booked
        //   `produced`), so node regen stays the world's only source.
        // - S14 (`forage_commons` set): a REAL depleting `ResourceNode` with the
        //   configured `stock/regen/cap`. Foragers harvest it through the GoHarvest haul
        //   cycle, so per-capita yield falls with the foraging population — and node regen
        //   is again the only source (the fixed credit is retired). Conservation holds in
        //   both modes.
        //
        // Place the node only when the own-labor path can actually run (the flag AND a
        // forage good in the content), matching `own_labor_subsistence_can_run`; a flag
        // set without a forage good degrades to off (no node) rather than panicking.
        let forage_node_id = if let Some(forage) = config
            .chain
            .as_ref()
            .filter(|chain| chain.own_labor_subsistence)
            .and_then(|chain| chain.content.forage())
        {
            let (stock, regen, cap) = config
                .chain
                .as_ref()
                .and_then(|chain| chain.forage_commons.as_ref())
                .map_or((0, 0, 0), |commons| {
                    (commons.stock, commons.regen, commons.cap)
                });
            // Capture the id so the forage path always targets THIS node, even when a
            // config ALSO defines a `NodeSpec` for the FORAGE good — a resolve-by-good
            // lookup would otherwise find that (earlier) node and deplete it, bypassing
            // the configured commons stock/regen/cap (the isolation S14 promises).
            let id = world
                .add_node(ResourceNode::new(
                    config.exchange,
                    forage,
                    stock,
                    regen,
                    cap,
                ))
                .expect("the forage node lands on the (passable) exchange tile");
            Some(id)
        } else {
            None
        };

        let consumers = usize::from(config.consumers);
        let gatherers = usize::from(config.gatherers);
        // The seeded producer counts (G3a) and the G3b *latent* producer counts:
        // all zero without a chain, so a plain settlement's population, ids, and
        // digest are byte-identical to G2b. Seeded millers/bakers (G3a) take a fixed
        // producer vocation; the latent pool (G3b) starts `Unassigned` and adopts
        // from the spread. Both bands follow the gatherers in id order.
        let (millers, bakers, latent_millers, latent_bakers) = match &config.chain {
            Some(chain) => (
                usize::from(chain.millers),
                usize::from(chain.bakers),
                usize::from(chain.latent_millers),
                usize::from(chain.latent_bakers),
            ),
            None => (0, 0, 0, 0),
        };
        // G6b seeded scholars + confectioners: both zero without a research chain, so
        // every pre-G6b config's population, ids, and digest are byte-identical. They
        // follow the latent pool in id order (the highest colonist ids).
        let (scholars, confectioners) = match &config.chain {
            Some(chain) => (
                usize::from(chain.scholars),
                usize::from(chain.confectioners),
            ),
            None => (0, 0),
        };
        let (cycle_a, cycle_b, cycle_c) = match &config.chain {
            Some(chain) => (
                usize::from(chain.cycle_a_producers),
                usize::from(chain.cycle_b_producers),
                usize::from(chain.cycle_c_producers),
            ),
            None => (0, 0, 0),
        };
        let population = consumers
            + gatherers
            + millers
            + bakers
            + latent_millers
            + latent_bakers
            + scholars
            + confectioners
            + cycle_a
            + cycle_b
            + cycle_c;

        // Resident traders (G2c caravans) take the LOWEST ids, *before* the
        // colonists, so they are processed first in the id-ordered market and their
        // resting orders are the **price-setting makers** the rest of the book
        // crosses (a caravan trader leads the book: a seller's cheap ask becomes the
        // realized price, pulling a dear market down toward the cheap one). A trader
        // is otherwise inert at generation — an EMPTY scale posts no orders until
        // the `Region` activates it — and it is not a colonist (no need/scale/task
        // phase touches it). It is given a *parked* world agent at the exchange (so
        // world and econ `AgentId`s stay coincident for the colonists that follow);
        // routes are abstract, so the trader is never tasked and its world agent
        // just idles, carrying nothing. No randomness is drawn for traders — the
        // `Region`, not the settlement, drives them deterministically.
        let num_traders = config.resident_traders.len();
        let mut colonists = Vec::with_capacity(population);
        let mut agents = Vec::with_capacity(num_traders + population);
        let mut trader_ids = Vec::with_capacity(num_traders);
        for (offset, endowment) in config.resident_traders.iter().enumerate() {
            let id = AgentId(offset as u64);
            let placed = world
                .add_agent(config.exchange, config.carry_cap, config.move_speed)
                .expect("trader lands on the exchange tile");
            debug_assert_eq!(placed, id, "world and econ trader ids must coincide");
            agents.push(build_trader_agent(id, endowment));
            trader_ids.push(id);
        }

        // Consumers take the LOWER colonist ids so their FOOD bids rest before the
        // gatherers' asks and set the realized price (the supply-sensitive,
        // buyers-lead book; see the module docs). Gatherers follow. Colonist ids
        // begin at `num_traders` (the trader pair, if any, leads); for a plain
        // settlement `num_traders == 0`, so colonists keep ids 0,1,2,… exactly as
        // in G2b and every existing config and golden is byte-identical. World
        // `AgentId`s match econ `AgentId`s by construction (assigned in this order).
        let colonist_id_base = num_traders as u64;
        let mortal_chain_producers = config_mortal_chain_producers_active(config);
        let mortal_producer_inheritance = config_mortal_producer_inheritance_active(config);
        let producer_household_start = if mortal_producer_inheritance {
            config.demography.as_ref().and_then(|demo| {
                demo.households
                    .len()
                    .checked_sub(MORTAL_PRODUCER_HOUSEHOLDS)
            })
        } else {
            None
        };
        // S18: the woodcutter→WOOD-node seam. With the multi-good money path on, every
        // non-lineage gatherer is pinned to the WOOD node (the lowest-id WOOD-yielding node
        // in `config.nodes`, matched into `node_ids` by build order); `None` off the flag,
        // so the gatherer node assignment stays the round-robin and every existing config is
        // byte-identical.
        let woodcutter_node = if config_multigood_money_active(config) {
            let wood_index = config
                .nodes
                .iter()
                .position(|spec| spec.good == WOOD)
                .expect("active multigood_money requires a WOOD resource node");
            Some(node_ids[wood_index])
        } else {
            None
        };
        for index in 0..population {
            let id = AgentId(colonist_id_base + index as u64);
            // World agent for every colonist (consumers idle at the exchange,
            // gatherers haul); placement at the exchange tile is always passable.
            let placed = world
                .add_agent(config.exchange, config.carry_cap, config.move_speed)
                .expect("colonist lands on the exchange tile");
            debug_assert_eq!(placed, id, "world and econ agent ids must coincide");

            // Vocation by id band: consumers (lowest ids, so their bids lead the
            // book), then gatherers, then the seeded producers (G3a) — millers,
            // then bakers — then the latent pool (G3b) — latent millers, then
            // latent bakers — that start `Unassigned` and adopt from the spread.
            // Producers do not gather (no node) and use the patient consumer
            // time-preference base so they keep offering their output and carry a
            // savings want the entrepreneurial appraisal can target.
            let seeded_end = consumers + gatherers + millers + bakers;
            let latent_end = seeded_end + latent_millers + latent_bakers;
            let scholar_end = latent_end + scholars;
            let confectioner_end = scholar_end + confectioners;
            let cycle_a_end = confectioner_end + cycle_a;
            let cycle_b_end = cycle_a_end + cycle_b;
            let (vocation, node, tp_base, latent) = if index < consumers {
                (
                    Vocation::Consumer,
                    None,
                    config.consumer_time_preference_base_bps,
                    None,
                )
            } else if index < consumers + gatherers {
                // S18: with the multi-good money path on, the non-lineage gatherers are the
                // WOODCUTTER role — pinned to the WOOD node (Codex P1b) instead of the
                // round-robin over `config.nodes`. With both a grain node (the cultivators'
                // input) and a WOOD node present, the round-robin would split the
                // woodcutters across grain and WOOD, drawing some off into a third surplus;
                // routing them all to WOOD keeps each role's only surplus its produced good.
                // Off the flag the assignment is exactly the round-robin, byte-identical.
                let node = woodcutter_node
                    .unwrap_or_else(|| node_ids[(index - consumers) % node_ids.len()]);
                (
                    Vocation::Gatherer,
                    Some(node),
                    config.gatherer_time_preference_base_bps,
                    None,
                )
            } else if index < consumers + gatherers + millers {
                (
                    Vocation::Miller,
                    None,
                    config.consumer_time_preference_base_bps,
                    None,
                )
            } else if index < seeded_end {
                (
                    Vocation::Baker,
                    None,
                    config.consumer_time_preference_base_bps,
                    None,
                )
            } else if index < seeded_end + latent_millers {
                (
                    Vocation::Unassigned,
                    None,
                    config.consumer_time_preference_base_bps,
                    Some(RecipeId::Mill),
                )
            } else if index < latent_end {
                (
                    Vocation::Unassigned,
                    None,
                    config.consumer_time_preference_base_bps,
                    Some(RecipeId::Bake),
                )
            } else if index < scholar_end {
                // G6b: a seeded scholar — patient (so it carries a savings want and
                // keeps offering nothing it needs), holding a library + grain buffer.
                (
                    Vocation::Scholar,
                    None,
                    config.consumer_time_preference_base_bps,
                    None,
                )
            } else if index < confectioner_end {
                // G6b: a seeded confectioner — holds an atelier + flour buffer, runs
                // the tier-2 recipe once unlocked.
                (
                    Vocation::Confectioner,
                    None,
                    config.consumer_time_preference_base_bps,
                    None,
                )
            } else if index < cycle_a_end {
                (
                    Vocation::CycleA,
                    None,
                    config.consumer_time_preference_base_bps,
                    None,
                )
            } else if index < cycle_b_end {
                (
                    Vocation::CycleB,
                    None,
                    config.consumer_time_preference_base_bps,
                    None,
                )
            } else {
                (
                    Vocation::CycleC,
                    None,
                    config.consumer_time_preference_base_bps,
                    None,
                )
            };
            // C3R.a: EVERY seeded chain producer becomes a lifespan-only mortal under
            // the flag — the active miller/baker band `[producer_band_start, seeded_end)`
            // AND the latent mill/bake pool `[seeded_end, latent_end)`. Covering the
            // whole producer band (not just the latent tail) makes the reservoir-closed
            // invariant `immortal_producer_count == 0` structural for any flag-on config,
            // not incidental to the millers=bakers=0 frontier. Byte-identical on that
            // frontier, where `producer_band_start == seeded_end`.
            let producer_band_start = consumers + gatherers;
            let (age, lifespan, colonist_seed) =
                if mortal_chain_producers && index >= producer_band_start && index < latent_end {
                    let demo = config
                        .demography
                        .as_ref()
                        .expect("active mortal_chain_producers requires demography");
                    let producer_seed = founder_seed(
                        seed,
                        MORTAL_CHAIN_PRODUCER_SEED_OFFSET + (index - producer_band_start),
                    );
                    (
                        demo.founder_start_age_ticks(producer_seed),
                        Some(demo.lifespan_ticks(producer_seed)),
                        producer_seed,
                    )
                } else {
                    (0, None, 0)
                };
            let producer_household = producer_household_start.and_then(|start| {
                (index >= producer_band_start && index < latent_end)
                    .then(|| index - producer_band_start)
                    .and_then(|producer_offset| {
                        (producer_offset < MORTAL_PRODUCER_HOUSEHOLDS)
                            .then(|| start + producer_offset)
                    })
            });
            let culture = draw_culture(
                &mut rng,
                tp_base,
                config.leisure_weight_base_bps,
                config.forecast_bias_base_bps,
                config.forecast_bias_jitter_bps,
            );
            let need = NeedState::rested();
            agents.push(build_agent(
                id, &need, &culture, &known, vocation, latent, config,
            ));
            colonists.push(Colonist {
                id,
                vocation,
                node,
                // S6: a spatial colonist's home role is its generated vocation+node.
                home_vocation: vocation,
                home_node: node,
                need,
                culture,
                critical_streak: 0,
                alive: true,
                latent,
                // Pre-G4b colonists carry no household; C3R.b is the narrow exception
                // that tags the seeded producer band into dedicated producer houses.
                household: producer_household,
                parent: None,
                age,
                lifespan,
                seed: colonist_seed,
                estate_destination: None,
                acquired_tool: false,
                foraging: false,
                cultivating: false,
                cultivate_pressure: 0,
                cultivation_stock_pending: false,
                cultivation_skill: 0,
                cultivation_return_window: VecDeque::new(),
                cultivation_tenure: 0,
                commitment_remaining: 0,
                commitment_renewals: 0,
                adopts_commitment_norm: false,
                next_norm_bit: None,
                commitment_norm_seed_adopter: false,
                commitment_norm_observations: VecDeque::new(),
                carried_grain_source: None,
                carried_share_contract_id: None,
                carried_in_kind_contract_id: None,
            });
        }

        // ---- G4b demography founders: the household members ----
        // When a demography overlay is present, its households' founders follow the
        // normal colonist roster in id order (a non-demography settlement adds none,
        // so it is byte-identical). Pre-S13 a founder is a NON-SPATIAL householder
        // (an econ agent, no world agent). S13 spatial households (gated, default off)
        // instead give each founder a world agent at its EXACT econ id via
        // `add_agent_with_id`, so `world_id == econ_id` by construction and the
        // reproducing population can be assigned world tasks — the structural
        // unification. The founder still stays Idle (no task) and fed exactly as
        // today; only the capability is new. Its stable seed (hashed from the world
        // seed + its global founder index — no extra `Rng` draw) fixes its staggered
        // starting age and its deterministic old-age lifespan; its culture is drawn
        // from the household's time-preference base (the heritable ordinal bias).
        let mut households: Vec<HouseholdRuntime> = Vec::new();
        if let Some(demo) = &config.demography {
            let spatial = demo.spatial_households;
            // S14: on the forage-commons path founders seed their starting food in the
            // FORAGE subsistence good (the colony's actual food), so the lineage can feed
            // and reproduce on forage from tick 0 rather than on a bread buffer that is
            // never replenished (the food mint is retired). Off the path it is the hunger
            // staple, byte-identical. Uses the SAME selector as the birth endowment.
            let founder_food = birth_food_good(config_forage_commons_active(config), &known);
            let mut founder_index = 0usize;
            for (household_index, spec) in demo.households.iter().enumerate() {
                households.push(HouseholdRuntime {
                    last_birth_tick: None,
                });
                for _ in 0..spec.founders {
                    let id = AgentId(colonist_id_base + colonists.len() as u64);
                    let seed = founder_seed(seed, founder_index);
                    founder_index += 1;
                    let culture = draw_culture(
                        &mut rng,
                        spec.time_preference_base_bps,
                        config.leisure_weight_base_bps,
                        config.forecast_bias_base_bps,
                        config.forecast_bias_jitter_bps,
                    );
                    let need = NeedState::rested();
                    let vocation = Vocation::Consumer;
                    let mut agent =
                        build_demography_agent(id, &need, &culture, &known, spec, founder_food);
                    if let Some(chain) = config.chain.as_ref() {
                        if chain.seeded_surplus_bread > 0
                            && seeded_surplus_seller_class(vocation, Some(household_index))
                        {
                            agent
                                .stock
                                .add(chain.content.bread(), chain.seeded_surplus_bread);
                        }
                    }
                    agents.push(agent);
                    if spatial {
                        // Mirror the founder's econ id into the world (generation 0, so
                        // it bumps the world's fresh-id watermark past it). Placement at
                        // the exchange tile is always passable.
                        let placed = world
                            .add_agent_with_id(
                                id,
                                config.exchange,
                                config.carry_cap,
                                config.move_speed,
                            )
                            .expect("founder world agent lands on the exchange tile");
                        debug_assert_eq!(placed, id, "founder world and econ ids must coincide");
                    }
                    colonists.push(Colonist {
                        id,
                        vocation,
                        node: None,
                        // A lineage founder is hearth-fed and never re-entered.
                        home_vocation: Vocation::Consumer,
                        home_node: None,
                        need,
                        culture,
                        critical_streak: 0,
                        alive: true,
                        latent: None,
                        household: Some(household_index),
                        parent: None,
                        age: demo.founder_start_age_ticks(seed),
                        lifespan: Some(demo.lifespan_ticks(seed)),
                        seed,
                        estate_destination: None,
                        acquired_tool: false,
                        foraging: false,
                        cultivating: false,
                        cultivate_pressure: 0,
                        cultivation_stock_pending: false,
                        cultivation_skill: 0,
                        cultivation_return_window: VecDeque::new(),
                        cultivation_tenure: 0,
                        commitment_remaining: 0,
                        commitment_renewals: 0,
                        adopts_commitment_norm: false,
                        next_norm_bit: None,
                        commitment_norm_seed_adopter: false,
                        commitment_norm_observations: VecDeque::new(),
                        carried_grain_source: None,
                        carried_share_contract_id: None,
                        carried_in_kind_contract_id: None,
                    });
                }
            }
        }

        // The promotion rejection list (see the `money_rejection_goods` field doc):
        // every renewable source the settlement runs, so econ's `winner` rule can
        // never commit a good the substrate keeps minting. The G5a slice had only the
        // spatial nodes; the G5b frontier adds the chain's recipe outputs and the
        // demography hearth, so the list finally bites and the durable medium (e.g.
        // SALT) is the only candidate left that the camp can monetize.
        let mut money_rejection_goods: Vec<GoodId> = Vec::new();
        let reject = |good: GoodId, list: &mut Vec<GoodId>| {
            if good != GOLD && !list.contains(&good) {
                list.push(good);
            }
        };
        // The spatial resource nodes (the world regenerates them).
        for spec in &config.nodes {
            reject(spec.good, &mut money_rejection_goods);
        }
        // The production-chain recipe outputs (a producer keeps minting them). Tools
        // are durable capital, never an emergent-money candidate, but rejecting them
        // too is harmless and keeps the list "no chain good can be money".
        if let Some(chain) = &config.chain {
            for good in chain.content.goods() {
                reject(good, &mut money_rejection_goods);
            }
        }
        // The demography household hearth (the renewable provision): the hunger staple
        // and WOOD. Empty without a demography overlay.
        if config.demography.is_some() {
            reject(known.hunger, &mut money_rejection_goods);
            reject(WOOD, &mut money_rejection_goods);
        }
        money_rejection_goods.sort();

        // The goods tracked for conservation: node goods plus anything a colonist
        // or resident trader starts holding (FOOD via nodes/buffers, WOOD via
        // endowments). Money is not a physical good, so it is excluded.
        let mut goods: Vec<GoodId> = Vec::new();
        let push_good = |g: GoodId, goods: &mut Vec<GoodId>| {
            if g != GOLD && !goods.contains(&g) {
                goods.push(g);
            }
        };
        // A demography settlement trades the hunger staple and WOOD (warmth) even if
        // a household starts a buffer at zero, and the per-member provision mints both
        // into econ stock — so both join the conservation ledger up front. The staple
        // is FOOD on a plain `lineages` colony and bread on the G5b frontier; both are
        // tracked here through [`KnownGoods::hunger`].
        if config.demography.is_some() {
            push_good(known.hunger, &mut goods);
            push_good(WOOD, &mut goods);
        }
        for spec in &config.nodes {
            push_good(spec.good, &mut goods);
        }
        for agent in &agents {
            for g in agent.stock.positive_goods() {
                push_good(g, &mut goods);
            }
        }
        // Every chain good is tracked even if no agent is seeded holding it yet
        // (flour, for instance, only appears once a miller produces it): the
        // production phase mints it into econ stock, and the conservation report
        // and the canonical digest must already account it.
        if let Some(chain) = &config.chain {
            for g in chain.content.goods() {
                push_good(g, &mut goods);
            }
        }
        goods.sort();

        let recipes = config
            .chain
            .as_ref()
            .map(|chain| chain.content.recipes().to_vec())
            .unwrap_or_default();
        // The market regime. A plain/chain/demography settlement runs the
        // designated-GOLD M1 spot market (`Camp`'s natural seam: the
        // consumption-log readback and the realized-price accessor live on this
        // path). The G5a barter camp instead runs econ's V2 emergence machinery
        // (`MengerSaltMoney` → `ScenarioKind::MarketV2` + `Emergent`): `step_v2`
        // clears barter and feeds the SaleabilityTracker until the reused
        // Mengerian `winner` rule promotes a money good, after which the same
        // V2 money phase clears the money-priced market. Both log consumption
        // (the additive V2 logging G5a wired into econ) and realize prices.
        //
        // G8a adds the M3 ledger-money settlement: `EmergedGoldSoundControl` is the
        // pure-specie M3 scenario (`ScenarioKind::MarketM3`, SoundGold regime, no banks,
        // no issuers, no project lines, default specie tenders), so the society builds a
        // `MoneySystem` whose only active machinery is the ledger-settled spot market —
        // economically the same designated-GOLD market as M1, only ledger-accounted. The
        // money good is still GOLD (the specie). M3 is mutually exclusive with the barter
        // overlay (which runs the V2 emergent-money path).
        assert!(
            !(config.m3 && config.barter.is_some()),
            "an M3 ledger settlement is mutually exclusive with the barter (V2 emergent-money) overlay"
        );
        // G8b: a chartered bank takes deposits and lends fiduciary on the M3 ledger,
        // so it requires the M3 `MoneySystem` (there is no bank without ledger money).
        assert!(
            config.bank.is_none() || config.m3,
            "a chartered bank (G8b) requires the M3 ledger settlement (m3 = true)"
        );
        // The demography guard is layered intentionally BEFORE the layout-equality
        // guard below. `SettlementConfig::m3_settlement()` already has `demography:
        // None`, so the stricter layout check would reject a banked+demography config
        // regardless — but this earlier assert fires first to emit the *specific*
        // "cannot run with demography" message (the `bank_rejects_demography_until_
        // claim_estates_exist` test pins that wording). Keep both: this one names the
        // demography cause precisely (old-age/heir settlement of claims is unhandled —
        // the deposit-withdrawal-on-death below only covers the starvation path); the
        // layout check below scopes G8b to its two shipped bank controls.
        assert!(
            config.bank.is_none() || config.demography.is_none(),
            "a chartered bank (G8b) cannot run with demography until demand-claim \
             estate routing exists"
        );
        if let Some(bank_cfg) = config.bank {
            let mut bank_free_config = config.clone();
            bank_free_config.bank = None;
            assert!(
                bank_free_config == SettlementConfig::m3_settlement(),
                "a chartered bank (G8b) is limited to the curated M3 settlement layout \
                 (the shipped bank/full-reserve controls) until G8c finance"
            );
            assert!(
                is_supported_g8b_bank_charter(bank_cfg),
                "a chartered bank (G8b) is limited to the shipped bank/full-reserve \
                 charters until G8c finance"
            );
        }
        let (scenario_name, money) = match (&config.barter, config.m3) {
            (Some(barter), _) => (
                ScenarioName::MengerSaltMoney,
                MarketMoneyConfig::Emergent(barter.menger.clone()),
            ),
            (None, true) => (
                ScenarioName::EmergedGoldSoundControl,
                MarketMoneyConfig::Designated(DesignatedMoney { good: GOLD }),
            ),
            (None, false) => (
                ScenarioName::MarketBarterishGold,
                MarketMoneyConfig::Designated(DesignatedMoney { good: GOLD }),
            ),
        };
        let scenario = MarketScenario {
            name: "settlement",
            scenario: scenario_name,
            seed,
            periods: 0,
            agents,
            recipes,
            events: Vec::new(),
            money,
        };
        let mut society = Society::from_scenario(scenario);
        society.enable_consumption_log();
        // impl-76 / C3R.k: hand the satiated-surplus ask lever to the society, so its two
        // market-steering ask sites read the same per-tick value the settlement's appraisal does.
        society.set_satiated_surplus_ask(config.chain.as_ref().and_then(|chain| {
            chain
                .satiated_surplus_ask_at
                .map(|at| (at, chain.satiated_surplus_ask_scope))
        }));

        // G8b: charter the bank. The bank is a *settlement* entity (config-chartered
        // here; the player-`Command` charter is G8c/UI), so the sim adds it after the
        // econ society is built rather than through a new econ scenario — the spot
        // market stays byte-identical to G8a. Two game-only wirings, both reusing the
        // existing M3 machinery unchanged:
        //
        // 1. the regime is moved to `FractionalConvertible` (econ's existing command
        //    surface, `apply_command(SetRegime)`) so the bank may issue fiduciary
        //    against fractional reserves — this is the bank's fixed operating regime,
        //    not the G8c regime *ladder* (which transitions regimes over time to drive
        //    the boom/bust cycle); and
        // 2. one econ `Bank` is pushed into `society.banks` with zero reserves — the
        //    deposit phase builds them. The ledger's `bank_reserves` is likewise zero
        //    at construction, so `sum(bank.reserves) == bank_reserves` holds and the
        //    money invariant reconciles from tick zero.
        //
        // The deposit/lend amounts run through the existing M3 ledger / bank paths in
        // `run_bank_phase`; no bank logic is added to econ. A `full-reserve` charter
        // is the falsification twin — its `fiduciary_lend_capacity` is zero, so the
        // same phase lends nothing.
        if let Some(bank_cfg) = config.bank {
            // `SetRegime` always applies (it only sets the field); the M3 society is
            // built `SoundGold`, which forbids fiduciary, so this is the one charter-
            // time move that lets a fractional bank lend at all.
            let result = society.apply_command(EventKind::SetRegime(Regime::FractionalConvertible));
            assert!(
                result.is_applied(),
                "setting the G8b bank operating regime must apply"
            );
            society.banks.push(Bank {
                id: BANK_ID,
                name: bank_cfg.name,
                reserves: Gold::ZERO,
                demand_deposits: Gold::ZERO,
                time_deposits: Gold::ZERO,
                loans_outstanding: Gold::ZERO,
                fiduciary_issued: Gold::ZERO,
                reserve_ratio_bps: bank_cfg.reserve_ratio_bps,
                convertible: true,
                policy: BankPolicy {
                    // Generous per-tick cap: the binding limit on lending is the
                    // reserve ratio (via `convertible_deposit_capacity`), not this.
                    max_new_fiduciary_per_tick: Gold(1_000_000_000),
                    // The one-unit loan policy must be nonzero for
                    // `fiduciary_lend_capacity` to be positive at all; the actual
                    // amount is gated by the reserve ratio.
                    loan_present: Gold(1),
                    loan_horizon: 7,
                    loan_future_due: Gold(1),
                    enabled: true,
                },
            });
        }

        // G8a resolves the G4b deferral: M3 (ledger-money) demography now settles. A
        // funded M3 colonist's death drains its ledger specie into the estate via
        // `Society::remove_agent` (`can_remove_agent` no longer refuses a funded specie
        // balance), the heir credit re-credits that specie through the ledger, and a
        // birth endowment is a conserved within-ledger `transfer_gold`. So demography
        // runs on either money regime; the G4b pre-G8a assert that forbade M3 demography
        // is retired (banks/fiat — not specie — remain G8b/c, and a fiat/claims balance
        // is still refused upstream).

        // Build the production-chain runtime and register the content good names
        // so the society's registry resolves them (the viewer reads names through
        // `Society::good_name`). The ids the society interns must equal those the
        // `ContentSet` assigned — both intern over the same lab catalog in the
        // same order — which the assert pins loudly.
        let chain = config.chain.as_ref().map(|chain| {
            for (name, id) in chain.content.good_entries() {
                let interned = society.intern_good(name);
                assert_eq!(
                    interned, id,
                    "content good {name:?} interned to {interned:?} in the society, \
                     not the ContentSet id {id:?}"
                );
            }
            // S6 hysteresis invariant: for an active re-entry phase, the exit
            // threshold must sit strictly below the entry threshold. Otherwise a
            // re-entrant can satisfy both sides of the band and churn between grain
            // and its home role.
            assert!(
                !chain.productive_reentry
                    || !chain.subsistence_on_grain
                    || chain.reentry_hunger_out < chain.reentry_hunger_in,
                "re-entry hysteresis requires reentry_hunger_out < reentry_hunger_in"
            );
            // S12 hysteresis invariant: when the own-labor forage path can run, the
            // exit threshold must sit strictly below the entry threshold. Otherwise
            // a forager can satisfy both sides of the band and stay in a degenerate
            // always-forage state.
            assert!(
                !chain.own_labor_subsistence
                    || chain.content.forage().is_none()
                    || chain.forage_hunger_out < chain.forage_hunger_in,
                "own-labor subsistence hysteresis requires forage_hunger_out < forage_hunger_in"
            );
            if chain_config_own_use_cultivation_active(chain) {
                let cultivate = chain
                    .content
                    .cultivate_recipe()
                    .expect("active cultivation carries a Cultivate recipe");
                assert!(
                    chain.content.bread() == known.hunger,
                    "own-use cultivation requires cultivated bread to be the hunger good"
                );
                assert!(
                    cultivate
                        .input_good
                        .is_some_and(|(good, _)| good == chain.content.grain())
                        && cultivate.output_good == chain.content.bread(),
                    "own-use cultivation requires Cultivate to convert grain into bread"
                );
                assert!(
                    config
                        .nodes
                        .iter()
                        .any(|spec| spec.good == chain.content.grain()),
                    "own-use cultivation requires a grain resource node"
                );
                assert!(
                    chain.cultivate_hunger_out < chain.cultivate_hunger_in,
                    "own-use cultivation requires cultivate_hunger_out < cultivate_hunger_in"
                );
                // S21f: the forage→cultivation tier ordering applies only on the
                // own-labor/forage substrate, where cultivation is the SECOND tier above
                // foraging. On the household-barter path there is no forage tier beneath
                // cultivation (it fires directly from sustained hunger), so the forage knob
                // is a no-op and this ordering is not required.
                if own_labor_subsistence_fields_active(
                    chain.own_labor_subsistence,
                    chain.content.forage().is_some(),
                ) {
                    assert!(
                        chain.forage_hunger_in < chain.cultivate_hunger_in,
                        "own-use cultivation requires forage_hunger_in < cultivate_hunger_in"
                    );
                }
                // The sustained-scarcity gate has to span at least one tick. With a
                // patience of 0 the streak threshold `pressure >= cultivate_patience` is
                // satisfied even when the pressure streak is 0 (hunger below
                // `cultivate_hunger_in`), so every eligible colonist would escalate to
                // cultivation regardless of hunger — bypassing the scarcity gate entirely.
                assert!(
                    chain.cultivate_patience > 0,
                    "own-use cultivation requires cultivate_patience > 0 (a 0 patience \
                     escalates eligible colonists to cultivation regardless of hunger)"
                );
                // The escape valve only relieves hunger if the cultivated bread is
                // actually eaten through the readback seam. With a draw of 0 cultivators
                // produce bread but never consume it, so hunger never falls and bread
                // silently hoards — the gate that would otherwise catch the misconfig.
                assert!(
                    chain.cultivate_consume > 0,
                    "own-use cultivation requires cultivate_consume > 0 (a 0 draw never \
                     eats the cultivated bread, so the escape valve never relieves hunger)"
                );
                if let Some(demo) = &config.demography {
                    assert!(
                        chain.cultivate_hunger_in < demo.birth_hunger_ceiling,
                        "own-use cultivation requires cultivate_hunger_in < birth_hunger_ceiling"
                    );
                }
            }
            // S21h.1: the emergency self-provisioning threshold ordering (mirror the
            // cultivation ordering checks above). When the gated phase is on it must fire
            // (1) ABOVE the lineage `cultivate_hunger_in` trigger (so the non-lineage
            // emergency floor is distinct from — and slower than — the lineage's cultivation
            // hysteresis, and never pre-empts it), and (2) STRICTLY BELOW `hunger_critical`
            // (so the floor fires within the alive-but-lethal-pressure window, before the
            // hunger clamp, rather than only once a death streak has begun). Off (every
            // existing config, threshold 0) both checks are skipped and the run is unchanged.
            if chain.emergency_hunger_threshold > 0 {
                // The emergency floor produces and eats the chain's BREAD to relieve hunger,
                // but the need readback only treats `known.hunger` (or `known.subsistence`)
                // as food — so a chain whose bread is NOT the hunger staple would make the
                // floor produce + consume a good that never lowers hunger, and the seam would
                // silently fail (the role still starves). Require the produced bread to be the
                // hunger good (mirror the own-use cultivation check above).
                assert!(
                    chain.content.bread() == known.hunger,
                    "emergency self-provisioning requires the produced bread to be the hunger \
                     good (the needs readback only relieves hunger from known.hunger / \
                     known.subsistence, so a non-staple emergency bread floor would never \
                     lower hunger and the seam would silently fail)"
                );
                assert!(
                    chain.emergency_hunger_threshold < config.dynamics.hunger_critical,
                    "emergency self-provisioning requires emergency_hunger_threshold < \
                     hunger_critical (else the floor only fires once the role is already \
                     critical / past the hunger clamp)"
                );
                if chain_config_own_use_cultivation_active(chain) {
                    assert!(
                        chain.cultivate_hunger_in < chain.emergency_hunger_threshold,
                        "emergency self-provisioning requires cultivate_hunger_in < \
                         emergency_hunger_threshold (the non-lineage emergency floor is \
                         distinct from and slower than the lineage cultivation trigger)"
                    );
                }
            }
            if chain_config_rival_subsistence_commons_active(chain) {
                assert!(
                    chain.rival_subsistence_commons_phi_bps > 0,
                    "rival subsistence commons requires a positive phi_bps"
                );
                assert!(
                    chain.emergency_hunger_threshold > 0,
                    "rival subsistence commons composes on the S21h emergency residual set"
                );
                assert!(
                    chain.content.bread() == known.hunger,
                    "rival subsistence commons requires bread to be the hunger staple"
                );
            }
            debug_assert!(
                !(chain_config_wage_labor_active(chain)
                    && chain_config_share_tenancy_active(chain)),
                "wage_labor and share_tenancy are not composed in C1R"
            );
            // S7.2 prerequisite: a built tool is useless unless holding it makes the
            // builder eligible to adopt (S7.1), so producible capital requires the
            // tool-acquisition gate. The capital scenario composes both; this guards a
            // misconfiguration that would build tools no colonist could ever use.
            assert!(
                !chain.producible_capital || chain.tool_acquisition_eligibility,
                "producible capital (S7.2) requires tool-acquisition eligibility (S7.1)"
            );
            // S10: the per-agent intertemporal decision steers the SAME per-builder build
            // substrate, so it requires the producible-capital phase to be on (the
            // decision is meaningless without a build to start).
            assert!(
                !chain.per_agent_capital || chain.producible_capital,
                "per-agent capital (S10) requires producible capital (S7.2)"
            );
            assert!(
                !chain.per_agent_capital
                    || u64::from(chain.tool_build_labor) < max_savings_ladder_horizon(),
                "per-agent capital requires tool_build_labor below the deepest savings horizon"
            );
            if chain_config_private_land_tenure_active(chain) {
                assert!(
                    chain.land_idle_limit > 0,
                    "private land tenure requires a positive land_idle_limit"
                );
                assert!(
                    chain.land_good_plots > 0 || chain.land_marginal_plots > 0,
                    "private land tenure requires at least one grain plot"
                );
                assert!(
                    private_land_layout_width(chain.land_good_plots, chain.land_marginal_plots)
                        .is_some(),
                    "private land tenure layout must fit in the generated 1-D strip"
                );
            }
            ChainRuntime {
                content: chain.content.clone(),
                throughput: chain.throughput,
                seeded_surplus_bread: chain.seeded_surplus_bread,
                operating_cost: chain.operating_cost,
                tier2_threshold: chain.tier2_threshold,
                tier2_recipe_id: chain.content.tier2_recipe_id(),
                scholar_grain_buffer: chain.scholar_grain_buffer,
                confectioner_flour_buffer: chain.confectioner_flour_buffer,
                capital_advance: chain.capital_advance,
                perishable_decay_bps: chain.perishable_decay_bps,
                subsistence_advance: chain.subsistence_advance,
                input_advance: chain.input_advance,
                recurring_motive: chain.recurring_motive,
                project_input_bids: chain.project_input_bids,
                producer_subsistence: chain.producer_subsistence,
                own_labor_subsistence: chain.own_labor_subsistence,
                forage_yield: chain.forage_yield,
                forage_hunger_in: chain.forage_hunger_in,
                forage_hunger_out: chain.forage_hunger_out,
                forage_commons: chain.forage_commons,
                own_use_cultivation: chain.own_use_cultivation,
                cultivate_hunger_in: chain.cultivate_hunger_in,
                cultivate_hunger_out: chain.cultivate_hunger_out,
                cultivate_consume: chain.cultivate_consume,
                cultivate_patience: chain.cultivate_patience,
                cultivation_sells_surplus: chain.cultivation_sells_surplus,
                multigood_money: chain.multigood_money,
                household_barter_cultivation: chain.household_barter_cultivation,
                endogenous_cultivation_entry: chain.endogenous_cultivation_entry,
                cultivation_skill: chain.cultivation_skill,
                profit_driven_retention: chain.profit_driven_retention,
                return_window: chain.return_window,
                retention_margin_bps: chain.retention_margin_bps,
                retention_material_floor: chain.retention_material_floor,
                skill_gain: chain.skill_gain,
                skill_decay: chain.skill_decay,
                skill_cap: chain.skill_cap,
                skill_haul_ceiling: chain.skill_haul_ceiling,
                durable_cultivation_tool: chain.durable_cultivation_tool,
                tool_build_patience: chain.tool_build_patience,
                cultivation_tool_haul_ceiling: chain.cultivation_tool_haul_ceiling,
                cultivation_tool_non_durable: chain.cultivation_tool_non_durable,
                endowed_cultivation_capital: chain.endowed_cultivation_capital,
                endowed_tool_count: chain.endowed_tool_count,
                cultivation_tool_inheritance: chain.cultivation_tool_inheritance,
                voluntary_cultivation_commitment: chain.voluntary_cultivation_commitment,
                commitment_term: chain.commitment_term,
                commitment_entry_floor: chain.commitment_entry_floor,
                commitment_fiat_pin: chain.commitment_fiat_pin,
                commitment_norm_spread: chain.commitment_norm_spread,
                abandonable_norm: chain.abandonable_norm,
                group_payoff_imitation: chain.group_payoff_imitation,
                fixed_commitment_norm_prevalence: chain.fixed_commitment_norm_prevalence,
                commitment_seed_share_bps: chain.commitment_seed_share_bps,
                imitation_period: chain.imitation_period,
                imitation_window: chain.imitation_window,
                imitation_margin_bps: chain.imitation_margin_bps,
                imitation_radius: chain.imitation_radius,
                imitation_max_models: chain.imitation_max_models,
                food_window_target: chain.food_window_target,
                no_imitation: chain.no_imitation,
                random_imitation: chain.random_imitation,
                salt_in_score: chain.salt_in_score,
                private_land_tenure: chain.private_land_tenure,
                land_idle_limit: chain.land_idle_limit,
                harvest_gate: chain.harvest_gate,
                forfeit_on_idle: chain.forfeit_on_idle,
                reclaim_reserved_for_prior_owner: chain.reclaim_reserved_for_prior_owner,
                land_good_plots: chain.land_good_plots,
                land_marginal_plots: chain.land_marginal_plots,
                land_marginal_regen: chain.land_marginal_regen,
                secure_land_tenure: chain.secure_land_tenure,
                inheritance_regime: chain.inheritance_regime,
                land_market: chain.land_market,
                mortal_landowner_demography: chain.mortal_landowner_demography,
                rival_subsistence_commons: chain.rival_subsistence_commons,
                rival_subsistence_commons_phi_bps: chain.rival_subsistence_commons_phi_bps,
                wage_labor: chain.wage_labor,
                wage_labor_mode: chain.wage_labor_mode,
                share_tenancy: chain.share_tenancy,
                share_tenancy_mode: chain.share_tenancy_mode,
                share_forward_provisioning: chain.share_forward_provisioning,
                share_contract_succession: chain.share_contract_succession,
                in_kind_wage: chain.in_kind_wage,
                mortal_chain_producers: chain.mortal_chain_producers,
                mortal_producer_inheritance: chain.mortal_producer_inheritance,
                mortal_producer_tool_inheritance: chain.mortal_producer_tool_inheritance,
                producer_house_cap: chain.producer_house_cap,
                earned_provisioning: chain.earned_provisioning,
                producer_stock_provisioning_control: chain.producer_stock_provisioning_control,
                birth_stock_saving: chain.birth_stock_saving,
                birth_stock_saving_mode: chain.birth_stock_saving_mode,
                saving_allocation_obs: chain.saving_allocation_obs,
                birth_gate_obs: chain.birth_gate_obs,
                share_bps: chain.share_bps,
                share_term: chain.share_term,
                land_carrying_cost: chain.land_carrying_cost,
                land_price_cap_factor: chain.land_price_cap_factor,
                gatherer_food_cushion: chain.gatherer_food_cushion,
                emergency_hunger_threshold: chain.emergency_hunger_threshold,
                birth_stock_ignition_at: chain.birth_stock_ignition_at,
                producer_house_starting_staple: chain.producer_house_starting_staple,
                producer_support_until_tick: chain.producer_support_until_tick,
                retire_food_mints: chain.retire_food_mints,
                acquisition_ledger: chain.acquisition_ledger,
                productive_reentry: chain.productive_reentry,
                reentry_hunger_in: chain.reentry_hunger_in,
                reentry_hunger_out: chain.reentry_hunger_out,
                tool_acquisition_eligibility: chain.tool_acquisition_eligibility,
                producible_capital: chain.producible_capital,
                per_agent_capital: chain.per_agent_capital,
                entrepreneurial_forecasts: chain.entrepreneurial_forecasts,
                stale_input_price_fix: chain.stale_input_price_fix,
                capital_payback_cycles: chain.capital_payback_cycles,
                tool_build_wood: chain.tool_build_wood,
                tool_build_labor: chain.tool_build_labor,
                capital_build_hunger_max: chain.capital_build_hunger_max,
                // impl-76 / C3R.k: carry the satiated-surplus ask lever into the runtime chain.
                satiated_surplus_ask_at: chain.satiated_surplus_ask_at,
                satiated_surplus_ask_scope: chain.satiated_surplus_ask_scope,
            }
        });

        let live_colonist_slots: Vec<usize> = (0..colonists.len()).collect();
        let colonist_slot_by_id: BTreeMap<AgentId, usize> = colonists
            .iter()
            .enumerate()
            .map(|(slot, colonist)| (colonist.id, slot))
            .collect();

        let mut settlement = Self {
            generation_seed: seed,
            #[cfg(test)]
            test_fault_mint_birth_gold: 0,
            world,
            society,
            colonists,
            live_colonist_slots,
            colonist_slot_by_id,
            dynamics,
            known,
            exchange,
            forage_node_id,
            carry_cap: config.carry_cap,
            move_speed: config.move_speed,
            goods,
            money_rejection_goods,
            pending_deposits: BTreeMap::new(),
            trader_ids,
            chain,
            capital_loans: BTreeMap::new(),
            capital_builds: Vec::new(),
            next_capital_project_id: 0,
            tools_built: 0,
            mortal_producer_old_age_deaths: 0,
            role_readoptions: 0,
            mortal_capital_builds: 0,
            producer_tool_inheritances: 0,
            heirless_producer_deaths: 0,
            heir_tool_adoptions: 0,
            producer_house_hearth_food_minted: 0,
            non_producer_hearth_food_minted: 0,
            producer_house_births: 0,
            producer_house_deaths: 0,
            producer_house_person_ticks: 0,
            producer_recipe_pay_rejections: 0,
            producer_build_rejections: 0,
            producer_adoption_rejections: 0,
            producer_tool_inheritors: BTreeSet::new(),
            last_capital_decisions: Vec::new(),
            peak_pre_promotion_hunger: 0,
            critical_ticks_pre_promotion: 0,
            bread_provenance: BreadProvenance::default(),
            multigood: MultigoodMoney::default(),
            acquisition: AcquisitionLedger::default(),
            earned_provisioning: EarnedProvisioningLedger::default(),
            birth_stock_wants_emitted: 0,
            birth_stock_attributable_purchases: 0,
            birth_stock_below_target_agents: BTreeSet::new(),
            birth_stock_reached_agents: BTreeSet::new(),
            birth_stock_held_max: 0,
            birth_stock_held_at_death: 0,
            birth_stock_eligible_opportunities: 0,
            birth_stock_injections_completed: 0,
            birth_stock_source_shortfalls: 0,
            ignition_injected_qty: 0,
            ignition_gate_blocked_interval: 0,
            ignition_gate_extinct: 0,
            ignition_gate_blocked_cap: 0,
            ignition_gate_blocked_hunger: 0,
            ignition_gate_suppressed_at_target: 0,
            ignition_gate_donor_shortfall: 0,
            producer_birth_funded_by_channel: [0; FoodChannel::COUNT],
            producer_birth_funded_intervention: 0,
            birth_stock_injection_records: Vec::new(),
            birth_stock_births_by_household: vec![0; households.len()],
            last_birth_stock_attribution_snapshot: BTreeSet::new(),
            saving_allocation_obs: SavingAllocationObs::default(),
            birth_gate_obs: birth_gate::BirthGateObs::default(),
            saving_obs_stock_tick: None,
            saving_obs_pending_offerable: None,
            bootstrap_trace: BootstrapTrace::default(),
            flour_census: None,
            flour_census_armed: false,
            bread_seller_trace: Vec::new(),
            seeded_surplus_trace: SeededSurplusTrace::default(),
            seeded_minted_bread_sold_for_salt: 0,
            emergency_bread_provisioned: 0,
            subsistence_commons_stock: 0,
            subsistence_commons_cap: 0,
            subsistence_commons_regen: 0,
            subsistence_commons_phi_bps: 0,
            subsistence_commons_drawn_total: 0,
            subsistence_commons_unmet_total: 0,
            subsistence_commons_depleted_ticks: 0,
            subsistence_commons_shortfall_ticks: 0,
            subsistence_commons_eligible_need_total: 0,
            wage_escrow_gold: Gold::ZERO,
            wage_escrows: Vec::new(),
            next_wage_contract_id: 0,
            wage_retained_earnings: BTreeMap::new(),
            wage_proceeds_buckets: BTreeMap::new(),
            wage_workers_ever: BTreeSet::new(),
            wage_employers_ever: BTreeSet::new(),
            wage_hires_total: 0,
            wage_hires_post_promotion: 0,
            wage_below_ask_not_hired: 0,
            wage_endowment_funded_wages: Gold::ZERO,
            wage_financed_output_buys: Gold::ZERO,
            wage_nonowner_output_buys: Gold::ZERO,
            wage_circular_loop_turnovers: 0,
            share_contracts: Vec::new(),
            next_share_contract_id: 0,
            share_workers_ever: BTreeSet::new(),
            share_owners_ever: BTreeSet::new(),
            share_contracts_total: 0,
            share_voluntary_contracts_total: 0,
            share_forced_contracts_total: 0,
            share_renewals_total: 0,
            share_worker_bread_income: 0,
            share_owner_bread_income: 0,
            share_worker_declined: 0,
            share_worker_unmatched: 0,
            share_forward_only_eligibility: 0,
            share_renewal_hints_total: 0,
            share_renewal_fed_out: 0,
            share_renewal_base_ineligible: 0,
            share_renewal_owner_not_candidate: 0,
            share_renewal_bread_declined: 0,
            share_renewal_matched_elsewhere: 0,
            share_owner_candidates_total: 0,
            share_owner_no_atcap_plot: 0,
            share_stock_opportunity_refusal: 0,
            share_reservation_collision: 0,
            share_stock_drawdown: 0,
            share_unattributed_share_deposit: 0,
            share_owner_grain_settled: 0,
            share_successions_total: 0,
            share_succession_heir_declined: 0,
            share_succession_worker_re_declined: 0,
            share_post_succession_renewals: 0,
            share_succeeded_live_ids: BTreeSet::new(),
            in_kind_contracts: Vec::new(),
            next_in_kind_contract_id: 0,
            in_kind_workers_ever: BTreeSet::new(),
            in_kind_employers_ever: BTreeSet::new(),
            in_kind_hires_total: 0,
            in_kind_worker_advance_bread: 0,
            in_kind_employer_bread_income: 0,
            in_kind_expected_output_total: 0,
            in_kind_worker_declined: 0,
            in_kind_worker_unmatched: 0,
            in_kind_owner_candidates_total: 0,
            in_kind_owner_no_atcap_plot: 0,
            in_kind_owner_insufficient_fund: 0,
            in_kind_productivity_declined: 0,
            in_kind_reservation_collision: 0,
            in_kind_stock_drawdown: 0,
            in_kind_unattributed_deposit: 0,
            in_kind_employer_grain_settled: 0,
            in_kind_endowment_funded_hires: 0,
            in_kind_term_starvations: 0,
            ever_landowner_ids: BTreeSet::new(),
            owner_first_claim_tick: BTreeMap::new(),
            owner_age_at_first_claim: BTreeMap::new(),
            owner_tenure_before_death: Vec::new(),
            owner_bread_consumed: BTreeMap::new(),
            owner_surplus_sold_before_death: BTreeMap::new(),
            owner_inventory_at_death: Vec::new(),
            inherited_stock_to_heirs: 0,
            buyer_purchases_by_owner_age_cohort: BTreeMap::new(),
            owner_seller_attributed_bought: 0,
            cultivation_skill_producers: BTreeSet::new(),
            cultivation_grain_harvested: BTreeMap::new(),
            cultivation_bread_produced: BTreeMap::new(),
            cultivation_proceeds_scratch: BTreeMap::new(),
            profit_retained_ids: BTreeSet::new(),
            profit_retained_ever: BTreeSet::new(),
            commitment_committed_ever: BTreeSet::new(),
            commitment_uptake: BTreeMap::new(),
            commitment_fiat_ever: BTreeSet::new(),
            commitment_below_floor_ever: BTreeSet::new(),
            commitment_exit_override_ids: BTreeSet::new(),
            commitment_exit_override_ever: BTreeSet::new(),
            commitment_norm_copy_events: Vec::new(),
            commitment_norm_flip_events: Vec::new(),
            commitment_norm_adoptions: 0,
            commitment_norm_abandonments: 0,
            commitment_norm_imitation_adopters: BTreeSet::new(),
            commitment_norm_group_covariance_sum: 0,
            commitment_norm_group_covariance_count: 0,
            cultivation_tool_builds: Vec::new(),
            next_cultivation_tool_project_id: 0,
            cultivation_tool_producers: BTreeSet::new(),
            cultivation_tools_built: 0,
            cultivation_tool_wood_consumed: 0,
            cultivation_tools_destroyed: 0,
            endowed_cultivation_tools_total: 0,
            endowed_households: Vec::new(),
            endowed_member_ids: Vec::new(),
            cultivation_tool_inherited_total: 0,
            cultivation_tool_inheritor_ids: BTreeSet::new(),
            land_plots: BTreeMap::new(),
            secure_land_inheritance_events: Vec::new(),
            secure_land_owner_old_age_deaths_total: 0,
            secure_land_inherit_eligible_owner_deaths_total: 0,
            secure_land_stranded_shares_total: 0,
            land_claims_total: 0,
            land_idle_losses_total: 0,
            land_harvest_denials_total: 0,
            land_owner_gate_denials_total: 0,
            land_nonowner_harvest_of_owned_total: 0,
            land_reclaims_by_other_total: 0,
            land_marginal_nonowner_claims_total: 0,
            land_lapsed_reentry_worse_total: 0,
            land_plot_harvest_totals: BTreeMap::new(),
            land_lapsed_losses: BTreeMap::new(),
            land_lost_prior_owners: BTreeMap::new(),
            land_market_plots: BTreeMap::new(),
            land_market_yield_this_tick: BTreeMap::new(),
            land_market_sales: Vec::new(),
            land_market_trade_count: 0,
            land_market_pre_promotion_trade_count: 0,
            land_market_carrying_paid_total: 0,
            land_market_pre_promotion_charges: 0,
            land_market_foreclosure_listings_total: 0,
            land_market_priced_out_total: 0,
            land_market_lapsed_priced_out_total: 0,
            land_market_ask_bid_gap_sum: 0,
            land_market_ask_bid_gap_count: 0,
            land_market_title_history: BTreeMap::new(),
            land_fee_pool_salt: Gold::ZERO,
            econ_tick: 0,
            last_report: EconTickReport::default(),
            commons_gold: Gold::ZERO,
            commons_stock: BTreeMap::new(),
            demography: config.demography.clone(),
            households,
            birth_seq: 0,
            births_total: 0,
            old_age_deaths_total: 0,
            starvation_deaths_total: 0,
            birth_block_interval: 0,
            birth_block_size_cap: 0,
            birth_block_hunger_ceiling: 0,
            birth_block_endowment: 0,
            barter: config.barter.clone(),
            // The medium-demand scale extension runs only when a medium is
            // actually supplied (the camp). The control endows none, so its
            // colonists carry no medium want — they barter FOOD-for-WOOD only, the
            // symmetric trade structure that cannot monetize. This is what makes
            // the pair a clean falsification twin: the medium (its demand AND its
            // supply) is the only difference.
            barter_medium: config.barter.as_ref().and_then(|barter| {
                let supplied = barter.gatherer_medium_endowment > 0
                    || barter.consumer_medium_endowment > 0
                    || barter.cycle_producer_medium_endowment > 0;
                supplied.then_some((barter.medium_good, barter.medium_want_qty))
            }),
            // S9: the heterogeneous real direct use of the medium (SALT). Active only
            // when both the consumption quantity and the heterogeneity period are set
            // (default off — `None` — for every pre-S9 scenario).
            salt_direct_use: config.barter.as_ref().and_then(|barter| {
                (barter.salt_direct_use_qty > 0 && barter.salt_direct_use_period > 0).then_some((
                    barter.medium_good,
                    barter.salt_direct_use_qty,
                    barter.salt_direct_use_period,
                ))
            }),
            // G6b: Knowledge starts at zero and tier 2 starts locked. A non-research
            // settlement never touches either (no scholar runs, the threshold is 0),
            // so its digest is byte-identical.
            knowledge: 0,
            tier2_unlocked_at: None,
            // G8b: the chartered-bank config (or `None`). A detached copy — the bank
            // entity itself lives in `society.banks`; this drives `run_bank_phase`.
            bank: config.bank,
            // G8c-1/G8c-2: a spatial settlement runs no credit cycle and no tender
            // bench (the finance path returns early from `generate`), so these are
            // always `None` here.
            cycle: None,
            shadow_cycle_cache: RefCell::new(None),
            bench: None,
            // G8c-3: a spatial settlement levies no tax (the finance path returns early
            // from `generate`), so the tax overlay is always absent here.
            tax: None,
            // DH.a (impl-68): copy the closed-circulation marker so the runtime can gate the
            // whole-population provenance ledger + closure observation on it. Default-false, so a
            // non-closed settlement is byte-identical.
            closed_circulation: config.closed_circulation,
            closure: closure::ClosureLedger::default(),
            burden: burden::BurdenTelemetry::default(),
        };
        // S22e: endow a minority of lineage households with a plow at generation (a
        // conservation-safe INITIAL endowment, no earning required). A no-op off the gate, so
        // every existing config is byte-identical. Runs after construction so the colonist roster
        // + econ agents exist; the granted plows land in agent stock before the first `econ_tick`,
        // so they are part of the tick-0 whole-system baseline and conservation holds every tick.
        settlement.init_rival_subsistence_commons();
        settlement.apply_endowed_cultivation_capital(seed, config);
        settlement.init_private_land_tenure();
        settlement.init_commitment_norm_seed(seed);
        settlement.init_earned_provisioning_buckets();
        settlement.init_birth_stock_reach_baseline();
        // C3R.e-obs (impl-66): enable the econ allocation trace ONCE, iff the obs flag is
        // on (and thus the motive is on). Record-only — no decision path reads it — so a
        // settlement with it enabled steps byte-identically to one without.
        if settlement.saving_allocation_obs_active() {
            settlement.society.enable_allocation_trace();
        }
        // DH.a (impl-68): seed the whole-population provenance ledger from the generated holdings
        // (registry, endowed gold/physical buckets, the InitialHolding/A2FrontLoad tape). Pure
        // observation; a no-op off the closed-circulation marker.
        settlement.closure_init();
        settlement
    }
    /// Generate a G8c-1 **finance** settlement: the Austrian business cycle (or its
    /// sound-money control) on econ's unchanged credit-ladder scenario. There is no
    /// spatial colony — the society IS the cycle, and [`Settlement::econ_tick`] just
    /// steps it. The shadow scenario is retained so the natural-rate baseline can be
    /// replayed on demand.
    fn generate_finance(seed: u64, config: &SettlementConfig) -> Self {
        // Scope: a finance settlement is either the G8c-1 credit cycle/control or a
        // G8c-2 tender bench. It requires the M3 ledger and is mutually exclusive with
        // every spatial overlay — its colony is empty by construction.
        assert!(
            config.m3,
            "a finance (G8c-1/G8c-2) settlement requires the M3 ledger (m3 = true)"
        );
        assert!(
            !(config.cycle.is_some() && config.tender_bench.is_some()),
            "a finance settlement is either the credit cycle or a tender bench, not both"
        );
        assert!(
            config.chain.is_none()
                && config.demography.is_none()
                && config.barter.is_none()
                && config.bank.is_none()
                && config.resident_traders.is_empty(),
            "a finance (G8c-1/G8c-2) settlement has no spatial overlay \
             (chain/demography/barter/bank/resident_traders); the demonstration runs in \
             the econ society"
        );
        assert!(
            config.gatherers == 0 && config.consumers == 0 && config.nodes.is_empty(),
            "a finance (G8c-1/G8c-2) settlement has no spatial colony (no \
             gatherers/consumers/nodes); use the credit_cycle / sound_money / \
             *_tender_* constructors"
        );
        // G8c-3: the tax overlay rides on the credit-cycle settlement (the chartalist
        // counter-lever to the wage refusal) — never on a tender bench, which exercises
        // a different surface. The levy/receivability route through econ's M21 machinery
        // on the cycle society.
        assert!(
            config.tax.is_none() || config.cycle.is_some(),
            "a G8c-3 tax overlay requires the credit cycle (use tax_in_fiat / tax_in_specie)"
        );

        // Build the society from econ's unchanged scenario — the credit-ladder cycle
        // (with its tender policy layered in) or a fiat-displacement tender bench. The
        // scenario is stamped with this run's seed so the demonstration is reproducible
        // per `(seed, config)`; the cycle additionally retains it (credit-disabled) for
        // the shadow replay.
        let (mut scenario, cycle_runtime, bench_runtime) = match (config.cycle, config.tender_bench)
        {
            (Some(cycle), None) => {
                let scenario = cycle_scenario(cycle.kind, cycle.tender);
                (scenario, Some(cycle.kind), None)
            }
            (None, Some(bench)) => {
                let scenario = tender_bench_scenario(bench);
                (scenario, None, Some(bench.surface))
            }
            _ => unreachable!(
                "the finance branch is taken only with a cycle, a bench, or a tax on the cycle \
                 (cycle and bench are asserted mutually exclusive above)"
            ),
        };
        // G8c-3: layer the tax overlay's M21 events (SetTaxReceivability + the levies)
        // onto the cycle scenario, before stamping the seed and building the society, so
        // the events flow into the society, the retained cycle scenario (canonical bytes
        // + shadow replay), and the run identically. A `None` overlay adds nothing, so a
        // tax-free cycle is byte-identical.
        let tax_runtime = config.tax.as_ref().map(|tax| {
            tax.apply_to(&mut scenario);
            TaxRuntime {
                receivability: tax.receivability,
                levied: tax.total_levied(),
            }
        });
        scenario.seed = seed;
        let mut society = Society::from_scenario(scenario.clone());
        society.enable_consumption_log();
        // impl-76 / C3R.k: the cycle society carries no chain (`config.chain` is `None` here), so
        // the lever stays off — but populate it uniformly with the frontier path for parity.
        society.set_satiated_surplus_ask(config.chain.as_ref().and_then(|chain| {
            chain
                .satiated_surplus_ask_at
                .map(|at| (at, chain.satiated_surplus_ask_scope))
        }));

        // A minimal spatial shell: an empty grid + an exchange stockpile so the
        // (no-op) world phases and the exchange accessor have a valid world to read.
        let grid = Grid::new(config.width.max(1), config.height.max(1));
        let mut world = World::new(grid);
        let exchange = world
            .add_stockpile(Stockpile::new(config.exchange, config.exchange_cap))
            .expect("exchange lands on a passable tile");

        Self {
            generation_seed: seed,
            #[cfg(test)]
            test_fault_mint_birth_gold: 0,
            world,
            society,
            colonists: Vec::new(),
            live_colonist_slots: Vec::new(),
            colonist_slot_by_id: BTreeMap::new(),
            dynamics: config.dynamics,
            known: KnownGoods {
                hunger: FOOD,
                warmth: WOOD,
                savings: GOLD,
                subsistence: None,
            },
            exchange,
            // A finance settlement has no spatial colony — no FORAGE node is created.
            forage_node_id: None,
            carry_cap: config.carry_cap,
            move_speed: config.move_speed,
            // No spatial goods are tracked: the demonstration's goods live inside
            // econ's own (conserving) market + project machinery, and the finance
            // settlement's conservation is the M3 ledger reconcile + the fiat base
            // identity. An empty set makes the per-tick whole-system receipt vacuously
            // hold.
            goods: Vec::new(),
            money_rejection_goods: Vec::new(),
            pending_deposits: BTreeMap::new(),
            trader_ids: Vec::new(),
            chain: None,
            capital_loans: BTreeMap::new(),
            capital_builds: Vec::new(),
            next_capital_project_id: 0,
            tools_built: 0,
            mortal_producer_old_age_deaths: 0,
            role_readoptions: 0,
            mortal_capital_builds: 0,
            producer_tool_inheritances: 0,
            heirless_producer_deaths: 0,
            heir_tool_adoptions: 0,
            producer_house_hearth_food_minted: 0,
            non_producer_hearth_food_minted: 0,
            producer_house_births: 0,
            producer_house_deaths: 0,
            producer_house_person_ticks: 0,
            producer_recipe_pay_rejections: 0,
            producer_build_rejections: 0,
            producer_adoption_rejections: 0,
            producer_tool_inheritors: BTreeSet::new(),
            last_capital_decisions: Vec::new(),
            peak_pre_promotion_hunger: 0,
            critical_ticks_pre_promotion: 0,
            bread_provenance: BreadProvenance::default(),
            multigood: MultigoodMoney::default(),
            acquisition: AcquisitionLedger::default(),
            earned_provisioning: EarnedProvisioningLedger::default(),
            birth_stock_wants_emitted: 0,
            birth_stock_attributable_purchases: 0,
            birth_stock_below_target_agents: BTreeSet::new(),
            birth_stock_reached_agents: BTreeSet::new(),
            birth_stock_held_max: 0,
            birth_stock_held_at_death: 0,
            birth_stock_eligible_opportunities: 0,
            birth_stock_injections_completed: 0,
            birth_stock_source_shortfalls: 0,
            ignition_injected_qty: 0,
            ignition_gate_blocked_interval: 0,
            ignition_gate_extinct: 0,
            ignition_gate_blocked_cap: 0,
            ignition_gate_blocked_hunger: 0,
            ignition_gate_suppressed_at_target: 0,
            ignition_gate_donor_shortfall: 0,
            producer_birth_funded_by_channel: [0; FoodChannel::COUNT],
            producer_birth_funded_intervention: 0,
            birth_stock_injection_records: Vec::new(),
            birth_stock_births_by_household: Vec::new(),
            last_birth_stock_attribution_snapshot: BTreeSet::new(),
            saving_allocation_obs: SavingAllocationObs::default(),
            birth_gate_obs: birth_gate::BirthGateObs::default(),
            saving_obs_stock_tick: None,
            saving_obs_pending_offerable: None,
            bootstrap_trace: BootstrapTrace::default(),
            flour_census: None,
            flour_census_armed: false,
            bread_seller_trace: Vec::new(),
            seeded_surplus_trace: SeededSurplusTrace::default(),
            seeded_minted_bread_sold_for_salt: 0,
            emergency_bread_provisioned: 0,
            subsistence_commons_stock: 0,
            subsistence_commons_cap: 0,
            subsistence_commons_regen: 0,
            subsistence_commons_phi_bps: 0,
            subsistence_commons_drawn_total: 0,
            subsistence_commons_unmet_total: 0,
            subsistence_commons_depleted_ticks: 0,
            subsistence_commons_shortfall_ticks: 0,
            subsistence_commons_eligible_need_total: 0,
            wage_escrow_gold: Gold::ZERO,
            wage_escrows: Vec::new(),
            next_wage_contract_id: 0,
            wage_retained_earnings: BTreeMap::new(),
            wage_proceeds_buckets: BTreeMap::new(),
            wage_workers_ever: BTreeSet::new(),
            wage_employers_ever: BTreeSet::new(),
            wage_hires_total: 0,
            wage_hires_post_promotion: 0,
            wage_below_ask_not_hired: 0,
            wage_endowment_funded_wages: Gold::ZERO,
            wage_financed_output_buys: Gold::ZERO,
            wage_nonowner_output_buys: Gold::ZERO,
            wage_circular_loop_turnovers: 0,
            share_contracts: Vec::new(),
            next_share_contract_id: 0,
            share_workers_ever: BTreeSet::new(),
            share_owners_ever: BTreeSet::new(),
            share_contracts_total: 0,
            share_voluntary_contracts_total: 0,
            share_forced_contracts_total: 0,
            share_renewals_total: 0,
            share_worker_bread_income: 0,
            share_owner_bread_income: 0,
            share_worker_declined: 0,
            share_worker_unmatched: 0,
            share_forward_only_eligibility: 0,
            share_renewal_hints_total: 0,
            share_renewal_fed_out: 0,
            share_renewal_base_ineligible: 0,
            share_renewal_owner_not_candidate: 0,
            share_renewal_bread_declined: 0,
            share_renewal_matched_elsewhere: 0,
            share_owner_candidates_total: 0,
            share_owner_no_atcap_plot: 0,
            share_stock_opportunity_refusal: 0,
            share_reservation_collision: 0,
            share_stock_drawdown: 0,
            share_unattributed_share_deposit: 0,
            share_owner_grain_settled: 0,
            share_successions_total: 0,
            share_succession_heir_declined: 0,
            share_succession_worker_re_declined: 0,
            share_post_succession_renewals: 0,
            share_succeeded_live_ids: BTreeSet::new(),
            in_kind_contracts: Vec::new(),
            next_in_kind_contract_id: 0,
            in_kind_workers_ever: BTreeSet::new(),
            in_kind_employers_ever: BTreeSet::new(),
            in_kind_hires_total: 0,
            in_kind_worker_advance_bread: 0,
            in_kind_employer_bread_income: 0,
            in_kind_expected_output_total: 0,
            in_kind_worker_declined: 0,
            in_kind_worker_unmatched: 0,
            in_kind_owner_candidates_total: 0,
            in_kind_owner_no_atcap_plot: 0,
            in_kind_owner_insufficient_fund: 0,
            in_kind_productivity_declined: 0,
            in_kind_reservation_collision: 0,
            in_kind_stock_drawdown: 0,
            in_kind_unattributed_deposit: 0,
            in_kind_employer_grain_settled: 0,
            in_kind_endowment_funded_hires: 0,
            in_kind_term_starvations: 0,
            ever_landowner_ids: BTreeSet::new(),
            owner_first_claim_tick: BTreeMap::new(),
            owner_age_at_first_claim: BTreeMap::new(),
            owner_tenure_before_death: Vec::new(),
            owner_bread_consumed: BTreeMap::new(),
            owner_surplus_sold_before_death: BTreeMap::new(),
            owner_inventory_at_death: Vec::new(),
            inherited_stock_to_heirs: 0,
            buyer_purchases_by_owner_age_cohort: BTreeMap::new(),
            owner_seller_attributed_bought: 0,
            cultivation_skill_producers: BTreeSet::new(),
            cultivation_grain_harvested: BTreeMap::new(),
            cultivation_bread_produced: BTreeMap::new(),
            cultivation_proceeds_scratch: BTreeMap::new(),
            profit_retained_ids: BTreeSet::new(),
            profit_retained_ever: BTreeSet::new(),
            commitment_committed_ever: BTreeSet::new(),
            commitment_uptake: BTreeMap::new(),
            commitment_fiat_ever: BTreeSet::new(),
            commitment_below_floor_ever: BTreeSet::new(),
            commitment_exit_override_ids: BTreeSet::new(),
            commitment_exit_override_ever: BTreeSet::new(),
            commitment_norm_copy_events: Vec::new(),
            commitment_norm_flip_events: Vec::new(),
            commitment_norm_adoptions: 0,
            commitment_norm_abandonments: 0,
            commitment_norm_imitation_adopters: BTreeSet::new(),
            commitment_norm_group_covariance_sum: 0,
            commitment_norm_group_covariance_count: 0,
            cultivation_tool_builds: Vec::new(),
            next_cultivation_tool_project_id: 0,
            cultivation_tool_producers: BTreeSet::new(),
            cultivation_tools_built: 0,
            cultivation_tool_wood_consumed: 0,
            cultivation_tools_destroyed: 0,
            endowed_cultivation_tools_total: 0,
            endowed_households: Vec::new(),
            endowed_member_ids: Vec::new(),
            cultivation_tool_inherited_total: 0,
            cultivation_tool_inheritor_ids: BTreeSet::new(),
            land_plots: BTreeMap::new(),
            secure_land_inheritance_events: Vec::new(),
            secure_land_owner_old_age_deaths_total: 0,
            secure_land_inherit_eligible_owner_deaths_total: 0,
            secure_land_stranded_shares_total: 0,
            land_claims_total: 0,
            land_idle_losses_total: 0,
            land_harvest_denials_total: 0,
            land_owner_gate_denials_total: 0,
            land_nonowner_harvest_of_owned_total: 0,
            land_reclaims_by_other_total: 0,
            land_marginal_nonowner_claims_total: 0,
            land_lapsed_reentry_worse_total: 0,
            land_plot_harvest_totals: BTreeMap::new(),
            land_lapsed_losses: BTreeMap::new(),
            land_lost_prior_owners: BTreeMap::new(),
            land_market_plots: BTreeMap::new(),
            land_market_yield_this_tick: BTreeMap::new(),
            land_market_sales: Vec::new(),
            land_market_trade_count: 0,
            land_market_pre_promotion_trade_count: 0,
            land_market_carrying_paid_total: 0,
            land_market_pre_promotion_charges: 0,
            land_market_foreclosure_listings_total: 0,
            land_market_priced_out_total: 0,
            land_market_lapsed_priced_out_total: 0,
            land_market_ask_bid_gap_sum: 0,
            land_market_ask_bid_gap_count: 0,
            land_market_title_history: BTreeMap::new(),
            land_fee_pool_salt: Gold::ZERO,
            econ_tick: 0,
            last_report: EconTickReport::default(),
            commons_gold: Gold::ZERO,
            commons_stock: BTreeMap::new(),
            demography: None,
            households: Vec::new(),
            birth_seq: 0,
            births_total: 0,
            old_age_deaths_total: 0,
            starvation_deaths_total: 0,
            birth_block_interval: 0,
            birth_block_size_cap: 0,
            birth_block_hunger_ceiling: 0,
            birth_block_endowment: 0,
            barter: None,
            barter_medium: None,
            salt_direct_use: None,
            knowledge: 0,
            tier2_unlocked_at: None,
            bank: None,
            cycle: cycle_runtime.map(|kind| CycleRuntime {
                kind,
                scenario: scenario.clone(),
            }),
            shadow_cycle_cache: RefCell::new(None),
            bench: bench_runtime.map(|surface| BenchRuntime { surface, scenario }),
            tax: tax_runtime,
            // DH.a (impl-68): a finance settlement is never closed-circulation.
            closed_circulation: false,
            closure: closure::ClosureLedger::default(),
            burden: burden::BurdenTelemetry::default(),
        }
    }
}
