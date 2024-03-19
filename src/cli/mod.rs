use crate::engine::{
  get_todos, try_new as try_new_engine, Allesatt, ReadWriteLogger, Store, Task, TaskId,
  TodoCompleted, TodoDate, TodoId,
};
use clap::{Parser, Subcommand};
use humantime::Duration as HumanDuration;
use std::borrow::{Borrow, BorrowMut};
use std::error::Error;
use std::fs::OpenOptions;
use std::io::{self, stderr, stdin, stdout, Stdout, Write};
use time::format_description::FormatItem;
use time::macros::format_description;
use time::OffsetDateTime;

const DAY_FORMAT: &[FormatItem<'static>] = format_description!("[year]-[month]-[day]");

#[derive(Debug, Parser)]
#[command(name = "Allesatt", author, version, about)]
struct Opts {
  #[arg(long, short, default_value = "-")]
  /// File to read from and write to. If missing or -, will use stdout and stdin.
  file: String,

  #[command(subcommand)]
  cmd: Option<Cmd>,
}

#[derive(Debug, Subcommand)]
enum Cmd {
  #[clap(visible_alias("ls"))]
  /// List tasks
  List {
    #[arg(long)]
    /// Show all todos (the default is to only show a few todos)
    all: bool,
  },

  /// Add a new task
  Add {
    #[arg(long, default_value = "30days")]
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

  /// Mark a task as not needing doing currently
  Pause { id: TaskId },

  /// Mark a task as needing doing again
  Unpause { id: TaskId },
}

impl Cmd {
  pub const fn readonly(&self) -> bool {
    matches!(self, Self::List { .. } | Self::Done { .. })
  }
}

pub fn cli<S: Store>(store: S) -> Result<(), Box<dyn Error>> {
  let opts = Opts::parse();
  match opts.file.as_ref() {
    "-" => handle_command(
      opts.cmd,
      try_new_engine(
        store,
        ReadWriteLogger::<_, Stdout, _>::new(stdin(), &mut stdout()),
      )?,
    ),
    file_name => {
      let file = OpenOptions::new().read(true).append(true).open(file_name)?;
      let engine = try_new_engine(store, ReadWriteLogger::new(&file, &file))?;
      handle_command(opts.cmd, engine)
    }
  }
}
fn handle_command<S: Store, A: Allesatt<Store = S>, B: BorrowMut<A> + Borrow<A>>(
  command: Option<Cmd>,
  app: B,
) -> Result<(), Box<dyn Error>> {
  let cmd = command.unwrap_or_else(|| Cmd::List {
    all: atty::isnt(atty::Stream::Stdout),
  });
  if cmd.readonly() {
    handle_command_impl(&cmd, app, &mut stdout())
  } else {
    handle_command_impl(&cmd, app, &mut stderr())
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
    Cmd::Pause { id } => pause_task(app, output, id),
    Cmd::Unpause { id } => unpause_task(app, output, id),
  }
}

fn list_todos<S: Store, A: Allesatt<Store = S>, B: Borrow<A>, W: Write>(
  app: B,
  output: &mut W,
  all: bool,
) -> Result<(), Box<dyn Error>> {
  let (todos, paused_tasks, and_more) = get_todos(app.borrow().get_store(), all);
  let Some(max_id_len) = todos
    .iter()
    .map(|(todo, _)| todo.task.to_string().len())
    .chain(paused_tasks.iter().map(|task| task.id.to_string().len()))
    .max()
  else {
    return Ok(());
  };
  for (todo, task) in &todos {
    write_todo(output, max_id_len, task, &todo.due)?;
  }
  if and_more {
    writeln!(output, "(and more)")?;
  }

  if !paused_tasks.is_empty() {
    if !todos.is_empty() {
      writeln!(output)?;
    }
    writeln!(output, "Paused tasks:")?;
    for task in paused_tasks {
      write_paused_task(output, max_id_len, task)?;
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
      (task, todo.completed.as_ref().unwrap().date)
    })
    .collect();
  if let Some(max_id_len) = todos
    .iter()
    .map(|(task, _)| task.id.to_string().len())
    .max()
  {
    todos.sort_unstable_by(|(_, completed1), (_, completed2)| completed1.cmp(completed2));
    for (task, completed) in todos {
      write_todo(output, max_id_len, task, &completed)?;
    }
  }
  Ok(())
}

fn create_task<S: Store, A: Allesatt<Store = S>, B: BorrowMut<A> + Borrow<A>, W: Write>(
  mut app: B,
  output: &mut W,
  description: &str,
  due_every: &HumanDuration,
) -> Result<(), Box<dyn Error>> {
  let (task_id, todo_id) = app
    .borrow_mut()
    .create_task(description.into(), Some(**due_every));
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
  write_todo(output, 0, task, &todo.due)
}

fn write_todo(
  output: &mut impl Write,
  width: usize,
  Task { id, title }: &Task,
  date: &TodoDate,
) -> Result<(), Box<dyn Error>> {
  let date = date.format(&DAY_FORMAT)?;
  writeln!(output, "{id:width$} {date} {title}")?;
  Ok(())
}

fn write_paused_task(
  output: &mut impl Write,
  width: usize,
  Task { id, title }: &Task,
) -> io::Result<()> {
  writeln!(output, "{id:width$} {title}")
}

fn do_task<S: Store, A: Allesatt<Store = S>, B: BorrowMut<A> + Borrow<A>, W: Write>(
  mut app: B,
  output: &mut W,
  id: &TaskId,
) -> Result<(), Box<dyn Error>> {
  let todo_id = app
    .borrow()
    .get_store()
    .find_open_todo(id)
    .ok_or("Task not found")?
    .id
    .clone();
  app
    .borrow_mut()
    .complete_todo(&todo_id, TodoCompleted::new(OffsetDateTime::now_utc()))?;
  let store = app.borrow().get_store();
  let todo = store.find_open_todo(id).ok_or("Task not found")?;
  print_todo(store, output, id, &todo.id)
}

fn pause_task<S: Store, A: Allesatt<Store = S>, B: BorrowMut<A> + Borrow<A>, W: Write>(
  mut app: B,
  output: &mut W,
  id: &TaskId,
) -> Result<(), Box<dyn Error>> {
  app.borrow_mut().pause_task(id)?;
  let store = app.borrow().get_store();
  write_paused_task(output, 0, store.get_task(id).unwrap())?;
  Ok(())
}

fn unpause_task<S: Store, A: Allesatt<Store = S>, B: BorrowMut<A> + Borrow<A>, W: Write>(
  mut app: B,
  output: &mut W,
  id: &TaskId,
) -> Result<(), Box<dyn Error>> {
  let todo_id = app.borrow_mut().unpause_task(id)?;
  let store = app.borrow().get_store();
  print_todo(store, output, id, &todo_id)
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
  use super::{handle_command_impl, Cmd, DAY_FORMAT};
  use crate::engine::{try_new as try_new_engine, MemStore, ReadWriteLogger, TaskId};
  use regex::{escape, Regex};
  use std::borrow::Borrow;
  use std::fmt::Display;
  use std::str::FromStr;
  use time::{Duration, OffsetDateTime};

  fn today_plus(days: i64) -> impl Display {
    (OffsetDateTime::now_utc() + Duration::days(days))
      .format(&DAY_FORMAT)
      .unwrap()
  }

  fn exec_command(cmd: impl Borrow<Cmd>, log_in: impl Borrow<str>) -> (String, String) {
    let log_in = log_in.borrow();
    let mut output = Vec::new();
    let mut log_out: Vec<u8> = Vec::new();
    handle_command_impl(
      cmd.borrow(),
      try_new_engine(
        MemStore::new(),
        ReadWriteLogger::<_, Vec<u8>, _>::new(log_in.as_bytes(), &mut log_out),
      )
      .unwrap(),
      &mut output,
    )
    .unwrap();
    let log_out = std::str::from_utf8(&log_out).unwrap();
    (
      String::with_capacity(log_in.len() + log_out.len()) + log_in + log_out,
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
      log_out,
    );
    assert_eq!(output, format!("1 {} task\n", today_plus(0)));
    assert_eq!(
      log_out,
      "create_task1: [\"task\", {\"secs\":2592000,\"nanos\":0}, 1, 1]\n"
    );

    let (log_out, output) = exec_command(
      Cmd::Do {
        id: TaskId::from_str("1").unwrap(),
      },
      log_out,
    );
    assert_eq!(output, format!("1 {} task\n", today_plus(30)));
    let r = Regex::new(
      &(escape(
        r#"create_task1: ["task", {"secs":2592000,"nanos":0}, 1, 1]
complete_todo1: [1, ""#,
      ) + &today_plus(0).to_string()
        + r#"T[0-9]{2}:[0-9]{2}:[0-9]{2}\.[0-9]+"\]
"#),
    )
    .unwrap();
    assert!(r.is_match(&log_out));

    let (log_out, output) = exec_command(
      Cmd::Later {
        id: TaskId::from_str("1").unwrap(),
      },
      log_out,
    );
    assert_eq!(output, format!("1 {} task\n", today_plus(30 + 6)));
    let r = Regex::new(&(r.to_string() + "todo_later1: \\[2\\]\n")).unwrap();
    assert!(r.is_match(&log_out));

    let (log_out, output) = exec_command(
      Cmd::Pause {
        id: TaskId::from_str("1").unwrap(),
      },
      log_out,
    );
    assert_eq!(output, "1 task\n");
    let r = Regex::new(&(r.to_string() + "pause_task1: \\[1\\]\n")).unwrap();
    assert!(r.is_match(&log_out));

    let (new_log_out, output) = exec_command(Cmd::List { all: false }, log_out.as_ref());
    assert_eq!(output, "Paused tasks:\n1 task\n");
    assert_eq!(new_log_out, log_out);

    let (log_out, output) = exec_command(
      Cmd::Unpause {
        id: TaskId::from_str("1").unwrap(),
      },
      log_out,
    );
    assert_eq!(output, format!("1 {} task\n", today_plus(0)));
    let r = Regex::new(&(r.to_string() + "unpause_task1: \\[1\\]\n")).unwrap();
    assert!(r.is_match(&log_out));

    let (new_log_out, output) = exec_command(Cmd::List { all: false }, log_out.as_ref());
    assert_eq!(output, format!("1 {} task\n", today_plus(0)));
    assert_eq!(new_log_out, log_out);
  }

  #[test]
  fn list_todos() {
    let log_out = [
      &Cmd::Add {
        every: "30days".parse().unwrap(),
        description: "Task 1".into(),
      },
      &Cmd::Do {
        id: TaskId::from_str("1").unwrap(),
      },
      &Cmd::Add {
        every: "30days".parse().unwrap(),
        description: "Task 2".into(),
      },
      &Cmd::Do {
        id: TaskId::from_str("2").unwrap(),
      },
      &Cmd::Add {
        every: "30days".parse().unwrap(),
        description: "Task 3".into(),
      },
      &Cmd::Do {
        id: TaskId::from_str("3").unwrap(),
      },
      &Cmd::Add {
        every: "30days".parse().unwrap(),
        description: "Task 4 due".into(),
      },
      &Cmd::Add {
        every: "30days".parse().unwrap(),
        description: "Task 5 due".into(),
      },
    ]
    .iter()
    .fold(String::new(), |log_out, &cmd| exec_command(cmd, log_out).0);

    let (_, output) = exec_command(Cmd::List { all: false }, &*log_out);
    let r = Regex::new(&format!(
      "^4 {0} Task 4 due\n5 {0} Task 5 due\n1 {1} Task 1\n$",
      today_plus(0),
      today_plus(30)
    ))
    .unwrap();
    assert!(r.is_match(&output));

    let (_, output) = exec_command(Cmd::List { all: true }, &*log_out);
    let r = Regex::new(&format!(
      "^4 {0} Task 4 due\n5 {0} Task 5 due\n1 {1} Task 1\n2 {1} Task 2\n3 {1} Task 3\n$",
      today_plus(0),
      today_plus(30)
    ))
    .unwrap();
    assert!(r.is_match(&output));

    let log_out = [
      &Cmd::Add {
        every: "30days".parse().unwrap(),
        description: "Task 6 paused".into(),
      },
      &Cmd::Add {
        every: "30days".parse().unwrap(),
        description: "Task 7 paused".into(),
      },
      &Cmd::Pause {
        id: TaskId::from_str("7").unwrap(),
      },
      &Cmd::Pause {
        id: TaskId::from_str("6").unwrap(),
      },
    ]
    .iter()
    .fold(log_out, |log_out, &cmd| exec_command(cmd, log_out).0);

    let (_, output) = exec_command(Cmd::List { all: false }, &*log_out);
    let r = Regex::new(&format!(
      "^4 {0} Task 4 due\n5 {0} Task 5 due\n1 {1} Task 1\n\nPaused tasks:\n6 Task 6 paused\n7 Task 7 paused\n$",
      today_plus(0),
      today_plus(30)
    ))
    .unwrap();
    assert!(r.is_match(&output));

    let (_, output) = exec_command(Cmd::List { all: true }, &*log_out);
    let r = Regex::new(&format!(
      "^4 {0} Task 4 due\n5 {0} Task 5 due\n1 {1} Task 1\n2 {1} Task 2\n3 {1} Task 3\n\nPaused tasks:\n6 Task 6 paused\n7 Task 7 paused\n$",
      today_plus(0),
      today_plus(30)
    ))
    .unwrap();
    assert!(r.is_match(&output));

    let log_out = [
      &Cmd::Add {
        every: "30days".parse().unwrap(),
        description: "Task 8 due".into(),
      },
      &Cmd::Add {
        every: "30days".parse().unwrap(),
        description: "Task 9 due".into(),
      },
      &Cmd::Add {
        every: "30days".parse().unwrap(),
        description: "Task 10".into(),
      },
      &Cmd::Do {
        id: TaskId::from_str("10").unwrap(),
      },
    ]
    .iter()
    .fold(log_out, |log_out, &cmd| exec_command(cmd, log_out).0);

    let (_, output) = exec_command(Cmd::List { all: false }, &*log_out);
    let r = Regex::new(&format!(
      "^4 {0} Task 4 due\n5 {0} Task 5 due\n8 {0} Task 8 due\n9 {0} Task 9 due\n\nPaused tasks:\n6 Task 6 paused\n7 Task 7 paused\n$",
      today_plus(0),
    ))
    .unwrap();
    assert!(r.is_match(&output));

    let (_, output) = exec_command(Cmd::List { all: true }, &*log_out);
    let r = Regex::new(&format!(
      "^ 4 {0} Task 4 due\n 5 {0} Task 5 due\n 8 {0} Task 8 due\n 9 {0} Task 9 due\n 1 {1} Task 1\n 2 {1} Task 2\n 3 {1} Task 3\n10 {1} Task 10\n\nPaused tasks:\n 6 Task 6 paused\n 7 Task 7 paused\n$",
      today_plus(0),
      today_plus(30),
    ))
    .unwrap();
    assert!(r.is_match(&output));
  }
}
