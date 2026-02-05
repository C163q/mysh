pub mod builtin;
pub mod completion;
pub mod env;
pub mod parse;
pub mod redirect;
pub mod result;

use std::{cell::RefCell, rc::Rc};

use crate::{
    env::{ExecEnv, PathEnv},
    result::ExecResult,
};

pub fn get_path_env() -> PathEnv {
    match std::env::var_os("PATH") {
        None => PathEnv::new(),
        Some(paths) => {
            let paths: Vec<_> = std::env::split_paths(&paths).collect();
            PathEnv::from_paths(paths)
        }
    }
}

pub fn get_input_and_run(input: &str, env: Rc<RefCell<ExecEnv>>) -> ExecResult {
    let parse_data = match parse::parse_command(input) {
        Some(data) => data,
        None => return ExecResult::Normal,
    };

    match parse_data.first_arg {
        None => ExecResult::Normal,
        Some(cmd) => parse::execute_command(cmd, parse_data.arguments, env, parse_data.redirect),
    }
}
