//! Comparaison de textes pour la recherche dans un nom de fichier.
//!
//! Les noms sont normalisés en Unicode NFC : macOS écrit souvent les accents
//! en forme décomposée (NFD), Windows en forme composée. En mode souple
//! (`case_sensitive = false`), la casse et les accents sont ignorés :
//! « echeance » trouve « Échéance » (D-32).

use unicode_normalization::UnicodeNormalization as _;
use unicode_normalization::char::is_combining_mark;

/// Forme de comparaison de `text`.
#[must_use]
pub fn comparable(text: &str, case_sensitive: bool) -> String {
    if case_sensitive {
        text.nfc().collect()
    } else {
        text.nfd()
            .filter(|c| !is_combining_mark(*c))
            .flat_map(char::to_lowercase)
            .nfc()
            .collect()
    }
}

/// Indique si `haystack` contient `needle`, selon le mode de comparaison.
#[must_use]
pub fn contains(haystack: &str, needle: &str, case_sensitive: bool) -> bool {
    comparable(haystack, case_sensitive).contains(&comparable(needle, case_sensitive))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ignores_case_and_accents_in_lenient_mode() {
        let cases = [
            ("Facture_Dupont.pdf", "facture", true),
            ("FACTURÉ.pdf", "facture", true),
            ("Échéance_2026.pdf", "echeance", true),
            ("echeance.pdf", "Échéance", true),
            ("Relevé.pdf", "devis", false),
        ];
        for (name, needle, expected) in cases {
            assert_eq!(contains(name, needle, false), expected, "{name} / {needle}");
        }
    }

    #[test]
    fn respects_case_and_accents_in_strict_mode() {
        assert!(contains("Facture.pdf", "Facture", true));
        assert!(!contains("Facture.pdf", "facture", true));
        assert!(!contains("Facturé.pdf", "Facture", true));
    }

    #[test]
    fn matches_decomposed_names_from_macos() {
        // « é » écrit comme « e » suivi de l'accent aigu combinant (NFD).
        let macos = "Re\u{301}sultats.pdf";
        assert!(contains(macos, "Résultats", true));
        assert!(contains(macos, "resultats", false));
        assert!(contains("Résultats.pdf", "Re\u{301}sultats", true));
    }
}
