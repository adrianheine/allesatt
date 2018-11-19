use std::collections::hash_map::{Entry, HashMap};
use std::io::{stdin, stdout, BufRead, Cursor};

use chrono::{NaiveDate, NaiveDateTime};

use yaml_rust::parser::Parser;
use yaml_rust::Event;

use cli::buf_read_char_iterator::BufReadCharIterator;
use core::logger::ReadWriteLogger;
use core::mem_store::MemStore;
use core::model::{Store, TaskId, TodoCompleted};
use core::{Allesatt, AllesattImpl};

struct YamlIterator<I: Iterator<Item = char>> {
  parser: Parser<I>,
  scalars: Vec<String>,
}

impl<B: BufRead> YamlIterator<BufReadCharIterator<B>> {
  pub fn new(source: B) -> Self {
    Self {
      parser: Parser::new(BufReadCharIterator::new(source)),
      scalars: vec![],
    }
  }
}

impl<I: Iterator<Item = char>> Iterator for YamlIterator<I> {
  type Item = Vec<String>;

  fn next(&mut self) -> Option<Self::Item> {
    while {
      let event = self.parser.next().ok()?;
      match event.0 {
        Event::SequenceStart(_) => {
          let scalars = self.scalars.clone();
          self.scalars = vec![];
          if !scalars.is_empty() {
            return Some(scalars);
          }
          true
        }
        Event::Scalar(v, _, _, _) => {
          self.scalars.push(v);
          true
        }
        Event::StreamEnd => {
          let scalars = self.scalars.clone();
          self.scalars = vec![];
          if !scalars.is_empty() {
            return Some(scalars);
          }
          false
        }
        _ => true,
      }
    } {}
    None
  }
}

pub fn yaml_app() -> impl Allesatt<Store = MemStore> {
  let mut app = AllesattImpl::new(
    MemStore::new(),
    ReadWriteLogger::new(Cursor::new(""), stdout()),
  );
  let stdin = stdin();
  let iterator = YamlIterator::new(stdin.lock());
  let mut task_by_description: HashMap<String, TaskId> = HashMap::new();
  let mut last_date = NaiveDateTime::from_timestamp(0, 0);
  for scalars in iterator {
    let desc = if scalars[1] == "Wolle" {
      "Wollwäsche waschen".into()
    } else {
      format!("{}-{}°C-Wäsche waschen", scalars[1], scalars[2])
    }; //match scalars[2].as_ref() { "60" => "40".into(), v => v}) };
    let entry = task_by_description.entry(desc.clone());
    let todo_id = match entry {
      Entry::Occupied(e) => {
        let task_id = e.get().clone();
        app
          .get_store()
          .get_todos(Some(&task_id), Some(false))
          .first()
          .unwrap()
          .clone()
      }
      Entry::Vacant(e) => {
        let (task_id, todo_id) = app.create_task(desc, None);
        e.insert(task_id);
        todo_id.clone()
      }
    };
    let date = NaiveDate::parse_from_str(&scalars[0], "%Y-%m-%d")
      .expect("Could not parse date")
      .and_hms(12, 0, 0);
    assert!(date >= last_date);
    last_date = date;
    app
      .complete_todo(&todo_id, TodoCompleted::new(date))
      .unwrap();
  }
  app
}
