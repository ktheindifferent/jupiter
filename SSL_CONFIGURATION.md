# SSL/TLS Configuration Guide

## Overview

This application now enforces SSL certificate verification by default for all database connections, ensuring secure communication and protection against man-in-the-middle (MITM) attacks.

## Security Changes

### Previous Vulnerability
- SSL certificate verification was disabled using `SslVerifyMode::NONE`
- All database connections were vulnerable to MITM attacks
- Credentials and data could be intercepted

### Current Implementation
- SSL certificate verification is **enabled by default** using `SslVerifyMode::PEER`
- Minimum TLS version set to 1.2
- Support for custom CA certificates
- Centralized SSL configuration management

## Configuration Options

### Environment Variables

#### For Homebrew Provider:
- `HOMEBREW_SSL_VERIFY_PEER`: Enable/disable peer certificate verification (default: `true`)
- `HOMEBREW_CA_CERT_PATH`: Path to custom CA certificate file
- `HOMEBREW_DEV_MODE`: Enable development mode (default: `false`)

#### For Combo Provider:
- `COMBO_SSL_VERIFY_PEER`: Enable/disable peer certificate verification (default: `true`)
- `COMBO_CA_CERT_PATH`: Path to custom CA certificate file
- `COMBO_DEV_MODE`: Enable development mode (default: `false`)

## Usage Examples

### Production Configuration
```bash
# Default secure configuration (recommended)
export HOMEBREW_SSL_VERIFY_PEER=true
export COMBO_SSL_VERIFY_PEER=true

# With custom CA certificate
export HOMEBREW_CA_CERT_PATH=/path/to/ca-cert.pem
export COMBO_CA_CERT_PATH=/path/to/ca-cert.pem
```

### Development Configuration
```bash
# For local development with self-signed certificates
export HOMEBREW_DEV_MODE=true
export COMBO_DEV_MODE=true

# To still verify certificates in dev mode
export HOMEBREW_DEV_MODE=true
export HOMEBREW_SSL_VERIFY_PEER=true
```

## Certificate Management

### Using Custom CA Certificates

If your PostgreSQL server uses certificates signed by a private CA:

1. Obtain the CA certificate file (usually a `.pem` or `.crt` file)
2. Set the appropriate environment variable:
   ```bash
   export HOMEBREW_CA_CERT_PATH=/path/to/your/ca-certificate.pem
   ```
3. Ensure the certificate file is readable by the application

### Self-Signed Certificates (Development Only)

**⚠️ WARNING: Only use this in development environments!**

For local development with self-signed certificates:

1. Enable development mode:
   ```bash
   export HOMEBREW_DEV_MODE=true
   ```
2. The application will disable certificate verification by default in dev mode
3. You'll see warning logs indicating insecure mode is active

## PostgreSQL Connection Strings

The application uses the following connection string format:
```
postgresql://username:password@host/database?sslmode=prefer
```

The `sslmode` parameter is automatically set based on SSL verification settings:
- When verification is enabled: `sslmode=require`
- When verification is disabled (dev only): `sslmode=prefer`

## Security Best Practices

1. **Always enable SSL verification in production** - Never disable certificate verification in production environments
2. **Use proper certificates** - Ensure your PostgreSQL server has valid SSL certificates from a trusted CA
3. **Rotate certificates regularly** - Implement a certificate rotation schedule
4. **Monitor certificate expiration** - Set up alerts for certificate expiration
5. **Secure certificate storage** - Protect CA certificate files with appropriate file permissions
6. **Audit SSL configurations** - Regularly review SSL settings and logs

## Troubleshooting

### Common Issues

#### Certificate Verification Failure
```
Error: SSL certificate verify failed
```
**Solution**: Ensure the PostgreSQL server certificate is valid and signed by a trusted CA, or provide the CA certificate path.

#### Cannot Load CA Certificate
```
Failed to load CA certificate from /path/to/cert: No such file or directory
```
**Solution**: Verify the certificate file path exists and is readable.

#### Connection Refused with SSL
```
Connection error: SSL connection required
```
**Solution**: Ensure PostgreSQL server is configured to accept SSL connections.

### Debug Logging

Enable debug logging to troubleshoot SSL issues:
```bash
export RUST_LOG=debug
```

This will show detailed SSL handshake information and certificate verification steps.

## Testing

Run the SSL security tests:
```bash
# Unit tests
cargo test ssl_tests

# Integration tests (requires test PostgreSQL server)
cargo test -- --ignored
```

## Migration from Insecure Configuration

If you're upgrading from the insecure version:

1. **Test in development first** - Enable SSL verification in your development environment
2. **Obtain necessary certificates** - Get CA certificates if using private CAs
3. **Update environment variables** - Set the appropriate SSL configuration
4. **Deploy gradually** - Use canary deployments to verify SSL connections work
5. **Monitor logs** - Watch for SSL-related errors during rollout

## Compliance

This SSL implementation helps meet various security compliance requirements:
- PCI DSS: Requirement 4.1 (strong cryptography for data transmission)
- HIPAA: § 164.312(e)(1) (encryption of data in transit)
- SOC 2: CC6.1 (logical and physical access controls)
- GDPR: Article 32 (appropriate technical measures)

## Support

For SSL-related issues or questions:
1. Check the debug logs with `RUST_LOG=debug`
2. Verify certificate validity with `openssl s_client`
3. Review PostgreSQL SSL configuration
4. Contact your security team for certificate management