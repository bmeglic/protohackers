use clap::Parser;

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    /// TCP port to listen on
    #[arg(short, long, default_value_t = 7777)]
    port: u16,
}

fn main() {
    let args = Args::parse();

    println!("Will start listening on TCP port: {}", args.port);
}
