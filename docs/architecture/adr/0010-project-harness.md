# ADR 0010 — Project Harness

**Status:** aceito. Estendido pela [ADR 0011](0011-harness-knowledge-model.md) (evidências, proveniência, análise
semântica, diff de atualização).

## O que é

O Harness é o que o Atlas sabe sobre um projeto, guardado **no projeto** em `.atlas/` (não na configuração do próprio
Atlas, nem dentro de um worktree). Ele é versionável com o Git do projeto, legível por humanos e independente de
qualquer provedor, modelo ou runtime.

```
Usuário → Inicializar Projeto
        ProjectScanner → ScanSnapshot → ProjectAnalyzer → achados
                                              ↓ (revisão humana: desmarcar, corrigir, adicionar contexto)
                                        HarnessGenerator → HarnessStore → <projeto>/.atlas/

Execução → HarnessContextBuilder → PromptBuilder → Runtime (no worktree)
```

```
.atlas/
├── project.yaml          só metadados estruturais (versão, projeto, repositório, stack, correções)
├── context/              architecture.md, stack.md, conventions.md, business.md, constraints.md
├── agents/ workflows/ policies/ memory/    (marcadores; o Atlas só garante que existam)
└── backups/<timestamp>/  o que uma atualização substituiu
```

## Peças

| Peça                                               | Responsabilidade                                                                                                                                              |
| -------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `infrastructure/project_scanner.rs`                | O único código que lê uma pasta de projeto para análise. Nomes de entradas mais o texto de uma curta lista de manifestos permitidos.                          |
| `application/harness/analyzer` — `ProjectAnalyzer` | Snapshot entra, achados saem. `DeterministicAnalyzer` = analisadores de Git + stack + estrutura + CI. Um analisador semântico pode implementar a trait.       |
| `domain/harness.rs` — `Finding`                    | `category`, `key`, `label`, `value`, `confidence` (high/medium/low), `origin` (fact/inference/user), `source`, `evidence`.                                    |
| `application/harness/generator.rs`                 | Achados + contexto do usuário → arquivos. Diz de onde vem cada afirmação; ausência continua ausência ("não foi documentado").                                 |
| `infrastructure/harness_store.rs` — `HarnessStore` | O único código que escreve em `.atlas/`. Backup antes de substituir, renomeação atômica, nunca por um symlink, caminhos não saem de `.atlas/`.                |
| `application/harness/context.rs`                   | `HarnessContextBuilder`: lê o Harness na **raiz do projeto** e renderiza texto limitado e sem segredos para o prompt.                                         |
| `application/harness/mod.rs` — `HarnessService`    | `analyze`, `initialize` (criar / usar existente / atualizar existente), `refresh`, `get`. Os comandos têm formato de domínio; sem acesso genérico a arquivos. |

## Regras

- **Analisar é ler.** Nada é executado, compilado, instalado, iniciado ou baixado. Symlinks não são seguidos, então a
  varredura não sai da raiz. Pastas geradas (`node_modules`, `target`, `dist`, `.git`, …) são anotadas, não percorridas.
  Limites: profundidade 4, 5.000 entradas (atingir o limite de entradas é informado como _parcial_), 256 KiB por
  manifesto.
- **Segredos nunca entram no Harness.** Arquivos `.env` são anotados pelo nome (`.env detected`) e nunca lidos. Arquivos
  compose contribuem apenas com nomes de imagens. No texto digitado pelo usuário, e em todo arquivo de contexto quando é
  carregado, as linhas com aparência de segredo são redigidas.
- **Sem contexto inventado.** Arquitetura é sempre só uma inferência ("parece ser…", confiança média no máximo) e exige
  várias pastas lado a lado; do contrário `unknown`. As correções do usuário vencem e são registradas em
  `project.yaml`, então um refresh não as desfaz.
- **Nada é destruído.** Criar sobre um manifesto existente é recusado. Atualizar faz backup do que substitui. Arquivos que
  guardam as palavras do usuário (`business.md`, `constraints.md`) só são reescritos quando o usuário fornece texto novo.
- **Contexto não é permissão.** O prompt coloca o Harness depois das regras do Atlas e o rotula como contexto sem
  autoridade. O que um agente pode fazer é decidido pelo guard de processos, que nunca lê prompts. O Harness não pode
  conceder direitos de sistema de arquivos, rede, Git, runtime ou merge.
- **Worktrees.** O Harness é lido uma vez na raiz do projeto e chega ao agente como texto de prompt. Ele não é copiado
  para os worktrees, e o runtime e o `WorktreeManager` nunca o veem. Mudanças em `.atlas/` seguem o ciclo de vida normal
  do Git.
- Escrever o Harness nunca toca no `.gitignore`. Manter `.atlas/` fora do Git é uma etapa separada que o usuário pode
  desligar na revisão (**ligada por padrão**): ela acrescenta uma entrada `.atlas/` ao `.gitignore` do projeto (criado se
  não existir; nada mais no arquivo muda; ignorada se já estiver ignorado ou fora de um repositório Git; uma falha é
  informada, não é fatal). Ela não desfaz o rastreamento de arquivos já commitados.

## Limitações conhecidas

- Só determinístico; sem análise semântica (LLM), sem entendimento de negócio ou arquitetura além do formato das pastas.
- Sem diff entre um Harness regenerado e um editado: uma atualização substitui os arquivos gerados depois de fazer
  backup deles.
- `harnessStatus` é calculado a partir de `.atlas/` sob demanda (`get_project_harness`), não guardado no workspace.
- `agents/`, `workflows/`, `policies/` e `memory/` são marcadores: nada os lê nem executa ainda.
