# Security Policy

## Reporting a Vulnerability

If you discover a security vulnerability in Harbor, please report it responsibly.

**Do not open a public issue.**

Instead, email **security@harbormcp.ai** with:

- A description of the vulnerability
- Steps to reproduce
- Potential impact

You should receive a response within 48 hours. We will work with you to understand the issue and coordinate a fix before any public disclosure.

## Supported Versions

| Version | Supported |
|---------|-----------|
| 0.5.x   | Yes       |
| 0.3.x   | Yes       |
| 0.2.x   | No        |
| < 0.2   | No        |

## Security Considerations

### Vault / Keychain

Harbor stores secrets in your operating system's keychain (macOS Keychain, Windows Credential Manager, or Linux Secret Service). Keep in mind:

- **Vault references** (`vault:SECRET_NAME`) are resolved at runtime by the gateway — secrets are never written to plain-text config files
- **Fleet sync** only commits vault references, not actual secrets — teammates must run `harbor crew provision` to stow their own credentials
- **Host configs** receive resolved environment variables only when the gateway bridges a server connection
- **Keychain access** may prompt for permission when Harbor first accesses a secret after a binary update or system restart
