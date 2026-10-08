//! Extraction de texte des documents PDF et DOCX.

#[cfg(test)]
mod tests {
    #[test]
    fn crate_is_wired_into_the_workspace() {
        assert_eq!(env!("CARGO_PKG_NAME"), "minim-extract");
    }
}
