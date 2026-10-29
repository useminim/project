//! Trousseau du système (Windows, macOS, Secret Service sous Linux) : clé de
//! la base locale (D-31) et session du compte (D-37).

/// Service des entrées de minim dans le trousseau.
pub const SERVICE: &str = "com.useminim.desktop";

/// Entrée `account` du trousseau de minim. Sous Windows, la persistance
/// `Local` garde le secret sur la machine (la valeur par défaut `Enterprise`
/// le ferait voyager avec un profil itinérant).
///
/// # Erreurs
///
/// Renvoie une erreur si le trousseau du système est inaccessible.
pub fn entry(account: &str) -> Result<keyring_core::Entry, keyring_core::Error> {
    use keyring_core::api::CredentialStoreApi as _;

    #[cfg(target_os = "windows")]
    let (store, modifiers) = (
        windows_native_keyring_store::Store::new()?,
        Some(std::collections::HashMap::from([("persistence", "Local")])),
    );
    #[cfg(target_os = "macos")]
    let (store, modifiers) = (
        apple_native_keyring_store::keychain::Store::new()?,
        None::<std::collections::HashMap<&str, &str>>,
    );
    #[cfg(all(unix, not(target_os = "macos")))]
    let (store, modifiers) = (
        zbus_secret_service_keyring_store::Store::new()?,
        None::<std::collections::HashMap<&str, &str>>,
    );

    store.build(SERVICE, account, modifiers.as_ref())
}
