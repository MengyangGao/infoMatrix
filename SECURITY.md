# Security Policy

## Supported Versions

| Version | Supported          |
| ------- | ------------------ |
| 0.1.x   | :white_check_mark: |

## Reporting a Vulnerability

If you discover a security vulnerability in InfoMatrix, please report it privately via email:

**gao.mengyang@outlook.com**

Please include:
- A clear description of the vulnerability
- Steps to reproduce (if applicable)
- Potential impact assessment
- Any suggested fixes or mitigations

We aim to respond within 5 business days and will coordinate a fix and disclosure timeline with you.

## Security Design Principles

- **Local-first**: All feed data, read-later items, and memos are stored locally in SQLite by default.
- **No telemetry**: InfoMatrix does not collect analytics or crash reports by default.
- **Deterministic fetching**: Feed refresh behavior is deterministic and testable.
- **Network isolation**: Networking is handled entirely in Rust core, isolated from UI layers.
