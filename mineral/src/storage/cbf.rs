use std::{collections::BTreeMap, time::SystemTime};

use crate::error::Error;

type PageNo = u64;
type PageCap = u64;
type EntryPos = u64;
type VersionNo = u64;

type Entry = Vec<u8>;

#[derive(Debug, Clone)]
struct Page {
    page_no: PageNo,
    cap: PageCap,
    entrys: BTreeMap<EntryPos, Entry>,
}

impl Page {
    fn new(page_no: PageNo) -> Self {
        Page {
            page_no,
            cap: 0,
            entrys: BTreeMap::new(),
        }
    }

    fn append(&mut self, entry_pos: EntryPos, entry: Entry) {
        self.entrys.insert(entry_pos, entry);
    }
}

pub struct Cbf {
    version: VersionNo,
    pages: BTreeMap<PageNo, Page>,
    page_max_cap: PageCap,
    active_page: Page,

    rotation_live_time: u64,
    rotation_time: SystemTime,
}

impl Cbf {
    fn new(page_max_cap: PageCap) -> Self {
        Cbf {
            version: 0,
            pages: BTreeMap::new(),
            page_max_cap,
            active_page: Page::new(0),

            rotation_live_time: 300,   // 5min
            rotation_time: SystemTime::now(),
        }
    }

    fn append(&mut self, version: VersionNo, entry_pos: EntryPos, buf: Vec<u8>) -> Result<(), Error> {
        self.version = version;

        self.rotation_page(buf.len());

        self.active_page.append(entry_pos, buf);

        Ok(())
    }

    fn rotation_page(&mut self, buf_len: usize) {

        if self.active_page.cap + buf_len as u64 > self.page_max_cap  || 
            (self.rotation_live_time > 0 &&
            SystemTime::now().duration_since(self.rotation_time).unwrap().as_secs() > self.rotation_live_time) {

                self.pages.insert(self.active_page.page_no, self.active_page.clone());
                self.active_page = Page::new(self.version);
        }

    }

    
}