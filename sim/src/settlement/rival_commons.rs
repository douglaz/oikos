use super::*;

impl Settlement {
    pub(super) fn init_rival_subsistence_commons(&mut self) {
        if !self.rival_subsistence_commons_active() {
            return;
        }
        let phi_bps = self
            .chain
            .as_ref()
            .map_or(0, |chain| chain.rival_subsistence_commons_phi_bps);
        let regen = rival_subsistence_commons_regen_for_phi(phi_bps);
        let cap = regen.saturating_mul(RIVAL_COMMONS_K_TICKS);
        self.subsistence_commons_phi_bps = phi_bps;
        self.subsistence_commons_regen = regen;
        self.subsistence_commons_cap = cap;
        self.subsistence_commons_stock = cap;
    }

    /// EMERGENCY SELF-PROVISIONING phase (S21h.1): the demand-side survival bridge. A
    /// **non-lineage** `Consumer`/`Gatherer` (the SALT-rich buyers + the specialist
    /// woodcutters — `household: None`, never cultivators) that has reached
    /// [`ChainConfig::emergency_hunger_threshold`] produces, BY ITS OWN LABOR and from no
    /// input, just enough of the tracked hunger staple (BREAD — not a FORAGE/subsistence
    /// good, so the value scale never gains a `known.subsistence` term) to pull its
    /// projected hunger down to one notch BELOW the threshold, then immediately eats all of
    /// it through the same consumption-readback seam the own-use cultivation uses.
    ///
    /// The bread is booked `report.produced` (a conserved own-labor source, never a mint or
    /// endowment) and credited [`FoodChannel::SelfProduced`] — the credit the fixed
    /// own-labor forage path lacks but the cultivation path has — so `seeded_minted` stays
    /// 0. **Produced units == eaten units in the same phase, so nothing offerable remains
    /// after the tick** (no sellable remainder ⇒ emergency bread can never be sold for SALT
    /// ⇒ it cannot fake supply or monetize the token). The pull-to-`threshold-1` cap is the
    /// near-critical floor that keeps the role ALIVE without satiating it: residual hunger
    /// stays high, so the role still demands and prefers to BUY bread (demand-preserving),
    /// self-provisioning only the shortfall its market purchases this tick did not cover.
    ///
    /// Runs AFTER the market step (so the bread is made post-clearing and never offered) and
    /// after the own-use cultivation phase, BEFORE the provenance/acquisition own-use consume
    /// passes (so its consume-log tail is sinked by them). A no-op unless the gated seam is
    /// active, so every other run is byte-identical. Deterministic: slot order, integer
    /// thresholds, nothing drawn.
    pub(super) fn run_emergency_self_provision(&mut self, report: &mut EconTickReport) {
        if !self.emergency_self_provision_active() {
            return;
        }
        let Some(bread) = self.provenance_bread_good() else {
            return;
        };
        let threshold = self
            .chain
            .as_ref()
            .map_or(0, |chain| chain.emergency_hunger_threshold);
        if threshold == 0 {
            return;
        }
        // Pull projected hunger to one notch below the trigger — "just enough to not die"
        // while staying hungry (demand-preserving). Never to 0 (that would satiate the role
        // out of the bread market — the too-strong failure the cushion sweep exhibits).
        let target = threshold.saturating_sub(1);
        let acquisition = self.acquisition_ledger_active();
        let provenance = self.bread_provenance_active();
        let hunger_deplete = self.dynamics.hunger_deplete;
        let hunger_per_food = self.dynamics.hunger_per_food;
        // The goods the need readback counts as hunger relief (mirrors the own-use
        // cultivation phase): the hunger staple plus any directly-edible subsistence food.
        let hunger_staple = self.known.hunger;
        let subsistence_food = self.known.subsistence;
        let commons_active = self.rival_subsistence_commons_active();
        if commons_active {
            self.regenerate_subsistence_commons(bread, report);
        }
        let live = self.live_colonist_slots.clone();
        let mut commons_requests = Vec::new();
        for slot in live {
            let (id, vocation, household, hunger) = {
                let colonist = &self.colonists[slot];
                (
                    colonist.id,
                    colonist.vocation,
                    colonist.household,
                    colonist.need.hunger,
                )
            };
            // Only the NON-LINEAGE demand-side roles self-provision in the emergency: the
            // buyers (`Consumer`) and the woodcutters (`Gatherer`). Lineage members already
            // cultivate (their own hysteresis), and active producers earn by the market.
            if household.is_some()
                || !matches!(vocation, Vocation::Consumer | Vocation::Gatherer)
                || hunger < threshold
            {
                continue;
            }
            // Net out food this role ALREADY ate in this tick's market consume pass. Also
            // consume any bread it bought later in the same market tick and still holds in
            // stock BEFORE minting emergency bread: otherwise the stock debit below could eat
            // the bought loaf and leave the newly credited SelfProduced loaf offerable.
            let already_food = self
                .society
                .consumption_log_last_tick()
                .iter()
                .filter(|&&(a, g, _)| {
                    a == id && (g == hunger_staple || Some(g) == subsistence_food)
                })
                .fold(0u32, |acc, &(_, _, qty)| acc.saturating_add(qty));
            let needed =
                food_needed_to_reach_hunger(hunger, hunger_deplete, hunger_per_food, target)
                    .saturating_sub(already_food);
            if needed == 0 {
                continue;
            }
            let held_bread = self.society.free_stock_after_all_reserves(id, bread);
            let held_eat = held_bread.min(needed);
            self.consume_own_use_stock(id, bread, held_eat);
            let eat = needed.saturating_sub(held_eat);
            if eat == 0 {
                continue;
            }
            if commons_active {
                commons_requests.push(SubsistenceCommonsRequest {
                    agent: id,
                    hunger,
                    need: eat,
                });
                continue;
            }
            // PRODUCE the floor from own labor (no grain input): conserved `report.produced`,
            // credited SelfProduced — then immediately eat ALL of it, so no offerable unit
            // remains. `credit_produced` books the produced side; `consume_own_use_stock`
            // debits + logs the consume the next-tick readback advances hunger from.
            self.credit_produced(id, bread, eat, report);
            // Book the produced-origin units into the bread-provenance ledger too (symmetric
            // with the own-use cultivation phase) so the immediately-following own-use consume
            // sink draws THESE produced units rather than over-drawing some other produced
            // bread the role happens to hold — keeping the produced-vs-minted attribution
            // exact if the emergency seam is ever composed with a seeded/minted-bread path.
            // Conserves: produced credited here is sunk by the own-use consume pass this tick.
            if provenance {
                let lineage = self.is_lineage_agent(id);
                self.bread_provenance
                    .credit_produced(id, u64::from(eat), lineage);
            }
            if acquisition {
                self.acquisition
                    .credit(id, FoodChannel::SelfProduced, u64::from(eat));
            }
            self.consume_own_use_stock(id, bread, eat);
            self.emergency_bread_provisioned = self
                .emergency_bread_provisioned
                .saturating_add(u64::from(eat));
        }
        if commons_active {
            self.fulfill_subsistence_commons_requests(bread, commons_requests, report);
        }
    }

