use std::{collections::{BTreeMap, HashMap}, time::SystemTime};

use crate::error::Error;

use super::{slot::Slot, Bytes};

type PageNo = usize;
type PageCap = usize;
type VersionNo = usize;


#[derive(Debug, Clone)]
pub struct Page {
    page_no: PageNo,
    pub max_version: usize,
    cap: PageCap,
    pub entrys: HashMap<usize, Bytes>,
}

impl Page {
    fn new(page_no: PageNo) -> Self {
        Page {
            page_no,
            max_version: page_no,
            cap: 0,
            entrys: HashMap::new(),
        }
    }

    fn insert(&mut self, version: usize, slot_no: usize, slot_data: Bytes) {
        if version > self.max_version {
            self.max_version = version;
        }
        self.cap += slot_data.len();
        self.entrys.insert(slot_no, slot_data);
    }
}

#[derive(Debug)]
pub struct Cbf {
    version: VersionNo,
    pages: BTreeMap<PageNo, Page>,
    page_max_cap: PageCap,
    active_page: Page,

    rotation_live_time: u64,
    rotation_time: SystemTime,
}

impl Cbf {
    pub fn new(cap: usize) -> Self {
        Cbf {
            version: 0,
            pages: BTreeMap::new(),
            page_max_cap: cap,
            active_page: Page::new(0),

            rotation_live_time: 5,   // 5s
            rotation_time: SystemTime::now(),

        }
    }

    pub fn insert(&mut self, version: usize, slot: &Slot) -> Result<(), Error> {
        self.version = version;

        let slot_buf = slot.encode().unwrap();
        self.rotation_page(slot_buf.len());
        
        self.active_page.insert(version, slot.slot_no, slot_buf);

        Ok(())
    }

    pub fn rotation_page(&mut self, buf_len: usize) {
        if self.active_page.cap + buf_len > self.page_max_cap  || 
            (self.rotation_live_time > 0 &&
            SystemTime::now().duration_since(self.rotation_time).unwrap().as_secs() > self.rotation_live_time) {
                
                self.pages.insert(self.active_page.page_no, self.active_page.clone());
                self.active_page = Page::new(self.version);
                self.rotation_time = SystemTime::now();
        }
    }

    pub fn get(&mut self, slot_no: usize) -> Option<Slot> {
        let opt_data = self.active_page.entrys.get(&slot_no);
        if let Some(data) = opt_data {
            return Some(Slot::new(slot_no, data.clone()).unwrap());
        }

        for (page_no, page) in self.pages.range(..).rev() {
            let opt_data = page.entrys.get(&slot_no);
            if let Some(data) = opt_data {
                return Some(Slot::new(slot_no, data.clone()).unwrap());
            }
        }
        
        None
    }

    pub fn pop_first_page(&mut self) -> Option<(PageNo, Page)> {
        if self.pages.len() > 0 {
            return self.pages.pop_first()
        } else if self.active_page.cap > 0 {
            self.rotation_page(0);
        }
        None
    }



}
