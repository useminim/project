//! Conversions du corpus `fixtures/pdf/` avec la vraie bibliothèque PDFium.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    reason = "fichier de tests : clippy.toml n'autorise unwrap que dans les fonctions #[test]"
)]

use image::{GenericImageView as _, ImageFormat as Format, Rgb, RgbImage};
use minim_core::ports::{
    ConvertError, DocumentConverter as _, DocumentKind, ImageFormat, ReadError, RenderOptions,
};
use minim_extract::{PdfConverter, PdfLimits};

mod common;

const FACTURE: &[u8] = include_bytes!("../../../fixtures/pdf/facture.pdf");
const POLICE: &[u8] = include_bytes!("../../../fixtures/pdf/police.pdf");
const SCAN: &[u8] = include_bytes!("../../../fixtures/pdf/scan.pdf");
const VIDE: &[u8] = include_bytes!("../../../fixtures/pdf/vide.pdf");
const CHIFFRE: &[u8] = include_bytes!("../../../fixtures/pdf/chiffre.pdf");

fn converter() -> PdfConverter {
    PdfConverter::new(common::worker(PdfLimits::default()))
}

fn png(dpi: u16) -> RenderOptions {
    RenderOptions {
        format: ImageFormat::Png,
        dpi,
    }
}

/// Pages rendues, décodées.
fn render(pdf: &[u8], options: RenderOptions) -> Result<Vec<image::DynamicImage>, ConvertError> {
    let mut pages = Vec::new();
    let count = converter().pdf_to_images(pdf, options, &mut |number, bytes| {
        assert_eq!(usize::try_from(number).unwrap(), pages.len() + 1);
        pages.push(image::load_from_memory(&bytes).unwrap());
        Ok(())
    })?;
    assert_eq!(usize::try_from(count).unwrap(), pages.len());
    Ok(pages)
}

#[test]
fn renders_every_page_of_the_corpus_at_150_dpi() {
    // Pages au format Letter d'Edge (612 × 792 points) : 1275 × 1650 pixels à
    // 150 points par pouce.
    for (pdf, pages) in [(FACTURE, 1), (POLICE, 1), (SCAN, 2), (VIDE, 1)] {
        let images = render(pdf, png(150)).unwrap();

        assert_eq!(images.len(), pages);
        for image in images {
            assert_eq!(image.dimensions(), (1275, 1650));
        }
    }
}

#[test]
fn renders_at_the_requested_resolution_and_format() {
    let images = render(FACTURE, png(300)).unwrap();
    assert_eq!(images[0].dimensions(), (2550, 3300));

    let mut sizes = Vec::new();
    for quality in [30, 95] {
        let options = RenderOptions {
            format: ImageFormat::Jpeg { quality },
            dpi: 150,
        };
        let mut size = 0;
        converter()
            .pdf_to_images(FACTURE, options, &mut |_, bytes| {
                assert_eq!(image::guess_format(&bytes).unwrap(), Format::Jpeg);
                size = bytes.len();
                Ok(())
            })
            .unwrap();
        sizes.push(size);
    }
    assert!(sizes[0] < sizes[1], "{sizes:?}");
}

#[test]
fn renders_a_white_page_with_dark_text() {
    let image = render(FACTURE, png(72)).unwrap().remove(0).to_rgb8();

    let pixels = image.pixels().count();
    let white = image
        .pixels()
        .filter(|pixel| pixel.0 == [255, 255, 255])
        .count();
    let dark = image
        .pixels()
        .filter(|pixel| pixel.0.iter().all(|&c| c < 100))
        .count();
    assert!(white > pixels / 2, "fond blanc");
    assert!(dark > 100, "du texte");
}

#[test]
fn reports_unreadable_documents() {
    assert!(matches!(
        render(CHIFFRE, png(150)),
        Err(ConvertError::Read(ReadError::Encrypted))
    ));
    let truncated = &FACTURE[..FACTURE.len() / 2];
    assert!(matches!(
        render(truncated, png(150)),
        Err(ConvertError::Read(ReadError::Corrupted))
    ));
}

#[test]
fn stops_when_writing_a_page_fails() {
    let mut calls = 0;
    let result = converter().pdf_to_images(SCAN, png(72), &mut |_, _| {
        calls += 1;
        Err(std::io::ErrorKind::StorageFull.into())
    });

    assert!(matches!(result, Err(ConvertError::Output(_))));
    assert_eq!(calls, 1);
    // Le fil PDFium reste disponible.
    assert_eq!(render(VIDE, png(72)).unwrap().len(), 1);
}

/// Image de `width` × `height` : moitié gauche rouge, moitié droite bleue.
fn two_colors(width: u32, height: u32) -> RgbImage {
    RgbImage::from_fn(width, height, |x, _| {
        if x < width / 2 {
            Rgb([220, 20, 20])
        } else {
            Rgb([20, 20, 220])
        }
    })
}

