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
pub struct RawDiscordRecvPayload<'a> {
    pub op: u8,
    #[serde(borrow)]
    pub d: &'a RawValue,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct DiscordHello {
    pub heartbeat_interval: f64,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct DiscordReady {
    pub ssrc: u32,
    pub ip: String,
    pub port: u16,
    pub modes: Vec<String>,
}

#[derive(Serialize, Deserialize)]
pub struct DiscordHeartbeat {
    pub op: u8,
    pub d: u128,
}

#[derive(Serialize, Deserialize)]
pub struct DiscordIdentify {
    pub op: u8,
    pub d: DiscordIdentifyData,
}

#[derive(Serialize, Deserialize)]
pub struct DiscordIdentifyData {
    pub server_id: String,
    pub user_id: String,
    pub session_id: String,
    pub token: String,
}

#[derive(Serialize, Deserialize)]
pub struct DiscordSelectProtocol {
    pub op: u8,
    pub d: DiscordSelectProtocolData,
}

#[derive(Serialize, Deserialize)]
pub struct DiscordSelectProtocolData {
    pub protocol: String,
    pub data: DiscordSelectProtocolDataInfo,
}

#[derive(Serialize, Deserialize)]
pub struct DiscordSelectProtocolDataInfo {
    pub address: String,
    pub port: u16,
    pub mode: String,
}
