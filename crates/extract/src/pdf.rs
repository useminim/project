//! Lecture des PDF avec PDFium (D-34).
//!
//! PDFium ne peut être chargé qu'une fois par processus et n'est pas sûr
//! entre fils : un fil dédié possède l'unique instance et traite les lectures
//! une par une. [`PdfReader`] lui envoie les documents et attend la réponse,
//! dans la limite de durée configurée.
//!
//! Une panique de `pdfium-render` est rattrapée et devient
//! [`ReadError::Corrupted`]. Un crash du code C++ de PDFium, lui, arrête le
//! processus : la lecture devra se faire dans un processus séparé (prompt 30).
//!
//! Les erreurs ne contiennent ni chemin ni contenu de document.

use std::panic::{AssertUnwindSafe, catch_unwind};
use std::path::Path;
use std::sync::OnceLock;
use std::sync::mpsc::{self, RecvTimeoutError, Sender};
use std::thread;
use std::time::Duration;

use chrono::{DateTime, FixedOffset, NaiveDate, TimeZone as _, Utc};
use minim_core::Redacted;
use minim_core::ports::{
    DocumentKind, DocumentMetadata, DocumentReader, Page, PageText, RawDocument, ReadError, Rect,
    TextSpan,
};
use pdfium_render::prelude::{
    PdfDocument, PdfDocumentMetadataTagType, PdfPage, PdfRect, PdfSecurityHandlerRevision, Pdfium,
    PdfiumError, PdfiumInternalError,
};
use thiserror::Error;

/// Limites de lecture d'un PDF.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PdfLimits {
    /// Taille maximale du fichier, en octets.
    pub max_bytes: u64,
    /// Nombre maximal de pages.
    pub max_pages: u32,
    /// Durée maximale d'une lecture.
    pub timeout: Duration,
}

impl Default for PdfLimits {
    /// 200 Mo, 2 000 pages, 60 secondes.
    fn default() -> Self {
        Self {
            max_bytes: 200 * 1024 * 1024,
            max_pages: 2000,
            timeout: Duration::from_secs(60),
        }
    }
}

/// La bibliothèque PDFium n'a pas pu être chargée.
#[derive(Debug, Clone, Error)]
#[error("bibliothèque PDFium introuvable ou incompatible")]
pub struct LibraryUnavailable;

/// Le fil de lecture s'est arrêté.
#[derive(Debug, Clone, Error)]
#[error("le fil de lecture des PDF s'est arrêté")]
struct WorkerStopped;

struct Job {
    bytes: Vec<u8>,
    max_pages: u32,
    reply: Sender<Result<RawDocument, ReadError>>,
}

/// Fil qui possède l'unique instance de PDFium du processus.
static WORKER: OnceLock<Result<Sender<Job>, LibraryUnavailable>> = OnceLock::new();

fn start_worker(library_dir: &Path) -> Result<Sender<Job>, LibraryUnavailable> {
    let library = Pdfium::pdfium_platform_library_name_at_path(library_dir);
    let (ready_tx, ready_rx) = mpsc::channel();
    let (jobs_tx, jobs_rx) = mpsc::channel::<Job>();
    thread::Builder::new()
        .name("minim-pdfium".to_owned())
        .spawn(move || {
            let pdfium = match Pdfium::bind_to_library(library) {
                Ok(bindings) => Pdfium::new(bindings),
                Err(_) => {
                    let _ = ready_tx.send(false);
                    return;
                }
            };
            let _ = ready_tx.send(true);
            for job in jobs_rx {
                let result = catch_unwind(AssertUnwindSafe(|| {
                    parse(&pdfium, &job.bytes, job.max_pages)
                }))
                .unwrap_or(Err(ReadError::Corrupted));
                // Le demandeur a pu abandonner (délai dépassé) : sans effet.
                let _ = job.reply.send(result);
            }
        })
        .map_err(|_| LibraryUnavailable)?;
    match ready_rx.recv() {
        Ok(true) => Ok(jobs_tx),
        _ => Err(LibraryUnavailable),
    }
}

/// Lecteur de PDF.
#[derive(Debug, Clone)]
pub struct PdfReader {
    jobs: Sender<Job>,
    limits: PdfLimits,
}

