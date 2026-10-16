//! Debug information for generated code, registered with debuggers
//! through the GDB JIT interface (also read by LLDB): DWARF line tables
//! from `Stmt::Loc` source positions, one subprogram per function with its
//! named locals (locations from Cranelift value labels: a register or a
//! frame offset per code range), base types by representation, and
//! `.debug_frame` unwind entries from Cranelift's unwind information. The
//! DWARF is wrapped in an in-memory ELF image whose `.text` section is
//! `NOBITS` at the generated code's address range, so every address in it
//! is the absolute runtime address.
//!
//! Enabled with `lang run --debug` (or `LANG_DEBUG_INFO=1`): then
//! `gdb --args lang run --debug prog.lang` can break at `prog.lang:12`,
//! step by source line, show backtraces through generated frames, and
//! print or watch locals.

use cranelift_codegen::isa::TargetIsa;
use cranelift_codegen::isa::unwind::UnwindInfo;
use gimli::write::{
    Address, AttributeValue, DwarfUnit, EndianVec, Expression, FrameTable, LineProgram, LineString, Range,
    RangeList, Sections,
};
use gimli::{Encoding, Format, LineEncoding, LittleEndian, Register};
use lang_mir::Repr;

#[derive(Debug, Clone)]
pub struct VarDebug {
    pub name: String,
    pub repr: Repr,
    pub param: bool,
    /// Where the value is kept, relative to the frame pointer (debug
    /// builds store every named local in a stack slot).
    pub fp_offset: i64,
}

/// DWARF number of the frame pointer register.
const FP: u16 = if cfg!(target_arch = "aarch64") { 29 } else { 6 };

pub struct FuncDebug {
    pub name: String,
    pub size: u32,
    /// (code offset, file index, line, column), sorted by offset.
    pub lines: Vec<(u32, u32, u32, u32)>,
    pub vars: Vec<VarDebug>,
    pub unwind: Option<UnwindInfo>,
}

// ------------------------------------------------------------ GDB JIT interface

#[repr(C)]
pub struct JitCodeEntry {
    next: *mut JitCodeEntry,
    prev: *mut JitCodeEntry,
    symfile_addr: *const u8,
    symfile_size: u64,
}

#[repr(C)]
pub struct JitDescriptor {
    version: u32,
    action_flag: u32,
    relevant_entry: *mut JitCodeEntry,
    first_entry: *mut JitCodeEntry,
}

const JIT_REGISTER_FN: u32 = 1;

/// The debugger sets a breakpoint here and reads the descriptor when hit.
#[unsafe(no_mangle)]
#[inline(never)]
pub extern "C" fn __jit_debug_register_code() {
    // keep the call (and this function) from being optimized away
    std::hint::black_box(());
}

#[unsafe(no_mangle)]
pub static mut __jit_debug_descriptor: JitDescriptor = JitDescriptor {
    version: 1,
    action_flag: 0,
    relevant_entry: std::ptr::null_mut(),
    first_entry: std::ptr::null_mut(),
};

static REGISTER: std::sync::Mutex<()> = std::sync::Mutex::new(());

/// Hand an object image to an attached debugger (it stays registered for
/// the life of the process, like the code it describes).
fn register_image(image: Vec<u8>) {
    let _g = REGISTER.lock().unwrap();
    let image = Box::leak(image.into_boxed_slice());
    unsafe {
        let d = &raw mut __jit_debug_descriptor;
        let entry = Box::into_raw(Box::new(JitCodeEntry {
            next: (*d).first_entry,
            prev: std::ptr::null_mut(),
            symfile_addr: image.as_ptr(),
            symfile_size: image.len() as u64,
        }));
        if !(*d).first_entry.is_null() {
            (*(*d).first_entry).prev = entry;
        }
        (*d).first_entry = entry;
        (*d).relevant_entry = entry;
        (*d).action_flag = JIT_REGISTER_FN;
        __jit_debug_register_code();
    }
}

// ------------------------------------------------------------ DWARF

fn base_type(repr: Repr) -> (&'static str, gimli::DwAte, u8) {
    match repr {
        Repr::I8 => ("i8", gimli::DW_ATE_signed, 1),
        Repr::I16 => ("i16", gimli::DW_ATE_signed, 2),
        Repr::I32 => ("i32", gimli::DW_ATE_signed, 4),
        Repr::I64 => ("i64", gimli::DW_ATE_signed, 8),
        Repr::F32 => ("f32", gimli::DW_ATE_float, 4),
        Repr::F64 => ("f64", gimli::DW_ATE_float, 8),
        Repr::Ptr => ("ref", gimli::DW_ATE_address, 8),
    }
}

