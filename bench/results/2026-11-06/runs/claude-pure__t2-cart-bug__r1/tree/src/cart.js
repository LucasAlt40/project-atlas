// Cart total with optional coupon.
const COUPONS = {
  TENOFF: { type: 'fixed', value: 10 },
  OFF20: { type: 'percent', value: 20 },
  HALF: { type: 'percent', value: 50 },
};

/**
 * @param {{ price: number, qty: number }[]} items
 * @param {string} [couponCode]
 * @returns {number} total rounded to cents, never negative
 */
function total(items, couponCode) {
  let sum = 0;
  for (const item of items) {
    sum += item.price * item.qty;
  }
  // Own-property check so inherited keys like "toString" are not treated as coupons.
  const coupon = Object.hasOwn(COUPONS, couponCode) ? COUPONS[couponCode] : null;
  if (coupon) {
    if (coupon.type === 'percent') {
      sum = (sum * (100 - coupon.value)) / 100;
    } else {
      sum = sum - coupon.value;
    }
  }
  sum = Math.max(0, sum);
  return Math.round(sum * 100) / 100;
}

module.exports = { total };
