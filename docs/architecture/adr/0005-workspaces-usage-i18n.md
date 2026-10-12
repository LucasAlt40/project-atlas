# ADR 0005 — Workspaces, uso e localização

**Status:** aceito

## Workspace e contexto do projeto

Um **workspace** é a unidade de trabalho: uma pasta de projeto mais os agentes colocados nela e suas conversas. Antes
desta decisão o app tinha um único layout em memória e um projeto fixo no código. Agora `Workspace { id, name,
projectPath, description, createdAt, updatedAt, layout }` é persistido, e podem existir vários.

Agentes, personalidades e runtimes instalados continuam **globais** (um agente pode ser colocado em vários workspaces,
então sua configuração é reaproveitada em vez de duplicada). Posicionamento, conversas, execuções e uso são
**por workspace** e indexados por workspace + agente. O layout 2 × 2 e o limite de quatro agentes exibidos são dados
de UI/layout, não regras de domínio.

A pasta do projeto é o limite de contexto. Escolhê-la registra um caminho e nada mais; a única coisa lida dela é a
lista de nomes de arquivos do nível superior (e os nomes das dependências do `package.json`) para nomear as
tecnologias, de forma local e determinística. Permissões e sandbox ficam para um marco posterior.

A persistência reaproveita o `config.json` atrás da porta `ConfigStore` existente (workspaces, configurações, registro
de uso). Um banco de dados deliberadamente não é introduzido ainda; a porta mantém a opção aberta. Segredos nunca são
guardados ali.

## Uso é autoritativo ou indisponível

Todo número é o que um runtime informou para uma execução, uma soma desses valores sobre execuções que o Atlas
observou (`atlas_calculated`), ou uma cota informada pelo provedor. Um número que o runtime não informou é `null`,
nunca `0`, e as somas dizem quantas execuções cobrem. Moedas não são misturadas. Os runtimes anunciam `usageMetrics`,
`costMetrics` e `quotaMetrics` como capacidades, então a interface diz "não disponível para este runtime" em vez de
adivinhar. O uso registrado pelo Atlas não é a cobrança do provedor, e a interface deixa isso claro.

## Localização

A interface é traduzida no frontend (pt-BR por padrão, en-US), com o idioma persistido como configuração global. O
núcleo nunca produz texto para o usuário: ele devolve códigos estáveis (`AppError.code`, `failureKind`, `notice` do
runtime, `modelHint`, tipos de evento) e a interface os redige. Adicionar um idioma é um novo dicionário mais uma
entrada em `LANGUAGES`.
