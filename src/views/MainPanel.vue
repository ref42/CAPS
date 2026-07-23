<template>
    <div class="panel-container">
        <div v-if="themeMode === 'coverglass'" class="coverglass-bg-container">
            <div class="coverglass-bg-image" :style="coverUrl ? { backgroundImage: `url(${coverUrl})` } : {}"></div>
            <div class="coverglass-blur-layer"></div>
            <div class="coverglass-noise-layer"></div>
        </div>
        <div class="custom-titlebar">
            <div data-tauri-drag-region class="titlebar-drag-area"></div>

            <div class="titlebar-controls">
                <button class="titlebar-btn" @click="minimizeWindow">
                    <svg viewBox="0 0 12 12" fill="currentColor">
                        <rect x="1" y="5" width="10" height="1.5" rx="0.5" />
                    </svg>
                </button>
                <button class="titlebar-btn close-btn" @click="closeWindow">
                    <svg viewBox="0 0 12 12" stroke="currentColor" stroke-width="1.2" stroke-linecap="round">
                        <path d="M2.5 2.5L9.5 9.5M9.5 2.5L2.5 9.5" />
                    </svg>
                </button>
            </div>
        </div>

        <header class="panel-header">
            <div class="brand">
                <img src="../assets/qiuniu.logo" class="logo-icon">
                <div>
                    <h1>QiuNiu</h1>
                    <p class="subtitle">{{ t('appSubtitle') }} v{{ appVersion }}</p>
                </div>
            </div>

            <div class="header-controls">
                <button v-if="false" class="dynamicset-btn" :class="{ 'is-active': isDynamicSet }" @click="toggleDynamicSet">
                    {{ t('personalizeCenter') }}
                </button>
                <span class="control-separator"></span>

                <span class="status-badge" :class="{ 'is-active': isWidgetVisible }">
                    {{ isWidgetVisible ? t('enabled') : t('disabled') }}
                </span>
                <label class="switch header-switch">
                    <input type="checkbox" :checked="isWidgetVisible" @change="toggleWidget">
                    <span class="slider"></span>
                </label>
            </div>
        </header>

        <hr class="divider" />

        <div class="main-content" :class="{ 'dynamicset-layout': isDynamicSet }">
            <template v-if="!isDynamicSet">
                <div class="card status-card">
                    <div class="card-header-row">
                        <h3>{{ t('realtimeStatus') }}</h3>
                        <button v-if="false" class="stats-toggle-btn" @click="toggleRightPanel">
                            {{ rightPanel === 'settings' ? t('trafficStats') : t('exit') }}
                        </button>
                    </div>
                    <div class="speed-monitor">
                        <div class="speed-item">
                            <span class="arrow up">
                                <svg viewBox="0 0 32 32" fill="none" xmlns="http://www.w3.org/2000/svg">
                                    <path
                                        d="M16 4C16.8 4 17.5 4.3 18.1 4.9L28.1 14.9C29.3 16.1 29.3 18 28.1 19.1C26.9 20.3 25 20.3 23.9 19.1L18 13.2V26C18 27.7 16.7 29 15 29C13.3 29 12 27.7 12 26V13.2L6.1 19.1C4.9 20.3 3 20.3 1.9 19.1C0.7 18 0.7 16.1 1.9 14.9L11.9 4.9C12.5 4.3 13.2 4 14 4H16Z"
                                        fill="currentColor" />
                                </svg>
                            </span>
                            <div class="speed-info">
                                <span class="label">{{ t('uploadSpeed') }}</span>
                                <span class="value">{{ uploadSpeed }}</span>
                            </div>
                        </div>
                        <div class="speed-item">
                            <span class="arrow down">
                                <svg viewBox="0 0 32 32" fill="none" xmlns="http://www.w3.org/2000/svg">
                                    <path
                                        d="M16 28C15.2 28 14.5 27.7 13.9 27.1L3.9 17.1C2.7 15.9 2.7 14 3.9 12.9C5.1 11.7 7 11.7 8.1 12.9L14 18.8V6C14 4.3 15.3 3 17 3C18.7 3 20 4.3 20 6V18.8L25.9 12.9C27.1 11.7 29 11.7 30.1 12.9C31.3 14 31.3 15.9 30.1 17.1L20.1 27.1C19.5 27.7 18.8 28 18 28H16Z"
                                        fill="currentColor" />
                                </svg>
                            </span>
                            <div class="speed-info">
                                <span class="label">{{ t('downloadSpeed') }}</span>
                                <span class="value">{{ downloadSpeed }}</span>
                            </div>
                        </div>
                    </div>
                    <div ref="chartRef" class="mini-chart"></div>
                </div>

                <div class="card settings-card" v-if="false">
                    <h3>{{ t('consoleSettings') }}</h3>
                    <div class="setting-item flex-row-item" :class="{ 'is-dropdown-open': isThemeModeDropdownOpen }">
                        <div class="item-meta">
                            <span class="item-title">{{ t('themeColor') }}</span>
                            <span class="item-desc">{{ t('themeColorDesc') }}</span>
                        </div>

                        <div class="custom-dropdown" tabindex="0" @blur="isThemeModeDropdownOpen = false">
                            <div class="dropdown-trigger" style="width: 110px;"
                                @click="isThemeModeDropdownOpen = !isThemeModeDropdownOpen">
                                <div class="current-item">
                                    <template v-if="themeMode === 'light'">{{ t('lightMode') }}</template>
                                    <template v-else-if="themeMode === 'dark'">{{ t('darkMode') }}</template>
                                    <template v-else-if="themeMode === 'coverglass'">{{ t('coverglassMode') }}</template>
                                    <template v-else-if="themeMode === 'system'">{{ t('systemMode') }}</template>
                                </div>
                                <svg viewBox="0 0 24 24" class="arrow-icon"
                                    :class="{ 'is-open': isThemeModeDropdownOpen }">
                                    <path d="M7 10l5 5 5-5" fill="none" stroke="currentColor" stroke-width="2"
                                        stroke-linecap="round" />
                                </svg>
                            </div>

                            <transition name="dropdown">
                                <div class="dropdown-menu" v-show="isThemeModeDropdownOpen" style="width: 100%;">
                                    <div class="dropdown-item" :class="{ 'is-active': themeMode === 'light' }"
                                        @click="handleSelectThemeMode('light')">
                                        {{ t('lightMode') }}
                                    </div>
                                    <div class="dropdown-item" :class="{ 'is-active': themeMode === 'dark' }"
                                        @click="handleSelectThemeMode('dark')">
                                        {{ t('darkMode') }}
                                    </div>
                                    <div class="dropdown-item" :class="{ 'is-active': themeMode === 'coverglass' }"
                                        @click="handleSelectThemeMode('coverglass')">
                                        {{ t('coverglassMode') }}
                                    </div>
                                    <div class="dropdown-item" :class="{ 'is-active': themeMode === 'system' }"
                                        @click="handleSelectThemeMode('system')">
                                        {{ t('systemMode') }}
                                    </div>
                                </div>
                            </transition>
                        </div>
                    </div>
                    <div class="setting-item">
                        <div class="item-meta">
                            <span class="item-title">{{ t('autoStart') }}</span>
                            <span class="item-desc">{{ t('autoStartDesc') }}</span>
                        </div>
                        <label class="switch">
                            <input type="checkbox" v-model="autoStart" @change="toggleAutoStart">
                            <span class="slider"></span>
                        </label>
                    </div>
                    <div class="setting-item slider-item">
                        <div class="item-meta" style="width: 100%;">
                            <div class="combo-title-row">
                                <span class="item-title">{{ t('islandOpacity') }}</span>
                                <span class="title-separator">|</span>
                                <span class="item-title-sec">
                                    {{ t('pinToTaskbar') }}
                                    <span class="tooltip-wrapper" :data-tooltip="t('pinToTaskbarTip')">
                                        <p class="set-item-tips-tag">🙋</p>
                                    </span>
                                </span>
                                <label class="switch mini-switch" style="opacity: 0.8;">
                                    <input type="checkbox" v-model="pinToTaskbar" @change="togglePinTaskbar">
                                    <span class="slider"></span>
                                </label>
                            </div>
                            <span class="item-desc">{{ t('islandOpacityDesc') }} ({{ opacity }}%)</span>
                        </div>
                        <input type="range" min="0" max="100" v-model="opacity" class="range-input" />
                    </div>
                </div>

                <template v-else>
                    <div class="card stats-card">
                        <div class="card-header-row">
                            <h3>{{ t('trafficStats') }}</h3>

                            <div class="custom-dropdown" tabindex="0" @blur="isStatChartDropdownOpen = false">
                                <div class="dropdown-trigger" style="width: 90px;"
                                    @click="isStatChartDropdownOpen = !isStatChartDropdownOpen">
                                    <div class="current-item">
                                        <template v-if="statChartType === 'bar'">{{ t('barChart') }}</template>
                                        <template v-else-if="statChartType === 'line'">{{ t('lineChart') }}</template>
                                    </div>
                                    <svg viewBox="0 0 24 24" class="arrow-icon"
                                        :class="{ 'is-open': isStatChartDropdownOpen }">
                                        <path d="M7 10l5 5 5-5" fill="none" stroke="currentColor" stroke-width="2"
                                            stroke-linecap="round" />
                                    </svg>
                                </div>

                                <transition name="dropdown">
                                    <div class="dropdown-menu" v-show="isStatChartDropdownOpen" style="width: 100%;">
                                        <div class="dropdown-item" :class="{ 'is-active': statChartType === 'bar' }"
                                            @click="handleSelectStatChart('bar')">
                                            {{ t('barChart') }}
                                        </div>
                                        <div class="dropdown-item" :class="{ 'is-active': statChartType === 'line' }"
                                            @click="handleSelectStatChart('line')">
                                            {{ t('lineChart') }}
                                        </div>
                                    </div>
                                </transition>
                            </div>
                        </div>
                        <div class="stats-overview">
                            <div class="stat-box">
                                <span class="stat-label">{{ t('totalUpload') }}</span>
                                <span class="stat-val">{{ formatBytesValue(totalUpload) }} {{
                                    formatBytesUnit(totalUpload) }}</span>
                                <span class="stat-unit"></span>
                            </div>
                            <div class="stat-box">
                                <span class="stat-label">{{ t('totalDownload') }}</span>
                                <span class="stat-val">{{ formatBytesValue(totalDownload) }} {{
                                    formatBytesUnit(totalDownload) }}</span>
                            </div>
                            <div class="stat-box">
                                <span class="stat-label">{{ t('monthTraffic') }}</span>
                                <span class="stat-val">{{ formatBytesValue(monthTraffic) }} {{
                                    formatBytesUnit(monthTraffic) }}</span>
                            </div>
                        </div>
                        <div ref="statsChartRef" class="stats-chart-container"></div>
                    </div>
                </template>

                <div v-if="false" class="dynamicset-grid bottom-grid-card">
                    <div class="set-item" :class="{ 'is-dropdown-open': isLanguageDropdownOpen }">
                        <div class="set-item-meta">
                            <span class="set-item-title">{{ t('language') }}</span>
                            <span class="set-item-desc">{{ t('languageDesc') }}</span>
                        </div>
                        <div class="custom-dropdown" tabindex="0" @blur="isLanguageDropdownOpen = false">
                            <div class="dropdown-trigger" @click="isLanguageDropdownOpen = !isLanguageDropdownOpen">
                                <div class="current-item">{{ t(currentLanguage === 'zh-CN' ? 'simplifiedChinese' : 'english') }}</div>
                                <svg viewBox="0 0 24 24" class="arrow-icon"
                                    :class="{ 'is-open': isLanguageDropdownOpen }">
                                    <path d="M7 10l5 5 5-5" fill="none" stroke="currentColor" stroke-width="2"
                                        stroke-linecap="round" />
                                </svg>
                            </div>

                            <transition name="dropdown">
                                <div class="dropdown-menu" v-show="isLanguageDropdownOpen">
                                    <div v-for="option in languageOptions" :key="option.value" class="dropdown-item"
                                        :class="{ 'is-active': currentLanguage === option.value }"
                                        @click="handleSelectLanguage(option.value)">
                                        {{ t(option.labelKey) }}
                                    </div>
                                </div>
                            </transition>
                        </div>
                    </div>
                    <div class="set-item" :class="{ 'is-dropdown-open': isPlayerDropdownOpen }">
                        <div class="set-item-meta">
                            <span class="set-item-title">{{ t('targetMediaPlatform') }}</span>
                            <span class="set-item-desc">{{ t('targetMediaPlatformDesc') }}</span>
                        </div>
                        <div class="custom-dropdown" tabindex="0" @blur="isPlayerDropdownOpen = false">
                            <div class="dropdown-trigger" @click="isPlayerDropdownOpen = !isPlayerDropdownOpen">
                                <div class="current-item">
                                    <template v-if="targetPlayer === 'netease'"><span
                                            class="platform-icon platform-glyph">N</span> {{ t('netease') }}</template>
                                    <template v-else-if="targetPlayer === 'spotify'"><span
                                            class="platform-icon platform-glyph">S</span> Spotify</template>
                                    <template v-else-if="targetPlayer === 'apple'"><span
                                            class="platform-icon platform-glyph">A</span> Apple</template>
                                    <template v-else-if="targetPlayer === 'qqmusic'"><span
                                            class="platform-icon platform-glyph">Q</span> {{ t('qqMusic') }}</template>
                                    <template v-else-if="targetPlayer === 'kugou'"><span
                                            class="platform-icon platform-glyph">K</span> {{ t('kugouMusic') }}</template>
                                    <template v-else-if="targetPlayer === 'echo'"><span
                                            class="platform-icon platform-glyph">E</span> EchoMusic</template>
                                    <template v-else-if="targetPlayer === 'lx-music'"><span
                                            class="platform-icon platform-glyph">L</span> {{ t('lxMusic') }}</template>
                                    <template v-else-if="targetPlayer === 'other'">
                                        <svg viewBox="0 0 24 24" class="platform-icon" fill="currentColor">
                                            <path
                                                d="M12 2C6.48 2 2 6.48 2 12s4.48 10 10 10 10-4.48 10-10S17.52 2 12 2zm-2 14.5v-9l6 4.5-6 4.5z" />
                                        </svg>
                                        {{ t('genericMedia') }}
                                    </template>
                                </div>
                                <svg viewBox="0 0 24 24" class="arrow-icon"
                                    :class="{ 'is-open': isPlayerDropdownOpen }">
                                    <path d="M7 10l5 5 5-5" fill="none" stroke="currentColor" stroke-width="2"
                                        stroke-linecap="round" />
                                </svg>
                            </div>

                            <transition name="dropdown">
                                <div class="dropdown-menu" v-show="isPlayerDropdownOpen">
                                    <div class="dropdown-item" :class="{ 'is-active': targetPlayer === 'netease' }"
                                        @click="handleSelectPlayer('netease')">
                                        <span class="platform-icon platform-glyph">N</span> {{ t('netease') }}
                                    </div>
                                    <div class="dropdown-item" :class="{ 'is-active': targetPlayer === 'spotify' }"
                                        @click="handleSelectPlayer('spotify')">
                                        <span class="platform-icon platform-glyph">S</span> Spotify
                                    </div>
                                    <div class="dropdown-item" :class="{ 'is-active': targetPlayer === 'apple' }"
                                        @click="handleSelectPlayer('apple')">
                                        <span class="platform-icon platform-glyph">A</span> Apple
                                    </div>
                                    <div class="dropdown-item" :class="{ 'is-active': targetPlayer === 'qqmusic' }"
                                        @click="handleSelectPlayer('qqmusic')">
                                        <span class="platform-icon platform-glyph">Q</span> {{ t('qqMusic') }}
                                    </div>
                                    <div class="dropdown-item" :class="{ 'is-active': targetPlayer === 'kugou' }"
                                        @click="handleSelectPlayer('kugou')">
                                        <span class="platform-icon platform-glyph">K</span> {{ t('kugouMusic') }}
                                    </div>
                                    <div class="dropdown-item" :class="{ 'is-active': targetPlayer === 'echo' }"
                                        @click="handleSelectPlayer('echo')">
                                        <span class="platform-icon platform-glyph">E</span> EchoMusic
                                    </div>
                                    <div class="dropdown-item" :class="{ 'is-active': targetPlayer === 'lx-music' }"
                                        @click="handleSelectPlayer('lx-music')">
                                        <span class="platform-icon platform-glyph">L</span> {{ t('lxMusic') }}
                                    </div>
                                    <div class="dropdown-item" :class="{ 'is-active': targetPlayer === 'other' }"
                                        @click="handleSelectPlayer('other')">
                                        <svg viewBox="0 0 24 24" class="platform-icon" fill="currentColor">
                                            <path
                                                d="M12 2C6.48 2 2 6.48 2 12s4.48 10 10 10 10-4.48 10-10S17.52 2 12 2zm-2 14.5v-9l6 4.5-6 4.5z" />
                                        </svg>
                                        {{ t('otherMediaControl') }}
                                    </div>
                                </div>
                            </transition>
                        </div>
                    </div>
                    <div class="set-item">
                        <div class="set-item-meta">
                            <span class="set-item-title">{{ t('mediaController') }}</span>
                            <span class="set-item-desc">{{ t('mediaControllerDesc') }}</span>
                        </div>
                        <label class="switch">
                            <input type="checkbox" v-model="enableMusicCtrl">
                            <span class="slider"></span>
                        </label>
                    </div>
                    <div class="set-item">
                        <div class="set-item-meta">
                            <span class="set-item-title">{{ t('messageNotifications') }}</span>
                            <span class="set-item-desc">{{ t('messageNotificationsDesc') }}</span>
                        </div>
                        <label class="switch">
                            <input type="checkbox" v-model="enableMsgNotify" @change="toggleMsgNotify">
                            <span class="slider"></span>
                        </label>
                    </div>
                    <div class="set-item">
                        <div class="set-item-meta">
                            <span class="set-item-title">{{ t('quietMode') }}</span>
                            <span class="set-item-desc">{{ t('quietModeDesc') }}</span>
                        </div>
                        <label class="switch">
                            <input type="checkbox" v-model="msgModeEnabled" @change="toggleMsgMode">
                            <span class="slider"></span>
                        </label>
                    </div>
                    <div class="set-item">
                        <div class="set-item-meta">
                            <span class="set-item-title">{{ t('fullscreenAutoHide') }}</span>
                            <span class="set-item-desc">{{ t('fullscreenAutoHideDesc') }}</span>
                        </div>
                        <label class="switch">
                            <input type="checkbox" v-model="autoHideFullscreen" @change="toggleAutoHide">
                            <span class="slider"></span>
                        </label>
                    </div>
                </div>
            </template>

            <template v-else>
                <DynamicSet />
            </template>
        </div>

        <footer class="panel-footer">
            <div class="ft_left">
                <span>&copy; 2026 <button class="openmywebsite" @click="openMywebsite">ref42.</button> All rights
                    reserved.</span>
                <span>QiuNiu v{{ appVersion }}</span>
            </div>
            <div class="ft_right">
                <span class="action-link" @click="openNSDweb">{{ t('officialWebsite') }}</span>
                <span class="action-link" @click="openNSDdata">{{ t('openSourceData') }}</span>
                <span class="action-link"
                    :style="{ opacity: isChecking ? 0.5 : 1, pointerEvents: isChecking ? 'none' : 'auto', position: 'relative' }"
                    @click="checkUpdate">
                    <span v-if="hasNewVersion" class="update-dot"></span>
                    {{ isChecking ? t('checking') : (hasNewVersion ? t('newVersionDetected') : t('checkUpdate')) }}
                </span>
            </div>
        </footer>

        <Transition name="fade">
            <div v-if="dialog.visible" class="modal-overlay" @click.self="closeDialog">
                <div class="modal-card">
                    <div class="modal-header">
                        <h4>{{ dialog.title }}</h4>
                    </div>
                    <div class="modal-body">
                        <p>{{ dialog.message }}</p>
                    </div>
                    <div class="modal-footer">
                        <button v-if="dialog.isConfirm" class="btn btn-secondary" @click="closeDialog">{{ t('cancel') }}</button>
                        <button class="btn btn-primary" @click="handleDialogConfirm">{{ t('confirm') }}</button>
                    </div>
                </div>
            </div>
        </Transition>
    </div>