/// Build the DWARF and ELF image for `funcs` (each with its code address)
/// and register it.
pub fn register(files: &[String], funcs: &[(FuncDebug, u64)], isa: &dyn TargetIsa) -> Result<(), String> {
    if funcs.is_empty() {
        return Ok(());
    }
    let image = build_image(files, funcs, isa).map_err(|e| format!("debug info: {e}"))?;
    if let Ok(dir) = std::env::var("LANG_DEBUG_IMAGE") {
        // for inspection with readelf/objdump
        let _ = std::fs::write(format!("{dir}/lang-jit-{}.elf", std::process::id()), &image);
    }
    register_image(image);
    Ok(())
}

fn build_image(
    files: &[String],
    funcs: &[(FuncDebug, u64)],
    isa: &dyn TargetIsa,
) -> gimli::write::Result<Vec<u8>> {
    let encoding = Encoding { format: Format::Dwarf32, version: 4, address_size: 8 };
    let mut dwarf = DwarfUnit::new(encoding);
    // line program: directories and files
    let first = files.first().cloned().unwrap_or_else(|| "<generated>".into());
    let dir_of =
        |p: &str| std::path::Path::new(p).parent().map(|d| d.display().to_string()).unwrap_or_default();
    let name_of = |p: &str| {
        std::path::Path::new(p).file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default()
    };
    let mut program = LineProgram::new(
        encoding,
        LineEncoding::default(),
        LineString::String(dir_of(&first).into_bytes()),
        None,
        LineString::String(name_of(&first).into_bytes()),
        None,
    );
    let file_ids: Vec<_> = files
        .iter()
        .map(|f| {
            let dir = program.add_directory(LineString::String(dir_of(f).into_bytes()));
            program.add_file(LineString::String(name_of(f).into_bytes()), dir, None)
        })
        .collect();
    let root = dwarf.unit.root();
    {
        let cu = dwarf.unit.get_mut(root);
        cu.set(gimli::DW_AT_producer, AttributeValue::String(b"lang".to_vec()));
        cu.set(gimli::DW_AT_language, AttributeValue::Language(gimli::DW_LANG_C99));
        cu.set(gimli::DW_AT_name, AttributeValue::String(first.clone().into_bytes()));
        cu.set(gimli::DW_AT_comp_dir, AttributeValue::String(dir_of(&first).into_bytes()));
        cu.set(gimli::DW_AT_low_pc, AttributeValue::Address(Address::Constant(0)));
    }
    // base types
    let mut types = std::collections::HashMap::new();
    for r in [Repr::I8, Repr::I16, Repr::I32, Repr::I64, Repr::F32, Repr::F64, Repr::Ptr] {
        let (name, enc, size) = base_type(r);
        let t = dwarf.unit.add(root, gimli::DW_TAG_base_type);
        let e = dwarf.unit.get_mut(t);
        e.set(gimli::DW_AT_name, AttributeValue::String(name.as_bytes().to_vec()));
        e.set(gimli::DW_AT_encoding, AttributeValue::Encoding(enc));
        e.set(gimli::DW_AT_byte_size, AttributeValue::Data1(size));
        types.insert(r, t);
    }
    let mut ranges = vec![];
    for (f, addr) in funcs {
        ranges.push(Range::StartLength { begin: Address::Constant(*addr), length: f.size as u64 });
        // line rows
        if !f.lines.is_empty() {
            program.begin_sequence(Some(Address::Constant(*addr)));
            for &(off, file, line, col) in &f.lines {
                let Some(fid) = file_ids.get(file as usize) else { continue };
                let row = program.row();
                row.address_offset = off as u64;
                row.file = *fid;
                row.line = line as u64;
                row.column = col as u64;
                row.is_statement = true;
                program.generate_row();
            }
            program.end_sequence(f.size as u64);
        }
        let sp = dwarf.unit.add(root, gimli::DW_TAG_subprogram);
        {
            let e = dwarf.unit.get_mut(sp);
            e.set(gimli::DW_AT_name, AttributeValue::String(f.name.clone().into_bytes()));
            e.set(gimli::DW_AT_low_pc, AttributeValue::Address(Address::Constant(*addr)));
            e.set(gimli::DW_AT_high_pc, AttributeValue::Udata(f.size as u64));
            let mut fb = Expression::new();
            fb.op(gimli::DW_OP_call_frame_cfa);
            e.set(gimli::DW_AT_frame_base, AttributeValue::Exprloc(fb));
            e.set(gimli::DW_AT_external, AttributeValue::Flag(true));
            if let Some(&(_, file, line, _)) = f.lines.first()
                && let Some(fid) = file_ids.get(file as usize)
            {
                e.set(gimli::DW_AT_decl_file, AttributeValue::FileIndex(Some(*fid)));
                e.set(gimli::DW_AT_decl_line, AttributeValue::Udata(line as u64));
            }
        }
        for v in &f.vars {
            let tag = if v.param { gimli::DW_TAG_formal_parameter } else { gimli::DW_TAG_variable };
            let var = dwarf.unit.add(sp, tag);
            let mut x = Expression::new();
            x.op_breg(Register(FP), v.fp_offset);
            let e = dwarf.unit.get_mut(var);
            e.set(gimli::DW_AT_name, AttributeValue::String(v.name.clone().into_bytes()));
            e.set(gimli::DW_AT_type, AttributeValue::UnitRef(types[&v.repr]));
            e.set(gimli::DW_AT_location, AttributeValue::Exprloc(x));
        }
    }
    let rl = dwarf.unit.ranges.add(RangeList(ranges));
    dwarf.unit.get_mut(root).set(gimli::DW_AT_ranges, AttributeValue::RangeListRef(rl));
    dwarf.unit.line_program = program;
    let mut sections = Sections::new(EndianVec::new(LittleEndian));
    dwarf.write(&mut sections)?;
    let mut out: Vec<(String, Vec<u8>)> = vec![];
    sections.for_each(|id, data| {
        if !data.slice().is_empty() {
            out.push((id.name().to_string(), data.slice().to_vec()));
        }
        Ok::<(), gimli::write::Error>(())
    })?;
    // unwind entries
    if let Some(cie) = isa.create_systemv_cie() {
        let mut table = FrameTable::default();
        let cie_id = table.add_cie(cie);
        for (f, addr) in funcs {
            if let Some(UnwindInfo::SystemV(u)) = &f.unwind {
                table.add_fde(cie_id, u.to_fde(Address::Constant(*addr)));
            }
        }
        let mut df = gimli::write::DebugFrame(EndianVec::new(LittleEndian));
        table.write_debug_frame(&mut df)?;
        out.push((".debug_frame".into(), df.0.into_vec()));
    }
    let lo = funcs.iter().map(|(_, a)| *a).min().unwrap_or(0);
    let hi = funcs.iter().map(|(f, a)| a + f.size as u64).max().unwrap_or(0);
    let syms: Vec<(&str, u64, u64)> =
        funcs.iter().map(|(f, a)| (f.name.as_str(), *a, f.size as u64)).collect();
    Ok(elf(lo, hi, &out, &syms))
}

