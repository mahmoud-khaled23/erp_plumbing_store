use crate::{
    error::{Result, ensure},
    models::arithmetic::{SCALE, prorate, sum},
};

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Pool {
    pub quantity: i64,
    pub value: i64,
}
impl Pool {
    pub fn issue(
        &mut self,
        quantity: i64,
        fallback_cost: i64,
        allow_negative: bool,
    ) -> Result<i64> {
        ensure(
            quantity > 0 && fallback_cost >= 0,
            "الكمية أو التكلفة غير صالحة",
        )?;
        ensure(
            allow_negative || quantity <= self.quantity,
            "الرصيد غير كاف؛ المخزون السالب غير مسموح",
        )?;
        let cost = if self.quantity > 0 {
            if quantity <= self.quantity {
                if quantity == self.quantity {
                    self.value
                } else {
                    prorate(self.value, quantity, self.quantity)?
                }
            } else {
                sum([
                    self.value,
                    prorate(self.value, quantity - self.quantity, self.quantity)?,
                ])?
            }
        } else if self.quantity < 0 {
            prorate(-self.value, quantity, -self.quantity)?
        } else {
            prorate(fallback_cost, quantity, SCALE)?
        };
        ensure(cost >= 0, "قيمة المخزون غير سليمة")?;
        self.quantity = sum([self.quantity, -quantity])?;
        self.value = sum([self.value, -cost])?;
        Ok(cost)
    }
    /// Returns (inventory addition, COGS correction) when replenishing provisional negative stock.
    pub fn receive(&mut self, quantity: i64, actual_cost: i64) -> Result<(i64, i64)> {
        ensure(
            quantity > 0 && actual_cost >= 0,
            "كمية أو تكلفة الاستلام غير صالحة",
        )?;
        let correction = if self.quantity < 0 {
            let covered = quantity.min(-self.quantity);
            prorate(actual_cost, covered, quantity)?
                - prorate(-self.value, covered, -self.quantity)?
        } else {
            0
        };
        let stock = sum([actual_cost, -correction])?;
        self.quantity = sum([self.quantity, quantity])?;
        self.value = sum([self.value, stock])?;
        Ok((stock, correction))
    }
}
