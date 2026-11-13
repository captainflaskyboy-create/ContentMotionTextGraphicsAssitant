//! A private DirectWrite font collection backed by static bytes, never installed or extracted.
use crate::error::{Error, Result};
use std::{
    ffi::c_void,
    sync::atomic::{AtomicBool, Ordering},
};
use windows::{
    core::{implement, Ref, BOOL},
    Win32::{Foundation::E_INVALIDARG, Graphics::DirectWrite::*},
};

const BYTES: &[u8] = include_bytes!("../assets/fonts/PressStart2P-Regular.ttf");
const KEY: u8 = 0;

#[implement(IDWriteFontFileStream)]
struct FontStream;
impl IDWriteFontFileStream_Impl for FontStream_Impl {
    fn ReadFileFragment(
        &self,
        start: *mut *mut c_void,
        offset: u64,
        size: u64,
        context: *mut *mut c_void,
    ) -> windows::core::Result<()> {
        if start.is_null()
            || context.is_null()
            || offset
                .checked_add(size)
                .is_none_or(|end| end > BYTES.len() as u64)
        {
            return Err(E_INVALIDARG.into());
        }
        // The checked fragment points into immutable static storage for the entire process lifetime.
        unsafe {
            *start = BYTES.as_ptr().add(offset as usize).cast_mut().cast();
            *context = std::ptr::null_mut();
        }
        Ok(())
    }
    fn ReleaseFileFragment(&self, _context: *mut c_void) {}
    fn GetFileSize(&self) -> windows::core::Result<u64> {
        Ok(BYTES.len() as u64)
    }
    fn GetLastWriteTime(&self) -> windows::core::Result<u64> {
        Ok(0)
    }
}
#[implement(IDWriteFontFileLoader)]
struct FileLoader;
impl IDWriteFontFileLoader_Impl for FileLoader_Impl {
    fn CreateStreamFromKey(
        &self,
        key: *const c_void,
        size: u32,
    ) -> windows::core::Result<IDWriteFontFileStream> {
        if size != 1 || key.is_null() || unsafe { *key.cast::<u8>() } != KEY {
            return Err(E_INVALIDARG.into());
        }
        Ok(FontStream.into())
    }
}
#[implement(IDWriteFontFileEnumerator)]
struct FontEnumerator {
    file: IDWriteFontFile,
    visited: AtomicBool,
}
impl IDWriteFontFileEnumerator_Impl for FontEnumerator_Impl {
    fn MoveNext(&self) -> windows::core::Result<BOOL> {
        Ok((!self.visited.swap(true, Ordering::Relaxed)).into())
    }
    fn GetCurrentFontFile(&self) -> windows::core::Result<IDWriteFontFile> {
        Ok(self.file.clone())
    }
}
#[implement(IDWriteFontCollectionLoader)]
struct CollectionLoader {
    file: IDWriteFontFile,
}
impl IDWriteFontCollectionLoader_Impl for CollectionLoader_Impl {
    fn CreateEnumeratorFromKey(
        &self,
        _factory: Ref<IDWriteFactory>,
        _key: *const c_void,
        _size: u32,
    ) -> windows::core::Result<IDWriteFontFileEnumerator> {
        Ok(FontEnumerator {
            file: self.file.clone(),
            visited: AtomicBool::new(false),
        }
        .into())
    }
}

pub struct PixelFont {
    pub face: IDWriteFontFace,
    factory: IDWriteFactory,
    _collection: IDWriteFontCollection,
    collection_loader: IDWriteFontCollectionLoader,
    file_loader: IDWriteFontFileLoader,
}
impl PixelFont {
    pub fn new() -> Result<Self> {
        unsafe {
            let factory: IDWriteFactory = DWriteCreateFactory(DWRITE_FACTORY_TYPE_ISOLATED)?;
            let file_loader: IDWriteFontFileLoader = FileLoader.into();
            factory.RegisterFontFileLoader(&file_loader)?;
            let create = (|| -> Result<Self> {
                let file = factory.CreateCustomFontFileReference(
                    (&KEY as *const u8).cast(),
                    1,
                    &file_loader,
                )?;
                let collection_loader: IDWriteFontCollectionLoader =
                    CollectionLoader { file }.into();
                factory.RegisterFontCollectionLoader(&collection_loader)?;
                let collection = match factory.CreateCustomFontCollection(
                    &collection_loader,
                    (&KEY as *const u8).cast(),
                    1,
                ) {
                    Ok(value) => value,
                    Err(e) => {
                        let _ = factory.UnregisterFontCollectionLoader(&collection_loader);
                        return Err(e.into());
                    }
                };
                let face = match collection
                    .GetFontFamily(0)
                    .and_then(|family| {
                        family.GetFirstMatchingFont(
                            DWRITE_FONT_WEIGHT_NORMAL,
                            DWRITE_FONT_STRETCH_NORMAL,
                            DWRITE_FONT_STYLE_NORMAL,
                        )
                    })
                    .and_then(|font| font.CreateFontFace())
                {
                    Ok(value) => value,
                    Err(e) => {
                        let _ = factory.UnregisterFontCollectionLoader(&collection_loader);
                        return Err(e.into());
                    }
                };
                Ok(Self {
                    face,
                    factory: factory.clone(),
                    _collection: collection,
                    collection_loader,
                    file_loader: file_loader.clone(),
                })
            })();
            if create.is_err() {
                let _ = factory.UnregisterFontFileLoader(&file_loader);
            }
            create
        }
    }
    pub fn glyphs(&self, text: &str, index: usize) -> Result<Vec<u16>> {
        let chars: Vec<u32> = text.chars().map(u32::from).collect();
        let mut glyphs = vec![0; chars.len()];
        unsafe {
            self.face
                .GetGlyphIndices(chars.as_ptr(), chars.len() as u32, glyphs.as_mut_ptr())?;
        }
        for (character, glyph) in chars.iter().zip(&glyphs) {
            if *glyph == 0 {
                return Err(Error::new("invalid_scene",format!("words[{index}].word: bundled Press Start 2P has no glyph for U+{character:04X}")));
            }
        }
        Ok(glyphs)
    }
}
impl Drop for PixelFont {
    fn drop(&mut self) {
        unsafe {
            let _ = self
                .factory
                .UnregisterFontCollectionLoader(&self.collection_loader);
            let _ = self.factory.UnregisterFontFileLoader(&self.file_loader);
        }
    }
}
