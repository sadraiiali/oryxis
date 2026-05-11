//! End-to-end tests for the PPK parser against fixtures emitted by
//! the real `puttygen` binary. Sourced from philr/putty-key under MIT
//! (see `tests/fixtures/ppk/LICENSE-MIT`).
//!
//! Each `.ppk` has a sibling `.pem` derived from the same private key
//! material. We parse both and compare SHA-256 public-key
//! fingerprints; if the PPK parser hits the spec correctly the two
//! must agree.

use oryxis_vault::import_key;
use ssh_key::{HashAlg, PrivateKey};

const FIXTURES: &str = "tests/fixtures/ppk";
const PASSPHRASE: &str = "Test Passphrase";

fn pem_fingerprint(path: &str) -> String {
    let pem = std::fs::read_to_string(format!("{}/{}", FIXTURES, path))
        .unwrap_or_else(|e| panic!("read {}: {}", path, e));
    // PuTTY's exported PEM is OpenSSL PKCS#1 / SEC1; let `ssh-key` try
    // the OpenSSH path first, then fall back to the rsa / p256 crates.
    if let Ok(key) = PrivateKey::from_openssh(&pem) {
        return key.public_key().fingerprint(HashAlg::Sha256).to_string();
    }
    // PKCS#1 RSA
    if pem.contains("BEGIN RSA PRIVATE KEY") {
        use rsa::pkcs1::DecodeRsaPrivateKey;
        let rsa = rsa::RsaPrivateKey::from_pkcs1_pem(&pem).expect("PKCS#1 parse");
        let kp = ssh_key::private::RsaKeypair::try_from(rsa).expect("RSA convert");
        return PrivateKey::from(kp)
            .public_key()
            .fingerprint(HashAlg::Sha256)
            .to_string();
    }
    // SEC1 ECDSA
    if pem.contains("BEGIN EC PRIVATE KEY")
        && let Ok(sk) = p256::SecretKey::from_sec1_pem(&pem)
    {
        let public = sk.public_key().into();
        let private = ssh_key::private::EcdsaPrivateKey::<32>::from(sk);
        let kp = ssh_key::private::EcdsaKeypair::NistP256 { public, private };
        return PrivateKey::from(kp)
            .public_key()
            .fingerprint(HashAlg::Sha256)
            .to_string();
    }
    panic!("unrecognized PEM in {}", path);
}

fn ppk_fingerprint(path: &str, passphrase: Option<&str>) -> String {
    let pem = std::fs::read_to_string(format!("{}/{}", FIXTURES, path))
        .unwrap_or_else(|e| panic!("read {}: {}", path, e));
    let imported = import_key("fixture", &pem, passphrase)
        .unwrap_or_else(|e| panic!("import {}: {}", path, e));
    imported.key.fingerprint
}

#[test]
fn rsa_2048_format_2_unencrypted() {
    assert_eq!(
        ppk_fingerprint("rsa-2048-format-2.ppk", None),
        pem_fingerprint("rsa-2048.pem"),
    );
}

#[test]
fn rsa_2048_format_3_unencrypted() {
    assert_eq!(
        ppk_fingerprint("rsa-2048-format-3.ppk", None),
        pem_fingerprint("rsa-2048.pem"),
    );
}

#[test]
fn rsa_2048_format_2_encrypted() {
    assert_eq!(
        ppk_fingerprint("rsa-2048-encrypted-format-2.ppk", Some(PASSPHRASE)),
        pem_fingerprint("rsa-2048.pem"),
    );
}

#[test]
fn rsa_2048_format_3_encrypted() {
    assert_eq!(
        ppk_fingerprint("rsa-2048-encrypted-format-3.ppk", Some(PASSPHRASE)),
        pem_fingerprint("rsa-2048.pem"),
    );
}

#[test]
fn ecdsa_p256_format_2_unencrypted() {
    assert_eq!(
        ppk_fingerprint("ecdsa-sha2-nistp256-format-2.ppk", None),
        pem_fingerprint("ecdsa-sha2-nistp256.pem"),
    );
}

#[test]
fn ecdsa_p256_format_3_unencrypted() {
    assert_eq!(
        ppk_fingerprint("ecdsa-sha2-nistp256-format-3.ppk", None),
        pem_fingerprint("ecdsa-sha2-nistp256.pem"),
    );
}

#[test]
fn ecdsa_p256_format_2_encrypted() {
    assert_eq!(
        ppk_fingerprint("ecdsa-sha2-nistp256-encrypted-format-2.ppk", Some(PASSPHRASE)),
        pem_fingerprint("ecdsa-sha2-nistp256.pem"),
    );
}

#[test]
fn ecdsa_p256_format_3_encrypted() {
    assert_eq!(
        ppk_fingerprint("ecdsa-sha2-nistp256-encrypted-format-3.ppk", Some(PASSPHRASE)),
        pem_fingerprint("ecdsa-sha2-nistp256.pem"),
    );
}

#[test]
fn wrong_passphrase_format_2() {
    let pem = std::fs::read_to_string(format!(
        "{}/rsa-2048-encrypted-format-2.ppk",
        FIXTURES
    ))
    .unwrap();
    let err = import_key("fixture", &pem, Some("wrong")).unwrap_err();
    assert!(
        matches!(err, oryxis_vault::VaultError::WrongKeyPassphrase),
        "expected WrongKeyPassphrase, got: {:?}",
        err
    );
}

#[test]
fn wrong_passphrase_format_3() {
    let pem = std::fs::read_to_string(format!(
        "{}/rsa-2048-encrypted-format-3.ppk",
        FIXTURES
    ))
    .unwrap();
    let err = import_key("fixture", &pem, Some("wrong")).unwrap_err();
    assert!(
        matches!(err, oryxis_vault::VaultError::WrongKeyPassphrase),
        "expected WrongKeyPassphrase, got: {:?}",
        err
    );
}
