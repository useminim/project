//! Workflow : un déclencheur suivi d'une suite d'actions, et sa validation.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::action::Action;
use crate::id::{OrganizationId, UserId, WorkflowId};
use crate::trigger::{Condition, Trigger};

/// Version du schéma JSON des workflows écrits par cette version de minim.
pub const CURRENT_SCHEMA_VERSION: u32 = 1;

/// Longueur maximale du nom d'un workflow, en caractères.
pub const MAX_NAME_LENGTH: usize = 100;

/// Automatisation créée par l'utilisateur.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Workflow {
    /// Version du schéma JSON du workflow. Absente dans les workflows écrits
    /// avant son introduction, elle vaut alors 1.
    #[serde(default = "first_schema_version")]
    pub schema_version: u32,
    /// Identifiant du workflow.
    pub id: WorkflowId,
    /// Nom affiché.
    pub name: String,
    /// Incrémenté à chaque modification ; sert à la synchronisation. Commence à 1.
    pub version: u32,
    /// Un workflow désactivé n'est pas évalué.
    pub enabled: bool,
    /// Ordre d'évaluation : plus petit = évalué en premier (D-10).
    pub priority: u32,
    /// Si vrai, le workflow suivant est aussi évalué après une correspondance (D-10).
    pub continue_on_match: bool,
    /// Déclencheur qui reconnaît les documents concernés.
    pub trigger: Trigger,
    /// Actions exécutées dans l'ordre.
    pub actions: Vec<Action>,
    /// Propriétaire du workflow.
    pub owner: Owner,
    /// Date de création.
    pub created_at: DateTime<Utc>,
    /// Date de dernière modification.
    pub updated_at: DateTime<Utc>,
}

const fn first_schema_version() -> u32 {
    1
}

/// Propriétaire d'un workflow.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", content = "id", rename_all = "snake_case")]
pub enum Owner {
    /// Workflow personnel.
    User(UserId),
    /// Workflow partagé au sein d'un compte entreprise.
    Organization(OrganizationId),
}

/// Problème qui empêche d'enregistrer ou d'exécuter un workflow.
///
/// Sérialisé à plat : `{ "path": "actions[2]", "code": "empty_destination" }`.
/// Le chemin désigne l'élément concerné (`name`, `trigger.conditions[1]`,
/// `trigger.conditions[1].conditions[0]`, `actions[2]`…), les index commençant
/// à 0. L'interface affiche le message de la clé de traduction
/// `validation.<code>`.
#[derive(Debug, Clone, PartialEq, Eq, Error, Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[error("{path} : {kind}")]
pub struct ValidationError {
    /// Chemin de l'élément concerné dans le workflow.
    pub path: String,
    /// Nature du problème.
    #[serde(flatten)]
    #[cfg_attr(feature = "ts", ts(flatten))]
    pub kind: ValidationErrorKind,
}

impl ValidationError {
    /// Crée un problème sur l'élément désigné par `path`.
    pub fn new(path: impl Into<String>, kind: ValidationErrorKind) -> Self {
        Self {
            path: path.into(),
            kind,
        }
    }
}

