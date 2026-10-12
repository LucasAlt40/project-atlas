# 7. Importar e exportar configurações

> Parte do [guia do Atlas](README.md). ADR: [0032](../architecture/adr/0032-configuration-import-export.md).
> Código: `src-tauri/src/application/portability/`, `src-tauri/src/commands/portability.rs`, `src/features/portability/`.

Uma configuração do Atlas (personalidade, agente, workflow, template salvo) vira um **arquivo** (`<nome>.atlas.json`) que outra pessoa, ou
outra instalação, importa sem recriar nada à mão.

## Como usar

| Quero…                         | Onde                                                                               |
| ------------------------------ | ---------------------------------------------------------------------------------- |
| Exportar um agente             | Ícone de upload no cartão do agente (tela **Agentes**)                             |
| Exportar uma personalidade     | Ícone de upload no cartão (tela **Personalidades**)                                |
| Exportar um workflow           | **Exportar** na barra do workflow aberto                                           |
| Exportar um template salvo     | **Exportar**, ao escolher o template na criação de workflow                        |
| Exportar vários / fazer backup | **Configurações → Importar e exportar → Exportar vários…**                         |
| Importar                       | **Importar** nas telas Agentes, Personalidades e Workflow, ou em **Configurações** |

Ao exportar, o diálogo mostra o que o arquivo contém e avisa o que merece conferência; "incluir o que ele precisa" leva junto os agentes e as
personalidades de que um workflow depende (desligado, o arquivo só os cita e quem importa escolhe os seus). Ao importar, o Atlas abre o diálogo
de arquivo do sistema, mostra **tudo o que o arquivo faria aqui** e só altera algo no botão **Importar**.

## O que cada categoria preserva

| Categoria      | Preservado                                                                                                                                                |
| -------------- | --------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Personalidade  | nome, descrição, instruções, comportamento, tags, contrato e perfil sugeridos (e o id, se for embutida)                                                   |
| Agente         | nome, personalidade, runtime (id, nome, provedor), modelo, instruções, perfil de permissão, isolamento em worktree, contrato de resultado                 |
| Workflow       | nome, descrição, modo, etapas (agente, instruções, critério de conclusão, tentativas, rota de falha, só leitura), condições, fins, rotas, laços, posições |
| Template salvo | o mesmo grafo, mais as **sugestões** de ferramentas por etapa (nunca concessões)                                                                          |

Nunca vão no arquivo: ids locais, workspaces e caminhos, histórico, execuções, logs, conversas, conexões MCP e segredos, concessões de
ferramentas, configurações do app. Veja a tabela completa no ADR.

## Conflitos, dependências e modelos

- **Mesmo nome**: cada item tem uma decisão — _importar como novo_ (ganha "(2)" se o nome estiver em uso), _usar o que já tenho_, _substituir_ (só com
  confirmação explícita, item a item) ou _não importar_. Conteúdo idêntico ao que já existe é reaproveitado por padrão; importar o mesmo arquivo duas
  vezes não duplica nada.
- **Dependências ausentes**: o diálogo pergunta qual dos seus agentes/personalidades cada uma deve ser; o de mesmo nome vem sugerido, mas só vale quando você importa.
- **Modelo indisponível**: o diálogo mostra runtime e modelo originais, diz se o modelo existe aqui (e por que não) e oferece alternativas
  prontas. Nada é trocado sozinho: manter o original deixa o agente "a configurar"; um runtime que não existe ou não executa tarefas exige escolher outro.
- **Permissões**: agentes que podem editar arquivos chegam **só com leitura**, a menos que você marque "manter a permissão de editar".

## Segurança

O arquivo é tratado como entrada não confiável (limite de 2 MiB, formato, versão e referências validados antes de olhar qualquer item; as mesmas
regras dos editores valem para cada item). Importar só adiciona definições: não executa nada, não inicia workflow e não concede ferramenta. A
importação é atômica: se qualquer item falhar, nada é gravado. Textos que parecem credenciais geram aviso ao exportar e ao importar.
