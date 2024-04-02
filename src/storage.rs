// use std::fs::{self, File, OpenOptions};
// use std::io::{self, Read, Seek, SeekFrom, Write};
// use std::path::Path;

use crate::bytemap;

pub struct Storage {
    bitmap: bytemap::BitMap,
    
}

// struct Storage {
//     table: Option<String>,
//     base_path: String,
//     byte_handle: Option<File>,
//     main_handle: Option<File>,
//     data_handle: Option<File>,
//     main_header: Option<MainHeader>,
//     data_area: u64,
// }

// struct MainHeader {
//     s_id: u64,
//     n_id: u64,
//     table_type: String,
// }

// impl Storage {
//     pub fn new(db_space: &str) -> io::Result<Self> {
//         let base_path = format!("{}{}/", WECLU_BASE_PATH, db_space);
//         if !Path::new(&base_path).exists() {
//             fs::create_dir_all(&base_path)?;
//         }
//         Ok(Self {
//             table: None,
//             base_path,
//             byte_handle: None,
//             main_handle: None,
//             data_handle: None,
//             main_header: None,
//             data_area: 0,
//         })
//     }

//     pub fn close(&mut self) {
//         self.byte_handle = None;
//         self.main_handle = None;
//         self.data_handle = None;
//         self.data_area = 0;
//         self.main_header = None;
//     }

//     pub fn create_table(&mut self, table: &str, table_type: &str, start_id: u64) -> io::Result<bool> {
//         let main_path = format!("{}{}@main.data", self.base_path, table);
//         if !Path::new(&main_path).exists() {
//             self.table = Some(table.to_string());
//             self.close(); // Close previous handles
//             self.main_handle = Some(File::create(main_path)?);
//             self.init_main_header(table_type, start_id)?;
//             Ok(true)
//         } else {
//             Ok(false)
//         }
//     }

//     // Other methods would be translated similarly...
    
//     fn init_main_header(&mut self, table_type: &str, start_id: u64) -> io::Result<()> {
//         let header = MainHeader {
//             s_id: start_id,
//             n_id: if start_id > 0 { start_id - 1 } else { 0 },
//             table_type: table_type.to_string(),
//         };
//         self.set_main_header(header)
//     }

//     fn set_main_header(&mut self, header: MainHeader) -> io::Result<()> {
//         if let Some(ref mut file) = self.main_handle {
//             file.seek(SeekFrom::Start(0))?;
//             // Serialize the header struct to a JSON string for simplicity
//             let header_str = serde_json::to_string(&header)?;
//             file.write_all(&header_str.as_bytes())?;
//             // Pad the rest of the block with spaces
//             let padding = vec![' ' as u8; WECLU_HEADER_BLOCK_SIZE - header_str.len()];
//             file.write_all(&padding)?;
//             self.main_header = Some(header);
//         }
//         Ok(())
//     }
// }

// pub trait Storage {
//     pub fn set();
// }