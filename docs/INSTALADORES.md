# Instaladores e assinatura de código

Antivírus, SmartScreen (Windows) e Gatekeeper (macOS) desconfiam de executáveis **sem assinatura**
vindos de um editor desconhecido. Isso é reputação, não necessariamente vírus. A única correção
definitiva é assinar os builds com um certificado em nome do publicador.

## O que o workflow já faz

- Metadados do instalador preenchidos (publisher, copyright, descrição, categoria).
- Windows: instalador NSIS por usuário (não pede administrador) além do `.msi`.
- macOS: assinatura ad-hoc + hardened runtime por padrão; assinatura real + notarização se houver secrets.
- Windows: assinatura se houver secrets.
- `SHA256SUMS.txt` anexado a cada release, para o usuário conferir o download.

## Para eliminar os avisos de vez (exige conta/certificado pagos)

### macOS (Apple Developer Program, US$ 99/ano)

1. Crie um certificado **Developer ID Application** e exporte como `.p12`.
2. Gere uma _app-specific password_ em appleid.apple.com.
3. Secrets do repositório:
   - `APPLE_CERTIFICATE`: `base64 -i cert.p12 | pbcopy`
   - `APPLE_CERTIFICATE_PASSWORD`, `APPLE_SIGNING_IDENTITY` (ex.: `Developer ID Application: Nome (TEAMID)`)
   - `APPLE_ID`, `APPLE_PASSWORD` (app-specific), `APPLE_TEAM_ID`

### Windows

- Certificado de code signing (OV ou EV) exportado como `.pfx`:
  - `WINDOWS_CERTIFICATE`: `base64 -w0 cert.pfx`
  - `WINDOWS_CERTIFICATE_PASSWORD`
- Certificados EV removem o aviso do SmartScreen imediatamente; OV precisa acumular reputação.
- Alternativa mais barata: Azure Trusted Signing (exige adaptar o `signCommand`).

### Linux

Não há aviso de sistema; use o `SHA256SUMS.txt` para verificar.

## Falso positivo de antivírus

Mesmo assinado, um app novo pode ser sinalizado. Envie o instalador como falso positivo ao fornecedor
(Microsoft: https://www.microsoft.com/wdsi/filesubmission) e mantenha o código aberto e os builds
reprodutíveis pelo GitHub Actions.
