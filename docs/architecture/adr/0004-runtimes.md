# ADR 0004 — Provedores, runtimes e modelos

**Status:** aceito

A V0.2 modelava "provedor + modelo". Isso misturava duas coisas: _quem_ fornece uma IA (Anthropic) e _como_ o Atlas a
alcança nesta máquina (hoje a CLI do Claude, depois a API da Anthropic). Agora um agente referencia um **runtime**
(`runtimeId`) e um modelo; o provedor é uma propriedade do runtime, usada só para agrupar na interface.

Um runtime implementa `ModelRuntime` (`info`, `detect`, `list_models`, `execute`) e declara `RuntimeCapabilities`. A
camada de execução enxerga apenas essa trait, então suportar uma nova ferramenta significa adicionar um adaptador,
nunca editar agentes, prompts ou execuções. As diferenças entre ferramentas (sem descoberta de modelos, sem flag de
system prompt, autenticação diferente) são expressas como capacidades e status estruturado, não como ramificações no
código compartilhado.

Os runtimes são donos do seu executável, argumentos, ambiente e diretório de trabalho. A webview só nomeia um
`runtimeId`. Toda execução de processo passa pela porta `ProcessRunner`.

A autenticação é modelada (`AuthKind`, `AuthState`), mas só a sessão já logada da própria ferramenta está
implementada. Chaves de API devem ser guardadas no armazenamento de credenciais do sistema operacional, nunca no
`config.json`.
