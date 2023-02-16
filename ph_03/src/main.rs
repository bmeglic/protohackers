use clap::Parser;
use futures::sink::SinkExt;
use std::collections::HashMap;
use std::error::Error;
use std::sync::Arc;
use tokio_stream::StreamExt;
use tokio_util::codec::Framed;
use tokio_util::codec::LinesCodec;
use tokio::net::TcpListener;
use tokio::net::TcpStream;
use tokio::sync::{mpsc, Mutex};

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

/*
#[derive(Debug)]
enum PhMsgWrite {
    WelcomeMsg,
    RoomList(Vec<User>),
    UserMsg(String),
    UserEntered(User),
    UserExited(User),
}
*/

type Tx = mpsc::UnboundedSender<String>;
type Rx = mpsc::UnboundedReceiver<String>;

#[derive(Debug)]
struct Client {
    rx: Rx,
}

impl Client {
    async fn new(username: String, state: Arc<Mutex<PhState>>) -> Client {
        let (tx, rx) = mpsc::unbounded_channel();
        state.lock().await.clients.insert(username, tx);

        Client { rx }
    }
}

#[derive(Debug)]
struct PhState {
    clients: HashMap<String, Tx>,
}

impl PhState {
    fn new() -> PhState {
        PhState {
            clients: HashMap::new(),
        }
    }

    async fn broadcast(&mut self, sender: &str, msg: &str) {
        for client in self.clients.iter_mut() {
            if *client.0 != sender {
                let _ = client.1.send(msg.into());
            }
        }
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    let args = Args::parse();

    let hostname = format!("{}:{}", args.host, args.port);
    println!("Will start listening on {hostname}");

    let listener = TcpListener::bind(hostname).await?;
    let state = Arc::new(Mutex::new(PhState::new()));

    loop {
        let (stream, _addr) = listener.accept().await?;
        let state = state.clone();

        tokio::spawn(async move {
            if let Err(e) = handle_client(stream, state).await {
                println!("Error occurred: {e:?}");
            }
        });
    }
}

async fn handle_client(
    stream: TcpStream,
    state: Arc<Mutex<PhState>>,
) -> Result<(), Box<dyn Error>> {
    println!("New connection: {}", stream.peer_addr().unwrap());

    let mut codec = Framed::new(stream, LinesCodec::new_with_max_length(2000));

    codec.send("Welcome! What is your name?").await.unwrap();

    let username = match codec.next().await {
        Some(Ok(name)) => name,
        _ => {
            println!("Failed to read username");
            return Ok(());
        }
    };

    if username.is_empty() {
        println!("Username not set, abort");
        return Ok(());
    }
    if !username.chars().all(char::is_alphanumeric) {
        println!("Username contains non-alphanumeric characters, abort");
        return Ok(());
    }

    {
        let state = state.lock().await;
        let usernames = state
            .clients
            .keys()
            .map(|s| &**s)
            .collect::<Vec<_>>()
            .join(", ");
        let msg = "* The room contains: ".to_owned() + &usernames;
        codec.send(&msg).await?;
    }

    let mut client = Client::new(username.clone(), state.clone()).await;

    {
        let mut state = state.lock().await;
        let msg = format!("* {username} has entered the room");
        state.broadcast(&username, &msg).await;
    }

    loop {
        tokio::select! {
             Some(msg) = client.rx.recv() => {
                 println!("Sending: {msg}");
                 codec.send(&msg).await?;
             },
             result = codec.next() => match result {
                 Some(Ok(msg)) => {
                     println!("Received: {msg}");
                     let mut state = state.lock().await;

                     let msg = format!("[{username}] {msg}");
                     state.broadcast(&username, &msg).await;
                 },
                 Some(Err(e)) => {
                     println!("Error: {e:?}");
                 }
                 None => break,
             },
        }
    }

    {
        println!("Closing connection");
        let mut state = state.lock().await;
        state.clients.remove(&username);

        let msg = format!("* {username} has left the room");
        state.broadcast(&username, &msg).await;
    }

    Ok(())
}
