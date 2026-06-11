use rand::random;
use sui_crypto::Signer as _;
use sui_crypto::ed25519::Ed25519PrivateKey;
use sui_sdk_types::Address;
use sui_sdk_types::Ed25519PublicKey;
use sui_sdk_types::Ed25519Signature;
use zeroize::Zeroizing;

use crate::error::MemWalError;

#[derive(Debug)]
pub struct DelegateKey {
    secret: Zeroizing<[u8; 32]>,
}

impl Clone for DelegateKey {
    fn clone(&self) -> Self {
        Self {
            secret: Zeroizing::new(*self.secret),
        }
    }
}

impl DelegateKey {
    pub fn generate() -> Self {
        Self {
            secret: Zeroizing::new(random()),
        }
    }

    pub fn from_hex(hex_key: &str) -> Result<Self, MemWalError> {
        let bytes = hex::decode(hex_key.strip_prefix("0x").unwrap_or(hex_key))
            .map_err(|error| MemWalError::crypto(error.to_string()))?;
        let secret: [u8; 32] = bytes
            .try_into()
            .map_err(|_| MemWalError::crypto("delegate key must be 32 bytes"))?;
        Ok(Self {
            secret: Zeroizing::new(secret),
        })
    }

    pub fn to_hex(&self) -> String {
        hex::encode(*self.secret)
    }

    pub fn public_key(&self) -> Ed25519PublicKey {
        self.private_key().public_key()
    }

    pub fn public_key_hex(&self) -> String {
        hex::encode(self.public_key().inner())
    }

    pub fn sui_address(&self) -> Address {
        self.public_key().derive_address()
    }

    pub(crate) fn private_key(&self) -> Ed25519PrivateKey {
        Ed25519PrivateKey::new(*self.secret)
    }

    pub(crate) fn sign_raw(&self, message: &[u8]) -> Result<Ed25519Signature, MemWalError> {
        self.private_key()
            .try_sign(message)
            .map_err(|error| MemWalError::crypto(error.to_string()))
    }
}

#[cfg(test)]
mod tests {
    use super::DelegateKey;

    #[test]
    fn delegate_key_round_trip_hex() {
        let key = DelegateKey::generate();
        let hex = key.to_hex();
        let reparsed = DelegateKey::from_hex(&hex).expect("reparse");
        assert_eq!(key.public_key_hex(), reparsed.public_key_hex());
    }
}
