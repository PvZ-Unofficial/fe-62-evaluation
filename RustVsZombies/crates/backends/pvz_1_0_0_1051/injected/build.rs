fn main() {
    println!("cargo:rerun-if-changed=src/paint/native_canvas.cpp");
    if std::env::var("TARGET").as_deref() != Ok("i686-pc-windows-msvc") {
        return;
    }
    cc::Build::new()
        .cpp(true)
        .std("c++17")
        .flag("/EHsc")
        .file("src/paint/native_canvas.cpp")
        .compile("rsvz_paint_1051");
    println!("cargo:rustc-link-lib=gdi32");
    println!("cargo:rustc-link-lib=user32");
}
