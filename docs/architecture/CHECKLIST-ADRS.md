# Checklist: fechar o que as ADRs deixaram pendente

Documento de trabalho, escrito para ser executado por outra sessão sem depender da conversa que o originou.

- **Fonte:** seções "Não feito", "Limites", "Não verificado", "Consequências" e "Decisões pendentes" das ADRs 0009–0044
  em `docs/architecture/adr/`, lidas em 2026-11-11.
- **Fora daqui, de propósito:** o que foi descartado (RTK/ADR 0023, planejador LLM, chat entre agentes, `ToolCatalog`
  próprio do Atlas) e o que já tem trabalho próprio (ai-memory, ADR 0040/0041, branch de POC).
- **Aviso de confiança:** os itens vêm do texto das ADRs, não de uma auditoria do código. Algumas ADRs estão desatualizadas
  (a 0013 e a 0024 ainda descrevem MCP como inexistente). **Antes de implementar um item, confirme no código que ele
  continua pendente**; se já estiver feito, só corrija a ADR e marque o item.
- Os caminhos em "Onde" são ponto de partida (módulos que a árvore atual tem), não garantia de que a mudança cabe só ali.

---

## 0. Como a sessão executora deve trabalhar

### 0.1 Antes de começar

1. Ler, nesta ordem: `docs/architecture/README.md`, `docs/RESUMO.md`, a ADR do item e as ADRs que ela cita.
2. Rodar `git status` e `git branch --show-current`. O working tree pode ter mudanças não commitadas de outro trabalho
   (por exemplo `bench/`, `orchestration/`, `runtimes/` e docs novos). **Não** misturar o trabalho do checklist com elas:
   abrir branch própria a partir de `main` (ou worktree) e commitar só os arquivos do item.
3. Conferir o estado do item no código (ver o aviso de confiança acima).

### 0.2 Regras de arquitetura (valem para todo item)

- Camadas: `domain/` (tipos e regras puras) → `application/` (casos de uso) → `commands/` (Tauri) e `infrastructure/`/
  `platform/` (I/O). Todo I/O novo fica atrás de uma porta (ADR 0002); nada de `infrastructure/` criado vazio.
- Toda superfície nova (rota remota, comando Tauri, ferramenta MCP, flag de runtime) entra como **uma linha de política
  mais um teste** (`application/remote/policy.rs`, guardrails, `RuntimeCapabilities`), nunca como atalho.
- **Padrão seguro:** funcionalidade nova nasce **desligada** e, desligada, o prompt e o comportamento ficam idênticos
  byte a byte. Ligar por padrão é uma decisão separada, registrada na ADR.
- O Atlas **não** dá autoridade a texto vindo de agente, ferramenta ou arquivo de projeto: tudo isso é `Untrusted`
  (ADR 0025/0027). Nenhum item pode alargar isso.
- UI: seguir o design do Stitch (ver memória "Stitch design source") e textos nos três idiomas já suportados.
- Windows/Linux: o código novo é multiplataforma ou isolado com `cfg`; o CI roda `npm run check` nos três SOs
  (`.github/workflows/ci.yml`), então não quebre os outros dois.

### 0.3 Definição de pronto (todos os itens)

1. Testes determinísticos (runtime falso/roteirizado, serviços reais) para o caminho feliz **e** cada recusa.
2. `npm run check` verde, rodado **uma vez no fim do lote**, não a cada fase (preferência do usuário).
3. Teste real só quando o item pede, **por nome exato** (`cargo test <nome> -- --ignored --exact`), nunca com filtro
   amplo de `--ignored` (chama modelos pagos). Gasto de tokens/premium requests: **perguntar antes**.
   - Nunca usar OpenCode em teste real (trava o terminal do usuário). Usar Codex luna ou haiku, **conferindo o login**
     antes (o Claude já apareceu deslogado).
4. Nada de `bench/results/` nem saídas de execução no Git. Antes de qualquer push, conferir a contagem de arquivos.
5. Não rodar prettier em diretórios de dados (`bench/`).
6. A ADR é atualizada no **mesmo commit** (protocolo abaixo) e este checklist também.

### 0.4 Protocolo de fechamento e limpeza de ADR

Objetivo: a ADR descreve só o estado atual e o que de fato falta.

