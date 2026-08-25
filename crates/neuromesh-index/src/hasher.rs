use std::io;
use std::path::Path;

pub struct ContentHasher;

impl ContentHasher {
    pub fn hash_bytes(data: &[u8]) -> String {
        blake3::hash(data).to_hex().to_string()
    }

    pub fn hash_str(text: &str) -> String {
        Self::hash_bytes(text.as_bytes())
    }

    pub fn hash_file(path: &Path) -> io::Result<String> {
        let mut hasher = blake3::Hasher::new();
        let mut file = std::fs::File::open(path)?;
        std::io::copy(&mut file, &mut hasher)?;
        Ok(hasher.finalize().to_hex().to_string())
    }
}
