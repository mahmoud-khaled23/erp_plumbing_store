pub mod administration;
pub mod auth;
pub mod catalog;
pub mod costing;
pub mod documents;
pub mod files;
pub mod finance;
pub mod inventory;
pub mod printing;
pub mod reporting;
pub mod returns;

use crate::{error::Result, models::*, state::Actor};
use rusqlite::Connection;
use serde_json::Value;

pub fn permission(m: &Mutation) -> &'static str {
    match m {
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
