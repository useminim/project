// Tests du backend Supabase (D-37), en local comme en CI, une fois
// `pnpm backend:start` lancé :
//   1. linter de la base (avertissements compris) ;
//   2. tests de sécurité en base (pgTAP, supabase/tests) : RLS activé et
//      forcé sur chaque table exposée, aucun droit pour le rôle anon,
//      isolation entre utilisateurs ;
//   3. tests « comme un attaquant » contre l'API (crates/sync/tests/live.rs),
//      avec les clés de l'instance locale.

import { execFileSync } from "node:child_process";

const supabase = (args, options = {}) =>
  execFileSync("pnpm", ["exec", "supabase", ...args], { stdio: "inherit", ...options });

supabase(["db", "lint", "--local", "--level", "warning", "--fail-on", "warning"]);
supabase(["test", "db"]);

const status = JSON.parse(
  execFileSync("pnpm", ["exec", "supabase", "status", "-o", "json"], { encoding: "utf8" }),
);
const env = {
  ...process.env,
  MINIM_SUPABASE_TEST_URL: status.API_URL,
  MINIM_SUPABASE_TEST_ANON_KEY: status.ANON_KEY ?? status.PUBLISHABLE_KEY,
  MINIM_SUPABASE_TEST_SERVICE_KEY: status.SERVICE_ROLE_KEY ?? status.SECRET_KEY,
};
for (const name of [
  "MINIM_SUPABASE_TEST_URL",
  "MINIM_SUPABASE_TEST_ANON_KEY",
  "MINIM_SUPABASE_TEST_SERVICE_KEY",
]) {
  if (!env[name]) {
    console.error(`${name} introuvable dans « supabase status » : Supabase est-il démarré ?`);
    process.exit(1);
  }
}
execFileSync("cargo", ["test", "-p", "minim-sync", "--locked", "--test", "live"], {
  stdio: "inherit",
  env,
});
