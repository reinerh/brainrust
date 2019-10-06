/*
 * Copyright 2019 Reiner Herrmann <reiner@reiner-h.de>
 * License: GPL-3+
 */

use std::env;
use std::fs::File;
use std::io::{self, Read, BufReader};
use std::collections::HashMap;

#[derive(PartialEq,Copy,Clone,Debug)]
enum Command {
    INCPTR { amount: isize },
    DECPTR { amount: isize },
    INCVAL { amount: u8 },
    DECVAL { amount: u8 },
    PUTC,
    GETC,
    LOOPSTART { end: usize },
    LOOPEND { start: usize },
}

struct Program {
    commands: Vec<Command>,
}

impl Program {
    pub fn run(&mut self, input: &mut dyn io::Read, output: &mut dyn io::Write) -> Result<(), String> {
        let mut memory : HashMap<isize, u8> = HashMap::new();
        let mut pos : isize = 0;
        let mut pc : usize = 0;
        loop {
            if pc >= self.commands.len() {
                break;
            }
            match self.commands[pc] {
                Command::INCPTR { amount } => pos = pos.checked_add(amount).ok_or("Pointer overflow")?,
                Command::DECPTR { amount } => pos = pos.checked_sub(amount).ok_or("Pointer underflow")?,
                Command::INCVAL { amount } => {
                    let val = memory.entry(pos).or_insert(0);
                    *val = val.wrapping_add(amount);
                },
                Command::DECVAL { amount } => {
                    let val = memory.entry(pos).or_insert(0);
                    *val = val.wrapping_sub(amount);
                },
                Command::PUTC => {
                    let char_out = *memory.get(&pos).unwrap_or(&0);
                    output.write(&[char_out]).or(Err("Writing to output failed"))?;
                },
                Command::GETC => {
                    let mut char_in = [0];
                    match input.read_exact(&mut char_in) {
                        Ok(_) => memory.insert(pos, char_in[0]),
                        Err(ref err) if err.kind() == io::ErrorKind::UnexpectedEof => None /* do nothing */,
                        Err(_) => return Err("Reading from input failed".to_string()),
                    };
                },
                Command::LOOPSTART { end } => {
                    if *memory.get(&pos).unwrap_or(&0) == 0 {
                        pc = end;
                    }
                },
                Command::LOOPEND { start } => {
                    pc = start;
                    continue;
                },
            };
            pc = pc.checked_add(1).ok_or("PC overflow")?;
        }
        Ok(())
    }
}

/// read the program from the specified file into a string
fn read_program(filename: &str) -> Result<String, io::Error> {
    let file = File::open(filename)?;
    let mut content = String::new();
    let mut reader = BufReader::new(file);
    reader.read_to_string(&mut content)?;
    Ok(content)
}

/// update loop tokens and look for the matching opposing parts
fn find_loops(commands: &mut Vec<Command>) -> Result<(), String> {
    let mut loop_starts = Vec::new();
    for i in 0 .. commands.len() {
        match commands[i] {
            Command::LOOPSTART { .. } => {
                loop_starts.push(i);
            }
            Command::LOOPEND { .. } => {
                let start = loop_starts.pop().ok_or("Cannot find opening bracket for closing bracket")?;
                commands[start] = Command::LOOPSTART { end: i };
                commands[i] = Command::LOOPEND { start };
            },
            _ => {},
        };
    }
    if !loop_starts.is_empty() {
        return Err("More opening than closing loop brackets".to_string());
    }
    Ok(())
}

/// remove pairs of commands that cancel each other,
/// like (increase_value, decrease_value)
fn optimize_cancelling_pairs(program: &mut Vec<Command>) {
    let mut remove_pair = |first: Command, second: Command| -> bool {
        for i in 0 .. program.len() - 1 {
            if (program[i] == first && program[i+1] == second) ||
               (program[i] == second && program[i+1] == first) {
                program.remove(i+1);
                program.remove(i);
                return true;
            }
        }
        false
    };

    while remove_pair(Command::INCPTR { amount: 1 }, Command::DECPTR { amount: 1 }) {}
    while remove_pair(Command::INCVAL { amount: 1 }, Command::DECVAL { amount: 1 }) {}
}

