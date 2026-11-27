use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;
use std::time::{Instant, SystemTime, UNIX_EPOCH};

const PBKDF2_ITERATIONS: u32 = 100_000;

#[derive(Clone, Debug, Serialize, Deserialize)]
struct MasterConfig {
    #[serde(default)]
    disabled: bool,
    #[serde(default)]
    salt_hex: String,
    #[serde(default)]
    hash_hex: String,
    #[serde(default = "default_iterations")]
    iterations: u32,
}

fn default_iterations() -> u32 {
    PBKDF2_ITERATIONS
}

/// Manages the optional startup master password stored in `master.json`.
pub struct MasterAuth {
    path: PathBuf,
    config: Option<MasterConfig>,
}

impl MasterAuth {
    pub fn load() -> Result<Self, String> {
        let path = dirs::data_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join("rdpmanager")
            .join("master.json");
        Self::load_from(path)
    }

    pub fn load_from(path: PathBuf) -> Result<Self, String> {
        let config = match fs::read_to_string(&path) {
            Ok(text) => {
                let cfg = serde_json::from_str::<MasterConfig>(&text)
                    .map_err(|e| format!("读取主密码配置失败: {e}"))?;
                Some(cfg)
            }
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => None,
            Err(e) => return Err(format!("读取主密码配置失败: {e}")),
        };
        Ok(Self { path, config })
    }

    pub fn empty() -> Self {
        let path = dirs::data_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join("rdpmanager")
            .join("master.json");
        Self { path, config: None }
    }

    /// True if the user has never configured or skipped the master password yet.
    pub fn needs_initial_setup(&self) -> bool {
        self.config.is_none()
    }

    /// True if a master password is currently active and must be verified on startup.
    pub fn is_enabled(&self) -> bool {
        matches!(
            &self.config,
            Some(c) if !c.disabled && !c.salt_hex.is_empty() && !c.hash_hex.is_empty()
        )
    }

    pub fn verify(&self, password: &str) -> bool {
        let Some(cfg) = &self.config else {
            return false;
        };
        if cfg.disabled {
            return false;
        }
        let Some(salt) = hex_decode(&cfg.salt_hex) else {
            return false;
        };
        let Some(expected) = hex_decode(&cfg.hash_hex) else {
            return false;
        };
        let iterations = cfg.iterations.max(1);
        let derived = pbkdf2_hmac_sha256(password.as_bytes(), &salt, iterations);
        constant_time_eq(&derived, &expected)
    }

    pub fn set_password(&mut self, password: &str) -> Result<(), String> {
        if password.is_empty() {
            return Err("主密码不能为空".to_string());
        }
        let salt = generate_salt();
        let hash = pbkdf2_hmac_sha256(password.as_bytes(), &salt, PBKDF2_ITERATIONS);
        let cfg = MasterConfig {
            disabled: false,
            salt_hex: hex_encode(&salt),
            hash_hex: hex_encode(&hash),
            iterations: PBKDF2_ITERATIONS,
        };
        self.save_config(cfg)
    }

    pub fn disable(&mut self) -> Result<(), String> {
        let cfg = MasterConfig {
            disabled: true,
            salt_hex: String::new(),
            hash_hex: String::new(),
            iterations: PBKDF2_ITERATIONS,
        };
        self.save_config(cfg)
    }

    fn save_config(&mut self, cfg: MasterConfig) -> Result<(), String> {
        if let Some(parent) = self.path.parent() {
            fs::create_dir_all(parent).map_err(|e| format!("创建目录失败: {e}"))?;
        }
        let text = serde_json::to_string_pretty(&cfg)
            .map_err(|e| format!("序列化主密码配置失败: {e}"))?;
        fs::write(&self.path, text).map_err(|e| format!("保存主密码配置失败: {e}"))?;
        self.config = Some(cfg);
        Ok(())
    }
}

fn generate_salt() -> [u8; 16] {
    let mut seed = Vec::with_capacity(64);
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    seed.extend_from_slice(&nanos.to_le_bytes());
    seed.extend_from_slice(&std::process::id().to_le_bytes());
    let inst = Instant::now();
    let ptr = &inst as *const Instant as usize;
    seed.extend_from_slice(&ptr.to_le_bytes());
    let stack_var = 0u64;
    let stack_ptr = &stack_var as *const u64 as usize;
    seed.extend_from_slice(&stack_ptr.to_le_bytes());
    let digest = sha256(&seed);
    let mut salt = [0u8; 16];
    salt.copy_from_slice(&digest[..16]);
    salt
}

