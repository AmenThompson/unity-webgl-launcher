<script setup lang="ts">
import { Info } from 'lucide-vue-next'
import { version } from '../../package.json'
</script>

<template>
  <section class="panel page-panel about" aria-labelledby="about-title">
    <header class="section-header">
      <span class="section-icon"><Info aria-hidden="true" /></span>
      <h1 id="about-title">关于软件</h1>
    </header>
    <h2>Unity WebGL 启动器</h2>
    <p>用于在 Windows 上预览本地 Unity WebGL 发布内容，通过系统默认浏览器打开场景。</p>
    <p>
      支持直接打开单个项目，或扫描发布总目录后选择和切换项目。启动时会自动识别压缩格式与解压回退状态。
    </p>
    <dl class="release-info">
      <dt>发布版本</dt>
      <dd class="mono" data-testid="release-version">{{ version }}</dd>
    </dl>
  </section>
</template>

<style scoped>
.about h1 {
  margin: 0;
  font-size: clamp(20px, 3vw, 27px);
}
.about h2 {
  margin: 0 0 8px;
  font-size: 19px;
}
.about p {
  margin: 6px 0;
  color: var(--muted);
  line-height: 1.7;
}
.release-info {
  display: flex;
  align-items: baseline;
  gap: 12px;
  margin: 16px 0 0;
}
.release-info dd {
  margin: 0;
  padding: 4px 10px;
  border-radius: 4px;
  background: var(--accent-wash);
  color: var(--accent);
  font-weight: 650;
}
</style>
