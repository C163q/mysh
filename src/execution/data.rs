use crate::{parse::ParseData, redirect::Redirect};

#[derive(Debug)]
pub struct RawCommand {
    pub cmd: String,
    pub arguments: Vec<String>,
    pub redirect: Redirect,
}

impl RawCommand {
    pub fn new(cmd: String, arguments: Vec<String>, redirect: Redirect) -> Self {
        Self {
            cmd,
            arguments,
            redirect,
        }
    }

    pub(crate) fn from_parse_data(data: ParseData) -> Option<Self> {
        match data.first_arg {
            Some(cmd) => Some(Self::new(cmd, data.arguments, data.redirect)),
            None => None,
        }
    }
}

#[derive(Debug)]
pub enum CommandDescriptor {
    Begin(RawCommand),
    Pipe(RawCommand),
}
