pub fn compile(
    source_text: &str,
    shader_kind: shaderc::ShaderKind,
    name: &str,
    entry: &str,
) -> shaderc::CompilationArtifact {
    let compiler = shaderc::Compiler::new().expect("Failed to create shaderc compiler");
    let options = shaderc::CompileOptions::new().expect("Failed to create shaderc options");
    compiler
        .compile_into_spirv(source_text, shader_kind, name, entry, Some(&options))
        .expect("Failed to compile shader source")
}