// ------------------------------------------------------------ ELF

fn elf(lo: u64, hi: u64, sections: &[(String, Vec<u8>)], syms: &[(&str, u64, u64)]) -> Vec<u8> {
    const EHDR: usize = 64;
    const PHDR: usize = 56;
    const SHDR: usize = 64;
    let mut shstr = vec![0u8];
    let mut name = |n: &str| {
        let off = shstr.len() as u32;
        shstr.extend_from_slice(n.as_bytes());
        shstr.push(0);
        off
    };
    // section list: null, .text, debug sections, .symtab, .strtab, .shstrtab
    struct Sec {
        name: u32,
        kind: u32,
        flags: u64,
        addr: u64,
        data: Vec<u8>,
        size: u64,
        link: u32,
        info: u32,
        align: u64,
        entsize: u64,
    }
    let mut secs: Vec<Sec> = vec![Sec {
        name: 0,
        kind: 0,
        flags: 0,
        addr: 0,
        data: vec![],
        size: 0,
        link: 0,
        info: 0,
        align: 0,
        entsize: 0,
    }];
    let text_name = name(".text");
    secs.push(Sec {
        name: text_name,
        kind: 8,          // SHT_NOBITS
        flags: 0x2 | 0x4, // ALLOC | EXECINSTR
        addr: lo,
        data: vec![],
        size: hi - lo,
        link: 0,
        info: 0,
        align: 16,
        entsize: 0,
    });
    for (n, d) in sections {
        let nm = name(n);
        secs.push(Sec {
            name: nm,
            kind: 1, // SHT_PROGBITS
            flags: 0,
            addr: 0,
            size: d.len() as u64,
            data: d.clone(),
            link: 0,
            info: 0,
            align: 1,
            entsize: 0,
        });
    }
    // symbols
    let mut strtab = vec![0u8];
    let mut symtab = vec![0u8; 24];
    for (n, addr, size) in syms {
        let off = strtab.len() as u32;
        strtab.extend_from_slice(n.as_bytes());
        strtab.push(0);
        symtab.extend_from_slice(&off.to_le_bytes());
        symtab.push((1 << 4) | 2); // STB_GLOBAL, STT_FUNC
        symtab.push(0);
        symtab.extend_from_slice(&1u16.to_le_bytes()); // .text
        symtab.extend_from_slice(&addr.to_le_bytes());
        symtab.extend_from_slice(&size.to_le_bytes());
    }
    let symtab_idx = secs.len() as u32;
    let symtab_name = name(".symtab");
    let strtab_name = name(".strtab");
    let shstrtab_name = name(".shstrtab");
    secs.push(Sec {
        name: symtab_name,
        kind: 2,
        flags: 0,
        addr: 0,
        size: symtab.len() as u64,
        data: symtab,
        link: symtab_idx + 1,
        info: 1,
        align: 8,
        entsize: 24,
    });
    secs.push(Sec {
        name: strtab_name,
        kind: 3,
        flags: 0,
        addr: 0,
        size: strtab.len() as u64,
        data: strtab,
        link: 0,
        info: 0,
        align: 1,
        entsize: 0,
    });
    let shstrndx = secs.len() as u16;
    secs.push(Sec {
        name: shstrtab_name,
        kind: 3,
        flags: 0,
        addr: 0,
        size: shstr.len() as u64,
        data: shstr.clone(),
        link: 0,
        info: 0,
        align: 1,
        entsize: 0,
    });
    // layout: header, program header, section data, section headers
    let mut out = vec![0u8; EHDR + PHDR];
    let mut offsets = vec![];
    for s in &secs {
        while !out.len().is_multiple_of(8) {
            out.push(0);
        }
        offsets.push(out.len() as u64);
        out.extend_from_slice(&s.data);
    }
    while !out.len().is_multiple_of(8) {
        out.push(0);
    }
    let shoff = out.len() as u64;
    for (s, off) in secs.iter().zip(&offsets) {
        out.extend_from_slice(&s.name.to_le_bytes());
        out.extend_from_slice(&s.kind.to_le_bytes());
        out.extend_from_slice(&s.flags.to_le_bytes());
        out.extend_from_slice(&s.addr.to_le_bytes());
        out.extend_from_slice(&(if s.kind == 0 { 0 } else { *off }).to_le_bytes());
        out.extend_from_slice(&s.size.to_le_bytes());
        out.extend_from_slice(&s.link.to_le_bytes());
        out.extend_from_slice(&s.info.to_le_bytes());
        out.extend_from_slice(&s.align.to_le_bytes());
        out.extend_from_slice(&s.entsize.to_le_bytes());
    }
    // ELF header
    let h = &mut out[..EHDR];
    h[..4].copy_from_slice(b"\x7fELF");
    h[4] = 2; // 64-bit
    h[5] = 1; // little endian
    h[6] = 1; // version
    h[16..18].copy_from_slice(&2u16.to_le_bytes()); // ET_EXEC
    let machine: u16 = if cfg!(target_arch = "aarch64") { 183 } else { 62 };
    h[18..20].copy_from_slice(&machine.to_le_bytes());
    h[20..24].copy_from_slice(&1u32.to_le_bytes());
    h[24..32].copy_from_slice(&lo.to_le_bytes()); // entry
    h[32..40].copy_from_slice(&(EHDR as u64).to_le_bytes()); // phoff
    h[40..48].copy_from_slice(&shoff.to_le_bytes());
    h[52..54].copy_from_slice(&(EHDR as u16).to_le_bytes());
    h[54..56].copy_from_slice(&(PHDR as u16).to_le_bytes());
    h[56..58].copy_from_slice(&1u16.to_le_bytes());
    h[58..60].copy_from_slice(&(SHDR as u16).to_le_bytes());
    h[60..62].copy_from_slice(&(secs.len() as u16).to_le_bytes());
    h[62..64].copy_from_slice(&shstrndx.to_le_bytes());
    // one PT_LOAD covering the code (no file bytes)
    let p = &mut out[EHDR..EHDR + PHDR];
    p[..4].copy_from_slice(&1u32.to_le_bytes());
    p[4..8].copy_from_slice(&5u32.to_le_bytes()); // R | X
    p[16..24].copy_from_slice(&lo.to_le_bytes());
    p[24..32].copy_from_slice(&lo.to_le_bytes());
    p[40..48].copy_from_slice(&(hi - lo).to_le_bytes());
    p[48..56].copy_from_slice(&0x1000u64.to_le_bytes());
    out
}
