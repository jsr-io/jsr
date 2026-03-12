// Copyright 2024 the JSR authors. All rights reserved. MIT license.
// Copyright Deno Land Inc. All Rights Reserved. Proprietary and confidential.

use crate::db::*;
use chrono::DateTime;
use chrono::Utc;
use sha2::Digest;
use uuid::Uuid;

/// Generate a SHA256 hash of a string.
pub fn hash(data: &str) -> String {
  format!("{:x}", sha2::Sha256::digest(data.as_bytes()))
}

pub async fn create_token(
  db: &Database,
  user_id: Uuid,
  token_type: TokenType,
  description: Option<String>,
  expires_at: Option<DateTime<Utc>>,
  permissions: Option<Permissions>,
) -> anyhow::Result<String> {
  let token_string = generate_token(token_type);
  let hashed_token = hash(&token_string);

  db.insert_token(NewToken {
    hash: hashed_token,
    user_id,
    r#type: token_type,
    description,
    expires_at,
    permissions,
  })
  .await?;

  Ok(token_string)
}

const MAX_DECODED_LEN: usize = 111;
const BASE62: &[u8] =
  b"0123456789ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz";

pub fn generate_token(token_type: TokenType) -> String {
  let prefix = token_type.prefix();
  let mut random_string = String::new();
  for _ in 0..29 {
    random_string.push(BASE62[rand::random::<usize>() % 62] as char);
  }
  let token_without_checksum = format!("{prefix}_{random_string}");
  let mut hasher = crc32fast::Hasher::new();
  hasher.update(token_without_checksum.as_bytes());
  let checksum = hasher.finalize();
  let checksum_string = encode_base62(checksum);
  // token + 0 left padded checksum
  let token = format!("{token_without_checksum}{checksum_string:0>6}");
  debug_assert!(token.len() == 40);
  token
}

pub fn encode_base62(mut num: u32) -> String {
  if num == 0 {
    return "0".to_owned();
  }
  let mut bytes = [0_u8; MAX_DECODED_LEN];

  let mut i: usize = MAX_DECODED_LEN;
  loop {
    if num == 0 {
      break;
    }
    i -= 1;

    bytes[i] = BASE62[(num % 62) as usize];
    num /= 62;
  }

  String::from_utf8(bytes[i..MAX_DECODED_LEN].to_vec()).unwrap()
}

#[cfg(test)]
mod tests {
  use super::TokenType;
  use super::encode_base62;
  use super::generate_token;

  #[test]
  fn test_encode_base62() {
    assert_eq!(encode_base62(0), "0");
    assert_eq!(encode_base62(1), "1");
    assert_eq!(encode_base62(62), "10");
    assert_eq!(encode_base62(72), "1A");
  }

  #[test]
  fn test_generate_token() {
    let token = generate_token(TokenType::Web);
    println!("{}", token);
    assert_eq!(token.len(), 40);
    assert!(token.starts_with("jsrw_"));
    assert!(!token.contains(' '));
  }
}
