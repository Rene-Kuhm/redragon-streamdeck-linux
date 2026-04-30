fn main() {
    let git_head = "../.git/HEAD";
    println!("cargo:rerun-if-changed={git_head}");

    if let Ok(head) = std::fs::read_to_string(git_head) {
        if let Some(ref_path) = head.trim().strip_prefix("ref: ") {
            println!("cargo:rerun-if-changed=../.git/{ref_path}");
        }
    }

    if let Ok(output) = std::process::Command::new("git")
        .args(["rev-parse", "HEAD"])
        .output()
    {
        if output.status.success() {
            let commit = String::from_utf8_lossy(&output.stdout).trim().to_string();
            println!("cargo:rustc-env=REDRAGON_CURRENT_COMMIT={commit}");
        }
    }

    tauri_build::build()
}
