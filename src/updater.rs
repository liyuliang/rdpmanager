use serde::Deserialize;
use std::fs::{self, File};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::mpsc::Sender;
use std::time::{Duration, Instant};

pub const REPO_OWNER: &str = "liyuliang";
pub const REPO_NAME: &str = "rdpmanager";

#[derive(Clone, Debug)]
pub struct ReleaseInfo {
    pub current_version: String,
    pub latest_version: String,
    pub release_name: String,
    pub release_notes: String,
    pub html_url: String,
    pub asset_name: String,
    pub asset_url: String,
    pub asset_size: u64,
    pub has_update: bool,
    pub is_dev: bool,
    pub can_self_update: bool,
}

#[derive(Debug)]
pub enum UpdateEvent {
    CheckDone {
        manual: bool,
        result: Result<ReleaseInfo, String>,
    },
    Progress {
        done: u64,
        total: u64,
    },
    ApplyDone,
    ApplyError(String),
}

#[derive(Deserialize)]
struct GhRelease {
    tag_name: String,
    #[serde(default)]
    name: String,
    #[serde(default)]
    body: String,
    #[serde(default)]
    html_url: String,
    #[serde(default)]
    assets: Vec<GhAsset>,
}

#[derive(Deserialize)]
struct GhAsset {
    name: String,
    #[serde(default)]
    size: u64,
    browser_download_url: String,
}

pub fn current_version() -> &'static str {
    option_env!("RDPMANAGER_VERSION").unwrap_or(concat!("v", env!("CARGO_PKG_VERSION")))
}

pub fn is_dev_version(v: &str) -> bool {
    let s = v.trim().to_ascii_lowercase();
    s.is_empty() || s == "dev" || s.starts_with("build-")
}

fn parse_semver(raw: &str) -> Option<(u64, u64, u64, String)> {
    let mut s = raw.trim();
    s = s.strip_prefix('v').or_else(|| s.strip_prefix('V')).unwrap_or(s);
    if let Some((before_plus, _)) = s.split_once('+') {
        s = before_plus;
    }
    let (core, pre) = match s.split_once('-') {
        Some((c, p)) => (c, p.to_string()),
        None => (s, String::new()),
    };
    let mut parts = core.split('.');
    let major: u64 = parts.next()?.parse().ok()?;
    let minor: u64 = parts.next()?.parse().ok()?;
    let patch: u64 = match parts.next() {
        Some(p) => p.parse().ok()?,
        None => 0,
    };
    if parts.next().is_some() {
        return None;
    }
    Some((major, minor, patch, pre))
}

pub fn compare_semver(a: &str, b: &str) -> std::cmp::Ordering {
    match (parse_semver(a), parse_semver(b)) {
        (Some((amaj, amin, apat, apre)), Some((bmaj, bmin, bpat, bpre))) => {
            (amaj, amin, apat)
                .cmp(&(bmaj, bmin, bpat))
                .then_with(|| match (apre.is_empty(), bpre.is_empty()) {
                    (true, false) => std::cmp::Ordering::Greater,
                    (false, true) => std::cmp::Ordering::Less,
                    _ => apre.cmp(&bpre),
                })
        }
        _ => {
            let na = a.trim().trim_start_matches(['v', 'V']).to_ascii_lowercase();
            let nb = b.trim().trim_start_matches(['v', 'V']).to_ascii_lowercase();
            na.cmp(&nb)
        }
    }
}

pub fn is_newer(current: &str, latest: &str) -> bool {
    if is_dev_version(current) || latest.trim().is_empty() {
        return false;
    }
    compare_semver(current, latest) == std::cmp::Ordering::Less
}

pub fn expected_asset_name() -> &'static str {
    if cfg!(target_os = "windows") && cfg!(target_arch = "x86_64") {
        "rdpmanager-windows-amd64.exe"
    } else if cfg!(target_os = "macos") && cfg!(target_arch = "aarch64") {
        "rdpmanager-macos-arm64.tar.gz"
    } else if cfg!(target_os = "macos") && cfg!(target_arch = "x86_64") {
        "rdpmanager-macos-amd64.tar.gz"
    } else if cfg!(target_os = "linux") && cfg!(target_arch = "x86_64") {
        "rdpmanager-linux-amd64.tar.gz"
    } else {
        ""
    }
}

