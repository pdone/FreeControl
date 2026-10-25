//! Windows 的 DirectWrite 字体枚举（raw COM vtable）。
//!
//! 不把 `windows` crate 的 DirectWrite 绑定编进产物（实测会多出约 0.8 MiB）；
//! 这里只用 `windows-sys` 已有的 LoadLibrary / GetProcAddress / COM 初始化，按
//! Magpie `fonts_windows.go` 的方法序号直接调用 dwrite.dll 的 COM 接口。
//!
//! 方法序号以 Windows SDK 的 dwrite.h 为准：
//! - IDWriteFactory 3 = GetSystemFontCollection；
//! - IDWriteFontCollection 3 = GetFontFamilyCount、4 = GetFontFamily；
//! - IDWriteFontList 4 = GetFontCount、5 = GetFont；IDWriteFontFamily 6 = GetFamilyNames；
//! - IDWriteFont 10 = GetSimulations、13 = CreateFontFace；
//! - IDWriteFontFace 10 = GetDesignGlyphMetrics、11 = GetGlyphIndices；
//! - IDWriteLocalizedStrings 3 = GetCount、5/6 = GetLocaleNameLength/Name、7/8 = GetStringLength/String。

use std::ffi::c_void;
use windows_sys::core::GUID;
use windows_sys::Win32::System::Com::{
    CoInitializeEx, CoUninitialize, COINIT_APARTMENTTHREADED,
};
use windows_sys::Win32::System::LibraryLoader::{GetProcAddress, LoadLibraryW};

use super::{collect_fonts, dedupe_names, SystemFont};

const IID_IDWRITE_FACTORY: GUID = GUID {
    data1: 0xb859_ee5a,
    data2: 0xd838,
    data3: 0x4b5b,
    data4: [0xa2, 0xe8, 0x1a, 0xdc, 0x7d, 0x93, 0xdb, 0x48],
};
const DWRITE_FACTORY_TYPE_SHARED: u32 = 0;
const DWRITE_FONT_SIMULATIONS_NONE: i32 = 0;

/// `DWRITE_GLYPH_METRICS` 的前缀布局；只需要 advanceWidth。
#[repr(C)]
#[derive(Clone, Copy, Default)]
struct GlyphMetrics {
    left_side_bearing: i32,
    advance_width: u32,
    right_side_bearing: i32,
    top_side_bearing: i32,
    advance_height: u32,
    bottom_side_bearing: i32,
    vertical_origin_y: i32,
}

/// COM 接口指针的所有权包装：离开作用域时调用 Release。
struct Com(*mut c_void);

impl Drop for Com {
    fn drop(&mut self) {
        if !self.0.is_null() {
            unsafe { release(self.0) };
        }
    }
}

unsafe fn vtable(p: *mut c_void) -> *const usize {
    *(p as *const *const usize)
}

unsafe fn release(p: *mut c_void) {
    let f: extern "system" fn(*mut c_void) -> u32 =
        std::mem::transmute(*vtable(p).add(2));
    f(p);
}

unsafe fn call_u32(p: *mut c_void, index: usize) -> u32 {
    let f: extern "system" fn(*mut c_void) -> u32 =
        std::mem::transmute(*vtable(p).add(index));
    f(p)
}

unsafe fn call_i32(p: *mut c_void, index: usize) -> i32 {
    let f: extern "system" fn(*mut c_void) -> i32 =
        std::mem::transmute(*vtable(p).add(index));
    f(p)
}

unsafe fn call_hr_ptr(p: *mut c_void, index: usize, out: *mut *mut c_void) -> i32 {
    let f: extern "system" fn(*mut c_void, *mut *mut c_void) -> i32 =
        std::mem::transmute(*vtable(p).add(index));
    f(p, out)
}

