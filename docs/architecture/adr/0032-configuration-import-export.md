# ADR 0032 — Compartilhar configurações: exportar e importar como arquivo

Status: aceito e implementado. Baseia-se na [ADR 0002](0002-ports-and-adapters.md) (portas e adaptadores), na
[ADR 0004](0004-runtimes.md) (runtimes e modelos) e na [ADR 0030](0030-workflow-templates-and-tools.md) (templates
salvos); não muda nenhuma das suas invariantes.

## Contexto

O que uma pessoa constrói no Atlas (uma personalidade, um agente, um workflow, um template salvo) vivia só no
`config.json` daquela instalação. Entregar um agente a um colega, mudar para outra máquina ou manter um backup
significava refazer tudo à mão. Os registros guardados não servem para um arquivo: carregam ids locais, ids de
workspace, horários, versões e histórico que não significam nada em outro lugar, e seu formato muda com o banco de
dados. Um arquivo trocado entre pessoas precisa sobreviver a ambos.

## Decisões

1. **JSON, em um formato próprio e versionado.** `serde_json` é o que o núcleo e a webview já falam. O JSON não tem
   tipagem implícita, âncoras ou aliases para nos surpreender (como o YAML), lê-se e edita-se à mão, e um parse com
   limite de tamanho de texto não confiável é simples de raciocinar. O TOML não expressa bem um grafo. O arquivo é `{
format: "atlas-config", schemaVersion, exportedAt, source, roots, items, external }`, gravado como
   `<nome>.atlas.json`.
2. **O arquivo é o seu próprio contrato, não um despejo do armazenamento.** Cada tipo tem seus próprios tipos `*Spec`
   (`application/portability/spec.rs`) e um mapeamento explícito de e para o domínio, com `match`es exaustivos, de modo
   que um campo renomeado ou dividido no modelo guardado seja absorvido em um só lugar e arquivos antigos continuem
   legíveis. Nenhum id local é exportado: os itens se nomeiam com os próprios `ref`s do arquivo (`agent-2`); os ids de
   passos e arestas de um workflow são os únicos ids mantidos, porque são do próprio grafo. Ids de workspace, caminhos de
   projeto, horários, versões, histórico de reparo de rotas, viewport do canvas, execuções, conversas e concessões MCP
   ficam de fora.
3. **Versionamento.** `schemaVersion` é conferido primeiro: uma mais nova é recusada dizendo isso, uma mais antiga é
   trazida adiante pela cadeia `UPGRADES` (vazia enquanto o esquema é 1; o mecanismo é testado com um passo de
   substituição). Adicionar um campo opcional não muda a versão e campos desconhecidos são ignorados; renomear ou
   mudar o sentido de um muda, com um passo de upgrade.
4. **Uma `Category` por tipo** (`personality`, `agent`, `workflow`, `workflow_template`), por trás de `category(kind)`.
   Uma categoria é dona de verificar sua spec, exportar, inspecionar e gravar; o fluxo (`export.rs`, `inspect.rs`,
   `apply.rs`) não sabe o que é um agente. Um novo tipo é uma nova variante de `Kind` e uma nova categoria. Não há
   mecanismo de plugins: quatro implementações de uma trait bastaram.
5. **Importar nunca é "desserializar e salvar"**: parse (tamanho, JSON, identidade, versão, formato, referências,
   specs) → _prévia_ (`preview_config_import`, não muda nada) → decisões → _importar_ (`import_configs`) em um único
   `ConfigRepository::modify`, de modo que a instalação tenha tudo ou nada, inclusive quando o salvamento falha. O
   arquivo é lido de novo contra o que existe aqui _no momento da importação_; a prévia é um conselho, não um bilhete.
