# Revisão de design — Agentes e Personalidades

Módulo 2 da repaginação (processo em [`../_comum/README.md`](../_comum/README.md)). As duas telas
são o catálogo global do app: **Agentes** (quem executa) e **Personalidades** (como se comporta).
Capturas do app em 1440×900 com dados de exemplo em [`atual/`](atual/). Os agentes e as
personalidades são globais; só a colocação do agente pertence ao workspace.

Capturar: `?agents=0..3`, `?custom=1` (acrescenta uma personalidade customizada),
`?runtimes=down` (Codex deixa de estar instalado → agente "Indisponível").

## 1. Moldura

Igual à do módulo 1 (barra lateral, barra superior). Não muda.

## 2. Telas, estados e interações

### 2.1 Agentes — lista **[img 01, 06]**

- **Cabeçalho**: título "Agentes" + chip "3 no total"; subtítulo "Gerencie agentes autônomos, conexões
  de runtime e permissões."; à direita busca "Buscar agente ou modelo…", `Importar` (secundário) e
  `+ Criar agente` (primário).
- **Faixa de resumo** de 4 caixas: Agentes, Runtimes prontos (2/2), Com isolamento Git, Com
  resultado definido.
- **Filtro** em abas: `Todos (3) · Prontos (3) · Indisponíveis (0)`.
- **Cartão do agente** (grade de 2 colunas): ícone, nome, selo da personalidade (`Arquiteto`), runtime
  em mono (`Claude CLI`), selo de disponibilidade (`PRONTO` verde / `INDISPONÍVEL` âmbar); caixa interna
  com quatro pares (Modelo, Permissões, Resultado, Isolamento Git); três ícones sem legenda:
  exportar (↑), editar (✎), excluir (🗑).
- **Excluir**: troca os ícones por "Excluir <nome>?" + `Confirmar exclusão` / `Cancelar` no próprio
  cartão (**[código]**). Falha quando o agente está trabalhando.
- **Sem agentes** (**[código]**): texto "Nenhum agente ainda. Crie um para começar."; sem resultado de busca:
  "Nenhum agente corresponde à busca."; erro do núcleo e "carregando" são frases soltas.
- **Aviso** após criar: "<nome> foi salvo. Crie um workspace para usá-lo." ou "<nome> foi salvo, mas não foi
  adicionado ao workspace. <motivo>".
- Fluxo: criar o agente o coloca no workspace ativo e leva ao Workspace.

### 2.2 Agentes — criar e editar **[img 02, 03]**

Substitui a lista (breadcrumb "Agentes", título "Criar agente" ou "Editar <nome>"). Duas colunas:

