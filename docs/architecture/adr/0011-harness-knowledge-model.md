# ADR 0011 — Modelo de conhecimento do Harness, evidências e análise semântica

**Status:** aceito. Estende a [ADR 0010](0010-project-harness.md).

## Princípio

`Evidência > Inferência > Palpite`, e `Desconhecido > Inventado`. O Atlas nunca ensina a um agente algo sobre o projeto
para o qual não consiga apontar evidência, e diz o que não sabe. Um agente que recebe só o contexto do Harness e a sua
tarefa deve conseguir distinguir quatro coisas:

```text
O QUE SABEMOS (lido do repositório)        fatos
O QUE INFERIMOS (candidatos, não confirmados)   inferências de confiança média
O QUE O USUÁRIO NOS DISSE                  negócio, restrições, decisões, correções, confirmações
O QUE NÃO SABEMOS                          lacunas, sinais fracos, conflitos, análise parcial
```

## Pipeline

```text
ProjectScanner ─▶ ScanSnapshot ─▶ DeterministicAnalyzer ──────────────┐
      │                                                                 ├▶ ConsistencyChecker ─▶ conflitos
      └▶ RepositorySampler ─▶ EvidenceBundle ─▶ SemanticAnalyzer (opt-in)┘
                                                          │
                       achados + conflitos + diff ──▶ REVISÃO HUMANA ──▶ HarnessGenerator ─▶ HarnessStore ─▶ .atlas/
.atlas/ ─▶ HarnessContextBuilder (resumo compacto, com orçamento) ─▶ PromptBuilder ─▶ Runtime ─▶ Agente
```

`ProjectAnalyzer` é a única trait (`analyze_project(&AnalysisInput) -> AnalysisOutput`). O `DeterministicAnalyzer`
compõe `SnapshotAnalyzer`s baseados em regras (Git, stack, estrutura, CI, comandos de build, pontos de entrada,
dependências, testes). O `SemanticAnalyzer` é a segunda implementação. A revisão é obrigatória: um modelo nunca escreve
o Harness.

## Modelo de conhecimento

Um `Finding` é `category`, `key`, `value`, `confidence`, `origin`, um `reason` opcional e **`evidence`: uma lista de
`{source, field}`**. Toda afirmação que veio de análise tem pelo menos uma evidência; a única exceção é o
`architecture: unknown` do próprio Atlas, cuja origem é `generated`.

| Origem           | Significado                                      | Confiança                     |
| ---------------- | ------------------------------------------------ | ----------------------------- |
| `user_corrected` | O usuário substituiu um valor                    | Máxima; sobrevive ao refresh  |
| `user_confirmed` | O usuário confirmou uma inferência               | Alta; sobrevive ao refresh    |
| `fact`           | Lido do repositório (campo do `package.json`)    | Alta                          |
| `inference`      | Concluído pelo formato ou por um modelo          | No máximo média               |
| `generated`      | Afirmação do próprio Atlas, sem vir de evidência | Não é conhecimento, só rótulo |

Confiança: **high** = evidência direta e inequívoca; **medium** = vários sinais mais interpretação (`domain/` +
`application/` + `infrastructure/` lado a lado → _possível_ arquitetura limpa); **low** = sinais fracos. Um modelo nunca
pode produzir `high` (é limitado a `medium`). **Low nunca chega a um agente como conhecimento**: o construtor de
contexto o lista em "o que não sabemos" como sinal fraco.

As categorias agora incluem dependências (curadas, não centenas), dados (ORM, migrações), integrações (sempre
"possíveis": uma dependência sugere mas não prova o uso), comandos de build/execução (observados nos manifestos,
**nunca executados**), pontos de entrada, testes (framework, arquivos de teste, tipos de teste, comandos e cobertura são
fatos _separados_: um framework instalado não prova nada além disso), convenções (p. ex. `*.spec.ts` com arquivos de
exemplo como evidência), módulos e arquitetura.

## Análise semântica

Opt-in por requisição, escolhendo um agente. Um modelo recebe **somente** o prompt: uma árvore de pastas limitada, os
fatos determinísticos e um conjunto limitado e selecionado de arquivos (`RepositorySampler`: README, manifestos, pontos
de entrada, docs, alguns arquivos de código e de teste por pasta de camada; nunca `.env`/chaves/credenciais; linhas com
aparência de segredo redigidas). Os limites são explícitos (`SampleLimits`, `ScanLimits`); atingir um torna a análise
**parcial**, registrado no Harness.

- O runtime é executado **sem ferramentas** (`RuntimeRequest.text_only`; Claude `--tools ""`). Um runtime que não
  consegue (OpenCode) é recusado (`SemanticUnsupported`). O guard de processos ainda o inicia, sob a política do próprio
  agente.
