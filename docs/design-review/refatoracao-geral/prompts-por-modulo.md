# Refatoração geral — prompts do Stitch por módulo

Segunda rodada da refatoração geral. O prompt completo ([prompt-stitch.md](prompt-stitch.md)) gerou só a tela
"Design system". Aqui cada chamada pede **uma tela**, com a lista exata do que ela contém e do que não contém,
para o Stitch não inventar e para as telas saírem no mesmo padrão.

## Como enviar

- Projeto `526454930665111857`, `deviceType: DESKTOP`, `designSystem: assets/7ac197f798c544509257ebc1201068fc`
  (o design system gerado pelo prompt completo, corpo de 14px).
- Texto de cada chamada = **BASE** inteira + o bloco **TELA** dela.
- Um módulo por rodada, uma tela por chamada. A chamada dá timeout, mas a geração termina: não reenviar; conferir
  com `list_screens` (comparar com os ids já registrados) e anotar no registro no fim deste arquivo.
- O que o Stitch inventar vai para "O Stitch mostra, o app não tem" na avaliação, nunca para a implementação.

## BASE (vai no começo de toda chamada)

A partir do módulo 2, a base tem quatro regras a mais (logo, selo, caixa normal e fundo de modais), porque no
módulo 1 o logo e o selo mudaram de tela para tela.

```text
APP: "Atlas", app DESKTOP (janela nativa) que orquestra agentes de IA em projetos de software. Esta é uma
refatoração visual: mesma função, leitura e acesso mais fáceis, tudo padronizado. Use o design system do
projeto (tema escuro único; superfícies #0b0d10 / #101317 / #15191e / #1c2128 / #242c37 separadas por borda de
1px, sem sombra exceto em menus, modais e gavetas; acento índigo #6366f1; sucesso #10b981, aviso #f59e0b,
erro #ef4444, executando #60a5fa, informação #38bdf8, política #ec4899, ocioso #64748b; Geist no texto,
JetBrains Mono só em valores técnicos: caminhos, ids, comandos, modelos, números, código). Corpo 14px, apoio
13px, mínimo 12px. Títulos de seção em caixa normal 16px, NUNCA em caixa alta mono. Raios 4/6/8px. Janela
1440×900. Todo texto em português do Brasil.

MOLDURA (igual em toda tela; desenhe exatamente isto e nada mais):
- Barra lateral de 216px: no topo só o logo e a palavra "Atlas". Abaixo, o seletor de workspace com
  "Transport ERP" e o caminho "/Users/lucas/dev/transport-erp" (com seta de abrir). Navegação, nesta ordem:
  Workspace, Workflow, Agentes, Personalidades, Specs, MCP Hub, Configurações (só estes 7 itens; o da tela
  atual destacado). Rodapé: "Project Atlas" e "v0.2.0 · macOS". Nada mais na barra lateral.
- Barra superior de 48px: botão de recolher a barra lateral; breadcrumb "Transport ERP / <tela>". À direita,
  só os itens que a TELA disser entre: chip âmbar "N agentes aguardando você", chip "N agentes em execução",
  botão "Celular" (só a palavra e o ícone de celular; com bolinha âmbar e número quando a TELA disser) e a
  alternância "PT | EN". Nada mais na barra superior.

PROIBIDO EM QUALQUER TELA (o app não tem): busca global, ⌘K (exceto no MCP Hub), paleta de comandos,
notificações, sino, avatar, conta, e-mail, login, item "Design system" na navegação, "Documentação", "Ajuda",
"Status do Daemon", "Core", "Cluster", "Core Tokens", número de versão diferente de v0.2.0, "Sync" ou "Sync
OK", telemetria (CPU, RAM, latência, região), barra de status inferior, gráficos, tema claro, botão "Atlas
CLI", commit/push/PR, chat entre agentes, contadores, métricas, selos, filtros, botões, abas, subtítulos,
textos de ajuda ou placeholders que a TELA não descreva. Use só os textos e dados descritos; se sobrar
espaço, deixe vazio. Cada ação aparece UMA vez na tela.

PADRÕES:
- Termos: "Agente", "Etapa" (nunca "step"), "Execução", "Workflow", "Spec", "Personalidade", "Runtime",
  "Modelo", "Projeto", "Harness", "Achado", "Passagem (handoff)", "Concessão", "Servidor MCP".
- Estados, sempre com o mesmo selo (ponto colorido + texto em caixa normal) e a mesma cor: Pronto (verde),
  Executando (azul), Aguardando você (âmbar), Pausado (cinza), Interrompido — pode retomar (âmbar),
  Concluído (verde), Falhou (vermelho), Cancelado (cinza), Pendente (cinza), Indisponível (âmbar),
  Desligado (cinza), Desatualizado (âmbar), Bloqueado (vermelho).
- Ação principal da página: botão índigo no canto direito do cabeçalho da página, um só. Ações raras e
  destrutivas no menu "⋯" (destrutivas em vermelho, no fim). Botões com texto; ícone sozinho só em fechar,
  recolher e "⋯". Formulários com rodapé fixo: "Cancelar" e o botão principal à direita.
- No máximo um modal aberto. Nunca caixa dentro de caixa dentro de caixa. Rótulo perto do valor.
- LOGO, igual em toda tela: quadrado índigo de 20px com a letra "A" branca, seguido da palavra "Atlas".
- SELO DE ESTADO, igual em toda tela: pílula com fundo da cor a 14%, ponto de 6px e texto Geist 12px em caixa
  normal. Nunca caixa retangular mono, nunca CAIXA ALTA. Selos neutros (categoria) são pílulas cinza sem ponto.
- Rótulos de grupo, de campo e de lista sempre em caixa normal (nunca "HOJE", "EXECUÇÃO" em caixa alta).
- Atrás de modais e gavetas aparece só a tela descrita, escurecida, sem dados, agentes ou textos novos.
- O item destacado na navegação lateral é o que a TELA diz em "Navegação:"; nenhum outro item tem destaque ou ponto.
- Painéis e colunas têm só o título descrito: sem rótulos de coluna, rodapé de status, contadores, "stream",
  latência, taxa de tokens, "conexão ativa" ou qualquer texto técnico extra.
- Tudo cabe em 1440px de largura: nada é cortado na borda direita; se faltar espaço, a linha quebra.
- Sem barra de título da janela do sistema (sem semáforos do macOS): a tela começa na barra lateral e na
  barra superior do app.
- Cartões, linhas e nós mostram só os textos descritos: sem subtítulos, ids, "foco atual" ou rótulos extras.
- Espaço vazio embaixo do conteúdo fica vazio: nunca uma barra de status no rodapé; sem dicas de atalho
  ("ESC para fechar").
```

---

## Módulo 1 — Workspace

### 1.1 Workspace — Lista

```text
TELA: "Workspace — Lista". Breadcrumb "Transport ERP / Workspace". Barra superior: só "Celular" e "PT | EN".

CONTÉM, de cima para baixo:
1. Cabeçalho da página: título "Transport ERP"; abaixo, em mono pequeno, "Hoje US$ 1,24 · Semana US$ 3,72".
   À direita: botão secundário "Projetos (2)", botão secundário "Integrações" e o botão principal
   "+ Adicionar agente".
2. Linha de controles: controle segmentado "Lista expansível | Grade" (Lista ativa) e, à direita, botão
   fantasma "Recolher todos".
3. Três agentes em lista, um abaixo do outro:
   - "Arquiteto" EXPANDIDO. Cabeçalho do cartão: nome; selo "Pronto"; uma linha secundária
     "Architect · Claude CLI · Anthropic"; o modelo "sonnet" em mono; "Isolamento Git ligado"; à direita
     "Editar", menu "⋯" (com "Remover do workspace") e o botão de recolher. Abas: Chat (ativa), Atividade,
     Terminal, Detalhes, Execuções (3). Conteúdo do Chat: mensagem da pessoa à direita "Revise a camada de
     autenticação do transport-erp e proponha melhorias."; resposta do agente à esquerda com nome
     "Arquiteto" e hora "14:32", em markdown com um título "Pontos encontrados", uma lista de 3 itens e um
     bloco de código TypeScript curto com o botão "Copiar". Compositor no rodapé do cartão: chip de escopo
     "transport-erp" (abre seletor de projetos), campo "Responder ao Arquiteto ou pedir uma alteração" e
     botão "Enviar".
   - "Desenvolvedor" RECOLHIDO: só a linha do cabeçalho (nome, selo "Pronto", "Developer · Codex · OpenAI",
     modelo "gpt-5-codex", botão de expandir).
   - "QA" RECOLHIDO: nome, selo "Pronto", "QA · OpenCode", modelo "opencode/qwen3-coder", botão de expandir.

NÃO CONTÉM: faixa de métricas, gráfico de custo, busca, filtros, avatar dos agentes com foto, contador de
tokens no cartão, botão "Executar workflow", outro botão "Adicionar agente" no fim da lista, rodapé da página.
```

