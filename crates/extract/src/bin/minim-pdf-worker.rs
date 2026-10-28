//! Processus PDFium autonome, utilisé par les tests de `minim-extract` :
//! `minim-pdf-worker <dossier de la bibliothèque>`. L'application, elle, se
//! relance elle-même avec l'argument `--pdf-worker`.

use std::path::Path;
use std::process::ExitCode;

fn main() -> ExitCode {
    let code = std::env::args_os()
        .nth(1)
        .map_or(2, |dir| minim_extract::serve(Path::new(&dir)));
    ExitCode::from(u8::try_from(code).unwrap_or(1))
}