| Situação do item                | O que fazer na ADR                                                                                                   |
| ------------------------------- | -------------------------------------------------------------------------------------------------------------------- |
| Feito e verificado              | Apagar a linha de "Não feito/Limites". Reescrever o corpo para o estado atual (sem "agora", sem texto riscado `~~`). |
| Feito, **não** verificado       | Mover para "Não verificado" com o motivo exato (o que faltou para verificar e como fechar).                          |
| Decidido que não se fará        | Mover para uma seção curta "Descartado", com o motivo; apagar daqui deste checklist.                                 |
| ADR sem nenhuma pendência       | `Status: aceito e implementado` (ou o equivalente da ADR) e remover as seções de pendência.                          |
| Conteúdo superado por outra ADR | Cabeçalho `Substituída pela ADR NNNN` e apontar; não duplicar texto.                                                 |
| ADR sem campo de status         | Acrescentar o campo no formato das outras (`Status: ...`).                                                           |

Também atualizar, quando o estado mudar: `docs/architecture/README.md`, `docs/RESUMO.md` e, se a decisão mudou,
`docs/context-and-tooling-platform.md`. Marcar o item aqui com `[x]` e a data; quando um bloco inteiro estiver
concluído, apagar o bloco. Quando o arquivo esvaziar, apagá-lo.

### 0.5 Formato de cada item

`- [ ] **ID. Título.** (ADR) — Onde: ... — Pronto quando: ... — Verificação: det | real(nome) | plataforma — Depende de: ...`

Verificação **det** = testes determinísticos; **real** = precisa de modelo/CLI real (custa; pedir autorização);
**plataforma** = precisa de outro SO ou de hardware.

---

## Bloco 1 · Segurança e sandbox (decide ligar o sandbox por padrão)

ADRs: 0038, 0039, 0044, 0026, 0027, 0025.

- [ ] **S1. Fuga do sandbox com a CLI real.** (0038 "Não verificado"; 0039)
  - Onde: `application/security/sandbox/` (`macos.rs`, `profile.rs`, `cli.rs`), testes `sandbox_real_*_cannot_escape` nos
    runtimes (`runtimes/claude.rs`, `codex.rs`, `copilot.rs`).
  - Pronto quando: para Claude, Codex e Copilot, uma escrita **fora** do perfil, tentada pela ferramenta de arquivo da
    CLI, é negada pelo sistema operacional e nada aparece fora do projeto. Hoje o teste do Copilot "roda, mas não
    exercita" (o modelo se recusa a tentar).
  - Como: invocar a ferramenta de arquivo da CLI **sem passar pelo modelo**, ou forçar o modelo (se a conta permitir).
    Se não houver como forçar, registrar na ADR como limite permanente, não como pendência.
  - Verificação: real (por nome). Depende de: nada.
  - 2026-11-11: testes `sandbox_real_bare_*` (a CLI sem o prompt do Atlas). Claude: a própria CLI recusa antes do SO;
    Copilot: o modelo não tenta. Os dois viraram limite permanente na 0038. **Falta o Codex** (cota até 2026-11-16).
- [x] **S2. Ferramentas de build e PTY sob o perfil.** (0038) — 2026-11-11: medido sem modelo (tabela e lista de
      caminhos na 0038); faltou `cargo`/`npm` com download de dependências.
  - Pronto quando: `npm` e `cargo` reais (caches de ferramentas) e o modo terminal rodam confinados sem quebrar, com a
    lista de caminhos liberados registrada na ADR.
  - Verificação: real local (sem modelo) + det do perfil gerado. Depende de: S1 (mesmo perfil).
- [ ] **S3. Quais escritas cada CLI tenta e é negada.** (0038)
  - Pronto quando: há uma tabela por CLI (Claude engole falhas; as outras não falharam até agora) e um teste que pega
    regressão de escrita negada.
  - Verificação: real. Depende de: S1.
  - 2026-11-11: tabela de Claude e Copilot na 0038; o macOS não registra as negações. A regressão fica com os testes
    de trabalho e o do S2. **Falta o Codex**, junto com o S1.
