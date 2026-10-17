# Plano de implementação — Repaginação do Celular

Branch `design/celular`, criada a partir de `design/workspace-harness` (ou de `main`, se essa já tiver
sido integrada). Princípios, fases, fonte do visual, relatório e regras de divergência:
[`../_comum/principios-implementacao.md`](../_comum/principios-implementacao.md), com **duas adaptações**:
a página do celular é **mobile-first** (390 px, sem a moldura do desktop) e **não usa** a base
compartilhada do desktop obrigatoriamente (veja §2). Dossiê: [`README.md`](README.md).

## 1. Telas e código

Ids das telas geradas no Stitch (registro e avaliação em `prompts.md`).

| #   | Tela (nome no Stitch)                  | Id da tela nova                    | Código do app (hipótese a confirmar)                                             |
| --- | -------------------------------------- | ---------------------------------- | -------------------------------------------------------------------------------- |
| M1  | Celular — Painel de execuções          | `353aa8a106af44408c2dc337f053d82f` | `src/mobile/Dashboard.tsx`, `Banners.tsx`, `StatusChip.tsx`, `Mobile.module.css` |
| M2  | Celular — Execução esperando resposta  | `068a7930a0ec4643ad87d2ea16a2f904` | `RunScreen.tsx`, `InteractionCard.tsx`                                           |
| M3  | Celular — Execução: etapas e resultado | `35e8c067f9364cdfa829f555d3c89832` | `RunScreen.tsx`, `Timeline.tsx`, `labels.ts`                                     |
| M4  | Celular — Conectar este celular        | `6c341e698dc048c28b6782f9af6cd74c` | `PairScreen.tsx`                                                                 |
| M5a | Celular — Sem conexão                  | `62b2772d85634fd7ba9794c4983b88a2` | `MobileApp.tsx` (fase `unreachable`)                                             |
| M5b | Celular — Sem execuções                | `2c352cadb9e94203b96e2173b81f1a35` | `Dashboard.tsx` (vazio)                                                          |
| D1  | Computador — Acesso pelo celular       | `c86519c324ba40a9b41f4552b9c5527d` | `src/features/remote/MobileAccessDialog.tsx`, `Remote.module.css`                |

O botão "Celular" da barra superior (`MobileAccessButton`) é **moldura**: não muda. Textos em `mobile.*` e
`remote.*` de `pt-BR.ts` e `en-US.ts`; a página do celular escolhe o idioma pelo navegador (`pt` → pt-BR).

## 2. Decisões e diferenças deste módulo

- **A página do celular é outro bundle** (`mobile.html`, entrada `mobile` do Vite), servida pelo próprio
  servidor do app a um navegador na rede local. Ela carrega só o que `src/mobile/main.tsx` importa. **Não
  importe componentes de `src/components/ui` que tragam peso ou dependências do desktop** (por exemplo
  `Modal`, `Icon` com muitos ícones) sem checar o tamanho do bundle; `Badge` e `CollapsibleSection` podem
  ser reaproveitados se forem leves e só usarem tokens CSS. Registrar o tamanho do bundle `mobile` antes e
  depois no relatório (`npm run build`).
- **Barra de ações fixa no pé** na execução esperando resposta (M2): as respostas continuam as mesmas
  (`Aprovar`, `Pedir ajustes`, `Refazer o plano`; ou as do tipo da pergunta) e a lógica de envio de
  `InteractionCard` não muda. Respeitar `env(safe-area-inset-*)` e o teclado virtual (a barra não pode cobrir
  o campo que está sendo digitado).
- **Cartão de execução de 3 linhas** (M1): nada se perde — o que sai do cartão (agente da etapa, "N de M
  etapas") passa para a linha 3 ou para a tela da execução. O agrupamento por urgência e os alertas continuam.
- **Cancelar a execução** separado e com a confirmação atual (M3).
- **Segurança e limites do produto** (ADR 0034): a página não inicia execuções, não concede permissões, não
  aplica código, não mostra terminal. A repaginação não pode introduzir nenhuma dessas ações. Os avisos de
  "isto só no computador" continuam.
- **Diálogo do computador (D1)**: `Modal` largo em duas colunas; "O que saber" em `CollapsibleSection` fechada
  **com todos os pontos intactos** (são avisos de segurança). O seletor de endereço (rede local / VPN) e os
  estados de erro (`remote.state.*`, endereço perdido) continuam.
- **Toque**: alvos ≥ 44 px, sem depender de hover; foco visível.

## 3. Cuidados específicos

- `MobileApp.test.tsx` (26 KB) simula o servidor (`testServer.ts`) e cobre pareamento, respostas, controle
  e reconexão: manter nomes acessíveis; se um botão muda de lugar (barra fixa), ajustar só o caminho.
- O título da aba "(N) Atlas" quando há pergunta esperando continua.
- Atualização por `EventSource`: a repaginação não muda o `useRemote`.
- Conteúdo longo: nome de workflow de 60 caracteres, tarefa de 400, documento de plano de 300 linhas, 30
  execuções, 12 etapas.
- iOS Safari e Chrome Android: `100dvh`, `position: sticky`/`fixed` com teclado, rolagem elástica.

## 4. Fases

| Fase | Entrega                                                                                                                                                                          |
| ---- | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| 0    | Preparação (ver `_comum`), `npm run build` e tamanho do bundle `mobile`; capturas "antes" com `mobile.html` (`?phase=…`, `?runs=…`, `#/run/run-1`) em 390×844, 360×740 e 430×932 |
| 1    | `implementacao/levantamento.md` + exportação do Stitch para `stitch/`                                                                                                            |
| 2    | Base: tokens e componentes leves da página do celular (selo, seção recolhível, barra fixa)                                                                                       |
| 3    | M1 e M5 (painel; sem conexão e vazio)                                                                                                                                            |
| 4    | M2 e M3 (execução; barra de ações; etapas)                                                                                                                                       |
| 5    | M4 (parear)                                                                                                                                                                      |
| 6    | D1 (diálogo do computador)                                                                                                                                                       |
| 7    | Responsividade da página: 320 a 430 px em pé, e 568–844 px em paisagem; estados (erro de resposta, sessão expirada, reconectando, 30 execuções, texto longo)                     |
| 8    | Testes, `npm run check`, tamanho do bundle, relatório                                                                                                                            |

## 5. Captura

`_comum/captura/mobile.html` + `mobile-capture.tsx` para a página do celular (viewport 390×844; usar o
preset "mobile" do navegador); `index.html` com `?remote=off|on|error` para o diálogo do computador. Para
estados de erro, `fetch` com falha ou `EventSource` com `error`, estender o stub em `tmp/`.

## 6. Fora do escopo

Servidor remoto em Rust, protocolo, pareamento, ADR 0034; a barra superior do desktop (botão "Celular");
Rust, DTOs, comandos; os outros módulos.
