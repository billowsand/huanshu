use aes_gcm::{
    aead::{Aead, KeyInit},
    Aes256Gcm, Nonce,
};
use argon2::{password_hash::SaltString, Argon2, PasswordHasher};
use base64::{engine::general_purpose::STANDARD as BASE64, Engine};
use rand::{rngs::OsRng, RngCore};
use serde::{Deserialize, Serialize};

const TEST_PLAINTEXT: &str = "KNN_VALIDATION_TOKEN_v1";
const MAGIC: &str = "KNN2";
const VERSION: &str = "2";

#[derive(Debug, Serialize, Deserialize)]
pub struct CryptoHeader {
    pub magic: String,
    pub version: String,
    pub algorithm: String,
    pub kdf: String,
    pub salt: String,
    pub nonce: String,
    pub test_nonce: String,
    pub test_ciphertext: String,
    pub original_name: String,
}

pub struct CryptoService;

impl CryptoService {
    pub fn derive_key(password: &str, salt: &[u8]) -> Result<[u8; 32], String> {
        let salt_b64 = BASE64.encode(salt);
        let salt_str =
            SaltString::from_b64(&salt_b64).map_err(|e| format!("Failed to create salt: {}", e))?;

        let argon2 = Argon2::new(
            argon2::Algorithm::Argon2id,
            argon2::Version::V0x13,
            argon2::Params::new(65536, 3, 4, Some(32))
                .map_err(|e| format!("Invalid Argon2 params: {}", e))?,
        );

        let hash = argon2
            .hash_password(password.as_bytes(), &salt_str)
            .map_err(|e| format!("Key derivation failed: {}", e))?;

        let hash_bytes = hash.hash.ok_or("No hash output")?;
        let mut key = [0u8; 32];
        key.copy_from_slice(&hash_bytes.as_bytes()[..32]);
        Ok(key)
    }

    pub fn generate_salt() -> [u8; 16] {
        let mut salt = [0u8; 16];
        OsRng.fill_bytes(&mut salt);
        salt
    }

    pub fn generate_nonce() -> [u8; 12] {
        let mut nonce = [0u8; 12];
        OsRng.fill_bytes(&mut nonce);
        nonce
    }

    pub fn encrypt(data: &[u8], password: &str, original_name: &str) -> Result<Vec<u8>, String> {
        let salt = Self::generate_salt();
        let nonce_bytes = Self::generate_nonce();
        let test_nonce_bytes = Self::generate_nonce();
        let key = Self::derive_key(password, &salt)?;

        let cipher = Aes256Gcm::new_from_slice(&key)
            .map_err(|e| format!("Failed to create cipher: {}", e))?;
        let nonce = Nonce::from_slice(&nonce_bytes);

        let ciphertext = cipher
            .encrypt(nonce, data)
            .map_err(|e| format!("Encryption failed: {}", e))?;

        let test_ciphertext = {
            let test_cipher = Aes256Gcm::new_from_slice(&key)
                .map_err(|e| format!("Failed to create test cipher: {}", e))?;
            let test_nonce = Nonce::from_slice(&test_nonce_bytes);
            test_cipher
                .encrypt(test_nonce, TEST_PLAINTEXT.as_bytes())
                .map_err(|e| format!("Test encryption failed: {}", e))?
        };

        let header = CryptoHeader {
            magic: MAGIC.to_string(),
            version: VERSION.to_string(),
            algorithm: "AES-256-GCM".to_string(),
            kdf: "Argon2id".to_string(),
            salt: BASE64.encode(salt),
            nonce: BASE64.encode(nonce_bytes),
            test_nonce: BASE64.encode(test_nonce_bytes),
            test_ciphertext: BASE64.encode(test_ciphertext),
            original_name: original_name.to_string(),
        };

        let header_json = serde_json::to_string(&header)
            .map_err(|e| format!("Header serialization failed: {}", e))?;

        let mut result = Vec::new();
        result.extend_from_slice(header_json.as_bytes());
        result.push(b'\n');
        result.extend_from_slice(&ciphertext);

        Ok(result)
    }

