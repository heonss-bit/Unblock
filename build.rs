// Встраивает в exe иконку и просьбу о правах администратора (они нужны драйверу WinDivert).
fn main() {
    println!("cargo:rerun-if-changed=unblock.rc");
    println!("cargo:rerun-if-changed=unblock.manifest");
    println!("cargo:rerun-if-changed=icon.ico");
    if std::env::var("CARGO_CFG_TARGET_OS").map(|v| v == "windows").unwrap_or(false) {
        let _ = embed_resource::compile("unblock.rc", embed_resource::NONE);
    }
}