- A resposta deve ser JSON em um esquema fixo (campos desconhecidos rejeitados). Prosa nunca é interpretada:
  `SemanticAnalysisFailed`. Cada afirmação deve citar caminhos que de fato foram enviados, com categoria/chave/tamanho
  permitidos; caso contrário é descartada e contada. O resultado lista exatamente quais arquivos foram enviados.
- Uma etapa semântica que falha não faz a análise falhar; os achados determinísticos permanecem.
- Não é registrada em conversas nem em uso (limitação).

## Consistência

O `ConsistencyChecker` registra contradições sem escolher: modelo vs. fato, README/docs vs. fatos (versões), e uma
correção do usuário que o repositório já superou. Um `Conflict` guarda as duas afirmações com suas evidências; a
decisão do usuário (uma correção para aquele achado) o resolve e prevalece. `HarnessHealth`: `Conflicted` > `Partial` >
`NeedsReview` (inferências médias não confirmadas, ou um Harness da V0.7 sem conhecimento) > `Healthy`; `Stale` é
modelado, não detectado.

## Arquivos e o que o Atlas pode sobrescrever

```text
.atlas/
├── project.yaml                 metadados + as decisões do usuário (correções, exclusões, confirmações)
├── knowledge/findings.yaml      GERADO: achados brutos com evidências, conflitos, informações da análise
├── context/architecture.md stack.md conventions.md testing.md      visões GERADAS (o aviso no topo diz isso)
├── context/business.md constraints.md decisions.md                 DO USUÁRIO
├── agents/ workflows/ policies/ memory/                            marcadores
└── backups/<timestamp>/         o que uma atualização substituiu
```

- Os arquivos gerados são substituídos na atualização, só depois que o usuário viu o diff, e com backup.
- Os arquivos do usuário só são escritos se não existirem ou se o usuário mudou aquele campo na revisão. Um campo vazio
  mantém o que o arquivo diz. O `business.md` é lido por seus quatro títulos; um formato escrito à mão que o Atlas não
  consegue reproduzir é **deixado intocado** e informado (ainda é enviado aos agentes como palavra do usuário). Do texto
  digitado são removidas as linhas com aparência de segredo.
- São guardados os achados brutos, não as escolhas do usuário aplicadas a eles, para que um refresh possa comparar bruto
  com bruto e reaplicar as correções. Achados de modelo são carregados por um refresh enquanto a evidência deles ainda
  existir.

## Refresh

`refresh_project_harness(confirm=false)` reanalisa (determinístico) e devolve diff + conflitos, sem escrever nada.
`confirm=true` aplica: arquivos gerados atualizados (backup), correções/confirmações/exclusões mantidas, arquivos do
usuário inalterados. O Atlas pode assim ir do Angular 18 ao 21 sem perder o contexto de negócio nem as decisões.

## Construção do contexto

O `HarnessContextBuilder` produz um _Resumo de Contexto do Projeto_ compacto sob um orçamento (padrão 6.000
caracteres). Os itens são mantidos por prioridade — restrições/decisões (nunca descartadas: cada uma é limitada por si),
fatos verificados e stack, o que pode estar desatualizado e conflitos, arquitetura, módulos, convenções, testes,
inferências, negócio, desconhecidos — e o que é omitido é nomeado no texto e registrado na execução. Cada item guarda sua
área para que um filtro posterior ciente da tarefa possa selecionar itens sem mudar o armazenamento (não implementado).
O texto começa declarando que é contexto, não autoridade. Camadas, da mais externa para a mais interna: contexto global
do usuário → Harness do projeto → contexto do workspace → personalidade → contexto da execução → tarefa. A Política de
Segurança não é uma camada: é aplicada pelo guard de processos e não pode ser alterada por nenhum contexto.

## Limitações conhecidas

- A análise semântica só funciona por runtimes que conseguem rodar sem ferramentas (Claude); não é contabilizada no uso.
- Convenções, módulos e fluxo de dados além das regras listadas vêm só do modelo, sempre como inferências média/baixa.
- A detecção de conflitos cobre fato-vs-modelo, versões docs-vs-fato e usuário-vs-fato-novo; não toda contradição
  possível.
- A obsolescência é detectada comparando arquivos e pastas relevantes, não entendendo a mudança: a edição de um arquivo
  de código comum é invisível para ela, e uma mudança relevante marca só os achados cuja evidência está ali (o resto do
  Harness ainda diz que o projeto mudou). O refresh é iniciado pelo usuário. O contexto de negócio nunca é inferido.
- Nada é verificado executando um comando na V0.7.2; `command_execution` existe no modelo para uma versão futura.

## V0.7.2 — Verificação, obsolescência e conhecimento negativo

O Harness precisa conseguir dizer o que sabe, como sabe, quando, e se isso ainda pode valer.

