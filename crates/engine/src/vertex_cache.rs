//! 持久顶点缓存
//!
//! 元素顶点在世界空间生成一次，之后只重写变脏的那一个元素。
//! 视口平移/缩放只改投影 uniform，不经过这里。
//!
//! ```text
//! 元素 dirty ──► 只重生成该元素 ──► GpuVertexWrite（缓冲内偏移）
//! 增删元素   ──► 按 z-index 重排一次
//! 无变化     ──► geometry_unchanged，调用方只更新 uniform
//! ```
//!
//! 直线、箭头和自由画笔路径的三角形带放进填充缓冲（与原先的收集器一致），
//! 矩形/圆形的 1px 描边仍走线条缓冲。图片和文本走纹理缓冲；只有图片先画纯色底。

use crate::blend::BlendMode;
use crate::element::blur::blur_active;
use crate::scene::{AnyElement, Scene};
use crate::ElementId;
use infinitecanvas_math::Vertex;
use std::collections::HashMap;

/// 一条 GPU `write_buffer` 覆盖的顶点区间。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct GpuVertexWrite {
    /// 写入哪一块持久缓冲
    pub kind: VertexBufferKind,
    /// 顶点偏移（不是字节）
    pub vertex_offset: u32,
    /// 顶点数量
    pub vertex_count: u32,
}

/// 持久顶点缓冲的种类
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum VertexBufferKind {
    /// 三角形列表：矩形/圆形填充，以及直线、箭头、画笔的三角形带
    Fill,
    /// 线段列表：矩形/圆形/多边形的细描边
    Stroke,
    /// 三角形列表：图片
    Textured,
}

/// 一个叶子在持久缓冲里的绘制区间，以及它的混合模式。
///
/// 顶点仍按填充、描边、纹理分缓冲。混合模式不进顶点色；渲染器在有非 Normal
/// 叶子时按这个区间逐个合成。
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PaintSpan {
    /// 该叶子的混合模式。
    pub blend: BlendMode,
    /// 填充三角形起点。
    pub fill_offset: u32,
    /// 填充三角形数量。
    pub fill_count: u32,
    /// 细描边线段起点。
    pub stroke_offset: u32,
    /// 细描边线段数量。
    pub stroke_count: u32,
    /// 纹理三角形起点。
    pub tex_offset: u32,
    /// 纹理三角形数量。
    pub tex_count: u32,
    /// 纹理 id。没有纹理时为 `None`。
    pub texture_id: Option<String>,
    /// 图片先画纯色底。
    pub underlay: bool,
}

/// 一次纹理绘制
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TexturedDraw {
    /// 纹理 id
    pub texture_id: String,
    /// 在纹理顶点缓冲中的起点
    pub vertex_offset: u32,
    /// 顶点数量
    pub vertex_count: u32,
    /// 图片先画纯色底。文本纹理有透明背景，纯色底会挡住字形。
    pub underlay: bool,
    /// 使用 5×5 高斯纹理管线。模糊为 0 的绘制仍走原来的单次采样。
    pub blurred: bool,
}

/// [`VertexCache::sync`] 的结果。缓冲内容用缓存上的切片读取。
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct VertexSync {
    /// 本帧重新生成顶点的元素个数
    pub regenerated: usize,
    /// 需要增量上传的区间。`full_upload` 为真时可以忽略。
    pub writes: Vec<GpuVertexWrite>,
    /// 没有任何元素变化。平移/缩放只需更新 uniform。
    pub geometry_unchanged: bool,
    /// 缓冲被整表重排，调用方应整段上传。
    pub full_upload: bool,
}

/// 优化前的每帧全量重生成结果，只给基准对比用。
#[derive(Clone, Debug)]
pub struct RebuiltGeometry {
    /// 填充与粗描边三角形
    pub fill: Vec<Vertex>,
    /// 细描边线段
    pub stroke: Vec<Vertex>,
    /// 图片三角形
    pub textured: Vec<Vertex>,
}

#[derive(Clone, Debug)]
struct Slot {
    fill_offset: u32,
    fill_count: u32,
    stroke_offset: u32,
    stroke_count: u32,
    tex_offset: u32,
    tex_count: u32,
    texture_id: Option<String>,
    underlay: bool,
    blend: BlendMode,
    blurred: bool,
}

struct Mesh {
    fill: Vec<Vertex>,
    stroke: Vec<Vertex>,
    textured: Vec<Vertex>,
    texture_id: Option<String>,
    underlay: bool,
    blend: BlendMode,
    blurred: bool,
}

impl Mesh {
    fn empty() -> Self {
        Self {
            fill: Vec::new(),
            stroke: Vec::new(),
            textured: Vec::new(),
            texture_id: None,
            underlay: false,
            blend: BlendMode::Normal,
            blurred: false,
        }
    }
}

const ZERO_VERTEX: Vertex = Vertex {
    position: [0.0, 0.0],
    color: [0.0, 0.0, 0.0, 0.0],
    uv: [0.0, 0.0],
    blur_uv: [0.0, 0.0],
};

/// 按元素缓存世界空间顶点，并给出 GPU 增量写入区间。
///
/// # 示例
/// ```
/// use infinitecanvas_engine::{Rectangle, Scene, VertexCache};
/// use infinitecanvas_engine::math::Vec2;
///
/// let mut scene = Scene::new();
/// scene.add_rect(Rectangle::new(1, Vec2::ZERO, Vec2::new(10.0, 10.0)));
/// let mut cache = VertexCache::new();
/// let first = cache.sync(&mut scene);
/// assert!(first.full_upload);
/// assert_eq!(first.regenerated, 1);
/// let second = cache.sync(&mut scene);
/// assert!(second.geometry_unchanged);
/// assert!(second.writes.is_empty());
/// ```
#[derive(Debug)]
pub struct VertexCache {
    slots: HashMap<ElementId, Slot>,
    order: Vec<ElementId>,
    structure_epoch: u64,
    /// 为 true 时下一次 [`Self::sync`] 忽略 epoch，整表重建并要求 GPU 全量上传。
    force_repack: bool,
    fill: Vec<Vertex>,
    stroke: Vec<Vertex>,
    textured: Vec<Vertex>,
    textured_draws: Vec<TexturedDraw>,
}

impl Default for VertexCache {
    fn default() -> Self {
        Self::new()
    }
}

