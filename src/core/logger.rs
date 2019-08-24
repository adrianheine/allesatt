use core::model::{TaskId, TodoCompleted, TodoId};
use core::Allesatt;
use serde_json::{from_str as from_json, to_string as to_json};
use std::error::Error;
use std::io::{BufRead, BufReader, Lines, Read, Write};
use std::time::Duration;

pub trait Logger {
  fn play_back<A: Allesatt>(&mut self, app: &mut A) -> Result<(), Box<dyn Error>>;
  fn log_create_task(
    &mut self,
    title: &str,
    due_every: &Option<Duration>,
    task_id: &TaskId,
    todo_id: &TodoId,
  ) -> Result<(), Box<dyn Error>>;
  fn log_complete_todo(
    &mut self,
    todo_id: &TodoId,
    completed: &TodoCompleted,
  ) -> Result<(), Box<dyn Error>>;
  fn log_todo_later(&mut self, todo_id: &TodoId) -> Result<(), Box<dyn Error>>;
}

pub struct ReadWriteLogger<R: Read, W: Write> {
  source: Lines<BufReader<R>>,
  target: W,
}

impl<R: Read, W: Write> ReadWriteLogger<R, W> {
  pub fn new(source: R, target: W) -> Self {
    Self {
      source: BufReader::new(source).lines(),
      target,
    }
  }
}

impl<R: Read, W: Write> Logger for ReadWriteLogger<R, W> {
  fn play_back<A: Allesatt>(&mut self, app: &mut A) -> Result<(), Box<dyn Error>> {
    while let Some(line_result) = self.source.next() {
      let line = line_result?;
      match line.split_at(line.find(':').ok_or_else(|| String::from("Invalid line"))? + 1) {
        ("create_task1:", v) => {
          let (title, due_every, task_id, todo_id) = from_json(v)?;
          if (task_id, todo_id) != app.create_task(title, due_every) {
            return Err("Mismatch in task or todo ids".into());
          }
        }
        ("complete_todo1:", v) => {
          let (todo_id, completed) = from_json(v)?;
          app.complete_todo(&todo_id, completed)?;
        }
        ("todo_later1:", v) => {
          let [todo_id]: [TodoId; 1] = from_json(v)?;
          app.todo_later(&todo_id)?;
        }
        (something, something_else) => {
          return Err(format!("Unexpected {}:{}", something, something_else).into());
        }
      }
    }
    Ok(())
  }

  fn log_create_task(
    &mut self,
    title: &str,
    due_every: &Option<Duration>,
    task_id: &TaskId,
    todo_id: &TodoId,
  ) -> Result<(), Box<dyn Error>> {
    writeln!(
      &mut self.target,
      "create_task1: [{}, {}, {}, {}]",
      to_json(title)?,
      to_json(due_every)?,
      to_json(task_id)?,
      to_json(todo_id)?
    )?;
    Ok(())
  }

  fn log_complete_todo(
    &mut self,
    todo_id: &TodoId,
    completed: &TodoCompleted,
  ) -> Result<(), Box<dyn Error>> {
    writeln!(
      &mut self.target,
      "complete_todo1: [{}, {}]",
      to_json(todo_id)?,
      to_json(completed)?
    )?;
    Ok(())
  }

  fn log_todo_later(&mut self, todo_id: &TodoId) -> Result<(), Box<dyn Error>> {
    writeln!(&mut self.target, "todo_later1: [{}]", to_json(todo_id)?)?;
    Ok(())
  }
}
