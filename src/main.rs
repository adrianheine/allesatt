use allesatt::{cli::cli, engine::MemStore};
use std::error::Error;

fn main() -> Result<(), Box<dyn Error>> {
  // Don't ignore SIGPIPE
  // https://github.com/rust-lang/rust/issues/62569
  unsafe {
    libc::signal(libc::SIGPIPE, libc::SIG_DFL);
  }
  cli(MemStore::new())
}
