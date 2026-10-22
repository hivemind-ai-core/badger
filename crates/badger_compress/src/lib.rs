//! `badger_compress`: gzip, zlib, raw deflate, and zstd for `stdx.compress`.
//! Each call runs on the blocking pool (the calling task parks). A
//! decompression stops with `#compress/too_large` once its output passes
//! the caller's limit, so a small hostile input cannot exhaust memory.

use std::io::{Read, Write};

use badger_native::{Args, Error, Handler, ModuleDef, Outcome, Value};

#[derive(Clone, Copy)]
enum Format {
    Gzip,
    Zlib,
    Deflate,
    Zstd,
}

fn format(name: &str) -> Result<Format, Error> {
    Ok(match name {
        "gzip" => Format::Gzip,
        "zlib" => Format::Zlib,
        "deflate" => Format::Deflate,
        "zstd" => Format::Zstd,
        other => return Err(Error::new("compress/format", format!("unknown format `{other}`"))),
    })
}

fn io(e: std::io::Error) -> Error {
    Error::new("compress/corrupt", e.to_string())
}

fn compress(a: &Args) -> Outcome {
    let (f, data, level) = (format(a.str(0)?)?, a.bytes(1)?, a.int(2)?);
    match f {
        Format::Zstd => Ok(Value::Bytes(zstd::bulk::compress(data, level.clamp(-7, 22) as i32).map_err(io)?)),
        _ => compress_flate(f, data, flate2::Compression::new(level.clamp(0, 9) as u32)),
    }
}

fn compress_flate(f: Format, data: &[u8], level: flate2::Compression) -> Outcome {
    let out = match f {
        Format::Gzip => {
            let mut e = flate2::write::GzEncoder::new(Vec::new(), level);
            e.write_all(data).map_err(io)?;
            e.finish().map_err(io)?
        }
        Format::Zlib => {
            let mut e = flate2::write::ZlibEncoder::new(Vec::new(), level);
            e.write_all(data).map_err(io)?;
            e.finish().map_err(io)?
        }
        _ => {
            let mut e = flate2::write::DeflateEncoder::new(Vec::new(), level);
            e.write_all(data).map_err(io)?;
            e.finish().map_err(io)?
        }
    };
    Ok(Value::Bytes(out))
}

fn decompress(a: &Args) -> Outcome {
    let (f, data, limit) = (format(a.str(0)?)?, a.bytes(1)?, a.int(2)?.max(0) as u64);
    let reader: Box<dyn Read + '_> = match f {
        // concatenated gzip members are one stream
        Format::Gzip => Box::new(flate2::read::MultiGzDecoder::new(data)),
        Format::Zlib => Box::new(flate2::read::ZlibDecoder::new(data)),
        Format::Deflate => Box::new(flate2::read::DeflateDecoder::new(data)),
        Format::Zstd => Box::new(zstd::stream::read::Decoder::new(data).map_err(io)?),
    };
    let mut out = Vec::new();
    // one byte past the limit tells "exactly the limit" from "more"
    reader.take(limit + 1).read_to_end(&mut out).map_err(io)?;
    if out.len() as u64 > limit {
        return Err(Error::new(
            "compress/too_large",
            format!("the data decompresses to more than {limit} bytes"),
        ));
    }
    Ok(Value::Bytes(out))
}

pub static DEF: ModuleDef = ModuleDef {
    name: "badger_compress",
    functions: &[("compress", Handler::Blocking(compress)), ("decompress", Handler::Blocking(decompress))],
    drop_handle: None,
    cancel: None,
    recorded: &[],
    pure: &["compress", "decompress"],
};

badger_native::export!("badger_compress", DEF);
