pub mod builtin;
pub mod env;
pub mod parse;
pub mod result;
pub mod redirect;

use crate::{
    env::{ExecEnv, PathEnv}, result::ExecResult
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

pub fn get_input_and_run(input: &str, env: &mut ExecEnv) -> ExecResult {
    let parse_data = match parse::parse_command(input) {
        Some(data) => data,
        None => return ExecResult::Normal,
    };

    match parse_data.first_arg {
        None => ExecResult::Normal,
        Some(cmd) => parse::execute_command(cmd, parse_data.arguments, env, parse_data.redirect),
    }
}
