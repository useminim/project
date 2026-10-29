import { describe, expect, it } from "vitest";
import type { Action } from "../bindings/Action";
import type { Condition } from "../bindings/Condition";
import { i18n } from "../i18n";
import {
  describeAction,
  describeActions,
  describeCondition,
  describeTrigger,
  folderName,
  readableTemplate,
} from "./summary";

const t = i18n.t.bind(i18n);
// Espaces insécables de la typographie française, rendus lisibles dans les attentes.
const plain = (text: string) => text.replaceAll("\u00a0", " ");

describe("describeCondition", () => {
  const cases: Array<[Condition, string]> = [
    [
      { type: "file_name_contains", value: "facture", case_sensitive: false },
      "le nom contient « facture »",
    ],
    [
      { type: "file_name_contains", value: "URSSAF", case_sensitive: true },
      "le nom contient « URSSAF » (majuscules comprises)",
    ],
    [{ type: "file_name_matches_date" }, "le nom contient une date"],
    [
      { type: "file_name_matches_date", format: "YYYY-MM-DD" },
      "le nom contient une date au format YYYY-MM-DD",
    ],
    [{ type: "file_extension_is", values: ["PDF", "jpg"] }, "l'extension est .pdf ou .jpg"],
    [
      { type: "content_contains_text", value: "SIRET", case_sensitive: false },
      "le contenu contient « SIRET »",
    ],
    [
      { type: "content_has_field", field: "invoice_total" },
      "le document contient le champ « invoice_total »",
    ],
    [{ type: "content_contains_table" }, "le document contient un tableau"],
    [
      { type: "content_contains_table", min_rows: 1 },
      "le document contient un tableau d'au moins 1 ligne",
    ],
    [
      { type: "content_contains_table", min_rows: 3 },
      "le document contient un tableau d'au moins 3 lignes",
    ],
    [
      { type: "similar_to_examples", profile_id: "prf_01J8ZK3V9Q4X7M2N5P6R8T0W1Y", threshold: 0.8 },
      "le document ressemble aux exemples",
    ],
    [
      {
        type: "group",
        match: "any",
        conditions: [
          { type: "file_name_contains", value: "devis", case_sensitive: false },
          { type: "file_name_contains", value: "offre", case_sensitive: false },
        ],
      },
      "(le nom contient « devis » ou le nom contient « offre »)",
    ],
  ];
  it.each(cases)("décrit %j", (condition, expected) => {
    expect(plain(describeCondition(t, condition))).toBe(expected);
  });
});

describe("describeAction", () => {
  const cases: Array<[Action, string]> = [
    [{ type: "rename", template: "{date}.pdf" }, "renommer en « {date}.pdf »"],
    [
      { type: "move", destination: "C:\\Users\\Alice\\Comptabilité\\", create_folders: false },
      "déplacer vers Comptabilité",
    ],
    [{ type: "copy", destination: "/home/alice/Archives" }, "copier vers Archives"],
    [{ type: "convert", to: "pdf" }, "convertir en PDF"],
    [{ type: "convert", to: "png", dpi: 300 }, "convertir en images PNG"],
    [{ type: "convert", to: "jpeg" }, "convertir en images JPEG"],
    [{ type: "extract_fields", fields: ["date"], output: "csv" }, "extraire 1 champ en CSV"],
    [
      { type: "extract_fields", fields: ["date", "invoice_total"], output: "json" },
      "extraire 2 champs en JSON",
    ],
  ];
  it.each(cases)("décrit %j", (action, expected) => {
    expect(plain(describeAction(t, action))).toBe(expected);
  });
});

describe("whole sentences", () => {
  it("joins conditions with the match mode and capitalizes", () => {
    const sentence = describeTrigger(t, {
      match: "all",
      conditions: [
        { type: "file_name_contains", value: "facture", case_sensitive: false },
        { type: "file_extension_is", values: ["pdf"] },
      ],
      source: "manual",
    });
    expect(plain(sentence)).toBe("Le nom contient « facture » et l'extension est .pdf");
  });

  it("chains actions with « puis »", () => {
    const sentence = describeActions(t, [
      { type: "rename", template: "Facture.pdf" },
      { type: "move", destination: "/home/alice/Comptabilité", create_folders: true },
    ]);
    expect(plain(sentence)).toBe("Renommer en « Facture.pdf », puis déplacer vers Comptabilité");
  });

  it("handles empty lists and English", async () => {
    expect(describeTrigger(t, { match: "all", conditions: [], source: "manual" })).toBe(
      "Aucune condition",
    );
    expect(describeActions(t, [])).toBe("Aucune action");
    await i18n.changeLanguage("en");
    expect(
      describeActions(t, [
        { type: "convert", to: "pdf" },
        { type: "copy", destination: "/a/b" },
      ]),
    ).toBe("Convert to PDF, then copy to b");
  });

  it("shows name template variables in plain words", () => {
    expect(readableTemplate(t, "{date_in_name:YYYY-MM-DD}_{original_name}.{extension}")).toBe(
      "[Date dans le nom]_[Nom d'origine].[Extension]",
    );
    expect(readableTemplate(t, "{client}_{today}")).toBe("{client}_[Date du jour]");
  });

  it("keeps the last folder of any path", () => {
    expect(folderName("\\\\serveur\\partage\\Factures")).toBe("Factures");
    expect(folderName("/")).toBe("/");
  });
});
