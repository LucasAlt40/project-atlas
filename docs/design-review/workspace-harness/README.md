# Revisão de design — Módulo Workspace + Harness

Dossiê para redesenhar o módulo no Stitch (projeto `526454930665111857`, design system
`assets/7aa0774b734b4149b2d38a23136aadfd`, "Atlas Engineering Core"). O Stitch só aceita texto
(ver `prompts.md`), então este documento descreve cada tela, seus estados e suas interações com
precisão. As imagens em `atual/` são o app real, capturado em 1440×900 com dados de exemplo.

## 1. O que não pode mudar (restrições do produto)

- Tema escuro único, denso, de ferramenta de engenharia. Superfícies separadas por tom e borda
  de 1px, nunca por sombra (a única sombra é a de overlay).
- Tokens atuais (`src/styles/global.css`): superfícies `#0b0d10 → #242c37`, texto `#f1f5f9 /
  #e2e8f0 / #94a3b8 / #64748b`, acento `#6366f1`, semânticos `info #38bdf8`, `running #60a5fa`,
  `success #10b981`, `warning #f59e0b`, `danger #ef4444`, `policy #ec4899`. Fontes Geist
  (interface) e JetBrains Mono (dados, rótulos técnicos). Raios 4 / 6 / 8px.
- Idioma da interface: português (pt-BR) com alternância PT/EN no topo. Textos nas telas devem
  ficar em português.
- Todo comportamento existente continua: o redesenho muda aparência e organização, não função.
  Nenhuma tela pode ganhar recurso que o app não tem.
- Janela desktop (Tauri), uso principal em ≥ 1280px de largura.

## 2. Moldura global (presente em todas as telas)

| Parte | Conteúdo |
|---|---|
| Barra lateral (13,5rem, recolhível) | Logo Atlas; **seletor de workspace** (nome + caminho; menu com "Novo workspace" e "Gerenciar workspaces"); navegação: Workspace, Workflow, Agentes, Personalidades, Specs, MCP Hub, Configurações; rodapé com nome e versão |
| Barra superior | Botão de recolher; breadcrumb "Workspace / tela"; à direita: indicador de ação pendente, execuções ativas, botão **Celular** (acesso móvel), alternância **PT / EN** |

## 3. Mapa do módulo

```
Workspace (tela principal)
├── Cabeçalho do workspace ── uso (hoje/semana) · Projetos · Integrações · + Adicionar agente
├── Visões: Lista expansível | Grade (2×2)
├── Cartão do agente ── abas: Chat · Atividade · Terminal · Detalhes · Execuções
│   ├── Detalhes (gaveta lateral) ── permissões, contexto do projeto, última execução, consumo, git
│   └── Execuções ── lista por dia → Inspetor de execução (modal: Detalhes/Contexto/Atividade/Chat/Terminal)
├── Adicionar / editar agente (painel inline)
├── Sem workspace (onboarding) · Seletor / criar / gerenciar workspaces
└── Projetos do workspace (modal) ── é a porta de entrada do Harness
    └── Harness
        ├── Analisar projeto (configuração da análise)
        ├── Execução ao vivo da análise (análise com IA)
        ├── Revisar contexto do projeto (confirmação e auditoria de achados)
        ├── Atualizar / Recarregar / Resetar
        └── Contexto do workspace + Notas do workspace
```

## 4. Inventário de telas

Legenda de captura: **[img]** capturada do app; **[código]** descrita a partir do código, sem
captura (estado difícil de reproduzir sem backend real).

### 4.1 Workspace — Lista expansível **[img 01]**
- **Cabeçalho**: título do workspace; chip de uso (`Hoje US$ 1,24` · `Semana US$ 3,72`, fonte
  mono); botões `Projetos (2)` (abre modal), `Integrações` (navega para o MCP Hub) e o primário
  `+ Adicionar agente`.