/// Nature d'un problème de validation, sérialisée dans le champ `code`.
#[derive(Debug, Clone, PartialEq, Eq, Error, Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(tag = "code", rename_all = "snake_case")]
pub enum ValidationErrorKind {
    /// Le workflow a été écrit par une version plus récente de minim.
    #[error("version de schéma {found} non prise en charge")]
    UnsupportedSchemaVersion {
        /// Version trouvée dans le workflow.
        found: u32,
    },
    /// Le nom est vide ou ne contient que des espaces.
    #[error("le nom du workflow est vide")]
    EmptyName,
    /// Le nom dépasse la longueur maximale.
    #[error("le nom du workflow dépasse {max} caractères")]
    NameTooLong {
        /// Longueur maximale, en caractères.
        max: usize,
    },
    /// La version vaut 0.
    #[error("la version du workflow doit être au moins 1")]
    ZeroVersion,
    /// La date de modification précède la date de création.
    #[error("la date de modification précède la date de création")]
    UpdatedBeforeCreated,
    /// Le déclencheur n'a aucune condition.
    #[error("le déclencheur n'a aucune condition")]
    NoConditions,
    /// Un groupe de conditions est vide.
    #[error("le groupe ne contient aucune condition")]
    EmptyGroup,
    /// Un groupe dépasse la profondeur maximale.
    #[error("les groupes de conditions sont imbriqués sur plus de {max_depth} niveaux")]
    GroupTooDeep {
        /// Profondeur maximale, niveau du déclencheur compris.
        max_depth: usize,
    },
    /// Le texte recherché par une condition est vide.
    #[error("le texte recherché est vide")]
    EmptySearchText,
    /// Le format de date d'une condition est vide.
    #[error("le format de date est vide")]
    EmptyDateFormat,
    /// Une condition sur l'extension n'en liste aucune.
    #[error("aucune extension indiquée")]
    NoExtensions,
    /// Une extension contient autre chose que des lettres et des chiffres.
    #[error("extension « {extension} » invalide")]
    InvalidExtension {
        /// Extension refusée.
        extension: String,
    },
    /// Le nombre minimal de lignes d'un tableau vaut 0.
    #[error("le nombre minimal de lignes doit être au moins 1")]
    ZeroMinRows,
    /// Le seuil de similarité n'est pas compris entre 0 et 1.
    #[error("le seuil doit être compris entre 0 et 1")]
    ThresholdOutOfRange,
    /// Le workflow n'a aucune action.
    #[error("le workflow n'a aucune action")]
    NoActions,
    /// Le modèle de nom contient un séparateur de chemin.
    #[error("le nouveau nom ne doit pas contenir « / » ni « \\ »")]
    PathSeparatorInName,
    /// Le dossier de destination est vide.
    #[error("le dossier de destination est vide")]
    EmptyDestination,
    /// Le dossier de destination n'est pas un chemin absolu.
    #[error("le dossier de destination doit être un chemin absolu")]
    RelativeDestination,
    /// Une extraction ne liste aucun champ.
    #[error("aucun champ à extraire")]
    NoFieldsToExtract,
}

/// Profondeur maximale des conditions : le niveau du déclencheur compte pour 1,
/// chaque groupe imbriqué ajoute un niveau.
pub const MAX_CONDITION_DEPTH: usize = 3;

impl Workflow {
    /// Vérifie la cohérence du workflow.
    ///
    /// La forme des données (identifiants, noms de champs, modèles) est déjà
    /// garantie par les types ; cette méthode contrôle les règles qui portent
    /// sur les valeurs.
    ///
    /// # Erreurs
    ///
    /// Renvoie tous les problèmes trouvés, pour que le wizard puisse les
    /// afficher ensemble.
    pub fn validate(&self) -> Result<(), Vec<ValidationError>> {
        use ValidationErrorKind as Kind;

        let mut errors = Vec::new();
        let mut push = |path: &str, kind| errors.push(ValidationError::new(path, kind));

        if self.schema_version > CURRENT_SCHEMA_VERSION {
            push(
                "schema_version",
                Kind::UnsupportedSchemaVersion {
                    found: self.schema_version,
                },
            );
        }
        if self.name.trim().is_empty() {
            push("name", Kind::EmptyName);
        } else if self.name.chars().count() > MAX_NAME_LENGTH {
            push(
                "name",
                Kind::NameTooLong {
                    max: MAX_NAME_LENGTH,
                },
            );
        }
        if self.version == 0 {
            push("version", Kind::ZeroVersion);
        }
        if self.updated_at < self.created_at {
            push("updated_at", Kind::UpdatedBeforeCreated);
        }

        if self.trigger.conditions.is_empty() {
            push("trigger.conditions", Kind::NoConditions);
        }
        validate_conditions(
            "trigger.conditions",
            &self.trigger.conditions,
            1,
            &mut errors,
        );

        if self.actions.is_empty() {
            errors.push(ValidationError::new("actions", Kind::NoActions));
        }
        for (index, action) in self.actions.iter().enumerate() {
            validate_action(&format!("actions[{index}]"), action, &mut errors);
        }

        if errors.is_empty() {
            Ok(())
        } else {
            Err(errors)
        }
    }
}

