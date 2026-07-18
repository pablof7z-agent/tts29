#[cfg(unix)]
fn main() {
    if let Err(error) = run() {
        eprintln!("tts29-live-e2e: {error}");
        std::process::exit(1);
    }
}

#[cfg(unix)]
fn run() -> Result<(), String> {
    let mut arguments = std::env::args_os().skip(1);
    let path = match (arguments.next(), arguments.next(), arguments.next()) {
        (Some(flag), Some(path), None) if flag == "--config" => path,
        _ => return Err("usage: tts29-live-e2e --config <daemon.json>".into()),
    };
    let evidence = tts29_daemon::run_live_relay_smoke(path)?;
    serde_json::to_writer_pretty(std::io::stdout(), &evidence)
        .map_err(|error| format!("live evidence could not be encoded: {error}"))?;
    println!();
    Ok(())
}

#[cfg(not(unix))]
fn main() {
    eprintln!("tts29-live-e2e: the live daemon smoke requires Unix sockets");
    std::process::exit(1);
}
