use std::io::{self, Write};

use mysh::{env::ExecEnv, result::ExecResult};

fn main() {
    let path_env = mysh::get_path_env();
    let mut env = ExecEnv::build(path_env);
    loop {
        print!("$ ");
        io::stdout().flush().unwrap();

        let mut input = String::new();
        io::stdin().read_line(&mut input).unwrap();
        let input = input.trim();
        let ret = mysh::get_input_and_run(input, &mut env);
        io::stdout().flush().unwrap();

        if ret == ExecResult::Exit {
            break;
        }
    }
}
