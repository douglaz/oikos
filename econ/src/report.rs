//! Output rendering for records.

use crate::barter::{BarterReason, BarterTrade};
use crate::factor::LaborTrade;
use crate::good::{good_name, Gold, GoodId};
use crate::market::Trade;
use crate::menger::SaleabilitySnapshot;
use crate::money::{
    bank_repayment_tender_name, issuer_repayment_tender_name, labor_wage_tender_name,
    public_debt_tender_name, public_spot_tender_name, tax_receivability_name,
};
use crate::record::{
    BankAuditRecord, BankRepaymentAuditRecord, DebtPaymentAuditRecord, DebtPaymentState,
    IssuerRepaymentAuditRecord, M2Record, M3Record, M4Record, MarketRecord, MoneyAuditRecord,
    PaymentAuditRecord, PaymentKind, Record, RedemptionAuditRecord, RedemptionOutcome,
    TaxAuditRecord, V2Phase, V2Record, WagePaymentAuditRecord,
};
use crate::sweep::{SweepKey, SweepRecord};
use crate::timemarket::LoanTrade;

pub const M3_CSV_HEADER: &str = "tick,regime,total_gold,spot_trades,labor_trades,loan_trades,public_specie,public_fiat,demand_claims,bank_reserves,fiduciary,time_deposits,tms,bank_credit_issued,fiat_credit_issued,fiat_fiscal_issued,credit_retired,bank_loan_trades,fiat_loan_trades,market_rate_bps,shadow_natural_rate_bps,shadow_rate_gap_bps,structure_length_ticks_x100,boom_projects_started,bust_abandoned_projects,abandoned_projects,capital_labor_consumed,capital_goods_consumed,debts_settled,debts_defaulted,project_debts_defaulted,early_receiver_wealth_delta,late_receiver_wealth_delta";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum OutputFormat {
    Table,
    Csv,
}

const TABLE_COLUMNS: [(&str, usize); 17] = [
    ("tick", 4),
    ("food", 4),
    ("wood", 4),
    ("nets", 4),
    ("labor", 5),
    ("leisure", 7),
    ("ate", 3),
    ("hungry", 6),
    ("active", 6),
    ("done", 4),
    ("drop", 4),
    ("lost_labor", 10),
    ("lost_goods", 10),
    ("gather", 6),
    ("cut", 3),
    ("fish", 4),
    ("project", 7),
];

pub fn render(records: &[Record], format: OutputFormat) -> String {
    match format {
        OutputFormat::Table => render_table(records),
        OutputFormat::Csv => render_csv(records),
    }
}

pub fn render_market(records: &[MarketRecord], format: OutputFormat) -> String {
    match format {
        OutputFormat::Table => render_market_table(records),
        OutputFormat::Csv => render_market_csv(records),
    }
}

pub fn render_m2(records: &[M2Record], format: OutputFormat) -> String {
    match format {
        OutputFormat::Table => render_m2_table(records),
        OutputFormat::Csv => render_m2_csv(records),
    }
}

pub fn render_m3(records: &[M3Record], format: OutputFormat) -> String {
    match format {
        OutputFormat::Table => render_m3_table(records),
        OutputFormat::Csv => render_m3_csv(records),
    }
}

pub fn render_m4(records: &[M4Record], format: OutputFormat) -> String {
    match format {
        OutputFormat::Table => render_m4_table(records),
        OutputFormat::Csv => render_m4_csv(records),
    }
}

pub fn render_v2(records: &[V2Record], format: OutputFormat) -> String {
    match format {
        OutputFormat::Table => render_v2_table(records),
        OutputFormat::Csv => render_v2_csv(records),
    }
}

pub fn render_sweep(records: &[SweepRecord], format: OutputFormat) -> String {
    match format {
        OutputFormat::Table => render_sweep_table(records),
        OutputFormat::Csv => render_sweep_csv(records),
    }
}

pub fn render_tape(trades: &[Trade]) -> String {
    let mut out = String::new();
    out.push_str("tick,good,buyer,seller,price,qty\n");
    for trade in trades {
        out.push_str(&format!(
            "{},{},{},{},{},{}\n",
            trade.tick,
            good_name(trade.good),
            trade.buyer,
            trade.seller,
            trade.price.0,
            trade.qty
        ));
    }
    out
}

pub fn render_barter_tape(trades: &[BarterTrade]) -> String {
    let mut out = String::new();
    out.push_str("tick,a,b,a_gives,b_gives,qty,a_reason,b_reason\n");
    for trade in trades {
        out.push_str(&format!(
            "{},{},{},{},{},{},{},{}\n",
            trade.tick,
            trade.a,
            trade.b,
            good_name(trade.a_gives),
            good_name(trade.b_gives),
            trade.qty,
            barter_reason_name(trade.a_reason),
            barter_reason_name(trade.b_reason)
        ));
    }
    out
}

pub fn render_saleability_tape(snapshots: &[SaleabilitySnapshot]) -> String {
    let mut sorted = snapshots.to_vec();
    sorted.sort_by_key(|snapshot| (snapshot.tick, snapshot.good));

    let mut out = String::new();
    out.push_str("tick,good,acceptances,acceptance_share_bps,medium_share_bps,acceptor_agents,counterpart_goods,eligible,winner\n");
    for snapshot in sorted {
        out.push_str(&format!(
            "{},{},{},{},{},{},{},{},{}\n",
            snapshot.tick,
            good_name(snapshot.good),
            snapshot.acceptances,
            snapshot.acceptance_share_bps,
            snapshot.medium_share_bps,
            snapshot.acceptor_agents,
            snapshot.counterpart_goods,
            snapshot.eligible,
            snapshot.winner
        ));
    }
    out
}

pub fn render_loan_tape(trades: &[LoanTrade]) -> String {
    let mut out = String::new();
    out.push_str("tick,lender,borrower,present,future_due,horizon,debt,purpose,project,funding,lender_party\n");
    for trade in trades {
        let project = trade
            .project
            .map(|project| project.0.to_string())
            .unwrap_or_default();
        let lender = trade
            .lender
            .agent()
            .map(|agent| agent.to_string())
            .unwrap_or_default();
        out.push_str(&format!(
            "{},{},{},{},{},{},{},{},{},{},{}\n",
            trade.tick,
            lender,
            trade.borrower,
            trade.present.0,
            trade.future_due.0,
            trade.horizon,
            trade.debt.0,
            trade.purpose,
            project,
            trade.funding,
            trade.lender
        ));
    }
    out
}

pub fn render_labor_tape(trades: &[LaborTrade]) -> String {
    let mut out = String::new();
    out.push_str("tick,employer,worker,wage,qty,project\n");
    for trade in trades {
        out.push_str(&format!(
            "{},{},{},{},{},{}\n",
            trade.tick, trade.employer, trade.worker, trade.wage.0, trade.qty, trade.project.0
        ));
    }
    out
}

