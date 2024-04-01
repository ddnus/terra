use std::io;


#[derive(Debug, Clone)]
pub struct Config {

}

impl Default for Config {
    fn default() -> Self {
        Config{

        }
    }
}

impl Config {
    pub(crate) fn validate(&self) -> io::Result<()> {
        Ok(())
    }
}