### 1.2 Workspace — Grade

```text
TELA: "Workspace — Grade". Breadcrumb "Transport ERP / Workspace". Barra superior: só "Celular" e "PT | EN".

CONTÉM:
1. O mesmo cabeçalho da página da tela "Workspace — Lista" (título, consumo, "Projetos (2)",
   "Integrações", "+ Adicionar agente").
2. Controle segmentado "Lista expansível | Grade" com Grade ativa (sem "Recolher todos" nesta visão).
3. Grade 2×2 ocupando a altura útil:
   - Célula 1 "Arquiteto": cabeçalho compacto em UMA linha (nome, selo "Pronto", modelo "sonnet" em mono,
     menu "⋯"); abas Chat, Atividade, Terminal, Detalhes, Execuções; Chat com a última resposta do agente
     (markdown curto); compositor de UMA linha: campo "Mensagem para Arquiteto" + "Enviar" (o escopo vira
     um ícone com dica dentro do campo).
   - Célula 2 "Desenvolvedor": igual, com selo "Pronto" e Chat vazio mostrando
     "Nenhuma mensagem ainda. Pergunte algo a este agente."
   - Célula 3 "QA": igual, selo "Pronto", Chat com uma troca curta.
   - Célula 4 LIVRE: borda tracejada, texto "Espaço livre" e botão secundário "+ Adicionar agente aqui".

NÃO CONTÉM: mais de 4 células, paginação, métricas por célula, terminal aberto, botões de maximizar.
```

### 1.3 Workspace — Agente executando com aprovação pendente

```text
TELA: "Workspace — Agente executando". Breadcrumb "Transport ERP / Workspace". Barra superior: chip âmbar
"1 agente aguardando você", chip "1 agente em execução", botão "Celular" com bolinha âmbar "1", "PT | EN".

CONTÉM: a visão Lista (como "Workspace — Lista"), com "Desenvolvedor" EXPANDIDO e os outros dois recolhidos.
Cartão "Desenvolvedor": selo "Aguardando você"; aba Atividade ativa, mostrando uma linha do tempo vertical com
hora em mono à esquerda: "14:40:02 Iniciando o runtime Codex", "14:40:04 Prompt enviado", "14:40:11 Lendo
src/auth/session.ts", "14:40:19 Lendo src/auth/token.ts", "14:40:31 Pedido de permissão". No fim da linha do
tempo, um aviso cor de política (#ec4899) "Permissão necessária": "Desenvolvedor quer executar: npm test" em
mono, a frase "Expira em 4:12 se você não responder." e dois botões: "Aprovar" (principal) e "Rejeitar"
(secundário). Abaixo do cartão expandido, os cartões recolhidos "Arquiteto" (selo "Pronto") e "QA"
(selo "Pronto").

NÃO CONTÉM: "Sempre permitir", "Aprovar uma vez", barra de progresso, porcentagem, estimativa de tempo,
contador de tokens ao vivo, botão de pausar o agente.
```

### 1.4 Workspace — Detalhes do agente

```text
TELA: "Workspace — Detalhes do agente". Breadcrumb "Transport ERP / Workspace / Arquiteto". Barra superior:
só "Celular" e "PT | EN".

CONTÉM: ao fundo, a visão Lista com "Arquiteto" expandido e a aba "Detalhes" ativa. Os detalhes abrem como
GAVETA à direita (520px) sobre o conteúdo, com fundo escurecido. Gaveta:
1. Cabeçalho: "Arquiteto", selo "Pronto", linha "Architect · Claude CLI · Anthropic · sonnet", botão fechar ×.
2. Abas da gaveta: "Resumo" (ativa), "Permissões", "Contexto", "Consumo".
3. Conteúdo da aba Resumo, em grupos com título:
   - "Última execução": tarefa "Revise a camada de autenticação…", selo "Concluído", "hoje 14:32 · 2m 41s",
     link "Abrir execução".
   - "Consumo de hoje": três números lado a lado em mono: "US$ 0,42", "38.120 tokens", "3 execuções"; abaixo a
     nota "Valores informados pelo runtime."
   - "Agente": pares rótulo/valor próximos: Runtime "Claude CLI 2.1.285"; Modelo "sonnet"; Perfil de
     permissão "Somente leitura"; Isolamento "Git worktree isolado"; Contrato de resultado "Revisão
     (approved / changes_requested)".
   - "Contexto do projeto": selo "Analisado" e a frase "O Harness é contexto, nunca permissão."
   - Ações no rodapé da gaveta: "Editar agente" (secundário).

As outras abas (não mostradas abertas) guardam: Permissões (perfil e quatro áreas: Sistema de arquivos,
Processos, Git, Rede), Contexto (regras, memória do agente, ferramentas MCP concedidas, revisão de contexto,
Git do worktree), Consumo (esta conversa / hoje / semana; cota).

NÃO CONTÉM: gráfico, histórico de custo, avatar, botão de excluir agente na gaveta, botões "Salvar".
```

### 1.5 Workspace — Inspetor de execução

```text
TELA: "Workspace — Execuções do agente". Breadcrumb "Transport ERP / Workspace / Arquiteto / Execuções".
Barra superior: só "Celular" e "PT | EN".

CONTÉM: a visão Lista com "Arquiteto" expandido e a aba "Execuções (3)" ativa, em layout mestre-detalhe
DENTRO do cartão:
- Esquerda (320px): lista agrupada por dia. Grupo "Hoje": três itens, cada um com ponto de estado, a tarefa
  em uma linha cortada com reticências, "Claude CLI · sonnet · 14:32" e a duração à direita em mono
  ("2m 41s", "48s", "1m 05s"). O primeiro selecionado (Concluído), o segundo "Falhou", o terceiro Concluído.
- Direita: detalhe da execução selecionada. Cabeçalho: a tarefa completa e o selo "Concluído". Abas:
  "Detalhes" (ativa), "Contexto", "Atividade", "Chat", "Terminal". Aba Detalhes em dois grupos com pares
  rótulo/valor: "Execução" (Workspace "Transport ERP", Agente "Arquiteto", Personalidade "Architect",
  Runtime "Claude CLI", Modelo "sonnet", Início "14:29:51", Fim "14:32:32", Duração "2m 41s",
  Processo "pid 48213") e "Consumo" (Entrada "31.402 tokens", Saída "6.718 tokens", Total "38.120 tokens",
  Custo "US$ 0,42").

NÃO CONTÉM: modal, botão "Reexecutar", "Exportar log", "Comparar", gráfico, filtros de data, busca.
```

### 1.6 Workspace — Boas-vindas

```text
TELA: "Workspace — Boas-vindas" (primeira abertura, sem nenhum workspace). Barra lateral: no lugar do seletor
de workspace, o texto "Nenhum workspace" (os 7 itens de navegação continuam). Breadcrumb só "Workspace".
Barra superior: só "Celular" e "PT | EN".

CONTÉM, centralizado na área de conteúdo com largura máxima de 560px:
- Título "Bem-vindo ao Atlas".
- Frase: "Um workspace reúne as pastas de um projeto e os agentes que trabalham nele."
- Três passos numerados, cada um com título e uma frase:
  1 "Escolha a pasta do projeto" — "O Atlas só lê nomes de pastas e manifestos; nada é executado."
  2 "Crie seus agentes" — "Cada agente tem personalidade, runtime e modelo."
  3 "Analise o projeto com o Harness" — "O contexto do projeto fica gravado em .atlas/ dentro dele."
- Botão principal "Criar workspace".

NÃO CONTÉM: ilustração, vídeo, links de documentação, "Importar workspace", login, templates de projeto.
```

---

## Módulo 2 — Projetos e Harness

### 2.1 Projetos e Harness

```text
TELA: "Projetos e Harness" (página própria, não modal). Breadcrumb "Transport ERP / Workspace / Projetos".
Barra superior: só "Celular" e "PT | EN".

CONTÉM:
1. Cabeçalho: botão "← Workspace"; título "Projetos de Transport ERP"; à direita o botão principal
   "+ Adicionar pasta de projeto".
2. Lista de projetos (cartões em coluna):
   - "transport-erp": selos "Principal", "Backend" (neutros) e estado "Analisado" (verde); caminho
     "/Users/lucas/dev/transport-erp" em mono; linha "Saudável · Rust, Tauri, PostgreSQL"; linha de números
     em mono "42 achados · 31 verificados · 9 inferidos · 2 seus · 0 desconhecidos"; "Analisado em 20/10/2026
     18:04". À direita: "Atualizar" (secundário), "Editar" (fantasma) e "⋯" com "Recarregar", e, separados em
     vermelho, "Resetar Harness" e "Remover do workspace".
   - "transport-web": selos "Frontend" e estado "Desatualizado" (âmbar); caminho
     "/Users/lucas/dev/transport-web"; "React, TypeScript, Vite"; "Usa: transport-erp"; à direita
     "Atualizar" (secundário), "Editar", "⋯".
3. Seção "Harness": uma frase "O Harness grava o contexto de cada projeto em .atlas/ para os agentes." e o
   botão secundário "Resetar todos os Harnesses".
4. Seção "Contexto do workspace": selo "Atualizado" e botão secundário "Refazer contexto".
5. Seção "Notas do workspace": área de texto com 3 linhas de notas de exemplo e o botão "Salvar notas".

NÃO CONTÉM: gráficos, dependências em grafo, busca, filtros, botão "Analisar todos" além do descrito.
```

