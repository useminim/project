//! Modèles de texte à variables, utilisés par l'action `rename`.
//!
//! Syntaxe : du texte libre et des variables `{champ}` ou `{champ:format}`,
//! par exemple `{client_name}_{date:YYYY-MM-DD}.pdf`. Le format est conservé
//! tel quel ; son interprétation dépend du type du champ et se fait à
//! l'exécution.

use std::fmt;

use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::field::FieldName;

/// Modèle de texte analysé.
///
/// Sérialisé sous forme de texte source ; la désérialisation refuse un modèle
/// invalide.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct Template {
    source: String,
    segments: Vec<Segment>,
}

/// Partie d'un modèle.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Segment {
    /// Texte recopié tel quel.
    Literal(String),
    /// Variable remplacée par la valeur d'un champ du document.
    Field {
        /// Champ dont la valeur est insérée.
        name: FieldName,
        /// Format d'affichage optionnel (`YYYY-MM-DD` pour une date…).
        format: Option<String>,
    },
}

/// Erreur d'analyse d'un modèle. Les positions sont des index en octets dans
/// le texte source.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum TemplateError {
    /// Le modèle est vide.
    #[error("le modèle est vide")]
    Empty,
    /// Une accolade ouvrante n'est jamais fermée, ou une autre s'ouvre avant.
    #[error("accolade ouvrante non fermée à la position {position}")]
    UnclosedBrace {
        /// Position de l'accolade ouvrante.
        position: usize,
    },
    /// Une accolade fermante n'a pas d'accolade ouvrante correspondante.
    #[error("accolade fermante inattendue à la position {position}")]
    UnexpectedClosingBrace {
        /// Position de l'accolade fermante.
        position: usize,
    },
    /// Le nom de la variable n'est pas un nom de champ valide.
    #[error("nom de champ invalide « {name} » à la position {position}")]
    InvalidFieldName {
        /// Position de l'accolade ouvrante.
        position: usize,
        /// Nom lu entre les accolades.
        name: String,
    },
    /// Le format qui suit `:` est vide.
    #[error("format vide à la position {position}")]
    EmptyFormat {
        /// Position de l'accolade ouvrante.
        position: usize,
    },
}

impl Template {
    /// Analyse un modèle.
    ///
    /// # Erreurs
    ///
    /// Renvoie une [`TemplateError`] si le modèle est vide, si ses accolades
    /// ne sont pas équilibrées ou si une variable est mal formée.
    pub fn parse(source: &str) -> Result<Self, TemplateError> {
        if source.is_empty() {
            return Err(TemplateError::Empty);
        }

        let mut segments = Vec::new();
        let mut literal = String::new();
        let mut chars = source.char_indices();

        while let Some((position, c)) = chars.next() {
            match c {
                '{' => {
                    if !literal.is_empty() {
                        segments.push(Segment::Literal(std::mem::take(&mut literal)));
                    }
                    let mut inner = String::new();
                    let mut closed = false;
                    for (_, c) in chars.by_ref() {
                        match c {
                            '}' => {
                                closed = true;
                                break;
                            }
                            '{' => return Err(TemplateError::UnclosedBrace { position }),
                            _ => inner.push(c),
                        }
                    }
                    if !closed {
                        return Err(TemplateError::UnclosedBrace { position });
                    }
                    segments.push(parse_field(&inner, position)?);
                }
                '}' => return Err(TemplateError::UnexpectedClosingBrace { position }),
                _ => literal.push(c),
            }
        }
        if !literal.is_empty() {
            segments.push(Segment::Literal(literal));
        }

        Ok(Self {
            source: source.to_owned(),
            segments,
        })
    }

    /// Renvoie le texte source du modèle.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.source
    }

    /// Renvoie les parties du modèle, dans l'ordre.
    #[must_use]
    pub fn segments(&self) -> &[Segment] {
        &self.segments
    }

    /// Parcourt les champs utilisés par le modèle.
    pub fn fields(&self) -> impl Iterator<Item = &FieldName> {
        self.segments.iter().filter_map(|segment| match segment {
            Segment::Field { name, .. } => Some(name),
            Segment::Literal(_) => None,
        })
    }

    /// Parcourt le texte fixe du modèle, hors variables.
    pub fn literals(&self) -> impl Iterator<Item = &str> {
        self.segments.iter().filter_map(|segment| match segment {
            Segment::Literal(text) => Some(text.as_str()),
            Segment::Field { .. } => None,
        })
    }
}

