use crate::builtins::*;
use std::fs::OpenOptions;
use std::io::{self, Write};
use std::path::Path;
use std::process::Command;
pub struct PartialSuccess {
    pub success_data: String,
    pub error_info: String,
}

pub enum ErrorKind {
    PartialSuccess(PartialSuccess),
    CompleteFailure(String),
}

pub enum Output {
    AppendStdErr(Vec<String>),
    AppendStdOut(Vec<String>),
    RedirectStdOut(Vec<String>),
    RedirectStdErr(Vec<String>),
    StdOut,
}

pub fn tokenize(input: &str) -> Vec<String> {
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
pub fn process_partial_results(
    results: String,
    error_results: String,
) -> Result<String, ErrorKind> {
    match (results.is_empty(), error_results.is_empty()) {
        (false, false) => Err(ErrorKind::PartialSuccess(PartialSuccess {
            success_data: results,
            error_info: error_results,
        })),
        (true, _) => Err(ErrorKind::CompleteFailure(error_results)),
        (false, true) => Ok(results),
    }
}
pub fn execute_single_command(input: &str, stdout: &mut impl Write) -> io::Result<()> {
    let mut parts: Vec<String> = tokenize(input);
    let io_stream = if let Some(redirect_index) = parts.iter().position(|x| x == "2>>") {
        let vec2 = parts.split_off(redirect_index);
        Output::AppendStdErr(vec2)
    } else if let Some(redirect_index) = parts.iter().position(|x| x == "2>") {
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
        "echo" => Builtins.echo(args.as_slice()),
        "cat" => Builtins.cat(args.as_slice()),
        "cd" => Builtins.cd(args.next().unwrap()),
        "pwd" => Builtins.pwd(),
        "type" => Builtins._type(args.next().map(|x| x.as_str())),
        _ => {
            if let Ok(child) = std::process::Command::new(&command)
                .args(args)
                .stdout(std::process::Stdio::piped())
                .output()
            {
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
    handle_output(output, io_stream, stdout)?;
    Ok(())
}
pub fn execute_pipeline(input: &str, stdout: &mut impl Write) -> io::Result<()> {
    let commands: Vec<Vec<String>> = input
        .split('|')
        .map(|cmd| tokenize(cmd.trim()))
        .filter(|parts| !parts.is_empty())
        .collect();

    if commands.is_empty() {
        return Ok(());
    }

    // Single command - no pipe
    if commands.len() == 1 {
        return execute_single_command(input, stdout);
    }

    // Multiple commands - set up pipeline
    let mut children = vec![];
    let mut prev_stdout = None;

    for (i, parts) in commands.iter().enumerate() {
        let cmd = &parts[0];
        let args = &parts[1..];

        let mut command = std::process::Command::new(cmd);
        command.args(args);

        // Hook up stdin from previous command
        if let Some(prev) = prev_stdout.take() {
            // ← Use take() here
            command.stdin(prev);
        }

        // Set up stdout
        let is_last = i == commands.len() - 1;
        if is_last {
            // Last command: capture both stdout and stderr
            command.stdout(std::process::Stdio::piped());
            command.stderr(std::process::Stdio::piped());
        } else {
            // Middle command: pipe stdout to next
            command.stdout(std::process::Stdio::piped());
        }

        // Spawn the process
        match command.spawn() {
            Ok(mut child) => {
                // Only take stdout if NOT the last command
                if !is_last {
                    prev_stdout = child.stdout.take().map(std::process::Stdio::from);
                }
                children.push(child);
            }
            Err(_) => {
                write!(stdout, "{}: command not found\r\n", cmd)?;
                for mut child in children {
                    let _ = child.kill();
                }
                return Ok(());
            }
        }
    }

    // Wait for last process first
    if let Some(last_child) = children.pop() {
        let output = last_child.wait_with_output()?;

        // Wait for remaining processes
        for mut child in children {
            let _ = child.wait();
        }

        // Write output with \n -> \r\n conversion
        let stdout_str = String::from_utf8_lossy(&output.stdout);
        if !stdout_str.is_empty() {
            write!(stdout, "{}", stdout_str.replace('\n', "\r\n"))?;
        }

        let stderr_str = String::from_utf8_lossy(&output.stderr);
        if !stderr_str.is_empty() {
            write!(stdout, "{}", stderr_str.replace('\n', "\r\n"))?;
        }

        stdout.flush()?;
    }

    Ok(())
}

fn handle_output(
    output: Result<String, ErrorKind>,
    io_stream: Output,
    stdout: &mut impl Write,
) -> io::Result<()> {
    match io_stream {
        Output::AppendStdErr(vec2) => {
            let filename = vec2.iter().skip(1).next().unwrap();
            let path = Path::new(filename);

            match output {
                Ok(correct_output) => {
                    append_to_file(path, "").expect("Error happened");
                    println!("{}", correct_output.as_str().trim());
                }
                Err(ErrorKind::CompleteFailure(error_message)) => {
                    append_to_file(path, error_message.trim()).expect("Error happened");
                }
                Err(ErrorKind::PartialSuccess(partial_success)) => {
                    append_to_file(path, &partial_success.error_info.trim())
                        .expect("Error happened");
                    println!("{}", partial_success.success_data.trim());
                }
            }
        }
        Output::AppendStdOut(vec2) => {
            let filename = vec2.iter().skip(1).next().unwrap();
            let path = Path::new(filename);

            match output {
                Ok(correct_output) => {
                    append_to_file(path, correct_output.as_str().trim()).expect("Error happened");
                }
                Err(ErrorKind::CompleteFailure(error_message)) => {
                    if let Ok(_) = append_to_file(path, "") {}
                    println!("{}", error_message.trim());
                }
                Err(ErrorKind::PartialSuccess(partial_success)) => {
                    append_to_file(path, &partial_success.success_data).expect("Error happened");
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
                    std::fs::write(filename, partial_success.error_info + "\n").expect("failed");
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
                    std::fs::write(filename, partial_success.success_data + "\n").expect("failed");
                    println!("{}", partial_success.error_info);
                }
            }
        }
        Output::StdOut => match output {
            Ok(correct_output) => {
                if !correct_output.is_empty() {
                    let output = correct_output.replace("\n", "\r\n");
                    write!(stdout, "{}\r\n", output.trim())?;
                    stdout.flush()?;
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
    Ok(())
}

pub fn append_to_file(path: &Path, content: &str) -> std::io::Result<()> {
    let mut file = OpenOptions::new()
        .create(true) // Create the file if it doesn't exist
        .append(true) // Open in append mode
        .open(path)?;

    if !content.is_empty() {
        writeln!(file, "{}", content)?;
    }

    Ok(())
}