### 2.2 Harness — Analisar projeto

```text
TELA: "Harness — Analisar projeto" (página com passos). Breadcrumb "Transport ERP / Projetos / Harness".
Barra superior: só "Celular" e "PT | EN".

CONTÉM:
1. Cabeçalho: "← Projetos"; título "Analisar transport-web"; caminho "/Users/lucas/dev/transport-web" em mono.
2. Passos numerados no topo: "1 Analisar" (atual), "2 Análise ao vivo", "3 Revisar contexto".
3. Aviso de informação com cadeado: "Só nomes de pastas e manifestos são lidos. Nada é executado e o .env
   nunca é lido."
4. Bloco "Análise com IA (recomendado)": uma frase "Um agente lê os manifestos e propõe o contexto; você revisa
   tudo antes de gravar."; campo "O que o agente deve saber? (opcional)" com contador "0 / 2.000 tokens";
   seletor "Agente que analisa" com "Arquiteto · Claude CLI · sonnet".
5. Bloco "Detectado no projeto", em três colunas com título e itens marcados: "Stack e runtime" (React 19,
   TypeScript 6, Vite 8), "Git e testes" (Repositório Git, Vitest), "Topologia" (Projeto único, Usa
   transport-erp).
6. Rodapé fixo: "Cancelar" (fantasma), "Revisar sem IA" (secundário), "Analisar com IA" (principal).

NÃO CONTÉM: selos "ATLAS ENGINE" ou "Local Core", estimativa de custo, estimativa de tempo, escolha de modelo
separada do agente.
```

### 2.3 Harness — Análise ao vivo

```text
TELA: "Harness — Análise ao vivo". Breadcrumb "Transport ERP / Projetos / Harness". Barra superior: chip
"1 agente em execução", "Celular", "PT | EN".

CONTÉM:
1. Cabeçalho: título "Arquiteto está analisando transport-web"; selo "Executando"; à direita o botão
   secundário "Interromper".
2. Passos: "1 Analisar" (concluído ✓), "2 Análise ao vivo" (atual), "3 Revisar contexto".
3. Coluna esquerda "Ferramentas usadas": lista em mono com tempo: "Read package.json 120ms",
   "Read vite.config.ts 84ms", "Glob src/**/*.tsx 210ms", "Read src/main.tsx 66ms" (a última com ponto azul
   "rodando").
4. Coluna direita "Saída do agente": texto corrido em fluxo, 6 a 8 linhas descrevendo o que encontrou,
   com cursor no fim.

NÃO CONTÉM: barra de porcentagem, tempo restante, gráficos, botão de pausar, chat com o agente.
```

### 2.4 Harness — Revisar contexto

```text
TELA: "Harness — Revisar contexto". Breadcrumb "Transport ERP / Projetos / Harness". Barra superior: só
"Celular" e "PT | EN".

CONTÉM:
1. Cabeçalho: "← Voltar"; título "Revisar contexto de transport-web"; chip mono ".atlas/"; à direita o botão
   principal "Gravar no projeto".
2. Passos: 1 ✓, 2 ✓, "3 Revisar contexto" (atual).
3. Linha de números: "Total 24 · Verificados 15 · Inferidos 7 · Pendentes 2".
4. Abas por área com contagem: "Stack (8)" ativa, "Testes e comandos (5)", "Arquitetura e módulos (7)",
   "Contexto de negócio (4)".
5. Mestre-detalhe:
   - Esquerda: achados agrupados por categoria com título em caixa normal ("Linguagem", "Framework",
     "Infraestrutura"). Cada achado numa linha: caixa de seleção (incluir), o valor editável ("TypeScript 6",
     "React 19", "Vite 8", "Vitest"), selo de origem ("Fato" verde, "Inferência" azul, "Seu" índigo), a
     confiança ("alta", "média") e o arquivo de origem em mono ("package.json"). Um achado selecionado.
   - Direita "Evidência": para o achado selecionado, o arquivo "package.json" e um trecho de código de 6 linhas
     com a linha relevante destacada; abaixo "Confiança alta · Fato verificado".
6. Aviso âmbar recolhível "2 achados estão desatualizados: a evidência mudou desde a última análise."
7. Rodapé: chave "Não enviar o .atlas/ ao Git" com a frase "Adiciona .atlas/ ao .gitignore deste projeto."

NÃO CONTÉM: painel de evidência vazio, "Aprovar todos", pontuação, gráfico, comentários.
```

---

## Módulo 3 — Workflow

### 3.1 Workflow — Editar

```text
TELA: "Workflow — Editar". Breadcrumb "Transport ERP / Workflow". Barra superior: só "Celular" e "PT | EN".

CONTÉM (o canvas é a figura principal e ocupa pelo menos 60% da altura):
1. Barra do workflow (uma linha): seletor "Recuperação de senha" (abre a lista de workflows); selo neutro
   "Automático · versão 1"; "+ Novo workflow" (secundário); à direita "Personalizar" (secundário), "Salvar"
   (secundário) e "⋯" com "Salvar como template", "Exportar", "Importar" e, em vermelho, "Excluir workflow".
2. Barra de execução logo abaixo (uma linha): campo largo "O que os agentes devem fazer?" com o texto
   "Implementar recuperação de senha por e-mail"; chip de escopo "transport-erp"; chave "Aprovar planos
   automaticamente" (desligada); botão principal "Executar". À esquerda do campo, um controle compacto
   "Vibe coding ▾" (abre: "Vibe coding" e "A partir de uma spec aprovada").
3. Canvas (fundo void com grade de pontos): nós "Arquiteto" (agente; "Architect · Claude CLI"),
   "Desenvolvedor" (agente; "Developer · Codex"), "QA" (agente; "QA · OpenCode"), nó de condição "QA passou?"
   e nó "Fim". Arestas: Arquiteto → Desenvolvedor, Desenvolvedor → QA, QA → condição; condição → Fim com rótulo
   "pass"; condição → Desenvolvedor com rótulo "fail" (loop). Rótulos de aresta legíveis (13px).
   Barra de ferramentas flutuante no canto superior esquerdo do canvas: "+ Agente", "+ Condição", "+ Fim",
   "Conectar", "Remover", separador, "Desfazer", "Refazer", "Redefinir layout". No canto inferior direito:
   zoom "− 76% +", "Ajustar" e o minimapa (sem cobrir nós). No canto inferior esquerdo, em mono:
   "5 etapas · 5 rotas".
4. Coluna direita (320px) "Visão geral": nome editável "Recuperação de senha"; "Automático · versão 1 ·
   5 etapas"; seção "Ferramentas (MCP)" com "github" (selo "Conectado", "concedido a Desenvolvedor") e
   "chrome-devtools" (selo "Desligado", "concedido a ninguém"); link "Gerenciar no MCP Hub".

NÃO CONTÉM: faixa "Como você quer começar" separada, botões de editar desabilitados duplicados,
histórico de execuções no canvas, chat, métricas de custo.
```

### 3.2 Workflow — Etapa selecionada

```text
TELA: "Workflow — Etapa selecionada". Breadcrumb "Transport ERP / Workflow / Desenvolvedor". Barra superior:
só "Celular" e "PT | EN".

CONTÉM: a mesma tela "Workflow — Editar", com o nó "Desenvolvedor" selecionado (borda índigo) e a coluna
direita trocada pelo INSPETOR DA ETAPA (380px), que rola por dentro:
1. Cabeçalho: "Desenvolvedor" · "Etapa de agente"; selo verde "Etapa válida"; id "node_dev" em mono pequeno;
   botão fechar ×.
2. Seções recolhíveis, a primeira aberta e as outras fechadas mostrando só título e um resumo de uma linha:
   - "Agente" (aberta): seletor "Agente" = "Desenvolvedor"; campo "Rótulo" = "Desenvolvedor"; fatos em pares
     (não editáveis): Personalidade "Developer", Runtime "Codex", Modelo "gpt-5-codex", Isolamento
     "Worktree isolado", Contexto "Por tarefa".
   - "Contrato de resultado" — resumo "Implementação: implemented, partial, blocked".
   - "Instruções e critério de conclusão" — resumo "Implementar o fluxo descrito pelo Arquiteto…".
   - "Permissões" — resumo "Desenvolvedor".
   - "Resiliência" — resumo "2 tentativas · ao falhar: parar · loop máximo 3".
   - "Ferramentas (MCP)" — resumo "github: todas as ferramentas".
   - "Dependências e rotas" — resumo "depois de Arquiteto · vai para QA".
3. Rodapé fixo do inspetor: texto "Alterações não salvas" (âmbar) e botão principal "Salvar configuração".

NÃO CONTÉM: o painel "Visão geral" ao mesmo tempo, campos de temperatura ou tokens, escolha de modelo aqui.
```