</template>

<script setup lang="ts">
import { ref, onMounted, onUnmounted, watch, nextTick, computed } from 'vue';
import { invoke } from '@tauri-apps/api/core';
import { emit, listen } from '@tauri-apps/api/event';
import { getVersion } from '@tauri-apps/api/app';
import * as echarts from 'echarts';
import { enable, disable, isEnabled } from '@tauri-apps/plugin-autostart';
import { openUrl } from '@tauri-apps/plugin-opener';
import { getCurrentWindow } from '@tauri-apps/api/window';
import DynamicSet from '../components/DynamicSet.vue';
import { t, currentLanguage, setLanguage, languageOptions, type AppLanguage } from '../i18n';

const isWidgetVisible = ref(false);
const autoStart = ref(false);
const opacity = ref(Number(localStorage.getItem('nsd_island_opacity') || '100'));

const savedTheme = localStorage.getItem('nsd_theme_mode') || 'light';
const themeMode = ref(['light', 'dark', 'coverglass', 'system'].includes(savedTheme) ? savedTheme : 'light');

const coverUrl = ref('');
const coverCache = new Map<string, string>();
const currentTrackInfo = ref('');
let coverTimer: number | null = null;

const uploadSpeed = ref('0 B/s');
const downloadSpeed = ref('0 B/s');