6. **Os conflitos são decididos pela pessoa, por item**: `create` (novo id local; um nome já usado ganha ` (2)`),
   `use_existing`, `replace`, `skip`. Um item é casado pelo nome (uma personalidade embutida também pelo seu id, que
   significa a mesma coisa em todo lugar). Conteúdo igual ao que existe aqui tem como padrão `use_existing` (então
   importar um arquivo duas vezes não acrescenta nada); um casamento de nome com outro conteúdo tem como padrão `create`;
   nada é sobrescrito sem `replace`, que a UI faz a pessoa confirmar item por item. Um casamento sem decisão é recusado
   (`decision_required`). Quando duas ou mais configurações daqui têm o mesmo nome no mesmo escopo, o casamento é sempre
   a primeira salva, e a prévia avisa (`same_name`, com a contagem) antes de a pessoa reaproveitar ou substituir. Um
   workflow com uma execução em andamento ou interrompida não pode ser substituído: a prévia não oferece `replace` para
   ele e diz por quê (`workflow_running`), deixando `create`, `use_existing` e `skip`; pedido assim mesmo, a importação
   recusa. Testes: `two_configurations_with_the_same_name_here_are_matched_to_the_first_and_the_person_is_told` e
   `a_workflow_with_a_run_in_progress_cannot_be_replaced`.
7. **Dependências.** Um arquivo carrega o que os seus itens precisam (a personalidade de um agente, os agentes de um
   workflow) a menos que seja exportado sem isso, caso em que `external` os nomeia e quem importa vincula cada um a um
   dos seus (`bindings`). O candidato de mesmo nome é sugerido e pré-selecionado na prévia, mas só é aplicado quando a
   pessoa importa. Pular algo de que outros itens precisam, ou deixar um vínculo vazio, recusa a importação
   (`dependency_unresolved`). Os workflows são importados em um workspace escolhido; os passos mantêm ordem, rotas,
   tentativas, rotas de falha, loops, condições, flags de somente leitura e instruções por passo. Um que ainda não
   valida é mantido como rascunho e isso é informado.
8. **Modelos são conferidos, nunca presumidos.** O arquivo guarda o id do runtime do agente, os nomes de runtime e
   provedor e o id do modelo. A prévia pergunta aos runtimes desta instalação (`RuntimeRegistry::inspect_all`, uma vez, e
   só quando o arquivo tem agentes) e informa um de: `available`, `unverified` (o runtime não consegue listar modelos,
   como ao digitar um id de modelo no formulário), `model_missing`, `runtime_not_ready`, `runtime_unsupported` (não
   consegue executar tarefas), `runtime_missing`, mais os runtimes prontos com seus modelos como alternativas (o mesmo
   runtime primeiro). O original é mantido a menos que a pessoa escolha uma alternativa; os quatro primeiros podem ser
   importados com o original e configurados depois (o agente aparece como indisponível como qualquer outro cujo runtime
   não está pronto); os dois últimos precisam de outro runtime ou o item é pulado. O relatório guarda o runtime e o
   modelo originais ao lado do que foi aplicado. Quando a pessoa escolhe uma alternativa, o agente também guarda o que o
   arquivo pedia (`Agent.imported_model`), salvo com ele e mostrado no formulário do agente; some quando a pessoa muda o
   runtime ou o modelo do agente. Teste: `the_original_model_stays_on_the_agent_until_the_person_changes_where_it_runs`.
9. **Importar não concede nada e não executa nada.** Nenhuma execução, concessão, conexão ou segredo é criado. Os agentes
   são importados com o perfil de permissão `read_only`, a menos que a pessoa marque "manter a permissão de editar
   arquivos" na prévia (a prévia diz quais agentes pedem isso e avisa quando o runtime deles não consegue editar). Os
   templates salvos mantêm as suas _sugestões_ de ferramentas (uma capacidade ou o nome de uma conexão), que só viram
   concessões quando uma pessoa inicia um workflow com essa opção (ADR 0030). Instruções e prompts são texto: são
   mostrados na prévia (expansível) e guardados literalmente, e nada em um arquivo é executado, expandido ou
   interpretado.