fn encode(image: &RgbImage, format: Format) -> Vec<u8> {
    let mut bytes = std::io::Cursor::new(Vec::new());
    image.write_to(&mut bytes, format).unwrap();
    bytes.into_inner()
}

/// Insère dans un JPEG un bloc EXIF qui ne contient que l'orientation.
fn with_exif_orientation(jpeg: &[u8], orientation: u8) -> Vec<u8> {
    let mut exif = b"Exif\0\0MM\0\x2a\0\0\0\x08\0\x01".to_vec();
    exif.extend([0x01, 0x12, 0x00, 0x03, 0, 0, 0, 1, 0, orientation, 0, 0]);
    exif.extend([0, 0, 0, 0]);
    let length = u16::try_from(exif.len() + 2).unwrap().to_be_bytes();
    let mut out = jpeg[..2].to_vec(); // SOI
    out.extend([0xFF, 0xE1, length[0], length[1]]);
    out.extend(exif);
    out.extend(&jpeg[2..]);
    out
}

fn is_red(pixel: Rgb<u8>) -> bool {
    pixel.0[0] > 150 && pixel.0[2] < 100
}

fn is_blue(pixel: Rgb<u8>) -> bool {
    pixel.0[2] > 150 && pixel.0[0] < 100
}

#[test]
fn assembles_images_into_a_pdf_page_per_image() {
    let picture = two_colors(400, 200);
    let png_bytes = encode(&picture, Format::Png);
    let jpeg_bytes = encode(&picture, Format::Jpeg);

    let pdf = converter()
        .images_to_pdf(&[
            (DocumentKind::Png, &png_bytes),
            (DocumentKind::Jpeg, &jpeg_bytes),
        ])
        .unwrap();

    let pages = render(&pdf, png(72)).unwrap();
    assert_eq!(pages.len(), 2);
    for page in pages {
        // Paysage 2:1 ajusté à une page A4 : 842 × 421 points.
        assert_eq!(page.dimensions(), (842, 421));
        let page = page.to_rgb8();
        assert!(is_red(*page.get_pixel(100, 210)), "gauche rouge");
        assert!(is_blue(*page.get_pixel(740, 210)), "droite bleue");
    }
}

#[test]
fn applies_the_exif_orientation_of_a_photo() {
    // Orientation 6 : l'image stockée doit être tournée de 90° dans le sens
    // horaire. Le bord gauche (rouge) se retrouve en haut.
    let jpeg = with_exif_orientation(&encode(&two_colors(400, 200), Format::Jpeg), 6);

    let pdf = converter()
        .images_to_pdf(&[(DocumentKind::Jpeg, &jpeg)])
        .unwrap();

    let page = render(&pdf, png(72)).unwrap().remove(0).to_rgb8();
    assert_eq!(page.dimensions(), (421, 842));
    assert!(is_red(*page.get_pixel(210, 100)), "haut rouge");
    assert!(is_blue(*page.get_pixel(210, 740)), "bas bleu");
}

#[test]
fn keeps_a_jpeg_without_recompressing_it() {
    let jpeg = encode(&two_colors(1600, 1200), Format::Jpeg);

    let pdf = converter()
        .images_to_pdf(&[(DocumentKind::Jpeg, &jpeg)])
        .unwrap();

    assert!(
        pdf.windows(jpeg.len().min(4096))
            .any(|window| window == &jpeg[..jpeg.len().min(4096)]),
        "les octets du JPEG sont incorporés tels quels"
    );
}

#[test]
fn compresses_a_png_in_the_pdf() {
    let picture = two_colors(1600, 1200);
    let png_bytes = encode(&picture, Format::Png);

    let pdf = converter()
        .images_to_pdf(&[(DocumentKind::Png, &png_bytes)])
        .unwrap();

    let raw = picture.as_raw().len();
    assert!(
        pdf.len() < raw / 10,
        "{} octets pour {raw} octets bruts",
        pdf.len()
    );
}

#[test]
fn refuses_what_is_not_a_png_or_a_jpeg() {
    let converter = converter();

    assert!(matches!(
        converter.images_to_pdf(&[(DocumentKind::Docx, b"PK".as_slice())]),
        Err(ConvertError::Read(ReadError::Unsupported(
            DocumentKind::Docx
        )))
    ));
    assert!(matches!(
        converter.images_to_pdf(&[(DocumentKind::Png, b"pas une image".as_slice())]),
        Err(ConvertError::Read(ReadError::Corrupted))
    ));
    assert!(matches!(
        converter.images_to_pdf(&[(DocumentKind::Png, FACTURE)]),
        Err(ConvertError::Read(ReadError::Corrupted))
    ));
}
