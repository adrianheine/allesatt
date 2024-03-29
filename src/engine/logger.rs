use serde_json::{from_str as from_json, to_string as to_json};
use std::borrow::BorrowMut;
use std::error::Error;
use std::io::{BufRead, BufReader, Lines, Read, Write};
use std::marker::PhantomData;
use std::time::Duration;

use super::{Allesatt, TaskId, TodoCompleted, TodoId};

pub trait Logger {
  fn play_back<A: Allesatt>(&mut self, app: &mut A) -> Result<(), Box<dyn Error>>;
  fn log_create_task(
    &mut self,
    title: &str,
    due_every: &Option<Duration>,
    task_id: &TaskId,
    todo_id: &TodoId,
  ) -> Result<(), Box<dyn Error>>;
  fn log_clone_task(
    &mut self,
    task_id: &TaskId,
    title: &str,
    new_task_id: &TaskId,
    todo_id: &TodoId,
  ) -> Result<(), Box<dyn Error>>;
  fn log_complete_todo(
    &mut self,
    todo_id: &TodoId,
    completed: &TodoCompleted,
  ) -> Result<(), Box<dyn Error>>;
  fn log_todo_later(&mut self, todo_id: &TodoId) -> Result<(), Box<dyn Error>>;
  fn log_pause_task(&mut self, task_id: &TaskId) -> Result<(), Box<dyn Error>>;
  fn log_unpause_task(&mut self, task_id: &TaskId) -> Result<(), Box<dyn Error>>;
}

#[derive(Debug)]
pub struct ReadWriteLogger<R: Read, IW: Write, W: BorrowMut<IW>> {
  source: Lines<BufReader<R>>,
  target: W,
  phantom: PhantomData<IW>,
}

impl<R: Read, IW: Write, W: BorrowMut<IW>> ReadWriteLogger<R, IW, W> {
  pub fn new(source: R, target: W) -> Self {
    Self {
      source: BufReader::new(source).lines(),
      target,
      phantom: PhantomData::<IW>,
    }
  }
}

fn parse_line(line: &str, app: &mut impl Allesatt) -> Result<(), Box<dyn Error>> {
  match line.split_at(line.find(':').ok_or_else(|| String::from("Invalid line"))? + 1) {
    ("create_task1:", v) => {
      let (title, due_every, task_id, todo_id) = from_json(v)?;
      if (task_id, todo_id) != app.create_task(title, due_every) {
        return Err("Mismatch in task or todo ids".into());
      }
    }
    ("clone_task1:", v) => {
      let (task_id, title, new_task_id, todo_id) = from_json(v)?;
      let expected_result = (new_task_id, todo_id);
      let result = app.clone_task(&task_id, title)?;
      if expected_result != result {
        return Err(
          format!("Mismatch in task or todo ids: expected {expected_result:?}, found {result:?}")
            .into(),
        );
      }
    }
    ("complete_todo1:", v) => {
      let (todo_id, completed) = from_json(v)?;
      app.complete_todo(&todo_id, completed)?;
    }
    ("todo_later1:", v) => {
      let (todo_id,): (TodoId,) = from_json(v)?;
      app.todo_later(&todo_id)?;
    }
    ("pause_task1:", v) => {
      let (task_id,): (TaskId,) = from_json(v)?;
      app.pause_task(&task_id)?;
    }
    ("unpause_task1:", v) => {
      let (task_id,): (TaskId,) = from_json(v)?;
      app.unpause_task(&task_id)?;
    }
    (something, something_else) => {
      return Err(format!("Unexpected {something}:{something_else}").into());
    }
  }
  Ok(())
}

impl<R: Read, IW: Write, W: BorrowMut<IW>> Logger for ReadWriteLogger<R, IW, W> {
  fn play_back<A: Allesatt>(&mut self, app: &mut A) -> Result<(), Box<dyn Error>> {
    for line_result in &mut self.source {
      let line = line_result?;
      parse_line(&line, app).map_err(|e| e.to_string() + "\nLine content: " + &line)?;
    }
    Ok(())
  }

  fn log_clone_task(
    &mut self,
    task_id: &TaskId,
    title: &str,
    new_task_id: &TaskId,
    todo_id: &TodoId,
  ) -> Result<(), Box<dyn Error>> {
    writeln!(
      self.target.borrow_mut(),
      "clone_task1: [{}, {}, {}, {}]",
      to_json(task_id)?,
      to_json(title)?,
      to_json(new_task_id)?,
      to_json(todo_id)?
    )?;
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
      self.target.borrow_mut(),
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
      self.target.borrow_mut(),
      "complete_todo1: [{}, {}]",
      to_json(todo_id)?,
      to_json(completed)?
    )?;
    Ok(())
  }

  fn log_todo_later(&mut self, todo_id: &TodoId) -> Result<(), Box<dyn Error>> {
    writeln!(
      self.target.borrow_mut(),
      "todo_later1: [{}]",
      to_json(todo_id)?
    )?;
    Ok(())
  }

  fn log_pause_task(&mut self, task_id: &TaskId) -> Result<(), Box<dyn Error>> {
    writeln!(
      self.target.borrow_mut(),
      "pause_task1: [{}]",
      to_json(task_id)?
    )?;
    Ok(())
  }

  fn log_unpause_task(&mut self, task_id: &TaskId) -> Result<(), Box<dyn Error>> {
    writeln!(
      self.target.borrow_mut(),
      "unpause_task1: [{}]",
      to_json(task_id)?
    )?;
    Ok(())
  }
}
