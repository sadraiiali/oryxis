use ssh_key::PrivateKey;

use crate::store::VaultError;

/// Re-wrap the base64 body of a traditional PEM block to exactly 64 chars
/// per line. The pem-rfc7468 parser used by `rsa` / `pkcs8` is strict about
/// the 64-char convention (RFC 7468 §3) and rejects OpenSSL's legacy
/// 76-char wrapping with a misleading "invalid Base64 encoding" error.
/// Returns the input unchanged if no BEGIN/END envelope is found.
fn rewrap_pem_body(pem: &str) -> String {
    let begin = match pem.find("-----BEGIN ") {
        Some(i) => i,
        None => return pem.to_string(),
    };
    let begin_line_end = match pem[begin..].find('\n') {
        Some(off) => begin + off,
        None => return pem.to_string(),
    };
    let end_marker = match pem[begin_line_end..].find("-----END ") {
        Some(off) => begin_line_end + off,
        None => return pem.to_string(),
    };
    let body: String = pem[begin_line_end..end_marker]
        .chars()
        .filter(|c| !c.is_whitespace())
        .collect();
    let mut wrapped = String::with_capacity(body.len() + body.len() / 64 + 8);
    for chunk in body.as_bytes().chunks(64) {
        wrapped.push('\n');
        wrapped.push_str(std::str::from_utf8(chunk).unwrap_or(""));
    }
    wrapped.push('\n');
    let mut out = String::with_capacity(pem.len() + 16);
    out.push_str(&pem[..begin_line_end]);
    out.push_str(&wrapped);
    out.push_str(&pem[end_marker..]);
    out
}

/// Returns true if the PEM carries an OpenSSL-legacy DEK-Info header
/// (`Proc-Type: 4,ENCRYPTED`) inside a PKCS#1 or SEC1 envelope. We
/// surface this as a clear error rather than attempting to decrypt:
/// the format uses PBKDF1-MD5 + DES-EDE3-CBC which we don't carry,
/// and PuTTYgen (the most common source of such files) can export
/// PPK directly which we now support.
pub fn is_traditional_encrypted(pem: &str) -> bool {
    if pem.contains("BEGIN ENCRYPTED PRIVATE KEY") {
        return true;
    }
    pem.contains("ENCRYPTED")
        && (pem.contains("BEGIN RSA PRIVATE KEY") || pem.contains("BEGIN EC PRIVATE KEY"))
        && pem.contains("DEK-Info:")
}

/// Decrypt a PKCS#8 `ENCRYPTED PRIVATE KEY` PEM with `passphrase` and
/// hand the inner plaintext PKCS#8 PEM back to the regular dispatcher,
/// so RSA, ECDSA, and Ed25519 all benefit from the same probe order.
fn decrypt_pkcs8(pem: &str, passphrase: &[u8]) -> Result<PrivateKey, VaultError> {
    use base64::Engine;

    let begin_tag = "-----BEGIN ENCRYPTED PRIVATE KEY-----";
    let end_tag = "-----END ENCRYPTED PRIVATE KEY-----";
    let begin = pem
        .find(begin_tag)
        .ok_or_else(|| VaultError::Crypto("missing PEM begin".into()))?
        + begin_tag.len();
    let end = pem[begin..]
        .find(end_tag)
        .ok_or_else(|| VaultError::Crypto("missing PEM end".into()))?
        + begin;
    let b64: String = pem[begin..end]
        .chars()
        .filter(|c| !c.is_whitespace())
        .collect();
    let der = base64::engine::general_purpose::STANDARD
        .decode(b64)
        .map_err(|e| VaultError::Crypto(format!("PEM base64: {}", e)))?;

    let info = pkcs8::EncryptedPrivateKeyInfo::try_from(der.as_slice())
        .map_err(|e| VaultError::Crypto(format!("encrypted PKCS#8 parse: {}", e)))?;
    let plaintext = info
        .decrypt(passphrase)
        .map_err(|_| VaultError::WrongKeyPassphrase)?;

    // Re-wrap the decrypted DER as a plain "BEGIN PRIVATE KEY" PEM and
    // dispatch through the plaintext path. This keeps all algorithm
    // probes (RSA, ECDSA, Ed25519) in one place.
    let plain_pem = der_to_pkcs8_pem(plaintext.as_bytes());
    parse(&plain_pem, None)
}

fn der_to_pkcs8_pem(der: &[u8]) -> String {
    use base64::Engine;
    let body = base64::engine::general_purpose::STANDARD.encode(der);
    let mut out = String::with_capacity(body.len() + 64);
    out.push_str("-----BEGIN PRIVATE KEY-----\n");
    for chunk in body.as_bytes().chunks(64) {
        out.push_str(std::str::from_utf8(chunk).unwrap_or(""));
        out.push('\n');
    }
    out.push_str("-----END PRIVATE KEY-----\n");
    out
}