fn constant_time_eq(a: &[u8], b: &[u8]) -> bool {
    if a.len() != b.len() {
        return false;
    }
    let mut diff = 0u8;
    for (x, y) in a.iter().zip(b.iter()) {
        diff |= x ^ y;
    }
    diff == 0
}

fn hex_encode(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut out = String::with_capacity(bytes.len() * 2);
    for &b in bytes {
        out.push(HEX[(b >> 4) as usize] as char);
        out.push(HEX[(b & 0x0f) as usize] as char);
    }
    out
}

fn hex_decode(s: &str) -> Option<Vec<u8>> {
    if !s.len().is_multiple_of(2) {
        return None;
    }
    let bytes = s.as_bytes();
    let mut out = Vec::with_capacity(bytes.len() / 2);
    for pair in bytes.chunks_exact(2) {
        let hi = (pair[0] as char).to_digit(16)? as u8;
        let lo = (pair[1] as char).to_digit(16)? as u8;
        out.push((hi << 4) | lo);
    }
    Some(out)
}

// ────────────────────────────── SHA-256 / PBKDF2 ──────────────────────────────

const K: [u32; 64] = [
    0x428a2f98, 0x71374491, 0xb5c0fbcf, 0xe9b5dba5, 0x3956c25b, 0x59f111f1, 0x923f82a4, 0xab1c5ed5,
    0xd807aa98, 0x12835b01, 0x243185be, 0x550c7dc3, 0x72be5d74, 0x80deb1fe, 0x9bdc06a7, 0xc19bf174,
    0xe49b69c1, 0xefbe4786, 0x0fc19dc6, 0x240ca1cc, 0x2de92c6f, 0x4a7484aa, 0x5cb0a9dc, 0x76f988da,
    0x983e5152, 0xa831c66d, 0xb00327c8, 0xbf597fc7, 0xc6e00bf3, 0xd5a79147, 0x06ca6351, 0x14292967,
    0x27b70a85, 0x2e1b2138, 0x4d2c6dfc, 0x53380d13, 0x650a7354, 0x766a0abb, 0x81c2c92e, 0x92722c85,
    0xa2bfe8a1, 0xa81a664b, 0xc24b8b70, 0xc76c51a3, 0xd192e819, 0xd6990624, 0xf40e3585, 0x106aa070,
    0x19a4c116, 0x1e376c08, 0x2748774c, 0x34b0bcb5, 0x391c0cb3, 0x4ed8aa4a, 0x5b9cca4f, 0x682e6ff3,
    0x748f82ee, 0x78a5636f, 0x84c87814, 0x8cc70208, 0x90befffa, 0xa4506ceb, 0xbef9a3f7, 0xc67178f2,
];

