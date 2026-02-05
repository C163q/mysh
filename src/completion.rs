use std::{
    cell::RefCell,
    fs::{self, DirEntry, ReadDir},
    rc::Rc,
};

use is_executable::IsExecutable;
use rustyline::{
    Helper, Highlighter, Hinter, Validator,
    completion::{Completer, Pair},
};

use crate::{
    builtin::BUILTIN_COMMANDS,
    env::ExecEnv,
    parse::{self, ParseFragment},
};

#[derive(Debug, Clone, Helper, Validator, Highlighter, Hinter)]
pub struct ShellCompleter {
    builtins: Vec<&'static str>,
    env: Rc<RefCell<ExecEnv>>,
}

impl ShellCompleter {
    pub fn new(env: Rc<RefCell<ExecEnv>>) -> Self {
        let builtins = BUILTIN_COMMANDS.with(|map| map.keys().copied().collect());
        Self { builtins, env }
    }

    fn candidate_executable_in_path(prefix: &str, env: &ExecEnv) -> impl Iterator<Item = DirEntry> {
        fn dir_candidate_executable(
            prefix: &str,
            reader: ReadDir,
        ) -> impl Iterator<Item = DirEntry> {
            reader.flatten().filter(move |entry| {
                entry.path().is_executable()
                    && entry.file_name().to_string_lossy().starts_with(prefix)
            })
        }

        env.path_env
            .iter()
            .filter_map(move |dir| {
                if let Ok(entries) = fs::read_dir(dir) {
                    Some(dir_candidate_executable(prefix, entries))
                } else {
                    None
                }
            })
            .flatten()
    }
}

impl Completer for ShellCompleter {
    type Candidate = Pair;

    fn complete(
        &self,
        line: &str,
        pos: usize,
        _ctx: &rustyline::Context<'_>,
    ) -> rustyline::Result<(usize, Vec<Self::Candidate>)> {
        let fragments = parse::parse_to_fragments(line);

        if fragments.is_empty() {
            return Ok((0, Vec::new()));
        }

        let fragment_index = fragments.len() - 1;
        let last_fragment = &fragments[fragment_index];

        // Inplement basic completion for the first fragment only
        if fragment_index == 0
            && let ParseFragment::Argument(frag) = last_fragment
        {
            let env = self.env.borrow();
            let iter = Self::candidate_executable_in_path(frag, &env);
            let mut matches: Vec<_> = self
                .builtins
                .iter()
                .filter(|cmd| cmd.starts_with(frag))
                .map(|r| r.to_string())
                .chain(iter.map(|entry| entry.file_name().to_string_lossy().to_string()))
                .map(|cmd| {
                    let mut replacement = cmd.clone();
                    replacement.push(' ');
                    Pair {
                        display: cmd,
                        replacement,
                    }
                })
                .collect();
            matches.sort_unstable_by(|a, b| a.display.cmp(&b.display));
            matches.dedup_by(|a, b| a.display == b.display);
            return Ok((0, matches));
        }

        Ok((pos, Vec::new()))
    }
}
