use aes_gcm::{
    aead::{Aead, KeyInit},
    Aes256Gcm, Nonce,
};
use base64::{engine::general_purpose::STANDARD_NO_PAD, Engine};
use rand::{rngs::OsRng, RngCore};

pub(crate) fn new_key() -> [u8; 32] {
    let mut key = [0_u8; 32];
    OsRng.fill_bytes(&mut key);
    key
}

pub(crate) fn seal(key: &[u8; 32], plaintext: &[u8]) -> Result<String, String> {
    let cipher =
        Aes256Gcm::new_from_slice(key).map_err(|_| "ledger key has invalid length".to_owned())?;
    let mut nonce = [0_u8; 12];
    OsRng.fill_bytes(&mut nonce);
    let ciphertext = cipher
        .encrypt(Nonce::from_slice(&nonce), plaintext)
        .map_err(|_| "ledger encryption failed".to_owned())?;
    let mut packed = nonce.to_vec();
    packed.extend(ciphertext);
    Ok(STANDARD_NO_PAD.encode(packed))
}

pub(crate) fn open(key: &[u8; 32], encoded: &[u8]) -> Result<Vec<u8>, String> {
    let cipher =
        Aes256Gcm::new_from_slice(key).map_err(|_| "ledger key has invalid length".to_owned())?;
    let packed = STANDARD_NO_PAD
        .decode(encoded)
        .map_err(|_| "encrypted metadata is malformed".to_owned())?;
    if packed.len() < 12 {
        return Err("encrypted metadata is malformed".into());
    }
    let (nonce, ciphertext) = packed.split_at(12);
    cipher
        .decrypt(Nonce::from_slice(nonce), ciphertext)
        .map_err(|_| "encrypted metadata could not be opened".to_owned())
}