impl VertexCache {
    /// 空缓存。第一次 [`Self::sync`] 会按场景当前内容建表。
    pub fn new() -> Self {
        Self {
            slots: HashMap::new(),
            order: Vec::new(),
            structure_epoch: 0,
            force_repack: false,
            fill: Vec::new(),
            stroke: Vec::new(),
            textured: Vec::new(),
            textured_draws: Vec::new(),
        }
    }

    /// 填充/粗描边三角形（世界空间）
    pub fn fill(&self) -> &[Vertex] {
        &self.fill
    }

    /// 细描边线段（世界空间）
    pub fn stroke(&self) -> &[Vertex] {
        &self.stroke
    }

    /// 图片三角形（世界空间）
    pub fn textured(&self) -> &[Vertex] {
        &self.textured
    }

    /// 纹理绘制区间。无图片变化时保持不变。
    pub fn textured_draws(&self) -> &[TexturedDraw] {
        &self.textured_draws
    }

    /// 按 z 顺序返回有几何的叶子。隐藏叶子（空网格）不出现。
    pub fn paint_spans(&self) -> Vec<PaintSpan> {
        let mut spans = Vec::new();
        for id in &self.order {
            let Some(slot) = self.slots.get(id) else {
                continue;
            };
            if slot.fill_count == 0 && slot.stroke_count == 0 && slot.tex_count == 0 {
                continue;
            }
            spans.push(PaintSpan {
                blend: slot.blend,
                fill_offset: slot.fill_offset,
                fill_count: slot.fill_count,
                stroke_offset: slot.stroke_offset,
                stroke_count: slot.stroke_count,
                tex_offset: slot.tex_offset,
                tex_count: slot.tex_count,
                texture_id: slot.texture_id.clone(),
                underlay: slot.underlay,
            });
        }
        spans
    }

    /// 有可见叶子使用非 Normal 混合时，渲染器改走逐元素合成。
    pub fn needs_blend_composite(&self) -> bool {
        self.order.iter().any(|id| {
            self.slots.get(id).is_some_and(|slot| {
                slot.blend != BlendMode::Normal
                    && (slot.fill_count > 0 || slot.stroke_count > 0 || slot.tex_count > 0)
            })
        })
    }

    /// 某个元素在填充缓冲中的顶点
    pub fn fill_slice(&self, id: ElementId) -> Option<&[Vertex]> {
        self.slots.get(&id).map(|slot| {
            let start = slot.fill_offset as usize;
            let end = start + slot.fill_count as usize;
            &self.fill[start..end]
        })
    }

    /// 某个元素在纹理缓冲中的顶点
    pub fn textured_slice(&self, id: ElementId) -> Option<&[Vertex]> {
        self.slots.get(&id).map(|slot| {
            let start = slot.tex_offset as usize;
            let end = start + slot.tex_count as usize;
            &self.textured[start..end]
        })
    }

    /// 某个元素在描边缓冲中的顶点
    pub fn stroke_slice(&self, id: ElementId) -> Option<&[Vertex]> {
        self.slots.get(&id).map(|slot| {
            let start = slot.stroke_offset as usize;
            let end = start + slot.stroke_count as usize;
            &self.stroke[start..end]
        })
    }

    /// 丢掉已同步的结构版本。下一次 [`Self::sync`] 整表重建，并让 GPU 全量上传。
    ///
    /// JSON 导入会替换整个场景。只靠结构版本可能和缓存里记下的版本撞上，
    /// 已删除的元素就会留在缓冲里。
    pub fn invalidate(&mut self) {
        self.force_repack = true;
    }

    /// 把场景里变脏的元素写进缓存。
    ///
    /// 不读取视口。平移和缩放不应调用 [`Scene::mark_dirty`]，这样这里会返回
    /// [`VertexSync::geometry_unchanged`]。
    pub fn sync(&mut self, scene: &mut Scene) -> VertexSync {
        if self.force_repack || scene.structure_epoch() != self.structure_epoch {
            self.force_repack = false;
            return self.repack(scene);
        }
        if !scene.has_dirty_elements() {
            return VertexSync {
                regenerated: 0,
                writes: Vec::new(),
                geometry_unchanged: true,
                full_upload: false,
            };
        }
        let dirty = scene.take_dirty_ids();
        self.update_dirty(scene, dirty)
    }

    fn repack(&mut self, scene: &mut Scene) -> VertexSync {
        self.slots.clear();
        self.fill.clear();
        self.stroke.clear();
        self.textured.clear();
        self.textured_draws.clear();

        let mut order: Vec<(i32, ElementId)> = scene
            .iter()
            .map(|element| (element.as_element().props().z_index, element.id()))
            .collect();
        order.sort_unstable();
        self.order = order.into_iter().map(|(_, id)| id).collect();

        let count = self.order.len();
        self.fill.reserve(count.saturating_mul(6));
        self.stroke.reserve(count.saturating_mul(8));

        for index in 0..self.order.len() {
            let id = self.order[index];
            let mesh = {
                let element = scene.get(id).expect("ordered element exists");
                mesh_for(scene, element)
            };
            self.append_slot(id, mesh);
            if let Some(element) = scene.get_mut(id) {
                element.as_element_mut().clear_dirty();
            }
        }
        self.rebuild_textured_draws();
        let _ = scene.take_dirty_ids();
        self.structure_epoch = scene.structure_epoch();

        VertexSync {
            regenerated: self.order.len(),
            writes: Vec::new(),
            geometry_unchanged: false,
            full_upload: true,
        }
    }

    fn update_dirty(&mut self, scene: &mut Scene, mut ids: Vec<ElementId>) -> VertexSync {
        ids.sort_unstable();
        ids.dedup();

        let mut writes = Vec::new();
        let mut regenerated = 0;
        for id in ids {
            let Some(element) = scene.get(id) else {
                continue;
            };
            if !element.as_element().is_dirty() {
                continue;
            }
            if !self.slots.contains_key(&id) {
                return self.repack(scene);
            }
            let mesh = mesh_for(scene, element);
            if let Some(element) = scene.get_mut(id) {
                element.as_element_mut().clear_dirty();
            }
            regenerated += 1;
            if self.write_mesh(id, mesh, &mut writes) {
                return self.repack(scene);
            }
        }

        VertexSync {
            regenerated,
            writes,
            geometry_unchanged: regenerated == 0,
            full_upload: false,
        }
    }

