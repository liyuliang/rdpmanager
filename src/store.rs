use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};

/// One saved remote desktop connection.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Entry {
    pub id: String,
    pub name: String,
    pub host: String,
    pub username: String,
    #[serde(default)]
    pub password: String,
    /// File name of the .rdp file inside the app's data directory.
    pub file: String,
}

#[derive(Default, Serialize, Deserialize)]
struct Db {
    entries: Vec<Entry>,
}

pub struct Store {
    root: PathBuf,
    rdp_dir: PathBuf,
    pub entries: Vec<Entry>,
}

impl Store {
    pub fn load() -> Result<Self, String> {
        let root = dirs::data_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join("rdpmanager");
        let rdp_dir = root.join("rdp");
        fs::create_dir_all(&rdp_dir)
            .map_err(|e| format!("无法创建数据目录 {}: {e}", rdp_dir.display()))?;

        let db_path = root.join("entries.json");
        let entries = match fs::read_to_string(&db_path) {
            Ok(text) => serde_json::from_str::<Db>(&text)
                .map(|db| db.entries)
                .map_err(|e| format!("读取 {} 失败: {e}", db_path.display()))?,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Vec::new(),
            Err(e) => return Err(format!("读取 {} 失败: {e}", db_path.display())),
        };

        Ok(Self {
            root,
            rdp_dir,
            entries,
        })
    }

    /// A store backed by the default location but with no loaded data, used when
    /// loading fails so the UI can still start and show the error.
    pub fn empty() -> Self {
        let root = dirs::data_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join("rdpmanager");
        let rdp_dir = root.join("rdp");
        Self {
            root,
            rdp_dir,
            entries: Vec::new(),
        }
    }

    fn db_path(&self) -> PathBuf {
        self.root.join("entries.json")
    }

    pub fn save(&self) -> Result<(), String> {
        let text = serde_json::to_string_pretty(&Db {
            entries: self.entries.clone(),
        })
        .map_err(|e| format!("序列化失败: {e}"))?;
        fs::create_dir_all(&self.root).map_err(|e| format!("创建目录失败: {e}"))?;
        fs::write(self.db_path(), text).map_err(|e| format!("保存失败: {e}"))
    }

    pub fn rdp_path(&self, entry: &Entry) -> PathBuf {
        self.rdp_dir.join(&entry.file)
    }

    fn new_id(&self) -> String {
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos();
        format!("{nanos}-{}", self.entries.len())
    }

    fn unique_file(&self, stem: &str) -> String {
        let stem = sanitize(stem);
        let stem = if stem.is_empty() {
            "connection".to_string()
        } else {
            stem
        };
        let mut candidate = format!("{stem}.rdp");
        let mut n = 1;
        while self.rdp_dir.join(&candidate).exists()
            || self.entries.iter().any(|e| e.file == candidate)
        {
            candidate = format!("{stem}_{n}.rdp");
            n += 1;
        }
        candidate
    }

    /// Create a brand new connection by generating an .rdp file.
    pub fn create(
        &mut self,
        name: String,
        host: String,
        username: String,
        password: String,
    ) -> Result<Entry, String> {
        let name = name.trim().to_string();
        let host = host.trim().to_string();
        let username = username.trim().to_string();
        let password = password.trim().to_string();
        if host.is_empty() {
            return Err("地址不能为空".to_string());
        }

        let display_name = if name.is_empty() { host.clone() } else { name };
        let file = self.unique_file(&display_name);
        fs::create_dir_all(&self.rdp_dir).map_err(|e| format!("创建目录失败: {e}"))?;
        fs::write(self.rdp_dir.join(&file), build_rdp(&host, &username))
            .map_err(|e| format!("写入 .rdp 失败: {e}"))?;

        let entry = Entry {
            id: self.new_id(),
            name: display_name,
            host,
            username,
            password,
            file,
        };
        self.entries.push(entry.clone());
        self.save()?;
        Ok(entry)
    }

