fn main() {
    // Simge değişince exe kaynağı yeniden gömülsün (önbellekte eski simge kalmasın).
    println!("cargo:rerun-if-changed=icons");
    println!("cargo:rerun-if-changed=tauri.conf.json");
    tauri_build::build();
}