    fn append_slot(&mut self, id: ElementId, mesh: Mesh) {
        let slot = Slot {
            fill_offset: self.fill.len() as u32,
            fill_count: mesh.fill.len() as u32,
            stroke_offset: self.stroke.len() as u32,
            stroke_count: mesh.stroke.len() as u32,
            tex_offset: self.textured.len() as u32,
            tex_count: mesh.textured.len() as u32,
            texture_id: mesh.texture_id,
            underlay: mesh.underlay,
            blend: mesh.blend,
            blurred: mesh.blurred,
        };
        self.fill.extend(mesh.fill);
        self.stroke.extend(mesh.stroke);
        self.textured.extend(mesh.textured);
        self.slots.insert(id, slot);
    }

    /// 把一个元素写回原槽位。顶点数量变了且不在缓冲末尾时返回 true，调用方整表重排。
    fn write_mesh(&mut self, id: ElementId, mesh: Mesh, writes: &mut Vec<GpuVertexWrite>) -> bool {
        let Some(mut slot) = self.slots.get(&id).cloned() else {
            return true;
        };
        let repack = store_vertices(
            &mut self.fill,
            &mut slot.fill_offset,
            &mut slot.fill_count,
            &mesh.fill,
            VertexBufferKind::Fill,
            writes,
        ) || store_vertices(
            &mut self.stroke,
            &mut slot.stroke_offset,
            &mut slot.stroke_count,
            &mesh.stroke,
            VertexBufferKind::Stroke,
            writes,
        ) || store_vertices(
            &mut self.textured,
            &mut slot.tex_offset,
            &mut slot.tex_count,
            &mesh.textured,
            VertexBufferKind::Textured,
            writes,
        );
        if repack {
            return true;
        }
        let texture_changed = slot.texture_id != mesh.texture_id
            || slot.underlay != mesh.underlay
            || slot.blurred != mesh.blurred;
        slot.texture_id = mesh.texture_id;
        slot.underlay = mesh.underlay;
        slot.blend = mesh.blend;
        slot.blurred = mesh.blurred;
        self.slots.insert(id, slot);
        if texture_changed
            || writes
                .iter()
                .any(|write| write.kind == VertexBufferKind::Textured)
        {
            self.rebuild_textured_draws();
        }
        false
    }

    fn rebuild_textured_draws(&mut self) {
        self.textured_draws.clear();
        for id in &self.order {
            let Some(slot) = self.slots.get(id) else {
                continue;
            };
            if slot.tex_count == 0 {
                continue;
            }
            let Some(texture_id) = slot.texture_id.clone() else {
                continue;
            };
            if let Some(last) = self.textured_draws.last_mut() {
                let adjacent = last.vertex_offset + last.vertex_count == slot.tex_offset;
                if last.texture_id == texture_id
                    && adjacent
                    && last.underlay == slot.underlay
                    && last.blurred == slot.blurred
                {
                    last.vertex_count += slot.tex_count;
                    continue;
                }
            }
            self.textured_draws.push(TexturedDraw {
                texture_id,
                vertex_offset: slot.tex_offset,
                vertex_count: slot.tex_count,
                underlay: slot.underlay,
                blurred: slot.blurred,
            });
        }
    }
}

/// 每帧为所有元素重新生成顶点。
///
/// 这是优化前的 CPU 路径，供基准对比。渲染应使用 [`VertexCache::sync`]。
pub fn rebuild_all_vertices(scene: &Scene) -> RebuiltGeometry {
    let mut order: Vec<(i32, ElementId)> = scene
        .iter()
        .map(|element| (element.as_element().props().z_index, element.id()))
        .collect();
    order.sort_unstable();

    let mut geometry = RebuiltGeometry {
        fill: Vec::new(),
        stroke: Vec::new(),
        textured: Vec::new(),
    };
    for (_, id) in order {
        let Some(element) = scene.get(id) else {
            continue;
        };
        let mesh = mesh_for(scene, element);
        geometry.fill.extend(mesh.fill);
        geometry.stroke.extend(mesh.stroke);
        geometry.textured.extend(mesh.textured);
    }
    geometry
}

/// 组没有自己的三角形。被隐藏的组会让子孙也返回空网格。
fn mesh_for(scene: &Scene, element: &AnyElement) -> Mesh {
    if !scene.is_effectively_visible(element.id()) {
        return Mesh::empty();
    }
    let mut mesh = generate_mesh(element);
    mesh.blend = element.blend_mode();
    mesh
}

