use super::{Store, TaskId, TodoCompleted, TodoDate, TodoId};
use std::collections::HashMap;
use std::convert::TryInto;
use std::time::Duration as StdDuration;
use time::Duration;
use time::OffsetDateTime;

#[derive(Copy, Clone, Debug)]
enum DueIn {
  Calculated(Duration, u32),
  Fixed(Duration),
  None,
}

const DEFAULT_PERIOD: Duration = Duration::days(30);

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

  pub fn init_task<S: Store>(
    &mut self,
    _store: &S,
    task_id: &TaskId,
    due_after: Option<StdDuration>,
  ) {
    self.info.insert(
      task_id.clone(),
      DueInfo {
        due_in: match due_after {
          Some(d) => DueIn::Fixed(d.try_into().unwrap()),
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
        let diff = completed.date - last_completed;
        info.due_in = info.due_in.add_duration(diff);
      }
      info.last_completed = Some(completed.date);
    }
  }

  pub fn guess_due<S: Store>(&self, _store: &S, task_id: &TaskId) -> TodoDate {
    self.info.get(task_id).map_or_else(
      || OffsetDateTime::now_utc() + DEFAULT_PERIOD,
      |info| info.last_completed.unwrap_or_else(OffsetDateTime::now_utc) + info.due_in.get(),
    )
  }

  pub fn guess_later<S: Store>(&self, store: &S, todo_id: &TodoId) -> TodoDate {
    let todo = store.get_todo(todo_id).expect("Todo not found");
    let one_day = Duration::day();
    OffsetDateTime::now_utc().max(todo.due)
      + self
        .info
        .get(&todo.task)
        .map_or(one_day, |info| one_day.max(info.due_in.get() / 5))
  }
}

#[cfg(test)]
mod test {
  use super::DueGuesser;
  use crate::engine::mem_store::MemStore;
  use crate::engine::{Store, TodoCompleted};
  use std::convert::TryInto;
  use time::Duration;
  use time::OffsetDateTime;

  #[test]
  fn ignores_fixed_after_two_completions() {
    let mut due_guesser = DueGuesser::new();
    let mut store = MemStore::new();
    let task_id = store.create_task("Task".into());
    let now = OffsetDateTime::now_utc();
    let todo1_id = store.create_todo(&task_id, now);
    due_guesser.init_task(
      &store,
      &task_id,
      Some(Duration::days(5).try_into().unwrap()),
    );
    let completed = TodoCompleted::new(now);
    due_guesser.handle_completion(&store, &todo1_id, &completed);
    assert_eq!(
      due_guesser.guess_due(&store, &task_id),
      now + Duration::days(5)
    );
    store
      .set_todo_completed(&todo1_id, Some(completed))
      .unwrap();
    let todo2_id = store.create_todo(&task_id, now);
    due_guesser.handle_completion(
      &store,
      &todo2_id,
      &TodoCompleted::new(now + Duration::days(2)),
    );
    assert_eq!(
      due_guesser.guess_due(&store, &task_id),
      now + Duration::days(4)
    );
  }

  #[test]
  fn makes_a_good_guess() {
    let mut due_guesser = DueGuesser::new();
    let mut store = MemStore::new();
    let task_id = store.create_task("Task".into());
    due_guesser.init_task(&store, &task_id, None);

    let mut now = OffsetDateTime::now_utc();
    let completed = TodoCompleted::new(now);
    let todo1_id = store.create_todo(&task_id, now);
    due_guesser.handle_completion(&store, &todo1_id, &completed);
    store
      .set_todo_completed(&todo1_id, Some(completed))
      .unwrap();

    now += Duration::days(2);
    let completed = TodoCompleted::new(now);
    let todo2_id = store.create_todo(&task_id, now);
    due_guesser.handle_completion(&store, &todo2_id, &TodoCompleted::new(now));
    store
      .set_todo_completed(&todo2_id, Some(completed))
      .unwrap();

    now += Duration::days(4);
    let completed = TodoCompleted::new(now);
    let todo3_id = store.create_todo(&task_id, now);
    due_guesser.handle_completion(&store, &todo3_id, &TodoCompleted::new(now));
    store
      .set_todo_completed(&todo3_id, Some(completed))
      .unwrap();
    assert_eq!(
      due_guesser.guess_due(&store, &task_id),
      now + Duration::days(3)
    );
  }
}
