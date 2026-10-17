# Prompts para o Stitch — Agentes e Personalidades

Ainda **não enviados**. Cada prompt = **P0** de [`../_comum/p0-regras.md`](../_comum/p0-regras.md)
(com `<NOME DA TELA>` trocado) + o bloco **TELA** abaixo. Projeto Stitch `526454930665111857`,
design system `assets/7aa0774b734b4149b2d38a23136aadfd`, `deviceType: DESKTOP`.

Telas novas aparecem ao lado das originais (o `edit_screens` não sobrescreve). A chamada dá timeout
e a geração termina: não reenviar; esperar o usuário confirmar e conferir com `list_screens`.

| Prompt | Nome da tela                                 | Ferramenta                  | Base                               |
| ------ | -------------------------------------------- | --------------------------- | ---------------------------------- |
| A1     | Agentes — Lista de agentes                   | `edit_screens`              | `a692a557a01d43e599392fcec8edac49` |
| A2     | Agentes — Sem agentes                        | `generate_screen_from_text` | (nova)                             |
| A3     | Agentes — Criar e editar agente              | `edit_screens`              | `9e4c180f787042859d3a05dc0ed55c03` |
| P1     | Personalidades — Biblioteca                  | `edit_screens`              | `ada6c661994542aca6046e7022ee5a03` |
| P2     | Personalidades — Nova e editar personalidade | `generate_screen_from_text` | (nova)                             |

Ordem de envio: A1 → A3 → P1 → A2 → P2 (as telas que existem primeiro).

## A1 — Agentes — Lista de agentes

```text
TELA: página "Agentes", visão de lista.
Cabeçalho: título "Agentes" com o chip "3 no total"; subtítulo "Gerencie agentes autônomos, conexões
de runtime e permissões."; à direita a busca "Buscar agente ou modelo…", o botão secundário
"Importar" e o primário "+ Criar agente".
Resumo: UMA linha discreta de texto (não quatro caixas grandes): "3 agentes · 2/2 runtimes prontos ·
3 com isolamento Git · 0 com resultado definido".
Filtro em abas: "Todos (3) | Prontos (2) | Indisponíveis (1)".
Lista: cartões em grade de 2 colunas (1 coluna em janela estreita). Cada cartão tem no topo o ícone,
o nome do agente, o selo da personalidade (ex.: "Arquiteto") e, à direita, o selo de disponibilidade
("PRONTO" verde ou "INDISPONÍVEL" âmbar); logo abaixo o runtime em mono ("Claude CLI"). No corpo,
os quatro dados em duas linhas de pares rótulo/valor, sem caixa interna e com o valor perto do
rótulo: Modelo "sonnet", Permissões "Somente leitura", Resultado "Geral", Isolamento Git "Ativo".
Ações com rótulo de texto: botão secundário "Editar" e menu "⋯" com "Exportar" e, separado e em
vermelho, "Excluir". Mostre 3 agentes: Arquiteto Backend (selo Arquiteto, Claude CLI, sonnet, PRONTO),
QA Regressão (selo QA, Claude CLI, sonnet, PRONTO), Implementador API (selo Arquiteto, Codex CLI,
gpt-5, INDISPONÍVEL).
USE só esses dados. NÃO mude: o filtro, a busca, o que cada cartão mostra.
```

## A2 — Agentes — Sem agentes

```text
TELA: a mesma página "Agentes" sem nenhum agente. Cabeçalho igual ao da lista (título "Agentes",
subtítulo, busca, "Importar", "+ Criar agente"), sem resumo nem abas. No centro, um estado vazio
simples: o texto "Nenhum agente ainda. Crie um para começar." e o botão primário "Criar agente".
Nada mais.
```

## A3 — Agentes — Criar e editar agente

