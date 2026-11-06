fn main() {
    println!("cargo:rerun-if-env-changed=HEYDAY_API_URL");
    // SQLx embeds migrations; newly added files must rebuild the native app too.
    println!("cargo:rerun-if-changed=migrations");
    tauri_build::build()
}
