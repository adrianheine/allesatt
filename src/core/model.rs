use chrono::NaiveDateTime;
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use std::error::Error;
use std::fmt::{Display, Error as FmtError, Formatter};
use std::num::ParseIntError;
use std::str::FromStr;

// FIXME TaskId should probably be store-dependent and just move there
#[derive(Debug, Eq, Hash, PartialEq, Serialize, Deserialize, Clone)]
pub struct TaskId(pub(super) u64);

#[derive(Debug, Serialize, Deserialize)]
pub struct Task {
  pub id: TaskId,
  pub title: String,
}

impl Display for TaskId {
  fn fmt(&self, formatter: &mut Formatter<'_>) -> Result<(), FmtError> {
    self.0.fmt(formatter)
  }
}

impl FromStr for TaskId {
  type Err = ParseIntError;
  fn from_str(s: &str) -> Result<Self, Self::Err> {
    s.parse().map(Self)
  }
}

#[derive(Debug, Eq, Hash, PartialEq, Serialize, Deserialize, Clone)]
pub struct TodoId(pub u64);
pub type TodoDate = NaiveDateTime;

#[derive(Clone, Debug)]
pub struct TodoCompleted {
  pub date: TodoDate,
}

impl TodoCompleted {
  pub const fn new(date: TodoDate) -> Self {
    Self { date }
  }
}

impl Serialize for TodoCompleted {
  fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
  where
    S: Serializer,
  {
    self.date.serialize(serializer)
  }
}

impl<'de> Deserialize<'de> for TodoCompleted {
  fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
  where
    D: Deserializer<'de>,
  {
    TodoDate::deserialize(deserializer).map(Self::new)
  }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Todo {
  pub id: TodoId,
  pub task: TaskId,
  pub completed: Option<TodoCompleted>,
  pub due: TodoDate,
}

pub trait Store {
  fn create_task(&mut self, title: String) -> TaskId;
  fn create_todo(&mut self, task: &TaskId, due: TodoDate) -> TodoId;
  fn set_todo_completed(
    &mut self,
    todo: &TodoId,
    completed: Option<TodoCompleted>,
  ) -> Result<(), Box<dyn Error>>;
  fn set_todo_due(&mut self, todo: &TodoId, due: TodoDate) -> Result<(), Box<dyn Error>>;

  fn get_task(&self, task: &TaskId) -> Option<&Task>;
  fn get_todo(&self, todo: &TodoId) -> Option<&Todo>;
  fn get_tasks(&self) -> Vec<TaskId>;
  fn get_todos(&self, task_id_filter: Option<&TaskId>, completed_filter: Option<bool>)
    -> Vec<Todo>;
  fn find_open_todo(&self, task: &TaskId) -> Option<&Todo>;
}
