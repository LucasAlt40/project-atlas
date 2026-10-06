# Project Atlas

Aplicativo desktop para orquestrar agentes de IA em projetos de desenvolvimento. Cada **workspace**
representa um projeto (uma pasta local) onde agentes, com personalidade, runtime e modelo próprios,
conversam e trabalham de forma independente.

> **Status:** em desenvolvimento inicial. A API e a interface ainda vão mudar.

## Stack

Tauri 2 · Rust · React · TypeScript · Vite

## Requisitos

- Node.js 22.12 ou superior (veja `.nvmrc`)
- Rust 1.90 ou superior
- [Dependências do Tauri](https://v2.tauri.app/start/prerequisites/) para o seu sistema operacional
- Opcional: CLIs de IA já instaladas na máquina (OpenCode, Claude)

## Como rodar

```bash
npm install
npm run tauri dev
```

## Verificações

```bash
npm run check   # tipos, lint, formatação e testes (frontend e Rust)
```

## Documentação

- Guia técnico completo (arquitetura, IA, contexto, workflow, segurança): [docs/guia](docs/guia/README.md)
- Resumo em uma página: [docs/RESUMO.md](docs/RESUMO.md)
- Decisões e arquitetura (ADRs): [docs/architecture](docs/architecture/README.md)

## Licença

[MIT](LICENSE)
