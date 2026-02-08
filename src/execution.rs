use std::{
    cell::RefCell,
    collections::VecDeque,
    io::{self, PipeReader, PipeWriter},
    process::{self, Child},
    rc::Rc,
};

use crate::{
    env::{ExecContext, ExecEnv},
    parse::ParseData,
    redirect::{Redirect, RedirectHandler},
    result::{CommandResult, ExecResult},
};

#[derive(Debug)]
pub struct Execution {
    pub cmd: String,
    pub arguments: Vec<String>,
    pub redirect: Redirect,
}

impl Execution {
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
pub enum ExecutionDescriptor {
    Begin(Execution),
    Pipe(Execution),
}

pub fn execute_command_chain(
    mut exec_chain: VecDeque<ExecutionDescriptor>,
    env: Rc<RefCell<ExecEnv>>,
    mut context: ExecContext,
) -> CommandResult {
    /// pools of child processes to wait for
    struct ExecChainGuard {
        processes: VecDeque<Child>,
    }

    impl ExecChainGuard {
        fn new() -> Self {
            Self {
                processes: VecDeque::new(),
            }
        }
    }

    impl Drop for ExecChainGuard {
        fn drop(&mut self) {
            for mut child in self.processes.drain(..) {
                let _ = child.wait(); // TODO: handle error
            }
        }
    }

    let mut pool = ExecChainGuard::new();

    let mut first = match exec_chain.pop_front() {
        Some(ExecutionDescriptor::Begin(exec)) => exec,
        _ => return CommandResult::Normal, // empty or invalid
    };

    let mut pipe_in = None;
    while let Some(ExecutionDescriptor::Pipe(exec)) = exec_chain.pop_front() {
        let (reader, writer) = io::pipe().unwrap(); // TODO: handle error
        let ret = execute_command(first, pipe_in, Some(writer), Rc::clone(&env), &mut context);

        first = exec;
        match ret {
            ExecResult::Running(child) => pool.processes.push_back(child),
            ExecResult::Exit => return CommandResult::Exit,
            ExecResult::Error(msg) => {
                eprintln!("{}", msg);
                return CommandResult::Normal;
            }
            ExecResult::Normal => { /* continue */ }
        }
        pipe_in = Some(reader);
    }

    let ret = execute_command(first, pipe_in, None, env, &mut context);
    match ret {
        ExecResult::Running(child) => {
            pool.processes.push_back(child);
            CommandResult::Normal
        }
        ExecResult::Exit => CommandResult::Exit,
        ExecResult::Error(msg) => {
            eprintln!("{}", msg);
            CommandResult::Normal
        }
        ExecResult::Normal => CommandResult::Normal,
    }
}

pub fn execute_command(
    exec: Execution,
    pipe_in: Option<PipeReader>,
    pipe_out: Option<PipeWriter>,
    env: Rc<RefCell<ExecEnv>>,
    context: &mut ExecContext,
) -> ExecResult {
    if exec.cmd == "exit" {
        return ExecResult::Exit;
    }

    let f = crate::builtin::BUILTIN_COMMANDS.with(|map| map.get(exec.cmd.as_str()).copied());
    if let Some(func) = f {
        // RedirectHandler scope
        let _handler = RedirectHandler::new(&exec.redirect);
        {
            let mut e = env.borrow_mut();
            e.pipe_in = pipe_in;
            e.pipe_out = pipe_out;

            func(exec.arguments, e, context);

            let mut e = env.borrow_mut();
            e.reset_pipes();
        }
        return ExecResult::Normal;
    }

    let mut command = process::Command::new(&exec.cmd);
    {
        // RedirectHandler scope
        let _handler = RedirectHandler::new(&exec.redirect);
        if let Some(pipe_in) = pipe_in {
            command.stdin(pipe_in);
        }
        if let Some(pipe_out) = pipe_out {
            command.stdout(pipe_out);
        }
        if let Ok(child) = command.args(exec.arguments).spawn() {
            return ExecResult::Running(child);
        }
    } // RedirectHandler dropped here, restore fds

    ExecResult::Error(format!("{}: command not found", &exec.cmd))
}
