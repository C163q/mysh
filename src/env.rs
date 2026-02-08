use std::{
    io::{PipeReader, PipeWriter},
    ops::{Deref, DerefMut},
    path::PathBuf,
};

use directories::BaseDirs;
use rustyline::history::FileHistory;

#[derive(Debug, Clone)]
pub struct PathEnv {
    pub paths: Vec<PathBuf>,
}

impl PathEnv {
    pub fn new() -> Self {
        Self { paths: Vec::new() }
    }

    pub fn from_paths(paths: Vec<PathBuf>) -> Self {
        Self { paths }
    }
}

impl Default for PathEnv {
    fn default() -> Self {
        Self::new()
    }
}

impl Deref for PathEnv {
    type Target = Vec<PathBuf>;

    fn deref(&self) -> &Self::Target {
        &self.paths
    }
}

impl DerefMut for PathEnv {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.paths
    }
}

#[derive(Debug)]
pub struct ExecEnv {
    pub path_env: PathEnv,
    pub histfile_env: Option<PathBuf>,
    pub base_dirs: BaseDirs,
    pub pipe_in: Option<PipeReader>,
    pub pipe_out: Option<PipeWriter>,
}

impl ExecEnv {
    pub fn new(base_dirs: BaseDirs) -> Self {
        Self {
            path_env: PathEnv::new(),
            histfile_env: None,
            base_dirs,
            pipe_in: None,
            pipe_out: None,
        }
    }

    pub fn build(path_env: PathEnv, histfile_env: Option<PathBuf>, base_dirs: BaseDirs) -> Self {
        Self {
            path_env,
            histfile_env,
            base_dirs,
            pipe_in: None,
            pipe_out: None,
        }
    }

    pub fn reset_pipes(&mut self) {
        self.pipe_in = None;
        self.pipe_out = None;
    }
}

pub struct ExecContext<'a> {
    pub history: &'a mut FileHistory,
}

impl<'a> ExecContext<'a> {
    pub fn new(history: &'a mut FileHistory) -> Self {
        Self { history }
    }
}
