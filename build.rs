fn main() {
    println!("cargo:rerun-if-changed=assets/icon.ico");
    let mut res = winresource::WindowsResource::new();
    res.set_icon("assets/icon.ico");
    res.set("ProductName", "Yandex Music RPC");
    res.set("FileDescription", "Yandex Music RPC");
    res.compile().unwrap();
}