### 3.3 Workflow — Novo workflow

```text
TELA: "Workflow — Novo workflow" (página com passos). Breadcrumb "Transport ERP / Workflow / Novo".
Barra superior: só "Celular" e "PT | EN".

CONTÉM:
1. Cabeçalho: "← Workflow"; título "Novo workflow"; à direita, "⋯" com "Importar".
2. Passos: "1 Tarefa" (concluído ✓), "2 Modelo" (atual), "3 Nome".
3. Resumo do passo 1 numa linha: "Tarefa: Implementar recuperação de senha por e-mail · Vibe coding ·
   Automático" com link "Alterar".
4. Faixa de sugestão: "Sugerido para a sua tarefa: Implementar".
5. Mestre-detalhe:
   - Esquerda: cartões de template em coluna, cada um com nome, uma frase e "N etapas · N ferramentas
     sugeridas": "Implementar" (selo "Recomendado", selecionado; "4 etapas · 2 ferramentas sugeridas"),
     "Mudança rápida" ("2 etapas · 0 ferramentas sugeridas"), "Depurar e corrigir" ("3 etapas · 1 ferramenta
     sugerida"), "Revisar e auditar" ("2 etapas · 1 ferramenta sugerida · Somente leitura").
   - Direita "Implementar": descrição curta; "Etapas" numeradas (Arquiteto — planeja; Desenvolvedor —
     implementa, "feito quando: os testes passam"; QA — valida, loop até 3 vezes com o Desenvolvedor;
     Revisor — revisa); "Saídas" (plano, alterações de código, relatório de validação); "Ferramentas
     sugeridas": "github — pronta" (verde) e "chrome-devtools — faltando" (âmbar) e o botão secundário
     "Aplicar ferramentas".
6. Rodapé fixo: "Voltar" (secundário) e "Continuar" (principal).

NÃO CONTÉM: geração por IA do grafo, preço, avaliação com estrelas, autor do template, busca de templates.
```

### 3.4 Workflow — Execução aguardando você

```text
TELA: "Workflow — Execução aguardando você". Breadcrumb "Transport ERP / Workflow / Recuperação de senha".
Barra superior: chip âmbar "1 agente aguardando você", chip "1 agente em execução", "Celular" com bolinha "1",
"PT | EN".

CONTÉM:
1. Barra da execução (uma linha): "← Voltar ao editor"; seletor de execução "21/10/2026 14:52 · Aguardando
   você"; selo âmbar "Aguardando você"; chave "Aprovar planos automaticamente"; à direita "Pausar"
   (secundário) e "⋯" com "Cancelar execução" (vermelho).
2. Área dividida: canvas à esquerda (55% da largura, altura toda) com os nós em estado: Arquiteto "Concluído",
   Desenvolvedor "Aguardando você" (borda âmbar), QA "Pendente", condição e Fim "Pendente".
3. À direita (45%), o painel "Ação necessária" com borda âmbar:
   - "Desenvolvedor está aguardando sua resposta"; selo "Plano para aprovar"; "Etapa 2 de 4".
   - A pergunta: "Você aprova este plano?"
   - "Plano" em markdown (título "Recuperação de senha", lista de 5 passos, um bloco de código curto), com
     "Mostrar tudo".
   - "Contexto" recolhido.
   - Campo opcional "Uma nota para o agente".
   - Botões: "Aprovar" (principal), "Pedir ajustes" (secundário), "Refazer o plano" (secundário).
   - Nota: "Sua resposta nunca aplica mudanças ao projeto: isso é uma revisão separada."

NÃO CONTÉM: "Cancelar execução" visível fora do "⋯", chat livre com o agente, estimativa de custo, botão
"Aprovar sempre".
```

### 3.5 Workflow — Execução em andamento

```text
TELA: "Workflow — Execução em andamento". Breadcrumb "Transport ERP / Workflow / Recuperação de senha".
Barra superior: chip "2 agentes em execução", "Celular", "PT | EN".

CONTÉM:
1. Barra da execução: "← Voltar ao editor"; seletor "21/10/2026 14:52 · Executando"; selo azul "Executando";
   chave "Aprovar planos automaticamente" (ligada); à direita "Pausar" e "⋯" com "Cancelar execução".
2. Canvas à esquerda (60%): Arquiteto "Concluído", Desenvolvedor "Concluído", QA "Executando" (borda azul),
   condição e Fim "Pendente".
3. Painel à direita (40%) com abas "Visão geral" (ativa), "Passagens (2)", "Alterações (3)", "Ao vivo":
   - Barra de progresso "2 / 4 etapas concluídas · 50%".
   - Lista de etapas: Arquiteto ✓ "3m 12s"; Desenvolvedor ✓ "11m 40s"; QA ● "executando há 1m 05s";
     Fim ○ "Pendente".
   - Linha de contadores em mono: "Executando 1 · Pendentes 1 · Falharam 0 · Bloqueados 0".
   - "Tarefa": "Implementar recuperação de senha por e-mail".
   - "Ferramentas (MCP)": "github · Conectado".

NÃO CONTÉM: tempo restante estimado, gráfico, logs de terminal no painel, chat.
```

### 3.6 Workflow — Recuperar execução

```text
TELA: "Workflow — Recuperar execução". Breadcrumb "Transport ERP / Workflow / Recuperação de senha".
Barra superior: só "Celular" e "PT | EN".

CONTÉM:
1. Barra da execução: "← Voltar ao editor"; seletor "20/10/2026 22:07 · Interrompido"; selo âmbar
   "Interrompido — pode retomar"; à direita "⋯" com "Cancelar execução".
2. Canvas à esquerda (55%): Arquiteto "Concluído", Desenvolvedor "Interrompido" (borda âmbar), QA e Fim
   "Pendente".
3. Painel à direita "Esta execução pode ser retomada" com uma frase "O Atlas foi fechado enquanto a etapa
   rodava." e pares: "Último passo concluído: Arquiteto"; "Parou em: Desenvolvedor"; "Continua a partir de:
   Desenvolvedor"; "Repete o passo que falhou: Desenvolvedor"; "Mantidos sem refazer: Arquiteto";
   "Retomada 0 vez(es)". Botão principal "Retomar de onde parou".

NÃO CONTÉM: "Reiniciar do zero", escolha de etapa para retomar, logs, gráfico.
```

### 3.7 Workflow — Revisão e entrega do código

```text
TELA: "Workflow — Revisão e entrega". Breadcrumb "Transport ERP / Workflow / Recuperação de senha".
Barra superior: só "Celular" e "PT | EN".

CONTÉM:
1. Barra da execução: "← Voltar ao editor"; seletor "21/10/2026 14:52 · Concluído"; selo verde "Concluído".
2. Linha de números em mono: "4 etapas · 3 agentes · 3 arquivos +113 −4 · 18m 0s · 1 achado".
3. Cartão de ENTREGA em destaque (largura toda, borda índigo):
   - Título "O código está pronto para revisão"; selos "Pronto para revisão" e "Não aplicado".
   - Frase: "As alterações estão num worktree isolado e ainda não estão no seu projeto. Aplicar só altera seus
     arquivos; nunca faz commit nem push."
   - "3 arquivos alterados · +113 −4".
   - Ações: "Aplicar alterações" (principal), "Revisar diff" (secundário), "Abrir na IDE" (secundário) e "⋯"
     com "Manter isolado" e, em vermelho, "Descartar".
4. Abaixo, duas colunas:
   - Esquerda "Achados (1)": um achado âmbar "Aviso": "O link de recuperação não expira"; "Encontrado por QA";
     "Recomendação: expirar o link em 30 minutos"; selo "Sem confirmação de correção".
   - Direita "Linha do tempo": Arquiteto ✓ 3m 12s; Desenvolvedor ✓ 11m 40s (2 tentativas); QA ✓ 2m 08s;
     Revisor ✓ 1m 00s; cada um com o link "Abrir execução".
5. Seções recolhidas (título + contagem), uma por linha: "Passagens (3)", "Decisões (2)", "Artefatos (2)",
   "Conjunto de alterações (3)".

NÃO CONTÉM: "Fazer commit", "Abrir PR", "Mesclar", gráfico, avaliação do resultado.
```

---

## Módulo 4 — Agentes e Personalidades

### 4.1 Agentes — Lista

