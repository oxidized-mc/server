# Security Policy

## Supported Versions

| Version | Supported |
|---------|-----------|
| `main` (latest) | ✅ |
| older releases | ❌ |

## Reporting a Vulnerability

**Please do not open a public GitHub issue for security vulnerabilities.**

Report security issues by emailing **security@your-org.example** (replace with
your actual address) or by using
[GitHub's private vulnerability reporting](https://docs.github.com/en/code-security/security-advisories/guidance-on-reporting-and-writing/privately-reporting-a-security-vulnerability).

Include:
- A description of the vulnerability and its impact
- Steps to reproduce (proof-of-concept if possible)
- Affected versions/commits

We will acknowledge your report within **72 hours** and aim to release a fix
within **7 days** for critical issues.

## Scope

This project is a **server-side** implementation. Relevant vulnerability classes:

- Remote code execution via malformed packets
- Authentication bypass (online-mode circumvention)
- Denial of service via packet flooding or malformed chunk/NBT data
- Path traversal in world file loading
- Server property or secret exposure via RCON/JSON-RPC

Out of scope: client-side rendering bugs, vanilla client exploits.
