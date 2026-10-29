//! Aperçu d'un modèle de nom sur un nom de fichier d'exemple, pour le wizard :
//! même rendu et même assainissement que l'action `rename`.

use chrono::NaiveDate;
use serde::Serialize;

use crate::actions::render::{RenderContext, RenderError, render};
use crate::actions::sanitize::sanitize_file_name;
use crate::matching::FileFacts;
use crate::template::Template;

/// Résultat de l'aperçu.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum NamePreview {
    /// Nom que recevrait le fichier.
    Name {
        /// Nom produit, assaini.
        name: String,
    },
    /// Le modèle est mal écrit (accolade non fermée, variable vide…).
    InvalidTemplate,
    /// Une variable n'a pas de valeur pour ce fichier (aucune date dans le
    /// nom, pas d'extension…).
    MissingVariable {
        /// Nom de la variable.
        variable: String,
    },
    /// Un format a été donné à une variable qui n'est pas une date, ou le
    /// format de date est invalide.
    InvalidFormat {
        /// Nom de la variable.
        variable: String,
    },
    /// Le nom produit est vide une fois assaini.
    EmptyName,
}

/// Nom que recevrait un fichier nommé `sample` avec le modèle `template`. La
/// date du fichier est supposée être `today`.
#[must_use]
pub fn preview_file_name(template: &str, sample: &str, today: NaiveDate) -> NamePreview {
    let Ok(template) = Template::parse(template) else {
        return NamePreview::InvalidTemplate;
    };
    let file = FileFacts::new(sample, 0);
    let context = RenderContext {
        file: &file,
        file_date: Some(today),
        today,
    };
    match render(&template, &context) {
        Ok(name) => sanitize_file_name(&name)
            .map_or(NamePreview::EmptyName, |name| NamePreview::Name { name }),
        Err(RenderError::MissingVariable { name }) => {
            NamePreview::MissingVariable { variable: name }
        }
        Err(RenderError::UnexpectedFormat { name } | RenderError::InvalidDateFormat { name }) => {
            NamePreview::InvalidFormat { variable: name }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn preview(template: &str, sample: &str) -> NamePreview {
        preview_file_name(
            template,
            sample,
            NaiveDate::from_ymd_opt(2026, 10, 30).unwrap(),
        )
    }

    fn name(value: &str) -> NamePreview {
        NamePreview::Name {
            name: value.to_owned(),
        }
    }

    #[test]
    fn previews_a_name_like_the_rename_action() {
        assert_eq!(
            preview(
                "{date_in_name:YYYY-MM-DD}_{original_name}.{extension}",
                "Facture 15.03.2026.pdf"
            ),
            name("2026-03-15_Facture 15.03.2026.pdf")
        );
        assert_eq!(
            preview("{today:DD-MM-YYYY} scan", "x.pdf"),
            name("30-10-2026 scan")
        );
        assert_eq!(
            preview("Devis: client/2026", "x.pdf"),
            name("Devis_ client_2026")
        );
    }

    #[test]
    fn explains_why_there_is_no_name() {
        assert_eq!(preview("{", "x.pdf"), NamePreview::InvalidTemplate);
        assert_eq!(
            preview("{date_in_name}", "sans date.pdf"),
            NamePreview::MissingVariable {
                variable: "date_in_name".to_owned()
            }
        );
        assert_eq!(
            preview("{extension:YYYY}", "x.pdf"),
            NamePreview::InvalidFormat {
                variable: "extension".to_owned()
            }
        );
        assert_eq!(preview("...", "x.pdf"), NamePreview::EmptyName);
    }
}
