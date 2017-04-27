use std::io::prelude::*;
use std::io;
use std::io::SeekFrom;
use std::str;

#[derive(Debug)]
pub struct DocLoader {
}

use persistence::Persistence;

impl DocLoader {
    pub fn load(persistence:&mut Persistence) {
        persistence.load_index_64("data.offsets").unwrap();
    }

    pub fn get_doc(persistence:&Persistence, pos: usize) -> Result<String, io::Error> {
        let (start, end) = {
            let offsets = persistence.cache.index_64.get("data.offsets").unwrap();
            (offsets[pos] as usize, offsets[pos as usize + 1] as usize) // @Temporary array access by get - option
        };

        let mut f = persistence.get_file_handle("data")?;
        // println!("OPen Time: {}", (now.elapsed().as_secs() as f64 * 1_000.0) + (now.elapsed().subsec_nanos() as f64 / 1000_000.0));
        // let start = offsets[pos] as usize;
        // let end = offsets[pos as usize + 1] as usize;
        let mut buffer:Vec<u8> = Vec::with_capacity((end - start) as usize);
        unsafe { buffer.set_len(end - start ); }
        // println!("Buffer Create Time: {}", (now.elapsed().as_secs() as f64 * 1_000.0) + (now.elapsed().subsec_nanos() as f64 / 1000_000.0));

        f.seek(SeekFrom::Start(start as u64))?;
        f.read_exact(&mut buffer)?;
        // println!("Load Buffer: {}", (now.elapsed().as_secs() as f64 * 1_000.0) + (now.elapsed().subsec_nanos() as f64 / 1000_000.0));

        let s = unsafe {str::from_utf8_unchecked(&buffer).to_string()};
        // println!("To String: {}", (now.elapsed().as_secs() as f64 * 1_000.0) + (now.elapsed().subsec_nanos() as f64 / 1000_000.0));

        Ok(s)
    }

}

