Add order cancellation to this shop (src/orders.js, src/inventory.js, src/reports.js; src/events.js is the event bus).

- orders.cancel(orderId, reason): cancels a placed order. The items go back to stock, the order's status becomes 'cancelled' and keeps the reason, and a single 'order.cancelled' event is emitted with { orderId, reason }. It returns the cancelled order.
- Cancelling an order that does not exist throws an Error whose message contains "not found". Cancelling one that is already cancelled throws an Error whose message contains "already cancelled", and changes nothing (no stock moves, no event).
- reason must be a non-empty string, otherwise a TypeError, and nothing changes.
- Reports must stay consistent: reports.revenue() and reports.unitsSold(sku) count only orders that are not cancelled.

Everything that works today must keep working, including the existing tests in test/. Add tests for the new behaviour in test/.