- **Alternância de visão**: `Lista expansível | Grade`; ação `Recolher todos`.
- **Cartão de agente expandido**: cabeçalho com nome, selo de estado (`PRONTO`, em execução,
  aguardando resposta, falha), linha de metadados (personalidade · runtime · provedor, modelo em
  chip, `Isolamento Git: ATIVO`), ações de editar e remover, botão de recolher. Abas
  `Chat · Atividade · Terminal · Detalhes · Execuções`.
- **Chat**: mensagens do usuário (balão à direita) e do agente (avatar, nome, hora; markdown com
  títulos, listas e blocos de código com botão `Copiar`). Rodapé: chip do escopo (`transport-erp`,
  seletor de projetos), dica "Responder ao <agente> ou solicitar alteração", campo de mensagem e
  botão `Enviar`.
- Estados: sem mensagens ("Nenhuma mensagem ainda. Pergunte algo a este agente."), executando,
  aguardando aprovação (**[código]** `ApprovalRequests`), falha, recebendo resposta.

### 4.2 Workspace — Grade **[img 02]**
- Grade de 2 colunas com até 4 agentes; cada célula repete o cartão (mesmas abas e compositor).
- Célula vazia mostra "Disponível". Cartões com o cabeçalho quebrando em duas linhas.

### 4.3 Agente — Detalhes (gaveta lateral) **[img 03]**
Seções em coluna única, na ordem: cabeçalho (nome, personalidade · runtime · provedor, modelo,
selo de estado) → **Permissões** (seletor de perfil: Somente leitura / Desenvolvedor; quatro
cartões: Sistema de arquivos, Processos, Git, Rede, com aviso âmbar para ações destrutivas) →
**Contexto do projeto** (estado do Harness, nota de que é só contexto) → **Última execução** →
**Consumo** (esta conversa / hoje / semana: custo, tokens, nº de execuções; nota sobre valores
informados pelo runtime) → **Cota** → dados do agente (runtime, modelo, estado, isolamento Git).
**[código]** também há `ContextPlatformPanel`, `ContextReviewPanel`, `GitDetails`, `MemoryPanel`,
`McpPanel`, `RulesPanel`.

### 4.4 Agente — Execuções e Inspetor **[img 04, 05]**
- Aba Execuções: grupos por dia ("Hoje"), item com ícone de estado, tarefa, `runtime · modelo ·
  Concluída · hora`, duração à direita.
- **Inspetor** (modal, 5 abas): Detalhes (tarefa, status, workspace, agente, personalidade,
  runtime, modelo, início, fim, duração, processo, entrada/saída/total de tokens, custo),
  Contexto, Atividade (linha do tempo de eventos), Chat, Terminal. **[código]** `ExecutionDetails`,
  `ExecutionInspector`, `AgentActivity`; terminal ao vivo com xterm.

### 4.5 Adicionar / editar agente **[img 06]**
Painel inline que empurra o conteúdo: título, botão `Criar novo agente` e `Cancelar`. Quando há
agentes não colocados, **[código]** lista-os para escolha. Editar usa `EditAgentPanel` (mesmo
formulário da tela Agentes).

### 4.6 Seletor de workspace e onboarding **[img 11, 12]**
- Menu: lista de workspaces (nome + caminho), `+ Novo workspace`, `Gerenciar workspaces`.
- Sem workspace: título "Bem-vindo ao Atlas", uma frase, botão `Criar workspace`. Totalmente
  centrado, sem ilustração nem passos.
- **[código]** `WorkspaceForm` (criar/editar: nome, pasta do projeto), `ManageWorkspaces`.

