<script setup lang="ts">
import { computed } from 'vue'
import MarkdownIt from 'markdown-it'
import DOMPurify from 'dompurify'

const props = defineProps<{ source: string }>()
const markdown = new MarkdownIt({ html: false, linkify: false })
const html = computed(() =>
  DOMPurify.sanitize(markdown.render(props.source), { FORBID_TAGS: ['a', 'img'] }),
)
</script>

<template>
  <!-- eslint-disable-next-line vue/no-v-html -- Markdown output is sanitized by DOMPurify. -->
  <article class="markdown" v-html="html" />
</template>

<style scoped>
.markdown {
  max-width: 82ch;
  color: var(--muted);
  line-height: 1.8;
  overflow-wrap: anywhere;
}
.markdown :deep(h2) {
  margin: 13px 0 6px;
  font-size: 16px;
  color: var(--text);
}
.markdown :deep(ol),
.markdown :deep(ul) {
  padding-inline-start: 1.5em;
}
.markdown :deep(code) {
  font-family: var(--mono);
  color: var(--accent);
}
.markdown :deep(pre) {
  overflow: auto;
  padding: 10px;
  background: var(--surface-inset);
  border: 1px solid var(--border-soft);
  border-radius: 8px;
}
</style>
