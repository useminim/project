//! Déclencheurs : les conditions qui reconnaissent un type de document.

use serde::{Deserialize, Serialize};

use crate::field::FieldName;
use crate::id::ProfileId;

/// Déclencheur d'un workflow : des conditions combinées par `all` (ET) ou
/// `any` (OU), éventuellement regroupées (voir [`Condition::Group`]).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct Trigger {
    /// Mode de combinaison des conditions.
    #[serde(rename = "match")]
    pub match_mode: MatchMode,
    /// Conditions évaluées sur le profil du document.
    pub conditions: Vec<Condition>,
    /// Origine du déclencheur.
    pub source: TriggerSource,
}

/// Mode de combinaison des conditions d'un déclencheur.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(rename_all = "snake_case")]
pub enum MatchMode {
    /// Toutes les conditions doivent être remplies (ET).
    All,
    /// Au moins une condition doit être remplie (OU).
    Any,
}

/// Origine d'un déclencheur.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(rename_all = "snake_case")]
pub enum TriggerSource {
    /// Choisi par l'utilisateur dans le wizard.
    Manual,
    /// Proposé par minim à partir de 3 documents exemples.
    Learned,
}

/// Condition élémentaire d'un déclencheur.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Condition {
    /// Le nom du fichier contient un texte.
    FileNameContains {
        /// Texte recherché.
        value: String,
        /// Respecte la casse si vrai.
        #[serde(default)]
        case_sensitive: bool,
    },
    /// Le nom du fichier contient une date.
    FileNameMatchesDate {
        /// Format de date attendu ; toute date reconnue convient s'il est absent.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        #[cfg_attr(feature = "ts", ts(optional))]
        format: Option<String>,
    },
    /// L'extension du fichier fait partie d'une liste.
    FileExtensionIs {
        /// Extensions acceptées, sans le point (`pdf`, `png`…).
        values: Vec<String>,
    },
    /// Le texte du document contient un texte.
    ContentContainsText {
        /// Texte recherché.
        value: String,
        /// Respecte la casse si vrai.
        #[serde(default)]
        case_sensitive: bool,
    },
    /// Un champ a été détecté dans le document.
    ContentHasField {
        /// Champ attendu.
        field: FieldName,
    },
    /// Le document contient un tableau.
    ContentContainsTable {
        /// Nombre minimal de lignes du tableau.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        #[cfg_attr(feature = "ts", ts(optional))]
        min_rows: Option<u32>,
    },
    /// Le document ressemble aux documents exemples d'un profil appris.
    SimilarToExamples {
        /// Profil appris à partir des documents exemples.
        profile_id: ProfileId,
        /// Score de similarité minimal, entre 0 et 1.
        threshold: f64,
    },
    /// Groupe de conditions combinées par leur propre mode, pour imbriquer
    /// un `any` dans un `all` (ou l'inverse). Profondeur limitée à 3 niveaux,
    /// celui du déclencheur compris.
    Group {
        /// Mode de combinaison des conditions du groupe.
        #[serde(rename = "match")]
        match_mode: MatchMode,
        /// Conditions du groupe.
        conditions: Vec<Condition>,
    },
}

/// Catégorie d'une condition.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConditionCategory {
    /// Porte sur les propriétés du fichier.
    External,
    /// Porte sur le contenu du document.
    Internal,
    /// Porte sur la ressemblance avec des documents exemples.
    Example,
}

impl Condition {
    /// Renvoie la catégorie de la condition, ou `None` pour un groupe, qui
    /// n'a pas de catégorie propre.
    #[must_use]
    pub const fn category(&self) -> Option<ConditionCategory> {
        match self {
            Self::FileNameContains { .. }
            | Self::FileNameMatchesDate { .. }
            | Self::FileExtensionIs { .. } => Some(ConditionCategory::External),
            Self::ContentContainsText { .. }
            | Self::ContentHasField { .. }
            | Self::ContentContainsTable { .. } => Some(ConditionCategory::Internal),
            Self::SimilarToExamples { .. } => Some(ConditionCategory::Example),
            Self::Group { .. } => None,
        }
    }
}

/// Résultat de l'évaluation d'un déclencheur sur un profil de document.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
#[serde(rename_all = "snake_case")]
pub enum MatchResult {
    /// Le document correspond : le workflow s'exécute.
    Match,
    /// Le document ne correspond pas.
    NoMatch,
    /// Correspondance incertaine : le document part en zone d'attente.
    Uncertain,
}

impl MatchResult {
    /// ET en logique de Kleene : `NoMatch` l'emporte, puis `Uncertain`.
    #[must_use]
    pub const fn and(self, other: Self) -> Self {
        match (self, other) {
            (Self::NoMatch, _) | (_, Self::NoMatch) => Self::NoMatch,
            (Self::Uncertain, _) | (_, Self::Uncertain) => Self::Uncertain,
            (Self::Match, Self::Match) => Self::Match,
        }
    }

    /// OU en logique de Kleene : `Match` l'emporte, puis `Uncertain`.
    #[must_use]
    pub const fn or(self, other: Self) -> Self {
        match (self, other) {
            (Self::Match, _) | (_, Self::Match) => Self::Match,
            (Self::Uncertain, _) | (_, Self::Uncertain) => Self::Uncertain,
            (Self::NoMatch, Self::NoMatch) => Self::NoMatch,
        }
    }

