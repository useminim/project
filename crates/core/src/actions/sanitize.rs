//! Assainissement des noms de fichiers produits par minim.
//!
//! Les règles de Windows, les plus strictes, sont appliquées sur tous les OS :
//! un workflow synchronisé produit ainsi les mêmes noms partout, et un fichier
//! reste copiable vers un disque ou un partage Windows.
//!
//! - les caractères interdits (`< > : " / \ | ? *` et les caractères de
//!   contrôle) sont remplacés par `_` : un nom ne peut donc contenir aucun
//!   séparateur ni sortir du dossier cible ;
//! - les points et espaces finaux sont retirés ;
//! - un nom réservé (`CON`, `PRN`, `AUX`, `NUL`, `COM1`…`COM9`, `LPT1`…`LPT9`,
//!   avec ou sans extension, sans tenir compte de la casse) reçoit un `_` ;
//! - un nom de plus de 255 octets est raccourci en gardant son extension.
//!
//! Les chemins de plus de 260 caractères sont pris en charge par la
//! bibliothèque standard de Rust sous Windows (préfixe `\\?\` ajouté
//! automatiquement).

use thiserror::Error;

/// Longueur maximale d'un nom de fichier, en octets (NTFS, APFS, ext4).
pub const MAX_NAME_BYTES: usize = 255;

const FORBIDDEN: [char; 9] = ['<', '>', ':', '"', '/', '\\', '|', '?', '*'];

const RESERVED: [&str; 22] = [
    "CON", "PRN", "AUX", "NUL", "COM1", "COM2", "COM3", "COM4", "COM5", "COM6", "COM7", "COM8",
    "COM9", "LPT1", "LPT2", "LPT3", "LPT4", "LPT5", "LPT6", "LPT7", "LPT8", "LPT9",
];

/// Le nom ne contient plus rien d'utilisable après assainissement.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
#[error("le nom de fichier produit est vide")]
pub struct EmptyFileName;

/// Rend `name` utilisable comme nom de fichier sur tous les OS.
///
/// # Erreurs
///
/// Renvoie [`EmptyFileName`] si le nom est vide après assainissement (par
/// exemple `..` ou `   `).
pub fn sanitize_file_name(name: &str) -> Result<String, EmptyFileName> {
    let replaced: String = name
        .chars()
        .map(|c| {
            if FORBIDDEN.contains(&c) || c.is_control() {
                '_'
            } else {
                c
            }
        })
        .collect();
    let trimmed = replaced
        .trim_end_matches(['.', ' '])
        .trim_start_matches(' ');
    if trimmed.is_empty() {
        return Err(EmptyFileName);
    }

    // Windows compare la partie qui précède le premier point, espaces finaux
    // ignorés : « con.tar.gz » et « CON .pdf » sont réservés.
    let (base, rest) = trimmed.split_at(trimmed.find('.').unwrap_or(trimmed.len()));
    let reserved = RESERVED
        .iter()
        .any(|reserved| base.trim_end().eq_ignore_ascii_case(reserved));
    let name = if reserved {
        format!("{}_{rest}", base.trim_end())
    } else {
        trimmed.to_owned()
    };
    Ok(fit_length(&name, ""))
}

/// Nom avec un suffixe avant l'extension, pour éviter une collision :
/// `Facture.pdf` → `Facture (2).pdf`. Le résultat tient en 255 octets.
#[must_use]
pub fn with_collision_suffix(name: &str, number: u32) -> String {
    fit_length(name, &format!(" ({number})"))
}

/// Insère `suffix` avant la dernière extension et raccourcit le radical (à une
/// frontière de caractère) pour que le tout tienne en [`MAX_NAME_BYTES`].
fn fit_length(name: &str, suffix: &str) -> String {
    let (stem, extension) = match name.rfind('.') {
        Some(dot) if dot > 0 => name.split_at(dot),
        _ => (name, ""),
    };
    let budget = MAX_NAME_BYTES.saturating_sub(suffix.len() + extension.len());
    let mut end = stem.len().min(budget);
    while !stem.is_char_boundary(end) {
        end -= 1;
    }
    format!("{}{suffix}{extension}", stem[..end].trim_end())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn applies_the_windows_rules_everywhere() {
        let cases = [
            ("Facture Dupont.pdf", "Facture Dupont.pdf"),
            ("Facture: mars?.pdf", "Facture_ mars_.pdf"),
            ("a/b\\c.pdf", "a_b_c.pdf"),
            ("..\\..\\secret", ".._.._secret"),
            ("tab\there", "tab_here"),
            ("facture.", "facture"),
            ("facture . ", "facture"),
            ("CON.pdf", "CON_.pdf"),
            ("con", "con_"),
            ("Lpt9.tar.gz", "Lpt9_.tar.gz"),
            ("CON .pdf", "CON_.pdf"),
            ("CONSOLE.pdf", "CONSOLE.pdf"),
            ("COM10.pdf", "COM10.pdf"),
            ("Échéance été 2026.pdf", "Échéance été 2026.pdf"),
            ("報告書.pdf", "報告書.pdf"),
        ];
        for (name, expected) in cases {
            assert_eq!(sanitize_file_name(name).as_deref(), Ok(expected), "{name}");
        }
    }

    #[test]
    fn refuses_names_that_end_up_empty() {
        for name in ["", "..", ".", "   ", " . . "] {
            assert_eq!(sanitize_file_name(name), Err(EmptyFileName), "{name:?}");
        }
    }

    #[test]
    fn shortens_long_names_and_keeps_the_extension() {
        let long = format!("{}.pdf", "é".repeat(200));

        let sanitized = sanitize_file_name(&long).unwrap();

        assert!(sanitized.len() <= MAX_NAME_BYTES);
        assert!(sanitized.ends_with("é.pdf"));
    }

    #[test]
    fn adds_a_collision_suffix_before_the_extension() {
        assert_eq!(with_collision_suffix("Facture.pdf", 2), "Facture (2).pdf");
        assert_eq!(
            with_collision_suffix("archive.tar.gz", 3),
            "archive.tar (3).gz"
        );
        assert_eq!(with_collision_suffix("LISEZMOI", 2), "LISEZMOI (2)");
        assert_eq!(with_collision_suffix(".bashrc", 2), ".bashrc (2)");

        let long = format!("{}.pdf", "a".repeat(251));
        let suffixed = with_collision_suffix(&long, 12);
        assert_eq!(suffixed.len(), MAX_NAME_BYTES);
        assert!(suffixed.ends_with(" (12).pdf"));
    }
}
