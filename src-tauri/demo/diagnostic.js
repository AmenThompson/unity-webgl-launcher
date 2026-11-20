const canvas = document.querySelector('#scene')
const status = document.querySelector('#status')
const fullscreen = document.querySelector('#fullscreen')
const gl = canvas.getContext('webgl', { antialias: true })

function initialize(context) {
  function compile(kind, source) {
    const shader = context.createShader(kind)
    context.shaderSource(shader, source)
    context.compileShader(shader)
    if (!context.getShaderParameter(shader, context.COMPILE_STATUS)) {
      throw new Error(context.getShaderInfoLog(shader))
    }
    return shader
  }

  const program = context.createProgram()
  context.attachShader(program, compile(context.VERTEX_SHADER, `
    attribute vec2 position;
    attribute vec3 color;
    uniform float angle;
    uniform vec2 scale;
    varying vec3 vColor;
    void main() {
      float c = cos(angle), s = sin(angle);
      gl_Position = vec4((mat2(c, s, -s, c) * position) * scale, 0.0, 1.0);
      vColor = color;
    }
  `))
  context.attachShader(program, compile(context.FRAGMENT_SHADER, `
    precision mediump float;
    varying vec3 vColor;
    void main() { gl_FragColor = vec4(vColor, 1.0); }
  `))
  context.linkProgram(program)
  if (!context.getProgramParameter(program, context.LINK_STATUS)) {
    throw new Error(context.getProgramInfoLog(program))
  }
  context.useProgram(program)
  context.bindBuffer(context.ARRAY_BUFFER, context.createBuffer())
  context.bufferData(context.ARRAY_BUFFER, new Float32Array([
     0.00,  0.62, 0.25, 0.72, 1.00,
    -0.55, -0.36, 0.56, 0.33, 1.00,
     0.55, -0.36, 0.26, 0.95, 0.78,
  ]), context.STATIC_DRAW)
  const position = context.getAttribLocation(program, 'position')
  const color = context.getAttribLocation(program, 'color')
  context.enableVertexAttribArray(position)
  context.enableVertexAttribArray(color)
  context.vertexAttribPointer(position, 2, context.FLOAT, false, 20, 0)
  context.vertexAttribPointer(color, 3, context.FLOAT, false, 20, 8)
  const angleUniform = context.getUniformLocation(program, 'angle')
  const scaleUniform = context.getUniformLocation(program, 'scale')
  let angle = 0
  let previousX = 0

  function render() {
    const ratio = Math.min(devicePixelRatio, 2)
    const width = Math.max(1, Math.round(canvas.clientWidth * ratio))
    const height = Math.max(1, Math.round(canvas.clientHeight * ratio))
    if (canvas.width !== width) canvas.width = width
    if (canvas.height !== height) canvas.height = height
    context.viewport(0, 0, width, height)
    context.clearColor(0.034, 0.063, 0.11, 1)
    context.clear(context.COLOR_BUFFER_BIT)
    context.uniform1f(angleUniform, angle)
    context.uniform2f(scaleUniform, Math.min(1, height / width), Math.min(1, width / height))
    context.drawArrays(context.TRIANGLES, 0, 3)
    status.textContent = `WebGL 已渲染 · 角度 ${Math.round(angle * 180 / Math.PI)}° · 跨源隔离${crossOriginIsolated ? '已启用' : '未启用'}`
  }

  canvas.addEventListener('pointerdown', (event) => {
    previousX = event.clientX
    canvas.focus()
    canvas.setPointerCapture(event.pointerId)
  })
  canvas.addEventListener('pointermove', (event) => {
    if (!canvas.hasPointerCapture(event.pointerId)) return
    angle += (event.clientX - previousX) / 100
    previousX = event.clientX
    render()
  })
  canvas.addEventListener('keydown', (event) => {
    const step = { ArrowLeft: -1, ArrowRight: 1, ArrowUp: 1, ArrowDown: -1 }[event.key]
    if (!step) return
    event.preventDefault()
    angle += step * 0.12
    render()
  })
  canvas.addEventListener('webglcontextlost', () => {
    status.textContent = 'WebGL 上下文丢失。请刷新页面重新检测。'
  })
  new ResizeObserver(render).observe(canvas)
  fullscreen.addEventListener('click', () => {
    canvas.requestFullscreen().then(() => canvas.focus()).catch((error) => {
      status.textContent = `无法全屏：${error.message}`
    })
  })
  render()
}

try {
  if (!gl) throw new Error('当前浏览器或显卡未提供 WebGL。')
  initialize(gl)
} catch (error) {
  status.textContent = `自检失败：${error.message}`
  fullscreen.disabled = true
}
