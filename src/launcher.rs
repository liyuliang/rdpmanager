use std::path::Path;
use std::process::Command;

/// Launch the platform's RDP client with the given .rdp file.
/// On Windows, if credentials are provided, store them via cmdkey first
/// and create a temporary .rdp copy with credential prompts disabled.
pub fn launch(path: &Path, host: &str, username: &str, password: &str) -> Result<(), String> {
    if !path.exists() {
        return Err(format!("找不到 .rdp 文件: {}", path.display()));
    }
    save_credential(host, username, password);

    // Also save credentials for the RD Gateway if present in the .rdp file,
    // so mstsc won't prompt for gateway authentication either.
    if !username.is_empty() && !password.is_empty() {
        if let Ok(bytes) = std::fs::read(path) {
            let text = decode_rdp_text(&bytes);
            if let Some(gw) = extract_gateway_host(&text) {
                if !gw.is_empty() {
                    save_credential(&gw, username, password);
                }
            }
        }
    }

    // If we have credentials, create a temp copy of the .rdp file with
    // credential prompting disabled so mstsc uses the saved credential.
    let launch_path;
    let _cleanup; // dropped at end of scope
    if !username.is_empty() && !password.is_empty() {
        let temp = patch_rdp_for_launch(path)?;
        launch_path = temp.clone();
        _cleanup = TempFile(Some(temp));
    } else {
        launch_path = path.to_path_buf();
        _cleanup = TempFile(None);
    }

    spawn_client(&launch_path)
}

/// Tiny RAII guard that deletes a temp file when dropped.
struct TempFile(Option<std::path::PathBuf>);
impl Drop for TempFile {
    fn drop(&mut self) {
        if let Some(p) = self.0.take() {
            // Small delay so mstsc has time to read the file.
            std::thread::spawn(move || {
                std::thread::sleep(std::time::Duration::from_secs(5));
                let _ = std::fs::remove_file(p);
            });
        }
    }
}

/// Create a temporary copy of the .rdp file with credential prompts disabled.
fn patch_rdp_for_launch(original: &Path) -> Result<std::path::PathBuf, String> {
    let bytes = std::fs::read(original)
        .map_err(|e| format!("读取 .rdp 失败: {e}"))?;

    let text = decode_rdp_text(&bytes);
    let mut lines: Vec<String> = text.lines().map(|l| l.to_string()).collect();
    let mut found_prompt = false;
    let mut found_prompt_client = false;
    let mut found_prompt_once = false;
    let mut found_gw_cred_source = false;

    for line in &mut lines {
        let key = line.split(':').next().unwrap_or("").trim().to_ascii_lowercase();
        match key.as_str() {
            "prompt for credentials" => {
                *line = "prompt for credentials:i:0".to_string();
                found_prompt = true;
            }
            "prompt for credentials on client" => {
                *line = "prompt for credentials on client:i:0".to_string();
                found_prompt_client = true;
            }
            "promptcredentialonce" => {
                // 1 = prompt once and reuse the same credentials for both
                // the gateway and the remote host.  With "prompt for
                // credentials:i:0" mstsc reads them from cmdkey instead
                // of actually prompting.
                *line = "promptcredentialonce:i:1".to_string();
                found_prompt_once = true;
            }
            "gatewaycredentialssource" => {
                // 0 = NTLM – combined with promptcredentialonce:i:1 the
                // gateway receives the same stored credential as the
                // remote host.
                *line = "gatewaycredentialssource:i:0".to_string();
                found_gw_cred_source = true;
            }
            // Remove the signature – it becomes invalid once we patch any field,
            // and leaving it causes mstsc to show a "publisher" warning.
            "signscope" | "signature" => {
                *line = String::new();
            }
            _ => {}
        }
    }
    // Remove blanked-out lines (signature fields).
    lines.retain(|l| !l.is_empty());

    if !found_prompt {
        lines.push("prompt for credentials:i:0".to_string());
    }
    if !found_prompt_client {
        lines.push("prompt for credentials on client:i:0".to_string());
    }
    if !found_prompt_once {
        lines.push("promptcredentialonce:i:1".to_string());
    }
    if !found_gw_cred_source {
        lines.push("gatewaycredentialssource:i:0".to_string());
    }

    let mut result = lines.join("\n");
    if !result.ends_with('\n') {
        result.push('\n');
    }

    // Write to a temp file next to the original.
    let stem = original.file_stem().and_then(|s| s.to_str()).unwrap_or("conn");
    let temp_name = format!("{stem}_launch.rdp");
    let temp_path = original.parent().unwrap_or(Path::new(".")).join(temp_name);
    std::fs::write(&temp_path, result)
        .map_err(|e| format!("创建临时 .rdp 失败: {e}"))?;
    Ok(temp_path)
}

