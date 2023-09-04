use clap::Parser;
use ping_template::{cli_args::CliArgs, ping_app, utils};

#[tokio::main]
async fn main() {
    env_logger::init();
    console_subscriber::init();

    let ping_app = ping_app::PingApp::new(CliArgs::parse());
    if let Err(err) = ping_app.run().await {
        log::error!("Application error: {}", err);
    }

    utils::dump_conn_map(ping_app.conn_map()).await;
}
