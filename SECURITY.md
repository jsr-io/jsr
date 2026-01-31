# Security Policy

## Reporting a Vulnerability

The JSR team takes security vulnerabilities seriously. We appreciate your efforts to responsibly disclose your findings.

### Where to Report

**Please do not report security vulnerabilities through public GitHub issues.**

Instead, please email security concerns to: **security@deno.com**

### What to Include

When reporting a vulnerability, please include:

- A description of the vulnerability
- Steps to reproduce the issue
- Potential impact of the vulnerability
- Any suggested fixes (if available)

### Response Timeline

- We will acknowledge receipt of your report within 48 hours
- We will provide a more detailed response within 7 days
- We will work with you to understand and resolve the issue

### Disclosure Policy

- We will coordinate with you on disclosure timing
- We will credit reporters who follow responsible disclosure practices (unless they prefer to remain anonymous)

## Supported Versions

Security updates are applied to the latest version of JSR running in production at https://jsr.io.

## Security Best Practices for Package Authors

When publishing packages to JSR:

1. **Keep dependencies updated**: Regularly update your dependencies to get security fixes
2. **Review your code**: Look for common vulnerabilities before publishing
3. **Use the provenance feature**: Enable GitHub Actions publishing with provenance for supply chain security
4. **Document security considerations**: Include security notes in your README if applicable

## Additional Resources

- [JSR Usage Policy](https://jsr.io/docs/usage-policy)
- [JSR Trust Documentation](https://jsr.io/docs/trust)
- [JSR Provenance](https://jsr.io/docs/trust#provenance)