- **Esquerda**: _Identidade & Papel_ (Nome, Personalidade com a descrição dela embaixo); _Provedor &
  Runtime de IA_ (Provedor, Conexão com estado "✓ Pronto · 2.1.285", Modelo — campo livre "ID do
  modelo" com dica por runtime, ou seletor quando o runtime lista modelos; `Detectar runtimes
novamente`; avisos como "Este runtime não suporta edição de arquivos."); _Instruções Adicionais_
  (textarea).
- **Direita**: _Contrato de Saída_ (Geral, Validação, Revisão, Implementação, Personalizado; para
  Validação e Revisão uma lista de resultados com checkbox, por exemplo `approved — Aprovado`;
  Personalizado abre um editor de resultados); _Permissões & Isolamento_ (perfil Desenvolvedor /
  Somente leitura com dica, "Sugerido pela personalidade …", checkbox "Sempre trabalhar em um Git
  worktree isolado" com explicação).
- **Rodapé fixo**: `Cancelar` e `Criar agente` / `Salvar agente`.
- Escolher a personalidade sugere contrato e perfil. Estados: detectando runtimes, falha ao detectar,
  runtime sem autenticação / não instalado, erro de validação ao salvar, salvando.

### 2.3 Personalidades — biblioteca **[img 04, 07]**

- **Cabeçalho**: título + três chips (`2 disponíveis`, `0 customizadas`, `2 nativas`); `Importar`,
  `Restaurar personalidades padrão`, `+ Nova personalidade`; introdução.
- **Aviso fixo** "Injeção global do protocolo Atlas" com o texto sobre a narração ao vivo.
- **Abas** `Todas · Padrão do sistema · Customizadas` com contagem, e busca ("Buscar personalidade, tag ou
  instrução…").
- **Cartão** (grade de até 3 colunas): ícone, nome, selo `Padrão` / `Personalizada`; descrição numa caixa;
  **Comportamento** (lista de pontos); **Instruções do sistema** (bloco mono); tags; ações
  `Usar esta personalidade` (primário, abre Criar agente já com ela), exportar (↑), `Editar`, `Excluir`.
- Sem resultado: "Nenhuma personalidade corresponde à busca."

### 2.4 Personalidades — nova e editar **[img 05]**

Painel à direita como terceira coluna (a grade encolhe): título "Nova personalidade" / "Editar
personalidade" + selo; Nome, Descrição, Instruções, Tags (separadas por vírgula); `Cancelar`,
`Salvar personalidade`. Para uma nativa o painel mostra uma nota de que ela não é editável
(**[código]** `personalities.form.builtinNote`).

### 2.5 Compartilhado com outro módulo

`Importar` e Exportar abrem os diálogos de portabilidade (ADR 0032), que são tratados no módulo
**Configurações**. O formulário de agente também é usado pelo painel "editar agente" do Workspace
(`EditAgentPanel`): ao mudar `AgentForm`, conferir esse painel.

## 3. Problemas de design observados

1. **Ações só com ícone** nos cartões de agente (exportar, editar, excluir): sem legenda, e o excluir
   fica colado no editar.
2. **Caixa dentro de caixa**: cada cartão tem uma caixa escura interna com quatro pares rótulo/valor, o
   valor fica longe do rótulo (linha larga), o que faz o olho viajar.
3. **Faixa de resumo decorativa**: quatro caixas grandes ("Com resultado definido 0") ocupam uma faixa
   inteira acima da lista sem ajudar a decidir nada.
4. **Selos sem sistema**: personalidade (azul), disponibilidade (verde/âmbar, caixa alta), `Padrão` /
   `Personalizada` (cinza) usam formatos diferentes.
5. **Formulário longo e denso**: dois níveis de borda (cartão > fieldset) em _Contrato de Saída_ e
   _Isolamento_; títulos de seção com barra de acento repetida; textos de ajuda longos e em cinza
   pequeno; avisos soltos ("Este runtime não suporta edição de arquivos.").
6. **Ordem de leitura**: a coluna direita (resultado, permissões) vem depois da esquerda, mas depende da
   personalidade escolhida lá em cima; o usuário não vê o efeito.
7. **Cartões de personalidade com tudo aberto**: descrição, comportamento, instruções e tags em todos os
   cartões; três cartões já passam de uma tela, e nenhuma informação é escolhida como principal.
8. **Painel de nova personalidade espreme a grade**: vira a terceira coluna e os cartões ficam estreitos
   e altos.
9. **Aviso global sempre aberto** na biblioteca, antes do conteúdo, mesmo depois de lido.
10. **Estados vazio, erro e carregando** são frases soltas, sem ação nem hierarquia.

## 4. Telas do Stitch que já existem (versões anteriores)

| Tela do app                 | Tela anterior no Stitch (id)                                                                                                                            |
| --------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Agentes — lista             | Atlas - Gerenciador de Agentes (`a692a557a01d43e599392fcec8edac49`)                                                                                     |
| Agentes — criar/editar      | Atlas - Edição e Configuração do Agente (`9e4c180f787042859d3a05dc0ed55c03`); Atlas - Agents Management & Creation (`00e8d85bc115418087b26f478c88e758`) |
| Personalidades — biblioteca | Atlas - Gerenciador de Personalidades (`ada6c661994542aca6046e7022ee5a03`)                                                                              |

O app já seguiu esse desenho na primeira rodada; esta é a segunda passada com as regras novas
(legibilidade e separação).

## 5. Lacunas

Não capturados: lista vazia, busca sem resultado, confirmação de exclusão, formulário em edição,
erro de salvamento, runtime sem autenticação, os diálogos de importar/exportar (módulo Configurações).
