//! Conversions avec PDFium (D-34) : pages d'un PDF en images PNG ou JPEG, et
//! images en PDF.
//!
//! - Rendu : à la résolution demandée, sur fond blanc, formulaires compris.
//!   Une page dont l'image dépasserait [`MAX_PAGE_PIXELS`] est refusée.
//! - Assemblage : une page par image, à la taille de l'image réduite ou
//!   agrandie pour tenir dans une page A4 (portrait ou paysage selon
//!   l'image). Un JPEG est incorporé tel quel, sans perte ; un PNG est
//!   décodé puis incorporé sans perte. L'orientation EXIF est appliquée.

use std::io::{self, Cursor};

use image::codecs::jpeg::JpegEncoder;
use image::codecs::png::PngEncoder;
use image::metadata::Orientation;
use image::{DynamicImage, ImageDecoder as _, ImageEncoder as _, ImageReader, Limits};
use minim_core::ports::{
    ConvertError, DocumentConverter, DocumentKind, ImageFormat, ReadError, RenderOptions,
};
use pdfium_render::prelude::{
    PdfColor, PdfDocument, PdfMatrix, PdfPageImageObject, PdfPageObjectsCommon as _,
    PdfPagePaperSize, PdfPoints, PdfRenderConfig, Pdfium,
};

use crate::pdf;
use crate::process::{PdfWorker, Step, unexpected};
use crate::wire::{Request, Response};

/// Nombre maximal de pixels d'une page rendue (50 millions : une page A0 à
/// 150 points par pouce, ou A4 à 600).
pub const MAX_PAGE_PIXELS: u64 = 50_000_000;
/// Largeur ou hauteur maximale d'une image, rendue ou lue, en pixels.
pub const MAX_SIDE_PIXELS: u32 = 20_000;

/// Page A4, en points PDF.
const A4: (f32, f32) = (595.28, 841.89);

/// Convertisseur de documents, client du processus PDFium.
#[derive(Debug, Clone)]
pub struct PdfConverter {
    worker: PdfWorker,
}

impl PdfConverter {
    /// Convertisseur qui confie les documents à `worker`.
    #[must_use]
    pub const fn new(worker: PdfWorker) -> Self {
        Self { worker }
    }
}

impl DocumentConverter for PdfConverter {
    fn max_input_bytes(&self) -> u64 {
        self.worker.limits().max_bytes
    }

    fn pdf_to_images(
        &self,
        pdf: &[u8],
        options: RenderOptions,
        page: &mut dyn FnMut(u32, Vec<u8>) -> io::Result<()>,
    ) -> Result<u32, ConvertError> {
        let limits = self.worker.limits();
        limits.check_size(pdf)?;
        let request = Request::Render {
            max_pages: limits.max_pages,
            options,
            bytes: pdf.to_vec(),
        };
        // Les pages arrivent une par une : le délai s'applique à chacune.
        self.worker.exchange(&request, |response| match response {
            Response::Page(number, image) => match page(number, image) {
                Ok(()) => Step::Continue,
                Err(error) => Step::Abort(ConvertError::Output(error)),
            },
            Response::Done(count) => Step::Finish(count),
            _ => unexpected(),
        })
    }

    fn images_to_pdf(&self, images: &[(DocumentKind, &[u8])]) -> Result<Vec<u8>, ConvertError> {
        let limits = self.worker.limits();
        let mut owned = Vec::with_capacity(images.len());
        for (kind, bytes) in images {
            if !matches!(kind, DocumentKind::Png | DocumentKind::Jpeg) {
                return Err(ReadError::Unsupported(*kind).into());
            }
            limits.check_size(bytes)?;
            owned.push((*kind, bytes.to_vec()));
        }
        self.worker.exchange(
            &Request::Compose { images: owned },
            |response| match response {
                Response::Pdf(pdf) => Step::Finish(pdf),
                _ => unexpected(),
            },
        )
    }
}

/// Rend chaque page de `bytes` et la passe à `page`, qui renvoie `false` pour
/// arrêter. Renvoie le nombre de pages rendues.
pub(crate) fn render(
    pdfium: &Pdfium,
    bytes: &[u8],
    max_pages: u32,
    options: RenderOptions,
    page_done: &mut dyn FnMut(u32, Vec<u8>) -> bool,
) -> Result<u32, ConvertError> {
    let document = pdf::open(pdfium, bytes, max_pages)?;
    let scale = f32::from(options.dpi) / 72.0;
    let mut count = 0;
    for (page, number) in document.pages().iter().zip(1..) {
        let (width, height) = pixels(page.width().value * scale, page.height().value * scale)?;
        let config = PdfRenderConfig::new()
            .set_target_size(width, height)
            .set_clear_color(PdfColor::WHITE)
            .render_form_data(true);
        let image = page
            .render_with_config(&config)
            .and_then(|bitmap| bitmap.as_image())
            .map_err(|error| pdf::map_error(&error))?;
        let encoded = encode(&image, options.format)?;
        if !page_done(number, encoded) {
            break;
        }
        count = number;
    }
    Ok(count)
}

