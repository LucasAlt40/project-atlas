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
  // Work in integer cents so float error never leaks into the total.
  let cents = 0;
  for (const item of items) {
    cents += Math.round(item.price * 100) * item.qty;
  }
  if (Object.hasOwn(COUPONS, couponCode)) {
    const coupon = COUPONS[couponCode];
    if (coupon.type === 'percent') {
      cents = (cents * (100 - coupon.value)) / 100;
    } else {
      cents -= Math.round(coupon.value * 100);
    }
  }
  // Never negative, and always a whole number of cents.
  return Math.max(0, Math.round(cents)) / 100;
}

module.exports = { total };