impl PdfReader {
    /// Démarre le lecteur avec la bibliothèque PDFium du dossier
    /// `library_dir` (ressources de l'application). La bibliothèque n'est
    /// chargée qu'une fois par processus : les appels suivants réutilisent la
    /// même instance, quel que soit le dossier donné.
    ///
    /// # Erreurs
    ///
    /// Renvoie [`ReadError::Unavailable`] si la bibliothèque ne peut pas être
    /// chargée.
    pub fn start(library_dir: &Path, limits: PdfLimits) -> Result<Self, ReadError> {
        let jobs = WORKER
            .get_or_init(|| start_worker(library_dir))
            .clone()
            .map_err(|error| ReadError::Unavailable(Box::new(error)))?;
        Ok(Self { jobs, limits })
    }

    /// Limites appliquées par ce lecteur.
    #[must_use]
    pub const fn limits(&self) -> PdfLimits {
        self.limits
    }
}

impl DocumentReader for PdfReader {
    fn supports(&self, kind: DocumentKind) -> bool {
        kind == DocumentKind::Pdf
    }

    fn read(&self, bytes: &[u8], kind: DocumentKind) -> Result<RawDocument, ReadError> {
        if kind != DocumentKind::Pdf {
            return Err(ReadError::Unsupported(kind));
        }
        if u64::try_from(bytes.len()).unwrap_or(u64::MAX) > self.limits.max_bytes {
            return Err(ReadError::TooLarge {
                limit_bytes: self.limits.max_bytes,
            });
        }
        let (reply, response) = mpsc::channel();
        self.jobs
            .send(Job {
                bytes: bytes.to_vec(),
                max_pages: self.limits.max_pages,
                reply,
            })
            .map_err(|_| ReadError::Unavailable(Box::new(WorkerStopped)))?;
        match response.recv_timeout(self.limits.timeout) {
            Ok(result) => result,
            // Le fil reste occupé par ce document : seul un processus séparé
            // (prompt 30) permettra de l'interrompre.
            Err(RecvTimeoutError::Timeout) => Err(ReadError::Timeout),
            Err(RecvTimeoutError::Disconnected) => {
                Err(ReadError::Unavailable(Box::new(WorkerStopped)))
            }
        }
    }
}

fn map_error(error: &PdfiumError) -> ReadError {
    match error {
        PdfiumError::PdfiumLibraryInternalError(
            PdfiumInternalError::PasswordError | PdfiumInternalError::SecurityError,
        ) => ReadError::Encrypted,
        _ => ReadError::Corrupted,
    }
}

fn parse(pdfium: &Pdfium, bytes: &[u8], max_pages: u32) -> Result<RawDocument, ReadError> {
    let document = pdfium
        .load_pdf_from_byte_slice(bytes, None)
        .map_err(|error| map_error(&error))?;
    let count = u32::try_from(document.pages().len()).unwrap_or(0);
    if count > max_pages {
        return Err(ReadError::TooManyPages { limit: max_pages });
    }
    let metadata = metadata(&document);
    let pages = document
        .pages()
        .iter()
        .zip(1..)
        .map(|(page, number)| read_page(&page, number))
        .collect::<Result<_, _>>()?;
    Ok(RawDocument {
        kind: DocumentKind::Pdf,
        metadata,
        pages,
    })
}

fn metadata(document: &PdfDocument<'_>) -> DocumentMetadata {
    let tag = |kind| {
        document
            .metadata()
            .get(kind)
            .map(|tag| tag.value().trim().to_owned())
            .filter(|value| !value.is_empty())
    };
    DocumentMetadata {
        title: tag(PdfDocumentMetadataTagType::Title).map(Redacted::new),
        author: tag(PdfDocumentMetadataTagType::Author).map(Redacted::new),
        created_at: tag(PdfDocumentMetadataTagType::CreationDate)
            .as_deref()
            .and_then(parse_pdf_date),
        modified_at: tag(PdfDocumentMetadataTagType::ModificationDate)
            .as_deref()
            .and_then(parse_pdf_date),
        producer: tag(PdfDocumentMetadataTagType::Producer),
        restricted: !matches!(
            document.permissions().security_handler_revision(),
            Ok(PdfSecurityHandlerRevision::Unprotected)
        ),
    }
}

