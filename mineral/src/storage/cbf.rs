use std::{collections::BTreeMap, time::SystemTime};

use crate::error::Error;

type PageNo = usize;
type PageCap = usize;
type EntryPos = usize;
type VersionNo = usize;

type Entry = Vec<u8>;

#[derive(Debug, Clone)]
pub struct Page {
    page_no: PageNo,
    pub max_version: usize,
    cap: PageCap,
    pub entrys: BTreeMap<EntryPos, Entry>,
}

impl Page {
    fn new(page_no: PageNo) -> Self {
        Page {
            page_no,
            max_version: page_no,
            cap: 0,
            entrys: BTreeMap::new(),
        }
    }

    fn insert(&mut self, version: usize, entry_pos: EntryPos, entry: Entry) {
        if version > self.max_version {
            self.max_version = version;
        }
        self.cap += entry.len();
        self.entrys.insert(entry_pos, entry);
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

            rotation_live_time: 5,   // 5min
            rotation_time: SystemTime::now(),

        }
    }

    pub fn insert(&mut self, version: VersionNo, entry_pos: EntryPos, buf: Vec<u8>) -> Result<(), Error> {
        self.version = version;

        self.rotation_page(buf.len());
        self.active_page.insert(version, entry_pos, buf);

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

    pub fn get(&mut self, entry_pos: EntryPos) -> Option<Vec<u8>> {
        let opt_data = self.active_page.entrys.get(&entry_pos);
        if let Some(data) = opt_data {
            return Some(data.clone());
        }

        for (page_no, page) in self.pages.range(..).rev() {
            let opt_data = page.entrys.get(&entry_pos);
            if let Some(data) = opt_data {
                return Some(data.clone());
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

#[cfg(test)]
mod tests {
    use rand::Rng;

    use super::*;

    #[test]
    fn insert_test() {
        let mut cbf = Cbf::new(1024);
        let version = 0;
        let mut list: Vec<(usize, usize, Vec<u8>)> = vec![];
        for i in 0..100 {
            let secret_number = rand::thread_rng().gen_range(0..100);
            list.push((i as usize, i as usize, vec![(i + 1) as u8; secret_number]));
        }
        
        for item in list.clone() {
            cbf.insert(item.0, item.1, item.2);
        }

        for item in list {
            let a = cbf.get(item.0);
            assert!(a.is_some());
            assert_eq!(a.unwrap(), item.2);
        }
        println!("pages: {}", cbf.pages.len());
    }
}