use std::{cell::RefCell, path::PathBuf, process, rc::Rc};

use crate::{
    env::ExecEnv,
    redirect::{
        InputRedirect, OutputRedirect, Redirect, RedirectHandler, RedirectParseFragment,
        RedirectParseInfo,
    },
    result::ExecResult,
};

pub fn execute_command(
    cmd: String,
    args: Vec<String>,
    env: Rc<RefCell<ExecEnv>>,
    redirect: Redirect,
) -> ExecResult {
    if cmd == "exit" {
        return ExecResult::Exit;
    }

    let f = crate::builtin::BUILTIN_COMMANDS.with(|map| map.get(cmd.as_str()).copied());

    if let Some(func) = f {
        // RedirectHandler scope
        let _handler = RedirectHandler::new(&redirect);
        func(args, env.borrow_mut());
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
    } // RedirectHandler dropped here, restore fds

    eprintln!("{}: command not found", &cmd);
    ExecResult::Normal
}

pub(crate) struct ParseData {
    pub first_arg: Option<String>,
    pub arguments: Vec<String>,
    pub redirect: Redirect,
}

#[derive(Debug)]
pub enum ParseFragment {
    Argument(String),
    Redirect(RedirectParseFragment),
}

// use `Result<ParseData, Error>` later
fn parse_to_data(fragments: Vec<ParseFragment>) -> ParseData {
    let mut first_arg: Option<String> = None;
    let mut arguments: Vec<String> = Vec::new();
    let mut redirect = Redirect::new();
    let mut redirect_pending: Option<RedirectParseFragment> = None;

    fn update_args(first_arg: &mut Option<String>, arguments: &mut Vec<String>, arg: String) {
        if first_arg.is_none() {
            first_arg.replace(arg);
        } else {
            arguments.push(arg);
        }
    }

    fn add_redirect(redirect: &mut Redirect, rfrag: RedirectParseFragment, next_frag: String) {
        if rfrag.is_input {
            let mut input_redirect = InputRedirect::new(PathBuf::from(next_frag));
            input_redirect.set_fd(rfrag.fd);
            redirect.push_input(input_redirect);
        } else {
            let mut output_redirect = OutputRedirect::new(PathBuf::from(next_frag));
            output_redirect.set_append(rfrag.append);
            output_redirect.set_fd(rfrag.fd);
            redirect.push_output(output_redirect);
        }
    }

    for frag in fragments {
        match frag {
            ParseFragment::Argument(arg) => {
                match redirect_pending.take() {
                    // normal argument
                    None => {
                        update_args(&mut first_arg, &mut arguments, arg);
                    }
                    // filename for redirect
                    Some(rfrag) => {
                        // Sepcial condition when parsing "> file echo value"
                        // For zsh, this will redirect stdout to file, then execute "echo value"
                        add_redirect(&mut redirect, rfrag, arg);
                    }
                }
            }
            ParseFragment::Redirect(rfrag) => {
                // If redirect_pending.is_some(), then the previous redirect has no filename
                // This is a syntax error in real shell, but we just ignore it here.
                redirect_pending.replace(rfrag);
            }
        }
    }

    ParseData {
        first_arg,
        arguments,
        redirect,
    }
}

