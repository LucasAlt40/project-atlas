# P0 — Regras para todo prompt do Stitch (versão 2)

Anexar **inteiro** no começo de cada prompt, trocando `<NOME DA TELA>`. Mudanças em relação à
versão 1: a moldura agora é descrita por completo e há uma lista do que **não** desenhar (o Stitch
inventou esses itens no módulo 1), e a repaginação pede componentes já existentes.

```text
Nome da tela: <NOME DA TELA>

App desktop "Atlas", workspace de agentes de IA para engenharia de software. Tema escuro único,
denso, de ferramenta técnica. Superfícies em camadas de tom (#0b0d10, #101317, #15191e, #1c2128,
#242c37) separadas por borda de 1px, sem sombras (só menus e painéis sobrepostos têm sombra).
Acento índigo #6366f1; estados: sucesso #10b981, aviso #f59e0b, erro #ef4444, executando #60a5fa,
info #38bdf8. Geist para texto, JetBrains Mono para rótulos técnicos, números e caminhos. Raios
4/6/8px. Todo texto em português do Brasil. Janela de 1440×900 (a janela real vai de 720 a 2560).

MOLDURA FIXA (igual em todas as telas, não redesenhe e não acrescente nada):
- Barra lateral de 216px: logo Atlas; seletor de workspace (nome e caminho); itens Workspace,
  Workflow, Agentes, Personalidades, Specs, MCP Hub, Configurações; rodapé com "Project Atlas" e a
  versão. O item da tela atual fica destacado.
- Barra superior de 48px: botão de recolher a barra lateral, breadcrumb "<workspace> / <tela>", e à
  direita o botão "Celular" e a alternância PT | EN.
- NÃO desenhe: busca ou atalho ⌘K, botão "Atlas CLI", ícones de terminal, notificações ou ajustes na
  barra superior, avatar ou e-mail, "Sync OK", botão "Novo Fluxo", telemetria (cluster, latência,
  RAM, região), barra de status inferior, número de versão inventado.

TIPO DE TRABALHO: é uma REPAGINAÇÃO, não um redesenho do zero. Mantenha a identidade atual (tema
escuro, tokens, fontes, tom técnico). O objetivo é facilidade de acesso e de leitura: hoje o Atlas
mostra informação demais ao mesmo tempo. Toda a informação descrita é importante e deve continuar
acessível, então NÃO remova dados: SEPARE melhor. Use hierarquia clara (o essencial em destaque, o
resto em segundo plano), agrupe o que é do mesmo assunto sob um título, divida o que é longo em
seções recolhíveis, abas ou etapas, deixe respiro entre grupos, limite cada linha a uma ideia e
mostre detalhes secundários só quando o usuário pedir. Ações frequentes ficam à vista; ações raras
ou destrutivas vão para um menu "⋯" e ficam separadas das seguras (em vermelho). Prefira rótulos de
texto a ícones sem legenda. Use um único sistema de selos (pequenos, com ponto de estado e cor
semântica) em vez de formatos diferentes. Cada ação aparece UMA vez (sem botão repetido no cartão e
no rodapé).

USE SÓ O QUE ESTÁ DESCRITO: não invente dados, métricas, contadores, botões, filtros, buscas,
atalhos de teclado, selos, etapas nem textos de status que não estejam descritos abaixo. Se faltar
informação para preencher um espaço, deixe o espaço vazio ou use um estado vazio simples.

NOME DA TELA: dê um nome curto, claro e em português que diga o que a tela é e para que serve. Use o
mesmo nome no título, no breadcrumb e nos botões que levam a ela. Estados diferentes da mesma tela
levam o mesmo nome seguido do estado ("<tela> — Sem dados").
```
