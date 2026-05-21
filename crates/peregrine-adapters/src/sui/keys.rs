use anyhow::{anyhow, bail, Context};
use bip32::DerivationPath;
use fastcrypto::encoding::{Encoding, Hex};
use serde::{Deserialize, Serialize};
use std::{path::PathBuf, str::FromStr};
use sui_config::{
    Config, SUI_CLIENT_CONFIG, SUI_CONFIG_DIR, SUI_KEYSTORE_ALIASES_FILENAME, SUI_KEYSTORE_FILENAME,
};
use sui_keys::{
    external::External,
    key_derive::{derive_key_pair_from_path, generate_new_key},
    key_identity::KeyIdentity,
    keystore::{AccountKeystore, Alias, FileBasedKeystore, Keystore},
};
use sui_sdk::sui_client_config::{SuiClientConfig, SuiEnv};
use sui_types::{
    base_types::SuiAddress,
    crypto::{EncodeDecodeBase64, PublicKey, SignatureScheme, SuiKeyPair},
};

const SUI_HOME_DIR: &str = ".sui";
const EXTERNAL_KEYSTORE_FILENAME: &str = "external.keystore";
const HEX_IMPORT_REJECTION: &str = "Sui Keystore and Sui Wallet no longer support importing private key as Hex. If you are sure your private key is encoded in Hex, convert it to a Bech32 private key starting with `suiprivkey` before importing.";

#[derive(Clone, Debug)]
pub struct SuiKeyManager {
    config_dir: PathBuf,
}

impl SuiKeyManager {
    pub fn new_default() -> Result<Self, anyhow::Error> {
        Ok(Self::new(default_sui_config_dir()?))
    }

    pub fn new(config_dir: impl Into<PathBuf>) -> Self {
        Self {
            config_dir: config_dir.into(),
        }
    }

    pub fn load_state(&self) -> Result<SuiKeyState, anyhow::Error> {
        let paths = self.paths();

        if !paths.client_config_path.is_file() {
            return Ok(self.empty_state(SuiKeyConfigStatus::Missing));
        }

        match SuiClientConfig::load_with_lock(&paths.client_config_path) {
            Ok(config) => self.state_from_config(&config, SuiKeyConfigStatus::Loaded),
            Err(error) => {
                let mut state = self.empty_state(SuiKeyConfigStatus::Invalid);
                state.diagnostics.push(SuiKeyDiagnostic {
                    level: SuiKeyDiagnosticLevel::Error,
                    message: format!(
                        "Could not load Sui client config {}: {error}",
                        paths.client_config_path.display()
                    ),
                    path: Some(paths.client_config_path.display().to_string()),
                });
                Ok(state)
            }
        }
    }

    pub async fn generate_key(
        &self,
        request: SuiGenerateKeyRequest,
    ) -> Result<SuiGenerateKeyResponse, anyhow::Error> {
        let paths = self.paths();
        let (mut config, created_config) = self.load_or_initialize_config()?;
        let key_scheme = parse_user_signature_scheme(&request.key_scheme)?;
        let derivation_path = parse_derivation_path(request.derivation_path.as_deref())?;
        let word_length = normalized_word_length(request.word_length.as_deref())?;
        let alias = normalized_alias(request.alias);
        let (address, keypair, scheme, phrase) =
            generate_new_key(key_scheme, derivation_path, word_length)
                .map_err(|error| anyhow!("Failed to generate new Sui key: {error}"))?;

        ensure_address_is_new(&config.keystore, &address)?;
        config.keystore.import(alias, keypair).await?;

        if config.active_address.is_none() || created_config {
            config.active_address = Some(address);
        }

        config
            .save_with_lock(&paths.client_config_path)
            .with_context(|| {
                format!(
                    "Could not save Sui client config {}",
                    paths.client_config_path.display()
                )
            })?;

        let state = self.state_from_config(&config, SuiKeyConfigStatus::Loaded)?;
        let generated = state
            .accounts
            .iter()
            .find(|account| account.address == address.to_string())
            .cloned()
            .ok_or_else(|| anyhow!("Generated key was not found after saving the keystore"))?;

        Ok(SuiGenerateKeyResponse {
            generated,
            recovery_phrase: request.reveal_recovery_phrase.then_some(phrase),
            state,
            key_scheme: scheme.to_string(),
        })
    }

