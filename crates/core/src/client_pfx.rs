//! PKCS#12 (`.pfx` / `.p12`) client certificates, turned into PEM for rustls. The password is a
//! workspace secret in the keychain.

use std::path::Path;

use base64::Engine;
use p12_keystore::{KeyStore, Pkcs12ImportPolicy};

use crate::{Error, Result, secrets};

/// Keychain scope for workspace-wide secrets. Like every scope starting with `$`, no
/// environment can reach it (see [`secrets::internal`]).
const KEYCHAIN_SCOPE: &str = "$workspace";
const PASSWORD_NAME: &str = "clientPfxPassword";

/// Stores the PFX password in the keychain; an empty value removes it.
pub fn set_password(root: &Path, password: &str) -> Result<()> {
    if password.is_empty() {
        return secrets::internal::remove(root, KEYCHAIN_SCOPE, PASSWORD_NAME);
    }
    secrets::internal::set(root, KEYCHAIN_SCOPE, PASSWORD_NAME, password)
}

/// Whether a PFX password is stored for this workspace. An unreadable keychain counts as no.
pub fn has_password(root: &Path) -> bool {
    matches!(
        secrets::internal::get(root, KEYCHAIN_SCOPE, PASSWORD_NAME),
        Ok(Some(_))
    )
}

/// The stored password, or empty for PFX files without one.
pub(crate) fn password(root: &Path) -> Result<String> {
    Ok(secrets::internal::get(root, KEYCHAIN_SCOPE, PASSWORD_NAME)?.unwrap_or_default())
}

/// The private key and certificate chain of a PFX file as one PEM document, ready for
/// `reqwest::Identity::from_pem`.
pub fn to_pem(pfx: &[u8], password: &str) -> Result<Vec<u8>> {
    let store =
        KeyStore::from_pkcs12(pfx, password, Pkcs12ImportPolicy::Strict).map_err(|err| {
            Error::Invalid(format!(
                "failed to open the PFX client certificate (wrong password?): {err}"
            ))
        })?;
    let (_, chain) = store.private_key_chain().ok_or_else(|| {
        Error::Invalid("the PFX file has no private key with a certificate".into())
    })?;
    let mut pem = pem_block("PRIVATE KEY", chain.key().as_der());
    for cert in chain.certs() {
        pem.push_str(&pem_block("CERTIFICATE", cert.as_der()));
    }
    Ok(pem.into_bytes())
}

fn pem_block(label: &str, der: &[u8]) -> String {
    let encoded = base64::engine::general_purpose::STANDARD.encode(der);
    let lines: Vec<&str> = encoded
        .as_bytes()
        .chunks(64)
        .filter_map(|chunk| std::str::from_utf8(chunk).ok())
        .collect();
    format!(
        "-----BEGIN {label}-----\n{}\n-----END {label}-----\n",
        lines.join("\n")
    )
}

#[cfg(test)]
pub(crate) mod tests {
    use p12_keystore::{Certificate, KeyStoreEntry, PrivateKey, PrivateKeyChain};

    use super::*;

    /// A throwaway self-signed certificate packed as a PFX with `password`, generated per test
    /// run so no key material is checked in.
    pub fn test_pfx(password: &str) -> Vec<u8> {
        let generated = rcgen::generate_simple_self_signed(vec!["localhost".to_string()])
            .expect("generate test certificate");
        let key = PrivateKey::from_der(&generated.signing_key.serialize_der()).expect("key");
        let cert = Certificate::from_der(generated.cert.der()).expect("cert");
        let mut store = KeyStore::new();
        store.add_entry(
            "client",
            KeyStoreEntry::PrivateKeyChain(PrivateKeyChain::new(b"1".to_vec(), key, vec![cert])),
        );
        store.writer(password).write().expect("write pfx")
    }

    #[test]
    fn converts_pfx_to_a_pem_identity() {
        let pem = to_pem(&test_pfx("s3cret"), "s3cret").unwrap();
        let text = String::from_utf8(pem.clone()).unwrap();

        assert!(text.starts_with("-----BEGIN PRIVATE KEY-----\n"));
        assert!(text.contains("-----BEGIN CERTIFICATE-----\n"));
        assert!(reqwest::Identity::from_pem(&pem).is_ok());
    }

    #[test]
    fn wrong_password_says_so() {
        let err = to_pem(&test_pfx("right"), "wrong").unwrap_err();
        assert!(err.to_string().contains("wrong password"));
        assert!(to_pem(b"not a pfx", "").is_err());
    }
}