/// combine sequences of identical commands into a single command
fn optimize_sequences(program: &mut Vec<Command>) {
    let mut to_remove = Vec::new();
    let mut seq_len = 1;
    for i in 1 .. program.len() {
        if program[i-1] != program[i] {
            seq_len = 1;
            continue;
        }
        // update first element of the sequence
        match program[i-seq_len] {
            Command::INCPTR { amount } => program[i-seq_len] = Command::INCPTR { amount: amount.wrapping_add(1) },
            Command::DECPTR { amount } => program[i-seq_len] = Command::DECPTR { amount: amount.wrapping_add(1) },
            Command::INCVAL { amount } => program[i-seq_len] = Command::INCVAL { amount: amount.wrapping_add(1) },
            Command::DECVAL { amount } => program[i-seq_len] = Command::DECVAL { amount: amount.wrapping_add(1) },
            _ => continue,
        };
        to_remove.push(i);
        seq_len += 1;
    }
    // remove elements in reverse direction to keep indexes valid
    for i in to_remove.iter().rev() {
        program.remove(*i);
    }
}

fn optimize(program: &mut Vec<Command>) {
    optimize_cancelling_pairs(program);
    optimize_sequences(program);
}

/// filter out all non-syntax characters from the input
fn preprocess(program: &str) -> String {
    let allowed_chars = ['>', '<', '+', '-', '.', ',', '[', ']'];
    program.chars().filter(|c| allowed_chars.contains(c)).collect()
}

/// convert input string into syntax tokens
fn tokenize(input: &str) -> Vec<Command> {
    input.chars().map(|token|
        match token {
            '>' => Command::INCPTR { amount: 1 },
            '<' => Command::DECPTR { amount: 1 },
            '+' => Command::INCVAL { amount: 1 },
            '-' => Command::DECVAL { amount: 1 },
            '.' => Command::PUTC,
            ',' => Command::GETC,
            '[' => Command::LOOPSTART { end: 0 },
            ']' => Command::LOOPEND { start: 0 },
            _ => panic!("Trying to tokenize invalid character: {}", token),
        }
    ).collect()
}

fn run() -> Result<(), String> {
    let args: Vec<String> = env::args().collect();
    if args.len() < 2 {
        return Err(format!("Usage: {} filename", args[0]));
    }
    let input = match read_program(&args[1]) {
        Ok(p) => preprocess(&p),
        Err(e) => return Err(format!("Cannot open file: {}", e)),
    };
    let mut commands = tokenize(&input);
    optimize(&mut commands);
    find_loops(&mut commands)?;

    let mut program = Program{commands};
    program.run(&mut io::stdin(), &mut io::stdout())
}

