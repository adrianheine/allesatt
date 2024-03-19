#![deny(
  macro_use_extern_crate,
  meta_variable_misuse,
  missing_debug_implementations,
  non_ascii_idents,
  single_use_lifetimes,
  trivial_casts,
  trivial_numeric_casts,
  unstable_features,
  variant_size_differences,
  rust_2018_idioms,
  future_incompatible,
  clippy::cargo,
  clippy::nursery,
  clippy::pedantic
)]
#![warn(clippy::module_name_repetitions, unused)]
// Fix these for an actually usable crate
#![allow(
  clippy::cargo_common_metadata,
  unreachable_pub,
  rustdoc::all,
  missing_docs,
  clippy::missing_panics_doc,
  clippy::missing_errors_doc
)]
pub mod cli;
pub mod engine;
