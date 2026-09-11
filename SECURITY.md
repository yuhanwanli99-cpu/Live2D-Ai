# Security Policy

## Supported versions

Only the latest PC Preview release receives best-effort security fixes. Android, native Windows, and unreleased development branches are not supported release targets.

## Reporting a vulnerability

Do not open a public issue containing API keys, access tokens, private conversations, screenshots with personal information, or exploit details. Use GitHub's private vulnerability reporting feature for this repository. If it is unavailable, contact the repository owner through the GitHub profile before disclosing details.

Include the affected version, reproduction steps, expected impact, and a minimally redacted log. Never attach a real secret. Revoke exposed provider keys immediately.

## Security defaults

The PC service is intended to bind locally by default. LAN access and external text input must be explicitly enabled. Users supply their own provider credentials; the project does not ship shared keys.