fn main() {
    if let Err(e) = run() {
        println!("Error: {}", e);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_preprocess() {
        assert_eq!(preprocess(">foo<bar[hello] world. this,is+a-comment"), "><[].,+-");
        assert_eq!(preprocess(""), "");
        assert_eq!(preprocess("not a real program"), "");
        assert_eq!(preprocess("[>>>+++,---.<<<]"), "[>>>+++,---.<<<]");
    }

    #[test]
    fn test_tokenize() {
        assert_eq!(tokenize(""), []);
        assert_eq!(tokenize("><+-.,[]"),
                   [
                    Command::INCPTR { amount: 1 },
                    Command::DECPTR { amount: 1 },
                    Command::INCVAL { amount: 1 },
                    Command::DECVAL { amount: 1 },
                    Command::PUTC,
                    Command::GETC,
                    Command::LOOPSTART { end: 0 },
                    Command::LOOPEND { start: 0 },
                   ]
        );
    }

    #[test]
    #[should_panic]
    fn test_tokenize_panic() {
        tokenize("42");
    }

    #[test]
    fn test_find_loops() {
        let mut commands = vec![
            Command::INCPTR { amount: 1 },
            Command::INCVAL { amount: 1 },
            Command::LOOPSTART { end: 0 },
            Command::DECPTR { amount: 1 },
            Command::LOOPSTART { end: 0 },
            Command::DECVAL { amount: 1 },
            Command::LOOPEND { start: 0 },
            Command::PUTC,
            Command::LOOPEND { start: 0 },
        ];
        assert!(find_loops(&mut commands).is_ok());
        assert_eq!(commands[2], Command::LOOPSTART { end: 8 });
        assert_eq!(commands[4], Command::LOOPSTART { end: 6 });
        assert_eq!(commands[6], Command::LOOPEND { start: 4 });
        assert_eq!(commands[8], Command::LOOPEND { start: 2 });
    }

    #[test]
    fn test_find_loop_unbalanced() {
        let mut commands = vec![
                                Command::INCPTR { amount: 1 },
                                Command::LOOPSTART { end: 0 },
                                Command::INCVAL { amount: 1 }
                               ];
        assert!(find_loops(&mut commands).is_err());

        let mut commands = vec![
                                Command::INCPTR { amount: 1 },
                                Command::LOOPEND { start: 0 },
                                Command::INCVAL { amount: 1 },
                               ];
        assert!(find_loops(&mut commands).is_err());
    }

    #[test]
    fn test_program_run() {
        use std::io::Cursor;

        let mut buf_in = Cursor::new("31abc".as_bytes());
        let mut buf_out = Cursor::new(Vec::new());
        let code = "++[>[>],+[<]>-]>[.>]";  // reads 2 chars, increments them, and prints them at the end
        let mut commands = tokenize(code);
        find_loops(&mut commands).unwrap();
        let mut program = Program{commands};
        program.run(&mut buf_in, &mut buf_out).unwrap();

        let expected = vec!['4' as u8, '2' as u8];
        assert_eq!(buf_out.get_ref(), &expected);
    }

    #[test]
    fn test_optimize_pair_removal() {
        let incv = Command::INCVAL { amount: 1 };
        let decv = Command::DECVAL { amount: 1 };
        let incp = Command::INCPTR { amount: 1 };
        let decp = Command::DECPTR { amount: 1 };
        let mut commands = vec![
            incp, decp, decp, incp,
            incv, decv, decv, incv,
            incp, Command::GETC, decp,
            incv, incv, decp, incp, decv, decv,
            Command::PUTC, incv,
        ];
        optimize_cancelling_pairs(&mut commands);
        assert_eq!(commands, [incp, Command::GETC, decp, Command::PUTC, incv]);
    }

    #[test]
    fn test_optimize_sequences() {
        let incv = Command::INCVAL { amount: 1 };
        let decv = Command::DECVAL { amount: 1 };
        let incp = Command::INCPTR { amount: 1 };
        let decp = Command::DECPTR { amount: 1 };
        let mut commands = vec![incv, decv, incp, decp, incv, incp, incv, incp];
        optimize_sequences(&mut commands);
        assert_eq!(commands, [incv, decv, incp, decp, incv, incp, incv, incp]);

        let mut commands = vec![incv, incv, incv, incv, decp, decp, incp, incp, incp, decv, decv];
        optimize_sequences(&mut commands);
        assert_eq!(commands, [
                              Command::INCVAL { amount: 4 },
                              Command::DECPTR { amount: 2 },
                              Command::INCPTR { amount: 3 },
                              Command::DECVAL { amount: 2 },
                             ]
        );

        let mut commands = vec![
                                Command::PUTC, Command::PUTC, Command::GETC, Command::GETC,
                                Command::LOOPSTART { end: 0 }, Command::LOOPSTART { end: 0 },
                                Command::LOOPEND { start: 0 }, Command::LOOPEND { start: 0 }
                               ];
        let expected = commands.clone();
        optimize_sequences(&mut commands);
        assert_eq!(commands, expected);
    }

    #[test]
    fn test_optimize() {
        let incv = Command::INCVAL { amount: 1 };
        let decv = Command::DECVAL { amount: 1 };
        let incp = Command::INCPTR { amount: 1 };
        let decp = Command::DECPTR { amount: 1 };
        let mut commands = vec![incv, incp, decp, incv, incv, decv, incv];
        optimize(&mut commands);
        assert_eq!(commands, [Command::INCVAL { amount: 3 }]);
    }
}
