# Security Policy

## Supported Versions

| Version | Supported |
|---------|-----------|
| 0.1.x   | ✓         |

## Reporting a Vulnerability

**Do not open a public GitHub issue for security vulnerabilities.**

Please report security issues by emailing: **security@veil-project.com**

Include:
- Description of the vulnerability
- Steps to reproduce
- Potential impact
- Suggested fix (if any)

You will receive a response within **48 hours** and a fix timeline within **7 days** for critical issues.

## Scope

In scope:
- Authentication bypass
- Key leakage / cryptographic weaknesses
- Remote code execution
- Information disclosure (IP/DNS leaks)
- Protocol downgrade attacks
- Anti-probing bypass

Out of scope:
- DoS attacks requiring physical access
- Social engineering
- Issues in dependencies (report to the respective project)

## Cryptography

Veil uses only standard, audited cryptographic primitives:
- **TLS 1.3** via `rustls`
- **QUIC** via `quinn`
- **AES-256-GCM** for AEAD
- **HMAC-SHA256** for token signing
- No custom cryptography

## Responsible Disclosure

We follow a **90-day disclosure timeline**. After 90 days (or when a patch is released, whichever comes first), the reporter may publish their findings.
