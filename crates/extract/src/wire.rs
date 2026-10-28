//! Protocole entre l'application et le processus PDFium : messages binaires
//! préfixés par leur longueur, échangés par l'entrée et la sortie standard.
//!
//! Un message est une longueur (`u32`, petit-boutiste) suivie de son contenu.
//! Les entiers sont petit-boutistes, les octets et les textes précédés de leur
//! longueur (`u64`). Le format ne sert qu'entre deux copies du même
//! exécutable : il n'a pas de version.

use std::io::{self, Read, Write};

use chrono::{DateTime, Utc};
use minim_core::Redacted;
use minim_core::ports::{
    ConvertError, DocumentKind, DocumentMetadata, ImageFormat, Page, PageText, RawDocument,
    ReadError, Rect, RenderOptions, TextSpan,
};

/// Taille maximale d'un message (512 Mio).
pub(crate) const MAX_FRAME: usize = 512 * 1024 * 1024;

/// Message mal formé.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
#[error("message du processus PDFium mal formé")]
pub(crate) struct Malformed;

/// Demande de l'application au processus PDFium.
#[derive(Debug, PartialEq)]
pub(crate) enum Request {
    /// Lecture du texte et des métadonnées.
    Read { max_pages: u32, bytes: Vec<u8> },
    /// Rendu des pages en images, renvoyées une par une.
    Render {
        max_pages: u32,
        options: RenderOptions,
        bytes: Vec<u8>,
    },
    /// Assemblage d'images en PDF.
    Compose {
        images: Vec<(DocumentKind, Vec<u8>)>,
    },
}

/// Réponse du processus PDFium.
#[derive(Debug)]
pub(crate) enum Response {
    /// PDFium est chargé, le processus attend des demandes.
    Ready,
    /// La bibliothèque PDFium n'a pas pu être chargée.
    LibraryUnavailable,
    /// Document lu.
    Document(RawDocument),
    /// Image d'une page (numéro à partir de 1).
    Page(u32, Vec<u8>),
    /// Rendu terminé, avec le nombre de pages.
    Done(u32),
    /// PDF assemblé.
    Pdf(Vec<u8>),
    /// Échec de la demande.
    Failed(ConvertError),
}

/// Écrit un message.
pub(crate) fn write_frame(out: &mut impl Write, payload: &[u8]) -> io::Result<()> {
    let length = u32::try_from(payload.len()).map_err(|_| io::ErrorKind::InvalidInput)?;
    out.write_all(&length.to_le_bytes())?;
    out.write_all(payload)?;
    out.flush()
}

/// Lit un message ; `None` à la fin du flux.
pub(crate) fn read_frame(input: &mut impl Read) -> io::Result<Option<Vec<u8>>> {
    let mut length = [0; 4];
    match input.read_exact(&mut length) {
        Ok(()) => {}
        Err(error) if error.kind() == io::ErrorKind::UnexpectedEof => return Ok(None),
        Err(error) => return Err(error),
    }
    let length = usize::try_from(u32::from_le_bytes(length)).unwrap_or(usize::MAX);
    if length > MAX_FRAME {
        return Err(io::ErrorKind::InvalidData.into());
    }
    let mut payload = vec![0; length];
    input.read_exact(&mut payload)?;
    Ok(Some(payload))
}

#[derive(Default)]
struct Writer(Vec<u8>);

impl Writer {
    fn u8(&mut self, value: u8) {
        self.0.push(value);
    }
    fn u16(&mut self, value: u16) {
        self.0.extend(value.to_le_bytes());
    }
    fn u32(&mut self, value: u32) {
        self.0.extend(value.to_le_bytes());
    }
    fn u64(&mut self, value: u64) {
        self.0.extend(value.to_le_bytes());
    }
    fn i64(&mut self, value: i64) {
        self.0.extend(value.to_le_bytes());
    }
    fn f32(&mut self, value: f32) {
        self.0.extend(value.to_le_bytes());
    }
    fn bool(&mut self, value: bool) {
        self.u8(u8::from(value));
    }
    fn bytes(&mut self, value: &[u8]) {
        self.u64(u64::try_from(value.len()).unwrap_or(u64::MAX));
        self.0.extend(value);
    }
    fn text(&mut self, value: &str) {
        self.bytes(value.as_bytes());
    }
    fn option<T>(&mut self, value: Option<T>, write: impl FnOnce(&mut Self, T)) {
        self.bool(value.is_some());
        if let Some(value) = value {
            write(self, value);
        }
    }
    fn date(&mut self, value: Option<DateTime<Utc>>) {
        self.option(value, |w, date| w.i64(date.timestamp_millis()));
    }
    fn kind(&mut self, kind: DocumentKind) {
        self.u8(match kind {
            DocumentKind::Pdf => 0,
            DocumentKind::Png => 1,
            DocumentKind::Jpeg => 2,
            DocumentKind::Tiff => 3,
            DocumentKind::Docx => 4,
            DocumentKind::Xlsx => 5,
        });
    }
}