10. **Limites rígidos para entrada não confiável**: 2 MiB por arquivo (conferido antes do parse), 200 itens, 200 passos
    e 400 rotas por grafo, refs de `[a-z0-9_-]{1,64}`, nomes de no máximo 80 caracteres imprimíveis, grafos cujos passos
    e rotas se nomeiam de forma consistente (qualquer outra coisa é recusada inteira), regras de domínio para nomes, ids
    de modelo (sem ids parecidos com flags ou com espaços), contratos de resultado e perfis de permissão. As mensagens de
    erro carregam códigos e posições, nunca o conteúdo do arquivo.
11. **Os arquivos são escolhidos em diálogos nativos abertos pelo núcleo** (`pick_config_file`, `save_config_file`, com o
    `tauri-plugin-dialog` já em uso). A webview nunca nomeia um caminho: ela guarda o texto do arquivo.
    `save_config_file` só grava texto que é lido como um arquivo do Atlas, e grava por um arquivo temporário e uma
    renomeação. Nenhum plugin de sistema de arquivos nem capability foi adicionado. Um arquivo também pode ser **solto
    no diálogo de importação**: o webview lê o texto do arquivo (nunca um caminho), recusa sem ler um arquivo acima de
    2 MiB e segue a mesma prévia. A janela usa `dragDropEnabled: false` para o webview receber o arquivo, e uma soltura
    fora do diálogo não faz nada.
12. **Credenciais nunca viajam.** Agentes não têm nenhuma; as conexões MCP (executáveis, argumentos, segredos) não são
    exportadas de modo algum: o MCP Hub tem sua própria importação/exportação para arquivos de configuração das CLIs. Um
    texto que _parece_ uma credencial (o detector de segredos do Harness) gera um aviso ao exportar e de novo ao
    pré-visualizar; não é redigido, porque alterar em silêncio a instrução de alguém é pior do que avisá-lo.

## O que é portátil, e o que não é

| Exportado / importado                                       | Por quê                                                                 |
| ----------------------------------------------------------- | ----------------------------------------------------------------------- |
| Personalidades (personalizadas, e embutidas como editadas)  | Comportamento reutilizável; sem dados locais                            |
| Agentes (com a sua personalidade)                           | Configuração reutilizável; runtime e modelo conferidos na chegada       |
| Workflows (com seus agentes e personalidades)               | Processo reutilizável; importado em um workspace à escolha              |
| Templates de workflow salvos (com sugestões de ferramentas) | Processo reutilizável; as sugestões não concedem nada                   |
| _Não_ exportado: workspaces                                 | Ligados a uma pasta de projeto de uma máquina                           |
| _Não_ exportado: conexões MCP, concessões, segredos         | Executáveis e credenciais; o MCP Hub tem seu próprio fluxo              |
| _Não_ exportado: conversas, execuções, uso                  | Histórico operacional, possivelmente privado                            |
| _Não_ exportado: runs, worktrees, cotas, configurações      | Estado local de uma instalação                                          |
| _Não_ exportado: rules                                      | Ainda sem editor para o usuário; as rules de projeto vivem em `.atlas/` |
| _Não_ exportado: harness (`.atlas/`)                        | Já vive no projeto e viaja com o seu Git                                |

## Descartado

- **Importar por URL** (decisão do usuário, 2026-11-12). O núcleo não tem cliente HTTP, e construir isso traria uma
  dependência nova e uma superfície de rede (SSRF, tamanho, tipo, redirecionamentos) para um caso que baixar o arquivo e
  soltá-lo no diálogo já cobre, sem rede.

## Limites

- Templates de workflow embutidos são código e não são exportados.

## Não verificado

- Soltar um arquivo na **janela real** do app. O caminho é testado no componente (`ImportDialog.test.tsx`, "dropping a
  file"), mas o efeito de `dragDropEnabled: false` no webview de cada SO (macOS, Windows, Linux) não foi visto. Para
  fechar: abrir o app, arrastar um `.atlas.json` para o diálogo de importação e ver a prévia.
