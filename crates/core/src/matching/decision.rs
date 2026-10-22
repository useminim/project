//! Décision de correspondance : quels workflows exécuter sur un document
//! (D-10).
//!
//! - Seuls les workflows activés et valides sont évalués, par priorité
//!   croissante puis par identifiant (ordre de création) à priorité égale.
//! - Le premier qui correspond s'exécute ; s'il a `continue_on_match`, les
//!   suivants qui correspondent s'ajoutent à la chaîne, jusqu'au premier qui
//!   ne demande pas à continuer.
//! - Si un résultat `Uncertain` peut changer ce qui sera exécuté (avant la
//!   première correspondance, ou dans la chaîne), rien n'est exécuté : le
//!   document part en zone d'attente avec les candidats.
//! - Un workflow invalide n'est jamais évalué ; il est signalé.

use serde::Serialize;

use crate::id::WorkflowId;
use crate::matching::{Evaluation, FileFacts, evaluate};
use crate::trigger::MatchResult;
use crate::waiting_zone::WaitingReason;
use crate::workflow::Workflow;

/// Ce qu'il faut faire du document.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum MatchDecision {
    /// Exécuter ces workflows, dans cet ordre.
    Execute {
        /// Workflows à exécuter.
        workflows: Vec<WorkflowId>,
    },
    /// Placer le document en zone d'attente.
    WaitingZone {
        /// Raison : aucune correspondance, ou correspondance incertaine.
        reason: WaitingReason,
    },
}

/// Évaluation du déclencheur d'un workflow.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct WorkflowEvaluation {
    /// Workflow évalué.
    pub workflow_id: WorkflowId,
    /// Résultat et explication.
    pub evaluation: Evaluation,
}

/// Décision, avec ce qui l'explique.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
pub struct MatchOutcome {
    /// Ce qu'il faut faire du document.
    pub decision: MatchDecision,
    /// Workflows évalués, dans l'ordre d'évaluation. L'évaluation s'arrête
    /// dès que la décision ne peut plus changer.
    pub evaluations: Vec<WorkflowEvaluation>,
    /// Workflows activés mais invalides, ignorés (à signaler à l'utilisateur).
    pub skipped_invalid: Vec<WorkflowId>,
}

