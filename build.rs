fn main() {
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows") {
        let mut res = winresource::WindowsResource::new();
        res.set_icon("assets/app.ico");
        res.set("ProductName", "RDP 管理器");
        res.set("FileDescription", "远程桌面连接管理器");
        res.compile().expect("Failed to compile Windows resources");
    }
}
