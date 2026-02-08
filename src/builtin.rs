use std::{
    cell::RefMut,
    collections::HashMap,
    fs::{DirEntry, ReadDir, read_dir},
    io::{self, Write},
    ops::Deref,
    path::{Path, PathBuf},
};

use is_executable::IsExecutable;
use rustyline::history::History;

use crate::env::{ExecContext, ExecEnv};

type BuiltinExecFunc = fn(Vec<String>, RefMut<ExecEnv>, &mut ExecContext);

// single thread, so we use thread_local
thread_local! {
    /// list of built-in commands
    pub static BUILTIN_COMMANDS: HashMap<&'static str, BuiltinExecFunc> = {
        let mut map = HashMap::<&'static str, BuiltinExecFunc>::new();
        map.insert("exit",    exit_command);
        map.insert("echo",    echo_command);
        map.insert("type",    type_command);
        map.insert("pwd",     pwd_command);
        map.insert("cd",      cd_command);
        map.insert("history", history_command);
        map
    };
}

macro_rules! builtin_output {
    ($env:expr, $($arg:tt)*) => {
        #[allow(clippy::explicit_write)]
        match &mut $env.pipe_out {
            // We use write! to avoid capturing stdout in tests.
            None => write!(io::stdout(), $($arg)*).unwrap(),
            Some(pipe_out) => write!(pipe_out, $($arg)*).unwrap(),
        }
    };
}

macro_rules! builtin_error {
    ($env:expr, $($arg:tt)*) => {
        #[allow(clippy::explicit_write)]
        write!(io::stderr(), $($arg)*).unwrap()
    };
}

/// echo command implementation
pub fn echo_command(args: Vec<String>, mut env: RefMut<ExecEnv>, _: &mut ExecContext) {
    builtin_output!(env, "{}\n", args.join(" "));
}

/// exit command should be handled earlier, so it does nothing here
pub fn exit_command(_: Vec<String>, _: RefMut<ExecEnv>, _: &mut ExecContext) {}

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
pub fn type_command(args: Vec<String>, mut env: RefMut<ExecEnv>, _: &mut ExecContext) {
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
        builtin_output!(env, "{} is a shell builtin\n", first_arg);
        return;
    }

    // external command
    if let Some(entry) = get_executable_in_path(first_arg, env.deref()) {
        builtin_output!(env, "{} is {}\n", first_arg, entry.path().display());
        return;
    }

    builtin_error!(env, "{}: not found\n", first_arg);
}

pub fn pwd_command(_: Vec<String>, mut env: RefMut<ExecEnv>, _: &mut ExecContext) {
    if let Ok(path) = std::env::current_dir() {
        builtin_output!(env, "{}\n", path.display());
    }
}

pub fn cd_command(args: Vec<String>, _env: RefMut<ExecEnv>, _: &mut ExecContext) {
    fn navigate(path: &Path) {
        if std::env::set_current_dir(path).is_err() {
            builtin_error!(_env, "cd: {}: No such file or directory\n", path.display());
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

struct HistoryArgs {
    num: Option<usize>,
    read: Option<String>,
    write: Option<String>,
    append: Option<String>,
}

impl HistoryArgs {
    fn new() -> Self {
        Self {
            num: None,
            read: None,
            write: None,
            append: None,
        }
    }

    fn with_num(mut self, num: usize) -> Self {
        self.num = Some(num);
        self
    }

    fn with_read(mut self, read: String) -> Self {
        self.read = Some(read);
        self
    }

    fn with_write(mut self, write: String) -> Self {
        self.write = Some(write);
        self
    }

    fn with_append(mut self, append: String) -> Self {
        self.append = Some(append);
        self
    }
}

fn parse_history_args(args: Vec<String>) -> HistoryArgs {
    let args_len = args.len();
    for (i, arg) in args.iter().enumerate() {
        if arg == "-r" && i + 1 < args_len {
            return HistoryArgs::new().with_read(args[i + 1].clone());
        } else if arg == "-w" && i + 1 < args_len {
            return HistoryArgs::new().with_write(args[i + 1].clone());
        } else if arg == "-a" && i + 1 < args_len {
            return HistoryArgs::new().with_append(args[i + 1].clone());
        } else if let Ok(num) = arg.parse::<usize>() {
            return HistoryArgs::new().with_num(num);
        }
    }

    HistoryArgs::new()
}

fn list_history(mut env: RefMut<ExecEnv>, context: &ExecContext, num: usize) {
    let ignore = context.history.len().saturating_sub(num);

    context
        .history
        .iter()
        .enumerate()
        .skip(ignore)
        .for_each(|(index, entry)| {
            builtin_output!(env, "    {}  {}\n", index + 1, entry);
        });
}

pub fn history_command(args: Vec<String>, env: RefMut<ExecEnv>, context: &mut ExecContext) {
    // Some shells don't add the `history` command to the history list,
    // but we will add it for simplicity.
    let args = parse_history_args(args);

    if let Some(read_file) = args.read {
        let path = PathBuf::from(read_file);
        if let Err(e) = context.history.load(&path) {
            builtin_error!(env, "history: {}: {}\n", path.display(), e);
        }
        return;
    }

    if let Some(write_file) = args.write {
        let path = PathBuf::from(write_file);
        if let Err(e) = context.history.save(&path) {
            builtin_error!(env, "history: {}: {}\n", path.display(), e);
        }
        return;
    }

    if let Some(append_file) = args.append {
        let path = PathBuf::from(append_file);
        if let Err(e) = context.history.append(&path) {
            builtin_error!(env, "history: {}: {}\n", path.display(), e);
        }
        return;
    }

    let num = args.num.unwrap_or(context.history.len());

    list_history(env, context, num);
}