### 4.7 Projetos do workspace (modal) **[img 07, 08]**
- Título "Projetos de <workspace>", texto explicativo, `+ Adicionar pasta de projeto`.
- **Linha de projeto**: nome, selos (`Principal`, papel `Backend/Frontend`, estado do Harness:
  `Analisado` / `Não analisado` / desatualizado / precisa revisão), caminho, saúde do Harness
  (`Saudável`) + pilha tecnológica + contagens (`42 achados · 31 verificados · 9 inferidos · 2
  seus · 0 desconhecidos`) + data da última análise, dependências ("Usa: transport-erp").
  Ações em texto azul: `Atualizar · Recarregar · Resetar · Editar · Excluir` (projeto sem
  Harness: `Inicializar projeto · Editar · Excluir`).
- Bloco **Harness**: explicação + botão primário `Iniciar o Harness de N projeto(s)` ou, quando
  todos têm, `Resetar todos os Harnesses`.
- Bloco **Contexto do workspace**: estado ("Atualizado" / "Ainda não foi montado") + `Refazer
  contexto`.
- Bloco **Notas do workspace**: textarea + `Salvar notas`.

### 4.8 Harness — Analisar projeto **[img 09]**
Modal sobre o modal. Cabeçalho "Analisar projeto" com selos `ATLAS ENGINE` e `Local Core`,
projeto e caminho, aviso de privacidade (cadeado: só nomes de pastas e manifestos, nada é
executado). **Análise com IA (recomendado)**: texto explicativo, campo "O que o agente deve saber?
(opcional)" com contador de tokens, seletor de **agente executor**. **Diagnóstico inicial
detectado**: três colunas (Stack/runtime, Git e testes, Topologia) com itens marcados.
Rodapé: `Cancelar`, `Revisar sem IA`, `Analisar com IA` (primário).

### 4.9 Harness — Execução ao vivo da análise **[código]**
`AgentAnalysisScreen`: progresso da análise com IA (etapas, saída do agente, interromper). Não
capturada: exige um agente em execução.

### 4.10 Harness — Revisar contexto do projeto **[img 10]**
Terceiro nível de modal. Cabeçalho com selos `.atlas/` e nome do projeto, `Voltar` e `Inicializar`
(primário). Quatro métricas (Total, Verificados, Inferidos, Pendentes). Abas por área (Stack,
Testes e comandos, Arquitetura e módulos, Contexto de negócio) com contador. Lista de achados
agrupada por categoria (`LANGUAGE`, `FRAMEWORK`, `INFRASTRUCTURE`), cada um com checkbox de
inclusão, valor editável, arquivo de origem e link `Evidência`. Painel direito **Inspetor de
evidência** (vazio até escolher um achado). Rodapé: estado do Harness, alternância "Não enviar o
.atlas/ ao Git" com explicação. **[código]** `ConflictsPanel`, `DiffView`, `FindingRow`,
`StaleNotice`, `HealthBadge`, `HarnessStats`.

### 4.11 Harness — Atualizar / Recarregar / Contexto de tarefa **[código]**
`RefreshHarnessModal` (compara o projeto com a última análise e aplica), `TaskContextModal` e
`TaskContextPreview` (por que cada item entrou ou ficou de fora do contexto de uma tarefa, com
pontuação e orçamento de caracteres). `WorkspaceInitializeModal` (inicializar vários projetos).

## 5. Problemas de design observados

Observações feitas sobre as capturas; nenhuma é regressão, são pontos que o redesenho deve resolver.

1. **Cabeçalho do workspace sem hierarquia.** Chip de uso, dois botões secundários e o primário
   ficam na mesma linha com pesos parecidos. `Projetos` abre modal e `Integrações` troca de
   tela, mas os dois parecem o mesmo tipo de ação.
2. **Cartão de agente poluído.** Nome, selo, personalidade, runtime, provedor, modelo e
   isolamento Git disputam o cabeçalho e quebram em duas linhas na grade. A aba ativa mal se
   distingue das inativas.
3. **Compositor repetido.** Cada cartão leva seu próprio campo de mensagem, chip de escopo e
   dica; na grade são três cópias quase idênticas ocupando a metade de cada cartão.