pub fn render_money_tape(records: &[MoneyAuditRecord]) -> String {
    let mut sorted = records.to_vec();
    sorted.sort_by_key(|record| (record.tick, record.agent));

    let mut out = String::new();
    out.push_str("tick,agent,public_specie,public_fiat,demand_claims,spendable_money\n");
    for record in sorted {
        out.push_str(&format!(
            "{},{},{},{},{},{}\n",
            record.tick,
            record.agent,
            record.public_specie.0,
            record.public_fiat.0,
            record.demand_claims.0,
            record.spendable_money.0
        ));
    }
    out
}

pub fn render_payment_tape(records: &[PaymentAuditRecord]) -> String {
    let mut out = String::new();
    out.push_str("tick,kind,from,to,amount,public_fiat,demand_claims,public_specie,tender\n");
    for record in records {
        out.push_str(&format!(
            "{},{},{},{},{},{},{},{},{}\n",
            record.tick,
            payment_kind_name(record.kind),
            record.from,
            record.to,
            record.amount.0,
            record.public_fiat.0,
            record.demand_claims.0,
            record.public_specie.0,
            public_spot_tender_name(record.tender)
        ));
    }
    out
}

pub fn render_wage_payment_tape(records: &[WagePaymentAuditRecord]) -> String {
    let mut out = String::new();
    out.push_str("tick,project,employer,worker,wage,qty,amount,public_fiat,demand_claims,public_specie,tender\n");
    for record in records {
        out.push_str(&format!(
            "{},{},{},{},{},{},{},{},{},{},{}\n",
            record.tick,
            record.project.0,
            record.employer,
            record.worker,
            record.wage.0,
            record.qty,
            record.amount.0,
            record.public_fiat.0,
            record.demand_claims.0,
            record.public_specie.0,
            labor_wage_tender_name(record.tender)
        ));
    }
    out
}

pub fn render_debt_payment_tape(records: &[DebtPaymentAuditRecord]) -> String {
    let mut out = String::new();
    out.push_str("tick,debt,from,to,owed,paid,remaining,public_fiat,demand_claims,public_specie,tender,state\n");
    for record in records {
        out.push_str(&format!(
            "{},{},{},{},{},{},{},{},{},{},{},{}\n",
            record.tick,
            record.debt,
            record.from,
            record.to,
            record.owed.0,
            record.paid.0,
            record.remaining.0,
            record.public_fiat.0,
            record.demand_claims.0,
            record.public_specie.0,
            public_debt_tender_name(record.tender),
            debt_payment_state_name(record.state)
        ));
    }
    out
}

pub fn render_bank_repayment_tape(records: &[BankRepaymentAuditRecord]) -> String {
    let mut out = String::new();
    out.push_str("tick,debt,borrower,bank,owed,paid,remaining,public_fiat,demand_claims,public_specie,credit_retired,tender,state\n");
    for record in records {
        out.push_str(&format!(
            "{},{},{},{},{},{},{},{},{},{},{},{},{}\n",
            record.tick,
            record.debt,
            record.borrower,
            record.bank.0,
            record.owed.0,
            record.paid.0,
            record.remaining.0,
            record.public_fiat.0,
            record.demand_claims.0,
            record.public_specie.0,
            record.credit_retired.0,
            bank_repayment_tender_name(record.tender),
            debt_payment_state_name(record.state)
        ));
    }
    out
}

pub fn render_issuer_repayment_tape(records: &[IssuerRepaymentAuditRecord]) -> String {
    let mut sorted = records.to_vec();
    sorted.sort_by_key(|record| (record.tick, record.debt, record.borrower, record.issuer));

    let mut out = String::new();
    out.push_str("tick,debt,borrower,issuer,owed,paid,remaining,public_fiat,public_specie,credit_retired,tender,state\n");
    for record in sorted {
        out.push_str(&format!(
            "{},{},{},{},{},{},{},{},{},{},{},{}\n",
            record.tick,
            record.debt,
            record.borrower,
            record.issuer.0,
            record.owed.0,
            record.paid.0,
            record.remaining.0,
            record.public_fiat.0,
            record.public_specie.0,
            record.credit_retired.0,
            issuer_repayment_tender_name(record.tender),
            debt_payment_state_name(record.state)
        ));
    }
    out
}

pub fn render_tax_tape(records: &[TaxAuditRecord]) -> String {
    let mut sorted = records.to_vec();
    sorted.sort_by_key(|record| (record.tick, record.debt, record.agent, record.issuer));

    let mut out = String::new();
    out.push_str(
        "tick,debt,agent,issuer,owed,paid,remaining,paid_fiat,paid_specie,receivability,state\n",
    );
    for record in sorted {
        out.push_str(&format!(
            "{},{},{},{},{},{},{},{},{},{},{}\n",
            record.tick,
            record.debt,
            record.agent,
            record.issuer.0,
            record.owed.0,
            record.paid.0,
            record.remaining.0,
            record.paid_fiat.0,
            record.paid_specie.0,
            tax_receivability_name(record.receivability),
            debt_payment_state_name(record.state)
        ));
    }
    out
}

fn payment_kind_name(kind: PaymentKind) -> &'static str {
    match kind {
        PaymentKind::Spot => "spot",
    }
}

fn debt_payment_state_name(state: DebtPaymentState) -> &'static str {
    match state {
        DebtPaymentState::Settled => "settled",
        DebtPaymentState::Defaulted => "defaulted",
    }
}

pub fn render_bank_tape(records: &[BankAuditRecord]) -> String {
    let mut sorted = records.to_vec();
    sorted.sort_by_key(|record| (record.tick, record.bank));

    let mut out = String::new();
    out.push_str("tick,bank,reserves,demand_deposits,time_deposits,loans_outstanding,fiduciary_issued,reserve_ratio_bps,convertible,policy_enabled,policy_max_new_fiduciary_per_tick\n");
    for record in sorted {
        out.push_str(&format!(
            "{},{},{},{},{},{},{},{},{},{},{}\n",
            record.tick,
            record.bank.0,
            record.reserves.0,
            record.demand_deposits.0,
            record.time_deposits.0,
            record.loans_outstanding.0,
            record.fiduciary_issued.0,
            record.reserve_ratio_bps.0,
            record.convertible,
            record.policy_enabled,
            record.policy_max_new_fiduciary_per_tick.0
        ));
    }
    out
}

pub fn render_redemption_tape(records: &[RedemptionAuditRecord]) -> String {
    let mut out = String::new();
    out.push_str("tick,bank,agent,requested,honored,failed,outcome\n");
    for record in records {
        out.push_str(&format!(
            "{},{},{},{},{},{},{}\n",
            record.tick,
            record.bank.0,
            record.agent,
            record.requested.0,
            record.honored.0,
            record.failed.0,
            redemption_outcome_name(record.outcome)
        ));
    }
    out
}

fn redemption_outcome_name(outcome: RedemptionOutcome) -> &'static str {
    match outcome {
        RedemptionOutcome::Honored => "honored",
        RedemptionOutcome::PartiallyHonored => "partial",
        RedemptionOutcome::ReserveExhausted => "reserve-exhausted",
        RedemptionOutcome::Suspended => "suspended",
        RedemptionOutcome::NoClaim => "no-claim",
        RedemptionOutcome::BankMissing => "bank-missing",
    }
}

