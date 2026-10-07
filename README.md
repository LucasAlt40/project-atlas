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

## Instalar (para testar)

Baixe o instalador do seu sistema na página de
[Releases](https://github.com/LucasAlt40/project-atlas/releases) (macOS `.dmg`, Windows `.msi`/`.exe`,
Linux `.AppImage`/`.deb`). Cada release traz um `SHA256SUMS.txt` para conferir o download. Sem certificado de assinatura pago, o sistema ainda pode avisar na primeira abertura (detalhes em [docs/INSTALADORES.md](docs/INSTALADORES.md)):

- **macOS:** se aparecer "app danificado" ou "desenvolvedor não identificado", rode
  `xattr -cr "/Applications/Project Atlas.app"` e abra de novo (ou botão direito → Abrir).
  Use o `aarch64` em Macs com chip Apple e o `x64` em Macs Intel.
- **Windows:** no aviso do SmartScreen, clique em "Mais informações" → "Executar assim mesmo".
- **Linux:** `chmod +x Project-Atlas*.AppImage` e execute.

O Atlas procura as CLIs de IA (OpenCode, Claude, Codex, Gemini) no `PATH` e nos diretórios usuais
(`~/.local/bin`, `~/.npm-global/bin`, `~/.cargo/bin`, Homebrew). Instale ao menos uma delas e faça login
nela antes de criar agentes.

## Publicar uma versão

```bash
# 1. suba a versão em package.json, src-tauri/Cargo.toml e src-tauri/tauri.conf.json
git tag v0.1.0
git push origin main v0.1.0
```

O workflow [release.yml](.github/workflows/release.yml) compila para macOS (Apple Silicon e Intel),
Windows e Linux e anexa os instaladores a uma release.

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