fn generate_mesh(element: &AnyElement) -> Mesh {
    match element {
        AnyElement::Rectangle(rect) => {
            // 外投影在填充之前；内阴影在填充之后、描边之前。
            let mut fill = rect.shadow_vertices();
            fill.extend(rect.generate_fill_vertices());
            fill.extend(rect.inner_shadow_vertices());
            // 线段管线宽度固定，线宽变化靠填充缓冲里的三角形带立刻画出来。
            fill.extend(rect.stroke_ribbon_vertices());
            Mesh {
                fill,
                stroke: rect.generate_stroke_vertices(),
                textured: Vec::new(),
                texture_id: None,
                underlay: false,
                blend: BlendMode::Normal,
                blurred: false,
            }
        }
        AnyElement::Circle(circle) => {
            let mut fill = circle.shadow_vertices();
            fill.extend(circle.generate_fill_vertices());
            fill.extend(circle.inner_shadow_vertices());
            fill.extend(circle.stroke_ribbon_vertices());
            Mesh {
                fill,
                stroke: circle.generate_stroke_vertices(),
                textured: Vec::new(),
                texture_id: None,
                underlay: false,
                blend: BlendMode::Normal,
                blurred: false,
            }
        }
        AnyElement::Polygon(polygon) => {
            let mut fill = polygon.shadow_vertices();
            fill.extend(polygon.generate_fill_vertices());
            fill.extend(polygon.inner_shadow_vertices());
            fill.extend(polygon.stroke_ribbon_vertices());
            Mesh {
                fill,
                stroke: polygon.generate_stroke_vertices(),
                textured: Vec::new(),
                texture_id: None,
                underlay: false,
                blend: BlendMode::Normal,
                blurred: false,
            }
        }
        AnyElement::Image(image) => {
            if !image.props.visible {
                return Mesh::empty();
            }
            let blurred = blur_active(image.blur_radius);
            let mut fill = image.shadow_vertices();
            if blurred {
                fill.extend(image.plate_vertices());
            }
            fill.extend(image.inner_shadow_vertices());
            Mesh {
                fill,
                stroke: Vec::new(),
                textured: image.generate_vertices(),
                texture_id: Some(image.texture_id.clone()),
                underlay: !blurred,
                blend: BlendMode::Normal,
                blurred,
            }
        }
        AnyElement::Line(line) => {
            let mut fill = line.shadow_vertices();
            fill.extend(line.generate_vertices());
            fill.extend(line.inner_shadow_vertices());
            Mesh {
                fill,
                stroke: Vec::new(),
                textured: Vec::new(),
                texture_id: None,
                underlay: false,
                blend: BlendMode::Normal,
                blurred: false,
            }
        }
        AnyElement::Path(path) => {
            let mut fill = path.shadow_vertices();
            fill.extend(path.generate_vertices());
            fill.extend(path.inner_shadow_vertices());
            Mesh {
                fill,
                stroke: Vec::new(),
                textured: Vec::new(),
                texture_id: None,
                underlay: false,
                blend: BlendMode::Normal,
                blurred: false,
            }
        }
        AnyElement::Text(text) => {
            let textured = text.generate_vertices();
            let blurred = blur_active(text.blur_radius) && !textured.is_empty();
            Mesh {
                fill: {
                    let mut fill = text.shadow_vertices();
                    fill.extend(text.inner_shadow_vertices());
                    fill
                },
                stroke: Vec::new(),
                textured,
                texture_id: text.texture_id.clone(),
                underlay: false,
                blend: BlendMode::Normal,
                blurred,
            }
        }
        AnyElement::Frame(frame) => {
            let rect = frame.shape_proxy();
            let mut fill = rect.shadow_vertices();
            fill.extend(rect.generate_fill_vertices());
            fill.extend(rect.inner_shadow_vertices());
            fill.extend(rect.stroke_ribbon_vertices());
            Mesh {
                fill,
                stroke: rect.generate_stroke_vertices(),
                textured: Vec::new(),
                texture_id: None,
                underlay: false,
                blend: BlendMode::Normal,
                blurred: false,
            }
        }
        AnyElement::Group(_) => Mesh::empty(),
    }
}

