//! Types partagés entre l'application desktop et l'API (DTO de synchronisation, droits liés à l'offre).

#[cfg(test)]
mod tests {
    #[test]
    fn crate_is_wired_into_the_workspace() {
        assert_eq!(env!("CARGO_PKG_NAME"), "minim-shared");
    }
}