struct Reader<'a> {
    data: &'a [u8],
}

impl Reader<'_> {
    fn take(&mut self, count: usize) -> Result<&[u8], Malformed> {
        if count > self.data.len() {
            return Err(Malformed);
        }
        let (head, tail) = self.data.split_at(count);
        self.data = tail;
        Ok(head)
    }
    fn array<const N: usize>(&mut self) -> Result<[u8; N], Malformed> {
        self.take(N)?.try_into().map_err(|_| Malformed)
    }
    fn u8(&mut self) -> Result<u8, Malformed> {
        Ok(self.array::<1>()?[0])
    }
    fn u16(&mut self) -> Result<u16, Malformed> {
        Ok(u16::from_le_bytes(self.array()?))
    }
    fn u32(&mut self) -> Result<u32, Malformed> {
        Ok(u32::from_le_bytes(self.array()?))
    }
    fn u64(&mut self) -> Result<u64, Malformed> {
        Ok(u64::from_le_bytes(self.array()?))
    }
    fn i64(&mut self) -> Result<i64, Malformed> {
        Ok(i64::from_le_bytes(self.array()?))
    }
    fn f32(&mut self) -> Result<f32, Malformed> {
        Ok(f32::from_le_bytes(self.array()?))
    }
    fn bool(&mut self) -> Result<bool, Malformed> {
        match self.u8()? {
            0 => Ok(false),
            1 => Ok(true),
            _ => Err(Malformed),
        }
    }
    fn bytes(&mut self) -> Result<Vec<u8>, Malformed> {
        let length = usize::try_from(self.u64()?).map_err(|_| Malformed)?;
        Ok(self.take(length)?.to_vec())
    }
    fn text(&mut self) -> Result<String, Malformed> {
        String::from_utf8(self.bytes()?).map_err(|_| Malformed)
    }
    fn option<T>(
        &mut self,
        read: impl FnOnce(&mut Self) -> Result<T, Malformed>,
    ) -> Result<Option<T>, Malformed> {
        if self.bool()? {
            read(self).map(Some)
        } else {
            Ok(None)
        }
    }
    fn date(&mut self) -> Result<Option<DateTime<Utc>>, Malformed> {
        self.option(|r| DateTime::from_timestamp_millis(r.i64()?).ok_or(Malformed))
    }
    fn kind(&mut self) -> Result<DocumentKind, Malformed> {
        Ok(match self.u8()? {
            0 => DocumentKind::Pdf,
            1 => DocumentKind::Png,
            2 => DocumentKind::Jpeg,
            3 => DocumentKind::Tiff,
            4 => DocumentKind::Docx,
            5 => DocumentKind::Xlsx,
            _ => return Err(Malformed),
        })
    }
    fn finish(&self) -> Result<(), Malformed> {
        if self.data.is_empty() {
            Ok(())
        } else {
            Err(Malformed)
        }
    }
}

impl Request {
    pub(crate) fn encode(&self) -> Vec<u8> {
        let mut w = Writer::default();
        match self {
            Self::Read { max_pages, bytes } => {
                w.u8(1);
                w.u32(*max_pages);
                w.bytes(bytes);
            }
            Self::Render {
                max_pages,
                options,
                bytes,
            } => {
                w.u8(2);
                w.u32(*max_pages);
                w.u16(options.dpi);
                match options.format {
                    ImageFormat::Png => w.u8(0),
                    ImageFormat::Jpeg { quality } => {
                        w.u8(1);
                        w.u8(quality);
                    }
                }
                w.bytes(bytes);
            }
            Self::Compose { images } => {
                w.u8(3);
                w.u32(u32::try_from(images.len()).unwrap_or(u32::MAX));
                for (kind, bytes) in images {
                    w.kind(*kind);
                    w.bytes(bytes);
                }
            }
        }
        w.0
    }

