use crate::{
    error::{Error, Result},
    models::arithmetic::sum,
};
use rusqlite::{Connection, Row};

fn mismatch() -> Error {
    Error::new(
        "stock_balance_mismatch",
        "أرصدة المخزون لا تطابق سجل الحركات؛ أوقف الترحيل واستعد نسخة احتياطية سليمة بعد مراجعة الدعم",
    )
}

fn compare(
    row: Option<&Row<'_>>,
    key: (i64, i64),
    quantity: i64,
    value: i64,
    last: i64,
) -> Result<()> {
    let row = row.ok_or_else(mismatch)?;
    if (row.get::<_, i64>(0)?, row.get::<_, i64>(1)?) != key
        || row.get::<_, i64>(2)? != quantity
        || row.get::<_, i64>(3)? != value
        || row.get::<_, i64>(4)? != last
    {
        return Err(mismatch());
    }
    Ok(())
}

/// O(movements + pools), bounded memory, independent recomputation with checked i128
/// arithmetic. The caller owns a read snapshot (or an exclusive startup connection).
/// Includes in-flight rows so it can also verify a transaction before commit.
pub fn verify_stock_balances(db: &Connection) -> Result<(i64, i64)> {
    let result = verify(db);
    result.map_err(|error| if error.retryable { error } else { mismatch() })
}

fn verify(db: &Connection) -> Result<(i64, i64)> {
    let mut movements = db.prepare("SELECT product_id,warehouse_id,quantity_micros,value_minor,id FROM stock_movements ORDER BY product_id,warehouse_id,id")?;
    let mut balances = db.prepare("SELECT product_id,warehouse_id,quantity_micros,value_minor,last_movement_id FROM stock_balances ORDER BY product_id,warehouse_id")?;
    let mut ledger = movements.query([])?;
    let mut cache = balances.query([])?;
    let mut key = None;
    let (mut quantity, mut value, mut last, mut pools, mut count) = (0, 0, 0, 0, 0);
    while let Some(row) = ledger.next()? {
        let next = (row.get::<_, i64>(0)?, row.get::<_, i64>(1)?);
        if key != Some(next) {
            if let Some(previous) = key {
                compare(cache.next()?, previous, quantity, value, last)?;
            }
            key = Some(next);
            quantity = 0;
            value = 0;
            pools = sum([pools, 1])?;
        }
        quantity = sum([quantity, row.get(2)?])?;
        value = sum([value, row.get(3)?])?;
        last = row.get(4)?;
        count = sum([count, 1])?;
    }
    if let Some(previous) = key {
        compare(cache.next()?, previous, quantity, value, last)?;
    }
    if cache.next()?.is_some() {
        return Err(mismatch());
    }
    Ok((pools, count))
}