- [ ] **S4. Decidir `auto` e ligar o sandbox por padrão.** (0038 "Plano para ligar por padrão", passos 2–3)
  - Pronto quando: decisão registrada na ADR com o critério (uso real por um tempo, S1–S3 fechados) e a mudança do
    padrão feita **só nesta decisão**, com teste que prova o padrão e o desligar.
  - Atenção: decisão do usuário, não da sessão. **Perguntar** antes de mudar o padrão.
  - Depende de: S1, S2, S3.
- [ ] **S5. Claude com o sandbox do Atlas ligado e Linux/Windows.** (0044)
  - Pronto quando: o Claude com sandbox do Atlas ligado foi medido (a troca "segue a do Codex por analogia" hoje não foi
    medida) e o comportamento em Linux/Windows (onde o Claude segue sem shell) está testado ou registrado como limite.
  - Verificação: real + plataforma. Depende de: S1.
- [x] **S6. `--disallowedTools` falha aberto.** (0026 "Consequências") — 2026-11-11: já estava mitigado (ADR 0028
      decisão 9, testes em `executions/mcp_tests.rs`); só a 0026 foi corrigida.
  - Onde: `application/mcp/launch.rs`, `plan.rs`, adaptador do Claude.
  - Pronto quando: ou há mitigação provada (autorização por servidor, conferir `init.tools`, parar antes da primeira
    requisição) **ou** a ADR/UI dizem que a autorização por ferramenta é de melhor esforço. Hoje uma ferramenta que o
    servidor adicione depois passa.
  - Verificação: det (stream-json falso com ferramenta extra) + real se a mitigação depender do `init` real.
- [x] **S7. Invariante "resultado de ferramenta MCP é `Untrusted`".** (0027 "Contrato para o MCP", "nada implementado")
      — 2026-11-11: teste `an_mcp_tool_result_is_untrusted_scanned_and_changes_nothing`; 0027 reescrita.
  - Onde: `application/optimization/context/`, `application/security/guardrails/`, `domain/context`.
  - Pronto quando: teste prova que um resultado de ferramenta MCP (a) entra com `ContextAuthority = Untrusted`,
    (b) é escaneado como skill/handoff (alegações sinalizadas, segredo redigido), (c) **não** muda Rule, Permission,
    Approval, Policy, nem decisão de Guardrail, (d) contradizer uma rule obrigatória gera Aviso de que a rule governa.
  - Verificação: det. Depende de: nada (a invariante vale antes do MCP real).
- [ ] **S8. Escanear tarefa e instruções do agente por alegações de autoridade.** (0027 "Não alterado")
  - Pronto quando: há decisão registrada. A ADR já disse que escanear gera ruído ("o usuário aprovou" é comum numa
    tarefa de workflow). Se não implementar, mover para "Descartado" com o motivo; se implementar, só sinalizar, nunca
    bloquear.
- [ ] **S9. Decisões abertas de autonomia.** (0025 "Decisões pendentes")
  - (a) Introduzir autonomia Manual/Guiado/Auto e o que cada uma significa para o `ASK`.
  - (b) `--restricted` para o Claude (decisão C): precisa de **uma execução real pequena** antes.
  - (c) Ampliar o léxico de conflitos a partir de projetos reais (`application/rules/`).
  - Atenção: (a) é decisão do usuário; **perguntar**. Cada uma vira ADR/emenda própria, não item de código solto.
- [ ] **S10. Editor de rules.** (0027 "Consequências"; marcado "decidido: não agora")
  - Pronto quando: decisão reconfirmada. Se continuar "não agora", mover para "Descartado/Adiado" com data.

## Bloco 2 · MCP completo

ADRs: 0028, 0029, 0030, 0024, 0026.

- [ ] **M1. Transporte HTTP.** (0028 "Não feito")
  - Onde: `domain/mcp.rs` (`McpTransport::Http` existe e é recusado), `application/mcp/validate.rs`, `config_io.rs`,
    `launch.rs`, `application/security/` para a camada de rede.
  - Pronto quando: servidor HTTP é aceito, com **camada de SSRF** (bloqueio de loopback/rede privada/metadata salvo
    permissão explícita), timeout, limite de tamanho, **interceptação por chamada** e segredo nunca em log/URL.
    Cada dialeto de runtime (Claude, Codex, Gemini, OpenCode, Copilot) tem a tradução ou recusa com motivo.
  - Verificação: det (servidor HTTP local de teste) + real opcional. Depende de: S6, S7.
