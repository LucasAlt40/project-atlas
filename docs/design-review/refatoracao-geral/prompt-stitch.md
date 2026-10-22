# Refatoração visual geral do Atlas — prompt para o Stitch

Prompt único para testar até onde o Stitch vai numa refatoração visual do app inteiro: design system,
moldura, todas as telas de desktop, padronização de termos e comportamento em larguras de 720 a 2560 px.

Diferente dos prompts por módulo (`../<módulo>/prompts.md`), este dá **liberdade de layout**: pode mudar
navegação, organização e componentes. O que não pode mudar é a função: nada novo, nada removido.

## Como usar

1. Envie o bloco **PROMPT COMPLETO** inteiro. Ele está dividido em partes (A a G) com títulos claros.
2. Se o Stitch recusar ou cortar por tamanho, envie em rodadas: **A + B + C + D** primeiro (base, design
   system e moldura); depois **A + E.<módulo> + F + G** para cada módulo. A parte A sempre vai junto.
3. O Stitch costuma inventar moldura e funções. O que ele inventar vai para a lista "O Stitch mostra, o app
   não tem" na avaliação, nunca para a implementação.

Fontes do inventário: os dossiês `../<módulo>/README.md`, o código da `main` em 2026-10-21
(`src/app/navigation.ts`, `src/styles/global.css`, `src/i18n/pt-BR.ts`) e a documentação em `docs/guia/`.

## PROMPT COMPLETO