    /// Combine des résultats selon le mode : `all` vaut `Match` pour une liste
    /// vide, `any` vaut `NoMatch`.
    pub fn combine(mode: MatchMode, results: impl IntoIterator<Item = Self>) -> Self {
        match mode {
            MatchMode::All => results.into_iter().fold(Self::Match, Self::and),
            MatchMode::Any => results.into_iter().fold(Self::NoMatch, Self::or),
        }
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    #[test]
    fn reads_the_documented_example() {
        let value: serde_json::Value =
            serde_json::from_str(include_str!("../tests/fixtures/trigger.json")).unwrap();
        let trigger: Trigger = serde_json::from_value(value.clone()).unwrap();

        assert_eq!(trigger.match_mode, MatchMode::All);
        assert_eq!(trigger.source, TriggerSource::Manual);
        assert_eq!(
            trigger.conditions[1],
            Condition::ContentHasField {
                field: FieldName::new("invoice_total").unwrap()
            }
        );
        assert_eq!(serde_json::to_value(&trigger).unwrap(), value);
    }

    #[test]
    fn reads_every_condition_type() {
        let conditions = json!([
            { "type": "file_name_contains", "value": "facture", "case_sensitive": true },
            { "type": "file_name_matches_date", "format": "YYYY-MM-DD" },
            { "type": "file_name_matches_date" },
            { "type": "file_extension_is", "values": ["pdf", "png"] },
            { "type": "content_contains_text", "value": "TVA", "case_sensitive": false },
            { "type": "content_has_field", "field": "date" },
            { "type": "content_contains_table", "min_rows": 3 },
            {
                "type": "similar_to_examples",
                "profile_id": "prf_01J8ZK3V9Q4X7M2N5P6R8T0W1Y",
                "threshold": 0.8
            },
            {
                "type": "group",
                "match": "any",
                "conditions": [{ "type": "content_has_field", "field": "date" }]
            }
        ]);
        let parsed: Vec<Condition> = serde_json::from_value(conditions.clone()).unwrap();

        assert_eq!(serde_json::to_value(&parsed).unwrap(), conditions);
        assert_eq!(
            parsed.iter().map(Condition::category).collect::<Vec<_>>(),
            [
                Some(ConditionCategory::External),
                Some(ConditionCategory::External),
                Some(ConditionCategory::External),
                Some(ConditionCategory::External),
                Some(ConditionCategory::Internal),
                Some(ConditionCategory::Internal),
                Some(ConditionCategory::Internal),
                Some(ConditionCategory::Example),
                None,
            ]
        );
    }

    #[test]
    fn case_sensitivity_defaults_to_false() {
        let condition: Condition =
            serde_json::from_value(json!({ "type": "file_name_contains", "value": "facture" }))
                .unwrap();
        assert_eq!(
            condition,
            Condition::FileNameContains {
                value: "facture".to_owned(),
                case_sensitive: false
            }
        );
    }

    const RESULTS: [MatchResult; 3] = [
        MatchResult::Match,
        MatchResult::NoMatch,
        MatchResult::Uncertain,
    ];

    /// Valeur numérique de Kleene : faux = 0, inconnu = 1, vrai = 2. ET est
    /// le minimum, OU le maximum.
    fn rank(result: MatchResult) -> u8 {
        match result {
            MatchResult::NoMatch => 0,
            MatchResult::Uncertain => 1,
            MatchResult::Match => 2,
        }
    }

    #[test]
    fn follows_kleene_logic_for_every_pair() {
        for a in RESULTS {
            for b in RESULTS {
                assert_eq!(rank(a.and(b)), rank(a).min(rank(b)), "{a:?} et {b:?}");
                assert_eq!(rank(a.or(b)), rank(a).max(rank(b)), "{a:?} ou {b:?}");
                assert_eq!(a.and(b), b.and(a));
                assert_eq!(a.or(b), b.or(a));
            }
        }
    }

    #[test]
    fn combines_every_list_of_up_to_three_results() {
        let mut lists: Vec<Vec<MatchResult>> = vec![Vec::new()];
        for _ in 0..3 {
            let longer: Vec<_> = lists
                .iter()
                .filter(|list| list.len() == lists.last().map_or(0, Vec::len))
                .flat_map(|list| {
                    RESULTS.iter().map(move |result| {
                        let mut next = list.clone();
                        next.push(*result);
                        next
                    })
                })
                .collect();
            lists.extend(longer);
        }
        assert_eq!(lists.len(), 1 + 3 + 9 + 27);

        for list in lists {
            let all = list.iter().map(|r| rank(*r)).min().unwrap_or(2);
            let any = list.iter().map(|r| rank(*r)).max().unwrap_or(0);
            assert_eq!(
                rank(MatchResult::combine(MatchMode::All, list.clone())),
                all,
                "all {list:?}"
            );
            assert_eq!(
                rank(MatchResult::combine(MatchMode::Any, list.clone())),
                any,
                "any {list:?}"
            );
        }
    }

    #[test]
    fn rejects_unknown_condition_types() {
        let result = serde_json::from_value::<Condition>(json!({ "type": "file_size_above" }));
        assert!(result.is_err());
    }
}