const appVersion = ref('1.0.0');

const isDynamicSet = ref(false);

const isChecking = ref(false);
const hasNewVersion = ref(false);


const targetPlayer = ref(localStorage.getItem('nsd_target_player') || 'netease');

const setTargetPlayer = async (player: string) => {
    targetPlayer.value = player;
    localStorage.setItem('nsd_target_player', player); 
    try {
        await invoke('set_target_player', { player }); 
    } catch (e) {
        console.error('切换平台失败', e);
    }
};


const bakeBlurImage = (url: string): Promise<string> => {
    return new Promise((resolve) => {
        const img = new Image();
        
        if (url.startsWith('http')) {
            img.crossOrigin = 'anonymous';
        }

        img.onload = () => {
            const canvas = document.createElement('canvas');
            
            canvas.width = 120;
            canvas.height = 120;
            const ctx = canvas.getContext('2d');
            if (!ctx) return resolve(url);

            
            ctx.filter = 'blur(10px)';
            
            ctx.drawImage(img, -10, -10, 140, 140);

            try {
                
                resolve(canvas.toDataURL('image/jpeg', 0.6));
            } catch (e) {
                
                resolve(url);
            }
        };
        img.onerror = () => resolve(url);
        img.src = url;
    });
};

const syncMusicCover = async () => {
    
    if (themeMode.value !== 'coverglass') return;
    try {
        const res = await invoke<[string, string, boolean] | null>('fetch_netease_music_info');
        if (res) {
            const [song, artist] = res;
            const newTrackInfo = artist ? `${song} - ${artist}` : song;

            if (currentTrackInfo.value !== newTrackInfo) {
                currentTrackInfo.value = newTrackInfo;

                
                if (coverCache.has(newTrackInfo)) {
                    coverUrl.value = coverCache.get(newTrackInfo)!;
                } else {
                    try {
                        const realCoverUrl = await invoke<string>('get_random_cover_url', {
                            songName: song,
                            artistName: artist
                        });

                        
                        const bakedImage = await bakeBlurImage(realCoverUrl);

                        coverUrl.value = bakedImage;

                        if (coverCache.size > 50) coverCache.clear();
                        
                        coverCache.set(newTrackInfo, bakedImage);
                    } catch (coverErr) {
                        coverUrl.value = '';
                    }
                }
            }
        } else {
            
            currentTrackInfo.value = '';
            coverUrl.value = '';
        }
    } catch (err) {
        console.error('沉浸模式封面同步失败:', err);
    }
};


watch(themeMode, (newMode) => {
    if (newMode === 'coverglass') {
        syncMusicCover(); 
        if (!coverTimer) {
            
            coverTimer = window.setInterval(syncMusicCover, 2000);
        }
    } else {
        if (coverTimer) {
            clearInterval(coverTimer);
            coverTimer = null;
        }
    }
}, { immediate: true });


const isPlayerDropdownOpen = ref(false);
const handleSelectPlayer = (player: string) => {
    setTargetPlayer(player);
    isPlayerDropdownOpen.value = false;
};


const isStatChartDropdownOpen = ref(false);
const handleSelectStatChart = (type: 'bar' | 'line') => {
    statChartType.value = type;
    isStatChartDropdownOpen.value = false; 
    updateStatsChart(); 
};


const isThemeModeDropdownOpen = ref(false);
const handleSelectThemeMode = (mode: string) => {
    themeMode.value = mode;                 
    isThemeModeDropdownOpen.value = false;  
    handleThemeChange();                    
};

const isLanguageDropdownOpen = ref(false);
const handleSelectLanguage = async (language: AppLanguage) => {
    setLanguage(language);
    isLanguageDropdownOpen.value = false;
    updateStatsChart();
    await emit('control-language', { language });
};


const enableMusicCtrl = ref(localStorage.getItem('nsd_music_ctrl') === 'true');
const enableMsgNotify = ref(localStorage.getItem('nsd_msg_notify') === 'true');
const msgModeEnabled = ref(localStorage.getItem('nsd_msg_mode') === 'true');
const autoHideFullscreen = ref(localStorage.getItem('nsd_autohide_fs') === 'true');


const pinToTaskbar = ref(localStorage.getItem('nsd_pin_taskbar') === 'true');

const togglePinTaskbar = async () => {
    localStorage.setItem('nsd_pin_taskbar', String(pinToTaskbar.value));
    await emit('control-pin-taskbar', { enabled: pinToTaskbar.value });
};


