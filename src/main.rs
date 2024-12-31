use std::fs::OpenOptions;
#[allow(unused_imports)]
use std::io::{self, Write};
use std::path::Path;
use std::process;

use std::io::prelude::*;

struct PartialSuccess {
    success_data: String,
    error_info: String,
}

enum ErrorKind {
    PartialSuccess(PartialSuccess),
    CompleteFailure(String),
}

enum Output {
    AppendStdOut(Vec<String>),
    RedirectStdOut(Vec<String>),
    RedirectStdErr(Vec<String>),
    StdOut,
}

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

fn process_partial_results(results: String, error_results: String) -> Result<String, ErrorKind> {
    match (results.is_empty(), error_results.is_empty()) {
        (false, false) => Err(ErrorKind::PartialSuccess(PartialSuccess {
            success_data: results,
            error_info: error_results,
        })),
        (true, _) => Err(ErrorKind::CompleteFailure(error_results)),
        (false, true) => Ok(results),
    }
}

fn append_to_file(path: &Path, content: &str) -> std::io::Result<()> {
    let mut file = OpenOptions::new()
        .create(true) // Create the file if it doesn't exist
        .append(true) // Open in append mode
        .open(path)?;

    writeln!(file, "{}", content)?;
    Ok(())
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

        let io_stream = if let Some(redirect_index) = parts.iter().position(|x| x == "2>") {
            let vec2 = parts.split_off(redirect_index);
            Output::RedirectStdErr(vec2)
        } else if let Some(redirect_index) = parts.iter().position(|x| x == ">>" || x == "1>>") {
            let vec2 = parts.split_off(redirect_index);
            Output::AppendStdOut(vec2)
        } else if let Some(redirect_index) = parts.iter().position(|x| x == ">" || x == "1>") {
            let vec2 = parts.split_off(redirect_index);
            Output::RedirectStdOut(vec2)
        } else {
            Output::StdOut
        };

        let mut args = parts.iter_mut();
        let command = args.next().unwrap();

        let output: Result<String, ErrorKind> = match command.as_str() {
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

                Ok(result)
            }
            "cat" => {
                use std::fs::File;
                use std::path::Path;

                let mut result = vec![];
                let mut error_result = vec![];
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
                        error_result.push(format!("cat: {}: No such file or directory", file_name));
                    }
                }
                process_partial_results(result.concat(), error_result.concat())
            }
            "cd" => {
                let next = args.next();
                let path = next.unwrap();

                if path == "" || path == " " || path == "~" {
                    // Go home on empty path
                    let target = Path::new(&home_env);
                    match std::env::set_current_dir(&target) {
                        Err(_) => Err(ErrorKind::CompleteFailure(format!(
                            "cd: {}: No such file or directory",
                            target.display()
                        ))),
                        Ok(_) => Ok(String::new()),
                    }
                } else if &path[0..1] == "/" {
                    // Handle absolute paths
                    let target = Path::new(&path);

                    match std::env::set_current_dir(&target) {
                        Err(_) => Err(ErrorKind::CompleteFailure(format!(
                            "cd: {}: No such file or directory",
                            target.display()
                        ))),
                        _ => Ok(String::new()),
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

                    match std::env::set_current_dir(&target) {
                        Err(_) => Err(ErrorKind::CompleteFailure(format!(
                            "cd: {}: No such file or directory",
                            target.display()
                        ))),
                        _ => Ok(String::new()),
                    }
                }
            }
            "pwd" => {
                // println!(
                Ok(format!(
                    "{}",
                    std::env::current_dir()
                        .expect("Invalid Directory")
                        .display()
                ))
            }
            "exit" => {
                if let Some(code) = args.next() {
                    process::exit(code.parse::<i32>().expect("Not a number"));
                }
                Ok(String::new())
            }
            "type" => {
                let mut paths = path_env.split(":");
                if let Some(argument) = args.next() {
                    if argument == "cd"
                        || argument == "echo"
                        || argument == "exit"
                        || argument == "type"
                        || argument == "pwd"
                    // || argument == "cat"
                    {
                        Ok(format!("{argument} is a shell builtin"))
                    } else if let Some(found) =
                        paths.find(|path| std::fs::metadata(format!("{path}/{argument}")).is_ok())
                    {
                        Ok(format!("{argument} is {found}/{argument}"))
                    } else {
                        Err(ErrorKind::CompleteFailure(format!("{argument}: not found")))
                    }
                } else {
                    // TODO
                    Ok(String::new())
                }
            }
            _ => {
                if let Ok(child) = std::process::Command::new(&command)
                    .args(args)
                    .stdout(std::process::Stdio::piped())
                    .output()
                {
                    // if let Ok(output) = child.wait_with_output() {
                    //     Ok(String::from_utf8_lossy(&output.stdout).to_string())
                    // } else {
                    //     Err(ErrorKind::CompleteFailure(format!("Failed to get output")))
                    // }
                    let stdout = String::from_utf8_lossy(&child.stdout).to_string();
                    let stderr = String::from_utf8_lossy(&child.stderr).to_string();
                    process_partial_results(stdout, stderr)
                } else {
                    Err(ErrorKind::CompleteFailure(format!(
                        "{}: command not found",
                        &command
                    )))
                }
            }
        };

        match io_stream {
            Output::AppendStdOut(vec2) => {
                let filename = vec2.iter().skip(1).next().unwrap();
                let path = Path::new(filename);

                match output {
                    Ok(correct_output) => {
                        append_to_file(path, correct_output.as_str().trim())
                            .expect("Error happened");
                    }
                    Err(ErrorKind::CompleteFailure(error_message)) => {
                        append_to_file(path, "").expect("Error happened");
                        println!("{}", error_message.trim());
                    }
                    Err(ErrorKind::PartialSuccess(partial_success)) => {
                        append_to_file(path, &partial_success.success_data)
                            .expect("Error happened");
                        println!("{}", partial_success.error_info.trim());
                    }
                }
            }
            Output::RedirectStdErr(vec2) => {
                let filename = vec2.iter().skip(1).next().unwrap();

                match output {
                    Ok(correct_output) => {
                        if !correct_output.is_empty() {
                            println!("{}", correct_output.trim());
                        }
                        std::fs::write(filename, "").expect("failed");
                    }
                    Err(ErrorKind::CompleteFailure(error_message)) => {
                        std::fs::write(filename, error_message + "\n").expect("failed");
                    }
                    Err(ErrorKind::PartialSuccess(partial_success)) => {
                        std::fs::write(filename, partial_success.error_info + "\n")
                            .expect("failed");
                        println!("{}", partial_success.success_data.trim());
                    }
                }
            }
            Output::RedirectStdOut(vec2) => {
                let filename = vec2.iter().skip(1).next().unwrap();

                match output {
                    Ok(correct_output) => {
                        std::fs::write(filename, correct_output + "\n").expect("failed")
                    }
                    Err(ErrorKind::CompleteFailure(error_message)) => {
                        println!("{}", error_message.trim())
                    }
                    Err(ErrorKind::PartialSuccess(partial_success)) => {
                        std::fs::write(filename, partial_success.success_data + "\n")
                            .expect("failed");
                        println!("{}", partial_success.error_info);
                    }
                }
            }
            Output::StdOut => match output {
                Ok(correct_output) => {
                    if !correct_output.is_empty() {
                        println!("{}", correct_output.trim());
                    }
                }
                Err(ErrorKind::CompleteFailure(error_message)) => {
                    eprintln!("{}", error_message.trim())
                }
                Err(ErrorKind::PartialSuccess(partial_success)) => {
                    println!("{}", partial_success.success_data.trim());
                    eprintln!("{}", partial_success.error_info.trim());
                }
            },
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
