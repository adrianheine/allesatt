use chrono::Local;
use std::collections::HashMap;
use std::time::Duration;
use time::Duration as OldDuration;

use super::{Store, TaskId, TodoCompleted, TodoDate, TodoId};

#[derive(Copy, Clone, Debug)]
enum DueIn {
  Calculated(Duration, u32),
  Fixed(Duration),
  None,
}

const DEFAULT_PERIOD: Duration = Duration::from_secs(30 * 24 * 60 * 60);

impl DueIn {
  fn get(&self) -> Duration {
    match *self {
      Self::Calculated(sum, count) => sum / count,
      Self::Fixed(v) => v,
      Self::None => DEFAULT_PERIOD,
    }
  }

  fn add_duration(&self, duration: Duration) -> Self {
    if let Self::Calculated(sum, count) = *self {
      let new_count = 10.min(count + 1);
      Self::Calculated(duration + (sum / count) * (new_count - 1), new_count)
    } else {
      Self::Calculated(duration, 1)
    }
  }
}

#[derive(Clone, Debug)]
struct DueInfo {
  due_in: DueIn,
  last_completed: Option<TodoDate>,
}

#[derive(Debug, Default)]
pub struct DueGuesser {
  info: HashMap<TaskId, DueInfo>,
}

impl DueGuesser {
  pub fn new() -> Self {
    Self::default()
  }

  pub fn init_task<S: Store>(&mut self, _store: &S, task_id: &TaskId, due_after: Option<Duration>) {
    self.info.insert(
      task_id.clone(),
      DueInfo {
        due_in: match due_after {
          Some(d) => DueIn::Fixed(d),
          None => DueIn::None,
        },
        last_completed: None,
      },
    );
  }

  pub fn copy_task<S: Store>(&mut self, _store: &S, task_id: &TaskId, other_task_id: &TaskId) {
    self.info.insert(
      task_id.clone(),
      (*self.info.get(other_task_id).unwrap()).clone(),
    );
  }

  pub fn handle_completion<S: Store>(
    &mut self,
    store: &S,
    todo_id: &TodoId,
    completed: &TodoCompleted,
  ) {
    let task_id = &store.get_todo(todo_id).expect("Todo not found").task;
    if let Some(info) = self.info.get_mut(task_id) {
      if let Some(last_completed) = info.last_completed {
        let diff = (completed.date - last_completed).to_std().unwrap();
        info.due_in = info.due_in.add_duration(diff);
      }
      info.last_completed = Some(completed.date);
    }
  }

  pub fn guess_due<S: Store>(&self, _store: &S, task_id: &TaskId) -> TodoDate {
    if let Some(info) = self.info.get(task_id) {
      info
        .last_completed
        .unwrap_or_else(|| Local::now().naive_local())
        + OldDuration::from_std(info.due_in.get()).unwrap()
    } else {
      Local::now().naive_local() + OldDuration::from_std(DEFAULT_PERIOD).unwrap()
    }
  }

  pub fn guess_later<S: Store>(&self, store: &S, todo_id: &TodoId) -> TodoDate {
    let todo = store.get_todo(todo_id).expect("Todo not found");
    let one_day = OldDuration::from_std(Duration::from_secs(24 * 60 * 60)).unwrap();
    Local::now().naive_local().max(todo.due)
      + if let Some(info) = self.info.get(&todo.task) {
        one_day.max(OldDuration::from_std(info.due_in.get()).unwrap() / 5)
      } else {
        one_day
      }
  }
}

#[cfg(test)]
mod test {
  use chrono::Local;
  use std::time::Duration;
  use time::Duration as OldDuration;

  use super::DueGuesser;
  use crate::engine::mem_store::MemStore;
  use crate::engine::{Store, TodoCompleted};

  #[test]
  fn ignores_fixed_after_two_completions() {
    let mut due_guesser = DueGuesser::new();
    let mut store = MemStore::new();
    let task_id = store.create_task("Task".into());
    let now = Local::now().naive_local();
    let todo1_id = store.create_todo(&task_id, now);
    due_guesser.init_task(
      &store,
      &task_id,
      Some(Duration::from_secs(5 * 24 * 60 * 60)),
    );
    let completed = TodoCompleted::new(now);
    due_guesser.handle_completion(&store, &todo1_id, &completed);
    assert_eq!(
      due_guesser.guess_due(&store, &task_id),
      now + OldDuration::days(5)
    );
    store
      .set_todo_completed(&todo1_id, Some(completed))
      .unwrap();
    let todo2_id = store.create_todo(&task_id, now);
    due_guesser.handle_completion(
      &store,
      &todo2_id,
      &TodoCompleted::new(now + OldDuration::days(2)),
    );
    assert_eq!(
      due_guesser.guess_due(&store, &task_id),
      now + OldDuration::days(4)
    );
  }

  #[test]
  fn makes_a_good_guess() {
    let mut due_guesser = DueGuesser::new();
    let mut store = MemStore::new();
    let task_id = store.create_task("Task".into());
    due_guesser.init_task(&store, &task_id, None);

    let mut now = Local::now().naive_local();
    let completed = TodoCompleted::new(now);
    let todo1_id = store.create_todo(&task_id, now);
    due_guesser.handle_completion(&store, &todo1_id, &completed);
    store
      .set_todo_completed(&todo1_id, Some(completed))
      .unwrap();

    now += OldDuration::days(2);
    let completed = TodoCompleted::new(now);
    let todo2_id = store.create_todo(&task_id, now);
    due_guesser.handle_completion(&store, &todo2_id, &TodoCompleted::new(now));
    store
      .set_todo_completed(&todo2_id, Some(completed))
      .unwrap();

    now += OldDuration::days(4);
    let completed = TodoCompleted::new(now);
    let todo3_id = store.create_todo(&task_id, now);
    due_guesser.handle_completion(&store, &todo3_id, &TodoCompleted::new(now));
    store
      .set_todo_completed(&todo3_id, Some(completed))
      .unwrap();
    assert_eq!(
      due_guesser.guess_due(&store, &task_id),
      now + OldDuration::days(3)
    );
  }
}
