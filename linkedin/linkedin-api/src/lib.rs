//! LinkedIn API client library.
//!
//! Provides typed access to LinkedIn's REST API, including authentication,
//! session management, and domain-specific models.
//!
//! ## TLS Backend Limitation
//!
//! This crate currently uses `rustls` (reqwest's default TLS backend).
//! LinkedIn's Android app uses Cronet/BoringSSL, producing a distinct TLS
//! fingerprint (JA3/JA4). For production use against fingerprint-checking
//! endpoints, switch to `boring-tls` once reqwest supports it as a feature
//! flag. See `re/tls_configuration.md` for details.

pub mod auth;
pub mod client;
pub mod error;
pub mod models;
pub mod restli;
