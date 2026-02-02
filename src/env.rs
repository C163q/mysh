use std::{
    ops::{Deref, DerefMut},
    path::PathBuf,
};

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

#[derive(Debug, Clone)]
pub struct ExecEnv {
    pub path_env: PathEnv,
}

impl Default for ExecEnv {
    fn default() -> Self {
        Self::new()
    }
}

impl ExecEnv {
    pub fn new() -> Self {
        Self {
            path_env: PathEnv::new(),
        }
    }

    pub fn build(path_env: PathEnv) -> Self {
        Self { path_env }
    }
}