    pub(crate) fn decode(data: &[u8]) -> Result<Self, Malformed> {
        let mut r = Reader { data };
        let request = match r.u8()? {
            1 => Self::Read {
                max_pages: r.u32()?,
                bytes: r.bytes()?,
            },
            2 => {
                let max_pages = r.u32()?;
                let dpi = r.u16()?;
                let format = match r.u8()? {
                    0 => ImageFormat::Png,
                    1 => ImageFormat::Jpeg { quality: r.u8()? },
                    _ => return Err(Malformed),
                };
                Self::Render {
                    max_pages,
                    options: RenderOptions { format, dpi },
                    bytes: r.bytes()?,
                }
            }
            3 => {
                let count = r.u32()?;
                let mut images = Vec::new();
                for _ in 0..count {
                    images.push((r.kind()?, r.bytes()?));
                }
                Self::Compose { images }
            }
            _ => return Err(Malformed),
        };
        r.finish()?;
        Ok(request)
    }
}

impl Response {
    pub(crate) fn encode(&self) -> Vec<u8> {
        let mut w = Writer::default();
        match self {
            Self::Ready => w.u8(0),
            Self::LibraryUnavailable => w.u8(1),
            Self::Document(document) => {
                w.u8(2);
                encode_document(&mut w, document);
            }
            Self::Page(number, bytes) => {
                w.u8(3);
                w.u32(*number);
                w.bytes(bytes);
            }
            Self::Done(count) => {
                w.u8(4);
                w.u32(*count);
            }
            Self::Pdf(bytes) => {
                w.u8(5);
                w.bytes(bytes);
            }
            Self::Failed(error) => {
                w.u8(6);
                encode_error(&mut w, error);
            }
        }
        w.0
    }

    pub(crate) fn decode(data: &[u8]) -> Result<Self, Malformed> {
        let mut r = Reader { data };
        let response = match r.u8()? {
            0 => Self::Ready,
            1 => Self::LibraryUnavailable,
            2 => Self::Document(decode_document(&mut r)?),
            3 => Self::Page(r.u32()?, r.bytes()?),
            4 => Self::Done(r.u32()?),
            5 => Self::Pdf(r.bytes()?),
            6 => Self::Failed(decode_error(&mut r)?),
            _ => return Err(Malformed),
        };
        r.finish()?;
        Ok(response)
    }
}

fn encode_document(w: &mut Writer, document: &RawDocument) {
    w.kind(document.kind);
    let metadata = &document.metadata;
    w.option(metadata.title.as_ref(), |w, title| w.text(title.expose()));
    w.option(metadata.author.as_ref(), |w, author| {
        w.text(author.expose())
    });
    w.date(metadata.created_at);
    w.date(metadata.modified_at);
    w.option(metadata.producer.as_deref(), Writer::text);
    w.bool(metadata.restricted);
    w.u32(u32::try_from(document.pages.len()).unwrap_or(u32::MAX));
    for page in &document.pages {
        w.u32(page.number);
        w.text(&page.text.text);
        w.u32(u32::try_from(page.text.spans.len()).unwrap_or(u32::MAX));
        for span in &page.text.spans {
            w.text(&span.text);
            for value in [
                span.bounds.x,
                span.bounds.y,
                span.bounds.width,
                span.bounds.height,
            ] {
                w.f32(value);
            }
            w.option(span.confidence, Writer::f32);
        }
    }
}

fn decode_document(r: &mut Reader<'_>) -> Result<RawDocument, Malformed> {
    let kind = r.kind()?;
    let metadata = DocumentMetadata {
        title: r.option(Reader::text)?.map(Redacted::new),
        author: r.option(Reader::text)?.map(Redacted::new),
        created_at: r.date()?,
        modified_at: r.date()?,
        producer: r.option(Reader::text)?,
        restricted: r.bool()?,
    };
    let mut pages = Vec::new();
    for _ in 0..r.u32()? {
        let number = r.u32()?;
        let text = r.text()?;
        let mut spans = Vec::new();
        for _ in 0..r.u32()? {
            spans.push(TextSpan {
                text: r.text()?,
                bounds: Rect {
                    x: r.f32()?,
                    y: r.f32()?,
                    width: r.f32()?,
                    height: r.f32()?,
                },
                confidence: r.option(Reader::f32)?,
            });
        }
        pages.push(Page {
            number,
            text: PageText { text, spans },
        });
    }
    Ok(RawDocument {
        kind,
        metadata,
        pages,
    })
}

/// Erreur transmise : seul son type passe, jamais de message système.
fn encode_error(w: &mut Writer, error: &ConvertError) {
    match error {
        ConvertError::Read(error) => match error {
            ReadError::Unsupported(kind) => {
                w.u8(0);
                w.kind(*kind);
            }
            ReadError::Encrypted => w.u8(1),
            ReadError::Corrupted | ReadError::Io(_) => w.u8(2),
            ReadError::TooLarge { limit_bytes } => {
                w.u8(3);
                w.u64(*limit_bytes);
            }
            ReadError::TooManyPages { limit } => {
                w.u8(4);
                w.u32(*limit);
            }
            ReadError::Timeout => w.u8(5),
            ReadError::Unavailable(_) => w.u8(8),
        },
        ConvertError::PageTooLarge => w.u8(6),
        ConvertError::Output(_) => w.u8(7),
    }
}

