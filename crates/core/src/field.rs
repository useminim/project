//! Noms des champs extraits d'un document (`client_name`, `invoice_total`…).

use std::fmt;

use serde::{Deserialize, Serialize};
use thiserror::Error;

/// Nom d'un champ : une lettre minuscule ASCII suivie de minuscules, chiffres
/// ou `_`.
///
/// La liste des champs reste ouverte : de nouveaux champs apparaissent avec les
/// capacités d'analyse, sans changement du modèle.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(type = "string"))]
#[serde(try_from = "String", into = "String")]
pub struct FieldName(String);

/// Nom de champ invalide.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
#[error("nom de champ invalide : « {0} »")]
pub struct FieldNameError(pub String);

impl FieldName {
    /// Valide et construit un nom de champ.
    ///
    /// # Erreurs
    ///
    /// Renvoie [`FieldNameError`] si le nom est vide, ne commence pas par une
    /// lettre minuscule ou contient un caractère autre que `a-z`, `0-9` ou `_`.
    pub fn new(name: impl Into<String>) -> Result<Self, FieldNameError> {
        let name = name.into();
        let mut chars = name.chars();
        let valid = chars.next().is_some_and(|c| c.is_ascii_lowercase())
            && chars.all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_');
        if valid {
            Ok(Self(name))
        } else {
            Err(FieldNameError(name))
        }
    }

    /// Renvoie le nom sous forme de texte.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl TryFrom<String> for FieldName {
    type Error = FieldNameError;

    fn try_from(name: String) -> Result<Self, Self::Error> {
        Self::new(name)
    }
}

impl From<FieldName> for String {
    fn from(name: FieldName) -> Self {
        name.0
    }
}

impl fmt::Display for FieldName {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepts_snake_case_names() {
        for name in ["date", "invoice_total", "client_name", "line2"] {
            assert_eq!(FieldName::new(name).unwrap().as_str(), name);
        }
    }

    #[test]
    fn rejects_invalid_names() {
        for name in [
            "",
            "Date",
            "2nd",
            "_total",
            "client-name",
            "nom client",
            "échéance",
        ] {
            assert_eq!(FieldName::new(name), Err(FieldNameError(name.to_owned())));
        }
    }

    #[test]
    fn deserialization_validates_the_name() {
        assert!(serde_json::from_str::<FieldName>("\"client_name\"").is_ok());
        assert!(serde_json::from_str::<FieldName>("\"Client\"").is_err());
    }
}
