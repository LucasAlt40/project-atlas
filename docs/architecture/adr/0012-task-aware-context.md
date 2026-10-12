# ADR 0012 — Contexto ciente da tarefa

**Status:** aceito. Estende a [ADR 0011](0011-harness-knowledge-model.md).

## Por quê

Até a V0.7.2, toda execução recebia o mesmo contexto de projeto: o Harness inteiro, cortado até um orçamento por uma
prioridade fixa. Uma tarefa de recuperação de senha e um ajuste de CI recebiam as mesmas informações, então o orçamento
era gasto com o que a tarefa não precisava e, num Harness grande, podia descartar o que ela precisava.

A V0.8 escolhe a parte do Harness de que uma tarefa precisa:

```text
Repositório ─▶ Harness ─▶ Seletor de Contexto da Tarefa ─▶ Task Context ─▶ PromptBuilder ─▶ Runtime ─▶ Agente
```

O Harness continua sendo a única fonte de conhecimento. O seletor **não cria conhecimento**: ele seleciona, ranqueia e
ordena o que existe. (A única frase que ele pode acrescentar é a honesta, de que uma tecnologia que a tarefa nomeia não
foi identificada no que o Harness guarda.)

## Contexto do Harness vs. Task Context

|                 | Contexto do Harness (V0.7)                 | Task Context (V0.8)                                    |
| --------------- | ------------------------------------------ | ------------------------------------------------------ |
| Escolhido por   | prioridade fixa por categoria              | o texto da tarefa, depois o mesmo orçamento            |
| Igual para toda | tarefa                                     | nunca: muda com a tarefa                               |
| Explicável      | "descartado por tamanho"                   | todo candidato tem uma pontuação e os acertos por trás |
| Papel agora     | fallback e a linha de base para comparação | o padrão                                               |

Os dois são produzidos pelo `HarnessContextBuilder` (`build` e `build_for_task`) a partir dos mesmos itens carregados,
então um achado é redigido de forma idêntica nos dois. As camadas do prompt não mudam: regras do Atlas → (Task Context |
Harness do Projeto) → contexto do projeto → tarefa. O prompt carrega um ou outro, nunca os dois; os runtimes nunca veem
o `.atlas/`.

## Modelo

- `ContextItem` (`domain/task_context.rs`) é uma **projeção** de um achado (ou das restrições, decisões e arquivos de
  negócio do usuário, ou de uma lacuna): `id`, `area`, `category`, `content`, `provenance`, `verification`, `confidence`,
  `evidence`, `source_finding_id`, `tags`. Não copia nada que o achado não guarde. Substituiu o `Item` privado do
  construtor, então há uma representação, não duas.
- **Áreas** (conjunto fechado, 13): `architecture`, `stack`, `modules`, `conventions`, `business`, `testing`,
  `dependencies`, `entry_points`, `ci`, `constraints`, `decisions`, `infrastructure` e `general` (afirmações sobre o
  próprio Harness). `Area::of(FindingCategory)` mapeia as categorias existentes; nada foi adicionado aos achados
  guardados.
- **Tags** são derivadas, nunca guardadas e nunca produzidas por um modelo. `taxonomy.rs` tem um léxico fixo (cerca de
  20 tags: `authentication`, `password`, `api`, `backend`, `frontend`, `ui`, `database`, `migrations`, `testing`,
  `build`, `ci`, `deployment`, `caching`, …) com termos em inglês e português e algumas implicações (uma senha é sobre
  autenticação; um endpoint é backend). As tags de um achado vêm da sua categoria, das suas palavras e das palavras dos
  caminhos da sua evidência (`src/auth/password-reset.ts` → `authentication`, `password`). Uma palavra fora do léxico
  não é uma tag. O mesmo léxico lê a tarefa, então um acerto significa que as mesmas palavras foram pretendidas dos
  dois lados.
- `TaskContextRequest { task, workspace_id, agent_id }`: a tarefa é só o seu texto. Isto não é um gerenciador de tarefas.
- `TaskAnalyzer` (`task_context.rs`) → `TaskSignals { keywords, areas, baseline_areas, tags, intent, technologies }`: as
  palavras são passadas para minúsculas, sem acentos e separadas por camelCase e separadores; stop words e verbos de
  intenção não são palavras-chave; qualquer idioma funciona (palavras que ele não conhece não geram tag); uma tarefa
  vazia não tem sinais.

## Ranking

A pontuação de um item é a soma de pesos centralizados em `Weights`:

| Sinal                                                                 | Peso                  |
| --------------------------------------------------------------------- | --------------------- |
| tag exata da tarefa no item (no máximo duas contam)                   | +50 cada              |
| área que as tags da tarefa pedem, **somente junto com outro acerto**  | +30                   |
| área que orienta qualquer tarefa da intenção (arquitetura para _add_) | +25                   |
| item de stack sem tag de camada (linguagem, runtime, ferramentas)     | +22                   |
| palavra-chave da tarefa no texto do item                              | +25                   |
| categoria que as tags da tarefa pedem                                 | +20                   |
| um caminho da evidência compartilha uma palavra com a tarefa          | +20                   |
| o usuário confirmou, corrigiu ou escreveu (quando algo já casou)      | +40                   |
| verificado (quando algo já casou)                                     | +5                    |
| a camada do item (frontend, backend, banco) não é a camada da tarefa  | −40                   |
| restrição ou decisão                                                  | +100, sempre incluído |

Relevante significa `score ≥ 20`. A regra "área só com outro acerto" existe porque as tags apontam para áreas: sem ela,
todo módulo é "sobre autenticação" assim que a tarefa menciona uma senha.

