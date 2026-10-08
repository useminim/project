//! Propriété : tout workflow bien formé survit à l'aller-retour JSON.

#![allow(
    clippy::unwrap_used,
    reason = "fichier de tests : clippy.toml n'autorise unwrap que dans les fonctions #[test]"
)]

use std::path::PathBuf;

use chrono::{DateTime, Utc};
use minim_core::{
    Action, Condition, ConvertFormat, ExportFormat, FieldName, MatchMode, OrganizationId, Owner,
    ProfileId, Template, Trigger, TriggerSource, UserId, Workflow, WorkflowId,
};
use proptest::prelude::*;
use ulid::Ulid;

fn ulid() -> impl Strategy<Value = Ulid> {
    any::<u128>().prop_map(Ulid::from)
}

fn field_name() -> impl Strategy<Value = FieldName> {
    "[a-z][a-z0-9_]{0,15}".prop_map(|name| FieldName::new(name).unwrap())
}

/// Modèle valide : une suite de textes et de variables `{champ}` ou
/// `{champ:format}`.
fn template() -> impl Strategy<Value = Template> {
    let segment = prop_oneof![
        "[A-Za-z0-9 _.-]{1,8}",
        field_name().prop_map(|name| format!("{{{name}}}")),
        (field_name(), "[A-Za-z-]{1,10}").prop_map(|(name, format)| format!("{{{name}:{format}}}")),
    ];
    prop::collection::vec(segment, 1..5).prop_map(|parts| Template::parse(&parts.concat()).unwrap())
}

/// Seuil fini entre 0 et 1, avec une écriture décimale courte : un `NaN`
/// casserait l'égalité.
fn threshold() -> impl Strategy<Value = f64> {
    (0_u32..=1000).prop_map(|n| f64::from(n) / 1000.0)
}

fn date() -> impl Strategy<Value = DateTime<Utc>> {
    (0_i64..4_102_444_800, 0_u32..1_000_000_000)
        .prop_map(|(seconds, nanos)| DateTime::from_timestamp(seconds, nanos).unwrap())
}

fn match_mode() -> impl Strategy<Value = MatchMode> {
    prop_oneof![Just(MatchMode::All), Just(MatchMode::Any)]
}

fn leaf_condition() -> impl Strategy<Value = Condition> {
    prop_oneof![
        (any::<String>(), any::<bool>()).prop_map(|(value, case_sensitive)| {
            Condition::FileNameContains {
                value,
                case_sensitive,
            }
        }),
        proptest::option::of(any::<String>())
            .prop_map(|format| Condition::FileNameMatchesDate { format }),
        prop::collection::vec("[a-z0-9]{1,4}", 0..3)
            .prop_map(|values| Condition::FileExtensionIs { values }),
        (any::<String>(), any::<bool>()).prop_map(|(value, case_sensitive)| {
            Condition::ContentContainsText {
                value,
                case_sensitive,
            }
        }),
        field_name().prop_map(|field| Condition::ContentHasField { field }),
        proptest::option::of(any::<u32>())
            .prop_map(|min_rows| Condition::ContentContainsTable { min_rows }),
        (ulid(), threshold()).prop_map(|(ulid, threshold)| Condition::SimilarToExamples {
            profile_id: ProfileId::from_ulid(ulid),
            threshold,
        }),
    ]
}

fn condition() -> impl Strategy<Value = Condition> {
    leaf_condition().prop_recursive(3, 16, 3, |inner| {
        (match_mode(), prop::collection::vec(inner, 0..3)).prop_map(|(match_mode, conditions)| {
            Condition::Group {
                match_mode,
                conditions,
            }
        })
    })
}

fn action() -> impl Strategy<Value = Action> {
    let destination = "[ -~]{0,20}".prop_map(PathBuf::from);
    prop_oneof![
        template().prop_map(|template| Action::Rename { template }),
        (destination.clone(), any::<bool>()).prop_map(|(destination, create_folders)| {
            Action::Move {
                destination,
                create_folders,
            }
        }),
        destination.prop_map(|destination| Action::Copy { destination }),
        prop_oneof![Just(ConvertFormat::Pdf), Just(ConvertFormat::Png)]
            .prop_map(|to| Action::Convert { to }),
        (
            prop::collection::vec(field_name(), 0..3),
            prop_oneof![Just(ExportFormat::Csv), Just(ExportFormat::Json)],
        )
            .prop_map(|(fields, output)| Action::ExtractFields { fields, output }),
    ]
}

fn owner() -> impl Strategy<Value = Owner> {
    prop_oneof![
        ulid().prop_map(|ulid| Owner::User(UserId::from_ulid(ulid))),
        ulid().prop_map(|ulid| Owner::Organization(OrganizationId::from_ulid(ulid))),
    ]
}

fn trigger() -> impl Strategy<Value = Trigger> {
    (
        match_mode(),
        prop::collection::vec(condition(), 0..4),
        prop_oneof![Just(TriggerSource::Manual), Just(TriggerSource::Learned)],
    )
        .prop_map(|(match_mode, conditions, source)| Trigger {
            match_mode,
            conditions,
            source,
        })
}

prop_compose! {
    fn workflow()(
        schema_version in any::<u32>(),
        ulid in ulid(),
        name in any::<String>(),
        version in any::<u32>(),
        enabled in any::<bool>(),
        priority in any::<u32>(),
        continue_on_match in any::<bool>(),
        trigger in trigger(),
        actions in prop::collection::vec(action(), 0..4),
        owner in owner(),
        created_at in date(),
        updated_at in date(),
    ) -> Workflow {
        Workflow {
            schema_version,
            id: WorkflowId::from_ulid(ulid),
            name,
            version,
            enabled,
            priority,
            continue_on_match,
            trigger,
            actions,
            owner,
            created_at,
            updated_at,
        }
    }
}

proptest! {
    #[test]
    fn every_workflow_survives_a_json_round_trip(workflow in workflow()) {
        let json = serde_json::to_string(&workflow).unwrap();
        let parsed: Workflow = serde_json::from_str(&json).unwrap();

        prop_assert_eq!(parsed, workflow);
    }
}
