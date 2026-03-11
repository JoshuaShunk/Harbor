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
}
