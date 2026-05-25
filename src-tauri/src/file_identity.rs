use std::{fs, path::Path};

use sha2::{Digest, Sha512};

#[derive(Clone, Debug)]
pub(crate) struct FileIdentity {
    pub sha512: String,
    pub curseforge_fingerprint: u32,
}

pub(crate) fn read_file_identity(path: &Path) -> Result<FileIdentity, String> {
    let bytes = fs::read(path).map_err(|error| error.to_string())?;
    Ok(FileIdentity {
        sha512: sha512_hex(&bytes),
        curseforge_fingerprint: curseforge_fingerprint(&bytes),
    })
}

fn sha512_hex(bytes: &[u8]) -> String {
    let digest = Sha512::digest(bytes);
    let mut result = String::with_capacity(digest.len() * 2);
    for byte in digest {
        result.push_str(&format!("{byte:02x}"));
    }
    result
}

fn curseforge_fingerprint(bytes: &[u8]) -> u32 {
    let normalized: Vec<u8> = bytes
        .iter()
        .copied()
        .filter(|byte| !matches!(byte, 9 | 10 | 13 | 32))
        .collect();
    murmur2(&normalized)
}

fn murmur2(bytes: &[u8]) -> u32 {
    const SEED: u32 = 1;
    const M: u32 = 0x5bd1e995;
    const R: u32 = 24;

    let len = bytes.len() as u32;
    let mut hash = SEED ^ len;
    let mut chunks = bytes.chunks_exact(4);

    for chunk in &mut chunks {
        let mut k = u32::from_le_bytes([chunk[0], chunk[1], chunk[2], chunk[3]]);
        k = k.wrapping_mul(M);
        k ^= k >> R;
        k = k.wrapping_mul(M);

        hash = hash.wrapping_mul(M);
        hash ^= k;
    }

    let tail = chunks.remainder();
    match tail.len() {
        3 => {
            hash ^= (tail[2] as u32) << 16;
            hash ^= (tail[1] as u32) << 8;
            hash ^= tail[0] as u32;
            hash = hash.wrapping_mul(M);
        }
        2 => {
            hash ^= (tail[1] as u32) << 8;
            hash ^= tail[0] as u32;
            hash = hash.wrapping_mul(M);
        }
        1 => {
            hash ^= tail[0] as u32;
            hash = hash.wrapping_mul(M);
        }
        _ => {}
    }

    hash ^= hash >> 13;
    hash = hash.wrapping_mul(M);
    hash ^= hash >> 15;
    hash
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sha512_hex_formats_digest() {
        assert_eq!(
            sha512_hex(b"abc"),
            "ddaf35a193617abacc417349ae20413112e6fa4e89a97ea20a9eeee64b55d39a\
             2192992a274fc1a836ba3c23a3feebbd454d4423643ce80e2a9ac94fa54ca49f"
                .replace(' ', "")
        );
    }

    #[test]
    fn curseforge_fingerprint_ignores_whitespace() {
        assert_eq!(
            curseforge_fingerprint(b"ab cd\n"),
            curseforge_fingerprint(b"abcd")
        );
    }
}