    pub async fn import_key(
        &self,
        request: SuiImportKeyRequest,
    ) -> Result<SuiImportKeyResponse, anyhow::Error> {
        let paths = self.paths();
        let (mut config, created_config) = self.load_or_initialize_config()?;
        let input = request.input_string.trim();

        if input.is_empty() {
            bail!("Sui key import input cannot be empty.");
        }

        if input.contains('\0') {
            bail!("Sui key import input contains an invalid null byte.");
        }

        if Hex::decode(input).is_ok() {
            bail!(HEX_IMPORT_REJECTION);
        }

        let alias = normalized_alias(request.alias);
        let keypair = match SuiKeyPair::decode(input) {
            Ok(keypair) => keypair,
            Err(_) => {
                let key_scheme = parse_user_signature_scheme(&request.key_scheme)?;
                let derivation_path = parse_derivation_path(request.derivation_path.as_deref())?;
                derive_keypair_from_mnemonic(input, key_scheme, derivation_path)?
            }
        };
        let address = SuiAddress::from(&keypair.public());

        ensure_address_is_new(&config.keystore, &address)?;
        config.keystore.import(alias, keypair).await?;

        if config.active_address.is_none() || created_config {
            config.active_address = Some(address);
        }

        config
            .save_with_lock(&paths.client_config_path)
            .with_context(|| {
                format!(
                    "Could not save Sui client config {}",
                    paths.client_config_path.display()
                )
            })?;

        let state = self.state_from_config(&config, SuiKeyConfigStatus::Loaded)?;
        let imported = state
            .accounts
            .iter()
            .find(|account| account.address == address.to_string())
            .cloned()
            .ok_or_else(|| anyhow!("Imported key was not found after saving the keystore"))?;

        Ok(SuiImportKeyResponse { imported, state })
    }

    pub async fn rename_alias(
        &self,
        request: SuiRenameKeyAliasRequest,
    ) -> Result<SuiKeyState, anyhow::Error> {
        let paths = self.paths();
        let mut config = self.load_existing_config_for_write()?;
        let identity = parse_key_identity(&request.alias_or_address)?;
        let address = config.keystore.get_by_identity(&identity)?;
        let old_alias = config.keystore.get_alias(&address)?;

        config
            .keystore
            .update_alias(&old_alias, Some(request.new_alias.trim()))
            .await?;
        config
            .save_with_lock(&paths.client_config_path)
            .with_context(|| {
                format!(
                    "Could not save Sui client config {}",
                    paths.client_config_path.display()
                )
            })?;

        self.state_from_config(&config, SuiKeyConfigStatus::Loaded)
    }

    pub fn set_active_address(
        &self,
        request: SuiSetActiveAddressRequest,
    ) -> Result<SuiKeyState, anyhow::Error> {
        let paths = self.paths();
        let mut config = self.load_existing_config_for_write()?;
        let identity = parse_key_identity(&request.alias_or_address)?;
        let address = resolve_managed_address(&config, &identity)?;

        config.active_address = Some(address);
        config
            .save_with_lock(&paths.client_config_path)
            .with_context(|| {
                format!(
                    "Could not save Sui client config {}",
                    paths.client_config_path.display()
                )
            })?;

        self.state_from_config(&config, SuiKeyConfigStatus::Loaded)
    }

    pub async fn remove_key(
        &self,
        request: SuiRemoveKeyRequest,
    ) -> Result<SuiKeyState, anyhow::Error> {
        let paths = self.paths();
        let mut config = self.load_existing_config_for_write()?;
        let identity = parse_key_identity(&request.alias_or_address)?;
        let address = config.keystore.get_by_identity(&identity)?;
        let alias = config.keystore.get_alias(&address)?;

        require_confirmation(&request.confirmation, &alias, &address)?;
        config.keystore.remove(address).await?;

        if config.active_address == Some(address) {
            config.active_address = config.keystore.addresses().first().copied();
        }

        config
            .save_with_lock(&paths.client_config_path)
            .with_context(|| {
                format!(
                    "Could not save Sui client config {}",
                    paths.client_config_path.display()
                )
            })?;

        self.state_from_config(&config, SuiKeyConfigStatus::Loaded)
    }

