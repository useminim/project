use proptest::prelude::*;
use serde_json::json;

use super::*;
use crate::field::FieldName;
use crate::trigger::{MatchMode, TriggerSource};

fn file(name: &str) -> FileFacts {
    FileFacts::new(name, 1)
}

fn trigger(match_mode: MatchMode, conditions: Vec<Condition>) -> Trigger {
    Trigger {
        match_mode,
        conditions,
        source: TriggerSource::Manual,
    }
}

fn eval_one(condition: Condition, name: &str) -> MatchResult {
    evaluate(&trigger(MatchMode::All, vec![condition]), &file(name)).result
}

fn contains(value: &str, case_sensitive: bool) -> Condition {
    Condition::FileNameContains {
        value: value.to_owned(),
        case_sensitive,
    }
}

fn extension(values: &[&str]) -> Condition {
    Condition::FileExtensionIs {
        values: values.iter().map(|value| (*value).to_owned()).collect(),
    }
}

fn date(format: Option<&str>) -> Condition {
    Condition::FileNameMatchesDate {
        format: format.map(str::to_owned),
    }
}

use MatchResult::{Match, NoMatch, Uncertain};

#[test]
fn file_name_contains() {
    let cases = [
        ("facture", false, "Facture_Dupont.pdf", Match),
        ("facture", false, "FACTURÉ_mars.pdf", Match),
        ("echeance", false, "Échéance 2026.pdf", Match),
        ("Résultats", true, "Re\u{301}sultats.pdf", Match),
        ("facture", true, "Facture.pdf", NoMatch),
        ("devis", false, "Facture.pdf", NoMatch),
        ("pdf", false, "Facture.pdf", Match),
        ("tar.gz", false, "sauvegarde.tar.gz", Match),
    ];
    for (value, case_sensitive, name, expected) in cases {
        assert_eq!(
            eval_one(contains(value, case_sensitive), name),
            expected,
            "{value} dans {name}"
        );
    }
}

#[test]
fn file_name_matches_date() {
    let cases = [
        (None, "Facture_2026-03-15.pdf", Match),
        (None, "scan_15.03.2026.pdf", Match),
        (None, "Facture_2026-02-30.pdf", NoMatch),
        (None, "Facture.pdf", NoMatch),
        (Some("YYYY.MM.DD"), "PV 2026.03.15.pdf", Match),
        (Some("YYYY.MM.DD"), "PV 15.03.2026.pdf", NoMatch),
        (Some("AAAA-MM-JJ"), "2026-03-15.pdf", NoMatch),
    ];
    for (format, name, expected) in cases {
        assert_eq!(eval_one(date(format), name), expected, "{format:?} {name}");
    }
}

#[test]
fn file_extension_is() {
    let cases = [
        (&["pdf"][..], "Facture.PDF", Match),
        (&["PDF"][..], "Facture.pdf", Match),
        (&["jpg"][..], "photo.jpeg", Match),
        (&["jpeg"][..], "photo.JPG", Match),
        (&["tiff"][..], "scan.tif", Match),
        (&["gz"][..], "sauvegarde.tar.gz", Match),
        (&["tar"][..], "sauvegarde.tar.gz", NoMatch),
        (&["pdf", "png"][..], "scan.png", Match),
        (&["pdf"][..], "LISEZMOI", NoMatch),
        (&["bashrc"][..], ".bashrc", NoMatch),
    ];
    for (values, name, expected) in cases {
        assert_eq!(
            eval_one(extension(values), name),
            expected,
            "{values:?} {name}"
        );
    }
}

#[test]
fn handles_names_of_255_characters() {
    let name = format!("{}_2026-03-15.pdf", "é".repeat(255 - 15));
    assert_eq!(name.chars().count(), 255);

    assert_eq!(eval_one(contains("ÉÉÉ", false), &name), Match);
    assert_eq!(eval_one(date(None), &name), Match);
    assert_eq!(eval_one(extension(&["pdf"]), &name), Match);
}

#[test]
fn content_and_example_conditions_are_not_available_yet() {
    let conditions = [
        Condition::ContentContainsText {
            value: "TVA".to_owned(),
            case_sensitive: false,
        },
        Condition::ContentHasField {
            field: FieldName::new("date").unwrap(),
        },
        Condition::ContentContainsTable { min_rows: None },
        Condition::SimilarToExamples {
            profile_id: "prf_01J8ZK3V9Q4X7M2N5P6R8T0W1Y".parse().unwrap(),
            threshold: 0.8,
        },
    ];
    for condition in conditions {
        let evaluation = evaluate(&trigger(MatchMode::All, vec![condition]), &file("a.pdf"));
        assert_eq!(evaluation.result, Uncertain);
        assert_eq!(
            evaluation.conditions[0].explanation,
            Explanation::NotAvailable
        );
    }
}

