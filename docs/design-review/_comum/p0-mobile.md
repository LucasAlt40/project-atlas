# P0 móvel — regras para os prompts do Stitch da página do celular

Variante do [`p0-regras.md`](p0-regras.md) para as telas **da página que abre no navegador do celular**
(módulo `celular/`). A página do celular **não tem a moldura do desktop** (sem barra lateral e sem
barra superior de 48px). Use `deviceType: MOBILE`. O diálogo "Acesso pelo celular" do computador usa o P0
normal, não este.

```text
Nome da tela: <NOME DA TELA>

Página web "Atlas" para o navegador do celular (390×844, em pé), que acompanha execuções de workflows
de agentes de IA que rodam no computador da pessoa e deixa responder o que espera por ela. Tema escuro
único, denso, de ferramenta técnica: superfícies #0b0d10, #101317, #15191e, #1c2128, #242c37 separadas
por borda de 1px, sem sombras. Acento índigo #6366f1; estados: sucesso #10b981, aviso/espera #f59e0b,
erro #ef4444, executando #60a5fa. Geist para texto, JetBrains Mono para números, ids e caminhos. Raios
4/6/8px, botões de 12px de raio. Todo texto em português do Brasil.

ESTRUTURA FIXA (igual em todas as telas, não acrescente nada): barra de topo de 48px com "Atlas" à
esquerda e o nome do celular ("iPhone do Lucas") à direita, e, nas telas internas, o botão "‹ Voltar".
Sem barra lateral, sem abas inferiores, sem menu, sem busca, sem notificações, sem avatar, sem
configurações, sem rodapé com marca.

REGRAS DE CELULAR: um polegar só. Alvos de toque de pelo menos 44px. Uma coluna. Sem rolagem
horizontal. Texto de pelo menos 15px (13px só para metadados). A ação principal da tela fica ao alcance
do polegar (parte de baixo da área visível) e ações destrutivas ficam separadas e pedem confirmação.
Respeite a área segura de cima e de baixo.

TIPO DE TRABALHO: é uma REPAGINAÇÃO, não um redesenho do zero. Mantenha a identidade atual (cores,
fontes, tom). O objetivo é facilidade de acesso e de leitura numa tela pequena: hoje há informação
demais em cada cartão. Toda a informação descrita continua acessível: SEPARE melhor, com hierarquia
(o que pede ação em destaque, o resto em segundo plano), agrupe sob um título, use seções recolhíveis
para o que é longo e deixe respiro. Use um único sistema de selos (pequenos, com ponto de estado e cor
semântica). Cada ação aparece UMA vez.

USE SÓ O QUE ESTÁ DESCRITO: não invente dados, métricas, botões, filtros, notificações, gráficos nem
textos que não estejam descritos abaixo.

NOME DA TELA: dê um nome curto e claro em português que diga o que a tela é e para que serve; use o
mesmo nome no título da tela. Estados diferentes levam o mesmo nome seguido do estado.
```
