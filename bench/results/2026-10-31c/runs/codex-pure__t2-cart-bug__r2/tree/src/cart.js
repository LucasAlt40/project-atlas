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
  let sumCents = 0;
  for (const item of items) {
    sumCents += Math.round(item.price * 100) * item.qty;
  }
  const coupon = COUPONS[couponCode];
  if (coupon) {
    if (coupon.type === 'percent') {
      sumCents = Math.round(sumCents * (100 - coupon.value) / 100);
    } else {
      sumCents -= coupon.value * 100;
    }
  }
  return Math.max(0, sumCents) / 100;
}

module.exports = { total };
