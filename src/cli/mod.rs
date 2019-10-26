use chrono::{Local, NaiveDateTime};
use humantime::Duration as HumanDuration;
use std::borrow::{Borrow, BorrowMut};
use std::convert::TryInto;
use std::error::Error;
use std::fs::OpenOptions;
use std::io::{stderr, stdin, stdout, Stdout, Write};
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use structopt::StructOpt;

use crate::core::logger::ReadWriteLogger;
use crate::core::model::{Store, TaskId, TodoCompleted, TodoId};
use crate::core::{Allesatt, AllesattImpl};

#[derive(Debug, StructOpt)]
#[structopt(name = "Allesatt", author, about)]
struct Opts {
  #[structopt(long, short, default_value = "-")]
  /// File to read from and write to. If missing or -, will use stdout and stdin.
  file: String,

  #[structopt(subcommand)]
  cmd: Option<Cmd>,
}

#[derive(Debug, StructOpt)]
enum Cmd {
  #[structopt(visible_alias = "ls")]
  /// List tasks
  List {
    #[structopt(long)]
    /// Show all todos (the default is to only show a few todos)
    all: bool,
  },

  /// Add a new task
  Add {
    #[structopt(long, default_value = "30days")]
    every: HumanDuration,
    description: String,
  },

  /// Clone a task
  Clone { id: TaskId, description: String },

  /// Complete a task
  Do { id: TaskId },

  /// Show completed tasks
  Done { id: Option<TaskId> },

  /// Mark a task as being due later
  Later { id: TaskId },
}

impl Cmd {
  pub fn readonly(&self) -> bool {
    match self {
      Self::List { .. } | Self::Done { .. } => true,
      _ => false,
    }
  }
}

pub fn cli<S: Store>(store: S) -> Result<(), Box<dyn Error>> {
  let opts = Opts::from_args();
  match opts.file.as_ref() {
    "-" => handle_command(
      &opts.cmd,
      AllesattImpl::new(
        store,
        ReadWriteLogger::<_, Stdout, _>::new(stdin(), &mut stdout()),
      ),
    ),
    file_name => {
      let file = OpenOptions::new().read(true).append(true).open(file_name)?;
      handle_command(
        &opts.cmd,
        AllesattImpl::new(store, ReadWriteLogger::new(&file, &file)),
      )
    }
  }
}
fn handle_command<S: Store, A: Allesatt<Store = S>, B: BorrowMut<A> + Borrow<A>>(
  command: &Option<Cmd>,
  app: B,
) -> Result<(), Box<dyn Error>> {
  let default = Cmd::List {
    all: atty::isnt(atty::Stream::Stdout),
  };
  let cmd = command.as_ref().unwrap_or(&default);
  if cmd.readonly() {
    handle_command_impl(cmd, app, &mut stdout())
  } else {
    handle_command_impl(cmd, app, &mut stderr())
  }
}

fn handle_command_impl<S: Store, A: Allesatt<Store = S>, B: BorrowMut<A> + Borrow<A>, W: Write>(
  command: &Cmd,
  app: B,
  output: &mut W,
) -> Result<(), Box<dyn Error>> {
  match command {
    Cmd::Add { description, every } => create_task(app, output, description, every),
    Cmd::Clone { id, description } => clone_task(app, output, id, description),
    Cmd::Do { id } => do_task(app, output, id),
    Cmd::Done { id } => list_done_todos(app, output, id),
    Cmd::Later { id } => task_later(app, output, id),
    Cmd::List { all } => list_todos(app, output, *all),
  }
}

fn list_todos<S: Store, A: Allesatt<Store = S>, B: Borrow<A>, W: Write>(
  app: B,
  output: &mut W,
  all: bool,
) -> Result<(), Box<dyn Error>> {
  let store = app.borrow().get_store();
  let mut todos: Vec<_> = store
    .get_todos(None, Some(false))
    .into_iter()
    .map(|todo| {
      let task = store.get_task(&todo.task).unwrap();
      (todo, task.title.clone())
    })
    .collect();
  if let Some(max_id_len) = todos
    .iter()
    .map(|(todo, _)| todo.task.to_string().len())
    .max()
  {
    let tomorrow = NaiveDateTime::from_timestamp(
      (SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs()
        + 60 * 60 * 24)
        .try_into()
        .unwrap(),
      0,
    );
    todos.sort_unstable_by(|(todo1, _), (todo2, _)| todo1.due.cmp(&todo2.due));
    for (count, (todo, title)) in todos.iter().enumerate() {
      if !all && count >= 3 && (todo.due > tomorrow || count >= 5) {
        if todo.due <= tomorrow {
          writeln!(output, "(and more)")?;
        }
        break;
      }
      writeln!(
        output,
        "{:0width$} {} {}",
        todo.task,
        todo.due.format("%Y-%m-%d"),
        title,
        width = max_id_len
      )?;
    }
  }
  Ok(())
}