`SelectionReason { score, matched_areas, matched_tags, matched_categories, matched_paths, matched_keywords,
always_included, penalties }` é guardado para **todo** candidato, incluído ou não, e é o que a prévia mostra. A seleção
é uma função pura: o mesmo Harness e a mesma tarefa sempre dão o mesmo contexto.

## Restrições, decisões, a palavra do usuário, inferência, obsolescência, lacunas

- **Restrições e decisões nunca são julgadas.** São sempre incluídas (já são limitadas), as de segurança primeiro. Isso é
  deliberadamente mais rígido que "manter as relevantes": dividir o texto do usuário para decidir do que uma tarefa
  precisa é onde uma restrição crítica se perderia.
- **A palavra do usuário** supera a análise (+40) e é contada em `O QUE O USUÁRIO NOS DISSE`. Um conflito em que uma
  afirmação é do usuário é contado como _conflito detectado; a decisão do usuário é autoritativa para a orientação do
  projeto; a evidência do repositório pode indicar divergência_, quer o usuário já tenha escolhido um lado ou não. O
  Atlas não o resolve.
- **Inferência continua inferência.** `Possível arquitetura: … (confiança média; não verificada; evidência: …)`; um
  fato verificado e uma inferência nunca ficam no mesmo bloco.
- Achados **obsoletos** nunca entram em `O QUE SABEMOS`: aparecem em `O QUE PODE ESTAR DESATUALIZADO` quando relevantes,
  com a data da última verificação, e a linha de status diz que o projeto mudou.
- **Não encontrado / desconhecido** continuam distintos. Uma tecnologia que a tarefa nomeia (`Redis`) e que nenhum
  achado, rótulo ou caminho de evidência menciona vira `Não encontrado: Redis não foi identificado nas evidências
analisadas. Isso não prova que o Redis não é usado.` — nunca "o projeto não usa Redis".

## Orçamento e o que ficou de fora

O orçamento é o mesmo de ~6.000 caracteres, mas agora é gasto **depois** do ranking. Ordem: restrições de segurança,
decisões, outras restrições, as demais palavras do usuário, fatos verificados, arquitetura, módulos, convenções,
testes, dependências, outras inferências, desconhecido/não encontrado, desatualizado; dentro de uma classe, por
pontuação. Restrições e decisões são mantidas mesmo além do orçamento.

Nada é escondido: o texto termina com `NÃO SELECIONADO (julgado não relacionado a esta tarefa; …): 3 módulos, 2
infraestrutura` e, quando itens relevantes não couberam, `O contexto foi truncado. Omitido por tamanho: 4 achado(s) de
testes, …`.

## Fallback e metadados

Se uma seleção não puder ser feita (um Harness de antes de existir conhecimento; uma tarefa sem sinais legíveis; nada
relevante) usa-se o **contexto inteiro do Harness** e o motivo é registrado (`harness_without_knowledge`,
`task_without_signals`, `no_relevant_knowledge`). Uma execução nunca roda sem contexto em silêncio: `Execution.context` /
`StoredExecution.context` registra `mode` (`task_aware` | `fallback`), `totalHarnessCharacters`,
`selectedContextCharacters`, `selectedItems`, `omittedItems` e `fallbackReason`. O texto é o prompt que a execução já
guarda; não é duplicado.

## Prévia

`preview_task_context(request)` devolve o mesmo `TaskContext` que uma execução usaria (a mesma instância do construtor),
com os sinais, cada entrada com o seu motivo, as áreas incluídas e excluídas, os tamanhos frente ao Harness inteiro e o
texto exato. Não lê nem escreve nada. A UI o mostra, com debounce, acima do campo de entrada; vê-lo é opcional e ele não
pode ser editado nesta versão. O fingerprint de um projeto (para a obsolescência) é reaproveitado por cinco segundos para
que digitar não reescaneie o projeto.

## Segurança

`TaskContextService` e `taxonomy` são puros: sem disco, sem comandos, sem rede, sem permissões, sem worktrees. Um teste
lê o código-fonte deles e falha se aparecer `std::fs`, `std::process`, `std::net`, `Command::new`, um store ou um
scanner. O Task Context mantém o mesmo aviso de autoridade do contexto do Harness: é pano de fundo, não concede
permissão de sistema de arquivos, processo, rede, Git ou merge, e perde para as regras do Atlas e para a tarefa. Ele lê
apenas o que a V0.7.2 já lia (o Harness persistido e o fingerprint existente); não há um segundo scanner.

## Por que não RAG, embeddings ou um modelo

O primeiro seletor deve ser **testável e explicável**: para cada item podemos dizer quais palavras e quais pesos o
colocaram dentro ou fora, e a mesma entrada dá a mesma saída. Um modelo ou um banco vetorial adicionaria custo,
latência, não determinismo e uma segunda opinião, não auditável, sobre o que o agente não pode ver. Uma versão futura
pode acrescentar _re-ranqueamento semântico_ sobre esta seleção determinística de candidatos; o `SelectionReason` é o
contrato que uma camada dessas teria de estender.

## Limites conhecidos

- O léxico é pequeno e feito à mão; um projeto cujo vocabulário não está nele recebe só o contexto base (arquitetura,
  convenções, testes, stack), mais o que suas palavras-chave casarem em rótulos e caminhos.
- Restrições, decisões e texto de negócio são itens dos arquivos do usuário, não divididos por frase; um arquivo de
  negócio que menciona uma palavra-chave da tarefa é incluído inteiro (cada campo é limitado a 1.500 caracteres).
- A relevância olha o Harness, não o código: uma tarefa em um módulo que a análise não encontrou não recebe contexto
  específico do módulo.