fn parse_ec_p256(sk: p256::SecretKey) -> PrivateKey {
    let public = sk.public_key().into();
    let private = ssh_key::private::EcdsaPrivateKey::<32>::from(sk);
    PrivateKey::from(ssh_key::private::EcdsaKeypair::NistP256 { public, private })
}

fn parse_ec_p384(sk: p384::SecretKey) -> PrivateKey {
    let public = sk.public_key().into();
    let private = ssh_key::private::EcdsaPrivateKey::<48>::from(sk);
    PrivateKey::from(ssh_key::private::EcdsaKeypair::NistP384 { public, private })
}

/// Parse a PKCS#8 OneAsymmetricKey (RFC 5958) DER body and return the
/// inner Ed25519 32-byte seed, if the algorithm OID matches
/// `1.3.101.112` (RFC 8410). The structure we expect is fixed:
///
/// ```text
/// SEQUENCE {
///   INTEGER 0,                       -- version
///   SEQUENCE { OID 1.3.101.112 },    -- algorithm
///   OCTET STRING { OCTET STRING { 32 bytes seed } },
///   [0] OPTIONAL public key          -- ignored
/// }
/// ```
fn try_extract_ed25519_seed(pem: &str) -> Option<[u8; 32]> {
    use base64::Engine;
    let begin_tag = "-----BEGIN PRIVATE KEY-----";
    let end_tag = "-----END PRIVATE KEY-----";
    let begin = pem.find(begin_tag)? + begin_tag.len();
    let end = pem[begin..].find(end_tag)? + begin;
    let b64: String = pem[begin..end].chars().filter(|c| !c.is_whitespace()).collect();
    let der = base64::engine::general_purpose::STANDARD.decode(b64).ok()?;

    // Outer SEQUENCE.
    let body = read_tlv(&der, 0x30)?;
    let mut cur = body;

    // version INTEGER (0)
    let (_, rest) = take_tlv(cur, 0x02)?;
    cur = rest;

    // algorithm SEQUENCE { OID }
    let (algo_body, rest) = take_tlv(cur, 0x30)?;
    cur = rest;
    let (oid_bytes, _) = take_tlv(algo_body, 0x06)?;
    // OID 1.3.101.112 encodes to bytes: 2b 65 70
    if oid_bytes != [0x2b, 0x65, 0x70] {
        return None;
    }

    // privateKey OCTET STRING { OCTET STRING { 32 bytes } }
    let (outer_octets, _) = take_tlv(cur, 0x04)?;
    let (inner_octets, _) = take_tlv(outer_octets, 0x04)?;
    if inner_octets.len() != 32 {
        return None;
    }
    let mut seed = [0u8; 32];
    seed.copy_from_slice(inner_octets);
    Some(seed)
}

/// Minimal DER reader: read a single TLV at the start of `buf` whose tag
/// matches `expected_tag`, returning the value bytes. Lengths follow
/// the short/long form rules from X.690. Returns None on any mismatch.
fn read_tlv(buf: &[u8], expected_tag: u8) -> Option<&[u8]> {
    take_tlv(buf, expected_tag).map(|(v, _)| v)
}

fn take_tlv(buf: &[u8], expected_tag: u8) -> Option<(&[u8], &[u8])> {
    if buf.first()? != &expected_tag {
        return None;
    }
    let len_byte = *buf.get(1)?;
    let (len, header_len) = if len_byte & 0x80 == 0 {
        (len_byte as usize, 2)
    } else {
        let n = (len_byte & 0x7f) as usize;
        if n == 0 || n > 4 {
            return None;
        }
        let mut len: usize = 0;
        for i in 0..n {
            len = (len << 8) | (*buf.get(2 + i)? as usize);
        }
        (len, 2 + n)
    };
    let end = header_len.checked_add(len)?;
    if end > buf.len() {
        return None;
    }
    Some((&buf[header_len..end], &buf[end..]))
}

