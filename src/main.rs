use clap::Parser;
use primes;
use serde::Serialize;
use serde_json::{Result, Value};
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
        if let Ok((out, shutdown)) = parse_line(&line) {
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

            if shutdown == true {
                stream.shutdown(Shutdown::Both).unwrap();
                break;
            }
        } else {
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

fn parse_line(input: &String) -> Result<(String, bool)> {
    println!("inp: {}", &input);
    let json: Result<Value> = serde_json::from_str(input);

    let resp_ok = PrimeResponse {
        method: "isPrime".to_owned(),
        prime: true,
    };
    let resp_nok = PrimeResponse {
        method: "isPrime".to_owned(),
        prime: false,
    };
    let resp_malform = PrimeResponse {
        method: "Error".to_owned(),
        prime: false,
    };

    match json {
        Ok(json) => {
            //dbg!(&json);

            let method = json.get("method");
            let method = match method {
                None => {
                    return Ok((serde_json::to_string(&resp_malform).unwrap(), true));
                }
                Some(method) => method,
            };

            if method != "isPrime" {
                return Ok((serde_json::to_string(&resp_malform).unwrap(), true));
            }

            let number = json.get("number");
            let number = match number {
                None => {
                    return Ok((serde_json::to_string(&resp_malform).unwrap(), true));
                }
                Some(number) => number,
            };

            if number.is_number() != true {
                return Ok((serde_json::to_string(&resp_malform).unwrap(), true));
            }
            if number.is_u64() != true {
                return Ok((serde_json::to_string(&resp_nok).unwrap(), false));
            }

            let number = number.as_u64().unwrap();
            if primes::is_prime(number) {
                Ok((serde_json::to_string(&resp_ok).unwrap(), false))
            } else {
                Ok((serde_json::to_string(&resp_nok).unwrap(), false))
            }
        }
        Err(e) => {
            println!("Err: {}", e);
            Ok((serde_json::to_string(&resp_malform).unwrap(), true))
            //Err(e)
        }
    }
}
