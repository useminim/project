//! Rassemblement des informations de diagnostic.
//!
//! La future commande `export_diagnostics` réunira les fichiers de logs, la
//! version de l'application et l'OS dans une archive choisie par
//! l'utilisateur. Ce module fournit ce qu'il faut inclure ; le choix de
//! l'archive et sa création viendront avec les plugins nécessaires.

use std::io;
use std::path::{Path, PathBuf};

use crate::logging::{LOG_FILE_PREFIX, LOG_FILE_SUFFIX};

/// Informations sur l'application et la machine, sans donnée personnelle.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DiagnosticInfo {
    /// Version de l'application.
    pub app_version: &'static str,
    /// Système d'exploitation (`windows`, `macos`, `linux`).
    pub os: &'static str,
    /// Architecture du processeur (`x86_64`, `aarch64`).
    pub arch: &'static str,
}

impl DiagnosticInfo {
    /// Informations de l'application en cours d'exécution.
    #[must_use]
    pub const fn current() -> Self {
        Self {
            app_version: env!("CARGO_PKG_VERSION"),
            os: std::env::consts::OS,
            arch: std::env::consts::ARCH,
        }
    }
}

/// Liste les fichiers de logs de `log_dir`, du plus ancien au plus récent.
///
/// Les autres fichiers du dossier sont ignorés. Un dossier absent donne une
/// liste vide.
///
/// # Erreurs
///
/// Renvoie une erreur si le dossier existe mais ne peut pas être lu.
pub fn log_files(log_dir: &Path) -> io::Result<Vec<PathBuf>> {
    let entries = match std::fs::read_dir(log_dir) {
        Ok(entries) => entries,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(error) => return Err(error),
    };

    let mut files = Vec::new();
    for entry in entries {
        let entry = entry?;
        if entry.file_type()?.is_file() && is_log_file_name(&entry.file_name().to_string_lossy()) {
            files.push(entry.path());
        }
    }
    // Les noms contiennent la date (`minim.2026-10-08.log`) : l'ordre
    // alphabétique est l'ordre chronologique.
    files.sort();
    Ok(files)
}

fn is_log_file_name(name: &str) -> bool {
    name.strip_prefix(LOG_FILE_PREFIX)
        .and_then(|rest| rest.strip_prefix('.'))
        .and_then(|rest| rest.strip_suffix(LOG_FILE_SUFFIX))
        .is_some_and(|date| date.ends_with('.'))
}

#[cfg(test)]
mod tests {
    use std::fs;

    use super::*;

    #[test]
    fn lists_log_files_from_oldest_to_newest() {
        let dir = tempfile::tempdir().unwrap();
        for name in [
            "minim.2026-10-08.log",
            "minim.2026-10-07.log",
            "notes.txt",
            "minim.log",
        ] {
            fs::write(dir.path().join(name), "").unwrap();
        }
        fs::create_dir(dir.path().join("minim.2026-10-09.log")).unwrap();

        let names: Vec<_> = log_files(dir.path())
            .unwrap()
            .iter()
            .map(|path| path.file_name().unwrap().to_string_lossy().into_owned())
            .collect();

        assert_eq!(names, ["minim.2026-10-07.log", "minim.2026-10-08.log"]);
    }

    #[test]
    fn returns_nothing_for_a_missing_directory() {
        let dir = tempfile::tempdir().unwrap();

        assert!(log_files(&dir.path().join("absent")).unwrap().is_empty());
    }

    #[test]
    fn describes_the_running_application() {
        let info = DiagnosticInfo::current();

        assert_eq!(info.app_version, env!("CARGO_PKG_VERSION"));
        assert!(!info.os.is_empty());
        assert!(!info.arch.is_empty());
    }
}
