fn main() {
 let args: Vec<String> = std::env::args().collect();
 let data=std::fs::read(&args[1]).unwrap();
 let tree=usvg::Tree::from_data(&data,&usvg::Options::default()).unwrap();
 let size=tree.size().to_int_size();
 let mut pixmap=tiny_skia::Pixmap::new(size.width(),size.height()).unwrap();
 resvg::render(&tree,tiny_skia::Transform::default(),&mut pixmap.as_mut());
 pixmap.save_png(&args[2]).unwrap();
}
