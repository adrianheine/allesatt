use serde::de::Error as _;
use serde::{self, Deserializer, Serializer};
use serde_derive::{Deserialize, Serialize};
use std::fmt::{Display, Error as FmtError, Formatter};
use std::num::ParseIntError;
use std::str::FromStr;
use time::{OffsetDateTime, PrimitiveDateTime};

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
pub type TodoDate = OffsetDateTime;

#[derive(Clone, Debug)]
pub struct TodoCompleted {
  pub date: TodoDate,
}

impl TodoCompleted {
  pub const fn new(date: TodoDate) -> Self {
    Self { date }
  }
}

impl serde::Serialize for TodoCompleted {
  fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
  where
    S: Serializer,
  {
    serializer.serialize_str(&self.date.format("%FT%T.%N"))
  }
}

impl<'de> serde::Deserialize<'de> for TodoCompleted {
  fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
  where
    D: Deserializer<'de>,
  {
    let s = String::deserialize(deserializer)?;
    match PrimitiveDateTime::parse(&s, "%FT%T.%N").or_else(|_| PrimitiveDateTime::parse(s, "%FT%T"))
    {
      Ok(v) => Ok(Self::new(v.assume_utc())),
      Err(e) => Err(D::Error::custom(e.to_string())),
    }
  }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Todo {
  pub id: TodoId,
  pub task: TaskId,
  pub completed: Option<TodoCompleted>,
  pub due: TodoDate,
}