```text
TELA: "Agentes — Lista". Breadcrumb "Transport ERP / Agentes". Barra superior: só "Celular" e "PT | EN".

CONTÉM:
1. Cabeçalho: título "Agentes" com contagem "3"; à direita o botão principal "+ Criar agente" e "⋯" com
   "Importar".
2. Linha de controles: campo de busca "Buscar agente ou modelo" e filtros "Todos (3) · Prontos (2) ·
   Indisponíveis (1)".
3. Lista em linhas (tabela simples, sem caixa dentro de caixa), colunas: Agente (nome + personalidade embaixo),
   Runtime, Modelo (mono), Permissões, Resultado, Isolamento, e ações à direita.
   - "Arquiteto" · Architect · "Claude CLI" selo "Pronto" · "sonnet" · "Somente leitura" · "Revisão" ·
     "Worktree" · "Editar" e "⋯".
   - "Desenvolvedor" · Developer · "Codex" selo "Pronto" · "gpt-5-codex" · "Desenvolvedor" ·
     "Implementação" · "Worktree" · "Editar" e "⋯".
   - "QA" · QA · "Gemini CLI" selo "Indisponível" com o motivo embaixo "Sem login" · "gemini-3-pro" ·
     "Somente leitura" · "Validação" · "Worktree" · "Editar" e "⋯".
   O "⋯" de cada linha tem "Exportar" e, em vermelho, "Excluir".

NÃO CONTÉM: faixa de métricas, gráfico, avatar com foto, ícones sem legenda, ordenação inventada, paginação.
```

### 4.2 Agentes — Criar agente

```text
TELA: "Agentes — Criar agente". Breadcrumb "Transport ERP / Agentes / Criar agente". Barra superior: só
"Celular" e "PT | EN".

CONTÉM: formulário em UMA coluna centralizada (largura máxima 720px), seções com título, rodapé fixo.
1. Cabeçalho: "← Agentes"; título "Criar agente".
2. "Identidade": campo "Nome" = "Revisor"; seletor "Personalidade" = "Architecture & Code Validator" com a
   descrição dela embaixo em uma frase; logo abaixo, um aviso de informação: "Esta personalidade sugere:
   contrato Revisão e perfil Somente leitura." com o botão fantasma "Aplicar sugestão".
3. "Runtime e modelo": seletor "Runtime" = "Claude CLI" com selo "Pronto · 2.1.285"; campo "Modelo" = "opus"
   com a ajuda "Claude não lista modelos: digite o id ou o alias (sonnet, opus)."; botão fantasma "Detectar
   runtimes novamente".
4. "Instruções adicionais": área de texto com 3 linhas.
5. "Contrato de resultado": rádio em cartões compactos numa linha: Geral, Validação, Revisão (selecionado),
   Implementação, Personalizado; abaixo, os desfechos de Revisão: "approved — Aprovado" e
   "changes_requested — Alterações pedidas".
6. "Permissões e isolamento": rádio "Somente leitura" (selecionado; "Lê arquivos; não edita nem executa")
   e "Desenvolvedor" ("Edita arquivos no worktree isolado"); caixa marcada "Sempre trabalhar em um Git
   worktree isolado".
7. Rodapé fixo: "Cancelar" e "Criar agente" (principal).

NÃO CONTÉM: chave de API, temperatura, max tokens, prompt de sistema editável aqui, avatar, segunda coluna.
```

### 4.3 Personalidades — Biblioteca

```text
TELA: "Personalidades — Biblioteca". Breadcrumb "Transport ERP / Personalidades". Barra superior: só "Celular"
e "PT | EN".

CONTÉM:
1. Cabeçalho: título "Personalidades"; linha em mono "6 nativas · 1 sua"; à direita o botão principal
   "+ Nova personalidade" e "⋯" com "Importar" e "Restaurar personalidades padrão".
2. Aviso de informação recolhível, fechado em uma linha: "Toda personalidade recebe o protocolo do Atlas
   (narração ao vivo e formato de resultado)." com "Ver mais".
3. Controles: busca "Buscar por nome, tag ou instrução" e filtros "Todas (7) · Padrão do sistema (6) ·
   Customizadas (1)".
4. Grade de 3 colunas de cartões RESUMIDOS (todos com a mesma altura): nome; selo neutro "Padrão" ou
   "Personalizada"; descrição em 2 linhas; até 3 tags; rodapé do cartão com "Usar" (secundário), "Editar"
   (fantasma) e "⋯" (Exportar; Excluir em vermelho). Cartões: Architect, Spec Architect, Developer, QA,
   Architecture & Code Validator, Bug Fixer, e "Revisor de segurança" (Personalizada).
   O cartão "Developer" está EXPANDIDO, mostrando também "Comportamento" (3 pontos) e "Instruções do
   sistema" (bloco mono de 5 linhas) e "Mostrar menos".

NÃO CONTÉM: avaliações, contagem de uso, autor, faixa de métricas, ícones sem legenda.
```

### 4.4 Personalidades — Editar

```text
TELA: "Personalidades — Editar". Breadcrumb "Transport ERP / Personalidades / Developer". Barra superior:
só "Celular" e "PT | EN".

CONTÉM: a biblioteca ao fundo (escurecida) e uma GAVETA à direita (560px):
1. Cabeçalho: "Editar personalidade"; selo "Padrão"; botão fechar ×.
2. Aviso de informação: "Esta é uma personalidade padrão. Suas alterações são salvas como sua própria versão;
   “Restaurar personalidades padrão” as desfaz."
3. Campos: "Nome" = "Developer"; "Descrição" (2 linhas); "Instruções" (área de texto mono, 10 linhas);
   "Tags" = "implementação, código" com a ajuda "Separadas por vírgula".
4. Rodapé fixo da gaveta: "Cancelar" e "Salvar personalidade" (principal).

NÃO CONTÉM: escolha de modelo, temperatura, prévia de conversa, histórico de versões.
```

---

## Módulo 5 — Specs

### 5.1 Specs — Lista

```text
TELA: "Specs — Lista". Breadcrumb "Transport ERP / Specs". Barra superior: chip âmbar "1 agente aguardando
você", "Celular" com bolinha "1", "PT | EN".

CONTÉM:
1. Cabeçalho: título "Specs"; uma frase "Especifique antes de implementar: agentes escrevem, você aprova.";
   linha em mono "2 em andamento · 1 aguardando você · 1 pronta para implementar"; à direita o botão
   principal "Nova especificação" e "⋯" com "Importar specs".
2. Controles: busca "Buscar por título, id ou pedido" e filtros "Todas (4) · Em andamento (2) · Aguardando
   você (1) · Bloqueadas (0) · Prontas (1) · Arquivadas (0)".
3. Lista em linhas (uma spec por linha), cada uma com: título; id em mono; selo de estado; o pedido em uma
   linha cortada; "N/4 aprovados" com uma barra fina; quem escreve; e à direita o tempo relativo.
   - "Recuperação de senha" · "password-reset" · "Aguardando você" · "2/4 aprovados" · "Arquiteto" ·
     alerta âmbar "2 perguntas para você" · "há 4 min".
   - "Modo escuro" · "dark-mode" · "Executando" (texto "Agente escrevendo: Design") · "2/4" · "há 1 min".
   - "Exportar relatório em PDF" · "pdf-export" · "Pronto para implementação" (verde) · "4/4" · "ontem".
   - "Auditoria de login" · "login-audit" · "Rascunho" · "0/4" · "há 3 dias".

NÃO CONTÉM: grade de cartões com barra colorida lateral, kanban, gráficos, responsáveis com avatar.
```

### 5.2 Specs — Documento em revisão

```text
TELA: "Specs — Documento em revisão". Breadcrumb "Transport ERP / Specs / Recuperação de senha". Barra
superior: chip âmbar "1 agente aguardando você", "Celular", "PT | EN".

CONTÉM:
1. Cabeçalho: "← Todas as specs"; título "Recuperação de senha"; selo "Aguardando você"; à direita
   "Agentes e ferramentas" (secundário) e "⋯" com "Excluir" (vermelho). Linha de metadados em mono:
   "password-reset · spec-driven/1 · transport-erp · escreve: Arquiteto · revisa: QA".
2. Passos: "Proposta" ✓ Aprovado, "Requisitos" (atual, "Aguardando revisão"), "Design" ("Aguardando documentos
   anteriores"), "Tarefas", "Revisão final".
3. Duas colunas:
   - Esquerda (documento): barra com "specs/password-reset/spec.md" em mono, seletor "Revisão 2", "escrito pelo
     Arquiteto há 4 min" e o controle "Renderizado | Markdown"; o documento em markdown renderizado (títulos
     "Requisito: Pedir recuperação", "Cenário: e-mail cadastrado", listas com QUANDO/ENTÃO) com largura de
     leitura de ~72 caracteres.
   - Direita (300px): "Validações" com selo "Aprovado" e "Nenhum problema na estrutura." e o botão fantasma
     "Rodar validações novamente"; "Perguntas do agente (2)" com as duas perguntas e o botão "Responder";
     "Consumo": "Entrada 42.100 · Saída 8.300 tokens · limite 200.000"; "Histórico": "Revisão 2 · revisada",
     "Revisão 1 · gerada".
4. Barra "Sua decisão" fixa no fundo: "Revise os requisitos. Nada roda até você pedir." e os botões "Solicitar
   alterações" (secundário), "Escrever de novo" (fantasma) e "Aprovar e continuar" (principal).

NÃO CONTÉM: comentários no documento, edição livre do documento, colaboradores, gráfico de consumo.
```

