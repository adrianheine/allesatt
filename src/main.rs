#![warn(clippy::pedantic, clippy::cargo, clippy::nursery)]
#![allow(clippy::filter_map)]
#![allow(clippy::non_ascii_literal)]
extern crate atty;
extern crate chrono;
extern crate humantime;
extern crate libc;
extern crate serde;
#[macro_use]
extern crate serde_derive;
extern crate serde_json;
extern crate structopt;
extern crate time;

mod cli;
mod core;

use core::mem_store::MemStore;
use std::error::Error;

use cli::cli;

fn main() -> Result<(), Box<dyn Error>> {
  // Don't ignore SIGPIPE
  // https://github.com/rust-lang/rust/issues/62569
  unsafe {
    libc::signal(libc::SIGPIPE, libc::SIG_DFL);
  }
  cli(MemStore::new())
}
