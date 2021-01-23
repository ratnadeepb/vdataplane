use bindgen::Builder;
use std::{
	env,
	path::PathBuf,
	process::{exit, Command, Stdio},
};

fn main() {
	println!("cargo:rerun-if-changed=build.rs");
	println!("cargo:rerun-if-changed=src/bindings.h");
	println!("cargo:rerun-if-changed=src/shim.c");
	println!("cargo:rustc-link-search=/usr/local/lib/x86_64-linux-gnu/");

	let child = Command::new("./pkg_names.sh")
		.stdout(Stdio::piped())
		.spawn()
		.expect("command failed");
	let output = child.wait_with_output().unwrap();

	let mut rte_libs = Vec::new();

	if output.status.success() {
		let output = String::from_utf8_lossy(&output.stdout);
		for out in output.split_ascii_whitespace() {
			rte_libs.push(out.to_owned());
		}
	} else {
		exit(1);
	}

	rte_libs
		.iter()
		.for_each(|lib| println!("cargo:rustc-link-lib=dylib={}", lib));

	cc::Build::new()
		.file("src/shim.c")
		.flag("-march=corei7")
		.flag("-mavx")
		.compile("rte_shim");

	let bindings = Builder::default()
		.clang_arg("-I${RTE_SDK}/build/lib")
		.header("src/bindings.h")
		.layout_tests(false)
		.generate_inline_functions(true)
		.parse_callbacks(Box::new(bindgen::CargoCallbacks))
		// .blacklist_type("rte_arp_ipv4")
		// .blacklist_type("rte_arp_hdr")
		.opaque_type(r"rte_arp_ipv4|rte_arp_hdr")
		.whitelist_type(r"(rte|eth|pcap)_.*")
		.whitelist_function(r"(_rte|rte|_pkt|eth|numa|pcap)_.*")
		.whitelist_var(r"(RTE|DEV|ETH|MEMPOOL|PKT|rte)_.*")
		.derive_copy(true)
		.derive_debug(true)
		.derive_default(true)
		.derive_partialeq(true)
		.default_enum_style(bindgen::EnumVariation::ModuleConsts)
		.clang_arg("-finline-functions")
		.clang_arg("-march=corei7")
		.rustfmt_bindings(true)
		.generate()
		.expect("Failed to generate bindings");

	let out_path = PathBuf::from(env::var("OUT_DIR").unwrap());
	bindings
		.write_to_file(out_path.join("bindings.rs"))
		.expect("Failed to write bindings");
}
