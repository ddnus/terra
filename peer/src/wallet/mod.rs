//! A wallet for managing accounts and signing/verifying transactions.

use k256::{
    ecdsa::{SigningKey, VerifyingKey},
    schnorr::signature::DigestVerifier,
};
use rand_core::OsRng;
use serde::Deserialize;
use sha3::{Digest, Keccak256};

mod signature;

pub use signature::Signature;

use crate::{db::Db, Error};

#[derive(Debug, Clone)]
pub struct Wallet {
    db: Db,
}

impl Wallet {
    /// Create a new wallet.
    pub fn new(db: Db) -> Self {
        Self {
            db,
        }
    }

    /// Create a new account.
    pub fn new_account(&self) -> Result<String, Error> {
        let privkey = SigningKey::random(&mut OsRng);
        let address = gen_address(&privkey);
        let key_bytes = privkey.to_bytes().to_vec();
        self.db.set(address.as_bytes(), key_bytes, None);

        Ok(address)
    }

    /// Sign a message.
    pub fn sign(&self, msg: &[u8], addr: &str) -> Result<Signature, Error> {
        let privkey = self.get_privkey(addr)?;
        let digest = Keccak256::new_with_prefix(msg);
        let (sig, recid) = privkey.sign_digest_recoverable(digest)?;

        Ok(Signature::from((sig, recid)))
    }

    fn get_privkey(&self, addr: &str) -> Result<SigningKey, Error> {
        let privkey = self
            .db
            .get(addr.as_bytes())?
            .map(|k| k.to_vec())
            .ok_or_else(|| Error::AccountNotFound(addr.to_string()))?;

        SigningKey::from_bytes(privkey.as_slice().into())
            .map_err(|_| Error::AccountNotFound(addr.to_string()))
    }
}

/// Verify a signature for a message, does not require a wallet.
pub fn verify_signature(msg: &[u8], signature: impl Into<Signature>) -> Result<(), Error> {
    let signature = signature.into();
    let (sig, recid) = signature.try_into()?;
    let digest = Keccak256::new_with_prefix(msg);

    let recovered_key = VerifyingKey::recover_from_digest(digest.clone(), &sig, recid)
        .map_err(|_| Error::InvalidSignature)?;

    recovered_key
        .verify_digest(digest, &sig)
        .map_err(|_| Error::InvalidSignature)?;

    Ok(())
}

fn gen_address(privkey: &SigningKey) -> String {
    let pubkey = privkey.verifying_key().to_encoded_point(false);
    let pubkey = pubkey.as_bytes();
    let hash = Keccak256::digest(&pubkey[1..]);

    let mut bytes = [0u8; 20];
    bytes.copy_from_slice(&hash[12..]);
    String::from("0x") + &hex::encode(bytes)
}

#[derive(Clone, Debug, Default, Deserialize)]
pub struct WalletConfig {
    pub keystore_dir: String,
}

#[cfg(test)]
mod tests {
    use std::fs;

    use super::*;

    #[test]
    fn wallet_works() {
        let tmpdir = tempdir_with_prefix("tmp");
        let _ = fs::remove_dir_all(&tmpdir);

        let wallet = Wallet::new(&tmpdir);
        let addr = wallet.new_account().unwrap();
        let msg = b"hello world";
        let sig = wallet.sign(msg, &addr).unwrap();

        assert!(verify_signature(msg, sig).is_ok());
    }

    fn tempdir_with_prefix(prefix: &str) -> String {
        tempfile::Builder::new()
            .prefix(prefix)
            .tempdir()
            .unwrap()
            .path()
            .to_str()
            .unwrap()
            .to_string()
    }
}
