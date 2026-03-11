//! Noise Protocol encryption layer for Harbor relay tunnels.
//!
//! Uses Noise_NK_25519_ChaChaPoly_BLAKE2s:
//! - NK pattern: client knows server's static public key (pinned)
//! - X25519 for key exchange
//! - ChaChaPoly for symmetric encryption
//! - BLAKE2s for hashing
//!
//! This provides:
//! - Server authentication (client pins the relay's public key)
//! - Forward secrecy (ephemeral keys per session)
//! - No certificates or CA required (ideal for self-hosted relays)
//!
//! The relay server generates a keypair on first start and prints the
//! public key. Clients pin this key in their config.

use crate::error::{HarborError, Result};

const NOISE_PATTERN: &str = "Noise_NK_25519_ChaChaPoly_BLAKE2s";

/// A static Noise keypair (32-byte public + private keys).
#[derive(Clone)]
pub struct Keypair {
    pub public: [u8; 32],
    pub private: Vec<u8>,
}

impl Keypair {
    /// Generate a new random static keypair.
    pub fn generate() -> Result<Self> {
        let builder = snow::Builder::new(NOISE_PATTERN.parse().map_err(noise_err)?);
        let kp = builder.generate_keypair().map_err(noise_err)?;
        let mut public = [0u8; 32];
        public.copy_from_slice(&kp.public);
        Ok(Self {
            public,
            private: kp.private,
        })
    }

    /// Encode public key as hex string (for config/display).
    pub fn public_hex(&self) -> String {
        hex::encode(self.public)
    }

    /// Serialize keypair for persistence as "pubhex:privhex".
    pub fn to_file_format(&self) -> String {
        format!("{}:{}", hex::encode(self.public), hex::encode(&self.private))
    }

    /// Load keypair from "pubhex:privhex" format.
    pub fn from_file_format(s: &str) -> Result<Self> {
        let (pub_hex, priv_hex) = s.trim().split_once(':').ok_or_else(|| {
            HarborError::NoiseHandshakeFailed("Invalid keypair file format".into())
        })?;
        let pub_bytes = hex::decode(pub_hex).map_err(|e| {
            HarborError::NoiseHandshakeFailed(format!("Invalid public key hex: {e}"))
        })?;
        let priv_bytes = hex::decode(priv_hex).map_err(|e| {
            HarborError::NoiseHandshakeFailed(format!("Invalid private key hex: {e}"))
        })?;
        if pub_bytes.len() != 32 {
            return Err(HarborError::NoiseHandshakeFailed(
                "Public key must be 32 bytes".into(),
            ));
        }
        let mut public = [0u8; 32];
        public.copy_from_slice(&pub_bytes);
        Ok(Self {
            public,
            private: priv_bytes,
        })
    }

    /// Decode public key from hex string.
    pub fn public_from_hex(hex_str: &str) -> Result<[u8; 32]> {
        let bytes = hex::decode(hex_str).map_err(|e| {
            HarborError::NoiseHandshakeFailed(format!("Invalid hex public key: {e}"))
        })?;
        if bytes.len() != 32 {
            return Err(HarborError::NoiseHandshakeFailed(format!(
                "Public key must be 32 bytes, got {}",
                bytes.len()
            )));
        }
        let mut key = [0u8; 32];
        key.copy_from_slice(&bytes);
        Ok(key)
    }
}

/// Handshake state — used during the Noise handshake phase.
pub struct HandshakeState {
    inner: snow::HandshakeState,
}

impl HandshakeState {
    /// Create initiator handshake (client side).
    /// NK pattern: client knows the relay's public key but has no static key.
    pub fn initiator(relay_public_key: &[u8; 32]) -> Result<Self> {
        let state = snow::Builder::new(NOISE_PATTERN.parse().map_err(noise_err)?)
            .remote_public_key(relay_public_key)
            .map_err(noise_err)?
            .build_initiator()
            .map_err(noise_err)?;
        Ok(Self { inner: state })
    }

    /// Create responder handshake (relay side).
    pub fn responder(relay_keypair: &Keypair) -> Result<Self> {
        let state = snow::Builder::new(NOISE_PATTERN.parse().map_err(noise_err)?)
            .local_private_key(&relay_keypair.private)
            .map_err(noise_err)?
            .build_responder()
            .map_err(noise_err)?;
        Ok(Self { inner: state })
    }

