//! Парсер Mach-O 64 (ARM64). Используется для проверки объектных файлов
//! и исполняемых файлов, а также валидации IPA.

use anyhow::{bail, Result};
use serde::{Deserialize, Serialize};

pub const MH_MAGIC_64: u32 = 0xfeedfacf;
pub const MH_CIGAM_64: u32 = 0xcffaedfe;

pub const CPU_TYPE_ARM64: u32 = 0x0100_000c;
pub const CPU_TYPE_X86_64: u32 = 0x0100_0007;

pub const MH_EXECUTE: u32 = 0x2;
pub const MH_DYLIB: u32 = 0x6;
pub const MH_OBJECT: u32 = 0x1;

pub const LC_SEGMENT_64: u32 = 0x19;
pub const LC_ID_DYLIB: u32 = 0xd;
pub const LC_LOAD_DYLIB: u32 = 0xc;
pub const LC_LOAD_WEAK_DYLIB: u32 = 0x8000_0018;
pub const LC_REEXPORT_DYLIB: u32 = 0x8000_001f;
pub const LC_LAZY_LOAD_DYLIB: u32 = 0x20;
pub const LC_LOAD_UPWARD_DYLIB: u32 = 0x8000_0023;
pub const LC_CODE_SIGNATURE: u32 = 0x1d;
pub const LC_BUILD_VERSION: u32 = 0x32;
pub const LC_UUID: u32 = 0x1b;
pub const LC_SYMTAB: u32 = 0x2;
pub const LC_MAIN: u32 = 0x8000_0028;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MachOInfo {
    pub is_64: bool,
    pub little_endian: bool,
    pub cpu_type: u32,
    pub cpu_subtype: u32,
    pub filetype: u32,
    pub ncmds: u32,
    pub sizeofcmds: u32,
    pub flags: u32,
    pub architecture: String,
    pub filetype_name: String,
    pub load_commands: Vec<LoadCommandInfo>,
    pub dylibs: Vec<String>,
    pub has_code_signature: bool,
    pub build_version: Option<BuildVersion>,
    pub uuid: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BuildVersion {
    pub platform: u32,
    pub minos: String,
    pub sdk: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LoadCommandInfo {
    pub cmd: u32,
    pub cmdsize: u32,
    pub name: String,
}

fn read_u32(data: &[u8], off: usize, le: bool) -> Result<u32> {
    let bytes: [u8; 4] = data
        .get(off..off + 4)
        .ok_or_else(|| anyhow::anyhow!("Mach-O обрезан по адресу {off}"))?
        .try_into()?;
    Ok(if le {
        u32::from_le_bytes(bytes)
    } else {
        u32::from_be_bytes(bytes)
    })
}

fn lc_name(cmd: u32) -> &'static str {
    match cmd {
        LC_SEGMENT_64 => "LC_SEGMENT_64",
        LC_ID_DYLIB => "LC_ID_DYLIB",
        LC_LOAD_DYLIB => "LC_LOAD_DYLIB",
        LC_LOAD_WEAK_DYLIB => "LC_LOAD_WEAK_DYLIB",
        LC_REEXPORT_DYLIB => "LC_REEXPORT_DYLIB",
        LC_LAZY_LOAD_DYLIB => "LC_LAZY_LOAD_DYLIB",
        LC_LOAD_UPWARD_DYLIB => "LC_LOAD_UPWARD_DYLIB",
        LC_CODE_SIGNATURE => "LC_CODE_SIGNATURE",
        LC_BUILD_VERSION => "LC_BUILD_VERSION",
        LC_UUID => "LC_UUID",
        LC_SYMTAB => "LC_SYMTAB",
        LC_MAIN => "LC_MAIN",
        0xb => "LC_DYSYMTAB",
        0xe => "LC_LOAD_DYLINKER",
        0x1c => "LC_RPATH",
        0x24 => "LC_VERSION_MIN_IPHONEOS",
        0x1e => "LC_FUNCTION_STARTS",
        0x29 => "LC_DATA_IN_CODE",
        0x80000022 => "LC_DYLD_INFO_ONLY",
        0x8000001c => "LC_DYLD_EXPORTS_TRIE",
        0x8000001e => "LC_DYLD_CHAINED_FIXUPS",
        _ => "UNKNOWN",
    }
}

fn decode_version(v: u32) -> String {
    format!("{}.{}.{}", v >> 16, (v >> 8) & 0xff, v & 0xff)
}

fn cstr_at(data: &[u8], start: usize) -> String {
    let end = data[start..]
        .iter()
        .position(|b| *b == 0)
        .map(|i| start + i)
        .unwrap_or(data.len());
    String::from_utf8_lossy(&data[start.min(data.len())..end]).into_owned()
}

pub fn parse_macho(data: &[u8]) -> Result<MachOInfo> {
    if data.len() < 32 {
        bail!("слишком короткий файл для Mach-O 64 ({} байт)", data.len());
    }
    let magic = read_u32(data, 0, true)?;
    let (is_64, little_endian) = match magic {
        MH_MAGIC_64 => (true, true),
        MH_CIGAM_64 => (true, false),
        0xfeedface | 0xcefaedfe => (false, magic == 0xfeedface),
        _ => bail!("не Mach-O: magic=0x{magic:08x}"),
    };
    if !is_64 {
        bail!("поддерживается только Mach-O 64-бит");
    }
    let cpu_type = read_u32(data, 4, little_endian)?;
    let cpu_subtype = read_u32(data, 8, little_endian)?;
    let filetype = read_u32(data, 12, little_endian)?;
    let ncmds = read_u32(data, 16, little_endian)?;
    let sizeofcmds = read_u32(data, 20, little_endian)?;
    let flags = read_u32(data, 24, little_endian)?;

    let architecture = match cpu_type {
        CPU_TYPE_ARM64 => "arm64".to_string(),
        CPU_TYPE_X86_64 => "x86_64".to_string(),
        other => format!("unknown(0x{other:x})"),
    };
    let filetype_name = match filetype {
        MH_OBJECT => "MH_OBJECT".into(),
        MH_EXECUTE => "MH_EXECUTE".into(),
        MH_DYLIB => "MH_DYLIB".into(),
        0x8 => "MH_BUNDLE".into(),
        _ => format!("MH_UNKNOWN({filetype})"),
    };

    let mut load_commands = Vec::new();
    let mut dylibs = Vec::new();
    let mut has_code_signature = false;
    let mut build_version = None;
    let mut uuid = None;
    let mut offset = 32usize;
    let cmds_end = offset
        .checked_add(sizeofcmds as usize)
        .filter(|end| *end <= data.len())
        .ok_or_else(|| anyhow::anyhow!("sizeofcmds выходит за границы файла"))?;

    for i in 0..ncmds as usize {
        if offset + 8 > cmds_end {
            bail!("load command {i} выходит за границы sizeofcmds");
        }
        let cmd = read_u32(data, offset, little_endian)?;
        let cmdsize = read_u32(data, offset + 4, little_endian)?;
        if cmdsize < 8 || offset + cmdsize as usize > cmds_end {
            bail!("некорректный cmdsize={cmdsize} в load command {i}");
        }

        let mut name = lc_name(cmd).to_string();
        if matches!(
            cmd,
            LC_LOAD_DYLIB
                | LC_ID_DYLIB
                | LC_LOAD_WEAK_DYLIB
                | LC_REEXPORT_DYLIB
                | LC_LAZY_LOAD_DYLIB
                | LC_LOAD_UPWARD_DYLIB
        ) {
            if offset + 24 <= cmds_end {
                let name_offset = read_u32(data, offset + 8, little_endian)? as usize;
                let str_start = offset + name_offset;
                if str_start < offset + cmdsize as usize && str_start < data.len() {
                    let s = cstr_at(data, str_start);
                    name = s.clone();
                    if cmd != LC_ID_DYLIB {
                        dylibs.push(s);
                    }
                }
            }
        } else if cmd == LC_CODE_SIGNATURE {
            has_code_signature = true;
        } else if cmd == LC_BUILD_VERSION && offset + 24 <= cmds_end {
            let platform = read_u32(data, offset + 8, little_endian)?;
            let minos = read_u32(data, offset + 12, little_endian)?;
            let sdk = read_u32(data, offset + 16, little_endian)?;
            build_version = Some(BuildVersion {
                platform,
                minos: decode_version(minos),
                sdk: decode_version(sdk),
            });
        } else if cmd == LC_UUID && offset + 24 <= cmds_end {
            uuid = Some(
                data[offset + 8..offset + 24]
                    .iter()
                    .map(|b| format!("{b:02x}"))
                    .collect(),
            );
        }

        load_commands.push(LoadCommandInfo { cmd, cmdsize, name });
        offset += cmdsize as usize;
    }

    Ok(MachOInfo {
        is_64,
        little_endian,
        cpu_type,
        cpu_subtype,
        filetype,
        ncmds,
        sizeofcmds,
        flags,
        architecture,
        filetype_name,
        load_commands,
        dylibs,
        has_code_signature,
        build_version,
        uuid,
    })
}

pub fn parse_macho_file(path: &std::path::Path) -> Result<MachOInfo> {
    let data = std::fs::read(path)
        .map_err(|e| anyhow::anyhow!("не удалось прочитать {}: {e}", path.display()))?;
    parse_macho(&data)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn minimal_header() -> Vec<u8> {
        let mut d = vec![0u8; 32];
        d[0..4].copy_from_slice(&MH_MAGIC_64.to_le_bytes());
        d[4..8].copy_from_slice(&CPU_TYPE_ARM64.to_le_bytes());
        d[12..16].copy_from_slice(&MH_EXECUTE.to_le_bytes());
        d
    }

    #[test]
    fn parses_header() {
        let info = parse_macho(&minimal_header()).unwrap();
        assert_eq!(info.architecture, "arm64");
        assert_eq!(info.filetype_name, "MH_EXECUTE");
        assert!(info.is_64 && info.little_endian);
    }

    #[test]
    fn rejects_non_macho() {
        assert!(parse_macho(b"not a macho file at all!!!").is_err());
    }
}
