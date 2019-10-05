/*
 * Copyright 2019 Reiner Herrmann <reiner@reiner-h.de>
 * License: GPL-3+
 */

use std::env;
use std::fs::File;
use std::io::{self, Read, BufReader};
use std::collections::HashMap;

#[derive(PartialEq,Debug)]
enum Command {
    INCPTR,
    DECPTR,
    INCVAL,
    DECVAL,
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
                Command::INCPTR => pos = pos.checked_add(1).ok_or("Pointer overflow")?,
                Command::DECPTR => pos = pos.checked_sub(1).ok_or("Pointer underflow")?,
                Command::INCVAL => {
                    let val = memory.entry(pos).or_insert(0);
                    *val = val.wrapping_add(1);
                },
                Command::DECVAL => {
                    let val = memory.entry(pos).or_insert(0);
                    *val = val.wrapping_sub(1);
                },
                Command::PUTC => {
                    let char_out = *memory.get(&pos).unwrap_or(&0);
                    output.write(&[char_out]).or(Err("Writing to output failed"))?;
                },
                Command::GETC => {
                    let mut char_in = [0];
                    input.read_exact(&mut char_in).or(Err("Reading the input failed"))?;
                    memory.insert(pos, char_in[0]);
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

fn read_program(filename: &str) -> Result<String, io::Error> {
    let file = File::open(filename)?;
    let mut content = String::new();
    let mut reader = BufReader::new(file);
    reader.read_to_string(&mut content)?;
    Ok(content)
}

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

fn preprocess(program: &str) -> String {
    let allowed_chars = ['>', '<', '+', '-', '.', ',', '[', ']'];
    program.chars().filter(|c| allowed_chars.contains(c)).collect()
}

fn tokenize(input: &str) -> Vec<Command> {
    input.chars().map(|token|
        match token {
            '>' => Command::INCPTR,
            '<' => Command::DECPTR,
            '+' => Command::INCVAL,
            '-' => Command::DECVAL,
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
                    Command::INCPTR,
                    Command::DECPTR,
                    Command::INCVAL,
                    Command::DECVAL,
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
            Command::INCPTR,
            Command::INCVAL,
            Command::LOOPSTART { end: 0 },
            Command::DECPTR,
            Command::LOOPSTART { end: 0 },
            Command::DECVAL,
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
        let mut commands = vec![Command::INCPTR, Command::LOOPSTART { end: 0 }, Command::INCVAL];
        assert!(find_loops(&mut commands).is_err());

        let mut commands = vec![Command::INCPTR, Command::LOOPEND { start: 0 }, Command::INCVAL];
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
}