    pub fn export_private_key(
        &self,
        request: SuiExportPrivateKeyRequest,
    ) -> Result<SuiExportPrivateKeyResponse, anyhow::Error> {
        let config = self.load_existing_config_for_write()?;
        let identity = parse_key_identity(&request.alias_or_address)?;
        let address = config.keystore.get_by_identity(&identity)?;
        let alias = config.keystore.get_alias(&address)?;

        require_confirmation(&request.confirmation, &alias, &address)?;

        let keypair = config.keystore.export(&address)?;
        let exported_private_key = keypair
            .encode()
            .map_err(|_| anyhow!("Could not encode Sui private key as Bech32"))?;
        let account = self.account_from_address(&config, &address)?;

        Ok(SuiExportPrivateKeyResponse {
            account,
            exported_private_key,
        })
    }

    fn paths(&self) -> SuiKeyPaths {
        SuiKeyPaths {
            aliases_path: self.config_dir.join(SUI_KEYSTORE_ALIASES_FILENAME),
            client_config_path: self.config_dir.join(SUI_CLIENT_CONFIG),
            config_dir: self.config_dir.clone(),
            external_keystore_path: self.config_dir.join(EXTERNAL_KEYSTORE_FILENAME),
            keystore_path: self.config_dir.join(SUI_KEYSTORE_FILENAME),
        }
    }

    fn empty_state(&self, config_status: SuiKeyConfigStatus) -> SuiKeyState {
        let paths = self.paths();

        SuiKeyState {
            accounts: vec![],
            active_address: None,
            active_env: None,
            aliases_path: paths.aliases_path.display().to_string(),
            client_config_path: paths.client_config_path.display().to_string(),
            config_dir: paths.config_dir.display().to_string(),
            config_status,
            diagnostics: vec![],
            external_keystore_path: paths.external_keystore_path.display().to_string(),
            keystore_path: paths.keystore_path.display().to_string(),
            supported_schemes: supported_schemes(),
            supported_word_lengths: supported_word_lengths(),
        }
    }

    fn state_from_config(
        &self,
        config: &SuiClientConfig,
        config_status: SuiKeyConfigStatus,
    ) -> Result<SuiKeyState, anyhow::Error> {
        let mut state = self.empty_state(config_status);
        let active_address = config
            .active_address
            .or_else(|| config.keystore.addresses().first().copied());

        state.active_address = active_address.map(|address| address.to_string());
        state.active_env = config.active_env.clone();
        state.accounts.extend(self.accounts_from_keystore(
            &config.keystore,
            active_address,
            false,
        )?);

        if let Some(external_keys) = config.external_keys.as_ref() {
            state.accounts.extend(self.accounts_from_keystore(
                external_keys,
                active_address,
                true,
            )?);
        }

        Ok(state)
    }

    fn accounts_from_keystore(
        &self,
        keystore: &Keystore,
        active_address: Option<SuiAddress>,
        is_external: bool,
    ) -> Result<Vec<SuiKeyAccount>, anyhow::Error> {
        keystore
            .addresses_with_alias()
            .into_iter()
            .map(|(address, alias)| {
                account_from_alias(*address, alias, active_address, is_external)
            })
            .collect()
    }

    fn account_from_address(
        &self,
        config: &SuiClientConfig,
        address: &SuiAddress,
    ) -> Result<SuiKeyAccount, anyhow::Error> {
        let alias = config.keystore.get_alias(address)?;
        let public_key = config.keystore.export(address)?.public();
        Ok(account_from_public_key(
            *address,
            Some(alias),
            public_key,
            config.active_address == Some(*address),
            false,
        ))
    }

    fn load_or_initialize_config(&self) -> Result<(SuiClientConfig, bool), anyhow::Error> {
        let paths = self.paths();

        if paths.client_config_path.is_file() {
            return Ok((
                SuiClientConfig::load_with_lock(&paths.client_config_path).with_context(|| {
                    format!(
                        "Could not load Sui client config {}",
                        paths.client_config_path.display()
                    )
                })?,
                false,
            ));
        }

        std::fs::create_dir_all(&paths.config_dir).with_context(|| {
            format!(
                "Could not create Sui config directory {}",
                paths.config_dir.display()
            )
        })?;

        let keystore = Keystore::from(FileBasedKeystore::load_or_create(&paths.keystore_path)?);
        let external_keys = Some(Keystore::External(External::load_or_create(
            &paths.external_keystore_path,
        )?));
        let default_env = SuiEnv::testnet();
        let active_env = Some(default_env.alias.clone());
        let config = SuiClientConfig {
            active_address: None,
            active_env,
            envs: vec![
                default_env,
                SuiEnv::mainnet(),
                SuiEnv::devnet(),
                SuiEnv::localnet(),
            ],
            external_keys,
            keystore,
        };

        config
            .save_with_lock(&paths.client_config_path)
            .with_context(|| {
                format!(
                    "Could not save Sui client config {}",
                    paths.client_config_path.display()
                )
            })?;

        Ok((config, true))
    }

