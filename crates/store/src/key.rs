//! Clé de la base et fournisseurs de clé.
//!
//! La clé est aléatoire (256 bits) et n'est jamais dérivée d'un mot de passe.
//! Elle est gardée par un [`KeyProvider`] : le trousseau du système dans
//! l'application, un fichier en repli sous Linux sans Secret Service, la
//! mémoire dans les tests (D-31). Elle est effacée de la mémoire quand elle
//! est libérée.

use std::fmt;
use std::fs;
use std::io;
use std::path::PathBuf;
use std::sync::{Mutex, PoisonError};

use thiserror::Error;
use zeroize::Zeroizing;

/// Taille de la clé, en octets (256 bits).
pub const KEY_LEN: usize = 32;

/// Clé de chiffrement de la base. `Debug` n'affiche jamais sa valeur.
#[derive(Clone, PartialEq, Eq)]
pub struct DatabaseKey(Zeroizing<[u8; KEY_LEN]>);

impl DatabaseKey {
    /// Tire une nouvelle clé au hasard (générateur du système).
    ///
    /// # Erreurs
    ///
    /// Renvoie [`KeyError::Random`] si le générateur du système échoue.
    pub fn generate() -> Result<Self, KeyError> {
        let mut bytes = Zeroizing::new([0_u8; KEY_LEN]);
        getrandom::fill(bytes.as_mut()).map_err(KeyError::Random)?;
        Ok(Self(bytes))
    }

    /// Reconstruit une clé à partir de ses octets.
    ///
    /// # Erreurs
    ///
    /// Renvoie [`KeyError::InvalidLength`] si `bytes` ne fait pas 32 octets.
    pub fn from_bytes(bytes: &[u8]) -> Result<Self, KeyError> {
        let array: [u8; KEY_LEN] = bytes.try_into().map_err(|_| KeyError::InvalidLength)?;
        Ok(Self(Zeroizing::new(array)))
    }

    /// Octets de la clé, à n'utiliser que pour la transmettre à un stockage.
    #[must_use]
    pub fn as_bytes(&self) -> &[u8; KEY_LEN] {
        &self.0
    }

    /// Clé brute au format attendu par `PRAGMA key` : `x'…'` en hexadécimal.
    pub(crate) fn sqlcipher_literal(&self) -> Zeroizing<String> {
        let mut literal = Zeroizing::new(String::with_capacity(KEY_LEN * 2 + 3));
        literal.push_str("x'");
        for byte in self.0.iter() {
            literal.push(hex_digit(byte >> 4));
            literal.push(hex_digit(byte & 0x0f));
        }
        literal.push('\'');
        literal
    }
}

const fn hex_digit(value: u8) -> char {
    match value {
        0..=9 => (b'0' + value) as char,
        _ => (b'a' + value - 10) as char,
    }
}

impl fmt::Debug for DatabaseKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("DatabaseKey([redacted])")
    }
}

