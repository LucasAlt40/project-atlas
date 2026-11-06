// Cart total with optional coupon.
const COUPONS = {
  TENOFF: { type: 'fixed', value: 10 },
  OFF20: { type: 'percent', value: 20 },
  HALF: { type: 'percent', value: 50 },
};

/**
 * @param {{ price: number, qty: number }[]} items
 * @param {string} [couponCode]
 * @returns {number}
 */
function total(items, couponCode) {
  // Work in integer cents to avoid floating-point drift.
  let sumCents = 0;
  for (const item of items) {
    sumCents += Math.round(item.price * 100) * item.qty;
  }
  const coupon = Object.hasOwn(COUPONS, couponCode) ? COUPONS[couponCode] : null;
  if (coupon) {
    if (coupon.type === 'percent') {
      sumCents = Math.round((sumCents * (100 - coupon.value)) / 100);
    } else {
      sumCents -= Math.round(coupon.value * 100);
    }
  }
  return Math.max(sumCents, 0) / 100;
}

module.exports = { total };
