//! Putting a file under a passphrase.
//!
//! One job, one place: an export that carries passwords. Everything else this
//! program keeps is either readable on purpose (the site files, which hold no
//! secret) or in the system's own store (the passwords themselves).
//!
//! The construction is the ordinary one, and deliberately so. A passphrase is
//! stretched with PBKDF2-HMAC-SHA256 over a random salt, and the result is the
//! key for ChaCha20-Poly1305 over a random nonce. Nothing here is invented:
//! home-made cryptography is the one kind that looks fine until somebody
//! competent reads it.
//!
//! The file says what it is in a header that is not encrypted — the format, the
//! round count, the salt and the nonce. That is how it has to be: a reader
//! needs them before it can decrypt anything, and none of them is a secret.
//! What the header also does is let a file made today still open in five years
//! when the round count has been raised.

use ring::aead::{self, Aad, LessSafeKey, Nonce, UnboundKey};
use ring::pbkdf2;
use ring::rand::{SecureRandom, SystemRandom};

use crate::error::{Error, Result};

/// What the file calls itself, so a wrong file is refused with a sentence
/// rather than a decryption failure.
const MAGIC: &[u8; 16] = b"AMBERBEAM-SEAL\x00\x01";

/// How hard the passphrase is stretched.
///
/// 600,000 is what OWASP asks for PBKDF2-HMAC-SHA256, and it costs a fraction
/// of a second once, when a file is written or opened. The number is written
/// into the file, so raising it later does not orphan the files made today.
const ROUNDS: u32 = 600_000;

const SALT_LEN: usize = 16;
const NONCE_LEN: usize = 12;
const KEY_LEN: usize = 32;

/// Whether these bytes are a sealed file.
///
/// Asked before a passphrase is, so somebody opening a plain export is not
/// prompted for one that does not exist.
pub fn is_sealed(bytes: &[u8]) -> bool {
    bytes.len() > MAGIC.len() && &bytes[..MAGIC.len()] == MAGIC
}

/// Wraps `plain` under `passphrase`.
pub fn seal(plain: &[u8], passphrase: &str) -> Result<Vec<u8>> {
    if passphrase.is_empty() {
        // Not a technical limit. A file that claims to be protected by nothing
        // is worse than one that is plainly not protected at all.
        return Err(Error::other("a passphrase is needed"));
    }

    let random = SystemRandom::new();
    let mut salt = [0_u8; SALT_LEN];
    let mut nonce = [0_u8; NONCE_LEN];
    random
        .fill(&mut salt)
        .map_err(|_| Error::other("no randomness"))?;
    random
        .fill(&mut nonce)
        .map_err(|_| Error::other("no randomness"))?;

    let key = stretch(passphrase, &salt, ROUNDS)?;
    let mut body = plain.to_vec();
    key.seal_in_place_append_tag(
        Nonce::assume_unique_for_key(nonce),
        // The header is authenticated as well, so nobody can lower the round
        // count of a file somebody else made and have it still open.
        Aad::from(header(&salt, &nonce, ROUNDS)),
        &mut body,
    )
    .map_err(|_| Error::other("the file could not be sealed"))?;

    let mut out = header(&salt, &nonce, ROUNDS);
    out.extend_from_slice(&body);
    Ok(out)
}

/// Opens a file made by [`seal`].
pub fn open(sealed: &[u8], passphrase: &str) -> Result<Vec<u8>> {
    let head = MAGIC.len() + 4 + SALT_LEN + NONCE_LEN;
    if sealed.len() < head + aead::CHACHA20_POLY1305.tag_len() {
        return Err(Error::other("this is not an AmberBeam export"));
    }
    if &sealed[..MAGIC.len()] != MAGIC {
        return Err(Error::other("this is not an AmberBeam export"));
    }

    let mut at = MAGIC.len();
    let rounds = u32::from_be_bytes(sealed[at..at + 4].try_into().unwrap_or_default());
    at += 4;
    let salt: [u8; SALT_LEN] = sealed[at..at + SALT_LEN].try_into().unwrap_or_default();
    at += SALT_LEN;
    let nonce: [u8; NONCE_LEN] = sealed[at..at + NONCE_LEN].try_into().unwrap_or_default();
    at += NONCE_LEN;

    // A file claiming an absurd round count would otherwise hang the program
    // for minutes before failing.
    if rounds == 0 || rounds > 10_000_000 {
        return Err(Error::other("this export was made by something else"));
    }

    let key = stretch(passphrase, &salt, rounds)?;
    let mut body = sealed[at..].to_vec();
    let plain = key
        .open_in_place(
            Nonce::assume_unique_for_key(nonce),
            Aad::from(header(&salt, &nonce, rounds)),
            &mut body,
        )
        // The one error that matters, and it means exactly one thing that the
        // person can act on.
        .map_err(|_| Error::other("wrong passphrase, or the file was altered"))?;
    Ok(plain.to_vec())
}

