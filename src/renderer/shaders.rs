use ash::vk;

pub struct Shaders {
    vertex_shader_module: vk::ShaderModule,
    fragment_shader_module: vk::ShaderModule,
}

impl Shaders {
    pub fn new(device: &ash::Device) -> Self {
        let vertex_shader_code = compile_shader(
            &include_str!("../../shaders/vertex_shader.glsl"),
            shaderc::ShaderKind::Vertex,
            "vertex_shader.glsl",
            "main",
        );

        let vertex_shader_info =
            vk::ShaderModuleCreateInfo::default().code(&vertex_shader_code.as_binary());

        let vertex_shader_module = unsafe {
            device
                .create_shader_module(&vertex_shader_info, None)
                .expect("Failed to create vertex shader module")
        };

        let fragment_shader_code = compile_shader(
            &include_str!("../../shaders/fragment_shader.glsl"),
            shaderc::ShaderKind::Fragment,
            "fragment_shader.glsl",
            "main",
        );

        let fragment_shader_info =
            vk::ShaderModuleCreateInfo::default().code(&fragment_shader_code.as_binary());

        let fragment_shader_module = unsafe {
            device
                .create_shader_module(&fragment_shader_info, None)
                .expect("Failed to create fragment shader module")
        };

        Self {
            vertex_shader_module,
            fragment_shader_module,
        }
    }
    pub fn shader_stage_infos(&self) -> Vec<vk::PipelineShaderStageCreateInfo> {
        vec![
            vk::PipelineShaderStageCreateInfo {
                module: self.vertex_shader_module,
                p_name: c"main".as_ptr(),
                stage: vk::ShaderStageFlags::VERTEX,
                ..Default::default()
            },
            vk::PipelineShaderStageCreateInfo {
                module: self.fragment_shader_module,
                p_name: c"main".as_ptr(),
                stage: vk::ShaderStageFlags::FRAGMENT,
                ..Default::default()
            },
        ]
    }
    pub fn cleanup(&self, device: &ash::Device) {
        unsafe {
            device.destroy_shader_module(self.vertex_shader_module, None);
            device.destroy_shader_module(self.fragment_shader_module, None);
        }
    }
}
fn compile_shader(
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
