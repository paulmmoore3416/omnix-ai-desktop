# Licensing and editions

OMNIX is proprietary software (see [LICENSE](../LICENSE)). This document covers the commercial editions and how
license keys work. It is for the owner and anyone issuing licenses.

## Editions

| Edition | Price model | For | Covers |
|---|---|---|---|
| **Community** | free (no key) | trying OMNIX | Everything the app does today |
| **Lifetime Pro** | one-time, $50–$100 | local power users | Advanced GPU monitoring and history, unlimited local memory, v1.x updates |
| **BYOK** | subscription, $10–$15 / month | people who want the OMNIX workspace with cloud models | Everything in Pro while subscribed, plus cloud providers with the user's own API keys |
| **Enterprise Hardened** | custom, outcome-based | firms that need air-gapped installs | Everything above, hardening review, custom support agreement, seat count |

**Status: informational.** The app verifies and shows the tier (Settings → License) but locks nothing behind it.
`LicenseStatus.enforced` is always `false`. Gating would be a later, deliberate change: it would need tests, a
changelog entry, and a decision on what Community keeps.

## How a license key works

A key is a single line:

```
OMNIX1.<base64url(payload JSON)>.<base64url(Ed25519 signature)>
```

- **Payload:** `v`, `id` (UUID), `kid` (signing key id), `tier` (`pro` | `byok` | `enterprise`), `licensee`,
  `email`, `issued`, `expires` (none = lifetime; **required for BYOK**), `seats`.
- **Signature:** Ed25519 over `omnix-license-v1.` followed by the payload part. The domain prefix means the key can't
  be tricked into signing something else that happens to verify.
- **Verification** (`src-tauri/src/license.rs`) is offline against `TRUSTED_KEYS`, which is compiled into the app.
  It uses `verify_strict`, so malleable signatures are rejected. Nothing phones home, which makes keys work
  air-gapped and leaves `local_only` untouched.
- **Expiry:** an expired key shows as expired and the effective tier drops to Community. Renewing a BYOK
  subscription means issuing a new key with a later `expires`.
- **Storage:** `~/.config/omnix/license.key` (mode 600). Installing or removing a key is audited with the license
  id and tier only, never the key or the licensee.

## Issuing licenses

The vendor tool is `scripts/omnix-license.py`. It needs `cryptography` and `keyring`, and the kb-core venv has both:

```bash
PY=~/.local/share/omnix/kb-core/venv/bin/python
$PY scripts/omnix-license.py issue --tier pro --licensee "Jane Doe" --email jane@example.com
$PY scripts/omnix-license.py issue --tier byok --licensee "Acme Ltd" --months 1
$PY scripts/omnix-license.py issue --tier enterprise --licensee "Clinic Group" --seats 25 --expires 2027-12-31
$PY scripts/omnix-license.py verify OMNIX1.…
```

The private signing key lives **only in the OS keyring**, under service `omnix-license-signing`, user `k1`. It was
created on 2026-09-26 with `keygen`. It is never printed or written to disk.

- **Back it up** with your keyring tool (Seahorse → Passwords → Login → `omnix-license-signing`), somewhere
  offline. If it's lost, keys already issued keep working, but new ones can't be signed with `k1`.
- **Rotation:** `$PY scripts/omnix-license.py --kid k2 keygen` prints a new `TRUSTED_KEYS` line. Add it to
  `license.rs` and ship a release, then issue with `--kid k2`. Keep `k1` in the list while keys signed with it are in
  use.
- **Compromise:** remove the key from `TRUSTED_KEYS` in a release. Every key it signed stops verifying, so reissue
  those with a new key.

`license.rs` has an interop test that verifies a key signed by the Python tool with the real `k1` key. That fixture
expired on 2026-09-27, so it grants nothing.