fn sha256(data: &[u8]) -> [u8; 32] {
    let mut state: [u32; 8] = [
        0x6a09e667, 0xbb67ae85, 0x3c6ef372, 0xa54ff53a,
        0x510e527f, 0x9b05688c, 0x1f83d9ab, 0x5be0cd19,
    ];

    let bit_len = (data.len() as u64) * 8;
    let mut padded = Vec::with_capacity(data.len() + 72);
    padded.extend_from_slice(data);
    padded.push(0x80);
    while (padded.len() % 64) != 56 {
        padded.push(0);
    }
    padded.extend_from_slice(&bit_len.to_be_bytes());

    for block in padded.chunks_exact(64) {
        let mut w = [0u32; 64];
        for i in 0..16 {
            let j = i * 4;
            w[i] = u32::from_be_bytes([block[j], block[j + 1], block[j + 2], block[j + 3]]);
        }
        for i in 16..64 {
            let s0 = w[i - 15].rotate_right(7) ^ w[i - 15].rotate_right(18) ^ (w[i - 15] >> 3);
            let s1 = w[i - 2].rotate_right(17) ^ w[i - 2].rotate_right(19) ^ (w[i - 2] >> 10);
            w[i] = w[i - 16]
                .wrapping_add(s0)
                .wrapping_add(w[i - 7])
                .wrapping_add(s1);
        }

        let [mut a, mut b, mut c, mut d, mut e, mut f, mut g, mut h] = state;
        for i in 0..64 {
            let s1 = e.rotate_right(6) ^ e.rotate_right(11) ^ e.rotate_right(25);
            let ch = (e & f) ^ ((!e) & g);
            let temp1 = h
                .wrapping_add(s1)
                .wrapping_add(ch)
                .wrapping_add(K[i])
                .wrapping_add(w[i]);
            let s0 = a.rotate_right(2) ^ a.rotate_right(13) ^ a.rotate_right(22);
            let maj = (a & b) ^ (a & c) ^ (b & c);
            let temp2 = s0.wrapping_add(maj);

            h = g;
            g = f;
            f = e;
            e = d.wrapping_add(temp1);
            d = c;
            c = b;
            b = a;
            a = temp1.wrapping_add(temp2);
        }

        let vals = [a, b, c, d, e, f, g, h];
        for i in 0..8 {
            state[i] = state[i].wrapping_add(vals[i]);
        }
    }

    let mut out = [0u8; 32];
    for (i, word) in state.iter().enumerate() {
        out[i * 4..i * 4 + 4].copy_from_slice(&word.to_be_bytes());
    }
    out
}

fn hmac_sha256(key: &[u8], data: &[u8]) -> [u8; 32] {
    let mut k_pad = [0u8; 64];
    if key.len() > 64 {
        let hashed = sha256(key);
        k_pad[..32].copy_from_slice(&hashed);
    } else {
        k_pad[..key.len()].copy_from_slice(key);
    }

    let mut inner = Vec::with_capacity(64 + data.len());
    for b in k_pad {
        inner.push(b ^ 0x36);
    }
    inner.extend_from_slice(data);
    let inner_hash = sha256(&inner);

    let mut outer = Vec::with_capacity(64 + 32);
    for b in k_pad {
        outer.push(b ^ 0x5c);
    }
    outer.extend_from_slice(&inner_hash);
    sha256(&outer)
}

fn pbkdf2_hmac_sha256(password: &[u8], salt: &[u8], iterations: u32) -> [u8; 32] {
    let mut msg = Vec::with_capacity(salt.len() + 4);
    msg.extend_from_slice(salt);
    msg.extend_from_slice(&1u32.to_be_bytes());

    let mut u = hmac_sha256(password, &msg);
    let mut out = u;
    for _ in 1..iterations {
        u = hmac_sha256(password, &u);
        for i in 0..32 {
            out[i] ^= u[i];
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sha256_known_vector() {
        let got = hex_encode(&sha256(b"abc"));
        assert_eq!(
            got,
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
    }

    #[test]
    fn pbkdf2_rfc6070_style_vector() {
        // PBKDF2-HMAC-SHA256("password", "salt", 1, 32)
        let got = hex_encode(&pbkdf2_hmac_sha256(b"password", b"salt", 1));
        assert_eq!(
            got,
            "120fb6cffcf8b32c43e7225256c4f837a86548c92ccc35480805987cb70be17b"
        );
    }

    #[test]
    fn master_auth_lifecycle() {
        let dir = std::env::temp_dir().join(format!(
            "rdpmanager_auth_test_{}_{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_nanos()
        ));
        let path = dir.join("master.json");
        let mut auth = MasterAuth::load_from(path.clone()).unwrap();
        assert!(auth.needs_initial_setup());
        assert!(!auth.is_enabled());

        auth.set_password("secret123").unwrap();
        assert!(!auth.needs_initial_setup());
        assert!(auth.is_enabled());
        assert!(auth.verify("secret123"));
        assert!(!auth.verify("wrong"));

        let reloaded = MasterAuth::load_from(path.clone()).unwrap();
        assert!(reloaded.is_enabled());
        assert!(reloaded.verify("secret123"));

        auth.disable().unwrap();
        assert!(!auth.needs_initial_setup());
        assert!(!auth.is_enabled());
        assert!(!auth.verify("secret123"));

        let _ = fs::remove_dir_all(dir);
    }
}