4. **Célula "Disponível" morta.** Um quadro grande e vazio sem ação; deveria convidar a
   adicionar um agente.
5. **Detalhes é uma coluna longa.** Rótulos de seção minúsculos em caixa alta, cartões dentro
   de cartões nas permissões, o consumo no fim exige rolar.
6. **Modal dentro de modal, três níveis.** Projetos → Analisar projeto → Revisar contexto. O
   usuário perde a noção de onde está e o fundo fica ilegível.
7. **Ações de linha indistinguíveis.** Em cada projeto, `Resetar` e `Excluir` (destrutivas) têm
   a mesma aparência de `Atualizar` e `Recarregar`.
8. **Selos sem sistema.** Principal, papel, estado do Harness, saúde e conta de achados usam
   formatos diferentes; o estado do Harness aparece em dois lugares com dois estilos.
9. **Painel de evidência vazio.** Na revisão de contexto, metade da largura mostra só uma frase
   até o usuário clicar em algo.
10. **Onboarding sem orientação.** Texto e botão soltos; não explica o que é um workspace nem o
    próximo passo (pasta do projeto, agentes, Harness).
11. **Inspetor de execução em modal** com cinco abas para dados que o usuário quer comparar
    entre execuções; abrir uma por vez impede a comparação.
12. **A confirmar**: a gaveta Detalhes mostra "Nenhuma execução ainda" para um agente que tem uma
    execução na aba Execuções. Pode ser artefato dos dados de exemplo; verificar no app real.

## 6. Correspondência com o que já existe no Stitch

| Tela do app | Tela no Stitch (id) |
|---|---|
| Workspace — Grade | Atlas - Main Workspace Grid 2x2 (`6eecbeea16364b5f9a06abda229b2a5b`) |
| Workspace — Lista expansível | Atlas - Workspace Expansível (Agentes Detalhados) (`ee4c12062027462ab20b218c9160b37f`) |
| Detalhes do agente / permissões | Atlas - Permissions & Project Context (`01d7d6b3223a47e59f87d197a8ee675b`) |
| Inspetor e terminal | Atlas - Live Terminal & Execution Inspector (`4c0c75ea9216431f9aa3af3d6631f137`) |
| Resultado de execução | Atlas - Resultados de Execução & Artefatos Gerados (`b0ff9a0657d14b59b7953901c33ea7d2`) |
| Projetos + Harness | Atlas - Project Harness & Context Analysis (`499b4695b5c44c19b8ef17a452d22d70`) |
| Analisar projeto | Atlas - Configuração de Análise de Projeto (`cf432e92b86d4709af191e653726e35e`) |
| Análise ao vivo | Atlas - Execução ao Vivo da Análise de Projeto (`220fef1a2a4a4462ab6e47d76f9225d2`) |
| Revisar contexto | Atlas - Confirmação e Auditoria de Regras Detectadas (`54508e9fe5c047bd8f1a85f4a353e4f0`) |
| Seletor/onboarding, Adicionar agente, Projetos como lista | sem tela equivalente |

O Stitch não deixa baixar as capturas dessas telas sem autenticação própria, então a comparação
visual com o app não foi feita; a tabela acima é só por título.

## 7. Lacunas desta captura

Não capturados (dependem de backend real ou de uma execução em andamento): agente em execução e
aguardando aprovação, estado de falha, terminal ao vivo, análise com IA em andamento, modais de
atualizar harness e de contexto de tarefa, formulários de workspace, painéis de memória, MCP e
git do agente, estado "desatualizado" do Harness na lista de projetos.

## 8. Como reproduzir as capturas

`tmp/design-capture/` (ignorado pelo git) roda o app real no navegador com um Tauri simulado:
`npx vite --port 1420` e abrir `http://localhost:1420/tmp/design-capture/index.html`. Parâmetros:
`?harness=none|review|ready|stale`, `?agents=0..3`, `?runs=0|1`, `?workspaces=0`.