#[test]
fn combines_conditions_with_kleene_logic() {
    let unknown = Condition::ContentContainsTable { min_rows: None };
    let name = "Facture_2026-03-15.pdf";
    let cases = [
        (
            MatchMode::All,
            vec![contains("facture", false), extension(&["pdf"])],
            Match,
        ),
        (
            MatchMode::All,
            vec![contains("facture", false), extension(&["png"])],
            NoMatch,
        ),
        (
            MatchMode::All,
            vec![contains("facture", false), unknown.clone()],
            Uncertain,
        ),
        (
            MatchMode::All,
            vec![extension(&["png"]), unknown.clone()],
            NoMatch,
        ),
        (
            MatchMode::Any,
            vec![extension(&["png"]), contains("facture", false)],
            Match,
        ),
        (
            MatchMode::Any,
            vec![extension(&["png"]), unknown.clone()],
            Uncertain,
        ),
        (
            MatchMode::Any,
            vec![contains("facture", false), unknown],
            Match,
        ),
        (
            MatchMode::Any,
            vec![extension(&["png"]), contains("devis", false)],
            NoMatch,
        ),
    ];
    for (mode, conditions, expected) in cases {
        assert_eq!(
            evaluate(&trigger(mode, conditions.clone()), &file(name)).result,
            expected,
            "{mode:?} {conditions:?}"
        );
    }
}

#[test]
fn explains_every_condition_including_groups() {
    let trigger = trigger(
        MatchMode::All,
        vec![
            extension(&["jpeg"]),
            Condition::Group {
                match_mode: MatchMode::Any,
                conditions: vec![contains("devis", false), date(None)],
            },
        ],
    );

    let evaluation = evaluate(&trigger, &file("Scan_15-03-2026.JPG"));

    assert_eq!(evaluation.result, Match);
    assert_eq!(
        serde_json::to_value(&evaluation).unwrap(),
        json!({
            "result": "match",
            "conditions": [
                { "path": "trigger.conditions[0]", "result": "match",
                  "explanation": "extension", "found": "jpg" },
                { "path": "trigger.conditions[1]", "result": "match", "explanation": "group" },
                { "path": "trigger.conditions[1].conditions[0]", "result": "no_match",
                  "explanation": "text_search" },
                { "path": "trigger.conditions[1].conditions[1]", "result": "match",
                  "explanation": "date_found", "date": "2026-03-15" }
            ]
        })
    );
}

#[test]
fn reports_a_missing_extension_and_date() {
    let evaluation = evaluate(
        &trigger(MatchMode::Any, vec![extension(&["pdf"]), date(None)]),
        &file("LISEZMOI"),
    );

    assert_eq!(evaluation.result, NoMatch);
    let explanations: Vec<_> = evaluation
        .conditions
        .iter()
        .map(|c| &c.explanation)
        .collect();
    assert_eq!(
        explanations,
        [&Explanation::NoExtension, &Explanation::NoDate]
    );
}

fn external_condition() -> impl Strategy<Value = Condition> {
    prop_oneof![
        ("[a-zA-Zéè0-9_ .-]{0,6}", any::<bool>())
            .prop_map(|(value, case_sensitive)| contains(&value, case_sensitive)),
        prop_oneof![Just(None), Just(Some("YYYY-MM-DD")), Just(Some("DDMMYYYY"))].prop_map(date),
        prop::collection::vec("[a-zA-Z]{1,4}", 1..3)
            .prop_map(|values| { Condition::FileExtensionIs { values } }),
        Just(Condition::ContentContainsTable { min_rows: None }),
    ]
}

proptest! {
    #[test]
    fn a_single_condition_decides_alone(
        condition in external_condition(),
        name in "[a-zA-Zéè0-9_ .-]{0,30}",
    ) {
        let alone = eval_one(condition.clone(), &name);
        for mode in [MatchMode::All, MatchMode::Any] {
            let evaluation = evaluate(&trigger(mode, vec![condition.clone()]), &file(&name));
            prop_assert_eq!(evaluation.result, alone);
        }
        let grouped = Condition::Group { match_mode: MatchMode::All, conditions: vec![condition] };
        prop_assert_eq!(eval_one(grouped, &name), alone);
    }
}
