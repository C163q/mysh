use std::{path::PathBuf, process};

use crate::{
    env::ExecEnv,
    redirect::{InputRedirect, OutputRedirect, Redirect, RedirectHandler, RedirectParseInfo},
    result::ExecResult,
};

pub fn execute_command(cmd: String, args: Vec<String>, env: &mut ExecEnv, redirect: Redirect) -> ExecResult {
    if cmd == "exit" {
        return ExecResult::Exit;
    }

    let f = crate::builtin::BUILTIN_COMMANDS.with(|map| map.get(cmd.as_str()).copied());

    if let Some(func) = f {
        // RedirectHandler scope
        let _handler = RedirectHandler::new(&redirect);
        func(args, env);
        return ExecResult::Normal;
    }

    let mut command = process::Command::new(&cmd);
    {
        // RedirectHandler scope
        let _handler = RedirectHandler::new(&redirect);
        if let Ok(mut child) = command.args(args).spawn() {
            let _ = child.wait(); // TODO: handle errors
            return ExecResult::Normal;
        }
    }   // RedirectHandler dropped here, restore fds

    eprintln!("{}: command not found", &cmd);
    ExecResult::Normal
}

pub(crate) struct ParseData {
    pub first_arg: Option<String>,
    pub arguments: Vec<String>,
    pub redirect: Redirect,
}

//TODO: Use `Result<ParseData, Error>` later
fn parse(input: &str) -> Option<ParseData> {
    let mut first_arg: Option<String> = None;
    let mut arguments: Vec<String> = Vec::new();
    // To build the current argument
    let mut str_builder = String::new();
    // To handle single quotes
    let mut single_quote = false;
    // To handle double quotes
    let mut double_quote = false;
    // To handle backslashes
    // TODO: handle backslashes with newline
    let mut backslash = false;
    // To handle redirections
    let mut redirect = Redirect::new();


    let mut redirect_info: Option<RedirectParseInfo> = None;

    fn update_args(
        first_arg: &mut Option<String>,
        arguments: &mut Vec<String>,
        str_builder: &mut String,
    ) {
        if str_builder.is_empty() {
            return;
        }
        if first_arg.is_none() {
            first_arg.replace(str_builder.clone());
        } else {
            arguments.push(str_builder.clone());
        }
        str_builder.clear();
    }

    fn add_redirect(redirect: &mut Redirect, info: &RedirectParseInfo, str_builder: &mut String) {
        if !str_builder.is_empty() {
            if info.is_input {
                let mut input_redirect = InputRedirect::new(PathBuf::from(str_builder.clone()));
                input_redirect.set_fd(info.fd.unwrap_or(0));
                redirect.push_input(input_redirect);
            } else {
                let mut output_redirect = OutputRedirect::new(PathBuf::from(str_builder.clone()));
                output_redirect.set_append(info.append);
                output_redirect.set_fd(info.fd.unwrap_or(1));
                redirect.push_output(output_redirect);
            }
        }
        str_builder.clear();
    }

    // Handle single quotes
    for c in input.chars() {
        if single_quote {
            if c == '\'' {
                single_quote = false;
                continue;
            }
            str_builder.push(c);
            continue;
        }

        if double_quote {
            // Within double quotes, a backslash only escapes certain special characters:
            // `"`, `\`, `$`, ```, and newline. For all other characters, the backslash is treated
            // literally.
            //
            // "\n" is still newline, but we don't it now.
            if backslash {
                match c {
                    '"' | '\\' | '$' | '`' => {}
                    _ => {
                        str_builder.push('\\');
                    }
                }
                str_builder.push(c);
                backslash = false;
                continue;
            }

            match c {
                '"' => double_quote = false,
                '\\' => backslash = true,
                _ => str_builder.push(c),
            }
            continue;
        }

        if backslash {
            str_builder.push(c);
            backslash = false;
            continue;
        }

        // Things after `>` or `<`
        // for example:
        // ```
        // > 1.txt
        // -------
        //
        // >1.txt value
        // ------
        // ```
        if let Some(info) = redirect_info.as_mut() {
            match c {
                '\\' => backslash = true,
                '\'' => single_quote = true,
                '\"' => double_quote = true,
                '>' => {
                    if info.is_input {
                        // Previous is input redirection
                        add_redirect(&mut redirect, info, &mut str_builder);
                        // info.fd is None, because `add_redirect` clear the `str_builder`
                        redirect_info = Some(RedirectParseInfo::new_output());
                    } else if info.append_pending {
                        // probably `>>`
                        info.append = true;
                    } else {
                        // This can be merged with the first branch, but for clarity, we
                        // separate them.
                        //
                        // !info.append_pending => a new output redirection
                        add_redirect(&mut redirect, info, &mut str_builder);
                        redirect_info = Some(RedirectParseInfo::new_output());
                    }
                    continue;
                }
                '<' => {
                    // A new input redirection, we don't care about previous one.
                    add_redirect(&mut redirect, info, &mut str_builder);
                    redirect_info = Some(RedirectParseInfo::new_input());
                    continue;
                }
                _ if c.is_whitespace() => {
                    // filename_pending == false => Redirection definition finished
                    // filename_pending == true => Filename is not defined yet
                    if !info.filename_pending {
                        add_redirect(&mut redirect, info, &mut str_builder);
                        redirect_info = None;
                        continue;
                    }
                    continue;   // info.filename_pending should not be set to false here
                }
                _ => {
                    if info.append_pending {
                        info.append_pending = false;
                        str_builder.clear();
                    }
                    str_builder.push(c);
                }
            }
            info.filename_pending = false;
            continue;
        }

        fn try_parse_redirect_fd(
            str_builder: &mut String,
            redirect_info: &mut RedirectParseInfo,
            first_arg: &mut Option<String>,
            arguments: &mut Vec<String>,
        ) {
            if !str_builder.is_empty() {
                let maybe_fd = str_builder.parse::<i32>();
                match maybe_fd {
                    Ok(fd) => {
                        // can parse as fd, make it as redirect fd
                        redirect_info.fd = Some(fd);
                        str_builder.clear();
                    }
                    Err(_) => {
                        // cannot parse as fd, so it's just a normal argument
                        update_args(first_arg, arguments, str_builder);
                    }
                }
            }
        }

        match c {
            '\\' => backslash = true,
            '\'' => single_quote = true,
            '"' => double_quote = true,
            '>' => {
                let mut info = RedirectParseInfo::new_output();
                try_parse_redirect_fd(&mut str_builder, &mut info, &mut first_arg, &mut arguments);
                redirect_info = Some(info);
            }
            '<' => {
                let mut info = RedirectParseInfo::new_input();
                try_parse_redirect_fd(&mut str_builder, &mut info, &mut first_arg, &mut arguments);
                redirect_info = Some(info);
            }
            _ if c.is_whitespace() => {
                update_args(&mut first_arg, &mut arguments, &mut str_builder);
            }
            _ => str_builder.push(c),
        }
    }

    if let Some(info) = redirect_info.as_mut() {
        add_redirect(&mut redirect, info, &mut str_builder);
    }

    // Don't forget the last argument
    update_args(&mut first_arg, &mut arguments, &mut str_builder);

    Some(ParseData {
        first_arg,
        arguments,
        redirect,
    })
}

pub(crate) fn parse_command(input: &str) -> Option<ParseData> {
    parse(input)
}
