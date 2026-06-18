# Security Policy

## 🔒 Security Overview

OMNIX takes security seriously. This document outlines our security practices, how to report vulnerabilities, and best practices for users.

## Supported Versions

| Version | Supported          |
| ------- | ------------------ |
| 1.0.x   | :white_check_mark: |
| < 1.0   | :x:                |

## 🛡️ Security Features

### Built-in Security Measures

1. **Privilege Escalation Protection**
   - User consent required for all sudo operations
   - Audit logging of elevated commands
   - Configurable confirmation prompts

2. **Data Encryption**
   - Sensitive data encrypted at rest
   - Secure credential storage
   - API keys never logged or exposed

3. **Sandboxed Execution**
   - Commands run in controlled environment
   - File system access restrictions
   - Network isolation options

4. **Audit Trail**
   - All system operations logged
   - Timestamped command history
   - User action tracking

## 🔐 Best Practices for Users

### Environment Configuration

1. **Never commit `.env` files**
   ```bash
   # Verify .env is in .gitignore
   git check-ignore .env
   ```

2. **Use strong encryption keys**
   ```bash
   # Generate a secure key
   openssl rand -base64 32
   ```

3. **Rotate API keys regularly**
   - Change keys every 90 days
   - Use different keys for dev/prod
   - Revoke unused keys immediately

### API Key Management

**DO:**
- ✅ Store keys in `.env` file (never committed)
- ✅ Use environment-specific keys
- ✅ Enable key rotation
- ✅ Monitor key usage

**DON'T:**
- ❌ Hardcode keys in source code
- ❌ Share keys in chat/email
- ❌ Use production keys in development
- ❌ Commit `.env` to version control

### System Permissions

1. **Review sudo operations**
   - Always read command before confirming
   - Understand what the command does
   - Enable `REQUIRE_CONFIRMATION=true`

2. **Limit file system access**
   - Use specific paths, not wildcards
   - Review file operations in logs
   - Backup important data regularly

3. **Network security**
   - Use HTTPS for all API calls
   - Verify SSL certificates
   - Monitor network activity

## 🚨 Reporting a Vulnerability

We take all security vulnerabilities seriously. If you discover a security issue, please follow these steps:

### Reporting Process

1. **DO NOT** open a public GitHub issue
2. Email security details to: **paulmmoore3416@gmail.com**
3. Include:
   - Description of the vulnerability
   - Steps to reproduce
   - Potential impact
   - Suggested fix (if any)

### What to Expect

- **Initial Response**: Within 48 hours
- **Status Update**: Within 7 days
- **Fix Timeline**: Depends on severity
  - Critical: 1-7 days
  - High: 7-30 days
  - Medium: 30-90 days
  - Low: 90+ days

### Disclosure Policy

- We follow responsible disclosure
- Security fixes released before public disclosure
- Credit given to reporters (if desired)
- CVE assigned for significant vulnerabilities

## 🔍 Security Audit Checklist

Before deploying OMNIX, verify:

- [ ] `.env` file is not in version control
- [ ] All API keys are valid and secure
- [ ] `ENCRYPTION_KEY` is changed from default
- [ ] `REQUIRE_CONFIRMATION=true` for sudo operations
- [ ] `LOG_ALL_COMMANDS=true` for audit trail
- [ ] Audit logs are monitored regularly
- [ ] System has latest security updates
- [ ] Firewall rules are configured
- [ ] Backup system is in place

## 🛠️ Security Configuration

### Recommended Settings

```env
# Security-focused configuration
ENABLE_SUDO=true
REQUIRE_CONFIRMATION=true
LOG_ALL_COMMANDS=true
ENCRYPTION_KEY=<generate-secure-key>
AUDIT_LOG_PATH=./logs/audit.log
DEBUG_MODE=false
ENABLE_TELEMETRY=false
```

### High-Security Mode

For maximum security, use these settings:

```env
# Maximum security configuration
ENABLE_SUDO=false              # Disable sudo entirely
REQUIRE_CONFIRMATION=true      # Always confirm
LOG_ALL_COMMANDS=true          # Log everything
ENABLE_AUTONOMOUS_MODE=false   # Disable autonomous actions
MAX_AUTONOMOUS_STEPS=0         # No autonomous steps
```

## 📋 Security Compliance

### Data Handling

- **Personal Data**: Stored locally, never transmitted
- **API Keys**: Encrypted at rest, never logged
- **Command History**: Stored locally, user-controlled
- **Telemetry**: Opt-in only, anonymized

### Third-Party Services

OMNIX may connect to:
- Local LLM servers (Ollama, LM Studio)
- Cloud AI APIs (OpenAI, Anthropic, etc.)
- GitHub API (optional)
- Google Drive API (optional)

**User Responsibility**: Review privacy policies of third-party services you enable.

## 🔄 Security Updates

### Staying Updated

```bash
# Check for updates
git fetch origin
git log HEAD..origin/main --oneline

# Update to latest version
git pull origin main
npm install
```

### Security Notifications

- Watch this repository for security advisories
- Enable GitHub security alerts
- Subscribe to release notifications

## 📞 Contact

- **Security Issues**: paulmmoore3416@gmail.com
- **General Support**: [GitHub Issues](https://github.com/Paulmmoore3416/omnix/issues)
- **Discussions**: [GitHub Discussions](https://github.com/Paulmmoore3416/omnix/discussions)

## 📜 License

This security policy is part of the OMNIX project, licensed under MIT License.

---

**Last Updated**: June 2026  
**Version**: 1.0.0