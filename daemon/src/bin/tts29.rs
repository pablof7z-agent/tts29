use std::ffi::OsString;
use std::fs::File;
use std::io::{self, Read, Write};
use std::path::PathBuf;

use tts29_daemon::{
    LocalPublishRequest, LocalPublishResponse, LocalTreeRequest, ProducerRequest, SpokenTree,
    LOCAL_PROTOCOL_VERSION, MAX_ANSWER_WAIT_SECONDS, MAX_LOCAL_FRAME_BYTES,
};

fn main() {
    if let Err(error) = run() {
        eprintln!("tts29: {error}");
        std::process::exit(1);
    }
}

fn run() -> Result<(), String> {
    let command = parse_arguments()?;
    let bytes = read_request(&command.request_path)?;
    let agent_nsec = optional_env("AGENT_NSEC")?;
    let response = if command.tree {
        let tree: SpokenTree = serde_json::from_slice(&bytes)
            .map_err(|error| format!("spoken tree is invalid JSON: {error}"))?;
        let local = LocalTreeRequest {
            version: LOCAL_PROTOCOL_VERSION,
            tree,
            agent_id: command.agent_id,
            agent_nsec,
        };
        local
            .validate()
            .map_err(|error| format!("{}: {}", error.code, error.message))?;
        tts29_daemon::submit_local_tree(command.socket_path, &local)
            .map_err(|error| format!("daemon request failed: {error}"))?
    } else {
        let request: ProducerRequest = serde_json::from_slice(&bytes)
            .map_err(|error| format!("producer request is invalid JSON: {error}"))?;
        let local = LocalPublishRequest {
            version: LOCAL_PROTOCOL_VERSION,
            request,
            wait_for_answer_seconds: command.wait_for_answer_seconds,
            agent_nsec,
        };
        local
            .validate()
            .map_err(|error| format!("{}: {}", error.code, error.message))?;
        tts29_daemon::submit_local(command.socket_path, &local)
            .map_err(|error| format!("daemon request failed: {error}"))?
    };
    let failed = matches!(response, LocalPublishResponse::Error { .. });
    let stdout = io::stdout();
    let mut output = stdout.lock();
    serde_json::to_writer(&mut output, &response)
        .map_err(|error| format!("response could not be written: {error}"))?;
    output
        .write_all(b"\n")
        .map_err(|error| format!("response could not be written: {error}"))?;
    output
        .flush()
        .map_err(|error| format!("response could not be flushed: {error}"))?;
    drop(output);
    if failed {
        std::process::exit(2);
    }
    Ok(())
}

struct Command {
    socket_path: PathBuf,
    request_path: Option<PathBuf>,
    wait_for_answer_seconds: Option<u64>,
    tree: bool,
    agent_id: Option<String>,
}

fn parse_arguments() -> Result<Command, String> {
    let mut socket_path = std::env::var_os("TTS29_SOCKET").map(PathBuf::from);
    let mut request_path = None;
    let mut wait_for_answer_seconds = None;
    let mut tree = false;
    let mut agent_id = None;
    let mut socket_seen = false;
    let mut request_seen = false;
    let mut arguments = std::env::args_os().skip(1);
    while let Some(flag) = arguments.next() {
        match flag.to_str() {
            Some("--socket") if !socket_seen => {
                socket_path = Some(required_value(&mut arguments, "--socket")?.into());
                socket_seen = true;
            }
            Some("--tree") if !tree => {
                tree = true;
            }
            Some("--agent-id") if agent_id.is_none() => {
                let value = required_value(&mut arguments, "--agent-id")?;
                agent_id = Some(
                    value
                        .to_str()
                        .ok_or_else(|| "--agent-id must be UTF-8".to_string())?
                        .to_string(),
                );
            }
            Some("--request") if !request_seen => {
                let value = required_value(&mut arguments, "--request")?;
                if value != "-" {
                    request_path = Some(value.into());
                }
                request_seen = true;
            }
            Some("--wait-seconds") if wait_for_answer_seconds.is_none() => {
                let value = required_value(&mut arguments, "--wait-seconds")?;
                let text = value
                    .to_str()
                    .ok_or_else(|| "--wait-seconds must be UTF-8".to_string())?;
                wait_for_answer_seconds = Some(
                    text.parse::<u64>()
                        .ok()
                        .filter(|value| *value > 0 && *value <= MAX_ANSWER_WAIT_SECONDS)
                        .ok_or_else(|| {
                            format!(
                                "--wait-seconds must be between 1 and {MAX_ANSWER_WAIT_SECONDS}"
                            )
                        })?,
                );
            }
            Some("--help" | "-h") => {
                println!(
                    "Usage: tts29 --socket <path> [--request <file|->] \
                     [--wait-seconds <1-300>] [--tree] [--agent-id <name>]"
                );
                std::process::exit(0);
            }
            _ => return Err("invalid or repeated command argument; use --help".into()),
        }
    }
    let socket_path = socket_path
        .ok_or_else(|| "a socket path is required through --socket or TTS29_SOCKET".to_string())?;
    if !tree && agent_id.is_some() {
        return Err("--agent-id is only valid with --tree".into());
    }
    Ok(Command {
        socket_path,
        request_path,
        wait_for_answer_seconds,
        tree,
        agent_id,
    })
}

fn required_value(
    arguments: &mut impl Iterator<Item = OsString>,
    flag: &str,
) -> Result<OsString, String> {
    arguments
        .next()
        .ok_or_else(|| format!("{flag} requires a value"))
}

fn read_request(path: &Option<PathBuf>) -> Result<Vec<u8>, String> {
    let mut source: Box<dyn Read> = match path {
        Some(path) => Box::new(
            File::open(path).map_err(|error| format!("request file could not be read: {error}"))?,
        ),
        None => Box::new(io::stdin().lock()),
    };
    let mut bytes = Vec::new();
    source
        .by_ref()
        .take(MAX_LOCAL_FRAME_BYTES as u64 + 1)
        .read_to_end(&mut bytes)
        .map_err(|error| format!("producer request could not be read: {error}"))?;
    if bytes.is_empty() || bytes.len() > MAX_LOCAL_FRAME_BYTES {
        return Err("producer request size is invalid".into());
    }
    Ok(bytes)
}

fn optional_env(name: &str) -> Result<Option<String>, String> {
    match std::env::var(name) {
        Ok(value) if value.is_empty() => Err(format!("{name} is empty")),
        Ok(value) => Ok(Some(value)),
        Err(std::env::VarError::NotPresent) => Ok(None),
        Err(std::env::VarError::NotUnicode(_)) => Err(format!("{name} is not valid UTF-8")),
    }
}
