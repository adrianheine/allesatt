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
}

const DEFAULT_PERIOD: Duration = Duration::days(30);

impl DueIn {
  const fn new(duration: Duration) -> Self {
    Self::Fixed(duration)
  }
  fn get(v: Option<Self>) -> Duration {
    match v {
      Some(Self::Calculated(sum, count)) => sum / count,
      Some(Self::Fixed(v)) => v,
      _ => DEFAULT_PERIOD,
    }
  }
  fn add(v: Option<Self>, duration: Duration) -> Option<Self> {
    Some(if let Some(Self::Calculated(sum, count)) = v {
      let new_count = 10.min(count + 1);
      Self::Calculated(duration + (sum / count) * (new_count - 1), new_count)
    } else {
      Self::Calculated(duration, 1)
    })
  }
}

#[derive(Clone, Debug)]
struct DueInfo {
  due_in: Option<DueIn>,
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
        due_in: due_after.map(|d| DueIn::new(d.try_into().unwrap())),
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
        info.due_in = DueIn::add(info.due_in, diff);
      }
      info.last_completed = Some(completed.date);
    }
  }

  pub fn guess_due<S: Store>(&self, _store: &S, task_id: &TaskId) -> TodoDate {
    let info = self.info.get(task_id);
    let base = info
      .and_then(|info| info.last_completed)
      .unwrap_or_else(OffsetDateTime::now_utc);
    let plus = DueIn::get(info.and_then(|info| info.due_in));
    base + plus
  }

  pub fn guess_later<S: Store>(&self, store: &S, todo_id: &TodoId) -> TodoDate {
    let todo = store.get_todo(todo_id).expect("Todo not found");
    OffsetDateTime::now_utc().max(todo.due)
      + Duration::day().max(DueIn::get(self.info.get(&todo.task).and_then(|info| info.due_in)) / 5)
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

  #[test]
  fn makes_a_good_later() {
    let mut due_guesser = DueGuesser::new();
    let mut store = MemStore::new();
    let task_id = store.create_task("Task".into());
    due_guesser.init_task(&store, &task_id, None);
    let mut now = OffsetDateTime::now_utc();
    let todo_id = store.create_todo(&task_id, now);
    let later = due_guesser.guess_later(&store, &todo_id);
    assert!(later >= now + Duration::days(6));
    now = OffsetDateTime::now_utc();
    assert!(later <= now + Duration::days(6));

    now += Duration::days(10);
    let completed = TodoCompleted::new(now);
    due_guesser.handle_completion(&store, &todo_id, &completed);
    store.set_todo_completed(&todo_id, Some(completed)).unwrap();

    now += Duration::days(30);
    let completed = TodoCompleted::new(now);
    let todo_id = store.create_todo(&task_id, now);
    due_guesser.handle_completion(&store, &todo_id, &TodoCompleted::new(now));
    store.set_todo_completed(&todo_id, Some(completed)).unwrap();

    now += Duration::days(50);
    let completed = TodoCompleted::new(now);
    let todo_id = store.create_todo(&task_id, now);
    due_guesser.handle_completion(&store, &todo_id, &TodoCompleted::new(now));
    store.set_todo_completed(&todo_id, Some(completed)).unwrap();

    let later = due_guesser.guess_later(&store, &todo_id);
    assert!(later >= now + Duration::days(8));
    now = OffsetDateTime::now_utc() + Duration::days(90);
    assert!(later <= now + Duration::days(8));
  }
}
