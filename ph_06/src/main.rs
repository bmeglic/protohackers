use clap::Parser;
use std::error::Error;
/*
pub mod codec;
pub mod consts;
pub mod heartbeat;
pub mod server;
pub mod test;
*/

use ph_06::server;

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    /// TCP port to listen on
    #[arg(short, long, default_value_t = 7777)]
    port: u16,

    /// IP to listen on
    #[arg(long, default_value_t = String::from("0.0.0.0"))]
    host: String,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    let args = Args::parse();

    let hostname = format!("{}:{}", args.host, args.port);
    println!("Will start listening on {hostname}");

    let server = server::SpeedDaemonServer::new();
    server.run(hostname).await?;

    Ok(())
}
