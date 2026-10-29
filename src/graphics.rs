//! Off-screen Direct2D compositor, using one shared D3D11 device, or WARP.
use crate::{
    error::{Error, Result},
    media,
};
use serde_json::{json, Value};
use std::path::Path;
use windows::{
    core::Interface,
    Win32::{
        Foundation::*,
        Graphics::{
            Direct2D::{Common::*, *},
            Direct3D::*,
            Direct3D11::*,
            Dxgi::{Common::*, *},
            Imaging::*,
        },
        Media::MediaFoundation::*,
        System::Com::*,
    },
};

pub struct Graphics {
    pub device: ID3D11Device,
    pub context: ID2D1DeviceContext,
    pub manager: IMFDXGIDeviceManager,
    pub info: Value,
    pub hardware: bool,
}
impl Graphics {
    pub fn new(software: bool) -> Result<Self> {
        unsafe {
            let create = |driver| -> Result<ID3D11Device> {
                let mut d = None;
                let flags = if driver == D3D_DRIVER_TYPE_WARP {
                    D3D11_CREATE_DEVICE_BGRA_SUPPORT
                } else {
                    D3D11_CREATE_DEVICE_BGRA_SUPPORT | D3D11_CREATE_DEVICE_VIDEO_SUPPORT
                };
                D3D11CreateDevice(
                    None,
                    driver,
                    HMODULE::default(),
                    flags,
                    None,
                    D3D11_SDK_VERSION,
                    Some(&mut d),
                    None,
                    None,
                )?;
                Ok(d.unwrap())
            };
            let (device, hardware, fallback) = if !software {
                match create(D3D_DRIVER_TYPE_HARDWARE) {
                    Ok(d) => (d, true, None),
                    Err(e) => (create(D3D_DRIVER_TYPE_WARP)?, false, Some(e.to_string())),
                }
            } else {
                (create(D3D_DRIVER_TYPE_WARP)?, false, None)
            };
            let immediate = device.GetImmediateContext()?;
            if let Ok(multithread) = immediate.cast::<ID3D11Multithread>() {
                let _ = multithread.SetMultithreadProtected(true);
            }
            let dxgi: IDXGIDevice = device.cast()?;
            let adapter = dxgi.GetAdapter()?;
            let desc = adapter.GetDesc()?;
            let name = String::from_utf16_lossy(
                &desc.Description[..desc.Description.iter().position(|c| *c == 0).unwrap_or(128)],
            );
            let factory: ID2D1Factory1 =
                D2D1CreateFactory(D2D1_FACTORY_TYPE_SINGLE_THREADED, None)?;
            let d2d = factory.CreateDevice(&dxgi)?;
            let context = d2d.CreateDeviceContext(D2D1_DEVICE_CONTEXT_OPTIONS_NONE)?;
            let mut token = 0;
            let mut manager = None;
            MFCreateDXGIDeviceManager(&mut token, &mut manager)?;
            let manager = manager.unwrap();
            manager.ResetDevice(&device, token)?;
            Ok(Self {
                device,
                context,
                manager,
                hardware,
                info: json!({"renderer":if hardware{"direct2d_d3d11"}else{"direct2d_warp"},"hardware":hardware,"adapter":name,"fallback_reason":fallback}),
            })
        }
    }
    pub fn bitmap(&self, w: u32, h: u32, flags: D2D1_BITMAP_OPTIONS) -> Result<ID2D1Bitmap1> {
        unsafe {
            Ok(self.context.CreateBitmap(
                D2D_SIZE_U {
                    width: w,
                    height: h,
                },
                None,
                0,
                &D2D1_BITMAP_PROPERTIES1 {
                    pixelFormat: D2D1_PIXEL_FORMAT {
                        format: DXGI_FORMAT_B8G8R8A8_UNORM,
                        alphaMode: D2D1_ALPHA_MODE_IGNORE,
                    },
                    dpiX: 96.0,
                    dpiY: 96.0,
                    bitmapOptions: flags,
                    colorContext: Default::default(),
                },
            )?)
        }
    }
    pub fn readback(
        &self,
        source: &ID2D1Bitmap1,
        cpu: &ID2D1Bitmap1,
        result: &mut Vec<u8>,
    ) -> Result<()> {
        unsafe {
            cpu.CopyFromBitmap(None, source, None)?;
            let m = cpu.Map(D2D1_MAP_OPTIONS_READ)?;
            let size = source.GetPixelSize();
            result.resize((size.width * size.height * 4) as usize, 0);
            for y in 0..size.height as usize {
                std::ptr::copy_nonoverlapping(
                    m.bits.add(y * m.pitch as usize),
                    result.as_mut_ptr().add(y * size.width as usize * 4),
                    size.width as usize * 4,
                );
            }
            cpu.Unmap()?;
            Ok(())
        }
    }
}
pub fn save_png(path: &Path, w: u32, h: u32, data: &[u8]) -> Result<()> {
    unsafe {
        let factory: IWICImagingFactory =
            CoCreateInstance(&CLSID_WICImagingFactory, None, CLSCTX_INPROC_SERVER)?;
        let stream = factory.CreateStream()?;
        stream.InitializeFromFilename(&media::native_path(path), GENERIC_WRITE.0)?;
        let encoder = factory.CreateEncoder(&GUID_ContainerFormatPng, std::ptr::null())?;
        encoder.Initialize(&stream, WICBitmapEncoderNoCache)?;
        let mut frame = None;
        let mut props = None;
        encoder.CreateNewFrame(&mut frame, &mut props)?;
        let frame = frame.unwrap();
        frame.Initialize(props.as_ref())?;
        frame.SetSize(w, h)?;
        let mut format = GUID_WICPixelFormat32bppBGRA;
        frame.SetPixelFormat(&mut format)?;
        if format != GUID_WICPixelFormat32bppBGRA {
            return Err(Error::new(
                "image_failed",
                "PNG encoder rejected BGRA pixels",
            ));
        }
        frame.WritePixels(h, w * 4, data)?;
        frame.Commit()?;
        encoder.Commit()?;
        Ok(())
    }
}
