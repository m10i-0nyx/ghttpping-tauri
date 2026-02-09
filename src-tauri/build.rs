fn main() {
    // Set MSI installer language to ja-JP
    std::env::set_var("TAURI_WINDOWS_MSI_LANGUAGES", "ja-JP");
    tauri_build::build()
}