```text
==================================================================
PARTE A — QUEM É O ATLAS E O QUE EU QUERO
==================================================================

Você vai fazer a REFATORAÇÃO VISUAL COMPLETA do "Atlas", um app DESKTOP (Tauri, janela nativa) que
orquestra agentes de IA em projetos de software. A pessoa cria agentes (personalidade + CLI de IA +
modelo), coloca-os num workspace (uma ou mais pastas de projeto), conversa com eles, monta workflows
(grafos de agentes) e decide o que entra no código. O Atlas não é um modelo: ele CONTROLA o processo.
Texto de agente é dado, nunca comando. Público: desenvolvedores; uso de horas seguidas.

Objetivo desta refatoração, em ordem de importância:
1. LEITURA FÁCIL: hoje o app mostra informação demais ao mesmo tempo, com letras pequenas e tudo com o
   mesmo peso. Quero hierarquia clara: o essencial em destaque, o secundário recolhido ou num segundo
   nível, um assunto por bloco.
2. ACESSO FÁCIL: achar cada função em poucos cliques, sem modal dentro de modal, sem rolar para achar o
   botão principal, com navegação previsível.
3. PADRONIZAÇÃO: um único design system aplicado em todas as telas. Mesmos componentes, mesmos nomes,
   mesmos estados, mesmas cores para o mesmo significado, mesmo lugar para o mesmo tipo de ação.
4. RESPONSIVO PARA DESKTOP: a janela vai de 720 a 2560 px de largura e de 600 a 1600 px de altura.
   Nada pode cortar, sobrepor ou exigir rolagem horizontal da página.

Você TEM liberdade para mudar layout, navegação interna das telas, agrupamentos, componentes, escala de
tipografia e espaçamento. Você NÃO tem liberdade para mudar o que o app faz: toda função, dado e estado
descritos na PARTE E continuam existindo e acessíveis, e nada que não esteja descrito pode aparecer.

Idioma: todo texto da interface em português do Brasil. Nomes próprios (Claude, Codex, OpenCode, Gemini,
Antigravity, GitHub Copilot, OpenSpec, MCP, Git) ficam como estão. Valores técnicos (caminhos, ids,
comandos, nomes de modelo, nomes de arquivo) em fonte mono, sem tradução.

==================================================================
PARTE B — DESIGN SYSTEM (gere uma tela "Design system" primeiro)
==================================================================

Mantenha a identidade: tema ESCURO ÚNICO (não existe tema claro), denso, de ferramenta técnica, sóbrio.
Pode refinar valores para ganhar contraste e legibilidade, mas mantenha a família de cores.

Cores (tokens):
- Superfícies em camadas de tom: void #0b0d10, base #101317, nível 1 #15191e, nível 2 #1c2128,
  nível 3 #242c37. Bordas: sutil #242c37, forte #2d3748, foco #3b4554.
- Texto: alto #f1f5f9, padrão #e2e8f0, secundário #94a3b8, apagado #64748b (só para texto de apoio
  grande; nunca para texto pequeno ou informação importante).
- Acento índigo #6366f1 (hover #4f46e5, suave #818cf8, fundo tingido índigo a 14%).
- Semânticas com significado FIXO: sucesso #10b981, aviso #f59e0b, erro/destrutivo #ef4444,
  executando #60a5fa, informação #38bdf8, política/permissão #ec4899, ocioso #64748b.
- Contraste mínimo WCAG AA (4,5:1 para texto normal, 3:1 para texto grande e ícones).

Tipografia:
- Geist para interface; JetBrains Mono para dados técnicos, números, caminhos, ids, código e terminal.
- Hoje o corpo é 13px e o texto pequeno 12px: está pequeno demais para leitura longa. Proponha uma
  escala legível, com corpo de 14px, texto de apoio de 13px, mínimo absoluto de 12px (só para selos e
  metadados mono), títulos de seção 16px, título de página 20-22px. Altura de linha 1,5 no corpo.
- Títulos de seção em caixa normal (não em CAIXA ALTA mono minúscula como hoje).
- Texto corrido (markdown, explicações, documentos) com largura máxima de leitura de ~72 caracteres.

Espaço, forma e profundidade:
- Grade de 4px (4, 8, 12, 16, 24, 32, 48). Respiro maior ENTRE grupos do que DENTRO deles.
- Raios: 4px controles pequenos, 6px contêineres e cartões, 8px modais e gavetas.
- Painéis separados por TOM e BORDA de 1px, sem sombra. Sombra só em elementos sobrepostos (menus,
  modais, gavetas). Overlays com fundo escurecido a 40% e desfoque de 12px.
- Nunca caixa dentro de caixa dentro de caixa: no máximo um nível de cartão dentro de uma superfície.

Componentes padronizados (desenhe cada um com seus estados: normal, hover, foco visível, ativo,
desabilitado, carregando, erro):
- Botões: primário (índigo, no máximo UM por área), secundário (contorno), fantasma (só texto) e
  destrutivo (vermelho). Tamanhos P (28px) e M (32px). Ícone sempre acompanhado de texto, exceto
  controles universais (fechar ×, recolher, ⋯), que precisam de dica ao passar o mouse.
- Menu "⋯" (overflow): ações raras ficam aqui; ações destrutivas no fim, separadas por linha e em
  vermelho.
- Confirmação: ações destrutivas pedem confirmação no próprio lugar ("Excluir X?" + Confirmar /
  Cancelar) ou num modal pequeno quando o efeito é grande.
- Selo (badge): UM único sistema. Pequeno, com ponto colorido + texto, cor pela semântica. Variantes:
  estado (com ponto), categoria (neutro, sem ponto), contagem (número). Sem caixa alta gritante.
- Abas, controle segmentado (2-4 opções), filtros com contagem "Todas (12)".
- Campos: texto, área de texto com contador de caracteres quando houver limite, número, seletor,
  seletor com busca, chave liga/desliga, caixa de seleção, rádio em cartões, campo de busca.
  Rótulo sempre visível acima do campo; ajuda curta abaixo; erro em vermelho abaixo do campo.
- Cartão, linha de lista (com ações à direita), tabela simples, lista de pares rótulo/valor (rótulo e
  valor próximos, não nas pontas de uma linha larga), linha de métricas compacta (números mono).
- Seção recolhível com título e contagem. Passos numerados (stepper). Barra de progresso.
- Avisos (callouts): informação, aviso, erro, política; com título curto, uma frase e no máximo uma ação.
  Avisos longos e permanentes podem ser recolhidos depois de lidos.
- Estados de tela padronizados: VAZIO (o que é, por que está vazio, UMA ação para começar), CARREGANDO
  (esqueleto com o formato do conteúdo, não frase solta), ERRO (o que falhou, o detalhe técnico recolhido,
  "Tentar de novo"), SEM WORKSPACE (frase + "Criar workspace" / "Abrir workspace").
- Superfícies: modal P (420px), M (640px), G (960px); gaveta lateral à direita (420-560px);
  página de detalhe com "← Voltar". REGRA: nunca mais de um modal aberto ao mesmo tempo. Fluxos com
  várias etapas viram uma página própria com passos, ou uma gaveta, nunca modal sobre modal.
- Blocos técnicos: bloco de código com "Copiar", visualizador de diff (lista de arquivos com A/M/D e
  +adições −remoções; diff por arquivo), terminal (fundo void, mono), markdown renderizado (títulos,
  listas, tabelas, código), linha do tempo de eventos com hora.
- Canvas de grafo (workflow): nós de agente, condição e fim; arestas com rótulo legível; minimapa que
  não cobre nós; controles de zoom.

==================================================================
PARTE C — PADRONIZAÇÃO DE TERMOS E ESTADOS
==================================================================

Use SEMPRE estes termos, em todas as telas (hoje o app mistura inglês e português):
- "Agente" (nunca "agent"); "Etapa" (nunca "step" ou "passo" para o nó do workflow); "Execução" (uma
  rodada de um agente ou de um workflow); "Workflow"; "Especificação" (curto: "Spec"); "Personalidade";
  "Runtime" (a CLI de IA); "Modelo"; "Workspace"; "Projeto" (uma pasta); "Harness" (o contexto do
  projeto gravado em .atlas/); "Achado" (finding); "Passagem" para o handoff entre etapas, com
  "(handoff)" na primeira menção; "Contrato de resultado"; "Desfecho" (outcome: pass, fail…);
  "Concessão" (grant de ferramenta MCP); "Servidor MCP"; "Memória"/"Nota".

Um único vocabulário de estado, cada um com uma cor fixa, igual em toda tela (selo, nó do grafo,
lista, cabeçalho):
- Pronto (sucesso) · Executando (executando, azul) · Aguardando você (aviso, âmbar) · Pausado (ocioso)
  · Interrompido — pode retomar (aviso) · Concluído (sucesso) · Falhou (erro) · Cancelado (ocioso)
  · Pendente / Não iniciado (ocioso) · Indisponível (aviso) · Desligado (ocioso) · Desatualizado (aviso)
  · Bloqueado (erro).
O estado de algo aparece num lugar principal (o cabeçalho do objeto) e, nos outros, só como ponto
colorido ou selo pequeno, nunca repetido em cinco formatos.

Padrões de ação, iguais em todas as telas:
- Ação principal da página: canto superior direito do cabeçalho da página. Uma só.
- Ação principal de formulário: rodapé fixo do formulário, à direita ("Cancelar" à esquerda dela).
- Ações de item (cartão/linha): à direita do item; frequentes visíveis, raras no "⋯".
- Importar/Exportar: sempre no "⋯" do cabeçalho da página, com o mesmo rótulo em todo lugar.
- Cada ação aparece UMA vez por tela.

==================================================================
PARTE D — MOLDURA DO APP (shell)
==================================================================

Barra lateral à esquerda (216px aberta; trilho de 56px só com ícones quando recolhida, com o nome
aparecendo ao passar o mouse):
- Topo: logo "Atlas" (só o logo e o nome, sem subtítulo nem versão).
- Seletor de workspace: nome e caminho da pasta; abre um menu com a lista de workspaces (nome +
  caminho), "+ Novo workspace" e "Gerenciar workspaces".
- Navegação, nesta ordem: Workspace, Workflow, Agentes, Personalidades, Specs, MCP Hub,
  Configurações. Item atual destacado.
- Rodapé: "Project Atlas", versão e plataforma (ex.: "v0.2.0 · macOS").

Barra superior (48px):
- Botão de recolher/abrir a barra lateral; breadcrumb "<workspace> / <tela>" (e "/ <item>" em telas de
  detalhe).
- À direita, nesta ordem: indicador "N agentes aguardando você" (âmbar; só aparece quando há; abre uma
  lista do que espera, cada item leva à execução); indicador "N agentes em execução" (só quando há; abre
  a lista: agente e workspace, cada item leva ao agente); botão "Celular" (com bolinha âmbar e contagem
  quando há pergunta esperando); alternância PT | EN.

NÃO DESENHE na moldura (não existe no app): busca global ou ⌘K global, central de notificações, sino,
avatar, conta, e-mail, login, botão "Atlas CLI", ícone de terminal na barra, "Sync OK", status de nuvem,
telemetria (cluster, latência, RAM, região, CPU), barra de status inferior, botão "Novo fluxo" na
barra, número de versão inventado, tema claro, ajuda/documentação.

==================================================================
PARTE E — INVENTÁRIO COMPLETO DE TELAS E FUNÇÕES (não remova nada daqui)
==================================================================

------------------------------------------------------------------
E.1 WORKSPACE (tela inicial)
------------------------------------------------------------------
Cabeçalho do workspace: nome; consumo "Hoje US$ X · Semana US$ Y" (mono); "Projetos (N)" (abre a
gestão de projetos e o Harness); "Integrações" (leva ao MCP Hub); ação principal "+ Adicionar agente".
Alternância de visão: "Lista expansível" | "Grade"; "Recolher todos" na lista.
Grade: até 4 agentes (2×2). Célula livre convida a adicionar um agente (hoje é um quadro morto
"Disponível").

Cartão do agente: nome; estado (Pronto, Executando, Aguardando você/aprovação, Falhou, recebendo
resposta); personalidade · runtime · provedor; modelo; isolamento Git (ligado/desligado); editar;
remover do workspace (não apaga o agente); recolher. Abas:
- Chat: mensagens da pessoa e do agente (markdown com títulos, listas, código com "Copiar"; nome e
  hora); pedidos de aprovação de comando no meio da conversa (Aprovar / Rejeitar; expira em 5 min);
  compositor com escopo (quais projetos do workspace o agente vê), campo de mensagem e "Enviar".
  Hoje cada cartão repete o compositor inteiro: proponha algo mais leve na grade.
  Vazio: "Nenhuma mensagem ainda. Pergunte algo a este agente."
- Atividade: linha do tempo do que o agente está fazendo (iniciando runtime, enviando prompt,
  ferramenta iniciada/concluída, concluído, falhou), com hora.
- Terminal: terminal ao vivo da execução; Interromper (Ctrl+C) e Encerrar.
- Detalhes (hoje uma gaveta lateral longa): Permissões (perfil "Somente leitura" ou
  "Desenvolvedor"; quatro áreas: Sistema de arquivos, Processos, Git, Rede; aviso para ações
  destrutivas); Contexto do projeto (estado do Harness; "é só contexto, não permissão");
  Regras; Memória do agente; Ferramentas MCP concedidas; Revisão de contexto (o que entrou no prompt e
  por quê, orçamento de tokens estimado); Última execução; Consumo (esta conversa / hoje / semana:
  custo, tokens, nº de execuções; "valores informados pelo runtime"; null nunca vira 0); Cota;
  Git do worktree; dados do agente (runtime, modelo, estado, isolamento). Organize em abas ou
  seções recolhíveis com o mais consultado primeiro.
- Execuções: lista agrupada por dia (estado, tarefa, runtime · modelo, hora, duração). Abre o
  Inspetor de execução: Detalhes (tarefa, estado, workspace, agente, personalidade, runtime, modelo,
  início, fim, duração, processo, tokens de entrada/saída/total, custo), Contexto (prompt exato
  enviado), Atividade, Chat, Terminal. Hoje é um modal que impede comparar; proponha painel de detalhe.

Adicionar agente: escolher entre agentes já criados que ainda não estão neste workspace, ou "Criar
novo agente" (leva ao formulário de Agentes). Editar agente: o mesmo formulário de E.3.

Sem workspace (primeira abertura): "Bem-vindo ao Atlas", o que é um workspace e os próximos passos
(escolher a pasta do projeto, criar agentes, analisar o projeto com o Harness) + "Criar workspace".
Formulário de workspace: nome e pasta do projeto (seletor nativo). Gerenciar workspaces: lista com
editar e excluir (não enquanto um agente trabalha nele).

Projetos do workspace (hoje modal; abre três níveis de modal, isso precisa acabar):
- Lista de projetos: nome; selos Principal, papel (Backend/Frontend…), estado do Harness (Analisado,
  Não analisado, Desatualizado, Precisa revisão); caminho; saúde do Harness; tecnologias detectadas;
  contagem de achados (total · verificados · inferidos · seus · desconhecidos); data da última análise;
  dependências ("Usa: X"). Ações: Atualizar, Recarregar, Editar, e no "⋯": Resetar, Excluir
  (destrutivas). Projeto sem Harness: "Inicializar projeto".
- "+ Adicionar pasta de projeto"; "Iniciar o Harness de N projeto(s)" / "Resetar todos os Harnesses".
- Contexto do workspace: estado (Atualizado / Ainda não foi montado) + "Refazer contexto".
- Notas do workspace: área de texto + "Salvar notas".

Harness (fluxo de análise de um projeto; vire uma página com passos, não modais empilhados):
1. Analisar projeto: projeto e caminho; aviso de privacidade (só nomes de pastas e manifestos são
   lidos, nada é executado, .env nunca é lido); "Análise com IA (recomendado)" com campo opcional "O que
   o agente deve saber?" (contador de tokens) e seletor do agente que analisa; diagnóstico inicial
   detectado em três grupos (Stack/runtime, Git e testes, Topologia). Ações: Cancelar, "Revisar sem IA",
   "Analisar com IA".
2. Análise ao vivo: progresso por etapas, saída do agente, ferramentas usadas com tempo, Interromper.
3. Revisar contexto: métricas (Total, Verificados, Inferidos, Pendentes); abas por área (Stack, Testes e
   comandos, Arquitetura e módulos, Contexto de negócio) com contagem; achados agrupados por categoria,
   cada um com incluir/excluir, valor editável, origem (fato / inferência / seu), confiança, arquivo de
   evidência e "Evidência" (abre o inspetor de evidência: hoje metade da tela fica vazia até clicar);
   conflitos entre achados; aviso de achados desatualizados; opção "Não enviar o .atlas/ ao Git".
   Ações: Voltar, "Inicializar"/"Gravar".
Atualizar Harness: compara o projeto com a última análise, mostra o diff do que muda e só grava após
confirmação. Contexto de tarefa: por que cada item entrou ou ficou de fora do contexto de uma tarefa
(pontuação, orçamento de caracteres).

------------------------------------------------------------------
E.2 WORKFLOW (a maior tela; hoje mistura editar, acompanhar e revisar numa coluna só)
------------------------------------------------------------------
Proponha três modos claros da mesma tela: EDITAR, ACOMPANHAR EXECUÇÃO, REVISAR RESULTADO, com o canvas
como figura principal (hoje fica com 150-280px de altura espremido entre faixas de controles).

Editar:
- Seletor de workflow; "+ Novo workflow"; selo do modo (Automático/Personalizado) e versão.
- Ferramentas do canvas: + Agente, + Condição, + Fim, Conectar, Remover; Desfazer, Refazer,
  Redefinir layout, Ajustar à tela, zoom (− 76% +); contagem "5 etapas · 5 rotas".
- Ações do workflow: Personalizar (passa de automático a personalizado), Salvar, e no "⋯": Salvar
  como template, Exportar, Importar, Excluir workflow.
- Iniciar: "Como você quer começar": "Vibe coding" (a tarefa escrita é toda a instrução) ou "A partir
  de uma spec aprovada" (desabilitado sem specs prontas; "Escrever uma nova spec" leva a Specs). Campo
  "O que os agentes devem fazer?", chave "Aprovar planos automaticamente", escopo (projetos), "Executar".
- Visão geral (lateral): nome editável, modo, versão, nº de etapas; Ferramentas (MCP) com estado e a
  quem estão concedidas; "Gerenciar no MCP Hub".
- Inspetor da etapa (ao selecionar um nó): rótulo; agente; personalidade/runtime/modelo/isolamento/
  contexto (fatos, não editáveis aqui); contrato de resultado e desfechos; instruções e critério de
  conclusão; permissões; resiliência (tentativas, ao falhar: parar ou rotear, loop máximo);
  ferramentas MCP da etapa (todas ou só leitura; Conceder, Revogar); dependências e rotas; validação
  ("Etapa válida" ou o problema); "Salvo" / "Alterações não salvas" + "Salvar configuração".
  Aresta selecionada: condição "quando" (campo: result.status/outcome/summary/next_action/findings/
  matched, validation.status; operador: igual, diferente, existe, não existe; valor) e rótulo.
- Workflow inválido: nós marcados + "Reparar rotas" (diálogo: por agente, contrato atual × sugerido,
  escolher rota, nota, Aplicar).

Novo workflow: "Importar"; como começar (vibe/spec); como montar: "Automático" (o Atlas propõe um
workflow pela tarefa) ou "Personalizado" (de um template ou em branco); campo Tarefa; "Sugerido para a
sua tarefa: <template>"; cartões de template (Implementar [Recomendado], Mudança rápida, Depurar e
corrigir, Revisar e auditar…; descrição; "N etapas · N ferramentas sugeridas · Somente leitura");
detalhe do template (etapas com papel, loop, instruções e "feito quando"; saídas; ferramentas sugeridas
com "pronta"/"faltando"; "Aplicar ferramentas"; excluir os seus templates); nome opcional;
"Criar workflow". Hoje é uma página longa demais antes do botão: proponha etapas.

Acompanhar execução: seletor de execução (data/hora · estado); Voltar ao editor; Pausar/Retomar;
Cancelar; selo de estado (Executando, Aguardando você, Concluído, Falhou, Pausado, Interrompido —
pode retomar, Cancelado); chave "Aprovar planos automaticamente" desta execução. Canvas com o estado
de cada nó (Concluído, Executando, Pendente, "etapa N de M"; até 4 etapas em paralelo). Painel com:
- Visão geral: progresso "1/4 etapas concluídas · 25%"; etapas com estado e duração; etapa atual;
  agentes ativos; contadores (Executando, Pendentes, Falharam, Bloqueados); ferramentas MCP; a tarefa.
- Passagens (handoffs): por passagem: de qual etapa para qual, quando, resultado; resumo, decisões,
  artefatos, arquivos alterados (medidos pelo Git), validação, falha. Hoje é texto técnico sem hierarquia.
- Alterações: arquivos (A/M/D, caminho, +/−) e "Revisar diff".
- Workspace ao vivo: arquivos e código sendo escritos durante a execução (Git, não o que o agente diz).
- Planos e estado compartilhado da execução.

Ação necessária (o momento mais importante do app; destaque âmbar, sem esconder o canvas):
"<agente> está aguardando sua resposta"; motivo (Esclarecimento, Plano para aprovar, Aprovação,
Permissão); a pergunta; documento/plano completo (markdown, recolhível); contexto (recolhido); resposta
conforme o tipo: plano → Aprovar / Pedir ajustes / Refazer; pergunta → campo + "Enviar resposta";
aprovação → Aprovar / Rejeitar; permissão → Permitir / Negar; "Cancelar execução"; e a frase "Sua
resposta nunca aplica mudanças ao projeto: isso é uma revisão separada."

Recuperar (execução interrompida ao fechar o app): "Esta execução pode ser retomada"; último passo
concluído; onde parou; de onde continua; o que repete; o que fica sem refazer; "Retomar de onde parou";
"Retomada N vez(es)"; ou "Esta execução ainda não pode ser retomada". Cancelar continua disponível.

Revisar resultado: estado final (Concluído / Falhou / Cancelado) e linha de números ("3 etapas · 3
agentes · 2 arquivos +113 −4 · 18m 0s · sem achados").
- ENTREGA DO CÓDIGO (a decisão mais séria do fluxo; deve se destacar): o código está num worktree
  isolado, ainda não está no projeto; "Concluir o workflow não significa que o código está no seu
  projeto: você decide o que entra. Aplicar só altera seus arquivos; nunca faz commit nem push."
  Ações: "Aplicar alterações" (principal), "Revisar diff", "Abrir na IDE", "Manter isolado",
  "Descartar" (destrutiva, com confirmação). Estados: Pronto para revisão, Aplicado (sem commit),
  Mantido isolado, Descartado, Conflitos (nada é escrito), Desfazer.
- Seções com prioridade (não seis acordeões iguais): Linha do tempo dos agentes (por etapa: agente,
  estado, duração, tentativas, "Abrir execução"); Achados (erro/aviso/info, encontrado por,
  recomendação, validação posterior passou / sem confirmação); Passagens; Decisões; Artefatos;
  Conjunto de alterações ("Revisar o diff de todos os arquivos").

Diálogos: Salvar como template (nome, descrição, escopo); Excluir workflow / template (confirmação);
Importar/Exportar (ver E.6).

------------------------------------------------------------------
E.3 AGENTES
------------------------------------------------------------------
Lista: título com total; busca "Buscar agente ou modelo"; filtros Todos / Prontos / Indisponíveis
(com contagem); ação principal "+ Criar agente"; Importar no "⋯". Cada agente: nome, personalidade,
runtime, disponibilidade (Pronto / Indisponível: não instalado, sem login), modelo, perfil de permissão,
contrato de resultado, isolamento Git; Editar; no "⋯": Exportar, Excluir (confirmação; não exclui
enquanto trabalha). Hoje há uma faixa de 4 métricas decorativas e ações só com ícone: resolva.
Vazio: "Nenhum agente ainda" + Criar agente. Sem resultado de busca. Aviso pós-criação ("<nome> foi
salvo. Crie um workspace para usá-lo." / "salvo, mas não foi adicionado ao workspace: <motivo>").

Criar/editar agente (página com rodapé fixo Cancelar / Criar agente ou Salvar agente):
- Identidade: Nome; Personalidade (com a descrição dela). Escolher a personalidade SUGERE contrato e
  perfil: mostre esse efeito perto da escolha.
- Runtime e modelo: Runtime (Claude CLI, OpenCode, Codex, Gemini CLI, Antigravity, GitHub Copilot CLI)
  com estado da conexão ("Pronto · versão", Não instalado, Sem login); Modelo (seletor quando o runtime
  lista modelos, campo livre "ID do modelo" com dica quando não lista); "Detectar runtimes novamente";
  avisos do runtime ("Este runtime não suporta edição de arquivos").
- Instruções adicionais (área de texto).
- Contrato de resultado: Geral, Validação (pass/fail), Revisão (approved/changes_requested),
  Implementação (implemented/partial/blocked), Personalizado (editor de desfechos: id, rótulo,
  descrição).
- Permissões e isolamento: perfil Somente leitura (padrão) / Desenvolvedor, com o que cada um permite;
  "Sugerido pela personalidade"; "Sempre trabalhar em um Git worktree isolado".
- Estados: detectando runtimes, falha ao detectar, erro de validação, salvando.

------------------------------------------------------------------
E.4 PERSONALIDADES
------------------------------------------------------------------
Biblioteca: contagens (disponíveis, customizadas, nativas); ação principal "+ Nova personalidade";
no "⋯": Importar, "Restaurar personalidades padrão". Aviso "Protocolo do Atlas incluído em toda
personalidade" (narração ao vivo etc.), recolhível depois de lido. Filtros Todas / Padrão do sistema /
Customizadas (com contagem) e busca por nome, tag ou instrução. Nativas: Architect, Spec Architect,
Developer, QA, Architecture & Code Validator, Bug Fixer.
Cada personalidade: nome, selo Padrão/Personalizada, descrição, comportamento (pontos), instruções do
sistema (bloco mono), tags; "Usar esta personalidade" (abre Criar agente com ela), Editar, e no "⋯":
Exportar, Excluir. Hoje todos os cartões abrem tudo: mostre resumo e expanda sob demanda.
Nova/editar: Nome, Descrição, Instruções, Tags; Cancelar / Salvar personalidade. Nativa editada vira
"alterada" e pode ser restaurada. Hoje o painel espreme a grade em 3 colunas: proponha gaveta ou página.

------------------------------------------------------------------
E.5 SPECS (especificação antes do workflow, formato OpenSpec)
------------------------------------------------------------------
Lista: título e explicação curta (SDD); contadores (em andamento, aguardando você, prontas para
implementar); ação principal "Nova especificação"; "Importar specs" no "⋯"; busca por título, id ou
pedido; filtros Todas / Em andamento / Aguardando você / Bloqueadas / Prontas / Arquivadas (contagem).
Cada spec: id (mono), estado (Rascunho, Agente escrevendo, Aguardando sua revisão, Alterações
solicitadas, Pronta para implementação, Bloqueada, Publicada, Arquivada, Falha na execução), título,
pedido em uma linha, "N/4 aprovados", quem escreve, etapa atual, alertas ("N perguntas para você", "N
achados em aberto"), "Implementada em N execuções", tempo relativo.
Vazios: sem workspace, nenhuma spec, nenhuma no filtro.

Detalhe: "← Todas as specs"; título; estado; "Agentes e ferramentas" (quem escreve, quem revisa,
ferramentas MCP para testar e revisar); Excluir no "⋯" (confirmação); metadados (id, formato, projeto).
- Passos: Proposta, Requisitos, Design, Tarefas, Revisão final, cada um com fase (Aprovado, Aguardando
  revisão, Pronto para escrever, Aguardando documentos anteriores, Escrevendo, Desatualizado, Falhou).
- Documento: arquivo, revisão (seletor), "escrito pelo agente há X", Renderizado | Markdown, arquivos;
  validações automáticas (erros, avisos, sugestões; "Rodar validações novamente"); histórico de revisões
  (gerada, revisada, regerada, importada); consumo (tokens de entrada/saída, limite, "Continuar (+N)").
- Barra "Sua decisão" fixa embaixo, com as ações da fase: Escrever este documento, Cancelar execução,
  Escrever de novo, Solicitar alterações ("O que deve mudar?"), Aprovar e continuar; "nada roda até
  você pedir".
- Perguntas do agente: responder uma a uma ou todas ("Enviar N respostas e revisar").
- Revisão final: sete verificações com "Abrir" (quatro documentos aprovados, estrutura validada,
  documentos coerentes, nenhuma pergunta em aberto, revisada pelo agente revisor, achados decididos, sua
  aprovação final); "Aprovar a especificação"; revisão pelo agente revisor (Rodar a revisão, Seguir sem a
  revisão com motivo; achados com categoria — Ambiguidade, Contradição, Risco, Dependência, Cobertura,
  Decisão, Outro —, severidade Baixa/Média/Alta, estado Em aberto/Resolvido/Descartado; Ver no documento,
  Marcar como resolvido, Não é problema, Aceitar o risco, Reabrir); Publicar no projeto (pasta, Publicar,
  Publicar de novo, Arquivar; Publicada/Falhou/Desatualizada; arquivos gravados); "Iniciar um workflow a
  partir desta spec"; OpenSpec CLI opcional (versão, segunda opinião, Arquivar com a CLI); painel de
  tarefas (por grupo, requisitos, dependências, pendente/reportada/confirmada).
Nova especificação: projeto; "O que você quer construir?"; perguntas de contexto que surgem conforme o
texto (dados, integrações, quem pode fazer, telas, dados pessoais, volume, regras; cada uma com
"porque…"); "Mais alguma coisa"; agente que escreve; agente que revisa; "Criar apenas guarda o pedido.
Nenhum agente roda até você iniciar."; Cancelar / Criar.
Importar specs: mudanças OpenSpec encontradas em openspec/changes (só lidas), projeto, agente que escreve,
problemas por item, "Nenhuma mudança encontrada", Importar.

------------------------------------------------------------------
E.6 MCP HUB (servidores de ferramentas para os agentes)
------------------------------------------------------------------
Lista: título e uma frase (o que uma ferramenta devolve nunca é instrução); ação principal "+ Adicionar
servidor MCP"; "Verificar conexões" (examina um por um: "Verificando 2/3…"); no "⋯": Sincronizar CLIs,
Importar/Exportar atlas-mcp.json. Métricas numa linha (Servidores, Conectados, Falha/login, Desligados,
Ferramentas, Concessões). Filtros: Todos / Locais (stdio) / Remotos (HTTP/SSE — desabilitado: ainda não
suportado); busca (com atalho ⌘K, que existe só aqui); "Apenas conectados".
Lista + detalhe lado a lado. Servidor: nome; stdio; obrigatória/opcional; estado (Conectado, Falhou,
Desligado, Não examinado, Pendente, Desconhecido); liga/desliga (ligar pede confirmação); comando (mono);
nº de ferramentas; agentes com acesso; "Examinado em … via <runtime>"; avisos (agente cujo runtime não
recebe MCP; falha do último exame; "Segredo faltando: X"); Inspecionar; Testar.
Inspetor: Ferramentas (examinar com um runtime escolhido; ferramentas listadas; quem pode usar:
concessões com Revogar; Conceder a um agente com todas ou só algumas ferramentas, por workspace,
workflow ou etapa); Variáveis e segredos (guardado/não guardado no cofre do sistema; Guardar; Esquecer);
Manifesto JSON (Editar/Salvar: salvar desliga o servidor); Atividade (último estado, via, quando,
ferramentas; regras de proteção do Atlas). No "⋯": Exportar, Remover (com concessões e segredos).
Adicionar servidor: Registro MCP (busca, resultados, "só remoto"/"não roda aqui", "Carregar mais",
"Adicionar também ao arquivo da CLI"); Conhecidos do Atlas (Chrome DevTools, Figma, Jira…: risco,
descrição, o que executaria, versão fixada, requisitos encontrados/faltando, "Adicionar (fica
desligada)", Fonte); Manual (nome, programa, argumentos, variáveis com valor ou segredo,
obrigatória/opcional). Todo servidor novo entra DESLIGADO.
Importar/Exportar: formato (Detectar, mcpServers genérico, Claude Code, Gemini CLI, OpenCode, Codex,
GitHub Copilot CLI, Antigravity); ler o arquivo da CLI, carregar arquivo, colar; "Importar (desligados)";
resultado (adicionados, deixados de fora com motivo, segredos guardados). Exportar: formato, só o
selecionado, prévia, Copiar, Salvar como arquivo, Adicionar ao arquivo da CLI (com cópia de segurança).
Sincronizar CLIs: tabela servidor × CLI (tem / será adicionado / será trazido / indisponível / sem
arquivo), estado de cada arquivo, aviso de segredos, "Sincronizar: adicionar N a M arquivos, trazer K",
"Tudo está em dia".
Vazios: sem workspace; nenhum servidor (+ Adicionar, Sincronizar CLIs); nada na busca.

------------------------------------------------------------------
E.7 CONFIGURAÇÕES (seções: Geral · Memória · Importar e exportar · Sobre e atualizações)
------------------------------------------------------------------
Geral: Idioma (Português (Brasil) / English; "a saída dos modelos e o seu conteúdo não são
traduzidos"); Limite de tokens por especificação (vazio = sem limite; Salvar); Sandbox dos runtimes
(só macOS: Desligado [padrão] / Ligado quando o sistema puder / Obrigatório; "Este sistema consegue
confinar processos" ou o motivo; "Deixar escrever nos caches de ferramentas (npm, cargo, pip…)"; o que
o sandbox protege e o que não); Presença no Discord (desligado por padrão; "Mostrar o que o Atlas está
fazendo no Discord"; "Mostrar nomes do workspace, do workflow e da etapa"; nunca envia prompts nem
arquivos).
Memória (notas de trabalhos anteriores; dicas, nunca instruções nem permissões): onde são usadas —
Minhas (todos os projetos), Workspace, Agente — cada uma Desligado / Propor, eu aprovo / Automático
(Minhas sem Automático); máximo de tokens por prompt; Esperando sua decisão (notas propostas: título,
tipo · escopo · dono, texto, "Não confirmada", "Aguardando validação", usada N×; Aprovar, Editar,
Descartar); Notas guardadas (Editar, Arquivar, Apagar; "Apagar as N notas automáticas"); Adicionar nota
(escopo, tipo: Como fazer / Armadilha / Preferência / Decisão, título, texto até 480 caracteres).
Importar e exportar (agentes, personalidades, templates e workflows, sem segredos, conversas, logs nem
caminhos): Exportar — escolher o que (quatro grupos), "Incluir o que ele precisa", o que o arquivo vai
conter, citados mas não incluídos, avisos, Salvar arquivo. Importar — escolher arquivo; "importar só
adiciona definições; nada é executado nem concedido"; revisão item a item (fatos, "Precisa de",
conflito: igual / diferente; decisão: Importar como novo / Substituir / Não importar, com confirmação
para sobrescrever; runtime e modelo a escolher; permissões) e resultado final.
Sobre e atualizações: nome, versão, plataforma; Buscar atualizações → "Você está na versão mais
recente" / "A versão X está disponível" + Instalar e reiniciar.

------------------------------------------------------------------
E.8 ACESSO PELO CELULAR (só o modal no computador; o site do celular está fora deste trabalho)
------------------------------------------------------------------
Aberto pelo botão "Celular" da barra superior. Explicação (pela rede local, nada passa pela
internet); estado Desligado + Ligar / Iniciando… / Ligado + Desligar / erro com motivo; "N perguntas
esperam resposta"; QR code + código de pareamento (mono, vale uma vez, expira em 5 min, contagem
regressiva, "Novo código"); endereço local com seletor quando há mais de uma rede (rede local, VPN) e
Copiar; celulares pareados (nome, IP, visto por último, Desconectar; Desconectar todos); "O que saber"
(recolhível: só funciona com o computador ligado e acordado, HTTP em rede local, o que o celular pode e
não pode fazer).

------------------------------------------------------------------
E.9 COMPORTAMENTOS GLOBAIS
------------------------------------------------------------------
- Agentes continuam rodando quando se troca de tela ou de workspace.
- Fechar o app com agentes rodando pergunta: "Cancelar e sair" ou "Manter aberto".
- Notificação do sistema quando um agente pergunta algo com o app em segundo plano; clicar leva à
  execução.
- Erros vindos do núcleo: mensagem para a pessoa + detalhe técnico recolhido.

==================================================================
PARTE F — RESPONSIVIDADE DESKTOP (720 a 2560 px)
==================================================================

Mostre cada tela principal em 1440×900 e indique (em telas extras ou anotações) como ela fica em:
- COMPACTO 720-1023px: barra lateral vira trilho de ícones (56px) com o nome ao passar o mouse; colunas
  laterais (inspetores, visão geral, detalhe) viram gaveta sobreposta à direita; grades com 1 coluna;
  ações secundárias do cabeçalho vão para o "⋯"; indicadores da barra superior viram só ícone + número.
- PADRÃO 1024-1439px: barra lateral aberta (216px); mestre-detalhe lado a lado só quando o detalhe tem
  ≥ 360px, senão gaveta; grades com 2 colunas.
- LARGO 1440-1919px: mestre-detalhe lado a lado; grades com 2-3 colunas; inspetor fixo no Workflow.
- ULTRA ≥ 1920px: conteúdo de leitura com largura máxima (texto ~72 caracteres, formulários ~720px,
  centralizados na área); listas e grades usam a largura extra (3-4 colunas); o canvas do workflow
  ocupa tudo.
Regras: nenhum texto cortado sem reticências e dica com o valor completo; caminhos longos cortados no
meio ("/Users/…/transport-erp"); cabeçalhos de página que quebram em duas linhas de forma ordenada
(título acima, ações abaixo) em vez de se sobrepor; rodapés de ação sempre visíveis (fixos);
altura mínima 600px: nada essencial depende de altura (painéis rolam por dentro); o canvas do workflow
nunca fica com menos de 50% da altura útil no modo Editar.

==================================================================
PARTE G — O QUE NÃO FAZER (o app não tem e não terá agora)
==================================================================

Não invente funções, dados ou telas. Em especial, NÃO desenhe:
- tema claro, personalização de cores, modo de alto contraste separado;
- contas, login no Atlas, avatar, perfis, times, compartilhamento, comentários, permissões de usuário,
  nuvem, sincronização, cobrança ou planos;
- busca global, paleta de comandos, ⌘K fora do MCP Hub, atalhos de teclado que não foram descritos;
- central de notificações, toasts empilhados, feed de atividades global, dashboard de métricas,
  gráficos, painéis de custo com gráfico, telemetria de máquina;
- chat entre agentes, agente que cria outros agentes, planejador por IA que monta workflow sozinho
  (os templates são determinísticos), "IA sugere" genérico;
- commit, push, pull request, merge automático ou integração com GitHub/GitLab (Aplicar só copia para
  os arquivos, sem commit);
- terminal livre para a pessoa digitar comandos, execução de testes ou shell pelos agentes;
- servidores MCP remotos (HTTP/SSE) funcionando (aparecem só como "ainda não suportado");
- edição de chaves de API (o Atlas não usa chave de API: usa o login que a pessoa já tem em cada CLI);
- telas do site do celular (fora deste trabalho);
- métricas, contadores, porcentagens, selos, filtros ou etapas que não estejam descritos;
- subtítulos, textos de ajuda ou placeholders inventados: use os textos descritos ou deixe o espaço
  vazio;
- o mesmo botão em dois lugares da mesma tela.

==================================================================
ENTREGA
==================================================================

Gere, nesta ordem, com nomes curtos em português:
1. "Design system" — tokens, tipografia, espaçamento e todos os componentes da PARTE B com estados.
2. "Workspace — Lista", "Workspace — Grade", "Workspace — Agente executando com aprovação pendente",
   "Workspace — Detalhes do agente", "Workspace — Inspetor de execução", "Workspace — Boas-vindas".
3. "Projetos e Harness", "Harness — Analisar projeto", "Harness — Análise ao vivo",
   "Harness — Revisar contexto".
4. "Workflow — Editar", "Workflow — Etapa selecionada", "Workflow — Novo workflow",
   "Workflow — Execução aguardando você", "Workflow — Execução em andamento",
   "Workflow — Recuperar execução", "Workflow — Revisão e entrega do código".
5. "Agentes — Lista", "Agentes — Criar agente", "Personalidades — Biblioteca",
   "Personalidades — Editar".
6. "Specs — Lista", "Specs — Documento em revisão", "Specs — Revisão final", "Specs — Nova especificação".
7. "MCP Hub — Servidores", "MCP Hub — Adicionar servidor", "MCP Hub — Sincronizar CLIs".
8. "Configurações — Geral", "Configurações — Memória", "Configurações — Importar e exportar",
   "Acesso pelo celular".
9. Para "Workspace — Lista", "Workflow — Editar" e "MCP Hub — Servidores": a mesma tela em 1024px e em
   1920px.
Use dados de exemplo realistas e coerentes entre as telas (workspace "Transport ERP" com os projetos
"transport-erp" e "transport-web"; agentes "Arquiteto", "Desenvolvedor", "QA"; workflow "Recuperação de
senha").
```

## Registro dos envios

| Data       | Como foi enviado                                                                                                                        | Telas geradas (ids)                                                         | Observações                                                                                                                                                                                                         |
| ---------- | --------------------------------------------------------------------------------------------------------------------------------------- | --------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| 2026-10-21 | Prompt completo (A–G + Entrega) numa chamada `generate_screen_from_text`, projeto `526454930665111857`, DESKTOP, sem design system fixo | só "Design system" (`641970d131bd4e99a9b39263bc9e119a`); nenhuma outra tela | A chamada deu timeout e não foi reenviada. Inventou: "v1.2 // Core Tokens" no logo, item "Design system" na navegação, "Documentação" e "Status do Daemon" no rodapé, "Sync" no botão Celular, "Inspecionar Tokens" |
