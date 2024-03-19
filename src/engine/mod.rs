mod data;
mod due_guesser;
mod engine;
mod logger;
mod mem_store;
mod store;

pub use data::{Task, TaskId, Todo, TodoCompleted, TodoDate, TodoId};
pub use engine::{try_new, Allesatt};
pub use logger::{Logger, ReadWriteLogger};
pub use mem_store::MemStore;
pub use store::Store;

use rand::{seq::index::sample, thread_rng};
use time::{Duration, OffsetDateTime};

const MAX_DUE: usize = 5;
const MAX_NOT_DUE: usize = 3;
const RANDOM_SAMPLE: bool = true;

pub fn get_todos(
  store: &'_ impl Store,
  all: bool,
) -> (Vec<(&'_ Todo, &'_ Task)>, Vec<&'_ Task>, bool) {
  let tasks = store.get_tasks();
  let mut todos_due: Vec<(&Todo, _)> = Vec::with_capacity(if all { tasks.len() } else { MAX_DUE });
  let mut todos_not_due: Vec<(&Todo, _)> =
    Vec::with_capacity(if all { tasks.len() } else { MAX_NOT_DUE });
  let mut paused_tasks: Vec<&Task> = Vec::new();
  let mut and_more = false;
  let tomorrow = OffsetDateTime::now_utc() + Duration::DAY;
  for task in tasks {
    #[allow(clippy::option_if_let_else)]
    if let Some(todo) = store.find_open_todo(&task.id) {
      let due = todo.due <= tomorrow;
      let todos = if due {
        &mut todos_due
      } else {
        &mut todos_not_due
      };
      let pos = todos
        .binary_search_by_key(&todo.due, |&(t, _)| t.due)
        .unwrap_or_else(|e| e);
      if all || pos < (if due { MAX_DUE } else { MAX_NOT_DUE }) {
        todos.insert(pos, (todo, task));
      } else if due {
        and_more = true;
      }
    } else {
      let pos = paused_tasks
        .binary_search_by_key(&&task.id, |&task| &task.id)
        .unwrap_or_else(|e| e);
      paused_tasks.insert(pos, task);
    }
  }
  let mut todos = todos_due;
  if all {
    todos.append(&mut todos_not_due);
  } else if todos.len() > MAX_DUE {
    and_more = true;
    if RANDOM_SAMPLE {
      let mut rng = thread_rng();
      let mut idxs = sample(&mut rng, todos.len(), MAX_DUE).into_vec();
      idxs.sort_unstable_by(|a, b| b.cmp(a)); // sort reverse
      let mut todos_new = Vec::with_capacity(MAX_DUE);
      for i in idxs {
        todos_new.push(todos.drain(i..).next().unwrap());
      }
      todos_new.reverse();
      todos = todos_new;
    } else {
      todos.truncate(MAX_DUE);
    }
  } else {
    todos_not_due.truncate(MAX_NOT_DUE.saturating_sub(todos.len()));
    todos.append(&mut todos_not_due);
  };
  (todos, paused_tasks, and_more)
}
