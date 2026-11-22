use crate::error::{Result, SophonError};
use libc::{c_uchar, c_uint, c_void};
use std::io::{Read, Seek, SeekFrom, Write};
use std::ptr;

#[allow(non_camel_case_types)]
type hpatch_BOOL = c_uint;
#[allow(non_camel_case_types)]
type hpatch_StreamPos_t = u64;

#[repr(C)]
struct hpatch_TStreamInput {
    stream_import: *mut c_void,
    stream_size: hpatch_StreamPos_t,
    read: extern "C" fn(
        *const hpatch_TStreamInput,
        hpatch_StreamPos_t,
        *mut c_uchar,
        *mut c_uchar,
    ) -> hpatch_BOOL,
    private_reserved: *mut c_void,
}

#[repr(C)]
struct hpatch_TStreamOutput {
    stream_import: *mut c_void,
    stream_size: hpatch_StreamPos_t,
    read_writed: Option<
        extern "C" fn(
            *const hpatch_TStreamOutput,
            hpatch_StreamPos_t,
            *mut c_uchar,
            *mut c_uchar,
        ) -> hpatch_BOOL,
    >,
    write: extern "C" fn(
        *const hpatch_TStreamOutput,
        hpatch_StreamPos_t,
        *const c_uchar,
        *const c_uchar,
    ) -> hpatch_BOOL,
}

extern "C" {
    fn patch_stream(
        out_new_data: *const hpatch_TStreamOutput,
        old_data: *const hpatch_TStreamInput,
        serialized_diff: *const hpatch_TStreamInput,
    ) -> hpatch_BOOL;
}

pub struct Patcher;

impl Patcher {
    pub fn apply_patch<R, D, W>(old_data: &mut R, diff_data: &mut D, new_data: &mut W) -> Result<()>
    where
        R: Read + Seek,
        D: Read + Seek,
        W: Write + Seek,
    {
        // Get sizes
        let old_size = old_data.seek(SeekFrom::End(0))?;
        old_data.seek(SeekFrom::Start(0))?;

        let diff_size = diff_data.seek(SeekFrom::End(0))?;
        diff_data.seek(SeekFrom::Start(0))?;

        let old_stream = hpatch_TStreamInput {
            stream_import: old_data as *mut _ as *mut c_void,
            stream_size: old_size,
            read: input_read::<R>,
            private_reserved: ptr::null_mut(),
        };

        let diff_stream = hpatch_TStreamInput {
            stream_import: diff_data as *mut _ as *mut c_void,
            stream_size: diff_size,
            read: input_read::<D>,
            private_reserved: ptr::null_mut(),
        };

        let new_stream = hpatch_TStreamOutput {
            stream_import: new_data as *mut _ as *mut c_void,
            stream_size: u64::MAX,
            read_writed: None,
            write: output_write::<W>,
        };

        let result = unsafe { patch_stream(&new_stream, &old_stream, &diff_stream) };

        if result == 0 {
            return Err(SophonError::Protocol("hpatchz patch failed".to_string()));
        }

        Ok(())
    }

    /// Applies HDiff/KrDiff patches, preferring hpatchz in PATH or falling back to native patcher.
    /// Writes atomically to a .tmp file and renames over target_path on success.
    pub fn apply_file_patch(
        old_path: &std::path::Path,
        diff_path: &std::path::Path,
        target_path: &std::path::Path,
    ) -> Result<()> {
        let parent = target_path
            .parent()
            .unwrap_or_else(|| std::path::Path::new("."));
        let rand_suffix: u64 = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos() as u64;
        let tmp_path = parent.join(format!(
            ".{}_{}.tmp",
            target_path
                .file_name()
                .and_then(|s| s.to_str())
                .unwrap_or("patch"),
            rand_suffix
        ));

        // 1. Try hpatchz in PATH if available
        let mut cli_attempted = false;
        if let Ok(status) = std::process::Command::new("hpatchz")
            .arg("-f")
            .arg(old_path)
            .arg(diff_path)
            .arg(&tmp_path)
            .status()
        {
            cli_attempted = true;
            if status.success() {
                std::fs::rename(&tmp_path, target_path)?;
                return Ok(());
            }
        }

        // 2. Try native C patch_stream
        let mut old_file = std::fs::File::open(old_path).map_err(|e| {
            SophonError::Io(std::io::Error::new(
                e.kind(),
                format!("Failed to open old file {:?}: {}", old_path, e),
            ))
        })?;
        let mut diff_file = std::fs::File::open(diff_path).map_err(|e| {
            SophonError::Io(std::io::Error::new(
                e.kind(),
                format!("Failed to open diff file {:?}: {}", diff_path, e),
            ))
        })?;
        let mut tmp_file = std::fs::OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(true)
            .open(&tmp_path)?;

        let native_res = Self::apply_patch(&mut old_file, &mut diff_file, &mut tmp_file);

        if native_res.is_ok() {
            drop(tmp_file);
            std::fs::rename(&tmp_path, target_path)?;
            return Ok(());
        }

        // Clean up tmp file
        let _ = std::fs::remove_file(&tmp_path);

        if !cli_attempted {
            Err(SophonError::Protocol(
                "Neither native patcher nor hpatchz in PATH is available to apply patch"
                    .to_string(),
            ))
        } else {
            Err(SophonError::Protocol(format!(
                "Failed to apply patch: hpatchz CLI failed and native patcher returned {:?}",
                native_res.err()
            )))
        }
    }
}

extern "C" fn input_read<T: Read + Seek>(
    stream: *const hpatch_TStreamInput,
    read_from_pos: hpatch_StreamPos_t,
    out_data: *mut c_uchar,
    out_data_end: *mut c_uchar,
) -> hpatch_BOOL {
    unsafe {
        let reader = &mut *((*stream).stream_import as *mut T);
        let len = out_data_end as usize - out_data as usize;
        let buf = std::slice::from_raw_parts_mut(out_data, len);

        if reader.seek(SeekFrom::Start(read_from_pos)).is_err() {
            return 0;
        }
        if reader.read_exact(buf).is_err() {
            return 0;
        }
        1
    }
}

extern "C" fn output_write<T: Write>(
    stream: *const hpatch_TStreamOutput,
    _write_to_pos: hpatch_StreamPos_t,
    data: *const c_uchar,
    data_end: *const c_uchar,
) -> hpatch_BOOL {
    unsafe {
        let writer = &mut *((*stream).stream_import as *mut T);
        let len = data_end as usize - data as usize;
        let buf = std::slice::from_raw_parts(data, len);

        if writer.write_all(buf).is_err() {
            return 0;
        }
        1
    }
}
