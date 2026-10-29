use crate::error::{Error, Result};
use std::path::Path;
pub struct StagedOutput {
    path: tempfile::TempPath,
}
impl StagedOutput {
    pub fn new(output: &Path, overwrite: bool) -> Result<Self> {
        if output
            .extension()
            .is_none_or(|e| !e.eq_ignore_ascii_case("mp4"))
        {
            return Err(Error::new(
                "invalid_arguments",
                "Output must have .mp4 extension",
            ));
        }
        if output.exists() && !overwrite {
            return Err(Error::new(
                "output_exists",
                "Output exists; use --overwrite to replace it",
            ));
        }
        if output.is_dir() {
            return Err(Error::new("output_exists", "Output path is a directory"));
        }
        let parent = output
            .parent()
            .filter(|p| !p.as_os_str().is_empty())
            .unwrap_or(Path::new("."));
        let path = tempfile::Builder::new()
            .prefix(".cmtga-")
            .suffix(".partial.mp4")
            .tempfile_in(parent)?
            .into_temp_path();
        Ok(Self { path })
    }
    pub fn path(&self) -> &Path {
        &self.path
    }
    pub fn publish(self, output: &Path, overwrite: bool) -> Result<()> {
        use windows::Win32::Storage::FileSystem::*;
        // Same-directory rename is atomic and does not require hard-link privileges.
        // Omitting REPLACE_EXISTING also handles another writer winning the race.
        let flags = if overwrite {
            MOVEFILE_WRITE_THROUGH | MOVEFILE_REPLACE_EXISTING
        } else {
            MOVEFILE_WRITE_THROUGH
        };
        unsafe {
            MoveFileExW(
                &crate::media::native_path(&self.path),
                &crate::media::native_path(output),
                flags,
            )
            .map_err(|e| {
                let code = if matches!(e.code().0 as u32, 0x80070050 | 0x800700B7) {
                    "output_exists"
                } else {
                    "io_failed"
                };
                Error::new(code, e.to_string())
            })?;
        }
        Ok(())
    }
}
