use std::path::{Path, PathBuf};

/// Localiza una fuente TrueType para dibujar el texto de los botones y la copia
/// a `OUT_DIR`, de donde `lib.rs` la embebe con `include_bytes!`.
///
/// Antes la fuente se embebía desde `/usr/share/fonts/TTF/DejaVuSans.ttf`, que
/// es la ruta de Arch. En cualquier otra distro el build fallaba con
/// "couldn't read ... No such file or directory", incluso en Fedora, para la que
/// el repo trae un `install-fedora.sh`.
fn find_button_font() -> Option<PathBuf> {
    // Ubicaciones habituales de DejaVu segun distro, y despues alternativas
    // metricamente parecidas por si DejaVu no esta instalada.
    const CANDIDATES: &[&str] = &[
        "/usr/share/fonts/TTF/DejaVuSans.ttf",                  // Arch
        "/usr/share/fonts/dejavu-sans-fonts/DejaVuSans.ttf",    // Fedora
        "/usr/share/fonts/dejavu/DejaVuSans.ttf",               // Fedora (antiguo)
        "/usr/share/fonts/truetype/dejavu/DejaVuSans.ttf",      // Debian / Ubuntu
        "/usr/share/fonts/truetype/DejaVuSans.ttf",
        "/usr/share/fonts/google-noto/NotoSans-Regular.ttf",    // Fedora, Noto
        "/usr/share/fonts/noto/NotoSans-Regular.ttf",
        "/usr/share/fonts/liberation-sans/LiberationSans-Regular.ttf",
        "/usr/share/fonts/liberation/LiberationSans-Regular.ttf",
    ];

    for candidate in CANDIDATES {
        let path = Path::new(candidate);
        if path.is_file() {
            return Some(path.to_path_buf());
        }
    }

    // Ultimo recurso: que fontconfig resuelva la sans-serif del sistema.
    let output = std::process::Command::new("fc-match")
        .args(["-f", "%{file}", "sans-serif"])
        .output()
        .ok()?;

    if !output.status.success() {
        return None;
    }

    let path = PathBuf::from(String::from_utf8(output.stdout).ok()?.trim());
    // fontconfig puede devolver OTF o TTC; ab_glyph solo carga TrueType.
    let usable = path.is_file()
        && path
            .extension()
            .and_then(|e| e.to_str())
            .map(|e| e.eq_ignore_ascii_case("ttf"))
            .unwrap_or(false);

    usable.then_some(path)
}

fn embed_button_font() {
    let font = find_button_font().expect(
        "No se encontro ninguna fuente TrueType para dibujar el texto de los botones.\n\
         Instala una de estas segun tu distro:\n  \
           Fedora:        sudo dnf install dejavu-sans-fonts\n  \
           Arch:          sudo pacman -S ttf-dejavu\n  \
           Debian/Ubuntu: sudo apt install fonts-dejavu-core",
    );

    let out_dir = std::env::var("OUT_DIR").expect("cargo no definio OUT_DIR");
    let dest = Path::new(&out_dir).join("button-font.ttf");

    std::fs::copy(&font, &dest)
        .unwrap_or_else(|e| panic!("no se pudo copiar la fuente {}: {e}", font.display()));

    println!("cargo:rerun-if-changed={}", font.display());
}

fn main() {
    embed_button_font()
}