- [ ] **M2. OAuth e Figma oficial.** (0028 "Consequências")
  - Pronto quando: fluxo OAuth com armazenamento de token no keyring; o Figma passa de "listado, não adicionável" para
    adicionável no catálogo (`application/mcp/catalog.rs`). Nunca logar token.
  - Atenção: OAuth exige conta real do usuário; **perguntar** antes de qualquer fluxo com credencial. Depende de: M1.
- [ ] **M3. Credenciais em Windows e Linux.** (0028)
  - Onde: `application/mcp/credentials.rs` (`keyring`).
  - Pronto quando: o crate inteiro compila para `x86_64-pc-windows-msvc` e o alvo Linux, e o ciclo gravar/ler/apagar
    passa nos dois (hoje só o Keychain do macOS foi exercitado; faltou `llvm-rc` localmente).
  - Verificação: plataforma (CI Windows/Linux já existe; criar teste que rode lá). Depende de: nada.
  - 2026-11-11: passos no CI que rodam o teste real por nome exato no Windows e no Linux; a compilação já era provada
    pelo CI. Falta o CI rodar (precisa de push); a 0028 diz "não verificado" até lá.
- [x] **M4. Descrições e esquemas das ferramentas MCP.** (0028, 0030) — 2026-11-11: ADR 0045 (cliente `tools/list`
      pelo guard, a pedido; inspetor, orçamento e `read_only`). Falta o real com DevTools (não gasta tokens) e Windows; estão
      em "Não verificado" na 0045.
  - Pronto quando: o Atlas lê as descrições (sonda `tools/list`), mostra na UI, **orça em tokens** e usa para substituir
    o palpite por verbos de leitura em `read_only` (hoje `read_only` não governa ferramentas MCP).
  - Verificação: det (servidor MCP falso) + real com DevTools. Depende de: nada.
- [ ] **M5. Gemini verificado.** (0029 "Não verificado")
  - Pronto quando: com uma conta aceita pela CLI, o `trust` deixando a chamada passar e os nomes de ferramentas usadas
    foram verificados. Se a conta continuar recusada, a ADR mantém "não verificado" com a mensagem exata da CLI.
  - Também: o arquivo gerenciado de configurações de sistema do Gemini seria substituído durante a execução; decidir
    se vira aviso na UI.
  - Verificação: real (por nome).
- [ ] **M6. Sugestão de concessões por conexão com outro nome.** (0030)
  - Pronto quando: o casamento deixa de depender só do nome (ou a limitação é mostrada ao usuário), com teste.
- [ ] **M7. Execução real com modelo para o que o `init` não prova.** (0026 "Consequências")
  - Pronto quando: foram medidos contagem real de tokens de entrada, morte de servidor no meio da execução,
    comportamento no cancelamento, `CLAUDE.md`/auto-memória chegando ao modelo e a semântica do
    `--append-system-prompt`.
  - Atenção: gasta tokens; **perguntar** antes. Agrupar tudo numa única rodada pequena. Depende de: nada.
- [ ] **M8. Superfície de OpenCode, Gemini e Antigravity.** (0024 "Não construído")
  - Pronto quando: cada um tem fatos de superfície medidos (sem OpenCode real em terminal: ver 0.3) ou a ADR declara
    explicitamente que ficam fora e por quê. Reescrever a 0024 para o estado atual (ela diz "nenhum MCP").
- [ ] **M9. Limpar a 0024, 0026 e 0028 do que já foi superado.** (0024, 0026, 0028)
  - Pronto quando: as ADRs deixam de afirmar "nenhum MCP", "só o Claude" e "Fase E não feita" onde isso mudou.

## Bloco 3 · Memória

ADRs: 0037 (e 0040/0041 como contexto; não duplicar).

- [ ] **MEM1. Fase M4: validação de custo/benefício.** (0037 fases)
  - Onde: testes `memory_experiment_*` (por nome; `ATLAS_M4_N` controla a amostra), `application/memory/`.
  - Pronto quando: números registrados (tokens de entrada, chamadas, acerto) com e sem memória, com amostra maior que 1
    por braço, em pelo menos dois runtimes.
  - Atenção: gasta tokens; **perguntar** antes. Depende de: nada.
