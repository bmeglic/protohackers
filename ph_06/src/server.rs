use futures::sink::SinkExt;
use std::cmp::Ordering;
use std::collections::hash_map::Entry;
use std::collections::HashMap;
use std::collections::HashSet;
use std::error::Error;
use std::hash::Hash;
use std::sync::Arc;
use tokio::net::TcpListener;
use tokio::net::TcpStream;
use tokio::sync::mpsc;
use tokio::sync::Mutex;
use tokio_stream::StreamExt;
use tokio_util::codec::Framed;

use crate::codec::SDMessage;
use crate::codec::SpeedDaemonCodec;
use crate::codec::SpeedDaemonCodecError;
use crate::heartbeat;

pub type MsgTx = mpsc::UnboundedSender<SDMessage>;
type MsgRx = mpsc::UnboundedReceiver<SDMessage>;

type Road = u16;
type Limit = u16;
type Mile = u16;
type Timestamp = u32;
type Plate = String;
type Day = u32;

#[derive(Debug, Eq, PartialEq, Hash)]
struct PlateReportKey {
    road: Road,
    plate: Plate,
}

#[derive(Debug)]
struct PlateReport {
    timestamp: Timestamp,
    mile: Mile,
}

#[derive(Debug)]
pub struct AppState {
    dispatchers_on_road: HashMap<Road, Vec<MsgTx>>,
    pending_tickets: HashMap<Road, Vec<SDMessage>>,
    cars_on_road: HashMap<(Road, Plate), Vec<PlateReport>>,
    tickets_per_day: HashSet<(Plate, Day)>,
}

fn add_pending_ticket(
    pending_tickets: &mut HashMap<Road, Vec<SDMessage>>,
    road: Road,
    ticket: SDMessage,
) {
    pending_tickets.entry(road).or_default().push(ticket);
}

impl AppState {
    fn new() -> AppState {
        AppState {
            dispatchers_on_road: HashMap::new(),
            pending_tickets: HashMap::new(),
            cars_on_road: HashMap::new(),
            tickets_per_day: HashSet::new(),
        }
    }

    fn add_dispatcher(&mut self, client: &Client) {
        if let Some(dispatcher) = &client.ticket_dispatcher {
            for road in &dispatcher.roads {
                self.dispatchers_on_road
                    .entry(*road)
                    .or_insert_with(Vec::new)
                    .push(client.tx.clone());

                /* send pending ticket if there are any for this road */
                if let Entry::Occupied(o) = self.pending_tickets.entry(*road) {
                    for ticket in o.remove_entry().1 {
                        println!("Sending pending ticket");
                        if let Err(e) = client.tx.send(ticket) {
                            println!("Err: {e:?}");
                        }
                    }
                }
            }
        }
    }

    fn remove_dispatcher(&mut self, client: &Client) {
        if let Some(dispatcher) = &client.ticket_dispatcher {
            for road in &dispatcher.roads {
                self.dispatchers_on_road.entry(*road).and_modify(|arr| {
                    if let Some(idx) = arr.iter().position(|x| x.same_channel(&client.tx)) {
                        arr.swap_remove(idx);
                    }
                });
            }
        }
    }

    fn report_plate(&mut self, client: &Client, plate: Plate, timestamp: Timestamp) {
        let camera = client.camera.as_ref().unwrap();

        let road = camera.road;
        let mile = camera.mile;

        let key = (road, plate.clone());
        let val = PlateReport {
            timestamp,
            mile: client.camera.as_ref().unwrap().mile,
        };

        let reports = self.cars_on_road.entry(key).or_default();
        for report in &mut *reports {
            let (timestamp1, timestamp2, mile1, mile2) = match timestamp.cmp(&report.timestamp) {
                Ordering::Greater => (report.timestamp, timestamp, report.mile, mile),
                Ordering::Less => (timestamp, report.timestamp, mile, report.mile),
                Ordering::Equal => todo!(),
            };
            let len_diff = match mile.cmp(&report.mile) {
                Ordering::Greater => mile - report.mile,
                Ordering::Less => report.mile - mile,
                Ordering::Equal => 0,
            };

            let ts_diff = timestamp2 - timestamp1;
            let speed = (len_diff as f32 / ts_diff as f32) * 3600.0;

            if speed > camera.limit.into() {
                println!(
                    "Issuing a ticket for {plate} on {road}; speed: {speed}; limit: {0}",
                    camera.limit
                );

                let ticket_start_day = timestamp1 / 86400;
                let ticket_end_day = timestamp2 / 86400;

                if self
                    .tickets_per_day
                    .contains(&(plate.clone(), ticket_start_day))
                    || self
                        .tickets_per_day
                        .contains(&(plate.clone(), ticket_end_day))
                {
                    println!("Already ticketed");
                    continue;
                }

                for day in ticket_start_day..=ticket_end_day {
                    self.tickets_per_day.insert((plate.clone(), day));
                }

                let ticket = SDMessage::Ticket {
                    plate: plate.clone(),
                    road,
                    mile1,
                    timestamp1,
                    mile2,
                    timestamp2,
                    speed: (speed * 100.0) as u16,
                };
                if let Some(dispatchers) = self.dispatchers_on_road.get(&road) {
                    let dispatcher = dispatchers.first().unwrap();
                    if dispatcher.is_closed() {
                        println!("Add to pending tickets");
                        add_pending_ticket(&mut self.pending_tickets, road, ticket);
                    } else if let Err(e) = dispatcher.send(ticket) {
                        println!("{e:?}");
                    }
                } else {
                    println!("Add to pending tickets");
                    add_pending_ticket(&mut self.pending_tickets, road, ticket);
                }
            }
        }

        reports.push(val);
    }
}

