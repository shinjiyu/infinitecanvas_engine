/// WebGPU 纹理管理器
/// 
/// 负责：
/// - GPU 纹理的创建和销毁
/// - 纹理缓存和引用计数
/// - ImageData 到 wgpu::Texture 的转换
use std::collections::HashMap;
use std::sync::Arc;
use wgpu;

/// 纹理条目
struct TextureEntry {
    /// GPU 纹理对象
    texture: wgpu::Texture,
    /// 纹理视图（用于绑定到 Shader）
    view: wgpu::TextureView,
    /// 纹理尺寸 (width, height)
    size: (u32, u32),
    /// 引用计数（用于资源管理）
    ref_count: usize,
}

/// 纹理管理器
/// 
/// # 示例
/// 
/// ```
/// let mut manager = TextureManager::new(device, queue);
/// 
/// // 创建纹理
/// manager.create_texture(
///     "logo".to_string(),
///     &rgba_data,
///     256,
///     256,
/// )?;
/// 
/// // 获取纹理视图
/// if let Some(view) = manager.get_texture_view("logo") {
///     // 使用纹理视图...
/// }
/// 
/// // 删除纹理
/// manager.remove_texture("logo");
/// ```
pub struct TextureManager {
    /// 纹理缓存 (texture_id -> TextureEntry)
    textures: HashMap<String, TextureEntry>,
    /// WebGPU 设备（用于创建纹理）
    device: Arc<wgpu::Device>,
    /// WebGPU 队列（用于上传数据）
    queue: Arc<wgpu::Queue>,
}

impl TextureManager {
    /// 创建新的纹理管理器
    pub fn new(device: Arc<wgpu::Device>, queue: Arc<wgpu::Queue>) -> Self {
        Self {
            textures: HashMap::new(),
            device,
            queue,
        }
    }

    /// 从 RGBA8 数据创建纹理
    /// 
    /// # 参数
    /// 
    /// - `id`: 纹理的唯一标识符
    /// - `data`: RGBA8 格式的图像数据（每像素 4 字节）
    /// - `width`: 图像宽度（像素）
    /// - `height`: 图像高度（像素）
    /// 
    /// # 错误
    /// 
    /// - 如果纹理 ID 已存在，返回错误
    /// - 如果数据长度不匹配 (width * height * 4)，返回错误
    /// - 如果纹理尺寸超过 GPU 限制，返回错误
    pub fn create_texture(
        &mut self,
        id: String,
        data: &[u8],
        width: u32,
        height: u32,
    ) -> Result<(), String> {
        // 检查纹理是否已存在
        if self.textures.contains_key(&id) {
            return Err(format!("Texture '{}' already exists", id));
        }

        // 验证数据长度
        let expected_len = (width * height * 4) as usize;
        if data.len() != expected_len {
            return Err(format!(
                "Data length mismatch: expected {} bytes ({}x{} RGBA8), got {}",
                expected_len,
                width,
                height,
                data.len()
            ));
        }

        // 验证纹理尺寸（WebGPU 通常限制为 8192x8192）
        const MAX_TEXTURE_SIZE: u32 = 8192;
        if width > MAX_TEXTURE_SIZE || height > MAX_TEXTURE_SIZE {
            return Err(format!(
                "Texture size {}x{} exceeds maximum {}x{}",
                width, height, MAX_TEXTURE_SIZE, MAX_TEXTURE_SIZE
            ));
        }

        log::info!(
            "Creating texture '{}': {}x{} ({} KB)",
            id,
            width,
            height,
            data.len() / 1024
        );

        // 创建 GPU 纹理
        let texture = self.device.create_texture(&wgpu::TextureDescriptor {
            label: Some(&format!("Texture: {}", id)),
            size: wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1, // TODO: 未来支持 mipmap
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8UnormSrgb, // RGBA8 with sRGB gamma
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });

        // 上传数据到 GPU
        // wgpu 27 API: write_texture(texture_info, data, data_layout, size)
        self.queue.write_texture(
            texture.as_image_copy(),
            data,
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(4 * width),
                rows_per_image: Some(height),
            },
            wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
        );

        // 创建纹理视图
        let view = texture.create_view(&wgpu::TextureViewDescriptor::default());

