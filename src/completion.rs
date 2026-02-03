use rustyline::{Helper, Highlighter, Hinter, Validator, completion::{Completer, Pair}};

use crate::{
    builtin::BUILTIN_COMMANDS,
    parse::{self, ParseFragment},
};

#[derive(Debug, Clone, Helper, Validator, Highlighter, Hinter)]
pub struct ShellCompleter {
    builtins: Vec<&'static str>,
}

impl Default for ShellCompleter {
    fn default() -> Self {
        Self::new()
    }
}

impl ShellCompleter {
    pub fn new() -> Self {
        let builtins = BUILTIN_COMMANDS.with(|map| map.keys().copied().collect());
        Self { builtins }
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
            let matches: Vec<_> = self
                .builtins
                .iter()
                .filter(|cmd| cmd.starts_with(frag))
                .map(|cmd| {
                    let mut replacement = cmd.to_string();
                    replacement.push(' ');
                    Pair {
                        display: cmd.to_string(),
                        replacement,
                    }
                })
                .collect();
            return Ok((0, matches));
        }

        Ok((pos, Vec::new()))
    }
}