fn render_table(records: &[Record]) -> String {
    let mut out = String::new();
    push_table_row(
        &mut out,
        &TABLE_COLUMNS
            .iter()
            .map(|(label, _)| (*label).to_string())
            .collect::<Vec<_>>(),
    );
    for r in records {
        push_table_row(&mut out, &table_record_values(r));
    }
    out
}

fn push_table_row(out: &mut String, values: &[String]) {
    for (index, ((label, min_width), value)) in TABLE_COLUMNS.iter().zip(values).enumerate() {
        if index > 0 {
            out.push(' ');
        }
        let width = (*min_width).max(label.len());
        out.push_str(&format!("{value:>width$}"));
    }
    out.push('\n');
}

fn table_record_values(record: &Record) -> Vec<String> {
    vec![
        record.tick.to_string(),
        record.food.to_string(),
        record.wood.to_string(),
        record.nets.to_string(),
        record.labor_used.to_string(),
        record.leisure_taken.to_string(),
        record.food_consumed.to_string(),
        record.hunger_deficit.to_string(),
        record.active_projects.to_string(),
        record.completed_projects.to_string(),
        record.abandoned_projects.to_string(),
        record.capital_labor_consumed.to_string(),
        record.capital_goods_consumed.to_string(),
        record.gather_actions.to_string(),
        record.cut_wood_actions.to_string(),
        record.fish_actions.to_string(),
        record.project_actions.to_string(),
    ]
}

fn render_csv(records: &[Record]) -> String {
    let mut out = String::new();
    out.push_str("tick,food,wood,nets,labor_used,leisure_taken,food_consumed,hunger_deficit,active_projects,completed_projects,abandoned_projects,capital_labor_consumed,capital_goods_consumed,gather_actions,cut_wood_actions,fish_actions,project_actions\n");
    for r in records {
        out.push_str(&format!(
            "{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{}\n",
            r.tick,
            r.food,
            r.wood,
            r.nets,
            r.labor_used,
            r.leisure_taken,
            r.food_consumed,
            r.hunger_deficit,
            r.active_projects,
            r.completed_projects,
            r.abandoned_projects,
            r.capital_labor_consumed,
            r.capital_goods_consumed,
            r.gather_actions,
            r.cut_wood_actions,
            r.fish_actions,
            r.project_actions,
        ));
    }
    out
}

const MARKET_TABLE_COLUMNS: [(&str, usize); 12] = [
    ("tick", 4),
    ("gold", 4),
    ("trades", 6),
    ("food_vol", 8),
    ("wood_vol", 8),
    ("net_vol", 7),
    ("food_px", 7),
    ("wood_px", 7),
    ("net_px", 6),
    ("bids", 4),
    ("asks", 4),
    ("expired", 7),
];

fn render_market_table(records: &[MarketRecord]) -> String {
    let mut out = String::new();
    push_market_table_row(
        &mut out,
        &MARKET_TABLE_COLUMNS
            .iter()
            .map(|(label, _)| (*label).to_string())
            .collect::<Vec<_>>(),
    );
    for record in records {
        push_market_table_row(&mut out, &market_table_record_values(record));
    }
    out
}

fn push_market_table_row(out: &mut String, values: &[String]) {
    for (index, ((label, min_width), value)) in MARKET_TABLE_COLUMNS.iter().zip(values).enumerate()
    {
        if index > 0 {
            out.push(' ');
        }
        let width = (*min_width).max(label.len());
        out.push_str(&format!("{value:>width$}"));
    }
    out.push('\n');
}

fn market_table_record_values(record: &MarketRecord) -> Vec<String> {
    vec![
        record.tick.to_string(),
        record.total_gold.0.to_string(),
        record.trades.to_string(),
        record.food_volume.to_string(),
        record.wood_volume.to_string(),
        record.net_volume.to_string(),
        gold_or_dash(record.last_food_price),
        gold_or_dash(record.last_wood_price),
        gold_or_dash(record.last_net_price),
        record.bid_count.to_string(),
        record.ask_count.to_string(),
        record.expired_orders.to_string(),
    ]
}

fn render_market_csv(records: &[MarketRecord]) -> String {
    let mut out = String::new();
    out.push_str("tick,total_gold,trades,food_volume,wood_volume,net_volume,last_food_price,last_wood_price,last_net_price,bid_count,ask_count,expired_orders\n");
    for record in records {
        out.push_str(&format!(
            "{},{},{},{},{},{},{},{},{},{},{},{}\n",
            record.tick,
            record.total_gold.0,
            record.trades,
            record.food_volume,
            record.wood_volume,
            record.net_volume,
            csv_gold(record.last_food_price),
            csv_gold(record.last_wood_price),
            csv_gold(record.last_net_price),
            record.bid_count,
            record.ask_count,
            record.expired_orders,
        ));
    }
    out
}

const V2_TABLE_COLUMNS: [(&str, usize); 13] = [
    ("tick", 4),
    ("phase", 6),
    ("money", 6),
    ("promote", 7),
    ("barter", 6),
    ("spot", 4),
    ("lead", 6),
    ("share", 5),
    ("runner", 6),
    ("units", 5),
    ("bids", 4),
    ("asks", 4),
    ("expired", 7),
];

fn render_v2_table(records: &[V2Record]) -> String {
    let mut out = String::new();
    push_v2_table_row(
        &mut out,
        &V2_TABLE_COLUMNS
            .iter()
            .map(|(label, _)| (*label).to_string())
            .collect::<Vec<_>>(),
    );
    for record in records {
        push_v2_table_row(&mut out, &v2_table_record_values(record));
    }
    out
}

fn push_v2_table_row(out: &mut String, values: &[String]) {
    for (index, ((label, min_width), value)) in V2_TABLE_COLUMNS.iter().zip(values).enumerate() {
        if index > 0 {
            out.push(' ');
        }
        let width = (*min_width).max(label.len());
        out.push_str(&format!("{value:>width$}"));
    }
    out.push('\n');
}

fn v2_table_record_values(record: &V2Record) -> Vec<String> {
    vec![
        record.tick.to_string(),
        v2_phase_name(record.phase).to_string(),
        good_or_dash(record.money_good),
        record.promoted_this_tick.to_string(),
        record.barter_trades.to_string(),
        record.spot_trades.to_string(),
        good_or_dash(record.candidate_good),
        u16_or_dash(record.candidate_share_bps),
        u16_or_dash(record.runner_up_share_bps),
        record.total_money_units.0.to_string(),
        record.bid_count.to_string(),
        record.ask_count.to_string(),
        record.expired_orders.to_string(),
    ]
}

fn render_v2_csv(records: &[V2Record]) -> String {
    let mut out = String::new();
    out.push_str("tick,phase,money_good,promoted_this_tick,barter_trades,spot_trades,candidate_good,candidate_share_bps,runner_up_share_bps,total_money_units,bid_count,ask_count,expired_orders\n");
    for record in records {
        out.push_str(&format!(
            "{},{},{},{},{},{},{},{},{},{},{},{},{}\n",
            record.tick,
            v2_phase_name(record.phase),
            csv_good(record.money_good),
            record.promoted_this_tick,
            record.barter_trades,
            record.spot_trades,
            csv_good(record.candidate_good),
            csv_u16(record.candidate_share_bps),
            csv_u16(record.runner_up_share_bps),
            record.total_money_units.0,
            record.bid_count,
            record.ask_count,
            record.expired_orders,
        ));
    }
    out
}