fn parse_field(inner: &str, position: usize) -> Result<Segment, TemplateError> {
    let (name, format) = match inner.split_once(':') {
        Some((name, format)) => (name, Some(format)),
        None => (inner, None),
    };
    if format == Some("") {
        return Err(TemplateError::EmptyFormat { position });
    }
    let name = FieldName::new(name).map_err(|_| TemplateError::InvalidFieldName {
        position,
        name: name.to_owned(),
    })?;
    Ok(Segment::Field {
        name,
        format: format.map(str::to_owned),
    })
}

impl TryFrom<String> for Template {
    type Error = TemplateError;

    fn try_from(source: String) -> Result<Self, Self::Error> {
        Self::parse(&source)
    }
}

impl From<Template> for String {
    fn from(template: Template) -> Self {
        template.source
    }
}

impl fmt::Display for Template {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.source)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn field(name: &str, format: Option<&str>) -> Segment {
        Segment::Field {
            name: FieldName::new(name).unwrap(),
            format: format.map(str::to_owned),
        }
    }

    #[test]
    fn parses_literals_and_fields() {
        let template = Template::parse("{client_name}_{date:YYYY-MM-DD}.pdf").unwrap();
        assert_eq!(
            template.segments(),
            [
                field("client_name", None),
                Segment::Literal("_".to_owned()),
                field("date", Some("YYYY-MM-DD")),
                Segment::Literal(".pdf".to_owned()),
            ]
        );
        assert_eq!(
            template.fields().map(FieldName::as_str).collect::<Vec<_>>(),
            ["client_name", "date"]
        );
        assert_eq!(template.literals().collect::<String>(), "_.pdf");
    }

    #[test]
    fn keeps_the_source_text() {
        let source = "Facture {client_name}.pdf";
        assert_eq!(Template::parse(source).unwrap().as_str(), source);
    }

    #[test]
    fn accepts_a_template_without_fields() {
        let template = Template::parse("facture.pdf").unwrap();
        assert_eq!(
            template.segments(),
            [Segment::Literal("facture.pdf".to_owned())]
        );
    }

    #[test]
    fn rejects_malformed_templates() {
        assert_eq!(Template::parse(""), Err(TemplateError::Empty));
        assert_eq!(
            Template::parse("a{date"),
            Err(TemplateError::UnclosedBrace { position: 1 })
        );
        assert_eq!(
            Template::parse("{a{date}}"),
            Err(TemplateError::UnclosedBrace { position: 0 })
        );
        assert_eq!(
            Template::parse("a}"),
            Err(TemplateError::UnexpectedClosingBrace { position: 1 })
        );
        assert_eq!(
            Template::parse("x{}"),
            Err(TemplateError::InvalidFieldName {
                position: 1,
                name: String::new()
            })
        );
        assert_eq!(
            Template::parse("{Client}"),
            Err(TemplateError::InvalidFieldName {
                position: 0,
                name: "Client".to_owned()
            })
        );
        assert_eq!(
            Template::parse("{date:}"),
            Err(TemplateError::EmptyFormat { position: 0 })
        );
    }

    #[test]
    fn positions_are_byte_offsets() {
        assert_eq!(
            Template::parse("échéance}"),
            Err(TemplateError::UnexpectedClosingBrace { position: 10 })
        );
    }

    #[test]
    fn serializes_as_its_source() {
        let template = Template::parse("{date}.pdf").unwrap();
        assert_eq!(serde_json::to_string(&template).unwrap(), "\"{date}.pdf\"");
        assert_eq!(
            serde_json::from_str::<Template>("\"{date}.pdf\"").unwrap(),
            template
        );
        assert!(serde_json::from_str::<Template>("\"{date\"").is_err());
    }
}