const toggleMsgMode = async () => {
    
    if (msgModeEnabled.value) { enableMsgNotify.value = true; toggleMsgNotify(); }

    localStorage.setItem('nsd_msg_mode', String(msgModeEnabled.value));
    await emit('control-msg-mode', { enabled: msgModeEnabled.value });
};


const toggleMsgNotify = () => {
    localStorage.setItem('nsd_msg_notify', String(enableMsgNotify.value));
};


const toggleDynamicSet = () => {
    isDynamicSet.value = !isDynamicSet.value;
};


const toggleAutoHide = async () => {
    localStorage.setItem('nsd_autohide_fs', String(autoHideFullscreen.value));
    await emit('control-autohide-fs', { enabled: autoHideFullscreen.value });
};


watch(isDynamicSet, async (newVal) => {
    if (!newVal) {
        
        chartInstance?.dispose();
        statsChartInstance?.dispose();

        
        await nextTick();

        
        initChart();

        
        if (rightPanel.value === 'stats') {
            initStatsChart();
        }
    }
});

const rightPanel = ref<'settings' | 'stats'>('stats');
const statChartType = ref<'bar' | 'line'>('bar');
const statsChartRef = ref<HTMLElement | null>(null);
let statsChartInstance: any = null;

const trafficData = ref<Record<string, { up: number; down: number }>>({});
let saveThrottleCounter = 0;


const formatBytesValue = (bytes: number) => {
    if (bytes === 0) return '0';
    const k = 1024;
    const i = Math.floor(Math.log(bytes) / Math.log(k));
    return parseFloat((bytes / Math.pow(k, i)).toFixed(2)).toString();
};

const formatBytesUnit = (bytes: number) => {
    if (bytes === 0) return 'B';
    const k = 1024;
    const sizes = ['B', 'KB', 'MB', 'GB', 'TB'];
    const i = Math.floor(Math.log(bytes) / Math.log(k));
    return sizes[i];
};

const totalUpload = computed(() => Object.values(trafficData.value).reduce((acc, curr) => acc + curr.up, 0));
const totalDownload = computed(() => Object.values(trafficData.value).reduce((acc, curr) => acc + curr.down, 0));
const monthTraffic = computed(() => {
    const currentMonth = getLocalYYYYMMDD(new Date()).slice(0, 7);
    return Object.entries(trafficData.value)
        .filter(([date]) => date.startsWith(currentMonth))
        .reduce((acc, [, data]) => acc + data.up + data.down, 0);
});


const getLocalYYYYMMDD = (date: Date) => {
    const y = date.getFullYear();
    const m = String(date.getMonth() + 1).padStart(2, '0');
    const d = String(date.getDate()).padStart(2, '0');
    return `${y}-${m}-${d}`;
};


const loadTrafficData = () => {
    try {
        const stored = localStorage.getItem('nsd_traffic_stats');
        if (stored) trafficData.value = JSON.parse(stored);
    } catch (e) {
        console.error("加载统计数据失败", e);
    }
};
loadTrafficData();


const toggleRightPanel = async () => {
    rightPanel.value = rightPanel.value === 'settings' ? 'stats' : 'settings';
    localStorage.setItem('nsd_traffic_stats', JSON.stringify(trafficData.value));
    saveThrottleCounter = 0;

    if (rightPanel.value === 'stats') {
        await nextTick();
        initStatsChart();
    } else {
        statsChartInstance?.dispose();
        statsChartInstance = null;
    }

    
    await nextTick();
    chartInstance?.resize();
};

const initStatsChart = () => {
    if (!statsChartRef.value || !echarts) return;
    statsChartInstance = echarts.init(statsChartRef.value);
    updateStatsChart();
};


const updateStatsChart = () => {
    if (!statsChartInstance) return;
    const isDark = document.documentElement.classList.contains('dark-theme');
    const textColor = isDark ? '#94a3b8' : '#64748b';
    const splitLineColor = isDark ? '#383c41' : '#f1f5f9';

    const days: string[] = [];
    const upData: number[] = [];
    const downData: number[] = [];

    for (let i = 6; i >= 0; i--) {
        const d = new Date();
        d.setDate(d.getDate() - i);
        const dateStr = getLocalYYYYMMDD(d);
        days.push(dateStr.slice(5));

        const dayData = trafficData.value[dateStr] || { up: 0, down: 0 };
        upData.push(Number((dayData.up / (1024 * 1024)).toFixed(2)));
        downData.push(Number((dayData.down / (1024 * 1024)).toFixed(2)));
    }

    statsChartInstance.setOption({
        tooltip: { trigger: 'axis', axisPointer: { type: 'shadow' } },
        legend: { data: [t('trafficLegendUpload'), t('trafficLegendDownload')], textStyle: { color: textColor }, top: 0 },
        grid: { top: 30, left: '2%', right: '2%', bottom: '0%', containLabel: true },
        xAxis: {
            type: 'category',
            data: days,
            axisLabel: { color: textColor },
            axisLine: { lineStyle: { color: splitLineColor } }
        },
        yAxis: {
            type: 'value',
            splitLine: { lineStyle: { color: splitLineColor, type: 'dashed' } },
            axisLabel: { color: textColor }
        },
        series: [
            {
                name: t('trafficLegendUpload'),
                type: statChartType.value,
                smooth: true,
                data: upData,
                itemStyle: { color: getChartColors().line },
                barMaxWidth: 15
            },
            {
                name: t('trafficLegendDownload'),
                type: statChartType.value,
                smooth: true,
                data: downData,
                itemStyle: { color: isDark ? '#34d399' : '#10b981' },
                barMaxWidth: 15
            }
        ]
    });
};

const toggleAutoStart = async () => {
    try {
        if (autoStart.value) {
            await enable();
        } else {
            await disable();
        }
    } catch (error) {
        autoStart.value = !autoStart.value;
        showDialog(t('settingFailedTitle'), t('autoStartFailed'));
    }
};

const dialog = ref({
    visible: false,
    title: 'QiuNiu',
    message: '',
    isConfirm: false,
    callback: null as (() => void) | null
});

const showDialog = (title: string, message: string, isConfirm = false, onConfirm: (() => void) | null = null) => {
    dialog.value = { visible: true, title, message, isConfirm, callback: onConfirm };
};

const closeDialog = () => {
    dialog.value.visible = false;
};

const handleDialogConfirm = () => {
    if (dialog.value.callback) dialog.value.callback();
    closeDialog();
};

const parseVersion = (v: string) => {
    
    const match = v.match(/\d+\.\d+\.\d+/);
    if (match) {
        return match[0].split('.').map(Number);
    }
    
    return [0, 0, 0];
};

let lastRx = 0;
let lastTx = 0;
let speedTimer: number;
let systemThemeMedia: MediaQueryList;

const chartRef = ref<HTMLElement | null>(null);
let chartInstance: any = null;
const chartDataQueue: number[] = Array(15).fill(0);

const formatSpeed = (bytes: number) => {
    if (bytes < 1024) return bytes + ' B/s';
    if (bytes < 1024 * 1024) return (bytes / 1024).toFixed(1) + ' KB/s';
    return (bytes / (1024 * 1024)).toFixed(1) + ' MB/s';
};

const getChartColors = () => {
    const isDark = document.documentElement.classList.contains('dark-theme');
    return {
        line: isDark ? '#60a5fa' : '#3b82f6',
        areaStart: isDark ? 'rgba(96, 165, 250, 0.4)' : 'rgba(59, 130, 246, 0.4)',
        areaEnd: isDark ? 'rgba(96, 165, 250, 0.0)' : 'rgba(59, 130, 246, 0.0)'
    };
};

const initChart = () => {
    if (!chartRef.value || !echarts) return;
    chartInstance = echarts.init(chartRef.value);
    updateChartOption();
};


const updateChartOption = () => {
    if (!chartInstance) return;
    const colors = getChartColors();
    chartInstance.setOption({
        grid: { top: 5, bottom: 5, left: 0, right: 0 },
        xAxis: { type: 'category', boundaryGap: false, show: false },
        yAxis: { type: 'value', show: false, min: 0 },
        series: [
            {
                data: chartDataQueue,
                type: 'line',
                smooth: true,
                symbol: 'none',
                lineStyle: { color: colors.line, width: 2 },
                areaStyle: {
                    color: new echarts.graphic.LinearGradient(0, 0, 0, 1, [
                        { offset: 0, color: colors.areaStart },
                        { offset: 1, color: colors.areaEnd }
                    ]),
                },
            },
        ],
    });
};


