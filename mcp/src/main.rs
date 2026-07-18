fn main() {
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .unwrap_or_else(|error| {
            eprintln!("tts29-mcp: runtime could not start: {error}");
            std::process::exit(1);
        });
    if let Err(error) = runtime.block_on(run()) {
        eprintln!("tts29-mcp: {error}");
        std::process::exit(1);
    }
}

async fn run() -> Result<(), String> {
    let config_path = config_argument()?;
    let config = tts29_mcp::load_config(config_path)?;
    tts29_mcp::run_server(config).await
}

fn config_argument() -> Result<std::path::PathBuf, String> {
    let mut arguments = std::env::args_os().skip(1);
    match (arguments.next(), arguments.next(), arguments.next()) {
        (Some(flag), Some(path), None) if flag == "--config" => Ok(path.into()),
        (Some(flag), None, None) if flag == "--help" || flag == "-h" => {
            println!("Usage: tts29-mcp --config <mcp.json>");
            std::process::exit(0);
        }
        _ => Err("usage: tts29-mcp --config <mcp.json>".into()),
    }
}
