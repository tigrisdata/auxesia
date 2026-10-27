use auxesia::server::App;
use clap::Parser;
use std::{net::SocketAddr, path::PathBuf, process::ExitCode};

#[derive(Parser)]
#[command(
    version,
    about = "Serve Markdown documentation and downloadable skills"
)]
struct Args {
    /// Folder containing Markdown pages and optional .auxesia.yaml.
    root: PathBuf,
    #[arg(long, default_value = "127.0.0.1:8080")]
    listen: SocketAddr,
    /// Show update errors on rendered HTML pages.
    #[arg(long)]
    dev: bool,
}

#[tokio::main]
async fn main() -> ExitCode {
    let args = Args::parse();
    let root = args.root;
    let app = match tokio::task::spawn_blocking(move || App::open(&root, args.dev)).await {
        Ok(Ok(app)) => app,
        Ok(Err(failure)) => {
            for warning in failure.warnings {
                eprintln!("warning: {warning}");
            }
            for error in failure.errors {
                eprintln!("error: {error}");
            }
            return ExitCode::FAILURE;
        }
        Err(error) => {
            eprintln!("error: {error}");
            return ExitCode::FAILURE;
        }
    };
    let listener = match tokio::net::TcpListener::bind(args.listen).await {
        Ok(listener) => listener,
        Err(error) => {
            eprintln!("error: cannot listen on {}: {error}", args.listen);
            return ExitCode::FAILURE;
        }
    };
    eprintln!(
        "Auxesia listening on http://{}",
        listener.local_addr().unwrap()
    );
    match axum::serve(listener, app.router())
        .with_graceful_shutdown(shutdown())
        .await
    {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("error: {error}");
            ExitCode::FAILURE
        }
    }
}

async fn shutdown() {
    #[cfg(unix)]
    {
        use tokio::signal::unix::{SignalKind, signal};
        let mut terminate = signal(SignalKind::terminate()).expect("install SIGTERM handler");
        tokio::select! {
            _ = tokio::signal::ctrl_c() => {},
            _ = terminate.recv() => {},
        }
    }
    #[cfg(not(unix))]
    let _ = tokio::signal::ctrl_c().await;
}
