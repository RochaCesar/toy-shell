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

fn main() {
    let path_env = std::env::var("PATH").unwrap();
    let home_env = std::env::var("HOME").unwrap();
    loop {
        print!("$ ");
        io::stdout().flush().unwrap();
        let stdin = io::stdin();
        let mut input = String::new();
        stdin.read_line(&mut input).unwrap();
        let trimmed = input.as_str().trim();

        if let Some(rest) = trimmed.strip_prefix("echo") {
            let result = tokenize(rest);

            println!("{}", result.join(" "));
        } else if let Some(files) = trimmed.strip_prefix("cat") {
            use std::fs::File;
            use std::path::Path;

            let result = tokenize(&files);

            for file_name in result {
                let path = Path::new(&file_name);
                let display = path.display();

                let mut file = match File::open(&path) {
                    Err(why) => panic!("couldn't open {}: {}", display, why),
                    Ok(file) => file,
                };
                // Read the file contents into a string, returns `io::Result<usize>`
                let mut s = String::new();
                match file.read_to_string(&mut s) {
                    Err(why) => panic!("couldn't read {}: {}", display, why),
                    Ok(_) => print!("{}", s),
                }
            }
        } else if let Some(path) = trimmed.strip_prefix("cd") {
            if path == "" || path == " " || path == " ~" {
                // Go home on empty path
                let target = Path::new(&home_env);
                if let Err(_) = std::env::set_current_dir(&target) {
                    println!("cd: {}: No such file or directory", target.display());
                }
            } else if path.chars().nth(1).unwrap() == '/' {
                // Handle absolute paths
                let absolute_path = path.chars().skip(1).collect::<String>();
                let target = Path::new(&absolute_path);

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
                    .chain(path.chars().skip(1))
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
        } else if let Some(_) = trimmed.strip_prefix("pwd") {
            println!(
                "{}",
                std::env::current_dir()
                    .expect("Invalid Directory")
                    .display()
            );
        } else if let Some(code) = trimmed.strip_prefix("exit ") {
            process::exit(code.parse::<i32>().expect("Not a number"));
        } else if let Some(command) = trimmed.strip_prefix("type ") {
            let mut paths = path_env.split(":");
            if command == "cd"
                || command == "echo"
                || command == "exit"
                || command == "type"
                || command == "pwd"
            {
                println!("{command} is a shell builtin")
            } else if let Some(found) =
                paths.find(|path| std::fs::metadata(format!("{path}/{command}")).is_ok())
            {
                println!("{command} is {found}/{command}")
            } else {
                println!("{command}: not found")
            }
        } else {
            let parts = tokenize(input.trim());
            // let mut parts = input.trim().split_whitespace();
            let command = parts.first().unwrap().clone();
            let args = parts.iter().skip(1).collect::<Vec<&String>>();

            if let Ok(mut child) = std::process::Command::new(&command).args(args).spawn() {
                let _ = child.wait();
            } else {
                println!("{}: command not found", &command);
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
