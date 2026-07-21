#[cfg(unix)]
fn main() {
    if let Err(error) = run() {
        eprintln!("tts29d: {error}");
        std::process::exit(1);
    }
}

#[cfg(unix)]
fn run() -> Result<(), String> {
    let config_path = config_argument()?;
    let loaded = tts29_daemon::load_daemon_config(config_path)?;
    let mut producer = tts29_daemon::ProductionProducer::open(loaded.production)?;
    let bootstrap = producer.bootstrap_evidence();
    eprintln!(
        "tts29d: daemon={} group_created={} owner_promoted={}",
        bootstrap.daemon_pubkey,
        bootstrap.group_created_event_id.is_some(),
        bootstrap.owner_admin_event_id.is_some()
    );
    let listener = tts29_daemon::PrivateUnixListener::bind(&loaded.socket_path)
        .map_err(|error| format!("local endpoint could not start: {error}"))?;
    let shutdown = tts29_daemon::LocalServerShutdown::new(listener.path());
    let signal_shutdown = shutdown.clone();
    ctrlc::set_handler(move || signal_shutdown.request())
        .map_err(|error| format!("shutdown signal handler could not start: {error}"))?;
    let result = tts29_daemon::serve_until_shutdown(&listener, &mut producer, &shutdown)
        .map_err(|error| format!("local endpoint failed: {error}"));
    producer.shutdown();
    result
}

#[cfg(unix)]
fn config_argument() -> Result<std::path::PathBuf, String> {
    let mut arguments = std::env::args_os().skip(1);
    match (arguments.next(), arguments.next(), arguments.next()) {
        (Some(flag), Some(path), None) if flag == "--config" => Ok(path.into()),
        (Some(flag), None, None) if flag == "--help" || flag == "-h" => {
            println!("Usage: tts29d --config <daemon.json>");
            std::process::exit(0);
        }
        _ => Err("usage: tts29d --config <daemon.json>".into()),
    }
}

#[cfg(not(unix))]
fn main() {
    eprintln!("tts29d: the local daemon endpoint requires Unix sockets");
    std::process::exit(1);
}