fn header(salt: &[u8; SALT_LEN], nonce: &[u8; NONCE_LEN], rounds: u32) -> Vec<u8> {
    let mut head = Vec::with_capacity(MAGIC.len() + 4 + SALT_LEN + NONCE_LEN);
    head.extend_from_slice(MAGIC);
    head.extend_from_slice(&rounds.to_be_bytes());
    head.extend_from_slice(salt);
    head.extend_from_slice(nonce);
    head
}

fn stretch(passphrase: &str, salt: &[u8], rounds: u32) -> Result<LessSafeKey> {
    let rounds = std::num::NonZeroU32::new(rounds).ok_or_else(|| Error::other("no rounds"))?;
    let mut key = [0_u8; KEY_LEN];
    pbkdf2::derive(
        pbkdf2::PBKDF2_HMAC_SHA256,
        rounds,
        salt,
        passphrase.as_bytes(),
        &mut key,
    );
    let unbound = UnboundKey::new(&aead::CHACHA20_POLY1305, &key)
        .map_err(|_| Error::other("the key could not be made"))?;
    Ok(LessSafeKey::new(unbound))
}

#[cfg(test)]
mod tests {
    use super::*;

    const PLAIN: &[u8] = b"{\"sites\":[{\"name\":\"Webserver\",\"password\":\"tannenbaum\"}]}";

    #[test]
    fn what_goes_in_comes_out() {
        let sealed = seal(PLAIN, "ein langes Kennwort").unwrap();
        assert_eq!(open(&sealed, "ein langes Kennwort").unwrap(), PLAIN);
    }

    #[test]
    fn the_passwords_are_not_in_the_file() {
        // The whole reason the export is sealed at all.
        let sealed = seal(PLAIN, "kennwort").unwrap();
        assert!(!sealed.windows(10).any(|window| window == b"tannenbaum"));
        assert!(!sealed.windows(9).any(|window| window == b"Webserver"));
    }

    #[test]
    fn the_wrong_passphrase_says_so_and_gives_nothing() {
        let sealed = seal(PLAIN, "richtig").unwrap();
        assert!(open(&sealed, "falsch").is_err());
        assert!(open(&sealed, "").is_err());
    }

    #[test]
    fn a_changed_file_will_not_open() {
        // Not merely "the bytes differ": a file somebody edited must fail, not
        // decrypt into something almost right.
        let mut sealed = seal(PLAIN, "kennwort").unwrap();
        let last = sealed.len() - 1;
        sealed[last] ^= 0x01;
        assert!(open(&sealed, "kennwort").is_err());
    }

    #[test]
    fn the_header_is_protected_too() {
        // Lowering the round count of somebody else's file must not make it
        // easier to attack — the header is signed along with the body.
        let mut sealed = seal(PLAIN, "kennwort").unwrap();
        sealed[MAGIC.len()..MAGIC.len() + 4].copy_from_slice(&1_u32.to_be_bytes());
        assert!(open(&sealed, "kennwort").is_err());
    }

    #[test]
    fn something_that_is_not_an_export_is_refused_by_name() {
        assert!(open(b"", "kennwort").is_err());
        assert!(open(
            b"just some text that is long enough to get past the length check",
            "k"
        )
        .is_err());
    }

    #[test]
    fn two_files_of_the_same_thing_look_nothing_alike() {
        // A fresh salt and nonce each time, so two exports of one list do not
        // reveal that they are the same list.
        let one = seal(PLAIN, "kennwort").unwrap();
        let two = seal(PLAIN, "kennwort").unwrap();
        assert_ne!(one, two);
        assert_eq!(
            open(&one, "kennwort").unwrap(),
            open(&two, "kennwort").unwrap()
        );
    }

    #[test]
    fn a_file_without_a_passphrase_is_not_offered() {
        assert!(seal(PLAIN, "").is_err());
    }
}
