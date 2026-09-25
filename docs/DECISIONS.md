# Journal des décisions

Chaque entrée consigne une décision structurante : son contexte, ce qui a été décidé et ses conséquences. Les entrées ne sont pas réécrites ; une décision remise en cause fait l'objet d'une nouvelle entrée qui la remplace.

---

## D-001 — Monorepo pnpm + Cargo, nommage des crates

- **Date** : 2026-09-24
- **Contexte** : le produit comprend une app desktop, un backend (à venir) et des crates Rust partagées.
- **Décision** :
  - Workspace pnpm (`apps/*`) et workspace Cargo (`apps/desktop/src-tauri`, `crates/*` ; `apps/api` sera ajouté plus tard).
  - Les crates sont nommées `minim-core`, `minim-extract` et `minim-shared` (dossiers `crates/core`, `crates/extract`, `crates/shared`). Un package nommé `core` entrerait en conflit avec la crate `core` de la bibliothèque standard Rust.
  - Le crate Tauri s'appelle `minim-desktop` (bibliothèque `minim_desktop_lib`) et le package JS `@minim/desktop`.
  - Les métadonnées communes (`version`, `edition = "2024"`, `rust-version`, `publish = false`) et les dépendances sont centralisées dans le `Cargo.toml` racine (`[workspace.package]`, `[workspace.dependencies]`).
- **Conséquences** : un seul `Cargo.lock` et un seul dossier `target/` à la racine ; un seul `pnpm-lock.yaml`. Comme `tauri` est déclaré dans `[workspace.dependencies]`, la CLI Tauri ne met plus à jour automatiquement ses features : si une option de `tauri.conf.json` exige une feature (ex. `tray-icon`), il faut l'ajouter à la main dans le `Cargo.toml` racine.

## D-002 — Identifiant d'application `com.useminim.desktop`

- **Date** : 2026-09-24
- **Contexte** : l'identifiant proposé initialement, `com.minim.app`, se termine par `.app`, ce qui entre en conflit avec l'extension des bundles macOS (la CLI Tauri émet un avertissement). L'identifiant doit correspondre à un domaine que nous contrôlons.
- **Décision** : `com.useminim.desktop` (domaine `useminim.com`). `productName` = `minim`, titre de fenêtre = `minim`.
- **Conséquences** : cet identifiant est quasi définitif. Il détermine le dossier de données de l'application, les entrées du trousseau de l'OS et le bundle ID utilisé pour la signature macOS. Le changer plus tard imposerait une migration des données utilisateur.

## D-003 — Application desktop uniquement

