use std::env::*;
#[allow(unused_imports)]
use std::io::{self, Write};
use std::path::Path;
use std::process;

use std::io::prelude::*;

fn tokenize(input: &str) -> Vec<String> {
    let mut result = Vec::new();
    let mut current = String::new();
    let mut quote_char = None;
    let mut chars = input.chars().peekable();

    while let Some(c) = chars.next() {
        match (c, quote_char) {
            // Backslash outside quotes
            ('\\', None) => {
                if let Some(next_char) = chars.next() {
                    current.push(next_char);
                }
            }
            // Backslash inside double quotes
            ('\\', Some('"')) => {
                if let Some(next_char) = chars.next() {
                    match next_char {
                        '\\' | '$' | '"' | '\n' => current.push(next_char),
                        _ => {
                            current.push('\\');
                            current.push(next_char);
                        }
                    }
                }
            }
            // Quote handling
            ('\'' | '"', None) => quote_char = Some(c),
            ('"', Some('"')) | ('\'', Some('\'')) => quote_char = None,
            // Space handling
            (' ', None) => {
                if !current.is_empty() {
                    result.push(current.clone());
                    current.clear();
                }
            }
            // All other characters
            (c, _) => current.push(c),
        }
    }

    if !current.is_empty() {
        result.push(current);
    }

    result
}

enum Output {
    RedirectStdOut(Vec<String>),
    StdOut,
}

fn main() {
    let path_env = std::env::var("PATH").unwrap();
    let home_env = std::env::var("HOME").unwrap();
    loop {
        print!("$ ");
        io::stdout().flush().unwrap();
        let stdin = io::stdin();
        let mut input = String::new();
        stdin.read_line(&mut input).unwrap();

        let mut parts: Vec<String> = tokenize(input.trim());
        let io_stream =
            if let Some(redirect_index) = parts.iter().position(|x| x == ">" || x == "1>") {
                let vec2 = parts.split_off(redirect_index);
                Output::RedirectStdOut(vec2)
            } else {
                Output::StdOut
            };

        let mut args = parts.iter_mut();
        let command = args.next().unwrap();

        let output: String = match command.as_str() {
            "echo" => {
                let mut result = String::new();
                let mut first = true;

                for s in args {
                    if !first {
                        result.push(' ');
                    }
                    result.push_str(s);
                    first = false;
                }

                result
            }
            "cat" => {
                use std::fs::File;
                use std::path::Path;

                let mut result = vec![];
                for file_name in args {
                    let path = Path::new(&file_name);
                    let display = path.display();

                    if let Ok(mut file) = File::open(&path) {
                        let mut s = String::new();

                        match file.read_to_string(&mut s) {
                            Err(why) => panic!("couldn't read {}: {}", display, why),
                            Ok(_) => result.push(s),
                        }
                    } else {
                        println!("cat: {}: No such file or directory", file_name);
                    }
                }
                format!("{}", result.concat())
            }
            "cd" => {
                let next = args.next();
                let path = next.unwrap();

                if path == "" || path == " " || path == " ~" {
                    // Go home on empty path
                    let target = Path::new(&home_env);
                    if let Err(_) = std::env::set_current_dir(&target) {
                        println!("cd: {}: No such file or directory", target.display());
                    }
                } else if &path[0..1] == "/" {
                    // Handle absolute paths
                    let target = Path::new(&path);

                    if let Err(_) = std::env::set_current_dir(&target) {
                        println!("cd: {}: No such file or directory", target.display());
                    }
                } else {
                    let get_current_directory = std::env::current_dir().expect("Invalid Directory");
                    let current_directory = get_current_directory
                        .to_str()
                        .expect("Error converting to string");
                    let target_directory = current_directory
                        .chars()
                        .chain(std::iter::once('/'))
                        .chain(path.chars())
                        .collect::<String>();
                    let mut destination = vec![];
                    for directory in target_directory.split("/") {
                        match directory {
                            "." => {}
                            ".." => {
                                destination.pop().unwrap();
                            }
                            _ => {
                                destination.push(directory);
                            }
                        }
                    }
                    let final_destination = destination.join("/");
                    let target = Path::new(&final_destination);

                    if let Err(_) = std::env::set_current_dir(&target) {
                        println!("cd: {}: No such file or directory", target.display());
                    }
                };

                // on successful `cd` do not print anything
                String::new()
            }
            "pwd" => {
                // println!(
                format!(
                    "{}",
                    std::env::current_dir()
                        .expect("Invalid Directory")
                        .display()
                )
            }
            "exit" => {
                if let Some(code) = args.next() {
                    process::exit(code.parse::<i32>().expect("Not a number"));
                }
                String::new()
            }
            "type" => {
                let mut paths = path_env.split(":");
                if let Some(argument) = args.next() {
                    if argument == "cd"
                        || argument == "echo"
                        || argument == "exit"
                        || argument == "type"
                        || argument == "pwd"
                        || argument == "cat"
                    {
                        format!("{argument} is a shell builtin")
                    } else if let Some(found) =
                        paths.find(|path| std::fs::metadata(format!("{path}/{argument}")).is_ok())
                    {
                        format!("{argument} is {found}/{argument}")
                    } else {
                        format!("{argument}: not found")
                    }
                } else {
                    // TODO
                    String::new()
                }
            }
            _ => {
                let command_output = if let Ok(child) = std::process::Command::new(&command)
                    .args(args)
                    .stdout(std::process::Stdio::piped())
                    .spawn()
                {
                    if let Ok(output) = child.wait_with_output() {
                        String::from_utf8_lossy(&output.stdout).to_string()
                    } else {
                        String::from("Failed to get output")
                    }
                } else {
                    format!("{}: command not found", &command)
                };
                command_output.trim().to_string()
            }
        };

        match io_stream {
            Output::RedirectStdOut(vec2) => {
                let filename = vec2.iter().skip(1).next().unwrap();

                std::fs::write(filename, output).expect("failed")
            }
            Output::StdOut => {
                if !output.is_empty() {
                    println!("{}", output.trim());
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn quoted_strings() {
        // Test with double quotes
        let s1 = "\"quz  hello\"  \"bar\"";
        assert_eq!(tokenize(s1), vec!["quz  hello", "bar"]);

        // Test with single quotes
        let s2 = "'quz  hello'  'bar'";
        assert_eq!(tokenize(s2), vec!["quz  hello", "bar"]);

        // Test with mixed quotes
        let s3 = "'hello world'  \"foo bar\"  'baz'";
        assert_eq!(tokenize(s3), vec!["hello world", "foo bar", "baz"]);
        let s4 = "before\\   after";
        assert_eq!(tokenize(s4), vec!["before\\   after"]);
        let s5 = "world\\ \\ \\ \\ \\ \\ script";
        assert_eq!(tokenize(s5), vec!["world      script"]);
    }
}