    pub(super) fn regenerate_subsistence_commons(
        &mut self,
        bread: GoodId,
        report: &mut EconTickReport,
    ) {
        report.subsistence_commons_stock_before = self.subsistence_commons_stock;
        report.subsistence_commons_cap = self.subsistence_commons_cap;
        report.subsistence_commons_phi_bps = self.subsistence_commons_phi_bps;
        if self.subsistence_commons_regen == 0
            || self.subsistence_commons_stock >= self.subsistence_commons_cap
        {
            report.subsistence_commons_stock_after = self.subsistence_commons_stock;
            return;
        }
        let before = self.subsistence_commons_stock;
        self.subsistence_commons_stock = self
            .subsistence_commons_stock
            .saturating_add(self.subsistence_commons_regen)
            .min(self.subsistence_commons_cap);
        let regen = self.subsistence_commons_stock - before;
        if regen > 0 {
            *report.subsistence_commons_regen.entry(bread).or_insert(0) += regen;
        }
        report.subsistence_commons_stock_after = self.subsistence_commons_stock;
    }

    pub(super) fn fulfill_subsistence_commons_requests(
        &mut self,
        bread: GoodId,
        mut requests: Vec<SubsistenceCommonsRequest>,
        report: &mut EconTickReport,
    ) {
        requests.sort_by_key(|request| (std::cmp::Reverse(request.hunger), request.agent.0));
        let total_need: u64 = requests.iter().map(|request| u64::from(request.need)).sum();
        self.subsistence_commons_eligible_need_total = self
            .subsistence_commons_eligible_need_total
            .saturating_add(total_need);
        let mut drawn_total = 0u64;
        let mut unmet_total = 0u64;
        for request in requests {
            if request.need == 0 {
                continue;
            }
            let available = self.subsistence_commons_stock.min(u64::from(u32::MAX));
            let Some(held) = self
                .society
                .agents
                .get(request.agent)
                .map(|agent| agent.stock.get(bread))
            else {
                unmet_total = unmet_total.saturating_add(u64::from(request.need));
                continue;
            };
            let headroom = u64::from(u32::MAX - held);
            let draw = u64::from(request.need).min(available).min(headroom);
            let draw_u32 = u32::try_from(draw).unwrap_or(0);
            if draw_u32 == 0 {
                unmet_total = unmet_total.saturating_add(u64::from(request.need));
                continue;
            }
            if self.society.credit_stock(request.agent, bread, draw_u32) {
                self.subsistence_commons_stock =
                    self.subsistence_commons_stock.saturating_sub(draw);
                drawn_total = drawn_total.saturating_add(draw);
                *report.subsistence_commons_draw.entry(bread).or_insert(0) += draw;
                if self.acquisition_ledger_active() {
                    self.acquisition
                        .credit(request.agent, FoodChannel::Commons, draw);
                }
                self.consume_own_use_stock(request.agent, bread, draw_u32);
                unmet_total =
                    unmet_total.saturating_add(u64::from(request.need).saturating_sub(draw));
            } else {
                unmet_total = unmet_total.saturating_add(u64::from(request.need));
            }
        }
        self.subsistence_commons_drawn_total = self
            .subsistence_commons_drawn_total
            .saturating_add(drawn_total);
        self.subsistence_commons_unmet_total = self
            .subsistence_commons_unmet_total
            .saturating_add(unmet_total);
        if total_need > 0 && self.subsistence_commons_stock == 0 {
            self.subsistence_commons_depleted_ticks =
                self.subsistence_commons_depleted_ticks.saturating_add(1);
        }
        if unmet_total > 0 {
            self.subsistence_commons_shortfall_ticks =
                self.subsistence_commons_shortfall_ticks.saturating_add(1);
        }
        report.subsistence_commons_stock_after = self.subsistence_commons_stock;
        debug_assert!(
            report.subsistence_commons_conserves(),
            "subsistence commons conservation broke at econ tick {}",
            self.econ_tick
        );
    }

