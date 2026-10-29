//! Carry licence notices inside the portable executable, including direct downloads.
use std::{env, fs, path::Path};

fn collect_notices(directory: &Path, output: &mut String) -> std::io::Result<()> {
    let mut entries = fs::read_dir(directory)?.collect::<std::io::Result<Vec<_>>>()?;
    entries.sort_by_key(|entry| entry.file_name());
    for entry in entries {
        let path = entry.path();
        if path.is_dir() {
            collect_notices(&path, output)?;
        } else {
            output.push_str(&format!("\n\n--- {} ---\n\n", path.display()));
            output.push_str(&fs::read_to_string(path)?);
        }
    }
    Ok(())
}

fn main() -> std::io::Result<()> {
    let mut notices = fs::read_to_string("LICENSE")?;
    notices.push_str("\n\n");
    notices.push_str(&fs::read_to_string("THIRD_PARTY_NOTICES.md")?);
    collect_notices(Path::new("third-party"), &mut notices)?;
    fs::write(
        Path::new(&env::var_os("OUT_DIR").expect("Cargo supplies OUT_DIR")).join("licenses.txt"),
        notices,
    )?;
    for path in ["LICENSE", "THIRD_PARTY_NOTICES.md", "third-party"] {
        println!("cargo:rerun-if-changed={path}");
    }
    Ok(())
}