/// Valide les conditions d'une liste située au niveau `depth` (1 pour le
/// déclencheur).
fn validate_conditions(
    list_path: &str,
    conditions: &[Condition],
    depth: usize,
    errors: &mut Vec<ValidationError>,
) {
    for (index, condition) in conditions.iter().enumerate() {
        let path = format!("{list_path}[{index}]");
        validate_condition(&path, condition, depth, errors);
    }
}

fn validate_condition(
    path: &str,
    condition: &Condition,
    depth: usize,
    errors: &mut Vec<ValidationError>,
) {
    use ValidationErrorKind as Kind;

    let mut push = |kind| errors.push(ValidationError::new(path, kind));
    match condition {
        Condition::FileNameContains { value, .. }
        | Condition::ContentContainsText { value, .. } => {
            if value.is_empty() {
                push(Kind::EmptySearchText);
            }
        }
        Condition::FileNameMatchesDate { format } => {
            if format
                .as_deref()
                .is_some_and(|format| format.trim().is_empty())
            {
                push(Kind::EmptyDateFormat);
            }
        }
        Condition::FileExtensionIs { values } => {
            if values.is_empty() {
                push(Kind::NoExtensions);
            }
            for extension in values {
                let valid =
                    !extension.is_empty() && extension.chars().all(|c| c.is_ascii_alphanumeric());
                if !valid {
                    push(Kind::InvalidExtension {
                        extension: extension.clone(),
                    });
                }
            }
        }
        Condition::ContentHasField { .. } => {}
        Condition::ContentContainsTable { min_rows } => {
            if *min_rows == Some(0) {
                push(Kind::ZeroMinRows);
            }
        }
        Condition::SimilarToExamples { threshold, .. } => {
            if !(0.0..=1.0).contains(threshold) {
                push(Kind::ThresholdOutOfRange);
            }
        }
        Condition::Group { conditions, .. } => {
            // Les conditions du groupe seraient au niveau `depth + 1`.
            if depth >= MAX_CONDITION_DEPTH {
                push(Kind::GroupTooDeep {
                    max_depth: MAX_CONDITION_DEPTH,
                });
                return;
            }
            if conditions.is_empty() {
                push(Kind::EmptyGroup);
            }
            validate_conditions(&format!("{path}.conditions"), conditions, depth + 1, errors);
        }
    }
}

fn validate_action(path: &str, action: &Action, errors: &mut Vec<ValidationError>) {
    use ValidationErrorKind as Kind;

    let mut push = |kind| errors.push(ValidationError::new(path, kind));
    match action {
        Action::Rename { template } => {
            if template.literals().any(|text| text.contains(['/', '\\'])) {
                push(Kind::PathSeparatorInName);
            }
        }
        Action::Move { destination, .. } | Action::Copy { destination } => {
            let destination = destination.to_string_lossy();
            if destination.is_empty() {
                push(Kind::EmptyDestination);
            } else if !is_absolute_on_any_os(&destination) {
                push(Kind::RelativeDestination);
            }
        }
        Action::Convert { .. } => {}
        Action::ExtractFields { fields, .. } => {
            if fields.is_empty() {
                push(Kind::NoFieldsToExtract);
            }
        }
    }
}