/// Extract the gateway hostname from .rdp text, if any.
fn extract_gateway_host(text: &str) -> Option<String> {
    for line in text.lines() {
        let mut parts = line.trim().splitn(3, ':');
        let key = parts.next()?.trim().to_ascii_lowercase();
        let _type = parts.next();
        let value = parts.next().unwrap_or("").trim();
        if key == "gatewayhostname" && !value.is_empty() {
            return Some(value.to_string());
        }
    }
    None
}

/// Decode .rdp file bytes (handles UTF-16 BOM).
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

/// Store RDP credentials in the platform's credential store so mstsc doesn't
/// prompt for a password.
fn save_credential(host: &str, username: &str, password: &str) {
    if username.is_empty() || password.is_empty() {
        return;
    }
    save_credential_platform(host, username, password);
}

#[cfg(target_os = "windows")]
fn save_credential_platform(host: &str, username: &str, password: &str) {
    // Strip port if present – cmdkey targets the hostname only.
    let target = host.split(':').next().unwrap_or(host);
    // Use /add: (not /generic:) to create a "Domain Password" credential
    // that mstsc actually reads. /generic: creates a "Generic" credential
    // which mstsc ignores.
    let _ = Command::new("cmdkey")
        .args([
            &format!("/add:TERMSRV/{target}"),
            &format!("/user:{username}"),
            &format!("/pass:{password}"),
        ])
        .output();
}

#[cfg(not(target_os = "windows"))]
fn save_credential_platform(_host: &str, _username: &str, _password: &str) {
    // No-op on non-Windows platforms for now.
}

#[cfg(target_os = "windows")]
fn spawn_client(path: &Path) -> Result<(), String> {
    Command::new("mstsc.exe")
        .arg(path)
        .spawn()
        .map(|_| ())
        .map_err(|e| format!("无法启动 mstsc.exe: {e}"))
}

#[cfg(target_os = "macos")]
fn spawn_client(path: &Path) -> Result<(), String> {
    // `open` resolves the .rdp file association (Windows App / Microsoft Remote Desktop).
    Command::new("open")
        .arg(path)
        .spawn()
        .map(|_| ())
        .map_err(|e| format!("无法启动 RDP 客户端 (open): {e}"))
}

#[cfg(target_os = "linux")]
fn spawn_client(path: &Path) -> Result<(), String> {
    // Prefer the desktop's file association (usually Remmina), fall back to FreeRDP.
    match Command::new("xdg-open").arg(path).spawn() {
        Ok(_) => Ok(()),
        Err(_) => Command::new("xfreerdp")
            .arg(path)
            .spawn()
            .map(|_| ())
            .map_err(|_| {
                "未找到可用的 RDP 客户端。请安装 Remmina 或 FreeRDP (xfreerdp)，或把 .rdp 关联到默认程序。"
                    .to_string()
            }),
    }
}

#[cfg(not(any(target_os = "windows", target_os = "macos", target_os = "linux")))]
fn spawn_client(_path: &Path) -> Result<(), String> {
    Err("当前平台暂不支持自动启动 RDP 客户端".to_string())
}
