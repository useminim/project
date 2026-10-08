// Vérifie que les types TypeScript générés par ts-rs sont à jour.
//
// Les fichiers sont supprimés puis régénérés par les tests `export_bindings_*`,
// puis comparés à l'index git : un type modifié, ajouté ou supprimé côté Rust
// sans régénération apparaît comme une différence. Les fichiers régénérés et
// ajoutés à l'index (`git add`) sont acceptés, pour que le lint passe avant le
// commit. Écrit en Node pour fonctionner aussi sous Windows, où pnpm exécute
// les scripts avec `cmd`.

import { execFileSync } from "node:child_process";
import { readdirSync, rmSync } from "node:fs";
import { join } from "node:path";

const bindingsDir = join("apps", "desktop", "src", "bindings");

for (const file of readdirSync(bindingsDir)) {
  if (file.endsWith(".ts")) {
    rmSync(join(bindingsDir, file));
  }
}

execFileSync("cargo", ["test", "--workspace", "--locked", "--quiet", "export_bindings"], {
  stdio: "inherit",
});

// Format `XY chemin` : Y décrit l'arbre de travail par rapport à l'index, `??`
// un fichier non suivi.
const changes = execFileSync("git", ["status", "--porcelain", "--", bindingsDir], {
  encoding: "utf8",
})
  .split("\n")
  .filter((line) => line.length > 1 && line[1] !== " ")
  .join("\n");

if (changes !== "") {
  console.error(
    `Les types TypeScript générés ne sont pas à jour :\n${changes}\n` +
      `Ajoutez à git les fichiers de ${bindingsDir} régénérés par cette commande.`,
  );
  process.exit(1);
}