/// Décide quels workflows exécuter sur le fichier `file`. L'ordre de
/// `workflows` est indifférent.
#[must_use]
pub fn decide(workflows: &[Workflow], file: &FileFacts) -> MatchOutcome {
    let mut skipped_invalid = Vec::new();
    let mut ordered: Vec<&Workflow> = Vec::new();
    for workflow in workflows.iter().filter(|workflow| workflow.enabled) {
        if workflow.validate().is_ok() {
            ordered.push(workflow);
        } else {
            skipped_invalid.push(workflow.id);
        }
    }
    ordered.sort_by_key(|workflow| (workflow.priority, workflow.id));
    skipped_invalid.sort();

    let mut evaluations = Vec::new();
    let mut matched = Vec::new();
    let mut candidates = Vec::new();
    let mut uncertain = false;
    for workflow in ordered {
        let evaluation = evaluate(&workflow.trigger, file);
        let result = evaluation.result;
        evaluations.push(WorkflowEvaluation {
            workflow_id: workflow.id,
            evaluation,
        });
        match result {
            MatchResult::Match => {
                matched.push(workflow.id);
                candidates.push(workflow.id);
                if !workflow.continue_on_match {
                    // Fin de la chaîne : les workflows suivants ne peuvent
                    // plus rien changer.
                    break;
                }
            }
            MatchResult::Uncertain => {
                // Il pourrait correspondre : on continue pour connaître tous
                // les candidats possibles.
                uncertain = true;
                candidates.push(workflow.id);
            }
            MatchResult::NoMatch => {}
        }
    }

    let decision = if uncertain {
        MatchDecision::WaitingZone {
            reason: WaitingReason::Uncertain { candidates },
        }
    } else if matched.is_empty() {
        MatchDecision::WaitingZone {
            reason: WaitingReason::NoMatch,
        }
    } else {
        MatchDecision::Execute { workflows: matched }
    };
    MatchOutcome {
        decision,
        evaluations,
        skipped_invalid,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testing::samples;
    use crate::trigger::{Condition, MatchMode, Trigger, TriggerSource};

    /// Comment le déclencheur d'un workflow de test répond pour `facture.pdf`.
    #[derive(Debug, Clone, Copy)]
    enum Answers {
        Yes,
        No,
        Unsure,
    }

    fn condition(answers: Answers) -> Condition {
        match answers {
            Answers::Yes => Condition::FileNameContains {
                value: "facture".to_owned(),
                case_sensitive: false,
            },
            Answers::No => Condition::FileNameContains {
                value: "devis".to_owned(),
                case_sensitive: false,
            },
            Answers::Unsure => Condition::ContentContainsTable { min_rows: None },
        }
    }

    fn workflow(n: u128, priority: u32, answers: Answers, continue_on_match: bool) -> Workflow {
        let mut workflow = samples::workflow(samples::workflow_id(n), priority);
        workflow.trigger = Trigger {
            match_mode: MatchMode::All,
            conditions: vec![condition(answers)],
            source: TriggerSource::Manual,
        };
        workflow.continue_on_match = continue_on_match;
        workflow
    }

    fn ids(ns: &[u128]) -> Vec<WorkflowId> {
        ns.iter().map(|n| samples::workflow_id(*n)).collect()
    }

    fn execute(ns: &[u128]) -> MatchDecision {
        MatchDecision::Execute { workflows: ids(ns) }
    }

    fn uncertain(ns: &[u128]) -> MatchDecision {
        MatchDecision::WaitingZone {
            reason: WaitingReason::Uncertain {
                candidates: ids(ns),
            },
        }
    }

    const NO_MATCH: MatchDecision = MatchDecision::WaitingZone {
        reason: WaitingReason::NoMatch,
    };

    fn decide_for(workflows: &[Workflow]) -> MatchOutcome {
        decide(workflows, &FileFacts::new("facture.pdf", 1))
    }

    use Answers::{No, Unsure, Yes};

    #[test]
    fn applies_the_d10_rules() {
        let cases: Vec<(&str, Vec<Workflow>, MatchDecision)> = vec![
            ("aucun workflow", vec![], NO_MATCH),
            (
                "aucun ne correspond",
                vec![workflow(1, 10, No, false)],
                NO_MATCH,
            ),
            (
                "le plus prioritaire l'emporte",
                vec![workflow(1, 20, Yes, false), workflow(2, 10, Yes, false)],
                execute(&[2]),
            ),
            (
                "égalité de priorité : le plus ancien d'abord",
                vec![workflow(2, 10, Yes, false), workflow(1, 10, Yes, false)],
                execute(&[1]),
            ),
            (
                "continue_on_match accumule les suivants qui correspondent",
                vec![
                    workflow(1, 10, Yes, true),
                    workflow(2, 20, No, false),
                    workflow(3, 30, Yes, true),
                    workflow(4, 40, Yes, false),
                    workflow(5, 50, Yes, false),
                ],
                execute(&[1, 3, 4]),
            ),
            (
                "incertain avant la première correspondance",
                vec![workflow(1, 10, Unsure, false), workflow(2, 20, Yes, false)],
                uncertain(&[1, 2]),
            ),
            (
                "incertain dans la chaîne",
                vec![
                    workflow(1, 10, Yes, true),
                    workflow(2, 20, Unsure, false),
                    workflow(3, 30, Yes, false),
                ],
                uncertain(&[1, 2, 3]),
            ),
            (
                "incertain après la fin de la chaîne : sans effet",
                vec![workflow(1, 10, Yes, false), workflow(2, 20, Unsure, false)],
                execute(&[1]),
            ),
            (
                "seulement des incertains",
                vec![workflow(1, 10, Unsure, false), workflow(2, 20, No, false)],
                uncertain(&[1]),
            ),
        ];
        for (label, workflows, expected) in cases {
            assert_eq!(decide_for(&workflows).decision, expected, "{label}");
        }
    }

    #[test]
    fn ignores_disabled_workflows() {
        let mut disabled = workflow(1, 10, Yes, false);
        disabled.enabled = false;

        let outcome = decide_for(&[disabled.clone(), workflow(2, 20, Yes, false)]);
        assert_eq!(outcome.decision, execute(&[2]));
        assert!(outcome.skipped_invalid.is_empty());

        assert_eq!(decide_for(&[disabled]).decision, NO_MATCH);
    }

    #[test]
    fn skips_and_reports_invalid_workflows() {
        let mut invalid = workflow(1, 10, Yes, false);
        invalid.name = String::new();
        let mut disabled_invalid = workflow(3, 30, Yes, false);
        disabled_invalid.name = String::new();
        disabled_invalid.enabled = false;

        let outcome = decide_for(&[invalid, workflow(2, 20, Yes, false), disabled_invalid]);

        assert_eq!(outcome.decision, execute(&[2]));
        assert_eq!(outcome.skipped_invalid, ids(&[1]));
        assert!(
            outcome
                .evaluations
                .iter()
                .all(|evaluation| evaluation.workflow_id != samples::workflow_id(1)),
            "un workflow invalide n'est jamais évalué"
        );
    }

    #[test]
    fn stops_evaluating_once_the_decision_is_final() {
        let outcome = decide_for(&[
            workflow(1, 10, No, false),
            workflow(2, 20, Yes, false),
            workflow(3, 30, Yes, false),
        ]);

        let evaluated: Vec<_> = outcome.evaluations.iter().map(|e| e.workflow_id).collect();
        assert_eq!(evaluated, ids(&[1, 2]));
        assert_eq!(
            outcome.evaluations[0].evaluation.result,
            MatchResult::NoMatch
        );
    }

    #[test]
    fn serializes_the_decision_for_the_interface() {
        let outcome = decide_for(&[workflow(1, 10, Unsure, false)]);

        assert_eq!(
            serde_json::to_value(&outcome.decision).unwrap(),
            serde_json::json!({
                "kind": "waiting_zone",
                "reason": { "type": "uncertain", "candidates": [samples::workflow_id(1).to_string()] }
            })
        );
    }
}
