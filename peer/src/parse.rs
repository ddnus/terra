use crate::{error::Error, Frame};

use bytes::Bytes;
use std::{fmt, str, vec};

#[derive(Debug)]
pub struct Parse {
    /// Array frame iterator.
    parts: vec::IntoIter<Frame>,
}


impl Parse {
    /// Create a new `Parse` to parse the contents of `frame`.
    ///
    /// Returns `Err` if `frame` is not an array frame.
    pub fn new(frame: Frame) -> Result<Parse, Error> {
        let array = match frame {
            Frame::Array(array) => array,
            frame => return Err(Error::InvalidFrameType(format!("protocol error; expected array, got {:?}", frame))),
        };

        Ok(Parse {
            parts: array.into_iter(),
        })
    }

    /// Return the next entry. Array frames are arrays of frames, so the next
    /// entry is a frame.
    fn next(&mut self) -> Result<Frame, Error> {
        self.parts.next().ok_or(Error::EndOfStream)
    }

    /// Return the next entry as a string.
    ///
    /// If the next entry cannot be represented as a String, then an error is returned.
    pub fn next_string(&mut self) -> Result<String, Error> {
        match self.next()? {
            // Both `Simple` and `Bulk` representation may be strings. Strings
            // are parsed to UTF-8.
            //
            // While errors are stored as strings, they are considered separate
            // types.
            Frame::Simple(s) => Ok(s),
            Frame::Bulk(data) => str::from_utf8(&data[..])
                .map(|s| s.to_string())
                .map_err(|_| Error::InvalidFrameType("protocol error; invalid string".to_string())),
            frame => Err(Error::InvalidFrameType(format!(
                "protocol error; expected simple frame or bulk frame, got {:?}",
                frame
            ))),
        }
    }

    /// Return the next entry as raw bytes.
    ///
    /// If the next entry cannot be represented as raw bytes, an error is
    /// returned.
    pub fn next_bytes(&mut self) -> Result<Bytes, Error> {
        match self.next()? {
            // Both `Simple` and `Bulk` representation may be raw bytes.
            //
            // Although errors are stored as strings and could be represented as
            // raw bytes, they are considered separate types.
            Frame::Simple(s) => Ok(Bytes::from(s.into_bytes())),
            Frame::Bulk(data) => Ok(data),
            frame => Err(Error::InvalidFrameType(format!(
                "protocol error; expected simple frame or bulk frame, got {:?}",
                frame
            ))),
        }
    }

    /// Return the next entry as an integer.
    ///
    /// This includes `Simple`, `Bulk`, and `Integer` frame types. `Simple` and
    /// `Bulk` frame types are parsed.
    ///
    /// If the next entry cannot be represented as an integer, then an error is
    /// returned.
    pub fn next_int(&mut self) -> Result<u64, Error> {
        use atoi::atoi;

        const MSG: &str = "protocol error; invalid number";

        match self.next()? {
            // An integer frame type is already stored as an integer.
            Frame::Integer(v) => Ok(v),
            // Simple and bulk frames must be parsed as integers. If the parsing
            // fails, an error is returned.
            Frame::Simple(data) => atoi::<u64>(data.as_bytes()).ok_or_else(|| MSG.into()),
            Frame::Bulk(data) => atoi::<u64>(&data).ok_or_else(|| MSG.into()),
            frame => Err(Error::InvalidFrameType(format!("protocol error; expected int frame but got {:?}", frame))),
        }
    }

    /// Ensure there are no more entries in the array
    pub fn finish(&mut self) -> Result<(), Error> {
        if self.parts.next().is_none() {
            Ok(())
        } else {
            Err(Error::InvalidFrameType("protocol error; expected end of frame, but there was more".to_string()))
        }
    }
}
