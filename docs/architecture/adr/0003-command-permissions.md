# ADR 0003 — Permissões de comando negadas por padrão

**Status:** aceito

Por padrão, o Tauri deixa a webview chamar qualquer `#[tauri::command]`. O Atlas, em vez disso, declara seus comandos
no `build.rs` (`AppManifest`), o que faz cada um exigir uma permissão `allow-<nome>` concedida em `capabilities/`. Isso
mantém a invariante de que adicionar um comando privilegiado é uma mudança explícita e revisável, o que importa
quando agentes podem executar código na máquina do usuário.
