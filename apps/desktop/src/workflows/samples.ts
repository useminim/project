// Workflows d'exemple, proposés seulement en développement tant que le wizard
// (prompt 16) n'existe pas, pour essayer l'écran Workflows.

import type { WorkflowDraft } from "../bindings/WorkflowDraft";

const documents = navigator.userAgent.includes("Windows")
  ? "C:\\Users\\Public\\Documents"
  : "/tmp/minim-exemples";

export function sampleDrafts(): WorkflowDraft[] {
  return [
    {
      name: "Factures fournisseurs",
      enabled: true,
      continue_on_match: false,
      trigger: {
        match: "all",
        conditions: [
          { type: "file_name_contains", value: "facture", case_sensitive: false },
          { type: "file_extension_is", values: ["pdf"] },
        ],
        source: "manual",
      },
      actions: [
        { type: "rename", template: "{date_in_name:YYYY-MM-DD} Facture.{extension}" },
        { type: "move", destination: `${documents}/Comptabilité`, create_folders: true },
      ],
    },
    {
      name: "Scans en images",
      enabled: false,
      continue_on_match: true,
      trigger: {
        match: "any",
        conditions: [
          { type: "file_name_contains", value: "scan", case_sensitive: false },
          { type: "file_name_matches_date" },
        ],
        source: "manual",
      },
      actions: [{ type: "convert", to: "png", dpi: 150 }],
    },
    {
      name: "Copie des contrats",
      enabled: true,
      continue_on_match: false,
      trigger: {
        match: "all",
        conditions: [{ type: "file_name_contains", value: "Contrat", case_sensitive: true }],
        source: "manual",
      },
      actions: [{ type: "copy", destination: `${documents}/Archives` }],
    },
  ];
}
