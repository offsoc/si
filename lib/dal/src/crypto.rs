use std::collections::HashMap;
use std::path::Path;
use std::sync::Arc;

use serde::{Deserialize, Serialize};
use sodiumoxide::crypto::secretbox;
use sodiumoxide::crypto::secretbox::{Key, Nonce};
use thiserror::Error;
use tokio::fs::File;
use tokio::io::{AsyncReadExt, AsyncWriteExt};

#[remain::sorted]
#[derive(Error, Debug)]
pub enum SymmetricCryptoError {
    #[error("error when decrypting ciphertext")]
    DecryptionFailed,
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
    #[error("no key present matching provided hash")]
    MissingDonkeyForHash,
    #[error("error deserializing json: {0}")]
    SerdeDeserializeJson(serde_json::Error),
    #[error("error serializing json: {0}")]
    SerdeSerializeJson(serde_json::Error),
}

pub type SymmetricCryptoResult<T> = Result<T, SymmetricCryptoError>;

type Hash = [u8; 32];

#[derive(Serialize, Deserialize, Clone, Debug, Eq, PartialEq)]
pub struct SymmetricKey(Key);

impl SymmetricKey {
    async fn save(&self, path: impl AsRef<Path>) -> SymmetricCryptoResult<()> {
        let file_data = SymmetricKeyFile { key: self.clone() };

        file_data.save(path).await
    }
    async fn load(path: impl AsRef<Path>) -> SymmetricCryptoResult<Self> {
        Ok(SymmetricKeyFile::load(path).await?.into())
    }
}

impl From<SymmetricKeyFile> for SymmetricKey {
    fn from(value: SymmetricKeyFile) -> Self {
        value.key
    }
}

#[derive(Serialize, Deserialize, Debug, Eq, PartialEq)]
struct SymmetricKeyFile {
    key: SymmetricKey,
}

impl SymmetricKeyFile {
    async fn save(&self, path: impl AsRef<Path>) -> SymmetricCryptoResult<()> {
        let mut file = File::create(&path).await?;
        file.write_all(
            &(serde_json::to_vec(self).map_err(SymmetricCryptoError::SerdeSerializeJson)?),
        )
        .await?;

        Ok(())
    }

    async fn load(path: impl AsRef<Path>) -> SymmetricCryptoResult<Self> {
        let mut file = File::open(&path).await?;
        let mut buf = Vec::new();
        file.read_to_end(&mut buf).await?;

        Ok(serde_json::from_slice(&buf).map_err(SymmetricCryptoError::SerdeDeserializeJson)?)
    }
}

#[derive(Clone, Debug)]
pub struct SymmetricCryptoService {
    donkeys: Arc<HashMap<Hash, secretbox::Key>>,
    active_key_hash: Arc<Hash>,
}

/// si-cli exec --key=~/keys/prod.key --extra-keys=~/keys/*.key

impl SymmetricCryptoService {
    pub fn new(active_key: SymmetricKey, extra_keys: Vec<SymmetricKey>) -> Self {
        let mut map = HashMap::new();

        let active_key_hash = *blake3::hash(active_key.0.as_ref()).as_bytes();

        map.insert(active_key_hash, active_key.0);

        for key in extra_keys {
            map.insert(*blake3::hash(key.0.as_ref()).as_bytes(), key.0);
        }

        Self {
            donkeys: Arc::new(map),
            active_key_hash: Arc::new(active_key_hash),
        }
    }

    pub fn generate_key() -> SymmetricKey {
        SymmetricKey(secretbox::gen_key())
    }

    pub fn encrypt(&self, message: &[u8]) -> (Vec<u8>, Nonce, &Hash) {
        let key = self
            .donkeys
            .get(self.active_key_hash.as_ref())
            .expect("active_key value not present in donkeys HashMap (bug!)");
        let nonce = secretbox::gen_nonce();

        (
            secretbox::seal(message, &nonce, key),
            nonce,
            self.active_key_hash.as_ref(),
        )
    }

    pub fn decrypt(
        &self,
        ciphertext: &[u8],
        nonce: &Nonce,
        key_hash: &Hash,
    ) -> SymmetricCryptoResult<Vec<u8>> {
        let key = self
            .donkeys
            .get(key_hash)
            .ok_or(SymmetricCryptoError::MissingDonkeyForHash)?;

        secretbox::open(ciphertext, nonce, key).map_err(|_| SymmetricCryptoError::DecryptionFailed)
    }
}

#[cfg(test)]
mod tests {
    use tempfile::NamedTempFile;

    use super::*;

    #[test]
    fn encryption_decryption_round_trip() {
        let key = SymmetricCryptoService::generate_key();
        let service = SymmetricCryptoService::new(key, vec![]);

        let message = b"Leave the gun. Take the cannoli.";

        let (ciphertext, nonce, key_hash) = service.encrypt(message);

        let decrypted = service
            .decrypt(ciphertext.as_ref(), &nonce, key_hash)
            .expect("Should be able to decrypt");

        assert_eq!(message.as_slice(), decrypted);
    }

    #[test]
    fn key_rotation() {
        let old_key = SymmetricCryptoService::generate_key();
        let old_service = SymmetricCryptoService::new(old_key.clone(), vec![]);

        let message = b"My father made him an offer he couldn't refuse.";

        let (ciphertext, nonce, old_key_hash) = old_service.encrypt(message);

        let new_key = SymmetricCryptoService::generate_key();
        let new_service = SymmetricCryptoService::new(new_key, vec![old_key]);

        let decrypted = new_service
            .decrypt(ciphertext.as_ref(), &nonce, old_key_hash)
            .expect("Should be able to decrypt");

        assert_eq!(message.as_slice(), decrypted);
    }

    #[test]
    fn missing_key() {
        let old_key = SymmetricCryptoService::generate_key();
        let old_service = SymmetricCryptoService::new(old_key.clone(), vec![]);

        let message = b"My father made him an offer he couldn't refuse.";

        let (ciphertext, nonce, old_key_hash) = old_service.encrypt(message);

        let new_key = SymmetricCryptoService::generate_key();
        let new_service = SymmetricCryptoService::new(new_key, vec![]);

        let result = new_service.decrypt(ciphertext.as_ref(), &nonce, old_key_hash);

        assert!(matches!(
            result,
            Err(SymmetricCryptoError::MissingDonkeyForHash)
        ));
    }

    #[tokio::test]
    async fn filesystem_round_trip() {
        let key = SymmetricCryptoService::generate_key();

        let file = NamedTempFile::new().expect("Should create temp file");
        key.save(file.path()).await.expect("Should write to file");

        let loaded_key = SymmetricKey::load(file.path())
            .await
            .expect("Should load from file");

        assert_eq!(key, loaded_key);
    }
}
