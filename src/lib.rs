pub mod builtin;
pub mod completion;
pub mod env;
pub mod execution;
pub mod parse;
pub mod redirect;

use std::{
    cell::{Ref, RefCell},
    fs::DirBuilder,
    path::PathBuf,
    rc::Rc,
};

use crate::{
    env::{ExecContext, ExecEnv, PathEnv},
    execution::result::CommandResult,
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

pub fn get_histfile_env() -> Option<PathBuf> {
    std::env::var_os("HISTFILE").map(PathBuf::from)
}

pub fn get_histfile_path(env: Ref<ExecEnv>) -> PathBuf {
    let opt_histfile_path = env.histfile_env.clone();

    match opt_histfile_path {
        Some(path) => path,
        None => {
            let path = env.base_dirs.data_local_dir();
            if !path.exists() {
                DirBuilder::new().recursive(true).create(path).unwrap(); // TODO: handle error
            }
            env.base_dirs
                .data_local_dir()
                .to_owned()
                .join("mysh_history")
        }
    }
}

pub fn get_input_and_run(
    input: &str,
    env: Rc<RefCell<ExecEnv>>,
    history: ExecContext,
) -> CommandResult {
    let exec = parse::parse_command(input);
    execution::execute_command_chain(exec, env, history)
}
