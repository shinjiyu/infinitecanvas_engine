//! 颜色工具
//!
//! 提供颜色类型和解析功能
use serde::{Deserialize, Serialize};

/// RGBA 颜色（GPU 格式）
///
/// 每个通道范围 [0.0, 1.0]
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Color {
    pub r: f32,
    pub g: f32,
    pub b: f32,
    pub a: f32,
}

impl Color {
    /// 创建颜色
    pub const fn new(r: f32, g: f32, b: f32, a: f32) -> Self {
        Self { r, g, b, a }
    }

    /// 从 RGB 创建（不透明）
    pub const fn rgb(r: f32, g: f32, b: f32) -> Self {
        Self::new(r, g, b, 1.0)
    }

    /// 转换为 [f32; 4] 数组（GPU 顶点格式）
    pub fn to_array(&self) -> [f32; 4] {
        [self.r, self.g, self.b, self.a]
    }

    /// 从数组创建
    pub fn from_array(arr: [f32; 4]) -> Self {
        Self {
            r: arr[0],
            g: arr[1],
            b: arr[2],
            a: arr[3],
        }
    }

    /// 从十六进制字符串解析
    ///
    /// # 支持格式
    /// - `#RGB` - 短格式
    /// - `#RRGGBB` - 标准格式
    /// - `#RRGGBBAA` - 带透明度
    ///
    /// # 示例
    /// ```
    /// use infinitecanvas_math::Color;
    ///
    /// let red = Color::from_hex("#ff0000").unwrap();
    /// assert_eq!(red, Color::rgb(1.0, 0.0, 0.0));
    ///
    /// let semi_blue = Color::from_hex("#0000ff80").unwrap();
    /// assert!((semi_blue.a - 0.5).abs() < 0.01);
    /// ```
    pub fn from_hex(hex: &str) -> Result<Self, String> {
        let hex = hex.trim_start_matches('#');

        match hex.len() {
            // #RGB
            3 => {
                let r = u8::from_str_radix(&hex[0..1].repeat(2), 16)
                    .map_err(|_| format!("Invalid hex: {}", hex))?;
                let g = u8::from_str_radix(&hex[1..2].repeat(2), 16)
                    .map_err(|_| format!("Invalid hex: {}", hex))?;
                let b = u8::from_str_radix(&hex[2..3].repeat(2), 16)
                    .map_err(|_| format!("Invalid hex: {}", hex))?;
                Ok(Color::rgb(
                    r as f32 / 255.0,
                    g as f32 / 255.0,
                    b as f32 / 255.0,
                ))
            }
            // #RRGGBB
            6 => {
                let r = u8::from_str_radix(&hex[0..2], 16)
                    .map_err(|_| format!("Invalid hex: {}", hex))?;
                let g = u8::from_str_radix(&hex[2..4], 16)
                    .map_err(|_| format!("Invalid hex: {}", hex))?;
                let b = u8::from_str_radix(&hex[4..6], 16)
                    .map_err(|_| format!("Invalid hex: {}", hex))?;
                Ok(Color::rgb(
                    r as f32 / 255.0,
                    g as f32 / 255.0,
                    b as f32 / 255.0,
                ))
            }
            // #RRGGBBAA
            8 => {
                let r = u8::from_str_radix(&hex[0..2], 16)
                    .map_err(|_| format!("Invalid hex: {}", hex))?;
                let g = u8::from_str_radix(&hex[2..4], 16)
                    .map_err(|_| format!("Invalid hex: {}", hex))?;
                let b = u8::from_str_radix(&hex[4..6], 16)
                    .map_err(|_| format!("Invalid hex: {}", hex))?;
                let a = u8::from_str_radix(&hex[6..8], 16)
                    .map_err(|_| format!("Invalid hex: {}", hex))?;
                Ok(Color::new(
                    r as f32 / 255.0,
                    g as f32 / 255.0,
                    b as f32 / 255.0,
                    a as f32 / 255.0,
                ))
            }
            _ => Err(format!("Invalid hex color length: {}", hex)),
        }
    }

    /// 转换为十六进制字符串
    pub fn to_hex(&self) -> String {
        format!(
            "#{:02x}{:02x}{:02x}{:02x}",
            (self.r * 255.0) as u8,
            (self.g * 255.0) as u8,
            (self.b * 255.0) as u8,
            (self.a * 255.0) as u8,
        )
    }

    // 常用颜色常量
    pub const BLACK: Color = Color::new(0.0, 0.0, 0.0, 1.0);
    pub const WHITE: Color = Color::new(1.0, 1.0, 1.0, 1.0);
    pub const RED: Color = Color::new(1.0, 0.0, 0.0, 1.0);
    pub const GREEN: Color = Color::new(0.0, 1.0, 0.0, 1.0);
    pub const BLUE: Color = Color::new(0.0, 0.0, 1.0, 1.0);
    pub const TRANSPARENT: Color = Color::new(0.0, 0.0, 0.0, 0.0);
}

impl Default for Color {
    fn default() -> Self {
        Self::BLACK
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_from_hex_3() {
        let color = Color::from_hex("#f00").unwrap();
        assert!((color.r - 1.0).abs() < 0.01);
        assert!((color.g - 0.0).abs() < 0.01);
        assert!((color.b - 0.0).abs() < 0.01);
    }

    #[test]
    fn test_from_hex_6() {
        let color = Color::from_hex("#ff0000").unwrap();
        assert_eq!(color, Color::RED);
    }

    #[test]
    fn test_from_hex_8() {
        let color = Color::from_hex("#ff000080").unwrap();
        assert!((color.r - 1.0).abs() < 0.01);
        assert!((color.a - 0.5).abs() < 0.01);
    }

    #[test]
    fn test_to_hex() {
        let color = Color::rgb(1.0, 0.0, 0.0);
        assert_eq!(color.to_hex(), "#ff0000ff");
    }

    #[test]
    fn test_to_array() {
        let color = Color::rgb(0.5, 0.25, 0.75);
        let arr = color.to_array();
        assert_eq!(arr, [0.5, 0.25, 0.75, 1.0]);
    }
}