- [ ] **MEM2. Decidir padrões e cotas com os números.** (0037 "Decisões pendentes")
  - Pronto quando: modo padrão por escopo, cotas e validades definidos pelos números de MEM1 e escritos na ADR.
- [ ] **MEM3. Agente propor notas, com amostra.** (0039)
  - Pronto quando: o resultado "0 em 1 execução" virou uma medição com amostra suficiente para dizer se é limite do
    Copilot ou do prompt.
- [ ] **MEM4. Fechar a 0037.** — `Status: aceito e implementado`; remover as fases e "Decisões pendentes". Conferir o que
      a 0041 (memória ligada por padrão, aprende com reject/merge/discard) já substituiu para não deixar duas verdades.

## Bloco 4 · Custo e contexto

ADRs: 0036, 0021, 0020, 0012, 0011.

- [ ] **C1. Comparação antes/depois do custo do SDD.** (0036)
  - Pronto quando: execuções reais de SDD, antes e depois das otimizações 3–5, lidas na barra de consumo, com a tabela
    na ADR. Atenção: gasta tokens; **perguntar**.
- [ ] **C2. Estimativa de custo antes de executar.** (0036)
  - Pronto quando: a estimativa aparece só quando há dado confiável (histórico) e **marca a precisão**
    (`Precision`/`TokenSource`); sem dado, não mostra número inventado.
- [ ] **C3. Prompt em blocos para caching.** (0036, 0026)
  - Onde: `ModelRuntime::delivery()` (única fonte do payload) e `manifest.promptHash`.
  - Pronto quando: nos runtimes que informam cache o prompt é entregue de forma que o prefixo estável seja aproveitado
    e a economia é medida; nos demais nada é declarado. O hash do manifest continua sendo o do payload entregue.
  - Verificação: det (ProcessRunner espião) + real para medir. Depende de: C1.
- [ ] **C4. `ContextQuality` e relevância por item.** (0021)
  - Onde: `application/optimization/context/engine/`, `domain/context` (`ContextItem.relevance` está vazio).
  - Pronto quando: o item carrega relevância e há um sinal sobre o total; sem inventar métrica que o Atlas não mede.
- [ ] **C5. Cache por fingerprint e obsolescência por item.** (0021)
  - Pronto quando: o fingerprint da fase 1 vira cache real **medido** (a ADR 0020 diz que `cacheHits/cacheMisses` são
    `None`) e a obsolescência do Harness entra por item. Há um flag no registro de perf que mostrou "sem efeito" no
    cache; reavaliar antes de religar.
- [ ] **C6. Condicionar o texto obrigatório repetido.** (0021)
  - Pronto quando: a instrução de plano (regra de plano × protocolo de interação) e os avisos de "não concede
    permissões" não aparecem repetidos, **sem** remover nenhuma regra de segurança, e o prompt desligado continua igual.
  - Atenção: é uma decisão (0021 pede decisão, não mais deduplicação); **perguntar** se mexer em texto obrigatório.
- [ ] **C7. Contexto ciente da tarefa além do léxico.** (0012, 0011)
  - Pronto quando: o filtro por tarefa sobre as áreas do Harness (0011 "não implementado") existe ou foi descartado por
    escrito; o léxico feito à mão ganha estratégia para projetos fora do vocabulário.
- [ ] **C8. Modelo por etapa do SDD.** (0036)
  - Pronto quando: só se houver evidência de ganho. Caso contrário, mover para "Descartado" com o motivo (hoje é
    "criaria um segundo sistema de configuração").
- [ ] **C9. Skills especializadas de elicitação/cenários/decomposição.** (0036)
  - Mesmo critério de C8: sem evidência, descartar por escrito.

## Bloco 5 · Workflow, spec e entrega

ADRs: 0013, 0014, 0018, 0019, 0035, 0009.

- [ ] **W1. Reativar o Live Workspace depois de reiniciar.** (0018 "Limitação conhecida")
  - Onde: `application/live_workspace/` (`mod.rs`, `tracker.rs`).
  - Pronto quando: uma execução existente volta a ser observada ao reabrir o app (sem esperar o retomar), com teste de
    reinício.
- [ ] **W2. Limites do Live Workspace.** (0018 "Limites")
  - Pronto quando: o corte de 500 arquivos (`MAX_CHANGESET_FILES`) e a leitura de contagens só até 1 MiB têm decisão
    (paginar/avisar na UI ou manter e registrar). Hoje o corte é silencioso na lista.
