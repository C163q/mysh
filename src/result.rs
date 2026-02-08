use std::process::Child;

// TODO: improve
#[derive(Debug)]
pub enum ExecResult {
    Exit,
    Normal,
    Running(Child),
    Error(String),
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub enum CommandResult {
    Exit,
    Normal,
}