```text
TELA: formulário de agente, que ocupa a área de conteúdo no lugar da lista. Breadcrumb "Agentes",
título "Criar agente" (na edição: "Editar Arquiteto Backend"). Duas colunas.
Coluna principal, três seções com título legível (sem barra de acento repetida) e sem caixa dentro
de caixa:
1. "Identidade & papel": campo "Nome" (placeholder "Especialista em Arquitetura") e seletor
   "Personalidade" (ex.: "Arquiteto") com, embaixo, a descrição dela em uma linha ("Perfil focado em
   arquitetura.").
2. "Provedor & runtime de IA": seletor "Provedor de IA" ("Anthropic"), seletor "Conexão" ("Claude
   CLI") com o estado "✓ Pronto · 2.1.285", campo "Modelo" (campo "ID do modelo" com a dica "Use um
   alias como sonnet ou opus, ou o nome completo do modelo."), botão ghost "Detectar runtimes
   novamente" e o aviso discreto "Este runtime não suporta edição de arquivos.".
3. "Instruções adicionais": área de texto grande.
Coluna lateral, duas seções:
4. "Resultado do agente": seletor "Contrato de resultado" ("Revisão") com a frase "O agente conclui com
   approved ou changes_requested." e dois itens com checkbox: "approved — Aprovado" e
   "changes_requested — Mudanças solicitadas".
5. "Permissões e isolamento": seletor "Permissões" ("Desenvolvedor") com a dica "Pode editar arquivos
   e executar comandos de desenvolvimento, no worktree isolado. A política do workspace continua
   limitando.", a linha "Sugerido pela personalidade Arquiteto. Você pode alterar." e a caixa
   "Sempre trabalhar em um Git worktree isolado" com a explicação curta.
Rodapé fixo: "Cancelar" (secundário) e "Criar agente" (primário; na edição, "Salvar agente").
USE só esses campos. NÃO mude: os campos, as opções dos seletores nem os textos de ajuda.
```

## P1 — Personalidades — Biblioteca

```text
TELA: página "Personalidades". Cabeçalho: título com os chips "2 disponíveis", "1 customizada",
"2 nativas"; à direita "Importar" (secundário), "Restaurar personalidades padrão" (ghost) e
"+ Nova personalidade" (primário); uma frase de introdução ("As personalidades definem como um
agente se comporta. Inspecione uma, edite-a ou use-a para criar um agente.").
Aviso de uma linha, recolhível e fechado por padrão: "Injeção global do protocolo Atlas", que ao abrir
mostra "O Atlas adiciona suas regras de narração ao vivo e de somente leitura a toda personalidade,
padrão ou personalizada. O prompt exato aparece depois de cada execução.".
Abas "Todas (3) | Padrão do sistema (2) | Customizadas (1)" e, à direita, a busca "Buscar
personalidade, tag ou instrução…".
Lista: cartões compactos em grade de até 3 colunas. Fechado, o cartão mostra o ícone, o nome, o selo
("Padrão" ou "Personalizada"), a descrição em duas linhas, as tags e o botão primário "Usar esta
personalidade" mais um menu "⋯" com "Editar", "Exportar" e, em vermelho, "Excluir". Ao expandir (um
cartão aberto por vez), mostra a seção "Comportamento" (lista de pontos) e "Instruções do sistema"
(bloco mono). Mostre 3 cartões: Arquiteto (Padrão, aberto, com cinco pontos de comportamento e a
instrução "Você é um arquiteto de software experiente."), QA (Padrão, fechado) e Arquiteto .NET Sênior
(Personalizada, fechado, tags #dotnet #ddd).
USE só esses dados. NÃO mude: as abas, a busca e as ações de cada personalidade.
```

## P2 — Personalidades — Nova e editar personalidade

```text
TELA: a biblioteca de personalidades (esmaecida ao fundo) com um painel lateral direito de 440px,
aberto sobre ela. Título "Nova personalidade" com o selo "Personalizada" (na edição: "Editar
personalidade"). Campos: "Nome" (placeholder "Arquiteto .NET Sênior"), "Descrição" (placeholder
"Foca em Clean Architecture e DDD."), "Instruções" (área de texto grande) e "Tags" (placeholder
"arquitetura, ddd (separadas por vírgula)"). Rodapé do painel: "Cancelar" e "Salvar personalidade"
(primário). Nada além disso.
```

## Registro de envios

