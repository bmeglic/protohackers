use anyhow::{anyhow, Error, Result};
use clap::Parser;
use primes;
use serde::Serialize;
use serde_json;
use serde_json::Value;
use std::io::{BufRead, BufReader, BufWriter, Write};
use std::net::{Shutdown, TcpListener, TcpStream};
use std::thread;

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

    let listener = TcpListener::bind(hostname).expect("Unable to bind to socket");

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

fn handle_client(stream: TcpStream) {
    println!("New connection: {}", stream.peer_addr().unwrap());

    let reader = BufReader::new(&stream);
    let mut writer = BufWriter::new(&stream);

    for line in reader.lines() {
        dbg!(&line);

        let line = &line.unwrap();
        if let Ok(out) = parse_line(&line) {
            //dbg!(&out);
            println!("out: {}", &out);

            if let Err(_) = writer.write(&out.as_bytes()) {
                stream.shutdown(Shutdown::Both).unwrap();
                break;
            }

            if let Err(_) = writer.write(b"\n") {
                stream.shutdown(Shutdown::Both).unwrap();
                break;
            }
            writer.flush().unwrap();
        } else {
            if let Err(_) = writer.write(b"\n") {
                stream.shutdown(Shutdown::Both).unwrap();
                break;
            }
            writer.flush().unwrap();
            stream.shutdown(Shutdown::Both).unwrap();
            break;
        }
    }

    println!("Client disconnected");
    stream.shutdown(Shutdown::Write).unwrap_or(())
}

#[derive(Serialize)]
struct PrimeResponse {
    method: String,
    prime: bool,
}

fn parse_line(input: &String) -> Result<String> {
    println!("inp: {}", &input);
    let json: serde_json::Result<Value> = serde_json::from_str(input);

    let resp_ok = PrimeResponse {
        method: "isPrime".to_owned(),
        prime: true,
    };
    let resp_nok = PrimeResponse {
        method: "isPrime".to_owned(),
        prime: false,
    };

    match json {
        Ok(json) => {
            //dbg!(&json);

            let method = json
                .get("method")
                .ok_or_else(|| anyhow!("Field method not found"))?;
            if method != "isPrime" {
                return Err(anyhow!("Field method is not isPrime"));
            }

            let number = json
                .get("number")
                .ok_or_else(|| anyhow!("Field number not found"))?;
            if number.is_number() != true {
                return Err(anyhow!("Field number is not a number"));
            }

            if number.is_u64() != true {
                return Ok(serde_json::to_string(&resp_nok).unwrap());
            }

            if primes::is_prime(number.as_u64().unwrap()) {
                Ok(serde_json::to_string(&resp_ok).unwrap())
            } else {
                Ok(serde_json::to_string(&resp_nok).unwrap())
            }
        }
        Err(e) => {
            println!("Err: {}", e);
            Err(Error::new(e))
        }
    }
}
