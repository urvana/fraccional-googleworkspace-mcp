//! The shared fraccional-googleworkspace-mcp OAuth client (Desktop app, Internal audience).
//!
//! This is a public client (RFC 6749 §2.1, RFC 8252): the ID and secret ship inside the app
//! and in the public repository and are not treated as secrets (RFC 8252 §8.5). Only
//! accounts in the Fraccional Workspace organization can consent, and a Desktop client
//! only redirects to loopback.
//! gws receives them through `GOOGLE_WORKSPACE_CLI_CLIENT_ID` / `_CLIENT_SECRET`.
//!
//! # Example
//!
//! ```
//! use haulmer_googleworkspace_mcp_core::oauth_client::{CLIENT_ID, CLIENT_SECRET};
//! assert!(CLIENT_ID.ends_with(".apps.googleusercontent.com"));
//! assert!(!CLIENT_SECRET.is_empty());
//! ```

/// OAuth client ID of the Google Cloud project `fraccional-googleworkspace-mcp`
/// (project ID `fraccional-gws-mcp`).
pub const CLIENT_ID: &str =
    "633993032477-luk0kslsqebr2341m1bp1f46scfpk4kf.apps.googleusercontent.com";

/// OAuth client secret of the Desktop client (public by design, RFC 8252 §8.5).
pub const CLIENT_SECRET: &str = "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";