    fn load_existing_config_for_write(&self) -> Result<SuiClientConfig, anyhow::Error> {
        let paths = self.paths();

        if !paths.client_config_path.is_file() {
            bail!(
                "No Sui client config found at {}. Generate or import a key first.",
                paths.client_config_path.display()
            );
        }

        SuiClientConfig::load_with_lock(&paths.client_config_path).with_context(|| {
            format!(
                "Could not load Sui client config {}",
                paths.client_config_path.display()
            )
        })
    }
}

#[derive(Clone, Debug)]
struct SuiKeyPaths {
    aliases_path: PathBuf,
    client_config_path: PathBuf,
    config_dir: PathBuf,
    external_keystore_path: PathBuf,
    keystore_path: PathBuf,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct SuiKeyState {
    pub accounts: Vec<SuiKeyAccount>,
    pub active_address: Option<String>,
    pub active_env: Option<String>,
    pub aliases_path: String,
    pub client_config_path: String,
    pub config_dir: String,
    pub config_status: SuiKeyConfigStatus,
    pub diagnostics: Vec<SuiKeyDiagnostic>,
    pub external_keystore_path: String,
    pub keystore_path: String,
    pub supported_schemes: Vec<String>,
    pub supported_word_lengths: Vec<String>,
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum SuiKeyConfigStatus {
    Missing,
    Loaded,
    Invalid,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct SuiKeyDiagnostic {
    pub level: SuiKeyDiagnosticLevel,
    pub message: String,
    pub path: Option<String>,
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum SuiKeyDiagnosticLevel {
    Error,
    Warning,
    Info,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct SuiKeyAccount {
    pub address: String,
    pub alias: Option<String>,
    pub can_export_private_key: bool,
    pub can_remove: bool,
    pub flag: u8,
    pub is_active: bool,
    pub is_external: bool,
    pub key_scheme: String,
    pub peer_id: Option<String>,
    pub public_base64_key: String,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct SuiGenerateKeyRequest {
    pub alias: Option<String>,
    pub derivation_path: Option<String>,
    pub key_scheme: String,
    #[serde(default)]
    pub reveal_recovery_phrase: bool,
    pub word_length: Option<String>,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct SuiGenerateKeyResponse {
    pub generated: SuiKeyAccount,
    pub key_scheme: String,
    pub recovery_phrase: Option<String>,
    pub state: SuiKeyState,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct SuiImportKeyRequest {
    pub alias: Option<String>,
    pub derivation_path: Option<String>,
    pub input_string: String,
    pub key_scheme: String,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct SuiImportKeyResponse {
    pub imported: SuiKeyAccount,
    pub state: SuiKeyState,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct SuiRenameKeyAliasRequest {
    pub alias_or_address: String,
    pub new_alias: String,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct SuiSetActiveAddressRequest {
    pub alias_or_address: String,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct SuiRemoveKeyRequest {
    pub alias_or_address: String,
    pub confirmation: String,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct SuiExportPrivateKeyRequest {
    pub alias_or_address: String,
    pub confirmation: String,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct SuiExportPrivateKeyResponse {
    pub account: SuiKeyAccount,
    pub exported_private_key: String,
}

fn default_sui_config_dir() -> Result<PathBuf, anyhow::Error> {
    if let Some(config_dir) = std::env::var_os("SUI_CONFIG_DIR") {
        return Ok(PathBuf::from(config_dir));
    }

    let home = dirs::home_dir().ok_or_else(|| anyhow!("Cannot obtain home directory path"))?;
    Ok(home.join(SUI_HOME_DIR).join(SUI_CONFIG_DIR))
}

fn account_from_alias(
    address: SuiAddress,
    alias: &Alias,
    active_address: Option<SuiAddress>,
    is_external: bool,
) -> Result<SuiKeyAccount, anyhow::Error> {
    let public_key = PublicKey::decode_base64(&alias.public_key_base64).map_err(|error| {
        anyhow!(
            "Invalid Sui alias public key for {}: {error:?}",
            alias.alias
        )
    })?;

    Ok(account_from_public_key(
        address,
        Some(alias.alias.clone()),
        public_key,
        active_address == Some(address),
        is_external,
    ))
}

fn account_from_public_key(
    address: SuiAddress,
    alias: Option<String>,
    public_key: PublicKey,
    is_active: bool,
    is_external: bool,
) -> SuiKeyAccount {
    SuiKeyAccount {
        address: address.to_string(),
        alias,
        can_export_private_key: !is_external,
        can_remove: !is_external,
        flag: public_key.flag(),
        is_active,
        is_external,
        key_scheme: public_key.scheme().to_string(),
        peer_id: peer_id(&public_key),
        public_base64_key: public_key.encode_base64(),
    }
}

fn peer_id(public_key: &PublicKey) -> Option<String> {
    if let PublicKey::Ed25519(public_key) = public_key {
        Some(anemo::PeerId(public_key.0).to_string())
    } else {
        None
    }
}

fn parse_user_signature_scheme(value: &str) -> Result<SignatureScheme, anyhow::Error> {
    let scheme = SignatureScheme::from_str(value.trim())
        .map_err(|_| anyhow!("Unsupported Sui key scheme `{}`.", value.trim()))?;

    match scheme {
        SignatureScheme::ED25519 | SignatureScheme::Secp256k1 | SignatureScheme::Secp256r1 => {
            Ok(scheme)
        }
        _ => bail!(
            "Unsupported Sui key scheme `{}`. Supported schemes are ed25519, secp256k1, and secp256r1.",
            value.trim()
        ),
    }
}

fn parse_derivation_path(
    derivation_path: Option<&str>,
) -> Result<Option<DerivationPath>, anyhow::Error> {
    let Some(derivation_path) = derivation_path
        .map(str::trim)
        .filter(|path| !path.is_empty())
    else {
        return Ok(None);
    };

    derivation_path
        .parse()
        .map(Some)
        .map_err(|error| anyhow!("Invalid Sui derivation path `{derivation_path}`: {error}"))
}

fn normalized_word_length(word_length: Option<&str>) -> Result<Option<String>, anyhow::Error> {
    let Some(word_length) = word_length.map(str::trim).filter(|value| !value.is_empty()) else {
        return Ok(None);
    };

    if supported_word_lengths()
        .iter()
        .any(|supported| supported == word_length)
    {
        Ok(Some(word_length.to_string()))
    } else {
        bail!("Invalid Sui recovery phrase word length `{word_length}`.")
    }
}

fn normalized_alias(alias: Option<String>) -> Option<String> {
    alias.and_then(|alias| {
        let alias = alias.trim().to_string();
        (!alias.is_empty()).then_some(alias)
    })
}

fn derive_keypair_from_mnemonic(
    phrase: &str,
    key_scheme: SignatureScheme,
    derivation_path: Option<DerivationPath>,
) -> Result<SuiKeyPair, anyhow::Error> {
    let mnemonic = bip39::Mnemonic::from_phrase(phrase, bip39::Language::English)
        .map_err(|error| anyhow!("Invalid mnemonic phrase: {error:?}"))?;
    let seed = bip39::Seed::new(&mnemonic, "");
    let (_address, keypair) =
        derive_key_pair_from_path(seed.as_bytes(), derivation_path, &key_scheme)
            .map_err(|error| anyhow!("Error getting Sui keypair from mnemonic: {error:?}"))?;

    Ok(keypair)
}

fn ensure_address_is_new(keystore: &Keystore, address: &SuiAddress) -> Result<(), anyhow::Error> {
    if keystore.addresses().contains(address) {
        bail!("Sui address {address} already exists in the keystore.");
    }

    Ok(())
}

fn parse_key_identity(value: &str) -> Result<KeyIdentity, anyhow::Error> {
    KeyIdentity::from_str(value.trim())
        .map_err(|error| anyhow!("Invalid address or alias: {error}"))
}

fn resolve_managed_address(
    config: &SuiClientConfig,
    identity: &KeyIdentity,
) -> Result<SuiAddress, anyhow::Error> {
    if let Ok(address) = config.keystore.get_by_identity(identity) {
        if config.keystore.addresses().contains(&address) {
            return Ok(address);
        }
    }

    if let Some(external_keys) = config.external_keys.as_ref() {
        if let Ok(address) = external_keys.get_by_identity(identity) {
            if external_keys.addresses().contains(&address) {
                return Ok(address);
            }
        }
    }

    bail!("No managed Sui address found for {identity}.")
}

fn require_confirmation(
    confirmation: &str,
    alias: &str,
    address: &SuiAddress,
) -> Result<(), anyhow::Error> {
    let confirmation = confirmation.trim();

    if confirmation == alias || confirmation == address.to_string() {
        Ok(())
    } else {
        bail!("Confirmation must match the Sui address or alias exactly.")
    }
}

fn supported_schemes() -> Vec<String> {
    ["ed25519", "secp256k1", "secp256r1"]
        .into_iter()
        .map(str::to_string)
        .collect()
}

fn supported_word_lengths() -> Vec<String> {
    ["word12", "word15", "word18", "word21", "word24"]
        .into_iter()
        .map(str::to_string)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn missing_config_lists_without_creating_files() {
        let temp_dir = TempDir::new().expect("temp dir");
        let config_dir = temp_dir.path().join("sui_config");
        let manager = SuiKeyManager::new(&config_dir);

        let state = manager.load_state().expect("state");

        assert_eq!(state.config_status, SuiKeyConfigStatus::Missing);
        assert!(state.accounts.is_empty());
        assert!(!config_dir.exists());
    }

    #[tokio::test]
    async fn first_generate_initializes_config_and_keystore() {
        let temp_dir = TempDir::new().expect("temp dir");
        let config_dir = temp_dir.path().join("sui_config");
        let manager = SuiKeyManager::new(&config_dir);

        let response = manager
            .generate_key(SuiGenerateKeyRequest {
                alias: Some("first_key".to_string()),
                derivation_path: None,
                key_scheme: "ed25519".to_string(),
                reveal_recovery_phrase: true,
                word_length: Some("word12".to_string()),
            })
            .await
            .expect("generated key");

        assert!(config_dir.join(SUI_CLIENT_CONFIG).is_file());
        assert!(config_dir.join(SUI_KEYSTORE_FILENAME).is_file());
        assert!(config_dir.join(SUI_KEYSTORE_ALIASES_FILENAME).is_file());
        assert_eq!(response.generated.alias.as_deref(), Some("first_key"));
        assert_eq!(response.generated.key_scheme, "ed25519");
        assert!(response.recovery_phrase.is_some());
        assert_eq!(
            response.state.active_address,
            Some(response.generated.address)
        );
    }

    #[tokio::test]
    async fn generate_can_avoid_returning_recovery_phrase() {
        let temp_dir = TempDir::new().expect("temp dir");
        let manager = SuiKeyManager::new(temp_dir.path().join("sui_config"));

        let response = manager
            .generate_key(SuiGenerateKeyRequest {
                alias: None,
                derivation_path: None,
                key_scheme: "secp256k1".to_string(),
                reveal_recovery_phrase: false,
                word_length: None,
            })
            .await
            .expect("generated key");

        assert_eq!(response.generated.key_scheme, "secp256k1");
        assert!(response.recovery_phrase.is_none());
    }

    #[tokio::test]
    async fn import_bech32_key_lists_account() {
        let source_dir = TempDir::new().expect("source dir");
        let source = SuiKeyManager::new(source_dir.path().join("sui_config"));
        let generated = source
            .generate_key(SuiGenerateKeyRequest {
                alias: Some("source_key".to_string()),
                derivation_path: None,
                key_scheme: "ed25519".to_string(),
                reveal_recovery_phrase: false,
                word_length: None,
            })
            .await
            .expect("generated source key");
        let exported = source
            .export_private_key(SuiExportPrivateKeyRequest {
                alias_or_address: "source_key".to_string(),
                confirmation: "source_key".to_string(),
            })
            .expect("exported source key");

        let target_dir = TempDir::new().expect("target dir");
        let target = SuiKeyManager::new(target_dir.path().join("sui_config"));
        let imported = target
            .import_key(SuiImportKeyRequest {
                alias: Some("imported_key".to_string()),
                derivation_path: None,
                input_string: exported.exported_private_key,
                key_scheme: "ed25519".to_string(),
            })
            .await
            .expect("imported key");

        assert_eq!(imported.imported.address, generated.generated.address);
        assert_eq!(imported.imported.alias.as_deref(), Some("imported_key"));
        assert_eq!(imported.state.accounts.len(), 1);
    }

    #[tokio::test]
    async fn import_mnemonic_rejects_duplicate_address() {
        let temp_dir = TempDir::new().expect("temp dir");
        let manager = SuiKeyManager::new(temp_dir.path().join("sui_config"));
        let generated = manager
            .generate_key(SuiGenerateKeyRequest {
                alias: Some("source_key".to_string()),
                derivation_path: None,
                key_scheme: "ed25519".to_string(),
                reveal_recovery_phrase: true,
                word_length: Some("word12".to_string()),
            })
            .await
            .expect("generated key");
        let phrase = generated.recovery_phrase.expect("phrase");

        let error = manager
            .import_key(SuiImportKeyRequest {
                alias: Some("duplicate".to_string()),
                derivation_path: None,
                input_string: phrase,
                key_scheme: "ed25519".to_string(),
            })
            .await
            .expect_err("duplicate address should fail");

        assert!(error.to_string().contains("already exists"));
    }

    #[tokio::test]
    async fn rename_set_active_and_remove_update_state() {
        let temp_dir = TempDir::new().expect("temp dir");
        let manager = SuiKeyManager::new(temp_dir.path().join("sui_config"));
        let first = manager
            .generate_key(SuiGenerateKeyRequest {
                alias: Some("first_key".to_string()),
                derivation_path: None,
                key_scheme: "ed25519".to_string(),
                reveal_recovery_phrase: false,
                word_length: None,
            })
            .await
            .expect("first key")
            .generated;
        let second = manager
            .generate_key(SuiGenerateKeyRequest {
                alias: Some("second_key".to_string()),
                derivation_path: None,
                key_scheme: "secp256r1".to_string(),
                reveal_recovery_phrase: false,
                word_length: None,
            })
            .await
            .expect("second key")
            .generated;

        let renamed = manager
            .rename_alias(SuiRenameKeyAliasRequest {
                alias_or_address: "second_key".to_string(),
                new_alias: "renamed_key".to_string(),
            })
            .await
            .expect("renamed");
        assert!(renamed
            .accounts
            .iter()
            .any(|a| a.alias.as_deref() == Some("renamed_key")));

        let active = manager
            .set_active_address(SuiSetActiveAddressRequest {
                alias_or_address: "renamed_key".to_string(),
            })
            .expect("active");
        assert_eq!(active.active_address, Some(second.address.clone()));

        let removed = manager
            .remove_key(SuiRemoveKeyRequest {
                alias_or_address: "renamed_key".to_string(),
                confirmation: "renamed_key".to_string(),
            })
            .await
            .expect("removed");

        assert_eq!(removed.accounts.len(), 1);
        assert_eq!(removed.active_address, Some(first.address));
    }

    #[tokio::test]
    async fn duplicate_alias_is_rejected() {
        let temp_dir = TempDir::new().expect("temp dir");
        let manager = SuiKeyManager::new(temp_dir.path().join("sui_config"));

        manager
            .generate_key(SuiGenerateKeyRequest {
                alias: Some("first_key".to_string()),
                derivation_path: None,
                key_scheme: "ed25519".to_string(),
                reveal_recovery_phrase: false,
                word_length: None,
            })
            .await
            .expect("first key");

        let error = manager
            .generate_key(SuiGenerateKeyRequest {
                alias: Some("first_key".to_string()),
                derivation_path: None,
                key_scheme: "ed25519".to_string(),
                reveal_recovery_phrase: false,
                word_length: None,
            })
            .await
            .expect_err("duplicate alias should fail");

        assert!(error.to_string().contains("already exists"));
    }

    #[test]
    fn malformed_config_is_reported_without_overwrite() {
        let temp_dir = TempDir::new().expect("temp dir");
        let config_dir = temp_dir.path().join("sui_config");
        std::fs::create_dir_all(&config_dir).expect("config dir");
        std::fs::write(config_dir.join(SUI_CLIENT_CONFIG), "not: [valid").expect("invalid config");
        let manager = SuiKeyManager::new(&config_dir);

        let state = manager.load_state().expect("state");

        assert_eq!(state.config_status, SuiKeyConfigStatus::Invalid);
        assert_eq!(state.diagnostics.len(), 1);
        assert_eq!(
            std::fs::read_to_string(config_dir.join(SUI_CLIENT_CONFIG)).expect("config"),
            "not: [valid"
        );
    }
}
