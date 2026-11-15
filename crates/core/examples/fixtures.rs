use std::{fs, path::PathBuf};
fn main() -> anyhow::Result<()> {
    let dir = PathBuf::from(std::env::args().nth(1).unwrap_or("target/fixtures".into()));
    fs::create_dir_all(&dir)?;
    let mut text = String::from("# 第一章 窗边\n今天有些疲惫。让文字陪你坐一会儿。\n");
    for i in 0..110 {
        text.push_str(&format!("第 {i} 段：一个人也可以慢慢读书。The light falls gently across the page. 字体、行距和窗口可以变化，故事里的位置应该被记住。\n"));
    }
    text.push_str("# 第二章 明天\n希望像一粒种子，不必急着开花。\n");
    fs::write(
        dir.join("book.epub"),
        glow_core::library::text_epub(&text, "测试阅读", "微光测试")?,
    )?;
    let mut external = String::new();
    for (index, heading) in ["CHAPTER I", "CHAPTER II", "CHAPTER III", "CHAPTER IV"]
        .iter()
        .enumerate()
    {
        external.push_str(&format!(
            "# {heading}\nThe light falls gently across page {}.\nReading should remain in place.\n\n",
            index + 1
        ));
    }
    fs::write(
        dir.join("external.epub"),
        glow_core::library::text_epub(&external, "External Fixture", "Test Author")?,
    )?;
    Ok(())
}
