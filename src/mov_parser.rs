// src/mov_parser.rs
use std::fs::File;
use std::io::{Read, Seek, SeekFrom};
use std::path::Path;

#[derive(Debug, Clone)]
pub struct MovAtom {
    pub size: u64,
    pub atom_type: [u8; 4],
    pub offset: u64,
}

pub struct MovParser {
    file: File,
}

impl MovParser {
    pub fn new(path: &Path) -> std::io::Result<Self> {
        let file = File::open(path)?;
        Ok(Self { file })
    }
    
    pub fn parse_atoms(&mut self) -> std::io::Result<Vec<MovAtom>> {
        let mut atoms = Vec::new();
        let file_size = self.file.metadata()?.len();
        
        self.file.seek(SeekFrom::Start(0))?;
        
        let mut offset = 0u64;
        while offset < file_size {
            let mut size_bytes = [0u8; 4];
            self.file.read_exact(&mut size_bytes)?;
            let size = u32::from_be_bytes(size_bytes) as u64;
            
            let mut atom_type = [0u8; 4];
            self.file.read_exact(&mut atom_type)?;
            
            atoms.push(MovAtom {
                size,
                atom_type,
                offset,
            });
            
            // Пропускаем содержимое атома
            if size > 8 {
                self.file.seek(SeekFrom::Current((size - 8) as i64))?;
            }
            
            offset += size;
        }
        
        Ok(atoms)
    }
    
    pub fn find_atom(&mut self, atom_type: &[u8; 4]) -> std::io::Result<Option<MovAtom>> {
        let atoms = self.parse_atoms()?;
        Ok(atoms.into_iter().find(|a| &a.atom_type == atom_type))
    }
    
    pub fn get_video_info(&mut self) -> Option<(u32, u32, f64)> {
        // Простая заглушка - в реальности нужно парсить trak атомы
        Some((1920, 1080, 30.0))
    }
}