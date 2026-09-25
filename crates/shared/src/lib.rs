//! Types shared between the desktop app and the API (sync DTOs, entitlements).

#[cfg(test)]
mod tests {
    #[test]
    fn crate_is_wired_into_the_workspace() {
        assert_eq!(env!("CARGO_PKG_NAME"), "minim-shared");
    }
}
