// Placing orders: checks and takes stock, records the order, announces it.
function createOrders({ inventory, bus, prices }) {
  const orders = new Map();
  let nextId = 1;
  return {
    place(items) {
      for (const { sku, qty } of items) {
        if (inventory.quantity(sku) < qty) throw new Error(`not enough ${sku}`);
      }
      for (const { sku, qty } of items) inventory.take(sku, qty);
      const total = items.reduce((sum, { sku, qty }) => sum + prices[sku] * qty, 0);
      const order = { id: nextId++, items: items.map((i) => ({ ...i })), total, status: 'placed' };
      orders.set(order.id, order);
      bus.emit('order.placed', { orderId: order.id, total });
      return { ...order };
    },
    get(id) {
      const order = orders.get(id);
      return order && { ...order, items: order.items.map((i) => ({ ...i })) };
    },
    all() {
      return [...orders.values()].map((o) => ({ ...o, items: o.items.map((i) => ({ ...i })) }));
    },
  };
}

module.exports = { createOrders };