/// Indique si `path` est absolu sous Windows, macOS ou Linux.
///
/// Un workflow synchronisé peut avoir été créé sur un autre OS : la règle ne
/// dépend donc pas de l'OS qui valide, contrairement à `Path::is_absolute`.
/// Sont absolus `/…`, `C:\…` ou `C:/…`, et les chemins réseau `\\serveur\…`.
fn is_absolute_on_any_os(path: &str) -> bool {
    let mut chars = path.chars();
    match (chars.next(), chars.next(), chars.next()) {
        (Some('/'), _, _) | (Some('\\'), Some('\\'), Some(_)) => true,
        (Some(drive), Some(':'), Some('\\' | '/')) => drive.is_ascii_alphabetic(),
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use serde_json::{Value, json};

    use super::*;
    use ValidationErrorKind as Kind;

    fn error(path: &str, kind: ValidationErrorKind) -> ValidationError {
        ValidationError::new(path, kind)
    }
    use crate::trigger::{MatchMode, TriggerSource};

    fn documented_example() -> Value {
        serde_json::from_str(include_str!("../tests/fixtures/workflow.json")).unwrap()
    }

    fn valid_workflow() -> Workflow {
        serde_json::from_value(documented_example()).unwrap()
    }

    fn conditions(conditions: Value) -> Workflow {
        let mut workflow = valid_workflow();
        workflow.trigger.conditions = serde_json::from_value(conditions).unwrap();
        workflow
    }

    fn actions(actions: Value) -> Workflow {
        let mut workflow = valid_workflow();
        workflow.actions = serde_json::from_value(actions).unwrap();
        workflow
    }

    #[test]
    fn reads_the_documented_example() {
        let workflow = valid_workflow();

        assert_eq!(workflow.trigger.match_mode, MatchMode::All);
        assert_eq!(workflow.trigger.source, TriggerSource::Manual);
        assert!(matches!(workflow.owner, Owner::User(_)));
        assert_eq!(
            serde_json::to_value(&workflow).unwrap(),
            documented_example()
        );
    }

    #[test]
    fn reads_an_organization_owner() {
        let value = json!({ "kind": "organization", "id": "org_01J8ZK3V9Q4X7M2N5P6R8T0W1Y" });
        let owner: Owner = serde_json::from_value(value.clone()).unwrap();
        assert!(matches!(owner, Owner::Organization(_)));
        assert_eq!(serde_json::to_value(owner).unwrap(), value);
    }

    #[test]
    fn rejects_an_owner_id_of_the_wrong_kind() {
        let value = json!({ "kind": "user", "id": "org_01J8ZK3V9Q4X7M2N5P6R8T0W1Y" });
        assert!(serde_json::from_value::<Owner>(value).is_err());
    }

    #[test]
    fn accepts_the_documented_example() {
        assert_eq!(valid_workflow().validate(), Ok(()));
    }

    #[test]
    fn reports_every_problem_at_once() {
        let mut workflow = valid_workflow();
        workflow.name = "  ".to_owned();
        workflow.version = 0;
        workflow.updated_at = workflow.created_at - chrono::Duration::seconds(1);
        workflow.trigger.conditions.clear();
        workflow.actions.clear();

        assert_eq!(
            workflow.validate(),
            Err(vec![
                error("name", Kind::EmptyName),
                error("version", Kind::ZeroVersion),
                error("updated_at", Kind::UpdatedBeforeCreated),
                error("trigger.conditions", Kind::NoConditions),
                error("actions", Kind::NoActions),
            ])
        );
    }

    #[test]
    fn checks_condition_values() {
        let workflow = conditions(json!([
            { "type": "file_name_contains", "value": "" },
            { "type": "content_contains_text", "value": "" },
            { "type": "file_name_matches_date", "format": " " },
            { "type": "file_extension_is", "values": [] },
            { "type": "file_extension_is", "values": ["pdf", ".png", ""] },
            { "type": "content_contains_table", "min_rows": 0 },
            {
                "type": "similar_to_examples",
                "profile_id": "prf_01J8ZK3V9Q4X7M2N5P6R8T0W1Y",
                "threshold": 1.5
            }
        ]));

        assert_eq!(
            workflow.validate(),
            Err(vec![
                error("trigger.conditions[0]", Kind::EmptySearchText),
                error("trigger.conditions[1]", Kind::EmptySearchText),
                error("trigger.conditions[2]", Kind::EmptyDateFormat),
                error("trigger.conditions[3]", Kind::NoExtensions),
                error(
                    "trigger.conditions[4]",
                    Kind::InvalidExtension {
                        extension: ".png".to_owned()
                    }
                ),
                error(
                    "trigger.conditions[4]",
                    Kind::InvalidExtension {
                        extension: String::new()
                    }
                ),
                error("trigger.conditions[5]", Kind::ZeroMinRows),
                error("trigger.conditions[6]", Kind::ThresholdOutOfRange),
            ])
        );
    }

    #[test]
    fn accepts_valid_conditions() {
        let workflow = conditions(json!([
            { "type": "file_name_matches_date" },
            { "type": "file_extension_is", "values": ["pdf", "PNG", "mp4"] },
            { "type": "content_has_field", "field": "invoice_total" },
            { "type": "content_contains_table", "min_rows": 1 },
            {
                "type": "similar_to_examples",
                "profile_id": "prf_01J8ZK3V9Q4X7M2N5P6R8T0W1Y",
                "threshold": 0.0
            }
        ]));
        assert_eq!(workflow.validate(), Ok(()));
    }

    #[test]
    fn rejects_a_nan_threshold() {
        let mut workflow = valid_workflow();
        workflow.trigger.conditions = vec![Condition::SimilarToExamples {
            profile_id: "prf_01J8ZK3V9Q4X7M2N5P6R8T0W1Y".parse().unwrap(),
            threshold: f64::NAN,
        }];
        assert_eq!(
            workflow.validate(),
            Err(vec![error(
                "trigger.conditions[0]",
                Kind::ThresholdOutOfRange
            )])
        );
    }

    #[test]
    fn checks_action_values() {
        let workflow = actions(json!([
            { "type": "rename", "template": "factures/{date}.pdf" },
            { "type": "rename", "template": "{date}\\copie.pdf" },
            { "type": "move", "destination": "" },
            { "type": "copy", "destination": "" },
            { "type": "extract_fields", "fields": [], "output": "json" }
        ]));

        assert_eq!(
            workflow.validate(),
            Err(vec![
                error("actions[0]", Kind::PathSeparatorInName),
                error("actions[1]", Kind::PathSeparatorInName),
                error("actions[2]", Kind::EmptyDestination),
                error("actions[3]", Kind::EmptyDestination),
                error("actions[4]", Kind::NoFieldsToExtract),
            ])
        );
    }

    #[test]
    fn reads_a_workflow_without_schema_version_as_version_1() {
        let mut value = documented_example();
        value.as_object_mut().unwrap().remove("schema_version");

        let workflow: Workflow = serde_json::from_value(value).unwrap();

        assert_eq!(workflow.schema_version, 1);
        assert_eq!(
            serde_json::to_value(&workflow).unwrap()["schema_version"],
            json!(1)
        );
    }

    #[test]
    fn rejects_a_newer_schema_version() {
        let mut workflow = valid_workflow();
        workflow.schema_version = CURRENT_SCHEMA_VERSION + 1;

        assert_eq!(
            workflow.validate(),
            Err(vec![error(
                "schema_version",
                Kind::UnsupportedSchemaVersion {
                    found: CURRENT_SCHEMA_VERSION + 1
                }
            )])
        );
    }

    #[test]
    fn counts_the_name_length_in_characters() {
        let mut workflow = valid_workflow();
        workflow.name = "é".repeat(MAX_NAME_LENGTH);
        assert_eq!(workflow.validate(), Ok(()));

        workflow.name.push('é');
        assert_eq!(
            workflow.validate(),
            Err(vec![error(
                "name",
                Kind::NameTooLong {
                    max: MAX_NAME_LENGTH
                }
            )])
        );
    }

    #[test]
    fn accepts_absolute_destinations_of_every_os() {
        let workflow = actions(json!([
            { "type": "move", "destination": "/home/factures" },
            { "type": "move", "destination": "C:\\Factures" },
            { "type": "copy", "destination": "d:/archives" },
            { "type": "copy", "destination": "\\\\serveur\\partage\\factures" }
        ]));
        assert_eq!(workflow.validate(), Ok(()));
    }

    #[test]
    fn rejects_relative_destinations() {
        let workflow = actions(json!([
            { "type": "move", "destination": "factures" },
            { "type": "move", "destination": "./factures" },
            { "type": "copy", "destination": "C:factures" },
            { "type": "copy", "destination": "\\factures" },
            { "type": "copy", "destination": "1:\\factures" }
        ]));

        assert_eq!(
            workflow.validate(),
            Err((0..5)
                .map(|index| error(&format!("actions[{index}]"), Kind::RelativeDestination))
                .collect())
        );
    }

    #[test]
    fn accepts_groups_up_to_the_maximum_depth() {
        let workflow = conditions(json!([
            { "type": "file_extension_is", "values": ["pdf"] },
            {
                "type": "group",
                "match": "any",
                "conditions": [
                    { "type": "file_name_contains", "value": "facture" },
                    {
                        "type": "group",
                        "match": "all",
                        "conditions": [{ "type": "content_has_field", "field": "date" }]
                    }
                ]
            }
        ]));
        assert_eq!(workflow.validate(), Ok(()));
    }

    #[test]
    fn locates_problems_inside_nested_groups() {
        let workflow = conditions(json!([
            {
                "type": "group",
                "match": "any",
                "conditions": [
                    { "type": "file_name_contains", "value": "facture" },
                    {
                        "type": "group",
                        "match": "all",
                        "conditions": [{ "type": "content_contains_text", "value": "" }]
                    }
                ]
            }
        ]));

        assert_eq!(
            workflow.validate(),
            Err(vec![error(
                "trigger.conditions[0].conditions[1].conditions[0]",
                Kind::EmptySearchText
            )])
        );
    }

    #[test]
    fn rejects_an_empty_group() {
        let workflow = conditions(json!([
            { "type": "group", "match": "all", "conditions": [] }
        ]));

        assert_eq!(
            workflow.validate(),
            Err(vec![error("trigger.conditions[0]", Kind::EmptyGroup)])
        );
    }

    #[test]
    fn rejects_groups_nested_beyond_the_maximum_depth() {
        let workflow = conditions(json!([
            {
                "type": "group",
                "match": "all",
                "conditions": [{
                    "type": "group",
                    "match": "any",
                    "conditions": [{
                        "type": "group",
                        "match": "all",
                        "conditions": [{ "type": "content_contains_text", "value": "" }]
                    }]
                }]
            }
        ]));

        // Le groupe trop profond est signalé seul : son contenu n'est pas validé.
        assert_eq!(
            workflow.validate(),
            Err(vec![error(
                "trigger.conditions[0].conditions[0].conditions[0]",
                Kind::GroupTooDeep {
                    max_depth: MAX_CONDITION_DEPTH
                }
            )])
        );
    }

    #[test]
    fn serializes_errors_with_a_code() {
        assert_eq!(
            serde_json::to_value(error(
                "trigger.conditions[2]",
                Kind::InvalidExtension {
                    extension: ".png".to_owned(),
                }
            ))
            .unwrap(),
            json!({ "path": "trigger.conditions[2]", "code": "invalid_extension", "extension": ".png" })
        );
        assert_eq!(
            serde_json::to_value(error("actions", Kind::NoActions)).unwrap(),
            json!({ "path": "actions", "code": "no_actions" })
        );
    }
}
