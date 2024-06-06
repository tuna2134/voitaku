use axum::{
    extract::{
        ws::{Message, WebSocket},
        State, WebSocketUpgrade,
    },
    response::IntoResponse,
    routing::get,
    Router,
};
use futures_util::{SinkExt, StreamExt, TryStreamExt};
use tokio::net::TcpListener;
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
    ws.on_upgrade(move |socket| wrapper(socket))
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
    let mut discord_ws = {
        let url = format!(
            "wss://{}/?v=4",
            data.endpoint.trim_start_matches("http://").trim_start_matches("https://")
        );
        tracing::info!("Connecting to Discord Voice Server: {}", url);
        let (ws, _) = tokio_tungstenite::connect_async(url).await?;
        ws
    };
    let heartbeat_interval = loop {
        let msg = discord_ws.next().await;
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
        let payload: types::RawDiscordPayload = serde_json::from_str(&msg)?;
        if payload.op == 8 {
            let hello: types::DiscordHello = serde_json::from_str(payload.d.get())?;
            break hello.heartbeat_interval;
        }
    };
    tokio::spawn(async move {
        let mut last_sequence = 0;
        loop {
            tokio::time::sleep(tokio::time::Duration::from_secs_f64(heartbeat_interval)).await;
            last_sequence += 1;
            let result = discord_ws
                .send(json_to_tmsg(&types::DiscordHeartbeat {
                    op: 1,
                    d: last_sequence,
                }).unwrap())
                .await;
            if let Err(e) = result {
                tracing::error!("{:?}", e);
                break;
            }
        }
    });
    Ok(())
}
