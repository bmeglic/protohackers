use tokio::time::Duration;

use crate::codec;
use crate::server::*;

pub async fn run(interval: u32, tx: MsgTx) {
    let interval = Duration::from_millis((interval * 100).into());
    let mut interval = tokio::time::interval(interval);

    interval.tick().await; /* first call to tick() expires immediately */
    loop {
        interval.tick().await;

        if tx.is_closed() {
            //println!("Closing heartbeat");
            break;
        }

        //println!("Sending heartbeat");
        if let Err(e) = tx.send(codec::SDMessage::Heartbeat) {
            println!("{e:?}");
            break;
        }
    }
}
