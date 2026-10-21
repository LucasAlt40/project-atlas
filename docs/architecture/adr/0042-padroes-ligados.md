# ADR 0042 — O que melhora o trabalho vem ligado

Status: implementada em 2026-10-20. Pedido do usuário: "tudo que for otimizar o processo para o usuário e/ou melhorar
tem que estar sempre ligado por padrão; se a pessoa quer desligar, é com ela nas configurações". Complementa a
[ADR 0041](0041-memoria-que-aprende.md), que fez o mesmo com a Memória.

## Decisão

| Recurso                     | Antes      | Agora                    | Por que é seguro ligar                                                                                                                                           |
| --------------------------- | ---------- | ------------------------ | ---------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Memória (ADR 0037/0041)     | desligada  | **Propor** nos 3 escopos | Nada entra sem a pessoa. Uma nota certa evitou a isca em 4 runtimes (0/3 contra 3/3).                                                                            |
| Context Engine (ADR 0021)   | desligado  | **ligado**               | Não perde nenhuma palavra: tira só o que o prompt repete. 4,5% num brief real, de 9 a 22% em briefs repetitivos.                                                 |
| Skills (ADR 0022)           | desligadas | **ligadas**              | Sem skills nada muda. Com skills, só a relevante vai (808 tokens contra 8.160 com dez skills).                                                                   |
| Sandbox do macOS (ADR 0038) | `off`      | **`auto`**               | O trabalho normal foi verificado confinado em quatro CLIs. Onde não dá para confinar, a execução segue e diz isso na atividade (`sandbox:unavailable:<motivo>`). |
| Métricas, guardrails        | ligados    | ligados                  | (sem mudança)                                                                                                                                                    |

**Continuam desligados, de propósito:**

- **Servidor ai-memory (ADR 0040):** depende de um servidor que a pessoa instala e mantém rodando. Ligado sem servidor,
  ele não faz nada além de tentar uma conexão local em toda execução. Compartilhar notas também escreve fora dos dados
  do Atlas.
- **Presença no Discord:** mostra no perfil o que a pessoa faz. É privacidade, não melhoria.
- **Orçamentos** (`contextMaxTokens`, `specsMaxTokens`): são limites que a pessoa escolhe. Um orçamento padrão poderia
  deixar de fora o que o agente precisa.
- **Modo automático da Memória:** escreveu lições falsas que passaram em todas as checagens (M4).

## Como chega a quem já usa o Atlas

`AppSettings.defaults_version` registra até que conjunto de padrões as configurações foram trazidas. A versão atual
é `DEFAULTS_VERSION = 1`. Configurações sem esse número são de antes desta ADR e, ao carregar, `adopt_current_defaults`
faz o seguinte uma única vez:

- liga o Context Engine e as Skills;
- passa o sandbox de `off` para `auto` (um `required` escolhido antes é mantido);
- grava a versão.

Depois disso, Atlas nunca religa o que a pessoa desligou. A Memória usa o marcador próprio `chosen` (ADR 0041).

Limite: quem tinha desligado um desses recursos de propósito antes desta versão não se distingue de quem nunca mexeu,
e ele volta ligado uma vez. Desligar de novo fica gravado.

## Onde a pessoa desliga

**Configurações → Geral**:

- **Otimização das execuções**: Context Engine, Skills, revisão do contexto e métricas (`set_optimization_switches`).
  Antes não havia tela nem comando para esses botões.
- **Sandbox dos runtimes**: o modo.
- **Memória**: os modos por escopo.

## Medição com modelos reais

Em [`spikes/memory-learning-results.md`](../spikes/memory-learning-results.md): 90 execuções reais (Claude haiku e
Codex luna) com os padrões novos, todas completas. Elas acharam um defeito do sandbox `auto`, já corrigido: os heredocs
do zsh iam para `/tmp/zsh` e falhavam, e agora `TMPPREFIX` aponta para a pasta temporária da execução.