pub fn cleanup_old_binary() {
    if let Ok(exe) = std::env::current_exe() {
        let old = old_backup_path(&exe);
        let _ = fs::remove_file(old);
    }
}

fn old_backup_path(exe: &Path) -> PathBuf {
    let mut os = exe.as_os_str().to_os_string();
    os.push(".old");
    PathBuf::from(os)
}

pub fn check_latest() -> Result<ReleaseInfo, String> {
    let cur = current_version().to_string();
    let is_dev = is_dev_version(&cur);
    let default_url = format!("https://github.com/{REPO_OWNER}/{REPO_NAME}/releases");
    let api_url = format!("https://api.github.com/repos/{REPO_OWNER}/{REPO_NAME}/releases/latest");

    let agent = ureq::AgentBuilder::new()
        .timeout(Duration::from_secs(15))
        .build();

    let resp = agent
        .get(&api_url)
        .set("Accept", "application/vnd.github+json")
        .set("User-Agent", &format!("rdpmanager/{cur}"))
        .call();

    let resp = match resp {
        Ok(r) => r,
        Err(ureq::Error::Status(404, _)) => {
            return Ok(ReleaseInfo {
                current_version: cur,
                latest_version: String::new(),
                release_name: String::new(),
                release_notes: String::new(),
                html_url: default_url,
                asset_name: String::new(),
                asset_url: String::new(),
                asset_size: 0,
                has_update: false,
                is_dev,
                can_self_update: false,
            });
        }
        Err(ureq::Error::Status(code, r)) => {
            let body = r.into_string().unwrap_or_default();
            return Err(format!("GitHub API 返回 HTTP {code}: {}", body.trim()));
        }
        Err(e) => return Err(format!("请求 GitHub Release 失败: {e}")),
    };

    let rel: GhRelease = resp
        .into_json()
        .map_err(|e| format!("解析 GitHub Release 响应失败: {e}"))?;

    let has_update = is_newer(&cur, &rel.tag_name);
    let want = expected_asset_name();
    let mut asset_name = String::new();
    let mut asset_url = String::new();
    let mut asset_size = 0;
    let mut can_self_update = false;

    for a in rel.assets {
        if !want.is_empty() && a.name == want {
            asset_name = a.name;
            asset_url = a.browser_download_url;
            asset_size = a.size;
            can_self_update = true;
            break;
        }
    }

    Ok(ReleaseInfo {
        current_version: cur,
        latest_version: rel.tag_name.clone(),
        release_name: if rel.name.is_empty() { rel.tag_name } else { rel.name },
        release_notes: rel.body,
        html_url: if rel.html_url.is_empty() { default_url } else { rel.html_url },
        asset_name,
        asset_url,
        asset_size,
        has_update,
        is_dev,
        can_self_update,
    })
}