/// Erreur produite dans le processus PDFium, sans détail.
#[derive(Debug, Clone, Copy, thiserror::Error)]
#[error("erreur dans le processus PDFium")]
pub(crate) struct RemoteError;

fn decode_error(r: &mut Reader<'_>) -> Result<ConvertError, Malformed> {
    Ok(match r.u8()? {
        0 => ReadError::Unsupported(r.kind()?).into(),
        1 => ReadError::Encrypted.into(),
        2 => ReadError::Corrupted.into(),
        3 => ReadError::TooLarge {
            limit_bytes: r.u64()?,
        }
        .into(),
        4 => ReadError::TooManyPages { limit: r.u32()? }.into(),
        5 => ReadError::Timeout.into(),
        6 => ConvertError::PageTooLarge,
        7 => ConvertError::Output(io::Error::other(RemoteError)),
        8 => ReadError::Unavailable(Box::new(RemoteError)).into(),
        _ => return Err(Malformed),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trips_requests() {
        let requests = [
            Request::Read {
                max_pages: 7,
                bytes: b"%PDF".to_vec(),
            },
            Request::Render {
                max_pages: 2,
                options: RenderOptions {
                    format: ImageFormat::Jpeg { quality: 90 },
                    dpi: 300,
                },
                bytes: vec![1, 2, 3],
            },
            Request::Compose {
                images: vec![
                    (DocumentKind::Png, vec![9]),
                    (DocumentKind::Jpeg, Vec::new()),
                ],
            },
        ];
        for request in requests {
            assert_eq!(Request::decode(&request.encode()).unwrap(), request);
        }
    }

    #[test]
    fn round_trips_a_document() {
        let document = RawDocument {
            kind: DocumentKind::Pdf,
            metadata: DocumentMetadata {
                title: Some(Redacted::new("Facture".to_owned())),
                author: None,
                created_at: DateTime::from_timestamp_millis(1_760_000_000_123),
                modified_at: None,
                producer: Some("Skia/PDF".to_owned()),
                restricted: true,
            },
            pages: vec![Page {
                number: 1,
                text: PageText {
                    text: "Total : 12,00 €".to_owned(),
                    spans: vec![TextSpan {
                        text: "Total".to_owned(),
                        bounds: Rect {
                            x: 0.1,
                            y: 0.2,
                            width: 0.3,
                            height: 0.04,
                        },
                        confidence: Some(0.5),
                    }],
                },
            }],
        };

        let Response::Document(decoded) =
            Response::decode(&Response::Document(document.clone()).encode()).unwrap()
        else {
            panic!("document attendu");
        };
        assert_eq!(decoded, document);
    }

    #[test]
    fn round_trips_errors_without_their_message() {
        let errors: [ConvertError; 5] = [
            ReadError::Encrypted.into(),
            ReadError::TooLarge { limit_bytes: 9 }.into(),
            ReadError::Unsupported(DocumentKind::Docx).into(),
            ConvertError::PageTooLarge,
            ConvertError::Output(io::Error::other("/home/alice/secret.png")),
        ];
        for error in errors {
            let Response::Failed(decoded) =
                Response::decode(&Response::Failed(error).encode()).unwrap()
            else {
                panic!("erreur attendue");
            };
            assert!(!format!("{decoded:?}").contains("alice"));
        }
    }

    #[test]
    fn rejects_malformed_messages() {
        for data in [&b""[..], &[9], &[1, 0, 0], &[0, 0], &[4, 1, 0, 0, 0, 0]] {
            assert!(Response::decode(data).is_err() || Request::decode(data).is_err());
        }
        assert_eq!(
            Request::decode(&[1, 0, 0, 0, 0, 255, 0, 0, 0, 0, 0, 0, 0]),
            Err(Malformed)
        );
    }

    #[test]
    fn frames_messages() {
        let mut buffer = Vec::new();
        write_frame(&mut buffer, b"abc").unwrap();
        write_frame(&mut buffer, b"").unwrap();
        let mut input = buffer.as_slice();

        assert_eq!(read_frame(&mut input).unwrap(), Some(b"abc".to_vec()));
        assert_eq!(read_frame(&mut input).unwrap(), Some(Vec::new()));
        assert_eq!(read_frame(&mut input).unwrap(), None);
    }
}
