pub mod administration;
pub mod auth;
pub mod catalog;
pub mod closing;
pub mod costing;
pub mod documents;
pub mod files;
pub mod finance;
pub mod inventory;
pub mod plumbing;
pub mod pricing;
pub mod printing;
pub mod purchasing;
pub mod quarantine;
pub mod receivable_adjustments;
pub mod reconciliation;
pub mod reporting;
pub mod reports;
pub mod returns;

use crate::{error::Result, models::*, state::Actor};
use rusqlite::Connection;
use serde_json::Value;

pub fn permission(m: &Mutation) -> &'static str {
    match m {
        Mutation::CreateVariants { .. } => "products.variants",
        Mutation::SaveBundle { .. } => "products.bundles",
        Mutation::SetWarranty { .. } => "products.warranty",
        Mutation::SetQuarantineWarehouse { .. } => "inventory.quarantine_manage",
        Mutation::QuarantineTransfer { .. } => "inventory.quarantine",
        Mutation::SaveProductPlanning { .. } | Mutation::CreateSuggestedOrders { .. } => {
            "purchases.plan"
        }
        Mutation::SavePricingPolicy { .. } => "pricing.manage",
        Mutation::ApplyPrices { .. } => "products.price_update",
        Mutation::AllocateLandedCost { .. } => "purchases.write",
        Mutation::CloseYear { .. } => "finance.close",
        Mutation::ReceivableAdjustment { kind, .. } => kind.permission(),
        Mutation::SaveBankStatement { .. }
        | Mutation::MatchStatement { .. }
        | Mutation::UnmatchStatement { .. }
        | Mutation::VoidBankStatement { .. } => "finance.reconcile",
        Mutation::SaveProduct { .. } => "products.write",
        Mutation::SaveParty { .. } => "parties.write",
        Mutation::SaveMaster { master } => match master {
            Master::Category { .. } | Master::Unit { .. } => "products.write",
            Master::WarehouseProduct { .. } => "inventory.adjust",
            _ => "settings.write",
        },
        Mutation::SaveDraft { draft } => draft.kind.permission(),
        Mutation::DocumentAction { .. } => "dashboard.read",
        Mutation::ConvertQuotation { .. } => "sales.write",
        Mutation::Return { .. } => "dashboard.read",
        Mutation::Reverse { .. } => "documents.reverse",
        Mutation::Adjustment { .. }
        | Mutation::StartStocktake { .. }
        | Mutation::CountStocktake { .. } => "inventory.adjust",
        Mutation::Voucher { .. } | Mutation::Settle { .. } | Mutation::Unsettle { .. } => {
            "payments.write"
        }
        Mutation::Journal { .. } | Mutation::Expense { .. } | Mutation::CashTransfer { .. } => {
            "finance.write"
        }
        Mutation::LandedCost { .. } => "purchases.write",
        Mutation::ChequeEvent { .. } => "cheques.write",
        Mutation::OpenShift { .. } | Mutation::CloseShift { .. } => "cash.shift",
        Mutation::SaveSettings { .. } => "settings.write",
        Mutation::FiscalPeriod { locked, .. } => {
            if *locked {
                "periods.lock"
            } else {
                "periods.unlock"
            }
        }
        Mutation::SetUserAccess { .. } => "users.manage",
    }
}
pub fn mutate(db: &Connection, actor: &Actor, m: &Mutation) -> Result<Value> {
    actor.require(permission(m))?;
    match m {
        Mutation::CreateVariants {
            template_id,
            variants,
            preview_hash,
            reason,
        } => plumbing::create_variants(db, actor, template_id.0, variants, preview_hash, reason),
        Mutation::SaveBundle { bundle, reason } => plumbing::save_bundle(db, actor, bundle, reason),
        Mutation::SetWarranty {
            product_id,
            revision,
            days,
            reason,
        } => plumbing::warranty(db, actor, product_id.0, revision.0, *days, reason),
        Mutation::SetQuarantineWarehouse {
            warehouse_id,
            revision,
            quarantine,
            reason,
        } => quarantine::set_warehouse(db, actor, warehouse_id.0, revision.0, *quarantine, reason),
        Mutation::QuarantineTransfer {
            date,
            from_id,
            to_id,
            lines,
            reason,
        } => quarantine::transfer(db, actor, date, from_id.0, to_id.0, lines, reason),
        Mutation::SaveProductPlanning { planning, reason } => {
            purchasing::save_planning(db, actor, planning, reason)
        }
        Mutation::SavePricingPolicy {
            revision,
            minimum_margin_bps,
            reason,
        } => pricing::save_policy(db, actor, revision.0, *minimum_margin_bps, reason),
        Mutation::CreateSuggestedOrders {
            filter,
            product_ids,
            preview_hash,
            reason,
        } => purchasing::create_orders(db, actor, filter, product_ids, preview_hash, reason),
        Mutation::ApplyPrices {
            unit_ids,
            rule,
            preview_hash,
            reason,
        } => pricing::apply(db, actor, unit_ids, rule, preview_hash, reason),
        Mutation::AllocateLandedCost {
            date,
            supplier_id,
            description,
            basis,
            amount,
            receipt_line_ids,
            preview_hash,
        } => purchasing::post_landed(
            db,
            actor,
            date,
            supplier_id.0,
            description,
            *basis,
            amount.0,
            receipt_line_ids,
            preview_hash,
        ),
        Mutation::CloseYear {
            period_id,
            date,
            preview_hash,
            reason,
        } => closing::post(db, actor, period_id.0, date, preview_hash, reason),
        Mutation::ReceivableAdjustment {
            kind,
            date,
            debit_line_id,
            amount,
            reason,
        } => {
            receivable_adjustments::post(db, actor, *kind, date, debit_line_id.0, amount.0, reason)
        }
        Mutation::SaveBankStatement { statement } => reconciliation::save(db, actor, statement),
        Mutation::MatchStatement {
            statement_line_id,
            journal_line_id,
            amount,
            date,
            reason,
        } => reconciliation::post_match(
            db,
            actor,
            statement_line_id.0,
            journal_line_id.0,
            amount.0,
            date,
            reason,
        ),
        Mutation::UnmatchStatement {
            match_id,
            date,
            reason,
        } => reconciliation::unmatch(db, actor, match_id.0, date, reason),
        Mutation::VoidBankStatement {
            statement_id,
            date,
            reason,
        } => reconciliation::void(db, actor, statement_id.0, date, reason),
        Mutation::SaveProduct { product } => catalog::product(db, product),
        Mutation::SaveParty { party } => catalog::party(db, party),
        Mutation::SaveMaster { master } => administration::master(db, master),
        Mutation::SaveDraft { draft } => documents::save(db, actor, draft),
        Mutation::DocumentAction {
            id,
            revision,
            action,
            reason,
            acknowledge_credit,
        } => documents::action(
            db,
            actor,
            id.0,
            revision.0,
            *action,
            reason,
            *acknowledge_credit,
        ),
        Mutation::ConvertQuotation { id, date } => documents::convert_quote(db, actor, id.0, date),
        Mutation::Return {
            source_id,
            date,
            warehouse_id,
            lines,
            payments,
            reason,
        } => returns::post_return(
            db,
            actor,
            source_id.0,
            date,
            warehouse_id.map(|v| v.0),
            lines,
            payments,
            reason,
        ),
        Mutation::Reverse { id, date, reason } => returns::reverse(db, actor, id.0, date, reason),
        Mutation::Adjustment {
            date,
            warehouse_id,
            product_id,
            quantity,
            unit_cost,
            reason,
        } => inventory::adjust(
            db,
            actor,
            date,
            warehouse_id.0,
            product_id.0,
            quantity.0,
            unit_cost.0,
            reason,
        ),
        Mutation::StartStocktake {
            date,
            warehouse_id,
            product_ids,
            reason,
        } => inventory::start_count(db, actor, date, warehouse_id.0, product_ids, reason),
        Mutation::CountStocktake {
            id,
            revision,
            lines,
        } => inventory::count(db, id.0, revision.0, actor.user_id, lines),
        Mutation::Voucher {
            date,
            party_id,
            supplier,
            outgoing,
            payments,
            notes,
        } => finance::voucher(
            db, actor, date, party_id.0, *supplier, *outgoing, payments, notes,
        ),
        Mutation::Journal {
            date,
            opening,
            lines,
            notes,
        } => finance::manual(db, actor, date, *opening, lines, notes),
        Mutation::Expense {
            date,
            party_id,
            lines,
            payments,
            notes,
        } => finance::expense(
            db,
            actor,
            date,
            party_id.map(|x| x.0),
            lines,
            payments,
            notes,
        ),
        Mutation::CashTransfer {
            date,
            from_id,
            to_id,
            amount,
            notes,
        } => finance::cash_transfer(db, actor, date, from_id.0, to_id.0, amount.0, notes),
        Mutation::Settle {
            date,
            debit_line_id,
            credit_line_id,
            amount,
        } => finance::settle(db, actor, date, debit_line_id.0, credit_line_id.0, amount.0),
        Mutation::Unsettle {
            date,
            settlement_id,
            reason,
        } => finance::unsettle(db, actor, date, settlement_id.0, reason),
        Mutation::LandedCost {
            date,
            supplier_id,
            description,
            shares,
        } => inventory::landed(db, actor, date, supplier_id.0, description, shares),
        Mutation::ChequeEvent {
            date,
            cheque_id,
            cleared,
            bank_account_id,
            reason,
        } => finance::cheque(
            db,
            actor,
            date,
            cheque_id.0,
            *cleared,
            bank_account_id.map(|x| x.0),
            reason,
        ),
        Mutation::OpenShift { cash_account_id } => {
            finance::open_shift(db, actor, cash_account_id.0)
        }
        Mutation::CloseShift {
            id,
            date,
            counted,
            reason,
        } => finance::close_shift(db, actor, id.0, date, counted.0, reason),
        Mutation::SaveSettings { settings } => administration::settings(db, settings),
        Mutation::FiscalPeriod {
            id,
            name,
            starts_on,
            ends_on,
            locked,
            reason,
        } => administration::period(
            db,
            actor,
            id.map(|x| x.0),
            name,
            starts_on,
            ends_on,
            *locked,
            reason,
        ),
        Mutation::SetUserAccess {
            user_id,
            active,
            roles,
        } => administration::access(db, actor, user_id.0, *active, roles),
    }
}
