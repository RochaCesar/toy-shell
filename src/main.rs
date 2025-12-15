pub mod builtins;
pub mod shell;
pub mod utils;
use crate::utils::*;
use shell::*;
use std::fs::OpenOptions;
#[allow(unused_imports)]
use std::io::{self, Write};
use std::io::{stdin, stdout};
use std::path::Path;
use std::process;
use termion::event::Event;
use termion::event::Key;
use utils::*;
// , Key, MouseEvent};
//
// use termion::event::Key;
use termion::input::MouseTerminal;
use termion::input::TermRead;
use termion::raw::IntoRawMode;

use std::io::prelude::*;

enum Output {
    AppendStdErr(Vec<String>),
    AppendStdOut(Vec<String>),
    RedirectStdOut(Vec<String>),
    RedirectStdErr(Vec<String>),
    StdOut,
}

fn main() -> io::Result<()> {
    let mut stdout = io::stdout().into_raw_mode().unwrap();
    let mut shell = Shell::new();
    loop {
        write!(stdout, "\r{}$ ", termion::clear::CurrentLine)?;
        io::stdout().flush()?;

        let stdin = io::stdin();
        // Read character by character
        for key in stdin.keys() {
            match key.unwrap() {
                Key::Char('\t') => {
                    let completions = shell.get_completions();

                    if completions.is_empty() {
                        write!(stdout, "\r\x07")?;
                        stdout.flush()?;
                    } else if completions.len() == 1 {
                        // Single completion - auto-complete
                        shell.input = completions[0].clone() + " ";
                        shell.cursor_pos = shell.input.len();
                        shell.redraw_line(&mut stdout)?;
                    } else if shell.last_key_was_tab {
                        // Multiple completions - show them
                        write!(stdout, "\r\n")?;
                        for completion in &completions {
                            write!(stdout, "{}  ", completion)?;
                        }
                        write!(stdout, "\r\n")?;
                        shell.redraw_line(&mut stdout)?;
                    } else {
                        // First tab - complete common prefix or ring bell
                        let common = shell.find_common_prefix(&completions);
                        if common.len() > shell.cursor_pos {
                            shell.input = common;
                            shell.cursor_pos = shell.input.chars().count();
                            shell.redraw_line(&mut stdout)?;
                        } else {
                            write!(stdout, "\x07")?;
                            stdout.flush()?;
                        }
                    }
                    shell.last_key_was_tab = true; // ← Mark that tab was pressed
                }
                Key::Char('\n') => {
                    shell.last_key_was_tab = false;
                    write!(stdout, "\r\n")?;
                    io::stdout().flush()?;

                    let input = shell.input.trim();

                    // Check for exit first
                    if input.starts_with("exit") {
                        let code = input
                            .split_whitespace()
                            .nth(1)
                            .and_then(|s| s.parse::<i32>().ok())
                            .unwrap_or(0);
                        drop(stdout);
                        io::stdout().flush()?;
                        process::exit(code);
                    }
                    if shell.input.contains("|") {
                        execute_pipeline(&shell.input, &mut stdout)?;
                    } else {
                        execute_single_command(&shell.input, &mut stdout)?;
                    }
                    // Reset for next command
                    shell.input.clear();
                    shell.cursor_pos = 0;
                    // Good - clear the entire line first
                    write!(stdout, "\r{}$ ", termion::clear::CurrentLine)?;
                    io::stdout().flush().unwrap();
                }
                Key::Char(c) => {
                    shell.last_key_was_tab = false;
                    // Convert character position to byte position
                    //
                    let byte_pos = shell
                        .input
                        .char_indices()
                        .nth(shell.cursor_pos)
                        .map(|(pos, _)| pos)
                        .unwrap_or(shell.input.len());

                    shell.input.insert(byte_pos, c);
                    shell.cursor_pos += 1;
                    shell.redraw_line(&mut stdout)?;
                }
                Key::Backspace => {
                    shell.last_key_was_tab = false;
                    if shell.cursor_pos > 0 {
                        shell.cursor_pos -= 1;

                        // Convert character position to byte position
                        let byte_pos = shell
                            .input
                            .char_indices()
                            .nth(shell.cursor_pos)
                            .map(|(pos, _)| pos)
                            .unwrap_or(shell.input.len());

                        shell.input.remove(byte_pos);
                        shell.redraw_line(&mut stdout)?;
                    }
                }
                Key::Ctrl('c') => {
                    shell.last_key_was_tab = false;
                    // Exit on Ctrl+C
                    write!(stdout, "\r\n").unwrap();
                    stdout.flush().unwrap();
                    drop(stdout); // Exit raw mode
                    std::process::exit(0);
                }
                _ => {
                    shell.last_key_was_tab = false;
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