| Prompt | Enviado    | Tela gerada no Stitch                                                                          | Observação                                                                                                                                |
| ------ | ---------- | ---------------------------------------------------------------------------------------------- | ----------------------------------------------------------------------------------------------------------------------------------------- |
| A1     | 2026-10-16 | "Agentes — Lista de agentes" (`42f5fb75c558460f8d86936f0028e54c`, 2560×2048)                   | `edit_screens` criou tela nova; a original `a692a557…` continua intacta. Timeout na chamada, geração concluída.                           |
| A3     | 2026-10-16 | "Agentes — Criar e editar agente" (`5b2df1e4aaa2428e8a009701747f3b6a`, 2560×2048)              | `edit_screens` criou tela nova; a original `9e4c180f…` continua intacta. Timeout na chamada, geração concluída.                           |
| P1     | 2026-10-16 | "Personalidades — Biblioteca" (`efdc4e192ab14fb39ee2c4f515a96ede`, 2560×2048)                  | `edit_screens` criou tela nova; a original `ada6c661…` continua intacta. Timeout na chamada, geração concluída.                           |
| A2     | 2026-10-16 | "Agentes — Sem agentes" (`387895715cfb402db603a057a1a6dfa4`, 2560×2048)                        | `generate_screen_from_text` com o design system `assets/7aa0774b…`; tela nova. Desta vez a chamada **não** deu timeout e devolveu a tela. |
| P2     | 2026-10-16 | "Personalidades — Nova e editar personalidade" (`2ca471ef773643e2a1a37920e186afc7`, 2560×2048) | `generate_screen_from_text` com o design system `assets/7aa0774b…`; tela nova. Timeout na chamada, geração concluída.                     |

Avaliação do A1 (vista pelo PNG exportado): fiel ao prompt (resumo numa linha, abas, cartões de 2 colunas, `Editar` + `⋯`, selo INDISPONÍVEL). Desvio: o Stitch pôs "Atlas Core · v2.4.0 · Cluster" no topo da barra lateral e "v0.1.0" no rodapé, mesmo com o P0 proibindo; a implementação ignora ("O Stitch mostra, o app não tem").

Avaliação do A3 (vista pelo PNG exportado): fiel em estrutura (duas colunas, cinco seções, rodapé fixo, `Detectar runtimes novamente` no cabeçalho da seção). O reforço do P0 funcionou: a barra lateral voltou a ter só "Atlas". Inventados (a implementação ignora ou troca pelo texto do app): subtítulo "Configure a identidade, runtime e políticas de isolamento."; texto de ajuda e placeholder de "Instruções adicionais" ("Diretrizes operacionais extras…", "// Forneça diretrizes específicas…") e o rótulo "Markdown / Plaintext"; a explicação do isolamento ("Cria uma árvore de trabalho temporária…", o app tem o seu próprio texto). As seções ainda têm um ponto de acento no título e continuam em cartões.

Avaliação do P1 (vista pelo PNG exportado): fiel em estrutura (cabeçalho com chips e ações, abas com contagem, busca, grade de 3 colunas, cartão expandido com Comportamento e Instruções, `Usar esta personalidade` + `⋯`). Desvios: o aviso "Injeção global do protocolo Atlas" veio **aberto** (o prompt pedia fechado); o título da página virou "Personalidades — Biblioteca" (o app mantém "Personalidades"); tags e descrição de "Arquiteto .NET Sênior" e as tags de Arquiteto e QA foram inventadas (a implementação usa os dados reais); ponto verde ao lado do workspace na barra lateral (inventado). O prompt trazia "2 disponíveis"; o Stitch corrigiu para 3 (correto: 2 nativas + 1 customizada). Os cartões fechados e o expandido têm alturas diferentes e a metade de baixo da página fica vazia.

Avaliação do A2 (vista pelo PNG exportado): fiel e simples — cabeçalho igual ao da lista (sem chip, resumo nem abas), estado vazio centralizado com o texto atual e o botão `Criar agente`. Observações: "Criar agente" aparece duas vezes (cabeçalho e centro), como o prompt pediu; o Stitch pôs "Configurações" no rodapé da barra lateral, separado dos demais itens (a moldura real as tem na mesma lista: a implementação segue o app).

Avaliação do P2 (vista pelo PNG exportado): estrutura fiel (painel de 440px sobre a biblioteca esmaecida, título com o selo "Personalizada", quatro campos, rodapé `Cancelar` / `Salvar personalidade`). Desvios: o placeholder de "Instruções" é um texto longo inventado (C#, .NET 8, MediatR) — o app tem o seu; o campo de instruções ocupa quase toda a altura do painel; a biblioteca ao fundo mostra "Nativa" e descrições inventadas (o app diz "padrão"/"nativas" conforme a tela); o breadcrumb traz o nome inteiro da tela.

## Fechamento do módulo

Cinco telas geradas no Stitch (A1, A2, A3, P1, P2), todas registradas acima. Resumo do que a implementação deve ignorar ou trocar pelo texto do app: subtítulo, ajuda e placeholder inventados em A3 e P2; tags e descrições inventadas em P1; "Atlas Core · v2.4.0 · Cluster" em A1; o aviso "Injeção global" que veio aberto em P1.