    pub fn decrypt(data: &[u8], password: &str) -> Result<Vec<u8>, String> {
        let separator_idx = data
            .iter()
            .position(|&b| b == b'\n')
            .ok_or("Invalid file format: missing separator")?;

        let header_json = &data[..separator_idx];
        let ciphertext = &data[separator_idx + 1..];

        let header: CryptoHeader =
            serde_json::from_slice(header_json).map_err(|e| format!("Invalid header: {}", e))?;

        if header.magic != MAGIC {
            return Err(format!(
                "Invalid file format: expected {}, got {}",
                MAGIC, header.magic
            ));
        }

        if header.version != VERSION {
            return Err(format!("Unsupported version: {}", header.version));
        }

        let salt = BASE64
            .decode(&header.salt)
            .map_err(|e| format!("Invalid salt: {}", e))?;
        let nonce_bytes = BASE64
            .decode(&header.nonce)
            .map_err(|e| format!("Invalid nonce: {}", e))?;
        let test_nonce_bytes = BASE64
            .decode(&header.test_nonce)
            .map_err(|e| format!("Invalid test nonce: {}", e))?;
        let test_ciphertext = BASE64
            .decode(&header.test_ciphertext)
            .map_err(|e| format!("Invalid test ciphertext: {}", e))?;

        let key = Self::derive_key(password, &salt)?;

        let test_cipher = Aes256Gcm::new_from_slice(&key)
            .map_err(|e| format!("Failed to create test cipher: {}", e))?;
        let test_nonce = Nonce::from_slice(&test_nonce_bytes);

        test_cipher
            .decrypt(test_nonce, test_ciphertext.as_ref())
            .map_err(|_| "Invalid password")?;

        let cipher = Aes256Gcm::new_from_slice(&key)
            .map_err(|e| format!("Failed to create cipher: {}", e))?;
        let nonce = Nonce::from_slice(&nonce_bytes);

        cipher
            .decrypt(nonce, ciphertext)
            .map_err(|e| format!("Decryption failed: {}", e))
    }

    pub fn validate_password(data: &[u8], password: &str) -> Result<bool, String> {
        let separator_idx = data
            .iter()
            .position(|&b| b == b'\n')
            .ok_or("Invalid file format: missing separator")?;

        let header_json = &data[..separator_idx];

        let header: CryptoHeader =
            serde_json::from_slice(header_json).map_err(|e| format!("Invalid header: {}", e))?;

        let salt = BASE64
            .decode(&header.salt)
            .map_err(|e| format!("Invalid salt: {}", e))?;
        let test_nonce_bytes = BASE64
            .decode(&header.test_nonce)
            .map_err(|e| format!("Invalid test nonce: {}", e))?;
        let test_ciphertext = BASE64
            .decode(&header.test_ciphertext)
            .map_err(|e| format!("Invalid test ciphertext: {}", e))?;

        let key = Self::derive_key(password, &salt)?;
        let test_cipher = Aes256Gcm::new_from_slice(&key)
            .map_err(|e| format!("Failed to create test cipher: {}", e))?;
        let test_nonce = Nonce::from_slice(&test_nonce_bytes);

        match test_cipher.decrypt(test_nonce, test_ciphertext.as_ref()) {
            Ok(_) => Ok(true),
            Err(_) => Ok(false),
        }
    }

    pub fn is_encrypted(data: &[u8]) -> bool {
        let separator_idx = match data.iter().position(|&b| b == b'\n') {
            Some(idx) => idx,
            None => return false,
        };
        let header_json = &data[..separator_idx];
        if let Ok(header) = serde_json::from_slice::<CryptoHeader>(header_json) {
            return header.magic == MAGIC;
        }
        false
    }
}

pub fn validate_password_strength(password: &str) -> Result<(), String> {
    if password.len() < 8 {
        return Err("Password must be at least 8 characters".to_string());
    }

    let has_lower = password.chars().any(|c| c.is_ascii_lowercase());
    let has_upper = password.chars().any(|c| c.is_ascii_uppercase());
    let has_digit = password.chars().any(|c| c.is_ascii_digit());

    if !has_lower {
        return Err("Password must contain at least one lowercase letter".to_string());
    }
    if !has_upper {
        return Err("Password must contain at least one uppercase letter".to_string());
    }
    if !has_digit {
        return Err("Password must contain at least one digit".to_string());
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_encrypt_decrypt_roundtrip() {
        let data = b"Hello, World!";
        let password = "TestPassword123";
        let original_name = "test.keynn";

        let encrypted = CryptoService::encrypt(data, password, original_name).unwrap();
        assert!(encrypted.len() > data.len());

        let decrypted = CryptoService::decrypt(&encrypted, password).unwrap();
        assert_eq!(decrypted, data);
    }

    #[test]
    fn test_wrong_password() {
        let data = b"Secret data";
        let password = "CorrectPassword123";
        let wrong_password = "WrongPassword123";

        let encrypted = CryptoService::encrypt(data, password, "test.keynn").unwrap();
        let result = CryptoService::decrypt(&encrypted, wrong_password);
        assert!(result.is_err());
    }

    #[test]
    fn test_validate_password_strength() {
        assert!(validate_password_strength("Password123").is_ok());
        assert!(validate_password_strength("pass").is_err());
        assert!(validate_password_strength("PASSWORD123").is_err());
        assert!(validate_password_strength("password").is_err());
        assert!(validate_password_strength("Pass1234").is_ok());
    }
}
