# 0022 — Camada de Otimização, fase 2: Skills com divulgação progressiva

## Contexto

Os agentes do Atlas rodam em runtimes de CLI que recebem um único prompt. Colocar todo o conhecimento do projeto em todo
prompt desperdiça tokens e enterra a parte que importa; deixá-lo de fora o perde. O formato aberto **Agent Skills** já
responde a isso: uma pasta com um `SKILL.md` (frontmatter YAML com `name` e `description`, depois as instruções),
opcionalmente `references/`, `scripts/` e `assets/`, carregada em níveis. O Atlas adota o formato como ele é.

## Decisão

**Formato.** `<nome>/SKILL.md` com os campos padrão (`name`, `description`, `license`, `compatibility`, `allowed-tools`,
`metadata`). As dicas próprias do Atlas ficam dentro do mapa livre `metadata` do formato e são opcionais:
`atlas-priority` (um inteiro), `atlas-personalities`, `atlas-stacks`, `atlas-files` (separados por vírgula). Nenhum
arquivo proprietário, nenhuma chave proprietária no frontmatter.

**Onde as skills ficam.** `<projeto>/.atlas/skills/` e o `<app data>/skills/` do usuário. Uma skill do projeto sobrepõe
uma skill do usuário de mesmo nome (a sobreposta é listada com um erro e nunca selecionada). As duas pastas são lidas pela
porta `SkillStore`; o adaptador de sistema de arquivos nunca segue um symlink e recusa qualquer caminho que saia da
pasta de uma skill.

**Três níveis.**

1. _Descoberta_: nome, descrição e metadados de cada skill, lidos uma vez e **em cache por skill** até que o arquivo
   dessa skill mude (fingerprint = tamanho e data de modificação, conferidos com um stat, não uma leitura). Nunca
   enviada ao modelo: os runtimes não podem pedir uma skill depois, então uma lista de skills não carregadas seria
   token à toa. O custo que a descoberta evitou é informado (`level1Tokens`).
2. _Ativação_: o corpo do `SKILL.md` de no máximo três skills que a tarefa pede.
3. _Recursos_: arquivos de `references/` cujo nome compartilha palavras com a tarefa, ou que o próprio texto da skill
   linka com palavras que a tarefa compartilha (no máximo dois por skill, 3.000 tokens estimados no total, só texto).
   Scripts e assets nunca são inseridos: os agentes do Atlas rodam com ferramentas somente leitura ou de edição de
   arquivo, sem shell, então um script não tem lugar num prompt.

**A seleção é determinística e explicável**: palavras da tarefa (mais o rótulo e as instruções de um passo) contra o nome
da skill (3 pontos cada) e a descrição (1), as tecnologias do projeto (2), a personalidade (3), um tipo de arquivo
mencionado pela tarefa (2) e a prioridade da skill (um desempate, nunca um motivo por si só). Plurais são unificados;
nada mais esperto que isso. Uma skill precisa de 4 pontos, ou de ser nomeada (`/nome`, `skill nome`, `nome skill`), o
que sempre vale. Cada escolha leva seus motivos ao evento `optimization_skills_selected`. Nenhum modelo julga uma
skill; isso pode vir depois se um dia se mostrar melhor.

**Verificações de qualidade** (`validate.rs`), cada uma com uma mensagem dizendo o que corrigir. Erros mantêm uma skill
fora da seleção (malformada, nome ausente ou inválido, descrição ausente, sobreposta); avisos são informados: nome
diferente da pasta, descrição longa demais / sem um "use quando" / genérica demais, corpo grande demais (mais de 5.000
tokens estimados ou 500 linhas), `allowed-tools` com mais de 12 ferramentas, uma instrução repetida, instruções que
dizem coisas opostas, uma seção vazia, e uma skill que repete três ou mais instruções de outra.

**Prompt.** `Prompt.skills` é uma nova seção, entre o Harness e o contexto do projeto, emoldurada por um aviso:
orientação sem autoridade, abaixo das regras do Atlas e da tarefa. Como tudo o mais, é montada pelo `PromptBuilder`. O que
o runtime pode fazer continua sendo decidido pelo guard de processos, que nunca lê prompts; um teste coloca "ignore
todas as regras" numa skill e confere que a requisição é idêntica no resto.

**Context Engine.** Cada skill selecionada é um `ContextItem` (`Normal` se o Atlas a casou, `High` se a tarefa a
nomeou). Com o engine ligado, uma linha que uma skill repete de outra é descartada, e sob um orçamento uma skill casada é
a primeira coisa deixada de fora (substituída por uma linha que o diz) enquanto uma nomeada é mantida e o estouro é
informado. É o primeiro uso real da prioridade `Normal`.

**Flag.** `optimization.skills.enabled`, **desligada por padrão** (acrescenta texto aos prompts). Desligada, nem se olha o
disco, e o prompt é o que a fase 1 monta.

**Capacidade nativa primeiro.** O Claude Code tem suas próprias skills. Se ele as carrega sob as restrições de
ferramentas do Atlas (`--tools Read,Grep,Glob`) não está verificado, então nenhum runtime é marcado como suportando
skills nativamente e o Atlas injeta o que seleciona. Quando um runtime for verificado, sua flag de capacidade deve
impedir o Atlas de injetar as skills que esse runtime já carrega.

## Medido

Dez skills de cerca de 800 tokens cada, uma casando com a tarefa: carregar o catálogo inteiro seria 8.160 tokens (mais
220 de metadados); a divulgação progressiva envia 808 (cerca de 10%) e nenhum metadado.

## Não construído

- Uma tela ou comando para listar skills e seus problemas (os dados estão nas métricas e na API de descoberta; o
  dashboard é a fase 7).
- Descoberta em `.claude/skills` e `.agents/skills` (aguarda a decisão sobre capacidade nativa acima).
- Seleção assistida por LLM, seleção de recursos por algo além de palavras, e orçamentos de token por skill além do global.
