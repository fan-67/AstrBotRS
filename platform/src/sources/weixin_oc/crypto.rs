use aes::Aes256;
use cipher::{BlockDecrypt, BlockEncrypt, KeyInit};
use generic_array::GenericArray;

pub fn pkcs7_pad(data: &[u8]) -> Vec<u8> {
    let block_size = 16;
    let pad_len = block_size - (data.len() % block_size);
    let mut padded = data.to_vec();
    padded.extend(std::iter::repeat(pad_len as u8).take(pad_len));
    padded
}

pub fn pkcs7_unpad(data: &[u8]) -> Vec<u8> {
    if data.is_empty() {
        return data.to_vec();
    }
    let pad_len = data[data.len() - 1] as usize;
    if pad_len == 0 || pad_len > 16 {
        return data.to_vec();
    }
    let start = data.len() - pad_len;
    if data[start..].iter().all(|&b| b == pad_len as u8) {
        data[..start].to_vec()
    } else {
        data.to_vec()
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

    if decoded.len() == 16 {
        return Ok(decoded);
    }

    let decoded_text = String::from_utf8_lossy(&decoded);
    if decoded.len() == 32
        && decoded_text
            .chars()
            .all(|c| c.is_ascii_hexdigit())
    {
        return hex::decode(decoded_text.as_ref())
            .map_err(|e| format!("hex decode failed: {e}"));
    }

    Err(format!(
        "unsupported media aes key format: len={}",
        decoded.len()
    ))
}

pub fn aes_ecb_encrypt(key: &[u8], data: &[u8]) -> Vec<u8> {
    let cipher = Aes256::new(GenericArray::from_slice(key));
    let padded = pkcs7_pad(data);
    let mut result = vec![0u8; padded.len()];
    for (chunk, out) in padded.chunks(16).zip(result.chunks_mut(16)) {
        let mut block = GenericArray::clone_from_slice(chunk);
        cipher.encrypt_block(&mut block);
        out.copy_from_slice(&block);
    }
    result
}

pub fn aes_ecb_decrypt(key: &[u8], data: &[u8]) -> Vec<u8> {
    let cipher = Aes256::new(GenericArray::from_slice(key));
    let mut result = vec![0u8; data.len()];
    for (chunk, out) in data.chunks(16).zip(result.chunks_mut(16)) {
        let mut block = GenericArray::clone_from_slice(chunk);
        cipher.decrypt_block(&mut block);
        out.copy_from_slice(&block);
    }
    pkcs7_unpad(&result)
}
