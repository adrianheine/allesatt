mod buf_read_char_iterator;
mod yaml_app;

use chrono::{Local, NaiveDateTime};
use humantime::Duration as HumanDuration;
use std::borrow::{Borrow, BorrowMut};
use std::error::Error;
use std::fs::OpenOptions;
use std::io::{stdin, stdout};
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use structopt::StructOpt;

use self::yaml_app::yaml_app;
use core::logger::ReadWriteLogger;
use core::model::{Store, TaskId, TodoCompleted};
use core::{Allesatt, AllesattImpl};

#[derive(Debug, StructOpt)]
#[structopt(name = "Allesatt", version = "v0.1", rename_all = "kebab-case")]
struct Opts {
  #[structopt(long, short, default_value = "-")]
  /// File to read from and write to. If missing or -, will use stdout and stdin.
  file: String,

  #[structopt(long)]
  import_legacy: bool,

  #[structopt(subcommand)]
  cmd: Option<Cmd>,
}

#[derive(Debug, StructOpt)]
enum Cmd {
  #[structopt(name = "list", visible_alias = "ls")]
  /// List tasks
  List {
    #[structopt(long)]
    /// Show all todos (the default is to only show a few todos)
    all: bool,
  },

  #[structopt(name = "add")]
  /// Add a new task
  Add {
    #[structopt(long, default_value = "30days")]
    every: HumanDuration,
    description: String,
  },

  #[structopt(name = "do")]
  /// Complete a task
  Do { id: TaskId },

  #[structopt(name = "done")]
  /// Show completed tasks
  Done,

  #[structopt(name = "later")]
  /// Mark a task as being due later
  Later { id: TaskId },
}

pub fn cli<S: Store>(store: S) -> Result<(), Box<Error>> {
  let opts = Opts::from_args();
  if opts.import_legacy {
    handle_command(&opts, yaml_app())
  } else {
    match opts.file.as_ref() {
      "-" => handle_command(
        &opts,
        AllesattImpl::new(store, ReadWriteLogger::new(stdin(), stdout())),
      ),
      file_name => {
        let file = OpenOptions::new().read(true).append(true).open(file_name)?;
        handle_command(
          &opts,
          AllesattImpl::new(store, ReadWriteLogger::new(&file, &file)),
        )
      }
    }
  }
}

fn handle_command<S: Store, A: Allesatt<Store = S>, B: BorrowMut<A> + Borrow<A>>(
  matches: &Opts,
  app: B,
) -> Result<(), Box<Error>> {
  if let Some(ref cmd) = matches.cmd {
    match cmd {
      Cmd::List { all } => list_todos(app, *all),
      Cmd::Add { description, every } => create_task(app, description, **every),
      Cmd::Do { id } => do_task(app, id),
      Cmd::Done => list_done_todos(app),
      Cmd::Later { id } => task_later(app, id),
    }
  } else {
    list_todos(app, atty::isnt(atty::Stream::Stdout))
  }
}

fn list_todos<S: Store, A: Allesatt<Store = S>, B: Borrow<A>>(
  app: B,
  all: bool,
) -> Result<(), Box<Error>> {
  let store = app.borrow().get_store();
  let todo_ids = store.get_todos(None, Some(false));
  let mut todos: Vec<_> = todo_ids
    .into_iter()
    .map(|todo_id| {
      let todo = store.get_todo(&todo_id).unwrap();
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
      SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs() as i64
        + 60 * 60 * 24,
      0,
    );
    todos.sort_unstable_by(|(todo1, _), (todo2, _)| todo1.due.cmp(&todo2.due));
    for (count, (todo, title)) in todos.iter().enumerate() {
      if !all && count >= 3 && (todo.due > tomorrow || count >= 5) {
        if todo.due <= tomorrow {
          println!("(and more)")
        }
        break;
      }
      println!(
        "{:0width$} {} {}",
        todo.task,
        todo.due.format("%Y-%m-%d"),
        title,
        width = max_id_len
      );
    }
  }
  Ok(())
}

fn list_done_todos<S: Store, A: Allesatt<Store = S>, B: Borrow<A>>(
  app: B,
) -> Result<(), Box<Error>> {
  let store = app.borrow().get_store();
  let todo_ids = store.get_todos(None, Some(true));
  let mut todos: Vec<_> = todo_ids
    .into_iter()
    .map(|todo_id| {
      let todo = store.get_todo(&todo_id).unwrap();
      let task = store.get_task(&todo.task).unwrap();
      (
        todo.task.clone(),
        todo.completed.clone().unwrap().date,
        task.title.clone(),
      )
    })
    .collect();
  if let Some(max_id_len) = todos
    .iter()
    .map(|(task_id, _, _)| task_id.to_string().len())
    .max()
  {
    todos.sort_unstable_by(|(_, completed1, _), (_, completed2, _)| completed1.cmp(completed2));
    for (task_id, completed, title) in todos {
      println!(
        "{:0width$} {} {}",
        task_id,
        completed.format("%Y-%m-%d"),
        title,
        width = max_id_len
      );
    }
  }
  Ok(())
}

fn list_tasks<S: Store, A: Allesatt<Store = S>, B: Borrow<A>>(app: B) -> Result<(), Box<Error>> {
  let store = app.borrow().get_store();
  let task_ids = store.get_tasks();
  if let Some(max_id_len) = task_ids.iter().map(|id| id.to_string().len()).max() {
    let mut tasks: Vec<_> = task_ids
      .into_iter()
      .map(|task_id| store.get_task(&task_id).unwrap())
      .collect();
    tasks.sort_unstable_by_key(|task| &task.title);
    for task in tasks {
      eprintln!("{:0width$} {}", task.id, task.title, width = max_id_len);
    }
  }
  Ok(())
}

fn create_task<S: Store, A: Allesatt<Store = S>, B: BorrowMut<A> + Borrow<A>>(
  mut app: B,
  description: &str,
  due_every: Duration,
) -> Result<(), Box<Error>> {
  app
    .borrow_mut()
    .create_task(description.into(), Some(due_every));
  list_tasks(app)
}

fn do_task<S: Store, A: Allesatt<Store = S>, B: BorrowMut<A> + Borrow<A>>(
  mut app: B,
  id: &TaskId,
) -> Result<(), Box<Error>> {
  let todo = app
    .borrow()
    .get_store()
    .get_todos(Some(id), Some(false))
    .first()
    .ok_or_else(|| String::from("Task not found"))?
    .clone();
  app
    .borrow_mut()
    .complete_todo(&todo, TodoCompleted::new(Local::now().naive_local()))?;

  Ok(())
}

fn task_later<S: Store, A: Allesatt<Store = S>, B: BorrowMut<A> + Borrow<A>>(
  mut app: B,
  id: &TaskId,
) -> Result<(), Box<Error>> {
  let todo = app
    .borrow()
    .get_store()
    .get_todos(Some(id), Some(false))
    .first()
    .ok_or_else(|| String::from("Task not found"))?
    .clone();
  app.borrow_mut().todo_later(&todo)?;

  Ok(())
}