fn list_done_todos<S: Store, A: Allesatt<Store = S>, B: Borrow<A>, W: Write>(
  app: B,
  output: &mut W,
  id: &Option<TaskId>,
) -> Result<(), Box<dyn Error>> {
  let store = app.borrow().get_store();
  let mut todos: Vec<_> = store
    .get_todos(id.as_ref(), Some(true))
    .into_iter()
    .map(|todo| {
      let task = store.get_task(&todo.task).unwrap();
      (todo.task, todo.completed.unwrap().date, task.title.clone())
    })
    .collect();
  if let Some(max_id_len) = todos
    .iter()
    .map(|(task_id, _, _)| task_id.to_string().len())
    .max()
  {
    todos.sort_unstable_by(|(_, completed1, _), (_, completed2, _)| completed1.cmp(completed2));
    for (task_id, completed, title) in todos {
      writeln!(
        output,
        "{:0width$} {} {}",
        task_id,
        completed.format("%Y-%m-%d"),
        title,
        width = max_id_len
      )?;
    }
  }
  Ok(())
}

fn create_task<S: Store, A: Allesatt<Store = S>, B: BorrowMut<A> + Borrow<A>, W: Write>(
  mut app: B,
  output: &mut W,
  description: &str,
  due_every: &Duration,
) -> Result<(), Box<dyn Error>> {
  let (task_id, todo_id) = app
    .borrow_mut()
    .create_task(description.into(), Some(*due_every));
  print_todo(app.borrow().get_store(), output, &task_id, &todo_id)
}

fn clone_task<S: Store, A: Allesatt<Store = S>, B: BorrowMut<A> + Borrow<A>, W: Write>(
  mut app: B,
  output: &mut W,
  id: &TaskId,
  description: &str,
) -> Result<(), Box<dyn Error>> {
  let (task_id, todo_id) = app.borrow_mut().clone_task(id, description.into())?;
  print_todo(app.borrow().get_store(), output, &task_id, &todo_id)
}

fn print_todo<S: Store, W: Write>(
  store: &S,
  output: &mut W,
  task_id: &TaskId,
  todo_id: &TodoId,
) -> Result<(), Box<dyn Error>> {
  let task = store.get_task(task_id).unwrap();
  let todo = store.get_todo(todo_id).unwrap();
  writeln!(
    output,
    "{} {} {}",
    task_id,
    todo.due.format("%Y-%m-%d"),
    task.title,
  )?;
  Ok(())
}

fn do_task<S: Store, A: Allesatt<Store = S>, B: BorrowMut<A> + Borrow<A>, W: Write>(
  mut app: B,
  output: &mut W,
  id: &TaskId,
) -> Result<(), Box<dyn Error>> {
  let todo = app
    .borrow()
    .get_store()
    .find_open_todo(id)
    .ok_or("Task not found")?
    .id
    .clone();
  app
    .borrow_mut()
    .complete_todo(&todo, TodoCompleted::new(Local::now().naive_local()))?;
  let store = app.borrow().get_store();
  let todo = store.find_open_todo(id).ok_or("Task not found")?;
  print_todo(store, output, id, &todo.id)
}

fn task_later<S: Store, A: Allesatt<Store = S>, B: BorrowMut<A> + Borrow<A>, W: Write>(
  mut app: B,
  output: &mut W,
  id: &TaskId,
) -> Result<(), Box<dyn Error>> {
  let todo = app
    .borrow()
    .get_store()
    .find_open_todo(id)
    .ok_or("Task not found")?
    .id
    .clone();
  app.borrow_mut().todo_later(&todo)?;
  let store = app.borrow().get_store();
  print_todo(store, output, id, &todo)
}

#[cfg(test)]
mod tests {
  use super::{handle_command_impl, Cmd};
  use crate::core::logger::ReadWriteLogger;
  use crate::core::mem_store::MemStore;
  use crate::core::AllesattImpl;
  use chrono::Local;

  fn exec_command(cmd: Cmd, log_in: &str) -> (String, String) {
    let mut output = Vec::new();
    let mut log_out: Vec<u8> = Vec::new();
    handle_command_impl(
      &cmd,
      AllesattImpl::new(
        MemStore::new(),
        ReadWriteLogger::<_, Vec<u8>, _>::new(log_in.as_bytes(), &mut log_out),
      ),
      &mut output,
    )
    .unwrap();
    (
      String::from_utf8(log_out).unwrap(),
      String::from_utf8(output).unwrap(),
    )
  }

  #[test]
  fn test_handle_command_impl() {
    let (log_out, output) = exec_command(Cmd::List { all: true }, "");
    assert_eq!(output, "");
    assert_eq!(log_out, "");

    let (log_out, output) = exec_command(
      Cmd::Add {
        every: "30days".parse().unwrap(),
        description: "task".into(),
      },
      "",
    );
    assert_eq!(
      output,
      format!("1 {} task\n", Local::now().naive_local().format("%Y-%m-%d"))
    );
    assert_eq!(
      log_out,
      "create_task1: [\"task\", {\"secs\":2592000,\"nanos\":0}, 1, 1]\n"
    );
  }
}