### 5.3 Specs — Revisão final

```text
TELA: "Specs — Revisão final". Breadcrumb "Transport ERP / Specs / Exportar relatório em PDF". Barra superior:
só "Celular" e "PT | EN".

CONTÉM:
1. Cabeçalho: "← Todas as specs"; título "Exportar relatório em PDF"; selo verde "Pronto para implementação";
   "Agentes e ferramentas" e "⋯". Metadados em mono "pdf-export · spec-driven/1 · transport-erp".
2. Passos: os quatro documentos ✓ e "Revisão final" atual.
3. Seção "O que falta para ficar pronta" com selo "Pronta": sete linhas de verificação, cada uma com ✓ verde,
   texto e o link "Abrir": "Os quatro documentos aprovados", "Estrutura validada", "Documentos coerentes entre
   si", "Nenhuma pergunta em aberto", "Revisada pelo agente revisor", "Achados decididos", "Sua aprovação
   final".
4. Seção "Revisão pelo agente revisor": selo "Concluído"; um achado resolvido: categoria "Risco", severidade
   "Média", estado "Resolvido", texto "O PDF pode expor dados de outro cliente", ações "Ver no documento" e
   "Reabrir".
5. Seção "Publicar no projeto": campo "Pasta da mudança" = "openspec/changes/pdf-export"; selo "Publicada";
   "4 arquivos gravados"; botão secundário "Publicar de novo"; "⋯" com "Arquivar".
6. Seção "Implementar": botão principal "Iniciar um workflow a partir desta spec".

NÃO CONTÉM: assinatura digital, aprovações de outras pessoas, gráficos, estimativa de esforço.
```

### 5.4 Specs — Nova especificação

```text
TELA: "Specs — Nova especificação" (modal M, 640px, sobre a lista escurecida). Barra superior: só "Celular" e
"PT | EN".

CONTÉM, dentro do modal, rolando por dentro:
1. Título "Nova especificação"; seletor "Projeto" = "transport-erp".
2. Área de texto "O que você quer construir?" com "Permitir que o cliente recupere a senha por e-mail".
3. Bloco "Ajude o agente a não adivinhar" com três perguntas que surgiram do texto, cada uma com um campo
   curto e um "porque…" em texto de apoio: "Que dados pessoais são tratados?", "Quem pode fazer isso?",
   "Há integrações envolvidas?".
4. Campo "Mais alguma coisa que o agente deve saber" (2 linhas).
5. Dois seletores lado a lado: "Agente que escreve" = "Arquiteto"; "Agente que revisa" = "QA".
6. Nota: "Criar apenas guarda o pedido. Nenhum agente roda até você iniciar."
7. Rodapé do modal: "Cancelar" e "Criar" (principal).

NÃO CONTÉM: modelos de spec, prioridade, prazo, responsáveis, anexos.
```

---

## Módulo 6 — MCP Hub

### 6.1 MCP Hub — Servidores

```text
TELA: "MCP Hub — Servidores". Breadcrumb "Transport ERP / MCP Hub". Barra superior: só "Celular" e "PT | EN".

CONTÉM:
1. Cabeçalho: título "MCP Hub"; frase "Servidores de ferramentas para os agentes. O que uma ferramenta devolve
   nunca é tratado como instrução."; à direita "Verificar conexões" (secundário), o botão principal
   "+ Adicionar servidor MCP" e "⋯" com "Sincronizar CLIs" e "Importar ou exportar atlas-mcp.json".
2. Linha de números em mono: "3 servidores · 1 conectado · 1 com falha · 1 desligado · 12 ferramentas ·
   2 concessões".
3. Controles: filtros "Todos (3) · Locais (stdio) (3) · Remotos (HTTP/SSE)" (este desabilitado com a dica
   "ainda não suportado"); busca "Buscar servidor, ferramenta ou comando" com o atalho "⌘K"; chave
   "Apenas conectados".
4. Mestre-detalhe:
   - Esquerda (lista): "github" (selo "Conectado", chave ligada, "npx -y @modelcontextprotocol/server-github"
     em mono, "5 ferramentas · Agentes: Desenvolvedor"); "postgres" (selo "Falhou", chave ligada, aviso âmbar
     "Segredo faltando: DATABASE_URL"); "chrome-devtools" (selo "Desligado", chave desligada, "Concedido a
     ninguém"). "github" selecionado.
   - Direita (inspetor de "github"): cabeçalho "github" + selo "Conectado" + "⋯" (Exportar; Remover em
     vermelho); abas "Ferramentas (5)" (ativa), "Variáveis e segredos", "Manifesto", "Atividade". Conteúdo:
     "Examinado em 21/10/2026 14:03 via Claude CLI" e o botão "Examinar"; as 5 ferramentas em chips mono
     (create_issue, get_file_contents, list_commits, search_code, create_pull_request); "Quem pode usar":
     "Desenvolvedor · todas as ferramentas · neste workspace" com "Revogar"; botão secundário "Conceder".

NÃO CONTÉM: servidores remotos funcionando, gráficos, logs do servidor, marketplace pago, métricas além da
linha descrita.
```

### 6.2 MCP Hub — Adicionar servidor

```text
TELA: "MCP Hub — Adicionar servidor" (modal G, 960px, sobre a lista escurecida). Barra superior: só "Celular" e
"PT | EN".

CONTÉM, no modal:
1. Título "Adicionar servidor MCP"; abas "Conhecidos do Atlas" (ativa), "Registro MCP", "Manual".
2. Nota: "Todo servidor novo entra desligado. Nada é iniciado nem concedido até você fazer isso."
3. Grade de 3 cartões:
   - "Chrome DevTools": selo vermelho "Risco alto"; uma frase do que faz; "Executaria: npx -y
     chrome-devtools-mcp@0.9.0" em mono; requisitos "✓ Node.js", "✓ npx", "✓ Google Chrome";
     botão "Adicionar (fica desligado)"; link "Fonte".
   - "Jira": selo vermelho "Risco alto"; frase; "Executaria: npx -y mcp-remote …" em mono; requisitos
     "✓ Node.js", "✓ npx", "✕ Login da Atlassian salvo"; botão "Adicionar (fica desligado)"; link "Fonte".
   - "Figma": selo vermelho "Risco alto"; frase; no lugar do botão, o texto "Ainda não disponível: precisa de
     HTTP e login OAuth"; link "Fonte".
   Todos os três têm o mesmo nível de risco (o app só tem "Risco alto").
4. Rodapé do modal: "Fechar".

NÃO CONTÉM: preços, avaliações, número de downloads, instalação automática ligada.
```

### 6.3 MCP Hub — Sincronizar CLIs

```text
TELA: "MCP Hub — Sincronizar CLIs" (modal G, 960px). Barra superior: só "Celular" e "PT | EN".

CONTÉM, no modal:
1. Título "Sincronizar servidores MCP entre suas CLIs"; uma frase "Compara os servidores do Atlas com os
   arquivos de configuração de cada CLI. Nada é mudado até você confirmar."
2. Tabela: linhas = servidores (github, postgres, linear); colunas = Atlas, Claude Code, Gemini CLI, OpenCode,
   Codex, GitHub Copilot CLI. Células com selos pequenos: "tem", "será adicionado" (índigo), "será trazido
   para o Atlas" (azul), "indisponível" (cinza), "sem arquivo" (cinza).
3. Linha abaixo do cabeçalho da tabela com o estado de cada arquivo: "arquivo encontrado", "sem arquivo",
   "ilegível (não mexemos)".
4. Aviso âmbar: "Segredos nunca são copiados entre arquivos; cada CLI guarda os seus."
5. Rodapé do modal: "Cancelar" e o botão principal "Sincronizar: adicionar 3 a 2 arquivos, trazer 1".

NÃO CONTÉM: sincronização automática, agendamento, histórico de sincronizações.
```

---

## Módulo 7 — Configurações e Celular

### 7.1 Configurações — Geral

