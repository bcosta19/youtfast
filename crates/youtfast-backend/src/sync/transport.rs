//! An authenticated, encrypted snapshot exchange. Both sides complete a
//! Noise PSK handshake (empty payloads only) before sending packet data.

use snow::{Builder, HandshakeState, TransportState};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;
use youtfast_storage::sync::{
    ERR_INVALID_RESPONSE, ERR_KEY_REJECTED, ERR_SIZE_LIMIT, ERR_UNREACHABLE, MAX_SYNC_BYTES,
    PAIRING_KEY_BYTES,
};

const PROTOCOL: &str = "Noise_NNpsk0_25519_ChaChaPoly_SHA256";
const PROLOGUE: &[u8] = b"YouTFast device sync v2";
// Each NNpsk0 message is a 32-byte ephemeral public key and an empty,
// authenticated payload. No lengths or application fields precede it.
const HANDSHAKE_BYTES: usize = 48;
const TAG_BYTES: usize = 16;
const CHUNK_BYTES: usize = 32 * 1024;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
pub(super) enum PacketKind {
    Request = 1,
    Ok = 2,
    Invalid = 3,
    Incompatible = 4,
    TooLarge = 5,
}

impl PacketKind {
    fn decode(code: u8) -> Result<Self, &'static str> {
        match code {
            1 => Ok(Self::Request),
            2 => Ok(Self::Ok),
            3 => Ok(Self::Invalid),
            4 => Ok(Self::Incompatible),
            5 => Ok(Self::TooLarge),
            _ => Err(ERR_INVALID_RESPONSE),
        }
    }
}

pub(super) struct SecurePeer {
    stream: TcpStream,
    transport: TransportState,
}

impl SecurePeer {
    pub(super) async fn handshake(
        mut stream: TcpStream,
        key: &[u8; PAIRING_KEY_BYTES],
        initiator: bool,
    ) -> Result<Self, &'static str> {
        let parameters = PROTOCOL.parse().map_err(|_| ERR_INVALID_RESPONSE)?;
        let builder = Builder::new(parameters)
            .prologue(PROLOGUE)
            .and_then(|builder| builder.psk(0, key))
            .map_err(|_| ERR_INVALID_RESPONSE)?;
        let mut handshake = if initiator {
            builder.build_initiator()
        } else {
            builder.build_responder()
        }
        .map_err(|_| ERR_INVALID_RESPONSE)?;

        if initiator {
            send_handshake(&mut stream, &mut handshake).await?;
            receive_handshake(&mut stream, &mut handshake).await?;
        } else {
            receive_handshake(&mut stream, &mut handshake).await?;
            send_handshake(&mut stream, &mut handshake).await?;
        }
        let transport = handshake
            .into_transport_mode()
            .map_err(|_| ERR_KEY_REJECTED)?;
        Ok(Self { stream, transport })
    }

    pub(super) async fn send(&mut self, kind: PacketKind, body: &[u8]) -> Result<(), &'static str> {
        if body.len() as u64 > MAX_SYNC_BYTES {
            return Err(ERR_SIZE_LIMIT);
        }
        let mut header = [0; 5];
        header[0] = kind as u8;
        header[1..].copy_from_slice(&(body.len() as u32).to_be_bytes());
        self.write_record(&header, &mut [0; 5 + TAG_BYTES]).await?;
        let mut ciphertext = vec![0; body.len().min(CHUNK_BYTES) + TAG_BYTES];
        for chunk in body.chunks(CHUNK_BYTES) {
            self.write_record(chunk, &mut ciphertext).await?;
        }
        self.stream.flush().await.map_err(|_| ERR_UNREACHABLE)
    }

    pub(super) async fn receive(&mut self) -> Result<(PacketKind, Vec<u8>), &'static str> {
        let mut header = [0; 5];
        self.read_record(&mut header, &mut [0; 5 + TAG_BYTES])
            .await?;
        let [kind, a, b, c, d] = header;
        let kind = PacketKind::decode(kind)?;
        let len = u32::from_be_bytes([a, b, c, d]);
        if u64::from(len) > MAX_SYNC_BYTES {
            return Err(ERR_SIZE_LIMIT);
        }
        if !matches!(kind, PacketKind::Request | PacketKind::Ok) && len != 0 {
            return Err(ERR_INVALID_RESPONSE);
        }
        let mut body = vec![0; len as usize];
        let mut ciphertext = vec![0; body.len().min(CHUNK_BYTES) + TAG_BYTES];
        for chunk in body.chunks_mut(CHUNK_BYTES) {
            self.read_record(chunk, &mut ciphertext[..chunk.len() + TAG_BYTES])
                .await?;
        }
        Ok((kind, body))
    }

    async fn write_record(
        &mut self,
        plaintext: &[u8],
        ciphertext: &mut [u8],
    ) -> Result<(), &'static str> {
        let count = self
            .transport
            .write_message(plaintext, ciphertext)
            .map_err(|_| ERR_INVALID_RESPONSE)?;
        self.stream
            .write_all(&ciphertext[..count])
            .await
            .map_err(|_| ERR_UNREACHABLE)
    }

    async fn read_record(
        &mut self,
        plaintext: &mut [u8],
        ciphertext: &mut [u8],
    ) -> Result<(), &'static str> {
        self.stream
            .read_exact(ciphertext)
            .await
            .map_err(|_| ERR_UNREACHABLE)?;
        let written = self
            .transport
            .read_message(ciphertext, plaintext)
            .map_err(|_| ERR_INVALID_RESPONSE)?;
        if written != plaintext.len() {
            return Err(ERR_INVALID_RESPONSE);
        }
        Ok(())
    }
}

async fn send_handshake(
    stream: &mut TcpStream,
    state: &mut HandshakeState,
) -> Result<(), &'static str> {
    let mut packet = [0; HANDSHAKE_BYTES];
    let written = state
        .write_message(&[], &mut packet)
        .map_err(|_| ERR_KEY_REJECTED)?;
    if written != packet.len() {
        return Err(ERR_KEY_REJECTED);
    }
    stream
        .write_all(&packet)
        .await
        .map_err(|_| ERR_KEY_REJECTED)
}

async fn receive_handshake(
    stream: &mut TcpStream,
    state: &mut HandshakeState,
) -> Result<(), &'static str> {
    let mut packet = [0; HANDSHAKE_BYTES];
    stream
        .read_exact(&mut packet)
        .await
        .map_err(|_| ERR_KEY_REJECTED)?;
    let written = state
        .read_message(&packet, &mut [])
        .map_err(|_| ERR_KEY_REJECTED)?;
    if written != 0 {
        return Err(ERR_KEY_REJECTED);
    }
    Ok(())
}
