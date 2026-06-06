mod cli;
mod config;
mod context;
mod conversation;
mod model;
mod output;
mod query;
mod redact;
mod risk;
mod route_cache;
mod selflog;
mod statefile;

#[tokio::main]
async fn main() {
    match cli::Cli::parse_and_run().await {
        Ok(code) => std::process::exit(code),
        Err(e) => {
            eprintln!("tiog: {e:#}");
            std::process::exit(1);
        }
    }
}
