# Backlog

Idées et travaux identifiés mais hors du périmètre du prompt en cours. Chaque entrée indique son origine.

## Outillage et CI

- **Workflow de release** : build des installeurs sur les 3 OS, signature Windows, signature et notarisation macOS, publication des artefacts. *(prompt 01)*
- **Épingler les GitHub Actions par SHA** plutôt que par tag, pour limiter le risque supply chain (Dependabot sait mettre à jour les SHA). *(prompt 01)*
- **Regrouper les mises à jour Dependabot** (`groups`) pour limiter le nombre de PR. *(prompt 01)*
- **Couverture de tests** : `cargo-llvm-cov` et `@vitest/coverage-v8`, avec publication en CI. *(prompt 01)*
- **cargo-deny** : passer `multiple-versions` de `allow` à `warn` une fois l'arbre de dépendances stabilisé. *(prompt 01)*

## Application desktop

- **Icônes et identité visuelle** : l'app utilise encore les icônes par défaut de Tauri (`apps/desktop/src-tauri/icons/`). *(prompt 01)*
- **Réduire la capability `core:default`** aux seules permissions réellement utilisées par l'UI. *(prompt 01)*
- **Évaluer `app.security.freezePrototype`** (durcissement Tauri contre la pollution de prototype). *(prompt 01)*