- **Date** : 2026-09-24
- **Contexte** : le template Tauri prévoit les cibles mobiles (`crate-type` `staticlib`/`cdylib`, `mobile_entry_point`, `TAURI_DEV_HOST` dans Vite).
- **Décision** : cibles Windows, macOS et Linux uniquement. Suppression des éléments propres au mobile.
- **Conséquences** : builds plus rapides (en release avec LTO, chaque type de crate supplémentaire allonge l'édition de liens). Un support mobile nécessiterait de les réintroduire.

## D-004 — Versions d'outils figées

- **Date** : 2026-09-24
- **Décision** :
  - Rust `1.98.1` via `rust-toolchain.toml` (avec `rustfmt` et `clippy`).
  - Node `24` (LTS) via `.nvmrc`, avec `engines.node >= 24`.
  - pnpm `12.6.0` via le champ `packageManager` (utilisé par corepack et par la CI).
- **Conséquences** : les mêmes versions en local et en CI. Une montée de version se fait par un changement explicite de ces fichiers.

## D-005 — Lints et formatage Rust

- **Date** : 2026-09-24
- **Décision** :
  - `[workspace.lints]` : `clippy::unwrap_used`, `clippy::expect_used` et `clippy::panic` en `deny` ; `unsafe_code` en `forbid`. Chaque crate hérite de ces lints (`[lints] workspace = true`).
  - `clippy.toml` autorise `unwrap`, `expect` et `panic` dans les tests (`allow-*-in-tests`).
  - `pnpm lint` exécute clippy avec `-D warnings` : tout avertissement fait échouer le lint.
  - `rustfmt.toml` n'utilise que des options stables (pas besoin de nightly).
- **Conséquences** : `run()` du crate desktop renvoie un `tauri::Result` au lieu d'appeler `.expect()` comme le template.

## D-006 — Profil release

- **Date** : 2026-09-24
- **Décision** : `lto = true`, `codegen-units = 1`, `strip = true`, `opt-level = 3`. **Pas de `panic = "abort"`** (présent dans le template Tauri).
- **Justification** : `opt-level = 3` plutôt que `"s"`, car l'extraction de texte et la conversion de documents privilégient la vitesse d'exécution. Le déroulement de pile (unwinding) permet d'isoler une tâche qui panique, par exemple un parseur tiers sur un document malformé, sans arrêter toute l'application.
- **Conséquences** : binaire légèrement plus gros qu'avec `panic = "abort"`.

## D-007 — Sécurité Tauri par défaut

- **Date** : 2026-09-24
- **Décision** :
  - CSP stricte à la place de `csp: null` : `default-src 'self'`, `connect-src ipc: http://ipc.localhost`, `img-src 'self' data:`, `object-src 'none'`, `base-uri 'self'`, `form-action 'none'`, `frame-ancestors 'none'`.
  - Suppression du plugin `opener` et de sa permission : la capability `default` ne contient que `core:default`.
- **Conséquences** : tout nouveau plugin ou toute nouvelle origine (réseau, images, polices) doit être ajouté explicitement à la CSP ou aux capabilities.

## D-008 — Source unique de la version

- **Date** : 2026-09-24
- **Décision** : `tauri.conf.json` ne définit pas de `version` ; Tauri reprend celle du crate, héritée de `[workspace.package]`.
- **Conséquences** : la version de l'application se modifie à un seul endroit (le `Cargo.toml` racine).

## D-009 — TypeScript strict et outillage JS

- **Date** : 2026-09-24
- **Décision** :
  - `tsconfig.base.json` à la racine, partagé par les packages TS : `strict`, `noUncheckedIndexedAccess`, `noImplicitOverride`, `noUnusedLocals`, `noUnusedParameters`, `noFallthroughCasesInSwitch`, `noUncheckedSideEffectImports`, `verbatimModuleSyntax`.
  - L'app desktop utilise des références de projet (`tsconfig.app.json` pour `src/`, `tsconfig.node.json` pour `vite.config.ts`) vérifiées par `tsc -b`.
- **Nouvelles dépendances** :
  - `@biomejs/biome` (dev, racine, version exacte) : lint, formatage et tri des imports en un seul outil rapide, à la place du couple ESLint + Prettier. La version est figée car la sortie du formateur peut changer d'une version à l'autre.
  - `vitest` (dev, `@minim/desktop`) : lanceur de tests natif Vite, réutilise la configuration de build.
- **Conséquences** : le test de fumée de l'UI utilise `react-dom/server` (`renderToString`) dans l'environnement Node de Vitest. L'ajout d'un DOM simulé (jsdom ou happy-dom) et de Testing Library se fera quand l'UI aura de l'interactivité.

## D-010 — Politique de dépendances Rust (cargo-deny)

- **Date** : 2026-09-24
- **Décision** : `deny.toml` vérifié en CI (`cargo deny check`) :
  - **Licences** : minim est un logiciel propriétaire. Seules les licences permissives sont autorisées (MIT, Apache-2.0, BSD-2/3-Clause, ISC, Zlib, 0BSD, BSL-1.0, CC0-1.0, Unlicense, Unicode-3.0, CDLA-Permissive-2.0), plus MPL-2.0 (copyleft au niveau fichier, compatible avec une distribution propriétaire tant que les fichiers MPL ne sont pas modifiés). GPL, LGPL et AGPL sont refusées.
  - **Advisories** : vulnérabilités et crates « unsound » bloquantes ; crates non maintenues signalées uniquement pour les dépendances directes du workspace (`unmaintained = "workspace"`), la pile GTK de Tauri sous Linux étant hors de notre contrôle.
  - **Sources** : crates.io uniquement ; aucun registre ni dépôt git inconnu.
  - **Doublons** : autorisés pour l'instant (arbre Tauri).
- **Conséquences** : toute nouvelle dépendance sous une licence non listée fait échouer la CI et demande une décision explicite. Toute exception d'advisory est ajoutée à `ignore` avec sa justification.

## D-011 — Fins de ligne LF

- **Date** : 2026-09-24
- **Contexte** : sur les runners Windows, git convertit par défaut les fichiers en CRLF au checkout, ce qui fait échouer Biome et rustfmt.
- **Décision** : `.gitattributes` avec `* text=auto eol=lf` ; `.editorconfig`, Biome et rustfmt configurés en LF.

## D-012 — Intégration continue

- **Date** : 2026-09-24
- **Décision** : le workflow existant `.github/workflows/ci.yml` est adapté (et non dupliqué) :
  - Un job `check` en matrice ubuntu / windows / macos (`fail-fast: false`) : `pnpm lint`, `pnpm test`, puis build du frontend.
  - Versions lues depuis le repo : pnpm (`packageManager`), Node (`.nvmrc`), Rust (`rustup toolchain install` lit `rust-toolchain.toml`).
  - Cache pnpm (`actions/setup-node`) et Cargo (`Swatinem/rust-cache`).
  - Dépendances système Tauri installées sur Linux.
  - Un job `cargo-deny` sur ubuntu (installation via `taiki-e/install-action`).
  - Actions mises à jour (`actions/checkout@v7`, `actions/setup-node@v7`, `pnpm/action-setup@v6`), ce qui rend caduques les PR Dependabot correspondantes.
  - `dependabot.yml` : l'écosystème cargo pointe sur la racine (`/`) au lieu de `/src-tauri`.

## D-013 — Scripts racine

- **Date** : 2026-09-24
- **Décision** :
  - `pnpm dev` / `pnpm build` : `tauri dev` / `tauri build` de `@minim/desktop`.
  - `pnpm lint` : `biome check`, `tsc -b` (via `typecheck` de chaque package), `cargo fmt --check`, `cargo clippy --workspace --all-targets --locked -D warnings`.
  - `pnpm test` : `vitest run` (via `test` de chaque package) puis `cargo test --workspace --locked`.
  - `pnpm format` : `biome check --write` puis `cargo fmt --all`.
- **Conséquences** : la CI appelle exactement les mêmes commandes qu'en local.