const fetchSpeedStats = async () => {
    try {
        const [currentRx, currentTx] = await invoke<[number, number]>('get_network_stats');
        if (lastRx !== 0) {
            const rxDiff = currentRx - lastRx;
            const txDiff = currentTx - lastTx;
            downloadSpeed.value = formatSpeed(rxDiff);
            uploadSpeed.value = formatSpeed(txDiff);

            const speedMB = rxDiff / (1024 * 1024);

            
            
            chartDataQueue.push(speedMB);
            if (chartDataQueue.length > 15) chartDataQueue.shift();

            chartInstance?.setOption({ series: [{ data: chartDataQueue }] });

            if (rxDiff > 0 || txDiff > 0) {
                const todayStr = getLocalYYYYMMDD(new Date());
                if (!trafficData.value[todayStr]) {
                    trafficData.value[todayStr] = { up: 0, down: 0 };
                }
                trafficData.value[todayStr].down += rxDiff;
                trafficData.value[todayStr].up += txDiff;

                saveThrottleCounter++;
                if (saveThrottleCounter >= 5) {
                    localStorage.setItem('nsd_traffic_stats', JSON.stringify(trafficData.value));
                    saveThrottleCounter = 0;
                }
            }
        }
        lastRx = currentRx;
        lastTx = currentTx;
    } catch (error) {
        console.error('控制台流量获取失败:', error);
    }
};

const openMywebsite = () => {
    openUrl('https://github.com/ref42');
}


const silentCheckUpdate = async () => {
    try {
        const localVersionStr = await getVersion();
        const response = await fetch('https://api.github.com/repos/ref42/qiuniu/releases/latest', {
            method: 'GET',
            headers: { 'Accept': 'application/vnd.github.v3+json', 'User-Agent': 'Tauri-App-QiuNiu' }
        });
        if (!response.ok) return;

        const data = await response.json();
        const remoteVersionStr = data.tag_name;
        const local = parseVersion(localVersionStr);
        const remote = parseVersion(remoteVersionStr);

        for (let i = 0; i < 3; i++) {
            const rNum = remote[i] || 0;
            const lNum = local[i] || 0;
            if (rNum > lNum) {
                hasNewVersion.value = true; 
                break;
            } else if (rNum < lNum) {
                break;
            }
        }
    } catch (error) {
        
    }
};

const openNSDweb = async () => {
    openUrl('https://github.com/ref42/qiuniu');
}

const openNSDdata = async () => {
    openUrl('https://github.com/ref42/qiuniu');
}

const checkUpdate = async () => {
    if (isChecking.value) return; 
    isChecking.value = true;

    try {
        const localVersionStr = await getVersion();

        
        const controller = new AbortController();
        const timeoutId = setTimeout(() => controller.abort(), 10000);

        const response = await fetch('https://api.github.com/repos/ref42/qiuniu/releases/latest', {
            method: 'GET',
            headers: {
                'Accept': 'application/vnd.github.v3+json',
                'User-Agent': 'Tauri-App-QiuNiu'
            },
            signal: controller.signal
        });

        clearTimeout(timeoutId);

        if (response.status === 404) {
            showDialog(t('checkUpdateTitle'), t('updateNotFound'));
            return;
        }

        if (!response.ok) {
            showDialog(t('checkUpdateTitle'), t('updateCheckFailed'));
            return;
        }

        const data = await response.json();
        const remoteVersionStr = data.tag_name;
        const local = parseVersion(localVersionStr);
        const remote = parseVersion(remoteVersionStr);

        let findNew = false;
        for (let i = 0; i < 3; i++) {
            const rNum = remote[i] || 0;
            const lNum = local[i] || 0;
            if (rNum > lNum) {
                findNew = true;
                break;
            } else if (rNum < lNum) {
                break;
            }
        }

        if (findNew) {
            hasNewVersion.value = true;
            showDialog(
                t('newVersionTitle'),
                t('newVersionMessage', { remote: remoteVersionStr, local: localVersionStr }),
                true,
                () => {
                    openUrl(data.html_url);
                    hasNewVersion.value = false; 
                }
            );
        } else {
            hasNewVersion.value = false;
            showDialog(t('tipTitle'), t('latestVersion'));
        }
    } catch (error: any) {
        console.error('检查更新时出错:', error);
        
        if (error.name === 'AbortError') {
            showDialog(t('networkTimeoutTitle'), t('networkTimeoutMessage'));
        } else {
            showDialog(t('networkErrorTitle'), t('networkErrorMessage'));
        }
    } finally {
        isChecking.value = false; 
    }
};

const applyTheme = () => {
    const root = document.documentElement;
    if (themeMode.value === 'dark' || themeMode.value === 'coverglass') {
        root.classList.add('dark-theme');
    } else if (themeMode.value === 'light') {
        root.classList.remove('dark-theme');
    } else if (themeMode.value === 'system') {
        const media = window.matchMedia('(prefers-color-scheme: dark)');
        if (media.matches) {
            root.classList.add('dark-theme');
        } else {
            root.classList.remove('dark-theme');
        }
    }
    updateChartOption();
};

const handleThemeChange = () => {
    localStorage.setItem('nsd_theme_mode', themeMode.value);
    applyTheme();
};

const handleSystemThemeUpdate = () => {
    if (themeMode.value === 'system') {
        applyTheme();
    }
};

watch(currentLanguage, () => {
    updateStatsChart();
});

watch(opacity, async (newVal) => {
    localStorage.setItem('nsd_island_opacity', newVal.toString());
    await emit('control-island-opacity', { opacity: newVal });
});


watch(enableMusicCtrl, async (newVal) => {
    localStorage.setItem('nsd_music_ctrl', newVal.toString());
    await emit('control-music-ctl', { enabled: newVal });
    console.log('音乐控制器状态切换为:', newVal);
});

onMounted(async () => {
    
    await invoke('set_target_player', { player: targetPlayer.value }).catch(() => { });

    silentCheckUpdate();

    window.addEventListener('contextmenu', (e) => {
        e.preventDefault();
    }, { capture: true });

    applyTheme();
    systemThemeMedia = window.matchMedia('(prefers-color-scheme: dark)');
    systemThemeMedia.addEventListener('change', handleSystemThemeUpdate);

    initChart();
    fetchSpeedStats();
    speedTimer = setInterval(fetchSpeedStats, 1000) as unknown as number;
    window.addEventListener('resize', () => {
        chartInstance?.resize();
        statsChartInstance?.resize();
    });

    try {
        autoStart.value = await isEnabled();
    } catch (e) {
        console.error("获取自启动状态失败:", e);
    }

    try {
        appVersion.value = await getVersion();
    } catch (e) {
        console.error("获取应用版本号失败:", e);
    }

    
    await listen('open-settings-panel', async () => {
        const appWindow = getCurrentWindow();
        await appWindow.show();        
        await appWindow.unminimize();  
        await appWindow.setFocus();    
    });

    await listen<{ visible: boolean }>('island-status-sync', (event) => {
        isWidgetVisible.value = event.payload.visible;
    });

    for (let i = 0; i < 6; i++) {
        try {
            const visible = await invoke<boolean>('is_widget_visible');
            if (visible) {
                isWidgetVisible.value = true;
                return;
            }
        } catch {  }
        await new Promise(r => setTimeout(r, 200));
    }
    isWidgetVisible.value = false;
});

onUnmounted(() => {
    clearInterval(speedTimer);
    chartInstance?.dispose();
    statsChartInstance?.dispose();
    systemThemeMedia?.removeEventListener('change', handleSystemThemeUpdate);
    localStorage.setItem('nsd_traffic_stats', JSON.stringify(trafficData.value));
    if (coverTimer) clearInterval(coverTimer);
});

const toggleWidget = async () => {
    const nextState = !isWidgetVisible.value;
    await emit('control-island-visibility', { show: nextState });
    isWidgetVisible.value = nextState;
};


const minimizeWindow = async () => {
    await getCurrentWindow().minimize();
};
const closeWindow = async () => {
    await getCurrentWindow().hide();
};
</script>

<style scoped>