/// 写入一个槽位。返回 true 表示数量增长且不在缓冲末尾，需要整表重排。
fn store_vertices(
    buffer: &mut Vec<Vertex>,
    offset: &mut u32,
    count: &mut u32,
    verts: &[Vertex],
    kind: VertexBufferKind,
    writes: &mut Vec<GpuVertexWrite>,
) -> bool {
    let new_len = verts.len() as u32;
    if new_len == *count {
        if new_len > 0 {
            let start = *offset as usize;
            buffer[start..start + verts.len()].copy_from_slice(verts);
            writes.push(GpuVertexWrite {
                kind,
                vertex_offset: *offset,
                vertex_count: new_len,
            });
        }
        return false;
    }

    if new_len < *count {
        let start = *offset as usize;
        let old = *count as usize;
        buffer[start..start + verts.len()].copy_from_slice(verts);
        for vertex in &mut buffer[start + verts.len()..start + old] {
            *vertex = ZERO_VERTEX;
        }
        let is_tail = *offset + *count == buffer.len() as u32;
        if is_tail {
            buffer.truncate(start + verts.len());
            *count = new_len;
            if new_len > 0 {
                writes.push(GpuVertexWrite {
                    kind,
                    vertex_offset: *offset,
                    vertex_count: new_len,
                });
            }
        } else {
            writes.push(GpuVertexWrite {
                kind,
                vertex_offset: *offset,
                vertex_count: *count,
            });
        }
        return false;
    }

    let is_tail = *offset + *count == buffer.len() as u32;
    if !is_tail {
        return true;
    }
    let start = *offset as usize;
    buffer.truncate(start);
    buffer.extend_from_slice(verts);
    *count = new_len;
    if new_len > 0 {
        writes.push(GpuVertexWrite {
            kind,
            vertex_offset: *offset,
            vertex_count: new_len,
        });
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::transform_handles::{HandleGesture, HandleKind, ResizeHandle, SelectionFrame};
    use crate::{
        Circle, Command, ElementStyle, Image, Line, MoveCommand, Path, Rectangle, Scene, Text,
        TransformElementsCommand, UpdateTextCommand,
    };
    use infinitecanvas_math::{Color, Vec2};

    fn rect_fill(scene: &Scene, id: ElementId) -> Vec<Vertex> {
        match scene.get(id).unwrap() {
            AnyElement::Rectangle(rect) => rect.generate_fill_vertices(),
            _ => panic!("rect"),
        }
    }

    fn sync_clean(scene: &mut Scene) -> VertexCache {
        let mut cache = VertexCache::new();
        let first = cache.sync(scene);
        assert!(first.full_upload);
        assert!(!scene.has_dirty_elements());
        cache
    }

    #[test]
    fn second_sync_does_not_rebuild_vertices() {
        let mut scene = Scene::new();
        scene.add_rect(Rectangle::new(1, Vec2::ZERO, Vec2::new(10.0, 8.0)));
        scene.add_circle(Circle::new(2, Vec2::new(30.0, 30.0), 12.0));
        let mut cache = sync_clean(&mut scene);

        let again = cache.sync(&mut scene);
        assert!(again.geometry_unchanged);
        assert!(again.writes.is_empty());
        assert_eq!(again.regenerated, 0);
        assert!(!again.full_upload);
    }

    #[test]
    fn dirty_element_rewrites_only_its_vertices() {
        let mut scene = Scene::new();
        scene.add_rect(Rectangle::new(
            1,
            Vec2::new(0.0, 0.0),
            Vec2::new(20.0, 10.0),
        ));
        scene.add_rect(Rectangle::new(
            2,
            Vec2::new(40.0, 0.0),
            Vec2::new(20.0, 10.0),
        ));
        let mut cache = sync_clean(&mut scene);
        let untouched = cache.fill_slice(2).unwrap().to_vec();

        scene
            .get_mut(1)
            .unwrap()
            .as_element_mut()
            .props_mut()
            .transform
            .translate(Vec2::new(5.0, 0.0));
        scene.mark_dirty(1);

        let sync = cache.sync(&mut scene);
        assert_eq!(sync.regenerated, 1);
        assert!(!sync.geometry_unchanged);
        assert!(!sync.full_upload);
        assert_eq!(
            sync.writes,
            vec![GpuVertexWrite {
                kind: VertexBufferKind::Fill,
                vertex_offset: 0,
                vertex_count: 6,
            }]
        );
        assert_eq!(cache.fill_slice(2).unwrap(), untouched.as_slice());
        let expected = match scene.get(1).unwrap() {
            AnyElement::Rectangle(rect) => rect.generate_fill_vertices(),
            _ => panic!("rect"),
        };
        assert_eq!(cache.fill_slice(1).unwrap(), expected.as_slice());
        assert!(!scene.get(1).unwrap().as_element().is_dirty());
    }

    #[test]
    fn rotation_and_scale_update_vertices() {
        let mut scene = Scene::new();
        scene.add_rect(Rectangle::new(
            1,
            Vec2::new(10.0, 10.0),
            Vec2::new(8.0, 4.0),
        ));
        let mut cache = sync_clean(&mut scene);
        let before = cache.fill_slice(1).unwrap().to_vec();

        {
            let transform = &mut scene
                .get_mut(1)
                .unwrap()
                .as_element_mut()
                .props_mut()
                .transform;
            transform.rotate(0.4);
            transform.scale_by(Vec2::new(2.0, 0.5));
        }
        scene.mark_dirty(1);

        let sync = cache.sync(&mut scene);
        assert_eq!(sync.regenerated, 1);
        let after = cache.fill_slice(1).unwrap();
        assert_ne!(after, before.as_slice());
        let expected = match scene.get(1).unwrap() {
            AnyElement::Rectangle(rect) => rect.generate_fill_vertices(),
            _ => panic!("rect"),
        };
        assert_eq!(after, expected.as_slice());
    }

    #[test]
    fn moving_a_group_updates_cached_child_vertices() {
        let mut scene = Scene::new();
        scene.add_rect(Rectangle::new(1, Vec2::ZERO, Vec2::new(10.0, 10.0)));
        scene.add_rect(Rectangle::new(
            2,
            Vec2::new(20.0, 0.0),
            Vec2::new(10.0, 10.0),
        ));
        let group_id = scene.group(&[1, 2]).unwrap();
        let mut cache = sync_clean(&mut scene);
        let before_left = cache.fill_slice(1).unwrap().to_vec();
        let before_right = cache.fill_slice(2).unwrap().to_vec();

        scene
            .translate_element(group_id, Vec2::new(5.0, -2.0))
            .unwrap();
        let sync = cache.sync(&mut scene);
        assert!(sync.regenerated >= 2);
        assert!(!sync.geometry_unchanged);
        assert_ne!(cache.fill_slice(1).unwrap(), before_left.as_slice());
        assert_ne!(cache.fill_slice(2).unwrap(), before_right.as_slice());
        assert_eq!(
            cache.fill_slice(1).unwrap(),
            rect_fill(&scene, 1).as_slice()
        );
        assert_eq!(
            cache.fill_slice(2).unwrap(),
            rect_fill(&scene, 2).as_slice()
        );
        assert!(cache.fill_slice(group_id).unwrap().is_empty());
    }

    #[test]
    fn hiding_a_group_clears_cached_child_vertices() {
        let mut scene = Scene::new();
        scene.add_rect(Rectangle::new(1, Vec2::ZERO, Vec2::new(10.0, 10.0)));
        scene.add_rect(Rectangle::new(
            2,
            Vec2::new(20.0, 0.0),
            Vec2::new(10.0, 10.0),
        ));
        let group_id = scene.group(&[1, 2]).unwrap();
        let mut cache = sync_clean(&mut scene);
        assert!(!cache.fill_slice(1).unwrap().is_empty());

        assert!(scene.set_visible(group_id, false));
        let _ = cache.sync(&mut scene);
        // 非尾部槽位会把顶点清零并保留长度，避免为一次显隐整表重排。
        assert_ne!(
            cache.fill_slice(1).unwrap(),
            rect_fill(&scene, 1).as_slice()
        );
        assert!(cache
            .fill_slice(1)
            .unwrap()
            .iter()
            .all(|vertex| vertex.position == [0.0, 0.0]));
        assert!(cache
            .fill_slice(2)
            .unwrap()
            .iter()
            .all(|vertex| vertex.position == [0.0, 0.0]));

        assert!(scene.set_visible(group_id, true));
        let _ = cache.sync(&mut scene);
        assert_eq!(
            cache.fill_slice(1).unwrap(),
            rect_fill(&scene, 1).as_slice()
        );
    }

    #[test]
    fn handle_rotate_and_scale_regenerate_vertices() {
        let mut scene = Scene::new();
        scene.add_rect(Rectangle::new(
            1,
            Vec2::new(100.0, 100.0),
            Vec2::new(80.0, 40.0),
        ));
        let mut cache = sync_clean(&mut scene);
        let before = cache.fill_slice(1).unwrap().to_vec();

        let frame = SelectionFrame::from_element(scene.get(1).unwrap().as_element());
        let originals = vec![(1, scene.get(1).unwrap().as_element().props().transform)];
        let resize = HandleGesture::start(
            frame,
            HandleKind::Resize(ResizeHandle::East),
            Vec2::new(140.0, 100.0),
            originals.clone(),
        );
        let scaled = resize.preview(Vec2::new(180.0, 100.0), false, 1.0);
        let mut scale_cmd = TransformElementsCommand::new(originals, scaled.transforms);
        scale_cmd.execute(&mut scene);

        let sync = cache.sync(&mut scene);
        assert_eq!(sync.regenerated, 1);
        assert!(!sync.full_upload);
        assert!(!sync.geometry_unchanged);
        let scaled_verts = cache.fill_slice(1).unwrap().to_vec();
        assert_ne!(scaled_verts, before);
        assert_eq!(scaled_verts, rect_fill(&scene, 1));

        let frame = SelectionFrame::from_element(scene.get(1).unwrap().as_element());
        let before_rotate = vec![(1, scene.get(1).unwrap().as_element().props().transform)];
        let rotate = HandleGesture::start(
            frame,
            HandleKind::Rotate,
            frame.center + Vec2::new(100.0, 0.0),
            before_rotate.clone(),
        );
        let rotated = rotate.preview(frame.center + Vec2::new(0.0, 100.0), false, 1.0);
        let mut rotate_cmd = TransformElementsCommand::new(before_rotate, rotated.transforms);
        rotate_cmd.execute(&mut scene);

        let sync = cache.sync(&mut scene);
        assert_eq!(sync.regenerated, 1);
        let rotated_verts = cache.fill_slice(1).unwrap().to_vec();
        assert_ne!(rotated_verts, scaled_verts);
        assert_eq!(rotated_verts, rect_fill(&scene, 1));
        assert!(
            (scene.get(1).unwrap().as_element().rotation() - std::f32::consts::FRAC_PI_2).abs()
                < 1e-3
        );

        rotate_cmd.undo(&mut scene);
        let sync = cache.sync(&mut scene);
        assert_eq!(sync.regenerated, 1);
        assert_eq!(cache.fill_slice(1).unwrap(), scaled_verts.as_slice());

        rotate_cmd.execute(&mut scene);
        let sync = cache.sync(&mut scene);
        assert_eq!(sync.regenerated, 1);
        assert_eq!(cache.fill_slice(1).unwrap(), rotated_verts.as_slice());

        rotate_cmd.undo(&mut scene);
        scale_cmd.undo(&mut scene);
        let sync = cache.sync(&mut scene);
        assert_eq!(sync.regenerated, 1);
        assert_eq!(cache.fill_slice(1).unwrap(), before.as_slice());
    }

    #[test]
    fn multi_select_handle_scale_updates_both_vertices() {
        let mut scene = Scene::new();
        scene.add_rect(Rectangle::new(1, Vec2::ZERO, Vec2::new(20.0, 20.0)));
        scene.add_rect(Rectangle::new(
            2,
            Vec2::new(40.0, 0.0),
            Vec2::new(20.0, 20.0),
        ));
        scene.add_rect(Rectangle::new(
            3,
            Vec2::new(200.0, 0.0),
            Vec2::new(10.0, 10.0),
        ));
        let mut cache = sync_clean(&mut scene);
        let before1 = cache.fill_slice(1).unwrap().to_vec();
        let before2 = cache.fill_slice(2).unwrap().to_vec();
        let untouched = cache.fill_slice(3).unwrap().to_vec();

        let originals = vec![
            (1, scene.get(1).unwrap().as_element().props().transform),
            (2, scene.get(2).unwrap().as_element().props().transform),
        ];
        let frame = {
            let left = scene.get(1).unwrap().as_element();
            let right = scene.get(2).unwrap().as_element();
            SelectionFrame::from_elements(&[left, right]).unwrap()
        };
        let east = frame
            .handles(10.0)
            .into_iter()
            .find(|handle| handle.kind == HandleKind::Resize(ResizeHandle::East))
            .unwrap();
        let gesture = HandleGesture::start(
            frame,
            HandleKind::Resize(ResizeHandle::East),
            east.position,
            originals.clone(),
        );
        let preview = gesture.preview(east.position + Vec2::new(60.0, 0.0), false, 1.0);
        assert_eq!(preview.transforms.len(), 2);

        let mut command = TransformElementsCommand::new(originals, preview.transforms);
        command.execute(&mut scene);
        let sync = cache.sync(&mut scene);
        assert_eq!(sync.regenerated, 2);
        assert!(!sync.full_upload);
        assert_ne!(cache.fill_slice(1).unwrap(), before1.as_slice());
        assert_ne!(cache.fill_slice(2).unwrap(), before2.as_slice());
        assert_eq!(
            cache.fill_slice(1).unwrap(),
            rect_fill(&scene, 1).as_slice()
        );
        assert_eq!(
            cache.fill_slice(2).unwrap(),
            rect_fill(&scene, 2).as_slice()
        );
        assert_eq!(cache.fill_slice(3).unwrap(), untouched.as_slice());

        command.undo(&mut scene);
        let sync = cache.sync(&mut scene);
        assert_eq!(sync.regenerated, 2);
        assert_eq!(cache.fill_slice(1).unwrap(), before1.as_slice());
        assert_eq!(cache.fill_slice(2).unwrap(), before2.as_slice());
        assert_eq!(cache.fill_slice(3).unwrap(), untouched.as_slice());
    }

    #[test]
    fn line_endpoint_and_arrow_update_vertices() {
        let mut scene = Scene::new();
        scene.add_rect(Rectangle::new(1, Vec2::ZERO, Vec2::new(10.0, 10.0)));
        let style = ElementStyle::stroke_only(Color::BLACK, 4.0);
        scene.add_line(Line::from_world(
            2,
            Vec2::new(0.0, 20.0),
            Vec2::new(40.0, 20.0),
            style,
            false,
        ));
        let mut cache = sync_clean(&mut scene);
        let rect_before = cache.fill_slice(1).unwrap().to_vec();

        if let Some(AnyElement::Line(line)) = scene.get_mut(2) {
            line.set_world_end(Vec2::new(80.0, 50.0));
        }
        scene.mark_dirty(2);
        let sync = cache.sync(&mut scene);
        assert_eq!(sync.regenerated, 1);
        assert_eq!(cache.fill_slice(1).unwrap(), rect_before.as_slice());
        let expected = match scene.get(2).unwrap() {
            AnyElement::Line(line) => line.generate_vertices(),
            _ => panic!("line"),
        };
        assert_eq!(cache.fill_slice(2).unwrap(), expected.as_slice());
        assert!(!expected.is_empty());

        if let Some(AnyElement::Line(line)) = scene.get_mut(2) {
            line.set_arrow(true);
        }
        scene.mark_dirty(2);
        let sync = cache.sync(&mut scene);
        assert_eq!(sync.regenerated, 1);
        let expected = match scene.get(2).unwrap() {
            AnyElement::Line(line) => line.generate_vertices(),
            _ => panic!("line"),
        };
        assert_eq!(cache.fill_slice(2).unwrap(), expected.as_slice());
        assert!(expected.len() > 6);
        assert_eq!(cache.fill_slice(1).unwrap(), rect_before.as_slice());
    }

    #[test]
    fn path_point_edit_updates_only_that_path() {
        let mut scene = Scene::new();
        scene.add_rect(Rectangle::new(1, Vec2::ZERO, Vec2::new(12.0, 12.0)));
        let style = ElementStyle::stroke_only(Color::BLACK, 3.0);
        scene.add_path(Path::from_world_points(
            2,
            &[Vec2::new(0.0, 30.0), Vec2::new(20.0, 30.0)],
            style,
        ));
        let mut cache = sync_clean(&mut scene);
        let rect_before = cache.fill_slice(1).unwrap().to_vec();
        let path_before = cache.fill_slice(2).unwrap().to_vec();

        if let Some(AnyElement::Path(path)) = scene.get_mut(2) {
            path.set_world_points(&[
                Vec2::new(0.0, 30.0),
                Vec2::new(20.0, 40.0),
                Vec2::new(40.0, 10.0),
            ]);
        }
        scene.mark_dirty(2);

        let sync = cache.sync(&mut scene);
        assert_eq!(sync.regenerated, 1);
        assert!(!sync.full_upload);
        assert_eq!(cache.fill_slice(1).unwrap(), rect_before.as_slice());
        let updated = cache.fill_slice(2).unwrap();
        assert_ne!(updated, path_before.as_slice());
        let expected = match scene.get(2).unwrap() {
            AnyElement::Path(path) => path.generate_vertices(),
            _ => panic!("path"),
        };
        assert_eq!(updated, expected.as_slice());
    }

    #[test]
    fn move_command_marks_element_dirty() {
        let mut scene = Scene::new();
        scene.add_rect(Rectangle::new(1, Vec2::ZERO, Vec2::new(10.0, 10.0)));
        let mut cache = sync_clean(&mut scene);
        let mut command = MoveCommand::new(vec![1], Vec2::new(3.0, -2.0));
        command.execute(&mut scene);
        assert!(scene.has_dirty_elements());
        let sync = cache.sync(&mut scene);
        assert_eq!(sync.regenerated, 1);
        let expected = match scene.get(1).unwrap() {
            AnyElement::Rectangle(rect) => rect.generate_fill_vertices(),
            _ => panic!("rect"),
        };
        assert_eq!(cache.fill_slice(1).unwrap(), expected.as_slice());
    }

    #[test]
    fn image_alpha_change_updates_textured_vertices() {
        let mut scene = Scene::new();
        scene.add_image(Image::new(
            1,
            Vec2::ZERO,
            Vec2::new(32.0, 16.0),
            "logo".to_string(),
        ));
        let mut cache = sync_clean(&mut scene);
        assert_eq!(cache.textured().len(), 6);

        if let Some(AnyElement::Image(image)) = scene.get_mut(1) {
            image.set_alpha(0.25);
        }
        scene.mark_dirty(1);
        let sync = cache.sync(&mut scene);
        assert_eq!(sync.regenerated, 1);
        assert_eq!(sync.writes.len(), 1);
        assert_eq!(sync.writes[0].kind, VertexBufferKind::Textured);
        let expected = match scene.get(1).unwrap() {
            AnyElement::Image(image) => image.generate_vertices(),
            _ => panic!("image"),
        };
        assert_eq!(cache.textured(), expected.as_slice());
    }

    #[test]
    fn moving_one_image_does_not_rebuild_the_other() {
        let mut scene = Scene::new();
        scene.add_image(Image::new(
            1,
            Vec2::ZERO,
            Vec2::new(20.0, 10.0),
            "a".to_string(),
        ));
        scene.add_image(Image::new(
            2,
            Vec2::new(40.0, 0.0),
            Vec2::new(20.0, 10.0),
            "b".to_string(),
        ));
        let mut cache = sync_clean(&mut scene);
        let untouched = cache.textured_slice(2).unwrap().to_vec();
        let mut command = MoveCommand::new(vec![1], Vec2::new(5.0, 0.0));
        command.execute(&mut scene);
        let sync = cache.sync(&mut scene);
        assert_eq!(sync.regenerated, 1);
        assert!(!sync.full_upload);
        assert_eq!(cache.textured_slice(2).unwrap(), untouched.as_slice());
        let expected = match scene.get(1).unwrap() {
            AnyElement::Image(image) => image.generate_vertices(),
            _ => panic!("image"),
        };
        assert_eq!(cache.textured_slice(1).unwrap(), expected.as_slice());
    }

    #[test]
    fn removing_an_element_drops_its_vertices() {
        let mut scene = Scene::new();
        scene.add_rect(Rectangle::new(1, Vec2::ZERO, Vec2::new(10.0, 10.0)));
        scene.add_rect(Rectangle::new(
            2,
            Vec2::new(30.0, 0.0),
            Vec2::new(10.0, 10.0),
        ));
        let mut cache = sync_clean(&mut scene);
        scene.remove(1);
        let sync = cache.sync(&mut scene);
        assert!(sync.full_upload);
        assert!(cache.fill_slice(1).is_none());
        assert_eq!(cache.fill().len(), 6);
        let again = cache.sync(&mut scene);
        assert!(again.geometry_unchanged);
    }

    #[test]
    fn many_elements_second_frame_is_uniform_only() {
        let mut scene = Scene::new();
        for id in 1..=2_000 {
            scene.add_rect(Rectangle::new(
                id,
                Vec2::new((id % 50) as f32 * 20.0, (id / 50) as f32 * 20.0),
                Vec2::new(12.0, 8.0),
            ));
        }
        let mut cache = VertexCache::new();
        let first = cache.sync(&mut scene);
        assert_eq!(first.regenerated, 2_000);
        assert!(first.full_upload);
        assert_eq!(cache.fill().len(), 2_000 * 6);

        let second = cache.sync(&mut scene);
        assert!(second.geometry_unchanged);
        assert_eq!(second.regenerated, 0);
        assert!(second.writes.is_empty());
    }

    #[test]
    fn import_json_rebuilds_vertices_and_omits_dirty() {
        let mut live = Scene::new();
        live.add_rect(Rectangle::new(1, Vec2::ZERO, Vec2::new(10.0, 10.0)));
        let mut cache = sync_clean(&mut live);
        assert_eq!(cache.fill().len(), 6);

        let empty_json = Scene::new().to_json().unwrap();
        assert!(!empty_json.contains("\"dirty\""));
        let mut empty = Scene::from_json(&empty_json).unwrap();
        assert_eq!(empty.structure_epoch(), live.structure_epoch());
        let stale = cache.sync(&mut empty);
        assert!(
            stale.geometry_unchanged,
            "same structure epoch and an empty dirty list would keep deleted vertices"
        );
        assert!(!cache.fill().is_empty());

        cache.invalidate();
        let mut empty = Scene::from_json(&empty_json).unwrap();
        let cleared = cache.sync(&mut empty);
        assert!(cleared.full_upload);
        assert!(cache.fill().is_empty());
        assert_eq!(cleared.regenerated, 0);

        let mut saved = Scene::new();
        let id = saved.next_id();
        saved.add_rect(Rectangle::new(
            id,
            Vec2::new(12.0, 3.0),
            Vec2::new(8.0, 6.0),
        ));
        saved
            .get_mut(id)
            .unwrap()
            .as_element_mut()
            .props_mut()
            .transform
            .rotate(0.4);
        let _ = VertexCache::new().sync(&mut saved);
        assert!(!saved.get(id).unwrap().as_element().is_dirty());

        let json = saved.to_json().unwrap();
        assert!(!json.contains("\"dirty\""));
        let mut value: serde_json::Value = serde_json::from_str(&json).unwrap();
        value["elements"][0]["data"]["props"]["dirty"] = serde_json::json!(false);
        let mut restored = Scene::from_json(&value.to_string()).unwrap();
        assert!(restored.get(id).unwrap().as_element().is_dirty());
        assert_eq!(restored, saved);

        cache.invalidate();
        let sync = cache.sync(&mut restored);
        assert!(sync.full_upload);
        assert_eq!(sync.regenerated, 1);
        let expected = match restored.get(id).unwrap() {
            AnyElement::Rectangle(rect) => rect.generate_fill_vertices(),
            _ => panic!("rect"),
        };
        assert_eq!(cache.fill_slice(id).unwrap(), expected.as_slice());
        assert!(!expected.is_empty());
        assert!(!restored.has_dirty_elements());

        let again = cache.sync(&mut restored);
        assert!(again.geometry_unchanged);
    }

    #[test]
    fn text_change_updates_vertices() {
        let mut scene = Scene::new();
        let mut text = Text::new(1, Vec2::new(10.0, 20.0), "A").with_font_size(32.0);
        text.commit_raster(text.revision, 0, "tex-a".into(), Vec2::new(30.0, 12.0))
            .unwrap();
        scene.add_text(text);

        let mut cache = VertexCache::new();
        let first = cache.sync(&mut scene);
        assert!(first.full_upload);
        let before = cache.textured().to_vec();
        assert_eq!(before.len(), 6);
        assert!(!cache.textured_draws().is_empty());
        assert!(!cache.textured_draws()[0].underlay);
        assert!(!scene.has_dirty_elements());

        let mut edited = match scene.get(1).unwrap() {
            AnyElement::Text(text) => text.clone(),
            _ => panic!("text"),
        };
        let before_visual = edited.visual();
        assert!(edited.set_content("Hello").is_some());
        let after_visual = edited.visual();
        let mut command = UpdateTextCommand::new(1, before_visual, after_visual);
        command.execute(&mut scene);
        assert!(scene.has_dirty_elements());

        let sync = cache.sync(&mut scene);
        assert_eq!(sync.regenerated, 1);
        assert!(!sync.geometry_unchanged);
        assert!(cache.textured().is_empty());
        assert_ne!(cache.textured(), before.as_slice());

        {
            let Some(AnyElement::Text(text)) = scene.get_mut(1) else {
                panic!("text");
            };
            text.commit_raster(text.revision, 0, "tex-b".into(), Vec2::new(80.0, 24.0))
                .unwrap();
        }
        scene.mark_dirty(1);
        let sync = cache.sync(&mut scene);
        assert_eq!(sync.regenerated, 1);
        let expected = match scene.get(1).unwrap() {
            AnyElement::Text(text) => text.generate_vertices(),
            _ => panic!("text"),
        };
        assert_eq!(cache.textured(), expected.as_slice());
        assert_ne!(cache.textured(), before.as_slice());
        assert_eq!(expected.len(), 6);

        command.undo(&mut scene);
        assert!(scene.has_dirty_elements());
        let sync = cache.sync(&mut scene);
        assert_eq!(sync.regenerated, 1);
        assert!(cache.textured().is_empty());
        match scene.get(1).unwrap() {
            AnyElement::Text(text) => assert_eq!(text.content, "A"),
            _ => panic!("text"),
        }

        command.execute(&mut scene);
        cache.sync(&mut scene);
        match scene.get(1).unwrap() {
            AnyElement::Text(text) => {
                assert_eq!(text.content, "Hello");
                assert!(text.texture_id.is_none());
            }
            _ => panic!("text"),
        }
        assert!(cache.textured().is_empty());
    }

    #[test]
    fn paint_spans_follow_z_order_and_skip_hidden_leaves() {
        let mut scene = Scene::new();
        let mut back = Rectangle::new(1, Vec2::ZERO, Vec2::new(10.0, 10.0));
        back.props.z_index = 2;
        back.style.blend_mode = BlendMode::Screen;
        scene.add_rect(back);
        let mut front = Rectangle::new(2, Vec2::new(4.0, 0.0), Vec2::new(10.0, 10.0));
        front.props.z_index = 0;
        front.style.blend_mode = BlendMode::Multiply;
        scene.add_rect(front);
        let mut hidden = Rectangle::new(3, Vec2::new(20.0, 0.0), Vec2::new(10.0, 10.0));
        hidden.props.visible = false;
        hidden.style.blend_mode = BlendMode::Overlay;
        scene.add_rect(hidden);

        let mut cache = VertexCache::new();
        cache.sync(&mut scene);
        let spans = cache.paint_spans();
        assert_eq!(spans.len(), 2);
        assert_eq!(spans[0].blend, BlendMode::Multiply);
        assert_eq!(spans[1].blend, BlendMode::Screen);
        assert!(cache.needs_blend_composite());

        match scene.get_mut(3).unwrap() {
            AnyElement::Rectangle(rect) => rect.props.visible = true,
            _ => panic!("rect"),
        }
        scene.mark_dirty(3);
        cache.sync(&mut scene);
        assert_eq!(cache.paint_spans().len(), 3);
    }
}
