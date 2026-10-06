# 3. Harness e contexto: como o Atlas conhece um projeto

> Parte do [guia do Atlas](README.md). ADRs: [0010](../architecture/adr/0010-project-harness.md),
> [0011](../architecture/adr/0011-harness-knowledge-model.md), [0012](../architecture/adr/0012-task-aware-context.md).

O **Harness** é o que o Atlas _sabe_ sobre um projeto, gravado **dentro do projeto** em `.atlas/` (versionável com o
Git do projeto, legível por humanos, independente de provedor/modelo/runtime). Ele é **contexto, nunca permissão**:
quem decide o que um agente pode fazer é o guard de processos, que nunca lê prompts.

Lema: **Evidência > Inferência > Palpite** e **Desconhecido > Inventado**. O Atlas nunca ensina a um agente algo sobre o
projeto que não consiga apontar a evidência — e diz o que não sabe.

## Fluxo geral

```text
Usuário → "Initialize Project"
  ProjectScanner ─▶ ScanSnapshot ─▶ DeterministicAnalyzer ─────────────┐
       │                                                               ├▶ ConsistencyChecker ─▶ conflitos
       └▶ RepositorySampler ─▶ EvidenceBundle ─▶ SemanticAnalyzer (opcional, via agente)┘
                       findings + conflitos + diff ─▶ REVISÃO HUMANA ─▶ HarnessGenerator ─▶ HarnessStore ─▶ <projeto>/.atlas/

Execução ─▶ HarnessContextBuilder (lê .atlas/) ─▶ Task Context (seleção p/ a tarefa) ─▶ PromptBuilder ─▶ runtime ─▶ agente
```

Um **modelo nunca escreve o Harness**: a revisão humana é obrigatória.

## Como o Harness é gerado

### 1. Escaneamento (`infrastructure/project_scanner.rs`)

Único código que lê a pasta do projeto para análise: nomes de entradas + texto de uma _whitelist_ curta de manifestos.

- **Analisar é ler.** Nada é executado, compilado, instalado, iniciado ou baixado.
- Symlinks não são seguidos; pastas geradas (`node_modules`, `target`, `dist`, `.git`…) são anotadas, não percorridas.
- Limites: profundidade 4, 5 000 entradas (atingir = análise **parcial**, registrada), 256 KiB por manifesto.
- **Segredos nunca entram**: `.env` é anotado só pelo nome (`.env detected`) e **nunca lido**; compose contribui apenas
  com nomes de imagens; todo texto digitado pelo usuário e todo arquivo de contexto carregado tem linhas com cara de
  segredo redigidas.

### 2. Análise determinística (`harness/analyzer/`)

`ProjectAnalyzer` é o trait único; `DeterministicAnalyzer` compõe analisadores por regras: Git, stack, estrutura, CI,
comandos de build, pontos de entrada, dependências (curadas, não centenas), dados (ORM/migrations), integrações
("possível", nunca prova), testes (framework, arquivos, tipos, comandos e cobertura são fatos _separados_), convenções,
módulos e arquitetura. **Comandos de build são observados nos manifestos e nunca executados.**

Cada afirmação é um **`Finding`**: `category`, `key`, `value`, `confidence`, `origin`, `reason?` e **`evidence`** (lista
de `{source, field}`). Sem evidência, a afirmação é rejeitada (nunca rebaixada para "baixa confiança"); a única exceção é
`architecture: unknown` do próprio Atlas (`origin: generated`).

| Origem (`origin`) | Significado                                   | Confiança                       |
| ----------------- | --------------------------------------------- | ------------------------------- |
| `user_corrected`  | O usuário substituiu o valor                  | Mais alta; sobrevive ao refresh |
| `user_confirmed`  | O usuário confirmou uma inferência            | Alta; sobrevive ao refresh      |
| `fact`            | Lido do repositório (campo de `package.json`) | Alta                            |
| `inference`       | Concluído pela forma do repositório/modelo    | No máximo média                 |
| `generated`       | Declaração do próprio Atlas                   | Não é conhecimento, só rótulo   |

Confiança: **alta** = evidência direta; **média** = vários sinais + interpretação (ex.: `domain/`+`application/`+
`infrastructure/` lado a lado → _possível_ arquitetura limpa); **baixa** = sinais fracos. Um modelo **nunca** produz
`high` (limitado a `medium`). **Baixa nunca chega ao agente como conhecimento** — aparece em "o que não sabemos".

Verificação é outro eixo: `verification` = `unverified` | `verified` (+ `verifiedAt`, `method`) | `stale`. Um fato é
verificado pelo próprio arquivo de onde foi lido; uma inferência fica `unverified`. `command_execution` está reservado
para o futuro; hoje **nada é executado** para verificar.

### 3. Análise semântica (opcional, por pedido)