:global(:root) {
    --bg-body: #f8fafc;
    --text-body: #1e293b;
    --h1-color: #0f172a;
    --subtitle-color: #798089;
    --control-bg: #ffffff;
    --control-border: #e2e8f0;
    --status-badge-inactive: #94a3b8;
    --status-badge-active: #2b2b2b;
    --divider-border: #e2e8f0;
    --card-bg: #ffffff;
    --card-border: #e2e8f0;
    --card-shadow: rgba(0, 0, 0, 0.03);
    --card-shadow-hover: rgba(0, 0, 0, 0.06);
    --card-h3-color: #334155;
    --arrow-up-bg: #eff6ff;
    --arrow-up-color: #3b82f6;
    --arrow-down-bg: #ecfdf5;
    --arrow-down-color: #10b981;
    --speed-label: #64748b;
    --speed-value: #0f172a;
    --chart-border: #f1f5f9;
    --item-title-color: #1e293b;
    --item-title-active-color: #f8fafc8c;
    --tag-dev-bg: #f1f5f9;
    --tag-dev-color: #64748b;
    --item-desc-color: #898f99df;
    --slider-bg: #d7dce2;
    --slider-checked-bg: #b9b9b9;
    --slider-disabled-bg: #e2e8f0;
    --range-bg: #e2e8f0;
    --range-thumb-bg: #ffffff;
    --range-thumb-border: #2b2b2b;
    --range-thumb-shadow: rgba(0, 0, 0, 0.3);
    --footer-text: #2b2b2b89;
    --overlay-bg: rgba(15, 23, 42, 0.3);
    --modal-bg: #ffffff;
    --modal-border: #e2e8f0;
    --modal-h4: #0f172a;
    --modal-p: #64748b;
    --btn-sec-bg: #ebebeb;
    --btn-sec-list-bg: #ebebeb;
    --btn-sec-color: #64748b;
    --btn-sec-border: #e2e8f0;
    --btn-sec-hover-bg: #e2e8f0;
    --btn-sec-hover-color: #ffffff;
    --btn-pri-bg: #2b2b2b;
    --btn-pri-color: #ffffff;
    --btn-pri-border: #2b2b2b;
    --btn-pri-hover-bg: #1a1a1a;
    --btn-pri-shadow-hover: rgba(0, 0, 0, 0.15);
    --select-bg: #ffffff;
    --select-border: #e2e8f0;
    --select-text: #1e293b;
    --data-tag-bg: #ececec;
    --data-tag-color: #2b2b2b;
}


:global(.dark-theme) {
    --bg-body: #1e1f1f;
    --text-body: #cbd5e1;
    --h1-color: #f8fafc;
    --subtitle-color: #a5aeba;
    --control-bg: #292b2ea9;
    --control-border: #383c41;
    --status-badge-inactive: #64748b;
    --status-badge-active: #f8fafc;
    --divider-border: #334155;
    --card-bg: #292b2e95;
    --card-border: #383c41;
    --card-shadow: rgba(0, 0, 0, 0.2);
    --card-shadow-hover: rgba(0, 0, 0, 0.3);
    --card-h3-color: #e2e8f0;
    --arrow-up-bg: rgba(59, 130, 246, 0.15);
    --arrow-up-color: #60a5fa;
    --arrow-down-bg: rgba(16, 185, 129, 0.15);
    --arrow-down-color: #34d399;
    --speed-label: #94a3b8;
    --speed-value: #f8fafc;
    --chart-border: #474c53;
    --item-title-color: #f8fafc;
    --item-title-active-color: #f8fafc8c;
    --tag-dev-bg: #334155;
    --tag-dev-color: #94a3b8;
    --item-desc-color: #898f99df;
    --slider-bg: #3e4247;
    --slider-checked-bg: #5d646d;
    --slider-disabled-bg: #334155;
    --range-bg: #42474e;
    --range-thumb-bg: #1e293b;
    --range-thumb-border: #60a5fa;
    --range-thumb-shadow: rgba(0, 0, 0, 0.5);
    --footer-text: #8b8f96aa;
    --overlay-bg: rgba(0, 0, 0, 0.6);
    --modal-bg: #292b2e;
    --modal-border: #383c41;
    --modal-h4: #f8fafc;
    --modal-p: #94a3b8;
    --btn-sec-bg: #1a1a1a;
    --btn-sec-list-bg: #202020;
    --btn-sec-color: #cbd5e1;
    --btn-sec-border: #475569;
    --btn-sec-hover-bg: #475569;
    --btn-sec-hover-color: #f8fafc;
    --btn-pri-bg: #1a1a1a;
    --btn-pri-color: #ffffff;
    --btn-pri-border: #2b2b2b;
    --btn-pri-hover-bg: #161616;
    --btn-pri-shadow-hover: rgba(0, 0, 0, 0.15);
    --select-bg: #292b2e;
    --select-border: #383c41;
    --select-text: #f8fafc;
    --data-tag-bg: #202020;
    --data-tag-color: #f8fafc;
}


:global(html) {
    color: var(--text-body);
    transition: background-color 0.3s ease, color 0.3s ease;
}

:global(body) {
    background-color: transparent !important;
    color: inherit;
    font-family: -apple-system, BlinkMacSystemFont, 'SF Pro Text', 'Segoe UI', Roboto, sans-serif;
    user-select: none;
    -webkit-font-smoothing: antialiased;
}

.panel-container {
    background-color: var(--bg-body);
    padding: 36px 32px 16px 32px;
    max-width: 900px;
    margin: 0 auto;
    display: flex;
    flex-direction: column;
    min-height: calc(100vh - 52px);
    position: relative;
}

.panel-header {
    display: flex;
    justify-content: space-between;
    align-items: center;
    margin-bottom: 16px;
}

.brand {
    display: flex;
    align-items: center;
    gap: 16px;
}

.logo-icon {
    width: 48px;
    height: 48px;
    border-radius: 12px;
    box-shadow: 0 4px 16px rgba(0, 0, 0, 0.25);
}

.brand h1 {
    font-size: 20px;
    margin: 0;
    font-weight: 700;
    letter-spacing: 0.2px;
    color: var(--h1-color);
}

.subtitle {
    font-size: 13px;
    color: var(--subtitle-color);
    margin: 4px 0 0 0;
}

.header-controls {
    display: flex;
    align-items: center;
    gap: 16px;
    background: var(--control-bg);
    padding: 8px 16px;
    border-radius: 24px;
    box-shadow: 0 2px 8px rgba(0, 0, 0, 0.04);
    border: 1px solid var(--control-border);
}

.status-badge {
    font-size: 13px;
    font-weight: 600;
    color: var(--status-badge-inactive);
    transition: all 0.3s;
}

.status-badge.is-active {
    color: var(--status-badge-active);
}

.divider {
    border: none;
    border-top: 1px solid var(--divider-border);
    margin-bottom: 16px;
}

.main-content {
    display: grid;
    grid-template-columns: 1fr 1.3fr;
    gap: 16px;
    flex-grow: 1;
    transition: all 0.3s ease;
}


.main-content.game-mode-layout {
    grid-template-columns: 1fr;
}

.card {
    background: var(--card-bg);
    border: 1px solid var(--card-border);
    border-radius: 20px;
    padding: 16px 20px;
    display: flex;
    flex-direction: column;
    box-shadow: 0 4px 20px -2px var(--card-shadow);
    transition: transform 0.2s, box-shadow 0.2s;
}

.card:hover {
    box-shadow: 0 8px 24px -4px var(--card-shadow-hover);
}

.card h3 {
    font-size: 15px;
    color: var(--card-h3-color);
    margin: 0 0 20px 0;
    font-weight: 600;
}

.speed-monitor {
    display: flex;
    flex-direction: column;
    gap: 12px;
    margin-bottom: 16px;
}

.speed-item {
    display: flex;
    align-items: center;
    gap: 16px;
}

.arrow {
    width: 36px;
    height: 36px;
    border-radius: 10px;
    display: flex;
    align-items: center;
    justify-content: center;
    font-weight: 800;
    font-size: 16px;
}

.arrow svg {
    width: 20px;
    height: 20px;
}

.arrow.up {
    background: var(--arrow-up-bg);
    color: var(--arrow-up-color);
}

.arrow.down {
    background: var(--arrow-down-bg);
    color: var(--arrow-down-color);
}

.speed-info {
    display: flex;
    flex-direction: column;
    gap: 4px;
}

.speed-info .label {
    font-size: 12px;
    color: var(--speed-label);
    font-weight: 500;
}

.speed-info .value {
    font-size: 22px;
    font-weight: 700;
    font-family: ui-monospace, SFMono-Regular, Menlo, Monaco, Consolas, monospace;
    color: var(--speed-value);
    letter-spacing: -0.5px;
}

.mini-chart {
    width: 100%;
    height: 80px;
    margin-top: auto;
    padding-top: 16px;
    border-top: 1px solid var(--chart-border);
}

.setting-item {
    display: flex;
    justify-content: space-between;
    align-items: center;
    padding: 12px 0;
    border-bottom: 1px solid var(--chart-border);
}

.setting-item:last-child {
    border-bottom: none;
    padding-bottom: 0;
}

.slider-item {
    flex-direction: column;
    align-items: flex-start;
    gap: 10px;
}

.flex-row-item {
    flex-direction: row;
    align-items: center;
}

.item-meta {
    display: flex;
    flex-direction: column;
    gap: 4px;
}

.item-title {
    font-size: 14px;
    font-weight: 600;
    color: var(--item-title-color);
    display: flex;
    align-items: center;
    gap: 8px;
}

.item-title-sec {
    height: 22px;
    font-size: 14px;
    font-weight: 600;
    color: var(--item-title-color);
    opacity: 0.8;
    display: flex;
    align-items: center;
}

