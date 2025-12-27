use crate::builtins::*;
use std::fs::OpenOptions;
use std::io::{self};
use std::path::Path;
use std::thread;
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
use std::fs::File;
// The output is wrapped in a Result to allow matching on errors.
// Returns an Iterator to the Reader of the lines of the file.
pub fn read_lines<P>(filename: P) -> io::Result<io::Lines<io::BufReader<File>>>
where
    P: AsRef<Path>,
{
    let file = File::open(filename)?;
    Ok(io::BufReader::new(file).lines())
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
use std::io::Read;

use std::io::{BufRead, BufReader, Write};
use std::process::{Command, Stdio};
use std::time::Duration;

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
                    append_to_file(path, correct_output.as_str().trim())?;
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
                Ok(correct_output) => std::fs::write(filename, correct_output).expect("failed"),
                Err(ErrorKind::CompleteFailure(error_message)) => {
                    write!(stdout, "{}", error_message.trim().replace("\n", "\r\n"))?;
                }
                Err(ErrorKind::PartialSuccess(partial_success)) => {
                    std::fs::write(filename, partial_success.success_data + "\n").expect("failed");
                    write!(
                        stdout,
                        "{}",
                        partial_success.error_info.replace("\n", "\r\n")
                    )?;
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

use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc,
};

#[cfg(unix)]
use std::os::unix::process::CommandExt;

// Wrapper to run commands with Ctrl+C support
pub fn execute_with_interrupt_support(input: &str, stdout: &mut impl Write) -> io::Result<()> {
    if input.contains('|') {
        execute_pipeline_interruptible(input, stdout)
    } else {
        execute_single_interruptible(input, stdout)
    }
}

// Single command with Ctrl+C support
pub fn execute_single_interruptible(input: &str, stdout: &mut impl Write) -> io::Result<()> {
    let mut parts = tokenize(input);

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

    if parts.is_empty() {
        return Ok(());
    }

    let cmd = &parts[0];
    let args = &parts[1..];

    // Handle builtins normally (they run in-process)
    if matches!(
        cmd.as_str(),
        "cd" | "exit" | "pwd" | "type" | "echo" | "history"
    ) {
        let output = Builtins.execute(cmd, args);
        handle_output(output, io_stream, stdout)?;
        return Ok(());
    }

    // Spawn external command in new process group
    let mut command = Command::new(cmd);
    command
        .args(args)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());

    #[cfg(unix)]
    unsafe {
        command.pre_exec(|| {
            // Create new process group
            libc::setpgid(0, 0);
            Ok(())
        });
    }

    let mut child = match command.spawn() {
        Ok(c) => c,
        Err(_) => {
            write!(stdout, "{}: command not found\r\n", cmd)?;
            return Ok(());
        }
    };

    let pid = child.id() as i32;

    // Read output in thread
    let child_stdout = child.stdout.take().unwrap();
    let child_stderr = child.stderr.take().unwrap();

    let killed = Arc::new(AtomicBool::new(false));
    let killed_clone = killed.clone();

    // Output thread
    let output_thread = thread::spawn(move || {
        let reader = BufReader::new(child_stdout);
        let mut output = String::new();
        for line in reader.lines() {
            if let Ok(line) = line {
                output.push_str(&line);
                output.push('\n');
            }
        }
        output
    });

    let error_thread = thread::spawn(move || {
        let reader = BufReader::new(child_stderr);
        let mut output = String::new();
        for line in reader.lines() {
            if let Ok(line) = line {
                output.push_str(&line);
                output.push('\n');
            }
        }
        output
    });

    // Wait for process or Ctrl+C
    loop {
        // Check if process exited
        match child.try_wait()? {
            Some(_status) => break,
            None => {
                // Still running, sleep briefly
                thread::sleep(Duration::from_millis(50));
            }
        }
    }

    // Get output
    let stdout_str = output_thread.join().unwrap();
    let stderr_str = error_thread.join().unwrap();

    let output = process_partial_results(
        stdout_str.replace('\n', "\r\n"),
        stderr_str.replace('\n', "\r\n"),
    );
    handle_output(output, io_stream, stdout)?;
    stdout.flush()?;
    Ok(())
}