Escolhendo-se um agente, o modelo recebe **só** o prompt: árvore de pastas limitada, os fatos determinísticos e um
conjunto limitado de arquivos (`RepositorySampler`: README, manifestos, pontos de entrada, docs, alguns arquivos de
código/teste por camada; nunca `.env`/chaves; linhas secretas redigidas).

- O runtime roda **sem ferramentas** (`text_only`; Claude com `--tools ""`). Runtime que não consegue é recusado.
- A resposta **precisa ser JSON em esquema fixo** (campos desconhecidos rejeitados). Prosa nunca é interpretada.
- Cada afirmação deve citar caminhos que **realmente foram enviados**, com categoria/chave/tamanho permitidos; senão é
  descartada e contada (`semantic.rejected`). O resultado lista exatamente quais arquivos foram enviados.
- Falha semântica **não** derruba a análise: os achados determinísticos valem.

### 4. Consistência

`ConsistencyChecker` registra contradições **sem escolher**: modelo × fato, README/docs × fatos (versões), correção do
usuário que o repositório já ultrapassou. Um `Conflict` guarda as duas afirmações com suas evidências; a decisão do
usuário resolve e vence.

### 5. Revisão e escrita

O usuário revisa (desmarca, corrige, adiciona contexto de negócio/restrições/decisões) e o `HarnessGenerator` +
`HarnessStore` escrevem `.atlas/`. O `HarnessStore` é o **único** código que escreve ali: backup antes de substituir,
renomeação atômica, nunca através de symlink, caminhos não saem de `.atlas/`. Criar sobre um manifesto existente é
recusado; atualizar faz backup. Modos: criar / usar existente / atualizar existente.

Opcionalmente (ligado por padrão) acrescenta uma linha `.atlas/` ao `.gitignore` do projeto (só isso; não "destrackeia"
nada já commitado; falha é reportada, não fatal).

## O que fica gravado em `.atlas/`

```text
.atlas/
├── project.yaml                  metadados + decisões do usuário (correções, exclusões, confirmações)
├── knowledge/findings.yaml       GERADO: achados brutos + evidência, conflitos, info da análise, fingerprint
├── context/
│   ├── architecture.md  stack.md  conventions.md  testing.md     GERADOS (com banner dizendo isso)
│   └── business.md  constraints.md  decisions.md                 DO USUÁRIO
├── agents/  workflows/  policies/  memory/                       placeholders (nada lê/executa ainda)
└── backups/<timestamp>/          o que uma atualização substituiu
```

Regras de propriedade:

- **Gerados** são substituídos na atualização, só depois de o usuário ver o diff, e com backup.
- **Do usuário** (`business.md`, `constraints.md`, `decisions.md`) só são escritos se ausentes ou se o usuário mudou
  aquele campo na revisão. Formato escrito à mão que o Atlas não sabe reproduzir é **deixado intacto** e reportado
  (ainda é enviado aos agentes como palavra do usuário).
- Guardam-se os achados **brutos**, não as escolhas aplicadas — assim o refresh compara bruto×bruto e reaplica as
  correções.

## Frescor: fingerprint e _staleness_

`knowledge/findings.yaml` guarda um **fingerprint** do projeto: um digest geral + digests por parte (manifestos e
lockfiles, configuração, CI e containers, `README*`, e a _existência_ de pastas até 2 níveis). Arquivos de código comuns
**não** contam (editar `Button.tsx` é irrelevante; criar `src/infrastructure/` não é). FNV-1a sobre texto com quebras
de linha normalizadas; segredos e pastas geradas excluídos; nada é executado.

Na leitura, o Atlas compara o fingerprint guardado com o projeto agora. Uma mudança marca como `stale` **só** os
achados cuja evidência está no caminho alterado; afirmações do usuário nunca ficam stale. Stale vai para "O QUE PODE
ESTAR DESATUALIZADO" com a data da última verificação. Sem fingerprint → `NeedsReview` (`no_fingerprint`), não stale.

Saúde (`HarnessHealth`), do mais forte ao mais fraco: `Stale > Conflicted > Partial > NeedsReview > Healthy`.

**Refresh** (`refresh_project_harness`): `confirm=false` reanalisa e devolve diff + conflitos **sem escrever nada**;
`confirm=true` aplica (gerados atualizados com backup; correções/confirmações/exclusões preservadas; arquivos do
usuário intactos). O Atlas pode ir de Angular 18 para 21 sem perder contexto de negócio ou decisões.

**Conhecimento negativo** (`negative.rs`): _"Not found"_ = busca limitada que voltou vazia (diz onde procurou e que não
prova ausência); _"Unknown"_ = o que nada disponível consegue estabelecer. O contexto nunca afirma ausência.

## Como o contexto chega ao agente

`HarnessContextBuilder` lê `.atlas/` na raiz do projeto e produz um texto compacto, **limitado a ~6 000 caracteres**,
com segredos redigidos. Ele **começa dizendo que é contexto, sem autoridade** (não concede acesso a arquivos,
processos, rede, Git ou merge; perde para as regras do Atlas e para a tarefa).

