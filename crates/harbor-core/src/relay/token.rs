//! Bearer token generation and validation for relay authentication.
//!
//! Two token types:
//! - **Relay auth tokens**: Used by the publish client to authenticate with the relay.
//! - **Remote access tokens**: Used by remote MCP clients to authenticate with the relay.
//!
//! Remote access tokens can be scoped to specific tools and have expiration times.
//! Phase 1 uses simple random bearer tokens. Phase 5 adds JWT with claims.

/// Generate a cryptographically secure random bearer token.
///
/// Format: `hbr_` prefix + 32 random hex characters (128 bits of entropy).
pub fn generate_bearer_token() -> String {
    use rand::Rng;
    let mut rng = rand::thread_rng();
    let bytes: [u8; 16] = rng.gen();
    format!("hbr_{}", hex::encode(bytes))
}

/// Validate a bearer token format (basic check).
pub fn validate_token_format(token: &str) -> bool {
    token.starts_with("hbr_") && token.len() == 36 // "hbr_" + 32 hex chars
}

// TODO Phase 5: JWT token generation with scoped claims
// pub struct TokenClaims { sub, tunnel_id, tools, exp, iat, jti }
// pub fn generate_jwt(claims: &TokenClaims, secret: &[u8]) -> Result<String>;
// pub fn validate_jwt(token: &str, secret: &[u8]) -> Result<TokenClaims>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_generate_token() {
        let token = generate_bearer_token();
        assert!(token.starts_with("hbr_"));
        assert_eq!(token.len(), 36);
    }

    #[test]
    fn test_tokens_are_unique() {
        let t1 = generate_bearer_token();
        let t2 = generate_bearer_token();
        assert_ne!(t1, t2);
    }

    #[test]
    fn test_validate_format() {
        let token = generate_bearer_token();
        assert!(validate_token_format(&token));
        assert!(!validate_token_format("invalid"));
        assert!(!validate_token_format("hbr_tooshort"));
    }

    #[test]
    fn test_token_prefix() {
        let token = generate_bearer_token();
        assert!(token.starts_with("hbr_"));
    }

    #[test]
    fn test_token_hex_chars() {
        let token = generate_bearer_token();
        let hex_part = &token[4..]; // after "hbr_"
        assert!(hex_part.chars().all(|c| c.is_ascii_hexdigit()));
    }

    #[test]
    fn test_validate_empty_string() {
        assert!(!validate_token_format(""));
    }

    #[test]
    fn test_validate_wrong_prefix() {
        assert!(!validate_token_format("abc_0123456789abcdef0123456789ab"));
    }

    #[test]
    fn test_validate_too_long() {
        assert!(!validate_token_format(
            "hbr_0123456789abcdef0123456789abextra"
        ));
    }

    #[test]
    fn test_validate_correct_length() {
        // Exactly 36 chars: "hbr_" (4) + 32 hex chars
        let valid = "hbr_0123456789abcdef0123456789abcdef";
        assert_eq!(valid.len(), 36);
        assert!(validate_token_format(valid));
    }

    #[test]
    fn test_generated_tokens_validate() {
        for _ in 0..10 {
            let token = generate_bearer_token();
            assert!(
                validate_token_format(&token),
                "Generated token should be valid: {}",
                token
            );
        }
    }

    #[test]
    fn test_token_uniqueness_many() {
        let tokens: std::collections::HashSet<String> =
            (0..100).map(|_| generate_bearer_token()).collect();
        // All should be unique
        assert_eq!(tokens.len(), 100);
    }
}