/// Parse a traditional PEM key (PKCS#1, PKCS#8, SEC1) and convert to
/// `ssh_key::PrivateKey`. Encrypted PKCS#8 is decrypted using
/// `passphrase`. Encrypted PKCS#1 (OpenSSL DEK-Info) is rejected with
/// an actionable error. The caller in `mod.rs` checks
/// `is_traditional_encrypted` first to short-circuit before reaching us.
pub fn parse(pem: &str, passphrase: Option<&str>) -> Result<PrivateKey, VaultError> {
    use rsa::pkcs1::DecodeRsaPrivateKey;
    use rsa::pkcs8::DecodePrivateKey;

    // Reject the legacy OpenSSL DEK-Info path explicitly. The UI maps
    // EncryptedLegacyPem to an i18n'd message that hints at PPK as the
    // user-friendlier alternative.
    if pem.contains("DEK-Info:")
        && (pem.contains("BEGIN RSA PRIVATE KEY") || pem.contains("BEGIN EC PRIVATE KEY"))
    {
        return Err(VaultError::EncryptedLegacyPem);
    }

    let normalized = rewrap_pem_body(pem);
    let pem = normalized.as_str();

    // Encrypted PKCS#8: "BEGIN ENCRYPTED PRIVATE KEY". Decrypt once
    // here, then re-dispatch to the plain PKCS#8 algorithm probes below.
    if pem.contains("BEGIN ENCRYPTED PRIVATE KEY") {
        let pass = passphrase.unwrap_or("");
        if pass.is_empty() {
            return Err(VaultError::KeyNeedsPassphrase);
        }
        return decrypt_pkcs8(pem, pass.as_bytes());
    }

    // PKCS#1 RSA: "BEGIN RSA PRIVATE KEY"
    if pem.contains("BEGIN RSA PRIVATE KEY") {
        let rsa_key = rsa::RsaPrivateKey::from_pkcs1_pem(pem)
            .map_err(|e| VaultError::Crypto(format!("PKCS#1 parse error: {}", e)))?;
        let keypair = ssh_key::private::RsaKeypair::try_from(rsa_key)
            .map_err(|e| VaultError::Crypto(format!("RSA key conversion error: {}", e)))?;
        return Ok(PrivateKey::from(keypair));
    }

    // SEC1 EC: "BEGIN EC PRIVATE KEY"
    if pem.contains("BEGIN EC PRIVATE KEY") {
        if let Ok(sk) = p256::SecretKey::from_sec1_pem(pem) {
            return Ok(parse_ec_p256(sk));
        }
        if let Ok(sk) = p384::SecretKey::from_sec1_pem(pem) {
            return Ok(parse_ec_p384(sk));
        }
        return Err(VaultError::Crypto(
            "Unsupported EC curve (only P-256 and P-384 are supported)".into(),
        ));
    }

    // PKCS#8: "BEGIN PRIVATE KEY" (RSA, EC, or Ed25519).
    if pem.contains("BEGIN PRIVATE KEY") {
        if let Ok(rsa_key) = rsa::RsaPrivateKey::from_pkcs8_pem(pem) {
            let keypair = ssh_key::private::RsaKeypair::try_from(rsa_key)
                .map_err(|e| VaultError::Crypto(format!("RSA key conversion error: {}", e)))?;
            return Ok(PrivateKey::from(keypair));
        }
        if let Ok(sk) = p256::SecretKey::from_pkcs8_pem(pem) {
            return Ok(parse_ec_p256(sk));
        }
        if let Ok(sk) = p384::SecretKey::from_pkcs8_pem(pem) {
            return Ok(parse_ec_p384(sk));
        }
        if let Some(seed) = try_extract_ed25519_seed(pem) {
            let keypair = ssh_key::private::Ed25519Keypair::from_seed(&seed);
            return Ok(PrivateKey::from(keypair));
        }
        return Err(VaultError::Crypto(
            "Unsupported PKCS#8 key type (supported: RSA, ECDSA P-256/P-384, Ed25519)".into(),
        ));
    }

    Err(VaultError::Crypto("Unrecognized PEM format".into()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn der_tlv_short_form() {
        // SEQUENCE len=3 { 02 01 00 }, an INTEGER(0).
        let buf = [0x30, 0x03, 0x02, 0x01, 0x00];
        let body = read_tlv(&buf, 0x30).unwrap();
        assert_eq!(body, &[0x02, 0x01, 0x00]);
        let (inner, rest) = take_tlv(body, 0x02).unwrap();
        assert_eq!(inner, &[0x00]);
        assert!(rest.is_empty());
    }

    #[test]
    fn der_tlv_long_form() {
        // OCTET STRING len=130 (0x82), long-form length encoding.
        let mut buf = vec![0x04, 0x81, 0x82];
        buf.extend(std::iter::repeat_n(0xAA, 130));
        let body = read_tlv(&buf, 0x04).unwrap();
        assert_eq!(body.len(), 130);
    }

    #[test]
    fn extract_ed25519_from_pkcs8() {
        // Minimal Ed25519 PKCS#8 with a known seed (all zeros for simplicity).
        let seed = [0u8; 32];
        let der: Vec<u8> = vec![
            0x30, 0x2e, // SEQUENCE len 46
            0x02, 0x01, 0x00, // INTEGER 0
            0x30, 0x05, // SEQUENCE len 5
            0x06, 0x03, 0x2b, 0x65, 0x70, // OID 1.3.101.112
            0x04, 0x22, // OCTET STRING len 34
            0x04, 0x20, // inner OCTET STRING len 32
        ];
        let mut full = der;
        full.extend_from_slice(&seed);
        use base64::Engine;
        let b64 = base64::engine::general_purpose::STANDARD.encode(&full);
        let pem = format!(
            "-----BEGIN PRIVATE KEY-----\n{}\n-----END PRIVATE KEY-----\n",
            b64
        );
        let got = try_extract_ed25519_seed(&pem).unwrap();
        assert_eq!(got, seed);
    }
}
