use std::io::{self, BufRead, Write};
use std::process;
use std::thread;
use std::time::Duration;

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let mut i = 1;

    while i < args.len() {
        match args[i].as_str() {
            "--echo" => {
                echo_stdin();
            }
            "--stderr-message" => {
                i += 1;
                if i < args.len() {
                    eprintln!("{}", args[i]);
                }
            }
            "--exit-code" => {
                i += 1;
                if i < args.len() {
                    if let Ok(code) = args[i].parse::<i32>() {
                        process::exit(code);
                    }
                }
            }
            "--sleep" => {
                i += 1;
                if i < args.len() {
                    if let Ok(ms) = args[i].parse::<u64>() {
                        thread::sleep(Duration::from_millis(ms));
                    }
                }
            }
            "--infinite-loop" => {
                // Infinite loop to test shutdown/kill behavior
                loop {
                    thread::sleep(Duration::from_secs(1));
                }
            }
            _ => {}
        }
        i += 1;
    }
}

fn echo_stdin() {
    let stdin = io::stdin();
    let stdout = io::stdout();
    let mut handle = stdout.lock();

    for line in stdin.lock().lines() {
        if let Ok(line) = line {
            writeln!(handle, "{}", line).unwrap();
            handle.flush().unwrap();
        }
    }
}