pub fn download_and_apply(
    asset_url: &str,
    asset_name: &str,
    tx: &Sender<UpdateEvent>,
    ctx: &eframe::egui::Context,
) -> Result<(), String> {
    if asset_url.is_empty() {
        return Err("缺少更新包下载地址".to_string());
    }
    let exe_path = std::env::current_exe().map_err(|e| format!("定位当前程序失败: {e}"))?;
    let exe_path = fs::canonicalize(&exe_path).unwrap_or(exe_path);
    let dir = exe_path
        .parent()
        .ok_or_else(|| "无法获取程序所在目录".to_string())?;

    let agent = ureq::AgentBuilder::new()
        .timeout_connect(Duration::from_secs(15))
        .timeout_read(Duration::from_secs(60))
        .build();

    let resp = agent
        .get(asset_url)
        .set("User-Agent", &format!("rdpmanager/{}", current_version()))
        .call()
        .map_err(|e| format!("下载更新包失败: {e}"))?;

    let total: u64 = resp
        .header("Content-Length")
        .and_then(|s| s.parse().ok())
        .unwrap_or(0);

    let tmp_dl = dir.join(".rdpmanager-update.tmp");
    {
        let mut reader = resp.into_reader();
        let mut out = File::create(&tmp_dl).map_err(|e| format!("创建临时文件失败: {e}"))?;
        let mut buf = [0u8; 32 * 1024];
        let mut done: u64 = 0;
        let mut last_emit = Instant::now();
        loop {
            let n = reader.read(&mut buf).map_err(|e| format!("读取更新包失败: {e}"))?;
            if n == 0 {
                break;
            }
            out.write_all(&buf[..n])
                .map_err(|e| format!("写入更新包失败: {e}"))?;
            done += n as u64;
            if last_emit.elapsed() >= Duration::from_millis(120) {
                let _ = tx.send(UpdateEvent::Progress { done, total });
                ctx.request_repaint();
                last_emit = Instant::now();
            }
        }
        let _ = tx.send(UpdateEvent::Progress { done, total });
        ctx.request_repaint();
    }

    let new_bin = if asset_name.to_ascii_lowercase().ends_with(".exe") {
        tmp_dl.clone()
    } else if asset_name.to_ascii_lowercase().ends_with(".tar.gz") {
        let extracted = dir.join(".rdpmanager-update.bin");
        extract_from_tar_gz(&tmp_dl, "rdpmanager", &extracted)?;
        let _ = fs::remove_file(&tmp_dl);
        extracted
    } else {
        let _ = fs::remove_file(&tmp_dl);
        return Err(format!("不支持的更新包格式: {asset_name}"));
    };

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = fs::set_permissions(&new_bin, fs::Permissions::from_mode(0o755));
    }

    let old_path = old_backup_path(&exe_path);
    let _ = fs::remove_file(&old_path);
    fs::rename(&exe_path, &old_path).map_err(|e| {
        let _ = fs::remove_file(&new_bin);
        format!("备份当前程序失败: {e}")
    })?;

    if let Err(e) = fs::rename(&new_bin, &exe_path) {
        let _ = fs::rename(&old_path, &exe_path);
        return Err(format!("替换新版本失败: {e}"));
    }

    Command::new(&exe_path)
        .spawn()
        .map_err(|e| format!("启动新版本失败: {e}"))?;

    Ok(())
}

fn extract_from_tar_gz(tar_gz: &Path, target_name: &str, dst: &Path) -> Result<(), String> {
    let f = File::open(tar_gz).map_err(|e| format!("打开更新包失败: {e}"))?;
    let gz = flate2::read::GzDecoder::new(f);
    let mut archive = tar::Archive::new(gz);
    let entries = archive.entries().map_err(|e| format!("读取 tar 失败: {e}"))?;
    for entry in entries {
        let mut entry = entry.map_err(|e| format!("读取 tar 条目失败: {e}"))?;
        let path = entry.path().map_err(|e| format!("解析路径失败: {e}"))?;
        if path.file_name().and_then(|n| n.to_str()) == Some(target_name) {
            let mut out = File::create(dst).map_err(|e| format!("创建解压文件失败: {e}"))?;
            std::io::copy(&mut entry, &mut out).map_err(|e| format!("解压文件失败: {e}"))?;
            return Ok(());
        }
    }
    Err(format!("更新包中未找到 {target_name}"))
}

pub fn open_url(url: &str) {
    if url.is_empty() {
        return;
    }
    #[cfg(target_os = "windows")]
    let _ = Command::new("cmd").args(["/C", "start", "", url]).spawn();
    #[cfg(target_os = "macos")]
    let _ = Command::new("open").arg(url).spawn();
    #[cfg(all(unix, not(target_os = "macos")))]
    let _ = Command::new("xdg-open").arg(url).spawn();
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_semver_compare() {
        assert_eq!(compare_semver("v0.1.0", "v0.2.0"), std::cmp::Ordering::Less);
        assert_eq!(compare_semver("0.1.0", "v0.1.0"), std::cmp::Ordering::Equal);
        assert_eq!(compare_semver("v1.2.3", "v1.2.2"), std::cmp::Ordering::Greater);
        assert_eq!(compare_semver("v1.0.0-rc.1", "v1.0.0"), std::cmp::Ordering::Less);
        assert!(is_newer("v0.1.0", "v0.2.0"));
        assert!(!is_newer("dev", "v0.2.0"));
        assert!(!is_newer("build-20261127-abc1234", "v0.2.0"));
    }
}
