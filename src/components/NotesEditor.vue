<script setup lang="ts">
import { MdEditor } from 'md-editor-v3'
import 'md-editor-v3/lib/style.css'

const notes = defineModel<string>({ required: true })
</script>

<template>
  <MdEditor
    v-model="notes"
    class="notes-editor"
    theme="light"
    language="zh-CN"
    placeholder="记录项目、构建选项或排查结果…"
    :toolbars="['bold', 'italic', 'title', 'unorderedList', 'orderedList', 'code', 'revoke', 'next']"
    :footers="['markdownTotal']"
    :preview="false"
    :no-prettier="true"
    :no-upload-img="true"
  />
</template>

<style scoped>
.notes-editor {
  height: 250px;
  border-radius: 9px;
  overflow: hidden;
  --md-color: var(--text);
  --md-bk-color: var(--surface-field);
  --md-border-color: var(--border);
  --md-hover-color: var(--accent);
  --md-bk-color-outstand: var(--surface-inset);
  --md-bk-hover-color: var(--accent-wash);
  --md-border-active-color: var(--focus);
}
</style>
