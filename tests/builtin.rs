use std::{
    fs::File,
    io::{self, Read, Seek, SeekFrom, Write},
};

use mysh::{env::ExecEnv, get_input_and_run};

use crate::common::{TempFile};

mod common;

#[allow(dead_code)]
fn read_from_temp(file: &mut File) -> String {
    let mut vec = Vec::new();
    file.read_to_end(&mut vec).unwrap();
    String::from_utf8(vec).unwrap()
}

fn read_from_temp_u8(file: &mut File) -> Vec<u8> {
    let mut vec = Vec::new();
    file.read_to_end(&mut vec).unwrap();
    vec
}

#[allow(dead_code)]
fn get_print_with_handler(file: &mut File) -> String {
    let output = read_from_temp(file);
    file.seek(SeekFrom::Start(0)).unwrap();
    file.set_len(0).unwrap();
    output
}

fn get_print_with_handler_u8(file: &mut File) -> Vec<u8> {
    let output = read_from_temp_u8(file);
    file.seek(SeekFrom::Start(0)).unwrap();
    file.set_len(0).unwrap();
    output
}

macro_rules! execute {
    ($path:expr, $env:expr, $str:literal) => {
        get_input_and_run(&format!($str, $path.display()), &mut $env);
    };
}

#[test]
fn cd_absolute() {
    let _lock = io::stdout().lock();
    let mut temp_file = TempFile::build("mysh-tests-cd_absolute").unwrap();
    let path = temp_file.path();
    let mut env = ExecEnv::new();

    get_input_and_run("cd /", &mut env);
    execute!(path, env, "pwd > {}");

    let output_path = get_print_with_handler_u8(temp_file.file());
    let result = b"/\n";

    assert_eq!(output_path, result);
}

#[test]
fn echo() {
    let _lock = io::stdout().lock();
    let mut temp_file = TempFile::build("mysh-tests-echo").unwrap();
    temp_file.as_file_mut().lock().unwrap();
    let path = temp_file.path().to_path_buf();
    let mut env = ExecEnv::new();

    execute!(path, env, "echo a1b2c3d   4e5f6g >> {}");             // a1b2c3d 4e5f6g
    execute!(path, env, "echo \"abc  def \"  >> {}");               // abc  def 
    execute!(path, env, "echo 'hello    world' >> {}");             // hello    world
    execute!(path, env, "echo hello''wo'rl'd >> {}");               // helloworld
    execute!(path, env, "echo \"shell's test\" >> {}");             // shell's test
    execute!(path, env, "echo \"quz  hello\"  \"bar\" >> {}");      // quz  hello bar
    execute!(path, env, r"echo three\ \ \ spaces >> {}");           // three   spaces
    execute!(path, env, r"echo before\     after >> {}");           // before  after
    execute!(path, env, r"echo hello\\world >> {}");                // hello\world
    execute!(path, env, r"echo \'hello\' >> {}");                   // 'hello'
    execute!(path, env, r#"echo \'\"literal quotes\"\' >> {}"#);    // '"literal quotes"'
    execute!(path, env, r"echo ignore\_backslash >> {}");           // ignore_backslash
    execute!(path, env, r#"echo 'example\"test' >> {}"#);           // example\"test
    execute!(path, env, r"echo 'multiple\\slashes' >> {}");         // multiple\\slashes
    execute!(path, env, r#"echo "\\ \" \' \_" >> {}"#);             // \ " \' \_
    execute!(path, env, r#"e''ch"o" hello  world   >>  {}"#);       // hello world

    temp_file.as_file_mut().flush().unwrap();

    let output = get_print_with_handler(temp_file.file());
    let result =
r#"a1b2c3d 4e5f6g
abc  def 
hello    world
helloworld
shell's test
quz  hello bar
three   spaces
before  after
hello\world
'hello'
'"literal quotes"'
ignore_backslash
example\"test
multiple\\slashes
\ " \' \_
hello world
"#;

    assert_eq!(output, result);
}
