use std::thread;
use std::{net::{TcpListener, TcpStream, Shutdown}};
use std::io::{Read, Write};
use clap::Parser;


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

fn main() {
    let args = Args::parse();

    let hostname = format!("{}:{}", args.host, args.port);
    println!("Will start listening on {}", hostname);

    let listener = TcpListener::bind(hostname).
        expect("Unable to bind to socket");

    for stream in listener.incoming() {
        match stream {
            Ok(stream) => {
                thread::spawn(|| {
                    handle_client(stream);
                });
            }
            Err(e) => {
                println!("Error: {}", e);
            }
        }
    }
}

fn handle_client(mut stream: TcpStream) {
    let mut buffer = [0; 1024];

    println!("New connection: {}", stream.peer_addr().unwrap());
    loop {
        if let Ok(read) = stream.read(&mut buffer) {
            if read == 0 {
                break;
            }
            if let Err(_) = stream.write(&buffer[0..read]) {
                stream.shutdown(Shutdown::Both).unwrap();
                break;
            }
        } else {
            break;
        }
    }
    println!("Disconnected: {}", stream.peer_addr().unwrap())
}
