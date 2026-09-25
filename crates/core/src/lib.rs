//! Domain model and workflow engine: model, validation, triggers, action pipeline and ports (traits).

#[cfg(test)]
mod tests {
    #[test]
    fn crate_is_wired_into_the_workspace() {
        assert_eq!(env!("CARGO_PKG_NAME"), "minim-core");
    }
}
