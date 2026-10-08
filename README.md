# minim

minim est une application desktop (Windows, macOS, Linux) pour automatiser le traitement de documents. Un **déclencheur** identifie un type de document, puis une **chaîne d'actions** s'exécute : renommer, déplacer, convertir, extraire le texte, envoyer par email…

Les documents ne quittent jamais la machine, sauf via une action explicitement configurée par l'utilisateur. L'application fonctionne entièrement hors ligne ; le cloud ne synchronise que des métadonnées (comptes, organisations, définitions de workflows).

## Prérequis

| Outil | Version | Installation |
|---|---|---|
| Rust | fixée par `rust-toolchain.toml` | [rustup](https://rustup.rs), puis `rustup toolchain install` à la racine du repo |
| Node.js | fixée par `.nvmrc` (LTS 24) | [nvm](https://github.com/nvm-sh/nvm), puis `nvm install` à la racine du repo |
| pnpm | fixée par `packageManager` dans `package.json` | `corepack enable` |

Dépendances système de Tauri ([documentation](https://v2.tauri.app/start/prerequisites/)) :

- **Linux (Debian / Ubuntu)** :

  ```bash
  sudo apt install libwebkit2gtk-4.1-dev build-essential curl wget file libxdo-dev libssl-dev libayatana-appindicator3-dev librsvg2-dev
  ```

- **macOS** : Xcode Command Line Tools (`xcode-select --install`).
- **Windows** : [Microsoft C++ Build Tools](https://visualstudio.microsoft.com/visual-cpp-build-tools/) (charge de travail « Développement desktop en C++ ») et WebView2 (préinstallé sur Windows 10 et 11).

## Commandes

| Commande | Description |
|---|---|
| `pnpm install` | Installe les dépendances JS |
| `pnpm dev` | Lance l'application en mode développement (Vite + Tauri) |
| `pnpm build` | Construit l'application et ses installeurs |
| `pnpm lint` | Biome, vérification TypeScript, rustfmt et clippy |
| `pnpm test` | Tests Vitest et `cargo test` |
| `pnpm format` | Formate le code (Biome et rustfmt) |

## Structure

```
apps/
  desktop/        Application Tauri 2 + React + TypeScript
    src/          Interface React
    src-tauri/    Crate Rust de l'application desktop
crates/
  core/           Domaine et moteur de workflows
  extract/        Extraction de texte PDF / DOCX
  shared/         Types partagés entre le desktop et l'API
```
