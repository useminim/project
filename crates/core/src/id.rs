//! Identifiants typés des entités du domaine.
//!
//! Chaque identifiant est un ULID précédé d'un préfixe propre au type
//! (`wf_01J8ZK3V9Q4X7M2N5P6R8T0W1Y`). Le préfixe rend les identifiants lisibles
//! dans les logs et empêche de confondre deux types d'entités ; l'ULID est
//! triable par date de création et peut être généré hors ligne.

use std::fmt;
use std::str::FromStr;

use serde::de::Error as _;
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use thiserror::Error;
use ulid::Ulid;

/// Erreur de lecture d'un identifiant.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum IdError {
    /// L'identifiant ne commence pas par le préfixe attendu suivi de `_`.
    #[error("l'identifiant doit commencer par « {expected}_ »")]
    Prefix {
        /// Préfixe attendu, sans le `_`.
        expected: &'static str,
    },
    /// La partie qui suit le préfixe n'est pas un ULID valide.
    #[error("ULID invalide : {0}")]
    Ulid(#[from] ulid::DecodeError),
}

macro_rules! prefixed_id {
    ($(#[$meta:meta])* $name:ident, $prefix:literal) => {
        $(#[$meta])*
        #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
        #[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(type = "string"))]
        pub struct $name(Ulid);

        impl $name {
            /// Préfixe de ce type d'identifiant, sans le `_`.
            pub const PREFIX: &'static str = $prefix;

            /// Génère un nouvel identifiant à partir de l'horloge courante.
            #[must_use]
            pub fn generate() -> Self {
                Self(Ulid::generate())
            }

            /// Construit un identifiant à partir d'un ULID existant.
            #[must_use]
            pub const fn from_ulid(ulid: Ulid) -> Self {
                Self(ulid)
            }

            /// Renvoie l'ULID sous-jacent.
            #[must_use]
            pub const fn ulid(self) -> Ulid {
                self.0
            }
        }

        impl fmt::Display for $name {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                write!(f, "{}_{}", Self::PREFIX, self.0)
            }
        }

        impl FromStr for $name {
            type Err = IdError;

            fn from_str(s: &str) -> Result<Self, Self::Err> {
                let ulid = s
                    .strip_prefix(Self::PREFIX)
                    .and_then(|rest| rest.strip_prefix('_'))
                    .ok_or(IdError::Prefix { expected: Self::PREFIX })?;
                Ok(Self(Ulid::from_string(ulid)?))
            }
        }

        impl Serialize for $name {
            fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
                serializer.collect_str(self)
            }
        }

        impl<'de> Deserialize<'de> for $name {
            fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
                let s = String::deserialize(deserializer)?;
                s.parse().map_err(D::Error::custom)
            }
        }
    };
}

prefixed_id!(
    /// Identifiant d'un workflow (`wf_…`).
    WorkflowId,
    "wf"
);
prefixed_id!(
    /// Identifiant d'un document reçu par minim (`doc_…`).
    DocumentId,
    "doc"
);
prefixed_id!(
    /// Identifiant d'une exécution de workflow (`run_…`).
    RunId,
    "run"
);
prefixed_id!(
    /// Identifiant d'un utilisateur (`usr_…`).
    UserId,
    "usr"
);
prefixed_id!(
    /// Identifiant d'une organisation, c'est-à-dire d'un compte entreprise (`org_…`).
    OrganizationId,
    "org"
);
prefixed_id!(
    /// Identifiant d'un profil appris à partir de documents exemples (`prf_…`).
    ProfileId,
    "prf"
);

#[cfg(test)]
mod tests {
    use super::*;

    const ULID: &str = "01J8ZK3V9Q4X7M2N5P6R8T0W1Y";

    #[test]
    fn displays_prefix_and_ulid() {
        let id: WorkflowId = format!("wf_{ULID}").parse().unwrap();
        assert_eq!(id.to_string(), format!("wf_{ULID}"));
    }

    #[test]
    fn generated_ids_round_trip() {
        let id = DocumentId::generate();
        assert_eq!(id.to_string().parse::<DocumentId>().unwrap(), id);
    }

    #[test]
    fn rejects_wrong_prefix() {
        assert_eq!(
            format!("doc_{ULID}").parse::<WorkflowId>(),
            Err(IdError::Prefix { expected: "wf" })
        );
        assert_eq!(
            format!("wf{ULID}").parse::<WorkflowId>(),
            Err(IdError::Prefix { expected: "wf" })
        );
    }

    #[test]
    fn rejects_invalid_ulid() {
        assert!(matches!(
            "wf_pas-un-ulid".parse::<WorkflowId>(),
            Err(IdError::Ulid(_))
        ));
    }

    #[test]
    fn serializes_as_a_string() {
        let id: UserId = format!("usr_{ULID}").parse().unwrap();
        let json = serde_json::to_string(&id).unwrap();
        assert_eq!(json, format!("\"usr_{ULID}\""));
        assert_eq!(serde_json::from_str::<UserId>(&json).unwrap(), id);
        assert!(serde_json::from_str::<UserId>(&format!("\"org_{ULID}\"")).is_err());
    }
}
