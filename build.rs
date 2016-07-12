extern crate gcc;

fn main() {
    gcc::Config::new()
                .file("src/libsfp/src/serial_framing_protocol.cpp")
                .include("src/libsfp/src")
                .include("src/libsfp/include")
                .include("src/cxx-util/include")
                .cpp(true)
                .flag("-std=c++11")
                .compile("libsfp.a");
}
