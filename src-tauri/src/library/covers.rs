use std::hash::{Hash, Hasher};
use std::path::{Path, PathBuf};

/// 封面缓存文件名 = 内容 hash + 扩展名（相同封面天然去重，技术设计文档 §9）
pub fn save(covers_dir: &Path, data: &[u8], ext: &str) -> std::io::Result<String> {
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    data.hash(&mut hasher);
    let file = format!("{:016x}.{ext}", hasher.finish());
    let dest = covers_dir.join(&file);
    if !dest.exists() {
        std::fs::write(&dest, data)?;
    }
    Ok(file)
}

pub fn path_of(covers_dir: &Path, file: &str) -> PathBuf {
    covers_dir.join(file)
}
