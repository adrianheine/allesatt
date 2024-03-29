use std::error::Error;
use std::time::Duration;
use time::OffsetDateTime;

use super::due_guesser::DueGuesser;
use super::{Logger, Store, TaskId, TodoCompleted, TodoId};

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
  fn unpause_task(&mut self, task_id: &TaskId) -> Result<TodoId, Box<dyn Error>>;
  fn get_store(&self) -> &Self::Store;
}

#[derive(Debug)]
struct AllesattInner<S> {
  store: S,
  due_guesser: DueGuesser,
}

impl<S> AllesattInner<S> {
  pub fn new(store: S) -> Self {
    Self {
      store,
      due_guesser: DueGuesser::new(),
    }
  }
}

impl<S: Store> Allesatt for AllesattInner<S> {
  type Store = S;

  fn create_task(&mut self, title: String, due_every: Option<Duration>) -> (TaskId, TodoId) {
    let task_id = self.store.create_task(title);
    self.due_guesser.init_task(&self.store, &task_id, due_every);
    let todo_id = self.store.create_todo(&task_id, OffsetDateTime::now_utc());
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
    let todos: Box<[_]> = self
      .store
      .get_todos(Some(task_id), Some(true))
      .into_iter()
      .map(|t| (t.due, t.completed.clone()))
      .collect();
    // Waiting for #116607 for dropping into_vec()
    for (due, completed) in todos.into_vec() {
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
    self.due_guesser.handle_pause(task_id);
    Ok(())
  }

  fn unpause_task(&mut self, task_id: &TaskId) -> Result<TodoId, Box<dyn Error>> {
    let todo_id = self.store.create_todo(task_id, OffsetDateTime::now_utc());
    Ok(todo_id)
  }

  // This is non-mutable
  fn get_store(&self) -> &Self::Store {
    &self.store
  }
}

#[derive(Debug)]
struct AllesattImpl<S: Store, L: Logger> {
  inner: AllesattInner<S>,
  logger: L,
}

impl<S: Store, L: Logger> AllesattImpl<S, L> {
  fn try_new(store: S, mut logger: L) -> Result<Self, Box<dyn Error>> {
    let mut inner = AllesattInner::new(store);
    logger.play_back(&mut inner)?;
    Ok(Self { inner, logger })
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

  fn unpause_task(&mut self, task_id: &TaskId) -> Result<TodoId, Box<dyn Error>> {
    let result = self.inner.unpause_task(task_id)?;
    self.logger.log_unpause_task(task_id)?;
    Ok(result)
  }

  // This is non-mutable
  fn get_store(&self) -> &Self::Store {
    &self.inner.store
  }
}

pub fn try_new(store: impl Store, logger: impl Logger) -> Result<impl Allesatt, Box<dyn Error>> {
  AllesattImpl::try_new(store, logger)
}

#[cfg(test)]
mod tests {
  use super::{
    super::{MemStore, Store, TodoCompleted},
    Allesatt, AllesattInner,
  };
  use std::time::Duration;
  use time::OffsetDateTime;

  #[test]
  fn default_duration_after_pausing() {
    let now = OffsetDateTime::now_utc();
    let day = Duration::from_secs(60 * 60 * 24);
    let mut engine = AllesattInner::new(MemStore::new());
    let (task_id, todo_id) = engine.create_task("x".into(), Some(day * 7));
    engine
      .complete_todo(&todo_id, TodoCompleted::new(now - day * 28))
      .unwrap();
    engine.pause_task(&task_id).unwrap();
    let todo_id = engine.unpause_task(&task_id).unwrap();
    let due = engine.get_store().get_todo(&todo_id).unwrap().due;
    assert!(due > now - day);
    assert!(due < now + day);
    engine
      .complete_todo(&todo_id, TodoCompleted::new(now - day * 7))
      .unwrap();
    let due = engine.get_store().find_open_todo(&task_id).unwrap().due;
    assert!(due > now - day);
    assert!(due < now + day);
  }
}
