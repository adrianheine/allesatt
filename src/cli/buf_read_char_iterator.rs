use std::io::BufRead;

use owned_chars::{OwnedChars, OwnedCharsExt};

#[derive(Debug)]
pub struct BufReadCharIterator<B: BufRead> {
  buf: OwnedChars,
  reader: B,
}

impl<B: BufRead> BufReadCharIterator<B> {
  pub fn new(reader: B) -> Self {
    Self {
      buf: String::new().into_chars(),
      reader,
    }
  }
}

impl<B: BufRead> Iterator for BufReadCharIterator<B> {
  type Item = char;

  fn next(&mut self) -> Option<Self::Item> {
    self.buf.next().or_else(|| {
      let mut buf = String::new();
      if let Ok(count) = self.reader.read_line(&mut buf) {
        if count > 0 {
          self.buf = buf.into_chars();
          self.next()
        } else {
          None
        }
      } else {
        None
      }
    })
  }
}
