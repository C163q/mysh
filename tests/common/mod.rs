use std::{
    ffi::OsStr,
    fs::File,
    io,
    ops::{Deref, DerefMut},
};

use tempfile::NamedTempFile;

pub struct TempFile {
    file: Option<NamedTempFile>,
}

impl Deref for TempFile {
    type Target = NamedTempFile;

    fn deref(&self) -> &Self::Target {
        self.file.as_ref().unwrap()
    }
}

impl DerefMut for TempFile {
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.file.as_mut().unwrap()
    }
}

impl TempFile {
    pub fn build<S: AsRef<OsStr>>(prefix: S) -> Result<Self, io::Error> {
        let file = NamedTempFile::with_prefix(prefix)?;
        Ok(TempFile { file: Some(file) })
    }

    pub fn file(&mut self) -> &mut File {
        self.file.as_mut().unwrap().as_file_mut()
    }
}

impl Drop for TempFile {
    fn drop(&mut self) {
        let _ = self.file.take().unwrap().close();
    }
}