- [ ] **W3. Persistir o status de revisão.** (0019)
  - Pronto quando: sobrevive a reabrir o app, sem divergir do status de integração (ele continua a fonte).
- [ ] **W4. `max_parallel_steps` configurável.** (0013)
  - Onde: `application/workflow/engine.rs`, `domain/workflow`, UI de configuração.
  - Pronto quando: o valor (hoje fixo em 4) é configurável por execução com limites validados, e execuções antigas sem o
    campo leem o padrão 4 (compatibilidade, como as outras migrações).
- [ ] **W5. Passos paralelos compartilham a árvore.** (0014)
  - Pronto quando: há decisão. Hoje o delta por passo pode incluir arquivos do outro; ou isolar por passo ou manter e
    deixar o aviso de sobreposição como o limite documentado.
- [ ] **W6. Spec: acompanhar/aprovar pelo celular.** (0035; 0034)
  - Onde: `application/remote/policy.rs`, rotas de `remote_server.rs`, `src/mobile/`.
  - Pronto quando: uma linha de política por ação nova, teste de política e de rota, e nenhuma ação nova sem
    pareamento válido (ADR 0034). Nada de caminhos de projeto/worktree nem diffs no celular.
- [ ] **W7. Spec: diff entre revisões.** (0035, 0036: **uma** implementação para as duas)
  - Pronto quando: a tela mostra o diff entre revisões do documento, calculado no núcleo, testado.
- [ ] **W8. Spec: edição manual e sincronização de arquivos editados à mão.** (0035)
  - Pronto quando: edição manual é aceita com validação, e a mudança feita fora do Atlas é detectada e reconciliada sem
    sobrescrever silenciosamente. Marca dependentes como desatualizados como já ocorre.
- [ ] **W9. `skip_specs`.** (0035) — mudança sem comportamento passa sem exigir uma capacidade, com validação própria.
- [ ] **W10. Agente isolado lê os arquivos publicados.** (0035 tabela)
  - Pronto quando: a spec publicada e ainda não commitada chega ao agente isolado de forma garantida (hoje só o digest;
    o conteúdo vai no brief). Ou documentar como garantia definitiva.
- [ ] **W11. Merge de deltas em `openspec/specs/` sem a CLI.** (0035)
  - Pronto quando: decisão. A ADR considera reimplementar o archive do OpenSpec arriscado; se continuar assim, mover
    para "Descartado" com o motivo.
- [ ] **W12. Detectar o Visual Studio.** (0014) — entrega mostra a opção quando detectado, só Windows (`cfg`).
- [ ] **W13. Submódulos e sparse checkouts.** (0009) — detecção e tratamento explícito (ou erro claro), com teste.
- [ ] **W14. Integração automática de mudanças.** (0014 "só o usuário aplica")
  - Pronto quando: decisão. Provavelmente **manter** (é invariante de segurança): mover para "Descartado" com motivo.
- [ ] **W15. Política que nega Git deixa trabalho sem commit.** (0014)
  - Pronto quando: o usuário vê claramente que o trabalho está sem commit e como aplicá-lo, ou o fluxo é ajustado.

## Bloco 6 · Multi-projeto

ADRs: 0033, 0043.

- [ ] **P1. Retomar a cadeia sozinho.** (0043) — após reabrir o app entre duas execuções, a próxima é criada
      automaticamente (hoje só o botão "Continuar"); com confirmação se mudar o comportamento atual. Teste de reinício.
- [ ] **P2. Escopo de workflow por passo.** (0033) — hoje por execução. Só se houver caso real; senão descartar por escrito.
- [ ] **P3. Handoff com diff grande.** (0043) — a execução seguinte recebe resumo estruturado do diff (arquivos,
      trechos relevantes) em vez de só o começo; respeitando o orçamento do contexto.
- [ ] **P4. Varredura de obsolescência por leitura.** (0033) — a visão geral não escaneia todo projeto a cada leitura
      (cache com invalidação), com teste.
- [ ] **P5. Execuções paralelas da cadeia.** (0043) — **descartar por escrito** se a decisão for manter sequencial
      (era a escolha da ADR); não implementar sem decisão do usuário.

## Bloco 7 · Importar/exportar configuração

