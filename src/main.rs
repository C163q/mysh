use std::{
    cell::RefCell,
    io::{self, Write},
    rc::Rc,
};

use mysh::{
    completion::ShellCompleter,
    env::{ExecContext, ExecEnv},
    result::CommandResult,
};
use rustyline::{CompletionType, Editor, error::ReadlineError};

fn main() -> anyhow::Result<()> {
    let mut rl = Editor::with_config(
        rustyline::Config::builder()
            .completion_show_all_if_ambiguous(true)
            .completion_type(CompletionType::List)
            .build(),
    )?;

    let path_env = mysh::get_path_env();
    let histfile_env = mysh::get_histfile_env();
    let base_dirs = directories::BaseDirs::new().expect("Failed to get base directories");
    let env = Rc::new(RefCell::new(ExecEnv::build(
        path_env,
        histfile_env,
        base_dirs,
    )));

    let completer = ShellCompleter::new(Rc::clone(&env));
    rl.set_helper(Some(completer));

    {
        let histfile_path = mysh::get_histfile_path(env.borrow());
        if rl.load_history(&histfile_path).is_err() {
            rl.save_history(&histfile_path)?;
        }
    }

    loop {
        let readline = rl.readline("$ ");
        let ret = match readline {
            Ok(line) => {
                rl.add_history_entry(line.as_str())?;
                let context = ExecContext::new(rl.history_mut());
                let ret = mysh::get_input_and_run(&line, Rc::clone(&env), context);
                io::stdout().flush()?;
                ret
            }
            Err(ReadlineError::Interrupted) => {
                // When Ctrl-C is pressed, bash and zsh just set return code to 130 (INT).
                // We follow their behavior here.

                // TODO: Return code is not implemented yet.
                CommandResult::Normal
            }
            Err(ReadlineError::Eof) => {
                // When Ctrl-D is pressed, bash and zsh just exit the shell.
                // While bash prints "exit" before exiting, zsh does not.
                // We follow zsh's behavior here.
                CommandResult::Exit
            }
            Err(e) => {
                return Err(anyhow::anyhow!(e));
            }
        };

        if ret == CommandResult::Exit {
            break;
        }
    }

    {
        let histfile_path = mysh::get_histfile_path(env.borrow());
        rl.save_history(&histfile_path)?;
    }

    Ok(())
}
