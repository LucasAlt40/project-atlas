# 0023 — RTK (Rust Token Killer): avaliado, construído e removido

**Status:** removido. Mantido como registro: o que foi descoberto, para que ninguém precise descobrir de novo.

## Contexto

O [RTK](https://github.com/rtk-ai/rtk) filtra a saída de comandos de desenvolvimento (`git`, testes, builds, `ls`, `grep`,
`tsc`, `cargo`, `npm`, `docker`…) antes que ela chegue a um modelo. A ideia era o Atlas sugeri-lo, mandar o usuário à
documentação do próprio RTK para instalá-lo e usá-lo depois de instalado. Uma primeira versão foi construída e testada
(detecção pelo guard, um guia de instalação em Configurações, um decorador de processo abaixo do guard que rodava os
comandos de agente aprovados por meio do `rtk`), incluindo um teste contra o RTK 0.51 real. Foi então removida, porque
não fazia nada pelo Atlas.

## Por que foi removido

**O RTK filtra a saída de um comando, e nenhum agente do Atlas roda comandos através do Atlas.** Os comandos dos agentes
rodariam dentro do próprio loop de ferramentas de cada runtime, e o Atlas os inicia sem shell (`claude -p --tools
Read,Grep,Glob`, o agente `plan` somente leitura do OpenCode, Gemini e Antigravity headless que negam shell). A camada de
segurança tem um contexto para comandos que um agente pede ao _Atlas_ para executar (`ProcessContext::AgentRequested`),
mas nenhuma funcionalidade emite um comando assim. O `git` que o próprio Atlas executa (worktrees, mudanças) é lido pelo
Atlas e nunca vai para um modelo, então não deve ser filtrado. Resultado: instalar o RTK não economizava nada nos agentes
do Atlas, e um card em Configurações sugerindo-o enganaria, mesmo com um aviso. O alcance comum do RTK (as sessões
próprias do usuário no Claude Code) nada tem a ver com o Atlas.

## O que foi descoberto (continua valendo, útil se isto voltar)

- O `rtk rewrite` documenta saída 0 para "reescrito" e 1 para "sem equivalente"; o RTK 0.51 sai com **3** quando reescreve.
  Leia a resposta pela saída (uma única linha começando com `rtk `), não pelo código de saída.
- A reescrita é impressa como texto simples com **as aspas perdidas**: `rtk rewrite grep "foo bar" .` imprime
  `rtk grep foo bar .`. Não dá para separá-la de volta em argumentos. Monte a lista de argumentos a partir do comando
  original: se o RTK só colocou `rtk` na frente, use os argumentos originais; se mapeou para outro subcomando (`cat f` →
  `rtk read f`), separe só quando nenhum argumento tiver espaço ou sintaxe de shell e todo argumento original ainda
  estiver no fim; caso contrário execute o comando como foi pedido.
- `cargo install rtk` do crates.io instala outra ferramenta (Rust Type Kit). A verificação de que é a certa é `rtk gain`
  (`rtk gain --help` é uma forma segura, somente leitura).
- O RTK grava a saída bruta dos comandos em `~/Library/Application Support/rtk/tee/` e mantém seu próprio banco de
  estatísticas: saída bruta (possivelmente com segredos) vive fora do projeto, fora do alcance do guard.
- O RTK tem telemetria opcional, opt-in. Processos `rtk` iniciados pelo Atlas podem definir `RTK_TELEMETRY_DISABLED=1` sem
  mexer na escolha do usuário.
- Não verificado: se um repositório pode embarcar um "filtro customizado" do RTK que altere a saída vista por um agente, e
  se o RTK preserva o código de saída de todo comando que envolve.
- Métodos de instalação documentados (INSTALL.md): `brew install rtk`, `winget install rtk-ai.rtk`, o script
  `install.sh`, `cargo install --git https://github.com/rtk-ai/rtk`. O `rtk init -g` edita as configurações de agentes do
  usuário; o Atlas nunca o executou.

## O que o traria de volta

Agentes que executam comandos **através do Atlas** (o caminho `AgentRequested`). Isso exige, nesta ordem: uma ferramenta
hospedada pelo Atlas que o runtime de CLI possa chamar (um servidor MCP via stdio ligado a uma execução e ao seu escopo,
não uma porta em escuta); um sandbox de SO para os processos que ela inicia (o ponto de extensão `SandboxProvider`
existe, só `NoSandbox` está implementado); uma pequena lista de permissão no começo (leituras de Git, executores de
teste/lint/typecheck no worktree isolado, sem rede, aprovação para todo o resto); e uma medição, antes do RTK, de
quanta saída de comando os agentes realmente produziriam e se filtrá-la muda seus resultados. Só então filtrar a saída
compensaria o binário a mais e seus pontos cegos.

O desenho usado (um decorador abaixo do guard, só para comandos `AgentRequested`, nunca para os do próprio Atlas) era
sólido e está registrado no histórico desta ADR para reaproveitamento.