    /// Write a handshake message (call alternately: initiator writes first).
    pub fn write_message(&mut self, payload: &[u8]) -> Result<Vec<u8>> {
        let mut buf = vec![0u8; 65535];
        let len = self.inner.write_message(payload, &mut buf).map_err(noise_err)?;
        buf.truncate(len);
        Ok(buf)
    }

    /// Read a handshake message.
    pub fn read_message(&mut self, message: &[u8]) -> Result<Vec<u8>> {
        let mut buf = vec![0u8; 65535];
        let len = self.inner.read_message(message, &mut buf).map_err(noise_err)?;
        buf.truncate(len);
        Ok(buf)
    }

    /// Check if the handshake is complete.
    pub fn is_finished(&self) -> bool {
        self.inner.is_handshake_finished()
    }

    /// Convert to transport mode after handshake completes.
    pub fn into_transport(self) -> Result<TransportCipher> {
        if !self.inner.is_handshake_finished() {
            return Err(HarborError::NoiseHandshakeFailed(
                "Handshake not complete".to_string(),
            ));
        }
        let transport = self.inner.into_transport_mode().map_err(noise_err)?;
        Ok(TransportCipher { inner: transport })
    }
}

/// Transport-mode cipher — used after handshake for encrypting/decrypting payloads.
pub struct TransportCipher {
    inner: snow::TransportState,
}

impl TransportCipher {
    /// Encrypt a plaintext payload.
    pub fn encrypt(&mut self, plaintext: &[u8]) -> Result<Vec<u8>> {
        let mut buf = vec![0u8; plaintext.len() + 64]; // AEAD tag overhead
        let len = self.inner.write_message(plaintext, &mut buf).map_err(noise_err)?;
        buf.truncate(len);
        Ok(buf)
    }

    /// Decrypt a ciphertext payload.
    pub fn decrypt(&mut self, ciphertext: &[u8]) -> Result<Vec<u8>> {
        let mut buf = vec![0u8; ciphertext.len()];
        let len = self.inner.read_message(ciphertext, &mut buf).map_err(noise_err)?;
        buf.truncate(len);
        Ok(buf)
    }
}

fn noise_err(e: snow::Error) -> HarborError {
    HarborError::NoiseHandshakeFailed(e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_keypair_generation() {
        let kp = Keypair::generate().unwrap();
        assert_eq!(kp.public.len(), 32);
        assert!(!kp.private.is_empty());
    }

    #[test]
    fn test_keypair_hex_roundtrip() {
        let kp = Keypair::generate().unwrap();
        let hex = kp.public_hex();
        let decoded = Keypair::public_from_hex(&hex).unwrap();
        assert_eq!(kp.public, decoded);
    }

    #[test]
    fn test_handshake_and_encrypt_decrypt() {
        let relay_kp = Keypair::generate().unwrap();

        // Client (initiator) knows relay's public key
        let mut client_hs = HandshakeState::initiator(&relay_kp.public).unwrap();
        let mut relay_hs = HandshakeState::responder(&relay_kp).unwrap();

        // NK pattern: client writes first (-> e, es)
        let msg1 = client_hs.write_message(b"").unwrap();
        relay_hs.read_message(&msg1).unwrap();

        // Relay writes back (-> e, ee)
        let msg2 = relay_hs.write_message(b"").unwrap();
        client_hs.read_message(&msg2).unwrap();

        assert!(client_hs.is_finished());
        assert!(relay_hs.is_finished());

        // Convert to transport mode
        let mut client_cipher = client_hs.into_transport().unwrap();
        let mut relay_cipher = relay_hs.into_transport().unwrap();

        // Client encrypts, relay decrypts
        let plaintext = b"Hello from client";
        let ciphertext = client_cipher.encrypt(plaintext).unwrap();
        let decrypted = relay_cipher.decrypt(&ciphertext).unwrap();
        assert_eq!(decrypted, plaintext);

        // Relay encrypts, client decrypts
        let plaintext2 = b"Hello from relay";
        let ciphertext2 = relay_cipher.encrypt(plaintext2).unwrap();
        let decrypted2 = client_cipher.decrypt(&ciphertext2).unwrap();
        assert_eq!(decrypted2, plaintext2);
    }
}
