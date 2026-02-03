use std::io::{self, Write};

use mysh::{completion::ShellCompleter, env::ExecEnv, result::ExecResult};
use rustyline::{Editor, error::ReadlineError};

fn main() -> anyhow::Result<()> {
    let mut rl = Editor::with_config(rustyline::Config::default())?;
    let completer = ShellCompleter::new();
    rl.set_helper(Some(completer));
    let path_env = mysh::get_path_env();
    let mut env = ExecEnv::build(path_env);
    loop {
        let readline = rl.readline("$ ");
        let ret = match readline {
            Ok(line) => {
                // TODO: History management
                // rl.add_history_entry(line.as_str())?;
                let ret = mysh::get_input_and_run(&line, &mut env);
                io::stdout().flush()?;
                ret
            }
            Err(ReadlineError::Interrupted) => {
                // When Ctrl-C is pressed, bash and zsh just set return code to 130 (INT).
                // We follow their behavior here.
                
                // TODO: Return code is not implemented yet.
                ExecResult::Normal
            }
            Err(ReadlineError::Eof) => {
                // When Ctrl-D is pressed, bash and zsh just exit the shell.
                // While bash prints "exit" before exiting, zsh does not.
                // We follow zsh's behavior here.
                ExecResult::Exit
            }
            Err(e) => {
                return Err(anyhow::anyhow!(e));
            }
        };

        if ret == ExecResult::Exit {
            break;
        }
    }
    Ok(())
}