fn gold_or_dash(gold: Option<Gold>) -> String {
    gold.map(|gold| gold.0.to_string())
        .unwrap_or_else(|| "-".to_string())
}

fn csv_gold(gold: Option<Gold>) -> String {
    gold.map(|gold| gold.0.to_string()).unwrap_or_default()
}

fn good_or_dash(good: Option<GoodId>) -> String {
    good.map(good_name)
        .map(str::to_string)
        .unwrap_or_else(|| "-".to_string())
}

fn csv_good(good: Option<GoodId>) -> &'static str {
    good.map(good_name).unwrap_or_default()
}

fn u16_or_dash(value: Option<u16>) -> String {
    value
        .map(|value| value.to_string())
        .unwrap_or_else(|| "-".to_string())
}

fn csv_u16(value: Option<u16>) -> String {
    value.map(|value| value.to_string()).unwrap_or_default()
}

fn v2_phase_name(phase: V2Phase) -> &'static str {
    match phase {
        V2Phase::Barter => "barter",
        V2Phase::Money => "money",
    }
}

fn barter_reason_name(reason: BarterReason) -> String {
    match reason {
        BarterReason::DirectWant => "direct".to_string(),
        BarterReason::IndirectFor { target } => format!("indirect_for:{}", good_name(target)),
    }
}

const M2_TABLE_COLUMNS: [(&str, usize); 30] = [
    ("tick", 4),
    ("gold", 4),
    ("spot", 4),
    ("labor", 5),
    ("loans", 5),
    ("proj_loans", 10),
    ("proj_borr", 9),
    ("open", 4),
    ("settled", 7),
    ("default", 7),
    ("proj_open", 9),
    ("proj_set", 8),
    ("proj_def", 8),
    ("proj_res", 8),
    ("active", 6),
    ("waiting", 7),
    ("mature", 6),
    ("sold", 4),
    ("drop", 4),
    ("labor_adv", 9),
    ("wages", 5),
    ("revenue", 7),
    ("profit", 6),
    ("lost_labor", 10),
    ("lost_goods", 10),
    ("gold_loss", 9),
    ("mkt_bps", 7),
    ("nat_bps", 7),
    ("gap_bps", 7),
    ("len_x100", 8),
];

fn render_m2_table(records: &[M2Record]) -> String {
    let mut out = String::new();
    push_m2_table_row(
        &mut out,
        &M2_TABLE_COLUMNS
            .iter()
            .map(|(label, _)| (*label).to_string())
            .collect::<Vec<_>>(),
    );
    for record in records {
        push_m2_table_row(&mut out, &m2_table_record_values(record));
    }
    out
}

fn push_m2_table_row(out: &mut String, values: &[String]) {
    for (index, ((label, min_width), value)) in M2_TABLE_COLUMNS.iter().zip(values).enumerate() {
        if index > 0 {
            out.push(' ');
        }
        let width = (*min_width).max(label.len());
        out.push_str(&format!("{value:>width$}"));
    }
    out.push('\n');
}

fn m2_table_record_values(record: &M2Record) -> Vec<String> {
    vec![
        record.tick.to_string(),
        record.total_gold.0.to_string(),
        record.spot_trades.to_string(),
        record.labor_trades.to_string(),
        record.loan_trades.to_string(),
        record.project_loan_trades.to_string(),
        record.project_borrowed_gold.0.to_string(),
        record.debts_open.to_string(),
        record.debts_settled.to_string(),
        record.debts_defaulted.to_string(),
        record.project_debts_open.to_string(),
        record.project_debts_settled.to_string(),
        record.project_debts_defaulted.to_string(),
        record.project_funding_reserved_gold.0.to_string(),
        record.active_projects.to_string(),
        record.waiting_projects.to_string(),
        record.mature_projects.to_string(),
        record.sold_projects.to_string(),
        record.abandoned_projects.to_string(),
        record.labor_advanced.to_string(),
        record.wages_paid.0.to_string(),
        record.project_revenue.0.to_string(),
        record.project_profit.to_string(),
        record.capital_labor_consumed.to_string(),
        record.capital_goods_consumed.to_string(),
        record.capital_gold_loss.0.to_string(),
        i64_or_dash(record.market_rate_bps),
        i64_or_dash(record.natural_rate_proxy_bps),
        i64_or_dash(record.rate_gap_bps),
        record.structure_length_ticks_x100.to_string(),
    ]
}

fn render_m2_csv(records: &[M2Record]) -> String {
    let mut out = String::new();
    out.push_str("tick,total_gold,spot_trades,labor_trades,loan_trades,project_loan_trades,project_borrowed_gold,debts_open,debts_settled,debts_defaulted,project_debts_open,project_debts_settled,project_debts_defaulted,project_funding_reserved_gold,active_projects,waiting_projects,mature_projects,sold_projects,abandoned_projects,labor_advanced,wages_paid,project_revenue,project_profit,capital_labor_consumed,capital_goods_consumed,capital_gold_loss,market_rate_bps,natural_rate_proxy_bps,rate_gap_bps,structure_length_ticks_x100\n");
    for record in records {
        out.push_str(&format!(
            "{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{}\n",
            record.tick,
            record.total_gold.0,
            record.spot_trades,
            record.labor_trades,
            record.loan_trades,
            record.project_loan_trades,
            record.project_borrowed_gold.0,
            record.debts_open,
            record.debts_settled,
            record.debts_defaulted,
            record.project_debts_open,
            record.project_debts_settled,
            record.project_debts_defaulted,
            record.project_funding_reserved_gold.0,
            record.active_projects,
            record.waiting_projects,
            record.mature_projects,
            record.sold_projects,
            record.abandoned_projects,
            record.labor_advanced,
            record.wages_paid.0,
            record.project_revenue.0,
            record.project_profit,
            record.capital_labor_consumed,
            record.capital_goods_consumed,
            record.capital_gold_loss.0,
            csv_i64(record.market_rate_bps),
            csv_i64(record.natural_rate_proxy_bps),
            csv_i64(record.rate_gap_bps),
            record.structure_length_ticks_x100,
        ));
    }
    out
}

const M3_TABLE_COLUMNS: [(&str, usize); 33] = [
    ("tick", 4),
    ("regime", 10),
    ("gold", 4),
    ("spot", 4),
    ("labor", 5),
    ("loans", 5),
    ("specie", 6),
    ("fiat", 4),
    ("claims", 6),
    ("reserves", 8),
    ("fiduc", 5),
    ("time_dep", 8),
    ("tms", 4),
    ("bank_iss", 8),
    ("fiat_cr", 7),
    ("fiat_fisc", 9),
    ("retired", 7),
    ("bank_ln", 7),
    ("fiat_ln", 7),
    ("mkt_bps", 7),
    ("sh_nat", 6),
    ("sh_gap", 6),
    ("len_x100", 8),
    ("starts", 6),
    ("bust", 4),
    ("drop", 4),
    ("lost_lab", 8),
    ("lost_gds", 8),
    ("settled", 7),
    ("default", 7),
    ("proj_def", 8),
    ("early", 6),
    ("late", 5),
];