ADR: 0032. Onde: `application/portability/` (`format.rs`, `apply.rs`, `inspect.rs`, `kinds/`).

- [ ] **I1. Arrastar e soltar um arquivo.** — passa pelo mesmo preview + decisões + importação atômica; nada aplica sem
      decisão.
- [ ] **I2. Importar por URL.** — só com política de rede (SSRF, tamanho, tipo), **confirmação explícita** e o mesmo
      fluxo de preview. Nunca seguir URL vinda de conteúdo observado.
- [ ] **I3. Duplicatas por nome.** — `use_existing` e `replace` casam pela primeira; passar a avisar/escolher de forma
      determinística.
- [ ] **I4. Modelo "pendente" no agente importado.** — registrar além do relatório de importação, para sobreviver à
      escolha de alternativa.
- [ ] **I5. Importar para workflow com execução em andamento.** — `replace` é recusado; oferecer a outra decisão no
      próprio fluxo, com explicação.

## Bloco 8 · Plataformas e verificação real

ADRs: 0009, 0034, 0028, 0038, 0039, 0044. **O CI já roda `npm run check` em ubuntu, macOS e Windows**
(`.github/workflows/ci.yml`); portanto "validar Windows/Linux" começa por ler o que o CI já cobre.

- [x] **X1. Auditar o que o CI já cobre.** — 2026-11-11: o CI compila, roda clippy e os testes não ignorados nos três
      SOs; os testes de PTY com processo real são só Unix; 79 testes são `#[ignore]` e não rodam. ADRs 0007, 0009, 0028 e
      0034 corrigidas; o que nunca rodou no Windows/Linux ficou como "não verificado" nelas. — mapear, por ADR, o que roda de fato em Windows/Linux (testes com `cfg`,
      testes ignorados). Atualizar as ADRs que dizem "só macOS foi compilado" onde o CI já prova o contrário.
- [ ] **X2. Worktrees em Windows e Linux.** (0009) — caminhos, locks e `git worktree` com layouts incomuns, com testes
      que rodem no CI.
- [ ] **X3. Remote access em Windows/Linux.** (0034) — servidor, pareamento e revogação.
- [ ] **X4. Celular físico em Wi-Fi.** (0034) — manual: pareamento por link, painel, aprovação, reconexão. Roteiro e
      resultado na ADR.
- [ ] **X5. App empacotado (`tauri build`) com o servidor remoto ligado.** (0034) — manual/CI de release.
- [ ] **X6. Copilot.** (0039) — login conhecido (hoje só `AuthenticationRequired` na execução), modelo diferente de
      `auto`, fluxo longo fora do sandbox.
- [ ] **X7. Servidores MCP do Copilot não estritos e sem sonda.** (0039) — decidir entre aceitar o limite ou filtrar.

## Bloco 9 · Higiene das ADRs (executar no fim de cada bloco)

- [ ] **H1. Remover texto riscado e estados superados.** Procurar `~~` e "ainda não" nas ADRs 0009–0044 e reescrever.
- [ ] **H2. Campo `Status:` em todas.** As ADRs 0016–0028 não têm o campo no formato das demais.
- [ ] **H3. 0013, 0024 e 0026–0028:** reescrever as seções que ainda afirmam o estado pré-MCP/pré-Fase E.
- [ ] **H4. Índices.** `docs/architecture/README.md` e `docs/RESUMO.md` refletem os status novos.
- [ ] **H5. Este arquivo.** Marcar `[x]` com data; apagar blocos concluídos; apagar o arquivo quando vazio.

---

## Ordem sugerida

1. **S7, S6, M3, M4** (determinísticos, sem custo, destravam o resto) e **X1**.
2. **S1–S3** (real, pedir autorização) → **S4/S9** (decisões do usuário).
3. **M1 → M2**, **MEM1 → MEM2/MEM4**.
4. **C1 → C3**, depois **C4–C7**.
5. **W1–W4**, **P1, P3, P4**, **I1–I5**, **W6–W10**.
6. **X2–X7** conforme dê acesso a máquinas/celular.
7. **H1–H5** ao fim de cada bloco.

Itens marcados "decisão"/"descartar por escrito" **não** viram código: viram uma linha na ADR. Itens marcados
"perguntar" não são executados sem o usuário.
