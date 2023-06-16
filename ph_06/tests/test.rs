use futures::SinkExt;
use tokio::net::TcpStream;
use tokio::time::{sleep, Duration};
use tokio_stream::StreamExt;
use tokio_util::codec::Framed;

use ph_06::codec::SpeedDaemonCodec;
use ph_06::codec::*;
use ph_06::server::SpeedDaemonServer;

async fn spawn_app() {
    let server = SpeedDaemonServer::new();
    tokio::spawn(async move {
        let _ = server.run("127.0.0.1:7777".to_string()).await;
    });
    sleep(Duration::from_millis(100)).await;
}

#[tokio::test]
async fn test_example_session() {
    spawn_app().await;

    let stream = TcpStream::connect("127.0.0.1:7777").await.unwrap();
    let mut codec_camera1 = Framed::new(stream, SpeedDaemonCodec::new());
    let stream = TcpStream::connect("127.0.0.1:7777").await.unwrap();
    let mut codec_camera2 = Framed::new(stream, SpeedDaemonCodec::new());
    let stream = TcpStream::connect("127.0.0.1:7777").await.unwrap();
    let mut codec_dispatcher = Framed::new(stream, SpeedDaemonCodec::new());

    /* send iamdispatcher */
    let iamdispatcher = SDMessage::IAmDispatcher { roads: vec![123] };
    assert_eq!(codec_dispatcher.send(iamdispatcher).await.is_ok(), true);
    sleep(Duration::from_millis(100)).await;

    /* send iamcamera1 and plates */
    let iamcamera = SDMessage::IAmCamera {
        road: 123,
        mile: 8,
        limit: 60,
    };
    assert_eq!(codec_camera1.send(iamcamera).await.is_ok(), true);
    let plate = SDMessage::Plate {
        plate: "UN1X".to_string(),
        timestamp: 0,
    };
    assert_eq!(codec_camera1.send(plate).await.is_ok(), true);

    /* send iamcamera2 and plates */
    let iamcamera = SDMessage::IAmCamera {
        road: 123,
        mile: 9,
        limit: 60,
    };
    assert_eq!(codec_camera2.send(iamcamera).await.is_ok(), true);
    let plate = SDMessage::Plate {
        plate: "UN1X".to_string(),
        timestamp: 45,
    };
    assert_eq!(codec_camera2.send(plate).await.is_ok(), true);
    sleep(Duration::from_millis(100)).await;

    /* expect a ticket */
    let msg = codec_dispatcher.next().await;
    println!("recv msg: {msg:?}");
    match msg {
        Some(Ok(SDMessage::Ticket {
            plate,
            road,
            mile1,
            timestamp1,
            mile2,
            timestamp2,
            speed,
        })) => {
            assert_eq!(plate, "UN1X".to_string());
            assert_eq!(road, 123);
            assert_eq!(mile1, 8);
            assert_eq!(timestamp1, 0);
            assert_eq!(mile2, 9);
            assert_eq!(timestamp2, 45);
            assert_eq!(speed, 8000);
        }
        _ => {
            panic!("Did not receive correct message: {msg:?}");
        }
    }
}

#[tokio::test]
async fn test_same_day_ticket() {
    spawn_app().await;

    let stream = TcpStream::connect("127.0.0.1:7777").await.unwrap();
    let mut codec_camera1 = Framed::new(stream, SpeedDaemonCodec::new());
    let stream = TcpStream::connect("127.0.0.1:7777").await.unwrap();
    let mut codec_camera2 = Framed::new(stream, SpeedDaemonCodec::new());
    let stream = TcpStream::connect("127.0.0.1:7777").await.unwrap();
    let mut codec_camera3 = Framed::new(stream, SpeedDaemonCodec::new());
    let stream = TcpStream::connect("127.0.0.1:7777").await.unwrap();
    let mut codec_dispatcher = Framed::new(stream, SpeedDaemonCodec::new());

    /* send iamdispatcher */
    let iamdispatcher = SDMessage::IAmDispatcher { roads: vec![4654] };
    assert_eq!(codec_dispatcher.send(iamdispatcher).await.is_ok(), true);
    sleep(Duration::from_millis(100)).await;

    /* send iamcamera1 and plates */
    let iamcamera = SDMessage::IAmCamera {
        road: 4654,
        mile: 1147,
        limit: 80,
    };
    assert_eq!(codec_camera1.send(iamcamera).await.is_ok(), true);
    let plate = SDMessage::Plate {
        plate: "ET78NYD".to_string(),
        timestamp: 57338624,
    };
    assert_eq!(codec_camera1.send(plate).await.is_ok(), true);

    /* send iamcamera2 and plates */
    let iamcamera = SDMessage::IAmCamera {
        road: 4654,
        mile: 1163,
        limit: 80,
    };
    assert_eq!(codec_camera2.send(iamcamera).await.is_ok(), true);
    let plate = SDMessage::Plate {
        plate: "ET78NYD".to_string(),
        timestamp: 57338325,
    };
    assert_eq!(codec_camera2.send(plate).await.is_ok(), true);
    sleep(Duration::from_millis(100)).await;

    /* send iamcamera3 and plates */
    let iamcamera = SDMessage::IAmCamera {
        road: 4654,
        mile: 1155,
        limit: 80,
    };
    assert_eq!(codec_camera3.send(iamcamera).await.is_ok(), true);
    let plate = SDMessage::Plate {
        plate: "ET78NYD".to_string(),
        timestamp: 57338929,
    };
    assert_eq!(codec_camera3.send(plate).await.is_ok(), true);
    sleep(Duration::from_millis(100)).await;

    /* expect a ticket */
    let msg = codec_dispatcher.next().await;
    println!("recv msg: {msg:?}");
    match msg {
        Some(Ok(SDMessage::Ticket {
            plate,
            road,
            mile1,
            timestamp1,
            mile2,
            timestamp2,
            speed,
        })) => {
            assert_eq!(plate, "ET78NYD".to_string());
            assert_eq!(road, 4654);
            assert_eq!(mile1, 1163);
            assert_eq!(timestamp1, 57338325);
            assert_eq!(mile2, 1147);
            assert_eq!(timestamp2, 57338624);
            assert_eq!(speed, 19264);
        }
        _ => {
            panic!("Did not receive correct message: {msg:?}");
        }
    }

    sleep(Duration::from_millis(100)).await;
    let msg = codec_dispatcher.next().await;
    dbg!(&msg);
    if msg.is_some() {
        panic!("Should not receive 2 tickets");
    }
}
