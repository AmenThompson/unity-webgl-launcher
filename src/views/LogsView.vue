<script setup lang="ts">
import EventLog from '../components/EventLog.vue'
import { useSession } from '../session'

const session = useSession()
</script>

<template>
  <EventLog
    class="page-panel"
    :events="session.snapshot.events"
    :diagnostics="session.snapshot.diagnostics"
    :live="session.running"
  />
</template>

<style scoped>
.page-panel {
  display: flex;
  flex-direction: column;
  /* Include the panel’s 1px border in the same visible gap as the bottom bar. */
  padding-bottom: calc(var(--footer-gap) - 1px);
}
.page-panel :deep(.log-output) {
  flex: 1 1 0;
  max-height: none;
}
</style>
