# Memória e notas

A memória do Atlas guarda **lições curtas** de trabalhos anteriores ("o limite de tentativas está em `deploy/limits.cfg`,
não em `config/app.toml`") e as entrega ao agente no começo de uma execução, **como dica e nunca como instrução**.
Decisões: [ADR 0037](../architecture/adr/0037-memory.md) (memória nativa, com a emenda de poda de 2026-10-20) e
[ADR 0040](../architecture/adr/0040-memoria-externa-ai-memory.md) (servidor ai-memory opcional) e
[ADR 0041](../architecture/adr/0041-memoria-que-aprende.md) (ligada por padrão, aprende com você). Números e limites:
[análise](../architecture/spikes/ai-memory-analysis.md).

## Como ligar

**Ela já vem ligada** em **Propor, eu aprovo** nos três escopos: nada entra num prompt sem você ter escrito ou aprovado.
Para mudar, vá em **Configurações → Memória → "Onde as notas são usadas"**; o que você salvar ali é respeitado e nunca
volta ao padrão. Cada escopo tem três modos:

| Modo                  | O que faz                                                                             |
| --------------------- | ------------------------------------------------------------------------------------- |
| **Desligado**         | Nada é lido nem proposto. O prompt é idêntico ao de quem não tem o recurso.           |
| **Propor, eu aprovo** | As notas ativas entram nos prompts; agentes podem **propor** notas, que esperam você. |
| **Automático**        | Como o anterior, mas propostas que passam em todas as checagens entram sozinhas.      |

Os escopos são **Minhas** (todos os projetos), **Workspace** e **Agente**. O modo automático só vale a pena com um
validador independente (ADR 0037) e **nunca** vale para notas de preferência nem para as suas. Com propostas ligadas,
todo prompt leva a regra de proposta no fim (~200 tokens), mesmo quando nenhuma nota combina.

## Como o Atlas aprende com você

Sem modelo extra e sem nada escondido, o workspace aprende com três decisões que você já toma:

| O que você faz                                                       | O que o Atlas aprende                                                                                                                                                                      |
| -------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ |
| **Recusa** uma proposta                                              | Guarda a recusa (aba **Recusadas**). Quando um agente propõe a mesma lição de novo, ela é descartada sem perguntar.                                                                        |
| **Aceita** o trabalho de uma execução (mesclar, ou aplicar um fluxo) | As notas que foram no prompt dela ganham um ponto e passam na frente das outras igualmente relevantes.                                                                                     |
| **Descarta** as mudanças de um fluxo                                 | As notas que foram no prompt perdem dois pontos. Uma nota **de agente** que esteve em 2 trabalhos descartados (e em menos aceitos) sai de uso com o selo **Fora de uso** até você revê-la. |
| **Corrige** o agente no chat ("está errado", "na verdade…", "wrong") | O prompt pede ao agente que registre o que você corrigiu como nota, para você aprovar.                                                                                                     |

Cada nota mostra quantas vezes foi aceita e descartada. Uma nota que **você** escreveu nunca sai de uso por isso: só
cai na ordem. Mudou de ideia sobre uma recusa? **Aprovar mesmo assim**. Quer que a lição possa voltar a ser proposta?
**Esquecer**.

## Como escrever uma nota que funciona

O Atlas escolhe as notas pelas **palavras** da tarefa, e o **título** pesa mais que o conteúdo. Por isso:

1. **Ponha no título as palavras que você usaria ao pedir a tarefa.** "Retry limit lives in deploy/limits.cfg" é achada
   por "mude o retry limit"; "Configuração importante" não é achada por nada.
2. **Uma lição por nota, no máximo 480 caracteres.** O que não cabe é conhecimento do projeto e vai no Harness.
3. **Diga onde está a resposta e o que não fazer.** "A chave `retries` em `config/app.toml` não é lida por nada."
4. **Nunca ponha segredos.** Notas que parecem segredo são recusadas, mas não conte com isso.
5. **Escolha o tipo:** _Armadilha_ (o que dá errado), _Como fazer_, _Decisão_, _Preferência_. Em empate, a armadilha vem
   antes da decisão, e a nota escrita por você vem antes da proposta de um agente.
6. **Escreva na língua em que você pede.** A comparação de palavras ignora plural e flexão em inglês (`retry`/`retries`);
   em português ela é mais grosseira.

**O melhor momento para escrever uma nota é quando você corrige o agente.** Experimento medido: das sessões em que a
pessoa disse "está errado, é `MAXTRY`", todas viraram uma nota certa; das sessões sem correção, nenhuma.

## O que o Atlas faz com as notas (e o que mudou)

Antes de cada execução o Atlas escolhe as notas que combinam com a tarefa. Depois da mudança desta POC:

- **Até 5 notas combinando, ou menos de 12 notas no total: nada muda.** Entram todas as que compartilham uma palavra.
- **Mais que isso, é "inundação"**: só ficam as notas **sobre** a tarefa (o título traz duas palavras dela, ou a nota
  traz uma palavra rara, ou duas incomuns). Palavras que quase toda nota tem ("change", "service") não contam.
- **Tarefa que nada combina de verdade não recebe nota.** O prompt fica como estava.
- O que fica é ordenado por relevância, depois por tipo, origem e **uso** (a nota que já foi para passos que terminaram).

Medido com 240 notas parecidas: 586 → 273 tokens de notas por prompt, de 374 para 32 notas de outro assunto. Em 46 notas
reais do projeto: 375 → 191 tokens por tarefa, ao custo de achar 81% das notas esperadas em vez de 88%. Com um modelo
real, numa tarefa com uma "isca", a mudança errada caiu de 3 em 3 para 0 em 3.

**Para ver o que entrou:** abra a execução → **Inspetor → aba Contexto**. O manifesto lista cada nota (enviada, fora do
orçamento, sem relação) e quanto ela custou.

## Servidor ai-memory (opcional)

Se você já usa o [ai-memory](https://github.com/akitaonrails/ai-memory) com outras ferramentas, o Atlas pode **ler** as
notas dele e **compartilhar** as que você aprova. É opcional, desligado por padrão e só fala com `127.0.0.1`.

1. Tenha o binário `ai-memory` (compile o repositório com `cargo build --release -p ai-memory-cli`) e suba o servidor com
   o ambiente que o Atlas espera (só loopback, sem baixar modelo, sem importar seu histórico):
   `scripts/ai-memory-local.sh --data-dir <pasta> --port 49374 --bin <caminho do ai-memory>`
2. **Configurações → Memória → Servidor de memória externo**: marque "Ler notas do servidor", informe o endereço e a
   **pasta de dados** (o Atlas lê o token de lá; ele nunca fica nas configurações), salve e clique em **Testar conexão**.
3. Marque **"Compartilhar com o servidor as notas que eu aprovar"** se quiser que suas outras ferramentas as vejam, e
   use **Publicar agora** para enviar o que já existe.

Não instale os hooks do ai-memory para isto: eles guardam seus prompts sem criptografia e importam seu histórico.

## Exemplo para testar no Atlas rodando local

Um projeto de brinquedo com uma **isca**: `config/app.toml` tem uma chave `retries` que ninguém lê; o limite de verdade é
`MAXTRY` em `deploy/limits.cfg`. Um agente sem memória costuma editar a isca. O script cria o projeto e 15 notas parecidas
(uma delas com a resposta), no formato do Atlas.

1. **Crie o projeto:**
   `python3 -I scripts/demo-memoria.py project ~/atlas-demo/billing-client`
2. **No Atlas** (`npm run tauri dev`), crie um workspace chamado **Demo memória** apontando para essa pasta, com um agente
   barato (por exemplo OpenCode `big-pickle`, que é gratuito, ou Claude `haiku`). **Feche o Atlas.**
3. **Veja o que o script faria**, e depois grave:
   `python3 -I scripts/demo-memoria.py notes --workspace "Demo memória"`
   `python3 -I scripts/demo-memoria.py notes --workspace "Demo memória" --write`
4. **Abra o Atlas.** Em **Configurações → Memória**, confira as 15 notas na aba **Notas guardadas**.
5. **Primeiro sem memória:** deixe o escopo **Workspace** em **Desligado** e peça ao agente:
   `Change the retry limit of this service from 3 to 5, and tell me briefly what you changed.`
   Veja o que ele alterou (`git -C ~/atlas-demo/billing-client log -p -1`). Com frequência ele edita `config/app.toml`.
6. **Volte o projeto ao início** (o Atlas funde o resultado da execução no projeto):
   `git -C ~/atlas-demo/billing-client reset --hard $(git -C ~/atlas-demo/billing-client rev-list --max-parents=0 HEAD)`
7. **Agora com memória:** coloque **Workspace** em **Propor, eu aprovo**, rode o **mesmo pedido** e abra
   **Inspetor → Contexto**. Você deve ver poucas notas enviadas (a do `Retry limit` entre elas, de ~15), as demais como
   "sem relação", e o agente alterando `deploy/limits.cfg` (`MAXTRY=5`).
8. **Repita 3 vezes cada** para ver a tendência: com um modelo pequeno o resultado varia de execução para execução. Nas
   medições (Claude haiku, 3 rodadas), sem a nota a isca foi tocada em 3 de 3 e com ela em 0 de 3.

**Para testar o ciclo "corrigi o agente → virou nota":** apague a nota certa (aba **Notas guardadas → ⋯ → Apagar**), rode
o pedido, e quando o agente editar a isca diga no chat que está errado. Depois use **Adicionar uma nota** com o título
`Retry limit lives in deploy/limits.cfg` e a lição, e rode o pedido de novo.

**Para limpar tudo:** feche o Atlas e rode `python3 -I scripts/demo-memoria.py clean --workspace "Demo memória" --write`
(só remove o que o script criou); apague o workspace pelo Atlas e a pasta `~/atlas-demo`.

## Limites que você deve conhecer

- A poda só atua com **mais de 5 notas casando e pelo menos 12 notas**. Quem tem poucas notas não vê diferença.
- Uma nota com título genérico, que não cita as palavras da tarefa, pode ser deixada de fora quando há muitas
  concorrendo. Escreva bons títulos.
- Tarefas ditas com outras palavras que as da nota recuperam pouco (19 de 40 no teste); só embeddings melhorariam e não
  ajudaram nos testes.
- As constantes da poda foram ajustadas em conjuntos de teste pequenos (um sintético e 46 notas do próprio projeto). Se
  você usar a memória de verdade, vale repetir a medição com as suas notas (`real_notes_tests`).
- Medido com modelos reais (Claude haiku e Codex): depois que você corrige o agente e aprova a nota certa, a execução
  seguinte deixou de cair na isca (0 de 9 vezes, contra 6 de 9 sem a correção). Mas **metade das notas que um agente
  propõe por conta própria é falsa**: leia antes de aprovar. Detalhes em
  [memory-learning-results.md](../architecture/spikes/memory-learning-results.md).
- O efeito de aceitar ou descartar o trabalho na ordem das notas só foi provado com testes, sem modelo real. Descartar
  é um sinal fraco; por isso só nota de agente sai de uso, e só depois de duas vezes. Mesclagem automática e execuções
  sem worktree não contam.
- O servidor ai-memory só foi exercitado no macOS. A captura automática por hooks dele **não** é recomendada (ver análise).