```text
TELA: "Configurações — Geral". Breadcrumb "Transport ERP / Configurações". Barra superior: só "Celular" e
"PT | EN".

CONTÉM: navegação de seções à esquerda da área (200px): "Geral" (ativa), "Memória", "Importar e exportar",
"Sobre e atualizações". À direita, coluna de largura máxima 720px com grupos separados por linha:
1. "Idioma": seletor "Português (Brasil)"; ajuda "A saída dos modelos e o seu conteúdo não são traduzidos."
2. "Limite de tokens por especificação": campo numérico vazio com placeholder "Sem limite" e botão "Salvar";
   ajuda "Ao chegar no limite, nenhuma execução nova começa até você decidir continuar."
3. "Sandbox dos runtimes": selo "Disponível neste sistema"; controle segmentado "Desligado | Quando possível
   | Obrigatório" (Desligado ativo); caixa "Deixar escrever nos caches de ferramentas (npm, cargo, pip…)";
   "O que o sandbox protege" recolhido.
4. "Presença no Discord": chave "Mostrar o que o Atlas está fazendo no Discord" (desligada); caixa
   "Mostrar nomes do workspace, do workflow e da etapa" (desabilitada enquanto a chave está desligada);
   ajuda "Prompts, arquivos e respostas nunca são enviados."

NÃO CONTÉM: tema, fonte, atalhos, conta, chaves de API, telemetria, "Salvar tudo".
```

### 7.2 Configurações — Memória

```text
TELA: "Configurações — Memória". Breadcrumb "Transport ERP / Configurações / Memória". Barra superior: só
"Celular" e "PT | EN".

CONTÉM: a mesma navegação de seções com "Memória" ativa. Coluna de 720px:
1. Frase de abertura: "Notas de trabalhos anteriores. Uma nota é uma dica para o agente, nunca uma instrução
   nem uma permissão."
2. "Onde as notas são usadas": três linhas, cada uma com rótulo e controle segmentado:
   "Minhas (todos os projetos)": "Desligado | Propor, eu aprovo" (Propor ativo);
   "Workspace": "Desligado | Propor, eu aprovo | Automático" (Propor ativo);
   "Agente": "Desligado | Propor, eu aprovo | Automático" (Desligado ativo).
   Campo "Máximo de tokens por prompt" = "600" e botão "Definir limite".
3. Abas: "Esperando sua decisão (2)" (ativa), "Notas guardadas (5)", "Adicionar nota".
   Conteúdo: duas notas propostas, cada uma com título ("Os testes rodam com npm run check"), linha
   "Como fazer · Workspace · proposta por Desenvolvedor", o texto em 2 linhas, selo "Não confirmada", e as
   ações "Aprovar" (principal pequeno), "Editar" (fantasma), "Descartar" (fantasma).

NÃO CONTÉM: busca de notas, gráfico de uso, importação de memória, notas de outras pessoas.
```

### 7.3 Configurações — Importar e exportar

```text
TELA: "Configurações — Importar e exportar". Breadcrumb "Transport ERP / Configurações / Importar e exportar".
Barra superior: só "Celular" e "PT | EN".

CONTÉM: navegação de seções com "Importar e exportar" ativa. Coluna de 720px:
1. Frase: "Compartilhe agentes, personalidades e workflows como arquivo. Nunca vão: segredos, conversas, logs
   nem caminhos."
2. Dois cartões lado a lado:
   - "Exportar": "Escolha o que entra no arquivo." e o botão secundário "Exportar…".
   - "Importar": "Importar só adiciona definições. Nada é executado nem concedido." e o botão secundário
     "Escolher arquivo…".
3. Abaixo, o passo de exportação aberto: "O que exportar" com quatro grupos de caixas de seleção:
   "Personalidades que você criou ou alterou" (Revisor de segurança ✓), "Agentes" (Arquiteto ✓,
   Desenvolvedor ✓, QA), "Templates de workflow salvos" (nenhum), "Workflows deste workspace"
   (Recuperação de senha ✓); caixa "Incluir o que eles precisam" marcada; botões "Cancelar" e "Continuar (4)"
   (principal).

NÃO CONTÉM: exportação para nuvem, link de compartilhamento, histórico de exportações.
```

### 7.4 Acesso pelo celular

```text
TELA: "Acesso pelo celular" (modal M, 640px, sobre o Workspace escurecido). Barra superior: chip âmbar
"1 agente aguardando você", "Celular" com bolinha "1", "PT | EN".

CONTÉM, no modal:
1. Título "Acesso pelo celular"; frase "Acompanhe execuções e responda o que espera por você pelo navegador do
   celular, na mesma rede. Nada passa pela internet."
2. Linha de estado: selo verde "Ligado" e botão secundário "Desligar".
3. Aviso âmbar: "1 pergunta espera uma resposta."
4. Bloco de pareamento em duas colunas: à esquerda o QR code; à direita "Escaneie com a câmera do celular.",
   o código "K7QD-M2XP-4RTA-9WCE" em mono grande, "Expira em 4:12", botão fantasma "Novo código", e o endereço
   "http://192.168.1.20:47831" em mono com seletor "Rede local" e botão "Copiar".
5. "Celulares pareados (1)": "iPhone de Lucas · 192.168.1.31 · visto há 2 min" com "Desconectar"; e
   "Desconectar todos" (vermelho, fantasma).
6. "O que saber" recolhido.
7. Rodapé: "Fechar".

NÃO CONTÉM: telas do celular, login, nuvem, notificações push, contas.
```

---

## Registro dos envios

