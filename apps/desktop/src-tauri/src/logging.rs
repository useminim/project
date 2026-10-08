//! Logs de l'application et journalisation des paniques.
//!
//! Les logs sont écrits dans un fichier par jour dans le dossier de logs de
//! l'application (14 fichiers conservés), et aussi dans la console en
//! développement. Le niveau par défaut est `info` ; la variable d'environnement
//! `MINIM_LOG` le remplace (syntaxe `EnvFilter`, ex. `MINIM_LOG=minim_core=debug`).
//!
//! Aucun contenu de document, donnée personnelle, nom de fichier ni chemin
//! n'est journalisé : ces valeurs passent par `minim_core::Redacted`, et les
//! événements d'un document sont corrélés par son `document_id`.

use std::io;
use std::path::Path;

use thiserror::Error;
use tracing::Subscriber;
use tracing::level_filters::LevelFilter;
use tracing_appender::non_blocking::WorkerGuard;
use tracing_appender::rolling::{InitError, RollingFileAppender, Rotation};
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::util::{SubscriberInitExt, TryInitError};
use tracing_subscriber::{EnvFilter, Layer, fmt};

/// Variable d'environnement qui remplace le niveau de log par défaut.
pub const LOG_ENV_VAR: &str = "MINIM_LOG";

/// Préfixe des fichiers de logs (`minim.2026-10-08.log`).
pub const LOG_FILE_PREFIX: &str = "minim";

/// Extension des fichiers de logs.
pub const LOG_FILE_SUFFIX: &str = "log";

/// Nombre de fichiers journaliers conservés.
pub const MAX_LOG_FILES: usize = 14;

/// Échec de la mise en place des logs.
#[derive(Debug, Error)]
pub enum LoggingError {
    /// Le dossier de logs ne peut pas être créé.
    #[error("impossible de créer le dossier de logs")]
    CreateDir(#[source] io::Error),
    /// Le fichier de logs ne peut pas être ouvert.
    #[error("impossible d'ouvrir le fichier de logs")]
    OpenFile(#[source] InitError),
    /// Un collecteur de logs global est déjà installé.
    #[error("les logs sont déjà initialisés")]
    AlreadyInitialized(#[source] TryInitError),
}

/// Garde l'écriture des logs active ; les derniers messages sont écrits sur le
/// disque quand elle est libérée. À conserver pendant toute la vie de
/// l'application.
#[must_use = "les logs cessent d'être écrits quand la garde est libérée"]
pub struct LogGuard {
    _worker: WorkerGuard,
}

/// Installe les logs dans `log_dir`, puis le hook qui journalise les paniques.
///
/// # Erreurs
///
/// Renvoie une erreur si le dossier ou le fichier de logs ne peut pas être
/// créé, ou si des logs sont déjà initialisés.
pub fn init(log_dir: &Path) -> Result<LogGuard, LoggingError> {
    let directives = std::env::var(LOG_ENV_VAR).ok();
    let filter = env_filter(directives.as_deref());
    let (subscriber, guard) = build_subscriber(log_dir, filter, cfg!(debug_assertions))?;
    subscriber
        .try_init()
        .map_err(LoggingError::AlreadyInitialized)?;
    install_panic_hook();
    Ok(guard)
}

/// Construit le collecteur sans l'installer, pour pouvoir le tester.
fn build_subscriber(
    log_dir: &Path,
    filter: EnvFilter,
    console: bool,
) -> Result<(impl Subscriber + Send + Sync + 'static, LogGuard), LoggingError> {
    std::fs::create_dir_all(log_dir).map_err(LoggingError::CreateDir)?;
    let appender = RollingFileAppender::builder()
        .rotation(Rotation::DAILY)
        .filename_prefix(LOG_FILE_PREFIX)
        .filename_suffix(LOG_FILE_SUFFIX)
        .max_log_files(MAX_LOG_FILES)
        .build(log_dir)
        .map_err(LoggingError::OpenFile)?;
    let (writer, worker) = tracing_appender::non_blocking(appender);

    let file_layer = fmt::layer().with_ansi(false).with_writer(writer);
    let console_layer = console.then(|| fmt::layer().with_writer(io::stdout).boxed());
    let subscriber = tracing_subscriber::registry()
        .with(filter)
        .with(file_layer)
        .with(console_layer);

    Ok((subscriber, LogGuard { _worker: worker }))
}

/// Niveau `info` par défaut, remplacé par les directives de `MINIM_LOG`. Une
/// directive invalide est ignorée plutôt que d'empêcher le démarrage.
fn env_filter(directives: Option<&str>) -> EnvFilter {
    EnvFilter::builder()
        .with_default_directive(LevelFilter::INFO.into())
        .parse_lossy(directives.unwrap_or_default())
}

/// Journalise chaque panique, puis laisse le hook précédent s'exécuter.
///
/// Seuls le message de la panique et son emplacement dans le code source sont
/// journalisés, jamais d'autre donnée attachée.
fn install_panic_hook() {
    let previous = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        let location = info
            .location()
            .map(|location| format!("{}:{}", location.file(), location.line()));
        tracing::error!(
            message = info.payload_as_str().unwrap_or("panique sans message"),
            location = location.as_deref().unwrap_or("inconnu"),
            "panique"
        );
        previous(info);
    }));
}

#[cfg(test)]
mod tests {
    use std::fs;

    use super::*;

    fn log_files(dir: &Path) -> Vec<String> {
        fs::read_dir(dir)
            .unwrap()
            .map(|entry| entry.unwrap().file_name().to_string_lossy().into_owned())
            .collect()
    }

    #[test]
    fn writes_logs_to_a_daily_file() {
        let dir = tempfile::tempdir().unwrap();
        let log_dir = dir.path().join("logs");

        let (subscriber, guard) = build_subscriber(&log_dir, env_filter(None), false).unwrap();
        tracing::subscriber::with_default(subscriber, || {
            tracing::info!(document_id = "doc_test", "document reçu");
        });
        drop(guard);

        let files = log_files(&log_dir);
        assert_eq!(files.len(), 1);
        let name = &files[0];
        assert!(
            name.starts_with("minim.") && name.ends_with(".log"),
            "{name}"
        );
        let content = fs::read_to_string(log_dir.join(name)).unwrap();
        assert!(content.contains("document reçu"));
        assert!(content.contains("doc_test"));
    }

    fn logged_content(directives: Option<&str>) -> String {
        let dir = tempfile::tempdir().unwrap();

        let (subscriber, guard) =
            build_subscriber(dir.path(), env_filter(directives), false).unwrap();
        tracing::subscriber::with_default(subscriber, || {
            tracing::info!("message d'information");
            tracing::debug!("message de débogage");
        });
        drop(guard);

        let name = &log_files(dir.path())[0];
        fs::read_to_string(dir.path().join(name)).unwrap()
    }

    #[test]
    fn ignores_messages_below_info_by_default() {
        let content = logged_content(None);

        assert!(content.contains("message d'information"));
        assert!(!content.contains("message de débogage"));
    }

    #[test]
    fn lets_minim_log_change_the_level() {
        let content = logged_content(Some("debug"));

        assert!(content.contains("message de débogage"));
    }

    #[test]
    fn ignores_an_invalid_directive() {
        let content = logged_content(Some("=pas une directive="));

        assert!(content.contains("message d'information"));
        assert!(!content.contains("message de débogage"));
    }
}
