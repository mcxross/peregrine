#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CreateAccountResult {
    pub account_id: sui_sdk_types::Address,
    pub owner: sui_sdk_types::Address,
    pub digest: sui_sdk_types::Digest,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AddDelegateKeyResult {
    pub digest: sui_sdk_types::Digest,
    pub public_key_hex: String,
    pub sui_address: sui_sdk_types::Address,
}
