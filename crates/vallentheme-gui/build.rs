//! Kompilasi GResource: CSS dan ikon yang dibundel ke dalam biner.

fn main() {
    glib_build_tools::compile_resources(
        &["../../data"],
        "../../data/vallentheme.gresource.xml",
        "vallentheme.gresource",
    );
}
