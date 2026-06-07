fn main() {
    let path = portus_lib::bindings::typescript_bindings_path();
    portus_lib::bindings::export_to(&path).expect("failed to export TypeScript bindings");
    println!("exported TypeScript bindings to {}", path.display());
}