fn render_m3_table(records: &[M3Record]) -> String {
    let mut out = String::new();
    push_m3_table_row(
        &mut out,
        &M3_TABLE_COLUMNS
            .iter()
            .map(|(label, _)| (*label).to_string())
            .collect::<Vec<_>>(),
    );
    for record in records {
        push_m3_table_row(&mut out, &m3_table_record_values(record));
    }
    out
}

fn push_m3_table_row(out: &mut String, values: &[String]) {
    for (index, ((label, min_width), value)) in M3_TABLE_COLUMNS.iter().zip(values).enumerate() {
        if index > 0 {
            out.push(' ');
        }
        let width = (*min_width).max(label.len());
        out.push_str(&format!("{value:>width$}"));
    }
    out.push('\n');
}

fn m3_table_record_values(record: &M3Record) -> Vec<String> {
    vec![
        record.m2.tick.to_string(),
        regime_name(record.regime).to_string(),
        record.m2.total_gold.0.to_string(),
        record.m2.spot_trades.to_string(),
        record.m2.labor_trades.to_string(),
        record.m2.loan_trades.to_string(),
        record.public_specie.0.to_string(),
        record.public_fiat.0.to_string(),
        record.demand_claims.0.to_string(),
        record.bank_reserves.0.to_string(),
        record.fiduciary.0.to_string(),
        record.time_deposits.0.to_string(),
        record.tms.0.to_string(),
        record.bank_credit_issued.0.to_string(),
        record.fiat_credit_issued.0.to_string(),
        record.fiat_fiscal_issued.0.to_string(),
        record.credit_retired.0.to_string(),
        record.bank_loan_trades.to_string(),
        record.fiat_loan_trades.to_string(),
        i64_or_dash(record.m2.market_rate_bps),
        i64_or_dash(record.shadow_natural_rate_bps),
        i64_or_dash(record.shadow_rate_gap_bps),
        record.m2.structure_length_ticks_x100.to_string(),
        record.boom_projects_started.to_string(),
        record.bust_abandoned_projects.to_string(),
        record.m2.abandoned_projects.to_string(),
        record.m2.capital_labor_consumed.to_string(),
        record.m2.capital_goods_consumed.to_string(),
        record.m2.debts_settled.to_string(),
        record.m2.debts_defaulted.to_string(),
        record.m2.project_debts_defaulted.to_string(),
        record.early_receiver_wealth_delta.to_string(),
        record.late_receiver_wealth_delta.to_string(),
    ]
}

fn render_m3_csv(records: &[M3Record]) -> String {
    let mut out = String::new();
    out.push_str(M3_CSV_HEADER);
    out.push('\n');
    for record in records {
        out.push_str(&format!(
            "{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{}\n",
            record.m2.tick,
            regime_name(record.regime),
            record.m2.total_gold.0,
            record.m2.spot_trades,
            record.m2.labor_trades,
            record.m2.loan_trades,
            record.public_specie.0,
            record.public_fiat.0,
            record.demand_claims.0,
            record.bank_reserves.0,
            record.fiduciary.0,
            record.time_deposits.0,
            record.tms.0,
            record.bank_credit_issued.0,
            record.fiat_credit_issued.0,
            record.fiat_fiscal_issued.0,
            record.credit_retired.0,
            record.bank_loan_trades,
            record.fiat_loan_trades,
            csv_i64(record.m2.market_rate_bps),
            csv_i64(record.shadow_natural_rate_bps),
            csv_i64(record.shadow_rate_gap_bps),
            record.m2.structure_length_ticks_x100,
            record.boom_projects_started,
            record.bust_abandoned_projects,
            record.m2.abandoned_projects,
            record.m2.capital_labor_consumed,
            record.m2.capital_goods_consumed,
            record.m2.debts_settled,
            record.m2.debts_defaulted,
            record.m2.project_debts_defaulted,
            record.early_receiver_wealth_delta,
            record.late_receiver_wealth_delta,
        ));
    }
    out
}

const SWEEP_TABLE_COLUMNS: [(&str, usize); 14] = [
    ("scenario", 22),
    ("variables", 28),
    ("tms", 4),
    ("fiduc", 5),
    ("fiat", 4),
    ("bank_iss", 8),
    ("fiat_cr", 7),
    ("fiat_fisc", 9),
    ("retired", 7),
    ("max_gap", 7),
    ("max_len", 7),
    ("bust", 4),
    ("gini", 5),
    ("idle", 5),
];

fn render_sweep_table(records: &[SweepRecord]) -> String {
    let mut out = String::new();
    let header = SWEEP_TABLE_COLUMNS
        .iter()
        .map(|(label, _)| (*label).to_string())
        .collect::<Vec<_>>();
    let rows = records
        .iter()
        .map(sweep_table_record_values)
        .collect::<Vec<_>>();
    let widths = sweep_table_widths(&header, &rows);

    push_sweep_table_row(&mut out, &header, &widths);
    for row in &rows {
        push_sweep_table_row(&mut out, row, &widths);
    }
    out
}

fn sweep_table_widths(header: &[String], rows: &[Vec<String>]) -> Vec<usize> {
    SWEEP_TABLE_COLUMNS
        .iter()
        .enumerate()
        .map(|(index, (label, min_width))| {
            let row_width = rows
                .iter()
                .filter_map(|row| row.get(index))
                .map(String::len)
                .max()
                .unwrap_or(0);
            (*min_width)
                .max(label.len())
                .max(header.get(index).map(String::len).unwrap_or(0))
                .max(row_width)
        })
        .collect()
}

fn push_sweep_table_row(out: &mut String, values: &[String], widths: &[usize]) {
    for (index, (value, width)) in values.iter().zip(widths).enumerate() {
        if index > 0 {
            out.push(' ');
        }
        let width = *width;
        out.push_str(&format!("{value:>width$}"));
    }
    out.push('\n');
}

fn sweep_table_record_values(record: &SweepRecord) -> Vec<String> {
    vec![
        record.scenario.to_string(),
        sweep_variables(&record.variables),
        record.final_tms.0.to_string(),
        record.final_fiduciary.0.to_string(),
        record.final_public_fiat.0.to_string(),
        record.total_bank_credit_issued.0.to_string(),
        record.total_fiat_credit_issued.0.to_string(),
        record.total_fiat_fiscal_issued.0.to_string(),
        record.total_credit_retired.0.to_string(),
        i64_or_dash(record.max_shadow_rate_gap_bps),
        record.max_structure_length_ticks_x100.to_string(),
        record.total_bust_abandoned_projects.to_string(),
        u32_or_dash(record.final_real_wealth_gini_bps),
        u32_or_dash(record.max_idle_labor_bps),
    ]
}

