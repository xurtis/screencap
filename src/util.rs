//! Utilities.

use std::env::var;
use std::io::{BufRead, BufReader};
use std::path::Path;
use std::process::{Command, Stdio};
use std::str::FromStr;

#[macro_export]
macro_rules! exec {
    ($command:ident $($args:tt)*) => {{
        let mut command: std::process::Command = which(stringify!($command))
            .expect(&format!("No command {:?} found", stringify!($command)));
        exec!(@(&mut command) $($args)*);
        command
    }};
    (($command:expr) $($args:tt)*) => {{
        let command_name = $command.to_string();
        let mut command: std::process::Command = which(&command_name)
            .expect(&format!("No command {:?} found", command_name));
        exec!(@(command) $($args)*);
        command
    }};
    (@($command:expr) --$argument:ident $($args:tt)*) => {
        exec!(@($command) (format!("--{}", stringify!($argument))) $($args)*)
    };
    (@($command:expr) -$argument:ident $($args:tt)*) => {
        exec!(@($command) (format!("-{}", stringify!($argument))) $($args)*)
    };
    (@($command:expr) $argument:ident $($args:tt)*) => {
        exec!(@($command) (stringify!($argument)) $($args)*)
    };
    (@($command:expr) ($argument:expr) $($args:tt)*) => {
        exec!(@($command.arg(&$argument.to_string())) $($args)*)
    };
    (@($command:expr)) => {
        $command
    };
}

/// Create a command from a given binary name.
pub fn which<P: AsRef<Path>>(binary: P) -> Option<Command> {
    if binary.as_ref().starts_with("./") && binary.as_ref().exists() {
        Some(Command::new(binary.as_ref()))
    } else {
        var("PATH")
            .ok()?
            .split(':')
            .map(|prefix| Path::new(prefix).to_owned())
            .map(|mut prefix| {
                prefix.push(&binary);
                prefix
            })
            .filter(|path| path.exists())
            .nth(0)
            .map(Command::new)
    }
}

/// An iterator over the lines output from a command.
pub fn command_output(mut command: Command) -> impl Iterator<Item = String> {
    let command_text = format!("{:?}", command);
    let child = command
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .expect(&format!("Execute {}", command_text));

    BufReader::new(child.stdout.expect(&format!("Read from {}", command_text)))
        .lines()
        .filter(Result::is_ok)
        .map(Result::unwrap)
}

/// Get the nth word in a line as a string.
pub fn line_nth(line: String, nth: usize) -> String {
    line.trim()
        .split_whitespace()
        .nth(nth)
        .expect(&format!("Read item #{} from {:?}", nth, line))
        .to_owned()
}

/// Get the next line matching the given predicate.
pub fn get_line(
    lines: impl Iterator<Item = String>,
    mut predicate: impl FnMut(&str) -> bool,
) -> (impl Iterator<Item = String>, String) {
    let mut lines = lines.skip_while(move |s| !predicate(s));
    let line = lines.next().expect("Read line matching predicate");
    (lines, line)
}

/// Get the neth item in the line matching the predicate.
pub fn get_nth_from_line(
    lines: impl Iterator<Item = String>,
    predicate: impl FnMut(&str) -> bool,
    nth: usize,
) -> (impl Iterator<Item = String>, String) {
    let (lines, line) = get_line(lines, predicate);
    (lines, line_nth(line, nth))
}

#[derive(Debug, Clone)]
pub struct FFMPEGSupport {
    names: Vec<String>,
    description: String,
    decode: bool,
    encode: bool,
}

#[derive(Debug, PartialEq, Eq)]
enum Type {
    Audio,
    Video,
    Subtitle,
    Format,
}
use self::Type::*;

impl FromStr for Type {
    type Err = ();

    fn from_str(s: &str) -> Result<Type, ()> {
        match s {
            "D" | "E" | "DE" => return Ok(Format),
            _ => {}
        };

        if s.len() != 6 {
            return Err(());
        }

        match (&s[0..1], &s[2..3]) {
            ("A", _) | (_, "A") => Ok(Audio),
            ("V", _) | (_, "V") => Ok(Video),
            ("S", _) | (_, "S") => Ok(Subtitle),
            _ => Err(()),
        }
    }
}

impl FFMPEGSupport {
    pub fn formats() -> impl Iterator<Item = FFMPEGSupport> {
        Self::parse(exec!(ffmpeg - formats))
            .filter(|(_, t)| *t == Format)
            .map(|(s, _)| s)
    }

    pub fn video_encoders() -> impl Iterator<Item = FFMPEGSupport> {
        Self::encoders()
            .filter(|(_, t)| *t == Video)
            .map(|(s, _)| s)
    }

    pub fn audio_encoders() -> impl Iterator<Item = FFMPEGSupport> {
        Self::encoders()
            .filter(|(_, t)| *t == Audio)
            .map(|(s, _)| s)
    }

    pub fn has_name(&self, name: &str) -> bool {
        for n in &self.names {
            if n == name {
                return true;
            }
        }

        false
    }

    pub fn name(&self) -> &str {
        &self.names[0]
    }

    pub fn encode(&self) -> bool {
        self.encode
    }

    pub fn decode(&self) -> bool {
        self.decode
    }

    fn encoders() -> impl Iterator<Item = (FFMPEGSupport, Type)> {
        Self::parse(exec!(ffmpeg - encoders)).map(|(mut s, t)| {
            s.encode = true;
            s.decode = false;
            (s, t)
        })
    }

    fn parse(mut command: Command) -> impl Iterator<Item = (FFMPEGSupport, Type)> {
        let child = command
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .expect("Launching ffmpeg process");

        let output = child.stdout.expect("Reading child output");

        BufReader::new(output)
            .lines()
            .filter(Result::is_ok)
            .map(Result::unwrap)
            .map(Self::decode_line)
            .filter(Option::is_some)
            .map(Option::unwrap)
    }

    fn decode_line(line: String) -> Option<(FFMPEGSupport, Type)> {
        let line = line.trim();

        let code_end = line.find(char::is_whitespace)?;
        let code = line[..code_end].trim();

        let type_ = code.parse().ok()?;

        let line = line[code_end..].trim();
        let names_end = line.find(char::is_whitespace)?;
        let names = line[..names_end].trim();
        let names = names.split(',').map(|s| s.to_owned()).collect();

        let description = line[names_end..].trim().to_owned();

        let (decode, encode) = match type_ {
            Format => (code.contains("D"), code.contains("E")),
            _ => (&code[0..1] == "D", &code[1..2] == "E"),
        };

        let support = FFMPEGSupport {
            names,
            description,
            decode,
            encode,
        };

        Some((support, type_))
    }
}