    pub(super) fn forecast_commons_sufficiency(
        &self,
        target_agent: AgentId,
        bread: GoodId,
    ) -> bool {
        if !self.rival_subsistence_commons_active() {
            return false;
        }
        let threshold = self
            .chain
            .as_ref()
            .map_or(0, |chain| chain.emergency_hunger_threshold);
        if threshold == 0 {
            return true;
        }
        let target_hunger = threshold.saturating_sub(1);
        let mut requests = Vec::new();
        for &slot in &self.live_colonist_slots {
            let colonist = &self.colonists[slot];
            if colonist.household.is_some()
                || !matches!(colonist.vocation, Vocation::Consumer | Vocation::Gatherer)
                || colonist.need.hunger < threshold
            {
                continue;
            }
            let needed = food_needed_to_reach_hunger(
                colonist.need.hunger,
                self.dynamics.hunger_deplete,
                self.dynamics.hunger_per_food,
                target_hunger,
            );
            let held = self
                .society
                .free_stock_after_all_reserves(colonist.id, bread);
            let need = needed.saturating_sub(held);
            if need > 0 {
                requests.push(SubsistenceCommonsRequest {
                    agent: colonist.id,
                    hunger: colonist.need.hunger,
                    need,
                });
            } else if colonist.id == target_agent {
                return true;
            }
        }
        if !requests.iter().any(|request| request.agent == target_agent) {
            return true;
        }
        requests.sort_by_key(|request| (std::cmp::Reverse(request.hunger), request.agent.0));
        let mut available = self
            .subsistence_commons_stock
            .saturating_add(self.subsistence_commons_regen)
            .min(self.subsistence_commons_cap);
        for request in requests {
            let held = self
                .society
                .agents
                .get(request.agent)
                .map_or(u32::MAX, |agent| agent.stock.get(bread));
            let headroom = u64::from(u32::MAX - held);
            let draw = u64::from(request.need).min(available).min(headroom);
            available = available.saturating_sub(draw);
            if request.agent == target_agent {
                return draw >= u64::from(request.need);
            }
        }
        true
    }

    pub(super) fn rival_subsistence_commons_active(&self) -> bool {
        self.chain
            .as_ref()
            .is_some_and(chain_runtime_rival_subsistence_commons_active)
            && self.provenance_bread_good().is_some()
    }

    /// S21h.1: whether the **emergency self-provisioning** seam is active this tick — the
    /// `emergency_hunger_threshold` is set (>0) AND the chain carries a bread good to make.
    /// When this holds, [`Self::run_emergency_self_provision`] runs the near-critical
    /// own-labor bread floor for the hungry non-lineage roles after the market step. Off
    /// (every existing config, threshold 0) the phase is inert and the run is byte-identical.
    pub(super) fn emergency_self_provision_active(&self) -> bool {
        self.chain
            .as_ref()
            .is_some_and(|chain| chain.emergency_hunger_threshold > 0)
            && self.provenance_bread_good().is_some()
    }

    pub fn subsistence_commons_stock_of(&self, good: GoodId) -> u64 {
        if self.rival_subsistence_commons_active()
            && self
                .provenance_bread_good()
                .is_some_and(|bread| bread == good)
        {
            self.subsistence_commons_stock
        } else {
            0
        }
    }

    pub fn rival_subsistence_commons_state(&self) -> RivalSubsistenceCommonsState {
        RivalSubsistenceCommonsState {
            stock: self.subsistence_commons_stock,
            cap: self.subsistence_commons_cap,
            regen: self.subsistence_commons_regen,
            phi_bps: self.subsistence_commons_phi_bps,
            drawn_total: self.subsistence_commons_drawn_total,
            unmet_total: self.subsistence_commons_unmet_total,
            depleted_ticks: self.subsistence_commons_depleted_ticks,
            shortfall_ticks: self.subsistence_commons_shortfall_ticks,
            eligible_need_total: self.subsistence_commons_eligible_need_total,
        }
    }
}