/// Taille en pixels d'une page, si elle reste dans les limites.
fn pixels(width: f32, height: f32) -> Result<(i32, i32), ConvertError> {
    let side = |value: f32| {
        let rounded = value.round().max(1.0);
        if rounded > f32::from(u16::MAX) {
            None
        } else {
            // Valeur entière et positive, vérifiée ci-dessus.
            #[expect(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
            Some(rounded as u32)
        }
    };
    let (Some(width), Some(height)) = (side(width), side(height)) else {
        return Err(ConvertError::PageTooLarge);
    };
    if width > MAX_SIDE_PIXELS
        || height > MAX_SIDE_PIXELS
        || u64::from(width) * u64::from(height) > MAX_PAGE_PIXELS
    {
        return Err(ConvertError::PageTooLarge);
    }
    Ok((
        i32::try_from(width).map_err(|_| ConvertError::PageTooLarge)?,
        i32::try_from(height).map_err(|_| ConvertError::PageTooLarge)?,
    ))
}

/// Encode une page rendue (fond blanc : la transparence est inutile).
fn encode(image: &DynamicImage, format: ImageFormat) -> Result<Vec<u8>, ConvertError> {
    let rgb = image.to_rgb8();
    let mut out = Vec::new();
    let (width, height) = rgb.dimensions();
    let result = match format {
        ImageFormat::Png => PngEncoder::new(&mut out).write_image(
            rgb.as_raw(),
            width,
            height,
            image::ExtendedColorType::Rgb8,
        ),
        ImageFormat::Jpeg { quality } => JpegEncoder::new_with_quality(
            &mut out,
            quality.clamp(1, 100),
        )
        .write_image(rgb.as_raw(), width, height, image::ExtendedColorType::Rgb8),
    };
    result.map_err(|error| ConvertError::Output(io::Error::other(error)))?;
    Ok(out)
}

/// Assemble `images` en un PDF, une page par image.
pub(crate) fn compose(
    pdfium: &Pdfium,
    images: &[(DocumentKind, Vec<u8>)],
) -> Result<Vec<u8>, ConvertError> {
    let mut document = pdfium
        .create_new_pdf()
        .map_err(|error| pdf::map_error(&error))?;
    for (_, bytes) in images {
        add_image_page(&mut document, bytes)?;
    }
    document
        .save_to_bytes()
        .map_err(|error| ConvertError::Output(io::Error::other(error)))
}

/// Image lue : format réel (d'après son contenu), dimensions et orientation.
struct ImageInfo {
    is_jpeg: bool,
    width: u32,
    height: u32,
    orientation: Orientation,
}

fn reader(bytes: &[u8]) -> Result<ImageReader<Cursor<&[u8]>>, ConvertError> {
    let mut reader = ImageReader::new(Cursor::new(bytes))
        .with_guessed_format()
        .map_err(|_| ReadError::Corrupted)?;
    let mut limits = Limits::default();
    limits.max_image_width = Some(MAX_SIDE_PIXELS);
    limits.max_image_height = Some(MAX_SIDE_PIXELS);
    reader.limits(limits);
    Ok(reader)
}

fn inspect(bytes: &[u8]) -> Result<ImageInfo, ConvertError> {
    let reader = reader(bytes)?;
    let is_jpeg = match reader.format() {
        Some(image::ImageFormat::Jpeg) => true,
        Some(image::ImageFormat::Png) => false,
        _ => return Err(ReadError::Corrupted.into()),
    };
    let mut decoder = reader.into_decoder().map_err(|_| ReadError::Corrupted)?;
    let (width, height) = decoder.dimensions();
    let orientation = decoder.orientation().unwrap_or(Orientation::NoTransforms);
    if width == 0 || height == 0 || u64::from(width) * u64::from(height) > MAX_PAGE_PIXELS {
        return Err(ConvertError::PageTooLarge);
    }
    Ok(ImageInfo {
        is_jpeg,
        width,
        height,
        orientation,
    })
}

