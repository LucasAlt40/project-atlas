// Cart total with optional coupon. Money is summed in integer cents so
// totals never pick up floating-point noise like 0.30000000000000004.
const COUPONS = {
  TENOFF: { type: 'fixed', value: 10 },
  OFF20: { type: 'percent', value: 20 },
  HALF: { type: 'percent', value: 50 },
};

/**
 * @param {{ price: number, qty: number }[]} items
 * @param {string} [couponCode] unknown codes are ignored
 * @returns {number} total rounded to 2 decimals, never negative
 */
function total(items, couponCode) {
  let cents = 0;
  for (const item of items) {
    cents += Math.round(item.price * 100) * item.qty;
  }
  if (Object.hasOwn(COUPONS, couponCode)) {
    const coupon = COUPONS[couponCode];
    if (coupon.type === 'percent') {
      cents = Math.round((cents * (100 - coupon.value)) / 100);
    } else {
      cents -= Math.round(coupon.value * 100);
    }
  }
  return Math.max(0, cents) / 100;
}

module.exports = { total };
