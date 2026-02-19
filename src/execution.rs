pub mod data;
pub mod process;
pub mod result;

use std::{
    cell::RefCell,
    collections::VecDeque,
    io::{self, PipeReader, PipeWriter},
    process::Child,
    rc::Rc,
};

use crate::{
    env::{ExecContext, ExecEnv},
    execution::{
        data::{CommandDescriptor, RawCommand},
        result::{CommandResult, ExecutionResult},
    },
    redirect::RedirectHandler,
};

pub fn execute_command_chain(
    mut exec_chain: VecDeque<CommandDescriptor>,
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
        Some(CommandDescriptor::Begin(exec)) => exec,
        _ => return CommandResult::Normal, // empty or invalid
    };

    let mut pipe_in = None;
    while let Some(CommandDescriptor::Pipe(exec)) = exec_chain.pop_front() {
        let (reader, writer) = io::pipe().unwrap(); // TODO: handle error
        let ret = execute_command(first, pipe_in, Some(writer), Rc::clone(&env), &mut context);

        first = exec;
        match ret {
            ExecutionResult::Running(child) => pool.processes.push_back(child),
            ExecutionResult::Exit => return CommandResult::Exit,
            ExecutionResult::Error(msg) => {
                eprintln!("{}", msg);
                return CommandResult::Normal;
            }
            ExecutionResult::Normal => { /* continue */ }
        }
        pipe_in = Some(reader);
    }

    let ret = execute_command(first, pipe_in, None, env, &mut context);
    match ret {
        ExecutionResult::Running(child) => {
            pool.processes.push_back(child);
            CommandResult::Normal
        }
        ExecutionResult::Exit => CommandResult::Exit,
        ExecutionResult::Error(msg) => {
            eprintln!("{}", msg);
            CommandResult::Normal
        }
        ExecutionResult::Normal => CommandResult::Normal,
    }
}

pub fn execute_command(
    raw_cmd: RawCommand,
    pipe_in: Option<PipeReader>,
    pipe_out: Option<PipeWriter>,
    env: Rc<RefCell<ExecEnv>>,
    context: &mut ExecContext,
) -> ExecutionResult {
    if raw_cmd.cmd == "exit" {
        return ExecutionResult::Exit;
    }

    let f = crate::builtin::BUILTIN_COMMANDS.with(|map| map.get(raw_cmd.cmd.as_str()).copied());
    if let Some(func) = f {
        // RedirectHandler scope
        let _handler = RedirectHandler::new(&raw_cmd.redirect);
        {
            let mut e = env.borrow_mut();
            e.pipe_in = pipe_in;
            e.pipe_out = pipe_out;

            func(raw_cmd.arguments, e, context);

            let mut e = env.borrow_mut();
            e.reset_pipes();
        }
        return ExecutionResult::Normal;
    }

    let mut builder = process::ChildBuilder::new(raw_cmd);
    if let Some(pipe_in) = pipe_in {
        builder.stdin(pipe_in);
    }
    if let Some(pipe_out) = pipe_out {
        builder.stdout(pipe_out);
    }
    builder
        .build()
        .map(ExecutionResult::Running)
        .unwrap_or_else(|e| ExecutionResult::Error(e.to_string()))
}
