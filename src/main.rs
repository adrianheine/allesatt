#![deny(
  macro_use_extern_crate,
  meta_variable_misuse,
  non_ascii_idents,
  single_use_lifetimes,
  trivial_casts,
  trivial_numeric_casts,
  unstable_features,
  variant_size_differences,
  rust_2018_idioms,
  missing_copy_implementations,
  missing_debug_implementations,
  future_incompatible,
  clippy::cargo,
  clippy::nursery,
  clippy::pedantic
)]
#![warn(clippy::module_name_repetitions, unused)]
// Fix these for an actually usable crate
#![allow(clippy::cargo_common_metadata, unreachable_pub, rustdoc, missing_docs)]

use std::error::Error;

mod cli;
mod engine;

use cli::cli;
use engine::MemStore;

fn main() -> Result<(), Box<dyn Error>> {
  // Don't ignore SIGPIPE
  // https://github.com/rust-lang/rust/issues/62569
  unsafe {
    libc::signal(libc::SIGPIPE, libc::SIG_DFL);
  }
  cli(MemStore::new())
}
