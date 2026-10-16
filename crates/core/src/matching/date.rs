//! Dates dans les noms de fichiers (`Facture_2026-03-15.pdf`).
//!
//! Formats reconnus par défaut : `YYYY-MM-DD`, `DD-MM-YYYY`, `DD.MM.YYYY`,
//! `DD_MM_YYYY`, `YYYYMMDD`, `DDMMYYYY`. Une condition peut imposer son propre
//! format, composé de `YYYY`, `MM`, `DD` et de séparateurs. Les années sont
//! limitées à 1900-2199 pour éviter de prendre un numéro pour une date, et les
//! dates impossibles (30 février) sont rejetées.

use std::sync::LazyLock;

use chrono::NaiveDate;
use regex::Regex;
use thiserror::Error;

/// Années acceptées.
const YEARS: std::ops::RangeInclusive<i32> = 1900..=2199;

/// Formats essayés quand la condition n'en impose pas.
pub const DEFAULT_FORMATS: [&str; 6] = [
    "YYYY-MM-DD",
    "DD-MM-YYYY",
    "DD.MM.YYYY",
    "DD_MM_YYYY",
    "YYYYMMDD",
    "DDMMYYYY",
];

/// Format de date invalide.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
#[error("format de date invalide : il faut YYYY, MM et DD une fois chacun, séparés par des signes")]
pub struct InvalidDateFormat;

/// Format de date analysé.
#[derive(Debug, Clone)]
pub struct DateFormat {
    pattern: Regex,
    /// Ordre des composantes dans le format : année, mois, jour.
    order: [Part; 3],
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Part {
    Year,
    Month,
    Day,
}

impl DateFormat {
    /// Analyse un format comme `DD.MM.YYYY`.
    ///
    /// # Erreurs
    ///
    /// Renvoie [`InvalidDateFormat`] si le format ne contient pas `YYYY`,
    /// `MM` et `DD` exactement une fois, ou contient des lettres ou des
    /// chiffres hors de ces composantes.
    pub fn parse(format: &str) -> Result<Self, InvalidDateFormat> {
        let mut pattern = String::from(r"(?:^|\D)");
        let mut order = Vec::with_capacity(3);
        let mut rest = format;
        while let Some(c) = rest.chars().next() {
            let (part, length, digits) = if rest.starts_with("YYYY") {
                (Some(Part::Year), 4, 4)
            } else if rest.starts_with("MM") {
                (Some(Part::Month), 2, 2)
            } else if rest.starts_with("DD") {
                (Some(Part::Day), 2, 2)
            } else {
                (None, c.len_utf8(), 0)
            };
            match part {
                Some(part) => {
                    if order.contains(&part) {
                        return Err(InvalidDateFormat);
                    }
                    order.push(part);
                    pattern.push_str(&format!(r"(\d{{{digits}}})"));
                }
                None if c.is_alphanumeric() => return Err(InvalidDateFormat),
                None => pattern.push_str(&regex::escape(&rest[..length])),
            }
            rest = &rest[length..];
        }
        pattern.push_str(r"(?:\D|$)");
        let order: [Part; 3] = order.try_into().map_err(|_| InvalidDateFormat)?;
        let pattern = Regex::new(&pattern).map_err(|_| InvalidDateFormat)?;
        Ok(Self { pattern, order })
    }

    /// Première date valide de ce format dans `text`, avec sa position.
    fn find(&self, text: &str) -> Option<(usize, NaiveDate)> {
        // Recherche par décalage : un séparateur consommé par une
        // correspondance invalide ne doit pas masquer une date qui suit.
        let mut start = 0;
        while let Some(captures) = self.pattern.captures_at(text, start) {
            let whole = captures.get(0)?;
            if let Some(date) = self.date(&captures) {
                return Some((whole.start(), date));
            }
            start = next_char_boundary(text, whole.start());
        }
        None
    }

