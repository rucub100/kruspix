fn main() {
    let target = std::env::var("TARGET").unwrap();
    let arch = SupportedArch::from_target(&target).expect(format!("Unsupported target: {}", target).as_str());

    let linker_script = format!("src/arch/{}/kernel.ld", arch.to_dir_name());
    println!("cargo::rustc-link-arg=-T{}", linker_script);
}

enum SupportedArch {
    Aarch64,
}

impl SupportedArch {
    pub fn from_target(target: &str) -> Option<Self> {
        match target {
            "aarch64-unknown-none" => Some(SupportedArch::Aarch64),
            "aarch64-unknown-none-softfloat" => Some(SupportedArch::Aarch64),
            _ => None,
        }
    }

    pub fn to_dir_name(&self) -> &str {
        match self {
            SupportedArch::Aarch64 => "arm64",
        }
    }
}