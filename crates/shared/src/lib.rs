//! Types échangés entre l'application et le backend (Supabase, D-37) : leur
//! forme JSON doit correspondre à ce qu'attendent les migrations de
//! `supabase/`.

pub mod account;

pub use account::SignUpMetadata;