fn read_page(page: &PdfPage<'_>, number: u32) -> Result<Page, ReadError> {
    let (width, height) = (page.width().value, page.height().value);
    let text = page.text().map_err(|error| map_error(&error))?;
    let spans = text
        .segments()
        .iter()
        .map(|segment| TextSpan {
            text: segment.text(),
            bounds: fractions(&segment.bounds(), width, height),
            confidence: None,
        })
        .filter(|span| !span.text.trim().is_empty())
        .collect();
    Ok(Page {
        number,
        text: PageText {
            text: text.all(),
            spans,
        },
    })
}

/// Rectangle en points PDF (origine en bas à gauche) converti en fractions de
/// la page, depuis le coin supérieur gauche.
fn fractions(rect: &PdfRect, width: f32, height: f32) -> Rect {
    if width <= 0.0 || height <= 0.0 {
        return Rect {
            x: 0.0,
            y: 0.0,
            width: 0.0,
            height: 0.0,
        };
    }
    let clamp = |value: f32| value.clamp(0.0, 1.0);
    let (left, top) = (
        clamp(rect.left().value / width),
        clamp(1.0 - rect.top().value / height),
    );
    let (right, bottom) = (
        clamp(rect.right().value / width),
        clamp(1.0 - rect.bottom().value / height),
    );
    Rect {
        x: left,
        y: top,
        width: (right - left).max(0.0),
        height: (bottom - top).max(0.0),
    }
}

/// Lit une date PDF : `D:AAAAMMJJHHmmSS` suivi de `Z` ou d'un décalage
/// `+HH'mm'`, les composantes finales étant facultatives.
fn parse_pdf_date(text: &str) -> Option<DateTime<Utc>> {
    let text = text.strip_prefix("D:").unwrap_or(text);
    let digits: String = text.chars().take_while(char::is_ascii_digit).collect();
    if digits.len() < 4 {
        return None;
    }
    let part = |range: std::ops::Range<usize>, default: u32| {
        digits
            .get(range)
            .map_or(Some(default), |value| value.parse().ok())
    };
    let year: i32 = digits.get(0..4)?.parse().ok()?;
    let date = NaiveDate::from_ymd_opt(year, part(4..6, 1)?, part(6..8, 1)?)?;
    let time = date.and_hms_opt(part(8..10, 0)?, part(10..12, 0)?, part(12..14, 0)?)?;

    let zone = &text[digits.len()..];
    let offset_seconds = match zone.chars().next() {
        Some(sign @ ('+' | '-')) => {
            let numbers: Vec<i32> = zone[1..]
                .split(|c: char| !c.is_ascii_digit())
                .filter(|part| !part.is_empty())
                .map(str::parse)
                .collect::<Result<_, _>>()
                .ok()?;
            let seconds = numbers.first().copied().unwrap_or(0) * 3600
                + numbers.get(1).copied().unwrap_or(0) * 60;
            if sign == '-' { -seconds } else { seconds }
        }
        _ => 0,
    };
    FixedOffset::east_opt(offset_seconds)?
        .from_local_datetime(&time)
        .single()
        .map(|date| date.with_timezone(&Utc))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn utc(text: &str) -> Option<DateTime<Utc>> {
        Some(
            DateTime::parse_from_rfc3339(text)
                .unwrap()
                .with_timezone(&Utc),
        )
    }

    #[test]
    fn parses_pdf_dates() {
        let cases = [
            ("D:20260315103000Z", utc("2026-03-15T10:30:00Z")),
            ("D:20260315103000+01'00'", utc("2026-03-15T09:30:00Z")),
            ("D:20260315103000-05'30", utc("2026-03-15T16:00:00Z")),
            ("D:202603", utc("2026-03-01T00:00:00Z")),
            ("20260315", utc("2026-03-15T00:00:00Z")),
            ("D:20261332000000Z", None),
            ("D:26", None),
            ("hier", None),
        ];
        for (text, expected) in cases {
            assert_eq!(parse_pdf_date(text), expected, "{text}");
        }
    }

    #[test]
    fn converts_pdf_points_to_page_fractions() {
        // Page A4 (595 × 842 points) ; rectangle en haut à gauche.
        let rect = PdfRect::new_from_values(742.0, 59.5, 842.0, 297.5);

        let bounds = fractions(&rect, 595.0, 842.0);

        assert!((bounds.x - 0.1).abs() < 1e-4);
        assert!(bounds.y.abs() < 1e-4);
        assert!((bounds.width - 0.4).abs() < 1e-4);
        assert!((bounds.height - 100.0 / 842.0).abs() < 1e-4);
    }
}
