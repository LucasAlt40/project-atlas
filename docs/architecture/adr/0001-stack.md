# ADR 0001 — Stack

**Status:** aceito

- **Tauri 2** (fixado na 2.x; a mais nova do crates.io é uma alpha 3.0) com **Rust** no núcleo.
- **React 19 + TypeScript (strict) + Vite**, como preferido no briefing.
- **TypeScript 6.0**, não 7: o `typescript-eslint` ainda não suporta a 7. Rever quando suportar.
- **Estilo: CSS Modules + propriedades customizadas de CSS** (tokens de design em `styles/global.css`). Já vem no
  Vite, sem dependências, com escopo local por padrão, e os tokens abrem caminho para um design system.
- **Testes: Vitest + Testing Library** (compartilham a configuração do Vite) e `cargo test` para o Rust.
- **Sem roteador, sem biblioteca de estado, sem kit de UI** por enquanto; cada um entra quando surgir uma
  necessidade concreta.
- **npm** como gerenciador de pacotes (já instalado, sem ferramenta extra).