fn render_sweep_csv(records: &[SweepRecord]) -> String {
    let mut out = String::new();
    out.push_str("scenario,seed,periods,variables,final_tms,final_fiduciary,final_public_fiat,total_bank_credit_issued,total_fiat_credit_issued,total_fiat_fiscal_issued,total_credit_retired,max_shadow_rate_gap_bps,max_structure_length_ticks_x100,total_bust_abandoned_projects,final_abandoned_projects,final_debts_defaulted,final_project_debts_defaulted,final_capital_labor_consumed,final_capital_goods_consumed,final_real_wealth_gini_bps,final_early_late_real_wealth_gap,max_idle_labor_bps,max_sector_price_dispersion_bps\n");
    for record in records {
        out.push_str(&format!(
            "{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{}\n",
            record.scenario,
            record.seed,
            record.periods,
            sweep_variables(&record.variables),
            record.final_tms.0,
            record.final_fiduciary.0,
            record.final_public_fiat.0,
            record.total_bank_credit_issued.0,
            record.total_fiat_credit_issued.0,
            record.total_fiat_fiscal_issued.0,
            record.total_credit_retired.0,
            csv_i64(record.max_shadow_rate_gap_bps),
            record.max_structure_length_ticks_x100,
            record.total_bust_abandoned_projects,
            record.final_abandoned_projects,
            record.final_debts_defaulted,
            record.final_project_debts_defaulted,
            record.final_capital_labor_consumed,
            record.final_capital_goods_consumed,
            csv_u32(record.final_real_wealth_gini_bps),
            record.final_early_late_real_wealth_gap,
            csv_u32(record.max_idle_labor_bps),
            csv_u64(record.max_sector_price_dispersion_bps),
        ));
    }
    out
}

fn sweep_variables(variables: &[(SweepKey, u64)]) -> String {
    let mut out = String::new();
    for (index, (key, value)) in variables.iter().enumerate() {
        if index > 0 {
            out.push(';');
        }
        out.push_str(key.as_str());
        out.push('=');
        out.push_str(&value.to_string());
    }
    out
}

const M4_TABLE_COLUMNS: [(&str, usize); 22] = [
    ("tick", 4),
    ("regime", 10),
    ("tms", 4),
    ("sh_gap", 6),
    ("len_x100", 8),
    ("active", 6),
    ("bust", 4),
    ("lost_lab", 8),
    ("defaults", 8),
    ("gini", 5),
    ("bottom50", 8),
    ("top10", 5),
    ("early_rw", 8),
    ("late_rw", 7),
    ("non_rw", 6),
    ("e_l_gap", 7),
    ("early_rd", 8),
    ("late_rd", 7),
    ("px_disp", 7),
    ("sec_spot", 8),
    ("idle_bps", 8),
    ("unpriced", 8),
];

const M4_CSV_COLUMNS: &[&str] = &[
    "tick",
    "regime",
    "tms",
    "public_specie",
    "public_fiat",
    "demand_claims",
    "fiduciary",
    "bank_credit_issued",
    "fiat_credit_issued",
    "fiat_fiscal_issued",
    "credit_retired",
    "market_rate_bps",
    "shadow_natural_rate_bps",
    "shadow_rate_gap_bps",
    "structure_length_ticks_x100",
    "active_projects",
    "waiting_projects",
    "mature_projects",
    "sold_projects",
    "abandoned_projects",
    "bust_abandoned_projects",
    "capital_labor_consumed",
    "capital_goods_consumed",
    "debts_defaulted",
    "project_debts_defaulted",
    "agent_count",
    "early_receiver_count",
    "late_receiver_count",
    "non_receiver_count",
    "real_wealth_gini_bps",
    "lorenz_bottom_50_share_bps",
    "lorenz_top_10_share_bps",
    "early_receiver_mean_real_wealth",
    "late_receiver_mean_real_wealth",
    "non_receiver_mean_real_wealth",
    "early_late_real_wealth_gap",
    "early_receiver_mean_realized_delta",
    "late_receiver_mean_realized_delta",
    "tick_price_dispersion_bps",
    "cumulative_price_dispersion_bps",
    "tick_sector_price_dispersion_bps",
    "cumulative_sector_price_dispersion_bps",
    "unpriced_stock_units",
    "labor_capacity",
    "labor_used",
    "idle_labor_units",
    "idle_labor_bps",
];

fn render_m4_table(records: &[M4Record]) -> String {
    let mut out = String::new();
    push_m4_table_row(
        &mut out,
        &M4_TABLE_COLUMNS
            .iter()
            .map(|(label, _)| (*label).to_string())
            .collect::<Vec<_>>(),
    );
    for record in records {
        push_m4_table_row(&mut out, &m4_table_record_values(record));
    }
    if m4_table_needs_sector_note(records) {
        out.push_str("# sec_spot blank: fewer than two non-money spot-good sectors traded; ");
        out.push_str("factor/project prices are not imputed.\n");
    }
    out
}

fn m4_table_needs_sector_note(records: &[M4Record]) -> bool {
    !records.is_empty()
        && records.iter().all(|record| {
            record.tick_sector_price_dispersion_bps.is_none()
                && record.cumulative_sector_price_dispersion_bps.is_none()
        })
}

fn push_m4_table_row(out: &mut String, values: &[String]) {
    for (index, ((label, min_width), value)) in M4_TABLE_COLUMNS.iter().zip(values).enumerate() {
        if index > 0 {
            out.push(' ');
        }
        let width = (*min_width).max(label.len());
        out.push_str(&format!("{value:>width$}"));
    }
    out.push('\n');
}

fn m4_table_record_values(record: &M4Record) -> Vec<String> {
    vec![
        record.tick.to_string(),
        regime_name(record.regime).to_string(),
        record.tms.0.to_string(),
        i64_or_dash(record.shadow_rate_gap_bps),
        record.structure_length_ticks_x100.to_string(),
        record.active_projects.to_string(),
        record.bust_abandoned_projects.to_string(),
        record.capital_labor_consumed.to_string(),
        record.debts_defaulted.to_string(),
        u32_or_dash(record.real_wealth_gini_bps),
        u32_or_dash(record.lorenz_bottom_50_share_bps),
        u32_or_dash(record.lorenz_top_10_share_bps),
        cohort_mean_or_dash(
            record.early_receiver_count,
            record.early_receiver_mean_real_wealth,
        ),
        cohort_mean_or_dash(
            record.late_receiver_count,
            record.late_receiver_mean_real_wealth,
        ),
        cohort_mean_or_dash(
            record.non_receiver_count,
            record.non_receiver_mean_real_wealth,
        ),
        if record.early_receiver_count == 0 || record.late_receiver_count == 0 {
            "-".to_string()
        } else {
            record.early_late_real_wealth_gap.to_string()
        },
        cohort_mean_or_dash(
            record.early_receiver_count,
            record.early_receiver_mean_realized_delta,
        ),
        cohort_mean_or_dash(
            record.late_receiver_count,
            record.late_receiver_mean_realized_delta,
        ),
        u64_or_dash(record.cumulative_price_dispersion_bps),
        u64_or_dash(record.cumulative_sector_price_dispersion_bps),
        u32_or_dash(record.idle_labor_bps),
        record.unpriced_stock_units.to_string(),
    ]
}

