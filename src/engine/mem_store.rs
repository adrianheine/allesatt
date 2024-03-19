use std::collections::HashMap;
use std::error::Error;

use super::{Store, Task, TaskId, Todo, TodoCompleted, TodoDate, TodoId};

#[derive(Debug)]
pub struct MemStore {
  tasks: HashMap<TaskId, Task>,
  last_task_id: TaskId,
  todos: HashMap<TodoId, Todo>,
  last_todo_id: TodoId,
}

impl Default for MemStore {
  fn default() -> Self {
    Self {
      last_task_id: TaskId(0),
      tasks: HashMap::default(),
      last_todo_id: TodoId(0),
      todos: HashMap::default(),
    }
  }
}

impl MemStore {
  #[must_use]
  pub fn new() -> Self {
    Self::default()
  }
}

impl Store for MemStore {
  fn create_task(&mut self, title: String) -> TaskId {
    self.last_task_id = TaskId(self.last_task_id.0 + 1);
    let task = Task {
      id: self.last_task_id.clone(),
      title,
    };
    self.tasks.insert(self.last_task_id.clone(), task);
    self.last_task_id.clone()
  }

  fn create_todo(&mut self, task: &TaskId, due: TodoDate) -> TodoId {
    if let Some(other) = self.find_open_todo(task) {
      panic!("Already has an open todo for {task:?} ({other:?})");
    }
    self.last_todo_id = TodoId(self.last_todo_id.0 + 1);
    let todo = Todo {
      id: self.last_todo_id.clone(),
      task: task.clone(),
      completed: None,
      due,
    };
    self.todos.insert(self.last_todo_id.clone(), todo);
    self.last_todo_id.clone()
  }
  fn delete_todo(&mut self, todo: &TodoId) -> Result<(), Box<dyn Error>> {
    self.todos.remove(todo).ok_or("Todo not found")?;
    Ok(())
  }

  fn get_task(&self, task: &TaskId) -> Option<&Task> {
    self.tasks.get(task)
  }

  fn get_todo(&self, todo: &TodoId) -> Option<&Todo> {
    self.todos.get(todo)
  }

  fn set_todo_completed(
    &mut self,
    id: &TodoId,
    completed: Option<TodoCompleted>,
  ) -> Result<(), Box<dyn Error>> {
    self.todos.get_mut(id).ok_or("Todo not found")?.completed = completed;
    Ok(())
  }

  fn set_todo_due(&mut self, id: &TodoId, due: TodoDate) -> Result<(), Box<dyn Error>> {
    self.todos.get_mut(id).ok_or("Todo not found")?.due = due;
    Ok(())
  }

  fn get_todos(
    &self,
    task_id_filter: Option<&TaskId>,
    completed_filter: Option<bool>,
  ) -> Vec<&Todo> {
    self
      .todos
      .values()
      .filter(|todo| {
        task_id_filter.map_or(true, |task_id| *task_id == todo.task)
          && completed_filter.map_or(true, |completed| completed == todo.completed.is_some())
      })
      .collect()
  }

  fn find_open_todo(&self, task: &TaskId) -> Option<&Todo> {
    self
      .todos
      .values()
      .find(|todo| todo.task == *task && todo.completed.is_none())
  }

  fn get_tasks(&self) -> Vec<&Task> {
    self.tasks.values().collect()
  }
}
