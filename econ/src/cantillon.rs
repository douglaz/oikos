//! Deterministic first-receiver routing for new fiat.

use crate::agent::{Agent, AgentId, Role};
use crate::good::Gold;
use crate::project::Tick;
use crate::purpose::CreditSource;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum CantillonRoute {
    Agents(Vec<AgentId>),
    Sector(CantillonSector),
    Helicopter,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CantillonSector {
    Capitalists,
    Households,
    Workers,
    Consumers,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CantillonReceipt {
    pub tick: Tick,
    pub agent: AgentId,
    pub amount: Gold,
    pub source: CreditSource,
}

pub struct CantillonRouter;

impl CantillonRouter {
    pub fn route(route: &CantillonRoute, agents: &[Agent], amount: Gold) -> Vec<(AgentId, Gold)> {
        let recipients = recipients(route, agents);
        split_amount(recipients, amount)
    }

    pub fn receipts(
        tick: Tick,
        credits: &[(AgentId, Gold)],
        source: CreditSource,
    ) -> Vec<CantillonReceipt> {
        credits
            .iter()
            .filter(|(_, amount)| *amount > Gold::ZERO)
            .map(|(agent, amount)| CantillonReceipt {
                tick,
                agent: *agent,
                amount: *amount,
                source,
            })
            .collect()
    }
}

fn recipients(route: &CantillonRoute, agents: &[Agent]) -> Vec<AgentId> {
    let mut known_agents = agents.iter().map(|agent| agent.id).collect::<Vec<_>>();
    known_agents.sort();
    known_agents.dedup();

    let mut recipients: Vec<AgentId> = match route {
        CantillonRoute::Agents(agents) => agents
            .iter()
            .copied()
            .filter(|agent| known_agents.binary_search(agent).is_ok())
            .collect(),
        CantillonRoute::Sector(sector) => agents
            .iter()
            .filter(|agent| {
                agent
                    .roles
                    .iter()
                    .any(|role| role_matches_sector(*role, *sector))
            })
            .map(|agent| agent.id)
            .collect(),
        CantillonRoute::Helicopter => agents.iter().map(|agent| agent.id).collect(),
    };
    recipients.sort();
    recipients.dedup();
    recipients
}

fn role_matches_sector(role: Role, sector: CantillonSector) -> bool {
    match sector {
        CantillonSector::Capitalists => role == Role::Capitalist,
        CantillonSector::Households => role == Role::Household,
        CantillonSector::Workers => role == Role::Worker,
        CantillonSector::Consumers => role == Role::Consumer,
    }
}

fn split_amount(recipients: Vec<AgentId>, amount: Gold) -> Vec<(AgentId, Gold)> {
    if recipients.is_empty() || amount == Gold::ZERO {
        return Vec::new();
    }
    let count = u64::try_from(recipients.len()).unwrap_or(u64::MAX).max(1);
    let base = amount.0 / count;
    let remainder = amount.0 % count;
    recipients
        .into_iter()
        .enumerate()
        .filter_map(|(index, agent)| {
            let extra = if u64::try_from(index).ok()? < remainder {
                1
            } else {
                0
            };
            let share = Gold(base.saturating_add(extra));
            (share > Gold::ZERO).then_some((agent, share))
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::{CantillonReceipt, CantillonRoute, CantillonRouter, CantillonSector};
    use crate::agent::{Agent, AgentId, Role};
    use crate::expect::PriceBelief;
    use crate::good::{Gold, Stock};
    use crate::ledger::IssuerId;
    use crate::project::Tick;
    use crate::purpose::CreditSource;

    fn agent(id: u32, roles: Vec<Role>) -> Agent {
        Agent {
            id: AgentId(id),
            scale: Vec::new(),
            stock: Stock::new(3),
            gold: Gold::ZERO,
            labor_capacity: 0,
            hunger_deficit: 0,
            roles,
            expect: vec![PriceBelief::new(Gold(1), Gold(1)); 4],
        }
    }

    #[test]
    fn agents_route_splits_integer_remainder_deterministically() {
        let agents = vec![agent(3, vec![]), agent(1, vec![]), agent(2, vec![])];
        let route = CantillonRoute::Agents(vec![AgentId(3), AgentId(1), AgentId(2)]);

        let credits = CantillonRouter::route(&route, &agents, Gold(5));

        assert_eq!(
            credits,
            vec![
                (AgentId(1), Gold(2)),
                (AgentId(2), Gold(2)),
                (AgentId(3), Gold(1))
            ]
        );
    }

    #[test]
    fn sector_route_selects_named_role_agents() {
        let agents = vec![
            agent(1, vec![Role::Household]),
            agent(2, vec![Role::Capitalist]),
            agent(3, vec![Role::Capitalist, Role::Consumer]),
        ];

        let credits = CantillonRouter::route(
            &CantillonRoute::Sector(CantillonSector::Capitalists),
            &agents,
            Gold(4),
        );

        assert_eq!(credits, vec![(AgentId(2), Gold(2)), (AgentId(3), Gold(2))]);
    }

    #[test]
    fn helicopter_route_is_deterministic_by_agent_id() {
        let agents = vec![agent(8, vec![]), agent(4, vec![]), agent(6, vec![])];

        let credits = CantillonRouter::route(&CantillonRoute::Helicopter, &agents, Gold(4));

        assert_eq!(
            credits,
            vec![
                (AgentId(4), Gold(2)),
                (AgentId(6), Gold(1)),
                (AgentId(8), Gold(1))
            ]
        );
    }

    #[test]
    fn receipts_record_tick_source_and_agent() {
        let source = CreditSource::FiatFiscal(IssuerId(1));
        let receipts = CantillonRouter::receipts(
            Tick(7),
            &[(AgentId(2), Gold(3)), (AgentId(4), Gold::ZERO)],
            source,
        );

        assert_eq!(
            receipts,
            vec![CantillonReceipt {
                tick: Tick(7),
                agent: AgentId(2),
                amount: Gold(3),
                source,
            }]
        );
    }

    #[test]
    fn agents_route_ignores_unknown_agent_ids() {
        let agents = vec![agent(1, vec![]), agent(3, vec![])];
        let route = CantillonRoute::Agents(vec![AgentId(99), AgentId(3), AgentId(1)]);

        let credits = CantillonRouter::route(&route, &agents, Gold(5));

        assert_eq!(credits, vec![(AgentId(1), Gold(3)), (AgentId(3), Gold(2))]);
    }
}
