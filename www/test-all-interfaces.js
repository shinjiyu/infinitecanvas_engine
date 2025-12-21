import init, { InfiniteCanvas } from '../packages/core/wasm/infinitecanvas_wasm.js';

const statusEl = (id) => document.getElementById(id);
const setStatus = (id, text) => {
  const el = statusEl(id);
  if (el) el.textContent = text;
};

const logContainer = document.getElementById('action-log');
const writeLog = (msg) => {
  const entry = document.createElement('div');
  entry.className = 'log-entry';
  entry.textContent = `${new Date().toLocaleTimeString()} ${msg}`;
  logContainer.prepend(entry);
  while (logContainer.children.length > 60) {
    logContainer.removeChild(logContainer.lastChild);
  }
};

const tests = [
  { id: 'init', name: '引擎初始化', desc: '初始化 WASM + WebGPU', run: async (ctx) => ctx.initEngine() },
  { id: 'rect', name: '添加矩形', desc: '验证 add_rectangle 接口', run: (ctx) => ctx.addSampleRectangle() },
  { id: 'circle', name: '添加圆形', desc: '验证 add_circle 接口', run: (ctx) => ctx.addSampleCircle() },
  {
    id: 'texture',
    name: '上传纹理',
    desc: 'upload_texture + 数据校验',
    run: (ctx) => ctx.loadSampleTexture(),
  },
  {
    id: 'image',
    name: '添加图片',
    desc: 'add_image + set_image_alpha',
    run: (ctx) => ctx.addSampleImage(),
  },
  {
    id: 'alpha',
    name: '图片透明度',
    desc: 'set_image_alpha(0.6)',
    run: (ctx) => ctx.updateImageAlpha(0.6),
  },
  {
    id: 'pan',
    name: '视口平移',
    desc: 'pan(40, -30)',
    run: (ctx) => ctx.panViewport(40, -30),
  },
  {
    id: 'zoom',
    name: '视口缩放',
    desc: 'zoom_at(1.15)',
    run: (ctx) => ctx.zoomViewport(1.15),
  },
  {
    id: 'select',
    name: '元素选择',
    desc: 'select_element + 状态同步',
    run: (ctx) => ctx.selectFirstElement(),
  },
  {
    id: 'hit',
    name: '命中测试',
    desc: 'hit_test_screen_point()',
    run: (ctx) => ctx.runHitTestDemo(),
  },
  {
    id: 'render',
    name: '渲染验证',
    desc: '调用 render() 3 次',
    run: (ctx) => ctx.renderMultiple(3),
  },
];

const renderTestRows = () => {
  const list = document.getElementById('test-list');
  tests.forEach((test) => {
    const row = document.createElement('div');
    row.className = 'test-item';
    row.innerHTML = `
      <div class="test-name">${test.name}</div>
      <div class="test-desc">${test.desc}</div>
      <div class="test-status status-wait" id="test-${test.id}-status">等待</div>
    `;
    list.appendChild(row);
  });
};

class TestHarness {
  constructor() {
    this.canvasEl = document.getElementById('test-canvas');
    this.wasm = null;
    this.textureId = null;
    this.imageId = null;
    this.elements = [];
    this.inputCleanup = [];
    this.renderScheduled = false;
  }

  async initEngine() {
    await init();
    this.wasm = new InfiniteCanvas();
    await this.wasm.init(this.canvasEl);
    this._bindInputEvents();
    setStatus('engine-status', '✓ 就绪');
    setStatus('wasm-status', '✓ 已加载');
    writeLog('引擎初始化完成');
    this.renderOnce();
  }

  renderOnce() {
    if (!this.wasm) return;
    const pendingEvents = this.wasm.debug_pending_event_count
      ? this.wasm.debug_pending_event_count()
      : 0;
    if (pendingEvents > 0) {
      writeLog(`处理输入事件 ${pendingEvents} 条`);
    }
    this.wasm.render();
    setStatus('stat-elements', this.wasm.get_element_count());
    setStatus('stat-textures', this.textureId ? '1' : '0');
    setStatus('stat-zoom', this.wasm.get_zoom().toFixed(2));
    setStatus(
      'stat-center',
      `(${this.wasm.get_viewport_center_x().toFixed(1)}, ${this.wasm
        .get_viewport_center_y()
        .toFixed(1)})`,
    );
    setStatus('stat-events', pendingEvents.toString());
    this._updateSelectionStatus();
  }

  addSampleRectangle() {
    const id = this.wasm.add_rectangle(160, 140, 180, 120, '#60a5fa', '#1f2937', 3);
    this.elements.push(id);
    writeLog(`添加矩形 #${id}`);
    this.renderOnce();
  }

