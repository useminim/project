//! Les exemples JSON du modèle de données, copiés dans `tests/fixtures/`, sont
//! lus puis réécrits à l'identique. Quand un exemple change dans la
//! documentation, sa fixture est mise à jour à la main.

#![allow(
    clippy::unwrap_used,
    reason = "fichier de tests : clippy.toml n'autorise unwrap que dans les fonctions #[test]"
)]

use minim_core::{Action, DocumentProfile, ExecutionLog, Trigger, Workflow};
use serde::Serialize;
use serde::de::DeserializeOwned;
use serde_json::Value;

fn assert_round_trip<T: Serialize + DeserializeOwned>(fixture: &str) {
    let original: Value = serde_json::from_str(fixture).unwrap();
    let parsed: T = serde_json::from_value(original.clone()).unwrap();

    assert_eq!(serde_json::to_value(&parsed).unwrap(), original);
}

#[test]
fn workflow() {
    assert_round_trip::<Workflow>(include_str!("fixtures/workflow.json"));
}

#[test]
fn trigger() {
    assert_round_trip::<Trigger>(include_str!("fixtures/trigger.json"));
}

#[test]
fn trigger_with_a_group() {
    assert_round_trip::<Trigger>(include_str!("fixtures/trigger_group.json"));
}

#[test]
fn action() {
    assert_round_trip::<Action>(include_str!("fixtures/action.json"));
}

#[test]
fn document_profile() {
    assert_round_trip::<DocumentProfile>(include_str!("fixtures/document_profile.json"));
}

#[test]
fn execution_log() {
    assert_round_trip::<ExecutionLog>(include_str!("fixtures/execution_log.json"));
}

#[test]
fn the_documented_workflow_is_valid() {
    let workflow: Workflow = serde_json::from_str(include_str!("fixtures/workflow.json")).unwrap();

    assert_eq!(workflow.validate(), Ok(()));
}
