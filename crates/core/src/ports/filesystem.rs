//! Accès au disque pour les actions sur les fichiers.
//!
//! Les primitives n'écrasent jamais un fichier existant : un lien physique
//! (`link_new`) et une copie en création exclusive (`copy_new`) échouent avec
//! `ErrorKind::AlreadyExists` si la destination existe, sans course possible
//! entre la vérification et l'écriture. `fs::rename`, qui écrase la
//! destination sous Unix comme sous Windows, n'est pas utilisé.
//!
//! [`StdFileSystem`] agit sur le disque ; [`DryRunFileSystem`] lit le disque
//! mais simule les écritures, pour la simulation d'un workflow.

use std::collections::{HashMap, HashSet};
use std::fs;
use std::io::{self, Write as _};
use std::path::{Path, PathBuf};
use std::sync::{Mutex, PoisonError};
use std::time::SystemTime;

use crate::execution::Fingerprint;

/// Propriétés d'un fichier lues sur le disque.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FileMetadata {
    /// Taille, en octets.
    pub size: u64,
    /// Date de dernière modification, si le système la fournit.
    pub modified: Option<SystemTime>,
    /// Le chemin désigne un fichier (et non un dossier).
    pub is_file: bool,
}

/// Opérations sur le disque utilisées par les actions.
pub trait FileSystem: Send + Sync {
    /// Propriétés du fichier ou du dossier à `path`.
    ///
    /// # Erreurs
    ///
    /// `ErrorKind::NotFound` si rien n'existe à `path`.
    fn metadata(&self, path: &Path) -> io::Result<FileMetadata>;

    /// Crée le dossier `path` et ses parents.
    ///
    /// # Erreurs
    ///
    /// Renvoie une erreur si un dossier ne peut pas être créé.
    fn create_dir_all(&self, path: &Path) -> io::Result<()>;

    /// Crée un lien physique `to` vers `from` : même contenu, aucune copie.
    ///
    /// # Erreurs
    ///
    /// `ErrorKind::AlreadyExists` si `to` existe ; une autre erreur si les
    /// deux chemins sont sur des volumes différents ou si le volume ne prend
    /// pas en charge les liens (FAT, certains partages réseau).
    fn link_new(&self, from: &Path, to: &Path) -> io::Result<()>;

    /// Copie `from` vers `to`, créé exclusivement, puis force l'écriture sur
    /// le disque.
    ///
    /// # Erreurs
    ///
    /// `ErrorKind::AlreadyExists` si `to` existe ; rien n'est alors écrit.
    fn copy_new(&self, from: &Path, to: &Path) -> io::Result<()>;

    /// Supprime le fichier `path`.
    ///
    /// # Erreurs
    ///
    /// Renvoie une erreur si le fichier ne peut pas être supprimé.
    fn remove_file(&self, path: &Path) -> io::Result<()>;

    /// Empreinte blake3 du contenu de `path`.
    ///
    /// # Erreurs
    ///
    /// Renvoie une erreur si le fichier ne peut pas être lu.
    fn fingerprint(&self, path: &Path) -> io::Result<Fingerprint>;

    /// Indique si quelque chose existe à `path`.
    fn exists(&self, path: &Path) -> bool {
        self.metadata(path).is_ok()
    }
}

/// [`FileSystem`] qui agit sur le disque.
#[derive(Debug, Clone, Copy, Default)]
pub struct StdFileSystem;

impl FileSystem for StdFileSystem {
    fn metadata(&self, path: &Path) -> io::Result<FileMetadata> {
        // Un lien symbolique n'est pas suivi : on agit sur le lien lui-même.
        let metadata = fs::symlink_metadata(path)?;
        Ok(FileMetadata {
            size: metadata.len(),
            modified: metadata.modified().ok(),
            is_file: metadata.is_file(),
        })
    }

    fn create_dir_all(&self, path: &Path) -> io::Result<()> {
        fs::create_dir_all(path)
    }

    fn link_new(&self, from: &Path, to: &Path) -> io::Result<()> {
        fs::hard_link(from, to)
    }

    fn copy_new(&self, from: &Path, to: &Path) -> io::Result<()> {
        let mut source = fs::File::open(from)?;
        let mut target = fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(to)?;
        let result = io::copy(&mut source, &mut target)
            .and_then(|_| target.flush())
            .and_then(|()| target.sync_all());
        if result.is_err() {
            // Ne laisse pas de copie partielle derrière soi.
            drop(target);
            let _ = fs::remove_file(to);
        }
        result
    }

    fn remove_file(&self, path: &Path) -> io::Result<()> {
        fs::remove_file(path)
    }

    fn fingerprint(&self, path: &Path) -> io::Result<Fingerprint> {
        let mut hasher = blake3::Hasher::new();
        hasher.update_reader(fs::File::open(path)?)?;
        Ok(Fingerprint::from_bytes(*hasher.finalize().as_bytes()))
    }
}

/// [`FileSystem`] de simulation : les lectures vont au disque, les écritures
/// sont seulement enregistrées en mémoire. Les collisions de noms et les
/// dossiers créés sont donc simulés fidèlement, sans rien modifier.
#[derive(Debug, Default)]
pub struct DryRunFileSystem<F = StdFileSystem> {
    disk: F,
    overlay: Mutex<Overlay>,
}

