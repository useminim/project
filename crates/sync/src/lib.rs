//! Client du backend de minim (Supabase hébergé en région UE, D-37).
//!
//! - [`SupabaseAuth`] : comptes (Supabase Auth), implémente `AuthProvider`.
//!
//! Les appels passent uniquement par ce client Rust : l'interface ne parle
//! jamais au backend. TLS par rustls (fournisseur `ring`, sans OpenSSL),
//! certificats vérifiés par le magasin du système.

mod http;
pub mod supabase;

pub use supabase::{SupabaseAuth, SupabaseConfig};
