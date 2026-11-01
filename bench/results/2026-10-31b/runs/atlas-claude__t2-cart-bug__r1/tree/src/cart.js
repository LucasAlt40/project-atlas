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
  let sum = 0;
  for (const item of items) {
    sum += item.price * item.qty;
  }
  if (Object.prototype.hasOwnProperty.call(COUPONS, couponCode)) {
    const coupon = COUPONS[couponCode];
    if (coupon.type === 'percent') {
      sum = sum - (sum * coupon.value) / 100;
    } else {
      sum = sum - coupon.value;
    }
  }
  return Math.round(Math.max(0, sum) * 100) / 100;
}

module.exports = { total };
