//! Clawdius UI -- Shared Leptos Component Library
//!
//! Provides reusable UI components for the Clawdius Tauri desktop app
//! and Leptos web app. All components follow the Spatial Materialism
//! design language with Amoebic UI layouts and Brutalist typography.
//!
//! `clippy::empty_enums` is allowed crate-wide: leptos `#[component]` +
//! `TypedBuilder` generate a zero-variant marker enum per component
//! under the workspace's ssr/csr feature mix — a macro artifact, not
//! real dead code (clippy 1.100 fires it from inside the derive).

#![deny(unsafe_code)]
#![allow(clippy::empty_enums)]

pub mod components;
pub mod hooks;
pub mod layouts;
pub mod theme;
