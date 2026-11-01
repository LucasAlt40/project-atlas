// A minimal synchronous event bus.
function createBus() {
  const handlers = new Map();
  return {
    on(name, handler) {
      if (!handlers.has(name)) handlers.set(name, []);
      handlers.get(name).push(handler);
    },
    emit(name, payload) {
      for (const handler of handlers.get(name) ?? []) handler(payload);
    },
  };
}

module.exports = { createBus };