.item-desc {
    font-size: 13px;
    color: var(--item-desc-color);
}

.switch {
    position: relative;
    display: inline-block;
    width: 48px;
    height: 28px;
}

.switch input {
    opacity: 0;
    width: 0;
    height: 0;
}

.slider {
    position: absolute;
    cursor: pointer;
    top: 0;
    left: 0;
    right: 0;
    bottom: 0;
    background-color: var(--slider-bg);
    transition: 0.4s cubic-bezier(0.4, 0.0, 0.2, 1);
    border-radius: 28px;
}

.slider:before {
    position: absolute;
    content: "";
    height: 22px;
    width: 22px;
    left: 3px;
    bottom: 3px;
    background-color: white;
    box-shadow: 0 2px 4px rgba(0, 0, 0, 0.2);
    transition: 0.4s cubic-bezier(0.4, 0.0, 0.2, 1);
    border-radius: 50%;
}

input:checked+.slider {
    background-color: var(--slider-checked-bg);
}

input:checked+.slider:before {
    transform: translateX(20px);
}

.range-input {
    width: 100%;
    -webkit-appearance: none;
    appearance: none;
    background: var(--range-bg);
    height: 6px;
    border-radius: 3px;
    outline: none;
}

.range-input::-webkit-slider-thumb {
    -webkit-appearance: none;
    width: 20px;
    height: 20px;
    border-radius: 50%;
    background: var(--range-thumb-bg);
    border: 2px solid var(--range-thumb-border);
    cursor: pointer;
    box-shadow: 0 2px 6px var(--range-thumb-shadow);
    transition: transform 0.1s;
}

.range-input::-webkit-slider-thumb:hover {
    transform: scale(1.1);
}

.panel-footer {
    margin-top: 25px;
    display: flex;
    flex-direction: row;
    justify-content: space-between;
    font-size: 12px;
    color: var(--footer-text);
    font-weight: 500;
}

.panel-footer span {
    display: flex;
}

.ft_left {
    display: flex;
    flex-direction: row;
    justify-content: left;
    align-items: center;
    gap: 10px;
}

.openmywebsite {
    background: none;
    border: none;
    cursor: pointer;
    outline: none;
    font-size: 12px;
    color: var(--footer-text);
    font-weight: bold;
}

.openmywebsite:hover {
    text-decoration: underline;
}

.ft_right {
    display: flex;
    flex-direction: row;
    justify-content: right;
    align-items: center;
    gap: 13px;
}

.action-link {
    color: var(--footer-text);
    cursor: pointer;
    transition: color 0.2s;
}

.action-link:hover {
    color: var(--footer-text);
    text-decoration: underline;
}

.update-dot {
    position: absolute;
    top: 2px;
    right: -8px;
    width: 5px;
    height: 5px;
    background-color: #ff3b30;
    border-radius: 50%;
    box-shadow: 0 0 4px rgba(255, 59, 48, 0.4);
}

.modal-overlay {
    position: fixed;
    top: 0;
    left: 0;
    width: 100vw;
    height: 100vh;
    background: var(--overlay-bg);
    display: flex;
    align-items: center;
    justify-content: center;
    z-index: 9999;
}

.modal-card {
    background: var(--modal-bg);
    border: 1px solid var(--modal-border);
    border-radius: 20px;
    width: 360px;
    padding: 24px;
    box-shadow: 0 20px 25px -5px rgba(0, 0, 0, 0.1), 0 10px 10px -5px rgba(0, 0, 0, 0.04);
}

.modal-header h4 {
    margin: 0 0 12px 0;
    font-size: 16px;
    font-weight: 700;
    color: var(--modal-h4);
}

.modal-body p {
    margin: 0 0 24px 0;
    font-size: 14px;
    color: var(--modal-p);
    line-height: 1.5;
}

.modal-footer {
    display: flex;
    justify-content: flex-end;
    gap: 12px;
}

.btn {
    padding: 8px 18px;
    font-size: 13px;
    font-weight: 600;
    border-radius: 10px;
    cursor: pointer;
    transition: all 0.2s cubic-bezier(0.4, 0, 0.2, 1);
    outline: none;
}

.btn-secondary {
    background: var(--btn-sec-bg);
    color: var(--btn-sec-color);
    border: 1px solid var(--btn-sec-border);
}

.btn-secondary:hover {
    background: var(--btn-sec-hover-bg);
    color: var(--btn-sec-hover-color);
}

.btn-primary {
    background: var(--btn-pri-bg);
    color: var(--btn-pri-color);
    border: 1px solid var(--btn-pri-border);
}

.btn-primary:hover {
    background: var(--btn-pri-hover-bg);
    box-shadow: 0 4px 12px var(--btn-pri-shadow-hover);
}

.fade-enter-active,
.fade-leave-active {
    transition: opacity 0.25s ease, transform 0.25s ease;
}

.fade-enter-from,
.fade-leave-to {
    opacity: 0;
}

.fade-enter-from .modal-card {
    transform: scale(0.95);
}

.fade-leave-to .modal-card {
    transform: scale(0.95);
}

.theme-select {
    padding: 6px 12px;
    font-size: 13px;
    font-weight: 600;
    border-radius: 8px;
    background-color: var(--select-bg);
    border: 1px solid var(--select-border);
    color: var(--select-text);
    outline: none;
    cursor: pointer;
    transition: all 0.2s ease;
}

.theme-select:hover {
    border-color: var(--slider-checked-bg);
}





.dynamicset-btn {
    background: transparent;
    border: 1px solid var(--control-border);
    color: var(--text-body);
    padding: 6px 14px;
    font-size: 12px;
    font-weight: 700;
    border-radius: 16px;
    cursor: pointer;
    transition: all 0.2s ease;
}

.dynamicset-btn:hover {
    background: var(--btn-sec-bg);
    border-color: var(--slider-checked-bg);
}

.dynamicset-btn.is-active {
    background: var(--btn-pri-bg);
    color: var(--btn-pri-color);
    border-color: var(--btn-pri-border);
    box-shadow: 0 2px 6px rgba(0, 0, 0, 0.15);
}

.control-separator {
    width: 1px;
    height: 16px;
    background: var(--control-border);
}










.main-content.dynamicset-layout {
    grid-template-columns: 1fr !important;
}


.dynamicset-grid {
    display: grid;
    align-content: center !important;
    grid-template-columns: 1fr 1fr 1fr;
    row-gap: 5px;
    background: var(--card-bg);
    border: 1px solid var(--card-border);
    border-radius: 20px;
    box-shadow: 0 4px 20px -2px var(--card-shadow);
    max-height: calc(100vh - 180px);
    overflow-y: auto;
    align-content: start;
}

.dynamicset-grid::-webkit-scrollbar {
    width: 5px;
}

.dynamicset-grid::-webkit-scrollbar-track {
    background: transparent;
    margin: 12px 0;
}

.dynamicset-grid::-webkit-scrollbar-thumb {
    background-color: var(--slider-bg);
    border-radius: 10px;
}

.dynamicset-grid::-webkit-scrollbar-thumb:hover {
    background-color: var(--slider-checked-bg);
}


.set-item {
    background: transparent;
    border: none;
    margin: 0;
    border-radius: 0;
    display: flex;
    align-items: center;
    justify-content: space-between;
    height: 70px;
    padding: 0 16px;
    box-sizing: border-box;
}

.disabled-set-item {
    opacity: 0.6;
}

.set-item-meta {
    display: flex;
    flex-direction: column;
    gap: 4px;
}

.set-item-title {
    font-size: 14px;
    font-weight: 600;
    display: flex;
    align-items: center;
    max-height: 24px;
    color: var(--item-title-color);
}


.tooltip-wrapper {
    position: relative;
    display: inline-flex;
    align-items: center;
    cursor: help;
    
}


.tooltip-wrapper::after {
    content: attr(data-tooltip);
    position: absolute;
    bottom: calc(100% + 2px);
    
    left: 50%;
    transform: translateX(-50%) translateY(4px);
    

    
    background: var(--modal-bg);
    color: var(--text-body);
    border: 1px solid var(--card-border);
    box-shadow: 0 4px 12px var(--card-shadow-hover);

    padding: 8px 12px;
    border-radius: 8px;
    font-size: 12px;
    font-weight: 500;
    line-height: 1.4;
    white-space: nowrap;
    

    
    opacity: 0;
    visibility: hidden;
    pointer-events: none;
    transition: all 0.2s cubic-bezier(0.4, 0, 0.2, 1);
    z-index: 999;
}


.tooltip-wrapper:hover::after {
    opacity: 1;
    visibility: visible;
    transform: translateX(-50%) translateY(0);
}

.tooltip-wrapper:hover::before {
    opacity: 1;
    transform: translateX(-50%) scale(1);
}

