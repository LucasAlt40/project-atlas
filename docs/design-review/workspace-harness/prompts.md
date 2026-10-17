# Prompts para o Stitch — Workspace + Harness

Ainda **não enviados**. Cada prompt é autocontido (o Stitch não vê as capturas). Todos usam o
design system `assets/7aa0774b734b4149b2d38a23136aadfd` e `deviceType: DESKTOP` (1440×900).
Ordem de envio: P0 → P1 → P2…; confira cada tela com `get_screen` antes da seguinte.

Quando a tela já existe no Stitch, usar `edit_screens` com o id; senão `generate_screen_from_text`.

## P0 — Regras do módulo (anexar no começo de todo prompt)

> App desktop "Atlas", workspace de agentes de IA para engenharia de software. Tema escuro único,
> denso, de ferramenta técnica. Superfícies em camadas de tom (#0b0d10, #101317, #15191e, #1c2128,
> #242c37) separadas por borda de 1px, sem sombras (só overlays têm sombra). Acento índigo
> #6366f1; estados: sucesso #10b981, aviso #f59e0b, erro #ef4444, executando #60a5fa. Geist para
> texto, JetBrains Mono para rótulos técnicos, números e caminhos. Raios 4/6/8px. Todo texto em
> português do Brasil. Moldura fixa: barra lateral de 216px (logo, seletor de workspace, itens
> Workspace, Workflow, Agentes, Personalidades, Specs, MCP Hub, Configurações) e barra superior de
> 48px (breadcrumb, botão "Celular", alternância PT/EN). Não invente recursos: use só o que está
> descrito.
>
> TIPO DE TRABALHO: é uma REPAGINAÇÃO, não um redesenho do zero. Mantenha a identidade atual
> (tema escuro, tokens, fontes, tom técnico) e não fuja dela. O objetivo é facilidade de acesso e
> de leitura: hoje o Atlas mostra informação demais ao mesmo tempo e isso dificulta a leitura e o
> entendimento. Toda a informação atual é importante e deve continuar acessível, então NÃO
> remova dados: SEPARE melhor. Use hierarquia clara (o essencial em destaque, o resto em
> segundo plano), agrupe o que é do mesmo assunto sob um título, divida o que é longo em seções,
> abas ou áreas recolhíveis, deixe respiro entre grupos, limite cada linha a uma ideia, e mostre
> detalhes secundários só quando o usuário pedir (expandir, passar o mouse, abrir). Ações
> frequentes ficam à vista; ações raras ou destrutivas ficam num menu e separadas das seguras.
> Prefira rótulos de texto a ícones sem legenda.
>
> NOMES DAS TELAS: dê a cada tela um nome curto, claro e em português que diga o que ela é e
> para que serve (por exemplo "Workspace — Lista de agentes", "Projetos e Harness",
> "Harness — Revisar achados"). Use o mesmo nome no título da tela no Stitch, no breadcrumb e
> nos botões que levam a ela. Estados diferentes da mesma tela levam o mesmo nome seguido do
> estado (por exemplo "Projetos e Harness — Sem análise").

### Nomes das telas (cada prompt começa com "Nome da tela: <nome>")

| Prompt | Nome da tela                           |
| ------ | -------------------------------------- |
| P1     | Workspace — Lista de agentes           |
| P2     | Workspace — Grade de agentes           |
| P3     | Agente — Detalhes e permissões         |
| P4     | Execução — Inspetor                    |
| P5     | Projetos e Harness                     |
| P6     | Harness — Analisar projeto             |
| P7     | Harness — Revisar achados              |
| P8a    | Boas-vindas — Criar primeiro workspace |
| P8b    | Seletor de workspace                   |

## P1 — Workspace: lista expansível (`edit_screens` em `ee4c1206…`)

> Redesenhe a tela principal do workspace "Transport ERP", visão "Lista expansível". Cabeçalho:
> título do workspace e, na mesma linha, um resumo de uso discreto ("Hoje US$ 1,24 · Semana
> US$ 3,72", mono); ações com hierarquia clara: botão primário "+ Adicionar agente", secundário
> "Projetos (2)" e um link de navegação "Integrações ↗" visualmente diferente de um botão (ele
> leva ao MCP Hub). Alternância "Lista expansível | Grade" e "Recolher todos". Cada agente é um
> cartão expansível: cabeçalho de UMA linha com nome, selo de estado (PRONTO, EXECUTANDO,
> AGUARDANDO RESPOSTA, FALHA) e, à direita, ações editar/remover/recolher; os metadados
> (personalidade, runtime, provedor, modelo, "Isolamento Git: ativo") vão numa segunda linha
> discreta em mono. Abas Chat · Atividade · Terminal · Detalhes · Execuções com aba ativa bem
> marcada (sublinhado de acento). Chat: balão do usuário à direita; resposta do agente com avatar,
> nome, hora, markdown (título, lista, bloco de código com "Copiar"). Compositor no pé do cartão:
> chip do projeto de escopo, campo "Responder ao Arquiteto Backend ou solicitar alteração", botão
> "Enviar". Mostre 3 agentes: um aberto com conversa, um recolhido pronto, um recolhido
> "AGUARDANDO RESPOSTA" com indicador âmbar. NÃO mude: as 5 abas, os dados mostrados, o idioma.

## P2 — Workspace: grade (`edit_screens` em `6eecbeea…`)

> Visão "Grade" do mesmo workspace, 2×2. Cartões compactos: cabeçalho de uma linha (nome + selo
> de estado + menu "⋯" com editar/remover), metadados do agente em linha secundária, abas
> Chat · Atividade · Terminal · Detalhes · Execuções. O compositor é compacto (campo de uma linha
> e botão de enviar em ícone). Três células com agentes (uma em execução com indicador azul, uma
> pronta, uma aguardando resposta com borda âmbar) e a quarta célula vazia como convite: ícone
> "+" e o texto "Adicionar agente", com borda tracejada, no lugar do quadro "Disponível". NÃO
> mude: a grade 2×2, as abas, o conteúdo do chat.

## P3 — Agente: Detalhes (`edit_screens` em `01d7d6b3…`)

> Painel lateral direito de 420px sobre a tela do workspace, com scrim de 40% e blur de 12px.
> Título: nome do agente, "Arquiteto · Claude CLI · Anthropic", modelo "sonnet", selo PRONTO.
> Organize em seções recolhíveis com cabeçalho legível (não em caixa alta minúscula): Permissões
> (seletor de perfil "Desenvolvedor"; quatro linhas compactas em vez de cartões aninhados: Sistema
> de arquivos "Somente a pasta do projeto · leitura + escrita", Processos "Comandos de
> desenvolvimento", Git "Leitura + escrita" com aviso âmbar "Ações destrutivas exigem aprovação",
> Rede "Restrita"), Contexto do projeto ("Harness: Carregado"), Última execução, Consumo (esta
> conversa / hoje / semana em tabela com custo e tokens em mono, e a nota "Modelos pelo Atlas:
> custo e tokens informados pelas execuções"), Dados do agente. Resumo fixo no topo com custo de
> hoje e perfil de permissão. NÃO mude: as informações nem o seletor de perfil.

## P4 — Execução: inspetor (`edit_screens` em `4c0c75ea…`)

> Inspetor da "Execução #e1 — Revisar a camada de faturamento" como painel largo (80% da largura)
> em vez de modal estreito. Cabeçalho com tarefa, selo "Concluída", duração 9m 00s, custo
> US$ 0,31. Abas: Detalhes · Contexto · Atividade · Chat · Terminal. Em Detalhes, grade de duas
> colunas de pares rótulo/valor (Status, Workspace, Agente, Personalidade, Runtime, Modelo, Início,
> Fim, Processo, Entrada 12 mil, Saída 3,1 mil, Total 15,1 mil, Custo) com valores em mono. Mostre
> também a linha do tempo de Atividade ao lado (iniciada, iniciando runtime, enviando prompt,
> aguardando modelo, ferramenta Read, concluída) com horários. NÃO mude: as 5 abas e os campos.

## P5 — Projetos do workspace e Harness (`edit_screens` em `499b4695…`)

> Substitua o modal "Projetos de Transport ERP" por uma página cheia do workspace (sem modal).
> Topo: título, "+ Adicionar pasta de projeto". Lista de projetos como linhas de tabela densa:
> nome + caminho em mono; selos num sistema único (Principal, papel Backend/Frontend, estado do
> Harness com cor semântica: Saudável verde, Desatualizado âmbar, Precisa revisão âmbar, Não
> analisado cinza); pilha "Angular · TypeScript · Docker"; contagens "42 achados · 31
> verificados · 9 inferidos · 2 seus · 0 desconhecidos" como mini-barra segmentada; "Última
> análise 14 out 2026, 18:05"; "Usa: transport-erp". Ações por linha: "Atualizar" e "Recarregar"
> como botões fantasma; "Editar"; e um menu "⋯" com "Resetar" e "Excluir" em vermelho,
> separados das seguras. Abaixo, três blocos: Harness (explicação + botão primário "Iniciar o
> Harness de 2 projeto(s)" ou "Resetar todos os Harnesses"), Contexto do workspace ("Atualizado" +
> "Refazer contexto"), Notas do workspace (textarea + "Salvar notas"). Mostre 2 projetos: um
> saudável, um não analisado. NÃO mude: as ações nem os textos explicativos.

## P6 — Harness: analisar projeto (`edit_screens` em `cf432e92…`)

> Em vez de modal sobre modal, tela de etapa "1 de 3 — Analisar projeto" dentro da página de
> Projetos, com indicador de passos (Analisar → Revisar → Inicializar). Projeto transport-erp
> /dev/transport-erp, selos "ATLAS ENGINE" e "Local Core", aviso com cadeado: "O Atlas lê apenas
> nomes de pastas e arquivos de manifesto. Nada é executado e nada sai da sua máquina."
> Duas opções lado a lado: "Análise com IA (recomendado)" com campo "O que o agente deve saber?
> (opcional)" + contador "~0 tokens" + seletor de agente executor "Arquiteto Backend · sonnet", e
> "Revisar sem IA". Abaixo, "Diagnóstico inicial detectado" com três colunas: Stack (TypeScript,
> Angular 21, Vitest, Docker), Git e testes (Git), Topologia (Em camadas, confiança média).
> Rodapé: "Cancelar" e o primário "Analisar com IA".

## P7 — Harness: revisar contexto (`edit_screens` em `54508e9f…`)

> Etapa "2 de 3 — Revisar contexto do projeto" em página cheia. Cabeçalho com `.atlas/`, nome do
> projeto, botões "Voltar" e "Inicializar" (primário). Quatro métricas: Total 5 itens, Verificados
> 4, Inferidos 1 (com "Confirmar"), Pendentes 0 conflitos. Abas: Stack, Testes e comandos,
> Arquitetura e módulos, Contexto de negócio, com contadores. Lista de achados em duas colunas
> quando há espaço; cada achado: checkbox de inclusão, categoria (LANGUAGE, FRAMEWORK,
> INFRASTRUCTURE), valor editável, arquivo de origem e selo de confiança (verificado/inferido),
> link "Evidência". O Inspetor de evidência fica recolhido e abre ao escolher um achado, mostrando
> arquivo, campo e método de verificação. Rodapé: "Estado do Harness: aguardando revisão" e a
> alternância "Não enviar o .atlas/ ao Git" com a explicação curta. NÃO mude: as abas, a
> inclusão por checkbox, a edição do valor.

## P8 — Sem workspace e seletor de workspace (`generate_screen_from_text`)

> Duas telas. (a) "Bem-vindo ao Atlas": estado vazio do workspace com a barra lateral mostrando
> "Sem workspace". Centro: ícone do Atlas, título, uma frase "Um workspace reúne um projeto, seus
> agentes e o contexto que eles compartilham", três passos numerados (1 Escolha a pasta do projeto,
> 2 Adicione agentes, 3 Inicialize o Harness) e o botão primário "Criar workspace". (b) Seletor
> de workspace aberto no topo da barra lateral: item atual "Transport ERP /dev/transport-erp" com
> marca de seleção, outro item "Billing API /dev/billing-api", separador, "+ Novo workspace" e
> "Gerenciar workspaces".

---

## Registro de envios

| Prompt | Enviado    | Tela gerada no Stitch                                                                    | Observação                                                                                                                          |
| ------ | ---------- | ---------------------------------------------------------------------------------------- | ----------------------------------------------------------------------------------------------------------------------------------- |
| P1     | 2026-10-16 | "Workspace — Lista de agentes" (`d1b03b16eb254cfe87d5bdd13109c61b`, 2560×2768)           | `edit_screens` criou uma tela nova; a original `ee4c1206…` continua intacta. A chamada deu timeout, mas a geração terminou.         |
| P2     | 2026-10-16 | "Workspace — Grade de agentes" (`aad74b5709b446468c817c8efefdf737`, 2560×2048)           | `edit_screens` criou tela nova; a original `6eecbeea…` continua intacta. Timeout na chamada, geração concluída.                     |
| P3     | 2026-10-16 | "Agente — Detalhes e permissões" (`967bb5d474564863823bfd2f31ddd137`, 2560×2048)         | `edit_screens` criou tela nova; a original `01d7d6b3…` continua intacta. Timeout na chamada, geração concluída.                     |
| P4     | 2026-10-16 | "Execução — Inspetor" (`5903404c41b6412c8cd0c79cab3ab2d6`, 2560×2048)                    | `edit_screens` criou tela nova; a original `4c0c75ea…` continua intacta. Timeout na chamada, geração concluída.                     |
| P5     | 2026-10-16 | "Projetos e Harness" (`299f698ba6a542aba7ac511ed4b29c64`, 2560×2048)                     | `edit_screens` criou tela nova; a original `499b4695…` continua intacta. Timeout na chamada, geração concluída.                     |
| P6     | 2026-10-16 | "Harness — Analisar projeto" (`68ed60d88752430d878d6d4648f5734c`, 2560×2186)             | `edit_screens` criou tela nova; a original `cf432e92…` continua intacta. Timeout na chamada, geração concluída.                     |
| P7     | 2026-10-16 | "Harness — Revisar achados" (`af24471b2e3f4ff4983290e1e5cba9f9`, 2560×2048)              | `edit_screens` criou tela nova; a original `54508e9f…` continua intacta. Timeout na chamada, geração concluída.                     |
| P8a    | 2026-10-16 | "Boas-vindas — Criar primeiro workspace" (`c65576984a6a4b6e849b57e0b758a70c`, 2560×2048) | `generate_screen_from_text` com o design system `assets/7aa0774b…`; tela nova, sem original. Timeout na chamada, geração concluída. |
| P8b    | 2026-10-16 | "Seletor de workspace" (`a1838a92b4de4eab99a02355f07136ce`, 2560×2048)                   | `generate_screen_from_text` com o design system `assets/7aa0774b…`; tela nova, sem original. Timeout na chamada, geração concluída. |

Módulo Workspace + Harness: 9 telas geradas no Stitch (P1 a P8b). As telas originais (`ee4c1206…`, `6eecbeea…`, `01d7d6b3…`, `4c0c75ea…`, `499b4695…`, `cf432e92…`, `54508e9f…`) continuam no projeto ao lado das novas.