fn render_m4_csv(records: &[M4Record]) -> String {
    let mut out = String::new();
    out.push_str(&M4_CSV_COLUMNS.join(","));
    out.push('\n');
    for record in records {
        out.push_str(&format!(
            "{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{}\n",
            record.tick,
            regime_name(record.regime),
            record.tms.0,
            record.public_specie.0,
            record.public_fiat.0,
            record.demand_claims.0,
            record.fiduciary.0,
            record.bank_credit_issued.0,
            record.fiat_credit_issued.0,
            record.fiat_fiscal_issued.0,
            record.credit_retired.0,
            csv_i64(record.market_rate_bps),
            csv_i64(record.shadow_natural_rate_bps),
            csv_i64(record.shadow_rate_gap_bps),
            record.structure_length_ticks_x100,
            record.active_projects,
            record.waiting_projects,
            record.mature_projects,
            record.sold_projects,
            record.abandoned_projects,
            record.bust_abandoned_projects,
            record.capital_labor_consumed,
            record.capital_goods_consumed,
            record.debts_defaulted,
            record.project_debts_defaulted,
            record.agent_count,
            record.early_receiver_count,
            record.late_receiver_count,
            record.non_receiver_count,
            csv_u32(record.real_wealth_gini_bps),
            csv_u32(record.lorenz_bottom_50_share_bps),
            csv_u32(record.lorenz_top_10_share_bps),
            csv_cohort_i128(
                record.early_receiver_count,
                record.early_receiver_mean_real_wealth,
            ),
            csv_cohort_i128(
                record.late_receiver_count,
                record.late_receiver_mean_real_wealth,
            ),
            csv_cohort_i128(
                record.non_receiver_count,
                record.non_receiver_mean_real_wealth,
            ),
            csv_early_late_gap(record),
            csv_cohort_i128(
                record.early_receiver_count,
                record.early_receiver_mean_realized_delta,
            ),
            csv_cohort_i128(
                record.late_receiver_count,
                record.late_receiver_mean_realized_delta,
            ),
            csv_u64(record.tick_price_dispersion_bps),
            csv_u64(record.cumulative_price_dispersion_bps),
            csv_u64(record.tick_sector_price_dispersion_bps),
            csv_u64(record.cumulative_sector_price_dispersion_bps),
            record.unpriced_stock_units,
            record.labor_capacity,
            record.labor_used,
            record.idle_labor_units,
            csv_u32(record.idle_labor_bps),
        ));
    }
    out
}

fn regime_name(regime: crate::money::Regime) -> &'static str {
    match regime {
        crate::money::Regime::SoundGold => "sound-gold",
        crate::money::Regime::FractionalConvertible => "fractional",
        crate::money::Regime::SuspendedConvertibility => "suspended",
        crate::money::Regime::Fiat => "fiat",
    }
}

fn i64_or_dash(value: Option<i64>) -> String {
    value
        .map(|value| value.to_string())
        .unwrap_or_else(|| "-".to_string())
}

fn u32_or_dash(value: Option<u32>) -> String {
    value
        .map(|value| value.to_string())
        .unwrap_or_else(|| "-".to_string())
}

fn u64_or_dash(value: Option<u64>) -> String {
    value
        .map(|value| value.to_string())
        .unwrap_or_else(|| "-".to_string())
}

fn cohort_mean_or_dash(count: u32, value: i128) -> String {
    if count == 0 {
        "-".to_string()
    } else {
        value.to_string()
    }
}

fn csv_i64(value: Option<i64>) -> String {
    value.map(|value| value.to_string()).unwrap_or_default()
}

fn csv_u32(value: Option<u32>) -> String {
    value.map(|value| value.to_string()).unwrap_or_default()
}

fn csv_u64(value: Option<u64>) -> String {
    value.map(|value| value.to_string()).unwrap_or_default()
}

fn csv_cohort_i128(count: u32, value: i128) -> String {
    if count == 0 {
        String::new()
    } else {
        value.to_string()
    }
}

