//! Core logic shared by the ProtoGrid desktop app.
//!
//! The crate is UI-agnostic: it parses share links and subscriptions, keeps the
//! data model and renders configuration files for sing-box and Xray.

pub mod clash;
pub mod geo;
pub mod link;
pub mod model;
pub mod openvpn;
pub mod ping;
pub mod ruleset;
pub mod singbox;
pub mod subscription;
pub mod sysproxy;
pub mod util;
pub mod xray;
