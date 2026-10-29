//! Native COM and Media Foundation helpers, adapted under the reference MIT licence.
use crate::error::{Error, Result};
use serde_json::{json, Value};
use std::{
    path::Path,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
};
use windows::{
    core::{GUID, HSTRING},
    Win32::{Media::MediaFoundation::*, System::Com::*},
};
pub struct Runtime;
impl Runtime {
    pub fn new() -> Result<Self> {
        unsafe {
            CoInitializeEx(None, COINIT_MULTITHREADED).ok()?;
            if let Err(e) = MFStartup(MF_VERSION, MFSTARTUP_FULL) {
                CoUninitialize();
                return Err(Error::new("native_unavailable",format!("Windows Media Foundation is unavailable. Windows N editions need the Media Feature Pack. {e}")));
            }
        }
        Ok(Self)
    }
}
impl Drop for Runtime {
    fn drop(&mut self) {
        unsafe {
            let _ = MFShutdown();
            CoUninitialize();
        }
    }
}
pub fn attributes() -> Result<IMFAttributes> {
    let mut a = None;
    unsafe {
        MFCreateAttributes(&mut a, 8)?;
    }
    Ok(a.unwrap())
}
pub fn packed(a: u32, b: u32) -> u64 {
    ((a as u64) << 32) | b as u64
}
pub fn check_cancel(cancel: &Arc<AtomicBool>) -> Result<()> {
    if cancel.load(Ordering::SeqCst) {
        Err(Error::new(
            "cancelled",
            "Operation cancelled; current partial output discarded",
        ))
    } else {
        Ok(())
    }
}
pub fn event(v: Value) {
    eprintln!("{v}");
}
pub fn sample_bytes(bytes: &[u8], time: i64, duration: i64) -> Result<IMFSample> {
    unsafe {
        let b = MFCreateMemoryBuffer(bytes.len() as u32)?;
        let mut ptr = std::ptr::null_mut();
        b.Lock(&mut ptr, None, None)?;
        std::ptr::copy_nonoverlapping(bytes.as_ptr(), ptr, bytes.len());
        b.Unlock()?;
        b.SetCurrentLength(bytes.len() as u32)?;
        let sample = MFCreateSample()?;
        sample.AddBuffer(&b)?;
        sample.SetSampleTime(time)?;
        sample.SetSampleDuration(duration)?;
        Ok(sample)
    }
}

pub fn encoders(category: GUID, subtype: GUID) -> Result<Vec<Value>> {
    unsafe {
        let mut ptr = std::ptr::null_mut();
        let mut count = 0;
        let output = MFT_REGISTER_TYPE_INFO {
            guidMajorType: if category == MFT_CATEGORY_AUDIO_ENCODER {
                MFMediaType_Audio
            } else {
                MFMediaType_Video
            },
            guidSubtype: subtype,
        };
        MFTEnumEx(
            category,
            MFT_ENUM_FLAG_ALL,
            None,
            Some(&output),
            &mut ptr,
            &mut count,
        )?;
        let mut result = vec![];
        if !ptr.is_null() {
            for entry in std::slice::from_raw_parts_mut(ptr, count as usize) {
                if let Some(a) = entry.take() {
                    let mut name = vec![0u16; 512];
                    let _ = a.GetString(&MFT_FRIENDLY_NAME_Attribute, &mut name, None);
                    let end = name.iter().position(|c| *c == 0).unwrap_or(name.len());
                    result.push(json!({"name":String::from_utf16_lossy(&name[..end]),"hardware_registered":a.GetStringLength(&MFT_ENUM_HARDWARE_URL_Attribute).is_ok()}));
                }
            }
            CoTaskMemFree(Some(ptr.cast()));
        }
        Ok(result)
    }
}

pub fn native_path(path: &Path) -> HSTRING {
    // Canonical Windows paths carry an extended prefix that Media Foundation's
    // URL resolver can mistake for a network URL. Preserve Unicode, remove it.
    let value = path.to_string_lossy();
    if let Some(unc) = value.strip_prefix("\\\\?\\UNC\\") {
        HSTRING::from(format!("\\\\{unc}"))
    } else {
        HSTRING::from(value.strip_prefix("\\\\?\\").unwrap_or(&value))
    }
}
pub fn peak_memory() -> Option<usize> {
    use windows::Win32::System::{ProcessStatus::*, Threading::GetCurrentProcess};
    unsafe {
        let mut counters = PROCESS_MEMORY_COUNTERS {
            cb: std::mem::size_of::<PROCESS_MEMORY_COUNTERS>() as u32,
            ..Default::default()
        };
        GetProcessMemoryInfo(
            GetCurrentProcess(),
            &mut counters,
            std::mem::size_of::<PROCESS_MEMORY_COUNTERS>() as u32,
        )
        .ok()
        .map(|_| counters.PeakWorkingSetSize)
    }
}
pub fn verify_video(path: &Path, cancel: &Arc<AtomicBool>) -> Result<Value> {
    unsafe {
        let reader = MFCreateSourceReaderFromURL(&native_path(path), None)?;
        let stream = MF_SOURCE_READER_FIRST_VIDEO_STREAM.0 as u32;
        reader.SetStreamSelection(MF_SOURCE_READER_ALL_STREAMS.0 as u32, false)?;
        reader.SetStreamSelection(stream, true)?;
        let native = reader.GetNativeMediaType(stream, 0)?;
        let size = native.GetUINT64(&MF_MT_FRAME_SIZE)?;
        let output = MFCreateMediaType()?;
        output.SetGUID(&MF_MT_MAJOR_TYPE, &MFMediaType_Video)?;
        output.SetGUID(&MF_MT_SUBTYPE, &MFVideoFormat_NV12)?;
        reader.SetCurrentMediaType(stream, None, &output)?;
        let mut count = 0u64;
        let mut first = None;
        let mut last = 0;
        loop {
            check_cancel(cancel)?;
            let mut sample = None;
            let mut flags = 0;
            let mut time = 0;
            reader.ReadSample(
                stream,
                0,
                None,
                Some(&mut flags),
                Some(&mut time),
                Some(&mut sample),
            )?;
            if flags & MF_SOURCE_READERF_ERROR.0 as u32 != 0 {
                return Err(Error::new(
                    "verification_failed",
                    "Native decoder reported an error",
                ));
            }
            if sample.is_some() {
                count += 1;
                first.get_or_insert(time);
                last = time;
            }
            if flags & MF_SOURCE_READERF_ENDOFSTREAM.0 as u32 != 0 {
                break;
            }
        }
        Ok(
            json!({"frames":count,"width":size>>32,"height":size as u32,"first_timestamp_ticks":first,"last_timestamp_ticks":last,"decoded":true}),
        )
    }
}
pub fn doctor() -> Result<Value> {
    let _runtime = Runtime::new()?;
    let g = crate::graphics::Graphics::new(false)?;
    let encoders = encoders(MFT_CATEGORY_VIDEO_ENCODER, MFVideoFormat_H264)?;
    Ok(
        json!({"ok":!encoders.is_empty(),"platform":"windows-x64","graphics":g.info,"h264_encoders":encoders,"runtime_dependencies":"Windows system components only","note":"Registration is diagnostic; actual render establishes encoder functionality."}),
    )
}
