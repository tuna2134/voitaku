use axum::{
    extract::{
        ws::{Message, WebSocket},
        State, WebSocketUpgrade,
    },
    response::IntoResponse,
    routing::get,
    Router,
};
use futures_util::{SinkExt, StreamExt};
use std::{
    sync::Arc,
    time::{SystemTime, UNIX_EPOCH},
};
use tokio::{
    net::{TcpListener, UdpSocket},
    sync::{Mutex},
};
use tokio_tungstenite::tungstenite;
use tracing_subscriber::{fmt::time::LocalTime, EnvFilter};

mod types;
use types::Payload;

fn json_to_msg<T>(data: &T) -> anyhow::Result<Message>
where
    T: ?Sized + serde::Serialize,
{
    let msg = serde_json::to_string(&data)?;
    Ok(Message::Text(msg))
}

fn json_to_tmsg<T>(data: &T) -> anyhow::Result<tungstenite::Message>
where
    T: ?Sized + serde::Serialize,
{
    let msg = serde_json::to_string(&data)?;
    Ok(tungstenite::Message::Text(msg))
}

#[derive(Clone)]
struct AppState {}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing::info!("Now booting...");
    dotenvy::dotenv().ok();
    tracing_subscriber::fmt()
        .with_timer(LocalTime::rfc_3339())
        .with_env_filter(EnvFilter::from_default_env())
        .with_file(true)
        .with_line_number(true)
        .init();
    let listener = TcpListener::bind("0.0.0.0:8000").await?;

    let app = Router::new()
        .route("/", get(|| async { "Hello, World!" }))
        .route("/ws", get(ws_handle))
        .with_state(AppState {});

    axum::serve(listener, app).await?;

    Ok(())
}

async fn ws_handle(ws: WebSocketUpgrade, State(state): State<AppState>) -> impl IntoResponse {
    ws.on_upgrade(wrapper)
}

async fn wrapper(ws: WebSocket) {
    if let Err(e) = handle_socket(ws).await {
        tracing::error!("{:?}", e);
    }
}

async fn handle_socket(ws: WebSocket) -> anyhow::Result<()> {
    let (mut write, mut read) = ws.split();
    write
        .send(json_to_msg(&Payload::Hello(
            "Hello, please send the required data to the voice server.".to_string(),
        ))?)
        .await?;
    let data = loop {
        let msg = read.next().await;
        let msg = match msg {
            Some(Ok(msg)) => {
                if let Message::Text(msg) = msg {
                    msg
                } else {
                    continue;
                }
            }
            Some(Err(e)) => return Err(e.into()),
            None => return Err(anyhow::anyhow!("No message received.")),
        };
        let payload: types::Payload = serde_json::from_str(&msg)?;
        if let Payload::VoiceConnectionData(data) = payload {
            break data;
        }
    };
    tracing::info!("{:?}", data);
    let (sender, mut receiver) = {
        let url = format!(
            "wss://{}/?v=4",
            data.endpoint
                .trim_start_matches("http://")
                .trim_start_matches("https://")
        );
        tracing::info!("Connecting to Discord Voice Server: {}", url);
        let (ws, _) = tokio_tungstenite::connect_async(url).await?;
        ws.split()
    };
    let sender = Arc::new(Mutex::new(sender));
    let heartbeat_interval = loop {
        let msg = receiver.next().await;
        let msg = match msg {
            Some(Ok(msg)) => {
                if let tungstenite::Message::Text(msg) = msg {
                    msg
                } else {
                    continue;
                }
            }
            Some(Err(e)) => return Err(e.into()),
            None => return Err(anyhow::anyhow!("No message received.")),
        };
        let payload: types::RawDiscordRecvPayload = serde_json::from_str(&msg)?;
        if payload.op == 8 {
            let mut sender_lock = sender.lock().await;
            let result = sender_lock
                .send(
                    json_to_tmsg(&types::DiscordIdentify {
                        op: 0,
                        d: types::DiscordIdentifyData {
                            server_id: data.guild_id,
                            user_id: data.user_id,
                            session_id: data.session_id,
                            token: data.token,
                        },
                    })
                    .unwrap(),
                )
                .await;
            if let Err(e) = result {
                return Err(e.into());
            }
            let hello: types::DiscordHello = serde_json::from_str(payload.d.get())?;
            tracing::info!("{:?}", hello);
            break hello.heartbeat_interval;
        }
    };
    let sender_clone = Arc::clone(&sender);
    tokio::spawn(async move {
        let mut last_sequence = 0;
        loop {
            tracing::info!("Sending heartbeat...");
            last_sequence += 1;
            let now = {
                // get unixtime (*1000)
                let now = SystemTime::now();
                let since_epoch = now.duration_since(UNIX_EPOCH).expect("Time went backwards");
                since_epoch.as_millis()
            };
            tracing::info!("Heartbeat: {}", now);
            let mut sender_lock = sender_clone.lock().await;
            let result = sender_lock
                .send(json_to_tmsg(&types::DiscordHeartbeat { op: 3, d: now }).unwrap())
                .await;
            tracing::info!("Heartbeat sent: {}", last_sequence);
            if let Err(e) = result {
                tracing::error!("{:?}", e);
                break;
            }
            tokio::time::sleep(tokio::time::Duration::from_secs_f64(
                heartbeat_interval / 1000.0,
            ))
            .await;
        }
    });
    let mut socket = None;
    loop {
        tokio::select! {
            Some(msg) = async {
                receiver.next().await
            } => {
                let msg = match msg? {
                    tungstenite::Message::Text(msg) => msg,
                    _ => continue,
                };
                println!("{}", msg);
                let payload: types::RawDiscordRecvPayload = serde_json::from_str(&msg)?;
                match payload.op {
                    2 => {
                        let ready: types::DiscordReady = serde_json::from_str(payload.d.get())?;
                        tracing::info!("{:?}", ready);
                        socket = {
                            let udp_socket = UdpSocket::bind("0.0.0.0:0").await?;
                            udp_socket.connect(format!("{}:{}", ready.ip, ready.port)).await?;
                            // send discovery packet
                            let mut buffer = [0; 74];
                            buffer[0..2].copy_from_slice(&1u16.to_be_bytes());
                            buffer[2..4].copy_from_slice(&70u16.to_be_bytes());
                            buffer[4..8].copy_from_slice(&ready.ssrc.to_be_bytes());
                            udp_socket.send(&buffer).await?;
                            let mut buffer = [0; 74];
                            udp_socket.recv(&mut buffer).await?;
                            let address = String::from_utf8_lossy(&buffer[8..72]).to_string();
                            let port = u16::from_be_bytes([buffer[72], buffer[73]]);
                            tracing::info!("Address: {}, Port: {}", address, port);
                            let mut sender_lock = sender.lock().await;
                            sender_lock.send(json_to_tmsg(&types::DiscordSelectProtocol {
                                op: 1,
                                d: types::DiscordSelectProtocolData {
                                    protocol: "udp".to_string(),
                                    data: types::DiscordSelectProtocolDataInfo {
                                        address,
                                        port,
                                        mode: "xsalsa20_poly1305".to_string(),
                                    }
                                }
                            })?).await?;
                            Some(udp_socket)
                        }
                    }
                    _ => {}
                }
            }

        }
    }
    Ok(())
}