    /// Update an existing connection's fields and patch its .rdp file
    /// (preserving all original settings like gateway, proxy, etc.).
    pub fn update(
        &mut self,
        id: &str,
        name: String,
        host: String,
        username: String,
        password: String,
    ) -> Result<Entry, String> {
        let name = name.trim().to_string();
        let host = host.trim().to_string();
        let username = username.trim().to_string();
        let password = password.trim().to_string();
        if host.is_empty() {
            return Err("地址不能为空".to_string());
        }

        let Some(entry) = self.entries.iter_mut().find(|e| e.id == id) else {
            return Err("找不到该连接".to_string());
        };
        let display_name = if name.is_empty() { host.clone() } else { name };
        entry.name = display_name;
        entry.host = host.clone();
        entry.username = username.clone();
        entry.password = password;

        // Patch only the changed fields in the existing .rdp file so we
        // preserve gateway, proxy, authentication and other original settings.
        let rdp_path = self.rdp_dir.join(&entry.file);
        if rdp_path.exists() {
            let bytes = fs::read(&rdp_path)
                .map_err(|e| format!("读取 .rdp 失败: {e}"))?;
            let patched = patch_rdp(&bytes, &host, &username);
            fs::write(&rdp_path, patched)
                .map_err(|e| format!("写入 .rdp 失败: {e}"))?;
        } else {
            // File missing – fall back to generating a new one.
            fs::write(&rdp_path, build_rdp(&host, &username))
                .map_err(|e| format!("写入 .rdp 失败: {e}"))?;
        }

        let updated = entry.clone();
        self.save()?;
        Ok(updated)
    }

    /// Import an existing .rdp file from its raw bytes.
    pub fn import_bytes(&mut self, suggested_name: &str, bytes: &[u8]) -> Result<Entry, String> {
        if !suggested_name.to_ascii_lowercase().ends_with(".rdp") {
            return Err(format!("不是 .rdp 文件: {suggested_name}"));
        }

        let text = decode_rdp_text(bytes);
        let (host, username) = parse_rdp_meta(&text);
        let stem = Path::new(suggested_name)
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("imported");
        let file = self.unique_file(stem);

        fs::create_dir_all(&self.rdp_dir).map_err(|e| format!("创建目录失败: {e}"))?;
        fs::write(self.rdp_dir.join(&file), bytes).map_err(|e| format!("保存 .rdp 失败: {e}"))?;

        let entry = Entry {
            id: self.new_id(),
            name: stem.to_string(),
            host,
            username,
            password: String::new(),
            file,
        };
        self.entries.push(entry.clone());
        self.save()?;
        Ok(entry)
    }

    pub fn import_path(&mut self, src: &Path) -> Result<Entry, String> {
        let is_rdp = src
            .extension()
            .and_then(|s| s.to_str())
            .is_some_and(|s| s.eq_ignore_ascii_case("rdp"));
        if !is_rdp {
            return Err(format!("不是 .rdp 文件: {}", src.display()));
        }
        let bytes = fs::read(src).map_err(|e| format!("读取 {} 失败: {e}", src.display()))?;
        let name = src
            .file_name()
            .and_then(|s| s.to_str())
            .unwrap_or("imported.rdp")
            .to_string();
        self.import_bytes(&name, &bytes)
    }

    pub fn remove(&mut self, id: &str) -> Result<(), String> {
        let Some(pos) = self.entries.iter().position(|e| e.id == id) else {
            return Ok(());
        };
        let entry = self.entries.remove(pos);
        self.save()?;
        let path = self.rdp_dir.join(&entry.file);
        if path.exists() {
            let _ = fs::remove_file(path);
        }
        Ok(())
    }
}

fn sanitize(s: &str) -> String {
    const BAD: [char; 9] = ['<', '>', ':', '"', '/', '\\', '|', '?', '*'];
    let cleaned: String = s
        .chars()
        .filter(|c| !c.is_control() && !BAD.contains(c))
        .take(64)
        .collect();
    cleaned.trim().trim_end_matches('.').trim().to_string()
}

