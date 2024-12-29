use std::env::*;
#[allow(unused_imports)]
use std::io::{self, Write};
use std::path::Path;
use std::process;

use std::io::prelude::*;

pub fn tokenize(s: &str) -> Vec<String> {
    let mut result = Vec::new();
    let mut current = String::new();
    let mut in_quotes = false;

    let mut iter = s.chars().skip_while(|x| *x == ' ');
    while let Some(current_char) = iter.next() {
        match current_char {
            '\'' => in_quotes = !in_quotes,
            ' ' if !in_quotes => {
                if !current.is_empty() {
                    result.push(current.clone());
                    current.clear();
                }
            }
            _ => current.push(current_char),
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
                    Ok(_) => print!("{} contains:\n{}", display, s),
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
                let target = Path::new(path);

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
            let mut parts = input.trim().split_whitespace();
            let command = parts.next().unwrap();
            let args = parts;

            if let Ok(mut child) = std::process::Command::new(command).args(args).spawn() {
                let _ = child.wait();
            } else {
                println!("{command}: command not found");
            }
        }
    }
}