/// Échec d'un accès à la clé.
#[derive(Debug, Error)]
pub enum KeyError {
    /// Le trousseau ou le fichier de clé est inaccessible : la base ne doit
    /// pas être ouverte, ni une nouvelle clé créée.
    #[error("stockage de la clé inaccessible")]
    Unavailable(#[source] Box<dyn std::error::Error + Send + Sync>),
    /// La clé enregistrée n'a pas la bonne taille.
    #[error("clé enregistrée invalide")]
    InvalidLength,
    /// Le générateur aléatoire du système a échoué.
    #[error("générateur aléatoire du système indisponible")]
    Random(#[source] getrandom::Error),
}

/// Garde la clé de la base.
pub trait KeyProvider: Send + Sync {
    /// Renvoie la clé enregistrée, ou `None` s'il n'y en a pas encore (premier
    /// lancement, ou clé perdue).
    ///
    /// # Erreurs
    ///
    /// Renvoie [`KeyError::Unavailable`] si le stockage de la clé ne répond
    /// pas : ce n'est pas la même chose qu'une clé absente.
    fn load(&self) -> Result<Option<DatabaseKey>, KeyError>;

    /// Enregistre la clé, en remplaçant la précédente.
    ///
    /// # Erreurs
    ///
    /// Renvoie [`KeyError::Unavailable`] si le stockage de la clé ne répond
    /// pas.
    fn save(&self, key: &DatabaseKey) -> Result<(), KeyError>;
}

/// Clé gardée en mémoire, pour les tests.
#[derive(Debug, Default)]
pub struct InMemoryKeyProvider {
    key: Mutex<Option<DatabaseKey>>,
}

impl InMemoryKeyProvider {
    /// Fournisseur qui détient déjà `key`.
    #[must_use]
    pub fn with_key(key: DatabaseKey) -> Self {
        Self {
            key: Mutex::new(Some(key)),
        }
    }
}

impl KeyProvider for InMemoryKeyProvider {
    fn load(&self) -> Result<Option<DatabaseKey>, KeyError> {
        Ok(self
            .key
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .clone())
    }

    fn save(&self, key: &DatabaseKey) -> Result<(), KeyError> {
        *self.key.lock().unwrap_or_else(PoisonError::into_inner) = Some(key.clone());
        Ok(())
    }
}

/// Clé gardée dans un fichier lisible par le seul utilisateur (droits 0600
/// sous Unix). Repli quand le trousseau est indisponible : la protection ne
/// tient alors qu'aux droits du compte (D-31).
#[derive(Debug, Clone)]
pub struct FileKeyProvider {
    path: PathBuf,
}

impl FileKeyProvider {
    /// Fournisseur qui garde la clé dans `path`.
    #[must_use]
    pub const fn new(path: PathBuf) -> Self {
        Self { path }
    }
}

fn unavailable(error: io::Error) -> KeyError {
    KeyError::Unavailable(Box::new(error))
}

impl KeyProvider for FileKeyProvider {
    fn load(&self) -> Result<Option<DatabaseKey>, KeyError> {
        match fs::read(&self.path) {
            Ok(bytes) => DatabaseKey::from_bytes(&Zeroizing::new(bytes)).map(Some),
            Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(None),
            Err(error) => Err(unavailable(error)),
        }
    }

    fn save(&self, key: &DatabaseKey) -> Result<(), KeyError> {
        use std::io::Write as _;

        if let Some(parent) = self.path.parent() {
            fs::create_dir_all(parent).map_err(unavailable)?;
        }
        let mut options = fs::OpenOptions::new();
        options.write(true).create(true).truncate(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt as _;
            options.mode(0o600);
        }
        let mut file = options.open(&self.path).map_err(unavailable)?;
        file.write_all(key.as_bytes()).map_err(unavailable)?;
        file.sync_all().map_err(unavailable)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn generates_distinct_keys() {
        assert_ne!(
            DatabaseKey::generate().unwrap(),
            DatabaseKey::generate().unwrap()
        );
    }

    #[test]
    fn never_prints_the_key() {
        let key = DatabaseKey::from_bytes(&[0xab; KEY_LEN]).unwrap();

        assert_eq!(format!("{key:?}"), "DatabaseKey([redacted])");
    }

    #[test]
    fn formats_the_raw_key_for_sqlcipher() {
        let key = DatabaseKey::from_bytes(&[0x0f; KEY_LEN]).unwrap();

        assert_eq!(
            *key.sqlcipher_literal(),
            format!("x'{}'", "0f".repeat(KEY_LEN))
        );
    }

    #[test]
    fn rejects_a_key_of_the_wrong_length() {
        assert!(matches!(
            DatabaseKey::from_bytes(&[0; 16]),
            Err(KeyError::InvalidLength)
        ));
    }

    #[test]
    fn keeps_the_key_in_a_private_file() {
        let dir = tempfile::tempdir().unwrap();
        let provider = FileKeyProvider::new(dir.path().join("cle").join("minim.key"));
        assert!(provider.load().unwrap().is_none());

        let key = DatabaseKey::generate().unwrap();
        provider.save(&key).unwrap();

        assert_eq!(provider.load().unwrap(), Some(key));
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt as _;
            let mode = fs::metadata(dir.path().join("cle").join("minim.key"))
                .unwrap()
                .permissions()
                .mode();
            assert_eq!(mode & 0o777, 0o600);
        }
    }
}
