//! Produit les PDF du corpus qu'Edge ne sait pas faire :
//!
//! - `chiffre.pdf` : `facture.pdf` chiffré en AES-128, mot de passe
//!   utilisateur « secret » ;
//! - `boucle.pdf` : arbre des pages qui se contient lui-même ;
//! - `bombe.pdf` : flux de contenu de 1 Mio compressé qui se décompresse en
//!   1 Gio ;
//! - `geante.pdf` : pages de 200 pouces de côté (le maximum de la norme) et
//!   de 100 kilomètres de côté.

use std::collections::BTreeMap;
use std::io::Write as _;
use std::sync::Arc;

use flate2::Compression;
use flate2::write::ZlibEncoder;
use lopdf::encryption::crypt_filters::{Aes128CryptFilter, CryptFilter};
use lopdf::{
    Dictionary, Document, EncryptionState, EncryptionVersion, Object, Permissions, Stream,
    StringFormat, dictionary,
};

type Result<T = ()> = std::result::Result<T, Box<dyn std::error::Error>>;

fn main() -> Result {
    encrypted()?;
    reference_loop()?;
    decompression_bomb()?;
    giant_pages()?;
    Ok(())
}

fn encrypted() -> Result {
    let mut document = Document::load("../facture.pdf")?;
    // Le chiffrement exige un identifiant de fichier ; Edge n'en écrit pas.
    let id = Object::String(b"minimcorpus00001".to_vec(), StringFormat::Hexadecimal);
    document.trailer.set("ID", Object::Array(vec![id.clone(), id]));
    let filter: Arc<dyn CryptFilter> = Arc::new(Aes128CryptFilter);
    let state = EncryptionState::try_from(EncryptionVersion::V4 {
        document: &document,
        encrypt_metadata: true,
        crypt_filters: BTreeMap::from([(b"StdCF".to_vec(), filter)]),
        stream_filter: b"StdCF".to_vec(),
        string_filter: b"StdCF".to_vec(),
        owner_password: "proprietaire",
        user_password: "secret",
        permissions: Permissions::all(),
    })?;
    document.encrypt(&state)?;
    document.save("../chiffre.pdf")?;
    Ok(())
}

/// Document d'une page avec `content` comme flux de contenu.
fn single_page(media_boxes: &[[f32; 4]], content: Stream) -> Document {
    let mut document = Document::with_version("1.7");
    let pages_id = document.new_object_id();
    let content_id = document.add_object(content);
    let kids = media_boxes
        .iter()
        .map(|media_box| {
            let page = dictionary! {
                "Type" => "Page",
                "Parent" => pages_id,
                "MediaBox" => media_box.iter().map(|&v| Object::Real(v)).collect::<Vec<_>>(),
                "Contents" => content_id,
            };
            Object::Reference(document.add_object(page))
        })
        .collect::<Vec<_>>();
    let count = i64::try_from(kids.len()).unwrap_or(0);
    document.objects.insert(
        pages_id,
        Object::Dictionary(dictionary! { "Type" => "Pages", "Kids" => kids, "Count" => count }),
    );
    let catalog = document.add_object(dictionary! { "Type" => "Catalog", "Pages" => pages_id });
    document.trailer.set("Root", catalog);
    document
}

fn text_stream() -> Stream {
    Stream::new(
        Dictionary::new(),
        b"BT /F1 12 Tf 72 712 Td (Page) Tj ET".to_vec(),
    )
}

fn reference_loop() -> Result {
    let mut document = single_page(&[[0.0, 0.0, 612.0, 792.0]], text_stream());
    // L'arbre des pages se contient lui-même.
    let pages_id = document
        .catalog()?
        .get(b"Pages")?
        .as_reference()?;
    let pages = document.get_object_mut(pages_id)?.as_dict_mut()?;
    pages.set("Kids", vec![Object::Reference(pages_id)]);
    document.save("../boucle.pdf")?;
    Ok(())
}

fn decompression_bomb() -> Result {
    let mut encoder = ZlibEncoder::new(Vec::new(), Compression::best());
    let chunk = vec![b' '; 1024 * 1024];
    for _ in 0..1024 {
        encoder.write_all(&chunk)?;
    }
    let compressed = encoder.finish()?;
    let stream = Stream::new(dictionary! { "Filter" => "FlateDecode" }, compressed)
        .with_compression(false);
    let mut document = single_page(&[[0.0, 0.0, 612.0, 792.0]], stream);
    document.save("../bombe.pdf")?;
    Ok(())
}

fn giant_pages() -> Result {
    let mut document = single_page(
        &[[0.0, 0.0, 14_400.0, 14_400.0], [0.0, 0.0, 2.8e8, 2.8e8]],
        text_stream(),
    );
    document.save("../geante.pdf")?;
    Ok(())
}
