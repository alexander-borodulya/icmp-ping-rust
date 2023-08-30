use clap::Parser;
use ping_template::{cli_args::CliArgs, ping_app};

#[tokio::main]
async fn main() {
    env_logger::init();
    console_subscriber::init();
    ping_app::run(CliArgs::parse()).await;
}
