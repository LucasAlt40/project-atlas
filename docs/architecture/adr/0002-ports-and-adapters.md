# ADR 0002 — Portas em `application`, adaptadores para fora

**Status:** aceito

O briefing desenha `Commands → Application → Domain → Infrastructure`. Tomado ao pé da letra, o código de
aplicação dependeria de infraestrutura concreta, o que prejudica a testabilidade e a troca entre plataformas. Em vez
disso, `application/` declara traits (portas) e `platform/` / a futura `infrastructure/` as implementam; o `lib.rs`
faz a ligação. A camada do briefing é preservada nas chamadas em tempo de execução, mas as dependências do código-fonte
apontam para dentro.

O diretório `infrastructure/` do layout sugerido ainda não é criado: ficaria vazio. Ele entra junto com o primeiro
adaptador que não seja específico de sistema operacional.

**Atualização (V0.2):** `infrastructure/` agora existe: abriga o executor de processos e o armazenamento de
configuração em JSON, os primeiros adaptadores que não são específicos de SO nem puros.
