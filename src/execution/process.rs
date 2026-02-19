use std::{
    fs::{File, OpenOptions},
    io::{self, Error},
    os::{
        fd::{AsRawFd, IntoRawFd, OwnedFd},
        unix::process::CommandExt,
    },
    process::{Child, Command},
};

use crate::execution::data::RawCommand;

pub struct ChildBuilder {
    commnad: RawCommand,
    stdout: Option<OwnedFd>,
    stdin: Option<OwnedFd>,
}

impl ChildBuilder {
    pub fn new(command: RawCommand) -> Self {
        Self {
            commnad: command,
            stdout: None,
            stdin: None,
        }
    }

    pub fn stdout<T: Into<OwnedFd>>(&mut self, fd: T) {
        self.stdout = Some(fd.into());
    }

    pub fn stdin<T: Into<OwnedFd>>(&mut self, fd: T) {
        self.stdin = Some(fd.into());
    }

    pub fn build(self) -> io::Result<Child> {
        let mut cmd = Command::new(&self.commnad.cmd);
        cmd.args(&self.commnad.arguments);
        unsafe {
            cmd.pre_exec(move || {
                for input in &self.commnad.redirect.input {
                    let f = File::open(&input.filename)?;
                    if f.as_raw_fd() == input.fd {
                        // stop closing the file when f goes out of scope
                        let _ = f.into_raw_fd();
                    } else {
                        let fd = f.as_raw_fd();
                        if libc::dup2(fd, input.fd) == -1 {
                            return Err(Error::last_os_error());
                        }
                    } // close f when it goes out of scope
                }
                for output in &self.commnad.redirect.output {
                    let f = if output.append {
                        OpenOptions::new()
                            .create(true)
                            .append(true)
                            .open(&output.filename)?
                    } else {
                        File::create(&output.filename)?
                    };
                    if f.as_raw_fd() == output.fd {
                        // stop closing the file when f goes out of scope
                        let _ = f.into_raw_fd();
                    } else {
                        let fd = f.as_raw_fd();
                        if libc::dup2(fd, output.fd) == -1 {
                            return Err(Error::last_os_error());
                        }
                    } // close f when it goes out of scope
                }
                Ok(())
            });
        }
        if let Some(stdout) = self.stdout {
            cmd.stdout(stdout);
        }
        if let Some(stdin) = self.stdin {
            cmd.stdin(stdin);
        }

        let child = cmd.spawn()?;
        Ok(child)
    }
}
