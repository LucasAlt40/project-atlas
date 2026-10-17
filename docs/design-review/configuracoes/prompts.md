# Prompts para o Stitch — Configurações

Ainda **não enviados**. Cada prompt = **P0** de [`../_comum/p0-regras.md`](../_comum/p0-regras.md)
(com `<NOME DA TELA>` trocado) + o bloco **TELA** abaixo. Projeto Stitch `526454930665111857`,
design system `assets/7aa0774b734b4149b2d38a23136aadfd`, `deviceType: DESKTOP`.

Não há tela anterior de Configurações no Stitch: todas usam `generate_screen_from_text`. A chamada
dá timeout e a geração termina: não reenviar; esperar o usuário confirmar e conferir com
`list_screens`.

Todas as telas são **seções da mesma página** "Configurações". Para o Stitch manter a página coerente,
cada prompt repete a estrutura: título "Configurações", um **índice lateral interno** de 200px com as
seções (Geral, Memória, Importar e exportar, Sobre e atualizações) e o conteúdo da seção ao lado, com
largura máxima de 880px. O índice é só navegação por âncoras dentro da página.

| Prompt | Nome da tela                         | Ferramenta                  |
| ------ | ------------------------------------ | --------------------------- |
| C1     | Configurações — Geral                | `generate_screen_from_text` |
| C2     | Configurações — Memória              | `generate_screen_from_text` |
| C3     | Configurações — Importar e exportar  | `generate_screen_from_text` |
| C4     | Configurações — Revisar importação   | `generate_screen_from_text` |
| C5     | Configurações — Sobre e atualizações | `generate_screen_from_text` |

Ordem: C1 → C2 → C3 → C4 → C5. O `designSystem` de cada chamada é o do projeto.

## C1 — Configurações — Geral

```text
TELA: página "Configurações", seção "Geral" selecionada no índice lateral. Título "Configurações".
Conteúdo, três grupos separados por linha fina, cada um com título legível, o controle em primeiro lugar
e a ajuda embaixo em texto secundário curto:
1. "Idioma": seletor "Português (Brasil)" (a outra opção é "English"); ajuda "O idioma da interface do
   Atlas. A saída dos modelos e o seu próprio conteúdo não são traduzidos.". A troca vale na hora (sem
   botão de salvar).
2. "Limite de tokens por especificação": campo numérico com o valor "200000" (placeholder "Sem limite")
   e o botão "Salvar" no mesmo grupo; ajuda "O quanto as execuções de uma especificação podem usar
   juntas (entrada mais saída, como os runtimes informam). Ao chegar nele, nenhuma execução nova
   começa até você decidir continuar. Vazio: sem limite.".
3. "Sandbox dos runtimes": primeiro o selo de estado "Este sistema consegue confinar processos." (verde),
   depois o seletor "Sandbox" com "Ligado quando o sistema puder" (opções: "Desligado", "Ligado quando o
   sistema puder", "Obrigatório: não iniciar sem ele") e a caixa "Deixar escrever nos caches de
   ferramentas (npm, cargo, pip…)" marcada, com a ajuda "Sem eles, instalar e testar falham dentro do
   sandbox. Com eles, um processo poderia alterar um cache que outros projetos usam.". A explicação longa
   ("O sistema operacional confina o programa do runtime e tudo o que ele inicia: só escreve no
   worktree, nas próprias pastas e numa pasta temporária da execução, e não lê suas chaves SSH,
   credenciais de nuvem nem dados do navegador. A rede continua aberta… Desligado por padrão: ligue
   depois de testar uma tarefa.") fica numa seção recolhida "Como funciona".
USE só esses controles. NÃO mude: as opções e os textos de ajuda.
```

## C2 — Configurações — Memória