// Fix pipeline to let builtins ignore stdin and just output
pub fn execute_pipeline_interruptible(input: &str, stdout: &mut impl Write) -> io::Result<()> {
    let commands: Vec<Vec<String>> = input
        .split('|')
        .map(|cmd| tokenize(cmd.trim()))
        .filter(|parts| !parts.is_empty())
        .collect();

    if commands.is_empty() {
        return Ok(());
    }

    if commands.len() == 1 {
        return execute_single_interruptible(input, stdout);
    }

    let builtins = Builtins;
    let mut children: Vec<std::process::Child> = vec![];
    let mut prev_stdout = None;

    for (i, parts) in commands.iter().enumerate() {
        let cmd = &parts[0];
        let args = &parts[1..];
        let is_last = i == commands.len() - 1;

        // Check if it's a builtin
        if matches!(
            cmd.as_str(),
            "cd" | "exit" | "pwd" | "type" | "echo" | "cat"
        ) {
            // Builtins ignore stdin and just execute normally
            let builtin_output = match builtins.execute(cmd, args) {
                Ok(output) => output,
                Err(ErrorKind::CompleteFailure(err)) => {
                    write!(stdout, "{}\r\n", err)?;
                    for mut child in children {
                        let _ = child.kill();
                    }
                    return Ok(());
                }
                Err(ErrorKind::PartialSuccess(partial)) => {
                    write!(stdout, "{}\r\n", partial.error_info)?;
                    partial.success_data
                }
            };

            if is_last {
                // Last command - output to terminal
                if !builtin_output.is_empty() {
                    write!(stdout, "{}\r\n", builtin_output.replace('\n', "\r\n"))?;
                }
                stdout.flush()?;

                // Wait for any previous processes
                for mut child in children {
                    let _ = child.wait();
                }
                return Ok(());
            } else {
                // Builtin in middle of pipe - use echo to pass output to next command
                let mut command = Command::new("sh");
                command.arg("-c");
                command.arg(format!(
                    "echo -n '{}'",
                    builtin_output.replace("'", "'\\''")
                ));

                #[cfg(unix)]
                unsafe {
                    command.pre_exec(|| {
                        libc::setpgid(0, 0);
                        Ok(())
                    });
                }

                // Hook up stdin from previous (but builtin ignores it)
                if let Some(prev) = prev_stdout.take() {
                    command.stdin(prev);
                } else {
                    command.stdin(Stdio::null());
                }

                command.stdout(Stdio::piped());

                match command.spawn() {
                    Ok(mut child) => {
                        prev_stdout = child.stdout.take().map(Stdio::from);
                        children.push(child);
                    }
                    Err(_) => {
                        write!(stdout, "Error: failed to pipe builtin output\r\n")?;
                        return Ok(());
                    }
                }
            }
        } else {
            // External command
            let mut command = Command::new(cmd);
            command.args(args);

            #[cfg(unix)]
            unsafe {
                command.pre_exec(|| {
                    libc::setpgid(0, 0);
                    Ok(())
                });
            }

            if let Some(prev) = prev_stdout.take() {
                command.stdin(prev);
            }

            if is_last {
                command.stdout(Stdio::piped());
                command.stderr(Stdio::piped());
            } else {
                command.stdout(Stdio::piped());
            }

            match command.spawn() {
                Ok(mut child) => {
                    if !is_last {
                        prev_stdout = child.stdout.take().map(Stdio::from);
                    }
                    children.push(child);
                }
                Err(_) => {
                    write!(stdout, "{}: command not found\r\n", cmd)?;
                    for mut c in children {
                        let _ = c.kill();
                    }
                    return Ok(());
                }
            }
        }
    }

    // Read output from last command
    if let Some(mut last) = children.pop() {
        if let Some(last_stdout) = last.stdout.take() {
            let reader = BufReader::new(last_stdout);

            for line in reader.lines() {
                if let Ok(line) = line {
                    write!(stdout, "{}\r\n", line)?;
                    stdout.flush()?;
                }
            }
        }

        let _ = last.kill();
        let _ = last.wait();

        for mut child in children {
            let _ = child.kill();
            let _ = child.wait();
        }
    }

    Ok(())
}
