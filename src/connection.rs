// use audiopus::{coder::Encoder, Application, Bitrate, Channels, SampleRate};
use crypto_secretbox::{
    aead::{AeadInPlace, KeyInit},
    XSalsa20Poly1305,
};
use std::sync::Arc;
use tokio::{net::UdpSocket, sync::Mutex};

pub struct RTPConnection {
    pub udp_socket: UdpSocket,
    secret_key: Option<Vec<u8>>,
    sequence: u16,
    timestamp: u32,
    ssrc: u32,
    // encoder: Arc<Mutex<Encoder>>,
}

impl RTPConnection {
    pub async fn new(ssrc: u32, ip: String, port: u16) -> anyhow::Result<Self> {
        let udp_socket = UdpSocket::bind("0.0.0.0:0").await?;
        udp_socket.connect(format!("{}:{}", ip, port)).await?;
        Ok(Self {
            ssrc,
            udp_socket,
            secret_key: None,
            sequence: 0,
            timestamp: 0,
            // encoder: Arc::new(Mutex::new(encoder)),
        })
    }

    pub async fn send_discovery_packet(&self, ssrc: u32) -> anyhow::Result<()> {
        let mut buffer = [0; 74];
        buffer[0..2].copy_from_slice(&1u16.to_be_bytes());
        buffer[2..4].copy_from_slice(&70u16.to_be_bytes());
        buffer[4..8].copy_from_slice(&ssrc.to_be_bytes());
        self.udp_socket.send(&buffer).await?;
        Ok(())
    }

    pub async fn recv_discovery_packet(&self) -> anyhow::Result<(String, u16)> {
        let mut buffer = [0; 74];
        self.udp_socket.recv(&mut buffer).await?;
        let address = String::from_utf8_lossy(&buffer[8..72]).to_string();
        let port = u16::from_be_bytes([buffer[72], buffer[73]]);
        Ok((address, port))
    }

    pub fn set_secret_key(&mut self, secret_key: Vec<u8>) {
        self.secret_key = Some(secret_key);
    }

    pub fn encrypt(&self, header: &[u8], mut data: Vec<u8>) -> anyhow::Result<Vec<u8>> {
        let secret_key = if let Some(secret_key) = &self.secret_key {
            secret_key
        } else {
            return Err(anyhow::anyhow!("Secret key not set"));
        };
        let mut nonce: [u8; 24] = [0; 24];
        nonce[0..12].copy_from_slice(&header);
        let cipher = XSalsa20Poly1305::new_from_slice(secret_key)?;
        cipher.encrypt_in_place(&nonce.into(), b"", &mut data)?;
        data.extend_from_slice(&nonce);
        Ok(data)
    }

    pub async fn send_voice_packet(&mut self, voice_data: Vec<u8>) -> anyhow::Result<()> {
        let secret_key = if let Some(secret_key) = &self.secret_key {
            secret_key
        } else {
            return Err(anyhow::anyhow!("Secret key not set"));
        };
        self.sequence = self.sequence.wrapping_add(1);
        let mut buffer = Vec::with_capacity(12 + voice_data.len());
        println!("voice data len: {}", voice_data.len());
        let mut header_buffer: [u8; 12] = [0; 12];
        header_buffer[0] = 0x80;
        header_buffer[1] = 0x78;
        header_buffer[2..4].copy_from_slice(&self.sequence.to_be_bytes());
        header_buffer[4..8].copy_from_slice(&self.timestamp.to_be_bytes());
        header_buffer[8..12].copy_from_slice(&self.ssrc.to_be_bytes());
        let encrpyted_voice = self.encrypt(&header_buffer, voice_data)?;
        buffer.extend_from_slice(&encrpyted_voice);
        self.udp_socket.send(&buffer).await?;
        tracing::info!("Sent voice packet");
        // add timestamp
        self.timestamp = self.timestamp.wrapping_add(960);
        Ok(())
    }
}