```text
TELA: página "Configurações", seção "Memória" selecionada. Título da seção "Memória" com a frase
"Lições curtas que os agentes e você deixam para a próxima execução (um comando que funciona, uma
armadilha, uma escolha). Uma nota é uma dica que o agente pode usar: não é instrução e não concede
nada. Tudo fica desligado até você ligar um escopo.".
Quatro blocos, em abas ou em seções, nesta ordem, separando configuração de conteúdo:
1. "Onde as notas são usadas" (configuração): três linhas com seletor — "Minhas (todos os projetos)":
   "Propor, eu aprovo"; "Workspace": "Propor, eu aprovo"; "Agente": "Automático" — com as opções
   "Desligado", "Propor, eu aprovo" e (exceto em "Minhas") "Automático"; a explicação dos três modos
   numa seção recolhida; e "Máximo de tokens por prompt" com o campo "800" e o botão "Definir limite".
2. "Esperando sua decisão (2)": cada nota proposta é uma linha com título ("Rodar os testes com
   --runInBand"), metadados "Armadilha · Workspace · Transport ERP", o texto, o selo único "Não
   confirmada" (o detalhe "automática, esperando um validador independente" aparece ao passar o mouse
   ou ao expandir), "Usada 0×" e as ações: "Aprovar" (primário), "Descartar" e "Editar".
3. "Notas guardadas (2)": linhas com título ("Mensagens de commit em português"), metadados
   "Preferência · Minhas (todos os projetos)", texto, "Usada 7×", ação "Editar" à vista e menu "⋯" com
   "Arquivar" e, em vermelho e separado, "Apagar". Ação de grupo, em ghost e fora do caminho:
   "Apagar as 2 nota(s) automática(s)".
4. "Adicionar uma nota": escopo ("Minhas (todos os projetos)"), tipo ("Como fazer"; opções Como
   fazer, Armadilha, Preferência, Decisão), título, texto com o contador "0/480 caracteres" e a nota "Um
   texto maior é conhecimento de projeto: o lugar dele é o Harness.", e o botão "Adicionar nota".
USE só esses dados. NÃO mude: os modos, os tipos, os limites.
```

## C3 — Configurações — Importar e exportar

```text
TELA: página "Configurações", seção "Importar e exportar" selecionada. Título da seção e o texto
"Compartilhe agentes, personalidades e workflows com outras pessoas, leve-os para outra instalação ou
guarde um backup. Um arquivo nunca contém chaves de API, tokens, credenciais, conversas, logs nem
caminhos desta máquina.". Dois cartões de ação lado a lado: "Importar" (botão "Importar") e "Exportar"
(botão "Exportar vários…"). Mostre o estado em que "Exportar vários…" foi acionado: abaixo dos cartões
abre o passo "O que exportar" com quatro grupos de caixas de seleção: "Personalidades que você criou ou
alterou (0)" (desabilitado), "Agentes (3)" (marcado), "Templates de workflow salvos (0)" (desabilitado)
e "Workflows do workspace aberto (0)" (desabilitado), e os botões "Continuar (3)" (primário) e
"Cancelar". Não desenhe nada além disso.
```

## C4 — Configurações — Revisar importação

```text
TELA: a seção "Importar e exportar" esmaecida ao fundo e, sobre ela, um modal largo "Importar
configuração" no passo de revisão (depois de escolher o arquivo). Cabeçalho: "Arquivo: equipe-backend.json"
e "Exportado pelo Atlas 0.1.5". Aviso fixo (uma linha, ícone de informação): "Importar só adiciona
definições. Nada é executado, nenhum workflow é iniciado e nenhuma ferramenta é concedida; agentes que
podem editar arquivos chegam só com leitura, a menos que você diga o contrário abaixo.". Seção "O que o
arquivo contém" com uma lista de itens; cada item é uma linha recolhível com o selo do tipo
("Personalidade", "Agente", "Workflow"), o nome e um selo de situação ("Novo", "Você já tem “QA”, e é
igual" ou "Você já tem “Arquiteto”, com conteúdo diferente"). Aberta, a linha mostra os fatos em pares
(Runtime "Claude CLI", Modelo "sonnet", Permissões "Somente leitura", Worktree isolado "Ligado",
Resultado "Revisão"), "Precisa de: Arquiteto" e a decisão em três opções de rádio: "Importar como novo",
"Substituir o que eu já tenho" (com o aviso e a caixa "Confirmo: sobrescrever “Arquiteto”") e "Não
importar". Se faltar algo, um bloco "Necessários, mas fora do arquivo" com "Agente “QA” é… Escolher…".
Rodapé: "Escolher outro arquivo" (ghost), "Cancelar" e o primário "Importar". Mostre também, em uma
faixa pequena, o estado final: "2 itens importados".
```

## C5 — Configurações — Sobre e atualizações

```text
TELA: página "Configurações", seção "Sobre e atualizações" selecionada. Um único cartão com duas
partes. "Sobre": três pares rótulo/valor em mono: Atlas "Project Atlas", Versão "0.1.5", Plataforma
"macos". "Atualizações": o botão "Buscar atualizações" e, ao lado, o estado em texto: no estado normal
"Você está usando a versão mais recente do Atlas."; descreva também (sem desenhar) os outros estados:
"Buscando…", "A versão 0.2.0 está disponível." com "Instalar e reiniciar", "Instalando…" e
"Atualização instalada. O Atlas vai reiniciar.". Nada além disso.
```

## Registro de envios

| Prompt         | Enviado | Tela gerada no Stitch | Observação |
| -------------- | ------- | --------------------- | ---------- |
| (nenhum ainda) |         |                       |            |
