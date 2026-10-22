// Télécharge PDFium (lecture et rendu des PDF, D-34) pour l'OS et
// l'architecture de cette machine, et le place dans les ressources du bundle
// Tauri : apps/desktop/src-tauri/resources/pdfium/.
//
// Les binaires viennent de bblanchon/pdfium-binaries, version figée
// ci-dessous. Chaque archive est vérifiée par son empreinte SHA-256 (D-33),
// publiée par GitHub avec la version ; une archive qui ne correspond pas est
// refusée. Rien n'est refait si la bonne version est déjà en place.
//
// Pour une autre cible que la machine : MINIM_PDFIUM_TARGET=win-x64 (etc.).

import { spawnSync } from "node:child_process";
import { createHash } from "node:crypto";
import { cpSync, existsSync, mkdirSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

const version = "chromium/8111";

const targets = {
  "linux-x64": {
    sha256: "bdc60126399f39fe768566a18d4be055a7b3681bc9e248e8a6656b4f6cae0aa2",
    library: "lib/libpdfium.so",
  },
  "linux-arm64": {
    sha256: "042de247138f96341534cccf24ed21c2550a7245b936bb4a60d2340120000f05",
    library: "lib/libpdfium.so",
  },
  "mac-univ": {
    sha256: "043638ac3d7830ff43a7cdbf0623e6b73d70d5cbff9e12e9531d5be7e03b8e9d",
    library: "lib/libpdfium.dylib",
  },
  "win-x64": {
    sha256: "3452d11031050ec65e57f180215b57b9e6d465e3fbedafa8a77ad90885c7d23e",
    library: "bin/pdfium.dll",
  },
  "win-arm64": {
    sha256: "99ec8b76d5ac30602a8488da745397393ff6cddc824ef36f398146f6b4f76d4e",
    library: "bin/pdfium.dll",
  },
};

function hostTarget() {
  const arch = process.arch === "arm64" ? "arm64" : "x64";
  if (process.platform === "darwin") return "mac-univ";
  if (process.platform === "win32") return `win-${arch}`;
  return `linux-${arch}`;
}

const root = join(dirname(fileURLToPath(import.meta.url)), "..");
const destination = join(root, "apps", "desktop", "src-tauri", "resources", "pdfium");
const targetName = process.env.MINIM_PDFIUM_TARGET ?? hostTarget();
const target = targets[targetName];
if (!target) {
  console.error(`Cible PDFium inconnue : ${targetName}`);
  process.exit(1);
}

const stamp = `${version} ${targetName} ${target.sha256}\n`;
const stampFile = join(destination, "VERSION");
if (existsSync(stampFile) && readFileSync(stampFile, "utf8") === stamp) {
  process.exit(0);
}

const asset = `pdfium-${targetName}.tgz`;
const url = `https://github.com/bblanchon/pdfium-binaries/releases/download/${version}/${asset}`;
const response = await fetch(url);
if (!response.ok) {
  console.error(`Téléchargement de PDFium impossible (${response.status}) : ${url}`);
  process.exit(1);
}
const archive = Buffer.from(await response.arrayBuffer());
const digest = createHash("sha256").update(archive).digest("hex");
if (digest !== target.sha256) {
  console.error(`Empreinte de ${asset} incorrecte : ${digest} au lieu de ${target.sha256}.`);
  process.exit(1);
}

const work = join(tmpdir(), `minim-pdfium-${process.pid}`);
rmSync(work, { recursive: true, force: true });
mkdirSync(work, { recursive: true });
const archivePath = join(work, asset);
writeFileSync(archivePath, archive);
// `tar` est fourni par Windows 10 et suivants, macOS et Linux.
const extracted = spawnSync("tar", ["-xzf", archivePath, "-C", work], { stdio: "inherit" });
if (extracted.status !== 0) {
  console.error("Extraction de l'archive PDFium impossible.");
  process.exit(1);
}

rmSync(destination, { recursive: true, force: true });
mkdirSync(join(destination, "licenses"), { recursive: true });
cpSync(join(work, target.library), join(destination, target.library.split("/").pop()));
cpSync(join(work, "LICENSE"), join(destination, "licenses", "pdfium-binaries.txt"));
cpSync(join(work, "licenses"), join(destination, "licenses"), { recursive: true });
writeFileSync(stampFile, stamp);
rmSync(work, { recursive: true, force: true });
console.log(`PDFium ${version} (${targetName}) installé dans ${destination}`);
