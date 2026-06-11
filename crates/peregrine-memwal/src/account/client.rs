use std::sync::Arc;

use tokio::sync::Mutex;

use crate::error::MemWalError;
use crate::sui::MemWalSigner;
use crate::sui::add_delegate_key_builder;
use crate::sui::create_account_builder;
use crate::sui::created_account_id;
use crate::sui::execute_account_transaction;
use crate::sui::remove_delegate_key_builder;
use crate::sui::transaction_digest;
use crate::types::AddDelegateKeyResult;
use crate::types::CreateAccountResult;

pub struct AccountClient {
    rpc_client: Mutex<sui_rpc::Client>,
    signer: Arc<dyn MemWalSigner>,
    package_id: sui_sdk_types::Address,
}

impl AccountClient {
    pub fn new(
        rpc_client: sui_rpc::Client,
        signer: Arc<dyn MemWalSigner>,
        package_id: sui_sdk_types::Address,
    ) -> Self {
        Self {
            rpc_client: Mutex::new(rpc_client),
            signer,
            package_id,
        }
    }

    pub async fn create_account(
        &self,
        registry_id: sui_sdk_types::Address,
    ) -> Result<CreateAccountResult, MemWalError> {
        let sender = self.signer.address()?;
        let builder = create_account_builder(self.package_id, registry_id, sender)?;
        let response =
            execute_account_transaction(&self.rpc_client, self.signer.as_ref(), builder).await?;
        Ok(CreateAccountResult {
            account_id: created_account_id(&response)?,
            owner: sender,
            digest: transaction_digest(&response)?,
        })
    }

    pub async fn add_delegate_key(
        &self,
        account_id: sui_sdk_types::Address,
        public_key: [u8; 32],
        label: &str,
    ) -> Result<AddDelegateKeyResult, MemWalError> {
        if label.len() > 64 {
            return Err(MemWalError::config(
                "delegate label must be 64 bytes or fewer",
            ));
        }

        let sender = self.signer.address()?;
        let delegate_address = sui_sdk_types::Ed25519PublicKey::new(public_key).derive_address();
        let builder = add_delegate_key_builder(
            self.package_id,
            account_id,
            sender,
            &public_key,
            delegate_address,
            label,
        )?;
        let response =
            execute_account_transaction(&self.rpc_client, self.signer.as_ref(), builder).await?;
        Ok(AddDelegateKeyResult {
            digest: transaction_digest(&response)?,
            public_key_hex: hex::encode(public_key),
            sui_address: delegate_address,
        })
    }

    pub async fn remove_delegate_key(
        &self,
        account_id: sui_sdk_types::Address,
        public_key: [u8; 32],
    ) -> Result<sui_sdk_types::Digest, MemWalError> {
        let sender = self.signer.address()?;
        let builder =
            remove_delegate_key_builder(self.package_id, account_id, sender, &public_key)?;
        let response =
            execute_account_transaction(&self.rpc_client, self.signer.as_ref(), builder).await?;
        transaction_digest(&response)
    }
}
