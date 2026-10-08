//! Profil de document : le résultat de l'analyse, sur lequel les déclencheurs
//! sont évalués. Un profil reste toujours sur la machine.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::field::FieldName;
use crate::id::DocumentId;

/// Résultat de l'analyse d'un document.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DocumentProfile {
    /// Document analysé.
    pub document_id: DocumentId,
    /// Propriétés du fichier.
    pub file: FileInfo,
    /// Origine du texte analysé.
    pub text_source: TextSource,
    /// Champs détectés, par nom.
    pub fields: BTreeMap<FieldName, DetectedField>,
    /// Le document contient au moins un tableau.
    pub has_tables: bool,
    /// Mots-clés significatifs du document.
    pub keywords: Vec<String>,
}

/// Propriétés du fichier d'un document.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FileInfo {
    /// Nom du fichier, extension comprise.
    pub name: String,
    /// Extension, sans le point.
    pub extension: String,
    /// Taille en octets.
    pub size: u64,
}

/// Origine du texte d'un document.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TextSource {
    /// Texte extrait directement du fichier.
    Native,
    /// Texte obtenu par OCR.
    Ocr,
}

/// Valeur d'un champ détecté dans un document.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DetectedField {
    /// Valeur lue dans le document.
    pub value: String,
    /// Devise, pour un montant (code ISO 4217 : `EUR`…).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub currency: Option<String>,
    /// Confiance de la détection, entre 0 et 1.
    pub confidence: f64,
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    #[test]
    fn reads_the_documented_example() {
        let value: serde_json::Value =
            serde_json::from_str(include_str!("../tests/fixtures/document_profile.json")).unwrap();
        let profile: DocumentProfile = serde_json::from_value(value.clone()).unwrap();

        assert_eq!(profile.text_source, TextSource::Native);
        let total = &profile.fields[&FieldName::new("invoice_total").unwrap()];
        assert_eq!(total.currency.as_deref(), Some("EUR"));
        assert_eq!(serde_json::to_value(&profile).unwrap(), value);
    }

    #[test]
    fn rejects_an_invalid_field_name() {
        let value = json!({
            "document_id": "doc_01J8ZK3V9Q4X7M2N5P6R8T0W1Y",
            "file": { "name": "a.pdf", "extension": "pdf", "size": 1 },
            "text_source": "ocr",
            "fields": { "Client": { "value": "Dupont", "confidence": 0.5 } },
            "has_tables": false,
            "keywords": []
        });
        assert!(serde_json::from_value::<DocumentProfile>(value).is_err());
    }
}
