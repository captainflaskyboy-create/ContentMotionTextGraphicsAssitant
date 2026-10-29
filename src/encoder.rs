//! H.264 sink writer. A sample owns its pixels until Media Foundation releases it.
use crate::{
    error::{Error, Result},
    graphics::Graphics,
    media::{self, *},
    scene::Scene,
};
use serde_json::{json, Value};
use std::{
    path::Path,
    sync::{atomic::AtomicBool, Arc},
};
use windows::{
    core::{implement, Interface, Ref, GUID},
    Win32::{
        Foundation::E_NOTIMPL,
        Graphics::{
            Direct3D11::*,
            Dxgi::{Common::*, IDXGISurface2},
        },
        Media::MediaFoundation::*,
    },
};

fn video_type(subtype: &GUID, s: &Scene) -> Result<IMFMediaType> {
    unsafe {
        let t = MFCreateMediaType()?;
        t.SetGUID(&MF_MT_MAJOR_TYPE, &MFMediaType_Video)?;
        t.SetGUID(&MF_MT_SUBTYPE, subtype)?;
        t.SetUINT64(&MF_MT_FRAME_SIZE, packed(s.canvas.width, s.canvas.height))?;
        t.SetUINT64(
            &MF_MT_FRAME_RATE,
            packed(s.canvas.fps.numerator, s.canvas.fps.denominator),
        )?;
        t.SetUINT64(&MF_MT_PIXEL_ASPECT_RATIO, packed(1, 1))?;
        t.SetUINT32(&MF_MT_INTERLACE_MODE, MFVideoInterlace_Progressive.0 as u32)?;
        Ok(t)
    }
}
pub struct Encoder {
    pub writer: IMFSinkWriter,
    pub video: u32,
    pub gpu_input: bool,
    pub info: Value,
    finalized: bool,
}
impl Encoder {
    pub fn new(path: &Path, s: &Scene, g: &Graphics, gpu: bool, software: bool) -> Result<Self> {
        unsafe {
            let a = attributes()?;
            a.SetUINT32(&MF_READWRITE_ENABLE_HARDWARE_TRANSFORMS, (!software) as u32)?;
            // Leave sink-writer throttling enabled: downstream backpressure bounds CPU input too.
            if gpu {
                a.SetUnknown(&MF_SINK_WRITER_D3D_MANAGER, &g.manager)?;
            }
            let writer = MFCreateSinkWriterFromURL(&native_path(path), None, Some(&a))?;
            let out = video_type(&MFVideoFormat_H264, s)?;
            out.SetUINT32(&MF_MT_AVG_BITRATE, s.canvas.bitrate_kbps * 1000)?;
            let video = writer.AddStream(&out)?;
            let input = video_type(
                if gpu {
                    &MFVideoFormat_ARGB32
                } else {
                    &MFVideoFormat_RGB32
                },
                s,
            )?;
            input.SetUINT32(&MF_MT_DEFAULT_STRIDE, s.canvas.width * 4)?;
            writer.SetInputMediaType(video, &input, None)?;
            writer.BeginWriting()?;
            let mut transforms = Vec::new();
            if let Ok(ex) = writer.cast::<IMFSinkWriterEx>() {
                for i in 0..8 {
                    let mut t = None;
                    let mut category = GUID::zeroed();
                    if ex
                        .GetTransformForStream(video, i, Some(&mut category), &mut t)
                        .is_err()
                    {
                        break;
                    }
                    if let Some(t) = t {
                        let attr = t.GetAttributes().ok();
                        let hardware = attr.as_ref().is_some_and(|a| {
                            a.GetStringLength(&MFT_ENUM_HARDWARE_URL_Attribute).is_ok()
                        });
                        transforms.push(json!({"hardware_observed":hardware,"category":format!("{category:?}")}));
                    }
                }
            }
            Ok(Self {
                writer,
                video,
                gpu_input: gpu,
                info: json!({"input":if gpu{"bounded_gpu_surfaces"}else{"system_memory_bgra"},"hardware_requested":!software,"transforms":transforms}),
                finalized: false,
            })
        }
    }
    pub fn video(&self, pixels: &[u8], time: i64, duration: i64) -> Result<()> {
        unsafe {
            self.writer
                .WriteSample(self.video, &sample_bytes(pixels, time, duration)?)?;
        }
        Ok(())
    }
    pub fn finish(mut self) -> Result<()> {
        unsafe {
            self.writer.Finalize()?;
        }
        self.finalized = true;
        Ok(())
    }
}
impl Drop for Encoder {
    fn drop(&mut self) {
        if !self.finalized {
            unsafe {
                let _ = self.writer.Flush(self.video);
            }
        }
    }
}
#[implement(IMFAsyncCallback)]
struct Recycle {
    slot: usize,
    sender: std::sync::mpsc::Sender<usize>,
}
impl IMFAsyncCallback_Impl for Recycle_Impl {
    fn GetParameters(&self, _flags: *mut u32, _queue: *mut u32) -> windows::core::Result<()> {
        Err(E_NOTIMPL.into())
    }
    fn Invoke(&self, _result: Ref<IMFAsyncResult>) -> windows::core::Result<()> {
        let _ = self.sender.send(self.slot);
        Ok(())
    }
}
/// A bounded GPU pool. Tracked samples recycle a texture only after downstream
/// transforms release it, preventing overwrites while an async encoder reads it.
pub struct SurfacePool {
    textures: Vec<ID3D11Texture2D>,
    callbacks: Vec<IMFAsyncCallback>,
    available: std::sync::mpsc::Receiver<usize>,
}
impl SurfacePool {
    pub fn new(g: &Graphics, w: u32, h: u32) -> Result<Self> {
        unsafe {
            let (tx, rx) = std::sync::mpsc::channel();
            let mut textures = vec![];
            let mut callbacks = vec![];
            for i in 0..8 {
                let mut texture = None;
                g.device.CreateTexture2D(
                    &D3D11_TEXTURE2D_DESC {
                        Width: w,
                        Height: h,
                        MipLevels: 1,
                        ArraySize: 1,
                        Format: DXGI_FORMAT_B8G8R8A8_UNORM,
                        SampleDesc: DXGI_SAMPLE_DESC {
                            Count: 1,
                            Quality: 0,
                        },
                        Usage: D3D11_USAGE_DEFAULT,
                        BindFlags: (D3D11_BIND_RENDER_TARGET | D3D11_BIND_SHADER_RESOURCE).0 as u32,
                        CPUAccessFlags: 0,
                        MiscFlags: 0,
                    },
                    None,
                    Some(&mut texture),
                )?;
                textures.push(texture.unwrap());
                callbacks.push(
                    Recycle {
                        slot: i,
                        sender: tx.clone(),
                    }
                    .into(),
                );
                tx.send(i).unwrap();
            }
            Ok(Self {
                textures,
                callbacks,
                available: rx,
            })
        }
    }
    pub fn write(
        &self,
        g: &Graphics,
        encoder: &Encoder,
        bitmap: &windows::Win32::Graphics::Direct2D::ID2D1Bitmap1,
        time: i64,
        duration: i64,
        cancel: &Arc<AtomicBool>,
    ) -> Result<()> {
        unsafe {
            let started = std::time::Instant::now();
            let index = loop {
                media::check_cancel(cancel)?;
                match self
                    .available
                    .recv_timeout(std::time::Duration::from_millis(30))
                {
                    Ok(i) => break i,
                    Err(_) => {
                        if started.elapsed().as_secs() > 5 {
                            return Err(Error::new(
                                "gpu_surface_failed",
                                "Native encoder did not release a GPU sample within five seconds",
                            ));
                        }
                    }
                }
            };
            let surface: IDXGISurface2 = bitmap.GetSurface()?.cast()?;
            let mut sub = 0;
            let source: ID3D11Texture2D = surface.GetResource(&mut sub)?;
            g.device
                .GetImmediateContext()?
                .CopyResource(&self.textures[index], &source);
            let buffer =
                MFCreateDXGISurfaceBuffer(&ID3D11Texture2D::IID, &self.textures[index], 0, false)?;
            buffer.SetCurrentLength(buffer.GetMaxLength()?)?;
            let tracked = MFCreateTrackedSample()?;
            let sample: IMFSample = tracked.cast()?;
            sample.AddBuffer(&buffer)?;
            sample.SetSampleTime(time)?;
            sample.SetSampleDuration(duration)?;
            tracked.SetAllocator(&self.callbacks[index], None)?;
            encoder
                .writer
                .WriteSample(encoder.video, &sample)
                .map_err(|e| {
                    Error::new(
                        "gpu_surface_failed",
                        format!("GPU surface encoding failed: {e}"),
                    )
                })?;
            Ok(())
        }
    }
}
