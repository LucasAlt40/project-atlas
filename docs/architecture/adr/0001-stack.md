# ADR 0001 — Stack

**Status:** accepted

- **Tauri 2** (pinned to 2.x; crates.io's newest is a 3.0 alpha) with **Rust** for the core.
- **React 19 + TypeScript (strict) + Vite**, as preferred in the brief.
- **TypeScript 6.0**, not 7: `typescript-eslint` does not yet support 7. Revisit when it does.
- **Styling: CSS Modules + CSS custom properties** (design tokens in `styles/global.css`). Built into Vite,
  zero dependencies, scoped by default, and tokens give a path to a design system.
- **Testing: Vitest + Testing Library** (shares Vite config), `cargo test` for Rust.
- **No router, no state library, no UI kit** yet; each is added when a concrete need appears.
- **npm** as package manager (already installed, no extra tooling).
