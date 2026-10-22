//! Rendu des modèles de nom (`{original_name}_{date_in_name:YYYY-MM-DD}.pdf`).
//!
//! Variables disponibles :
//!
//! | Variable | Valeur | Format |
//! | --- | --- | --- |
//! | `original_name` | nom du fichier reçu, sans sa dernière extension | non |
//! | `extension` | dernière extension, sans le point | non |
//! | `file_date` | date de dernière modification du fichier | oui |
//! | `today` | date du jour | oui |
//! | `date_in_name` | date trouvée dans le nom (`matching::find_date`) | oui |
//!
//! Un format de date se compose de `YYYY`, `MM`, `DD` et de séparateurs ;
//! sans format, une date s'écrit `YYYY-MM-DD`. Une variable absente au moment
//! de l'exécution (champ pas encore extrait, fichier sans extension…) est une
//! erreur, jamais une chaîne vide.

use chrono::{Datelike as _, NaiveDate};
use thiserror::Error;

use crate::matching::{FileFacts, find_date};
use crate::template::{Segment, Template};

/// Format d'une date sans format explicite.
pub const DEFAULT_DATE_FORMAT: &str = "YYYY-MM-DD";

/// Valeurs disponibles pour le rendu.
#[derive(Debug, Clone, Copy)]
pub struct RenderContext<'a> {
    /// Fichier reçu.
    pub file: &'a FileFacts,
    /// Date de dernière modification, dans le fuseau de l'utilisateur.
    pub file_date: Option<NaiveDate>,
    /// Date du jour, dans le fuseau de l'utilisateur.
    pub today: NaiveDate,
}

/// Échec du rendu d'un modèle.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum RenderError {
    /// La variable n'a pas de valeur pour ce document.
    #[error("la variable {{{name}}} n'a pas de valeur pour ce document")]
    MissingVariable {
        /// Nom de la variable.
        name: String,
    },
    /// Un format a été donné à une variable qui n'est pas une date.
    #[error("la variable {{{name}}} n'accepte pas de format")]
    UnexpectedFormat {
        /// Nom de la variable.
        name: String,
    },
    /// Le format de date contient autre chose que `YYYY`, `MM`, `DD` et des
    /// séparateurs.
    #[error("format de date invalide pour {{{name}}}")]
    InvalidDateFormat {
        /// Nom de la variable.
        name: String,
    },
}

/// Rend `template` avec les valeurs de `context`. Le résultat n'est pas
/// encore assaini (voir [`super::sanitize`]).
///
/// # Erreurs
///
/// Renvoie une [`RenderError`] si une variable n'a pas de valeur ou si son
/// format est invalide.
pub fn render(template: &Template, context: &RenderContext<'_>) -> Result<String, RenderError> {
    let mut output = String::new();
    for segment in template.segments() {
        match segment {
            Segment::Literal(text) => output.push_str(text),
            Segment::Field { name, format } => {
                let name = name.as_str();
                let missing = || RenderError::MissingVariable {
                    name: name.to_owned(),
                };
                let text_value = |value: Option<&str>| match format {
                    Some(_) => Err(RenderError::UnexpectedFormat {
                        name: name.to_owned(),
                    }),
                    None => value.map(str::to_owned).ok_or_else(missing),
                };
                let date_value = |date: Option<NaiveDate>| {
                    let date = date.ok_or_else(missing)?;
                    format_date(date, format.as_deref().unwrap_or(DEFAULT_DATE_FORMAT)).ok_or_else(
                        || RenderError::InvalidDateFormat {
                            name: name.to_owned(),
                        },
                    )
                };
                let value = match name {
                    "original_name" => text_value(Some(context.file.stem()))?,
                    "extension" => text_value(context.file.extension())?,
                    "file_date" => date_value(context.file_date)?,
                    "today" => date_value(Some(context.today))?,
                    "date_in_name" => date_value(find_date(context.file.name()))?,
                    _ => return Err(missing()),
                };
                output.push_str(&value);
            }
        }
    }
    Ok(output)
}

/// Écrit `date` selon `format` (`YYYY`, `MM`, `DD` et séparateurs). Renvoie
/// `None` si le format contient des lettres ou des chiffres hors de ces
/// composantes.
#[must_use]
pub fn format_date(date: NaiveDate, format: &str) -> Option<String> {
    let mut output = String::new();
    let mut rest = format;
    while let Some(c) = rest.chars().next() {
        let (text, length) = if rest.starts_with("YYYY") {
            (format!("{:04}", date.year()), 4)
        } else if rest.starts_with("MM") {
            (format!("{:02}", date.month()), 2)
        } else if rest.starts_with("DD") {
            (format!("{:02}", date.day()), 2)
        } else if c.is_alphanumeric() {
            return None;
        } else {
            (c.to_string(), c.len_utf8())
        };
        output.push_str(&text);
        rest = &rest[length..];
    }
    Some(output)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn context(file: &FileFacts) -> RenderContext<'_> {
        RenderContext {
            file,
            file_date: NaiveDate::from_ymd_opt(2026, 9, 30),
            today: NaiveDate::from_ymd_opt(2026, 10, 16).unwrap(),
        }
    }

    fn rendered(template: &str, name: &str) -> Result<String, RenderError> {
        let file = FileFacts::new(name, 1);
        render(&Template::parse(template).unwrap(), &context(&file))
    }

    #[test]
    fn renders_every_variable() {
        let cases = [
            (
                "{original_name}.{extension}",
                "Facture Dupont.PDF",
                "Facture Dupont.PDF",
            ),
            (
                "{date_in_name}_{original_name}.pdf",
                "scan 15.03.2026.pdf",
                "2026-03-15_scan 15.03.2026.pdf",
            ),
            (
                "{date_in_name:DD-MM-YYYY}.pdf",
                "Facture_2026-03-15.pdf",
                "15-03-2026.pdf",
            ),
            (
                "{today:YYYYMMDD}_{original_name}.pdf",
                "a.pdf",
                "20261016_a.pdf",
            ),
            ("{file_date:YYYY}/{file_date:MM}", "a.pdf", "2026/09"),
            ("Archive", "a.pdf", "Archive"),
        ];
        for (template, name, expected) in cases {
            assert_eq!(
                rendered(template, name).as_deref(),
                Ok(expected),
                "{template}"
            );
        }
    }

    #[test]
    fn refuses_a_missing_variable_instead_of_an_empty_string() {
        let missing = |name: &str| {
            Err(RenderError::MissingVariable {
                name: name.to_owned(),
            })
        };
        assert_eq!(
            rendered("{client_name}.pdf", "a.pdf"),
            missing("client_name")
        );
        assert_eq!(
            rendered("{original_name}.{extension}", "LISEZMOI"),
            missing("extension")
        );
        assert_eq!(
            rendered("{date_in_name}.pdf", "Facture.pdf"),
            missing("date_in_name")
        );

        let file = FileFacts::new("a.pdf", 1);
        let mut without_date = context(&file);
        without_date.file_date = None;
        assert_eq!(
            render(&Template::parse("{file_date}").unwrap(), &without_date),
            missing("file_date")
        );
    }

    #[test]
    fn checks_formats() {
        assert_eq!(
            rendered("{original_name:YYYY}", "a.pdf"),
            Err(RenderError::UnexpectedFormat {
                name: "original_name".to_owned()
            })
        );
        assert_eq!(
            rendered("{today:JJ-MM-AAAA}", "a.pdf"),
            Err(RenderError::InvalidDateFormat {
                name: "today".to_owned()
            })
        );
    }
}
