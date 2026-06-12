use std::sync::Arc;

use async_trait::async_trait;
use seal_fastcrypto::ed25519::Ed25519PublicKey as SealEd25519PublicKey;
use seal_fastcrypto::ed25519::Ed25519Signature as SealEd25519Signature;
use seal_fastcrypto::traits::ToFromBytes;
use sui_crypto::Signer as _;
use sui_crypto::SuiSigner as _;
use sui_crypto::ed25519::Ed25519PrivateKey;
use sui_sdk_types::Address;
use sui_sdk_types::Ed25519PublicKey;
use sui_sdk_types::PersonalMessage;
use sui_sdk_types::Transaction;
use sui_sdk_types::UserSignature;
use zeroize::Zeroizing;

use crate::auth::DelegateKey;
use crate::error::MemWalError;

pub trait MemWalSigner: Send + Sync {
    fn address(&self) -> Result<Address, MemWalError>;
    fn public_key(&self) -> Result<Ed25519PublicKey, MemWalError>;
    fn sign_transaction(&self, transaction: &Transaction) -> Result<UserSignature, MemWalError>;
    fn sign_personal_message(
        &self,
        message: &PersonalMessage<'_>,
    ) -> Result<UserSignature, MemWalError>;
    fn sign_raw_ed25519(&self, message: &[u8]) -> Result<[u8; 64], MemWalError>;
}

#[derive(Debug)]
pub struct Ed25519Signer {
    suiprivkey: Zeroizing<String>,
}

impl Clone for Ed25519Signer {
    fn clone(&self) -> Self {
        Self {
            suiprivkey: Zeroizing::new(self.suiprivkey.to_string()),
        }
    }
}

impl Ed25519Signer {
    pub fn generate() -> Result<Self, MemWalError> {
        Self::from_bytes(rand::random())
    }

    pub fn from_suiprivkey(s: &str) -> Result<Self, MemWalError> {
        Ed25519PrivateKey::from_suiprivkey(s)
            .map_err(|error| MemWalError::signer(error.to_string()))?;
        Ok(Self {
            suiprivkey: Zeroizing::new(s.to_owned()),
        })
    }

    pub fn from_bytes(bytes: [u8; 32]) -> Result<Self, MemWalError> {
        Self::from_private_key(Ed25519PrivateKey::new(bytes))
    }

    pub fn from_delegate_key(delegate_key: DelegateKey) -> Result<Self, MemWalError> {
        Self::from_private_key(delegate_key.private_key())
    }

    pub fn to_suiprivkey(&self) -> Result<String, MemWalError> {
        Ok(self.suiprivkey.to_string())
    }

    pub fn public_key_bytes(&self) -> Result<[u8; 32], MemWalError> {
        self.public_key().map(Ed25519PublicKey::into_inner)
    }

    fn private_key(&self) -> Result<Ed25519PrivateKey, MemWalError> {
        Ed25519PrivateKey::from_suiprivkey(&self.suiprivkey)
            .map_err(|error| MemWalError::signer(error.to_string()))
    }

    fn from_private_key(private_key: Ed25519PrivateKey) -> Result<Self, MemWalError> {
        let suiprivkey = private_key
            .to_suiprivkey()
            .map_err(|error| MemWalError::signer(error.to_string()))?;
        Ok(Self {
            suiprivkey: Zeroizing::new(suiprivkey),
        })
    }
}

impl MemWalSigner for Ed25519Signer {
    fn address(&self) -> Result<Address, MemWalError> {
        Ok(self.private_key()?.public_key().derive_address())
    }

    fn public_key(&self) -> Result<Ed25519PublicKey, MemWalError> {
        Ok(self.private_key()?.public_key())
    }

    fn sign_transaction(&self, transaction: &Transaction) -> Result<UserSignature, MemWalError> {
        self.private_key()?
            .sign_transaction(transaction)
            .map_err(|error| MemWalError::signer(error.to_string()))
    }

    fn sign_personal_message(
        &self,
        message: &PersonalMessage<'_>,
    ) -> Result<UserSignature, MemWalError> {
        self.private_key()?
            .sign_personal_message(message)
            .map_err(|error| MemWalError::signer(error.to_string()))
    }

    fn sign_raw_ed25519(&self, message: &[u8]) -> Result<[u8; 64], MemWalError> {
        let signature: sui_sdk_types::Ed25519Signature = self
            .private_key()?
            .try_sign(message)
            .map_err(|error| MemWalError::signer(error.to_string()))?;
        Ok(signature.into_inner())
    }
}

pub(crate) struct SealSignerAdapter {
    signer: Arc<dyn MemWalSigner>,
}

impl SealSignerAdapter {
    pub(crate) fn new(signer: Arc<dyn MemWalSigner>) -> Self {
        Self { signer }
    }
}

#[async_trait]
impl seal_sdk_rs::signer::Signer for SealSignerAdapter {
    type Error = seal_sdk_rs::error::SessionKeyError;

    async fn sign_personal_message(
        &mut self,
        message: Vec<u8>,
    ) -> Result<SealEd25519Signature, Self::Error> {
        let signature = self.signer.sign_raw_ed25519(&message).map_err(|error| {
            seal_sdk_rs::error::SessionKeyError::UnknownError(error.to_string())
        })?;
        <SealEd25519Signature as ToFromBytes>::from_bytes(&signature)
            .map_err(|error| seal_sdk_rs::error::SessionKeyError::UnknownError(error.to_string()))
    }

    fn get_public_key(&mut self) -> Result<SealEd25519PublicKey, Self::Error> {
        let public_key = self.signer.public_key().map_err(|error| {
            seal_sdk_rs::error::SessionKeyError::UnknownError(error.to_string())
        })?;
        <SealEd25519PublicKey as ToFromBytes>::from_bytes(public_key.inner())
            .map_err(|error| seal_sdk_rs::error::SessionKeyError::UnknownError(error.to_string()))
    }

    fn get_sui_address(&mut self) -> Result<seal_sdk_rs::generic_types::SuiAddress, Self::Error> {
        Ok(seal_sdk_rs::generic_types::SuiAddress(
            self.signer
                .address()
                .map_err(|error| {
                    seal_sdk_rs::error::SessionKeyError::UnknownError(error.to_string())
                })?
                .into_inner(),
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::Ed25519Signer;
    use super::MemWalSigner;
    use crate::error::MemWalError;

    #[test]
    fn suiprivkey_round_trip() -> Result<(), MemWalError> {
        let signer = Ed25519Signer::generate()?;
        let encoded = signer.to_suiprivkey()?;
        let decoded = Ed25519Signer::from_suiprivkey(&encoded)?;
        assert_eq!(signer.public_key()?, decoded.public_key()?);
        Ok(())
    }
}