fn add_image_page(document: &mut PdfDocument<'_>, bytes: &[u8]) -> Result<(), ConvertError> {
    let info = inspect(bytes)?;
    let turned = matches!(
        info.orientation,
        Orientation::Rotate90
            | Orientation::Rotate270
            | Orientation::Rotate90FlipH
            | Orientation::Rotate270FlipH
    );
    // Dimensions de l'image telle qu'elle s'affiche, en pixels.
    let (shown_width, shown_height) = if turned {
        (info.height, info.width)
    } else {
        (info.width, info.height)
    };
    let (page_width, page_height) = fit_a4(shown_width, shown_height);

    let corrupted = |_| ConvertError::from(ReadError::Corrupted);
    let mut object = if info.is_jpeg {
        // Le JPEG est incorporé tel quel (aucune recompression).
        PdfPageImageObject::new_from_jpeg_reader(document, Cursor::new(bytes)).map_err(corrupted)?
    } else {
        let image = reader(bytes)?.decode().map_err(|_| ReadError::Corrupted)?;
        PdfPageImageObject::new(document, &image).map_err(corrupted)?
    };
    object
        .apply_matrix(orientation_matrix(
            info.orientation,
            page_width,
            page_height,
        ))
        .map_err(corrupted)?;

    let mut page = document
        .pages_mut()
        .create_page_at_end(PdfPagePaperSize::Custom(
            PdfPoints::new(page_width),
            PdfPoints::new(page_height),
        ))
        .map_err(corrupted)?;
    page.objects_mut()
        .add_image_object(object)
        .map_err(corrupted)?;
    Ok(())
}

/// Taille de page, en points, d'une image de `width` × `height` pixels
/// ajustée à une page A4 dans le sens de l'image.
fn fit_a4(width: u32, height: u32) -> (f32, f32) {
    let (width, height) = (width as f32, height as f32);
    let (max_width, max_height) = if width > height { (A4.1, A4.0) } else { A4 };
    let scale = (max_width / width).min(max_height / height);
    (width * scale, height * scale)
}

/// Matrice qui place une image (carré unité de l'espace image) sur une page
/// de `width` × `height` points, en appliquant l'orientation EXIF : `x = a·u
/// + c·v + e`, `y = b·u + d·v + f`.
fn orientation_matrix(orientation: Orientation, width: f32, height: f32) -> PdfMatrix {
    let (w, h) = (width, height);
    let [a, b, c, d, e, f] = match orientation {
        Orientation::NoTransforms => [w, 0.0, 0.0, h, 0.0, 0.0],
        Orientation::FlipHorizontal => [-w, 0.0, 0.0, h, w, 0.0],
        Orientation::Rotate180 => [-w, 0.0, 0.0, -h, w, h],
        Orientation::FlipVertical => [w, 0.0, 0.0, -h, 0.0, h],
        Orientation::Rotate90FlipH => [0.0, -h, -w, 0.0, w, h],
        Orientation::Rotate90 => [0.0, -h, w, 0.0, 0.0, h],
        Orientation::Rotate270FlipH => [0.0, h, w, 0.0, 0.0, 0.0],
        Orientation::Rotate270 => [0.0, h, -w, 0.0, w, 0.0],
    };
    PdfMatrix::new(a, b, c, d, e, f)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Position sur la page du coin de l'image stockée `(u, v)` (espace
    /// image : `v = 1` en haut).
    fn place(orientation: Orientation, u: f32, v: f32) -> (f32, f32) {
        let m = orientation_matrix(orientation, 2.0, 1.0);
        (m.a() * u + m.c() * v + m.e(), m.b() * u + m.d() * v + m.f())
    }

    #[test]
    fn places_the_stored_top_left_corner_where_exif_says() {
        // Coin supérieur gauche de l'image stockée : (u, v) = (0, 1). Page de
        // 2 × 1 points ; le haut de la page est en y = 1.
        let cases = [
            (Orientation::NoTransforms, (0.0, 1.0)),
            (Orientation::FlipHorizontal, (2.0, 1.0)),
            (Orientation::Rotate180, (2.0, 0.0)),
            (Orientation::FlipVertical, (0.0, 0.0)),
            (Orientation::Rotate90FlipH, (0.0, 1.0)),
            (Orientation::Rotate90, (2.0, 1.0)),
            (Orientation::Rotate270FlipH, (2.0, 0.0)),
            (Orientation::Rotate270, (0.0, 0.0)),
        ];
        for (orientation, expected) in cases {
            assert_eq!(place(orientation, 0.0, 1.0), expected, "{orientation:?}");
        }
    }

    #[test]
    fn fits_images_in_an_a4_page() {
        let (width, height) = fit_a4(1240, 1754);
        assert!((width - 595.28).abs() < 0.5 && (height - 841.89).abs() < 1.0);

        let (width, height) = fit_a4(4000, 3000);
        // Paysage 4:3 : la hauteur est limitée à la largeur d'une page A4.
        assert!((width - 793.71).abs() < 0.01, "{width}");
        assert!((height - 595.28).abs() < 0.01, "{height}");
    }

    #[test]
    fn refuses_pages_too_large_to_render() {
        assert_eq!(pixels(1240.4, 1753.6).unwrap(), (1240, 1754));
        assert!(matches!(
            pixels(20_001.0, 10.0),
            Err(ConvertError::PageTooLarge)
        ));
        assert!(matches!(
            pixels(10_000.0, 10_000.0),
            Err(ConvertError::PageTooLarge)
        ));
        assert!(matches!(
            pixels(f32::INFINITY, 10.0),
            Err(ConvertError::PageTooLarge)
        ));
    }
}