  addSampleCircle() {
    const id = this.wasm.add_circle(360, 300, 70, '#f97316', '#111827', 4);
    this.elements.push(id);
    writeLog(`添加圆形 #${id}`);
    this.renderOnce();
  }

  async loadSampleTexture() {
    const imageData = await this._loadImageData('./woman.jpeg');
    const texId = 'sample-texture';
    this.wasm.upload_texture(texId, imageData.width, imageData.height, imageData.data);
    this.textureId = texId;
    setStatus('texture-status', `✓ ${imageData.width}×${imageData.height}`);
    writeLog(`上传纹理 ${imageData.width}x${imageData.height}`);
    this.renderOnce();
  }

  addSampleImage() {
    if (!this.textureId) throw new Error('纹理未加载');
    const size = 240;
    const x = (this.canvasEl.width - size) / 2;
    const y = (this.canvasEl.height - size) / 2;
    this.imageId = this.wasm.add_image(x, y, size, size, this.textureId);
    this.elements.push(this.imageId);
    writeLog(`添加图片元素 #${this.imageId}`);
    this.renderOnce();
  }

  updateImageAlpha(alpha) {
    if (!this.imageId) throw new Error('图片元素不存在');
    this.wasm.set_image_alpha(this.imageId, alpha);
    writeLog(`设置图片透明度 ${alpha.toFixed(2)}`);
    this.renderOnce();
  }

  panViewport(dx, dy) {
    this.wasm.pan(dx, dy);
    writeLog(`视口平移 (${dx}, ${dy})`);
    this.renderOnce();
  }

  zoomViewport(factor) {
    this.wasm.zoom_at(this.canvasEl.width / 2, this.canvasEl.height / 2, factor);
    writeLog(`视口缩放 ${factor.toFixed(2)}`);
    this.renderOnce();
  }

  selectFirstElement() {
    if (!this.wasm || !this.elements.length) {
      throw new Error('没有可选元素');
    }
    const targetId = this.elements[0];
    const success = this.wasm.select_element(targetId, false);
    if (!success) {
      throw new Error(`无法选中元素 #${targetId}`);
    }
    const count = this._updateSelectionStatus();
    writeLog(`选中元素 #${targetId} (总数 ${count})`);
    this.renderOnce();
    return success;
  }

  clearSelection() {
    if (!this.wasm) return;
    if (this.wasm.clear_selection) {
      this.wasm.clear_selection();
    } else if (this.wasm.clearSelection) {
      this.wasm.clearSelection();
    }
    this._updateSelectionStatus();
    writeLog('已清空选区');
    this.renderOnce();
  }

