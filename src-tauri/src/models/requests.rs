use super::Int;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Setup {
    pub store_name: String,
    pub username: String,
    pub display_name: String,
    pub password: String,
    pub fiscal_year: u16,
    pub demo: bool,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ProductUnit {
    pub id: Option<Int>,
    pub unit_id: Int,
    pub numerator: Int,
    pub denominator: Int,
    pub barcode: Option<String>,
    pub retail_price: Int,
    pub wholesale_price: Int,
    pub active: bool,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Product {
    pub id: Option<Int>,
    pub revision: Int,
    pub code: String,
    pub name: String,
    pub category_id: Int,
    pub brand: String,
    pub material: String,
    pub size: String,
    pub base_unit_id: Int,
    pub standard_cost: Int,
    pub min_stock: Int,
    pub active: bool,
    pub units: Vec<ProductUnit>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Party {
    pub id: Option<Int>,
    pub code: String,
    pub name: String,
    pub customer: bool,
    pub supplier: bool,
    pub phone: String,
    pub address: String,
    pub tax_number: String,
    pub credit_limit: Option<Int>,
    pub terms_days: u16,
    pub active: bool,
    pub notes: String,
}
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum DocumentKind {
    Sale,
    Quotation,
    PurchaseOrder,
    Purchase,
    GoodsReceipt,
    Transfer,
    OpeningStock,
}
impl DocumentKind {
    pub fn code(self) -> &'static str {
        match self {
            Self::Sale => "sale",
            Self::Quotation => "quotation",
            Self::PurchaseOrder => "purchase_order",
            Self::Purchase => "purchase",
            Self::GoodsReceipt => "goods_receipt",
            Self::Transfer => "transfer",
            Self::OpeningStock => "opening_stock",
        }
    }
    pub fn permission(self) -> &'static str {
        match self {
            Self::Sale | Self::Quotation => "sales.write",
            Self::PurchaseOrder | Self::Purchase => "purchases.write",
            Self::GoodsReceipt => "inventory.receive",
            Self::Transfer => "inventory.transfer",
            Self::OpeningStock => "inventory.adjust",
        }
    }
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PurchaseMatch {
    pub receipt_line_id: Int,
    pub base_quantity: Int,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DraftLine {
    pub unit_id: Int,
    pub quantity: Int,
    pub price: Int,
    pub discount: Int,
    pub tax_bps: u16,
    pub order_line_id: Option<Int>,
    pub damaged: Int,
    pub short: Int,
    pub matches: Vec<PurchaseMatch>,
    pub notes: String,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ChequeInput {
    pub number: String,
    pub bank: String,
    pub due_date: String,
}
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum PaymentMethod {
    Cash,
    Bank,
    Cheque,
}
impl PaymentMethod {
    pub fn code(self) -> &'static str {
        match self {
            Self::Cash => "cash",
            Self::Bank => "bank",
            Self::Cheque => "cheque",
        }
    }
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Payment {
    pub method: PaymentMethod,
    pub cash_account_id: Option<Int>,
    pub shift_id: Option<Int>,
    pub amount: Int,
    pub reference: String,
    pub cheque: Option<ChequeInput>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Draft {
    pub id: Option<Int>,
    pub revision: Int,
    pub kind: DocumentKind,
    pub date: String,
    pub due_date: Option<String>,
    pub party_id: Option<Int>,
    pub warehouse_id: Option<Int>,
    pub destination_id: Option<Int>,
    pub source_id: Option<Int>,
    pub wholesale: bool,
    pub discount: Int,
    pub lines: Vec<DraftLine>,
    pub payments: Vec<Payment>,
    pub notes: String,
    pub reason: String,
}
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum DocumentAction {
    Post,
    Hold,
    Resume,
    Approve,
    Close,
    Cancel,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct JournalLine {
    pub account_id: Int,
    pub party_id: Option<Int>,
    pub due_date: Option<String>,
    pub amount: Int,
    pub memo: String,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ReturnLine {
    pub source_line_id: Int,
    pub quantity: Int,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ExpenseLine {
    pub category_id: Int,
    pub amount: Int,
    pub tax_bps: u16,
    pub description: String,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CountLine {
    pub product_id: Int,
    pub quantity: Int,
    pub reason: String,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CostShare {
    pub receipt_line_id: Int,
    pub amount: Int,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
pub enum Master {
    Category {
        id: Option<Int>,
        name: String,
        parent_id: Option<Int>,
    },
    Unit {
        id: Option<Int>,
        name: String,
        code: String,
    },
    Warehouse {
        id: Option<Int>,
        name: String,
        code: String,
        address: String,
        active: bool,
    },
    Account {
        id: Option<Int>,
        code: String,
        name: String,
        parent_id: Option<Int>,
        kind: AccountKind,
        postable: bool,
        active: bool,
    },
    CashAccount {
        id: Option<Int>,
        name: String,
        bank: bool,
        account_id: Int,
        bank_name: String,
        account_number: String,
        active: bool,
    },
    ExpenseCategory {
        id: Option<Int>,
        name: String,
        account_id: Int,
        active: bool,
    },
    WarehouseProduct {
        warehouse_id: Int,
        product_id: Int,
        min_stock: Int,
        bin: String,
    },
    Sequence {
        kind: String,
        year: u16,
        prefix: String,
        padding: u8,
    },
}
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AccountKind {
    Asset,
    Liability,
    Equity,
    Income,
    Expense,
}
impl AccountKind {
    pub fn code(self) -> &'static str {
        match self {
            Self::Asset => "asset",
            Self::Liability => "liability",
            Self::Equity => "equity",
            Self::Income => "income",
            Self::Expense => "expense",
        }
    }
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Settings {
    pub revision: Int,
    pub store_name: String,
    pub tax_number: String,
    pub address: String,
    pub phones: String,
    pub currency_code: String,
    pub currency_symbol: String,
    pub currency_digits: u8,
    pub arabic_digits: bool,
    pub timezone: String,
    pub vat_enabled: bool,
    pub vat_bps: u16,
    pub negative_stock: bool,
    pub block_credit: bool,
    pub thermal: bool,
    pub receipt_footer: String,
    pub print_copies: u8,
    pub auto_backup: bool,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
pub enum Mutation {
    SaveProduct {
        product: Product,
    },
    SaveParty {
        party: Party,
    },
    SaveMaster {
        master: Master,
    },
    SaveDraft {
        draft: Draft,
    },
    DocumentAction {
        id: Int,
        revision: Int,
        action: DocumentAction,
        reason: String,
        acknowledge_credit: bool,
    },
    ConvertQuotation {
        id: Int,
        date: String,
    },
    Return {
        source_id: Int,
        date: String,
        warehouse_id: Option<Int>,
        lines: Vec<ReturnLine>,
        payments: Vec<Payment>,
        reason: String,
    },
    Reverse {
        id: Int,
        date: String,
        reason: String,
    },
    Adjustment {
        date: String,
        warehouse_id: Int,
        product_id: Int,
        quantity: Int,
        unit_cost: Int,
        reason: String,
    },
    StartStocktake {
        date: String,
        warehouse_id: Int,
        product_ids: Vec<Int>,
        reason: String,
    },
    CountStocktake {
        id: Int,
        revision: Int,
        lines: Vec<CountLine>,
    },
    Voucher {
        date: String,
        party_id: Int,
        supplier: bool,
        outgoing: bool,
        payments: Vec<Payment>,
        notes: String,
    },
    Journal {
        date: String,
        opening: bool,
        lines: Vec<JournalLine>,
        notes: String,
    },
    Expense {
        date: String,
        party_id: Option<Int>,
        lines: Vec<ExpenseLine>,
        payments: Vec<Payment>,
        notes: String,
    },
    CashTransfer {
        date: String,
        from_id: Int,
        to_id: Int,
        amount: Int,
        notes: String,
    },
    Settle {
        date: String,
        debit_line_id: Int,
        credit_line_id: Int,
        amount: Int,
    },
    Unsettle {
        date: String,
        settlement_id: Int,
        reason: String,
    },
    LandedCost {
        date: String,
        supplier_id: Int,
        description: String,
        shares: Vec<CostShare>,
    },
    ChequeEvent {
        date: String,
        cheque_id: Int,
        cleared: bool,
        bank_account_id: Option<Int>,
        reason: String,
    },
    OpenShift {
        cash_account_id: Int,
    },
    CloseShift {
        id: Int,
        date: String,
        counted: Int,
        reason: String,
    },
    SaveSettings {
        settings: Settings,
    },
    FiscalPeriod {
        id: Option<Int>,
        name: String,
        starts_on: String,
        ends_on: String,
        locked: bool,
        reason: String,
    },
    SetUserAccess {
        user_id: Int,
        active: bool,
        roles: Vec<Int>,
    },
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Request {
    pub request_id: String,
    pub mutation: Mutation,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
pub enum Query {
    Bootstrap,
    CashDesk,
    Products {
        search: String,
        after_id: Int,
        limit: u16,
    },
    Product {
        id: Int,
    },
    Parties {
        search: String,
        after_id: Int,
        limit: u16,
    },
    Documents {
        kind: String,
        status: Option<String>,
        after_id: Int,
        limit: u16,
    },
    Document {
        id: Int,
    },
    Inventory {
        warehouse_id: Option<Int>,
        after_id: Int,
        limit: u16,
    },
    StockCard {
        product_id: Int,
        warehouse_id: Int,
        after_id: Int,
        limit: u16,
    },
    Report {
        report: Report,
        from: String,
        to: String,
        party_id: Option<Int>,
        limit: u16,
        offset: u32,
    },
    Dashboard {
        date: String,
    },
    Audit {
        after_id: Int,
        limit: u16,
    },
    Users,
}
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Report {
    TrialBalance,
    IncomeStatement,
    CashFlow,
    ProductProfit,
    CategoryProfit,
    CustomerProfit,
    PartyStatement,
    Receivables,
    Payables,
    Aging,
    Vat,
    LowStock,
    Valuation,
    Cheques,
    PurchaseOrders,
    OpenItems,
    CashShifts,
}