fn build_rdp(host: &str, username: &str) -> String {
    let mut s = String::new();
    s.push_str("screen mode id:i:2\n");
    s.push_str("desktopwidth:i:1280\n");
    s.push_str("desktopheight:i:720\n");
    s.push_str("session bpp:i:32\n");
    s.push_str("compression:i:1\n");
    s.push_str("keyboardhook:i:2\n");
    s.push_str("displayconnectionbar:i:1\n");
    s.push_str("networkautodetect:i:1\n");
    s.push_str("bandwidthautodetect:i:1\n");
    s.push_str("allow font smoothing:i:1\n");
    s.push_str("allow desktop composition:i:1\n");
    s.push_str("disable full window drag:i:1\n");
    s.push_str("disable menu anims:i:1\n");
    s.push_str("bitmapcachepersistenable:i:1\n");
    s.push_str(&format!("full address:s:{host}\n"));
    if !username.is_empty() {
        s.push_str(&format!("username:s:{username}\n"));
    }
    s.push_str("audiomode:i:0\n");
    s.push_str("redirectprinters:i:1\n");
    s.push_str("redirectsmartcards:i:1\n");
    s.push_str("redirectclipboard:i:1\n");
    s.push_str("autoreconnection enabled:i:1\n");
    s.push_str("authentication level:i:2\n");
    s.push_str("prompt for credentials:i:0\n");
    s.push_str("negotiate security layer:i:1\n");
    s.push_str("remoteapplicationmode:i:0\n");
    s.push_str("alternate shell:s:\n");
    s.push_str("shell working directory:s:\n");
    s.push_str("gatewayhostname:s:\n");
    s.push_str("gatewayusagemethod:i:4\n");
    s.push_str("gatewaycredentialssource:i:4\n");
    s.push_str("gatewayprofileusagemethod:i:0\n");
    s.push_str("promptcredentialonce:i:0\n");
    s
}

/// Patch only the `full address` and `username` fields in an existing .rdp file,
/// preserving all other settings (gateway, authentication, proxy, etc.).
fn patch_rdp(existing_bytes: &[u8], host: &str, username: &str) -> String {
    let text = decode_rdp_text(existing_bytes);
    let mut lines: Vec<String> = text.lines().map(|l| l.to_string()).collect();
    let mut found_host = false;
    let mut found_user = false;

    for line in &mut lines {
        let key = line
            .split(':')
            .next()
            .unwrap_or("")
            .trim()
            .to_ascii_lowercase();
        match key.as_str() {
            "full address" => {
                *line = format!("full address:s:{host}");
                found_host = true;
            }
            "username" => {
                if username.is_empty() {
                    *line = String::new(); // will be filtered out below
                } else {
                    *line = format!("username:s:{username}");
                }
                found_user = true;
            }
            _ => {}
        }
    }

    // Remove blanked-out lines.
    lines.retain(|l| !l.is_empty());

    if !found_host {
        lines.push(format!("full address:s:{host}"));
    }
    if !found_user && !username.is_empty() {
        lines.push(format!("username:s:{username}"));
    }

    let mut result = lines.join("\n");
    if !result.ends_with('\n') {
        result.push('\n');
    }
    result
}

/// .rdp files are usually plain UTF-8, but mstsc sometimes writes UTF-16 with a BOM.
fn decode_rdp_text(bytes: &[u8]) -> String {
    if bytes.starts_with(&[0xFF, 0xFE]) {
        let units: Vec<u16> = bytes[2..]
            .chunks_exact(2)
            .map(|c| u16::from_le_bytes([c[0], c[1]]))
            .collect();
        return String::from_utf16_lossy(&units);
    }
    if bytes.starts_with(&[0xFE, 0xFF]) {
        let units: Vec<u16> = bytes[2..]
            .chunks_exact(2)
            .map(|c| u16::from_be_bytes([c[0], c[1]]))
            .collect();
        return String::from_utf16_lossy(&units);
    }
    let bytes = bytes.strip_prefix(&[0xEF, 0xBB, 0xBF]).unwrap_or(bytes);
    String::from_utf8_lossy(bytes).into_owned()
}

/// Pull the couple of fields we show in the list out of an .rdp file.
fn parse_rdp_meta(text: &str) -> (String, String) {
    let mut host = String::new();
    let mut username = String::new();
    for line in text.lines() {
        let mut parts = line.trim().splitn(3, ':');
        let key = parts.next().unwrap_or("").trim().to_ascii_lowercase();
        let _type = parts.next();
        let value = parts.next().unwrap_or("").trim();
        match key.as_str() {
            "full address" => host = value.to_string(),
            "username" => username = value.to_string(),
            _ => {}
        }
    }
    (host, username)
}
