//! Chiffre `facture.pdf` en AES-128 avec le mot de passe utilisateur
//! « secret » et écrit `chiffre.pdf`, pour tester la lecture d'un PDF protégé.

use std::collections::BTreeMap;
use std::sync::Arc;

use lopdf::encryption::crypt_filters::{Aes128CryptFilter, CryptFilter};
use lopdf::{EncryptionState, EncryptionVersion, Object, Permissions, StringFormat};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut document = lopdf::Document::load("../facture.pdf")?;
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
