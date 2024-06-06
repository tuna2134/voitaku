use serde::{Deserialize, Serialize};
use serde_json::value::RawValue;

#[derive(Serialize, Deserialize)]
#[serde(tag = "t", content = "d")]
pub enum Payload {
    #[serde(rename = "hello")]
    Hello(String),
    #[serde(rename = "voice.connection.data")]
    VoiceConnectionData(VoiceConnectionData),
}

#[derive(Serialize, Deserialize, Debug)]
pub struct VoiceConnectionData {
    pub user_id: String,
    pub session_id: String,
    pub token: String,
    pub guild_id: String,
    pub endpoint: String,
}

#[derive(Serialize, Deserialize)]
pub struct RawDiscordPayload<'a> {
    pub op: u8,
    #[serde(borrow)]
    pub d: &'a RawValue,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct DiscordHello {
    pub heartbeat_interval: f64,
}

#[derive(Serialize, Deserialize)]
pub struct DiscordHeartbeat {
    pub op: u8,
    pub d: u64,
}