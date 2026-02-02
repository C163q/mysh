use std::{
    fs::File,
    io::{self, Read, Write},
    os::fd::{AsRawFd, IntoRawFd},
    path::PathBuf,
};

#[derive(Debug, Clone)]
pub struct OutputRedirect {
    pub append: bool,
    pub filename: PathBuf,
    pub fd: i32,
}

impl OutputRedirect {
    pub fn new(filename: PathBuf) -> Self {
        Self {
            append: false,
            filename,
            fd: 1,
        }
    }

    pub fn set_append(&mut self, append: bool) {
        self.append = append;
    }

    pub fn set_fd(&mut self, fd: i32) {
        self.fd = fd;
    }
}

#[derive(Debug, Clone)]
pub struct InputRedirect {
    pub filename: PathBuf,
    pub fd: i32,
}

impl InputRedirect {
    pub fn new(filename: PathBuf) -> Self {
        Self { filename, fd: 0 }
    }

    pub fn set_fd(&mut self, fd: i32) {
        self.fd = fd;
    }
}

/// In `bash`, if we try `echo "value" > 1 > 2`, only the last redirection takes effect.
/// But in `zsh`, both redirections take effect, and `echo` writes to both file descriptors.
#[derive(Debug, Clone)]
pub struct Redirect {
    pub input: Vec<InputRedirect>,
    pub output: Vec<OutputRedirect>,
}

impl Default for Redirect {
    fn default() -> Self {
        Self::new()
    }
}

impl Redirect {
    pub fn new() -> Self {
        Self {
            input: Vec::new(),
            output: Vec::new(),
        }
    }

    pub fn push_input(&mut self, redirect: InputRedirect) {
        for r in &mut self.input {
            if r.fd == redirect.fd {
                r.filename = redirect.filename;
                return;
            }
        }
        self.input.push(redirect);
    }

    pub fn push_output(&mut self, redirect: OutputRedirect) {
        for r in &mut self.output {
            if r.fd == redirect.fd {
                r.filename = redirect.filename;
                r.append = redirect.append;
                return;
            }
        }
        self.output.push(redirect);
    }
}

pub(crate) struct RedirectParseInfo {
    pub is_input: bool,
    pub append_pending: bool,
    pub append: bool,
    pub filename_pending: bool,
    pub fd: Option<i32>,
}

impl RedirectParseInfo {
    pub fn new_output() -> Self {
        Self {
            is_input: false,
            append_pending: true,
            append: false,
            filename_pending: true,
            fd: None,
        }
    }

    pub fn new_input() -> Self {
        Self {
            is_input: true,
            append_pending: false,
            append: false,
            filename_pending: true,
            fd: None,
        }
    }
}

#[derive(Debug, Clone, Copy)]
struct RedirectPair {
    pub before: i32,
    pub after: i32,
}

pub struct RedirectHandler {
    input: Vec<RedirectPair>,
    output: Vec<RedirectPair>,
}

impl RedirectHandler {
    unsafe fn swap_fd(before: i32, after: i32) -> Result<(), io::Error> {
        unsafe {
            let fd = libc::dup(before);
            if fd == -1 {
                return Err(io::Error::last_os_error());
            }
            if libc::dup2(after, before) == -1 {
                return Err(io::Error::last_os_error());
            }
            if libc::dup2(fd, after) == -1 {
                return Err(io::Error::last_os_error());
            }
            if libc::close(fd) == -1 {
                return Err(io::Error::last_os_error());
            }
        }
        Ok(())
    }