| Data       | Tela                                    | id gerado                                                                         | Observações                                                                                                                                                                                                                                                                             |
| ---------- | --------------------------------------- | --------------------------------------------------------------------------------- | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| 2026-10-21 | 1.1 Workspace — Lista                   | `2641916f68264f6b8c239f8960043629`                                                | Fiel ao prompt; moldura correta, nada inventado. Selo "Pronto" em caixa com borda                                                                                                                                                                                                       |
| 2026-10-21 | 1.2 Workspace — Grade                   | `c8e0fa0a1cc74357b57039b61ef46eea`                                                | Fiel; célula livre virou convite. Desvios: os 3 agentes com modelo "sonnet"; selo "Pronto" sem caixa (difere da Lista); logo diferente da Lista                                                                                                                                         |
| 2026-10-21 | 1.3 Workspace — Agente executando       | `540a057470b34e21a391b9e889a28378`                                                | Fiel: chips na barra superior, linha do tempo e aviso de permissão com Aprovar/Rejeitar. Tela mais larga (2880px) com faixa vazia embaixo                                                                                                                                               |
| 2026-10-21 | 1.4 Workspace — Detalhes do agente      | `14efeaedf2604a1689f6f16ebd9401e5`                                                | Gaveta fiel. Inventou no fundo borrado: agentes "Dev Front-end" e "Auditor de Segurança", subtítulo do workspace, modelo "anthropic/claude-3-7-sonnet". Rótulos de consumo viraram "Volume" e "Sessões"                                                                                 |
| 2026-10-21 | 1.5 Workspace — Execuções do agente     | `271a0f44d0734610baaf51e4dfdf024a`                                                | Fiel; mestre-detalhe dentro do cartão. Grupo "HOJE" em caixa alta (contra a regra)                                                                                                                                                                                                      |
| 2026-10-21 | 1.6 Workspace — Boas-vindas             | `2c777bfb9adf4bcaa39f942775de7607`                                                | Fiel; nada inventado                                                                                                                                                                                                                                                                    |
| 2026-10-21 | Módulo 1 (geral)                        | —                                                                                 | Inconsistências entre telas: ícone do logo muda em cada tela; selo de estado alterna entre caixa mono e pílula. Pedir na base um logo e um selo únicos                                                                                                                                  |
| 2026-10-21 | 2.1 Projetos e Harness                  | `29e0429d7bb04e04a42edc2be126e79c`                                                | Fiel. Logo agora igual (regra nova funcionou). Botão saiu "+ + Adicionar pasta de projeto" (sinal duplicado); selos ainda retangulares, não pílula                                                                                                                                      |
| 2026-10-21 | 2.2 Harness — Analisar projeto          | `8d87e9da9c7e4148b9eec919d74a8df3`                                                | Fiel, passos e rodapé certos. Destacou "Workflow" na navegação (devia ser Workspace); placeholder inventado no campo ("Ex.: Este microsserviço…")                                                                                                                                       |
| 2026-10-21 | 2.3 Harness — Análise ao vivo           | `038a44d136864bc98fe70c91c7cf1718`                                                | Estrutura fiel, mas inventou rótulos técnicos: "4 operações", "OPERAÇÃO FS / AST", "LATÊNCIA", "[ATIVO]", "Stream stdout", "Raciocínio contínuo", "TOKEN BATCH #14", "Pipeline: AST Walk", "Conexão local ativa", "taxa de geração: 42 tokens/s". Navegação em "Workflow" (errado)      |
| 2026-10-21 | 2.4 Harness — Revisar contexto          | `ec5a1fd3d56d4ef7a0b2d5ee0e13898d`                                                | Fiel; painel de evidência preenchido. Navegação em "Workflow" com ponto (errado); rodapé "Não enviar o .atlas/ ao Git" não aparece no recorte; selos retangulares                                                                                                                       |
| 2026-10-21 | Módulo 2 (geral)                        | —                                                                                 | Base ganha: item da navegação explícito por tela, e painéis sem rótulos de coluna, rodapés de status ou contadores próprios                                                                                                                                                             |
| 2026-10-21 | 3.1 Workflow — Editar                   | `9fa72436d56f41b2804dbd0306dd4e23`                                                | Canvas bom (loop fail/pass legível, ferramentas e zoom nos cantos). ESTOUROU a largura: "Personalizar", a chave, "Executar", a coluna direita, Celular e PT/EN cortados. Inventou "Topologia de agentes"; "NOME DO WORKFLOW" em caixa alta; selos de estado dos servidores MCP faltando |
| 2026-10-21 | 3.2 Workflow — Etapa selecionada        | `f230e82933c64e568da20752675e1ad8`                                                | Inspetor fiel (seções recolhidas com resumo, rodapé "Alterações não salvas"). Canvas quebrado. Inventou "FOCO ATUAL", "03 · AGENTE", "fail: reexecutar" e barra de status "Motor orquestrador: ativo · Zoom: 100%"                                                                      |
| 2026-10-21 | 3.3 Workflow — Novo workflow            | `40f747ca08f54174a46c2b7605df9841`                                                | Fiel (passos, resumo da tarefa, mestre-detalhe, rodapé). Inventou "TEMPLATE ID: standard-impl"; títulos em caixa alta; "Aplicar ferramentas" fora do recorte                                                                                                                            |
| 2026-10-21 | 3.4 Workflow — Execução aguardando você | `8114ecd9439043e5ac0b6a3344a8d73c`                                                | Fiel no painel "Ação necessária". Inventou subtítulos nos nós ("Elaboração da especificação", "Plano de implementação") e selo "Foco atual". Botões Aprovar/Pedir ajustes fora do recorte (tela 2880px)                                                                                 |
| 2026-10-21 | 3.5 Workflow — Execução em andamento    | `9095a3fe4a854e8890680c17e5bcaca8`                                                | Fiel; desenhou o "⋯" aberto com "Cancelar execução". "ETAPAS DO WORKFLOW" em caixa alta; rótulo "decisão" no nó de condição                                                                                                                                                             |
| 2026-10-21 | 3.6 Workflow — Recuperar execução       | `7f100229e66d467da474c2e97d4b5e33`                                                | Fiel no painel. Inventou a barra de título do macOS (semáforos e título da janela) e subtítulos nos nós                                                                                                                                                                                 |
| 2026-10-21 | 3.7 Workflow — Revisão e entrega        | `7e04307ee63f450da02feca8040db21f`                                                | A mais fiel até agora: entrega em destaque, achado, linha do tempo, seções recolhidas                                                                                                                                                                                                   |
| 2026-10-21 | Módulo 3 (geral)                        | —                                                                                 | Base ganha: nada passa da largura de 1440px; sem barra de título do sistema; nós e cartões só com os textos descritos. Caixa alta em títulos de seção persiste mesmo proibida                                                                                                           |
| 2026-10-21 | 4.1 Agentes — Lista                     | `0b2b8d0e43cb4b0cbe79e2312637a10e`                                                | Muito fiel: tabela simples, selo e motivo "Sem login", Editar + ⋯. Selos ainda retangulares                                                                                                                                                                                             |
| 2026-10-21 | 4.2 Agentes — Criar agente              | `513fb3e1e800419b811e497e67c5eea3`                                                | Fiel: uma coluna, sugestão da personalidade logo abaixo da escolha, contrato em cartões, desfechos. Tela alta (3338px) com o rodapé fixo desenhado no meio; placeholder inventado em Instruções                                                                                         |
| 2026-10-21 | 4.3 Personalidades — Biblioteca         | `a02c550a093640ea8c61802c22d34f16`                                                | Fiel: cartões resumidos, Developer expandido, aviso numa linha. Tags com "#" inventado; "COMPORTAMENTO" e "INSTRUÇÕES DO SISTEMA" em caixa alta                                                                                                                                         |
| 2026-10-21 | 4.4 Personalidades — Editar             | `1fb35bff8a0a452c826b495473e695c5`                                                | Fiel (gaveta, aviso, campos, rodapé). Inventou o rótulo "JetBrains Mono" sobre Instruções e um subtítulo no fundo borrado                                                                                                                                                               |
| 2026-10-21 | Módulo 4 (geral)                        | —                                                                                 | O mais fiel até aqui: regras de largura, barra do sistema e textos extras funcionaram. Persistem: caixa alta em subtítulos e selos retangulares                                                                                                                                         |
| 2026-10-21 | 5.1 Specs — Lista                       | `80d3f54f775348ee9f61cee19908f677`                                                | Fiel (linhas, alertas, barras N/4). Inventou barra de status no rodapé ("Daemon local sincronizado com spec-tree v2", "4 especificações indexadas") e o agente "Arquiteto Backend"; tela desenhada com margem preta em volta                                                            |
| 2026-10-21 | 5.2 Specs — Documento em revisão        | `47df26de601d44458c101222e08956a2`                                                | Muito fiel: passos em cartões, documento com largura de leitura, lateral com validações/perguntas/consumo/histórico, barra "Sua decisão". Inventou "SEC-AUTH-091", selo "Bloqueante" e um subtítulo em caixa alta                                                                       |
| 2026-10-21 | 5.3 Specs — Revisão final               | `4e0ac8d54a7a4fed848026d7c8708532`                                                | Fiel nas 7 verificações, revisão e publicação. A seção "Implementar" virou "Pronto para execução" com uma frase inventada                                                                                                                                                               |
| 2026-10-21 | 5.4 Specs — Nova especificação          | `3f8de88a803042dbb8b8b853e5ac13a1`                                                | Fiel no modal. Inventou "ESC para fechar", cadeado no projeto, subtítulo do bloco de perguntas e placeholders; no fundo, botão "Importar specs" visível (devia estar no ⋯) e ids SPEC-001                                                                                               |
| 2026-10-21 | Módulo 5 (geral)                        | —                                                                                 | Bom nível. Barra de status no rodapé reaparece quando sobra espaço embaixo; atalhos de teclado ("ESC para fechar") também                                                                                                                                                               |
| 2026-10-21 | 6.1 MCP Hub — Servidores                | `24ccabd188534cf69b0400936fb2c4cc` e `cb12ef4342004a9f88c70ae82537ba72` (reenvio) | Saiu em dobro por causa do reenvio. `24cc…` é a mais fiel (só "+ +" duplicado no botão). `cb12…` inventou "Servidores ativos · 3 instâncias", "5 assinaturas stdio" e tags POST/GET/QUERY nas ferramentas                                                                               |
| 2026-10-21 | 6.2 MCP Hub — Adicionar servidor        | `d5c9859b071b425fa69c6733edad9254`                                                | Fiel: 3 cartões, todos "Risco alto", Figma indisponível. Inventou requisitos do Figma ("Conta Figma Professional / Org", "PAT"); "REQUISITOS LOCAIS" em caixa alta; fundo é a tela antiga do MCP Hub. Só uma das duas tentativas gerou tela                                             |
| 2026-10-21 | 6.3 MCP Hub — Sincronizar CLIs          | `19d536af39164d9782cf0c877b18ec09` e `d52fe7880d574a959ac9b9c6ab1e7618` (reenvio) | Em dobro. Tabela fiel nas duas, com linha de estado dos arquivos. Ambas inventaram "3 operações estruturais planejadas"; `19d5…` inventou também o fundo ("Gerencie daemons MCP…", "3 Servidores Ativos"). Preferir `d52f…`                                                             |
| 2026-10-21 | 7.1 Configurações — Geral               | `1ba2dfae89f24380b46d68865108ce72`                                                | Fiel, nada inventado. Demorou bem mais que as outras para aparecer                                                                                                                                                                                                                      |
| 2026-10-21 | 7.2 Configurações — Memória             | `1220e05404894d1f8dd58608d6e22dbf`                                                | Muito fiel: três controles segmentados, abas, notas propostas com Aprovar/Editar/Descartar                                                                                                                                                                                              |
| 2026-10-21 | 7.3 Configurações — Importar e exportar | `be601b5b22e247c9a9ad387a4876bdf8`                                                | Fiel, nada inventado                                                                                                                                                                                                                                                                    |
| 2026-10-21 | 7.4 Acesso pelo celular                 | `23533cbf7f3e4650bca6608fe9c461f8`                                                | Fiel. Inventou só "Servidor HTTP local ativo" ao lado do selo                                                                                                                                                                                                                           |
| 2026-10-21 | Depois da queda do Stitch               | —                                                                                 | As telas enviadas após a reconexão demoraram muito mais para aparecer em `list_screens` (dezenas de minutos), mas todas chegaram. O reenvio do MCP Hub criou duplicatas. Lição: esperar mais antes de concluir que se perdeu                                                            |
