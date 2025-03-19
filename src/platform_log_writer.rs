use crate::arrays::slice_assume_init_ref;
use crate::{LOGGING_MSG_MAX_LEN, LogId, android_log, uninit_array};
use log::Level;
#[cfg(target_os = "android")]
use log_ffi::LogPriority;
use std::ffi::CStr;
use std::mem::MaybeUninit;
use std::{fmt, mem, ptr};

/// The purpose of this "writer" is to split logged messages on whitespace when the log message
/// length exceeds the maximum. Without allocations.
pub struct PlatformLogWriter<'a> {
    #[cfg(target_os = "android")]
    priority: LogPriority,
    #[cfg(not(target_os = "android"))]
    priority: Level,
    #[cfg(target_os = "android")]
    buf_id: Option<log_ffi::log_id_t>,
    #[cfg(not(target_os = "android"))]
    buf_id: Option<LogId>,
    len: usize,
    last_newline_index: usize,
    tag: &'a CStr,
    buffer: [MaybeUninit<u8>; LOGGING_MSG_MAX_LEN + 1],
}

impl PlatformLogWriter<'_> {
    #[cfg(target_os = "android")]
    pub fn new_with_priority(
        buf_id: Option<LogId>,
        priority: log_ffi::LogPriority,
        tag: &CStr,
    ) -> PlatformLogWriter<'_> {
        #[allow(deprecated)] // created an issue #35 for this
        PlatformLogWriter {
            priority,
            buf_id: LogId::to_native(buf_id),
            len: 0,
            last_newline_index: 0,
            tag,
            buffer: uninit_array(),
        }
    }

    #[cfg(target_os = "android")]
    pub fn new(buf_id: Option<LogId>, level: Level, tag: &CStr) -> PlatformLogWriter<'_> {
        PlatformLogWriter::new_with_priority(
            buf_id,
            match level {
                Level::Warn => LogPriority::WARN,
                Level::Info => LogPriority::INFO,
                Level::Debug => LogPriority::DEBUG,
                Level::Error => LogPriority::ERROR,
                Level::Trace => LogPriority::VERBOSE,
            },
            tag,
        )
    }

    #[cfg(not(target_os = "android"))]
    pub fn new(buf_id: Option<LogId>, level: Level, tag: &CStr) -> PlatformLogWriter<'_> {
        #[allow(deprecated)] // created an issue #35 for this
        PlatformLogWriter {
            priority: level,
            buf_id,
            len: 0,
            last_newline_index: 0,
            tag,
            buffer: uninit_array(),
        }
    }

    /// Flush some bytes to android logger.
    ///
    /// If there is a newline, flush up to it.
    /// If there was no newline, flush all.
    ///
    /// Not guaranteed to flush everything.
    fn temporal_flush(&mut self) {
        let total_len = self.len;

        if total_len == 0 {
            return;
        }

        if self.last_newline_index > 0 {
            let copy_from_index = self.last_newline_index;
            let remaining_chunk_len = total_len - copy_from_index;

            unsafe { self.output_specified_len(copy_from_index) };
            self.copy_bytes_to_start(copy_from_index, remaining_chunk_len);
            self.len = remaining_chunk_len;
        } else {
            unsafe { self.output_specified_len(total_len) };
            self.len = 0;
        }
        self.last_newline_index = 0;
    }

    /// Flush everything remaining to android logger.
    pub fn flush(&mut self) {
        let total_len = self.len;

        if total_len == 0 {
            return;
        }

        unsafe { self.output_specified_len(total_len) };
        self.len = 0;
        self.last_newline_index = 0;
    }

    /// Output buffer up until the \0 which will be placed at `len` position.
    ///
    /// # Safety
    /// The first `len` bytes of `self.buffer` must be initialized and not contain nullbytes.
    unsafe fn output_specified_len(&mut self, len: usize) {
        let mut last_byte = MaybeUninit::new(b'\0');

        mem::swap(
            &mut last_byte,
            self.buffer.get_mut(len).expect("`len` is out of bounds"),
        );

        let initialized = unsafe { slice_assume_init_ref(&self.buffer[..len + 1]) };
        let msg = CStr::from_bytes_with_nul(initialized)
            .expect("Unreachable: nul terminator was placed at `len`");
        android_log(self.buf_id, self.priority, self.tag, msg);

        unsafe { *self.buffer.get_unchecked_mut(len) = last_byte };
    }

    /// Copy `len` bytes from `index` position to starting position.
    fn copy_bytes_to_start(&mut self, index: usize, len: usize) {
        let dst = self.buffer.as_mut_ptr();
        let src = unsafe { self.buffer.as_ptr().add(index) };
        unsafe { ptr::copy(src, dst, len) };
    }
}