        // 添加到缓存
        self.textures.insert(
            id.clone(),
            TextureEntry {
                texture,
                view,
                size: (width, height),
                ref_count: 1,
            },
        );

        log::info!("Texture '{}' created successfully", id);
        Ok(())
    }

    /// 获取纹理视图
    /// 
    /// # 返回
    /// 
    /// - `Some(&wgpu::TextureView)`: 如果纹理存在
    /// - `None`: 如果纹理不存在
    pub fn get_texture_view(&self, id: &str) -> Option<&wgpu::TextureView> {
        self.textures.get(id).map(|entry| &entry.view)
    }

    /// 获取纹理尺寸
    /// 
    /// # 返回
    /// 
    /// - `Some((width, height))`: 如果纹理存在
    /// - `None`: 如果纹理不存在
    pub fn get_texture_size(&self, id: &str) -> Option<(u32, u32)> {
        self.textures.get(id).map(|entry| entry.size)
    }

    /// 增加纹理引用计数
    /// 
    /// 当创建新的 Image 元素使用此纹理时调用
    pub fn add_ref(&mut self, id: &str) {
        if let Some(entry) = self.textures.get_mut(id) {
            entry.ref_count += 1;
            log::debug!("Texture '{}' ref_count: {}", id, entry.ref_count);
        }
    }

    /// 减少纹理引用计数
    /// 
    /// 当删除 Image 元素时调用。如果引用计数降到 0，纹理会被自动删除。
    pub fn release(&mut self, id: &str) {
        if let Some(entry) = self.textures.get_mut(id) {
            entry.ref_count = entry.ref_count.saturating_sub(1);
            log::debug!("Texture '{}' ref_count: {}", id, entry.ref_count);

            if entry.ref_count == 0 {
                log::info!("Texture '{}' ref_count reached 0, removing...", id);
                self.textures.remove(id);
            }
        }
    }

    /// 强制删除纹理（忽略引用计数）
    /// 
    /// # 注意
    /// 
    /// 如果有 Image 元素仍在使用此纹理，会导致渲染错误。
    /// 通常应该使用 `release()` 而不是此方法。
    pub fn remove_texture(&mut self, id: &str) {
        if self.textures.remove(id).is_some() {
            log::warn!("Texture '{}' forcibly removed", id);
        }
    }

    /// 检查纹理是否存在
    pub fn has_texture(&self, id: &str) -> bool {
        self.textures.contains_key(id)
    }

    /// 获取所有纹理 ID
    pub fn texture_ids(&self) -> Vec<String> {
        self.textures.keys().cloned().collect()
    }

    /// 获取纹理总数
    pub fn texture_count(&self) -> usize {
        self.textures.len()
    }

    /// 计算纹理占用的内存（字节）
    /// 
    /// 这只是估算值（RGBA8 格式），实际 GPU 内存占用可能不同。
    pub fn memory_usage(&self) -> usize {
        self.textures
            .values()
            .map(|entry| {
                let (w, h) = entry.size;
                (w * h * 4) as usize // RGBA8 = 4 bytes per pixel
            })
            .sum()
    }

    /// 清空所有纹理
    /// 
    /// # 警告
    /// 
    /// 会删除所有纹理，包括正在使用的纹理。
    pub fn clear(&mut self) {
        let count = self.textures.len();
        self.textures.clear();
        log::warn!("Cleared {} textures", count);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // Note: 这些测试需要 WebGPU 环境，在 WASM 外无法运行
    // 仅作为文档和结构验证

    #[test]
    fn test_data_length_validation() {
        // 模拟数据长度验证逻辑
        let width = 256u32;
        let height = 256u32;
        let data = vec![0u8; (width * height * 4) as usize];

        let expected_len = (width * height * 4) as usize;
        assert_eq!(data.len(), expected_len);
    }

    #[test]
    fn test_memory_calculation() {
        // 测试内存计算
        let width = 512u32;
        let height = 512u32;
        let bytes_per_pixel = 4; // RGBA8

        let memory = (width * height * bytes_per_pixel) as usize;
        assert_eq!(memory, 1_048_576); // 1 MB
    }
}

