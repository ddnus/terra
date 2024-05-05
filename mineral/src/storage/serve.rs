use core::panic;
use std::{sync::{Arc, Mutex}, thread, time::Duration};

use crate::{config::StorageConfig, error::Error};

use super::{cbf::Cbf, mainblock::MainBlock, wal::Wal};


const BLOCK_OP_SET: u8 = 1;
const BLOCK_OP_DEL: u8 = 3;
// BlockOp block operate
pub enum BlockOp {
    Set(u64, Vec<u8>),
    Del(u64),
}

impl BlockOp {
    pub fn encode_from(op: u8, pos: u64, data: Vec<u8>) -> Vec<u8> {
        let mut block_op_buf = vec![op];
        block_op_buf.extend_from_slice(&pos.to_be_bytes());
        if op != BLOCK_OP_DEL {
            block_op_buf.extend_from_slice(&data.to_vec());
        }
        block_op_buf
    }

    pub fn decode(buf: &Vec<u8>) -> Self {
        let op = buf[0];
        if op == BLOCK_OP_SET {
            
            BlockOp::Set(u64::from_be_bytes(buf[1..9].try_into().unwrap()), buf[9..].to_vec())
        } else {
            BlockOp::Del(u64::from_be_bytes(buf[1..9].try_into().unwrap()))
        }
    }

    pub fn get_pos(block_op: BlockOp) -> u64 {
        match block_op {
            BlockOp::Set(pos, _) => pos,
            BlockOp::Del(pos) => pos
        }
    }
}

pub struct Serve {
    mainblock: Arc<Mutex<MainBlock>>,
    wal: Arc<Mutex<Wal>>,
    cbf: Arc<Mutex<Cbf>>,
}

impl Serve {
    pub fn new(conf: StorageConfig) -> Self {
        let serve = Serve {
            wal: Arc::new(Mutex::new(Wal::new(&conf.path))),
            mainblock: Arc::new(Mutex::new(MainBlock::new(&conf.path, conf.block_size, true))),
            cbf: Arc::new(Mutex::new(Cbf::new(conf.page_max_cap))),
        };
        serve.init_wait_block();

        serve.run();
        serve
    }

    pub fn get(&mut self, pos: usize) -> Result<Vec<u8>, Error> {
        if let Some(cached) = self.cbf.lock().unwrap().get(pos) {
            let block_op = BlockOp::decode(&cached);
            return match block_op {
                BlockOp::Del(_) => Ok(vec![]),
                BlockOp::Set(_, data) => Ok(data),
            }
        }

        match self.mainblock.lock().unwrap().get(pos) {
            Ok(data) => Ok(data),
            Err(err) => Err(Error::BlockDataGetFailed(err)),
        }
    }

    pub fn set(&mut self, pos: usize, buf: Vec<u8>) -> Result<(), Error> {
        let buf = BlockOp::encode_from(BLOCK_OP_SET, pos as u64, buf);
        match self.wal.lock().unwrap().append(&buf) {
            Ok(version) => {
                self.cbf.lock().unwrap().insert(version as usize, pos, buf)
            },
            Err(err) => Err(err)
        }
    }

    pub fn del(&mut self, pos: usize) -> Result<(), Error> {
        if let Err(err) = self.mainblock.lock().unwrap().del(pos) {
            return Err(Error::MainDataDelFailed(err));
        }
        Ok(())
    }

    fn init_wait_block(&self) {
        // 初始化检查点后的数据，全部写入缓冲
        let checkpoint = self.mainblock.lock().unwrap().checkpoint();
        println!("checkpoint: {}", checkpoint);
        
        let wal_reder = self.wal.lock().unwrap().reader(checkpoint, 0);
        if wal_reder.is_none() {
            return;
        }

        for s in wal_reder.unwrap() {
            let payload = s.unwrap();
            let block_op = BlockOp::decode(&payload.data);
            self.cbf.lock().unwrap().insert(payload.version as usize, 
                BlockOp::get_pos(block_op) as usize, payload.data).unwrap();
        }
    }

    fn run(&self) {
        
        let cbf = self.cbf.clone();
        let mainblock = self.mainblock.clone();
        thread::spawn(move || {
            loop {
                if let Some((page_no, page)) = cbf.lock().unwrap().pop_first_page() {
                    // println!("page no: {}, entrys: {:?}", page_no, page.entrys);
                    for (pos, buf) in page.entrys {
                        // println!("pos: {}", pos);
                        let block_op = BlockOp::decode(&buf);
                        match block_op {
                            BlockOp::Set(p, data) => {
                                mainblock.lock().unwrap().set(p as usize, &data).unwrap()
                            },
                            BlockOp::Del(p) => {
                                mainblock.lock().unwrap().del(p as usize).unwrap()
                            }
                        }
                    }

                    if let Err(err) = mainblock.lock().unwrap().flush_datablock(page.max_version) {
                        panic!("flush page error");
                    }
                }

                thread::sleep(Duration::from_millis(100));
            }
        });
        
    }

}

#[cfg(test)]
mod tests {
    use std::vec;

    use rand::Rng;

    use super::*;

    fn get_conf() -> StorageConfig {
        StorageConfig {
            path: "/tmp/terra/tests/serve1".to_string(),
            block_size: 1024,
            page_max_cap: 1024 * 1024 * 50,
        }
    }

    #[test]
    fn set_test() {
        let conf = get_conf();
        let mut serve = Serve::new(conf);

        let mut list: Vec<(usize, Vec<u8>)> = vec![];
        for i in 100..500 {
            let secret_number = rand::thread_rng().gen_range(0..100);
            list.push((i as usize, vec![(i + 1) as u8; secret_number]));
        }

        for item in list.clone() {
            serve.set(item.0, item.1);
        }

        for item in list {
            let res = serve.get(item.0).unwrap();
            assert_eq!(res, item.1);
        }
        
        thread::sleep(Duration::from_millis(7000));
    }

    #[test]
    fn test_del() {
        let conf = get_conf();
        let mut serve = Serve::new(conf);

        serve.del(10);

        let del = serve.get(10);
        assert_eq!(del.unwrap(), vec![]);
    }

    #[test]
    fn test_byte() {
        let mv = [0, 0, 0, 0, 0, 0, 0, 1];

        assert_eq!(u64::from_be_bytes(mv), 1);
    }
}