/// TODO: handle multi-line input
pub(crate) fn parse_to_fragments(input: &str) -> Vec<ParseFragment> {
    let mut fragments: Vec<ParseFragment> = Vec::new();
    // To build the current fragment
    let mut str_builder = String::new();
    // To handle single quotes
    let mut single_quote = false;
    // To handle double quotes
    let mut double_quote = false;
    // To handle backslashes
    // TODO: handle backslashes with newline
    let mut backslash = false;
    // To handle redirections
    let mut redirect_info: Option<RedirectParseInfo> = None;

    fn update_args(fragments: &mut Vec<ParseFragment>, str_builder: &mut String) {
        if str_builder.is_empty() {
            return;
        }
        fragments.push(ParseFragment::Argument(str_builder.clone()));
        str_builder.clear();
    }

    fn add_redirect(
        fragments: &mut Vec<ParseFragment>,
        info: &RedirectParseInfo,
        str_builder: &mut String,
    ) {
        let frag = RedirectParseFragment::build(info, str_builder.clone());
        str_builder.clear();
        fragments.push(ParseFragment::Redirect(frag));
    }

    for c in input.chars() {
        // Handle single quotes
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
            // "\n" is still newline, but we don't handle it now.
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
        //  ------
        //
        // >1.txt value
        //  -----
        // ```
        if let Some(info) = redirect_info.as_mut() {
            match c {
                '\\' => backslash = true,
                '\'' => single_quote = true,
                '\"' => double_quote = true,
                '>' => {
                    if info.is_input {
                        // Previous is input redirection
                        // This only occurs when the input is "<>"
                        // We parse it as two separate redirections
                        add_redirect(&mut fragments, info, &mut str_builder);
                        redirect_info = Some(RedirectParseInfo::new_output());
                        // We don't need to parse fd again, because str_builder is cleared
                    } else if info.append_pending {
                        // probably `>>`
                        info.append = true;
                        info.append_pending = false;
                    } else {
                        // This can be merged with the first branch, but for clarity, we
                        // separate them.
                        //
                        // !info.append_pending => a new output redirection
                        // This only occurs when the input is ">>>"
                        add_redirect(&mut fragments, info, &mut str_builder);
                        redirect_info = Some(RedirectParseInfo::new_output());
                    }
                    str_builder.push(c); // for RedirectParseFragment.value
                    continue;
                }
                '<' => {
                    // A new input redirection, we don't care about previous one.
                    add_redirect(&mut fragments, info, &mut str_builder);
                    redirect_info = Some(RedirectParseInfo::new_input());
                    str_builder.push(c); // for RedirectParseFragment.value
                    continue;
                }
                _ if c.is_whitespace() => {
                    add_redirect(&mut fragments, info, &mut str_builder);
                    redirect_info = None;
                    continue;
                }
                _ => {
                    add_redirect(&mut fragments, info, &mut str_builder);
                    redirect_info = None;
                    str_builder.push(c);
                    continue;
                }
            }
            continue;
        }

        fn try_parse_redirect_fd(
            fragments: &mut Vec<ParseFragment>,
            str_builder: &mut String,
            redirect_info: &mut RedirectParseInfo,
        ) {
            if !str_builder.is_empty() {
                let maybe_fd = str_builder.parse::<i32>();
                if let Ok(fd) = maybe_fd {
                    // can parse as fd, make it as redirect fd
                    redirect_info.fd = Some(fd);
                } else {
                    // probably "value>"
                    // In this case, we treat it as normal argument
                    let redirect_symbol = str_builder.pop().unwrap(); // should be '>' or '<'
                    update_args(fragments, str_builder);
                    str_builder.push(redirect_symbol);
                }
                // We don't clear str_builder here, because we may need it later
            }
        }

        match c {
            '\\' => backslash = true,
            '\'' => single_quote = true,
            '"' => double_quote = true,
            '>' => {
                let mut info = RedirectParseInfo::new_output();
                try_parse_redirect_fd(&mut fragments, &mut str_builder, &mut info);
                redirect_info = Some(info);
                str_builder.push(c); // for RedirectParseFragment.value
            }
            '<' => {
                let mut info = RedirectParseInfo::new_input();
                try_parse_redirect_fd(&mut fragments, &mut str_builder, &mut info);
                redirect_info = Some(info);
                str_builder.push(c); // for RedirectParseFragment.value
            }
            _ if c.is_whitespace() => {
                update_args(&mut fragments, &mut str_builder);
            }
            _ => str_builder.push(c),
        }
    }

    // Don't forget the last fragment
    if let Some(info) = redirect_info.as_mut() {
        add_redirect(&mut fragments, info, &mut str_builder);
    }

    update_args(&mut fragments, &mut str_builder);

    fragments
}

pub(crate) fn parse_command(input: &str) -> Option<ParseData> {
    let fragments = parse_to_fragments(input);
    Some(parse_to_data(fragments))
}
