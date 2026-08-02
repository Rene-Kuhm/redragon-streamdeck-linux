fn main() {
    let git_head = "../.git/HEAD";
    println!("cargo:rerun-if-changed={git_head}");

    if let Ok(head) = std::fs::read_to_string(git_head) {
        if let Some(ref_path) = head.trim().strip_prefix("ref: ") {
            println!("cargo:rerun-if-changed=../.git/{ref_path}");
        }
    }

    // Publicar un release crea una etiqueta nueva sin tocar ningun archivo, asi
    // que sin esto cargo reutilizaria la compilacion anterior y grabaria la
    // version vieja.
    println!("cargo:rerun-if-changed=../.git/refs/tags");
    println!("cargo:rerun-if-changed=../.git/packed-refs");

    if let Ok(output) = std::process::Command::new("git")
        .args(["rev-parse", "HEAD"])
        .output()
    {
        if output.status.success() {
            let commit = String::from_utf8_lossy(&output.stdout).trim().to_string();
            println!("cargo:rustc-env=REDRAGON_CURRENT_COMMIT={commit}");
        }
    }

    // La version instalada la manda la etiqueta, no `Cargo.toml`.
    //
    // El release lo publica el CI al mergear a main, y no puede escribir en esa
    // rama porque exige pull request. Si la version viviera en `Cargo.toml`,
    // quedaria congelada mientras las etiquetas avanzan: el usuario actualizaria,
    // recompilaria, y su binario seguiria anunciandose con la version anterior,
    // o sea un aviso de actualizacion que nunca se resuelve.
    if let Ok(output) = std::process::Command::new("git")
        .args(["describe", "--tags", "--abbrev=0"])
        .output()
    {
        if output.status.success() {
            let tag = String::from_utf8_lossy(&output.stdout).trim().to_string();
            if !tag.is_empty() {
                println!("cargo:rustc-env=REDRAGON_RELEASE_TAG={tag}");
            }
        }
    }

    tauri_build::build()
}
