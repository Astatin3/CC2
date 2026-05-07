fn main() {
    println!("cargo:rerun-if-changed=src/doom.c");
    println!("cargo:rerun-if-changed=src/PureDOOM.h");

    cc::Build::new()
        .file("src/doom.c")
        .std("gnu99")
        .define("DOOM_IMPLEMENT_PRINT", None)
        .define("DOOM_IMPLEMENT_MALLOC", None)
        .define("DOOM_IMPLEMENT_FILE_IO", None)
        .define("DOOM_IMPLEMENT_GETTIME", None)
        .define("DOOM_IMPLEMENT_EXIT", None)
        .define("DOOM_IMPLEMENT_GETENV", None)
        .warnings(false)
        .compile("doom");
}