.set-item-desc {
    font-size: 12px;
    color: var(--item-desc-color);
}







.card-header-row {
    display: flex;
    justify-content: space-between;
    align-items: center;
    margin-bottom: 12px;
}

.card-header-row h3 {
    margin-bottom: 0;
}

.stats-toggle-btn {
    background: transparent;
    color: var(--item-title-color);
    border: 1px solid var(--chart-border);
    padding: 4px 10px;
    font-size: 12px;
    font-weight: 600;
    border-radius: 8px;
    cursor: pointer;
    transition: all 0.2s ease;
}

.stats-toggle-btn:hover {
    background: var(--btn-sec-bg);
}

.stats-card {
    display: flex;
    flex-direction: column;
}

.stats-overview {
    display: flex;
    gap: 8px;
    margin-bottom: 8px;
}

.stat-box {
    flex: 1;
    background: var(--control-bg);
    border: 1px solid var(--card-border);
    border-radius: 12px;
    padding: 12px;
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: start;
    gap: 4px;
    height: 65px;
    box-sizing: border-box;
    position: relative;
}

.stat-label {
    font-size: 12px;
    color: var(--item-desc-color);
    font-weight: 500;
    flex-shrink: 0;
    transform: translateY(-4px);
}

.stat-val {
    font-size: 16px;
    font-weight: 700;
    font-family: ui-monospace, SFMono-Regular, Menlo, Monaco, Consolas, monospace;
    color: var(--speed-value);
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
    max-width: 100%;
    flex-shrink: 0;
    transform: translateY(-2px);
}

.stats-chart-container {
    width: 100%;
    flex-grow: 1;
    min-height: 110px;
    border-top: 1px solid var(--chart-border);
    padding-top: 10px;
}





.combo-title-row {
    display: flex;
    align-items: center;
    gap: 8px;
}


.title-separator {
    color: var(--control-border);
    font-size: 14px;
    opacity: 0.8;
}


.mini-switch {
    transform: scale(0.65);
    transform-origin: left center;
    margin: 0;
}

input:disabled+.slider {
    cursor: not-allowed;
    opacity: 0.5;
}






.set-item.is-dropdown-open {
    z-index: 10;
}

.set-item-meta {
    display: flex;
    flex-direction: column;
    gap: 4px;
    flex-shrink: 0;
    
    min-width: 0;
}

.custom-dropdown {
    position: relative;
    outline: none;
}


.dropdown-trigger {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 6px;
    padding: 6px 10px;
    background: var(--select-bg);
    border: 1px solid var(--select-border);
    border-radius: 8px;
    cursor: pointer;
    width: 105px;
    
    box-sizing: border-box;
    transition: all 0.2s ease;
}

.dropdown-trigger:hover {
    border-color: var(--slider-checked-bg);
}

.current-item {
    display: flex;
    align-items: center;
    gap: 6px;
    font-size: 12px;
    font-weight: 600;
    color: var(--select-text);
    overflow: hidden;
    white-space: nowrap;
    text-overflow: ellipsis;
    
}

.current-item .platform-icon {
    width: 14px;
    height: 14px;
    object-fit: contain;
    border-radius: 3px;
    transform: translateY(1px);
}

.platform-glyph {
    display: inline-grid;
    place-items: center;
    background: var(--btn-sec-list-bg);
    color: var(--text-body);
    font-size: 9px;
    font-weight: 800;
    line-height: 1;
}

.arrow-icon {
    width: 12px;
    height: 12px;
    color: var(--item-desc-color);
    transition: transform 0.2s cubic-bezier(0.4, 0, 0.2, 1);
    flex-shrink: 0;
    transform: translateY(1px);
}

.arrow-icon.is-open {
    transform: rotate(180deg);
}


.dropdown-menu {
    position: absolute;
    top: calc(100% + 6px);
    right: 0;
    width: 116px;
    background: var(--modal-bg);
    border: 1px solid var(--modal-border);
    border-radius: 10px;
    padding: 4px;
    box-shadow: 0 10px 25px var(--card-shadow-hover);
    z-index: 100;
    display: flex;
    flex-direction: column;
    gap: 2px;
    max-height: 110px;
    overflow-y: auto;
}


.dropdown-menu::-webkit-scrollbar {
    width: 4px;
}

.dropdown-menu::-webkit-scrollbar-track {
    background: transparent;
    margin: 4px 0;
    
}

.dropdown-menu::-webkit-scrollbar-thumb {
    background-color: var(--slider-bg);
    border-radius: 4px;
}

.dropdown-menu::-webkit-scrollbar-thumb:hover {
    background-color: var(--slider-checked-bg);
}

.dropdown-item {
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 6px 8px;
    font-size: 12px;
    font-weight: 600;
    color: var(--text-body);
    border-radius: 6px;
    cursor: pointer;
    transition: all 0.2s ease;
}

.dropdown-item .platform-icon {
    width: 14px;
    height: 14px;
    object-fit: contain;
    border-radius: 3px;
    opacity: 0.8;
    transform: translateY(1px);
}

.dropdown-item:hover {
    background: var(--btn-sec-list-bg);
}

.dropdown-item.is-active {
    background: var(--btn-sec-list-bg);
}

.dropdown-item.is-active .platform-icon {
    opacity: 1;
}


.dropdown-enter-active,
.dropdown-leave-active {
    transition: all 0.15s cubic-bezier(0.4, 0, 0.2, 1);
    transform-origin: top right;
}

.dropdown-enter-from,
.dropdown-leave-to {
    opacity: 0;
    transform: scaleY(0.95) translateY(-4px);
}

.bottom-grid-card {
    grid-column: 1 / -1;
    max-height: none;
    overflow: visible;
}


.blank-dynamic-page {
    grid-column: 1 / -1;
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    min-height: 300px;
    color: var(--item-desc-color);
}


.custom-titlebar {
    position: absolute;
    top: 0;
    left: 0;
    right: 0;
    height: 32px;
    display: flex;
    justify-content: flex-end;
    align-items: center;
    z-index: 9999;
    border-top-left-radius: inherit;
    border-top-right-radius: inherit;
}


.titlebar-drag-area {
    flex-grow: 1;
    height: 100%;
    -webkit-app-region: drag;
}

.titlebar-controls {
    display: flex;
    height: 100%;
    -webkit-app-region: no-drag;
}

.titlebar-btn {
    background: transparent;
    border: none;
    color: var(--text-body);
    width: 45px;
    height: 100%;
    display: flex;
    align-items: center;
    justify-content: center;
    cursor: pointer;
    transition: background-color 0.2s ease, color 0.2s ease;
}

.titlebar-btn svg {
    width: 11px;
    height: 11px;
    opacity: 0.8;
}

.titlebar-btn:hover {
    background-color: var(--btn-sec-bg);
}

.titlebar-btn:hover svg {
    opacity: 1;
}

.close-btn:hover {
    background-color: #ff4757 !important;
    color: #ffffff !important;
}


.color-preview-icon {
    width: 14px;
    height: 14px;
    border-radius: 3px;
    flex-shrink: 0;
    box-sizing: border-box;
    transition: all 0.2s ease;
    transform: translateY(1px);
}


.color-preview-icon.theme-black {
    background-color: #1a1a1a;
}


.color-preview-icon.theme-white {
    background-color: #f5f5f5;
}

.dropdown-item .color-preview-icon {
    opacity: 0.8;
}

.dropdown-item:hover .color-preview-icon,
.dropdown-item.is-active .color-preview-icon {
    opacity: 1;
}


.coverglass-bg-container {
    position: fixed;
    top: 0;
    left: 0;
    width: 100%;
    height: 100%;
    z-index: 0;
    pointer-events: none;
    
    overflow: hidden;
}


.coverglass-bg-image {
    position: fixed;
    top: -5%;
    left: -5%;
    width: 110%;
    height: 110%;
    background-size: cover;
    background-position: center;
    opacity: 0.3;
    transition: background-image 0.8s ease;
    transform: translateZ(0);
}


.coverglass-blur-layer {
    display: none;
}


.coverglass-noise-layer {
    position: fixed;
    top: 0;
    left: 0;
    width: 100%;
    height: 100%;
    opacity: 0.08;
    background-image: url("data:image/svg+xml,%3Csvg xmlns='http://www.w3.org/2000/svg' width='256' height='256'%3E%3Cfilter id='noise'%3E%3CfeTurbulence type='fractalNoise' baseFrequency='2.5' numOctaves='3' stitchTiles='stitch'/%3E%3C/filter%3E%3Crect width='100%25' height='100%25' filter='url(%23noise)'/%3E%3C/svg%3E");
    background-repeat: repeat;
    background-size: 150px 150px;
}

.panel-header,
.divider,
.main-content,
.panel-footer {
    position: relative;
    z-index: 1;
}
</style>
