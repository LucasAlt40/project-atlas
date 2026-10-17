# Revisão de design — Configurações

Módulo 4 da repaginação (processo em [`../_comum/README.md`](../_comum/README.md)). Configurações
globais do app (as de cada workspace ficam no próprio workspace): idioma, limite de tokens por
especificação, sandbox dos runtimes, memória (notas de trabalhos anteriores), importar e exportar,
sobre e atualizações.

Capturas em 1440×900 com dados de exemplo em [`atual/`](atual/). Capturar: `?memory=none|some`
(padrão `some`: duas notas esperando decisão, duas guardadas). O mock responde sandbox, memória, sobre
e atualização (sempre "em dia"); o fluxo de importar precisa de `pick_config_file` e
`preview_config_import` (estender o mock em `tmp/`).

## 1. Moldura

Igual à dos outros módulos. A tela é **uma só página** com rolagem, título "Configurações".

## 2. Estrutura atual e interações

Página única, coluna de cerca de 630 px alinhada à esquerda, com sete cartões em sequência
(**[img 01 a 04]**). Cada cartão tem um título pequeno em caixa alta mono.

1. **Idioma**: seletor (Português (Brasil) / English), ajuda "O idioma da interface do Atlas. A saída
   dos modelos e o seu próprio conteúdo não são traduzidos." Troca na hora.
2. **Limite de tokens por especificação**: campo numérico + `Salvar`, placeholder "Sem limite", ajuda
   longa (o que uma especificação pode usar entre entrada e saída; ao chegar, nenhuma execução nova
   começa até você decidir continuar; vazio: sem limite). Vale para Specs.
3. **Sandbox dos runtimes**: parágrafo longo sobre o que o sistema confina e o que não; "Este sistema
   consegue confinar processos." (ou o motivo de não conseguir); seletor `Sandbox` (Desligado / Ligado
   quando o sistema puder / Obrigatório: não iniciar sem ele); caixa "Deixar escrever nos caches de
   ferramentas (npm, cargo, pip…)" com ajuda. Desligado por padrão.
4. **Notas de trabalhos anteriores (Memória, ADR 0037)** — a maior parte da página **[img 02, 03]**:
   - texto de ajuda longo (uma nota é uma dica, não instrução, e não concede nada);
   - _Onde as notas são usadas_: três seletores (Minhas (todos os projetos), Workspace, Agente), cada
     um Desligado / Propor, eu aprovo / Automático (o de "Minhas" não tem Automático), com um parágrafo
     explicando os três modos; _Máximo de tokens por prompt_ + `Definir limite`;
   - _Esperando sua decisão_: notas propostas por agentes, com selos "NÃO CONFIRMADA (AUTOMÁTICA)" e
     "ESPERANDO UM VALIDADOR INDEPENDENTE", título, tipo · escopo · dono, texto, "Usada 0×", `Aprovar`,
     `Descartar`, `Editar`;
   - _Notas guardadas_: título, tipo · escopo, texto, "Usada 7×", `Arquivar`, `Apagar` (vermelho),
     `Editar`; ação `Apagar as 2 nota(s) automática(s)`;
   - _Adicionar uma nota_: escopo, tipo (Como fazer / Armadilha / Preferência / Decisão), título, texto
     (limite 480 caracteres, contador), `Adicionar nota`.
5. **Importar e exportar** **[img 03, 05, 06]**: texto sobre compartilhar agentes, personalidades e
   workflows sem chaves, tokens, credenciais, conversas, logs nem caminhos; `Importar` e `Exportar
vários…`.
   - Exportar vários expande **na própria página**: "O que exportar" com quatro grupos de caixas
     (Personalidades que você criou ou alterou, Agentes, Templates de workflow salvos, Workflows do
     workspace aberto), `Continuar (3)` / `Cancelar`; depois o diálogo "Exportar configurações" (**[código]**:
     "Incluir o que ele precisa", "O arquivo vai conter", "Citados, não incluídos", "Vale uma segunda
     olhada", `Salvar arquivo…`).
   - Importar abre um **modal** "Importar configuração" (**[img 06]**): frase, aviso "Importar só adiciona
     definições. Nada é executado, nenhum workflow é iniciado e nenhuma ferramenta é concedida…",
     `Escolher arquivo…`. Depois (**[código]**): revisão do que o arquivo contém (Arquivo, "Exportado pelo
     Atlas 0.1.5", workspace dos workflows), cartão por item (Personalidade / Agente / Workflow) com
     fatos (Runtime, Modelo, Permissões, Worktree isolado, Resultado, Tags…), "Precisa de: …",
     conflitos ("Você já tem “X”, e é igual / com conteúdo diferente"), decisão (Importar como novo /
     Substituir o que eu já tenho / Não importar, com "Confirmo: sobrescrever…"), "Necessários, mas fora do
     arquivo", permissões e escolha de runtime e modelo, e o resultado final.
6. **Sobre** **[img 04]**: Atlas, Versão, Plataforma.
7. **Atualizações** **[img 04]**: `Buscar atualizações` → "Buscando…" → "Você está usando a versão mais
   recente do Atlas." / "A versão X está disponível." + `Instalar e reiniciar` / "Instalando…".

A troca de idioma também existe no botão PT | EN da barra superior (moldura).

## 3. Problemas de design observados

1. **Uma coluna estreita à esquerda, três quartos da tela vazios**, e uma rolagem enorme com tudo em
   sequência. Não há índice nem forma de ir direto a "Sobre" ou "Importar".
2. **A Memória domina a página** (mais de dois terços da altura) e mistura **configuração** (modos,
   limite) com **gestão de conteúdo** (propostas, notas guardadas, formulário). Quem só quer mudar o
   idioma passa por ela.
3. **Parágrafos longos em texto pequeno e cinza** antes de cada controle (sandbox, limite, memória,
   modos); o controle, que é o importante, fica depois do texto.
4. **Títulos de seção minúsculos em caixa alta mono** que parecem rótulos de campo; dentro da
   Memória há três níveis de título parecidos.
5. **Controles com formatos diferentes**: campo + `Salvar` lado a lado no limite, campo + `Definir
limite` na Memória, seletor com caixa, botões de texto sem borda (`Aprovar`, `Descartar`, `Editar`).
6. **Ações destrutivas e de decisão com o mesmo peso**: `Apagar` e `Apagar as 2 nota(s) automática(s)`
   convivem com `Arquivar` e `Editar`; `Aprovar` é o único primário mas aparece em cada nota.
7. **Selos de proveniência em caixa alta mono** ("NÃO CONFIRMADA (AUTOMÁTICA)", "ESPERANDO UM VALIDADOR
   INDEPENDENTE") longos e repetidos em cada nota.
8. **Importar e exportar se comportam de modos diferentes**: exportar expande na página, importar
   abre modal; o exportar tem um segundo passo em outro diálogo.
9. **Sobre e Atualizações** são dois cartões grandes para três linhas de texto e um botão.
10. **Sem sinal de estado salvo**: trocar o limite ou um modo grava, mas nada confirma.

## 4. Telas do Stitch que já existem

Nenhuma tela anterior do Stitch cobre Configurações. Todas as telas deste módulo são novas.

## 5. Lacunas

Não capturados: sandbox indisponível (sistema sem sandbox), limite inválido, erro ao salvar, memória
vazia (`?memory=none`), edição de nota, a revisão da importação e o resultado, o diálogo "Exportar
configurações", atualização disponível e instalando.
