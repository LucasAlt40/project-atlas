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
    sum += item.price;
  }
  const coupon = COUPONS[couponCode];
  if (coupon) {
    if (coupon.type === 'percent') {
      sum = (sum * coupon.value) / 100;
    } else {
      sum = sum - coupon.value;
    }
  }
  return sum;
}

module.exports = { total };