Forma do texto (V0.7.2):

```text
HARNESS STATUS            última análise, notas de stale/conflito
O QUE SABEMOS             fatos verificados (fato sem verificação é dito como "candidato")
O QUE INFERIMOS           sempre "não verificado"
O QUE O USUÁRIO DISSE     confirmações dizem "inferido pelo Atlas, confirmado pelo usuário"
O QUE NÃO SABEMOS         lacunas, sinais fracos, conflitos, análise parcial, "Not found"/"Unknown"
O QUE PODE ESTAR DESATUALIZADO
```

Camadas de contexto, da mais externa à mais interna: contexto global do usuário → Harness do projeto → contexto do
workspace → personalidade → contexto de execução → tarefa. (A Política de Segurança **não** é camada: é aplicada pelo
guard e nenhum contexto a altera.)

### Task Context (V0.8): só a parte que a tarefa precisa

Em vez de enviar o Harness inteiro, o Atlas escolhe a parte relevante com **regras determinísticas e explicáveis** (sem
modelo, sem embeddings): o `TaskContextService` **não cria conhecimento**, só seleciona, ranqueia e ordena.

- `ContextItem` = projeção de um achado (ou de restrição/decisão/negócio/lacuna): `area`, `category`, `content`,
  `provenance`, `verification`, `confidence`, `evidence`, `tags`.
- **13 áreas** fechadas: `architecture`, `stack`, `modules`, `conventions`, `business`, `testing`, `dependencies`,
  `entry_points`, `ci`, `constraints`, `decisions`, `infrastructure`, `general`.
- **Tags** derivadas (nunca guardadas, nunca de modelo) por um léxico fixo de ~20 tags (`authentication`, `password`,
  `api`, `backend`, `frontend`, `database`, `testing`, `ci`, `deployment`, `caching`…) com termos em inglês e português.
  O mesmo léxico lê a tarefa.
- **Pontuação** (soma de pesos centralizados em `Weights`): tag exata +50 (máx. 2), palavra-chave +25, área pedida
  (só junto de outro match) +30, área que orienta a intenção +25, caminho de evidência compartilha palavra +20,
  palavra do usuário +40, verificado +5, camada errada −40; **restrições e decisões: +100, sempre incluídas**.
  Relevante = pontuação ≥ 20.
- Cada candidato (incluído ou não) guarda `SelectionReason` com os motivos; a seleção é uma **função pura**: mesmo
  Harness + mesma tarefa = mesmo contexto.
- Ordem sob o orçamento (~6 000 chars, gasto **depois** do ranking): restrições de segurança, decisões, outras
  restrições, outras palavras do usuário, fatos verificados, arquitetura, módulos, convenções, testes, dependências,
  outras inferências, desconhecido/not found, desatualizado. Restrições/decisões ficam mesmo acima do orçamento.
- **Nada é escondido**: o texto termina com `NOT SELECTED (...)` e, se algo relevante não coube, `Context was truncated.
Omitted for size: …`.
- Tecnologia citada na tarefa e ausente do Harness vira _"Not found: Redis was not identified… This does not prove that
  Redis is not used."_ — nunca "o projeto não usa Redis".
- **Fallback**: se não dá para selecionar (`harness_without_knowledge`, `task_without_signals`, `no_relevant_knowledge`)
  usa-se o contexto do Harness inteiro e o motivo é registrado. **Nunca roda silenciosamente sem contexto.**
  `Execution.context` guarda `mode` (`task_aware` | `fallback`), tamanhos, itens selecionados/omitidos e o motivo.
- `preview_task_context` mostra, antes de rodar, exatamente o contexto que seria usado (com sinais e motivo de cada
  item); só lê, não altera, e não é editável nesta versão.
- `TaskContextService` e `taxonomy` são **puros**: um teste lê o código-fonte e falha se aparecer `std::fs`,
  `std::process`, `std::net`, `Command::new`, store ou scanner.

Em passos de workflow, o Task Context é selecionado por um **`context_query`** = tarefa + rótulo do passo + instruções
do passo — assim um passo de Backend e um de Frontend da mesma tarefa recebem partes diferentes do Harness.

## Limitações conhecidas

- Análise semântica só funciona com runtimes que rodam sem ferramentas (hoje, Claude); não entra no uso/custo.
- O léxico do Task Context é pequeno e feito à mão: vocabulário fora dele recebe só contexto base.
- A relevância olha o Harness, não o código: tarefa em módulo não encontrado pela análise não recebe contexto específico.
- Staleness compara arquivos/pastas relevantes, não "entende" a mudança; refresh é iniciado pelo usuário.
- `agents/`, `workflows/`, `policies/`, `memory/` são placeholders.
