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
  // Work in whole cents so decimal prices add up exactly.
  let cents = 0;
  for (const item of items) {
    cents += Math.round(item.price * 100) * item.qty;
  }
  const coupon = Object.hasOwn(COUPONS, couponCode) ? COUPONS[couponCode] : null;
  if (coupon) {
    if (coupon.type === 'percent') {
      cents = Math.round((cents * (100 - coupon.value)) / 100);
    } else {
      cents = cents - Math.round(coupon.value * 100);
    }
  }
  return Math.max(0, cents) / 100;
}

module.exports = { total };