unsafe fn call_hr_u32_ptr(
    p: *mut c_void,
    index: usize,
    arg: u32,
    out: *mut *mut c_void,
) -> i32 {
    let f: extern "system" fn(*mut c_void, u32, *mut *mut c_void) -> i32 =
        std::mem::transmute(*vtable(p).add(index));
    f(p, arg, out)
}

unsafe fn call_hr_len(p: *mut c_void, index: usize, arg: u32, out: *mut u32) -> i32 {
    let f: extern "system" fn(*mut c_void, u32, *mut u32) -> i32 =
        std::mem::transmute(*vtable(p).add(index));
    f(p, arg, out)
}

unsafe fn call_hr_text(
    p: *mut c_void,
    index: usize,
    arg: u32,
    buf: *mut u16,
    len: u32,
) -> i32 {
    let f: extern "system" fn(*mut c_void, u32, *mut u16, u32) -> i32 =
        std::mem::transmute(*vtable(p).add(index));
    f(p, arg, buf, len)
}

unsafe fn call_glyph_indices(
    p: *mut c_void,
    index: usize,
    codepoints: *const u32,
    count: u32,
    out: *mut u16,
) -> i32 {
    let f: extern "system" fn(*mut c_void, *const u32, u32, *mut u16) -> i32 =
        std::mem::transmute(*vtable(p).add(index));
    f(p, codepoints, count, out)
}

unsafe fn call_design_metrics(
    p: *mut c_void,
    index: usize,
    glyphs: *const u16,
    count: u32,
    out: *mut GlyphMetrics,
    sideways: i32,
) -> i32 {
    let f: extern "system" fn(
        *mut c_void,
        *const u16,
        u32,
        *mut GlyphMetrics,
        i32,
    ) -> i32 = std::mem::transmute(*vtable(p).add(index));
    f(p, glyphs, count, out, sideways)
}

/// 读一个本地化文本（长度索引 / 文本索引由调用方给出）。
unsafe fn read_text(
    p: *mut c_void,
    len_index: usize,
    text_index: usize,
    index: u32,
) -> Option<String> {
    let mut len = 0u32;
    if call_hr_len(p, len_index, index, &mut len) < 0 {
        return None;
    }
    let mut buf = vec![0u16; len as usize + 1];
    if call_hr_text(p, text_index, index, buf.as_mut_ptr(), buf.len() as u32) < 0 {
        return None;
    }
    let text = String::from_utf16_lossy(&buf[..len as usize]);
    (!text.is_empty()).then_some(text)
}

/// 取一个族名对象里的全部族名；en-US 有则排到最前作为 canonical。
unsafe fn family_aliases(names: *mut c_void) -> Vec<String> {
    let count = call_u32(names, 3);
    let mut all = Vec::with_capacity(count as usize);
    let mut english: Option<String> = None;
    for index in 0..count {
        let Some(name) = read_text(names, 7, 8, index) else {
            continue;
        };
        if english.is_none()
            && read_text(names, 5, 6, index)
                .is_some_and(|locale| locale.eq_ignore_ascii_case("en-us"))
        {
            english = Some(name.clone());
        }
        all.push(name);
    }
    if let Some(name) = english {
        all.retain(|candidate| !candidate.eq_ignore_ascii_case(&name));
        all.insert(0, name);
    }
    dedupe_names(all)
}