fn csv_early_late_gap(record: &M4Record) -> String {
    if record.early_receiver_count == 0 || record.late_receiver_count == 0 {
        String::new()
    } else {
        record.early_late_real_wealth_gap.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::{
        render, render_issuer_repayment_tape, render_m4, render_redemption_tape, render_sweep,
        OutputFormat,
    };
    use crate::agent::AgentId;
    use crate::good::Gold;
    use crate::ledger::{BankId, IssuerId};
    use crate::money::IssuerRepaymentTender;
    use crate::record::{
        DebtPaymentState, IssuerRepaymentAuditRecord, M4Record, Record, RedemptionAuditRecord,
        RedemptionOutcome,
    };
    use crate::sweep::{SweepKey, SweepRecord};

    #[test]
    fn csv_render_includes_header_and_record() {
        let output = render(
            &[Record {
                tick: 7,
                food: 2,
                wood: 1,
                nets: 0,
                labor_used: 1,
                leisure_taken: 2,
                food_consumed: 1,
                hunger_deficit: 0,
                active_projects: 1,
                completed_projects: 0,
                abandoned_projects: 0,
                capital_labor_consumed: 0,
                capital_goods_consumed: 0,
                gather_actions: 1,
                cut_wood_actions: 0,
                fish_actions: 0,
                project_actions: 0,
            }],
            OutputFormat::Csv,
        );

        let mut lines = output.lines();
        assert_eq!(
            lines.next(),
            Some("tick,food,wood,nets,labor_used,leisure_taken,food_consumed,hunger_deficit,active_projects,completed_projects,abandoned_projects,capital_labor_consumed,capital_goods_consumed,gather_actions,cut_wood_actions,fish_actions,project_actions")
        );
        assert_eq!(lines.next(), Some("7,2,1,0,1,2,1,0,1,0,0,0,0,1,0,0,0"));
        assert_eq!(lines.next(), None);
    }

    #[test]
    fn table_render_includes_header_and_record() {
        let output = render(
            &[Record {
                tick: 1,
                food: 2,
                ..Record::default()
            }],
            OutputFormat::Table,
        );

        assert!(output.starts_with("tick food wood nets labor leisure"));
        assert!(output.contains("   1    2"));
    }

    #[test]
    fn m4_table_marks_empty_cohort_means_as_dash() {
        let output = render_m4(
            &[M4Record {
                agent_count: 1,
                non_receiver_count: 1,
                ..M4Record::default()
            }],
            OutputFormat::Table,
        );
        let headers = output
            .lines()
            .next()
            .unwrap_or_default()
            .split_whitespace()
            .collect::<Vec<_>>();
        let row = output
            .lines()
            .nth(1)
            .unwrap_or_default()
            .split_whitespace()
            .collect::<Vec<_>>();
        let early = headers
            .iter()
            .position(|header| *header == "early_rw")
            .unwrap();
        let late = headers
            .iter()
            .position(|header| *header == "late_rw")
            .unwrap();
        let gap = headers
            .iter()
            .position(|header| *header == "e_l_gap")
            .unwrap();

        assert_eq!(row[early], "-");
        assert_eq!(row[late], "-");
        assert_eq!(row[gap], "-");
    }

    #[test]
    fn m4_table_includes_non_receiver_and_realized_delta_metrics() {
        let output = render_m4(
            &[M4Record {
                agent_count: 3,
                early_receiver_count: 1,
                late_receiver_count: 1,
                non_receiver_count: 1,
                early_receiver_mean_real_wealth: 11,
                late_receiver_mean_real_wealth: 7,
                non_receiver_mean_real_wealth: 5,
                early_receiver_mean_realized_delta: 3,
                late_receiver_mean_realized_delta: -2,
                early_late_real_wealth_gap: 4,
                ..M4Record::default()
            }],
            OutputFormat::Table,
        );
        let headers = output
            .lines()
            .next()
            .unwrap_or_default()
            .split_whitespace()
            .collect::<Vec<_>>();
        let row = output
            .lines()
            .nth(1)
            .unwrap_or_default()
            .split_whitespace()
            .collect::<Vec<_>>();

        for (header, expected) in [("non_rw", "5"), ("early_rd", "3"), ("late_rd", "-2")] {
            let index = headers
                .iter()
                .position(|candidate| *candidate == header)
                .unwrap();
            assert_eq!(row[index], expected);
        }
    }

    #[test]
    fn redemption_tape_preserves_audit_order() {
        let output = render_redemption_tape(&[
            RedemptionAuditRecord {
                tick: 4,
                bank: BankId(1),
                agent: AgentId(2),
                requested: Gold(1),
                honored: Gold(1),
                failed: Gold::ZERO,
                outcome: RedemptionOutcome::Honored,
            },
            RedemptionAuditRecord {
                tick: 4,
                bank: BankId(1),
                agent: AgentId(1),
                requested: Gold(1),
                honored: Gold::ZERO,
                failed: Gold(1),
                outcome: RedemptionOutcome::ReserveExhausted,
            },
        ]);

        let lines = output.lines().collect::<Vec<_>>();
        assert_eq!(
            lines,
            vec![
                "tick,bank,agent,requested,honored,failed,outcome",
                "4,1,2,1,1,0,honored",
                "4,1,1,1,0,1,reserve-exhausted",
            ]
        );
    }

    #[test]
    fn issuer_repayment_tape_sorts_by_tick_and_debt() {
        let output = render_issuer_repayment_tape(&[
            IssuerRepaymentAuditRecord {
                tick: 3,
                debt: 2,
                borrower: AgentId(20),
                issuer: IssuerId(1),
                owed: Gold(1),
                paid: Gold::ZERO,
                remaining: Gold(1),
                public_fiat: Gold::ZERO,
                public_specie: Gold::ZERO,
                credit_retired: Gold::ZERO,
                tender: IssuerRepaymentTender::FiatRefused,
                state: DebtPaymentState::Defaulted,
            },
            IssuerRepaymentAuditRecord {
                tick: 1,
                debt: 1,
                borrower: AgentId(10),
                issuer: IssuerId(1),
                owed: Gold(1),
                paid: Gold(1),
                remaining: Gold::ZERO,
                public_fiat: Gold(1),
                public_specie: Gold::ZERO,
                credit_retired: Gold(1),
                tender: IssuerRepaymentTender::FiatOnly,
                state: DebtPaymentState::Settled,
            },
        ]);

        let lines = output.lines().collect::<Vec<_>>();
        assert_eq!(
            lines,
            vec![
                "tick,debt,borrower,issuer,owed,paid,remaining,public_fiat,public_specie,credit_retired,tender,state",
                "1,1,10,1,1,1,0,1,0,1,fiat-only,settled",
                "3,2,20,1,1,0,1,0,0,0,fiat-refused,defaulted",
            ]
        );
    }

    #[test]
    fn m4_table_documents_blank_sector_spot_dispersion() {
        let output = render_m4(&[M4Record::default()], OutputFormat::Table);

        assert!(
            output.contains("# sec_spot blank: fewer than two non-money spot-good sectors traded")
        );
    }

    #[test]
    fn sweep_table_widens_variables_column_for_multi_axis_values() {
        let output = render_sweep(
            &[SweepRecord {
                scenario: "fractional-reserve",
                seed: 0,
                periods: 0,
                variables: vec![
                    (SweepKey::BankCreditPerTick, 0),
                    (SweepKey::ReserveRatioBps, 10_000),
                ],
                final_tms: Gold(42),
                final_fiduciary: Gold::ZERO,
                final_public_fiat: Gold::ZERO,
                total_bank_credit_issued: Gold::ZERO,
                total_fiat_credit_issued: Gold::ZERO,
                total_fiat_fiscal_issued: Gold::ZERO,
                total_credit_retired: Gold::ZERO,
                max_shadow_rate_gap_bps: None,
                max_structure_length_ticks_x100: 0,
                total_bust_abandoned_projects: 0,
                final_abandoned_projects: 0,
                final_debts_defaulted: 0,
                final_project_debts_defaulted: 0,
                final_capital_labor_consumed: 0,
                final_capital_goods_consumed: 0,
                final_real_wealth_gini_bps: None,
                final_early_late_real_wealth_gap: 0,
                max_idle_labor_bps: None,
                max_sector_price_dispersion_bps: None,
            }],
            OutputFormat::Table,
        );
        let mut lines = output.lines();
        let header = lines.next().unwrap_or_default();
        let row = lines.next().unwrap_or_default();
        let variables = "bank-credit-per-tick=0;reserve-ratio-bps=10000";
        let variables_end = row.find(variables).unwrap() + variables.len();

        assert!(header.find("tms").unwrap() > variables_end);
    }

    #[test]
    fn m4_csv_blanks_empty_cohort_metrics() {
        let output = render_m4(
            &[M4Record {
                agent_count: 1,
                early_receiver_count: 1,
                early_receiver_mean_real_wealth: 7,
                early_receiver_mean_realized_delta: 3,
                ..M4Record::default()
            }],
            OutputFormat::Csv,
        );
        let headers = output
            .lines()
            .next()
            .unwrap_or_default()
            .split(',')
            .collect::<Vec<_>>();
        let row = output
            .lines()
            .nth(1)
            .unwrap_or_default()
            .split(',')
            .collect::<Vec<_>>();
        let column = |name: &str| {
            headers
                .iter()
                .position(|header| *header == name)
                .unwrap_or_else(|| panic!("missing M4 CSV column {name}"))
        };

        assert_eq!(row[column("early_receiver_mean_real_wealth")], "7");
        assert_eq!(row[column("late_receiver_mean_real_wealth")], "");
        assert_eq!(row[column("non_receiver_mean_real_wealth")], "");
        assert_eq!(row[column("early_late_real_wealth_gap")], "");
        assert_eq!(row[column("early_receiver_mean_realized_delta")], "3");
        assert_eq!(row[column("late_receiver_mean_realized_delta")], "");
    }
}
