use aes::{Aes128, Aes256};
use cipher::{BlockDecrypt, BlockEncrypt, KeyInit};
use generic_array::GenericArray;

pub fn pkcs7_pad(data: &[u8]) -> Vec<u8> {
    let block_size = 16;
    let pad_len = block_size - (data.len() % block_size);
    let mut padded = data.to_vec();
    padded.extend(std::iter::repeat_n(pad_len as u8, pad_len));
    padded
}

pub fn pkcs7_unpad(data: &[u8]) -> Result<Vec<u8>, String> {
    if data.is_empty() || !data.len().is_multiple_of(16) {
        return Err("invalid ciphertext length".to_string());
    }
    let pad_len = data[data.len() - 1] as usize;
    if pad_len == 0 || pad_len > 16 {
        return Ok(data.to_vec());
    }
    if pad_len > data.len() {
        return Ok(data.to_vec());
    }
    let start = data.len() - pad_len;
    if data[start..].iter().all(|&b| b == pad_len as u8) {
        Ok(data[..start].to_vec())
    } else {
        Ok(data.to_vec())
    }
}

pub fn aes_padded_size(size: usize) -> usize {
    size + (16 - (size % 16))
}

pub fn parse_media_aes_key(aes_key_value: &str) -> Result<Vec<u8>, String> {
    let normalized = aes_key_value.trim();
    if normalized.is_empty() {
        return Err("empty media aes key".to_string());
    }
    let padding_len = (4 - normalized.len() % 4) % 4;
    let padded = format!("{normalized}{}", "=".repeat(padding_len));
    let decoded = base64::Engine::decode(
        &base64::engine::general_purpose::STANDARD,
        padded.as_bytes(),
    )
    .map_err(|e| format!("base64 decode failed: {e}"))?;

    if decoded.len() == 16 || decoded.len() == 32 {
        return Ok(decoded);
    }

    let decoded_text = String::from_utf8_lossy(&decoded);
    if decoded.len() == 64
        && decoded_text
            .chars()
            .all(|c| c.is_ascii_hexdigit())
    {
        let hex_decoded = hex::decode(decoded_text.as_ref())
            .map_err(|e| format!("hex decode failed: {e}"))?;
        if hex_decoded.len() == 32 {
            return Ok(hex_decoded);
        }
    }

    Err(format!(
        "unsupported media aes key format: len={}",
        decoded.len()
    ))
}

fn aes_ecb_encrypt_impl<C: BlockEncrypt + KeyInit>(key: &[u8], data: &[u8]) -> Vec<u8> {
    let cipher = C::new(GenericArray::from_slice(key));
    let padded = pkcs7_pad(data);
    let mut result = vec![0u8; padded.len()];
    for (chunk, out) in padded.chunks(16).zip(result.chunks_mut(16)) {
        let mut block = GenericArray::clone_from_slice(chunk);
        cipher.encrypt_block(&mut block);
        out.copy_from_slice(&block);
    }
    result
}

fn aes_ecb_decrypt_impl<C: BlockDecrypt + KeyInit>(key: &[u8], data: &[u8]) -> Result<Vec<u8>, String> {
    if data.is_empty() || !data.len().is_multiple_of(16) {
        return Err("invalid ciphertext length for AES decryption".to_string());
    }
    let cipher = C::new(GenericArray::from_slice(key));
    let mut result = vec![0u8; data.len()];
    for (chunk, out) in data.chunks(16).zip(result.chunks_mut(16)) {
        let mut block = GenericArray::clone_from_slice(chunk);
        cipher.decrypt_block(&mut block);
        out.copy_from_slice(&block);
    }
    pkcs7_unpad(&result)
}

pub fn aes_ecb_encrypt(key: &[u8], data: &[u8]) -> Result<Vec<u8>, String> {
    match key.len() {
        16 => Ok(aes_ecb_encrypt_impl::<Aes128>(key, data)),
        32 => Ok(aes_ecb_encrypt_impl::<Aes256>(key, data)),
        other => Err(format!("unsupported AES key length: {other} (expected 16 or 32)")),
    }
}

pub fn aes_ecb_decrypt(key: &[u8], data: &[u8]) -> Result<Vec<u8>, String> {
    match key.len() {
        16 => aes_ecb_decrypt_impl::<Aes128>(key, data),
        32 => aes_ecb_decrypt_impl::<Aes256>(key, data),
        other => Err(format!("unsupported AES key length: {other} (expected 16 or 32)")),
    }
}
