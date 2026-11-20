<script setup lang="ts">
import { onMounted, watch } from 'vue'
import { useIntervalFn } from '@vueuse/core'
import { RouterLink, RouterView } from 'vue-router'
import { NConfigProvider, NMessageProvider, zhCN, type GlobalThemeOverrides } from 'naive-ui'
import {
  BookOpen,
  Clock3,
  FolderTree,
  Info,
  LayoutGrid,
  LockKeyhole,
  Settings,
  Star,
  Terminal,
} from 'lucide-vue-next'
import brandIcon from '../design/webgl-launcher-icon.svg?no-inline'
import { useSession } from './session'

const session = useSession()
const theme: GlobalThemeOverrides = {
  common: {
    primaryColor: '#5652d9',
    primaryColorHover: '#4641c0',
    primaryColorPressed: '#3834a5',
    primaryColorSuppl: '#5652d9',
    borderRadius: '7px',
    fontFamily: 'inherit',
  },
  Button: {
    textColorPrimary: '#fff',
    textColorHoverPrimary: '#fff',
    textColorPressedPrimary: '#fff',
    textColorFocusPrimary: '#fff',
    fontWeight: '600',
  },
  Input: {
    color: '#ffffff',
    colorFocus: '#ffffff',
    border: '1px solid #dedeea',
    textColor: '#23243a',
    placeholderColor: '#65677c',
  },
  Switch: {
    railColor: '#aaaabd',
    railColorActive: '#5652d9',
  },
}

const { resume: resumeLogs, pause: pauseLogs } = useIntervalFn(() => void session.refreshLive(), 800, {
  immediate: false,
})
watch(
  () => session.running,
  (running) => (running ? resumeLogs() : pauseLogs()),
  { immediate: true },
)

onMounted(() => {
  void session.sync()
})
</script>

<template>
  <NConfigProvider :theme-overrides="theme" :locale="zhCN">
    <NMessageProvider>
      <div class="app-shell">
        <header class="topbar">
          <div class="topbar-inner">
            <RouterLink class="brand" to="/" aria-label="Unity WebGL 启动器首页">
              <img class="brand-mark" :src="brandIcon" alt="" width="34" height="34" />
              <span>Unity WebGL</span>
            </RouterLink>
            <nav class="navigation" aria-label="主导航">
              <RouterLink class="nav-link" to="/"><LayoutGrid aria-hidden="true" />单项目</RouterLink>
              <RouterLink class="nav-link" to="/catalog"
                ><FolderTree aria-hidden="true" />多项目</RouterLink
              >
              <RouterLink class="nav-link" to="/frequent"
                ><Star aria-hidden="true" />收藏项目</RouterLink
              >
              <RouterLink class="nav-link" to="/recent"
                ><Clock3 aria-hidden="true" />最近项目</RouterLink
              >
              <RouterLink class="nav-link" to="/logs"
                ><Terminal aria-hidden="true" />运行日志</RouterLink
              >
              <RouterLink class="nav-link" to="/settings"
                ><Settings aria-hidden="true" />设置</RouterLink
              >
              <RouterLink class="nav-link" to="/help"
                ><BookOpen aria-hidden="true" />使用指南</RouterLink
              >
              <RouterLink class="nav-link" to="/about"><Info aria-hidden="true" />关于软件</RouterLink>
            </nav>
          </div>
        </header>
        <main class="workspace"><RouterView /></main>
        <footer class="app-footer">
          <LockKeyhole :size="14" aria-hidden="true" />预览服务仅监听本机回环地址
        </footer>
      </div>
    </NMessageProvider>
  </NConfigProvider>
</template>

<style scoped>
.app-shell {
  min-height: 100vh;
  display: grid;
  grid-template-columns: 188px minmax(0, 1fr);
  grid-template-rows: 1fr auto;
}
.topbar {
  grid-row: 1 / -1;
  border-right: 1px solid var(--border);
  background: #fafafd;
}
.topbar-inner {
  position: sticky;
  top: 0;
  padding: 20px 12px;
}
.brand {
  display: flex;
  align-items: center;
  gap: 8px;
  min-height: 36px;
  padding-inline: 5px;
  color: var(--text);
  font-size: 14px;
  font-weight: 700;
  text-decoration: none;
}
.brand-mark {
  flex: none;
}
.navigation {
  display: grid;
  gap: 4px;
  margin-top: 21px;
}
.nav-link {
  display: flex;
  align-items: center;
  gap: 10px;
  min-height: 38px;
  padding: 6px 10px;
  border-radius: 7px;
  color: var(--muted);
  font-size: 13px;
  text-decoration: none;
  white-space: nowrap;
}
.nav-link svg {
  flex: none;
  width: 16px;
  height: 16px;
}
.nav-link:hover {
  background: var(--surface-inset);
  color: var(--accent);
}
.nav-link.router-link-exact-active {
  background: var(--accent-wash);
  color: var(--accent);
  font-weight: 650;
}
.nav-link[href='#/help'] {
  margin-top: 14px;
}
.workspace {
  display: flex;
  flex-direction: column;
  width: 100%;
  max-width: 1420px;
  min-width: 0;
  padding: 21px 24px var(--footer-gap);
  margin-inline: auto;
}
.app-footer {
  display: flex;
  align-items: center;
  gap: 7px;
  padding: 10px 24px;
  border-top: 1px solid var(--border);
  color: var(--muted);
  font-size: 12px;
}
@media (max-width: 760px) {
  .app-shell {
    grid-template-columns: minmax(0, 1fr);
    grid-template-rows: auto 1fr auto;
  }
  .topbar {
    grid-row: auto;
    border-right: 0;
    border-bottom: 1px solid var(--border);
  }
  .topbar-inner {
    padding: 8px 14px;
  }
  .navigation {
    display: flex;
    flex-wrap: wrap;
    gap: 4px;
    margin-top: 5px;
  }
  .nav-link {
    min-height: 34px;
    padding-inline: 8px;
  }
  .nav-link[href='#/help'] {
    margin-top: 0;
  }
  .workspace {
    padding: 16px 14px var(--footer-gap);
  }
  .app-footer {
    padding-inline: 14px;
  }
}
</style>
