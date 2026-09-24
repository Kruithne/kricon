fn main() {
	#[cfg(windows)]
	winresource::WindowsResource::new()
		.set_icon("res/kricon.ico")
		.compile()
		.unwrap();
}
