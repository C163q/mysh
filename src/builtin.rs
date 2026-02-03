use std::{
    collections::HashMap,
    fs::{DirEntry, ReadDir, read_dir},
    io::{self, Write},
    path::{Path, PathBuf},
};

use is_executable::IsExecutable;

use crate::env::ExecEnv;

type BuiltinExecFunc = fn(Vec<String>, &mut ExecEnv);

// single thread, so we use thread_local
thread_local! {
    /// list of built-in commands
    pub static BUILTIN_COMMANDS: HashMap<&'static str, BuiltinExecFunc> = {
        let mut map = HashMap::<&'static str, BuiltinExecFunc>::new();
        map.insert("exit", exit_command);
        map.insert("echo", echo_command);
        map.insert("type", type_command);
        map.insert("pwd",  pwd_command);
        map.insert("cd",   cd_command);
        map
    };
}

/// echo command implementation
///
/// We use writeln! to avoid capturing stdout in tests.
#[allow(clippy::explicit_write)]
pub fn echo_command(args: Vec<String>, _: &mut ExecEnv) {
    writeln!(io::stdout(), "{}", args.join(" ")).unwrap();
}

/// exit command should be handled earlier, so it does nothing here
pub fn exit_command(_: Vec<String>, _: &mut ExecEnv) {}

fn get_executable_in_path(cmd: &str, env: &ExecEnv) -> Option<DirEntry> {
    fn dir_get_executable(name: &str, reader: ReadDir) -> Option<DirEntry> {
        reader
            .flatten()
            .find(|entry| entry.path().is_executable() && entry.file_name() == name)
    }

    for dir in env.path_env.iter() {
        if let Ok(entries) = read_dir(dir)
            && let Some(entry) = dir_get_executable(cmd, entries)
        {
            return Some(entry);
        }
    }

    None
}

/// type command implementation
#[allow(clippy::explicit_write)]
pub fn type_command(args: Vec<String>, env: &mut ExecEnv) {
    // For now, we just handle one argument
    let first_arg = match args.first() {
        Some(arg) => arg,
        None => {
            // Handle no argument case, typically do nothing and return 1
            // We will do this later
            return;
        }
    };
    let builtin = BUILTIN_COMMANDS.with(|cmds| cmds.contains_key(first_arg.as_str()));

    // builtin command
    if builtin {
        writeln!(io::stdout(), "{} is a shell builtin", first_arg).unwrap();
        return;
    }

    // external command
    if let Some(entry) = get_executable_in_path(first_arg, env) {
        writeln!(io::stdout(), "{} is {}", first_arg, entry.path().display()).unwrap();
        return;
    }

    writeln!(io::stderr(), "{}: not found", first_arg).unwrap();
}

#[allow(clippy::explicit_write)]
pub fn pwd_command(_: Vec<String>, _: &mut ExecEnv) {
    if let Ok(path) = std::env::current_dir() {
        writeln!(io::stdout(), "{}", path.display()).unwrap();
    }
}

#[allow(clippy::explicit_write)]
pub fn cd_command(args: Vec<String>, _: &mut ExecEnv) {
    fn navigate(path: &Path) {
        if std::env::set_current_dir(path).is_err() {
            writeln!(
                io::stderr(),
                "cd: {}: No such file or directory",
                path.display()
            )
            .unwrap();
        }
    }

    fn navigate_to_home() {
        // When $HOME is not set, `bash` will print "bash: cd: HOME not set",
        // while `zsh` will just do nothing. We follow `zsh`'s behavior here.
        if let Some(home_dir) = std::env::home_dir() {
            navigate(&home_dir);
        }
    }

    match args.first() {
        None => {
            navigate_to_home();
        }
        Some(p) => {
            if p == "~" {
                navigate_to_home();
                return;
            }

            let path = PathBuf::from(p);
            navigate(&path);
        }
    }
}