impl fmt::Write for PlatformLogWriter<'_> {
    fn write_str(&mut self, s: &str) -> fmt::Result {
        let mut incoming_bytes = s.as_bytes();

        while !incoming_bytes.is_empty() {
            let len = self.len;

            // write everything possible to buffer and mark last \n
            let new_len = len + incoming_bytes.len();
            let last_newline = self.buffer[len..LOGGING_MSG_MAX_LEN]
                .iter_mut()
                .zip(incoming_bytes)
                .enumerate()
                .fold(None, |acc, (i, (output, input))| {
                    if *input == b'\0' {
                        // Replace nullbytes with whitespace, so we can put the message in a CStr
                        // later to pass it through a const char*.
                        output.write(b' ');
                    } else {
                        output.write(*input);
                    }
                    if *input == b'\n' { Some(i) } else { acc }
                });

            // update last \n index
            if let Some(newline) = last_newline {
                self.last_newline_index = len + newline;
            }

            // calculate how many bytes were written
            let written_len = if new_len <= LOGGING_MSG_MAX_LEN {
                // if the len was not exceeded
                self.len = new_len;
                new_len - len // written len
            } else {
                // if new length was exceeded
                self.len = LOGGING_MSG_MAX_LEN;
                self.temporal_flush();

                LOGGING_MSG_MAX_LEN - len // written len
            };

            incoming_bytes = &incoming_bytes[written_len..];
        }

        Ok(())
    }
}

#[cfg(test)]
pub mod tests {
    use crate::arrays::slice_assume_init_ref;
    use crate::platform_log_writer::PlatformLogWriter;
    use log::Level;
    use std::ffi::CStr;
    use std::fmt::Write;

    #[test]
    fn platform_log_writer_init_values() {
        let tag = CStr::from_bytes_with_nul(b"tag\0").unwrap();

        let writer = PlatformLogWriter::new(None, Level::Warn, tag);

        assert_eq!(writer.tag, tag);
        // Android uses LogPriority instead, which doesn't implement equality checks
        #[cfg(not(target_os = "android"))]
        assert_eq!(writer.priority, Level::Warn);
    }

    #[test]
    fn temporal_flush() {
        let mut writer = get_tag_writer();

        writer
            .write_str("12\n\n567\n90")
            .expect("Unable to write to PlatformLogWriter");

        assert_eq!(writer.len, 10);
        writer.temporal_flush();
        // Should have flushed up until the last newline.
        assert_eq!(writer.len, 3);
        assert_eq!(writer.last_newline_index, 0);
        assert_eq!(
            unsafe { slice_assume_init_ref(&writer.buffer[..writer.len]) },
            "\n90".as_bytes()
        );

        writer.temporal_flush();
        // Should have flushed all remaining bytes.
        assert_eq!(writer.len, 0);
        assert_eq!(writer.last_newline_index, 0);
    }

    #[test]
    fn flush() {
        let mut writer = get_tag_writer();
        writer
            .write_str("abcdefghij\n\nklm\nnopqr\nstuvwxyz")
            .expect("Unable to write to PlatformLogWriter");

        writer.flush();

        assert_eq!(writer.last_newline_index, 0);
        assert_eq!(writer.len, 0);
    }

    #[test]
    fn last_newline_index() {
        let mut writer = get_tag_writer();

        writer
            .write_str("12\n\n567\n90")
            .expect("Unable to write to PlatformLogWriter");

        assert_eq!(writer.last_newline_index, 7);
    }

    #[test]
    fn output_specified_len_leaves_buffer_unchanged() {
        let mut writer = get_tag_writer();
        let log_string = "abcdefghij\n\nklm\nnopqr\nstuvwxyz";
        writer
            .write_str(log_string)
            .expect("Unable to write to PlatformLogWriter");

        unsafe { writer.output_specified_len(5) };

        assert_eq!(
            unsafe { slice_assume_init_ref(&writer.buffer[..log_string.len()]) },
            log_string.as_bytes()
        );
    }

    #[test]
    fn copy_bytes_to_start() {
        let mut writer = get_tag_writer();
        writer
            .write_str("0123456789")
            .expect("Unable to write to PlatformLogWriter");

        writer.copy_bytes_to_start(3, 2);

        assert_eq!(
            unsafe { slice_assume_init_ref(&writer.buffer[..10]) },
            "3423456789".as_bytes()
        );
    }

    #[test]
    fn copy_bytes_to_start_nop() {
        let test_string = "Test_string_with\n\n\n\nnewlines\n";
        let mut writer = get_tag_writer();
        writer
            .write_str(test_string)
            .expect("Unable to write to PlatformLogWriter");

        writer.copy_bytes_to_start(0, 20);
        writer.copy_bytes_to_start(10, 0);

        assert_eq!(
            unsafe { slice_assume_init_ref(&writer.buffer[..test_string.len()]) },
            test_string.as_bytes()
        );
    }

    #[test]
    fn writer_substitutes_nullbytes_with_spaces() {
        let test_string = "Test_string_with\0\0\0\0nullbytes\0";
        let mut writer = get_tag_writer();
        writer
            .write_str(test_string)
            .expect("Unable to write to PlatformLogWriter");

        assert_eq!(
            unsafe { slice_assume_init_ref(&writer.buffer[..test_string.len()]) },
            test_string.replace("\0", " ").as_bytes()
        );
    }

    fn get_tag_writer() -> PlatformLogWriter<'static> {
        PlatformLogWriter::new(
            None,
            Level::Warn,
            CStr::from_bytes_with_nul(b"tag\0").unwrap(),
        )
    }
}
