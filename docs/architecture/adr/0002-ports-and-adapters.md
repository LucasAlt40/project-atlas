# ADR 0002 — Ports in `application`, adapters outward

**Status:** accepted

The brief draws `Commands → Application → Domain → Infrastructure`. Taken literally, application code would
depend on concrete infrastructure, which hurts testability and cross-platform swapping. Instead,
`application/` declares traits (ports) and `platform/` / future `infrastructure/` implement them; `lib.rs`
wires them. The layering of the brief is preserved for calls at runtime, but source dependencies point inward.

The `infrastructure/` directory from the suggested layout is not created yet: it would be empty. It is
added together with the first adapter that is not OS-specific.

**Update (V0.2):** `infrastructure/` now exists: it holds the process runner and the JSON config store,
the first adapters that are neither OS-specific nor pure.
