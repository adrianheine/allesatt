use std::error::Error;

use super::{Task, TaskId, Todo, TodoCompleted, TodoDate, TodoId};

pub trait Store {
  fn create_task(&mut self, title: String) -> TaskId;
  fn create_todo(&mut self, task: &TaskId, due: TodoDate) -> TodoId;
  fn set_todo_completed(
    &mut self,
    todo: &TodoId,
    completed: Option<TodoCompleted>,
  ) -> Result<(), Box<dyn Error>>;
  fn set_todo_due(&mut self, todo: &TodoId, due: TodoDate) -> Result<(), Box<dyn Error>>;
  fn delete_todo(&mut self, todo: &TodoId) -> Result<(), Box<dyn Error>>;

  fn get_task(&self, task: &TaskId) -> Option<&Task>;
  fn get_tasks(&self) -> Vec<&Task>;
  fn get_todo(&self, todo: &TodoId) -> Option<&Todo>;
  fn get_todos(
    &self,
    task_id_filter: Option<&TaskId>,
    completed_filter: Option<bool>,
  ) -> Vec<&Todo>;
  fn find_open_todo(&self, task: &TaskId) -> Option<&Todo>;
}