    pub fn new(redirect: &Redirect) -> Self {
        let mut ret = RedirectHandler {
            input: Vec::new(),
            output: Vec::new(),
        };

        // set input redirection
        for input_redirect in &redirect.input {
            if let Ok(file) = File::open(&input_redirect.filename) {
                let new_fd = file.into_raw_fd();
                let pair = RedirectPair {
                    before: input_redirect.fd,
                    after: new_fd,
                };
                ret.input.push(pair);
                unsafe {
                    Self::swap_fd(pair.before, pair.after).unwrap();
                }
            }
        }

        // set output redirection
        for output_redirect in &redirect.output {
            if let Ok(file) = if output_redirect.append {
                File::options()
                    .create(true)
                    .append(true)
                    .open(&output_redirect.filename)
            } else {
                File::create(&output_redirect.filename)
            } {
                let new_fd = file.into_raw_fd();
                let pair = RedirectPair {
                    before: output_redirect.fd,
                    after: new_fd,
                };
                ret.output.push(pair);
                unsafe {
                    Self::swap_fd(pair.before, pair.after).unwrap();
                }
            }
        }

        ret
    }
}

impl Drop for RedirectHandler {
    fn drop(&mut self) {
        unsafe fn drop_fd(before: i32, after: i32) -> Result<(), io::Error> {
            unsafe {
                if libc::dup2(after, before) == -1 {
                    return Err(io::Error::last_os_error());
                }
                libc::close(after);
            }
            Ok(())
        }
        unsafe {
            for input_pair in &self.input {
                drop_fd(input_pair.before, input_pair.after).unwrap();
            }

            for output_pair in &self.output {
                drop_fd(output_pair.before, output_pair.after).unwrap();
            }
        }
    }
}

#[allow(dead_code)]
pub struct BuiltinRedirectHandler {
    stdin: Option<File>,
    stdout: Option<File>,
    stderr: Option<File>,
}

#[allow(dead_code)]
impl BuiltinRedirectHandler {
    pub fn new(redirect: &Redirect) -> Self {
        let mut ret = BuiltinRedirectHandler {
            stdin: None,
            stdout: None,
            stderr: None,
        };
        let stdin_fd = io::stdin().as_raw_fd();
        let stdout_fd = io::stdout().as_raw_fd();
        let stderr_fd = io::stderr().as_raw_fd();

        for input_redirect in &redirect.input {
            if input_redirect.fd == stdin_fd
                && let Ok(file) = File::open(&input_redirect.filename)
            {
                ret.stdin = Some(file);
            }
        }

        for output_redirect in &redirect.output {
            if output_redirect.fd == stdout_fd
                && let Ok(file) = if output_redirect.append {
                    File::options()
                        .create(true)
                        .append(true)
                        .open(&output_redirect.filename)
                } else {
                    File::create(&output_redirect.filename)
                }
            {
                ret.stdout = Some(file);
            } else if output_redirect.fd == stderr_fd
                && let Ok(file) = if output_redirect.append {
                    File::options()
                        .create(true)
                        .append(true)
                        .open(&output_redirect.filename)
                } else {
                    File::create(&output_redirect.filename)
                }
            {
                ret.stderr = Some(file);
            }
        }

        ret
    }

    pub fn write_all_to_stderr(&mut self, buf: &[u8]) {
        if let Some(stderr) = &mut self.stderr {
            stderr.write_all(buf).unwrap();
            stderr.flush().unwrap();
        } else {
            io::stderr().write_all(buf).unwrap();
            io::stderr().flush().unwrap();
        }
    }

    pub fn write_all_to_stdout(&mut self, buf: &[u8]) {
        if let Some(stdout) = &mut self.stdout {
            stdout.write_all(buf).unwrap();
            stdout.flush().unwrap();
        } else {
            io::stdout().write_all(buf).unwrap();
            io::stdout().flush().unwrap();
        }
    }
}

impl Write for BuiltinRedirectHandler {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        if let Some(stdout) = &mut self.stdout {
            stdout.write(buf)
        } else {
            io::stdout().write(buf)
        }
    }

    fn flush(&mut self) -> io::Result<()> {
        if let Some(stdout) = &mut self.stdout {
            stdout.flush()
        } else {
            io::stdout().flush()
        }
    }
}

impl Read for BuiltinRedirectHandler {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        if let Some(stdin) = &mut self.stdin {
            stdin.read(buf)
        } else {
            io::stdin().read(buf)
        }
    }
}
