//! Valeurs masquées dans les logs.
//!
//! Tout nom de fichier, chemin, texte extrait ou donnée personnelle est
//! enveloppé dans [`Redacted`] avant d'apparaître dans un message de log ou
//! dans une erreur : `Debug` et `Display` n'affichent que `[redacted]`.
//!
//! [`Redacted`] n'implémente volontairement pas `Serialize` : une valeur
//! masquée ne doit pas pouvoir partir vers un fichier, l'interface ou le cloud
//! par inadvertance. Le code qui a réellement besoin de la valeur l'obtient
//! explicitement avec [`Redacted::expose`].
//!
//! ```compile_fail
//! use minim_core::Redacted;
//!
//! let path = Redacted::new("/home/alice/bilan.pdf");
//! let _ = serde_json::to_string(&path);
//! ```

use std::fmt;

/// Texte affiché à la place d'une valeur masquée.
pub const REDACTED: &str = "[redacted]";

/// Enveloppe une valeur sensible pour qu'elle n'apparaisse jamais dans un log.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct Redacted<T>(T);

impl<T> Redacted<T> {
    /// Enveloppe une valeur sensible.
    pub const fn new(value: T) -> Self {
        Self(value)
    }

    /// Donne accès à la valeur. À n'utiliser que pour la traiter, jamais pour
    /// la journaliser.
    pub const fn expose(&self) -> &T {
        &self.0
    }

    /// Renvoie la valeur enveloppée.
    pub fn into_inner(self) -> T {
        self.0
    }
}

impl<T> From<T> for Redacted<T> {
    fn from(value: T) -> Self {
        Self(value)
    }
}

impl<T> fmt::Debug for Redacted<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(REDACTED)
    }
}

impl<T> fmt::Display for Redacted<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(REDACTED)
    }
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::*;

    const SECRET: &str = "/home/alice/dossier-medical.pdf";

    #[test]
    fn hides_the_value_in_debug_and_display() {
        let value = Redacted::new(PathBuf::from(SECRET));

        assert_eq!(format!("{value:?}"), REDACTED);
        assert_eq!(format!("{value:#?}"), REDACTED);
        assert_eq!(format!("{value}"), REDACTED);
    }

    #[test]
    fn hides_the_value_inside_a_derived_debug() {
        #[derive(Debug)]
        #[allow(dead_code)]
        struct Event {
            path: Redacted<String>,
        }

        let event = Event {
            path: Redacted::new(SECRET.to_owned()),
        };

        let output = format!("{event:?}");
        assert!(!output.contains("alice"));
        assert!(output.contains(REDACTED));
    }

    #[test]
    fn exposes_the_value_on_request() {
        let value = Redacted::from(SECRET.to_owned());

        assert_eq!(value.expose(), SECRET);
        assert_eq!(value.into_inner(), SECRET);
    }
}
