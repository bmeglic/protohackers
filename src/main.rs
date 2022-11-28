use byteorder::{ByteOrder, NetworkEndian};
use clap::Parser;
use std::io::{Read, Write, ErrorKind};
use std::net::{Shutdown, TcpListener, TcpStream};
use std::thread;
use std::error::Error;
use std::collections::BTreeMap;


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

#[derive(Debug)]
enum Msg {
    Insert { timestamp: i32, price: i32 },
    Query { time_min: i32, time_max: i32 },
}

impl Msg {
    fn parse(stream: &mut TcpStream) -> Result<Msg, Box<dyn Error>> {
        let mut buf = vec![0u8; 9];
        let mut handle = stream.take(9);

        handle.read_exact(&mut buf)?;

        let msg_type = char::from_u32(buf[0] as u32).unwrap_or('0');
        let val1 = NetworkEndian::read_i32(&buf[1..5]);
        let val2 = NetworkEndian::read_i32(&buf[5..]);

        match msg_type {
            'I' => Ok(Msg::Insert { timestamp: val1, price: val2 }),
            'Q' => Ok(Msg::Query { time_min: val1, time_max: val2 }),
            _ => Err(Box::new(std::io::Error::new(ErrorKind::Other, "Unknown command"))),
        }
    }
}


fn write_result(stream: &mut TcpStream, val: i32) -> Result<(), Box<dyn Error>> {
    let mut buf = vec![0u8; 4];
    NetworkEndian::write_i32(&mut buf, val);
    stream.write(&buf)?;
    stream.flush()?;
    Ok(())
}

fn handle_client(stream: TcpStream) {
    println!("New connection: {}", stream.peer_addr().unwrap());

    let mut stream = stream;
    let mut prices: BTreeMap<i32, i32> = BTreeMap::new();

    loop {

        let msg = Msg::parse(&mut stream);
        match msg {
            Ok(msg) => {
                //dbg!(&msg);
                match msg {
                    Msg::Insert { timestamp, price } => {
                        println!("Processing insert: {}:{}", timestamp, price);
                        prices.insert(timestamp, price);
                    },
                    Msg::Query { time_min, time_max } => {
                        println!("Processing query: {}:{}", time_min, time_max);

                        let mut sum: i64 = 0;
                        let mut cnt = 0;

                        if time_min > time_max {
                            println!("mintime > maxtime");
                        }
                        else {
                            for (key, value) in prices.iter() {
                                if time_min <= *key && time_max >= *key {
                                    sum = sum + *value as i64;
                                    cnt += 1;
                                }
                            }
                        }
    
                        let val: i32 = if cnt > 0 { (sum/cnt) as i32 } else { 0 };
                        if let Err(e) = write_result(&mut stream, val) {
                            println!("Err: {}", e);
                            break;
                        }
                    },
                };
            }
            Err(e) => {
                println!("Err: {}", e);
                break;
            }
        }
    }

    println!("Client disconnected");
    stream.shutdown(Shutdown::Write).unwrap_or(())
}
