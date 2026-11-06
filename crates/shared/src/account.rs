//! Comptes.

use serde::{Deserialize, Serialize};

/// Métadonnées envoyées à l'inscription (champ `data` de Supabase Auth). Le
/// déclencheur `handle_new_user` (migration `profiles`) en tire le profil.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct SignUpMetadata {
    /// L'utilisateur exerce une profession de santé : la synchronisation est
    /// désactivée tant qu'il n'y a pas d'hébergement HDS (D-37).
    pub health_profession: bool,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn matches_the_metadata_read_by_the_database_trigger() {
        let json = serde_json::to_value(SignUpMetadata {
            health_profession: true,
        })
        .unwrap();

        // `raw_user_meta_data ->> 'health_profession' = 'true'`
        assert_eq!(json, serde_json::json!({ "health_profession": true }));
    }
}
