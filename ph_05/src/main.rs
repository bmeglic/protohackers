use clap::Parser;
use futures::sink::SinkExt;
use std::error::Error;
use std::str::Split;
use tokio::io::AsyncWriteExt;
use tokio::net::TcpListener;
use tokio::net::TcpStream;
use tokio_stream::StreamExt;
use tokio_util::codec::Framed;
use tokio_util::codec::LinesCodec;

const UPSTREAM_HOST: &str = "chat.protohackers.com:16963";
const BOGUSCOIN: &str = "7YWHMfk9JZe0LM0g1ZauHuiSxhI";

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

    let listener = TcpListener::bind(hostname).await?;
    loop {
        let (stream, _addr) = listener.accept().await?;

        tokio::spawn(async move {
            if let Err(e) = handle_client(stream).await {
                println!("Error occurred: {e:?}");
            }
        });
    }
}

fn is_boguscoin_addr(token: &str) -> bool {
    if !token.starts_with('7') {
        return false;
    }

    if !(token.len() >= 26 && token.len() <= 35) {
        return false;
    }

    if !token.chars().all(char::is_alphanumeric) {
        return false;
    }

    true
}

fn rewrite_line(line: &str) -> String {
    let tokens: Split<&str> = line.split(" ");
    let mut out_line: String = line.to_string();

    for token in tokens {
        if is_boguscoin_addr(token) {
            out_line = out_line.replace(token, BOGUSCOIN);
        }
    }

    out_line
}

async fn handle_client(stream: TcpStream) -> Result<(), Box<dyn Error>> {
    println!("New connection: {}", stream.peer_addr().unwrap());

    let mut codec = Framed::new(stream, LinesCodec::new_with_max_length(2000));

    let upstream_stream = TcpStream::connect(UPSTREAM_HOST).await?;
    let mut upstream_codec = Framed::new(upstream_stream, LinesCodec::new_with_max_length(2000));

    loop {
        tokio::select! {
            result = upstream_codec.next() => match result {
                Some(Ok(msg)) => {
                    let peer = upstream_codec.get_ref().peer_addr();
                    if let Err(_err) = peer {
                        println!("no msg");
                        break
                    };


                    println!("Received from upstream: {msg}");

                    let out_msg = rewrite_line(&msg);

                    println!("Sending: {out_msg}");
                    codec.send(&out_msg).await?;
                },
                Some(Err(e)) => {
                    println!("Error: {e:?}");
                },
                None => break,
            },
            result = codec.next() => match result {
                Some(Ok(msg)) => {

                    let peer = codec.get_ref().peer_addr();
                    if let Err(_err) = peer {
                        println!("no msg");
                        break
                    };

                    println!("Received: {msg}");

                    let out_msg = rewrite_line(&msg);

                    println!("Sending: {out_msg}");
                    upstream_codec.send(&out_msg).await?;
                },
                Some(Err(e)) => {
                    println!("Error: {e:?}");
                },
                None => {
                    break;
                }
            },
        }
    }

    if let Ok(_ok) = codec.get_ref().peer_addr() {
        codec.get_mut().shutdown().await?;
    }
    if let Ok(_ok) = upstream_codec.get_ref().peer_addr() {
        upstream_codec.get_mut().shutdown().await?;
    }

    Ok(())
}