    fn date(&self, captures: &regex::Captures<'_>) -> Option<NaiveDate> {
        let (mut year, mut month, mut day) = (0, 0, 0);
        for (index, part) in self.order.iter().enumerate() {
            let value: u32 = captures.get(index + 1)?.as_str().parse().ok()?;
            match part {
                Part::Year => year = i32::try_from(value).ok()?,
                Part::Month => month = value,
                Part::Day => day = value,
            }
        }
        YEARS
            .contains(&year)
            .then(|| NaiveDate::from_ymd_opt(year, month, day))
            .flatten()
    }
}

fn next_char_boundary(text: &str, index: usize) -> usize {
    text[index..]
        .chars()
        .next()
        .map_or(text.len(), |c| index + c.len_utf8())
}

static DEFAULTS: LazyLock<Vec<DateFormat>> = LazyLock::new(|| {
    DEFAULT_FORMATS
        .iter()
        .filter_map(|format| DateFormat::parse(format).ok())
        .collect()
});

/// Première date trouvée dans `text` avec les formats par défaut : la plus à
/// gauche, et à position égale celle du premier format de la liste.
#[must_use]
pub fn find_date(text: &str) -> Option<NaiveDate> {
    DEFAULTS
        .iter()
        .filter_map(|format| format.find(text))
        .min_by_key(|(position, _)| *position)
        .map(|(_, date)| date)
}

/// Première date trouvée dans `text` avec un format donné.
#[must_use]
pub fn find_date_with(text: &str, format: &DateFormat) -> Option<NaiveDate> {
    format.find(text).map(|(_, date)| date)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn date(year: i32, month: u32, day: u32) -> Option<NaiveDate> {
        NaiveDate::from_ymd_opt(year, month, day)
    }

    #[test]
    fn finds_dates_in_every_default_format() {
        let cases = [
            ("Facture_2026-03-15.pdf", date(2026, 3, 15)),
            ("Releve 15-03-2026.pdf", date(2026, 3, 15)),
            ("PV 15.03.2026.docx", date(2026, 3, 15)),
            ("scan_15_03_2026.pdf", date(2026, 3, 15)),
            ("20260315_scan.pdf", date(2026, 3, 15)),
            ("scan15032026.pdf", date(2026, 3, 15)),
            ("2026-03-15", date(2026, 3, 15)),
        ];
        for (name, expected) in cases {
            assert_eq!(find_date(name), expected, "{name}");
        }
    }

    #[test]
    fn rejects_impossible_dates_and_numbers() {
        let cases = [
            "Facture_2026-02-30.pdf",
            "Contrat_2026-13-01.pdf",
            "IMG_12345678.jpg",
            "Commande_123456789.pdf",
            "Facture.pdf",
            "1850-01-01.pdf",
        ];
        for name in cases {
            assert_eq!(find_date(name), None, "{name}");
        }
    }

    #[test]
    fn keeps_the_leftmost_date() {
        assert_eq!(
            find_date("2026-01-02 copie du 03.04.2025.pdf"),
            date(2026, 1, 2)
        );
        assert_eq!(find_date("x 29-02-2023 puis 29.02.2024"), date(2024, 2, 29));
    }

    #[test]
    fn uses_an_explicit_format() {
        let format = DateFormat::parse("YYYY.MM.DD").unwrap();

        assert_eq!(
            find_date_with("PV 2026.03.15.pdf", &format),
            date(2026, 3, 15)
        );
        assert_eq!(find_date_with("PV 15.03.2026.pdf", &format), None);
        assert_eq!(
            find_date_with("PV 2026.02.30 2026.03.01", &format),
            date(2026, 3, 1)
        );
    }

    #[test]
    fn rejects_invalid_formats() {
        for format in [
            "YYYY-MM",
            "DD-MM-YYYY-DD",
            "YYYY-MM-DDx",
            "AAAA-MM-JJ",
            "YY-MM-DD",
            "",
        ] {
            assert!(DateFormat::parse(format).is_err(), "{format}");
        }
    }
}
