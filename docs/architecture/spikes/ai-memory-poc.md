# Spike — ai-memory como memória externa do Atlas

Data: 2026-10-20. Projeto: <https://github.com/akitaonrails/ai-memory> (MIT, Rust, v3.1.1, commit `11ab100`).
Tudo aqui foi **medido** com o binário compilado do fonte; o que é só leitura de código está marcado.

## O que ele é

Um servidor local (binário único) com `wiki/` em markdown versionado no git, índice SQLite (FTS5, entidades, grafo,
embeddings opcionais), hooks de captura e uma ponte MCP (`ai-memory mcp-bridge`). A CLI e a ponte falam com o servidor
por HTTP, então **alguém precisa mantê-lo rodando**.

## Como foi instalado (e o que não foi feito)

- Clonado em pasta isolada e compilado com `cargo build --release -p ai-memory-cli` (3 min 54 s, binário de 40 MB).
  O instalador `curl | sh` **não** foi executado.
- Dados em pasta própria (`--data-dir`), servidor em `127.0.0.1:49391` (a porta padrão do ai-memory é 49374; o script `scripts/ai-memory-local.sh` usa a mesma por padrão), token gerado em
  `<data>/auth-token` (0600). `install-hooks`, `install-mcp` e `setup` **não** foram executados: nada foi escrito em
  `~/.claude`, `~/.codex` nem em LaunchAgents.

## Medido

| Fato            | Resultado                                                                                                                                                                                                                            |
| --------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ |
| Download padrão | O servidor baixa sozinho um modelo de embeddings (MiniLM, ~87 MB, Hugging Face) se `embedding_provider` não estiver definido. Interrompido; com `AI_MEMORY_EMBEDDING_PROVIDER=none` **nenhuma conexão externa** foi aberta (`lsof`). |
| Busca (FTS5)    | Acha a nota inclusive para a pergunta em linguagem natural ("where is the retry limit defined"). Na CLI e em `GET /admin/search`.                                                                                                    |
| API HTTP        | `GET /admin/search?q=&workspace=&project=&limit=` com `Authorization: Bearer <token>`; sem token, 401. Resposta: lista de `{id,path,title,snippet,rank}`; o `snippet` traz `<mark>`.                                                 |
| Ponte MCP       | Handshake ok, **24 ferramentas**. Os schemas somam **66 925 caracteres (~17 mil tokens)**; só `memory_query` tem 8 221 (~2 mil tokens). Para comparação, a regra de propostas da memória nativa custa ~200 tokens.                   |
| Escopo no MCP   | Sem `workspace` e `project` em cada chamada, o cliente cai em outro projeto (`scratch`) e a busca volta vazia. O modelo teria de informar o escopo certo toda vez.                                                                   |

## Lido no código (auditoria por agente, não executado)

- Sem telemetria e sem verificação de atualização (só `ai-memory upgrade`, explícito). `unsafe_code = "forbid"`.
- Chamadas a LLM só se `llm_provider` for configurado. Embeddings em nuvem, `capture_assistant` e `AI_MEMORY_RERANKER=llm`
  enviam texto para fora: opt-in.
- **Hooks capturam prompts (até 16 KiB) e entradas de ferramentas** e gravam sem criptografia em repouso, com redação
  de segredos "best effort" (não pega strings de alta entropia avulsas).
- **`backfill_on_start = true` por padrão**: o hook de início de sessão dispara um `backfill` que importa até 25
  transcrições de `~/.claude/projects` e de outros agentes. Desligar com `AI_MEMORY_BACKFILL_ON_START=false`.
- `setup` instala um LaunchAgent; `install-hooks --apply` altera `~/.claude/settings.json`, `~/.codex/hooks.json`,
  plugins do OpenCode. `AI_MEMORY_HOME`, `CLAUDE_CONFIG_DIR` e `CODEX_HOME` redirecionam.
- Retenção: páginas episódicas frias saem do índice em ~80 dias e são apagadas de vez após 180.

## Consequências para o desenho (ADR 0040)

1. **Não usar a ponte MCP no prompt do agente.** 17 mil tokens (ou 2 mil só com `memory_query`) por execução, mais o
   risco de o modelo errar o escopo, contra ~140 tokens de notas já filtradas.
2. **O Atlas faz o recall antes de montar o prompt**, direto na API HTTP, com consulta e escopo montados por ele.
3. **Sem hooks.** A captura de prompts e o backfill leem o que o Atlas não controla. O que entra no servidor entra por
   escrita explícita do usuário (ou, no futuro, por uma nota que ele aprovou no Atlas).
4. **Só loopback.** O token nunca sai da máquina; o endereço é validado na configuração e de novo no cliente.
5. **Embeddings desligados** no uso pelo Atlas (sem download, resultado determinístico).