/// 等宽判断：取该族第一个字体，比较 `i` / `W` 的设计 advance width。
///
/// DirectWrite 没有直接的「等宽」标记；这是 best-effort 启发式，失败时只影响 UI 上的
/// 「等宽」标记，不影响字体选择与回退。
unsafe fn family_is_monospaced(family: *mut c_void) -> bool {
    let mut font = Com(std::ptr::null_mut());
    if call_hr_u32_ptr(family, 5, 0, &mut font.0) < 0 || font.0.is_null() {
        return false;
    }
    if call_i32(font.0, 10) != DWRITE_FONT_SIMULATIONS_NONE {
        return false;
    }
    let mut face = Com(std::ptr::null_mut());
    if call_hr_ptr(font.0, 13, &mut face.0) < 0 || face.0.is_null() {
        return false;
    }
    let codepoints = [u32::from(b'i'), u32::from(b'W')];
    let mut glyphs = [0u16; 2];
    if call_glyph_indices(face.0, 11, codepoints.as_ptr(), 2, glyphs.as_mut_ptr()) < 0
        || glyphs[0] == 0
        || glyphs[1] == 0
    {
        return false;
    }
    let mut metrics = [GlyphMetrics::default(); 2];
    if call_design_metrics(face.0, 10, glyphs.as_ptr(), 2, metrics.as_mut_ptr(), 0) < 0 {
        return false;
    }
    metrics[0].advance_width == metrics[1].advance_width
}

type CreateFactory = unsafe extern "system" fn(u32, *const GUID, *mut *mut c_void) -> i32;

fn load_create_factory() -> Result<CreateFactory, String> {
    let mut path: Vec<u16> = "dwrite.dll".encode_utf16().collect();
    path.push(0);
    unsafe {
        let module = LoadLibraryW(path.as_ptr());
        if module.is_null() {
            return Err("无法加载 dwrite.dll".to_string());
        }
        let proc = GetProcAddress(module, b"DWriteCreateFactory\0".as_ptr());
        let Some(proc) = proc else {
            return Err("dwrite.dll 缺少 DWriteCreateFactory".to_string());
        };
        // FARPROC 与目标函数指针同为指针宽度；签名由 SDK 的 dwrite.h 保证。
        Ok(std::mem::transmute(proc))
    }
}

pub(super) fn scan() -> Result<Vec<SystemFont>, String> {
    let create = load_create_factory()?;

    unsafe {
        // DirectWrite 的共享 factory 通常不要求 STA，但接口调用仍按 COM 处理；
        // 线程可能已有不同的 COM 模型（RPC_E_CHANGED_MODE），此时不重复反初始化。
        let hr = CoInitializeEx(std::ptr::null(), COINIT_APARTMENTTHREADED as u32);
        let com_initialized = hr >= 0;
        let result = (|| -> Result<Vec<SystemFont>, String> {
            let mut factory = Com(std::ptr::null_mut());
            let hr = create(
                DWRITE_FACTORY_TYPE_SHARED,
                &IID_IDWRITE_FACTORY,
                &mut factory.0,
            );
            if hr < 0 {
                return Err(format!("DirectWrite factory: HRESULT {hr:#x}"));
            }
            if factory.0.is_null() {
                return Err("DirectWrite 未返回 factory".to_string());
            }

            let mut collection = Com(std::ptr::null_mut());
            let f: extern "system" fn(*mut c_void, *mut *mut c_void, i32) -> i32 =
                std::mem::transmute(*vtable(factory.0).add(3));
            let hr = f(factory.0, &mut collection.0, 1);
            if hr < 0 {
                return Err(format!("DirectWrite fonts: HRESULT {hr:#x}"));
            }
            if collection.0.is_null() {
                return Err("DirectWrite 未返回字体集合".to_string());
            }

            let count = call_u32(collection.0, 3);
            let mut faces = Vec::new();
            for index in 0..count {
                let mut family = Com(std::ptr::null_mut());
                if call_hr_u32_ptr(collection.0, 4, index, &mut family.0) < 0
                    || family.0.is_null()
                {
                    continue;
                }
                let mut names = Com(std::ptr::null_mut());
                if call_hr_ptr(family.0, 6, &mut names.0) < 0 || names.0.is_null() {
                    continue;
                }
                let aliases = family_aliases(names.0);
                if aliases.is_empty() {
                    continue;
                }
                faces.push((aliases, family_is_monospaced(family.0)));
            }
            Ok(collect_fonts(faces))
        })();
        if com_initialized {
            CoUninitialize();
        }
        result
    }
}
