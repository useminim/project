//! Correspondance : évaluation d'un déclencheur sur un document.
//!
//! L'évaluation est pure : elle ne lit ni le disque ni le réseau. Elle renvoie
//! un [`MatchResult`] à trois valeurs (logique de Kleene) et l'explication de
//! chaque condition, pour que l'interface puisse montrer « pourquoi ».
//!
//! Les conditions externes (nom, date, extension) ne sont jamais incertaines.
//! Les conditions sur le contenu et par l'exemple renvoient `Uncertain`
//! (« non disponible ») tant que l'analyse des documents n'existe pas.

pub mod date;
pub mod facts;
pub mod text;

use chrono::NaiveDate;
use serde::Serialize;

pub use date::{DEFAULT_FORMATS, DateFormat, InvalidDateFormat, find_date, find_date_with};
pub use facts::FileFacts;

use crate::trigger::{Condition, MatchResult, Trigger};

/// Résultat de l'évaluation d'un déclencheur.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
pub struct Evaluation {
    /// Résultat du déclencheur.
    pub result: MatchResult,
    /// Résultat et explication de chaque condition, groupes compris, dans
    /// l'ordre du déclencheur.
    pub conditions: Vec<ConditionOutcome>,
}

/// Résultat d'une condition.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct ConditionOutcome {
    /// Chemin de la condition, comme dans les erreurs de validation
    /// (`trigger.conditions[1].conditions[0]`).
    pub path: String,
    /// Résultat de la condition.
    pub result: MatchResult,
    /// Ce qui a été observé.
    #[serde(flatten)]
    #[cfg_attr(feature = "ts", ts(flatten))]
    pub explanation: Explanation,
}

/// Ce qui a été observé pour une condition.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(tag = "explanation", rename_all = "snake_case")]
pub enum Explanation {
    /// Recherche d'un texte dans le nom : le résultat dit s'il a été trouvé.
    TextSearch,
    /// Date trouvée dans le nom.
    DateFound {
        /// Date trouvée.
        date: NaiveDate,
    },
    /// Aucune date trouvée dans le nom.
    NoDate,
    /// Le format de date de la condition est invalide.
    InvalidDateFormat,
    /// Extension du fichier.
    Extension {
        /// Extension en minuscules, sans le point.
        found: String,
    },
    /// Le fichier n'a pas d'extension.
    NoExtension,
    /// Groupe de conditions, détaillé par les conditions qui suivent.
    Group,
    /// Condition sur le contenu ou par l'exemple, pas encore évaluable.
    NotAvailable,
}

/// Évalue `trigger` sur le fichier `file`.
#[must_use]
pub fn evaluate(trigger: &Trigger, file: &FileFacts) -> Evaluation {
    let mut conditions = Vec::new();
    let results: Vec<_> = trigger
        .conditions
        .iter()
        .enumerate()
        .map(|(index, condition)| {
            evaluate_condition(
                &format!("trigger.conditions[{index}]"),
                condition,
                file,
                &mut conditions,
            )
        })
        .collect();
    Evaluation {
        result: MatchResult::combine(trigger.match_mode, results),
        conditions,
    }
}

fn evaluate_condition(
    path: &str,
    condition: &Condition,
    file: &FileFacts,
    outcomes: &mut Vec<ConditionOutcome>,
) -> MatchResult {
    let (result, explanation) = match condition {
        Condition::FileNameContains {
            value,
            case_sensitive,
        } => (
            found(text::contains(file.name(), value, *case_sensitive)),
            Explanation::TextSearch,
        ),
        Condition::FileNameMatchesDate { format } => match format.as_deref() {
            None => date_outcome(find_date(file.name())),
            Some(format) => match DateFormat::parse(format) {
                Ok(format) => date_outcome(find_date_with(file.name(), &format)),
                Err(InvalidDateFormat) => (MatchResult::NoMatch, Explanation::InvalidDateFormat),
            },
        },
        Condition::FileExtensionIs { values } => match file.extension() {
            Some(extension) => {
                let extension = extension.to_lowercase();
                let wanted = canonical_extension(&extension);
                let matches = values
                    .iter()
                    .any(|value| canonical_extension(&value.to_lowercase()) == wanted);
                (found(matches), Explanation::Extension { found: extension })
            }
            None => (MatchResult::NoMatch, Explanation::NoExtension),
        },
        Condition::ContentContainsText { .. }
        | Condition::ContentHasField { .. }
        | Condition::ContentContainsTable { .. }
        | Condition::SimilarToExamples { .. } => {
            (MatchResult::Uncertain, Explanation::NotAvailable)
        }
        Condition::Group {
            match_mode,
            conditions,
        } => {
            // La ligne du groupe précède celles de ses conditions.
            let position = outcomes.len();
            outcomes.push(ConditionOutcome {
                path: path.to_owned(),
                result: MatchResult::Uncertain,
                explanation: Explanation::Group,
            });
            let results: Vec<_> = conditions
                .iter()
                .enumerate()
                .map(|(index, condition)| {
                    evaluate_condition(
                        &format!("{path}.conditions[{index}]"),
                        condition,
                        file,
                        outcomes,
                    )
                })
                .collect();
            let result = MatchResult::combine(*match_mode, results);
            if let Some(group) = outcomes.get_mut(position) {
                group.result = result;
            }
            return result;
        }
    };
    outcomes.push(ConditionOutcome {
        path: path.to_owned(),
        result,
        explanation,
    });
    result
}

const fn found(found: bool) -> MatchResult {
    if found {
        MatchResult::Match
    } else {
        MatchResult::NoMatch
    }
}

fn date_outcome(date: Option<NaiveDate>) -> (MatchResult, Explanation) {
    match date {
        Some(date) => (MatchResult::Match, Explanation::DateFound { date }),
        None => (MatchResult::NoMatch, Explanation::NoDate),
    }
}

/// Ramène les extensions synonymes à une seule forme (`jpeg` → `jpg`,
/// `tif` → `tiff`). L'entrée est déjà en minuscules.
fn canonical_extension(extension: &str) -> &str {
    match extension {
        "jpeg" | "jpe" => "jpg",
        "tif" => "tiff",
        other => other,
    }
}

#[cfg(test)]
mod tests;