**Proveniência e verificação são eixos diferentes.** `origin` diz quem fez uma afirmação (`fact` lido de um arquivo do
repositório, `inference`, `user_confirmed`, `user_corrected`); `verification` diz se ela foi _checada_ (`unverified` |
`verified` + `verifiedAt` + `method` | `stale`). Um fato é verificado pelo próprio arquivo do repositório de onde é lido
(`repository_file`); uma inferência é `unverified` até que algo a confira. `command_execution` é reservado para uma
versão futura que execute um comando sob pedido; a V0.7.2 nunca executa nada para obter uma verificação. O usuário
avalizar uma inferência a torna `user_confirmed` com `originalOrigin: inference` — ela nunca vira um fato. Uma correção
substitui o valor, então a verificação anterior deixa de se aplicar. O conhecimento escrito antes da V0.7.2 é lido como
era: seus fatos são marcados como verificados no momento da análise.

**Evidência não é opcional na análise.** Qualquer achado não gerado sem pelo menos uma fonte de evidência é rejeitado
(contado em `semantic.rejected`), nunca rebaixado para confiança baixa.

**Desconhecido vs. não encontrado** (`negative.rs`). _Não encontrado_ é uma busca limitada que voltou vazia (banco de
dados, framework de testes, CI, infraestrutura) e diz onde procurou e que isso não prova ausência. _Desconhecido_ é o
que nada disponível consegue estabelecer (implantação em produção, se os comandos funcionam, se uma arquitetura
inferida ainda é respeitada). O contexto diz "Não encontrado: …" / "Desconhecido: …" em O QUE NÃO SABEMOS e nunca afirma
ausência nem transforma um dos dois em fato.

**Fingerprint do projeto** (`fingerprint.rs`). Guardado em `knowledge/findings.yaml` (`analysis.fingerprint`) como um
digest mais digests por parte, para que uma verificação posterior possa nomear o que mudou. Partes: manifestos e
lockfiles, configuração (`tsconfig*`, `vite.config.*`, `angular.json`, `tauri.conf.json`, linters, toolchains), CI e
contêineres (`.github/workflows`, `Dockerfile`, compose, `docker/`), `README*`, e a _existência_ de pastas até dois
níveis de profundidade. Arquivos de código comuns não são partes: editar `Button.tsx` não é relevante, adicionar
`src/infrastructure/` é. Os digests são FNV-1a sobre texto com fins de linha normalizados (estável entre plataformas e
versões do Rust; não é uma fronteira de segurança), os caminhos usam `/`, links nunca são seguidos, segredos (`.env`,
chaves) e pastas geradas (`node_modules`, `target`, `dist`, `.git`…) são excluídos, e nada é executado. Arquivos que não
são analisados (lockfiles, configuração) só são digeridos pelo scanner (primeiros 4 MiB).

**Obsolescência.** `summarize` e o construtor de contexto comparam o fingerprint guardado com o projeto como está agora.
Uma mudança marca como `stale` apenas os achados cujo caminho de evidência é igual, contém ou está contido em um
caminho alterado; afirmações do usuário nunca são marcadas (não são afirmações sobre o repositório). O conhecimento
obsoleto é movido para O QUE PODE ESTAR DESATUALIZADO no contexto, com a data da última verificação, e o STATUS DO
HARNESS diz que o projeto mudou. Nada é apagado nem reescrito: a obsolescência é calculada na leitura. Um Harness sem
fingerprint é `NeedsReview` (`no_fingerprint`), não obsoleto, porque nada diz se ele mudou. Se o projeto não puder ser
escaneado, a obsolescência não é declarada.

**Precedência da saúde**, da mais forte à mais fraca: `Stale > Conflicted > Partial > NeedsReview > Healthy`. Stale
supera as demais porque os outros julgamentos descrevem um projeto que não existe mais como foi analisado, e atualizar é
o primeiro passo (pode dissolvê-los); todo motivo continua listado. `Healthy` significa não obsoleto, sem conflito não
resolvido, análise completa e nenhuma inferência relevante aguardando confirmação.

**Refresh e conhecimento humano** não mudam: o refresh mostra uma prévia primeiro (agora com as mudanças relevantes),
escreve só depois da confirmação, com backups, e nunca sobrescreve `business.md`, `constraints.md` ou `decisions.md`
nem as correções, confirmações e exclusões do usuário. Depois de um refresh aplicado o novo fingerprint é guardado e o
Harness deixa de estar obsoleto. Contradições entre a evidência do repositório e o conhecimento do usuário continuam
sendo conflitos para o usuário decidir.

**Formato do contexto.** `STATUS DO HARNESS` (última análise, notas de obsolescência/conflito) → O QUE SABEMOS (só fatos
verificados; um fato sem verificação é contado como candidato) → O QUE INFERIMOS (sempre "não verificado") → O QUE O
USUÁRIO NOS DISSE (as confirmações dizem "inferido pelo Atlas, confirmado pelo usuário") → O QUE NÃO SABEMOS → O QUE
PODE ESTAR DESATUALIZADO. O construtor recebe um `ProjectScanner` opcional para calcular o fingerprint do projeto em
tempo de execução; sem ele, nenhuma obsolescência é declarada.