#[derive(Debug, PartialEq)]
struct Camera {
    road: Road,
    mile: Mile,
    limit: Limit,
}

#[derive(Debug, PartialEq)]
struct TicketDispatcher {
    roads: Vec<Road>,
}

#[derive(Debug, PartialEq)]
enum ClientType {
    Unknown,
    Camera,
    TicketDispatcher,
}

#[derive(Debug)]
struct Client {
    typ: ClientType,
    camera: Option<Camera>,
    ticket_dispatcher: Option<TicketDispatcher>,
    rx: MsgRx,
    tx: MsgTx,
    is_heartbeat_running: bool,
}

impl Client {
    fn new() -> Client {
        let (tx, rx) = mpsc::unbounded_channel();

        Client {
            typ: ClientType::Unknown,
            camera: None,
            ticket_dispatcher: None,
            rx,
            tx,
            is_heartbeat_running: false,
        }
    }

    async fn send_error(
        self: &mut Client,
        codec: &mut Framed<TcpStream, SpeedDaemonCodec>,
        msg: String,
    ) -> Result<(), Box<dyn Error>> {
        codec.send(SDMessage::Error { msg }).await?;

        Ok(())
    }

    /* TODO: convert result error to something else. anyhow? thiserror? */

    async fn process_msg(
        self: &mut Client,
        msg: Option<Result<SDMessage, SpeedDaemonCodecError>>,
        state: &Arc<Mutex<AppState>>,
        codec: &mut Framed<TcpStream, SpeedDaemonCodec>,
    ) -> Result<(), Box<dyn Error>> {
        match msg {
            Some(Ok(SDMessage::IAmCamera { road, mile, limit })) => {
                if self.typ != ClientType::Unknown {
                    self.send_error(codec, "Client already identified".to_string())
                        .await?;
                    return Ok(());
                }

                self.typ = ClientType::Camera;
                self.camera = Some(Camera { road, mile, limit });
            }
            Some(Ok(SDMessage::IAmDispatcher { roads })) => {
                if self.typ != ClientType::Unknown {
                    self.send_error(codec, "Client already identified".to_string())
                        .await?;
                    return Ok(());
                }

                self.typ = ClientType::TicketDispatcher;
                self.ticket_dispatcher = Some(TicketDispatcher { roads });

                state.lock().await.add_dispatcher(self);
            }
            Some(Ok(SDMessage::Plate { plate, timestamp })) => {
                if self.typ != ClientType::Camera {
                    self.send_error(codec, "Client is not a camera".to_string())
                        .await?;
                    return Ok(());
                }

                state.lock().await.report_plate(self, plate, timestamp);
            }
            Some(Ok(SDMessage::WantHeartbeat { interval })) => {
                if self.is_heartbeat_running {
                    self.send_error(codec, "Client already requested heartbeats".to_string())
                        .await?;
                    return Ok(());
                }
                if interval > 0 {
                    self.is_heartbeat_running = true;
                    let tx = self.tx.clone();

                    /* TODO: how to cleanup when client disconnects? */
                    tokio::spawn(async move {
                        heartbeat::run(interval, tx).await;
                    });
                }
            }
            _ => {
                println!("Error processing message");
                self.send_error(codec, "Error processing message".to_string())
                    .await?;
            }
        }

        Ok(())
    }
}

#[derive(Debug, Default)]
pub struct SpeedDaemonServer {}

impl SpeedDaemonServer {
    pub fn new() -> SpeedDaemonServer {
        SpeedDaemonServer::default()
    }

    pub async fn run(self, hostname: String) -> Result<(), Box<dyn Error>> {
        let listener = TcpListener::bind(hostname).await?;
        let state = Arc::new(Mutex::new(AppState::new()));

        loop {
            let (stream, _addr) = listener.accept().await?;
            let state = state.clone();

            tokio::spawn(async move {
                if let Err(e) = SpeedDaemonServer::handle_client(stream, state).await {
                    println!("Error occurred: {e:?}");
                }
            });
        }
    }

    pub async fn handle_client(
        stream: TcpStream,
        state: Arc<Mutex<AppState>>,
    ) -> Result<(), Box<dyn Error>> {
        println!("New connection: {}", stream.peer_addr().unwrap());

        let mut codec = Framed::new(stream, SpeedDaemonCodec::new());
        let mut client = Client::new();

        loop {
            tokio::select! {
                msg = codec.next() => match msg {
                    Some(Ok(msg)) => {
                        println!("Received message: {msg:?}");

                        client.process_msg(Some(Ok(msg)), &state, &mut codec).await?;
                    },
                    Some(Err(e)) => {
                        let _ = client.send_error(&mut codec, "Unknown message type".to_string()).await;
                        break;
                    }
                    None => {
                        break;
                    }
                },
                msg = client.rx.recv() => match msg {
                    Some(msg) => {
                        println!("Sending message: {msg:?}");
                        codec.send(msg).await?;
                    },
                    None => {
                        break;
                    }
                }
            }
        }

        state.lock().await.remove_dispatcher(&client);

        Ok(())
    }
}
