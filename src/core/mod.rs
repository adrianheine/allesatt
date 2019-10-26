pub mod logger;
pub mod mem_store;
pub mod model;

mod due_guesser;

use chrono::Local;
use std::error::Error;
use std::time::Duration;

use self::due_guesser::DueGuesser;
use self::logger::Logger;
use self::model::{Store, TaskId, TodoCompleted, TodoId};

pub trait Allesatt {
  type Store: Store;
  fn create_task(&mut self, title: String, due_every: Option<Duration>) -> (TaskId, TodoId);
  fn clone_task(
    &mut self,
    task_id: &TaskId,
    title: String,
  ) -> Result<(TaskId, TodoId), Box<dyn Error>>;
  fn complete_todo(
    &mut self,
    todo_id: &TodoId,
    completed: TodoCompleted,
  ) -> Result<(), Box<dyn Error>>;
  fn todo_later(&mut self, todo_id: &TodoId) -> Result<(), Box<dyn Error>>;
  fn pause_task(&mut self, task_id: &TaskId) -> Result<(), Box<dyn Error>>;
  fn get_store(&self) -> &Self::Store;
}

#[derive(Debug)]
pub struct AllesattInner<S: Store> {
  store: S,
  due_guesser: DueGuesser,
}

impl<S: Store> Allesatt for AllesattInner<S> {
  type Store = S;

  fn create_task(&mut self, title: String, due_every: Option<Duration>) -> (TaskId, TodoId) {
    let task_id = self.store.create_task(title);
    self.due_guesser.init_task(&self.store, &task_id, due_every);
    let todo_id = self.store.create_todo(&task_id, Local::now().naive_local());
    (task_id, todo_id)
  }

  fn clone_task(
    &mut self,
    task_id: &TaskId,
    title: String,
  ) -> Result<(TaskId, TodoId), Box<dyn Error>> {
    self.store.get_task(task_id).ok_or("task not found")?;
    let new_task_id = self.store.create_task(title);
    self
      .due_guesser
      .copy_task(&self.store, &new_task_id, task_id);
    let todos: Vec<_> = self
      .store
      .get_todos(Some(task_id), Some(true))
      .iter()
      .map(|t| (t.due, t.completed.clone()))
      .collect();
    for (due, completed) in todos {
      let todo_id = self.store.create_todo(&new_task_id, due);
      self.store.set_todo_completed(&todo_id, completed)?;
    }
    let due = self
      .store
      .find_open_todo(task_id)
      .ok_or("Cloning paused tasks is not implemented")?
      .due;
    let todo_id = self.store.create_todo(&new_task_id, due);
    Ok((new_task_id, todo_id))
  }

  fn complete_todo(
    &mut self,
    todo_id: &TodoId,
    completed: TodoCompleted,
  ) -> Result<(), Box<dyn Error>> {
    self
      .due_guesser
      .handle_completion(&self.store, todo_id, &completed);
    self.store.set_todo_completed(todo_id, Some(completed))?;
    let task_id = self
      .store
      .get_todo(todo_id)
      .ok_or("Todo not found")?
      .task
      .clone();
    let due = self.due_guesser.guess_due(&self.store, &task_id);
    self.store.create_todo(&task_id, due);
    Ok(())
  }

  fn todo_later(&mut self, todo_id: &TodoId) -> Result<(), Box<dyn Error>> {
    let due = self.due_guesser.guess_later(&self.store, todo_id);
    self.store.set_todo_due(todo_id, due)?;
    Ok(())
  }

  fn pause_task(&mut self, task_id: &TaskId) -> Result<(), Box<dyn Error>> {
    let todo_id = self
      .store
      .find_open_todo(task_id)
      .ok_or("Task not found or already paused")?
      .id
      .clone();
    self.store.delete_todo(&todo_id)?;
    Ok(())
  }

  // This is non-mutable
  fn get_store(&self) -> &Self::Store {
    &self.store
  }
}

#[derive(Debug)]
pub struct AllesattImpl<S: Store, L: Logger> {
  inner: AllesattInner<S>,
  logger: L,
}

impl<S: Store, L: Logger> AllesattImpl<S, L> {
  pub fn new(store: S, mut logger: L) -> Self {
    let mut inner = AllesattInner {
      store,
      due_guesser: DueGuesser::new(),
    };
    logger
      .play_back(&mut inner)
      .expect("error playing back log");
    Self { inner, logger }
  }
}

impl<S: Store, L: Logger> Allesatt for AllesattImpl<S, L> {
  type Store = S;
  fn create_task(&mut self, title: String, due_every: Option<Duration>) -> (TaskId, TodoId) {
    let (task_id, todo_id) = self.inner.create_task(title.clone(), due_every);
    self
      .logger
      .log_create_task(title.as_ref(), &due_every, &task_id, &todo_id)
      .expect("Error logging task creation");
    (task_id, todo_id)
  }

  fn clone_task(
    &mut self,
    task_id: &TaskId,
    title: String,
  ) -> Result<(TaskId, TodoId), Box<dyn Error>> {
    let (new_task_id, todo_id) = self.inner.clone_task(task_id, title.clone())?;
    self
      .logger
      .log_clone_task(task_id, title.as_ref(), &new_task_id, &todo_id)
      .expect("Error logging task creation");
    Ok((new_task_id, todo_id))
  }

  fn complete_todo(
    &mut self,
    todo_id: &TodoId,
    completed: TodoCompleted,
  ) -> Result<(), Box<dyn Error>> {
    self.inner.complete_todo(todo_id, completed.clone())?;
    self.logger.log_complete_todo(todo_id, &completed)?;
    Ok(())
  }

  fn todo_later(&mut self, todo_id: &TodoId) -> Result<(), Box<dyn Error>> {
    self.inner.todo_later(todo_id)?;
    self.logger.log_todo_later(todo_id)?;
    Ok(())
  }

  fn pause_task(&mut self, task_id: &TaskId) -> Result<(), Box<dyn Error>> {
    self.inner.pause_task(task_id)?;
    self.logger.log_pause_task(task_id)?;
    Ok(())
  }

  // This is non-mutable
  fn get_store(&self) -> &Self::Store {
    &self.inner.store
  }
}