  runHitTestDemo() {
    if (!this.wasm) {
      throw new Error('WASM 未初始化');
    }
    const hitsArray = this.wasm.hit_test_screen_point(
      this.canvasEl.width / 2,
      this.canvasEl.height / 2,
      5,
    );
    const hits = Array.from(hitsArray ?? []).map((hit) => ({
      id: hit.id,
      type: hit.elementType,
      zIndex: hit.zIndex,
    }));
    if (!hits.length) {
      throw new Error('中心未命中任何元素');
    }
    writeLog(`命中测试: ${hits.map((hit) => `#${hit.id}(${hit.type})`).join(', ')}`);
    setStatus('render-status', `命中 ${hits.length} 个元素`);
    return hits;
  }

  renderMultiple(times) {
    for (let i = 0; i < times; i += 1) {
      this.wasm.render();
    }
    writeLog(`渲染 ${times} 帧完成`);
    setStatus('render-status', `已渲染 ${times} 帧`);
    this.renderOnce();
  }

  _bindInputEvents() {
    if (!this.canvasEl || !this.wasm) return;
    this._unbindInputEvents();
    const register = (target, type, handler, options) => {
      target.addEventListener(type, handler, options);
      this.inputCleanup.push(() => target.removeEventListener(type, handler, options));
    };

    const pointerHandler = (type) => (event) => {
      if (!this.wasm) return;
      event.preventDefault();
      const { x, y, mx, my } = this._relativePointerPosition(event);
      const button = typeof event.button === 'number' ? event.button : -1;
      const buttonsMask = typeof event.buttons === 'number' ? event.buttons : 0;
      this.wasm.handle_pointer_event(
        type,
        typeof event.pointerId === 'number' ? event.pointerId : 0,
        x,
        y,
        mx,
        my,
        button,
        buttonsMask,
        event.shiftKey,
        event.ctrlKey,
        event.altKey,
        event.metaKey,
      );
      if (type === 'down') {
        const target = event.target;
        if (target && typeof target.setPointerCapture === 'function') {
          try {
            target.setPointerCapture(event.pointerId);
          } catch (_) {
            // ignore capture errors
          }
        }
      }
      this._scheduleRender();
    };

    register(this.canvasEl, 'pointerdown', pointerHandler('down'));
    register(this.canvasEl, 'pointermove', pointerHandler('move'));
    register(this.canvasEl, 'pointerup', pointerHandler('up'));
    register(this.canvasEl, 'pointercancel', pointerHandler('up'));

    const wheelHandler = (event) => {
      if (!this.wasm) return;
      event.preventDefault();
      this.wasm.handle_wheel_event(
        event.deltaX || 0,
        event.deltaY || 0,
        typeof event.deltaMode === 'number' ? event.deltaMode : 0,
        event.shiftKey,
        event.ctrlKey,
        event.altKey,
        event.metaKey,
      );
      this._scheduleRender();
    };
    register(this.canvasEl, 'wheel', wheelHandler, { passive: false });

    const keyboardHandler = (type) => (event) => {
      if (!this.wasm) return;
      if (this._shouldIgnoreKeyboardEvent(event)) return;
      this.wasm.handle_keyboard_event(
        type,
        event.key || '',
        event.code || '',
        !!event.repeat,
        event.shiftKey,
        event.ctrlKey,
        event.altKey,
        event.metaKey,
      );
      this._scheduleRender();
    };

    register(window, 'keydown', keyboardHandler('down'), true);
    register(window, 'keyup', keyboardHandler('up'), true);

    writeLog('输入事件监听已绑定');
  }

  _unbindInputEvents() {
    if (!this.inputCleanup.length) return;
    this.inputCleanup.forEach((dispose) => dispose());
    this.inputCleanup = [];
  }

  _scheduleRender() {
    if (this.renderScheduled) return;
    this.renderScheduled = true;
    requestAnimationFrame(() => {
      this.renderScheduled = false;
      this.renderOnce();
    });
  }

  _updateSelectionStatus() {
    if (!this.wasm || typeof this.wasm.get_selection_count !== 'function') {
      setStatus('stat-selection', '0');
      return 0;
    }
    const count = this.wasm.get_selection_count();
    setStatus('stat-selection', count.toString());
    return count;
  }

  _relativePointerPosition(event) {
    const rect = this.canvasEl.getBoundingClientRect();
    const scaleX = rect.width ? this.canvasEl.width / rect.width : 1;
    const scaleY = rect.height ? this.canvasEl.height / rect.height : 1;
    const clientX = event.clientX ?? 0;
    const clientY = event.clientY ?? 0;
    return {
      x: (clientX - rect.left) * scaleX,
      y: (clientY - rect.top) * scaleY,
      mx: (event.movementX || 0) * scaleX,
      my: (event.movementY || 0) * scaleY,
    };
  }

  _shouldIgnoreKeyboardEvent(event) {
    const target = event.target;
    if (!target || !target.tagName) return false;
    const tag = target.tagName.toLowerCase();
    if (tag === 'input' || tag === 'textarea') return true;
    return Boolean(target.isContentEditable);
  }

  async _loadImageData(url) {
    return new Promise((resolve, reject) => {
      const img = new Image();
      img.crossOrigin = 'anonymous';
      img.onload = () => {
        const offscreen = document.createElement('canvas');
        offscreen.width = img.width;
        offscreen.height = img.height;
        const ctx = offscreen.getContext('2d');
        ctx.drawImage(img, 0, 0);
        resolve(ctx.getImageData(0, 0, img.width, img.height));
      };
      img.onerror = reject;
      img.src = url;
    });
  }
}

const runTestsSequentially = async (ctx) => {
  for (const test of tests) {
    const indicator = document.getElementById(`test-${test.id}-status`);
    indicator.textContent = '执行中...';
    indicator.className = 'test-status status-wait';
    try {
      await test.run(ctx);
      indicator.textContent = '通过';
      indicator.className = 'test-status status-pass';
    } catch (err) {
      indicator.textContent = '失败';
      indicator.className = 'test-status status-fail';
      writeLog(`❌ ${test.name} 失败：${err.message || err}`);
      throw err;
    }
  }
};

const main = async () => {
  renderTestRows();
  const harness = new TestHarness();
  try {
    await runTestsSequentially(harness);
    setStatus('render-status', '全部测试通过');
    writeLog('✅ 所有测试完成');
  } catch (error) {
    console.error(error);
    setStatus('render-status', '测试中断');
  }
};

main();

