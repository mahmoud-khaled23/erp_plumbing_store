use crate::error::{Error, Result, ensure};
use rusqlite::{
    ToSql,
    types::{FromSql, FromSqlResult, ToSqlOutput, ValueRef},
};
use serde::{Deserialize, Deserializer, Serialize, Serializer};

/// All i64 values cross IPC as strings, including identifiers and quantities.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord)]
pub struct Int(pub i64);
impl Serialize for Int {
    fn serialize<S: Serializer>(&self, s: S) -> std::result::Result<S::Ok, S::Error> {
        s.serialize_str(&self.0.to_string())
    }
}
impl<'de> Deserialize<'de> for Int {
    fn deserialize<D: Deserializer<'de>>(d: D) -> std::result::Result<Self, D::Error> {
        let s = String::deserialize(d)?;
        s.parse::<i64>().map(Self).map_err(serde::de::Error::custom)
    }
}
impl ToSql for Int {
    fn to_sql(&self) -> rusqlite::Result<ToSqlOutput<'_>> {
        self.0.to_sql()
    }
}
impl FromSql for Int {
    fn column_result(v: ValueRef<'_>) -> FromSqlResult<Self> {
        i64::column_result(v).map(Self)
    }
}
pub const SCALE: i64 = 1_000_000;
pub const MONEY_MAX: i64 = 1_000_000_000_000;
pub fn narrow(n: i128) -> Result<i64> {
    i64::try_from(n).map_err(|_| Error::validation("القيمة تتجاوز الحد المسموح"))
}
pub fn sum(values: impl IntoIterator<Item = i64>) -> Result<i64> {
    narrow(values.into_iter().map(i128::from).sum())
}
pub fn prorate(value: i64, numerator: i64, denominator: i64) -> Result<i64> {
    ensure(denominator > 0 && numerator >= 0, "نسبة الحساب غير صالحة")?;
    let n = i128::from(value) * i128::from(numerator);
    let d = i128::from(denominator);
    narrow(n.signum() * ((n.abs() + d / 2) / d))
}
pub fn convert(q: i64, numerator: i64, denominator: i64) -> Result<i64> {
    ensure(
        q >= 0 && numerator > 0 && denominator > 0,
        "الكمية أو معامل الوحدة غير صالح",
    )?;
    let n = i128::from(q) * i128::from(numerator);
    ensure(
        n % i128::from(denominator) == 0,
        "الكمية تحتاج إلى أكثر من ست خانات عشرية",
    )?;
    let q = narrow(n / i128::from(denominator))?;
    ensure(q <= 1_000_000_000_000_000, "الكمية تتجاوز الحد المسموح")?;
    Ok(q)
}
pub fn allocate(amount: i64, weights: &[i64]) -> Result<Vec<i64>> {
    ensure(
        amount >= 0 && weights.iter().all(|w| *w >= 0),
        "قيمة التوزيع غير صالحة",
    )?;
    let total = sum(weights.iter().copied())?;
    if total == 0 {
        ensure(amount == 0, "لا يمكن توزيع مبلغ على قيم صفرية")?;
        return Ok(vec![0; weights.len()]);
    }
    let mut shares = vec![0; weights.len()];
    let mut remainders = Vec::with_capacity(weights.len());
    for (i, weight) in weights.iter().enumerate() {
        let value = i128::from(amount) * i128::from(*weight);
        shares[i] = narrow(value / i128::from(total))?;
        remainders.push((value % i128::from(total), i));
    }
    remainders.sort_by(|a, b| b.0.cmp(&a.0).then(a.1.cmp(&b.1)));
    let left = amount - sum(shares.iter().copied())?;
    for (_, i) in remainders.into_iter().take(left as usize) {
        shares[i] += 1;
    }
    Ok(shares)
}
pub fn parse_decimal(text: &str, scale: u32) -> Result<i64> {
    ensure(scale <= 6, "دقة الرقم غير صالحة")?;
    let normalized: String = text
        .trim()
        .chars()
        .map(|c| match c {
            '٠'..='٩' => char::from(b'0' + (c as u32 - '٠' as u32) as u8),
            '۰'..='۹' => char::from(b'0' + (c as u32 - '۰' as u32) as u8),
            '٫' => '.',
            c => c,
        })
        .collect();
    let (sign, digits) = normalized
        .strip_prefix('-')
        .map_or((1_i128, normalized.as_str()), |s| (-1, s));
    let mut parts = digits.split('.');
    let whole = parts.next().unwrap_or("");
    let fraction = parts.next().unwrap_or("");
    ensure(
        !whole.is_empty()
            && parts.next().is_none()
            && fraction.len() <= scale as usize
            && whole.bytes().all(|c| c.is_ascii_digit())
            && fraction.bytes().all(|c| c.is_ascii_digit()),
        "الرقم غير صالح أو يحتوي على دقة زائدة",
    )?;
    let whole: i128 = whole
        .parse()
        .map_err(|_| Error::validation("الرقم كبير جداً"))?;
    let fraction: i128 = if fraction.is_empty() {
        0
    } else {
        fraction
            .parse()
            .map_err(|_| Error::validation("الرقم غير صالح"))?
    };
    let scaled = whole
        .checked_mul(10_i128.pow(scale))
        .and_then(|v| {
            v.checked_add(
                fraction * 10_i128.pow(scale - digits.split('.').nth(1).unwrap_or("").len() as u32),
            )
        })
        .ok_or_else(|| Error::validation("الرقم كبير جداً"))?;
    narrow(sign * scaled)
}