#[derive(Debug, Default)]
struct Overlay {
    /// Fichiers simulés, avec le fichier réel dont ils ont le contenu.
    created: HashMap<PathBuf, PathBuf>,
    removed: HashSet<PathBuf>,
    directories: HashSet<PathBuf>,
}

impl<F: FileSystem> DryRunFileSystem<F> {
    /// Simulation au-dessus de `disk`.
    pub fn new(disk: F) -> Self {
        Self {
            disk,
            overlay: Mutex::new(Overlay::default()),
        }
    }

    fn overlay(&self) -> std::sync::MutexGuard<'_, Overlay> {
        self.overlay.lock().unwrap_or_else(PoisonError::into_inner)
    }

    /// Fichier réel qui fournit le contenu de `path`.
    fn origin(&self, path: &Path) -> io::Result<PathBuf> {
        let overlay = self.overlay();
        if overlay.removed.contains(path) {
            return Err(io::ErrorKind::NotFound.into());
        }
        Ok(overlay
            .created
            .get(path)
            .cloned()
            .unwrap_or_else(|| path.to_owned()))
    }

    fn create(&self, from: &Path, to: &Path) -> io::Result<()> {
        if self.exists(to) {
            return Err(io::ErrorKind::AlreadyExists.into());
        }
        let origin = self.origin(from)?;
        self.disk.metadata(&origin)?;
        let mut overlay = self.overlay();
        overlay.removed.remove(to);
        overlay.created.insert(to.to_owned(), origin);
        Ok(())
    }
}

impl<F: FileSystem> FileSystem for DryRunFileSystem<F> {
    fn metadata(&self, path: &Path) -> io::Result<FileMetadata> {
        if self.overlay().directories.contains(path) {
            return Ok(FileMetadata {
                size: 0,
                modified: None,
                is_file: false,
            });
        }
        self.disk.metadata(&self.origin(path)?)
    }

    fn create_dir_all(&self, path: &Path) -> io::Result<()> {
        let mut overlay = self.overlay();
        for ancestor in path.ancestors() {
            overlay.directories.insert(ancestor.to_owned());
        }
        Ok(())
    }

    fn link_new(&self, from: &Path, to: &Path) -> io::Result<()> {
        self.create(from, to)
    }

    fn copy_new(&self, from: &Path, to: &Path) -> io::Result<()> {
        self.create(from, to)
    }

    fn remove_file(&self, path: &Path) -> io::Result<()> {
        self.origin(path)?;
        let mut overlay = self.overlay();
        overlay.created.remove(path);
        overlay.removed.insert(path.to_owned());
        Ok(())
    }

    fn fingerprint(&self, path: &Path) -> io::Result<Fingerprint> {
        self.disk.fingerprint(&self.origin(path)?)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn never_overwrites_an_existing_file() {
        let dir = tempfile::tempdir().unwrap();
        let (a, b) = (dir.path().join("a.txt"), dir.path().join("b.txt"));
        fs::write(&a, "a").unwrap();
        fs::write(&b, "b").unwrap();
        let disk = StdFileSystem;

        assert_eq!(
            disk.link_new(&a, &b).unwrap_err().kind(),
            io::ErrorKind::AlreadyExists
        );
        assert_eq!(
            disk.copy_new(&a, &b).unwrap_err().kind(),
            io::ErrorKind::AlreadyExists
        );
        assert_eq!(fs::read_to_string(&b).unwrap(), "b");
    }

    #[test]
    fn copies_and_fingerprints_a_file() {
        let dir = tempfile::tempdir().unwrap();
        let (a, b) = (dir.path().join("a.txt"), dir.path().join("b.txt"));
        fs::write(&a, "contenu").unwrap();
        let disk = StdFileSystem;

        disk.copy_new(&a, &b).unwrap();

        assert_eq!(fs::read_to_string(&b).unwrap(), "contenu");
        assert_eq!(disk.fingerprint(&a).unwrap(), disk.fingerprint(&b).unwrap());
        assert_eq!(
            disk.fingerprint(&a).unwrap().to_string(),
            blake3::hash(b"contenu").to_hex().to_string()
        );
    }

    #[test]
    fn simulates_writes_without_touching_the_disk() {
        let dir = tempfile::tempdir().unwrap();
        let a = dir.path().join("a.txt");
        let moved = dir.path().join("sous-dossier").join("a.txt");
        fs::write(&a, "a").unwrap();
        let dry = DryRunFileSystem::new(StdFileSystem);

        dry.create_dir_all(moved.parent().unwrap()).unwrap();
        dry.link_new(&a, &moved).unwrap();
        dry.remove_file(&a).unwrap();

        assert!(dry.exists(&moved));
        assert!(!dry.exists(&a));
        assert_eq!(
            dry.fingerprint(&moved).unwrap(),
            StdFileSystem.fingerprint(&a).unwrap()
        );
        assert_eq!(
            dry.link_new(&moved, &moved).unwrap_err().kind(),
            io::ErrorKind::AlreadyExists
        );
        // Rien n'a changé sur le disque.
        assert!(a.exists());
        assert!(!moved.parent().unwrap().exists());
    }
}
