use clap::Parser;
use std::collections::HashMap;
use std::error::Error;
use std::str;
use tokio::net::UdpSocket;

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

    let sock = UdpSocket::bind(hostname).await?;

    let mut map: HashMap<String, String> = HashMap::new();

    let mut buf = [0; 1024];
    loop {
        let (len, addr) = sock.recv_from(&mut buf).await?;
        println!("{len:?} bytes received from {addr:?}");

        let inp = str::from_utf8(&buf[0..len])?;
        println!("Input: {inp}");

        if inp.contains('=') {
            let (key, value) = inp.split_once('=').unwrap();
            println!("SET: {key}={value}");
            map.insert(key.to_string(), value.to_string());
        } else {
            println!("GET");
            if inp == "version" {
                sock.send_to(b"version=PH KV Store", addr).await?;
            } else {
                let key = inp;
                let val = map.get(key);
                let out = match val {
                    Some(val) => format!("{key}={val}"),
                    None => format!("{key}="),
                };
                println!("GET: {out}, {}", out.as_bytes().len());

                sock.send_to(out.as_bytes(), addr).await?;
            }
        }
    }
}
