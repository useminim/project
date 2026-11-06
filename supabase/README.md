# Backend Supabase de minim

Le backend est **Supabase hébergé dans une région européenne** (décision D-37). Il gère les comptes et, plus tard, la synchronisation chiffrée des workflows, les organisations et les licences. L'application ne lui parle que depuis Rust (crate `minim-sync`) ; l'interface ne voit jamais les jetons.

## Développement local

Prérequis : Docker (sous Windows : Docker Desktop avec l'intégration WSL activée).

```bash
pnpm backend:start   # base, authentification, API REST, e-mails de test (Mailpit)
pnpm backend:test    # linter de la base, tests de sécurité pgTAP, tests d'attaque contre l'API
pnpm backend:stop
```

`pnpm backend:start` affiche l'adresse de l'API et la clé publique (« anon » ou « publishable »). Pour que l'application les utilise :

```bash
MINIM_SUPABASE_URL=http://127.0.0.1:54321 MINIM_SUPABASE_ANON_KEY=<clé publique> pnpm dev
```

Les e-mails de confirmation et de réinitialisation arrivent dans Mailpit : http://127.0.0.1:54324.

La **clé « service »** (ou « secret ») ne va **jamais** dans l'application ni dans le dépôt : elle ne sert qu'aux tests locaux et aux secrets de la CI.

## Règles pour chaque table (obligatoire)

Une table créée par SQL ou par l'API n'a **pas** le RLS activé par défaut, contrairement à l'interface de Supabase. Une table oubliée est lisible par n'importe qui avec la clé publique de l'application. Donc, dans chaque migration :

1. `alter table … enable row level security;` **et** `alter table … force row level security;`
2. des règles (`create policy`) explicites, avec `(select auth.uid())` ;
3. des `GRANT` explicites (la configuration désactive l'exposition automatique des nouvelles tables), et **rien pour le rôle `anon`** sauf nécessité justifiée ;
4. les fonctions `security definer` fixent `search_path = ''` et retirent le droit d'exécution à `public` et `anon` ;
5. un test dans `supabase/tests/` et, pour une donnée sensible, un test d'attaque dans `crates/sync/tests/live.rs`.

La CI (job « Backend (Supabase) ») échoue si une table exposée n'a pas le RLS activé et forcé, si `anon` a un droit sur une table ou une fonction, ou si un utilisateur lit ou modifie les données d'un autre. Relisez quand même chaque migration à la main : une règle trop large passe les tests si personne ne l'a prévue.

## Préproduction et production

Deux projets Supabase distincts, région **Europe (Paris, `eu-west-3`)**, dans une organisation partagée par l'équipe. Mise en place (une fois par projet) :

1. Créer l'organisation et le projet sur https://supabase.com/dashboard, région Europe (Paris). Relire et accepter l'accord de traitement des données (DPA) et la liste des sous-traitants de Supabase.
2. Lier le dépôt au projet et appliquer les migrations :

   ```bash
   pnpm exec supabase login
   pnpm exec supabase link --project-ref <identifiant du projet>
   pnpm exec supabase db push
   ```

3. Authentification (Dashboard → Authentication) : confirmation de l'e-mail activée, longueur minimale du mot de passe 8 avec lettres et chiffres, inscription anonyme désactivée, protection contre les mots de passe divulgués activée (offre Pro). Adresses de redirection : celles de la page de confirmation de minim, quand elle existera.
4. **E-mails par Brevo** (France, serveurs dans l'UE) : créer un compte Brevo, authentifier le domaine d'envoi (SPF, DKIM, DMARC), créer une clé SMTP, puis dans Dashboard → Authentication → SMTP Settings : hôte `smtp-relay.brevo.com`, port `587`, identifiant et clé SMTP de Brevo, expéditeur `minim <no-reply@…>`. Le serveur d'e-mails intégré de Supabase n'est fait que pour les tests.
5. Lancer les **Security Advisors** du projet (Dashboard → Advisors) après chaque déploiement : aucune alerte de sécurité ne doit rester, sauf celle-ci, voulue : `delete_my_account()` est exécutable par `authenticated` (chaque utilisateur doit pouvoir supprimer son propre compte, et seulement le sien). Si l'option « Enable automatic RLS » a été cochée à la création du projet, la migration `lock_rls_auto_enable` retire l'accès par l'API à la fonction qu'elle ajoute.
6. Donner à l'équipe l'adresse du projet et sa clé publique ; elles sont compilées dans les versions publiées (`MINIM_SUPABASE_URL`, `MINIM_SUPABASE_ANON_KEY`).

Le déploiement est manuel pour l'instant (`supabase db push` vers la préproduction après chaque merge qui touche `supabase/migrations`) ; son automatisation par la CI viendra ensuite.

## Données de santé

Les comptes qui déclarent une profession de santé à l'inscription (`profiles.health_profession`) n'auront pas de synchronisation tant qu'il n'existe pas d'hébergement certifié HDS (D-37). Leurs workflows restent sur leur ordinateur.
